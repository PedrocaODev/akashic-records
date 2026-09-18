# Whole-Note Node Granularity

Section- or claim-level supersession within a single Markdown file requires brittle AST tracking that breaks when human authors edit notes in arbitrary Markdown editors. We decided that exactly one Markdown document represents one graph node, and supersession replaces whole notes within a declared scope. If a note is only partially obsolete, the migration planner recommends splitting the note into discrete concept documents.

## Consequences
- Preserves clean identity mapping and straightforward diff reviews.
- Directly aligns with Google OKF concept boundaries.
- Avoids complex block-level anchoring syntax.
