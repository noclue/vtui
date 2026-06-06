# Feature Specification: ESX Host Summary View

**Feature Branch**: `001-esx-host-summary-view`  
**Created**: 2026-05-06  
**Status**: Draft  
**Input**: User description: "We need ESX host summary view similar to the virtual machine summary view. I have jogged notes in /docs/esx_summary.md"

## Clarifications

### Session 2026-06-06

- Q: A local NVMe drive is enumerated by ESXi in both `config.storage_device.scsi_lun` (as `HostScsiDisk`) and `config.storage_device.nvme_topology` (as `HostNvmeNamespace`, with `name` equal to the SCSI `canonical_name`), producing duplicate disk rows. How should the disk table converge these? → A: Show SCSI LUN disks only; drop the NVMe topology source entirely for now to keep the code minimal and avoid duplicates. The disk section is labeled to indicate it lists SCSI LUNs.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open Host Summary From Host Rows (Priority: P1)

An operator browsing hosts can press `s` on the selected host row and see a scrollable summary
popup without losing the current resource browser context.

**Why this priority**: This is the core feature and matches the existing VM summary interaction model.

**Independent Test**: Can be tested by selecting a Host row, pressing `s`, observing a loading state,
then seeing host summary content while normal close/scroll keys work.

**Acceptance Scenarios**:

1. **Given** a Host resource view with a selected host, **When** the operator presses `s`, **Then**
   vtui opens a host summary loading popup and starts the fetch in the background.
2. **Given** a host summary popup is open, **When** the operator presses `Esc` or `q`, **Then** the
   popup closes and the previous resource browser view remains available.
3. **Given** a host summary popup contains more rows than fit on screen, **When** the operator uses
   arrows, `j`/`k`, Page Up/Down, `g`, `G`, or Ctrl+B/F, **Then** the popup scrolls consistently with
   the VM summary popup.

---

### User Story 2 - Inspect Host Hardware Inventory (Priority: P2)

An operator can inspect the selected host's hardware summary, physical NICs, disks, memory tiering,
and graphics devices in one popup.

**Why this priority**: Hardware inventory is the main information gap compared with the VM summary.

**Independent Test**: Can be tested with host summary data containing NICs, SCSI LUN disks, memory
tiers, and graphics devices, verifying each section renders with missing optional fields handled
gracefully.

**Acceptance Scenarios**:

1. **Given** a host with hardware summary data, **When** the summary fetch completes, **Then** the
   popup shows name, vendor/model, CPU, RAM, status, connection state, power state, uptime, CPU usage,
   and memory usage.
2. **Given** a host with physical NICs, **When** the summary renders, **Then** NIC rows show device,
   driver, MAC, link speed, and PCI data where available.
3. **Given** a host with SCSI/SATA/SAS/NVMe disks reported as SCSI LUNs, **When** the summary renders,
   **Then** disks appear in a single SCSI-LUN disk section with device, vendor, model, capacity, SSD,
   and local indicators where available, with no duplicate row per physical disk.
4. **Given** memory tiering or graphics information is absent, **When** the summary renders, **Then**
   those optional subsections are omitted rather than showing noisy empty tables.

---

### User Story 3 - Review Resident VMs on a Host (Priority: P3)

An operator can see a capped table of virtual machines running on the host, using columns that are
familiar from the resource browser.

**Why this priority**: It helps operators understand host placement and load from the host context,
but the feature remains useful with only hardware summary data.

**Independent Test**: Can be tested with hosts containing zero, fewer than 300, and more than 300 VMs.

**Acceptance Scenarios**:

1. **Given** a host with VMs, **When** the summary renders, **Then** the VM section shows ID, status,
   power state, name, guest OS, used space, CPU usage, and memory usage.
2. **Given** a host has more than 300 VMs, **When** the summary fetches VM details, **Then** vtui only
   fetches and displays the first 300 VM rows and the section header states the total count.
3. **Given** a host has no VMs, **When** the summary renders, **Then** the VM section clearly shows
   that no VMs are present.

---

### Edge Cases

- Terminal is resized while the host summary is loading or visible.
- The host is disconnected or permissions omit `config.storage_device`, `config.network`, or
  hardware summary properties.
- A local NVMe drive is reported both as a `HostScsiDisk` in `config.storage_device.scsi_lun` and as
  a namespace in `config.storage_device.nvme_topology`; the disk table must not show it twice.
- The selected host disappears or is no longer accessible before the background fetch completes.
- Inventory path resolution fails even though host summary properties are available.
- Hosts have no physical NICs, no disks, no graphics devices, no memory tiers, or no VMs.
- Optional vSphere 7.0.3+ and 8.0.0.1+ fields are absent on older ESXi/vCenter versions.
- `vcsim` omits fields that real vCenter/ESXi schemas mark required; simulator-backed tests must
  verify tolerant behavior without redefining real API expectations.
- Large host VM counts must not overfetch or allocate unbounded popup content.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST open a host summary popup with `s` when the selected row is a Host.
- **FR-002**: System MUST preserve the existing VM summary behavior when the selected row is a VM.
- **FR-003**: System MUST show `s summary` as an available action for Host rows.
- **FR-004**: System MUST fetch host summary data through the existing async ops pipeline without
  blocking input handling or redraw.
- **FR-005**: System MUST display the host inventory path in the popup title when it can be resolved.
- **FR-006**: System MUST display summary, physical NIC, disk, optional memory tiering, optional
  graphics, and resident VM sections in a scrollable dark-theme popup.
- **FR-007**: System MUST retrieve only the host properties, related VM properties, and inventory path
  data required by the popup.
- **FR-008**: System MUST cap resident VM detail retrieval at 300 VMs and show the total VM count when
  the cap is applied.
- **FR-009**: System MUST handle absent optional host properties without failing the entire popup when
  the host identity and core summary can still be shown.
- **FR-010**: System MUST surface background fetch failures through the existing error popup pattern.
- **FR-011**: System MUST include focused automated tests for row mapping, cap behavior, optional-data
  rendering, popup key handling, and resize-sensitive rendering where practical.
- **FR-012**: Simulator-backed tests MUST account for `vcsim` required-field omissions by using
  optional-path decoding or simulator-specific assertions where needed.

### Key Entities *(include if feature involves vSphere or local data)*

- **HostSummary**: UI-facing snapshot of a selected `HostSystem`, including identity, inventory path,
  health/runtime status, hardware summary, NIC rows, disk rows, optional memory tiers, optional graphics,
  resident VM rows, and total VM count.
- **HostPnicRow**: Physical NIC display row built from `config.network.pnic`.
- **HostDiskRow**: Host disk display row built from SCSI LUN disks (`config.storage_device.scsi_lun`
  downcast to `HostScsiDisk`); NVMe-topology rows are intentionally excluded to avoid duplicate rows.
- **HostMemoryTierRow**: Optional memory tier row built from `hardware.memory_tier_info`.
- **HostGraphicsRow**: Optional graphics device row built from `config.graphics_info`.
- **HostVmRow**: Resident VM display row built from capped `HostSystem.vm` references and VM summary
  properties.
- **HostSummaryUi**: Popup state machine with closed, loading, and ready states plus scroll handling.
- **PrefetchHostSummary**: Background operation request carrying request id and host reference.

### Operator Experience Requirements *(mandatory for UI changes)*

- **Actions Shown**: Host rows show `s summary`; the popup footer shows close and scroll keys.
- **Loading State**: The popup appears immediately in a loading state while the background fetch runs.
- **Error State**: Fetch failures close the loading popup and show the existing error dialog.
- **Resize Behavior**: The popup recomputes content width and scroll limits when the terminal size changes.
- **Contrast/Layers**: The popup follows the VM summary dark background, yellow border/value styling,
  visible scrollbar gutter, and readable table headers.
- **Live Update Behavior**: The summary is a point-in-time snapshot. It does not live-refresh; reopening
  the popup fetches current data.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operators can open a host summary from a Host row with the same `s` shortcut used for VM
  summary from a VM row.
- **SC-002**: Input handling, close keys, scroll keys, and terminal redraw remain responsive while host
  summary data is loading.
- **SC-003**: The implementation performs one host summary retrieve, one inventory path resolution, and
  at most one capped VM detail batch retrieve for the popup's core data.
- **SC-004**: Hosts with missing optional NIC, disk, memory tiering, graphics, or VM data render without
  panics.
- **SC-005**: `cargo fmt --check`, `cargo clippy`, and `cargo test` pass after implementation.

## Assumptions

- The existing VM summary popup is the behavioral and visual template for host summary.
- The first implementation uses static popup text with a 300 VM cap rather than virtual scrolling.
- The host summary view is available for `HostSystem` rows in the resource browser, not from arbitrary
  property browser nodes.
- The resident VM table uses point-in-time VM summary properties rather than live perf-worker samples.
- The disk table is sourced solely from `config.storage_device.scsi_lun`; ESXi reports local NVMe
  drives there as `HostScsiDisk`, so a separate NVMe-topology source is unnecessary for Phase 1 and is
  omitted to keep the code minimal and avoid duplicate rows.
