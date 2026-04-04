//! PerformanceManager polling for VM/Host tables: CPU/mem **usage %** history (sparklines) and
//! latest **absolute** usage (`cpu.usagemhz.average`, `mem.consumed.average`) for compact suffix text.
//!
//! Each poll fetches the last [`SLOTS`] samples for the percent counters and **replaces** the
//! stored history for each entity. Latest MHz and memory bytes (using each counter’s `unit_info`)
//! come from the **latest** sample in each series in the same `QueryPerf` response (invalid or
//! negative → no suffix). This avoids the
//! "accumulate on every UI interaction"
//! bug where a 1-sample-per-poll ring buffer filled with duplicates.

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

/// Pick `PerfCounterInfo` for `group` + `name`. Prefer average rollup.
fn counter_pick_prefer_average<'a>(
    counters: &'a [PerfCounterInfo],
    group: &str,
    name: &str,
) -> Option<&'a PerfCounterInfo> {
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
    best
}

fn counter_id_prefer_average(counters: &[PerfCounterInfo], group: &str, name: &str) -> Option<i32> {
    counter_pick_prefer_average(counters, group, name).map(|c| c.key)
}

/// Bytes per one reported unit from `PerfCounterInfo.unit_info` (vSphere `kiloBytes` = 1024 B, …).
fn bytes_per_counter_unit(counter: &PerfCounterInfo) -> i128 {
    match counter.unit_info.key.as_str() {
        "kiloBytes" => 1024,
        "megaBytes" => 1024 * 1024,
        "gigaBytes" => 1024_i128.pow(3),
        "teraBytes" => 1024_i128.pow(4),
        _ => 1024,
    }
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

/// Latest sample only (QueryPerf returns oldest→newest; we use the last element).
/// Missing or negative (e.g. no data while powered off) → `None` so the UI shows a blank suffix.
fn latest_valid_suffix_i64(raw: &[i64]) -> Option<i64> {
    match raw.last().copied() {
        Some(v) if v >= 0 => Some(v),
        _ => None,
    }
}

fn latest_valid_suffix_bytes(raw: &[i64], bytes_per_unit: i128) -> Option<i128> {
    match raw.last().copied() {
        Some(v) if v >= 0 => Some(i128::from(v).saturating_mul(bytes_per_unit)),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct EntitySpark {
    /// `cpu.usage` / `mem.usage` — hundredths of a percent for sparklines.
    cpu_pct: [Option<i64>; SLOTS],
    mem_pct: [Option<i64>; SLOTS],
    /// `cpu.usagemhz.average` — MHz.
    cpu_latest_mhz: Option<i64>,
    /// `mem.consumed.average` × unit (`kiloBytes` / `megaBytes` / …) → bytes for display.
    mem_latest_bytes: Option<i128>,
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
}

/// Shared, lock-protected snapshot updated by perf polling.
pub type PerfSnapshotShare = Arc<RwLock<PerfRowsSnapshot>>;

pub fn new_perf_snapshot_share() -> PerfSnapshotShare {
    Arc::new(RwLock::new(PerfRowsSnapshot::default()))
}

#[derive(Clone)]
struct PerfCounterIds {
    cpu_usage: i32,
    mem_usage: i32,
    cpu_usagemhz: i32,
    mem_consumed: i32,
    mem_consumed_bytes_per_unit: i128,
}

/// Lazily initialized PerformanceManager state (counter IDs + interval).
pub struct PerfPollerState {
    perf_manager: PerformanceManager,
    counter_ids: Option<PerfCounterIds>,
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
            counter_ids: None,
            interval_id: 20,
        })
    }

    async fn ensure_counters(&mut self) -> anyhow::Result<PerfCounterIds> {
        if let Some(ref ids) = self.counter_ids {
            return Ok(ids.clone());
        }
        let Some(counters) = self.perf_manager.perf_counter().await? else {
            anyhow::bail!("perf_counter returned no data");
        };
        let cpu_usage = counter_id_prefer_average(&counters, "cpu", "usage")
            .ok_or_else(|| anyhow::anyhow!("perf counter cpu.usage not found"))?;
        let mem_usage = counter_id_prefer_average(&counters, "mem", "usage")
            .ok_or_else(|| anyhow::anyhow!("perf counter mem.usage not found"))?;
        let cpu_usagemhz = counter_id_prefer_average(&counters, "cpu", "usagemhz")
            .ok_or_else(|| anyhow::anyhow!("perf counter cpu.usagemhz not found"))?;
        let mem_consumed_counter = counter_pick_prefer_average(&counters, "mem", "consumed")
            .ok_or_else(|| anyhow::anyhow!("perf counter mem.consumed not found"))?;
        let mem_consumed = mem_consumed_counter.key;
        let mem_consumed_bytes_per_unit = bytes_per_counter_unit(mem_consumed_counter);
        let ids = PerfCounterIds {
            cpu_usage,
            mem_usage,
            cpu_usagemhz,
            mem_consumed,
            mem_consumed_bytes_per_unit,
        };
        self.counter_ids = Some(ids.clone());
        Ok(ids)
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
        let ids = self.ensure_counters().await?;

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
                        counter_id: ids.cpu_usage,
                        instance: String::new(),
                    },
                    PerfMetricId {
                        counter_id: ids.mem_usage,
                        instance: String::new(),
                    },
                    PerfMetricId {
                        counter_id: ids.cpu_usagemhz,
                        instance: String::new(),
                    },
                    PerfMetricId {
                        counter_id: ids.mem_consumed,
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
            let mut cpu_pct_raw: Vec<i64> = Vec::new();
            let mut mem_pct_raw: Vec<i64> = Vec::new();
            let mut cpu_mhz_raw: Vec<i64> = Vec::new();
            let mut mem_kb_raw: Vec<i64> = Vec::new();
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
                    if cid == ids.cpu_usage {
                        cpu_pct_raw = vals.clone();
                    } else if cid == ids.mem_usage {
                        mem_pct_raw = vals.clone();
                    } else if cid == ids.cpu_usagemhz {
                        cpu_mhz_raw = vals.clone();
                    } else if cid == ids.mem_consumed {
                        mem_kb_raw = vals.clone();
                    }
                }
            }
            new_rows.insert(
                key,
                EntitySpark {
                    cpu_pct: pad_samples(&cpu_pct_raw, SLOTS),
                    mem_pct: pad_samples(&mem_pct_raw, SLOTS),
                    cpu_latest_mhz: latest_valid_suffix_i64(&cpu_mhz_raw),
                    mem_latest_bytes: latest_valid_suffix_bytes(
                        &mem_kb_raw,
                        ids.mem_consumed_bytes_per_unit,
                    ),
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
