# Contract: VM Resource Browser Responsive Columns

## Scope

Observable TUI behavior for the Virtual Machines resource browser table when terminal width changes.
This is a UI layout contract, not a vSphere API contract.

## Entry Context

- **View**: Resource browser showing `ResourceType::VirtualMachine`.
- **Widget**: `ResourceTableWidget` backed by `IndexedCache<VmData>`.
- **Trigger**: Terminal resize or initial draw with a given table inner width.

## Column Visibility Contract

### Tier 0 — Name only

- **Condition**: `columns_budget` &lt; 26 (cannot fit S + P + Name + spacing gaps).
- **Visible headers**: `Name` only (sort arrow allowed on Name).
- **Minimum Name width**: 20 character cells when usable width ≥ 20; otherwise Name uses all usable
  width.
- **Hidden**: ID, S, P, OS, Used Space, CPU, Memory — no blank columns, no header gaps.

### Tier 1 — S, P, Name

- **Condition**: `columns_budget` ≥ 26 and &lt; 39.
- **Visible headers**: `S`, `P`, `Name`.
- **Pairing**: S and P appear together.

### Tier 2 — ID, S, P, Name

- **Condition**: `columns_budget` ≥ 39 and &lt; 55.
- **Visible headers**: `ID`, `S`, `P`, `Name`.
- **ID width**: 12 character cells.

### Tier 3 — + OS

- **Condition**: `columns_budget` ≥ 55 and &lt; 68.
- **Added header**: `OS`.

### Tier 4 — + Used Space

- **Condition**: `columns_budget` ≥ 68 and &lt; 80.
- **Added header**: `Used Space`.

### Tier 5 — + Memory

- **Condition**: `columns_budget` ≥ 80 and &lt; 91.
- **Added header**: `Memory` (before CPU in column order).

### Tier 6 — Full layout

- **Condition**: `columns_budget` ≥ 91.
- **Visible headers**: `ID`, `S`, `P`, `Name`, `OS`, `Used Space`, `CPU`, `Memory`.
- **ID width**: 12 character cells (6 narrower than the 18-cell default used elsewhere).
- **Extra width**: Absorbed by Name column expansion; fixed columns do not grow beyond their max/length.

## Width Budget

```
table_inner_width = block.inner(area).width
  // bordered Block + Padding::right(1); excludes left/right border cells

columns_budget = table_inner_width - TABLE_HIGHLIGHT_PREFIX_WIDTH
TABLE_HIGHLIGHT_PREFIX_WIDTH = 2  // "▶ " selection prefix (HighlightSpacing::Always)

spacing_gaps = (visible_column_count - 1) * TABLE_COLUMN_SPACING
TABLE_COLUMN_SPACING = 1            // Ratatui Table default between columns

tier fits when: sum(visible column widths) + spacing_gaps <= columns_budget
```

## Behavioral Invariants

### Selection and navigation

- Row selection highlight (`▶ `) remains visible at all tiers.
- ↑↓ scrolling unchanged.
- Selected row index preserved across resize.

### Sorting

- Sortable logical columns: 0 (ID), 3 (Name), 4 (OS), 5 (Used Space).
- Active sort persists when the sorted column is hidden; sort arrow reappears when the column becomes
  visible again.
- Column header click/key sort behavior unchanged (uses logical indices).

### Filtering

- Filter matches ID, Name, and OS even when those columns are hidden.
- Filter banner in title unchanged.

### Live data

- CPU/Memory sparkline cells render when columns visible; placeholders when perf absent (unchanged).
- Resize does not invalidate cache or perf snapshot.

### Other resource types

- Host, Cluster, Datastore, Network, Task, and Events tables ignore tier logic; all columns remain
  always visible with existing constraints.

## Non-Goals

- Host summary embedded VM table (`host_summary_ui::format_vm_table`) — not modified in this feature.
- Changing global `ID_COLUMN_WIDTH` for non-VM views.
- Animations or transitional layouts when resizing.

## Verification Matrix

`columns_budget` values below (highlight already subtracted):

| `columns_budget` | Expected visible column count | First hidden column (when narrowing) |
| ---------------- | ----------------------------- | ------------------------------------ |
| 19               | 1 (Name truncated)            | S                                    |
| 20–25            | 1                             | S                                    |
| 26–38            | 3 (S, P, Name)                | ID                                   |
| 39–54            | 4 (ID, S, P, Name)            | OS                                   |
| 55–67            | 5                             | Used Space                           |
| 68–79            | 6                             | Memory                               |
| 80–90            | 7                             | CPU                                  |
| 91+              | 8                             | none                                 |
