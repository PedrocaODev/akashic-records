# 08 — Google OKF v0.2 Export & Portability Layer

**What to build:** A lossless export command (`akashic export <vault> --format okf --out <export_dir>`) that transforms the native vault into a Google OKF v0.2 concept catalog while preserving typed relations, evidence, and scope in an OKF extension profile.

**Blocked by:** 02 — Vault Traversal & In-Memory Graph Index

**Status:** completed

- [x] `akashic export <vault> --format okf --out <export_dir>` outputs valid Google OKF v0.2 concept files.
- [x] Standard OKF lifecycle metadata (`status`, `valid_from`, `valid_until`) maps directly to OKF definitions.
- [x] Preserves all typed relations (`supersedes`, `depends_on`, `supported_by`, etc.) and evidence in the documented `relations:` extension profile without data loss.
- [x] Converted Markdown body links remain navigable and valid in target directory.
- [x] Emits an export manifest summarizing exported nodes and extension attributes.
