# Data Model: Host and VM Performance Overview

## Domain

One of the four overview resources. Order is fixed: CPU, Memory, Network, Disk.

| Variant | Chart / spark series | Badge | Scale |
| ------- | -------------------- | ----- | ----- |
| `Cpu` | `cpu.usage.average` as percent | latest percent and `cpu.usagemhz.average` | Plot 0–100. MHz is badge-only |
| `Memory` | `mem.consumed.average` as bytes | consumed size, plus `mem.usage.average` percent when that series exists | Peak of the window, at least 1 |
| `Network` | received and transmitted, bytes/sec | `↓` / `↑`, one shared unit | Sum for sparklines. Both series on a chart |
| `Disk` | read and write, bytes/sec | `R` / `W`, one shared unit | Sum for sparklines. Both series on a chart |

**Validation**:

- Focus is one domain. Default is CPU.
- A missing series blanks that domain only. It does not remove the slot.
- Network or disk with one side missing treats the missing side as 0 in the sum. Both missing is a
  gap (`None`), not zero.

## SamplePoint

One timestamp in a replaced window.

| Field | Rule |
| ----- | ---- |
| `ts` | `vim_rs::perf::Timestamp` (`SystemTime`) |
| `value` | `Option<f64>` in the series unit (percent, bytes, or bytes/sec). `None` when raw `< 0` |

**Invariants**:

- Points stay aligned with the timestamps returned for that series. Leading `None` padding from
  `window::<90>` is not stored.
- At most 90 points, because the query asks for `.latest(90)`. Fewer is valid.
- The window is replaced as a whole. There is no append and no ring.

## PerfOverviewStore

Last result for the single open object. Shared with the worker the same way `PerfRowsSnapshot` is
shared (`Arc<RwLock<...>>`).

| Field | Meaning |
| ----- | ------- |
| `name` | Display name captured at open |
| `kind` | Virtual machine or host, for the title prefix |
| `series` | One sample list per counter that came back |
| `error` | `Option<String>` short message. `None` when the last status was not a failure |
| `sample_time` | Newest timestamp across present series, or none |

**Status transitions** (same merge idea as `merge_action_for_status`):

| Input | Series window | Title |
| ----- | ------------- | ----- |
| `EntityStatus::Ok` | Replace | Clear `error`. `sample_time` is the newest timestamp |
| `NoSamples`, `NoMetricsAvailable`, `Unsupported` | Clear | `sample —`. Clear `error` |
| Entity `Failed` | Keep | `ERR` plus a short message |
| Fetch `Err` (no `PerfResult`) | Keep | `ERR` plus a short message, then still publish `PerfOverviewResult` |
| Later `Ok` | Replace | Clear `error` |

A counter absent from an `Ok` entity blanks that domain and leaves the other series in place.

## PerfLayout

Pure output of `perf_overview_layout(width, height, focus)`.

| Field | Meaning |
| ----- | ------- |
| `template` | `1..=8`, or `9` for 9a/9b |
| `title` | `Option<Rect>`. Full width, height 1, `y = 0`, only when body height ≥ 8 |
| `slots` | One `Slot` per domain when content height ≥ 4. Empty for template 9b |
| `combined` | Template 9b only: one line for all four values. `None` otherwise |

`Slot` is `{ domain, rect, kind }`. `SlotKind` is `Chart`, `Badge`, `BadgeWithSparkline`, or
`Sparkline`.

**Invariants**, for every width `1..=200`, height `1..=60`, and each focus value:

- Every rect lies inside the body. No two rects overlap.
- The title, when present, is `y = 0` and does not overlap a slot.
- Every chart rect is at least 36×8, border included.
- Content height ≥ 4: four domains exactly once, `combined` is `None`.
- Content height < 4: `slots` is empty and `combined` is `Some`.

Content height is body height minus 1 when the title is shown, otherwise body height. Body size is
the rect `App::draw` already gave the pane.

Chart slots fill in focus order: the focused domain first, then CPU, Memory, Network, Disk,
skipping the focused domain. Template 8’s sparkline is the focused domain. The other three are
badge rows.

## PerfRequest

Watch-channel value from UI to `perf_worker`.

| Variant | Fields | When |
| ------- | ------ | ---- |
| `Table` | `entities`, `generation`, `snapshot` | Resource browser, summary closed. Empty `entities` means paused |
| `Overview` | `entity`, `generation`, `store` | Performance overview is the body pane |

`PerfDemandState::matches` compares generation, the entity or entity list, and store/snapshot
identity (`Arc::ptr_eq`), so an unchanged demand does not wake the worker.

`debounce_for`: `Table` → 500ms, `Overview` → none.

## PaneAction and header flag

`BodyKeyResult` is `{ handled, new_perf_view, action: Option<PaneAction> }`. `PaneAction` is
`Back` only. The overview sets `Back` for Esc. Backspace, `q`, and other unhandled keys leave
`handled` false so `app.rs` keeps ownership.

`App.header_hidden: bool` defaults to false. It is not on the history record. `back()` does not
clear it. `draw` uses header height 0 or 5.

## Relationships

```text
selected VM or Host row
        │ p  (save_state → history, OpenPerfOverview)
        ▼
PerfOverview ──reads──▶ PerfOverviewStore ◀──writes── perf worker
        │                                      ▲
        │ layout(width, height, focus)         │ PerfRequest::Overview
        ▼                                      │
PerfLayout → badges / sparklines / charts     generation must match
        │
        │ Esc or Backspace → back()
        ▼
ResourceManager restored from History::Resource
        │
        └── PerfRequest::Table resumes
```

Opening does not change `header_hidden`. Leaving does not change it either.
