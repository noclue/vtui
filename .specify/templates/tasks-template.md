---

description: "Task list template for feature implementation"
---

# Tasks: [FEATURE NAME]

**Input**: Design documents from `/specs/[###-feature-name]/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are REQUIRED for new features and bug fixes. Include focused
unit, integration, snapshot, or simulator-backed tests before implementation
tasks for each user story.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust TUI**: `src/` for application modules, module-local tests for focused
  logic, `tests/` for integration/simulator coverage where practical
- **Configuration/logging**: use platform temp/config/state directories in tests;
  do not write to the repository root
- Paths shown below assume vtui's Rust project structure - adjust based on
  plan.md structure

<!-- 
  ============================================================================
  IMPORTANT: The tasks below are SAMPLE TASKS for illustration purposes only.
  
  The /speckit-tasks command MUST replace these with actual tasks based on:
  - User stories from spec.md (with their priorities P1, P2, P3...)
  - Feature requirements from plan.md
  - Entities from data-model.md
  - vSphere object contracts, UI states, and background task flows
  
  Tasks MUST be organized by user story so each story can be:
  - Implemented independently
  - Tested independently
  - Delivered as an MVP increment
  
  DO NOT keep these sample tasks in the generated tasks.md file.
  ============================================================================
-->

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [ ] T001 Create project structure per implementation plan
- [ ] T002 Verify Rust, Ratatui, tokio, and vim_rs dependencies needed by this feature
- [ ] T003 [P] Confirm `cargo fmt`, `cargo clippy`, and test commands for this feature

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

Examples of foundational tasks (adjust based on your project):

- [ ] T004 Define vSphere PropertyCollector paths and visible-set retrieval strategy
- [ ] T005 [P] Define background task state/update channel for remote work
- [ ] T006 [P] Define terminal resize/input handling impact for affected views
- [ ] T007 Define dark-theme rendering and contrast-sensitive UI states
- [ ] T008 Configure error handling and operator-visible failure context
- [ ] T009 Confirm platform config/log path behavior if affected

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - [Title] (Priority: P1) 🎯 MVP

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T010 [P] [US1] Unit/state/rendering test for [behavior] in src/[module].rs
- [ ] T011 [P] [US1] Integration or simulator test for [vSphere/UI journey] in tests/[name].rs

### Implementation for User Story 1

- [ ] T012 [P] [US1] Implement UI state/data model for [behavior] in src/[module].rs
- [ ] T013 [P] [US1] Implement minimal vim_rs retrieval for [properties] in src/[module].rs
- [ ] T014 [US1] Implement background task/update flow in src/[module].rs (depends on T012, T013)
- [ ] T015 [US1] Implement Ratatui rendering and action hints in src/[module].rs
- [ ] T016 [US1] Add operator-visible validation and error handling
- [ ] T017 [US1] Add logging context for user story 1 operations

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - [Title] (Priority: P2)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 2

- [ ] T018 [P] [US2] Unit/state/rendering test for [behavior] in src/[module].rs
- [ ] T019 [P] [US2] Integration or simulator test for [vSphere/UI journey] in tests/[name].rs

### Implementation for User Story 2

- [ ] T020 [P] [US2] Implement UI state/data model in src/[module].rs
- [ ] T021 [US2] Implement minimal vim_rs retrieval/background flow in src/[module].rs
- [ ] T022 [US2] Implement Ratatui rendering/action hints in src/[module].rs
- [ ] T023 [US2] Integrate with User Story 1 components (if needed)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - [Title] (Priority: P3)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Tests for User Story 3

- [ ] T024 [P] [US3] Unit/state/rendering test for [behavior] in src/[module].rs
- [ ] T025 [P] [US3] Integration or simulator test for [vSphere/UI journey] in tests/[name].rs

### Implementation for User Story 3

- [ ] T026 [P] [US3] Implement UI state/data model in src/[module].rs
- [ ] T027 [US3] Implement minimal vim_rs retrieval/background flow in src/[module].rs
- [ ] T028 [US3] Implement Ratatui rendering/action hints in src/[module].rs

**Checkpoint**: All user stories should now be independently functional

---

[Add more user story phases as needed, following the same pattern]

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] TXXX [P] Documentation updates in docs/
- [ ] TXXX Code cleanup and refactoring
- [ ] TXXX Performance optimization across all stories
- [ ] TXXX [P] Additional unit/integration/snapshot tests for uncovered risk
- [ ] TXXX Security hardening
- [ ] TXXX Run quickstart.md validation
- [ ] TXXX Verify dark-theme contrast for affected views/popups
- [ ] TXXX Verify terminal resize behavior and SSH-friendly keyboard handling
- [ ] TXXX Run `cargo fmt --check`
- [ ] TXXX Run `cargo clippy`
- [ ] TXXX Run `cargo test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - May integrate with US1 but should be independently testable
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - May integrate with US1/US2 but should be independently testable

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Models before services
- Services before endpoints
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Models within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Unit/state/rendering test for [behavior] in src/[module].rs"
Task: "Integration or simulator test for [vSphere/UI journey] in tests/[name].rs"

# Launch all models for User Story 1 together:
Task: "Create [Entity1] model in src/models/[entity1].py"
Task: "Create [Entity2] model in src/models/[entity2].py"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
   - Developer C: User Story 3
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Keep UI input/redraw responsive while background tasks are pending
- Avoid overfetching: retrieve only properties listed in plan.md
- Include final `cargo fmt --check`, `cargo clippy`, and `cargo test` tasks
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
