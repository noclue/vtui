# Data Model: VM Resource Browser Responsive Columns

## VmColumn

Logical VM table column identifier. Ordinal values match `VmData::inventory_row` cell index and
`sort_by_column` index.

| Index | Variant     | Header label | Full-width constraint policy        |
| ----- | ----------- | ------------ | ----------------------------------- |
| 0     | `Id`        | `ID `        | `Length(12)` — VM-specific width    |
| 1     | `Status`    | `S `         | `Length(2)`                         |
| 2     | `Power`     | `P `         | `Length(2)`                         |
| 3     | `Name`      | `Name `      | `Min(20)` + `Fill(1)` when tier ≥ 6 |
| 4     | `Os`        | `OS `        | `Length(15)` when visible           |
| 5     | `UsedSpace` | `Used Space `| `Length(12)` when visible           |
| 6     | `Cpu`       | `CPU `       | `Length(10)` when visible           |
| 7     | `Memory`    | `Memory `    | `Length(11)` when visible           |

**Invariants**:

- `Status` and `Power` are always both visible or both hidden.
- `Name` is always visible.
- Reveal order when widening: `Name → Id → Status+Power → Os → UsedSpace → Memory → Cpu`.

## VmLayoutTier

Discrete layout band derived from usable table width.

| Tier | Value | Visible `VmColumn` set                                      |
| ---- | ----- | ----------------------------------------------------------- |
| 0    | `NameOnly` | `{ Name }`                                             |
| 1    | `WithId`   | `{ Id, Name }`                                         |
| 2    | `WithStatusPower` | `{ Id, Status, Power, Name }`                   |
| 3    | `WithOs`   | `{ Id, Status, Power, Name, Os }`                      |
| 4    | `WithUsedSpace` | `{ Id, Status, Power, Name, Os, UsedSpace }`      |
| 5    | `WithMemory` | `{ Id, Status, Power, Name, Os, UsedSpace, Memory }` |
| 6    | `Full`     | All eight columns                                           |

**State transition**: `tier = f(usable_width)` recomputed every render; no hysteresis. Widen/narrow
transitions are immediate.

**Validation**:

- `tier` monotonically increases with `usable_width` except in the sub-20 fallback where only Name
  is shown at `Length(usable_width)`.

## ColumnLayout

Render-time layout descriptor returned by `TableDataSource::column_layout`.

**Fields**:

- `visible_indices: Vec<usize>` — logical column indices in left-to-right display order.
- `constraints: Vec<Constraint>` — parallel Ratatui constraints for visible columns only.

**Invariants**:

- `visible_indices.len() == constraints.len()`.
- `visible_indices` is a subsequence of `[0, 1, 2, 3, 4, 5, 6, 7]` preserving canonical order.
- For non-VM resources, `visible_indices == 0..header_len`.

## VmTableLayoutConstants

**Fields** (module-level constants):

- `VM_ID_COLUMN_WIDTH: u16 = 12`
- `VM_NAME_MIN_WIDTH: u16 = 20`
- `VM_STATUS_WIDTH: u16 = 2` (reuse `STATUS_COLUMN_WIDTH`)
- `VM_POWER_WIDTH: u16 = 2`
- `VM_OS_WIDTH: u16 = 15`
- `VM_USED_SPACE_WIDTH: u16 = 12`
- `VM_CPU_WIDTH: u16 = 10`
- `VM_MEMORY_WIDTH: u16 = 11`
- `TABLE_HIGHLIGHT_PREFIX_WIDTH: u16 = 2` (`"▶ "`, `HighlightSpacing::Always`)
- `TABLE_COLUMN_SPACING: u16 = 1` (Ratatui `Table` default; 1 cell between each column pair)

**Width budget** (per render):

```
table_inner_width = block.inner(area).width
columns_budget    = table_inner_width - TABLE_HIGHLIGHT_PREFIX_WIDTH
spacing_gaps(n)   = (n - 1) * TABLE_COLUMN_SPACING
tier fits         = sum(visible column widths) + spacing_gaps(n) <= columns_budget
```

**Derived minimum `columns_budget`** (highlight already subtracted):

- `TIER_0_MIN = 20` (1 col, 0 gaps)
- `TIER_1_MIN = 33` (2 cols, 1 gap)
- `TIER_2_MIN = 39` (4 cols, 3 gaps)
- `TIER_3_MIN = 55` (5 cols, 4 gaps)
- `TIER_4_MIN = 68` (6 cols, 5 gaps)
- `TIER_5_MIN = 80` (7 cols, 6 gaps)
- `TIER_6_MIN = 91` (8 cols, 7 gaps)

Equivalent minimum `table_inner_width` = `TIER_N_MIN + 2`.

## Unaffected Entities

The following remain unchanged by this feature:

- `VmData` vSphere retrievable fields and `inventory_row` cell content.
- `IndexedCache` filter/sort state (`sort_column` logical index, `filter` string).
- `PerfRowsSnapshot` and CPU/Memory sparkline cell formatting.
- Other `TabularData` implementations (Host, Cluster, Datastore, etc.).

## Row Projection

**Operation**: `project_row(row: Row, visible_indices: &[usize]) -> Row`

Selects cells from the full eight-cell VM row by index. Header projection uses the same index list
against `VmData::header_row()` labels.

**Validation**:

- Projected row cell count equals `visible_indices.len()`.
- No placeholder cells for hidden columns.
