//! VM summary popup: fetch, merge, and formatting helpers.

mod fetch;
pub mod format;

pub use fetch::fetch_vm_summary;

use vim_rs::types::enums::{ManagedEntityStatusEnum, VirtualMachinePowerStateEnum};
use vim_rs::types::structs::GuestNicInfo;

/// Summary payload for the VM summary popup (UI-facing).
#[derive(Debug, Clone)]
pub struct VmSummary {
    pub vm_id: String,
    pub vm_name: String,
    pub overall_status: ManagedEntityStatusEnum,
    pub guest_os: Option<String>,
    pub power_state: VirtualMachinePowerStateEnum,
    pub uptime_seconds: Option<i32>,
    pub primary_ip: Option<String>,
    pub tools_line: Option<String>,
    pub vcpu_count: Option<i32>,
    pub host_memory_usage_mb: Option<i32>,
    pub memory_size_mb: Option<i32>,
    pub disk_used_bytes: Option<i64>,
    pub cpu_usage_mhz: Option<i32>,
    pub host: Option<VmSummaryHost>,
    pub networking: Vec<VmNetworkRow>,
    pub disks: Vec<VmDiskRow>,
}

#[derive(Debug, Clone)]
pub struct VmSummaryHost {
    pub host_id: String,
    pub host_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmNetworkRow {
    pub nic_label: String,
    pub network: String,
    pub mac: String,
    pub ips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmDiskRow {
    pub vmdk_file: String,
    pub datastore: String,
    pub capacity_bytes: u64,
    pub thin: Option<bool>,
    pub mode: String,
}

/// Merge hardware NICs with guest NIC info; append guest-only rows. Sort: labeled NICs by label, then guest-only (`-`).
pub fn merge_network_rows(
    hardware: Vec<HardwareNicInfo>,
    guest: Vec<GuestNicInfo>,
) -> Vec<VmNetworkRow> {
    use std::collections::{HashMap, HashSet};

    let mut by_key: HashMap<i32, HardwareNicInfo> = HashMap::new();
    for h in hardware {
        by_key.insert(h.key, h);
    }

    let mut used_guest: HashSet<i32> = HashSet::new();
    let mut rows: Vec<VmNetworkRow> = Vec::new();

    let mut keys: Vec<i32> = by_key.keys().copied().collect();
    keys.sort_unstable();

    for k in keys {
        let hw = by_key.get(&k).expect("key from map");
        let g = guest.iter().find(|n| n.device_config_id == k);
        if let Some(g) = g {
            used_guest.insert(g.device_config_id);
        }

        let ips = g.and_then(guest_ips).unwrap_or_default();

        // Always use config/hardware backing (resolved DVPG, standard switch MOR, etc.). Do not
        // prefer `guest.net[].network`, which hides portgroup MO ids for running VMs with Tools.
        let network = hw.network_fallback.clone();

        let mac = g
            .and_then(|n| n.mac_address.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| hw.mac_hw.clone());

        rows.push(VmNetworkRow {
            nic_label: hw.label.clone(),
            network,
            mac,
            ips,
        });
    }

    for g in guest {
        if used_guest.contains(&g.device_config_id) {
            continue;
        }
        let ips = guest_ips(&g).unwrap_or_default();
        rows.push(VmNetworkRow {
            nic_label: "-".to_string(),
            network: g.network.clone().unwrap_or_else(|| "-".to_string()),
            mac: g.mac_address.clone().unwrap_or_else(|| "-".to_string()),
            ips,
        });
    }

    rows.sort_by(|a, b| {
        let a_guest = a.nic_label == "-";
        let b_guest = b.nic_label == "-";
        match (a_guest, b_guest) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a.nic_label.cmp(&b.nic_label),
        }
    });

    rows
}

#[derive(Debug, Clone)]
pub struct HardwareNicInfo {
    pub key: i32,
    pub label: String,
    pub mac_hw: String,
    pub network_fallback: String,
    /// `portgroupKey` from DV port backing — vSphere uses this as the `DistributedVirtualPortgroup` MO id.
    /// When set, `network_fallback` is a placeholder until [`crate::vm_summary::fetch_vm_summary`] resolves the name.
    pub dv_portgroup_mo_id: Option<String>,
}

fn guest_ips(n: &GuestNicInfo) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    if let Some(ref ips) = n.ip_address {
        out.extend(ips.iter().cloned());
    }
    if let Some(ref cfg) = n.ip_config
        && let Some(ref infos) = cfg.ip_address
    {
        for e in infos {
            if !e.ip_address.is_empty() {
                out.push(e.ip_address.clone());
            }
        }
    }
    if out.is_empty() {
        return None;
    }
    dedupe_preserve_order(&mut out);
    Some(out)
}

fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|s| seen.insert(s.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use vim_rs::types::structs::GuestNicInfo;

    fn nic(key: i32, label: &str, mac_hw: &str, net_fb: &str) -> HardwareNicInfo {
        HardwareNicInfo {
            key,
            label: label.to_string(),
            mac_hw: mac_hw.to_string(),
            network_fallback: net_fb.to_string(),
            dv_portgroup_mo_id: None,
        }
    }

    fn guest(
        device_config_id: i32,
        network: Option<&str>,
        mac: Option<&str>,
        ips: Option<Vec<&str>>,
    ) -> GuestNicInfo {
        GuestNicInfo {
            network: network.map(String::from),
            ip_address: ips.map(|v| v.iter().map(|s| (*s).to_string()).collect()),
            mac_address: mac.map(String::from),
            connected: true,
            device_config_id,
            dns_config: None,
            ip_config: None,
            net_bios_config: None,
        }
    }

    #[test]
    fn merge_joins_guest_and_hardware_by_key() {
        let hw = vec![nic(4000, "Network adapter 1", "aa:bb", "pg-name")];
        let g = vec![guest(
            4000,
            Some("VM Network"),
            Some("aa:bb"),
            Some(vec!["10.0.0.1"]),
        )];
        let rows = merge_network_rows(hw, g);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nic_label, "Network adapter 1");
        assert_eq!(rows[0].network, "pg-name");
        assert_eq!(rows[0].ips, vec!["10.0.0.1"]);
    }

    #[test]
    fn merge_appends_guest_only() {
        let hw = vec![nic(4000, "Network adapter 1", "aa:bb", "n1")];
        let g = vec![
            guest(
                4000,
                Some("VM Network"),
                Some("aa:bb"),
                Some(vec!["10.0.0.1"]),
            ),
            guest(
                5000,
                Some("Other"),
                Some("cc:dd"),
                Some(vec!["192.168.1.2"]),
            ),
        ];
        let rows = merge_network_rows(hw, g);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].nic_label, "Network adapter 1");
        let orphan = rows.iter().find(|r| r.nic_label == "-").unwrap();
        assert_eq!(orphan.network, "Other");
        assert_eq!(orphan.ips, vec!["192.168.1.2"]);
    }

    #[test]
    fn merge_sorts_guest_only_last() {
        let hw = vec![
            nic(4001, "Network adapter 2", "m2", "n2"),
            nic(4000, "Network adapter 1", "m1", "n1"),
        ];
        let g = vec![guest(9999, None, None, None)];
        let rows = merge_network_rows(hw, g);
        assert_eq!(rows[2].nic_label, "-");
    }
}
