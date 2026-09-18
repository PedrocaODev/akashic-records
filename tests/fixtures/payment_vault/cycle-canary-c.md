---
id: cycle-canary-c
type: Decision
title: Canary Cycle Node C
status: draft
valid_from: "2026-08-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./cycle-canary-a.md
---
# Canary Cycle Node C

Canary test node C forming a 3-node cyclic supersession chain with A and B for Tarjan SCC validation.
