# 06 — Deterministic Conflict Abstention & Unresolved Reporting

**What to build:** The strict conflict abstention engine (ADR-0004). Prevents hallucination by refusing to guess a winner when encountering cycles or competing successors in overlapping scopes during queries. Emits a structured `[UNRESOLVED_CONFLICT]` block containing all competing notes and evidence.

**Blocked by:** 03 — Graph Linter & Tarjan SCC Cycle Detection, 04 — Current Guidance Query & Agent Context Packet

**Status:** completed

- [x] When a seed note is trapped in an $N$-node supersession cycle, `akashic query` strictly abstains from selecting a winner.
- [x] When two active notes both supersede the same predecessor in overlapping scopes without superseding each other, the query reports an unresolved conflict.
- [x] The output context packet includes an explicit `[UNRESOLVED_CONFLICT]` alert listing competing candidates and evidence links.
- [x] Never falls back to timestamps, file modification times (`mtime`), or lexical heuristics to silently break ties.
- [x] Returns a dedicated process exit code indicating an unresolved graph conflict.
