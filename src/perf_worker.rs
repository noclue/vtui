//! Background performance polling — keeps `PerformanceManager` work off the UI task.

use crate::event::{AppEvent, Event};
use crate::perf_overview::{PerfOverviewShare, record_overview_failure};
use crate::resource_browser::perf::{PerfPollerState, PerfSnapshotShare, new_perf_snapshot_share};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::MissedTickBehavior;
use vim_rs::core::client::VimClientHandle;
use vim_rs::types::structs::ManagedObjectReference;

const POLL_INTERVAL: Duration = Duration::from_secs(20);
const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

/// Sent from UI to perf worker via `tokio::sync::watch`.
#[derive(Clone)]
pub enum PerfRequest {
    Table {
        entities: Vec<ManagedObjectReference>,
        generation: u64,
        snapshot: PerfSnapshotShare,
    },
    Overview {
        entity: ManagedObjectReference,
        generation: u64,
        store: PerfOverviewShare,
    },
}

impl PerfRequest {
    pub fn initial() -> Self {
        Self::Table {
            generation: 0,
            entities: vec![],
            snapshot: new_perf_snapshot_share(),
        }
    }

    /// Empty entity set pauses table `QueryPerf`. `generation` discards stale worker results.
    pub fn paused(generation: u64) -> Self {
        Self::Table {
            generation,
            entities: vec![],
            snapshot: new_perf_snapshot_share(),
        }
    }

    pub fn generation(&self) -> u64 {
        match self {
            Self::Table { generation, .. } | Self::Overview { generation, .. } => *generation,
        }
    }
}

/// Table polls wait 500ms after a watch change. Overview polls fetch the latest request immediately.
pub fn debounce_for(request: &PerfRequest) -> Option<Duration> {
    match request {
        PerfRequest::Table { .. } => Some(DEBOUNCE_DELAY),
        PerfRequest::Overview { .. } => None,
    }
}

/// Write `ERR`, then build the event the worker sends. The store update happens first.
pub fn overview_failure_event(
    store: &PerfOverviewShare,
    generation: u64,
    message: &str,
) -> AppEvent {
    record_overview_failure(store, message);
    AppEvent::PerfOverviewResult { generation }
}

pub async fn run_perf_worker(
    client: VimClientHandle,
    mut watch_rx: watch::Receiver<PerfRequest>,
    event_tx: mpsc::UnboundedSender<Event>,
) {
    let mut poller: Option<PerfPollerState> = None;
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            res = watch_rx.changed() => {
                if res.is_err() {
                    return;
                }
                let peeked = watch_rx.borrow().clone();
                if let Some(delay) = debounce_for(&peeked) {
                    tokio::time::sleep(delay).await;
                }
                interval.reset();
            }
        }

        let req = watch_rx.borrow_and_update().clone();
        if matches!(&req, PerfRequest::Table { entities, .. } if entities.is_empty()) {
            continue;
        }

        if poller.is_none() {
            match PerfPollerState::new(client.clone()) {
                Ok(poller_state) => poller = Some(poller_state),
                Err(err) => {
                    if let PerfRequest::Overview {
                        generation, store, ..
                    } = &req
                    {
                        let event = overview_failure_event(store, *generation, &format!("{err:#}"));
                        let _ = event_tx.send(Event::App(Box::new(event)));
                    } else {
                        log::warn!("perf worker: PerfClient init failed: {err:#}");
                    }
                    continue;
                }
            }
        }
        let Some(poller_ref) = poller.as_mut() else {
            continue;
        };

        match &req {
            PerfRequest::Table {
                entities, snapshot, ..
            } => match poller_ref.poll_entities(entities, snapshot).await {
                Ok(()) => {
                    let _ = event_tx.send(Event::App(Box::new(AppEvent::PerfResult {
                        generation: req.generation(),
                    })));
                }
                Err(err) => {
                    log::warn!("perf worker: {err:#}");
                }
            },
            PerfRequest::Overview { entity, store, .. } => {
                match poller_ref.poll_overview(entity, store).await {
                    Ok(()) => {
                        let _ = event_tx.send(Event::App(Box::new(AppEvent::PerfOverviewResult {
                            generation: req.generation(),
                        })));
                    }
                    Err(err) => {
                        let event =
                            overview_failure_event(store, req.generation(), &format!("{err:#}"));
                        let _ = event_tx.send(Event::App(Box::new(event)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf_overview::new_overview_store;
    use vim_rs::types::enums::MoTypesEnum;

    #[test]
    fn paused_request_clears_entities_and_preserves_generation() {
        let paused = PerfRequest::paused(7);
        assert_eq!(paused.generation(), 7);
        match paused {
            PerfRequest::Table { entities, .. } => assert!(entities.is_empty()),
            PerfRequest::Overview { .. } => panic!("paused is a table request"),
        }
    }

    #[test]
    fn table_debounce_is_500ms_and_overview_is_immediate() {
        let table = PerfRequest::paused(1);
        assert_eq!(debounce_for(&table), Some(Duration::from_millis(500)));
        let overview = PerfRequest::Overview {
            entity: ManagedObjectReference {
                r#type: MoTypesEnum::VirtualMachine,
                value: "vm-1".into(),
            },
            generation: 3,
            store: new_overview_store("web".into(), "VM"),
        };
        assert_eq!(debounce_for(&overview), None);
    }

    #[test]
    fn overview_fetch_error_records_err_before_the_result_event() {
        let store = new_overview_store("web".into(), "VM");
        let event = overview_failure_event(&store, 9, "transport down");
        assert_eq!(store.read().unwrap().error(), Some("transport down"));
        match event {
            AppEvent::PerfOverviewResult { generation } => assert_eq!(generation, 9),
            other => panic!("unexpected {other:?}"),
        }
    }
}
