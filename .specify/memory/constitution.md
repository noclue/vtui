<!--
Sync Impact Report
Version change: 1.0.0 -> 1.0.1
Modified principles:
- none
Clarified sections:
- Operator Experience Standards: limited UI-facing requirements to visible states and controls
- Development Workflow and Quality Gates: moved live-data refresh requirements to specification/planning guidance
Added principles: none
Added sections: none
Removed sections: none
Templates requiring updates:
- ✅ .specify/templates/plan-template.md (already aligned)
- ✅ .specify/templates/spec-template.md (already aligned)
- ✅ .specify/templates/tasks-template.md (already aligned)
- ✅ .specify/templates/commands/*.md (no files present)
Runtime guidance requiring updates:
- ✅ README.md (no update required)
Follow-up TODOs: none
-->
# vtui Constitution

## Core Principles

### I. Terminal-Native Operator UX

vtui MUST remain an ergonomic, minimalist terminal interface for exploring vCenter
and standalone ESXi/ESX environments. Every interactive view MUST continuously
show the actions currently available to the operator, including navigation,
drill-down, search, export, and operation shortcuts that apply to the selected
object. UI text MUST favor concise operator language over decorative copy.

### II. Responsive Async UI

The UI task MUST never block on vSphere I/O, remote operations, logging, or
expensive computation. Remote operations MUST run as background tasks and report
progress, completion, and failures asynchronously through UI state. vtui MUST
continue monitoring user input and MUST be able to redraw promptly during terminal
resize events, including when background work is pending.

### III. Minimal Retrieval and Live State

vtui SHOULD display live-updated information whenever the vSphere API can support
it without harming responsiveness or operator clarity. Features MUST retrieve only
the properties and related objects needed for the current display or operation.
Plans that broaden PropertyCollector paths, polling, or performance sampling MUST
justify the added CPU, memory, and API cost and define how visible-set changes are
handled without overfetching.

### IV. Cross-Platform Remote-Ready Operation

vtui MUST run well on macOS, Windows, and Linux terminals, including sessions over
SSH to a jump host. Configuration and logs MUST use expected per-user platform
locations and MUST require zero project-local installation steps for normal use.
Terminal behavior MUST be validated against dynamic resize, keyboard input,
alternate terminal sizes, and platform path differences when affected by a change.

### V. Tested Rust Quality Gates

Every new feature and bug fix MUST include focused automated tests covering the
behavioral risk introduced by the change. Rust code MUST pass `cargo fmt`,
`cargo clippy`, and `cargo test` before merge. Snapshot, unit, integration, and
simulator-backed tests SHOULD be chosen according to risk, with UI behavior tested
at the state/rendering boundary where direct terminal assertions are impractical.

### VI. vSphere API Correctness

vtui MUST build on `vim_rs`, Ratatui, and vSphere PropertyCollector patterns that
preserve API correctness and efficient traversal. Managed-object queries,
polymorphic vSphere types, and background operations MUST follow documented
`vim_rs` usage instead of ad hoc object fetching. Operator-visible remote actions
MUST respect server-provided capability and disabled-method information and MUST
surface failures with enough context to diagnose the vSphere step that failed.

## Technology and Architecture Constraints

vtui is a Rust TUI built on Ratatui for rendering and `vim_rs` for vSphere
communication. Feature designs MUST preserve the asynchronous split between UI
state/rendering and vSphere I/O. Shared state MUST have clear ownership and update
paths so background tasks can refresh views without blocking input processing.

Configuration MUST remain discoverable through environment variables and
platform-standard config files. Logs MUST remain in platform-standard state
directories, separate operator-facing application logs from wire logs, and avoid
surprising current-working-directory output.

## Operator Experience Standards

The default visual design is a dark theme intended to be comfortable for long
operator sessions. Text, selection, popups, warnings, and layered surfaces MUST
maintain sufficient contrast against their backgrounds. Popups and overlays MUST
leave the underlying context understandable where practical and MUST define clear
dismissal keys.

Feature UI designs MUST describe user-visible actions, footer/action hints,
loading or background states, error states, and resize behavior.

## Development Workflow and Quality Gates

Plans MUST include a Constitution Check covering responsiveness, async background
work, minimal vSphere retrieval, cross-platform terminal behavior, test coverage,
and dark-theme accessibility. Any violation MUST be explicitly justified in the
plan's complexity tracking before implementation.

Feature specifications and plans MUST describe live-data refresh cadence,
stale-data handling, and how updates interact with search, sorting, and selected
rows when those concerns apply to the feature. This requirement governs feature
design, not a mandatory on-screen refresh-cadence indicator.

Tasks MUST be organized so each user story has test tasks before implementation
tasks. Final validation tasks MUST run `cargo fmt --check`, `cargo clippy`, and
`cargo test`; changes that affect terminal rendering, config paths, logging, or
remote operations MUST include targeted validation for those areas.

## Governance

This constitution supersedes conflicting feature plans, task lists, and informal
development practices for vtui. Amendments require a documented rationale, a
semantic version bump, updates to affected Spec Kit templates or runtime guidance,
and review before the amended rules are used for new feature work.

Versioning follows semantic versioning for governance: MAJOR for incompatible
principle removals or redefinitions, MINOR for added principles or materially
expanded requirements, and PATCH for clarifications that do not change compliance
obligations. Each feature plan and review MUST verify constitution compliance, and
unresolved violations MUST block implementation until explicitly accepted in the
plan.

**Version**: 1.0.1 | **Ratified**: 2026-05-06 | **Last Amended**: 2026-05-06
