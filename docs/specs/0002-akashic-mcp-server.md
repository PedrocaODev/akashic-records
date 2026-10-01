# Spec 0002: Akashic Records Model Context Protocol (MCP) Server

Status: ready-for-agent
Triage: ready-for-agent

## Problem Statement

Software engineers and AI coding agents operating across multiple project workspaces, IDEs, and CLI sessions (such as Antigravity CLI/IDE, Gemini CLI, Pi, Claude Code, and Cursor) lack direct, unified, and semantically sound access to their centralized personal and work knowledge graph. In daily workflows, agents make decisions, consult runbooks, and investigate architecture without awareness of active conventions or recent decisions documented in the user's knowledge vault.

Existing off-the-shelf solutions fail to solve this problem:
1. **Obsidian Desktop Dependency:** Community Obsidian MCP servers require the Obsidian Desktop application to remain open and active with the `obsidian-local-rest-api` plugin. This architecture breaks in headless environments, CI pipelines, background subagents, remote SSH sessions, and automated scheduled maintenance jobs.
2. **Knowledge Graph Blindness:** Generic filesystem-based MCP servers and text-indexing tools treat knowledge repositories as flat collections of Markdown documents. They are completely oblivious to Open Knowledge Format (OKF v0.2) semantics, including forward-only typed directed relations (`supersedes`, `depends_on`, `supported_by`, `caused_by`, `relates_to`, `contradicts`), temporal validity intervals, and scope boundaries.
3. **Agent Hallucination on Obsolete Context:** When querying architectural decisions or procedural runbooks, generic search tools return obsolete, superseded notes alongside active guidance based on lexical matching. Without deterministic supersession resolution, AI agents hallucinate and implement patterns that were explicitly replaced.
4. **Unsafe Concurrent Mutations:** Multiple agent sessions reading and writing to the knowledge vault simultaneously without synchronization risk race conditions, corrupted frontmatter, broken wikilinks, and violation of vault operating rules (such as dual-writing canonical updates with linked entries in chronological daily logs).

## Solution

Build a native Model Context Protocol (MCP) server directly into the reader-agnostic `akashic` Rust binary (`akashic mcp`). 

The MCP server exposes a high-leverage, deterministic tool surface over standard JSON-RPC 2.0 (via `stdio` for local child-process integration and optionally local HTTP/SSE for persistent daemon execution). It allows any MCP-compliant AI assistant to query authoritative current guidance, trace historical decision lineage with empirical evidence, perform dependency blast-radius analysis, inspect vault structure, verify graph integrity, and execute guarded atomic mutations conforming to the vault's dual-write operating contract.

Mutations are safeguarded by pre-write graph linting (preventing supersession cycles and broken links before disk writes) and advisory file locking, ensuring transactional integrity across concurrent agent sessions. Universal access is established by declaring the `akashic mcp` server once in global agent configuration files across runtimes.

## User Stories

1. As an AI coding agent starting a session, I want to discover available knowledge graph capabilities via standard MCP `tools/list`, so that I know what inspection, query, and mutation tools are supported.
2. As an AI coding agent, I want to invoke `akashic_query` with mode `current`, so that I retrieve only authoritative, active guidance for a concept without acting on superseded decisions.
3. As an AI coding agent querying a concept that was superseded, I want the tool response to explicitly identify the active successor and explain the transition, so that I navigate to current truth without manual searching.
4. As an AI coding agent investigating an architectural choice or post-mortem, I want to invoke `akashic_query` with mode `lineage`, so that I can trace the historical chain of replacements and inspect attached empirical evidence.
5. As an AI coding agent planning a refactor, I want to invoke `akashic_query` with mode `impact`, so that I calculate the blast radius and retrieve all downstream components that depend on the target node.
6. As an AI coding agent auditing past behavior, I want to invoke `akashic_query` with mode `historical` and an `as_of` date, so that I evaluate what guidance was officially active at a specific point in the past.
7. As an AI coding agent operating in a specific deployment context, I want `akashic_query` to accept scope parameters (such as `env=production` or `platform=android`), so that environment-specific decisions are correctly resolved.
8. As an AI coding agent querying an ambiguous graph topology (such as circular supersession or competing unranked successors), I want the query tool to strictly abstain and return an explicit `UNRESOLVED_CONFLICT` block, so that I never execute instructions based on heuristically guessed winners.
9. As an AI coding agent, I want to invoke `akashic_inspect` on the vault root, so that I receive an inventory summary of total nodes, total edges, edge distributions by type, and overall vault health.
10. As an AI coding agent, I want to invoke `akashic_inspect` for a single node identifier or relative path, so that I can inspect its complete metadata, forward relations, and dynamically projected inverse edges.
11. As an AI coding agent before finalizing changes, I want to invoke `akashic_lint`, so that I can verify that no broken targets, supersession cycles, or conflicting assertions exist in the graph.
12. As an AI coding agent, I want to invoke `vault_read` with a node identifier, so that I receive the raw Markdown body and parsed YAML frontmatter of that canonical note.
13. As an AI coding agent tasked with saving durable knowledge, I want to invoke `vault_write_canonical`, so that new concepts, decisions, runbooks, or evidence are safely added to the knowledge vault.
14. As an AI coding agent, I want `vault_write_canonical` to validate frontmatter against the OKF v0.2 specification, so that invalid node types or missing mandatory fields are rejected before touching the filesystem.
15. As an AI coding agent, I want `vault_write_canonical` to enforce forward-only relationship declarations, so that inverse relations (`superseded_by`, `required_by`) are rejected and cannot corrupt frontmatter.
16. As an AI coding agent, I want `vault_write_canonical` to automatically append a linked summary entry into today's daily log note (`Daily/YYYY-MM-DD.md`), so that chronological audit trails remain consistent with canonical knowledge.
17. As an AI coding agent, I want `vault_write_canonical` to execute the canonical write and daily note append as an atomic transaction, so that partial failures never leave the vault in an inconsistent state.
18. As a developer running multiple concurrent agent sessions, I want all vault write operations to acquire an advisory file lock, so that concurrent mutations from separate sessions serialize safely without file corruption or race conditions.
19. As a software engineer running in headless environments or remote SSH containers, I want the MCP server to operate without requiring the Obsidian Desktop app to be running, so that automated workflows never depend on a GUI window.
20. As an engineer running long-lived background daemons, I want `akashic mcp` to support an optional local HTTP/SSE transport, so that multiple tools can connect to a single persistent in-memory graph server.
21. As an AI assistant connected to the HTTP/SSE daemon, I want filesystem modifications to automatically refresh the in-memory graph index via filesystem event watching, so that query results are always fresh.
22. As an AI coding agent sending malformed parameters, I want structured JSON-RPC error responses with clear descriptive error messages, so that I can immediately self-correct my tool call.
23. As a developer configuring Antigravity CLI, Antigravity IDE, or Gemini CLI, I want to add `akashic mcp` to `~/.gemini/config/mcp_config.json`, so that all workspaces automatically inherit knowledge graph tools.
24. As a developer configuring the Pi agent, I want to add `akashic mcp` to `~/.pi/agent/mcp.json`, so that all Pi sessions have direct access to knowledge tools.
25. As a developer deploying agent environments, I want `akashic mcp` compiled directly into the static `akashic` binary, so that no Python, Node.js, or external runtime dependencies are required.
26. As an AI coding agent receiving large graph query responses, I want context packets to be bounded with explicit truncation markers, so that the response never overflows my context window.
27. As a human note author reading notes in any editor, I want all written notes to remain standard GitHub Flavored Markdown, so that MCP tools introduce no proprietary lock-in.
28. As a security-conscious engineer, I want `akashic mcp` to sandbox file operations to the designated vault path, so that malicious or accidental path traversal outside the vault is rejected.
29. As an AI coding agent, I want tool parameter schemas to include comprehensive JSON schema descriptions and usage examples, so that tool discovery is unambiguous.
30. As a system operator terminating an agent session, I want `akashic mcp` to handle termination signals (`SIGINT`, `SIGTERM`) gracefully, so that all advisory file locks and file descriptors are immediately released.

## Implementation Decisions

### 1. Protocol Architecture & Integration Layer
- Implement the Model Context Protocol server directly within the `akashic` crate as an `mcp` module, accessible via the CLI subcommand `akashic mcp --vault <PATH>`.
- Use the official Model Context Protocol Rust SDK (`rmcp`) from `modelcontextprotocol/rust-sdk` on crates.io, leveraging its native async server implementation, JSON-RPC 2.0 serialization, and JSON schema derivation via `schemars`.
- Support standard input/output (`stdio`) transport as the default mechanism, with an optional Streamable HTTP/SSE transport flag for daemon deployment.

### 2. Tool Surface & Parameter Contracts
The server exposes five distinct, high-leverage tools:
- `akashic_query`: Parameters: `seed` (string, required), `mode` (enum: `current`, `lineage`, `impact`, `historical`), `scope` (map of string key-values, optional), `as_of` (date string, optional). Evaluates queries by delegating directly to the existing `query` module and returns formatted Markdown context packets with explicit conflict blocks.
- `akashic_inspect`: Parameters: `target` (string, optional path or node ID). When omitted, returns vault-wide inventory and relation distributions. When provided, returns detailed metadata, forward relations, and computed inverse edges.
- `akashic_lint`: Parameters: None. Runs topological graph linting using the existing `linter` module, returning 0 violations or structured lists of broken targets, cycles, and conflicting assertions.
- `vault_read`: Parameters: `node_id` (string, required). Resolves the note by identifier or relative path and returns its complete Markdown body and YAML frontmatter.
- `vault_write_canonical`: Parameters: `id` (string, required), `title` (string, required), `type` (enum of permitted OKF types, required), `relations` (list of relation objects with `type`, `target`, and optional `scope`/`evidence`), `body` (string, required), `daily_summary` (string, required). Atomically writes the canonical note and appends a linked summary to today's daily log.

### 3. Concurrency, Locking, and Atomic Dual-Write Transaction
- Write operations acquire an exclusive advisory file lock on `.vault.lock` (located within the vault's `.git/` directory, or at the vault root if not a Git repository) using the `fs2` cross-platform file locking crate.
- Read operations (`akashic_query`, `akashic_inspect`, `vault_read`, `akashic_lint`) run concurrently without locking.
- The write transaction follows a strict preflight pipeline:
  1. Acquire exclusive advisory lock.
  2. Parse prospective note content and validate OKF schema.
  3. Validate prospective relations against the in-memory graph to verify that the write does not introduce supersession cycles or broken targets.
  4. Ensure today's daily log note exists (`Daily/YYYY-MM-DD.md`), creating it from standard daily template if absent.
  5. Write canonical note file atomically via temporary file rename.
  6. Append linked summary line to today's daily note.
  7. Release advisory lock.

### 4. Direct Module Reuse
- The MCP server acts as a clean transport adapter around the existing tested engine:
  - Query execution maps directly to `query::execute_query`.
  - Integrity checking maps directly to `linter::run_lint`.
  - Graph traversal and inverse edge computation map directly to `graph::build_vault_graph`.
  - Frontmatter parsing and serialization map directly to `parser` and `model`.

## Testing Decisions

### 1. The Primary Seam: The JSON-RPC Stdio Protocol Boundary
- In accordance with the Single Seam principle, all integration and acceptance tests for the MCP feature will be written against the highest possible seam: the **MCP JSON-RPC 2.0 protocol boundary over standard I/O**.
- Tests will spawn the compiled `akashic` binary in subprocess mode with arguments `mcp --vault <FIXTURE_PATH>`, write JSON-RPC request frames to `stdin`, and assert structured JSON-RPC responses from `stdout`.
- No internal components, functions, or data structures will be mocked or intercepted. Testing external behavior through the exact protocol used by real AI agents ensures complete end-to-end fidelity.

### 2. Test Suite Structure & Coverage
A dedicated test suite `tests/cli_mcp.rs` will cover:
- Protocol Handshake: `initialize` request, capability negotiation, and `notifications/initialized`.
- Tool Discovery: `tools/list` schema validation, verifying all 5 tools and their parameter schemas match expectations.
- Query Execution: Invoking `akashic_query` across all four modes (`current`, `lineage`, `impact`, `historical`) against representative graph fixtures.
- Conflict Abstention: Verifying that calling `akashic_query` against a cycle or competing successors returns the `UNRESOLVED_CONFLICT` block over the protocol without process panic.
- Structural Inspection & Linting: Invoking `akashic_inspect` and `akashic_lint` over JSON-RPC.
- Guarded Writes: Calling `vault_write_canonical` on a temporary test vault, verifying that the canonical note is written, the daily note is appended, and subsequent queries find the newly added node.
- Rejection of Invalid Writes: Verifying that attempting to write an inverse relation (e.g. `superseded_by`) or malformed frontmatter yields a clear JSON-RPC error response and leaves disk files unchanged.
- Concurrent Write Locking: Spawning two concurrent client processes attempting simultaneous mutations to verify serialization without race conditions.

### 3. Prior Art in the Codebase
- Existing acceptance suites in `tests/cli_query.rs`, `tests/cli_lint.rs`, and `tests/cli_acceptance.rs` demonstrate established patterns for fixture management, CLI invocation via `assert_cmd`, and JSON output verification.

## Out of Scope

1. **Full-Text Vector RAG Embeddings:** Semantic vector embeddings and nearest-neighbor search are out of scope. Akashic Records focuses on deterministic topological graph relationships and authoritative knowledge navigation; vector search can be layered externally if desired.
2. **Remote Multi-Tenant Cloud Authentication:** Multi-tenant user management, OAuth2, and public internet exposure are out of scope. The MCP server is designed as a local-first single-tenant tool.
3. **Web GUI Graph Visualizer:** An in-browser graph visualization server inside the binary is out of scope. Visual graph rendering is already handled via Mermaid diagrams (`mmdflux`) and standard Obsidian graph views.
4. **Natural Language Processing within the Engine:** The binary does not call LLMs or perform prompt synthesis. It acts as an authoritative, zero-hallucination data and tool provider to calling agents.

## Further Notes

- Once implemented, universal registration is accomplished by adding the server block to `~/.gemini/config/mcp_config.json` and `~/.pi/agent/mcp.json`.
- The `rmcp` crate supports async tokio runtimes. Adding `rmcp` to `Cargo.toml` will require enabling Tokio features (`rt-multi-thread`, `io-std`).
