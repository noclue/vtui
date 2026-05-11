# Quickstart: ESX Host Summary View

## Prerequisites

- Rust toolchain compatible with the repository.
- Access to a vCenter or standalone ESXi host with at least one visible `HostSystem`.
- Optional: govc `vcsim` for simulator checks where host properties are available. `vcsim` often
  leaves required vSphere fields blank, so simulator tests should validate tolerant decoding and
  broad fetch behavior rather than treating every omitted field as real-product behavior.

## Build and Run

```bash
cargo run --bin vtui -- <profile>
```

If using environment-only configuration:

```bash
VIM_SERVER=<server> VIM_USERNAME=<user> VIM_PASSWORD=<password> VIM_PROTOCOL=auto cargo run --bin vtui
```

## Manual Validation

1. Open vtui against a vCenter or ESXi endpoint.
2. Navigate to a Host resource view using existing navigation, such as `h host` from a parent view.
3. Confirm the footer/action hints include `s summary` for Host rows.
4. Select a host and press `s`.
5. Confirm a loading Host summary popup appears immediately.
6. Confirm the popup title includes the host inventory path when data loads.
7. Confirm the Summary section shows identity, vendor/model, CPU, RAM, status, connection, power,
   uptime, CPU usage, and memory usage where available.
8. Confirm Physical NICs, Disks, optional Memory Tiering, optional Graphics, and Virtual Machines
   sections render correctly.
9. Confirm VM section caps at 300 rows and states the total when the host has more than 300 VMs.
10. Confirm `Esc` and `q` close loading and ready popups.
11. Confirm arrow keys, `j`/`k`, Page Up/Down, `g`/`G`, and Ctrl+B/F scroll as expected.
12. Resize the terminal while loading and while ready; confirm redraw remains correct and responsive.
13. Repeat over an SSH session or constrained terminal size if available.

## Edge Case Validation

- Host with zero VMs.
- Host with no visible physical NIC data.
- Host with no disk data or inaccessible `config.storage_device`.
- Host where memory tiering and graphics data are absent.
- Disconnected or permission-limited host.
- vCenter/ESXi versions that omit newer physical NIC driver or memory tiering fields.

## Automated Validation

Run:

```bash
cargo fmt --check
cargo clippy
cargo test
```

Focused tests should cover:

- Host row `s` dispatch and Host hint availability.
- `HostSummaryUi` loading/ready close and scroll key handling.
- Resize-sensitive rendering/content rebuild and scroll clamping.
- Host disk extraction from SCSI rows and best-effort NVMe rows.
- VM cap behavior and `Showing 300 of N` header.
- Optional memory tiering and graphics section omission/rendering.
- Stale request id success/failure events are ignored.

## Optional `vcsim` Integration Validation

Use `vcsim` as a smoke test for connection, PropertyCollector retrieval, and no-panic summary fetches.
When assertions involve fields that `vcsim` commonly omits, prefer one of these patterns:

- Mark the retrievable property optional with the `vim_retrievable!` path suffix where appropriate.
- Assert that the summary fetch succeeds and renders placeholders for missing display fields.
- Keep exact field mapping assertions in pure unit tests with typed fixtures that represent real
  vCenter/ESXi data.

Do not make CI depend on simulator data matching real vCenter required-field guarantees exactly.
