//! Async fetch and assembly of [`VmSummary`](super::VmSummary).

use std::collections::HashMap;

use anyhow::Context;
use log::{debug, info};
use vim_rs::core::client::VimClientHandle;
use vim_rs::core::pc_retrieve::ObjectRetriever;
use vim_rs::types::convert::CastInto;
use vim_rs::types::enums::MoTypesEnum;
use vim_rs::types::struct_enum::StructType;
use vim_rs::types::structs::{
    DistributedVirtualSwitchPortConnection, GuestNicInfo, ManagedObjectReference, VirtualDisk,
    VirtualDiskFlatVer2BackingInfo, VirtualDiskRawDiskMappingVer1BackingInfo,
    VirtualDiskSeSparseBackingInfo, VirtualDiskSparseVer2BackingInfo,
    VirtualEthernetCardDistributedVirtualPortBackingInfo, VirtualEthernetCardNetworkBackingInfo,
    VirtualEthernetCardOpaqueNetworkBackingInfo, VirtualMachineStorageSummary,
    VirtualSriovEthernetCardSriovBackingInfo,
};
use vim_rs::types::traits::{VirtualDeviceBackingInfoTrait, VirtualEthernetCardTrait};
use vim_rs::vim_retrievable;

use super::{HardwareNicInfo, VmDiskRow, VmSummary, VmSummaryHost, merge_network_rows};
use crate::vm_summary::format::format_vmware_tools_summary;

vim_retrievable!(
    struct VmSummaryProps: VirtualMachine {
        name = "name",
        guest_os = "summary.guest.guest_full_name",
        overall_status = "overall_status",
        power_state = "runtime.power_state",
        uptime_seconds = "summary.quick_stats.uptime_seconds",
        host = "runtime.host",
        // guest.ip_address: live GuestInfo; summary.guest.ip_address: fallback when guest is unavailable.
        ip_guest = "guest.ip_address",
        ip_summary = "summary.guest.ip_address",
        num_cpu = "summary.config.num_cpu",
        memory_size_mb = "summary.config.memory_size_mb",
        host_memory_usage_mb = "summary.quick_stats.host_memory_usage",
        storage = "summary.storage",
        overall_cpu_usage_mhz = "summary.quick_stats.overall_cpu_usage",
        guest_network = "guest.net",
        guest_state = "guest.guest_state",
        tools_version = "guest.tools_version",
        tools_version_2_guest = "guest.tools_version_status_2",
        tools_version_2_summary = "summary.guest.tools_version_status_2",
        devices = "config.hardware.device",
    }
);

vim_retrievable!(
    struct HostNameProps: HostSystem {
        name = "name",
    }
);

vim_retrievable!(
    struct DatastoreNameProps: Datastore {
        name = "name",
    }
);

vim_retrievable!(
    struct DvPortgroupNameProps: DistributedVirtualPortgroup {
        name = "name",
    }
);

struct DiskRowBuild {
    vmdk_file: String,
    pending_ds: Option<ManagedObjectReference>,
    capacity_bytes: u64,
    thin: Option<bool>,
    mode: String,
}

/// Load full VM summary for popup display.
pub async fn fetch_vm_summary(
    client: VimClientHandle,
    vm: ManagedObjectReference,
) -> anyhow::Result<VmSummary> {
    let label = format!("{}:{}", vm.r#type.as_str(), vm.value);
    debug!(
        target: "vm_summary",
        "vm summary fetch: start vm={label}"
    );
    let retriever = ObjectRetriever::new(client.clone()).map_err(anyhow::Error::from)?;
    let mut rows = retriever
        .retrieve_objects_from_list::<VmSummaryProps>(std::slice::from_ref(&vm))
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("VM summary retrieve failed for {label}"))?;
    let mut row = rows
        .pop()
        .with_context(|| format!("VM summary: empty retrieve result for {label}"))?;

    let device_count = row.devices.as_ref().map(|d| d.len()).unwrap_or(0);
    debug!(
        target: "vm_summary",
        "vm summary fetch: retrieved vm={label} name={} config.hardware.device count={device_count}",
        row.name
    );

    let vm_id = vm.value.clone();
    let vm_name = row.name.clone();

    let guest_os = row.guest_os.clone();
    let overall_status = row.overall_status.clone();
    let power_state = row.power_state.clone();

    let uptime_seconds = row.uptime_seconds;
    let primary_ip = first_nonempty_opt(row.ip_guest.clone(), row.ip_summary.clone());

    let tools_ver_status = first_nonempty_str(
        row.tools_version_2_guest.as_deref(),
        row.tools_version_2_summary.as_deref(),
    );
    if row.tools_version_2_guest.as_deref() != row.tools_version_2_summary.as_deref() {
        debug!(
            target: "vm_summary",
            "vm summary fetch: tools versionStatus2 guest={:?} summary={:?}",
            row.tools_version_2_guest,
            row.tools_version_2_summary
        );
    }
    let guest_state_s = row.guest_state.as_deref().unwrap_or("").trim();
    let tools_line = format_vmware_tools_summary(
        guest_state_s,
        row.tools_version.as_deref(),
        tools_ver_status,
    );

    let vcpu_count = row.num_cpu;
    let memory_size_mb = row.memory_size_mb;
    let host_memory_usage_mb = row.host_memory_usage_mb;

    let disk_used_bytes = row
        .storage
        .as_ref()
        .map(|s: &VirtualMachineStorageSummary| s.committed);

    let cpu_usage_mhz = row.overall_cpu_usage_mhz;

    let host = if let Some(ref host_mor) = row.host {
        resolve_host_display(client.clone(), host_mor).await?
    } else {
        None
    };

    let guest_nics: Vec<GuestNicInfo> = row.guest_network.take().unwrap_or_default();
    let guest_nic_count = guest_nics.len();
    let mut hardware = collect_hardware_nics(&row);
    let hardware_nic_count = hardware.len();
    resolve_dv_portgroup_network_labels(client.clone(), &mut hardware).await?;
    let networking = merge_network_rows(hardware, guest_nics);
    debug!(
        target: "vm_summary",
        "vm summary fetch: merged network rows={} (hardware_nics={hardware_nic_count} guest.net entries={guest_nic_count})",
        networking.len(),
    );

    let disk_builds = collect_disk_builds(&row);
    if device_count > 0 && disk_builds.is_empty() {
        debug!(
            target: "vm_summary",
            "vm summary fetch: config.hardware has {device_count} device(s) but no virtual disks decoded (check VirtualDisk downcast path)"
        );
    }
    debug!(
        target: "vm_summary",
        "vm summary fetch: virtual disk rows={}",
        disk_builds.len()
    );
    let mut datastore_refs: Vec<ManagedObjectReference> = Vec::new();
    for d in &disk_builds {
        if let Some(ref mor) = d.pending_ds
            && !datastore_refs
                .iter()
                .any(|m| m.value == mor.value && m.r#type == mor.r#type)
        {
            datastore_refs.push(mor.clone());
        }
    }
    if !datastore_refs.is_empty() {
        debug!(
            target: "vm_summary",
            "vm summary fetch: resolving {} datastore name(s)",
            datastore_refs.len()
        );
    }
    let ds_names = resolve_datastore_names(client, &datastore_refs).await?;

    let disks: Vec<VmDiskRow> = disk_builds
        .into_iter()
        .map(|d| {
            let datastore = match &d.pending_ds {
                Some(mor) => ds_names
                    .get(&ds_key(mor))
                    .cloned()
                    .unwrap_or_else(|| format!("{} ({})", mor.r#type.as_str(), mor.value)),
                None => "-".to_string(),
            };
            VmDiskRow {
                vmdk_file: d.vmdk_file,
                datastore,
                capacity_bytes: d.capacity_bytes,
                thin: d.thin,
                mode: d.mode,
            }
        })
        .collect();

    info!(
        target: "vm_summary",
        "vm summary fetch: complete vm={label} name={} nics={} disks={} power_state={:?}",
        vm_name,
        networking.len(),
        disks.len(),
        power_state
    );

    Ok(VmSummary {
        vm_id,
        vm_name,
        overall_status,
        guest_os,
        power_state,
        uptime_seconds,
        primary_ip,
        tools_line,
        vcpu_count,
        host_memory_usage_mb,
        memory_size_mb,
        disk_used_bytes,
        cpu_usage_mhz,
        host,
        networking,
        disks,
    })
}

fn ds_key(mor: &ManagedObjectReference) -> String {
    format!("{}:{}", mor.r#type.as_str(), mor.value)
}

fn first_nonempty_opt(a: Option<String>, b: Option<String>) -> Option<String> {
    a.filter(|s| !s.trim().is_empty())
        .or_else(|| b.filter(|s| !s.trim().is_empty()))
}

fn first_nonempty_str<'a>(a: Option<&'a str>, b: Option<&'a str>) -> Option<&'a str> {
    a.filter(|s| !s.trim().is_empty())
        .or_else(|| b.filter(|s| !s.trim().is_empty()))
}

async fn resolve_host_display(
    client: VimClientHandle,
    host_mor: &ManagedObjectReference,
) -> anyhow::Result<Option<VmSummaryHost>> {
    if host_mor.r#type != MoTypesEnum::HostSystem {
        debug!(
            target: "vm_summary",
            "vm summary fetch: runtime.host mo type is {} (not HostSystem), skipping host name lookup",
            host_mor.r#type.as_str()
        );
        return Ok(None);
    }
    let retriever = ObjectRetriever::new(client).map_err(anyhow::Error::from)?;
    let mut rows = retriever
        .retrieve_objects_from_list::<HostNameProps>(std::slice::from_ref(host_mor))
        .await
        .map_err(anyhow::Error::from)?;
    let Some(row) = rows.pop() else {
        return Ok(None);
    };
    Ok(Some(VmSummaryHost {
        host_id: host_mor.value.clone(),
        host_name: row.name,
    }))
}

async fn resolve_datastore_names(
    client: VimClientHandle,
    refs: &[ManagedObjectReference],
) -> anyhow::Result<HashMap<String, String>> {
    if refs.is_empty() {
        return Ok(HashMap::new());
    }
    let retriever = ObjectRetriever::new(client).map_err(anyhow::Error::from)?;
    let rows = retriever
        .retrieve_objects_from_list::<DatastoreNameProps>(refs)
        .await
        .map_err(anyhow::Error::from)?;
    let mut map = HashMap::with_capacity(rows.len());
    for (mor, row) in refs.iter().zip(rows.into_iter()) {
        map.insert(ds_key(mor), format!("{} ({})", row.name, mor.value));
    }
    Ok(map)
}

fn collect_hardware_nics(row: &VmSummaryProps) -> Vec<HardwareNicInfo> {
    let mut out = Vec::new();
    let Some(ref devices) = row.devices else {
        return out;
    };
    for dev in devices {
        let eth: Option<&dyn VirtualEthernetCardTrait> = dev.as_ref().into_ref();
        let Some(eth) = eth else {
            continue;
        };
        let label = eth
            .device_info
            .as_ref()
            .map(|d| d.label.clone())
            .unwrap_or_else(|| format!("NIC {}", eth.key));
        let mac_hw = eth.mac_address.clone().unwrap_or_else(|| "-".to_string());
        let (network_fallback, dv_portgroup_mo_id) = network_from_eth_backing(eth);
        out.push(HardwareNicInfo {
            key: eth.key,
            label,
            mac_hw,
            network_fallback,
            dv_portgroup_mo_id,
        });
    }
    out
}

fn network_from_eth_backing(eth: &dyn VirtualEthernetCardTrait) -> (String, Option<String>) {
    let card = eth.get_virtual_ethernet_card();
    let Some(backing_box) = card.backing.as_ref() else {
        return ("-".to_string(), None);
    };
    let backing = backing_box.as_ref();
    if let Some(nb) = backing
        .as_any_ref()
        .downcast_ref::<VirtualEthernetCardNetworkBackingInfo>()
    {
        let label = nb.device_name.trim();
        if let Some(ref mor) = nb.network {
            let label = if !label.is_empty() {
                label
            } else {
                mor.r#type.as_str()
            };
            return (format!("{} ({})", label, mor.value), None);
        }
        if !label.is_empty() {
            return (label.to_string(), None);
        }
        return ("-".to_string(), None);
    }
    if let Some(op) = backing
        .as_any_ref()
        .downcast_ref::<VirtualEthernetCardOpaqueNetworkBackingInfo>()
    {
        return (label_opaque_network_backing(op), None);
    }
    if let Some(sb) = backing
        .as_any_ref()
        .downcast_ref::<VirtualSriovEthernetCardSriovBackingInfo>()
    {
        return (label_sriov_ethernet_backing(sb), None);
    }
    if let Some(dv) = backing
        .as_any_ref()
        .downcast_ref::<VirtualEthernetCardDistributedVirtualPortBackingInfo>()
    {
        let placeholder = dv_port_placeholder(&dv.port);
        let mo_id = dv
            .port
            .portgroup_key
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned();
        return (placeholder, mo_id);
    }
    ("-".to_string(), None)
}

fn label_opaque_network_backing(nb: &VirtualEthernetCardOpaqueNetworkBackingInfo) -> String {
    let id = nb.opaque_network_id.trim();
    let ty = nb.opaque_network_type.trim();
    match (id.is_empty(), ty.is_empty()) {
        (false, false) => format!("{ty}: {id}"),
        (false, true) => id.to_string(),
        (true, false) => ty.to_string(),
        (true, true) => "-".to_string(),
    }
}

fn label_sriov_ethernet_backing(sb: &VirtualSriovEthernetCardSriovBackingInfo) -> String {
    let pci = sb
        .virtual_function_backing
        .as_ref()
        .or(sb.physical_function_backing.as_ref());
    let Some(pci) = pci else {
        return "-".to_string();
    };
    let dn = pci.device_name.trim();
    if !dn.is_empty() {
        return dn.to_string();
    }
    let id = pci.id.trim();
    if !id.is_empty() {
        return format!("SR-IOV ({id})");
    }
    "-".to_string()
}

fn dv_port_placeholder(port: &DistributedVirtualSwitchPortConnection) -> String {
    if let Some(ref k) = port.portgroup_key
        && !k.is_empty()
    {
        let short: String = k.chars().take(24).collect();
        return format!("DVPG (unresolved) {short}");
    }
    "DVPG (unresolved)".to_string()
}

/// Resolve `DistributedVirtualPortgroup` names using `portgroupKey` as the MO id (vSphere encodes it
/// as `ManagedObjectReference.value` for type `DistributedVirtualPortgroup`).
async fn resolve_dv_portgroup_network_labels(
    client: VimClientHandle,
    hardware: &mut [HardwareNicInfo],
) -> anyhow::Result<()> {
    let mut ids: Vec<String> = hardware
        .iter()
        .filter_map(|h| h.dv_portgroup_mo_id.clone())
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    ids.sort();
    ids.dedup();

    let resolved = resolve_dv_portgroup_names_by_mo_id(client, &ids).await?;
    for h in hardware.iter_mut() {
        let Some(id) = h.dv_portgroup_mo_id.take() else {
            continue;
        };
        if let Some(label) = resolved.get(&id) {
            h.network_fallback = label.clone();
        }
    }
    Ok(())
}

async fn resolve_dv_portgroup_names_by_mo_id(
    client: VimClientHandle,
    mo_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    let mut out: HashMap<String, String> = HashMap::with_capacity(mo_ids.len());
    if mo_ids.is_empty() {
        return Ok(out);
    }

    let refs: Vec<ManagedObjectReference> = mo_ids
        .iter()
        .map(|id| ManagedObjectReference {
            r#type: MoTypesEnum::DistributedVirtualPortgroup,
            value: id.clone(),
        })
        .collect();

    let retriever = ObjectRetriever::new(client).map_err(anyhow::Error::from)?;
    let rows = retriever
        .retrieve_objects_from_list::<DvPortgroupNameProps>(&refs)
        .await
        .map_err(anyhow::Error::from)?;

    for (id, row) in mo_ids.iter().zip(rows.into_iter()) {
        let label = format!("{} ({})", row.name, id);
        out.insert(id.clone(), label);
    }

    Ok(out)
}

fn collect_disk_builds(row: &VmSummaryProps) -> Vec<DiskRowBuild> {
    let mut out = Vec::new();
    let Some(ref devices) = row.devices else {
        return out;
    };
    for dev in devices {
        if dev.data_type() != StructType::VirtualDisk {
            continue;
        }
        // `as_any_ref` on `&Box<dyn VirtualDeviceTrait>` targets the Box, not the inner device;
        // downcast to `VirtualDisk` would never succeed. Use the trait object reference.
        let Some(disk) = dev.as_ref().as_any_ref().downcast_ref::<VirtualDisk>() else {
            continue;
        };
        out.push(extract_disk_build(disk));
    }
    out
}

fn extract_disk_build(disk: &VirtualDisk) -> DiskRowBuild {
    let cap_bytes = disk_capacity_bytes(disk);
    let Some(ref backing) = disk.backing else {
        return DiskRowBuild {
            vmdk_file: "-".to_string(),
            pending_ds: None,
            capacity_bytes: cap_bytes,
            thin: None,
            mode: "-".to_string(),
        };
    };

    let b = backing.as_ref() as &dyn VirtualDeviceBackingInfoTrait;

    if let Some(flat) = b
        .as_any_ref()
        .downcast_ref::<VirtualDiskFlatVer2BackingInfo>()
    {
        let fi = &flat.virtual_device_file_backing_info_;
        return DiskRowBuild {
            vmdk_file: basename_vmdk(&fi.file_name),
            pending_ds: fi.datastore.clone(),
            capacity_bytes: cap_bytes,
            thin: flat.thin_provisioned,
            mode: normalize_disk_mode(&flat.disk_mode),
        };
    }
    if let Some(sp) = b
        .as_any_ref()
        .downcast_ref::<VirtualDiskSparseVer2BackingInfo>()
    {
        let fi = &sp.virtual_device_file_backing_info_;
        return DiskRowBuild {
            vmdk_file: basename_vmdk(&fi.file_name),
            pending_ds: fi.datastore.clone(),
            capacity_bytes: cap_bytes,
            thin: None,
            mode: normalize_disk_mode(&sp.disk_mode),
        };
    }
    if let Some(se) = b
        .as_any_ref()
        .downcast_ref::<VirtualDiskSeSparseBackingInfo>()
    {
        let fi = &se.virtual_device_file_backing_info_;
        return DiskRowBuild {
            vmdk_file: basename_vmdk(&fi.file_name),
            pending_ds: fi.datastore.clone(),
            capacity_bytes: cap_bytes,
            thin: None,
            mode: normalize_disk_mode(&se.disk_mode),
        };
    }
    if let Some(rdm) = b
        .as_any_ref()
        .downcast_ref::<VirtualDiskRawDiskMappingVer1BackingInfo>()
    {
        let fi = &rdm.virtual_device_file_backing_info_;
        let vmdk = rdm
            .device_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| basename_vmdk(&fi.file_name));
        let mode = rdm
            .disk_mode
            .as_deref()
            .map(normalize_disk_mode)
            .unwrap_or_else(|| "RDM".to_string());
        return DiskRowBuild {
            vmdk_file: if vmdk.is_empty() {
                "-".to_string()
            } else {
                vmdk
            },
            pending_ds: fi.datastore.clone(),
            capacity_bytes: cap_bytes,
            thin: None,
            mode,
        };
    }

    DiskRowBuild {
        vmdk_file: "-".to_string(),
        pending_ds: None,
        capacity_bytes: cap_bytes,
        thin: None,
        mode: backing.data_type().as_str().to_string(),
    }
}

fn disk_capacity_bytes(disk: &VirtualDisk) -> u64 {
    if let Some(b) = disk.capacity_in_bytes.filter(|v| *v > 0) {
        return b as u64;
    }
    let kb = disk.capacity_in_kb.max(0);
    (kb as u128).saturating_mul(1024).min(u64::MAX as u128) as u64
}

fn basename_vmdk(path: &str) -> String {
    let p = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if p.is_empty() {
        "-".to_string()
    } else {
        p.to_string()
    }
}

fn normalize_disk_mode(raw: &str) -> String {
    let s = raw.trim();
    match s {
        "persistent" | "nonpersistent" | "append" | "undoable" => "Dependent".to_string(),
        "independent_persistent" | "independent_nonpersistent" => "Independent".to_string(),
        _ => s.to_string(),
    }
}
