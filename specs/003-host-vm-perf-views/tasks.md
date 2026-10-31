# Tasks: Host and VM Performance Overview

**Input**: Design documents from `/specs/003-host-vm-perf-views/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/perf-overview-ui.md, quickstart.md

**Tests**: Tests are REQUIRED by the constitution and the plan. Each user story lists failing tests before implementation tasks.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- Rust TUI modules under `src/`. Tests live next to the module they cover.
- Do not add a `tests/` integration binary for this feature. Layout, store, and key behavior are unit-tested. `vcsim` does not prove aggregate disk throughput.
- Snapshots use `insta` in `src/perf_overview/render.rs`.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the module skeleton and confirm the extension points the plan names.

- [X] T001 Create empty modules `src/perf_overview/mod.rs`, `layout.rs`, `metrics.rs`, `store.rs`, `render.rs`, and `ui.rs`, and add `mod perf_overview;` in `src/main.rs`
- [X] T002 [P] Confirm the table poll still sleeps 500ms before `borrow_and_update` and sends `AppEvent::PerfResult` only on `Ok` in `src/perf_worker.rs`, and that `refresh_polling_demand` only builds a table request in `src/app.rs`. Do not change behavior
- [X] T003 [P] Confirm `BodyKeyResult` is `{ handled, new_perf_view }`, `back()` matches only the resource and property browsers, and `App::draw` uses `Constraint::Length(5)` for the header in `src/body_pane.rs` and `src/app.rs`. Do not change behavior

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types and the request enum every story compiles against. Table sparkline polling must still work when this phase ends.

**CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 [P] Add `PaneAction::Back` and `action: Option<PaneAction>` on `BodyKeyResult` in `src/body_pane.rs`. Existing arms set `action: None`. `App::handle_terminal_event` does not apply the action yet
- [X] T005 Replace the `PerfRequest` struct with `PerfRequest::Table { entities, generation, snapshot }` and `PerfRequest::Overview { entity, generation, store }` in `src/perf_worker.rs`. Map `initial()` and `paused()` onto `Table`. Keep the current 500ms sleep and table fetch so an empty entity list still skips `QueryPerf`
- [X] T006 Update `PerfDemandState` and every `PerfRequest` constructor in `src/app.rs` so table demand still matches on generation, entity list, and snapshot `Arc` identity. Leave the overview variant unmatched until US1
- [X] T007 [P] Add `PerfPollMode::{Off, Table, Overview}` in `src/polling_policy.rs`. `Table` keeps today’s rule (resource browser and no summary modal). `Overview` and `Off` are defined but not yet selected by `App`
- [X] T008 [P] Define `Domain` (CPU, Memory, Network, Disk), `SlotKind` (`Chart`, `Badge`, `BadgeWithSparkline`, `Sparkline`), `Slot`, and `PerfLayout` in `src/perf_overview/layout.rs` per `data-model.md`. No layout function yet
- [X] T009 [P] Define `PerfOverviewStore` (`name`, `kind`, per-counter samples, `error`, `sample_time`) behind `Arc<RwLock<...>>` in `src/perf_overview/store.rs`. No status merge yet

**Checkpoint**: `cargo check` succeeds. Inventory sparklines still poll. Newtypes exist for the overview.

---

## Phase 3: User Story 1 - Open a Live Overview and Leave It (Priority: P1) MVP

**Goal**: `p` on a VM or host replaces the inventory with the overview. Esc and Backspace restore that inventory. `q` quits. `r`, `s`, and `/` do nothing. The title shows the captured name when the body is at least 8 rows tall.

**Independent Test**: On a VM row and a host row, press `p`, confirm the table is gone, then leave with Esc and with Backspace. Press `q` on the overview and confirm the process exits. Press `r`, `s`, and `/` and confirm nothing opens.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T010 [P] [US1] Add key tests in `src/perf_overview/ui.rs`: Esc sets `PaneAction::Back`; Backspace and `q` leave `handled == false`; `r`, `s`, and `/` set `handled == true` and `action: None`; Tab, `KeyCode::BackTab`, and `1`–`4` set `handled == true`
- [X] T011 [P] [US1] Add hint tests in `src/resource_browser/hints.rs`: VM expand hints are `s summary`, `x actions`, `p perf`, `t task`, `e events`; host expand hints are exactly `n network`, `d datastore`, `s summary`, `p perf`, `v vm`
- [X] T012 [P] [US1] Add polling tests in `src/polling_policy.rs`: overview mode is `PerfPollMode::Overview` and does not request PropertyCollector waits; the resource browser with no summary modal stays `Table`
- [X] T013 [P] [US1] Add an overview hint test in `src/perf_overview/mod.rs`: expand lines `Tab focus`, `1 cpu`, `2 mem`, `3 net`, `4 disk`; help lines `Esc back`, `z header`, `q quit`

### Implementation for User Story 1

- [X] T014 [US1] Add `AppEvent::OpenPerfOverview { moref, name }` in `src/event.rs`
- [X] T015 [P] [US1] Handle `p` only for `ResourceType::VirtualMachine` and `ResourceType::Host` in `src/resource_browser/resource_mgr.rs`: call `save_state`, send `OpenPerfOverview` with both fields from `selected_item()`, return `handled: true` and `new_perf_view: false`. Other resource types ignore `p`
- [X] T016 [P] [US1] Add `BodyPane::PerfOverview` in `src/body_pane.rs` and the open/render/key/hint delegates in `src/perf_overview/mod.rs`. `wants_property_collector_waits` is false for this variant. Key handling calls `src/perf_overview/ui.rs`
- [X] T017 [US1] Handle `OpenPerfOverview` in `src/app.rs`: replace the body pane, bump `perf_generation` once, send `PerfRequest::Overview` for that MoRef and a new store holding the captured name, then `refresh_polling_demand`
- [X] T018 [US1] In `src/app.rs`, after `handle_key`, call `back()` when `action` is `Some(PaneAction::Back)`. Add a `BodyPane::PerfOverview` arm in `back()` that rebuilds `ResourceManager` from `History::Resource` the same way the property-browser arms do, bumps `perf_generation`, and calls `refresh_polling_demand`
- [X] T019 [US1] Render the title row in `src/perf_overview/render.rs` only when body height is at least 8: object name, `realtime 20s`, `30m`, and `sample —` until a sample time exists. Below 8 rows, draw no title
- [X] T020 [US1] Install the VM, host, and overview hint strings from T011 and T013 in `src/resource_browser/hints.rs` and `src/perf_overview/mod.rs`

**Checkpoint**: US1 is usable with an empty body. Inventory returns on Esc and Backspace. `q` still reaches the quit arm in `src/app.rs`. Table sparklines resume after `back()`.

---

## Phase 4: User Story 2 - Read All Four Resources at Any Practical Size (Priority: P1)

**Goal**: CPU, memory, network, and disk appear once whenever content height is at least 4. Layout follows the contract table. Large slots are line charts; smaller slots are sparklines or badges. Focus chooses the primary slot.

**Independent Test**: With fixture samples, resize through 32×12, 72×16, 58×17, 80×24, 135×42, and 40×15 with the header hidden. Template ids match the contract. Tab and `1`–`4` move the primary slot.

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T021 [P] [US2] Add layout tests in `src/perf_overview/layout.rs` for every checked size in `contracts/perf-overview-ui.md` (32×12 header on → template 8 and no title, 80×24 → 1, 135×42 → 1, 72×16 → 4, 58×17 → 6, 40×15 header on → 8, 40×15 header off body 40×15 → 7, 40×45 → 3) plus the invariant for widths `1..=200`, heights `1..=60`, and all four `Domain` values
- [X] T022 [P] [US2] Add formatter and bucket tests in `src/resource_browser/formatting.rs`: one decimal below 100 and none at 100 or above, no space before the unit, one unit per rate pair chosen from the larger absolute value, CPU MHz integer and GHz at 10000 MHz, `bucket_series` repeats samples when `width > n` and averages present samples when `width < n`, and a bucket with no present samples is `None`
- [X] T023 [P] [US2] Add `insta` snapshots of the sparkline-filled overview at terminal sizes 32×12, 80×24, and 135×42 with the header on in `src/perf_overview/render.rs`

### Implementation for User Story 2

- [X] T024 [P] [US2] Implement `perf_overview_layout(width, height, focus) -> PerfLayout` with first-match templates 1–9 in `src/perf_overview/layout.rs`. Content height is body height minus 1 when body height is at least 8. Chart rects are at least 36×8. Focus order is the focused domain, then CPU, Memory, Network, Disk, skipping the focused one. Template 8’s sparkline is the focused domain
- [X] T025 [P] [US2] Implement `format_binary_bytes(i128)`, `format_binary_rate(f64)` (input is bytes/sec), `bucket_series`, and `sparkline_line` in `src/resource_browser/formatting.rs`. Do not change `sparkline_from_perf_samples` or `format_compact_bitrate_bps`
- [X] T026 [US2] Draw badges and sparklines from `PerfLayout` in `src/perf_overview/render.rs`. A chart cell’s badge is the rounded-border title. Badge-only rows are one clipped line. A one-row spark uses `sparkline_line`. Taller spark areas and chart slots use Ratatui `Sparkline` with data already passed through `bucket_series(inner.width)`, `Option<u64>`, absent symbol `" "`, `.max(100)` for CPU, and `.max(window_max.max(1))` otherwise. Do not call `sparkline_from_perf_samples`
- [X] T027 [US2] Keep focus on the overview state in `src/perf_overview/ui.rs` (default CPU) and pass it into `perf_overview_layout` from `src/perf_overview/render.rs`
- [X] T028 [US2] Add chart tests in `src/perf_overview/render.rs`: network and disk chart slots build two datasets, a gap (`None`) splits a line into separate datasets, the legend is omitted when the panel is narrower than 50 columns, CPU Y stays 0–100, and other Y scales use about 10% headroom above the window peak
- [X] T029 [US2] Replace chart-slot `Sparkline` widgets with Ratatui `Chart` and `GraphType::Line` in `src/perf_overview/render.rs`. X is seconds before the newest returned sample. Tick labels are compact relative durations such as `30m`, `15m`, and `0`. Colors match the contract. Rail bands and template 8 stay sparklines
- [X] T030 [US2] Refresh the `insta` snapshots at 135×42 with the header on and 40×15 with the header off in `src/perf_overview/render.rs` so those sizes show line charts

**Checkpoint**: Fixture data produces the contract layouts. 135×42 shows four line-chart panels. 32×12 shows four badges and a focused sparkline with no title.

---

## Phase 5: User Story 3 - Trust the Latest Window, Including Gaps and Failures (Priority: P1)

**Goal**: Each poll replaces the 90-sample window for one object. The title clock is the newest sample time. Empty, missing, and failed refreshes follow the store table. Gaps stay gaps.

**Independent Test**: Drive the store with `Ok`, a missing disk series, `NoSamples`, and a transport `Err` after a good window. Confirm replace vs keep vs clear, and that a later `Ok` clears `ERR`. Confirm `debounce_for` is `None` for overview and 500ms for table.

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T031 [P] [US3] Add store tests in `src/perf_overview/store.rs`: `Ok` replaces samples and clears `ERR`; `NoSamples`, `NoMetricsAvailable`, and `Unsupported` clear the window and set `sample —`; entity `Failed` and a transport `Err` keep the previous window and set `ERR`; a later `Ok` clears `ERR`; a missing counter blanks only that domain; stored points come from raw values beside timestamps, `raw < 0` is `None`, and a short series is not left-padded to 90
- [X] T032 [P] [US3] Add worker tests in `src/perf_worker.rs`: `debounce_for(Table)` is `Some(500ms)`, `debounce_for(Overview)` is `None`, and a fetch `Err` records `ERR` on the store before `AppEvent::PerfOverviewResult` is sent

### Implementation for User Story 3

- [X] T033 [P] [US3] Build one overview `PreparedQuery` in `src/perf_overview/metrics.rs`: `cpu::USAGE_AVERAGE`, `cpu::USAGEMHZ_AVERAGE`, `mem::CONSUMED_AVERAGE`, `mem::USAGE_AVERAGE`, `net::RECEIVED_AVERAGE`, `net::TRANSMITTED_AVERAGE`, `disk::READ_AVERAGE`, `disk::WRITE_AVERAGE`, `Interval::Realtime`, `.latest(90)`, default aggregate instance. Do not use `virtualDisk.*` or the table query in `src/resource_browser/perf.rs`
- [X] T034 [P] [US3] Implement status application in `src/perf_overview/store.rs`. Scale non-negative raw values with the counter dimension (`Bytes`, `BytesPerSecond`, `Hertz::as_mhz()`, percent). Do not call `Series::window::<90>` or persist only `ScaledSeries::samples()`
- [X] T035 [US3] Change the wake loop in `src/perf_worker.rs` to `borrow`, sleep only when `debounce_for` is `Some`, then `borrow_and_update` and fetch that latest request. `Table` with an empty entity list skips the fetch. `Overview` fetches that one MoRef with the query from `src/perf_overview/metrics.rs`. Send `AppEvent::PerfOverviewResult { generation }` on success and on transport error
- [X] T036 [US3] Add `AppEvent::PerfOverviewResult { generation }` in `src/event.rs` and handle it in `src/app.rs`: ignore a mismatched generation, otherwise set `pending_redraw`
- [X] T037 [US3] Read `PerfOverviewStore` from `src/perf_overview/render.rs`: newest sample time as `sample HH:MM:SSZ`, `sample —` when empty, `ERR` plus a short message clipped to one title line, and `No samples` on a domain whose series is missing. Network and disk sparklines sum the two sides (missing side is 0; both missing is `None`)
- [X] T038 [US3] Select `PerfPollMode::Overview` only while that body pane is active, and `Table` only for the resource browser with no summary modal, in `src/polling_policy.rs` and `src/app.rs`. Leaving the overview must resume the table request

**Checkpoint**: A live poll replaces the window. Failures paint `ERR` without another key. Power-off clears to `sample —` on the next tick. The table query does not run during the overview.

---

## Phase 6: User Story 4 - Hide the Header on a Short Side Monitor (Priority: P2)

**Goal**: `z` hides the five-row header for every view for the rest of the session, except while search is active. Leaving the overview does not show the header again.

**Independent Test**: Press `z` on the inventory, the overview, and the property browser. The body grows by five rows. Leave the overview and confirm the header stays hidden. Type `z` during search and confirm it goes into the filter.

### Tests for User Story 4

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T039 [P] [US4] Add header tests in `src/app.rs`: hidden uses header height 0 and visible uses height 5; `back()` does not clear `header_hidden`; `z` toggles when search is inactive and does not toggle when search is active
- [X] T040 [P] [US4] Add help-hint tests in `src/resource_browser/hints.rs`: `HELP_HINTS` is `q quit`, `/ search`, `r resource`, `z header`, `Enter props`; `HELP_HINTS_EVENTS` is `q quit`, `/ search`, `r resource`, `z header`, `Enter details`
- [X] T041 [P] [US4] Add property-browser hint tests in `src/prop_browser/prop_mgr.rs`: both `get_hints()` help lists include `z header` and keep `q quit`, `r resource`, `j dump json`, and `Enter open`

### Implementation for User Story 4

- [X] T042 [US4] Add `header_hidden: bool` (default false) on `App` in `src/app.rs`. Handle `z` immediately after Ctrl-C and before modals, the body pane, and the `q` / `r` / Backspace fallthrough, unless `search_state.is_active()`. `draw` uses `Constraint::Length(0)` or `Constraint::Length(5)`
- [X] T043 [P] [US4] Update `HELP_HINTS` and `HELP_HINTS_EVENTS` in `src/resource_browser/hints.rs` to the lists in T040. Number keys still sort; only the visible `0..9 sort` line is removed
- [X] T044 [P] [US4] Append `z header` to both `get_hints()` help lists in `src/prop_browser/prop_mgr.rs`

**Checkpoint**: A 40×15 terminal with the header hidden uses layout 7. The flag survives `back()`.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Regression safety and the quality gates.

- [X] T045 [P] Add a regression test in `src/resource_browser/perf.rs` that the table prepared query is still the four CPU/memory counters and `latest(SLOTS)` with `SLOTS == 6`
- [X] T046 Run the manual steps in `specs/003-host-vm-perf-views/quickstart.md`, including the real vCenter disk check (host, VM on VMFS, VM on NFS or vSAN). Do not treat `vcsim` as that check. Do not add an `Instance::All` sum in this task
- [X] T047 [P] Run `cargo fmt --check`
- [X] T048 [P] Run `cargo clippy --all-targets --all-features -- -D warnings`
- [X] T049 Run `cargo test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies
- **Foundational (Phase 2)**: Depends on Setup. Blocks every user story
- **User Story 1 (Phase 3)**: Depends on Foundational. MVP
- **User Story 2 (Phase 4)**: Depends on Foundational and on the overview pane from US1 (T016, T019)
- **User Story 3 (Phase 5)**: Depends on Foundational. Store and debounce tests do not need US2. Rendering `ERR` and sample time (T037) should land after US2 render (T026)
- **User Story 4 (Phase 6)**: Depends on Foundational. Hint and header work do not need charts. The 40×15 layout-7 check needs US2 layout (T024)
- **Polish (Phase 7)**: Depends on the stories you intend to ship

### User Story Dependencies

- **US1 (P1)**: After Phase 2. No dependency on US2, US3, or US4
- **US2 (P1)**: After US1 pane and title exist. Layout math itself does not need a live poll
- **US3 (P1)**: After Phase 2 for the worker and store. Connects to US2 render to show the window. Chart gap datasets from T029 must exist before the chart half of T028 is meaningful; sparkline gaps can be checked against T026
- **US4 (P2)**: After Phase 2. Independent of the perf query. Best checked after US2 so hiding the header changes a real layout

### Within Each User Story

- Tests fail before implementation
- US2 layout function (T024) and formatters (T025) before sparkline render (T026)
- US2 sparkline snapshots (T023) update when T026 lands; chart snapshots (T030) update when T029 lands
- US3 query and store (T033, T034) before the worker fetch (T035)

### Parallel Opportunities

- Phase 1: T002 and T003 in parallel
- Phase 2: T004, T007, T008, and T009 in parallel; T006 after T005
- US1 tests: T010, T011, T012, and T013 in parallel
- US1 implementation: T015 and T016 in parallel after T014
- US2 tests: T021, T022, and T023 in parallel
- US2 implementation: T024 and T025 in parallel
- US3 tests: T031 and T032 in parallel
- US3 implementation: T033 and T034 in parallel
- US4 tests: T039, T040, and T041 in parallel
- US4 implementation: T043 and T044 in parallel after T042 starts, or with T042 because they touch different files
- Polish: T045, T047, and T048 in parallel; T049 after fmt and clippy are clean

---

## Parallel Example: User Story 2

```bash
# Tests together, after Phase 2:
# T021 layout table and invariant in src/perf_overview/layout.rs
# T022 formatters and bucket_series in src/resource_browser/formatting.rs
# T023 sparkline snapshots in src/perf_overview/render.rs

# Then implementation that does not share a file:
# T024 perf_overview_layout in src/perf_overview/layout.rs
# T025 binary formatters and bucket_series in src/resource_browser/formatting.rs
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 and Phase 2
2. Complete Phase 3 (US1)
3. Stop and validate: `p`, Esc, Backspace, `q`, and ignored `r` / `s` / `/` on a VM and a host

### Incremental Delivery

1. Setup + Foundational → types exist and table polling still works
2. US1 → open, leave, quit, hints, title (MVP)
3. US2 through T027 → badges and sparklines at the contract sizes
4. US2 T028–T030 → line charts in chart slots
5. US3 → live replace, `ERR`, gaps, table poll paused
6. US4 → session header toggle
7. Polish → table-poll regression, quickstart, fmt, clippy, test

### Parallel Team Strategy

1. Finish Setup and Foundational together
2. Then:
   - Developer A: US1 keys, pane, and history
   - Developer B: US2 layout and formatters
   - Developer C: US3 store and worker, after T005
3. US4 can start as soon as Phase 2 is done

---

## Notes

- Do not reuse `sparkline_from_perf_samples` for this view. `None` must stay a gap, not `▁`
- Do not sum `Instance::All` in v1. An empty disk series blanks that domain
- `q` is never consumed by the overview
- `header_hidden` is not part of the history record
- Commit after each task or logical group
- Stop at any checkpoint to validate that story on its own
