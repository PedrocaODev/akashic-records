# 07 — Migration Planner & Idempotent Applier

**What to build:** Non-destructive vault migration tooling (`akashic plan` and `akashic apply`). Inspects existing notes for missing IDs, top-level `supersedes:` fields, or legacy Obsidian wikilinks (`[[...]]`), produces a human-readable and machine-readable diff bound to SHA-256 hashes, and applies approved diffs idempotently.

**Blocked by:** 02 — Vault Traversal & In-Memory Graph Index

**Status:** completed

- [x] `akashic plan <vault> --out <plan.json>` generates a reviewable JSON plan containing unified diffs for metadata normalization.
- [x] Binds each proposed edit to the source file's SHA-256 content hash.
- [x] `akashic apply <vault> --plan <plan.json>` validates that target files match the expected SHA-256 hash before modifying them.
- [x] Aborts cleanly without partial mutations if any target file was modified after plan generation.
- [x] Repeated applications of the same plan produce zero changes and zero file churn (strict idempotency).
