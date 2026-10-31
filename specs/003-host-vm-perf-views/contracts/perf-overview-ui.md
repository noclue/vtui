# Contract: Host and VM Performance Overview

Observable terminal behavior and the perf query the overview is allowed to issue. Layout numbers
match [spec.md](../spec.md). This is not a public library API.

## Entry

- **Where**: Virtual-machine inventory and host inventory only.
- **Key**: `p` on the selected row.
- **Effect**: `ResourceManager::save_state` runs, then the body pane becomes the overview for that
  MoRef and captured name. `perf_generation` increments. The worker receives
  `PerfRequest::Overview` for that one object.
- **Not available from**: cluster, datastore, network, task, event, property browser, or summary
  popups.

## Keys while the overview is open

| Key | Result |
| --- | ------ |
| Tab / Shift-Tab | Move focus. Shift-Tab is `KeyCode::BackTab` |
| `1` `2` `3` `4` | Focus CPU, memory, network, disk |
| Esc | `PaneAction::Back`. App calls `back()` |
| Backspace | Not consumed. Existing `app.rs` Backspace arm calls `back()` |
| `q` | Not consumed. Application quits |
| `r`, `s`, `/` | Consumed. No resource switch, summary, or search |
| `z` | Toggles the app header even on this pane, unless search is active |

Leaving restores the inventory from the history record, bumps `perf_generation`, and resumes table
sparkline polling. It does not change header visibility.

## Header toggle

- **Key**: `z`, handled in `App::handle_terminal_event` immediately after Ctrl-C.
- **Exception**: search is active. `z` is filter text.
- **Hidden**: header constraint length 0. Body gets the full frame. Applies to inventory, property
  browser, and overview.
- **Default**: visible. Session-only. Not cleared by `back()`.

## Hint text

Hint column is 16×5.

| Surface | Expand | Help |
| ------- | ------ | ---- |
| VM inventory | `s summary`, `x actions`, `p perf`, `t task`, `e events` | `q quit`, `/ search`, `r resource`, `z header`, `Enter props` |
| Host inventory | `n network`, `d datastore`, `s summary`, `p perf`, `v vm` | same shared help. `t` and `e` still work and are not listed |
| Overview | `Tab focus`, `1 cpu`, `2 mem`, `3 net`, `4 disk` | `Esc back`, `z header`, `q quit` |
| Events inventory | unchanged expand list | `q quit`, `/ search`, `r resource`, `z header`, `Enter details` |
| Property browser (both) | empty, as today | existing lines plus `z header` |

`0..9` still sorts on the events table. The visible `0..9 sort` line is the one that drops.

## Layout

`perf_overview_layout(body_width, body_height, focus)` is the only size source. First match wins.
Content height is body height minus 1 when body height ≥ 8 (title row), otherwise body height.

| Id | Shown | Condition |
| -- | ----- | --------- |
| 1 | 2×2 charts | width ≥ 72 and content height ≥ 16 |
| 2 | 4×1 charts | width ≥ 144 and content height ≥ 8 |
| 3 | 1×4 charts | width ≥ 36 and content height ≥ 32 |
| 4 | 2 charts, then two badges side by side | width ≥ 72 and content height ≥ 9 |
| 5 | 2 stacked charts, then two badge rows | width ≥ 36 and content height ≥ 18 |
| 6 | 1 chart + 22-column rail | width ≥ 58 and content height ≥ 8 |
| 7 | 1 chart + three badge rows | width ≥ 36 and content height ≥ 11 |
| 8 | four badges, then focused sparkline if a row remains | width ≥ 21 and content height ≥ 4 |
| 9a | four clipped badges, no sparkline | width < 21 and content height ≥ 4 |
| 9b | one clipped line ending in `…` when needed | otherwise |

Checked terminals:

| Terminal | Header | Layout |
| -------- | ------ | ------ |
| 32×12 | on | 8, no title |
| 80×24 | on | 1 |
| 135×42 | on | 1 |
| 72×16 | on | 4 |
| 58×17 | on | 6 |
| 40×15 | on | 8 |
| 40×15 | off | 7 |
| 40×45 | on | 3 |

Chart rects include the border and are at least 36×8. The badge is the border title. A badge-only
domain is one clipped line. Focus picks the primary slot. No panel overlaps another or leaves the
body.

## Badges and color

```text
CPU 12% 480MHz
MEM 1.4GiB 35%
NET ↓2.1 ↑0.4MiB/s
DISK R1.2 W0.3MiB/s
```

- Binary units, no space before the unit. One decimal below 100, none at or above 100.
- CPU MHz is an integer. At 10000 MHz and above, the badge shows GHz under the same decimal rule.
- A pair uses one unit, chosen from the larger absolute value.
- Memory percent is `mem.usage.average` when present. It is not computed from capacity. There is
  no granted line.
- Colors: CPU cyan, memory green, net received blue, net transmitted magenta, disk read yellow,
  disk write red.
- Legend only when a chart panel is at least 50 columns wide.

## Refresh

- One object, eight counters, latest 90 realtime samples, aggregate instance, one `QueryPerf` per
  tick. Table polling is paused while this pane is active.
- Each success replaces the stored window. The title clock is the newest sample time in UTC
  (`sample 11:08:20Z`), not the wall clock.
- No samples: graphs clear, `sample —`, error clears.
- Transport or entity failure: previous window stays, title sample field becomes `ERR` plus a
  short message on that same line. The worker still emits `PerfOverviewResult`.
- A missing counter blanks that domain with `No samples` and does not fail the fetch.
- Gaps (`raw < 0`) break chart lines and are blank sparkline columns. Zero is the lowest bar.
- Spark and chart columns are `bucket_series` to the inner width. Wider than the sample count
  repeats samples. Narrower uses the mean of present samples in each bucket.
- Chart X spans seconds before the newest returned sample. Tick labels are compact relative
  durations. Y starts at 0. CPU is 0–100. Other domains use the window peak plus about 10%
  headroom, or 1 when the peak is 0.
- Resize only recomputes layout. It does not fetch.

## Query counters

| Domain | Counter constant | Unit after `vim_rs` scale |
| ------ | ---------------- | ------------------------- |
| CPU plot | `cpu::USAGE_AVERAGE` | percent (`raw` hundredths → percent) |
| CPU badge | `cpu::USAGEMHZ_AVERAGE` | `Hertz::as_mhz()` |
| Memory plot | `mem::CONSUMED_AVERAGE` | bytes |
| Memory badge percent | `mem::USAGE_AVERAGE` | percent |
| Network | `net::RECEIVED_AVERAGE`, `net::TRANSMITTED_AVERAGE` | bytes/sec |
| Disk | `disk::READ_AVERAGE`, `disk::WRITE_AVERAGE` | bytes/sec |

Not in this query: `virtualDisk.*`, `disk.numberReadAveraged`, host swap, balloon, `cpu.ready`.

## Out of scope

Historical presets, per-device series, IOPS, cluster / pool / datastore / GPU overviews, replacing
inventory sparklines, and persisting the header flag.
