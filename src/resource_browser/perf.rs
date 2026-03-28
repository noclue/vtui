//! PerformanceManager polling for VM/Host CPU and memory usage % (sparkline history).
//!
//! Each poll fetches the last [`SLOTS`] samples from `QueryPerf` and **replaces** the stored
//! history for each entity. This avoids the "accumulate on every UI interaction" bug where
//! a 1-sample-per-poll ring buffer filled with duplicates.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use log::debug;
use vim_rs::core::client::Client;
use vim_rs::mo::PerformanceManager;
use vim_rs::types::enums::PerfSummaryTypeEnum;
use vim_rs::types::structs::{
    ManagedObjectReference, PerfCounterInfo, PerfEntityMetric, PerfMetricId, PerfMetricIntSeries,
    PerfQuerySpec,
};

/// Number of sparkline slots displayed per entity.
pub const SLOTS: usize = 6;

/// Pick `PerfCounterInfo.key` for `group` + `name` (e.g. cpu + usage). Prefer average rollup.
fn usage_counter_id(counters: &[PerfCounterInfo], group: &str, name: &str) -> Option<i32> {
    let mut best: Option<&PerfCounterInfo> = None;
    for c in counters {
        if c.group_info.key != group || c.name_info.key != name {
            continue;
        }
        match best {
            None => best = Some(c),
            Some(prev) => {
                let new_avg = matches!(c.rollup_type, PerfSummaryTypeEnum::Average);
                let prev_avg = matches!(prev.rollup_type, PerfSummaryTypeEnum::Average);
                if new_avg && !prev_avg {
                    best = Some(c);
                }
            }
        }
    }
    best.map(|c| c.key)
}

/// Six raw `query_perf` samples (oldest first). Left-padded with `None` when fewer returned.
fn pad_samples(raw: &[i64], count: usize) -> [Option<i64>; SLOTS] {
    let mut out = [None; SLOTS];
    let n = raw.len().min(count);
    let pad = count.saturating_sub(n);
    for i in 0..n {
        let v = raw[raw.len() - n + i];
        out[pad + i] = if v >= 0 { Some(v) } else { None };
    }
    out
}

#[derive(Clone, Default)]
struct EntitySpark {
    cpu: [Option<i64>; SLOTS],
    mem: [Option<i64>; SLOTS],
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
        (e.cpu, e.mem)
    }

    /// Clear history when leaving VM/Host views.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

/// Shared, lock-protected snapshot updated by perf polling.
pub type PerfSnapshotShare = Arc<RwLock<PerfRowsSnapshot>>;

pub fn new_perf_snapshot_share() -> PerfSnapshotShare {
    Arc::new(RwLock::new(PerfRowsSnapshot::default()))
}

/// Lazily initialized PerformanceManager state (counter IDs + interval).
pub struct PerfPollerState {
    perf_manager: PerformanceManager,
    cpu_counter_id: Option<i32>,
    mem_counter_id: Option<i32>,
    interval_id: i32,
}

impl PerfPollerState {
    pub fn new(client: Arc<Client>) -> anyhow::Result<Self> {
        let Some(pm_moref) = client.service_content().perf_manager.clone() else {
            anyhow::bail!("PerformanceManager not available in ServiceContent");
        };
        let perf_manager = PerformanceManager::new(client, &pm_moref.value);
        Ok(Self {
            perf_manager,
            cpu_counter_id: None,
            mem_counter_id: None,
            interval_id: 20,
        })
    }

    async fn ensure_counters(&mut self) -> anyhow::Result<(i32, i32)> {
        if let (Some(c), Some(m)) = (self.cpu_counter_id, self.mem_counter_id) {
            return Ok((c, m));
        }
        let Some(counters) = self.perf_manager.perf_counter().await? else {
            anyhow::bail!("perf_counter returned no data");
        };
        let cpu = usage_counter_id(&counters, "cpu", "usage")
            .ok_or_else(|| anyhow::anyhow!("perf counter cpu.usage not found"))?;
        let mem = usage_counter_id(&counters, "mem", "usage")
            .ok_or_else(|| anyhow::anyhow!("perf counter mem.usage not found"))?;
        self.cpu_counter_id = Some(cpu);
        self.mem_counter_id = Some(mem);
        Ok((cpu, mem))
    }

    /// Fetch the last [`SLOTS`] CPU/mem % samples for each entity and **replace** the snapshot.
    pub async fn poll_entities(
        &mut self,
        entities: &[ManagedObjectReference],
        snapshot: &PerfSnapshotShare,
    ) -> anyhow::Result<()> {
        if entities.is_empty() {
            return Ok(());
        }
        let (cpu_id, mem_id) = self.ensure_counters().await?;

        if let Some(first) = entities.first()
            && let Ok(summary) = self.perf_manager.query_perf_provider_summary(first).await
            && let Some(rr) = summary.refresh_rate
            && rr > 0
        {
            self.interval_id = rr;
        }

        let specs: Vec<PerfQuerySpec> = entities
            .iter()
            .map(|entity| PerfQuerySpec {
                entity: entity.clone(),
                start_time: None,
                end_time: None,
                max_sample: Some(SLOTS as i32),
                metric_id: Some(vec![
                    PerfMetricId {
                        counter_id: cpu_id,
                        instance: String::new(),
                    },
                    PerfMetricId {
                        counter_id: mem_id,
                        instance: String::new(),
                    },
                ]),
                interval_id: Some(self.interval_id),
                format: None,
            })
            .collect();

        let Some(stats) = self.perf_manager.query_perf(&specs).await? else {
            debug!("query_perf returned None");
            return Ok(());
        };

        let mut new_rows: HashMap<ManagedObjectReference, EntitySpark> = HashMap::new();

        for stat in stats {
            let Some(pem) = stat
                .as_ref()
                .as_any_ref()
                .downcast_ref::<PerfEntityMetric>()
            else {
                continue;
            };
            let key = pem.entity.clone();
            let Some(values) = &pem.value else {
                new_rows.insert(key, EntitySpark::default());
                continue;
            };
            let mut cpu_raw: Vec<i64> = Vec::new();
            let mut mem_raw: Vec<i64> = Vec::new();
            for series in values {
                let Some(ints) = series
                    .as_ref()
                    .as_any_ref()
                    .downcast_ref::<PerfMetricIntSeries>()
                else {
                    continue;
                };
                let cid = ints.id.counter_id;
                if let Some(vals) = &ints.value {
                    if cid == cpu_id {
                        cpu_raw = vals.clone();
                    } else if cid == mem_id {
                        mem_raw = vals.clone();
                    }
                }
            }
            new_rows.insert(
                key,
                EntitySpark {
                    cpu: pad_samples(&cpu_raw, SLOTS),
                    mem: pad_samples(&mem_raw, SLOTS),
                },
            );
        }

        let mut g = snapshot
            .write()
            .map_err(|e| anyhow::anyhow!("perf snapshot lock poisoned: {e}"))?;
        for (k, v) in new_rows {
            g.rows.insert(k, v);
        }
        Ok(())
    }
}
