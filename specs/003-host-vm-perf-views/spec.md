# Feature Specification: Host and VM Performance Overview

**Feature Branch**: `004-host-vm-perf-views`  
**Created**: 2026-10-31  
**Status**: Draft  
**Input**: User description: "Add a full-body realtime performance overview for the selected virtual machine or host, covering CPU, memory, network, and disk throughput, sized for narrow side monitors and full screens. Source: `docs/proposals/host-vm-performance-views.md`."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open a Live Overview and Leave It (Priority: P1)

An operator looking at a virtual machine or a host can replace the inventory table with a
performance overview for that one object, read who it is and how fresh the data is, and return
to the same inventory they left. Quitting the application stays the same key it is everywhere
else.

**Why this priority**: Without a way in and a way back, none of the graphs are reachable. The
overview has to feel like the existing drill-in (the property browser), not like a second
application.

**Independent Test**: On a virtual-machine row and on a host row, press the open key, confirm
the inventory table is gone and the overview fills the working area, then leave with each
documented leave key and confirm the previous inventory returns. Press the quit key on the
overview and confirm the application exits.

**Acceptance Scenarios**:

1. **Given** a virtual machine or host row is selected in its inventory, **When** the operator
   presses `p`, **Then** the working area becomes the performance overview for that object and
   the inventory table is no longer shown.
2. **Given** the overview is open, **When** the operator presses Esc or Backspace, **Then** the
   previous inventory returns, including the row context they left, and the small inventory
   sparklines resume.
3. **Given** the overview is open, **When** the operator presses `q`, **Then** the application
   quits. `q` is never a back key.
4. **Given** the overview is open, **When** the operator presses `r`, `s`, or `/`, **Then**
   nothing happens: no resource switch, no summary popup, and no search.
5. **Given** the overview was opened on an object named in the inventory, **When** the title
   row is visible, **Then** the title shows that name, that this is a realtime view of about
   the last 30 minutes at about a 20-second sample, and the time of the newest sample.

---

### User Story 2 - Read All Four Resources at Any Practical Size (Priority: P1)

The operator can see CPU, memory, network, and disk at once. On a tall wide terminal each one
is a chart. As the terminal gets shorter or narrower, domains step down from chart to sparkline
to a one-line badge, and on a very short terminal they share one clipped line. The focused
domain gets the best slot.

**Why this priority**: This is the reason the view exists. A side-by-side monitor is often
short or narrow, and a layout that only works at full screen does not help.

**Independent Test**: With sample data on screen, resize through the sizes in the layout
reference and confirm which layout appears, that all four domains are present whenever the
content area is at least four rows, and that changing focus moves the largest slot.

**Acceptance Scenarios**:

1. **Given** the body is at least 72 columns wide and the content height is at least 16 rows,
   **When** the overview draws, **Then** it shows a two-by-two set of charts (layout 1).
2. **Given** a 32×12 terminal with the application header visible (body 32×7, no title),
   **When** the overview draws, **Then** it shows four badge lines and a sparkline for the
   focused domain (layout 8).
3. **Given** a 135×42 terminal with the header visible, **When** the overview draws, **Then**
   layout 1 shows CPU, consumed memory, network throughput, and disk throughput.
4. **Given** a 72×16 terminal with the header visible, **When** the overview draws, **Then**
   the layout is two charts plus a row of two badges (layout 4), not a two-by-two.
5. **Given** a 58×17 terminal with the header visible, **When** the overview draws, **Then**
   the layout is one chart plus a right-hand rail of the other three domains (layout 6).
6. **Given** a 40×15 terminal with the header hidden, **When** the overview draws, **Then**
   the layout is one chart plus three badge rows (layout 7).
7. **Given** the content height is at least 4 rows but the body is narrower than 21 columns,
   **When** the overview draws, **Then** four badge rows are shown, clipped on the right, and
   there is no sparkline (layout 9a).
8. **Given** the content height is under 4 rows, **When** the overview draws, **Then** one
   shared line shows the values that fit and ends with `…` when they do not (layout 9b).
9. **Given** the overview is open, **When** the operator presses Tab, Shift-Tab, or `1` `2`
   `3` `4`, **Then** focus moves (CPU, memory, network, disk for the number keys) and the
   layout gives the focused domain the primary slot.
10. **Given** a domain has a latest value, **When** its badge is drawn, **Then** it matches
    the badge reference: binary units with no space before the unit, one decimal below 100,
    none at 100 or above, and one shared unit for a down/up or read/write pair.

---

### User Story 3 - Trust the Latest Window, Including Gaps and Failures (Priority: P1)

Each refresh replaces the on-screen history. The operator can tell a fresh sample from an empty
object, a powered-off object, a missing metric, and a failed refresh. A hole in the series is
a gap, not a line or a fake zero.

**Why this priority**: A performance view that accumulates duplicate points, or that hides a
failed refresh until the next keypress, trains the operator to distrust the graph.

**Independent Test**: Feed a successful window, a window with a missing disk series, an empty
result, and a failed refresh after a good window. Confirm the title and the affected domain
match the rules below, and that a later success clears the error.

**Acceptance Scenarios**:

1. **Given** a successful refresh, **When** the overview updates, **Then** the stored window is
   replaced entirely (it does not grow by appending), the title error clears, and the sample
   time is the newest sample in that window.
2. **Given** the object has no samples (including after it is powered off), **When** the next
   refresh completes, **Then** the graphs clear, the title shows `sample —`, and any previous
   error clears.
3. **Given** a refresh fails and a previous window is on screen, **When** the overview updates,
   **Then** the previous graphs stay and the sample field becomes `ERR` plus a short message,
   clipped to one line of the body width. There is no separate error banner.
4. **Given** one series is missing and the others are present, **When** the overview draws,
   **Then** only that domain shows `No samples`. The refresh itself is not treated as a failure.
5. **Given** a gap in an otherwise valid series, **When** a chart draws, **Then** the line
   breaks at the gap. A sparkline leaves a blank for the gap. A true zero is the lowest bar,
   not a blank.
6. **Given** fewer than 90 samples have been returned, **When** the overview draws, **Then**
   the time axis and the bars cover the samples that exist. They do not invent a fixed
   30-minute axis or insert holes to pad the count.

---

### User Story 4 - Hide the Header on a Short Side Monitor (Priority: P2)

An operator who parks vtui in a short terminal beside other work can hide the five-row
application header and give those rows to the current view. The choice lasts for the session,
including after leaving the overview, and it is available in every view except while a search
filter is being typed.

**Why this priority**: Several useful layouts only appear once the body is a few rows taller.
The header is the cheapest way to get them on a side monitor.

**Independent Test**: Press `z` on the inventory, on the overview, and in the property browser.
Confirm the header disappears and the body grows by five rows. Leave the overview and confirm
the header stays hidden. Start a search and confirm `z` is typed into the filter instead.

**Acceptance Scenarios**:

1. **Given** any view except an active search, **When** the operator presses `z`, **Then** the
   header hides and the body uses the full terminal height. Pressing `z` again shows the header.
2. **Given** the header is hidden, **When** the operator leaves the overview or opens another
   view, **Then** the header stays hidden for the rest of the session.
3. **Given** search is active, **When** the operator types `z`, **Then** the letter goes into
   the filter and the header does not change.
4. **Given** a 40×15 terminal, **When** the header is hidden on the overview, **Then** the body
   is 40×15 and layout 7 is used, as in User Story 2.

---

### Edge Cases

- The terminal is resized while a refresh is still in flight. Layout follows the new size on
  the next redraw. Keys still work. The in-flight result applies when it arrives, or is ignored
  if the operator has already left the overview.
- The session is over SSH or the terminal is slow. Layout uses character cells, not pixels, and
  the same size produces the same layout.
- The overview is opened before the first sample arrives. The title shows `sample —` and domains
  show no samples until a successful window replaces that empty state.
- A network or disk side is missing while the other side exists. The missing side is treated as
  zero in the combined graph. The badge still shows the two sides. If both sides are missing,
  that point is a gap.
- The body is narrower than a badge. The line is cut on the right. It does not wrap onto a
  second row.
- The chart panel is narrower than 50 columns. No legend is drawn. The border title carries
  the down/up or read/write marks.
- Host hint keys `t` and `e` still work after `p perf` takes a visible hint slot. They are
  simply not listed.
- Very large values share one unit for a pair (network down/up, disk read/write), chosen from
  the larger absolute value, so the two numbers stay comparable.
- A flat zero series still draws the lowest bar. The scale does not divide by zero.
- CPU at or above 10000 MHz is shown in GHz in the badge. The chart stays on percent.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The virtual-machine inventory and the host inventory MUST open this overview with
  `p` for the selected row. `p` does not open it from other inventories.
- **FR-002**: The overview MUST replace the inventory table in the main working area. It is not
  a small centered popup. The summary popup remains the separate `s` view and MUST NOT open on
  top of the overview.
- **FR-003**: Esc and Backspace MUST leave the overview and restore the inventory the operator
  came from, the same way leaving the property browser restores it. `q` MUST quit the
  application from the overview, as it does everywhere else.
- **FR-004**: Tab and Shift-Tab MUST move focus among CPU, memory, network, and disk. Keys `1`
  `2` `3` `4` MUST focus those domains in that order. Default focus is CPU. There is no paging
  key.
- **FR-005**: The overview MUST show four domains: CPU, consumed memory, network throughput,
  and disk throughput. Whenever the content height is at least four rows, each domain appears
  exactly once. Below that, one shared clipped line is used instead.
- **FR-006**: Layout MUST follow the layout reference below. The first matching layout wins.
  Focus chooses which domain occupies the primary chart or sparkline slot; the others follow
  in CPU, memory, network, disk order, skipping the focused domain so it is not repeated.
- **FR-007**: A chart cell MUST be at least 36 columns by 8 rows, including its border. The
  badge is that border’s title, not an extra row. A domain that is only a badge is one text
  line, clipped on the right with no wrap.
- **FR-008**: The title row MUST appear only when the body height is at least 8. It is part of
  the body, not the application header. It names the object, says the view is realtime at about
  20 seconds covering about 30 minutes, and shows the newest sample time, `sample —`, or `ERR`
  plus a short one-line message.
- **FR-009**: Each refresh MUST replace the whole window of up to the last 90 samples. Samples
  MUST NOT accumulate across refreshes. While the overview is open, only that one object is
  refreshed; the inventory sparklines pause and resume when the operator returns.
- **FR-010**: Refresh MUST NOT block key handling or redraw. A failed refresh MUST still update
  the screen (keep the last good window and show `ERR`) without waiting for another key.
- **FR-011**: CPU MUST be plotted as a percent from 0 to 100, with the latest megahertz only in
  the badge (gigahertz at 10000 MHz and above). Memory MUST be consumed size. Memory percent,
  when shown, comes from the usage figure, not from a capacity or granted line. Network MUST
  show received and transmitted. Disk MUST show whole-object read and write throughput, not
  capacity, not operations per second, and not a per-disk breakdown.
- **FR-012**: Sizes and rates MUST use binary units with no space before the unit: KiB, MiB,
  GiB, TiB, and KiB/s, MiB/s, GiB/s. A displayed number below 100 has one decimal; 100 and
  above have none. CPU megahertz is an integer until the gigahertz rule applies. A pair of
  rates uses one unit, chosen from the larger absolute value.
- **FR-013**: A missing series MUST blank only that domain (`No samples`) and MUST NOT fail
  the refresh. No samples and unsupported objects clear the window and show `sample —`. A
  failed refresh keeps the previous window and shows `ERR`. A later success clears `ERR`.
- **FR-014**: Charts MUST break the line across gaps. Sparklines MUST draw a blank for a gap
  and the lowest bar for zero. A spark area taller than one row fills that area with bars. A
  one-row spark is a single line of bar characters. Network and disk charts draw both
  directions in two colors. Their sparklines draw the sum. If only one side of a pair exists,
  the other side is zero for the sum; if both are missing, the point is a gap.
- **FR-015**: Chart time MUST run from the oldest returned sample to the newest, in seconds
  before the newest sample, not as a fixed 30-minute axis and not as a sample index. The
  vertical scale starts at zero. CPU stays 0–100. Other domains use the peak of the window
  plus about ten percent headroom, or 1 when the peak is zero.
- **FR-016**: When a graph is narrower than the sample count, each column is the average of the
  samples it covers, or a gap if that range has none. When it is wider, samples repeat across
  columns instead of inserting holes. A gap MUST NOT be drawn as zero.
- **FR-017**: `z` MUST toggle the five-row application header for the whole session, in every
  view, except while search is active. Hiding it gives the body the full terminal height.
  Leaving the overview MUST NOT reset the toggle. The default is header visible.
- **FR-018**: Hint text MUST match the hint reference below. The overview replaces both hint
  columns while it is open. `z header` is added to the shared help hints and to the property
  browser help. The events help drops the visible `0..9 sort` line to make room; number keys
  still sort.
- **FR-019**: The view MUST stay usable on macOS, Windows, Linux, and SSH sessions. Resize
  only changes layout. No new files are written for this view.
- **FR-020**: Series colors on the dark background MUST be: CPU cyan, memory green, network
  received blue and transmitted magenta, disk read yellow and write red. A legend appears
  only when a chart panel is at least 50 columns wide. There is no second vertical axis.
- **FR-021**: This feature MUST NOT replace the inventory-table sparklines, and MUST NOT add
  historical ranges, per-CPU / per-NIC / per-disk series, an operations-per-second toggle,
  host memory-rate or balloon charts, or overviews for clusters, resource pools, datastores,
  or GPUs.

### Layout Reference

Body width and height are the working area after the application header. With the header
visible, body height is terminal height minus 5. With the header hidden, body height is the
terminal height. The title is shown only when body height is at least 8, and then content
height is body height minus 1. Otherwise content height equals body height. Every layout
condition uses content height.

| Layout | What the operator sees | When |
| ------ | ---------------------- | ---- |
| 1 | Four charts, two by two | width ≥ 72 and content height ≥ 16 |
| 2 | Four charts in one row | width ≥ 144 and content height ≥ 8 |
| 3 | Four charts in one column | width ≥ 36 and content height ≥ 32 |
| 4 | Two charts, then one row of two side-by-side badges | width ≥ 72 and content height ≥ 9 |
| 5 | Two charts stacked, then two badge rows | width ≥ 36 and content height ≥ 18 |
| 6 | One chart plus a 22-column rail. Each other domain gets an equal band: badge on the first row, sparkline below | width ≥ 58 and content height ≥ 8 |
| 7 | One chart, then three badge rows | width ≥ 36 and content height ≥ 11 |
| 8 | Four badge rows, then a sparkline for the focused domain if at least one row remains | width ≥ 21 and content height ≥ 4 |
| 9a | Four badge rows, clipped, no sparkline | width < 21 and content height ≥ 4 |
| 9b | One shared line, cut off with `…` | anything else |

Three-across and three-down grids are not used. Panels must lie inside the body and must not
overlap. The title, when present, is the full-width first row and does not overlap a panel.

Checked sizes, header on unless noted:

| Terminal | Header | Body | Content height | Layout |
| -------- | ------ | ---- | -------------- | ------ |
| 32×12 | on | 32×7 | 7, no title | 8 |
| 80×24 | on | 80×19 | 18 | 1 |
| 135×42 | on | 135×37 | 36 | 1 |
| 72×16 | on | 72×11 | 10 | 4 |
| 58×17 | on | 58×12 | 11 | 6 |
| 40×15 | on | 40×10 | 9 | 8 |
| 40×15 | off | 40×15 | 14 | 7 |
| 40×45 | on | 40×40 | 39 | 3 |

### Badge Reference

Unclipped examples:

```text
CPU 12% 480MHz
MEM 1.4GiB 35%
NET ↓2.1 ↑0.4MiB/s
DISK R1.2 W0.3MiB/s
```

Arrows are `↓` and `↑`. The longest ordinary two-value badge is 21 columns, which is why
layout 8 starts at that width.

### Hint Reference

The hint column is 16 columns by 5 rows.

- **Virtual-machine inventory**: `s summary`, `x actions`, `p perf`, `t task`, `e events`.
- **Host inventory**: `n network`, `d datastore`, `s summary`, `p perf`, `v vm`. Task and
  events keys still work and are not listed.
- **Overview**: expand lines `Tab focus`, `1 cpu`, `2 mem`, `3 net`, `4 disk`. Help lines
  `Esc back`, `z header`, `q quit`.
- **Shared inventory help**: `q quit`, `/ search`, `r resource`, `z header`, `Enter props`.
- **Events help**: `q quit`, `/ search`, `r resource`, `z header`, `Enter details`.
- **Property browser help**: include `z header` on the free row.

### Key Entities

- **Selected object**: The virtual machine or host row that was current when the overview
  opened. The overview keeps that name for the title. It does not follow a later selection
  change in the hidden inventory.
- **Resource domain**: One of CPU, memory, network, or disk, with a latest-value badge and,
  when the layout allows, a sparkline or a chart.
- **Sample window**: The latest set of up to 90 realtime samples for that one object, replaced
  as a whole on every successful refresh. Each point is a value or a gap, plus its timestamp.
- **Focus**: Which domain is primary. It changes layout assignment and does not change which
  data is fetched.
- **Header visibility**: A session-wide on/off flag for the five-row application header. It is
  not stored as a preference file and is not cleared by going back.

### Operator Experience Requirements *(mandatory for UI changes)*

- **Actions Shown**: Inventory hints gain `p perf` as specified above. The overview replaces
  both hint columns with focus keys, Esc, header toggle, and quit. Shared help and the
  property browser mention `z header`.
- **Loading State**: Before the first successful window, domains show no samples and the title
  shows `sample —`. Keys and resize work while a refresh is in progress. The screen updates
  when the refresh finishes, including when it fails.
- **Error State**: Failure is `ERR` and a short clipped message in the title line. The last
  good graphs stay. Empty and unsupported results clear the graphs and show `sample —`. A
  missing metric blanks only that domain.
- **Resize Behavior**: Every resize recomputes layout from the layout reference, including
  with the header hidden. Panels stay inside the body and do not overlap. Clipping is on the
  right, never by wrapping.
- **Contrast/Layers**: The overview is a full-body view on the existing dark background, not a
  floating panel. Series use the fixed colors in FR-020. Badge text stays readable against
  that background. The header toggle does not invent a second theme.
- **Live Update Behavior**: Realtime only, about every 20 seconds, about the last 30 minutes,
  last 90 samples, full replace. The title clock is the newest sample time, not the wall
  clock. Power-off shows up as no samples on the next refresh. The inventory’s own sparklines
  are unchanged and are not refreshed while this view is open.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: From a virtual-machine row and from a host row, an operator opens the overview
  with one key and returns to that inventory with either Esc or Backspace. `q` quits instead
  of returning.
- **SC-002**: At every checked terminal size in the layout reference, including 32×12,
  72×16, 58×17, 80×24, 135×42, and 40×15 with the header hidden, the on-screen layout matches
  the table. With the content height at least 4, all four domains are visible exactly once.
- **SC-003**: At 135×42 with the header visible, the operator sees CPU percent, consumed
  memory, network throughput, and disk read/write throughput together, without opening another
  view.
- **SC-004**: Disk values are read and write rates in binary units per second. They are never
  a capacity figure and never an operations-per-second figure.
- **SC-005**: After a refresh, the title shows the newest sample time, `sample —` when there
  are none, or a one-line `ERR` when the refresh failed. A second successful refresh clears
  `ERR`. Duplicate samples do not accumulate across refreshes.
- **SC-006**: While a refresh is in progress, the operator can resize, move focus, hide the
  header, leave the overview, or quit without waiting for the refresh to finish.
- **SC-007**: Hiding the header adds exactly five rows to the body in the inventory, the
  property browser, and the overview, and the choice survives leaving the overview.
- **SC-008**: The same terminal size produces the same layout on macOS, Windows, Linux, and
  an SSH session. No layout draws panels outside the body or on top of each other across the
  size range used for the layout checks (widths 1–200, heights 1–60, all four focus values).

## Assumptions

- Operators already use the inventory tables, the `s` summary popup, and the property browser.
  This view is the live counterpart to the static summary, not a replacement for either.
- The realtime sample interval is about 20 seconds, so 90 samples are about 30 minutes. The
  title says that even when fewer than 90 samples have arrived yet.
- “Whole-object disk throughput” means one read figure and one write figure for the host or
  virtual machine, not a line per disk. A simulated lab inventory is not proof that those
  figures exist. Before the view is considered done, a real vCenter must show disk read and
  write throughput for a host, a virtual machine on VMFS, and a virtual machine on NFS or
  vSAN. If a virtual machine has no whole-machine disk figures, the follow-up is one combined
  total of its disks. An operations-per-second switch waits until that check passes.
- The object name shown in the title is the name captured when the overview opens. The view
  does not keep watching the inventory only to rename the title.
- Chart tick labels are short relative times before the newest sample (how long ago). Exact
  tick wording stays inside that rule.
- Chart slots are line charts, including both network directions and both disk directions,
  with gaps that break the line.
- Header visibility is session state only. It is not written to the operator’s config file.
- Host tasks and events stay available from their keys when their hints are no longer listed.
  Redesigning the hint column so those hints fit again is out of scope.
- Out of scope for this version: past day or week ranges; per-vCPU, per-NIC, per-disk, or
  per-LUN series; operations per second; host swap, balloon, and active-memory charts; cluster,
  resource pool, datastore, and GPU overviews; replacing the six-slot inventory sparklines.
