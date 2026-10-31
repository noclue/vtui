//! PerformanceManager polling for VM/Host tables via `vim_rs::perf`: CPU/mem **usage %**
//! history (sparklines) and latest **absolute** usage (`cpu.usagemhz.average`,
//! `mem.consumed.average`) for compact suffix text.
//!
//! Each poll fetches the last [`SLOTS`] samples for the percent counters and **replaces** the
//! stored history for each entity. Latest MHz and memory bytes come from
//! [`Series::last_slot`] / scaled views (invalid or negative → no suffix). This avoids the
//! "accumulate on every UI interaction" bug where a 1-sample-per-poll ring buffer filled with
//! duplicates. Omitted / powered-off entities (`NoSamples` and similar) blank the snapshot row
//! so stale sparklines do not linger.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use log::debug;
use vim_rs::core::client::VimClientHandle;
use vim_rs::perf::error::EntityStatus;
use vim_rs::perf::metrics::{cpu, mem};
use vim_rs::perf::result::EntitySeries;
use vim_rs::perf::{Interval, PerfClient, PreparedQuery};
use vim_rs::types::structs::ManagedObjectReference;

/// Number of sparkline slots displayed per entity.
pub const SLOTS: usize = 6;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
struct EntitySpark {
    /// `cpu.usage` / `mem.usage` — hundredths of a percent for sparklines.
    cpu_pct: [Option<i64>; SLOTS],
    mem_pct: [Option<i64>; SLOTS],
    /// `cpu.usagemhz.average` — MHz.
    cpu_latest_mhz: Option<i64>,
    /// `mem.consumed.average` × unit (`kiloBytes` / `megaBytes` / …) → bytes for display.
    mem_latest_bytes: Option<i128>,
}

/// How to apply one entity's fetch status onto the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeAction {
    /// Replace the row with decoded samples.
    Replace,
    /// Clear the row (powered off / omitted / unsupported).
    Blank,
    /// Leave any previous row untouched (chunk-local failure).
    Keep,
}

/// Decide snapshot merge for one entity status.
///
/// `Ok` replaces; `NoSamples` / `NoMetricsAvailable` / `Unsupported` blank; `Failed` keeps.
fn merge_action_for_status(status: &EntityStatus) -> MergeAction {
    match status {
        EntityStatus::Ok => MergeAction::Replace,
        EntityStatus::NoSamples
        | EntityStatus::NoMetricsAvailable
        | EntityStatus::Unsupported(_) => MergeAction::Blank,
        EntityStatus::Failed(_) => MergeAction::Keep,
        // `EntityStatus` is `#[non_exhaustive]`; treat unknown variants like a soft failure.
        _ => MergeAction::Keep,
    }
}

fn spark_from_entity(entity: &EntitySeries) -> EntitySpark {
    let cpu_pct = entity
        .series(cpu::USAGE_AVERAGE)
        .map(|s| s.window::<SLOTS>())
        .unwrap_or([None; SLOTS]);
    let mem_pct = entity
        .series(mem::USAGE_AVERAGE)
        .map(|s| s.window::<SLOTS>())
        .unwrap_or([None; SLOTS]);
    let cpu_latest_mhz = entity
        .series(cpu::USAGEMHZ_AVERAGE)
        .and_then(|s| s.scaled().last_slot())
        .map(|sample| sample.value.as_mhz().round() as i64);
    let mem_latest_bytes = entity
        .series(mem::CONSUMED_AVERAGE)
        .and_then(|s| s.scaled().last_slot())
        .map(|sample| sample.value.0);
    EntitySpark {
        cpu_pct,
        mem_pct,
        cpu_latest_mhz,
        mem_latest_bytes,
    }
}

/// Published perf history keyed by entity MoRef.
#[derive(Clone, Default)]
pub struct PerfRowsSnapshot {
    rows: HashMap<ManagedObjectReference, EntitySpark>,
}

impl PerfRowsSnapshot {
    pub fn cpu_mem_slots(
        &self,
        m: &ManagedObjectReference,
    ) -> ([Option<i64>; SLOTS], [Option<i64>; SLOTS]) {
        let Some(e) = self.rows.get(m) else {
            return ([None; SLOTS], [None; SLOTS]);
        };
        (e.cpu_pct, e.mem_pct)
    }

    pub fn latest_cpu_mhz(&self, m: &ManagedObjectReference) -> Option<i64> {
        self.rows.get(m).and_then(|e| e.cpu_latest_mhz)
    }

    pub fn latest_mem_bytes(&self, m: &ManagedObjectReference) -> Option<i128> {
        self.rows.get(m).and_then(|e| e.mem_latest_bytes)
    }

    /// Clear history when leaving VM/Host views.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    fn apply_merge(
        &mut self,
        key: ManagedObjectReference,
        action: MergeAction,
        spark: EntitySpark,
    ) {
        match action {
            MergeAction::Replace => {
                self.rows.insert(key, spark);
            }
            MergeAction::Blank => {
                self.rows.insert(key, EntitySpark::default());
            }
            MergeAction::Keep => {}
        }
    }
}

/// Shared, lock-protected snapshot updated by perf polling.
pub type PerfSnapshotShare = Arc<RwLock<PerfRowsSnapshot>>;

pub fn new_perf_snapshot_share() -> PerfSnapshotShare {
    Arc::new(RwLock::new(PerfRowsSnapshot::default()))
}

/// Lazily initialized `vim_rs::perf` client plus a reusable prepared query.
pub struct PerfPollerState {
    prepared: PreparedQuery,
}

impl PerfPollerState {
    pub fn new(client: VimClientHandle) -> anyhow::Result<Self> {
        let perf = PerfClient::new(client).map_err(|e| anyhow::anyhow!("{e}"))?;
        let prepared = perf
            .prepare()
            .metric(cpu::USAGE_AVERAGE)
            .metric(mem::USAGE_AVERAGE)
            .metric(cpu::USAGEMHZ_AVERAGE)
            .metric(mem::CONSUMED_AVERAGE)
            .interval(Interval::Realtime)
            .latest(SLOTS as i32)
            .build();
        Ok(Self { prepared })
    }

    /// Fetch the last [`SLOTS`] CPU/mem % samples plus latest absolute usage per entity.
    pub async fn poll_entities(
        &mut self,
        entities: &[ManagedObjectReference],
        snapshot: &PerfSnapshotShare,
    ) -> anyhow::Result<()> {
        if entities.is_empty() {
            return Ok(());
        }

        let result = self
            .prepared
            .fetch(entities)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        for warning in &result.warnings {
            debug!("perf warning: {warning:?}");
        }

        let mut g = snapshot
            .write()
            .map_err(|e| anyhow::anyhow!("perf snapshot lock poisoned: {e}"))?;

        for entity in result.entities() {
            let action = merge_action_for_status(&entity.status);
            match action {
                MergeAction::Keep => {
                    debug!(
                        "perf entity {} failed; keeping previous snapshot row",
                        entity.entity.value
                    );
                    g.apply_merge(entity.entity.clone(), action, EntitySpark::default());
                }
                MergeAction::Blank => {
                    g.apply_merge(entity.entity.clone(), action, EntitySpark::default());
                }
                MergeAction::Replace => {
                    let spark = spark_from_entity(entity);
                    g.apply_merge(entity.entity.clone(), action, spark);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vim_rs::perf::error::{PerfError, UnsupportedReason};
    use vim_rs::types::enums::MoTypesEnum;
    use vim_rs::types::structs::ManagedObjectReference;

    fn sample_spark() -> EntitySpark {
        EntitySpark {
            cpu_pct: [Some(100), Some(200), None, Some(400), Some(500), Some(600)],
            mem_pct: [Some(10); SLOTS],
            cpu_latest_mhz: Some(428),
            mem_latest_bytes: Some(1_073_741_824),
        }
    }

    fn moref(value: &str) -> ManagedObjectReference {
        ManagedObjectReference {
            r#type: MoTypesEnum::VirtualMachine,
            value: value.to_string(),
        }
    }

    #[test]
    fn merge_ok_replaces() {
        assert_eq!(
            merge_action_for_status(&EntityStatus::Ok),
            MergeAction::Replace
        );
    }

    #[test]
    fn merge_no_samples_blanks() {
        assert_eq!(
            merge_action_for_status(&EntityStatus::NoSamples),
            MergeAction::Blank
        );
    }

    #[test]
    fn merge_no_metrics_blanks() {
        assert_eq!(
            merge_action_for_status(&EntityStatus::NoMetricsAvailable),
            MergeAction::Blank
        );
    }

    #[test]
    fn merge_unsupported_blanks() {
        assert_eq!(
            merge_action_for_status(&EntityStatus::Unsupported(UnsupportedReason::NotAProvider)),
            MergeAction::Blank
        );
    }

    #[test]
    fn merge_failed_keeps() {
        assert_eq!(
            merge_action_for_status(&EntityStatus::Failed(Arc::new(PerfError::NoEntities))),
            MergeAction::Keep
        );
    }

    #[test]
    fn snapshot_no_samples_blanks_previous_row() {
        let key = moref("vm-1");
        let mut snap = PerfRowsSnapshot::default();
        snap.rows.insert(key.clone(), sample_spark());

        snap.apply_merge(
            key.clone(),
            merge_action_for_status(&EntityStatus::NoSamples),
            sample_spark(),
        );

        let row = snap.rows.get(&key).expect("row present");
        assert_eq!(*row, EntitySpark::default());
        assert_eq!(snap.latest_cpu_mhz(&key), None);
        assert_eq!(snap.latest_mem_bytes(&key), None);
    }

    #[test]
    fn snapshot_failed_leaves_previous_row() {
        let key = moref("vm-2");
        let mut snap = PerfRowsSnapshot::default();
        snap.rows.insert(key.clone(), sample_spark());

        snap.apply_merge(
            key.clone(),
            merge_action_for_status(&EntityStatus::Failed(Arc::new(PerfError::NoEntities))),
            EntitySpark::default(),
        );

        let row = snap.rows.get(&key).expect("row present");
        assert_eq!(*row, sample_spark());
        assert_eq!(snap.latest_cpu_mhz(&key), Some(428));
        assert_eq!(snap.latest_mem_bytes(&key), Some(1_073_741_824));
    }
}
