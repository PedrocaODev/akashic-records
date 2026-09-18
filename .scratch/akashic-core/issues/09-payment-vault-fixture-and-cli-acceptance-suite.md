# 09 — 15-Note Payment Vault Fixture & CLI Acceptance Suite

**What to build:** The complete 15-note Payment Status Evolution test fixture under `tests/fixtures/payment_vault/` and black-box CLI integration tests (`assert_cmd`) exercising all 30 user stories and edge cases across the entire CLI surface.

**Blocked by:** 03 — Graph Linter & Tarjan SCC Cycle Detection, 05 — Multi-Mode Query Engine, 06 — Deterministic Conflict Abstention & Unresolved Reporting, 07 — Migration Planner & Idempotent Applier, 08 — Google OKF v0.2 Export Layer

**Status:** completed

- [x] Materializes all 15 test notes (concept, historical polling, active events, incident evidence, runbook, staging exception, canary cycle, competing successor, stale batch, corrupted target).
- [x] CLI integration tests verify `inspect`, `plan`, `apply`, `lint`, `query` (all modes), and `export` against real disk fixtures.
- [x] Validates that current guidance query resolves to event-driven decision when seeded with polling decision.
- [x] Validates that Tarjan SCC catches the 3-node canary cycle and fails lint.
- [x] Validates that competing successors trigger deterministic `[UNRESOLVED_CONFLICT]` output.
- [x] Validates that OKF export preserves all typed relations losslessly.
