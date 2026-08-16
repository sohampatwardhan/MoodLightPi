# Execution Ledger: Preact SPA Frontend

Durable evidence for implementing [`04_tasks.md`](04_tasks.md). The task checklist is the
source of truth for progress; this ledger records outcomes, verification, and decisions.

## Preflight

- Scoped `spec-audit` (medium) passed with two P2 fixes (AUDIT-1, AUDIT-2) applied and the
  affected design/tasks re-approved 2026-08-16; that serves as the self-hardening evidence for
  this run (no separate `plan-harden` pass). The artifact digest changed when the fixes were
  applied; the fixes were independently reviewed and re-checked via `spec-check.py`.
- Tooling on host: node v24.14.0, npm 11.9.0, cargo 1.97.1, rustc 1.97.1 — the frontend build is
  feasible on this host.

## Environment / branch blocker (pre-implementation)

Before the first edit: the checkout is on the protected branch `main` (HEAD `9f3ca41`), and the
working tree carries substantial uncommitted work-in-progress — `src/mqtt.rs`, `src/settings.rs`,
`src/homekit.rs`, and `vendor/` are untracked (absent from HEAD), alongside many modified files.
That WIP is the current application the tasks modify, so an isolated worktree created from HEAD
would not build. Awaiting the user's choice of branch strategy before editing.

## Execution Timing

### Run Intervals
| Run ID | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|
| run-20260816T175316Z | 2026-08-16T17:53:16Z | pending | pending | active |

### Task Attempt Intervals
| Run ID | Stage/Wave | Task | Attempt | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---|---|---|---:|---|
| (none yet — blocked on branch strategy) | | | | | | | |
