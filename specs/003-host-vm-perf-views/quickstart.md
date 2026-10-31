# Quickstart: Host and VM Performance Overview

## Prerequisites

- Rust toolchain matching the repository (`rust-version` 1.89, edition 2024).
- A vCenter or ESXi endpoint with at least one host and one virtual machine that have been
  collecting realtime stats.
- A terminal that can be resized, including a short side-by-side size such as 40×15.
- The disk exit check below needs a real vCenter. `vcsim` is not enough for disk throughput.

## Build and Run

```bash
cargo run --bin vtui -- <profile>
```

Or with environment configuration:

```bash
VIM_SERVER=<server> VIM_USERNAME=<user> VIM_PASSWORD=<password> VIM_PROTOCOL=auto cargo run --bin vtui
```

## Open and Leave

1. Open the virtual-machine inventory. Confirm the expand hints include `p perf` and the help
   hints include `z header`.
2. Select a VM and press `p`. The inventory table is replaced by the overview. The title, when
   the body is at least 8 rows, shows the VM name, `realtime 20s`, `30m`, and a sample time after
   the first successful poll.
3. Press `1` `2` `3` `4` and Tab / Shift-Tab. The primary panel follows focus.
4. Press `s`, `r`, and `/`. Nothing opens.
5. Press Esc. The same inventory returns. Repeat with Backspace from a second open.
6. Open again and press `q`. The application quits.
7. Repeat from the host inventory. Host hints show `n network`, `d datastore`, `s summary`,
   `p perf`, `v vm`. `t` and `e` still open tasks and events.

## Header

1. Press `z` on the inventory. The five-row header disappears and the table gains five rows.
2. Open the overview. The header stays hidden.
3. Leave the overview. The header stays hidden. Press `z` to show it again.
4. Start a search with `/` and type `z`. The filter gains the letter. The header does not toggle.
5. On the property browser, confirm help shows `z header` and that `z` still toggles the header.

## Sizes

Header on means body height = terminal height − 5.

| Terminal | Header | Expect |
| -------- | ------ | ------ |
| 32×12 | on | Four badge lines and a sparkline for the focused domain. No title |
| 72×16 | on | Two charts and a row of two badges. Not a 2×2 |
| 58×17 | on | One chart and a right rail |
| 80×24 | on | 2×2 |
| 135×42 | on | 2×2 with CPU, consumed memory, network, and disk throughput |
| 40×15 | off (`z`) | One chart and three badge rows |
| 40×45 | on | Four charts in one column |

Narrow the body below 21 columns with at least four content rows: four clipped badges, no
sparkline. Shrink below four content rows: one shared line, cut off with `…`.

## Data

1. Confirm CPU is a percent graph with MHz (or GHz at 10000 MHz and above) only in the badge.
2. Confirm memory is a consumed size such as `1.4GiB`, with a percent only when usage is present.
   There is no granted line.
3. Confirm network uses `↓` and `↑` and one unit for the pair (`KiB/s`, `MiB/s`, or `GiB/s`).
4. Confirm disk is read/write rate, not capacity and not IOPS.
5. Wait for two polls (about 20 seconds). The graph replaces. It does not grow a duplicate tail.
   The title clock matches the newest sample, or `sample —` when the object has none.
6. Stop the guest or pick a powered-off VM. The next poll clears the graphs and shows
   `sample —`.
7. If a refresh fails while a window is on screen, the graphs stay and the title shows `ERR` and
   a short message on that same line.

## Disk exit check (real vCenter)

On a real vCenter, not only `vcsim`, confirm non-empty aggregate disk read and disk write for:

- a host
- a virtual machine on VMFS
- a virtual machine on NFS or vSAN

An empty VM series does not fail the rest of the overview. It means v1 disk is not done for that
datastore type. The follow-up is a summed per-disk series, not an IOPS toggle. Do not treat
`vcsim` numbers as this check.

## Automated Checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Layout tests cover every row of the size table, the invariant for widths 1–200, heights 1–60, and
all four focus values, plus snapshots at 32×12, 80×24, and 135×42 with the header on, and 40×15
with the header off. After line charts land, refresh the 135×42 and 40×15 snapshots.
