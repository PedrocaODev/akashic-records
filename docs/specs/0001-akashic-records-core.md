# Spec 0001: Akashic Records Core Engine & CLI

Status: ready-for-agent
Triage: ready-for-agent

## Problem Statement

Software engineers and AI coding agents working with long-lived knowledge repositories struggle to balance current architectural truth with historical context. In traditional Markdown documentation vaults, when an architectural decision or runbook is replaced, older notes are either silently deleted (destroying institutional memory, incident rationale, and audit trails) or left unmaintained (causing AI agents and engineers to hallucinate based on obsolete guidance). 

Existing attempts to solve this (such as `obsidian-second-brain`) are tied to proprietary Obsidian client conventions, suffer from parser mismatches between graph linkers and freshness re-rankers, drop typed relationships upon Google Open Knowledge Format (OKF) export, and only detect trivial 2-node cycles. Furthermore, dynamic scripting implementations in Python introduce runtime overhead, environment friction, and sluggish AST parsing across vaults with thousands of Markdown documents.

## Solution

Akashic Records (`akashic`) is a reader-agnostic, single-binary Rust CLI and deterministic graph engine that preserves historical decisions while delivering explicit, verified current guidance to AI agents. It treats standard GitHub-flavored Markdown documents as whole-note nodes with forward-only typed assertions in YAML frontmatter, compliant with Google OKF v0.2 plus a documented typed-edge extension profile. 

`akashic` provides non-destructive inspection, generates evidence-backed migration diffs, applies changes idempotently using content hashes, strictly detects arbitrary $N$-node cycles via Tarjan's Strongly Connected Components algorithm, evaluates scoped supersessions, strictly abstains from guessing winners during ambiguous conflicts, and formats bounded Markdown context packets for coding agents via a single command interface.

## User Stories

1. As an AI coding agent, I want to query current guidance for a concept or decision, so that I never act on obsolete or superseded architectural patterns.
2. As an AI coding agent, I want obsolete guidance to automatically point to its active successor, so that I understand what replaced it without manually searching the vault.
3. As an engineer writing Markdown notes, I want to assert relationships using standard YAML frontmatter in any editor, so that I am not locked into Obsidian or any proprietary tool.
4. As an engineer maintaining a knowledge vault, I want a non-destructive audit tool, so that I can inspect missing IDs, dangling links, and untyped relationships without modifying any files.
5. As an engineer migrating an existing repository, I want a machine-readable migration plan with readable diffs, so that I can review proposed metadata additions before applying them.
6. As a security-conscious developer, I want plan applications to be bound to SHA-256 content hashes, so that files modified since plan generation are never overwritten.
7. As an engineer running repeated migration jobs, I want the apply command to be strictly idempotent, so that running it multiple times produces zero diffs and no file churn.
8. As an AI coding agent investigating a production incident, I want to trace historical context and decision lineage, so that I understand why an architecture was chosen and what empirical evidence justified it.
9. As a systems architect, I want to assert `supersedes` relations as forward-only edges in the successor note, so that I don't have to manually edit the predecessor note or cause Git merge conflicts.
10. As an AI coding agent, I want inverse relationships (`superseded_by`, `required_by`, `supports`) to be dynamically projected by the engine, so that I can navigate bidirectional relationships without dual-authoring in frontmatter.
11. As a platform engineer, I want supersession to be scoped by environment or system, so that a production decision does not silently invalidate a staging or mobile exception.
12. As an AI coding agent querying in a specific scope (e.g., staging), I want to see active guidance for that scope alongside an advisory notice when a successor exists in another scope (e.g., production), so that I understand cross-environment divergence.
13. As a developer running graph linting, I want arbitrary $N$-node supersession cycles ($A \rightarrow B \rightarrow C \rightarrow A$) to be caught, so that infinite loops and recursive redirects are prevented.
14. As an AI coding agent, I want the query engine to strictly abstain and emit an explicit `UNRESOLVED_CONFLICT` report when competing successors or cycles exist, so that I never execute against silently guessed heuristic winners.
15. As a developer auditing note evidence, I want `supported_by` relations to point to readable evidence notes (incident reviews, benchmarks), so that factual claims have clear provenance.
16. As a DevOps engineer, I want `depends_on` relations to track downstream dependencies, so that I can execute impact analysis before refactoring core architecture.
17. As an engineer working with external systems, I want to export our knowledge graph to Google OKF v0.2 format without losing typed edges, so that external enterprise catalogs can consume our knowledge base.
18. As an engineer managing a multi-thousand-note vault, I want AST parsing to be executed in sub-millisecond streaming Rust, so that graph operations and CLI commands complete instantaneously.
19. As a developer working in restricted or CI environments, I want `akashic` distributed as a standalone static binary, so that I don't need Python virtual environments or runtime interpreters.
20. As a note author writing relative links, I want canonical POSIX paths (`./relative/path.md`) to be resolved relative to the note's filesystem location, so that notes remain browsable in plain Git interfaces.
21. As a note author migrating from Obsidian, I want the parser to normalize legacy wikilinks (`[[Note Name]]`) and top-level `supersedes:` fields during inspection and planning, so that legacy notes can be cleanly upgraded.
22. As an engineer reviewing a note that is partially obsolete, I want the engine to flag the note and recommend splitting it into distinct concepts, so that whole-note supersession hygiene is maintained.
23. As an AI coding agent requesting context, I want the output to be bounded by a token or depth limit, so that the assembled context packet fits comfortably within my prompt budget.
24. As an AI coding agent, I want truncated graph traversals to emit an explicit truncation indicator, so that I do not mistake a bounded traversal for an exhaustive search.
25. As a compliance officer, I want historical notes to remain untouched in the filesystem with valid lifecycle dates (`valid_from`, `valid_until`), so that historical state can be audited as of any point in time.
26. As a developer inspecting a vault, I want malformed YAML frontmatter or unreadable files to be reported as structured findings, so that syntax mistakes can be corrected without crashing the CLI.
27. As an AI coding agent, I want to query `--mode impact` for a note, so that I receive all transitive dependents that rely on the specified component.
28. As an AI coding agent, I want to query `--mode lineage` for a decision, so that I see the chronological chain of replacements leading up to the current decision.
29. As an engineer running CI, I want `akashic lint` to exit with a non-zero exit code if dangling links, broken scopes, or supersession cycles are detected, so that broken graphs cannot be merged to the main branch.
30. As a user of the CLI, I want literal and intuitive commands (`inspect`, `plan`, `apply`, `lint`, `query`, `export`), so that I can learn and operate the tool without memorizing obscure flags.

## Implementation Decisions

### 1. Architectural Components & Modules
The engine is structured into six cohesive modules within a single Rust crate:

- **`parser`**: Reader-agnostic Markdown AST and frontmatter extractor. Uses `pulldown-cmark` for zero-allocation streaming event parsing of Markdown structure (headings, relative links) and `serde_yaml` for YAML frontmatter extraction.
- **`model`**: Domain data structures implementing the ubiquitous language: `Node`, `Relation`, `Scope`, `Plan`, `Finding`, and `Lifecycle`.
- **`graph`**: In-memory directed graph wrapping `petgraph::DiGraph`. Manages dynamic inverse edge projections, executes Tarjan's Strongly Connected Components algorithm (`petgraph::algo::tarjan_scc`) to detect arbitrary cycles, and evaluates scope intersection and coverage logic.
- **`query`**: Agent context packet assembler. Evaluates query modes (`current`, `historical`, `lineage`, `impact`), enforces deterministic abstention on unresolved conflicts, and formats bounded Markdown packets with explicit status annotations.
- **`planner`**: Non-destructive vault inspector, diff generator, and idempotent applier. Computes SHA-256 hashes of note contents to guard against concurrent edits and produces reviewable JSON plans.
- **`okf`**: Google OKF v0.2 export serializer that maps native node lifecycle fields to OKF metadata and preserves typed relations in an OKF extension profile.
- **`cli`**: Binary entrypoint using `clap` derive, exposing literal subcommands: `inspect`, `plan`, `apply`, `lint`, `query`, and `export`.

### 2. Frontmatter Schema & OKF Extension Profile
All nodes use standard YAML frontmatter conforming to Google OKF v0.2 base fields, with typed edges housed in a standard `relations:` list:

```yaml
---
id: decision-payment-status-events
type: Decision
title: Use event-driven payment status
status: stable
valid_from: "2026-06-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./review-incident-timeout.md
  - type: supported_by
    target: ./review-incident-timeout.md
---
```

### 3. Canonical Relation Types & Constraints
- `supersedes`: Forward-only, strictly acyclic. Must match or cover target scope. Inverse: `superseded_by`.
- `depends_on`: Forward-only. Downstream operational requirement. Inverse: `required_by`.
- `supported_by`: Forward-only. Empirical evidence link. Inverse: `supports`.
- `contradicts`: Symmetric. Flagged during query as an unresolved conflict if active in the same scope.
- `caused_by`: Forward-only. Causal attribution from incident/outcome to root cause. Inverse: `caused`.
- `relates_to`: Symmetric. Associative link; excluded from supersession and dependency chains.

### 4. Deterministic Conflict Resolution (Strict Abstention)
When an $N$-node cycle is detected or when competing successors supersede the same predecessor without superseding each other within an overlapping scope, the engine strictly abstains from choosing a winner. It emits an explicit `[UNRESOLVED_CONFLICT]` block containing the competing notes and their evidence, refusing to fall back to timestamps or file modification dates.

### 5. Plan & Diff Contract
Plans are serialized as machine-readable JSON containing:
- `plan_version`: Semantic schema version.
- `vault_root`: Canonicalized path.
- `actions`: List of proposed file edits, each carrying `path`, `source_sha256`, `expected_new_sha256`, and unified diff patch.
Application verifies that `SHA256(current_file) == source_sha256` before applying any edit. If a mismatch is detected, application aborts cleanly without partial mutation.

## Testing Decisions

### Testing Seam
The primary and highest testing seam is the **CLI integration seam**. Tests will invoke the compiled `akashic` binary directly against real disk fixtures using `assert_cmd` and `tempfile`, validating:
- Command stdout, stderr, and exit codes.
- Structured JSON outputs (`--json`).
- Filesystem immutability during `inspect`, `plan`, and `lint`.
- Idempotent filesystem mutation during `apply`.
- Formatted Markdown context packet correctness during `query`.

A secondary internal seam exists at the public Rust library interface (`akashic::Vault`), used for fast in-process unit testing of scope algebra, Tarjan cycle detection, and frontmatter serialization.

### The Worked 15-Note Payment Vault Fixture
Integration tests run against a standardized 15-note fixture (`tests/fixtures/payment_vault/`) designed to verify all critical edge cases:
1. `concept-payment-status.md`: Root concept baseline.
2. `decision-payment-status-polling.md`: Deprecated predecessor.
3. `decision-payment-status-events.md`: Active successor superseding polling.
4. `review-incident-timeout.md`: Supporting evidence for events decision.
5. `incident-2026-05-status-timeout.md`: Incident caused by polling decision.
6. `runbook-payment-reconciliation.md`: Operational runbook depending on events decision.
7. `decision-payment-staging-polling.md`: Scoped exception active only in staging.
8. `proposal-payment-grpc.md`: Draft proposal contradicting events decision.
9. `cycle-canary-a.md`, `cycle-canary-b.md`, `cycle-canary-c.md`: 3-node supersession cycle.
10. `competing-successor.md`: Divergent successor superseding polling.
11. `stale-unreplaced-batch.md`: Stale note requiring review without an invented successor.
12. `external-source-stripe.md`: Source provenance document.
13. `corrupted-target.md`: Broken target link for lint validation.

## Out of Scope

- **Interactive TUI / GUI**: `akashic` is exclusively a headless CLI tool and library.
- **Embedding / Vector Search**: Lexical indexing, vector embeddings, and GraphRAG retrieval rerankers are out of scope for the deterministic core.
- **Continuous Daemon / File Watcher**: `akashic` is an on-demand tool invoked explicitly by humans, CI, or agent wrappers.
- **Section/Claim-Level AST Mutations**: Supersession operates strictly at whole-note granularity.
- **Automatic Unapproved Rewrites**: The tool never alters user notes without an explicit, reviewable plan.

## Further Notes

- All decisions align with [ADR-0001](file:///home/pedrogoncalves/projects/akashic-records/docs/adr/0001-clean-room-core-with-okf.md) through [ADR-0006](file:///home/pedrogoncalves/projects/akashic-records/docs/adr/0006-rust-implementation.md) and the ubiquitous language in [`CONTEXT.md`](file:///home/pedrogoncalves/projects/akashic-records/CONTEXT.md).
- Upstream compatibility with `obsidian-second-brain` notes is maintained via normalizers in the `inspect` and `plan` phases.
