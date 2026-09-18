---
id: cycle-canary-b
type: Decision
title: Canary Cycle Node B
status: draft
valid_from: "2026-08-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./cycle-canary-c.md
---
# Canary Cycle Node B

Canary test node B forming a 3-node cyclic supersession chain with A and C for Tarjan SCC validation.
