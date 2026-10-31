# Implementation Plan: Host and VM Performance Overview

**Branch**: `004-host-vm-perf-views` | **Date**: 2026-10-31 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/003-host-vm-perf-views/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add a full-body realtime performance overview for the selected virtual machine or host. `p` opens
it, Esc and Backspace restore the inventory through the existing history path, and `q` still quits.
The body shows CPU, consumed memory, network throughput, and disk read/write throughput, stepping
from badge to sparkline to chart as width and height allow. Samples come from one
`vim_rs::perf` query (eight counters, latest 90, aggregate instance) on the existing perf worker.
The table sparkline poll pauses while the overview is open. `z` hides the five-row app header for
the whole session.

Phase A ships layout, badges, and Ratatui sparklines, including sparkline fills in chart-sized
slots. Phase B replaces those chart slots with line charts. Layout geometry does not change
between the phases.

## Technical Context

**Language/Version**: Rust edition 2024, `rust-version` 1.89  
**Primary Dependencies**: Ratatui 0.30 (`ratatui-widgets` 0.3 `Sparkline` and `Chart`), crossterm 0.29, tokio 1.53, `vim_rs` path dependency with features `xml`, `vcsim_compat`, `perf`  
**Storage**: N/A. The header flag is session memory only. The sample window lives in an `Arc<RwLock<...>>` owned by the overview request  
**Testing**: `cargo test`. Pure layout invariant, `bucket_series` / formatters, store merge, `debounce_for`, polling-policy mode, `insta` snapshots at the spec’s terminal sizes  
**Target Platform**: macOS, Windows, Linux terminals, including SSH jump hosts  
**Project Type**: Rust terminal UI application  
**Performance Goals**: Overview fetch stays off the UI task. Resize only recomputes `perf_overview_layout`. One `QueryPerf` per 20s tick for one object  
**Constraints**: UI task must not block. Table and overview queries must not run together. Dark-theme series colors are fixed. No dual axis. Badges clip instead of wrapping  
**Scale/Scope**: One MoRef, eight counters, 90 realtime samples (about 30 minutes at 20s). Inventory six-slot sparklines stay as they are  
**vSphere Data Access**: `PerfClient::prepare` with `Interval::Realtime`, `.latest(90)`, default `Instance::Aggregate`. Counters listed in [research.md](./research.md). No PropertyCollector wait for this pane. Title name is the string captured at open  
**Background Work**: Existing `perf_worker` task. `PerfRequest` becomes `Table` or `Overview`. Overview skips the 500ms debounce. Results are `AppEvent::PerfOverviewResult { generation }` on success and on transport error. The UI reads the store  
**Operator UX**: Hint lists in the spec. `z` toggles the header unless search is active. Failures are `ERR` on the title line. Empty objects show `sample —`. Missing series show `No samples` on that domain only

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

*GATE: Each item must be PASS or explicitly justified in Complexity Tracking.*

- **Terminal-native operator UX**: PASS. `p`, Tab, `1`–`4`, Esc, Backspace, and `q` are shown in the overview hints. `z header` is added to shared help, events help, and both property browsers.
- **Responsive async UI**: PASS. `QueryPerf` stays on `perf_worker`. Resize does not fetch. A failed fetch still sends `PerfOverviewResult` so `ERR` paints without another key.
- **Minimal retrieval and live state**: PASS. One entity, eight counters, 90 samples, aggregate instance. The six-slot table poll pauses while the overview is open and resumes on `back()`. Cost is one realtime `QueryPerf` per 20s tick instead of the visible-row table query. No PropertyCollector expansion.
- **Cross-platform remote readiness**: PASS. Layout is character-cell math. The same function covers header on and off. No new config or log paths.
- **Tested Rust quality gates**: PASS. Layout, bucket, format, store, debounce, and polling-policy tests, plus snapshots. Final `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
- **Dark-theme accessibility**: PASS. Fixed series colors on the existing dark background. Legend only at ≥ 50 columns. No second axis. `ERR` and `No samples` stay on the same surfaces as the data.
- **vSphere API correctness**: PASS. Uses `vim_rs::perf` prepared queries and `EntityStatus`, not hand-built PropertyCollector specs. Disk uses whole-entity `disk.read` / `disk.write`, not `virtualDisk.*`. A real vCenter check is a manual exit gate because `vcsim` does not prove the aggregate.

**Post-design re-check**: PASS. The store, request enum, and layout types in [data-model.md](./data-model.md) do not add a second worker, a second back stack, or extra counters.

## Project Structure

### Documentation (this feature)

```text
specs/003-host-vm-perf-views/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   └── perf-overview-ui.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/
├── perf_overview/
│   ├── mod.rs           # BodyPane glue: open, render, keys, hints
│   ├── layout.rs        # perf_overview_layout → template 1..=9
│   ├── metrics.rs       # eight counters, one prepared query
│   ├── store.rs         # replace-on-poll window, ERR / sample —
│   ├── render.rs        # badges, sparkline_line, Sparkline, later Chart
│   └── ui.rs            # Tab, BackTab, 1–4, PaneAction::Back
├── resource_browser/formatting.rs  # format_binary_bytes, format_binary_rate, sparkline_line, bucket_series
├── resource_browser/hints.rs       # p perf, z header, host hint list
├── resource_browser/perf.rs        # unchanged table poller and sparkline_from_perf_samples
├── perf_worker.rs       # PerfRequest::Table | Overview; peek-then-debounce
├── polling_policy.rs    # table vs overview vs off
├── body_pane.rs         # PerfOverview arm; PaneAction on BodyKeyResult
├── event.rs             # OpenPerfOverview { moref, name }; PerfOverviewResult { generation }
├── app.rs               # header_hidden, z, back() arm, demand matching
├── prop_browser/prop_mgr.rs  # z header on both get_hints()
└── main.rs              # mod perf_overview

Module-local tests
├── layout invariant and spec size table
├── bucket_series, formatters, store merge
├── debounce_for and polling_policy
└── insta snapshots at 32×12, 80×24, 135×42 (header on) and 40×15 (header off)
```

**Structure Decision**: New `perf_overview` module beside `perf_worker`. The table poller in
`resource_browser/perf.rs` stays the six-slot path. Shared binary formatting goes in the existing
`formatting.rs` so badges and later charts use one rule. History stays `History::Resource`. No new
config module.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
| --------- | ---------- | ------------------------------------ |
| None | N/A | N/A |

## Phase 0: Research

Research is captured in [research.md](./research.md). Decisions that bind implementation:

- One prepared query, eight counters, `disk.read` / `disk.write`, aggregate instance, `.latest(90)`.
- Persist raw-aligned samples. Do not use `window::<90>` (left-pads gaps) or `ScaledSeries::samples()` (drops gaps).
- `PerfRequest::Table | Overview`. Overview debounce is none. Transport errors still publish a result after the store records `ERR`.
- Table poll pauses for this pane. PropertyCollector stays off. Name is captured at open.
- Layout is a pure function. Phase A fills chart slots with `Sparkline` bucketed to inner width. Phase B uses `Chart` / `GraphType::Line` with one dataset per contiguous run. Tick labels are compact relative durations (`30m`, `15m`, `0`).
- `z` lives on `App` and is handled before panes, except during search.
- Esc uses `PaneAction::Back`. Backspace and `q` fall through to `app.rs`.
- Real vCenter disk check is manual. v1 does not sum `Instance::All`.

## Phase 1: Design

Design artifacts:

- [data-model.md](./data-model.md)
- [contracts/perf-overview-ui.md](./contracts/perf-overview-ui.md)
- [quickstart.md](./quickstart.md)

### Implementation sketch

1. **Layout** (`perf_overview/layout.rs`). `perf_overview_layout(width, height, focus) -> PerfLayout`
   implements the first-match table in the contract. Rects are body coordinates. Tests cover the
   spec’s terminal rows and the invariant (widths 1–200, heights 1–60, four focus values).

2. **Formatters** (`formatting.rs`). `format_binary_bytes(i128)`, `format_binary_rate(f64)` where
   the rate input is bytes/sec, and `bucket_series(&[Option<f64>], width) -> Vec<Option<f64>>`.
   `sparkline_line` is used only for a one-row spark. `sparkline_from_perf_samples` stays on the
   table path.

3. **Store and query**. `metrics.rs` builds the overview `PreparedQuery` (do not reuse the table
   query of four counters and six samples). `store.rs` applies `EntityStatus` with the merge table
   in the data model. Scale with `Dimension::scale` on non-negative raw values so gaps stay `None`.

4. **Worker**. Change the wake path to `borrow`, optional `debounce_for`, then `borrow_and_update`.
   `Table` with an empty entity list continues without a fetch. `Overview` always fetches that one
   MoRef. On `Err`, write `ERR` into the store, then send `PerfOverviewResult`. On `Ok`, apply
   entity status, then send the same event. Ignore results whose generation does not match, as
   `PerfResult` already does.

5. **Demand**. `refresh_polling_demand` asks the body pane which mode it wants.
   `polling_policy` returns table, overview, or off. Summary modals still pause the table.
   `PerfDemandState::matches` learns the overview shape (`Arc` identity of the store plus the
   MoRef and generation).

6. **Pane**. `BodyPane::PerfOverview`. `p` on VM and host rows calls `save_state` and sends
   `OpenPerfOverview { moref, name }` from `selected_item()`. App swaps the body pane and bumps
   `perf_generation`. `back()` gains a `PerfOverview` arm that rebuilds `ResourceManager` from
   `History::Resource`, matching the property-browser arms. `BodyKeyResult.action` is applied
   after `handle_key` when it is `Some(Back)`.

7. **Header**. `header_hidden` on `App`. After Ctrl-C, if the key is `z` and search is not active,
   toggle and return. `draw` uses `Constraint::Length(0)` or `Length(5)`. Update hint lists as in
   the contract. `decorate_hints` already splits on the first token, so `Tab`, `Esc`, and `1`–`4`
   need no new decorator.

8. **Render**. Title row from the store when body height ≥ 8. Badges use the formatters. Chart
   and tall spark slots: `bucket_series` to inner width, then `Sparkline` with `Option<u64>`,
   absent symbol `" "`, and `.max(100)` for CPU or `.max(window_max.max(1))` for other domains.
   Phase B swaps chart slots to `Chart`. Two datasets for network and for disk. Y headroom is
   about 10% except CPU, which stays 0–100. Gaps are separate datasets, not `None` points inside
   one line (Ratatui `GraphType::Line` connects consecutive points).

### Agent context

Updated `.cursor/rules/specify-rules.mdc` to reference this plan.

## Phase 2: Tasks (next command)

`/speckit-tasks` splits Phase A (query, store, layout, badges, sparklines, keys, header, tests)
from Phase B (line charts and the two snapshot refreshes). The real vCenter disk check stays a
manual quickstart step, not a CI test.
