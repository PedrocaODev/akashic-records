# Clean-Room Core with OKF and Reader-Agnostic Design

Upstream implementations in `obsidian-second-brain` rely on Obsidian-specific conventions, contain parser mismatches (nested vs. top-level supersession), exhibit data loss during OKF export by dropping the `relations:` block, and limit cycle detection to 2-node pairs. We decided to build a standalone, reader-agnostic engine that adopts standard Google OKF v0.2 metadata and defines a documented typed-edge extension profile. This guarantees mathematical graph correctness and cross-tool portability without tying users to Obsidian or inheriting upstream defects.

## Consequences
- The engine operates purely on standard GitHub-flavored Markdown and relative POSIX links.
- Interoperability with OKF consumers is maintained via an explicit extension schema.
- Upstream scripts and notes remain convertible via dedicated import/export adapters.
