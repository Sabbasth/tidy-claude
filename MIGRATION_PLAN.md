# Rust Migration Plan

## Purpose

This document is the roadmap and progress tracker for porting `tidy-claude` from Python to Rust.

It has two jobs:
1. define the migration strategy;
2. survive context compaction by keeping durable progress, usage, and cost data in-repo.

---

## 1. Migration goals

### Primary goal

Rebuild `tidy-claude` as a Rust CLI with feature parity for:
- config management;
- backup / restore of Claude assets;
- skill installation;
- git-based sync workflow;
- cleanup of old sessions;
- interactive project selection for cleanup;
- test coverage for current behavior.

### Secondary goals

- improve reliability around filesystem and subprocess handling;
- make the binary easy to install and distribute;
- keep UX close enough that existing users can switch with minimal friction.

### Non-goals for the first Rust release

- major UX redesign;
- new cloud features;
- changing backup data formats unless required;
- multi-platform polish beyond what is needed for macOS/Linux parity.

---

## 2. Current Python application inventory

### Entry points and modules

- `src/tidy_claude/cli.py`: Click CLI, command wiring, interactive cleanup menu
- `src/tidy_claude/ops.py`: filesystem, git, subprocess, cleanup logic
- `src/tidy_claude/helpers.py`: pure helpers
- `src/tidy_claude/config.py`: config and path resolution
- `src/tidy_claude/state.py`: runtime counters / debug logging

### Existing feature surface

- `sync`: pull → restore → install skills → backup → commit → push
- `status`: show git status of backup repo
- `config`: print or update local config, initialize backup git repo
- `cleanup`: interactive or bulk deletion of old Claude sessions

### Test surface already present

- config command behavior
- sync remote guard
- cleanup logic, including named-session protection
- helper tests in `tests/test_helpers.py`

### Migration implications

- the pure helper layer can be ported first and tested heavily;
- side-effectful operations should be isolated behind Rust modules for testability;
- interactive TUI behavior should be delayed until core cleanup logic is stable.

---

## 3. Proposed Rust target architecture

Suggested crate layout:

```text
src/
  main.rs            # CLI bootstrap
  cli.rs             # clap commands / args
  app.rs             # high-level orchestration
  config.rs          # config loading/saving, path discovery
  state.rs           # runtime stats, debug logging
  helpers.rs         # pure functions
  ops/
    mod.rs
    backup.rs
    restore.rs
    cleanup.rs
    git.rs
    skills.rs
  model.rs           # shared structs/results
```

### Suggested Rust ecosystem choices

- CLI: `clap`
- Serialization: `serde`, `serde_json`
- Paths/home dirs: `directories` or `dirs`
- Walk filesystem: `walkdir`
- Error handling: `anyhow` or `eyre`
- Time: `filetime` / `chrono` / `std::time` as needed
- Interactive cleanup menu: `inquire`, `dialoguer`, or `skim`-style selector
- Testing: built-in `cargo test`, plus temp dirs via `tempfile`

### Design principles

- keep pure logic separate from side effects;
- prefer small modules mirroring today’s Python boundaries;
- normalize all user-visible behavior with tests before optimizing internals;
- preserve on-disk compatibility where possible.

---

## 4. Model strategy by major phase

### Pricing assumptions used for planning

These are planning assumptions for cost tracking. They must be replaced if actual provider billing differs.

| Model | Planning rate assumption (input / output, USD per 1M tokens) | Confidence |
|---|---:|---|
| Claude Opus 4.7 | 5 / 25 | high |
| Sonnet 4.6 | 3 / 15 | high |
| Haiku 4.5 | 1 / 5 | medium |
| Gemini 3.1 pro | 1.5 / 7.5 | low |
| GPT 5.4 mini | 0.6 / 2.4 | low |
| GPT 5.4 | 2.5 / 10 | low |

### Selection rule

Use the cheapest model that still safely handles the risk level of the phase.

- **Low risk / mechanical / repetitive**: prefer **GPT 5.4 mini** or **Haiku 4.5**
- **Medium risk / architecture + non-trivial coding**: prefer **Sonnet 4.6**
- **High risk / final audit / thorny edge cases**: use **Claude Opus 4.7** selectively
- **Long-context comparative review**: use **Gemini 3.1 pro** only if a very large design review is needed
- **GPT 5.4** is a fallback when stronger reasoning than mini is needed but Opus/Sonnet are unnecessary

---

## 5. Phased roadmap

## Phase 0 — Planning and tracking bootstrap

**Status:** Done

**Goals**
- inspect the current Python codebase;
- define the migration phases;
- create persistent tracking files for usage and checkpoints;
- record this initial planning session as the first usage entry.

**Deliverables**
- `MIGRATION_PLAN.md`
- `migration_tracking/usage_log.csv`
- `migration_tracking/checkpoints.md`

**Recommended model**
- **Sonnet 4.6**
- **Why:** best quality/token tradeoff for repo analysis + migration planning, and pricing is known with high confidence.

**Acceptance criteria**
- migration phases are explicit;
- tracking workflow is documented;
- the first session is logged.

**Stats**

| Metric | Value |
|---|---|
| Model used | Sonnet 4.6 |
| Time spent | 25 min |
| Tokens consumed | ~9,500 |
| Cost estimated | ~$0.06 |

## Phase 1 — Baseline capture and parity specification

**Status:** Planned

**Goals**
- document exact current behavior per command;
- list edge cases from tests and code;
- define parity requirements for Rust v1;
- freeze CLI surface unless a deliberate change is approved.

**Tasks**
- inventory all commands/options/messages;
- map Python tests to Rust acceptance tests;
- identify behavior that is incidental vs contractual;
- write parity checklist.

**Recommended model**
- **GPT 5.4 mini**
- **Why:** mostly mechanical extraction and checklist work; best expected quality/token efficiency for low-risk analysis.

**Acceptance criteria**
- parity checklist exists;
- all current user-visible commands/options are accounted for;
- Rust test backlog is derived from Python tests.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | GPT 5.4 mini |
| Time budget | 60–90 min |
| Token budget | ~12k |
| Cost estimated | ~$0.02 |

## Phase 2 — Rust project bootstrap and architecture skeleton

**Status:** Planned

**Goals**
- create the Rust crate;
- wire `clap` command structure;
- create modules matching the target architecture;
- add basic error handling and logging scaffolding.

**Tasks**
- initialize Cargo project;
- set up module tree;
- add placeholder commands and result types;
- configure linting/tests/formatting.

**Recommended model**
- **Sonnet 4.6**
- **Why:** architecture and scaffolding decisions matter; Sonnet gives better reliability than mini models at a still-efficient token cost.

**Acceptance criteria**
- `cargo test` passes with initial scaffold;
- commands compile;
- module boundaries are established.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | Sonnet 4.6 |
| Time budget | 90–120 min |
| Token budget | ~18k |
| Cost estimated | ~$0.09 |

## Phase 3 — Port pure logic first (`helpers`, `config`, `state`)

**Status:** Planned

**Goals**
- port pure and near-pure logic before side effects;
- reproduce helper behavior exactly;
- port config loading/saving and runtime stats.

**Tasks**
- port `diff_files`, `deep_merge`, `format_size`, `resolve_claude_md`, `pretty_project_name`, `extract_keys`;
- port config path resolution and persistence;
- port runtime state counters and debug logging;
- add unit tests for helper parity.

**Recommended model**
- **GPT 5.4 mini**
- **Why:** translation is mostly mechanical once architecture is fixed; strong value per token.

**Acceptance criteria**
- helper parity tests pass;
- config round-trip works;
- no side-effectful git/process logic is mixed into these modules.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | GPT 5.4 mini |
| Time budget | 120–180 min |
| Token budget | ~22k |
| Cost estimated | ~$0.04 |

## Phase 4 — Port backup / restore / git / skills operations

**Status:** Planned

**Goals**
- port operational logic with strong filesystem/process safety;
- preserve backup repo layout and merge behavior;
- keep git workflow behavior compatible with current tool.

**Tasks**
- port backup copy/extract logic;
- port restore copy/merge logic;
- port skill installation behavior;
- port git pull/commit/push/status behavior;
- introduce integration tests with temp git repos where possible.

**Recommended model**
- **Sonnet 4.6**
- **Why:** side effects, subprocesses, and merge semantics are higher risk than pure code; Sonnet is the best efficiency/safety balance.

**Acceptance criteria**
- backup/restore round-trips work in temp dirs;
- git guard behavior matches Python app;
- settings/key merge behavior is tested.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | Sonnet 4.6 |
| Time budget | 180–240 min |
| Token budget | ~30k |
| Cost estimated | ~$0.15 |

## Phase 5 — Port cleanup engine and interactive selection UX

**Status:** Planned

**Goals**
- port cleanup behavior exactly, including named-session protection;
- reproduce interactive multi-select project picker;
- keep `--all`, `--dry-run`, `--older-than`, `--with-named-sessions` semantics.

**Tasks**
- port project collection;
- port named-session detection from metadata and `.jsonl` fallback;
- port deletion logic and byte accounting;
- implement interactive selection UI;
- add regression tests from `tests/test_cleanup.py`.

**Recommended model**
- **Sonnet 4.6**
- **Why:** medium-to-high behavioral complexity plus TUI integration; better reliability than mini models while staying efficient.

**Acceptance criteria**
- cleanup parity tests pass;
- dry-run output and selection logic are acceptable;
- non-interactive guard behavior is preserved.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | Sonnet 4.6 |
| Time budget | 180–240 min |
| Token budget | ~28k |
| Cost estimated | ~$0.14 |

## Phase 6 — End-to-end parity testing and fixture hardening

**Status:** Planned

**Goals**
- close the gap between Python behavior and Rust behavior;
- add missing regression coverage discovered during porting;
- measure reliability across realistic temp-directory scenarios.

**Tasks**
- port remaining tests;
- add integration fixtures for git repos and Claude directory layouts;
- compare output/messages where meaningful;
- document any deliberate deviations.

**Recommended model**
- **GPT 5.4 mini**
- **Why:** this phase is test-heavy and repetitive; mini is expected to be most efficient per token.

**Acceptance criteria**
- all parity tests pass;
- known deviations are documented;
- no open P0/P1 parity gaps remain.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | GPT 5.4 mini |
| Time budget | 120–180 min |
| Token budget | ~24k |
| Cost estimated | ~$0.04 |

## Phase 7 — Packaging, install path, docs, and migration guide

**Status:** Planned

**Goals**
- make the Rust binary easy to build/install;
- update docs and usage examples;
- describe migration path from the Python package.

**Tasks**
- document install options;
- update `README.md`;
- write user migration notes;
- define release artifact strategy.

**Recommended model**
- **Haiku 4.5**
- **Why:** mostly documentation and packaging boilerplate; cheaper model is likely sufficient.

**Acceptance criteria**
- docs reflect Rust usage;
- install instructions are tested;
- migration notes cover config/data compatibility.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | Haiku 4.5 |
| Time budget | 60–90 min |
| Token budget | ~12k |
| Cost estimated | ~$0.03 |

## Phase 8 — Final review, hardening, and release decision

**Status:** Planned

**Goals**
- perform final review of risky code paths;
- validate error handling and destructive operations;
- decide whether the Rust version is ready to replace Python.

**Tasks**
- audit cleanup/delete safety;
- review subprocess and shell execution points;
- verify config and data compatibility;
- make cutover/no-cutover decision.

**Recommended model**
- **Claude Opus 4.7**
- **Why:** reserve the strongest model for the smallest, highest-risk final audit slice.

**Acceptance criteria**
- final risk list is closed or explicitly accepted;
- release checklist is complete;
- cutover decision is recorded.

**Planned stats**

| Metric | Value |
|---|---|
| Model used | Claude Opus 4.7 |
| Time budget | 45–75 min |
| Token budget | ~10k |
| Cost estimated | ~$0.10 |

---

## 6. Recommended execution order

1. Phase 1 — Baseline capture
2. Phase 2 — Rust scaffold
3. Phase 3 — Pure/core modules
4. Phase 4 — Backup/restore/git/skills
5. Phase 5 — Cleanup/TUI
6. Phase 6 — Parity testing
7. Phase 7 — Packaging/docs
8. Phase 8 — Final audit

Rationale: this order minimizes risk by locking expected behavior first, then porting low-risk logic before destructive operations and UI.

---

## 7. Tracking workflow that survives compaction

### Durable files

- `MIGRATION_PLAN.md`: roadmap, phase status, summary stats
- `migration_tracking/usage_log.csv`: append-only usage ledger per session/task
- `migration_tracking/checkpoints.md`: compact summaries after each working session

### Required update steps after every migration session

1. append one line to `migration_tracking/usage_log.csv`;
2. add a short checkpoint entry to `migration_tracking/checkpoints.md`;
3. update the relevant phase status and actual totals in `MIGRATION_PLAN.md`.

### CSV schema

`date,phase,task,status,model,input_tokens_est,output_tokens_est,total_tokens_est,time_spent_min,cost_est_usd,notes`

### Checkpoint template

```md
## YYYY-MM-DD HH:MM
- Phase:
- Work completed:
- Decisions:
- Files changed:
- Tests run:
- Open issues / blockers:
- Next step:
```

### Roll-up rule

At the end of each phase:
- sum all matching CSV rows for that phase;
- replace the phase “Planned stats” with an additional “Actual stats” subsection;
- keep both plan and actual, so estimates remain comparable.

### Compaction-resilient working rule

Before starting any new session, read these files in order:
1. `MIGRATION_PLAN.md`
2. `migration_tracking/checkpoints.md`
3. last relevant rows of `migration_tracking/usage_log.csv`

That is the minimum context needed to resume safely after compaction.

---

## 8. Risks and mitigation

| Risk | Impact | Mitigation |
|---|---|---|
| Behavior drift during rewrite | High | freeze parity checklist before coding |
| Destructive cleanup mistakes | High | port tests first, add temp-dir integration tests |
| Interactive TUI mismatch | Medium | keep TUI as a late phase after core cleanup parity |
| Git workflow regressions | High | test with temporary repos and empty remotes |
| Cost tracking becomes inaccurate after compaction | Medium | append-only CSV + checkpoint habit after each session |
| Rust over-engineering | Medium | keep module boundaries close to current Python design |

---

## 9. Definition of done

The migration is done when:
- Rust CLI covers all current commands needed for v1;
- parity tests pass;
- destructive operations are reviewed;
- install/docs are updated;
- the release decision is recorded in this file.

---

## 10. Progress summary

| Phase | Status | Notes |
|---|---|---|
| 0. Planning and tracking bootstrap | Done | Plan and tracking files created |
| 1. Baseline capture and parity specification | Planned | Not started |
| 2. Rust bootstrap and architecture skeleton | Planned | Not started |
| 3. Pure logic port | Planned | Not started |
| 4. Operations port | Planned | Not started |
| 5. Cleanup + interactive UX | Planned | Not started |
| 6. Parity testing and hardening | Planned | Not started |
| 7. Packaging and docs | Planned | Not started |
| 8. Final audit and release decision | Planned | Not started |

## Usage totals so far

| Metric | Value |
|---|---|
| Sessions logged | 1 |
| Total time | 25 min |
| Total tokens | ~9,500 |
| Total cost | ~$0.06 |

## Next recommended action

Start **Phase 1** by producing a parity checklist from the current Python CLI, tests, and user-visible outputs.
