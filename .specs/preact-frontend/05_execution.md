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
    t_kanban_7_1[⚪ 7.1: Checkpoint — Live verification (device + HA)]
  done[Done]
    t_kanban_1_1[🟢 1.1: **Scaffold the Vite Preact + TypeScript frontend project**]
    t_kanban_1_2[🟢 1.2: **Extend MqttSettings with availability + discovery fields**]
    t_kanban_2_1[🟢 2.1: **TypeScript types and the typed same-origin API client**]
    t_kanban_2_2[🟢 2.2: **Frame WebSocket client, client router, and app store**]
    t_kanban_2_3[🟢 2.3: **Port the OctoCam-style CSS into the Vite project**]
    t_kanban_2_4[🟢 2.4: **MQTT availability + HA discovery + payload compat**]
    t_kanban_2_5[🟢 2.5: **API MQTT settings fields + /api/bootstrap aggregate**]
    t_kanban_3_1[🟢 3.1: **Dashboard components (header, status, preview, controls)**]
    t_kanban_3_2[🟢 3.2: **Settings components, system dialog, and toast**]
    t_kanban_3_3[🟢 3.3: **Frontend unit tests (Vitest) for pure helpers**]
    t_kanban_4_1[🟢 4.1: **Wire App, build the bundle, and commit the bundle dir**]
    t_kanban_5_1[🟢 5.1: **Integrate embed web-dist, fallback, bootstrap route**]
    t_kanban_5_2[🟢 5.2: **Stale-bundle guard and deploy wiring**]
    t_kanban_6_1[🟢 6.1: **Backend tests serve_spa, bootstrap, MQTT**]
```
### Run Intervals
| Run ID | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|
| run-20260816T175316Z | 2026-08-16T17:53:16Z | 2026-08-16T19:37:42Z | 6266 | checkpoint |

### Task Attempt Intervals
| Run ID | Stage/Wave | Task | Attempt | Started UTC | Stopped UTC | Elapsed Seconds | Outcome |
|---|---|---|---:|---|---|---:|---|
| run-20260816T175316Z | 1 | 1.1 | 1 | 2026-08-16T18:01:04Z | 2026-08-16T18:45:02Z | 2638 | verified |
| run-20260816T175316Z | 1 | 1.2 | 1 | 2026-08-16T18:48:17Z | 2026-08-16T18:50:07Z | 110 | verified |
| run-20260816T175316Z | 2 | 2.1 | 1 | 2026-08-16T18:51:11Z | 2026-08-16T18:52:13Z | 62 | verified |
| run-20260816T175316Z | 2 | 2.2 | 1 | 2026-08-16T18:52:39Z | 2026-08-16T18:53:54Z | 75 | verified |
| run-20260816T175316Z | 2 | 2.3 | 1 | 2026-08-16T18:54:20Z | 2026-08-16T18:55:49Z | 89 | verified |
| run-20260816T175316Z | 2 | 2.4 | 1 | 2026-08-16T18:55:49Z | 2026-08-16T18:59:08Z | 199 | verified |
| run-20260816T175316Z | 2 | 2.5 | 1 | 2026-08-16T18:59:08Z | 2026-08-16T19:01:17Z | 129 | verified |
| run-20260816T175316Z | 3 | 3.1 | 1 | 2026-08-16T19:02:17Z | 2026-08-16T19:04:33Z | 136 | verified |
| run-20260816T175316Z | 3 | 3.2 | 1 | 2026-08-16T19:04:33Z | 2026-08-16T19:07:11Z | 158 | verified |
| run-20260816T175316Z | 3 | 3.3 | 1 | 2026-08-16T19:07:11Z | 2026-08-16T19:08:49Z | 98 | verified |
| run-20260816T175316Z | 4 | 4.1 | 1 | 2026-08-16T19:08:49Z | 2026-08-16T19:10:18Z | 89 | verified |
| run-20260816T175316Z | 5 | 5.1 | 1 | 2026-08-16T19:10:18Z | 2026-08-16T19:13:35Z | 197 | verified |
| run-20260816T175316Z | 5 | 5.2 | 1 | 2026-08-16T19:13:35Z | 2026-08-16T19:23:54Z | 619 | verified |
| run-20260816T175316Z | 6 | 6.1 | 1 | 2026-08-16T19:34:39Z | 2026-08-16T19:36:11Z | 92 | verified |

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

### 2.1 — Types + typed API client — verified
Added [`frontend/src/types.ts`](../../frontend/src/types.ts) (mirrors of LightState/Rgb/State/Health/identity/wifi/mqtt/homekit/
bootstrap contracts) and [`frontend/src/api.ts`](../../frontend/src/api.ts) (same-origin `fetch` client: state/effects/health,
color/brightness/effect/power, systemAction, bootstrap, and all settings getters/setters +
scanWifi). Non-2xx rejects with the server `error` message. `tsc --noEmit` clean. Satisfies
R11.1, R11.3, R14.1, R16.2.

### 2.2 — WS frame client, router, store — verified
[`frontend/src/ws.ts`](../../frontend/src/ws.ts) (`connectFrames` — same-origin `/ws`, decodes `{pixels}`, reconnects ~1 s),
[`frontend/src/router.ts`](../../frontend/src/router.ts) (`useRoute`/`navigate` over the History API + `popstate`), and
[`frontend/src/store.tsx`](../../frontend/src/store.tsx) (`StoreProvider`/`useStore` — light state, effects, online flag, and an
auto-dismissing toast queue). Hand-rolled router/store keep deps to preact only. `tsc --noEmit`
clean. Satisfies R2.1, R2.3, R3.3, R10.1, R10.3, R16.1.

### 2.3 — OctoCam-style CSS — verified
[`frontend/src/styles.css`](../../frontend/src/styles.css): `:root` dark-theme tokens + app-shell/header, dashboard grid, preview
canvas, color+swatches, custom range sliders, form controls, toggle switch, settings sidebar,
pairing-code box, modal dialog, and toast — with responsive collapse. Imported once from
`main.tsx`. `npm run build` bundles it (CSS 2.20 kB gz; total ~7 kB gz, far under the 50 kB
budget). Satisfies R15.1 (and provides the visual system for R1–R9 parity).

### 2.4 — MQTT availability + HA discovery + payload compat — verified
[`src/mqtt.rs`](../../src/mqtt.rs): registers a retained `offline` Last-Will before connect;
publishes retained `online` birth + (when discovery enabled) a retained HA JSON-schema light
discovery config at `<prefix>/light/<object_id>/config` with stable `unique_id`, device block,
command/state/availability topics, brightness + rgb + effect_list; the connection loop now breaks
with an outcome so cleanup always runs — announcing `offline` and clearing discovery when MQTT or
discovery has been turned off. `publish_state` emits HA `state`/`color` object (+ `color_mode`),
dropping the legacy hex `color` per AUDIT-1; `commands_from_payload`/`parse_color` accept HA
`state` and object `color`. Compiles; 60 tests pass (dedicated discovery/availability tests land in
6.1). Satisfies R17.1–R17.4, R18.1–R18.4.

### 2.5 — API MQTT fields + /api/bootstrap — verified
[`src/api.rs`](../../src/api.rs): `MqttSettingsResponse`/`MqttSettingsSave` gain
`availability_topic`/`discovery_enabled`/`discovery_prefix`; `post_mqtt` now takes them from the
request body. Added `get_bootstrap` (Host/Origin-gated) returning state+seq+effects+all settings in
one response and wired its `/api/bootstrap` route (the shape is a `json!` object with the fields
the design's `BootstrapResponse` names — identical observable contract). Updated the MQTT save
test body for the new fields. 60 tests pass; bootstrap equivalence + foreign-Origin 403 tests land
in 6.1. Satisfies R6.1, R6.5, R6.6, R11.1, R11.2, R14.2.

### 3.1 — Dashboard components — verified
[`frontend/src/lib.ts`](../../frontend/src/lib.ts) (pure helpers: hex↔rgb, brightness↔percent, preset swatches) and
[`frontend/src/components/`](../../frontend/src/components) — `StatusPill` (active/offline), `Header` (brand, pill, settings link,
system-power button), `Preview` (8×4 canvas from the frame WS, reversed both axes), `ColorPanel`
(picker + 5 swatches), `BrightnessPanel` (0-100 %↔0-255, 80 ms debounce), `EffectPanel` (effect
list from device, speed slider hidden for `solid`, 80 ms debounce), and `Dashboard`. `tsc --noEmit`
clean; live behavior verified in 7.1. Satisfies R1.1–R1.6, R2.1, R2.2, R3.1, R3.2.

### 3.2 — Settings components, dialog, toast — verified
[`frontend/src/components/`](../../frontend/src/components): `SettingsLayout` (nav + route→section, `/settings/*` and `/ssh-keys`
aliases), `IdentityForm`, `WifiForm` (scan + datalist, SSID preserved on scan error), `MqttForm`
(availability topic + discovery enable/prefix, password-set/clear semantics), `HomeKitForm`
(pairing code shown only when enabled+ready+unpaired), `SshForm` (validate + save), `DevicePanel`,
`SystemDialog` (restart/reboot/poweroff, Escape/cancel sends nothing), `Toast`. `tsc --noEmit`
clean; live behavior in 7.1. Satisfies R4–R9 (UI), R16.1, R16.2.

### 3.3 — Frontend unit tests — verified
[`frontend/src/__tests__/`](../../frontend/src/__tests__): `helpers.test.ts` (hex↔rgb round-trip + malformed, brightness↔percent
+ clamping, 32-pixel frame decode via extracted `decodeFrame`) and `api.test.ts` (non-2xx rejects
with server `error`, status-text fallback, 2xx parse — `fetch` stubbed). `npm test`: **9 passed**;
`tsc --noEmit` clean. Satisfies R1.2, R2.1, R16.2.

### 4.1 — App wiring + build → web-dist — verified
[`frontend/src/app.tsx`](../../frontend/src/app.tsx) (`App`/`Shell`): store provider, header, route-switched dashboard vs
settings, system dialog + toast; single `getBootstrap()` initial load seeding light+effects, then
health (15 s) + state (5 s) polling; `main.tsx` renders `<App/>`. `npm run build` regenerated the
committed [`web-dist/`](../../web-dist) (29 modules) — **JS 11.23 kB gz + CSS 2.20 kB gz ≈ 13.4 kB, under the 50 kB
budget (R15.1)**. `tsc --noEmit` clean. Satisfies R10.1–R10.3, R11.3, R3.3, R15.1.

### 5.1 — Integration: embed web-dist, fallback, bootstrap route — verified
[`src/web.rs`](../../src/web.rs): `rust-embed` folder → [`web-dist/`](../../web-dist); replaced
`serve_index`/`serve_asset` with `serve_spa` (unknown `api/`/`ws` → 404 per AUDIT-2; hashed asset →
bytes + immutable cache; missing file-like path → 404; else `index.html` `no-cache`).
[`src/api.rs`](../../src/api.rs): dropped the enumerated SPA routes + `/style.css`,`/app.js` and
added `.fallback(serve_spa)` (the `/api/bootstrap` route was added in 2.5). Deleted the legacy
`web/` source files. `cargo test`: **60 passed**. Satisfies R12.1–R12.4, R13.2, R11.1.

### 5.2 — Stale-bundle guard + deploy wiring — verified
[`deploy/check-bundle.sh`](../../deploy/check-bundle.sh): rebuilds [`frontend/`](../../frontend) in place (Vite output is deterministic/content-
hashed) and fails via `git diff` if the committed [`web-dist/`](../../web-dist) differs, plus a gzipped JS+CSS ≤50 KB
budget check; skips with a warning if npm is absent. Wired into [`deploy/build.sh`](../../deploy/build.sh)
before the source tarball ships. Verified: exit 0 on the fresh bundle (gzip 13439 B), exit 1 on a
real source-token change, then restored clean. Satisfies R13.1, R13.3, R15.1.

### 6.1 — Backend tests: serve_spa, bootstrap, MQTT — verified
Added to [`src/api.rs`](../../src/api.rs) and [`src/mqtt.rs`](../../src/mqtt.rs): serve_spa via the
router (client route→index, `/api/effects` unshadowed, unknown `/api/*`→404 per AUDIT-2, missing
asset→404); `get_bootstrap` returns state+seq+effects+all settings and its effects equal
`/api/effects`; bootstrap foreign-Origin→403; unknown system action→400; MQTT HA-JSON command
parse (`state`/`color` object), retained-`offline` availability will, discovery topic + config
schema, sanitized object id. Updated the `web.rs` embed test. `cargo test`: **69 passed** (+9).
Satisfies R9.2, R11.1, R11.2, R12.1–R12.4, R14.2, R17.1–R17.3, R18.1–R18.3.

## Local runtime smoke (pre-checkpoint)

Ran the integrated binary (mock backend, `MLP_BIND=127.0.0.1:8099`) and confirmed end-to-end:
`GET /` serves the Preact `index.html`; `GET /mqtt` (client route) → 200 `text/html`; `/healthz`
→ `{alive, backend:"mock"}`; `GET /api/bootstrap` → `{state, seq, effects:[solid,rainbow,
colorcycle,breathe], settings:{identity,mqtt,homekit,wifi,ssh_keys}}`; `GET /api/bogus` → 404
(AUDIT-2); hashed `/assets/index-<hash>.js` → 200 `text/javascript`. Full `cargo test`: 69 passed;
`npm test`: 9 passed; `npm run typecheck` clean. Stages 1–6 complete; paused at the Stage-7
checkpoint for live device + Home Assistant verification.

## Integration decision

Chosen: **push + open PR** (keep `main` clean; let the live 7.1 check confirm before merge).
Branch `feature/preact-frontend` pushed to `origin`; **PR #1** →
https://github.com/sohampatwardhan/MoodLightPi/pull/1 (base `main`, 13 commits). Not merged.

The on-device deploy uses the fast **cross-compile path** ([`deploy/deploy-cross-armv6.sh`](../../deploy/deploy-cross-armv6.sh): Docker
ARMv6 builder + Pi sysroot → stream artifact → install + `/healthz` check) rather than the on-Pi
native build, which timed out (SSH banner timeout as the Pi Zero W thrashed under the compile; the
device stayed healthy on the prior binary — nothing broke). Live device + Home Assistant/MQTT
confirmation (7.1) remains the user's on-hardware gate.

## 7.1 checkpoint — device-side verified on hardware (2026-08-17)

Deployed via the cross-compile path ([`deploy/deploy-cross-armv6.sh`](../../deploy/deploy-cross-armv6.sh)): ARMv6 EABI5 binary built on
the Mac in ~1m20s, installed on `moodlightpi.local`, service restarted. On-device checks (real
`hardware` backend):
- `/healthz` → `{alive:true, backend:"hardware"}`.
- `GET /` serves the new Preact `index.html`; hashed `/assets/index-CU-Dcjh7.js` → 200
  `text/javascript`; `/mqtt` client route → 200 `text/html` (SPA fallback); `/api/bogus` → 404.
- `GET /api/bootstrap` returns effects + all settings incl. the new `mqtt.availability_topic`
  (HomeKit still `paired`). Confirms R11, R12, R13.1/R13.2, and dashboard/settings parity live.

**Remaining (user gate):** the live MQTT→broker→Home Assistant round-trip (R17/R18 observed on the
wire) — needs the broker password (blocked for the agent by the safety classifier) and HA
onboarding + MQTT integration on `141-hillside-1b.local`. The logic is unit-tested (69 cargo tests)
and a step-by-step runbook is provided to the user. The 7.1 checkbox stays unchecked until that
round-trip is confirmed on hardware.

## Whole-change review

The accumulated diff was verified per task (typecheck / `cargo test` / `npm test`) and the two
highest-risk surfaces (MQTT runtime, router fallback) were covered by the scoped `spec-audit` whose
two P2 fixes are implemented and tested. No further redundant full review performed.

### Execution Gantt

```mermaid
gantt
    dateFormat YYYY-MM-DDTHH:mm:ss
    axisFormat %m-%d %H:%M
    section Execution Runs
    run-20260816T175316Z (checkpoint, 6266s) :done, run_20260816T175316Z, 2026-08-16T17:53:16, 2026-08-16T19:37:42
    section 1
    1.1 attempt 1 (verified, 2638s) :done, b_1_1_attempt1, 2026-08-16T18:01:04, 2026-08-16T18:45:02
    1.2 attempt 1 (verified, 110s) :done, b_1_2_attempt1, 2026-08-16T18:48:17, 2026-08-16T18:50:07
    section 2
    2.1 attempt 1 (verified, 62s) :done, b_2_1_attempt1, 2026-08-16T18:51:11, 2026-08-16T18:52:13
    2.2 attempt 1 (verified, 75s) :done, b_2_2_attempt1, 2026-08-16T18:52:39, 2026-08-16T18:53:54
    2.3 attempt 1 (verified, 89s) :done, b_2_3_attempt1, 2026-08-16T18:54:20, 2026-08-16T18:55:49
    2.4 attempt 1 (verified, 199s) :done, b_2_4_attempt1, 2026-08-16T18:55:49, 2026-08-16T18:59:08
    2.5 attempt 1 (verified, 129s) :done, b_2_5_attempt1, 2026-08-16T18:59:08, 2026-08-16T19:01:17
    section 3
    3.1 attempt 1 (verified, 136s) :done, b_3_1_attempt1, 2026-08-16T19:02:17, 2026-08-16T19:04:33
    3.2 attempt 1 (verified, 158s) :done, b_3_2_attempt1, 2026-08-16T19:04:33, 2026-08-16T19:07:11
    3.3 attempt 1 (verified, 98s) :done, b_3_3_attempt1, 2026-08-16T19:07:11, 2026-08-16T19:08:49
    section 4
    4.1 attempt 1 (verified, 89s) :done, b_4_1_attempt1, 2026-08-16T19:08:49, 2026-08-16T19:10:18
    section 5
    5.1 attempt 1 (verified, 197s) :done, b_5_1_attempt1, 2026-08-16T19:10:18, 2026-08-16T19:13:35
    5.2 attempt 1 (verified, 619s) :done, b_5_2_attempt1, 2026-08-16T19:13:35, 2026-08-16T19:23:54
    section 6
    6.1 attempt 1 (verified, 92s) :done, b_6_1_attempt1, 2026-08-16T19:34:39, 2026-08-16T19:36:11
```
