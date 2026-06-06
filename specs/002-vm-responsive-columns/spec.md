# Feature Specification: VM Resource Browser Responsive Columns

**Feature Branch**: `002-vm-responsive-columns`  
**Created**: 2026-06-06  
**Status**: Draft  
**Input**: User description: "Improve the VM resource browser view so the VM name stays visible when the view is extremely narrow. As width grows, reveal columns in order: ID, then status and power (S & P), then OS, Used Space, Memory, and lastly CPU. Name column minimum width is 20 characters; when there is plenty of width the Name column should expand. Shrink the ID column by 6 characters compared to today."

## Clarifications

### Session 2026-06-06

- Q: After Name-only layout, should Status & Power or ID appear first when widening? → A: Status and Power appear before ID; at narrow widths Name + S & P is more useful than Name + ID. Full-width column order (ID, S, P, Name, …) is unchanged.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Identify VMs in a Narrow Terminal (Priority: P1)

An operator with a very narrow terminal or split-pane layout can still read VM names in the
resource browser without horizontal clutter from columns they cannot use at that width.

**Why this priority**: VM name is the primary identifier; losing it on narrow layouts makes the
browser unusable.

**Independent Test**: Resize the terminal to the narrowest supported width and verify only the Name
column is shown with at least 20 characters of readable VM name per row.

**Acceptance Scenarios**:

1. **Given** the VM resource browser is open and the table area is narrower than the width needed
   for any secondary column, **When** the table renders, **Then** only the Name column is visible
   and each row shows at least 20 characters of the VM name (truncated with ellipsis only if the
   name itself exceeds available space).
2. **Given** a narrow layout showing Name only, **When** the operator scrolls and selects rows,
   **Then** selection, filtering, and sorting by name continue to work as today.
3. **Given** the terminal is resized while viewing the VM list, **When** width decreases below a
   column's reveal threshold, **Then** that column hides immediately on the next redraw without
   corrupting row alignment or selection.

---

### User Story 2 - Progressive Column Reveal as Width Grows (Priority: P1)

As the operator widens the terminal, additional VM columns appear in a fixed priority order so the
most identity-critical information appears before operational metrics.

**Why this priority**: Defines the core responsive behavior the operator requested.

**Independent Test**: Gradually widen the terminal and confirm columns appear in the prescribed
order, one tier at a time, with no column skipping or reordering.

**Acceptance Scenarios**:

1. **Given** only Name is visible, **When** width increases past the Status and Power threshold,
   **Then** the S and P indicator columns appear together (to the left of Name) without displacing
   Name below its 20-character minimum.
2. **Given** Name, S, and P are visible, **When** width increases further, **Then** the ID column
   appears before any other secondary column.
3. **Given** ID, S, P, and Name are visible, **When** width increases further, **Then** columns
   appear in this order: Guest OS, Used Space, Memory, and finally CPU (sparkline + capacity).
4. **Given** a column is hidden due to width, **When** that column becomes visible again, **Then**
   its cell values match the same VM data as when the layout was last wide enough to show them.

---

### User Story 3 - Comfortable Reading at Full Width (Priority: P2)

An operator with ample terminal width sees all VM columns with a shorter ID field and an expanded
Name column that uses leftover horizontal space.

**Why this priority**: Improves readability at common desktop widths without changing data content.

**Independent Test**: Open the VM browser at a wide terminal size and verify all eight logical
columns render, ID is 6 characters narrower than the current default, and Name grows beyond its
minimum.

**Acceptance Scenarios**:

1. **Given** sufficient table width for all columns, **When** the table renders, **Then** all
   columns (ID, S, P, Name, OS, Used Space, CPU, Memory) are visible simultaneously.
2. **Given** full-width layout, **When** comparing ID column width to the pre-change default,
   **Then** the ID column is exactly 6 characters narrower.
3. **Given** full-width layout with unused horizontal space after fixed-width columns are allocated,
   **When** the table renders, **Then** the Name column absorbs the extra space and displays more
   of each VM name before truncation.

---

### Edge Cases

- What happens when the terminal is resized while background vSphere work is pending? Column
  visibility updates on redraw; loading and selection state are preserved.
- What happens over SSH or on terminals with inconsistent character-cell sizing? Reveal thresholds
  use character-cell counts consistent with the rest of vtui tables.
- What happens when a sort indicator is active on a column that becomes hidden? Sorting remains
  active in the data layer; the indicator reappears when the column becomes visible again.
- What happens when filter text matches a field in a hidden column (e.g., ID or OS)? Filtering
  continues to work; matching VMs remain in the list even if the matching column is not shown.
- What happens when VM names are shorter than 20 characters? The Name column still reserves at
  least 20 characters of width at narrow layouts; short names display without padding artifacts.
- What happens when even Name-only layout cannot fit 20 characters (extremely narrow terminal)?
  Name uses all available width and truncates gracefully; no other columns are shown.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The VM resource browser MUST always keep the Name column visible at every supported
  table width.
- **FR-002**: The Name column MUST reserve a minimum width of 20 character cells at all widths
  where it is shown.
- **FR-003**: When horizontal space remains after fixed-width columns are allocated, the Name column
  MUST expand to consume that space before any non-Name column grows beyond its defined maximum.
- **FR-004**: Columns MUST hide in reverse reveal order when width decreases: CPU first, then Memory,
  Used Space, OS, ID, then Status and Power together, with Name never hidden.
- **FR-005**: Columns MUST appear in this reveal order when width increases: Name (always), then
  Status and Power together, then ID, then Guest OS, then Used Space, then Memory, then CPU last.
- **FR-006**: The ID column width at full layout MUST be 6 character cells narrower than the
  current default ID column width used elsewhere in vtui (18 → 12 character cells).
- **FR-007**: Status (S) and Power (P) columns MUST appear and disappear as a pair; neither is shown
  without the other.
- **FR-008**: Column visibility changes MUST take effect on the next render after a terminal resize
  without requiring navigation away from the VM view.
- **FR-009**: Hidden columns MUST NOT consume visible horizontal space in the table layout.
- **FR-010**: Row selection highlight, filter state, and active sort order MUST remain consistent
  when columns are hidden or revealed.
- **FR-011**: Filtering MUST continue to match against ID, Name, and OS even when those columns are
  temporarily hidden.
- **FR-012**: This responsive layout applies ONLY to the Virtual Machines resource browser view;
  other resource types (hosts, clusters, datastores, etc.) are unchanged unless explicitly extended
  in a future feature.

### Column Layout Reference

Logical column order (left to right) and responsive behavior:

| Order | Column        | Hide priority (1 = first hidden) | Width behavior                                      |
| ----- | ------------- | -------------------------------- | --------------------------------------------------- |
| 1     | ID            | 3 (after S & P at narrow widths) | Fixed; 12 character cells at full layout            |
| 2     | Status (S)    | 2 (with Power, after Name-only)  | Fixed narrow indicator width                        |
| 3     | Power (P)     | 2 (with Status)                  | Fixed narrow indicator width                        |
| 4     | Name          | Never hidden                     | Minimum 20; expands with leftover width             |
| 5     | Guest OS      | 4                                | Fixed maximum; full width when revealed             |
| 6     | Used Space    | 5                                | Fixed maximum; full width when revealed             |
| 7     | CPU           | 7 (last revealed)                | Fixed width (sparkline + capacity) when revealed    |
| 8     | Memory        | 6                                | Fixed width (sparkline + capacity) when revealed    |

Reveal tiers (each tier includes all prior tiers):

| Tier | Visible columns                                      | Minimum width concept                                      |
| ---- | ---------------------------------------------------- | ---------------------------------------------------------- |
| 0    | Name                                                 | 20 (Name minimum)                                          |
| 1    | S, P, Name                                           | Tier 0 + Status + Power fixed widths                       |
| 2    | ID, S, P, Name                                       | Tier 1 + ID fixed width (12)                               |
| 3    | ID, S, P, Name, OS                                   | Tier 2 + OS maximum width                                  |
| 4    | ID, S, P, Name, OS, Used Space                       | Tier 3 + Used Space maximum width                          |
| 5    | ID, S, P, Name, OS, Used Space, Memory               | Tier 4 + Memory fixed width                                |
| 6    | ID, S, P, Name, OS, Used Space, Memory, CPU          | Tier 5 + CPU fixed width (all columns)                     |

Above Tier 6, additional width flows to the Name column.

### Key Entities

- **VM Table Row**: Eight logical data fields (ID, status indicator, power indicator, name, guest
  OS, used storage, CPU usage display, memory usage display) rendered as table cells.
- **Layout Tier**: A discrete visibility state determined by available table width; governs which
  columns are shown.
- **Column Width Policy**: Per-column rules (fixed, minimum, maximum, expandable) used to compute
  layout at each tier and at full width.

### Operator Experience Requirements *(mandatory for UI changes)*

- **Actions Shown**: No change to existing key bindings (↑↓ scroll, filter, sort); column hiding
  does not remove available actions.
- **Loading State**: Unchanged; perf-backed CPU/Memory cells show placeholders when data is absent,
  same as today, when those columns are visible.
- **Error State**: Unchanged from current VM browser behavior.
- **Resize Behavior**: Column set and widths recalculate on every resize; transitions between tiers
  are immediate with no animation required.
- **Contrast/Layers**: Hidden columns leave no blank placeholder headers; visible headers and cells
  retain current dark-theme styling.
- **Live Update Behavior**: Live refresh and perf sampling continue; columns hide or show based on
  current width, not data availability.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At the narrowest tier, operators can read VM names in 100% of rows without any
  non-Name column consuming horizontal space.
- **SC-002**: When widening the terminal through each tier in sequence, columns appear in the
  documented order with zero instances of out-of-order reveal across a standard resize test matrix
  (narrow → wide and wide → narrow).
- **SC-003**: At full width, the ID column is 6 character cells narrower than the pre-change
  default, and the Name column displays more characters on average for VMs with long names (measurable
  by comparing truncated name length before and after at identical terminal width).
- **SC-004**: Terminal resize during an active VM list session does not clear selection or filter
  state in 100% of manual resize tests.
- **SC-005**: Filtering by ID or OS still returns matching VMs when those columns are hidden,
  verified for at least one test case per hidden column type.

## Assumptions

- "6 symbols" and "20 symbols" refer to terminal character cells (monospace columns), consistent
  with how vtui measures table column widths today.
- The current default ID column width is 18 character cells; the new VM browser ID width is 12.
- Status and Power are treated as a single reveal unit because both are single-glyph indicators with
  equal operational importance at a glance.
- CPU appears after Memory in reveal order because CPU is the last column the operator requested to
  appear; Memory hides before CPU when narrowing.
- Only the top-level VM resource browser table is in scope; the host summary embedded VM table may
  be aligned in a follow-up if desired.
- Exact pixel-perfect breakpoint values are an implementation detail; this spec defines tier order,
  column width policies, and minimum Name width—the implementation derives thresholds from summed
  column widths.
