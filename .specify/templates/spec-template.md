# Feature Specification: [FEATURE NAME]

**Feature Branch**: `[###-feature-name]`  
**Created**: [DATE]  
**Status**: Draft  
**Input**: User description: "$ARGUMENTS"

## User Scenarios & Testing *(mandatory)*

<!--
  IMPORTANT: User stories should be PRIORITIZED as user journeys ordered by importance.
  Each user story/journey must be INDEPENDENTLY TESTABLE - meaning if you implement just ONE of them,
  you should still have a viable MVP (Minimum Viable Product) that delivers value.
  
  Assign priorities (P1, P2, P3, etc.) to each story, where P1 is the most critical.
  Think of each story as a standalone slice of functionality that can be:
  - Developed independently
  - Tested independently
  - Deployed independently
  - Demonstrated to users independently
-->

### User Story 1 - [Brief Title] (Priority: P1)

[Describe this user journey in plain language]

**Why this priority**: [Explain the value and why it has this priority level]

**Independent Test**: [Describe how this can be tested independently - e.g., "Can be fully tested by [specific action] and delivers [specific value]"]

**Acceptance Scenarios**:

1. **Given** [initial state], **When** [action], **Then** [expected outcome]
2. **Given** [initial state], **When** [action], **Then** [expected outcome]

---

### User Story 2 - [Brief Title] (Priority: P2)

[Describe this user journey in plain language]

**Why this priority**: [Explain the value and why it has this priority level]

**Independent Test**: [Describe how this can be tested independently]

**Acceptance Scenarios**:

1. **Given** [initial state], **When** [action], **Then** [expected outcome]

---

### User Story 3 - [Brief Title] (Priority: P3)

[Describe this user journey in plain language]

**Why this priority**: [Explain the value and why it has this priority level]

**Independent Test**: [Describe how this can be tested independently]

**Acceptance Scenarios**:

1. **Given** [initial state], **When** [action], **Then** [expected outcome]

---

[Add more user stories as needed, each with an assigned priority]

### Edge Cases

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with vtui-specific terminal, vSphere, and operator edge cases.
-->

- What happens when the terminal is resized while background vSphere work is pending?
- How does the feature behave over an SSH session or slow/high-latency connection?
- What happens when selected rows disappear or reorder during a live refresh?
- How does the system handle missing, partial, or permission-denied vSphere properties?
- How are API failures, disabled operations, and stale data communicated to the operator?

## Requirements *(mandatory)*

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right functional requirements.
-->

### Functional Requirements

- **FR-001**: System MUST [specific operator capability, e.g., "display VM summary data for the selected VM"]
- **FR-002**: System MUST show currently available operator actions for [view/state]
- **FR-003**: Users MUST be able to [key terminal interaction, e.g., "dismiss the popup with Esc or q"]
- **FR-004**: System MUST perform [vSphere retrieval/operation] without blocking input handling or redraw
- **FR-005**: System MUST retrieve only [specific properties/objects] needed for the current display
- **FR-006**: System MUST preserve usable behavior on macOS, Windows, Linux, and SSH terminal sessions
- **FR-007**: System MUST maintain dark-theme contrast for [text/background/layered element]
- **FR-008**: System MUST log or surface [operator-relevant failure/context] without writing unexpected local files

*Example of marking unclear requirements:*

- **FR-009**: System MUST refresh live data every [NEEDS CLARIFICATION: refresh cadence not specified]
- **FR-010**: System MUST support [NEEDS CLARIFICATION: exact vSphere API version/object types not specified]

### Key Entities *(include if feature involves vSphere or local data)*

- **[vSphere Object]**: [Managed object type, key displayed properties, relationships]
- **[UI State]**: [Selection, sorting, filtering, loading, error, and refresh state]
- **[Background Task]**: [Remote operation or retrieval, completion/error update path]

### Operator Experience Requirements *(mandatory for UI changes)*

- **Actions Shown**: [Where the feature displays available keys/actions]
- **Loading State**: [How pending background work appears without blocking the UI]
- **Error State**: [How recoverable and fatal failures are shown and dismissed]
- **Resize Behavior**: [Expected layout behavior when terminal size changes]
- **Contrast/Layers**: [Dark-theme text, selection, popup, and background contrast needs]
- **Live Update Behavior**: [Refresh cadence, stale-data handling, selection preservation]

## Success Criteria *(mandatory)*

<!--
  ACTION REQUIRED: Define measurable success criteria.
  These must be technology-agnostic and measurable.
-->

### Measurable Outcomes

- **SC-001**: [Operator outcome, e.g., "Users can identify the available action for the selected row without documentation"]
- **SC-002**: [Responsiveness metric, e.g., "Input and resize handling remain responsive while data loads"]
- **SC-003**: [Efficiency metric, e.g., "Feature retrieves only the properties listed in the plan"]
- **SC-004**: [Cross-platform outcome, e.g., "Behavior is equivalent on macOS, Windows, Linux, and SSH terminals"]
- **SC-005**: [Quality outcome, e.g., "Focused tests cover success, error, and resize/background states"]

## Assumptions

<!--
  ACTION REQUIRED: The content in this section represents placeholders.
  Fill them out with the right assumptions based on reasonable defaults
  chosen when the feature description did not specify certain details.
-->

- [Assumption about target users, e.g., "Users have stable internet connectivity"]
- [Assumption about scope boundaries, e.g., "Mobile support is out of scope for v1"]
- [Assumption about data/environment, e.g., "Existing authentication system will be reused"]
- [Dependency on existing system/service, e.g., "Requires access to the existing user profile API"]
