# Tasks: ESX Host Summary View

**Input**: Design documents from `/specs/001-esx-host-summary-view/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/host-summary-ui.md, quickstart.md

**Tests**: Tests are REQUIRED by the feature specification. Each user story includes focused tests before implementation tasks.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create host summary module entry points and reserve integration locations without changing behavior.

- [X] T001 Create host summary module files `src/host_summary/mod.rs` and `src/host_summary/fetch.rs`
- [X] T002 Create host summary UI module skeleton in `src/host_summary_ui.rs`
- [X] T003 Register `host_summary` and `host_summary_ui` modules in `src/main.rs`
- [X] T004 [P] Review existing VM summary rendering helpers for reuse points in `src/vm_summary_ui.rs`
- [X] T005 [P] Review existing VM summary fetch patterns and optional-path usage in `src/vm_summary/fetch.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define shared types and plumbing that all user stories depend on.

**CRITICAL**: No user story work can begin until this phase is complete.

- [X] T006 Define `HostSummary`, `HostPnicRow`, `HostDiskRow`, `HostMemoryTierRow`, `HostGraphicsRow`, `HostVmRow`, and `HostDiskSource` structs in `src/host_summary/mod.rs`
- [X] T007 Define `HostSummaryUi`, `HostSummaryLayer`, and `HostSummaryKeyOutcome` skeletons in `src/host_summary_ui.rs`
- [X] T008 Add `OpenHostSummary`, `HostSummarySucceeded`, and `HostSummaryFailed` event variants in `src/event.rs`
- [X] T009 Add `PrefetchHostSummary` operation request variant in `src/ops/types.rs`
- [X] T010 Add `fetch_host_summary` public export and placeholder function signature in `src/host_summary/mod.rs` and `src/host_summary/fetch.rs`
- [X] T011 [P] Define shared constants for host summary VM cap and log target in `src/host_summary/mod.rs`

**Checkpoint**: Foundation ready; host summary types compile as stubs and user story work can proceed.

---

## Phase 3: User Story 1 - Open Host Summary From Host Rows (Priority: P1) MVP

**Goal**: Operators can press `s` on Host rows, see a loading host summary popup, receive background fetch results, scroll/close the popup, and preserve existing VM summary behavior.

**Independent Test**: Select a Host row, press `s`, observe loading state, then show a minimal host summary while close and scroll keys work.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T012 [P] [US1] Add `HostSummaryUi` loading/ready close and scroll key tests in `src/host_summary_ui.rs`
- [X] T013 [P] [US1] Add host summary popup rendering snapshot tests for loading and minimal ready states in `src/host_summary_ui.rs`
- [X] T014 [P] [US1] Add host `s summary` hint test coverage in `src/resource_browser/hints.rs`
- [X] T015 [P] [US1] Add stale request id handling tests for `HostSummaryUi::pending_matches` and success application in `src/host_summary_ui.rs`

### Implementation for User Story 1

- [X] T016 [US1] Implement `HostSummaryUi::start_loading`, `close`, `pending_matches`, `apply_success`, and `handle_key` in `src/host_summary_ui.rs`
- [X] T017 [US1] Implement minimal host summary popup rendering with loading, title, footer, scroll, scrollbar, and resize-sensitive content rebuild in `src/host_summary_ui.rs`
- [X] T018 [US1] Add Host `s summary` action hint while preserving VM hints in `src/resource_browser/hints.rs`
- [X] T019 [US1] Dispatch `AppEvent::OpenHostSummary` on `s` when `ResourceType::Host` is selected in `src/resource_browser/resource_mgr.rs`
- [X] T020 [US1] Add `host_summary_ui` field initialization, modal input handling, and render call in `src/app.rs`
- [X] T021 [US1] Handle `OpenHostSummary`, `HostSummarySucceeded`, and `HostSummaryFailed` in `src/app.rs`
- [X] T022 [US1] Handle `OperationRequest::PrefetchHostSummary` in `src/ops/supervisor.rs`
- [X] T023 [US1] Implement minimal `HostSummaryProps` retrieval for host identity, status, runtime, summary hardware, quick stats, and inventory path in `src/host_summary/fetch.rs`
- [X] T024 [US1] Add contextual debug/warn logging for host summary open, fetch start, success, failure, and stale responses in `src/app.rs`, `src/ops/supervisor.rs`, and `src/host_summary/fetch.rs`

**Checkpoint**: US1 is functional and testable independently with a minimal host summary popup.

---

## Phase 4: User Story 2 - Inspect Host Hardware Inventory (Priority: P2)

**Goal**: Operators can inspect host hardware summary, physical NICs, disks, optional memory tiering, and optional graphics devices in the popup.

**Independent Test**: Render a host summary containing NICs, SCSI disks, optional NVMe rows, memory tiers, and graphics rows while missing optional fields render gracefully.

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T025 [P] [US2] Add physical NIC mapping and natural sort tests in `src/host_summary/mod.rs`
- [X] T026 [P] [US2] Add SCSI disk downcast/mapping helper tests with typed fixtures in `src/host_summary/fetch.rs`
- [X] T027 [P] [US2] Add optional memory tiering and graphics omission/rendering tests in `src/host_summary_ui.rs`
- [X] T028 [P] [US2] Add hardware-rich host summary rendering snapshot test in `src/host_summary_ui.rs`
- [X] T029 [P] [US2] Add optional-path tolerance tests for simulator-fragile host properties in `src/host_summary/fetch.rs`

### Implementation for User Story 2

- [X] T030 [US2] Extend `HostSummaryProps` with `config.network.pnic`, `config.storage_device.scsi_lun`, `config.storage_device.nvme_topology`, `hardware.memory_tiering_type`, `hardware.memory_tier_info`, and `config.graphics_info` in `src/host_summary/fetch.rs`
- [X] T031 [US2] Implement physical NIC row extraction and formatting-ready fields in `src/host_summary/fetch.rs`
- [X] T032 [US2] Implement SCSI LUN to `HostScsiDisk` downcasting and checked capacity mapping in `src/host_summary/fetch.rs`
- [X] T033 [US2] Implement best-effort NVMe topology extraction with graceful fallback when absent or undecodable in `src/host_summary/fetch.rs`
- [X] T034 [US2] Implement memory tier and graphics row extraction with omission of empty sections in `src/host_summary/fetch.rs`
- [X] T035 [US2] Render Summary, Memory Tiering, Graphics, Physical NICs, and Disks sections in `src/host_summary_ui.rs`
- [X] T036 [US2] Reuse existing byte, memory, CPU, and status formatting helpers from `src/resource_browser/formatting.rs` in `src/host_summary_ui.rs`
- [X] T037 [US2] Ensure missing optional hardware fields render placeholders or omitted sections without failing the popup in `src/host_summary/fetch.rs` and `src/host_summary_ui.rs`

**Checkpoint**: US1 and US2 work independently; host summary is useful for hardware inspection even without resident VM details.

---

## Phase 5: User Story 3 - Review Resident VMs on a Host (Priority: P3)

**Goal**: Operators can review a capped table of resident VMs using familiar resource-browser-like columns.

**Independent Test**: Hosts with zero VMs, fewer than 300 VMs, and more than 300 VMs render correct VM section content and cap messaging.

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T038 [P] [US3] Add VM cap helper tests for zero, under-cap, and over-cap host VM lists in `src/host_summary/mod.rs`
- [X] T039 [P] [US3] Add `HostVmInfo` optional quick-stats mapping tests with typed fixtures in `src/host_summary/fetch.rs`
- [X] T040 [P] [US3] Add VM section rendering snapshot tests for empty, under-cap, and `Showing 300 of N` states in `src/host_summary_ui.rs`
- [X] T041 [P] [US3] Add optional `vcsim` smoke test ignored by default for `fetch_host_summary` in `src/host_summary/fetch.rs`

### Implementation for User Story 3

- [X] T042 [US3] Extend `HostSummaryProps` with `vm` references and define `HostVmInfo` retrievable properties in `src/host_summary/fetch.rs`
- [X] T043 [US3] Implement resident VM reference capping at 300 and total count tracking in `src/host_summary/fetch.rs`
- [X] T044 [US3] Implement capped `ObjectRetriever::retrieve_objects_from_list::<HostVmInfo>` batch retrieval in `src/host_summary/fetch.rs`
- [X] T045 [US3] Map resident VM rows to `HostVmRow` with optional-path tolerant quick-stats, guest, storage, and runtime fields in `src/host_summary/fetch.rs`
- [X] T046 [US3] Render the Virtual Machines section with ID, status, power, name, guest OS, used space, CPU usage, memory usage, and cap header in `src/host_summary_ui.rs`
- [X] T047 [US3] Ensure VM summary and host summary `s` behavior both remain correct in `src/resource_browser/resource_mgr.rs` and `src/resource_browser/hints.rs`

**Checkpoint**: All user stories are independently functional; host summary includes hardware and capped resident VM context.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validate quality gates, documentation, simulator caveats, and operator experience across all stories.

- [X] T048 [P] Update feature documentation notes if implementation decisions differ from the plan in `docs/esx_summary.md`
- [X] T049 [P] Add or update README feature bullet for Host summary behavior in `README.md`
- [X] T050 [P] Add release-facing manual validation notes for Host summary in `specs/001-esx-host-summary-view/quickstart.md`
- [X] T051 Verify dark-theme contrast, popup layering, scrollbar gutter, and table readability in `src/host_summary_ui.rs`
- [X] T052 Verify terminal resize behavior and SSH-friendly key handling for Host summary in `src/host_summary_ui.rs`
- [X] T053 Run optional local `vcsim` smoke validation and record any simulator-specific optional-path gaps in `specs/001-esx-host-summary-view/quickstart.md`
- [X] T054 Run `cargo fmt --check` for the workspace using `Cargo.toml`
- [X] T055 Run `cargo clippy --all-targets -- -D warnings` for the workspace using `Cargo.toml`
- [X] T056 Run `cargo test` for the workspace using `Cargo.toml`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion; blocks all user stories.
- **US1 (Phase 3)**: Depends on Foundational; delivers MVP host summary open/loading/minimal popup.
- **US2 (Phase 4)**: Depends on Foundational and benefits from US1 UI shell; can be developed after US1 tests define popup behavior.
- **US3 (Phase 5)**: Depends on Foundational and can proceed after US1 fetch/UI shell exists.
- **Polish (Phase 6)**: Depends on completed desired user stories.

### User Story Dependencies

- **US1 (P1)**: MVP; required before a user can open any host summary popup.
- **US2 (P2)**: Extends the host summary payload and rendering; depends on US1 popup shell for visible validation.
- **US3 (P3)**: Extends host summary with resident VM rows; depends on US1 popup shell and shared `HostSummary` model.

### Within Each User Story

- Write tests first and verify they fail before implementation.
- Implement data/model mapping before rendering rows that consume it.
- Keep vSphere retrieval in `src/host_summary/fetch.rs`; keep rendering in `src/host_summary_ui.rs`.
- Preserve async UI behavior: no vSphere I/O in `src/app.rs`, `src/resource_browser/resource_mgr.rs`, or rendering code.
- Complete and validate each story checkpoint before moving to lower-priority stories.

### Parallel Opportunities

- Setup review tasks T004 and T005 can run in parallel.
- Foundational constants task T011 can run in parallel after skeleton files exist.
- US1 tests T012-T015 can be written in parallel.
- US2 tests T025-T029 can be written in parallel.
- US3 tests T038-T041 can be written in parallel.
- Documentation tasks T048-T050 can run in parallel after implementation decisions settle.

---

## Parallel Example: User Story 1

```bash
Task: "T012 [P] [US1] Add HostSummaryUi loading/ready close and scroll key tests in src/host_summary_ui.rs"
Task: "T013 [P] [US1] Add host summary popup rendering snapshot tests for loading and minimal ready states in src/host_summary_ui.rs"
Task: "T014 [P] [US1] Add host s summary hint test coverage in src/resource_browser/hints.rs"
Task: "T015 [P] [US1] Add stale request id handling tests in src/host_summary_ui.rs"
```

## Parallel Example: User Story 2

```bash
Task: "T025 [P] [US2] Add physical NIC mapping and natural sort tests in src/host_summary/mod.rs"
Task: "T026 [P] [US2] Add SCSI disk downcast/mapping helper tests in src/host_summary/fetch.rs"
Task: "T027 [P] [US2] Add optional memory tiering and graphics omission/rendering tests in src/host_summary_ui.rs"
Task: "T029 [P] [US2] Add optional-path tolerance tests in src/host_summary/fetch.rs"
```

## Parallel Example: User Story 3

```bash
Task: "T038 [P] [US3] Add VM cap helper tests in src/host_summary/mod.rs"
Task: "T039 [P] [US3] Add HostVmInfo optional quick-stats mapping tests in src/host_summary/fetch.rs"
Task: "T040 [P] [US3] Add VM section rendering snapshot tests in src/host_summary_ui.rs"
Task: "T041 [P] [US3] Add optional vcsim smoke test ignored by default in src/host_summary/fetch.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 and Phase 2.
2. Complete Phase 3.
3. Validate Host rows show `s summary`, pressing `s` opens loading state, minimal summary data renders, close/scroll keys work, and VM summary still opens from VM rows.
4. Stop and verify US1 independently before adding hardware and VM tables.

### Incremental Delivery

1. US1: Host summary shell, async fetch, minimal summary.
2. US2: Hardware sections with tolerant optional-property handling.
3. US3: Resident VM table with capped batch retrieval.
4. Polish: docs, manual checks, optional `vcsim`, cargo validation.

### Test Strategy

1. Prefer pure unit and rendering snapshot tests for deterministic behavior.
2. Use typed fixtures for field-level mappings that `vcsim` cannot represent reliably.
3. Use `vcsim` only as optional smoke coverage for connection/retrieval/no-panic behavior.
4. Keep cargo validation as the final merge gate.

---

## Notes

- `vcsim` can omit fields that real vCenter/ESXi marks required; do not treat simulator omissions as real API contract changes.
- Use `vim_retrievable!` optional-path suffixes or optional Rust fields for simulator-fragile properties.
- Do not add live refresh to the Host summary popup in this feature; it is a point-in-time snapshot.
- Do not fetch more than 300 resident VM detail rows for one host summary popup.
- Avoid refactoring VM summary unless a very small helper extraction reduces duplication without changing VM behavior.
