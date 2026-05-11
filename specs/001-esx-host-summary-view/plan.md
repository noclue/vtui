# Implementation Plan: ESX Host Summary View

**Branch**: `001-esx-host-summary-view` | **Date**: 2026-05-06 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-esx-host-summary-view/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add a Host summary popup, opened with `s` from Host rows, that mirrors the existing VM summary
interaction model. The implementation will add host summary data models/fetching, a background ops
request, app events, a scrollable Ratatui popup, and Host-specific action hints while preserving the
existing VM summary shortcut and async UI behavior.

## Technical Context

**Language/Version**: Rust edition 2024, Rust 1.85+ documented for source builds  
**Primary Dependencies**: Ratatui 0.30, crossterm 0.29, tokio 1.44, vim_rs 0.4.4 with `xml` and `vcsim_compat`  
**Storage**: N/A for feature data; existing platform log locations continue to be used  
**Testing**: `cargo test`, module-local unit tests, Ratatui snapshot tests with `insta`, mapping tests for fetch helpers, optional `vcsim` integration smoke tests with simulator caveats  
**Target Platform**: macOS, Windows, Linux terminals, including SSH jump hosts
**Project Type**: Rust terminal UI application  
**Performance Goals**: Popup opens loading state immediately; UI input/redraw remains responsive during vSphere I/O; VM detail retrieval capped at 300 rows  
**Constraints**: UI task must not block; retrieve only visible/needed vSphere properties; dark-theme contrast; dynamic resize support  
**Scale/Scope**: One selected HostSystem; physical NIC/disk/graphics inventory typically small; resident VM display capped at 300 of total count; no live refresh inside popup
**vSphere Data Access**: `vim_retrievable!` + `ObjectRetriever::retrieve_object` for HostSystem; capped `retrieve_objects_from_list` for resident VMs; `resolve_inventory_path` for title; use optional-path handling for simulator-fragile required fields
**Background Work**: `OperationRequest::PrefetchHostSummary` handled by ops supervisor task; success/failure returned through `AppEvent`
**Operator UX**: Host rows show `s summary`; popup supports loading, close, scroll, resize recomputation, dark-theme contrast, and existing error popup failures

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

*GATE: Each item must be PASS or explicitly justified in Complexity Tracking.*

- **Terminal-native operator UX**: PASS. Host rows add `s summary`; popup footer lists close and
  scroll controls.
- **Responsive async UI**: PASS. Fetch runs through the existing ops supervisor; UI state receives
  `HostSummarySucceeded`/`HostSummaryFailed`.
- **Minimal retrieval and live state**: PASS. Host paths are limited to summary/runtime,
  `config.network.pnic`, `config.storage_device.scsi_lun`, `config.storage_device.nvme_topology`,
  `hardware.memory_tier_*`, `config.graphics_info`, and `vm`; VM detail retrieval is capped at 300.
  The popup is a point-in-time snapshot and does not poll.
- **Cross-platform remote readiness**: PASS. No new platform paths or terminal-specific APIs are
  introduced; popup uses existing crossterm/Ratatui input and resize handling.
- **Tested Rust quality gates**: PASS. Plan includes mapping, cap, optional-data, key handling, and
  rendering tests plus final cargo validation.
- **Dark-theme accessibility**: PASS. Host summary reuses VM summary popup colors and styles, with
  explicit attention to table headers, popup background, border, scrollbar, and values.
- **vSphere API correctness**: PASS. Retrieval uses `vim_retrievable!`, `ObjectRetriever`, trait
  downcasting for polymorphic SCSI LUNs, and batch VM retrieval; no ad hoc PropertyCollector specs.
  `vcsim` omissions are handled as simulator compatibility concerns, not as proof that real API
  required fields are optional.

## Project Structure

### Documentation (this feature)

```text
specs/001-esx-host-summary-view/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
```text
src/
├── app.rs                         # host summary event handling, draw, modal input routing
├── event.rs                       # host summary AppEvent variants
├── main.rs                        # host_summary and host_summary_ui module declarations
├── ops/
│   ├── supervisor.rs              # PrefetchHostSummary background task
│   └── types.rs                   # OperationRequest variant
├── host_summary/
│   ├── mod.rs                     # HostSummary and row structs + helper tests
│   └── fetch.rs                   # vim_rs retrieval and row mapping
├── host_summary_ui.rs             # loading/ready popup, scrolling, rendering
└── resource_browser/
    ├── hints.rs                   # Host `s summary` hint
    └── resource_mgr.rs            # dispatch `s` on Host rows

Module-local tests and snapshots
├── src/host_summary/mod.rs        # row merge/sort/cap helper tests
├── src/host_summary/fetch.rs      # pure mapping helper tests where extraction is separated
├── src/host_summary_ui.rs         # key handling and rendering snapshots
└── existing resource browser tests updated if hints/table output changes
```

**Structure Decision**: Follow the existing VM summary architecture with parallel Host-specific
modules. Avoid broad refactors unless small helper extraction from `vm_summary_ui.rs` materially
reduces duplication without changing behavior.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | N/A |

## Phase 0: Research

Research is captured in [research.md](./research.md). Key decisions:

- Reuse VM summary async popup architecture.
- Use `vim_retrievable!` and `ObjectRetriever` for HostSystem and capped VM retrieval.
- Use `vcsim` integration tests for smoke coverage, with pure typed fixtures for field-level behavior
  that the simulator cannot represent reliably.
- Render a static `Text` body initially, capped at 300 VM rows.
- Treat NVMe topology as best-effort Phase 1 with fallback to SCSI-only if macro depth causes issues.

## Phase 1: Design

Design artifacts:

- [data-model.md](./data-model.md)
- [contracts/host-summary-ui.md](./contracts/host-summary-ui.md)
- [quickstart.md](./quickstart.md)

## Post-Design Constitution Check

- **Terminal-native operator UX**: PASS. Contract defines Host `s summary` hint and popup footer keys.
- **Responsive async UI**: PASS. Data model and contract use `HostSummaryUi` loading/ready state and
  ops-supervisor events.
- **Minimal retrieval and live state**: PASS. Data model lists exact properties and VM cap; popup is
  explicitly point-in-time.
- **Cross-platform remote readiness**: PASS. Quickstart includes terminal resize and SSH-friendly key
  validation; no new filesystem paths.
- **Tested Rust quality gates**: PASS. Quickstart includes `cargo fmt --check`, `cargo clippy`, and
  `cargo test`; plan calls for focused tests.
- **Dark-theme accessibility**: PASS. Contract requires VM summary style reuse and contrast checks for
  layered popup/table states.
- **vSphere API correctness**: PASS. Research confirms `vim_rs` retriever patterns and polymorphic
  SCSI LUN handling, plus explicit `vcsim` required-field caveats and optional-path handling.
