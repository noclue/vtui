//! Host summary popup: fetch paths and UI-facing rows.

mod fetch;

pub use fetch::fetch_host_summary;

use vim_rs::types::enums::{
    HostSystemConnectionStateEnum, HostSystemPowerStateEnum, ManagedEntityStatusEnum,
    VirtualMachinePowerStateEnum,
};

/// Max resident VM detail rows fetched for one host summary popup.
pub const HOST_SUMMARY_VM_CAP: usize = 300;

/// `log::` target for host summary fetch and UI.
pub const LOG_TARGET: &str = "host_summary";

/// Summary payload for the host summary popup (UI-facing).
#[derive(Debug, Clone)]
pub struct HostSummary {
    pub host_id: String,
    pub host_name: String,
    pub inventory_path: String,
    pub overall_status: ManagedEntityStatusEnum,
    pub connection_state: HostSystemConnectionStateEnum,
    pub power_state: HostSystemPowerStateEnum,
    pub uptime_seconds: Option<i32>,
    pub cpu_usage_mhz: Option<i32>,
    pub memory_usage_mb: Option<i32>,
    pub hw_vendor: Option<String>,
    pub hw_model: Option<String>,
    pub hw_cpu_model: Option<String>,
    pub hw_cpu_mhz: Option<i32>,
    pub hw_num_cpu_pkgs: Option<i16>,
    pub hw_num_cpu_cores: Option<i16>,
    pub hw_num_cpu_threads: Option<i16>,
    pub hw_memory_size_bytes: Option<i64>,
    pub nics: Vec<HostPnicRow>,
    pub disks: Vec<HostDiskRow>,
    pub memory_tiers: Vec<HostMemoryTierRow>,
    pub graphics: Vec<HostGraphicsRow>,
    pub vms: Vec<HostVmRow>,
    pub total_vm_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPnicRow {
    pub device: String,
    pub driver: Option<String>,
    pub driver_version: Option<String>,
    pub firmware_version: Option<String>,
    pub mac: String,
    pub link_speed_mbps: Option<i32>,
    pub duplex: Option<bool>,
    pub pci: String,
    pub wake_on_lan_supported: bool,
}

/// Host disk display row.
///
/// Sourced only from `config.storage_device.scsi_lun` (`HostScsiDisk`). ESXi reports local NVMe
/// drives here as SCSI LUNs, so `nvme_topology` is intentionally not used (it would duplicate rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDiskRow {
    pub device_name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub ssd: Option<bool>,
    pub local: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMemoryTierRow {
    pub name: String,
    pub tier_type: String,
    pub size_bytes: i64,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostGraphicsRow {
    pub device_name: String,
    pub vendor_name: String,
    pub pci_id: String,
    pub graphics_type: String,
    pub memory_size_kb: i64,
    pub vgpu_mode: Option<String>,
    pub attached_vm_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostVmRow {
    pub vm_id: String,
    pub vm_name: String,
    pub overall_status: ManagedEntityStatusEnum,
    pub power_state: VirtualMachinePowerStateEnum,
    pub guest_os: Option<String>,
    pub storage_used_bytes: Option<i64>,
    pub cpu_usage_mhz: Option<i32>,
    pub memory_usage_mb: Option<i32>,
}

/// Compare physical NIC device names with numeric suffix awareness (`vmnic2` before `vmnic10`).
pub(crate) fn cmp_natural_device_name(a: &str, b: &str) -> std::cmp::Ordering {
    fn split_tail(s: &str) -> (&str, Option<u32>) {
        let i = s
            .char_indices()
            .rfind(|(_, c)| !c.is_ascii_digit())
            .map(|(idx, _)| idx + 1)
            .unwrap_or(0);
        let (prefix, tail) = s.split_at(i);
        if tail.is_empty() {
            (s, None)
        } else if let Ok(n) = tail.parse::<u32>() {
            (prefix, Some(n))
        } else {
            (s, None)
        }
    }
    let (ap, an) = split_tail(a);
    let (bp, bn) = split_tail(b);
    match ap.cmp(bp) {
        std::cmp::Ordering::Equal => match (an, bn) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
        o => o,
    }
}

/// Total VM references and up to [`HOST_SUMMARY_VM_CAP`] refs for batch retrieve.
pub(crate) fn cap_vm_refs(
    refs: Vec<vim_rs::types::structs::ManagedObjectReference>,
) -> (usize, Vec<vim_rs::types::structs::ManagedObjectReference>) {
    let total = refs.len();
    let capped = refs.into_iter().take(HOST_SUMMARY_VM_CAP).collect();
    (total, capped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_vmnic_order() {
        let mut names = vec!["vmnic10", "vmnic2", "vmnic1"];
        names.sort_by(|a, b| cmp_natural_device_name(a, b));
        assert_eq!(names, vec!["vmnic1", "vmnic2", "vmnic10"]);
    }

    #[test]
    fn vm_cap_zero() {
        let (t, v) = cap_vm_refs(vec![]);
        assert_eq!(t, 0);
        assert!(v.is_empty());
    }

    #[test]
    fn vm_cap_under() {
        let refs: Vec<_> = (0..5)
            .map(|i| vim_rs::types::structs::ManagedObjectReference {
                r#type: vim_rs::types::enums::MoTypesEnum::VirtualMachine,
                value: format!("vm-{i}"),
            })
            .collect();
        let (t, v) = cap_vm_refs(refs.clone());
        assert_eq!(t, 5);
        assert_eq!(v.len(), 5);
    }

    #[test]
    fn vm_cap_over() {
        let refs: Vec<_> = (0..HOST_SUMMARY_VM_CAP + 50)
            .map(|i| vim_rs::types::structs::ManagedObjectReference {
                r#type: vim_rs::types::enums::MoTypesEnum::VirtualMachine,
                value: format!("vm-{i}"),
            })
            .collect();
        let (t, v) = cap_vm_refs(refs);
        assert_eq!(t, HOST_SUMMARY_VM_CAP + 50);
        assert_eq!(v.len(), HOST_SUMMARY_VM_CAP);
    }
}
