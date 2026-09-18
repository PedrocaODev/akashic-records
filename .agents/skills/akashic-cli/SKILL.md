---
name: akashic-cli
description: Manage and query Akashic Records Markdown knowledge graph vaults with the akashic CLI. Use when initializing new vaults, authoring or scaffolding notes, defining typed relations (supersedes, depends_on, supported_by, caused_by, relates_to, contradicts), fixing lint integrity errors and cycles, planning migrations, or extracting AI agent context packets via akashic query (current, lineage, impact, historical).
---

# Akashic Records CLI (`akashic`)

Workflows and guidelines for initializing, authoring, maintaining, linting, and querying Markdown knowledge graph vaults using the `akashic` CLI.

## Core Concepts & Vocabulary

Consult [`CONTEXT.md`](CONTEXT.md) for normative definitions:

- **Node**: A discrete Markdown document representing a single concept, decision, incident, runbook, or evidence source.
- **Relation**: A typed, directed edge asserted in YAML frontmatter connecting a source node to a target node with optional evidence and scope.
- **Supersession**: The explicit replacement of an obsolete node by an authoritative successor within an identical or covering scope.
- **Current Guidance**: The authoritative set of active nodes for a given scope, where superseded notes are resolved and redirected to their successors.
- **Historical Context**: The point-in-time state or lineage tracing how decisions evolved and why changes occurred.
- **Scope**: A set of key-value attributes (e.g., `system`, `env`, `platform`) bounding where a node or relationship applies.
- **Plan**: A machine-readable, reviewable diff proposing missing metadata, normalized IDs, or inferred edges with supporting evidence before any file mutation.
- **Unresolved Conflict**: An invalid graph topology (such as a supersession cycle or competing successors) where the engine strictly abstains from choosing a winner and surfaces an explicit alert.

---

## 1. Vault Creation & Node Authoring

### Vault Structure
An Akashic vault is any directory containing Markdown files with canonical YAML frontmatter. Nodes may reside flat in the root or in domain subdirectories.

### Canonical Node Template
Create notes with standard YAML frontmatter:

```markdown
---
id: <kebab-case-slug>
title: "<Human Readable Title>"
type: Concept | Decision | Incident | Runbook | Evidence
status: draft | proposed | active | stable | superseded | deprecated
valid_from: YYYY-MM-DD
valid_until: YYYY-MM-DD   # Optional
scope:
  system: <system-name>
  env: production | staging | dev
relations:
  - type: supersedes | depends_on | supported_by | caused_by | relates_to | contradicts
    target: ./<relative-path-to-target>.md
    evidence: ./<relative-path-to-evidence>.md   # Optional/recommended for supersedes
    scope:                                       # Optional relationship-level scope
      env: production
---

# Node Title

Document body in GitHub Flavored Markdown (GFM).
```

### Directed Relations Rules
1. **Always forward-directed**: Only assert forward relations (`supersedes`, `depends_on`, `supported_by`, `caused_by`, `relates_to`, `contradicts`).
2. **Never assert inverse relations**: Do not write `superseded_by`, `depended_on_by`, or `caused`. The engine derives inverses automatically during graph traversal.
3. **Relative links**: Use `./<filename>.md` or relative subpaths for targets and evidence.

---

## 2. Vault Inspection (`akashic inspect`)

Inspect a single note or summarize an entire vault:

```bash
# Summarize entire vault (total nodes, edges, edge breakdown)
akashic inspect <vault-dir>

# Output machine-readable JSON vault inventory
akashic inspect <vault-dir> --json

# Inspect single note attributes and outgoing/incoming relations
akashic inspect <vault-dir>/<note-file>.md
```

---

## 3. Graph Integrity & Lint Remediation (`akashic lint`)

Run graph verification before committing changes:

```bash
akashic lint <vault-dir>
# Or structured JSON:
akashic lint <vault-dir> --json
```

Exit codes:
- `0`: Vault is healthy.
- `1`: Integrity violations found.

### Remediation Playbook

| Finding Kind | Cause | Remediation |
|---|---|---|
| `broken_target` | Relation targets a nonexistent file or missing note ID. | Fix relative path in `target:` or create the missing note. |
| `supersession_cycle` | Directed supersession cycle detected via Tarjan SCC (e.g., A -> B -> C -> A). | Remove the loop-closing `supersedes` relation. |
| `conflicting_assertion` (competing successors) | Multiple nodes supersede the same predecessor in overlapping scopes without superseding each other. | Explicitly order the successors (make one supersede the other) or narrow their `scope:` so scopes are disjoint. |
| `conflicting_assertion` (active contradiction) | Two active nodes contradict each other in overlapping scopes. | Add a supersession edge resolving one over the other, or narrow scopes so they apply to disjoint environments/systems. |

---

## 4. Querying & Agent Context Extraction (`akashic query`)

Extract deterministic context packets for AI agents:

```bash
akashic query <vault-dir> --seed <note-id-or-path> [OPTIONS]
```

### Query Modes (`--mode`)

1. **`current`** (default):
   - Resolves active guidance for a seed note.
   - Automatically navigates supersession hops from historical/superseded notes to their authoritative successor.
   - Example:
     ```bash
     akashic query <vault-dir> --seed decision-payment-status-polling --mode current
     ```

2. **`lineage`**:
   - Traces the chronological chain of decisions from root predecessor to active guidance.
   - Gathers all attached empirical evidence nodes (`supported_by`, `evidence:`).
   - Example:
     ```bash
     akashic query <vault-dir> --seed decision-payment-status-events --mode lineage
     ```

3. **`impact`**:
   - Computes downstream blast radius for a node.
   - Traverses reverse `depends_on` edges and reports total dependents and max path depth.
   - Example:
     ```bash
     akashic query <vault-dir> --seed concept-payment-status --mode impact
     ```

4. **`historical`**:
   - Reconstructs active guidance as it existed at a past date.
   - Requires `--as-of YYYY-MM-DD`.
   - Example:
     ```bash
     akashic query <vault-dir> --seed decision-payment-status-polling --mode historical --as-of 2024-06-01
     ```

### Scope Filtering
Filter queries by key-value pairs (e.g., `--scope env=production` or `--scope system=payments`). When a successor exists in another scope, the engine outputs an explicit `[!NOTE]` advisory with the alternative scope.

### Deterministic Conflict Abstention (`[UNRESOLVED_CONFLICT]`)
When a graph topology contains competing successors, cycles, or unresolved contradictions:
- `akashic query` exits with code `2`.
- Outputs a structured `[UNRESOLVED_CONFLICT]` warning packet detailing the disputed candidates.
- **Rule for AI Agents**: Never guess or choose a winner when exit code 2 is returned; surface the conflict to the user for human resolution.

---

## 5. Migration Planning & Idempotent Applier (`akashic plan` & `akashic apply`)

Automate metadata normalization, missing IDs, and link cleanup:

1. **Generate plan**:
   ```bash
   akashic plan <vault-dir> --out plan.json
   ```
2. **Review & Dry-run**:
   Inspect the unified diffs in `plan.json` and verify pre-image SHA-256 hashes:
   ```bash
   akashic apply <vault-dir> --plan plan.json --dry-run
   ```
3. **Apply mutations**:
   ```bash
   akashic apply <vault-dir> --plan plan.json
   ```
   Aborts without mutating any files if any pre-image hash mismatches.

---

## 6. Open Knowledge Format Export (`akashic export`)

Export an Akashic vault to Google OKF v0.2:

```bash
akashic export <vault-dir> --format okf --out <output-dir> [--json]
```
- Converts WikiLinks to standard Markdown links.
- Writes a validated `manifest.json` preserving all 6 relation types losslessly.
