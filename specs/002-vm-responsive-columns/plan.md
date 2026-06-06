# Implementation Plan: VM Resource Browser Responsive Columns

**Branch**: `002-vm-responsive-columns` | **Date**: 2026-06-06 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/002-vm-responsive-columns/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Make the Virtual Machines resource browser table responsive to terminal width: always show Name (min
20 cells), progressively reveal ID → S/P → OS → Used Space → Memory → CPU as width grows, shrink VM
ID to 12 cells at full layout, and expand Name with leftover width. Implementation adds a pure
`vm_layout` tier calculator, a width-aware `ColumnLayout` on `TableDataSource`, and cell projection
in `ResourceTableWidget` — no vSphere or perf pipeline changes.

## Technical Context

**Language/Version**: Rust edition 2024, Rust 1.85+ documented for source builds  
**Primary Dependencies**: Ratatui 0.30, crossterm 0.29, tokio 1.44, vim_rs 0.5 with `xml` and `vcsim_compat`  
**Storage**: N/A for feature data  
**Testing**: `cargo test`, `vm_layout` unit tests, Ratatui `insta` snapshots for VM table tiers, existing `resource_table` mock snapshots for non-VM regression  
**Target Platform**: macOS, Windows, Linux terminals, including SSH jump hosts  
**Project Type**: Rust terminal UI application  
**Performance Goals**: Layout recomputation is O(1) per frame; no added vSphere I/O; resize redraw remains immediate  
**Constraints**: UI task must not block; no new PropertyCollector paths; dark-theme contrast unchanged; dynamic resize support required  
**Scale/Scope**: VM resource browser table only; eight logical columns; inventories of thousands of VMs unchanged (layout is width-only)  
**vSphere Data Access**: None — existing `VmData` `vim_retrievable!` paths unchanged  
**Background Work**: None — perf snapshot attachment for CPU/Memory cells unchanged  
**Operator UX**: No new key bindings; selection highlight preserved; sort/filter behave as today; columns hide/show on resize without navigation

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

*GATE: Each item must be PASS or explicitly justified in Complexity Tracking.*

- **Terminal-native operator UX**: PASS. No action hints change; VM browser footer keys unchanged.
- **Responsive async UI**: PASS. Layout math runs synchronously in render; no I/O added.
- **Minimal retrieval and live state**: PASS. No new vSphere properties or perf sampling; CPU/Memory
  cells simply hidden when columns are not visible.
- **Cross-platform remote readiness**: PASS. Character-cell tier thresholds; resize handled in existing
  `ResourceTableWidget` draw path.
- **Tested Rust quality gates**: PASS. Unit tests for tier boundaries, snapshots for rendered VM
  table, final `cargo fmt --check`, `cargo clippy`, `cargo test`.
- **Dark-theme accessibility**: PASS. No new colors; hidden columns removed entirely (no low-contrast
  placeholder columns).
- **vSphere API correctness**: PASS. No API changes.

**Post-design re-check**: PASS — design is render-layer only.

## Project Structure

### Documentation (this feature)

```text
specs/002-vm-responsive-columns/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   └── vm-responsive-columns-ui.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/resource_browser/
├── vm_layout.rs           # NEW: tier thresholds, VmColumn, vm_column_layout(width)
├── tabular_data.rs        # ADD: ColumnLayout struct, column_layout() default on TableDataSource
├── resource_table.rs      # USE: column_layout(inner), project header/rows, highlight width subtract
├── indexed_cache.rs       # OVERRIDE: column_layout for VmData → vm_column_layout
├── vm.rs                  # UPDATE: full-width constants align with vm_layout; tests if needed
└── mod.rs                 # declare vm_layout module

Module-local tests
├── src/resource_browser/vm_layout.rs   # tier boundary unit tests
├── src/resource_browser/resource_table.rs  # optional VM-tier snapshots with Vm fixture source
└── src/resource_browser/vm.rs          # existing filter/sort tests unchanged
```

**Structure Decision**: Keep responsive logic in `resource_browser` beside `vm.rs`. Avoid touching
`host_summary_ui.rs` per spec scope. Extend `TableDataSource` with a default `column_layout` so only
VM overrides behavior.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | N/A |

## Phase 0: Research

Research is captured in [research.md](./research.md). Key decisions:

- Width-aware `ColumnLayout` at `ResourceTableWidget` with default trait method for other resources.
- Pure `vm_layout` module for tier math and VM-specific `VM_ID_COLUMN_WIDTH = 12`.
- Use `block.inner(area).width` for borders/padding; subtract 2-cell highlight prefix; include
  `(n-1)`-cell Ratatui `column_spacing` (default 1) in tier fit checks.
- Logical sort/filter indices preserved across column hiding.
- Snapshot + unit tests at tier boundaries.

## Phase 1: Design

Design artifacts:

- [data-model.md](./data-model.md)
- [contracts/vm-responsive-columns-ui.md](./contracts/vm-responsive-columns-ui.md)
- [quickstart.md](./quickstart.md)

### Implementation sketch

1. **`ColumnLayout`** in `tabular_data.rs`:
   ```rust
   pub struct ColumnLayout {
       pub visible_indices: Vec<usize>,
       pub constraints: Vec<Constraint>,
   }
   ```
   Default `TableDataSource::column_layout(width)` returns all indices and `column_sizes()`.

2. **`vm_layout.rs`** exports `vm_column_layout(columns_budget: u16) -> ColumnLayout` implementing
   the tier table from the spec (budget excludes highlight; fit check includes `(n-1)` column
   spacing gaps at 1 cell each).

3. **`IndexedCache<VmData>`** — add specialized `column_layout` via helper trait or inherent
   method called from widget when `resource_type() == VirtualMachine`. Preferred: implement
   `column_layout` on `IndexedCache` checking `T::resource_type()` at runtime, delegating to
   `vm_column_layout` only for `VirtualMachine`.

4. **`ResourceTableWidget::render`**:
   - `let columns_budget = inner.width.saturating_sub(HIGHLIGHT_PREFIX_WIDTH);`
     (`inner` from `block.inner(area)` — borders and right padding already excluded)
   - `let layout = self.resources.column_layout(columns_budget);`
     (tier fit includes `(n-1) * TABLE_COLUMN_SPACING` where `n` = visible columns)
   - Build header from `header_row()` filtered by `layout.visible_indices`; map sort arrow by
     logical index.
   - Map `iter()` rows through `project_row(cells, &layout.visible_indices)`.
   - `Table::new(rows, layout.constraints)`.

5. **`vm.rs`**: Replace `ID_COLUMN_WIDTH` with `VM_ID_COLUMN_WIDTH` from `vm_layout` in
   `column_sizes()` fallback; keep `inventory_row` producing all eight cells.

### Agent context

Updated `.cursor/rules/specify-rules.mdc` to reference this plan.

## Phase 2: Tasks (next command)

Run `/speckit-tasks` to generate `tasks.md` with dependency-ordered implementation and test tasks.
