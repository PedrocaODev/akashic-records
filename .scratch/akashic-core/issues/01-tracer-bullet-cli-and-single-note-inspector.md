# 01 — Tracer Bullet CLI & Single-Note Inspector

**What to build:** The foundational CLI binary (`akashic`) and streaming Markdown AST/frontmatter parser that can inspect a single Markdown note, extract its metadata, lifecycle state, scope, and declared typed relations, and output structured inspection data in human-readable or JSON format.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] `akashic inspect <file.md>` parses YAML frontmatter and Markdown body without error.
- [x] Extracted metadata conforms to Google OKF v0.2 fields (`id`, `type`, `title`, `status`, `valid_from`, `valid_until`) and our typed `relations:` list.
- [x] Passing `--json` outputs clean, structured JSON containing the parsed node representation.
- [x] Returns a clear error code and diagnostic message if frontmatter is invalid YAML or unreadable.
- [x] Works cleanly with standard GitHub-flavored Markdown without requiring Obsidian or any proprietary editor plugins.
