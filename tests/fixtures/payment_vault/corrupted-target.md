---
id: corrupted-target
type: Decision
title: Corrupted Target Note
status: draft
valid_from: "2026-08-15"
scope:
  system: payments
  env: production
relations:
  - type: depends_on
    target: ./nonexistent-target-note.md
---
# Corrupted Target Note

Canary note designed for linter validation, referencing a nonexistent target `./nonexistent-target-note.md`.
