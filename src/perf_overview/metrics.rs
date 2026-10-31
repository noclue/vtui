//! One realtime aggregate query for the host or virtual-machine overview.

use std::collections::BTreeMap;

use vim_rs::perf::metrics::{cpu, disk, mem, net};
use vim_rs::perf::result::{EntitySeries, Series};
use vim_rs::perf::units::{Bytes, BytesPerSecond, Dimension, Hertz, Percent};
use vim_rs::perf::{Interval, PerfClient, PreparedQuery};
use vim_rs::types::structs::ManagedObjectReference;

use crate::perf_overview::store::{
    PerfOverviewStore, SamplePoint, SeriesId, StoreUpdate, points_from_raw, store_update_for_status,
};

/// Newest samples requested on every overview poll.
pub const OVERVIEW_LATEST: i32 = 90;

pub fn prepare_overview(perf: &PerfClient) -> PreparedQuery {
    perf.prepare()
        .metric(cpu::USAGE_AVERAGE)
        .metric(cpu::USAGEMHZ_AVERAGE)
        .metric(mem::CONSUMED_AVERAGE)
        .metric(mem::USAGE_AVERAGE)
        .metric(net::RECEIVED_AVERAGE)
        .metric(net::TRANSMITTED_AVERAGE)
        .metric(disk::READ_AVERAGE)
        .metric(disk::WRITE_AVERAGE)
        .interval(Interval::Realtime)
        .latest(OVERVIEW_LATEST)
        .build()
}

pub fn apply_entity(store: &mut PerfOverviewStore, entity: &EntitySeries) {
    match store_update_for_status(&entity.status) {
        StoreUpdate::Replace => store.apply(StoreUpdate::Replace, samples_from_entity(entity)),
        other => store.apply(other, BTreeMap::new()),
    }
}

pub fn samples_from_entity(entity: &EntitySeries) -> BTreeMap<SeriesId, Vec<SamplePoint>> {
    let mut samples = BTreeMap::new();
    insert(
        &mut samples,
        SeriesId::CpuUsage,
        entity.series(cpu::USAGE_AVERAGE),
        |percent: Percent| percent.0,
    );
    insert(
        &mut samples,
        SeriesId::CpuMhz,
        entity.series(cpu::USAGEMHZ_AVERAGE),
        Hertz::as_mhz,
    );
    insert(
        &mut samples,
        SeriesId::MemConsumed,
        entity.series(mem::CONSUMED_AVERAGE),
        |bytes: Bytes| bytes.0 as f64,
    );
    insert(
        &mut samples,
        SeriesId::MemUsage,
        entity.series(mem::USAGE_AVERAGE),
        |percent: Percent| percent.0,
    );
    insert(
        &mut samples,
        SeriesId::NetRx,
        entity.series(net::RECEIVED_AVERAGE),
        |rate: BytesPerSecond| rate.0,
    );
    insert(
        &mut samples,
        SeriesId::NetTx,
        entity.series(net::TRANSMITTED_AVERAGE),
        |rate: BytesPerSecond| rate.0,
    );
    insert(
        &mut samples,
        SeriesId::DiskRead,
        entity.series(disk::READ_AVERAGE),
        |rate: BytesPerSecond| rate.0,
    );
    insert(
        &mut samples,
        SeriesId::DiskWrite,
        entity.series(disk::WRITE_AVERAGE),
        |rate: BytesPerSecond| rate.0,
    );
    samples
}

fn insert<D, F>(
    out: &mut BTreeMap<SeriesId, Vec<SamplePoint>>,
    id: SeriesId,
    series: Option<Series<'_, D>>,
    map: F,
) where
    D: Dimension,
    F: Fn(D::Scaled) -> f64,
{
    let Some(series) = series else {
        return;
    };
    let samples: Vec<_> = series.samples().collect();
    let unit = &series.unit().0;
    let times: Vec<_> = samples.iter().map(|sample| sample.ts.0).collect();
    let raw: Vec<_> = samples.iter().map(|sample| sample.value).collect();
    let points = points_from_raw(&times, &raw, |value| D::scale(value, unit).map(&map));
    out.insert(id, points);
}

/// Apply one fetched entity, or clear the window when the server omitted it.
pub fn apply_fetch<'a>(
    store: &mut PerfOverviewStore,
    entity: &ManagedObjectReference,
    entities: impl IntoIterator<Item = &'a EntitySeries>,
) {
    if let Some(found) = entities.into_iter().find(|item| item.entity == *entity) {
        apply_entity(store, found);
    } else {
        store.apply(StoreUpdate::Clear, BTreeMap::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_query_is_eight_realtime_counters_without_virtual_disk() {
        assert_eq!(OVERVIEW_LATEST, 90);
        let source = include_str!("metrics.rs");
        let builder = source.split("mod tests").next().unwrap_or(source);
        for metric in [
            "cpu::USAGE_AVERAGE",
            "cpu::USAGEMHZ_AVERAGE",
            "mem::CONSUMED_AVERAGE",
            "mem::USAGE_AVERAGE",
            "net::RECEIVED_AVERAGE",
            "net::TRANSMITTED_AVERAGE",
            "disk::READ_AVERAGE",
            "disk::WRITE_AVERAGE",
        ] {
            assert!(builder.contains(metric), "{metric}");
        }
        assert!(builder.contains("Interval::Realtime"));
        assert!(builder.contains(".latest(OVERVIEW_LATEST)"));
        assert!(!builder.contains("virtualDisk"));
        assert!(!builder.contains("Instance::All"));
    }
}
