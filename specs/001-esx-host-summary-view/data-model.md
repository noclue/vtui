# Data Model: ESX Host Summary View

## HostSummary

UI-facing snapshot for one selected `HostSystem`.

**Fields**:

- `host_id: String`: managed object id value.
- `host_name: String`: host display name from `name`.
- `inventory_path: String`: resolved inventory path for popup title.
- `overall_status: ManagedEntityStatusEnum`: host health/status dot.
- `connection_state: HostSystemConnectionStateEnum`: runtime connection state.
- `power_state: HostSystemPowerStateEnum`: runtime power state.
- `uptime_seconds: Option<i32>`: `summary.quick_stats.uptime`.
- `cpu_usage_mhz: Option<i32>`: `summary.quick_stats.overall_cpu_usage`.
- `memory_usage_mb: Option<i32>`: `summary.quick_stats.overall_memory_usage`.
- `hw_vendor: Option<String>`: `summary.hardware.vendor`.
- `hw_model: Option<String>`: `summary.hardware.model`.
- `hw_cpu_model: Option<String>`: `summary.hardware.cpu_model`.
- `hw_cpu_mhz: Option<i32>`: `summary.hardware.cpu_mhz`.
- `hw_num_cpu_pkgs: Option<i16>`: `summary.hardware.num_cpu_pkgs`.
- `hw_num_cpu_cores: Option<i16>`: `summary.hardware.num_cpu_cores`.
- `hw_num_cpu_threads: Option<i16>`: `summary.hardware.num_cpu_threads`.
- `hw_memory_size_bytes: Option<i64>`: `summary.hardware.memory_size`.
- `nics: Vec<HostPnicRow>`: physical network adapters.
- `disks: Vec<HostDiskRow>`: converged disk table.
- `memory_tiers: Vec<HostMemoryTierRow>`: optional memory tier rows.
- `graphics: Vec<HostGraphicsRow>`: optional graphics device rows.
- `vms: Vec<HostVmRow>`: capped resident VM rows.
- `total_vm_count: usize`: total resident VM reference count before cap.

**Validation and state rules**:

- `host_id` and `host_name` must be present for a successful summary.
- Optional hardware/runtime properties render as `-` or are omitted according to section rules.
- `vms.len()` must be `<= 300`.
- `total_vm_count >= vms.len()`.

## HostPnicRow

Physical NIC row from `config.network.pnic`.

**Fields**:

- `device: String`: device name, e.g. `vmnic0`.
- `driver: Option<String>`: driver name.
- `driver_version: Option<String>`: driver version when available.
- `firmware_version: Option<String>`: firmware version when available.
- `mac: String`: MAC address.
- `link_speed_mbps: Option<i32>`: link speed in Mb/s.
- `duplex: Option<bool>`: full-duplex indicator when available.
- `pci: String`: PCI id.
- `wake_on_lan_supported: bool`: WOL capability.

**Validation and state rules**:

- Sort by natural device order where practical, falling back to lexical order.
- Missing optional driver/speed fields render as `-`.

## HostDiskRow

Unified disk row from SCSI LUNs and NVMe namespaces.

**Fields**:

- `device_name: String`: SCSI device name or NVMe controller/namespace label.
- `vendor: Option<String>`: disk or controller vendor.
- `model: Option<String>`: disk or controller model.
- `capacity_bytes: Option<u64>`: capacity from block count and block size.
- `ssd: Option<bool>`: SSD flag; NVMe defaults to `Some(true)`.
- `local: Option<bool>`: local disk flag; NVMe defaults to `Some(true)` unless better data exists.
- `source: HostDiskSource`: `Scsi` or `Nvme`.

**Validation and state rules**:

- Include only SCSI LUN rows that downcast to `HostScsiDisk`.
- Capacity multiplication must be checked or saturating to avoid panic on unexpected values.
- Sort stable by source and device name unless implementation finds a better existing table convention.

## HostMemoryTierRow

Optional memory tier row from `hardware.memory_tier_info`.

**Fields**:

- `name: String`: tier name.
- `tier_type: String`: tier type such as `dram`, `pmem`, or `nvdimm`.
- `size_bytes: i64`: tier size.
- `flags: Vec<String>`: optional flags.

**Validation and state rules**:

- Render the section only when tiering type is not `none` and at least one row exists.

## HostGraphicsRow

Optional graphics device row from `config.graphics_info`.

**Fields**:

- `device_name: String`: graphics device name.
- `vendor_name: String`: vendor name.
- `pci_id: String`: PCI id.
- `graphics_type: String`: graphics type, e.g. `sharedPassthru`.
- `memory_size_kb: i64`: VRAM in KB.
- `vgpu_mode: Option<String>`: vGPU mode if provided.
- `attached_vm_count: usize`: count of associated VM references.

**Validation and state rules**:

- Render the section only when rows exist.
- VRAM display converts from KB to bytes for existing byte formatting helpers.

## HostVmRow

Resident VM row from capped `HostSystem.vm` references and VM properties.

**Fields**:

- `vm_id: String`: VM managed object id.
- `vm_name: String`: VM display name.
- `overall_status: ManagedEntityStatusEnum`: VM status dot.
- `power_state: VirtualMachinePowerStateEnum`: VM power indicator.
- `guest_os: Option<String>`: guest OS display string.
- `storage_used_bytes: Option<i64>`: `summary.storage.committed`.
- `cpu_usage_mhz: Option<i32>`: `summary.quick_stats.overall_cpu_usage`.
- `memory_usage_mb: Option<i32>`: `summary.quick_stats.host_memory_usage`.

**Validation and state rules**:

- Fetch at most the first 300 VM refs.
- Preserve original host VM ref order unless existing resource browser ordering is easy to reuse.

## HostSummaryUi

Popup state machine.

**States**:

- `Closed`: no popup.
- `Loading { request_id }`: popup visible, fetch in progress.
- `Ready { summary, scroll, text, content_width, viewport_height }`: rendered summary.

**Transitions**:

- `Closed -> Loading`: `OpenHostSummary` creates request id and queues `PrefetchHostSummary`.
- `Loading -> Ready`: matching `HostSummarySucceeded`.
- `Loading -> Closed`: matching `HostSummaryFailed` or close key.
- `Ready -> Closed`: close key.
- `Ready -> Ready`: scroll keys or resize-triggered content rebuild.

## HostSummaryProps

Internal `vim_retrievable!` struct for `HostSystem`.

Fields that are required in the vSphere schema may still need optional-path decoding when `vcsim`
omits them. Treat this as simulator compatibility: keep the real API meaning in the model, but make
retrieval tolerant enough that one missing simulator field does not discard the entire host summary.

**Property paths**:

- `name`
- `overall_status`
- `runtime.connection_state`
- `runtime.power_state`
- `summary.quick_stats.uptime`
- `summary.quick_stats.overall_cpu_usage`
- `summary.quick_stats.overall_memory_usage`
- `summary.hardware.vendor`
- `summary.hardware.model`
- `summary.hardware.cpu_model`
- `summary.hardware.cpu_mhz`
- `summary.hardware.num_cpu_pkgs`
- `summary.hardware.num_cpu_cores`
- `summary.hardware.num_cpu_threads`
- `summary.hardware.memory_size`
- `hardware.memory_tiering_type`
- `hardware.memory_tier_info`
- `config.network.pnic`
- `config.storage_device.scsi_lun`
- `config.storage_device.nvme_topology`
- `config.graphics_info`
- `vm`

## HostVmInfo

Internal `vim_retrievable!` struct for resident `VirtualMachine` rows.

Use optional-path decoding for quick-stats, guest, storage, or runtime fields that `vcsim` or older
endpoints can omit. VM rows with missing optional display fields should render placeholders rather
than failing the host summary fetch.

**Property paths**:

- `name`
- `overall_status`
- `runtime.power_state`
- `summary.guest.guest_full_name`
- `summary.storage`
- `summary.quick_stats.overall_cpu_usage`
- `summary.quick_stats.host_memory_usage`
