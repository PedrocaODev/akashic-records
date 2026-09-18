![Akashic Records](res/Akashic-Records.png)

## Key Principles

- **Reader-agnostic**: Nodes remain readable plain text Markdown. Standard Markdown editors open and render each node without custom plugins.
- **Typed directed relations**: Nodes declare explicit relations in YAML frontmatter. Every relation defines a relation kind, a target, and optional evidence or scope.
- **Forward-only edges**: Frontmatter allows only forward relations. The engine rejects inverse relation declarations like `superseded_by` or `supports`. The engine computes inverse relations dynamically during graph traversal.
- **Deterministic conflict abstention**: The engine halts on invalid graph topologies. When supersession cycles or competing successors occur, the engine emits an unresolved conflict alert and exits with code 2. It avoids heuristic choices and silent fallbacks.

## Canonical Note Frontmatter Specification

Every node in an Akashic vault starts with YAML frontmatter bounded by triple dashes (`---`).

```markdown
---
id: decision-payment-status-events
title: Event-driven payment status updates
type: Decision
status: active
valid_from: 2026-01-15
valid_until: 2027-01-15
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./evidence-polling-latency-metrics.md
    scope:
      env: production
---

# Event-driven payment status updates

Body content in GitHub Flavored Markdown.
```

### Frontmatter Fields

- **`id`**: Unique kebab-case identifier for the node.
- **`title`**: Human-readable title of the node.
- **`type`**: Node kind. Recognized canonical kinds include:
  - `Concept`
  - `Decision`
  - `Incident`
  - `Runbook`
  - `Evidence`
- **`status`**: Lifecycle state of the node. Allowed values:
  - `draft`
  - `proposed`
  - `active`
  - `stable`
  - `superseded`
  - `deprecated`
- **`valid_from`**: Start date for temporal validity in `YYYY-MM-DD` format.
- **`valid_until`**: Optional end date for temporal validity in `YYYY-MM-DD` format.
- **`scope`**: Map of key-value attributes that define domain boundaries (`system`, `env`, `platform`).
- **`relations`**: List of directed relations asserted by this node.

### Relations Block

Each relation entry contains:

- **`type`**: The relation kind. Canonical forward types include:
  - `supersedes`: Explicit supersession of an obsolete predecessor node.
  - `depends_on`: Declares a functional dependency on a target node.
  - `supported_by`: Attaches empirical evidence from a target node.
  - `caused_by`: Identifies a root cause node for an incident or behavior.
  - `relates_to`: Associates related conceptual context.
  - `contradicts`: Identifies mutually exclusive guidance between nodes.
- **`target`**: Relative path to the target node (`./target-node.md`).
- **`evidence`**: Optional relative path to an evidence node (`./evidence-node.md`).
- **`scope`**: Optional scope override defining where the specific relation applies.

## CLI Usage & Subcommands

Run `akashic` with one of the following subcommands.

### `akashic inspect <path> [--json]`

Inspect a single node or summarize an entire vault.

```bash
# Summarize a vault
akashic inspect ./vault

# Output structured JSON vault inventory
akashic inspect ./vault --json

# Inspect a single node
akashic inspect ./vault/decision-payment.md
```

### `akashic lint <vault> [--json]`

Verify graph topology and detect integrity violations before committing changes.

```bash
# Run integrity checks
akashic lint ./vault

# Output findings as structured JSON
akashic lint ./vault --json
```

The command checks for:
- `broken_target`: Relation targets a missing node path or ID.
- `supersession_cycle`: Circular supersession chains (e.g. A -> B -> C -> A).
- `conflicting_assertion`: Competing successors or active contradictions in overlapping scopes.

Exit codes:
- `0`: Vault is valid.
- `1`: Lint violations detected.

### `akashic query <vault> --seed <id> [--mode current|lineage|impact|historical] [--scope key=val]`

Extract deterministic context packets for human review or AI agent prompts.

```bash
# Query active current guidance (default mode)
akashic query ./vault --seed decision-payment-status-polling

# Trace decision evolution and attached empirical evidence
akashic query ./vault --seed decision-payment-status-events --mode lineage

# Analyze downstream dependents and blast radius
akashic query ./vault --seed concept-payment-status --mode impact

# Reconstruct historical context as of a past date
akashic query ./vault --seed decision-payment-status-polling --mode historical --as-of 2024-06-01

# Filter by scope attributes
akashic query ./vault --seed decision-payment-status-polling --mode current --scope env=production
```

Query modes:
- `current`: Resolves active current guidance by traversing supersession chains to authoritative successors.
- `lineage`: Traces historical context backward from successor to predecessor nodes with supporting evidence.
- `impact`: Traverses reverse dependency relations to compute blast radius and dependent depth.
- `historical`: Evaluates node status and validity intervals at a specified point in time.

If an unresolved conflict occurs during traversal, `akashic query` outputs a diagnostic alert and exits with code 2.

### `akashic plan <vault> --out plan.json`

Scan a vault and propose missing identifiers, relative link normalizations, and metadata corrections.

```bash
# Generate a migration plan
akashic plan ./vault --out plan.json
```

The generated plan file contains unified diffs and pre-image SHA-256 hashes for review before application.

### `akashic apply <vault> --plan plan.json [--dry-run]`

Apply changes from an approved plan file to the vault.

```bash
# Validate hashes without modifying files
akashic apply ./vault --plan plan.json --dry-run

# Apply mutations idempotently
akashic apply ./vault --plan plan.json
```

The applier validates SHA-256 hashes against current node files. If any hash differs, the applier aborts without modifying any files.

### `akashic export <vault> --format okf --out <dir>`

Export the vault to Google Open Knowledge Format (OKF v0.2).

```bash
# Export vault to OKF format
akashic export ./vault --format okf --out ./dist-okf

# Export with JSON manifest output
akashic export ./vault --format okf --out ./dist-okf --json
```

The command exports standard Markdown files and generates a `manifest.json` file that preserves all six relation kinds.

## Agent Skills Integration

Akashic Records provides an agent skill package in `skills/akashic-cli`.

Install the skill into an agent environment with the skills CLI:

```bash
npx skills add ./skills/akashic-cli
```

AI agents use this skill to run queries, check current guidance, inspect decision lineage, and lint vaults before submitting pull requests.

## Testing

Run the test suite with cargo:

```bash
cargo test
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
