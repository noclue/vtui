# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust [version or NEEDS CLARIFICATION]  
**Primary Dependencies**: Ratatui, crossterm, tokio, vim_rs [additional crates or NEEDS CLARIFICATION]  
**Storage**: Platform config/state files, logs, or N/A [details]  
**Testing**: `cargo test`, focused unit/integration/snapshot/simulator tests [details]  
**Target Platform**: macOS, Windows, Linux terminals, including SSH jump hosts
**Project Type**: Rust terminal UI application  
**Performance Goals**: Responsive input/redraw during vSphere I/O; bounded CPU, memory, and API usage [specifics]  
**Constraints**: UI task must not block; retrieve only visible/needed vSphere properties; dark-theme contrast; dynamic resize support  
**Scale/Scope**: [vCenter/ESXi inventory size, visible row counts, refresh cadence, or NEEDS CLARIFICATION]
**vSphere Data Access**: [PropertyCollector paths, vim_rs helpers, polling/live update strategy, or NEEDS CLARIFICATION]
**Background Work**: [remote operations/background tasks and UI update path, or NEEDS CLARIFICATION]
**Operator UX**: [footer/action hints, loading/error states, resize behavior, contrast considerations]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

*GATE: Each item must be PASS or explicitly justified in Complexity Tracking.*

- **Terminal-native operator UX**: Feature exposes current available actions in the UI
  footer, popup, or equivalent operator hint surface.
- **Responsive async UI**: No vSphere I/O, remote operation, logging, or expensive
  computation can block input handling or terminal redraw; background updates are
  routed through UI state.
- **Minimal retrieval and live state**: Plan lists the exact vSphere properties,
  object relationships, polling cadence, and visible-set triggers needed for display.
- **Cross-platform remote readiness**: Plan covers macOS, Windows, Linux, SSH
  terminal behavior, dynamic resize, and platform config/log paths when affected.
- **Tested Rust quality gates**: Plan includes focused tests and final
  `cargo fmt --check`, `cargo clippy`, and `cargo test` validation.
- **Dark-theme accessibility**: Plan identifies visual states, layered elements,
  and contrast-sensitive text/background combinations.
- **vSphere API correctness**: Plan uses documented `vim_rs` and PropertyCollector
  patterns and respects disabled methods or capability data for remote operations.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
src/
├── app/ui state/rendering/input modules
├── vSphere data access/background task modules
├── configuration/logging modules
└── bin/lib entry points as applicable

tests/ or module-local tests
├── unit/state/rendering tests
├── integration/config/logging tests
└── simulator-backed vSphere tests where practical
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
