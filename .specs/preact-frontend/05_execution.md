# Execution Ledger: Preact SPA Frontend

<!-- spec-nav:start -->
**Spec navigation:** [State](00_state.md) · [Discovery](01_discovery.md) · [Requirements](02_requirements.md) · [Design](03_design.md) · [Tasks](04_tasks.md) · [Execution](05_execution.md)
<!-- spec-nav:end -->

Durable evidence for implementing [`04_tasks.md`](04_tasks.md). The task checklist is the
source of truth for progress; this ledger records outcomes, verification, and decisions.

## Preflight

- Scoped `spec-audit` (medium) passed with two P2 fixes (AUDIT-1, AUDIT-2) applied and the
  affected design/tasks re-approved 2026-08-16; that serves as the self-hardening evidence for
  this run (no separate `plan-harden` pass). The artifact digest changed when the fixes were
  applied; the fixes were independently reviewed and re-checked via `spec-check.py`.
- Tooling on host: node v24.14.0, npm 11.9.0, cargo 1.97.1, rustc 1.97.1 — the frontend build is
  feasible on this host.

## Branch and baseline

- Per the user's choice, created branch `feature/preact-frontend` carrying the working tree, then
  committed the pre-existing app WIP as baseline `cc11cd6` and the spec-driven plan as `34f666f`.
  Working directly on this branch (not an isolated worktree): HEAD lacked the untracked app
  modules (`mqtt.rs`/`settings.rs`/`homekit.rs` and the vendored-crates directory), so a worktree
  from HEAD would not build. Tree is clean at start of implementation.
- Baseline `cargo test` (host, mock backend, no `hardware` feature) at `cc11cd6`: **57 passed, 0
  failed**. Pre-existing warnings only (elided-lifetime hint in `homekit.rs`; dead-code
  `Source::WebSocket`). These are the baseline to compare against.

## Execution Timing

### Run Intervals
| Run ID | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|
| run-20260816T175316Z | 2026-08-16T17:53:16Z | pending | pending | active |

### Task Attempt Intervals
| Run ID | Stage/Wave | Task | Attempt | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|---|---:|---|
| run-20260816T175316Z | 1 | 1.1 | 1 | 2026-08-16T18:01:04Z | 2026-08-16T18:45:02Z | 2638 | verified |

## Task Results

### 1.1 — Frontend scaffold — verified
Created the [`frontend/`](../../frontend) Vite + Preact + TypeScript project (`package.json`, `package-lock.json`,
`tsconfig.json`, `vite.config.ts`, `index.html`, `src/main.tsx`) with `base:'/'`,
`outDir:'../web-dist'`, `emptyOutDir:true`; updated [`.gitignore`](../../.gitignore). `npm ci && npm run build`
emits [`web-dist/index.html`](../../web-dist/index.html) + `web-dist/assets/index-<hash>.js` (0.40 kB + 11.57 kB; gz 0.26 +
4.91 kB — far under the 50 kB budget). `tsc --noEmit` clean. Dependency-security: pre/post-change
`AuditResult 1.0` evidence under [`.security/dependency-audit/`](../../.security/dependency-audit) — **0 vulnerability findings** across
170 resolved packages; gate `warnings` reflects only incomplete transitive inventory resolution,
reviewed and accepted (reports under the repo dependency-audit evidence directory). Satisfies
R13.1, R15.1.
