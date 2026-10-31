//! Replace-on-poll window for one host or virtual machine.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use vim_rs::perf::error::EntityStatus;

use crate::perf_overview::layout::Domain;

/// Counter stored in the overview window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeriesId {
    CpuUsage,
    CpuMhz,
    MemConsumed,
    MemUsage,
    NetRx,
    NetTx,
    DiskRead,
    DiskWrite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePoint {
    pub ts: SystemTime,
    /// `None` is a gap (`raw < 0`). Values are already scaled.
    pub value: Option<f64>,
}

/// What a fetch did to the stored window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreUpdate {
    /// Replace every series with `samples`. Absent keys are cleared.
    Replace,
    /// Drop the window and clear `ERR`.
    Clear,
    /// Keep the window and set `ERR`.
    Keep { message: String },
}

#[derive(Debug, Clone)]
pub struct PerfOverviewStore {
    pub name: String,
    pub kind_label: &'static str,
    series: BTreeMap<SeriesId, Vec<SamplePoint>>,
    error: Option<String>,
    sample_time: Option<SystemTime>,
}

pub type PerfOverviewShare = Arc<RwLock<PerfOverviewStore>>;

pub fn new_overview_store(name: String, kind_label: &'static str) -> PerfOverviewShare {
    Arc::new(RwLock::new(PerfOverviewStore::empty(name, kind_label)))
}

/// Record `ERR` on the overview store. Call this before emitting `PerfOverviewResult`.
pub fn record_overview_failure(store: &PerfOverviewShare, message: &str) {
    if let Ok(mut guard) = store.write() {
        guard.apply(
            StoreUpdate::Keep {
                message: clip_message(message),
            },
            BTreeMap::new(),
        );
    }
}

impl PerfOverviewStore {
    pub fn empty(name: String, kind_label: &'static str) -> Self {
        Self {
            name,
            kind_label,
            series: BTreeMap::new(),
            error: None,
            sample_time: None,
        }
    }

    pub fn apply(&mut self, update: StoreUpdate, samples: BTreeMap<SeriesId, Vec<SamplePoint>>) {
        match update {
            StoreUpdate::Replace => {
                self.series = samples;
                self.error = None;
                self.sample_time = newest_timestamp(&self.series);
            }
            StoreUpdate::Clear => {
                self.series.clear();
                self.error = None;
                self.sample_time = None;
            }
            StoreUpdate::Keep { message } => {
                self.error = Some(message);
            }
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn sample_time(&self) -> Option<SystemTime> {
        self.sample_time
    }

    pub fn series(&self, id: SeriesId) -> Option<&[SamplePoint]> {
        self.series.get(&id).map(Vec::as_slice)
    }

    pub fn latest(&self, id: SeriesId) -> Option<f64> {
        self.series
            .get(&id)
            .and_then(|points| points.iter().rev().find_map(|point| point.value))
    }

    /// True when the domain has at least one returned series.
    pub fn domain_present(&self, domain: Domain) -> bool {
        match domain {
            Domain::Cpu => self.series.contains_key(&SeriesId::CpuUsage),
            Domain::Memory => self.series.contains_key(&SeriesId::MemConsumed),
            Domain::Network => {
                self.series.contains_key(&SeriesId::NetRx)
                    || self.series.contains_key(&SeriesId::NetTx)
            }
            Domain::Disk => {
                self.series.contains_key(&SeriesId::DiskRead)
                    || self.series.contains_key(&SeriesId::DiskWrite)
            }
        }
    }
}

/// Align raw samples to timestamps. `raw < 0` and values the unit cannot scale are gaps.
pub fn points_from_raw(
    times: &[SystemTime],
    raw: &[i64],
    mut scale: impl FnMut(i64) -> Option<f64>,
) -> Vec<SamplePoint> {
    times
        .iter()
        .zip(raw.iter())
        .map(|(ts, value)| SamplePoint {
            ts: *ts,
            value: if *value < 0 { None } else { scale(*value) },
        })
        .collect()
}

/// How an entity status changes the stored window. `Ok` still needs the new samples.
pub fn store_update_for_status(status: &EntityStatus) -> StoreUpdate {
    match status {
        EntityStatus::Ok => StoreUpdate::Replace,
        EntityStatus::NoSamples
        | EntityStatus::NoMetricsAvailable
        | EntityStatus::Unsupported(_) => StoreUpdate::Clear,
        EntityStatus::Failed(err) => StoreUpdate::Keep {
            message: clip_message(&err.to_string()),
        },
        _ => StoreUpdate::Keep {
            message: "perf status".into(),
        },
    }
}

pub fn clip_message(message: &str) -> String {
    let one_line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = one_line.chars();
    let clipped: String = chars.by_ref().take(48).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

fn newest_timestamp(series: &BTreeMap<SeriesId, Vec<SamplePoint>>) -> Option<SystemTime> {
    series
        .values()
        .flat_map(|points| points.iter().map(|point| point.ts))
        .max()
}

/// Sum a pair. A missing side is 0 when the other side exists. Both missing is a gap.
pub fn sum_pair(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (None, None) => None,
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn ts(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn point(secs: u64, value: Option<f64>) -> SamplePoint {
        SamplePoint {
            ts: ts(secs),
            value,
        }
    }

    fn cpu_only() -> BTreeMap<SeriesId, Vec<SamplePoint>> {
        let mut samples = BTreeMap::new();
        samples.insert(
            SeriesId::CpuUsage,
            vec![point(10, Some(12.0)), point(30, Some(40.0))],
        );
        samples
    }

    #[test]
    fn ok_replaces_and_clears_err_and_does_not_pad() {
        let mut store = PerfOverviewStore::empty("vm".into(), "VM");
        store.apply(
            StoreUpdate::Keep {
                message: "old".into(),
            },
            BTreeMap::new(),
        );
        store.apply(StoreUpdate::Replace, cpu_only());
        assert!(store.error().is_none());
        assert_eq!(store.sample_time(), Some(ts(30)));
        let usage = store.series(SeriesId::CpuUsage).unwrap();
        assert_eq!(usage.len(), 2);
        assert!(store.series(SeriesId::DiskRead).is_none());
        assert!(!store.domain_present(Domain::Disk));
        assert!(store.domain_present(Domain::Cpu));
    }

    #[test]
    fn clear_drops_the_window_and_a_later_ok_clears_err() {
        let mut store = PerfOverviewStore::empty("vm".into(), "VM");
        store.apply(StoreUpdate::Replace, cpu_only());
        store.apply(
            StoreUpdate::Keep {
                message: "boom".into(),
            },
            BTreeMap::new(),
        );
        store.apply(StoreUpdate::Clear, BTreeMap::new());
        assert!(store.series(SeriesId::CpuUsage).is_none());
        assert!(store.error().is_none());
        assert!(store.sample_time().is_none());

        store.apply(
            StoreUpdate::Keep {
                message: "timeout".into(),
            },
            BTreeMap::new(),
        );
        assert_eq!(store.error(), Some("timeout"));
        store.apply(StoreUpdate::Replace, cpu_only());
        assert!(store.error().is_none());
    }

    #[test]
    fn keep_preserves_samples() {
        let mut store = PerfOverviewStore::empty("host".into(), "Host");
        store.apply(StoreUpdate::Replace, cpu_only());
        store.apply(
            StoreUpdate::Keep {
                message: "transport".into(),
            },
            BTreeMap::new(),
        );
        assert_eq!(store.latest(SeriesId::CpuUsage), Some(40.0));
        assert_eq!(store.error(), Some("transport"));
        assert_eq!(store.sample_time(), Some(ts(30)));
    }

    #[test]
    fn negative_raw_is_a_gap_and_the_series_is_not_padded() {
        let points = points_from_raw(&[ts(1), ts(2), ts(3)], &[-1, 10, 20], |raw| {
            Some(raw as f64)
        });
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value, None);
        assert_eq!(points[1].value, Some(10.0));
        assert_eq!(points[2].value, Some(20.0));
        let unscaled = points_from_raw(&[ts(1)], &[1234], |_| None);
        assert_eq!(unscaled[0].value, None);
    }

    #[test]
    fn status_mapping_clears_or_keeps() {
        use vim_rs::perf::error::{PerfError, UnsupportedReason};

        assert!(matches!(
            store_update_for_status(&EntityStatus::Ok),
            StoreUpdate::Replace
        ));
        for status in [
            EntityStatus::NoSamples,
            EntityStatus::NoMetricsAvailable,
            EntityStatus::Unsupported(UnsupportedReason::NotAProvider),
        ] {
            assert!(matches!(
                store_update_for_status(&status),
                StoreUpdate::Clear
            ));
        }
        match store_update_for_status(&EntityStatus::Failed(Arc::new(PerfError::NoEntities))) {
            StoreUpdate::Keep { message } => assert!(message.contains("no entities")),
            other => panic!("expected keep, got {other:?}"),
        }
    }

    #[test]
    fn pair_sum_treats_one_missing_side_as_zero() {
        assert_eq!(sum_pair(Some(2.0), None), Some(2.0));
        assert_eq!(sum_pair(None, None), None);
        assert_eq!(sum_pair(Some(1.0), Some(0.5)), Some(1.5));
    }
}
