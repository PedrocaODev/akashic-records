# 02 — Vault Traversal & In-Memory Graph Index

**What to build:** Recursive directory scanning and in-memory graph construction across an entire Markdown vault. Resolves canonical POSIX relative paths and IDs, indexes all forward-declared typed relationships (`supersedes`, `depends_on`, `supported_by`, `caused_by`, `contradicts`, `relates_to`), and dynamically derives in-memory inverse views (`superseded_by`, `required_by`, `supports`).

**Blocked by:** 01 — Tracer Bullet CLI & Single-Note Inspector

**Status:** completed

- [x] `akashic inspect <vault_dir>` recursively discovers all Markdown notes in the target directory.
- [x] Builds an in-memory directed graph representing notes as nodes and typed relationships as edges.
- [x] Normalizes relative POSIX target paths (`./path/to/note.md`) and resolves them against existing vault nodes.
- [x] Dynamically projects inverse relationships without writing back to source Markdown files.
- [x] Reports vault-wide inventory metrics: total nodes, total edges by type, and malformed files.
