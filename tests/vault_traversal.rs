use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

use akashic::graph::VaultGraph;
use akashic::model::RelationType;

#[test]
fn test_vault_traversal_recursive_and_hidden_dirs() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create nested directory structure
    let arch_decisions = root.join("architecture").join("decisions");
    fs::create_dir_all(&arch_decisions).unwrap();
    let arch_reviews = root.join("architecture").join("reviews");
    fs::create_dir_all(&arch_reviews).unwrap();
    let hidden_git = root.join(".git").join("hooks");
    fs::create_dir_all(&hidden_git).unwrap();
    let hidden_obsidian = root.join(".obsidian");
    fs::create_dir_all(&hidden_obsidian).unwrap();

    // 1. architecture/decisions/adr-001.md
    let adr1_content = r#"---
id: adr-001
type: Decision
title: Use Microservices
relations:
  - type: supersedes
    target: ../decisions/adr-002.md
  - type: supported_by
    target: rev-001
---
Decision 1 body
"#;
    fs::write(arch_decisions.join("adr-001.md"), adr1_content).unwrap();

    // 2. architecture/decisions/adr-002.md
    let adr2_content = r#"---
id: adr-002
type: Decision
title: Monolith
relations:
  - type: depends_on
    target: ./adr-003.md
---
Decision 2 body
"#;
    fs::write(arch_decisions.join("adr-002.md"), adr2_content).unwrap();

    // 3. architecture/decisions/adr-003.md
    let adr3_content = r#"---
id: adr-003
type: Decision
title: Database Selection
relations:
  - type: caused_by
    target: ../reviews/rev-001.md
---
Decision 3 body
"#;
    fs::write(arch_decisions.join("adr-003.md"), adr3_content).unwrap();

    // 4. architecture/reviews/rev-001.md
    let rev1_content = r#"---
id: rev-001
type: Review
title: Microservices Performance Review
---
Review 1 body
"#;
    fs::write(arch_reviews.join("rev-001.md"), rev1_content).unwrap();

    // 5. Hidden files that must be ignored
    let git_note = r#"---
id: git-internal
type: Secret
title: Should Be Ignored
---
Ignored
"#;
    fs::write(hidden_git.join("hook.md"), git_note).unwrap();
    fs::write(hidden_obsidian.join("config.md"), git_note).unwrap();
    fs::write(root.join(".hidden_file.md"), git_note).unwrap();

    // Build VaultGraph
    let graph = VaultGraph::from_dir(root).expect("VaultGraph build should succeed");

    // Must discover exactly 4 notes (ignoring hidden files/dirs)
    assert_eq!(graph.graph.node_count(), 4);
    assert!(graph.get_node("adr-001").is_some());
    assert!(graph.get_node("adr-002").is_some());
    assert!(graph.get_node("adr-003").is_some());
    assert!(graph.get_node("rev-001").is_some());
    assert!(graph.get_node("git-internal").is_none());

    // Verify edge count
    // adr-001 -> adr-002 (supersedes via ../decisions/adr-002.md)
    // adr-001 -> rev-001 (supported_by via ID rev-001)
    // adr-002 -> adr-003 (depends_on via ./adr-003.md)
    // adr-003 -> rev-001 (caused_by via ../reviews/rev-001.md)
    assert_eq!(graph.graph.edge_count(), 4);

    // Dynamic inverse relationships
    let adr2_inverses = graph
        .inverse_relations("adr-002")
        .expect("Node should exist");
    assert_eq!(adr2_inverses.len(), 1);
    assert_eq!(adr2_inverses[0].relation_type, RelationType::SupersededBy);
    assert_eq!(adr2_inverses[0].target, "adr-001");

    let rev1_inverses = graph
        .inverse_relations("rev-001")
        .expect("Node should exist");
    assert_eq!(rev1_inverses.len(), 2);
    // rev-001 is target of adr-003 (caused_by -> caused) and adr-001 (supported_by -> supports)
    let inv_types: Vec<_> = rev1_inverses.iter().map(|r| &r.relation_type).collect();
    assert!(inv_types.contains(&&RelationType::Supports));
    assert!(inv_types.contains(&&RelationType::Caused));

    let adr3_inverses = graph
        .inverse_relations("adr-003")
        .expect("Node should exist");
    assert_eq!(adr3_inverses.len(), 1);
    assert_eq!(adr3_inverses[0].relation_type, RelationType::RequiredBy);
    assert_eq!(adr3_inverses[0].target, "adr-002");

    // Verify source files on disk were untouched
    assert_eq!(
        fs::read_to_string(arch_decisions.join("adr-001.md")).unwrap(),
        adr1_content
    );
    assert_eq!(
        fs::read_to_string(arch_decisions.join("adr-002.md")).unwrap(),
        adr2_content
    );
    assert_eq!(
        fs::read_to_string(arch_decisions.join("adr-003.md")).unwrap(),
        adr3_content
    );
    assert_eq!(
        fs::read_to_string(arch_reviews.join("rev-001.md")).unwrap(),
        rev1_content
    );
}

#[test]
fn test_vault_traversal_target_path_normalization() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let sub_a = root.join("sub_a");
    let sub_b = root.join("sub_b");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();

    // Target in root
    let root_note = r#"---
id: root-note
type: Concept
title: Root Note
---
Root
"#;
    fs::write(root.join("root.md"), root_note).unwrap();

    // Note in sub_b
    let b_note = r#"---
id: b-note
type: Concept
title: B Note
---
B
"#;
    fs::write(sub_b.join("target_b.md"), b_note).unwrap();

    // Note in sub_a targeting notes via various path notations
    let a_note = r#"---
id: a-note
type: Decision
title: A Note
relations:
  - type: relates_to
    target: ../root.md
  - type: contradicts
    target: ../sub_b/target_b.md
  - type: depends_on
    target: sub_b/target_b.md
---
A
"#;
    fs::write(sub_a.join("source_a.md"), a_note).unwrap();

    let graph = VaultGraph::from_dir(root).unwrap();
    assert_eq!(graph.graph.node_count(), 3);
    // All 3 relations should resolve
    assert_eq!(graph.graph.edge_count(), 3);
}

#[test]
fn test_cli_inspect_vault_human_and_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: note-1
type: Decision
title: First Note
relations:
  - type: supersedes
    target: ./note-2.md
---
Body 1
"#;
    let note2 = r#"---
id: note-2
type: Decision
title: Second Note
relations:
  - type: depends_on
    target: note-1
---
Body 2
"#;
    fs::write(root.join("note-1.md"), note1).unwrap();
    fs::write(root.join("note-2.md"), note2).unwrap();

    // Human output inspection
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Nodes: 2"))
        .stdout(predicate::str::contains("Total Edges: 2"))
        .stdout(predicate::str::contains("Edges by Type:"))
        .stdout(predicate::str::contains("supersedes: 1"))
        .stdout(predicate::str::contains("depends_on: 1"));

    // JSON output inspection
    let mut cmd_json = Command::cargo_bin("akashic").unwrap();
    let assert = cmd_json
        .arg("inspect")
        .arg(root)
        .arg("--json")
        .assert()
        .success();

    let stdout_str = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");

    assert_eq!(parsed["total_nodes"], 2);
    assert_eq!(parsed["total_edges"], 2);
    assert_eq!(parsed["edges_by_type"]["supersedes"], 1);
    assert_eq!(parsed["edges_by_type"]["depends_on"], 1);
    assert_eq!(parsed["malformed_files"].as_array().unwrap().len(), 0);
}

#[test]
fn test_cli_inspect_vault_with_malformed_notes() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let valid_note = r#"---
id: valid-1
type: Concept
title: Valid Note
---
Content
"#;
    fs::write(root.join("valid.md"), valid_note).unwrap();

    let malformed_yaml = r#"---
id: [unclosed
title: broken
---
Content
"#;
    fs::write(root.join("broken.md"), malformed_yaml).unwrap();

    let missing_fm = r#"# Just regular markdown
Without frontmatter.
"#;
    fs::write(root.join("no_frontmatter.md"), missing_fm).unwrap();

    // Human output: should succeed, report 1 node and 2 malformed files without crashing
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Nodes: 1"))
        .stdout(predicate::str::contains("Total Edges: 0"))
        .stdout(predicate::str::contains("Malformed Files (2):"))
        .stdout(predicate::str::contains("broken.md"))
        .stdout(predicate::str::contains("no_frontmatter.md"));

    // JSON output: should include malformed file details
    let mut cmd_json = Command::cargo_bin("akashic").unwrap();
    let assert = cmd_json
        .arg("inspect")
        .arg(root)
        .arg("--json")
        .assert()
        .success();

    let stdout_str = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");

    assert_eq!(parsed["total_nodes"], 1);
    assert_eq!(parsed["total_edges"], 0);
    let malformed = parsed["malformed_files"].as_array().unwrap();
    assert_eq!(malformed.len(), 2);
    let paths: Vec<&str> = malformed
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"broken.md"));
    assert!(paths.contains(&"no_frontmatter.md"));
}
