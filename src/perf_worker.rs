//! Background performance polling — keeps `PerformanceManager` work off the UI task.

use crate::event::{AppEvent, Event};
use crate::resource_browser::perf::{PerfPollerState, PerfSnapshotShare, new_perf_snapshot_share};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::MissedTickBehavior;
use vim_rs::core::client::Client;
use vim_rs::types::structs::ManagedObjectReference;

const POLL_INTERVAL: Duration = Duration::from_secs(20);
const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

/// Sent from UI to perf worker via `tokio::sync::watch`.
#[derive(Clone)]
pub struct PerfRequest {
    pub generation: u64,
    pub entities: Vec<ManagedObjectReference>,
    pub snapshot: PerfSnapshotShare,
}

impl PerfRequest {
    pub fn initial() -> Self {
        Self {
            generation: 0,
            entities: vec![],
            snapshot: new_perf_snapshot_share(),
        }
    }
}

pub async fn run_perf_worker(
    client: Arc<Client>,
    mut watch_rx: watch::Receiver<PerfRequest>,
    event_tx: mpsc::UnboundedSender<Event>,
) {
    let mut poller: Option<PerfPollerState> = None;
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Steady-state: poll with current visible set.
            }
            res = watch_rx.changed() => {
                if res.is_err() {
                    return;
                }
                tokio::time::sleep(DEBOUNCE_DELAY).await;
                interval.reset();
            }
        }

        let req = watch_rx.borrow_and_update().clone();
        if req.entities.is_empty() {
            continue;
        }

        if poller.is_none() {
            match PerfPollerState::new(client.clone()) {
                Ok(p) => poller = Some(p),
                Err(e) => {
                    log::warn!("perf worker: PerformanceManager init failed: {e:#}");
                    continue;
                }
            }
        }
        let poller_ref: &mut PerfPollerState = match poller.as_mut() {
            Some(p) => p,
            None => continue,
        };

        match poller_ref
            .poll_entities(&req.entities, &req.snapshot)
            .await
        {
            Ok(()) => {
                let _ = event_tx.send(Event::App(Box::new(AppEvent::PerfResult {
                    generation: req.generation,
                })));
            }
            Err(e) => {
                log::warn!("perf worker: {e:#}");
            }
        }
    }
}
