# Research: VM Resource Browser Responsive Columns

## Decision: Width-Aware `ColumnLayout` at the Table Widget Boundary

**Decision**: Extend `TableDataSource` with a `column_layout(area_width: u16) -> ColumnLayout` method
(default: all columns visible, existing static constraints). `ResourceTableWidget` passes
`inner.width` (minus row-highlight prefix width) into `column_layout`, filters header cells and row
cells by `ColumnLayout::visible_indices`, and builds the Ratatui `Table` from the filtered vectors
only.

**Rationale**: Column hiding must happen where terminal width is known (`ResourceTableWidget::render`).
A default trait method keeps every non-VM resource type unchanged. Filtering cells at render time
avoids duplicating `inventory_row` logic or changing `vim_retrievable!` data paths.

**Alternatives considered**:

- Static `column_sizes()` only with `Constraint::Percentage` tricks: rejected because Ratatui cannot
  hide columns or enforce tiered reveal order with percentages alone.
- VM-only branching inside `ResourceTableWidget` without a trait hook: rejected because it couples
  the generic widget to VM column indices and header labels.
- Rebuild rows inside `IndexedCache::iter` with stored width: rejected because width is render-time
  state and would require threading width through the data layer on every frame.

## Decision: Pure `vm_layout` Module for Tier Math

**Decision**: Add `src/resource_browser/vm_layout.rs` with VM column constants, tier thresholds, and
`vm_column_layout(usable_width: u16) -> ColumnLayout`. `IndexedCache<VmData>` overrides
`column_layout` to delegate here. `VmData::column_sizes()` remains as the full-width fallback for
any legacy callers but is no longer used directly by the widget.

**Rationale**: Tier thresholds are non-trivial (paired S/P, Name minimum, ID shrink, Fill for extra
width). Centralizing them in one testable module matches the spec's tier table and keeps `vm.rs`
focused on data retrieval and row content.

**Alternatives considered**:

- Inline tier logic in `vm.rs`: acceptable but harder to unit-test without pulling in Ratatui row types.
- Shared responsive layout for Host summary VM table: deferred per spec assumption (out of scope).

## Decision: VM-Specific ID Width Constant

**Decision**: Introduce `VM_ID_COLUMN_WIDTH: u16 = 12` in `vm_layout.rs` (or `vm.rs`). Do **not**
change global `ID_COLUMN_WIDTH` (18) used by hosts, clusters, datastores, tasks, and events.

**Rationale**: Spec FR-006 requires a 6-character reduction only in the VM resource browser. Other
views keep their established ID column width.

**Alternatives considered**:

- Lower global `ID_COLUMN_WIDTH`: rejected; violates FR-012 and would churn unrelated snapshots.

## Decision: Usable Width Budget (Borders, Highlight, Column Spacing)

**Decision**: Derive tier thresholds from the Ratatui `Table` layout budget inside the bordered
block, not the outer terminal width:

```
table_inner_width = block.inner(area).width   // left/right borders + right padding already excluded
columns_budget    = table_inner_width - TABLE_HIGHLIGHT_PREFIX_WIDTH
fit(tier)         = sum(column_widths[tier]) + column_spacing_gaps(tier) <= columns_budget

column_spacing_gaps(tier) = (visible_column_count(tier) - 1) * TABLE_COLUMN_SPACING
TABLE_HIGHLIGHT_PREFIX_WIDTH = 2   // "▶ " with HighlightSpacing::Always
TABLE_COLUMN_SPACING = 1           // Ratatui Table default; ResourceTableWidget does not override
```

**Rationale**:

- **Borders**: `Block::bordered()` plus `Padding::right(1)` shrink content via `block.inner(area)`;
  tier math MUST use that inner width, not the parent frame width.
- **Highlight**: Ratatui allocates a fixed selection column before column constraints run.
- **Column spacing**: Ratatui `Table` defaults to `column_spacing(1)` — one blank cell between each
  adjacent column pair. With `n` visible columns, `(n - 1)` gaps are consumed in addition to column
  widths. The host summary text tables already model this explicitly as `TABLE_COL_GAP = 1`; the
  resource browser `Table` widget gets the same gap implicitly from Ratatui.

**Alternatives considered**:

- Remove highlight symbol on narrow layouts: rejected; changes selection affordance unrelated to the
  feature request.
- Set `.column_spacing(0)` to simplify math: rejected; would tighten columns visually vs current
  shipped table and vs operator expectation of readable separation.

## Decision: Tier Thresholds from Widths + Spacing Gaps

**Decision**: Minimum `columns_budget` per tier (after highlight subtraction):

| Tier | Visible cols | Column widths sum | Spacing gaps `(n-1)×1` | Min `columns_budget` | Min `table_inner_width` |
| ---- | ------------ | ----------------- | ---------------------- | -------------------- | ----------------------- |
| 0    | 1 (Name)     | 20                | 0                      | 20                   | 22                      |
| 1    | 3 (S,P,Name) | 24                | 2                      | 26                   | 28                      |
| 2    | 4            | 36                | 3                      | 39                   | 41                      |
| 3    | 5            | 51                | 4                      | 55                   | 57                      |
| 4    | 6            | 63                | 5                      | 68                   | 70                      |
| 5    | 7            | 74                | 6                      | 80                   | 82                      |
| 6    | 8            | 84                | 7                      | 91                   | 93                      |

Tier selection walks from tier 6 downward and picks the highest tier whose `fit(tier)` is true.
Above tier 6, remaining `columns_budget` flows to Name via `Constraint::Fill(1)` with
`Constraint::Min(20)`. Below tier 0 (`columns_budget` &lt; 20), show Name only at
`Constraint::Length(columns_budget)`.

**Rationale**: Matches spec reveal order, current `vm.rs` fixed/max widths, and actual Ratatui table
layout allocation.

**Alternatives considered**:

- Percentage-based breakpoints: rejected; not deterministic in character cells across terminals.
- Single flat “fudge factor” added to all tiers: rejected; gap count changes per tier so spacing must
  be `(n - 1) * spacing` not a constant offset.

## Decision: Sort and Filter Use Logical Column Indices

**Decision**: Keep sort column indices 0–7 aligned with full `VmData` row cell order. When building
the filtered header, apply the sort arrow only if the sorted logical index is in `visible_indices`.
Filtering (`matches_filter`) is unchanged and continues to search ID, Name, and OS regardless of
visibility.

**Rationale**: Avoids migrating sort state when columns hide/show and satisfies FR-010/FR-011.

**Alternatives considered**:

- Remap sort indices to visible-only positions: rejected; would reset or confuse sort on resize.

## Decision: Snapshot and Unit Tests at Tier Boundaries

**Decision**: Unit-test `vm_column_layout` at widths 19, 20, 31, 32, 35, 36, 50, 51, 62, 63, 73,
74, 83, 84, and 120. Add Ratatui `insta` snapshots for VM table rendering at representative widths
using fixture rows with long names and full metric cells.

**Rationale**: Tier boundaries are the highest-risk regression surface; snapshots catch header/cell
misalignment that pure layout tests might miss.

**Alternatives considered**:

- Manual-only resize validation: insufficient for constitution quality gates.

## Decision: No vSphere or Perf Pipeline Changes

**Decision**: No PropertyCollector path changes, no perf sampling changes. CPU/Memory cells continue
to use existing perf snapshot data when those columns are visible.

**Rationale**: Feature is layout-only; constitution minimal-retrieval gate stays clean.

**Alternatives considered**:

- Skip perf fetch when CPU/Memory hidden: rejected for now; perf is already keyed to visible VM set
  elsewhere and hiding columns is transient with resize.
