# Akashic Records

Reader-agnostic Markdown knowledge graph engine that preserves historical decisions while delivering explicit, verified current guidance and typed graph edges to AI agents.

## Language

**Node**:
A discrete Markdown document representing a single concept, decision, incident, runbook, or evidence source.
_Avoid_: Document, file, page, block.

**Relation**:
A typed, directed edge asserted in frontmatter connecting a source node to a target node with optional evidence and scope.
_Avoid_: Link, backlink, connection.

**Supersession**:
The explicit replacement of an obsolete node by an authoritative successor within an identical or covering scope.
_Avoid_: Deprecation, deletion.

**Current Guidance**:
The authoritative set of active nodes for a given scope, where superseded notes are resolved and redirected to their successors.
_Avoid_: Fresh notes, latest edits.

**Historical Context**:
The point-in-time state or lineage tracing how decisions evolved and why changes occurred.
_Avoid_: Archive, graveyard.

**Scope**:
A set of key-value attributes (e.g. system, environment, platform) bounding the domain where a node or relationship applies.
_Avoid_: Tag, category.

**Plan**:
A machine-readable, reviewable diff proposing missing metadata, normalized IDs, or inferred edges with supporting evidence before any file mutation.
_Avoid_: Patch, migration script.

**Unresolved Conflict**:
An invalid graph topology (such as a supersession cycle or competing successors) where the engine strictly abstains from choosing a winner and surfaces an explicit alert.
_Avoid_: Heuristic choice, silent fallback.
