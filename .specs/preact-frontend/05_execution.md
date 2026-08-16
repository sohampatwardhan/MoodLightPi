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


### Task Board

```mermaid
kanban
  pending[Pending]
    t_kanban_2_1[⚪ 2.1: **TypeScript types and the typed same-origin API client**]
    t_kanban_2_2[⚪ 2.2: **Frame WebSocket client, client router, and app store**]
    t_kanban_2_3[⚪ 2.3: **Port the OctoCam-style CSS into the Vite project**]
    t_kanban_2_4[⚪ 2.4: **MQTT availability + HA discovery + payload compat**]
    t_kanban_2_5[⚪ 2.5: **API MQTT settings fields + /api/bootstrap aggregate**]
    t_kanban_3_1[⚪ 3.1: **Dashboard components (header, status, preview, controls)**]
    t_kanban_3_2[⚪ 3.2: **Settings components, system dialog, and toast**]
    t_kanban_3_3[⚪ 3.3: **Frontend unit tests (Vitest) for pure helpers**]
    t_kanban_4_1[⚪ 4.1: **Wire App, build the bundle, and commit the bundle dir**]
    t_kanban_5_1[⚪ 5.1: **Integrate embed web-dist, fallback, bootstrap route**]
    t_kanban_5_2[⚪ 5.2: **Stale-bundle guard and deploy wiring**]
    t_kanban_6_1[⚪ 6.1: **Backend tests serve_spa, bootstrap, MQTT**]
    t_kanban_7_1[⚪ 7.1: Checkpoint — Live verification (device + HA)]
  done[Done]
    t_kanban_1_1[🟢 1.1: **Scaffold the Vite Preact + TypeScript frontend project**]
    t_kanban_1_2[🟢 1.2: **Extend MqttSettings with availability + discovery fields**]
```
### Run Intervals
| Run ID | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|
| run-20260816T175316Z | 2026-08-16T17:53:16Z | pending | pending | active |

### Task Attempt Intervals
| Run ID | Stage/Wave | Task | Attempt | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|---|---:|---|
| run-20260816T175316Z | 1 | 1.1 | 1 | 2026-08-16T18:01:04Z | 2026-08-16T18:45:02Z | 2638 | verified |
| run-20260816T175316Z | 1 | 1.2 | 1 | 2026-08-16T18:48:17Z | 2026-08-16T18:50:07Z | 110 | verified |

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

### 1.2 — MqttSettings availability + discovery fields — verified
Added `availability_topic` (default `moodlightpi/availability`), `discovery_enabled` (default
`true`), and `discovery_prefix` (default `homeassistant`) to `MqttSettings` in
[`src/settings.rs`](../../src/settings.rs), each with a `#[serde(default = ...)]` so pre-existing
`settings.json` still loads (filling HA defaults). `validate_mqtt_settings` now requires a non-empty
availability topic when enabled and a non-empty discovery prefix when discovery is enabled, and
validates both as MQTT topics. Made the `post_mqtt` constructor in [`src/api.rs`](../../src/api.rs)
preserve the new fields (task 2.5 wires them to the request body). `cargo test`: **60 passed**
(+3 new: defaults, backward-compat load, empty-field rejection). Satisfies R6.4, R6.5, R6.6.

### Execution Gantt

```mermaid
gantt
    dateFormat YYYY-MM-DDTHH:mm:ss
    axisFormat %m-%d %H:%M
    section 1
    1.1 attempt 1 (verified, 2638s) :done, b_1_1_attempt1, 2026-08-16T18:01:04, 2026-08-16T18:45:02
    1.2 attempt 1 (verified, 110s) :done, b_1_2_attempt1, 2026-08-16T18:48:17, 2026-08-16T18:50:07
```
