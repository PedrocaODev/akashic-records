---
id: stale-unreplaced-batch
type: Decision
title: Legacy Nightly Batch Settlement
status: deprecated
valid_from: "2023-01-01"
valid_until: "2024-01-01"
scope:
  system: payments
  env: production
relations:
  - type: depends_on
    target: ./concept-payment-status.md
---
# Legacy Nightly Batch Settlement

## Context
Initial payment ledger reconciliation mechanism used batch settlement CSV exports generated at midnight.

## Status
Deprecated without an explicit superseding note recorded. Historical reference only, referencing [Payment Status Domain Model](./concept-payment-status.md).
