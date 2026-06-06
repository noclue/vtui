# Quickstart: VM Resource Browser Responsive Columns

## Prerequisites

- Rust toolchain compatible with the repository.
- Access to a vCenter or ESXi endpoint with multiple VMs (names of varying length help).
- Terminal that can be resized horizontally.

## Build and Run

```bash
cargo run --bin vtui -- <profile>
```

## Manual Validation

1. Navigate to a Virtual Machines resource view.
2. Shrink the terminal until only the **Name** column is visible; confirm VM names remain readable
   and no ID/S/P/OS/metric headers appear.
3. Slowly widen the terminal and confirm columns appear in order: **ID → S & P → OS → Used Space →
   Memory → CPU**.
4. At full width, confirm ID column is visibly narrower than Host/Cluster ID columns in other views.
5. Confirm long VM names show more characters at wide width than before (Name absorbs extra space).
6. Narrow past a column threshold and confirm the column disappears without leaving blank header space.
7. Apply a filter matching a VM ID while ID column is hidden; confirm the VM still appears.
8. Sort by OS, then narrow until OS hides; widen again and confirm sort arrow returns on OS.
9. Select a row, resize repeatedly; confirm selection is preserved.
10. Repeat over SSH if available.

## Tier Boundary Checks

Use `columns_budget = block_inner_width - 2` (highlight prefix). Tier fit also requires
`(visible_cols - 1)` spacing cells (Ratatui default `column_spacing = 1`). Borders and block
right padding are already excluded from `block_inner_width`.

| Target `columns_budget` | Expected columns        |
| ----------------------- | ----------------------- |
| 20–32                   | Name only               |
| 33–38                   | ID, Name                |
| 39–54                   | ID, S, P, Name          |
| 55–67                   | + OS                    |
| 68–79                   | + Used Space            |
| 80–90                   | + Memory                |
| 91+                     | + CPU (all eight)       |

## Automated Validation

```bash
cargo fmt --check
cargo clippy
cargo test
```

Focused tests:

- `vm_layout` tier selection at boundary widths.
- `ColumnLayout` index/constraint length parity.
- Row/header projection preserves cell order.
- VM table `insta` snapshots at narrow, mid, and full widths.
- Regression: non-VM `MockTableSource` snapshots unchanged.

## Out of Scope Checks

- Host summary popup VM table layout — unchanged.
- Other resource browser tabs — unchanged column behavior.
