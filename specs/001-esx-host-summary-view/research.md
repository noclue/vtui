# Research: ESX Host Summary View

## Decision: Reuse the VM Summary Async Popup Architecture

**Decision**: Implement Host summary as a parallel flow to VM summary: `AppEvent::OpenHostSummary`,
`OperationRequest::PrefetchHostSummary`, ops-supervisor background fetch, `HostSummaryUi` loading and
ready states, and a `render(frame)` call from `App::draw`.

**Rationale**: The existing VM summary path already satisfies vtui's responsiveness rule: key input
and redraw remain in the UI task while vSphere work happens in background ops. Reusing the same state
shape keeps close/scroll behavior and error handling predictable.

**Alternatives considered**:

- Fetch directly from the resource browser key handler: rejected because it would block UI input/redraw.
- Generalize VM and Host summaries into one summary framework first: deferred because it increases
  scope and risks changing shipped VM summary behavior.

## Decision: Use `vim_retrievable!` and `ObjectRetriever`

**Decision**: Define Host and VM retrievable structs and use `ObjectRetriever::retrieve_object` for
the selected HostSystem plus `retrieve_objects_from_list` for capped resident VMs. Properties that
`vcsim` or older endpoints commonly omit must use `vim_retrievable!`'s optional-path suffix or
optional Rust fields, even when the vSphere schema marks them required.

**Rationale**: This follows documented `vim_rs` PropertyCollector patterns, avoids manual specs, and
keeps retrieval to the properties used by the popup. `vcsim` is valuable for automation but often
violates real vCenter/ESXi required-field contracts by leaving required fields blank, so tests must
exercise tolerant decoding without treating simulator omissions as authoritative API behavior.

**Alternatives considered**:

- Managed-object property accessor calls: rejected because multiple per-field calls would overfetch
  round trips and complicate error handling.
- Manual PropertyCollector specs: rejected as an anti-pattern for this codebase and `vim_rs`.

## Decision: Use `vcsim` for Integration Tests With Explicit Caveats

**Decision**: Add simulator-backed tests for the host summary fetch path where practical, but keep
pure mapping/rendering tests as the primary assertions for fields that `vcsim` omits or mis-shapes.

**Rationale**: `vcsim` can provide automated end-to-end coverage for connection setup, PropertyCollector
retrieval, async fetch plumbing, and broad no-panic behavior. It is not a perfect contract oracle:
the simulator can omit fields that real vCenter/ESXi marks required, return sparse facade objects, or
leave optional nested structures empty. The implementation should therefore prefer tolerant optional
retrieval for simulator-fragile paths and separately test real expected mappings with synthetic typed
fixtures.

**Alternatives considered**:

- Require real vCenter/ESXi for integration tests: rejected for normal CI because credentials and
  infrastructure are not portable.
- Treat `vcsim` output as the only contract: rejected because it can encode simulator quirks rather
  than product behavior.
- Skip simulator tests entirely: rejected because they still catch transport, auth, and retrieval
  regressions cheaply.

## Decision: Point-In-Time Snapshot, No Live Refresh

**Decision**: Host summary is a point-in-time popup. Reopening the popup fetches current data; no
polling is added inside the popup.

**Rationale**: The popup is meant for inspection, and a static snapshot avoids stale selection issues
and extra API load. This also matches the notes for static VM stats in the resident VM table.

**Alternatives considered**:

- Poll while the popup is open: rejected for Phase 1 because it adds refresh cadence, stale-data, and
  selection-preservation complexity without being required for a summary view.

## Decision: Static Text Rendering With 300 VM Cap

**Decision**: Build a static `Text<'static>` body like `VmSummaryUi`, and cap the resident VM detail
fetch/display at 300 rows.

**Rationale**: The cap bounds memory and rendering cost while preserving the simple, known scroll
implementation. Hardware sections are expected to be small.

**Alternatives considered**:

- Virtual scrolling: deferred until there is evidence that capped static rendering is insufficient.
- Fetch all resident VMs: rejected because it can overfetch on dense hosts and violates the bounded
  retrieval requirement.

## Decision: Disk Rows Converge SCSI LUNs and NVMe Namespaces

**Decision**: Build one `HostDiskRow` list from `config.storage_device.scsi_lun` downcast to
`HostScsiDisk` plus `config.storage_device.nvme_topology` namespaces where retrievable.

**Rationale**: The vSphere API exposes SCSI/SATA/SAS/iSCSI/FC disks and NVMe topology separately, but
operators need one disk table. Application-level convergence is the clearest display model.

**Alternatives considered**:

- Show only `scsi_lun`: acceptable fallback if NVMe decoding fails, but not the target behavior.
- Separate SCSI and NVMe sections: rejected because the operator-facing columns overlap and a unified
  inventory is easier to scan.

## Decision: Optional Memory Tiering and Graphics Subsections

**Decision**: Include memory tiering and graphics as compact optional subsections when data is
present; omit them when absent.

**Rationale**: These properties are useful for ESXi host inspection but are version/hardware dependent.
Omitting empty subsections keeps the popup minimalist.

**Alternatives considered**:

- Always render empty sections: rejected because it adds noise.
- Defer both fields: rejected because the investigation identifies direct data sources and small row
  counts suitable for Phase 1.

## Decision: Inventory Path Title Best Effort

**Decision**: Resolve inventory path in the background fetch and use it in the title. If resolution
fails, the fetch should either include contextual error handling or fall back to host name/id if a
small helper can do so without hiding the error unexpectedly.

**Rationale**: The desired title style matches VM actions and helps operators orient the popup. Path
resolution is a separate async call and must not happen in rendering.

**Alternatives considered**:

- Title by host name only: simpler but less useful in large inventories with duplicate host names.
- Resolve path in the UI layer: rejected because rendering must not perform I/O.
