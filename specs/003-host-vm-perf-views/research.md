# Research: Host and VM Performance Overview

## Decision: One shared realtime query for host and VM

**Decision**: The overview uses one `PreparedQuery`: `Interval::Realtime`, `.latest(90)`, default
`Instance::Aggregate` (`""`). Counters are `cpu::USAGE_AVERAGE`, `cpu::USAGEMHZ_AVERAGE`,
`mem::CONSUMED_AVERAGE`, `mem::USAGE_AVERAGE`, `net::RECEIVED_AVERAGE`, `net::TRANSMITTED_AVERAGE`,
`disk::READ_AVERAGE`, and `disk::WRITE_AVERAGE`. VM and host share that query.

**Rationale**: After dropping `virtualDisk.*`, every v1 counter admits both `VirtualMachine` and
`HostSystem`. `disk::READ_AVERAGE` / `disk::WRITE_AVERAGE` are level 2 with per-device level 3 and
scope `DatastoreVmHost`, so a whole-entity series exists. `virtualDisk.read` / `write` are
`VmOnly`, per virtual disk, and often have no aggregate; `vcsim` hides that. `PreparedQueryBuilder`
does not enforce scope. A counter that does not apply comes back as `PerfWarning::ScopeMismatch`
while entity status stays `Ok`. A missing series blanks only that domain.

**Alternatives considered**:

- `virtualDisk.*` plus a client-side sum: rejected for v1 because the aggregate is often empty on
  real ESXi and IOPS is out of scope until a real vCenter check passes.
- Separate VM and host queries: rejected until a `VmOnly` or `HostOnly` counter is added.
- `disk.usage.average`: rejected because the view needs read and write separately.

## Decision: Store returned samples, not a padded window

**Decision**: Persist each series by walking raw values beside timestamps. `raw < 0` is a gap
(`None`). `raw >= 0` is scaled with the counter’s dimension. Do not call `Series::window::<90>`
and do not persist only `ScaledSeries::samples()`.

**Rationale**: `window::<N>` returns the newest N slots left-padded with `None`. A VM with 11
samples would draw 79 leading gaps. `ScaledSeries::samples()` skips negative raw values, so the
gap disappears and a chart line would connect across the hole. The spec requires the axis to span
samples actually returned, with gaps kept as breaks.

**Alternatives considered**:

- Fixed `[Option<T>; 90]` like the table’s six slots: rejected because padding is a visible hole.
- Ring buffer plus watermark: rejected. The table sparkline bug was duplicate samples accumulating
  across polls. The overview replaces the whole window on every tick.

## Decision: Scale with `vim_rs`, format in vtui

**Decision**: Memory uses `Bytes` (`mem.consumed` kiloBytes × 1024). Rates use `BytesPerSecond`
(`kiloBytesPerSecond` × 1024 via `throughput_factor`). CPU megahertz is `Hertz::as_mhz()` and is
badge-only. New `format_binary_bytes` and `format_binary_rate` live in
`resource_browser/formatting.rs`. Do not call `format_compact_bitrate_bps` (decimal bits) or
`vim_rs`’s own `format_iec_rate` (different precision and a space before the unit).

**Rationale**: The wire unit is already scaled inside `vim_rs::perf`. The overview’s precision rule
(one decimal below 100, none at 100 or above, no space before the unit, one unit per pair) is
vtui-specific. A pair picks the unit from the larger absolute value.

**Alternatives considered**:

- Decimal MB/s: rejected by the spec.
- Dual Y axis for CPU percent and MHz: rejected. The plot is 0–100 percent.

## Decision: Split the perf request and skip overview debounce

**Decision**: Replace the single `PerfRequest` struct with `PerfRequest::Table { entities, generation, snapshot }`
and `PerfRequest::Overview { entity, generation, store }`. An empty table entity list still means
paused. The worker peeks with `borrow`, sleeps only when `debounce_for` is `Some(500ms)` for
`Table`, then `borrow_and_update` and fetches that latest request. Overview debounce is `None`.
`PerfOverviewResult { generation }` is sent on success and on transport error. The event carries no
samples. A fetch `Err` writes `ERR` into the store before the event is sent. Today a fetch `Err`
is only logged, so the UI would never paint `ERR`.

**Rationale**: The current worker sleeps 500ms before `borrow_and_update`, so it cannot tell a
table refresh from an overview open. Overview should fetch immediately. Generation matching already
exists for `PerfResult` and is reused so a result for a pane the operator has left does not redraw
as current.

**Alternatives considered**:

- Second worker task: rejected. One `PerformanceManager` client and one watch channel are enough.
- Keep sleeping 500ms for overview: rejected. Opening `p` should not wait out a table debounce.
- Put samples on the event: rejected. The table already publishes through a shared store and a
  generation token. The overview follows that so a large 90-sample payload is not cloned through
  the UI channel.

## Decision: Pause the table poll while the overview is open

**Decision**: `polling_policy` grows a mode: table sparklines only for the resource browser with no
summary modal; overview only while that body pane is active; otherwise paused. PropertyCollector
waits stay off for the overview. `back()` restores the resource browser from the existing history
record and the table poll resumes. The header flag is not part of that record.

**Rationale**: Constitution III. One entity, eight counters, 90 samples, and the six-slot table
query do not run together. The title name comes from `OpenPerfOverview`. Power-off is
`EntityStatus::NoSamples` on the next tick, which clears the window. That is acceptable for v1.

**Alternatives considered**:

- Leave table polling running underneath: rejected. It doubles `QueryPerf` for no visible rows.
- PropertyCollector watch to refresh the title: rejected as a non-goal.

## Decision: Pure layout function, sparkline first, line chart second

**Decision**: `perf_overview_layout(width, height, focus) -> PerfLayout` is the only size source.
Tests assert template ids from the spec tables and the non-overlap invariant for every width
`1..=200`, height `1..=60`, and all four focus values. Phase A draws chart slots and any spark
area taller than one row with Ratatui 0.30 `Sparkline` (`Option<u64>`, `absent_value_symbol` a
space, data already bucketed to inner width, `.max` set). A one-row spark uses `sparkline_line`.
Phase B replaces chart-slot sparklines with `Chart` / `GraphType::Line`. Each contiguous run of
`raw >= 0` samples is its own `Dataset`. Network and disk charts draw both series. X is seconds
before the newest returned sample. Tick labels are compact relative durations (`30m`, `15m`, `0`),
not wall-clock times. Legend only when the panel is at least 50 columns wide.

**Rationale**: Ratatui 0.30 `Sparkline` drops columns past the rect and treats `None` as absent,
which matches gaps if the caller buckets first. Passing all 90 samples into a ~34-column cell
would show the oldest samples. `sparkline_from_perf_samples` maps `None` to `▁` and assumes
percent hundredths on 0–10000, so the overview must not call it. Shipping the query, store, and
layout before `Chart` puts the risky data path under test first. Layout geometry does not change
when bars become lines.

**Alternatives considered**:

- Draw `sparkline_line` once per row of a tall cell: rejected. It repeats one glyph row instead of
  filling the rect.
- Fixed X axis `[-1800, 0]`: rejected. A short series would sit in the corner with a false scale.
- Wall-clock tick labels: rejected. The title already shows the newest sample time. The axis is
  “how long before that sample”.

## Decision: Header toggle is app state, `z` is handled before panes

**Decision**: `App` stores `header_hidden` (default false). `draw` uses `Constraint::Length(0)` or
`Length(5)`. `z` is handled in `handle_terminal_event` immediately after Ctrl-C, before modals,
the body pane, and the `q` / `r` / Backspace fallthrough, unless search is active. Search consumes
every character. `back()` does not clear the flag.

**Rationale**: The header is one widget in `App::draw`, so the flag cannot live on a pane. Summary
and the property browser would swallow `z` if it were handled only in the body. The hint column is
16×5. VM hints gain `p perf` and still fit. Host hints become exactly five visible lines. Shared
help drops `0..9 sort` to fit `z header`. Both property-browser `get_hints()` lists have a free
row today (`q quit`, `r resource`, `j dump json`, `Enter open`).

**Alternatives considered**:

- Per-pane header flag: rejected. Leaving the overview must keep the extra rows.
- Key `h`: rejected. Inventory `h` expands hosts.
- Persist the flag in config: rejected. Session only.

## Decision: Leave through the existing history path

**Decision**: `p` on a VM or host row calls `ResourceManager::save_state` (which already emits
`ResourceManagerHistory`) and `AppEvent::OpenPerfOverview { moref, name }` from `selected_item()`.
`BodyKeyResult` gains `action: Option<PaneAction>` with `PaneAction::Back`. Esc on the overview
sets that action. Backspace is not consumed and falls through to `App::back`. `q`, and unhandled
keys, also fall through. `back()` matches every `BodyPane` variant, so `PerfOverview` is a required
new arm and rebuilds `ResourceManager` from the history record the same way the property browsers
do. `r`, `s`, and `/` on the overview are consumed and do nothing.

**Rationale**: `selected_item()` already returns `(ManagedObjectReference, String)`. History
restore already bumps `perf_generation` and calls `refresh_polling_demand`. A new back stack would
fork that behavior. `q` must stay the quit key in `app.rs`.

**Alternatives considered**:

- Make `q` leave the overview: rejected by the spec.
- Consume Backspace inside the pane and call `back()` there: rejected. The app already owns
  history pop.

## Decision: Real vCenter disk check is a manual exit gate

**Decision**: v1 code requests aggregate `disk.read` / `disk.write` only. It does not sum
`Instance::All`. Quickstart records the manual check: non-empty aggregate series for a host, a VM
on VMFS, and a VM on NFS or vSAN. If a VM series is empty, the follow-up is a later change. IOPS
is not implemented in this plan.

**Rationale**: `vcsim` invents numbers for per-disk counters and cannot prove the aggregate. CI
has no vCenter credentials. An empty series already blanks that domain without failing the fetch,
so a failed check does not panic. It does block calling the disk view done.

**Alternatives considered**:

- Implement `Instance::All` plus a sum now: rejected. It is the fallback, not the v1 query, and it
  would hide whether the aggregate exists.
- Block merge on an automated vCenter test: rejected. No portable credentials.
