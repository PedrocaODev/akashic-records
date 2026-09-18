# Rust Implementation for High-Performance AST and Graph Core

While upstream second-brain prototypes used Python, parsing large Markdown vaults with hundreds or thousands of files requires fast AST parsing and deterministic memory safety without Python runtime dependencies. We decided to implement `akashic` in Rust, leveraging `pulldown-cmark` for zero-allocation streaming Markdown parsing, `petgraph` for Tarjan strongly connected components and graph traversals, `serde` for schema validation, and `clap` for the CLI.

## Consequences
- Distributable as a single static binary with no external interpreter or runtime dependency.
- Sub-millisecond AST parsing and graph validation even across extensive personal or team knowledge repositories.
- Strict compile-time typing guarantees for graph invariants, cycle handling, and scope intersection math.
