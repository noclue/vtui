//! Async fetch and assembly of [`HostSummary`](super::HostSummary).

use anyhow::{Context, bail};
use log::{debug, warn};
use vim_rs::core::client::VimClientHandle;
use vim_rs::core::pc_retrieve::ObjectRetriever;
use vim_rs::types::structs::{
    HostGraphicsInfo, HostMemoryTierInfo, HostScsiDisk, ManagedObjectReference,
    VirtualMachineStorageSummary,
};
use vim_rs::types::traits::ScsiLunTrait;
use vim_rs::vim_retrievable;

use crate::host_summary::{
    HostDiskRow, HostGraphicsRow, HostMemoryTierRow, HostPnicRow, HostSummary, HostVmRow,
    LOG_TARGET, cap_vm_refs, cmp_natural_device_name,
};
use crate::inventory_path::resolve_inventory_path;

vim_retrievable!(
    struct HostSummaryProps: HostSystem {
        name = "name",
        overall_status = "overall_status",
        connection_state = "runtime.connection_state",
        power_state = "runtime.power_state",
        uptime_seconds = "summary.quick_stats.uptime",
        cpu_usage_mhz = "summary.quick_stats.overall_cpu_usage",
        memory_usage_mb = "summary.quick_stats.overall_memory_usage",
        hw_vendor = "summary.hardware.vendor",
        hw_model = "summary.hardware.model",
        hw_cpu_model = "summary.hardware.cpu_model",
        hw_cpu_mhz = "summary.hardware.cpu_mhz",
        hw_num_cpu_pkgs = "summary.hardware.num_cpu_pkgs",
        hw_num_cpu_cores = "summary.hardware.num_cpu_cores",
        hw_num_cpu_threads = "summary.hardware.num_cpu_threads",
        hw_memory_size_bytes = "summary.hardware.memory_size",
        memory_tiering_type = "hardware.memory_tiering_type",
        memory_tier_info = "hardware.memory_tier_info",
        pnics = "config.network.pnic",
        scsi_luns = "config.storage_device.scsi_lun",
        graphics_info = "config.graphics_info",
        vm_refs = "vm",
    }
);

vim_retrievable!(
    struct HostVmInfo: VirtualMachine {
        name = "name",
        overall_status = "overall_status",
        power_state = "runtime.power_state",
        guest_os = "summary.guest.guest_full_name",
        storage = "summary.storage",
        cpu_usage_mhz = "summary.quick_stats.overall_cpu_usage",
        memory_usage_mb = "summary.quick_stats.host_memory_usage",
    }
);

/// Load full host summary for popup display.
pub async fn fetch_host_summary(
    client: VimClientHandle,
    host: ManagedObjectReference,
) -> anyhow::Result<HostSummary> {
    let label = format!("{}:{}", host.r#type.as_str(), host.value);
    debug!(
        target: LOG_TARGET,
        "host summary fetch: start host={label}"
    );

    let retriever = ObjectRetriever::new(client.clone()).map_err(anyhow::Error::from)?;
    let props_opt = retriever
        .retrieve_object::<HostSummaryProps>(&host)
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("host summary retrieve failed for {label}"))?;

    let Some(mut props) = props_opt else {
        bail!("host summary: empty retrieve result for {label}");
    };

    let inventory_path = resolve_inventory_path(client.clone(), host.clone())
        .await
        .unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "host summary fetch: inventory path resolve failed host={label}: {e:#}"
            );
            String::new()
        });

    let host_id = host.value.clone();
    let host_name = props.name.clone();

    let mut nics = map_pnics(props.pnics.take());
    nics.sort_by(|a, b| cmp_natural_device_name(&a.device, &b.device));

    let mut disks = map_scsi_disks(props.scsi_luns.take());
    disks.sort_by(|a, b| cmp_natural_device_name(&a.device_name, &b.device_name));

    let memory_tiers = map_memory_tiers(
        props.memory_tiering_type.as_deref(),
        props.memory_tier_info.take(),
    );

    let graphics = map_graphics(props.graphics_info.take());

    let vm_refs = props.vm_refs.take().unwrap_or_default();
    let (total_vm_count, capped_refs) = cap_vm_refs(vm_refs);

    let mut vms: Vec<HostVmRow> = Vec::new();
    if !capped_refs.is_empty() {
        let batch = retriever
            .retrieve_objects_from_list::<HostVmInfo>(&capped_refs)
            .await
            .map_err(anyhow::Error::from)
            .with_context(|| format!("host summary VM batch retrieve failed for {label}"))?;

        if batch.len() != capped_refs.len() {
            warn!(
                target: LOG_TARGET,
                "host summary fetch: VM batch len mismatch host={label} expected={} got={}",
                capped_refs.len(),
                batch.len()
            );
        }

        for (mor, vm_props) in capped_refs.iter().zip(batch) {
            vms.push(HostVmRow {
                vm_id: mor.value.clone(),
                vm_name: vm_props.name.clone(),
                overall_status: vm_props.overall_status.clone(),
                power_state: vm_props.power_state.clone(),
                guest_os: vm_props.guest_os.clone(),
                storage_used_bytes: vm_props
                    .storage
                    .as_ref()
                    .map(|s: &VirtualMachineStorageSummary| s.committed),
                cpu_usage_mhz: vm_props.cpu_usage_mhz,
                memory_usage_mb: vm_props.memory_usage_mb,
            });
        }
    }

    debug!(
        target: LOG_TARGET,
        "host summary fetch: ok host={label} name={host_name} nics={} disks={} vms={}/{}",
        nics.len(),
        disks.len(),
        vms.len(),
        total_vm_count
    );

    Ok(HostSummary {
        host_id,
        host_name,
        inventory_path,
        overall_status: props.overall_status.clone(),
        connection_state: props.connection_state.clone(),
        power_state: props.power_state.clone(),
        uptime_seconds: props.uptime_seconds,
        cpu_usage_mhz: props.cpu_usage_mhz,
        memory_usage_mb: props.memory_usage_mb,
        hw_vendor: nonempty_opt(props.hw_vendor.take()),
        hw_model: nonempty_opt(props.hw_model.take()),
        hw_cpu_model: nonempty_opt(props.hw_cpu_model.take()),
        hw_cpu_mhz: props.hw_cpu_mhz,
        hw_num_cpu_pkgs: props.hw_num_cpu_pkgs,
        hw_num_cpu_cores: props.hw_num_cpu_cores,
        hw_num_cpu_threads: props.hw_num_cpu_threads,
        hw_memory_size_bytes: props.hw_memory_size_bytes,
        nics,
        disks,
        memory_tiers,
        graphics,
        vms,
        total_vm_count,
    })
}

fn nonempty_opt(s: Option<String>) -> Option<String> {
    s.filter(|x| !x.trim().is_empty())
}

fn map_pnics(pnics: Option<Vec<vim_rs::types::structs::PhysicalNic>>) -> Vec<HostPnicRow> {
    let Some(list) = pnics else {
        return Vec::new();
    };
    list.into_iter()
        .map(|p| {
            let link_speed_mbps = p.link_speed.as_ref().map(|l| l.speed_mb);
            let duplex = p.link_speed.as_ref().map(|l| l.duplex);
            HostPnicRow {
                device: p.device,
                driver: p.driver,
                driver_version: p.driver_version,
                firmware_version: p.firmware_version,
                mac: p.mac,
                link_speed_mbps,
                duplex,
                pci: p.pci,
                wake_on_lan_supported: p.wake_on_lan_supported,
            }
        })
        .collect()
}

fn scsi_disk_from_lun(lun: &dyn ScsiLunTrait) -> Option<&HostScsiDisk> {
    lun.as_any_ref().downcast_ref::<HostScsiDisk>()
}

fn map_scsi_disks(luns: Option<Vec<Box<dyn ScsiLunTrait>>>) -> Vec<HostDiskRow> {
    let Some(list) = luns else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for lun in list {
        let Some(disk) = scsi_disk_from_lun(lun.as_ref()) else {
            continue;
        };
        let lun_ref = disk.get_scsi_lun();
        let device_name = lun_ref.host_device_.device_name.clone();
        let vendor = nonempty_opt(lun_ref.vendor.clone());
        let model = nonempty_opt(lun_ref.model.clone());
        let capacity_bytes = scsi_capacity_bytes(disk);
        out.push(HostDiskRow {
            device_name,
            vendor,
            model,
            capacity_bytes,
            ssd: disk.ssd,
            local: disk.local_disk,
        });
    }
    out
}

fn scsi_capacity_bytes(disk: &HostScsiDisk) -> Option<u64> {
    let c = &disk.capacity;
    if c.block <= 0 || c.block_size <= 0 {
        return None;
    }
    let b = (c.block as u128).saturating_mul(c.block_size as u128);
    Some(u64::try_from(b.min(u128::from(u64::MAX))).unwrap_or(u64::MAX))
}

fn map_memory_tiers(
    tiering_type: Option<&str>,
    tiers: Option<Vec<HostMemoryTierInfo>>,
) -> Vec<HostMemoryTierRow> {
    let tt = tiering_type.unwrap_or("").trim();
    if tt.is_empty() || tt.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let Some(rows) = tiers else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|t| HostMemoryTierRow {
            name: t.name,
            tier_type: t.r#type,
            size_bytes: t.size,
            flags: t.flags.unwrap_or_default(),
        })
        .collect()
}

fn map_graphics(list: Option<Vec<HostGraphicsInfo>>) -> Vec<HostGraphicsRow> {
    let Some(list) = list else {
        return Vec::new();
    };
    list.into_iter()
        .map(|g| HostGraphicsRow {
            device_name: g.device_name,
            vendor_name: g.vendor_name,
            pci_id: g.pci_id,
            graphics_type: g.graphics_type,
            memory_size_kb: g.memory_size_in_kb,
            vgpu_mode: g.vgpu_mode,
            attached_vm_count: g.vm.as_ref().map(|v| v.len()).unwrap_or(0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_tiers_skipped_when_none() {
        assert!(map_memory_tiers(Some("none"), Some(vec![])).is_empty());
        assert!(map_memory_tiers(None, None).is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_host_summary_vcsim_smoke() {}
}
