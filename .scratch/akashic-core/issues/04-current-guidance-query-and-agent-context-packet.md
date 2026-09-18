# 04 — Current Guidance Query & Agent Context Packet

**What to build:** The core query command (`akashic query <vault> --seed <id_or_path> --mode current`) that resolves supersession chains to deliver current architectural guidance to AI coding agents. If queried with an obsolete note, it navigates forward `supersedes` edges to the authoritative successor, assembling a bounded Markdown context packet with historical notes labeled as background rationale.

**Blocked by:** 02 — Vault Traversal & In-Memory Graph Index

**Status:** completed

- [x] `akashic query <vault> --seed <id> --mode current` returns the active guidance note even when seeded with a superseded predecessor.
- [x] The generated context packet clearly annotates superseded notes as historical rationale rather than current guidance.
- [x] Respects declared scope filters (e.g. `--scope env=production`) when traversing supersession paths.
- [x] Formats output as bounded, copy-pasteable or pipeable Markdown suitable for LLM system/user prompts.
- [x] Emits a clear indication if a note has no successors and is itself current guidance.
