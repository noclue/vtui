# Tasks: VM Resource Browser Responsive Columns

**Input**: Design documents from `/specs/002-vm-responsive-columns/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/vm-responsive-columns-ui.md, quickstart.md

**Tests**: Tests are REQUIRED by the feature specification and constitution. Each user story includes focused tests before implementation tasks.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create module entry points and confirm integration points without changing behavior.

- [X] T001 Create `src/resource_browser/vm_layout.rs` module skeleton and declare `mod vm_layout;` in `src/resource_browser/mod.rs`
- [X] T002 Review `ResourceTableWidget::render` width path, `block.inner(area)`, highlight symbol, and Ratatui default `column_spacing` in `src/resource_browser/resource_table.rs`
- [X] T003 [P] Review `TableDataSource` and `IndexedCache` column/header/row flow in `src/resource_browser/tabular_data.rs` and `src/resource_browser/indexed_cache.rs`
- [X] T004 [P] Review current VM column constants and row cell order in `src/resource_browser/vm.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared layout types and tier calculator that all user stories depend on.

**CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 Define `ColumnLayout { visible_indices, constraints }` and default `column_layout(columns_budget: u16)` on `TableDataSource` in `src/resource_browser/tabular_data.rs`
- [X] T006 Define `VmColumn`, `VmLayoutTier`, and width constants (`VM_ID_COLUMN_WIDTH`, `VM_NAME_MIN_WIDTH`, `TABLE_HIGHLIGHT_PREFIX_WIDTH`, `TABLE_COLUMN_SPACING`) in `src/resource_browser/vm_layout.rs`
- [X] T007 Implement `tier_fits(columns_budget, tier)` with `(visible_cols - 1) * TABLE_COLUMN_SPACING` gap math in `src/resource_browser/vm_layout.rs`
- [X] T008 Implement `vm_column_layout(columns_budget: u16) -> ColumnLayout` tier walk (tiers 0–6) in `src/resource_browser/vm_layout.rs`
- [X] T009 [P] Add `project_row` and `project_header` helpers (filter cells/labels by `visible_indices`) in `src/resource_browser/tabular_data.rs`
- [X] T010 Override `column_layout` in `IndexedCache` to delegate to `vm_column_layout` when `T::resource_type() == ResourceType::VirtualMachine` in `src/resource_browser/indexed_cache.rs`

**Checkpoint**: `vm_column_layout` unit-testable in isolation; non-VM sources still return all columns via default `column_layout`.

---

## Phase 3: User Story 1 - Identify VMs in a Narrow Terminal (Priority: P1) MVP

**Goal**: At narrow widths, only the Name column is visible with a 20-character minimum; resize and selection behave correctly.

**Independent Test**: Render VM table at `columns_budget` 20–32; only Name column and header appear; scrolling and selection work.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T011 [P] [US1] Add tier 0–1 boundary tests (`columns_budget` 19, 20, 32, 33) in `src/resource_browser/vm_layout.rs`
- [X] T012 [P] [US1] Add sub-20 fallback test (Name uses all `columns_budget`) in `src/resource_browser/vm_layout.rs`
- [X] T013 [P] [US1] Add VM table name-only rendering snapshot at narrow width in `src/resource_browser/resource_table.rs`

### Implementation for User Story 1

- [X] T014 [US1] Wire `columns_budget = block.inner(area).width - TABLE_HIGHLIGHT_PREFIX_WIDTH` and `column_layout` into `ResourceTableWidget::render` in `src/resource_browser/resource_table.rs`
- [X] T015 [US1] Project header and data rows through `visible_indices` before `Table::new` in `src/resource_browser/resource_table.rs`
- [X] T016 [US1] Implement tier 0–1 constraints (Name `Min(20)` / `Length(budget)` fallback; tier 1 adds `Length(12)` ID) in `src/resource_browser/vm_layout.rs`

**Checkpoint**: US1 functional — narrow terminal shows Name only; widening to `columns_budget` 33 shows ID + Name.

---

## Phase 4: User Story 2 - Progressive Column Reveal as Width Grows (Priority: P1)

**Goal**: Columns appear in order ID → S & P → OS → Used Space → Memory → CPU; S and P hide/show as a pair; sort/filter unaffected when columns hidden.

**Independent Test**: Step through `columns_budget` values 39, 55, 68, 80, 91 and verify column set matches contract matrix; narrowing reverses order.

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T017 [P] [US2] Add tier 2–6 boundary and monotonic reveal-order tests in `src/resource_browser/vm_layout.rs`
- [X] T018 [P] [US2] Add S+P paired visibility tests (never one without the other) in `src/resource_browser/vm_layout.rs`
- [X] T019 [P] [US2] Add sort-arrow-on-logical-index tests when sorted column is hidden vs visible in `src/resource_browser/resource_table.rs`
- [X] T020 [P] [US2] Add filter-by-hidden-ID and filter-by-hidden-OS tests in `src/resource_browser/vm.rs`
- [X] T021 [P] [US2] Add mid-tier VM table rendering snapshots (`columns_budget` 39, 55, 68) in `src/resource_browser/resource_table.rs`

### Implementation for User Story 2

- [X] T022 [US2] Implement tier 2–6 visible column sets and fixed-width constraints in `src/resource_browser/vm_layout.rs`
- [X] T023 [US2] Apply sort indicator to filtered header using logical column index in `src/resource_browser/resource_table.rs`
- [X] T024 [US2] Verify `matches_filter` and `sort_by_column` require no changes beyond tests in `src/resource_browser/vm.rs`

**Checkpoint**: US1 and US2 work independently; full tier progression matches `contracts/vm-responsive-columns-ui.md`.

---

## Phase 5: User Story 3 - Comfortable Reading at Full Width (Priority: P2)

**Goal**: At full width all eight columns visible; ID is 12 cells (not 18); Name absorbs surplus width via `Fill`.

**Independent Test**: Render at `columns_budget` 91+ with a long VM name; all columns visible, ID narrower than other resource tables, Name shows more characters than pre-change layout.

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T025 [P] [US3] Add tier 6 full-layout fit test and Name `Fill` surplus-width test in `src/resource_browser/vm_layout.rs`
- [X] T026 [P] [US3] Add full-width eight-column VM table snapshot at `columns_budget` 120 in `src/resource_browser/resource_table.rs`
- [X] T027 [P] [US3] Assert `VM_ID_COLUMN_WIDTH == 12` and `ID_COLUMN_WIDTH == 18` remain distinct in `src/resource_browser/vm_layout.rs`

### Implementation for User Story 3

- [X] T028 [US3] Add `Constraint::Fill(1)` on Name when `columns_budget` exceeds tier-6 minimum in `src/resource_browser/vm_layout.rs`
- [X] T029 [US3] Update `VmData::column_sizes()` fallback to use `VM_ID_COLUMN_WIDTH` from `vm_layout` in `src/resource_browser/vm.rs`

**Checkpoint**: All three user stories independently testable; full-width layout matches spec SC-003.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Regression safety, validation, and quality gates.

- [X] T030 [P] Confirm existing non-VM `MockTableSource` snapshots in `src/resource_browser/resource_table.rs` remain unchanged
- [X] T031 [P] Add `column_layout` length parity assertion (`visible_indices.len() == constraints.len()`) in `src/resource_browser/vm_layout.rs` tests
- [X] T032 Run manual validation steps from `specs/002-vm-responsive-columns/quickstart.md`
- [X] T033 [P] Run `cargo fmt --check`
- [X] T034 [P] Run `cargo clippy`
- [X] T035 Run `cargo test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Setup — **BLOCKS** all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational — MVP
- **User Story 2 (Phase 4)**: Depends on Foundational; integrates with US1 widget wiring from T014–T015
- **User Story 3 (Phase 5)**: Depends on Foundational; benefits from US2 tier 2–6 but tier 6/Fill can be tested independently after T008
- **Polish (Phase 6)**: Depends on desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: After Phase 2 — no dependency on US2/US3
- **US2 (P1)**: After Phase 2 — extends `vm_layout` tiers and header sort behavior; builds on US1 widget projection (T014–T015)
- **US3 (P2)**: After Phase 2 — Name `Fill` and `vm.rs` constant; logically follows US2 tier 6 but independently testable via `vm_column_layout` unit tests

### Within Each User Story

- Tests MUST fail before implementation
- `vm_layout.rs` tier logic before widget snapshots that depend on it
- Widget wiring (US1) before mid/full-width snapshots (US2/US3)

### Parallel Opportunities

- Phase 1: T003 and T004 in parallel
- Phase 2: T009 parallel with T006–T008 after T005
- US1 tests: T011, T012, T013 in parallel
- US2 tests: T017–T021 in parallel
- US3 tests: T025–T027 in parallel
- Polish: T030, T031, T033, T034 in parallel

---

## Parallel Example: User Story 2

```bash
# Launch all US2 tests together (after Phase 2):
# T017 tier boundary tests in src/resource_browser/vm_layout.rs
# T018 S+P pairing tests in src/resource_browser/vm_layout.rs
# T019 sort-arrow tests in src/resource_browser/resource_table.rs
# T020 filter tests in src/resource_browser/vm.rs
# T021 mid-tier snapshots in src/resource_browser/resource_table.rs
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Name-only narrow layout at `columns_budget` 20–32
5. Demo resize showing ID appearing at 33+

### Incremental Delivery

1. Setup + Foundational → tier calculator ready
2. US1 → narrow Name-only layout (MVP)
3. US2 → full progressive reveal and sort/filter invariants
4. US3 → full-width Name expansion and ID width reduction
5. Polish → regression snapshots and cargo gates

### Parallel Team Strategy

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: US1 widget wiring + narrow snapshots
   - Developer B: US2 tier 2–6 logic + boundary tests
   - Developer C: US3 Fill/ID width + full-width snapshots
3. US2 widget sort tests (T019) should merge after US1 T014–T015 lands

---

## Notes

- `columns_budget` = `block.inner(area).width - 2` (highlight); tier fit also needs `(n-1) * 1` spacing gaps
- Do not change global `ID_COLUMN_WIDTH` (18) in `src/resource_browser/formatting.rs`
- `inventory_row` always emits eight cells; projection happens at render time only
- Host summary VM table (`src/host_summary_ui.rs`) is out of scope
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
