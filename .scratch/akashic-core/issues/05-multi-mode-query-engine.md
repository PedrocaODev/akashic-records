# 05 — Multi-Mode Query Engine (Lineage, Impact, Historical)

**What to build:** Expansion of the query engine to support historical point-in-time reconstruction, decision evolution lineage, and transitive impact analysis.

**Blocked by:** 04 — Current Guidance Query & Agent Context Packet

**Status:** completed

- [x] `--mode lineage` traces the chronological replacement chain ($A \rightarrow B \rightarrow C$), displaying why decisions changed and linking supporting evidence.
- [x] `--mode impact` traverses dynamic inverse `required_by` edges to list all downstream components that depend on the seed note.
- [x] `--mode historical --as-of <date>` evaluates `valid_from` and `valid_until` lifecycle intervals to reconstruct active guidance at that timestamp.
- [x] Handles missing dates gracefully with explicit warning indicators rather than silent guesses.
