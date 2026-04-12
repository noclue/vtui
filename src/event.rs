use crate::operation_types::OperationId;
use crate::prop_browser::PropertyHistoryRecord;
use crate::resource_browser::EventBrowserPayload;
use crate::resource_browser::HistoryRecord as ResourceHistoryRecord;
use crate::resource_type::ResourceType;
use crate::vm_power_actions::VmActionContext;
use crate::vm_summary::VmSummary;
use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use log::{debug, trace};
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::sync::{mpsc, watch};
use vim_rs::core::pc_cache::Monitor;
use vim_rs::types::structs::{ManagedObjectReference, PropertyFilterUpdate};

/// Long-poll timeout for PropertyCollector (`WaitForUpdatesEx`), in **seconds** (see `vim_rs::Monitor::wait_updates`).
const PROPERTY_COLLECTOR_WAIT_TIMEOUT_S: i32 = 60;

/// Representation of all possible events.
#[derive(Debug)]
pub enum Event {
    /// Crossterm events.
    ///
    /// These events are emitted by the terminal.
    Crossterm(CrosstermEvent),
    /// Application events.
    ///
    /// Events that are specific to the application.
    App(Box<AppEvent>),
}

/// Application events.
#[derive(Debug)]
pub enum AppEvent {
    /// Quit the application.
    Quit,
    /// Property collector events.
    ///
    /// These events are emitted by the property collector waiting for updates.
    PropertyCollector(Vec<PropertyFilterUpdate>),
    /// Error Message
    ErrorMessage(String),

    /// Search events.
    OpenSearch,
    SearchCompleted(String),

    /// Resource selection events.
    ResourceSelected(ResourceType),
    OpenResourceSelection,

    /// Load object properties.
    LoadProperties(ManagedObjectReference),

    /// Open a static JSON tree for an event (data object, not a managed object).
    LoadEventProperties(Box<EventBrowserPayload>),

    /// Open VM power-actions flow for the given VM (prefetch path + disabled methods in `App`).
    OpenVmActions(ManagedObjectReference),

    /// Open VM summary popup (async fetch via ops).
    OpenVmSummary(ManagedObjectReference),

    ResourceManagerHistory(ResourceHistoryRecord),
    PropertyManagerHistory(PropertyHistoryRecord),

    /// Background perf worker completed a poll cycle for `generation`.
    PerfResult {
        generation: u64,
    },

    // --- Async ops facility (see `crate::ops`) ---
    /// VM action prefetch finished successfully (`request_id` matches UI submission).
    VmActionPrefetchSucceeded {
        request_id: OperationId,
        context: VmActionContext,
    },
    /// VM action prefetch failed (`request_id` matches UI submission).
    VmActionPrefetchFailed {
        request_id: OperationId,
        error: String,
    },
    /// VM summary fetch succeeded (`request_id` matches UI submission).
    VmSummarySucceeded {
        request_id: OperationId,
        summary: VmSummary,
    },
    /// VM summary fetch failed (`request_id` matches UI submission).
    VmSummaryFailed {
        request_id: OperationId,
        error: String,
    },
    /// A queued inventory operation completed successfully.
    OperationSucceeded {
        op_id: OperationId,
        message: String,
    },
    /// A queued inventory operation failed.
    OperationFailed {
        op_id: OperationId,
        error: String,
    },
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,

    event_dispatch: Option<tokio::task::JoinHandle<Result<()>>>,
    /// When `true`, the PropertyCollector task arms `Monitor::wait_updates` sequentially (never concurrently with crossterm).
    pc_demand_tx: watch::Sender<bool>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns tasks for terminal input and
    /// PropertyCollector waits (demand-driven; no `select!` with crossterm).
    pub fn new(monitor: Monitor) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let internal_sender = sender.clone();
        let (pc_demand_tx, pc_demand_rx) = watch::channel(false);
        let event_dispatch = tokio::spawn(async move {
            let terminal = run_terminal_loop(internal_sender.clone());
            let pc = run_property_collector_loop(internal_sender, monitor, pc_demand_rx);
            tokio::try_join!(terminal, pc)?;
            Ok(())
        });
        Self {
            sender,
            receiver,
            event_dispatch: Some(event_dispatch),
            pc_demand_tx,
        }
    }

    /// Clone the channel used to enqueue [`Event`]s (e.g. for background workers).
    pub fn clone_event_sender(&self) -> mpsc::UnboundedSender<Event> {
        self.sender.clone()
    }

    /// Set whether PropertyCollector long-polls should run. When `false`, the current wait is
    /// allowed to finish; the next wait is not started until demand is `true` again.
    pub fn set_property_collector_demand(&self, wanted: bool) {
        let _ = self.pc_demand_tx.send(wanted);
        trace!(
            target: "pc_wait",
            "property collector demand set to {}",
            wanted
        );
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub async fn next(&mut self) -> Result<Event> {
        self.receiver
            .recv()
            .await
            .context("Failed to receive event")
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(Box::new(app_event)));
    }

    /// Shuts down the event handler. Safely closes the receiver and waits for the event thread to
    /// finish dropping objects allocated on the server.
    pub async fn shutdown(&mut self) -> Result<()> {
        self.receiver.close();
        if let Some(event_dispatch) = self.event_dispatch.take() {
            event_dispatch.await??;
        }
        Ok(())
    }
}

/// Crossterm only — does not share a `select!` with PropertyCollector waits.
async fn run_terminal_loop(sender: mpsc::UnboundedSender<Event>) -> Result<()> {
    let mut reader = crossterm::event::EventStream::new();
    loop {
        tokio::select! {
            _ = sender.closed() => {
                break;
            }
            Some(Ok(evt)) = reader.next().fuse() => {
                let _ = sender.send(Event::Crossterm(evt));
            }
        }
    }
    Ok(())
}

/// Sequential PropertyCollector waits only; respects [`PROPERTY_COLLECTOR_WAIT_TIMEOUT_S`].
async fn run_property_collector_loop(
    sender: mpsc::UnboundedSender<Event>,
    mut monitor: Monitor,
    mut pc_demand_rx: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        // Wait until demand is true or sender shut down
        loop {
            tokio::select! {
                _ = sender.closed() => {
                    return Ok(());
                }
                r = pc_demand_rx.changed() => {
                    if r.is_err() {
                        return Ok(());
                    }
                    if *pc_demand_rx.borrow() {
                        debug!(target: "pc_wait", "property collector wait loop: demand on, arming wait");
                        break;
                    }
                }
            }
        }

        // Demand is true: one wait at a time; re-check demand after each completion
        loop {
            if !*pc_demand_rx.borrow() {
                debug!(
                    target: "pc_wait",
                    "property collector wait loop: demand off, pausing after current cycle"
                );
                break;
            }

            tokio::select! {
                _ = sender.closed() => {
                    return Ok(());
                }
                updates_result = monitor.wait_updates(PROPERTY_COLLECTOR_WAIT_TIMEOUT_S) => {
                    debug!(target: "pc_wait", "property collector wait completed");
                    match updates_result {
                        Ok(None) => continue,
                        Err(e) => {
                            let _ = sender.send(Event::App(Box::new(AppEvent::ErrorMessage(
                                format!("Error waiting for updates: {}", e),
                            ))));
                            continue;
                        }
                        Ok(Some(updates)) => {
                            let _ = sender.send(Event::App(Box::new(AppEvent::PropertyCollector(
                                updates,
                            ))));
                            continue;
                        }
                    }
                }
            }
        }
    }
}
