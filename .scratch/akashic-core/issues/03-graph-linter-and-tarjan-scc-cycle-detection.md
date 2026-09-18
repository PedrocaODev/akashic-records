# 03 — Graph Linter & Tarjan SCC Cycle Detection

**What to build:** A dedicated linting command (`akashic lint`) that enforces graph integrity. Checks for dangling relation targets, verifies scope consistency, and executes Tarjan's Strongly Connected Components algorithm to catch arbitrary $N$-node supersession cycles ($A \rightarrow B \rightarrow C \rightarrow A$).

**Blocked by:** 02 — Vault Traversal & In-Memory Graph Index

**Status:** ready-for-agent

- [x] `akashic lint <vault_dir>` validates graph topology and reports all integrity violations.
- [x] Detects broken links where a relation target does not resolve to an existing note in the vault.
- [x] Identifies arbitrary $N$-node cycles in `supersedes` relations (including 2-node and 3+-node cycles) using Tarjan SCC.
- [x] Flags conflicting assertions (e.g. active `contradicts` edges within overlapping scopes).
- [x] Exits with a non-zero exit code when graph errors are present, making it suitable for CI gates.
