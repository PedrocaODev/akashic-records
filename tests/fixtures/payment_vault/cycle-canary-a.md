---
id: cycle-canary-a
type: Decision
title: Canary Cycle Node A
status: draft
valid_from: "2026-08-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./cycle-canary-b.md
---
# Canary Cycle Node A

Canary test node A forming a 3-node cyclic supersession chain with B and C for Tarjan SCC validation.
