use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::model::{MalformedFile, Node, Relation, RelationType, Scope, VaultInventory};
use crate::parser::parse_note_file;

/// Typed relationship edge data connecting two nodes in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeData {
    pub relation_type: RelationType,
    pub evidence: Option<String>,
    pub scope: Option<Scope>,
}

/// Normalizes a path string to a canonical relative POSIX path.
/// Converts Windows backslashes to forward slashes and resolves '.' and '..'.
/// Returns `None` if the path attempts to escape the root via '..'.
pub fn normalize_posix_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for comp in normalized.split('/') {
        match comp {
            "" | "." => continue,
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// Resolves a target path relative to a source directory.
/// Returns `None` if the target path attempts to escape the vault root.
pub fn resolve_relative_path(source_dir: &str, target: &str) -> Option<String> {
    let target_norm = target.replace('\\', "/");
    if target_norm.starts_with('/') {
        return normalize_posix_path(&target_norm);
    }

    if source_dir.is_empty() {
        normalize_posix_path(target)
    } else {
        normalize_posix_path(&format!("{}/{}", source_dir, target))
    }
}

/// Helper to determine whether a WalkDir entry is a hidden file or directory.
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() > 0 {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with('.') {
                return true;
            }
        }
    }
    false
}

/// In-memory graph representation of an entire Markdown vault.
#[derive(Debug)]
pub struct VaultGraph {
    pub graph: DiGraph<Node, EdgeData>,
    pub node_by_id: HashMap<String, NodeIndex>,
    pub node_by_path: HashMap<String, NodeIndex>,
    pub malformed_files: Vec<MalformedFile>,
    pub vault_path: String,
}

impl VaultGraph {
    /// Creates a new empty `VaultGraph`.
    pub fn new(vault_path: impl Into<String>) -> Self {
        Self {
            graph: DiGraph::new(),
            node_by_id: HashMap::new(),
            node_by_path: HashMap::new(),
            malformed_files: Vec::new(),
            vault_path: vault_path.into(),
        }
    }

    /// Recursively scans a vault directory, parsing all Markdown notes and building the graph.
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, std::io::Error> {
        let dir_ref = dir.as_ref();
        if !dir_ref.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Vault directory not found: {}", dir_ref.display()),
            ));
        }
        if !dir_ref.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {}", dir_ref.display()),
            ));
        }

        let mut vault = Self::new(dir_ref.display().to_string());
        let mut parsed_nodes: Vec<(NodeIndex, Vec<Relation>, String)> = Vec::new();

        // Pass 1: Walk directory and parse all markdown notes
        for entry_res in WalkDir::new(dir_ref)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
        {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                        let rel_path = match path.strip_prefix(dir_ref) {
                            Ok(p) => p,
                            Err(_) => path,
                        };
                        let rel_posix = normalize_posix_path(&rel_path.to_string_lossy())
                            .unwrap_or_else(|| rel_path.to_string_lossy().replace('\\', "/"));

                        match parse_note_file(path) {
                            Ok(mut node) => {
                                node.path = Some(rel_posix.clone());
                                let relations = node.relations.clone();
                                let id = node.id.clone();
                                let idx = vault.graph.add_node(node);
                                vault.node_by_id.insert(id, idx);
                                vault.node_by_path.insert(rel_posix.clone(), idx);
                                parsed_nodes.push((idx, relations, rel_posix));
                            }
                            Err(err) => {
                                vault.malformed_files.push(MalformedFile {
                                    path: rel_posix,
                                    error: err.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Pass 2: Resolve relations and add directed edges
        for (source_idx, relations, rel_posix) in parsed_nodes {
            for rel in relations {
                if let Some(target_idx) = vault.resolve_target(&rel_posix, &rel.target) {
                    vault.graph.add_edge(
                        source_idx,
                        target_idx,
                        EdgeData {
                            relation_type: rel.relation_type,
                            evidence: rel.evidence,
                            scope: rel.scope,
                        },
                    );
                }
            }
        }

        Ok(vault)
    }

    /// Looks up a node by its normalized relative POSIX path, with or without Markdown extension.
    pub fn find_by_normalized_path(&self, rel_path: &str) -> Option<NodeIndex> {
        if rel_path.is_empty() {
            return None;
        }
        if let Some(&idx) = self.node_by_path.get(rel_path) {
            return Some(idx);
        }
        if !rel_path.ends_with(".md") && !rel_path.ends_with(".markdown") {
            let with_md = format!("{}.md", rel_path);
            if let Some(&idx) = self.node_by_path.get(&with_md) {
                return Some(idx);
            }
            let with_markdown = format!("{}.markdown", rel_path);
            if let Some(&idx) = self.node_by_path.get(&with_markdown) {
                return Some(idx);
            }
        }
        None
    }

    /// Attempts to resolve a relation target to a NodeIndex in the graph.
    ///
    /// Resolution strategy:
    /// - If the target starts with `./` or `../`, it is resolved ONLY relative to the source
    ///   note's directory without falling back to the vault root.
    /// - Otherwise:
    ///   1. Direct match on note ID.
    ///   2. Normalized POSIX path relative to the source note's directory.
    ///   3. Normalized POSIX path relative to the vault root.
    pub fn resolve_target(&self, source_path: &str, target: &str) -> Option<NodeIndex> {
        let target_clean = target.replace('\\', "/");

        // Determine directory of source note
        let source_dir = Path::new(source_path)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let source_dir = if source_dir == "." { "" } else { &source_dir };

        if target_clean.starts_with("./") || target_clean.starts_with("../") {
            let rel_target = resolve_relative_path(source_dir, &target_clean)?;
            return self.find_by_normalized_path(&rel_target);
        }

        // 1. Direct ID lookup
        if let Some(&idx) = self.node_by_id.get(target) {
            return Some(idx);
        }

        // 2. Relative to source note's directory
        if let Some(rel_target) = resolve_relative_path(source_dir, &target_clean) {
            if let Some(idx) = self.find_by_normalized_path(&rel_target) {
                return Some(idx);
            }
        }

        // 3. Relative to vault root
        if let Some(root_target) = normalize_posix_path(&target_clean) {
            if let Some(idx) = self.find_by_normalized_path(&root_target) {
                return Some(idx);
            }
        }

        None
    }

    /// Dynamically projects inverse relations for a given note ID without writing to disk.
    pub fn inverse_relations(&self, node_id: &str) -> Option<Vec<Relation>> {
        let node_idx = self.node_by_id.get(node_id).copied().or_else(|| {
            normalize_posix_path(node_id).and_then(|norm| self.find_by_normalized_path(&norm))
        })?;

        let mut inv_relations = Vec::new();

        for edge_ref in self.graph.edges_directed(node_idx, Direction::Incoming) {
            let edge_data = edge_ref.weight();
            if let Some(inv_type) = edge_data.relation_type.inverse() {
                let source_node = &self.graph[edge_ref.source()];
                inv_relations.push(Relation {
                    relation_type: inv_type,
                    target: source_node.id.clone(),
                    evidence: edge_data.evidence.clone(),
                    scope: edge_data.scope.clone(),
                });
            }
        }

        inv_relations.sort_by(|a, b| {
            a.target
                .cmp(&b.target)
                .then_with(|| a.relation_type.as_str().cmp(b.relation_type.as_str()))
        });

        Some(inv_relations)
    }

    /// Computes vault-wide inventory metrics.
    pub fn inventory(&self) -> VaultInventory {
        let total_nodes = self.graph.node_count();
        let total_edges = self.graph.edge_count();

        let mut edges_by_type = BTreeMap::new();
        for edge in self.graph.edge_references() {
            let type_str = edge.weight().relation_type.as_str();
            if let Some(count) = edges_by_type.get_mut(type_str) {
                *count += 1;
            } else {
                edges_by_type.insert(type_str.to_string(), 1);
            }
        }

        let mut malformed_files = self.malformed_files.clone();
        malformed_files.sort_by(|a, b| a.path.cmp(&b.path));

        VaultInventory {
            vault_path: self.vault_path.clone(),
            total_nodes,
            total_edges,
            edges_by_type,
            malformed_files,
        }
    }

    /// Returns a reference to the inner directed graph.
    pub fn graph(&self) -> &DiGraph<Node, EdgeData> {
        &self.graph
    }

    /// Gets a note by its unique ID.
    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.node_by_id.get(id).map(|&idx| &self.graph[idx])
    }

    /// Gets a note by its normalized relative POSIX path.
    pub fn get_node_by_path(&self, path: &str) -> Option<&Node> {
        let normalized = normalize_posix_path(path)?;
        self.find_by_normalized_path(&normalized)
            .map(|idx| &self.graph[idx])
    }

    /// Lints the vault graph for integrity violations and returns a `LintReport`.
    pub fn lint(&self) -> crate::model::LintReport {
        crate::linter::lint_vault(self)
    }

    /// Finds all strongly connected components forming supersession cycles.
    pub fn find_supersession_cycles(&self) -> Vec<Vec<String>> {
        crate::linter::find_supersession_cycles(self)
    }

    /// Resolves a seed identifier or path (ID, relative path, or filesystem path) to a NodeIndex.
    pub fn resolve_seed(&self, seed: &str) -> Option<NodeIndex> {
        let seed_clean = seed.trim();
        if seed_clean.is_empty() {
            return None;
        }

        // 1. Direct ID lookup
        if let Some(&idx) = self.node_by_id.get(seed_clean) {
            return Some(idx);
        }

        // 2. Normalized POSIX path lookup (with or without extension)
        if let Some(norm) = normalize_posix_path(seed_clean) {
            if let Some(idx) = self.find_by_normalized_path(&norm) {
                return Some(idx);
            }
        }

        // 3. Filesystem path relative to vault root
        let seed_path = Path::new(seed_clean);
        let vault_path = Path::new(&self.vault_path);

        if let Ok(rel) = seed_path.strip_prefix(vault_path) {
            if let Some(norm) = normalize_posix_path(&rel.to_string_lossy()) {
                if let Some(idx) = self.find_by_normalized_path(&norm) {
                    return Some(idx);
                }
            }
        }

        if let (Ok(canon_seed), Ok(canon_vault)) =
            (seed_path.canonicalize(), vault_path.canonicalize())
        {
            if let Ok(rel) = canon_seed.strip_prefix(&canon_vault) {
                if let Some(norm) = normalize_posix_path(&rel.to_string_lossy()) {
                    if let Some(idx) = self.find_by_normalized_path(&norm) {
                        return Some(idx);
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_posix_path() {
        assert_eq!(
            normalize_posix_path("./a/b/c.md"),
            Some("a/b/c.md".to_string())
        );
        assert_eq!(
            normalize_posix_path("a/b/../c.md"),
            Some("a/c.md".to_string())
        );
        assert_eq!(
            normalize_posix_path("a/b/../../c.md"),
            Some("c.md".to_string())
        );
        assert_eq!(
            normalize_posix_path(".\\a\\b\\c.md"),
            Some("a/b/c.md".to_string())
        );
        assert_eq!(
            normalize_posix_path("a//b///c.md"),
            Some("a/b/c.md".to_string())
        );
        assert_eq!(normalize_posix_path("note.md"), Some("note.md".to_string()));
        assert_eq!(
            normalize_posix_path("./note.md"),
            Some("note.md".to_string())
        );

        // Root escape attempts must return None
        assert_eq!(normalize_posix_path("../c.md"), None);
        assert_eq!(normalize_posix_path("a/../../c.md"), None);
        assert_eq!(normalize_posix_path("./../../escape.md"), None);
    }

    #[test]
    fn test_resolve_relative_path() {
        assert_eq!(
            resolve_relative_path("docs/arch", "./adr.md"),
            Some("docs/arch/adr.md".to_string())
        );
        assert_eq!(
            resolve_relative_path("docs/arch", "../decisions/adr.md"),
            Some("docs/decisions/adr.md".to_string())
        );
        assert_eq!(
            resolve_relative_path("", "./adr.md"),
            Some("adr.md".to_string())
        );
        assert_eq!(
            resolve_relative_path("docs", "/root.md"),
            Some("root.md".to_string())
        );

        // Root escapes
        assert_eq!(resolve_relative_path("", "../outside.md"), None);
        assert_eq!(resolve_relative_path("docs", "../../outside.md"), None);
    }

    #[test]
    fn test_vault_graph_traversal_and_inverse() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create subdirectories
        let decisions_dir = root.join("decisions");
        std::fs::create_dir_all(&decisions_dir).unwrap();
        let reviews_dir = root.join("reviews");
        std::fs::create_dir_all(&reviews_dir).unwrap();
        let hidden_dir = root.join(".git");
        std::fs::create_dir_all(&hidden_dir).unwrap();

        // Note 1: decisions/adr-1.md (supersedes adr-2 by relative path, supported_by rev-1 by ID)
        let note1 = r#"---
id: adr-001
type: Decision
title: Use Rust
relations:
  - type: supersedes
    target: ./adr-2.md
  - type: supported_by
    target: rev-001
---
Content 1
"#;
        let mut f1 = File::create(decisions_dir.join("adr-1.md")).unwrap();
        f1.write_all(note1.as_bytes()).unwrap();

        // Note 2: decisions/adr-2.md
        let note2 = r#"---
id: adr-002
type: Decision
title: Use C++
---
Content 2
"#;
        let mut f2 = File::create(decisions_dir.join("adr-2.md")).unwrap();
        f2.write_all(note2.as_bytes()).unwrap();

        // Note 3: reviews/rev-1.md
        let note3 = r#"---
id: rev-001
type: Review
title: Benchmark Rust vs C++
---
Content 3
"#;
        let mut f3 = File::create(reviews_dir.join("rev-1.md")).unwrap();
        f3.write_all(note3.as_bytes()).unwrap();

        // Hidden note in .git should be ignored
        let hidden_note = r#"---
id: git-note
type: Internal
title: Hidden Git Note
---
Git content
"#;
        let mut f_git = File::create(hidden_dir.join("hidden.md")).unwrap();
        f_git.write_all(hidden_note.as_bytes()).unwrap();

        // Malformed note in root
        let malformed_note = r#"---
broken: [yaml
---
"#;
        let mut f_bad = File::create(root.join("bad.md")).unwrap();
        f_bad.write_all(malformed_note.as_bytes()).unwrap();

        let graph = VaultGraph::from_dir(root).unwrap();

        // Check node counts and indexing
        assert_eq!(graph.graph.node_count(), 3);
        assert!(graph.get_node("adr-001").is_some());
        assert!(graph.get_node("adr-002").is_some());
        assert!(graph.get_node("rev-001").is_some());
        assert!(graph.get_node("git-note").is_none());

        // Check path lookup
        assert!(graph.get_node_by_path("decisions/adr-1.md").is_some());
        assert!(graph.get_node_by_path("decisions/adr-2.md").is_some());
        assert!(graph.get_node_by_path("reviews/rev-1.md").is_some());

        // Check edges
        assert_eq!(graph.graph.edge_count(), 2);

        // Check inverse relations
        let inv_adr2 = graph.inverse_relations("adr-002").unwrap();
        assert_eq!(inv_adr2.len(), 1);
        assert_eq!(inv_adr2[0].relation_type, RelationType::SupersededBy);
        assert_eq!(inv_adr2[0].target, "adr-001");

        let inv_rev1 = graph.inverse_relations("rev-001").unwrap();
        assert_eq!(inv_rev1.len(), 1);
        assert_eq!(inv_rev1[0].relation_type, RelationType::Supports);
        assert_eq!(inv_rev1[0].target, "adr-001");

        // Check inventory
        let inv = graph.inventory();
        assert_eq!(inv.total_nodes, 3);
        assert_eq!(inv.total_edges, 2);
        assert_eq!(inv.edges_by_type.get("supersedes"), Some(&1));
        assert_eq!(inv.edges_by_type.get("supported_by"), Some(&1));
        assert_eq!(inv.malformed_files.len(), 1);
        assert_eq!(inv.malformed_files[0].path, "bad.md");
    }

    #[test]
    fn test_resolve_target_relative_does_not_fallback_to_root() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        // Note in root
        let root_note = r#"---
id: root-doc
type: Concept
title: Root Doc
---
Root
"#;
        std::fs::write(root.join("target.md"), root_note).unwrap();

        // Note in sub referencing ./target.md (does not exist in sub/, only in root/)
        let sub_note = r#"---
id: sub-doc
type: Decision
title: Sub Doc
relations:
  - type: relates_to
    target: ./target.md
  - type: depends_on
    target: ../../root-escape.md
---
Sub
"#;
        std::fs::write(sub.join("sub.md"), sub_note).unwrap();

        let graph = VaultGraph::from_dir(root).unwrap();
        assert_eq!(graph.graph.node_count(), 2);
        // Neither edge should be created:
        // 1. ./target.md must NOT fall back to root/target.md
        // 2. ../../root-escape.md escapes root
        assert_eq!(graph.graph.edge_count(), 0);
    }

    #[test]
    fn test_markdown_extension_recognition_and_lookup() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note1 = r#"---
id: note-md
type: Concept
title: Note MD
relations:
  - type: relates_to
    target: ./note-mkd.markdown
---
Content
"#;
        let note2 = r#"---
id: note-mkd
type: Concept
title: Note Markdown
relations:
  - type: relates_to
    target: note-md
---
Content
"#;
        std::fs::write(root.join("note-md.md"), note1).unwrap();
        std::fs::write(root.join("note-mkd.markdown"), note2).unwrap();

        let graph = VaultGraph::from_dir(root).unwrap();
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 2);

        // find_by_normalized_path should work with and without extensions
        assert!(graph.find_by_normalized_path("note-md").is_some());
        assert!(graph.find_by_normalized_path("note-md.md").is_some());
        assert!(graph.find_by_normalized_path("note-mkd").is_some());
        assert!(graph.find_by_normalized_path("note-mkd.markdown").is_some());

        // get_node_by_path
        assert!(graph.get_node_by_path("note-md").is_some());
        assert!(graph.get_node_by_path("note-mkd").is_some());
    }

    #[test]
    fn test_vault_graph_cycle_and_broken_target_detection() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: cyc-a
type: Decision
title: Cyc A
relations:
  - type: supersedes
    target: ./b.md
  - type: relates_to
    target: ./non-existent.md
---
A
"#;
        let note_b = r#"---
id: cyc-b
type: Decision
title: Cyc B
relations:
  - type: supersedes
    target: cyc-a
---
B
"#;
        std::fs::write(root.join("a.md"), note_a).unwrap();
        std::fs::write(root.join("b.md"), note_b).unwrap();

        let graph = VaultGraph::from_dir(root).unwrap();
        let cycles = graph.find_supersession_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["cyc-a", "cyc-b"]);

        let report = graph.lint();
        assert!(!report.healthy);
        // Expect 1 cycle and 1 broken target (non-existent.md)
        assert_eq!(report.errors, 2);
    }

    #[test]
    fn test_resolve_seed_by_id_and_paths() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let sub = root.join("decisions");
        std::fs::create_dir_all(&sub).unwrap();

        let note = r#"---
id: adr-test
type: Decision
title: Seed Test
---
Seed test body
"#;
        let file_path = sub.join("adr-test.md");
        std::fs::write(&file_path, note).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        assert!(vault.resolve_seed("adr-test").is_some());
        assert!(vault.resolve_seed("decisions/adr-test.md").is_some());
        assert!(vault.resolve_seed("decisions/adr-test").is_some());
        assert!(vault.resolve_seed("./decisions/adr-test.md").is_some());
        assert!(vault.resolve_seed(&file_path.to_string_lossy()).is_some());
        assert!(vault.resolve_seed("nonexistent").is_none());
    }
}
