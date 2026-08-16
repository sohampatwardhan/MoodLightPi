# State: Preact SPA Frontend

<!-- spec-nav:start -->
**Spec navigation:** [State](00_state.md) · [Discovery](01_discovery.md) · [Requirements](02_requirements.md) · [Design](03_design.md) · [Tasks](04_tasks.md) · [Execution](05_execution.md)
<!-- spec-nav:end -->

Feature slug: `preact-frontend`. Change control and gate status for the spec-driven pipeline.

## Gates

| Phase | Status | Notes |
|---|---|---|
| Discovery | approved | approved 2026-08-16; MQTT availability + HA discovery delta re-approved 2026-08-16 |
| Requirements | approved | [02_requirements.md](02_requirements.md) approved 2026-08-16 (18 requirements, 62 criteria) |
| Design | approved | [03_design.md](03_design.md) approved 2026-08-16; audit fixes AUDIT-1/2 re-approved 2026-08-16 |
| Tasks | approved | [04_tasks.md](04_tasks.md) approved 2026-08-16 (15 tasks, 7 stages); audit fixes re-approved 2026-08-16 |
| Audit | fixes_applied | medium, scoped (MQTT + router fallback) 2026-08-16; 2 P2 findings applied (AUDIT-1/2), no P0/P1 |
| Execution | checkpoint | Stages 1–6 complete & verified (69 cargo + 9 vitest tests); paused at Stage-7 live-verification checkpoint (7.1) awaiting user approval |
| Finish | not started | |

## Change Control

- Any material change to the approved discovery boundary (chosen approach, scope, or the parity
  inventory) invalidates all downstream gates and requires re-approval from Discovery forward.
- Backend cleanup is in scope but must not change device behavior (engine, effects, persistence,
  hardware, MQTT/HomeKit runtime).

## Decisions Log

- Toolchain: **Vite + Preact + TypeScript** (`preact-ts`). *(user, 2026-08-16)*
- Backend scope: **frontend + API cleanup** — aggregate bootstrap, catch-all SPA fallback, prune
  unused routes. *(user, 2026-08-16)*
- Visual design: **preserve the OctoCam-style layout**; parity, not redesign. *(user, 2026-08-16)*
- Build integration: **built bundle committed to the repo**, embedded via `rust-embed`; no Node on
  the Pi (Approach A). *(discovery)*
- MQTT scope expanded: device advertises **availability (LWT online/offline, retained)** and
  publishes **Home Assistant MQTT Discovery**; MQTT settings UI gains availability topic +
  discovery enable/prefix. Grounded in current HA MQTT docs via Context7. *(user, 2026-08-16)*

## Current Status

All planning gates approved and the scoped audit's two P2 fixes applied and re-approved 2026-08-16.
Next action: `spec-execute` starting at Stage 1 (tasks 1.1, 1.2).
