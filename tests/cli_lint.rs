use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_lint_healthy_vault_human() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: concept-auth
type: Concept
title: Authentication Baseline
---
# Authentication
"#;
    let note2 = r#"---
id: decision-oauth2
type: Decision
title: Use OAuth2
relations:
  - type: depends_on
    target: concept-auth
---
# OAuth2
"#;
    fs::write(root.join("concept-auth.md"), note1).unwrap();
    fs::write(root.join("decision-oauth2.md"), note2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Checked 2 nodes and 1 edges."))
        .stdout(predicate::str::contains(
            "✓ Graph is healthy. No integrity violations found.",
        ));
}

#[test]
fn test_cli_lint_healthy_vault_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: concept-cache
type: Concept
title: Cache Architecture
---
# Cache
"#;
    fs::write(root.join("cache.md"), note1).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd.arg("lint").arg(root).arg("--json").assert().success();

    let stdout_str = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");
    assert_eq!(parsed["healthy"], true);
    assert_eq!(parsed["errors"], 0);
    assert_eq!(parsed["total_nodes"], 1);
    assert_eq!(parsed["findings"].as_array().unwrap().len(), 0);
}

#[test]
fn test_cli_lint_broken_target_human() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note = r#"---
id: corrupted-target
type: Decision
title: Broken Decision
relations:
  - type: supersedes
    target: ./non-existent-predecessor.md
---
# Broken Link
"#;
    fs::write(root.join("corrupted-target.md"), note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("Integrity Violations (1)"))
        .stdout(predicate::str::contains("broken_target"))
        .stdout(predicate::str::contains("Node 'corrupted-target' has relation 'supersedes' targeting './non-existent-predecessor.md'"))
        .stdout(predicate::str::contains("Lint failed with 1 error(s)"));
}

#[test]
fn test_cli_lint_broken_target_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note = r#"---
id: note-with-dangling-edge
type: Decision
title: Note With Dangling Edge
relations:
  - type: supported_by
    target: missing-review-id
---
# Missing
"#;
    fs::write(root.join("dangling.md"), note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("lint")
        .arg(root)
        .arg("--json")
        .assert()
        .failure()
        .code(predicate::eq(1));

    let stdout_str = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");
    assert_eq!(parsed["healthy"], false);
    assert_eq!(parsed["errors"], 1);
    assert_eq!(parsed["findings"][0]["kind"], "broken_target");
    assert_eq!(parsed["findings"][0]["source"], "note-with-dangling-edge");
    assert_eq!(parsed["findings"][0]["target"], "missing-review-id");
}

#[test]
fn test_cli_lint_2_node_cycle() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: cycle-a
type: Decision
title: Cycle Node A
relations:
  - type: supersedes
    target: cycle-b
---
Cycle A
"#;
    let note2 = r#"---
id: cycle-b
type: Decision
title: Cycle Node B
relations:
  - type: supersedes
    target: cycle-a
---
Cycle B
"#;
    fs::write(root.join("a.md"), note1).unwrap();
    fs::write(root.join("b.md"), note2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("supersession_cycle"))
        .stdout(predicate::str::contains("cycle-a -> cycle-b -> cycle-a"));
}

#[test]
fn test_cli_lint_3_node_cycle_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: cycle-canary-a
type: Decision
title: Cycle Canary A
relations:
  - type: supersedes
    target: cycle-canary-b
---
A
"#;
    let note2 = r#"---
id: cycle-canary-b
type: Decision
title: Cycle Canary B
relations:
  - type: supersedes
    target: cycle-canary-c
---
B
"#;
    let note3 = r#"---
id: cycle-canary-c
type: Decision
title: Cycle Canary C
relations:
  - type: supersedes
    target: cycle-canary-a
---
C
"#;
    fs::write(root.join("canary-a.md"), note1).unwrap();
    fs::write(root.join("canary-b.md"), note2).unwrap();
    fs::write(root.join("canary-c.md"), note3).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("lint")
        .arg(root)
        .arg("--json")
        .assert()
        .failure()
        .code(predicate::eq(1));

    let stdout_str = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");
    assert_eq!(parsed["healthy"], false);
    assert_eq!(parsed["errors"], 1);
    assert_eq!(parsed["findings"][0]["kind"], "supersession_cycle");
    let cycle = parsed["findings"][0]["cycle"].as_array().unwrap();
    assert_eq!(cycle.len(), 3);
    assert_eq!(cycle[0], "cycle-canary-a");
    assert_eq!(cycle[1], "cycle-canary-b");
    assert_eq!(cycle[2], "cycle-canary-c");
}

#[test]
fn test_cli_lint_active_contradiction() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note1 = r#"---
id: decision-events
type: Decision
title: Use Events
status: stable
scope:
  env: production
relations:
  - type: contradicts
    target: proposal-grpc
---
Events
"#;
    let note2 = r#"---
id: proposal-grpc
type: Proposal
title: Use gRPC
status: proposed
scope:
  env: production
---
gRPC
"#;
    fs::write(root.join("events.md"), note1).unwrap();
    fs::write(root.join("grpc.md"), note2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("conflicting_assertion"))
        .stdout(predicate::str::contains(
            "Active contradiction between 'decision-events' and 'proposal-grpc'",
        ));
}

#[test]
fn test_cli_lint_competing_successors() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let old_node = r#"---
id: old-payment-polling
type: Decision
title: Polling
---
Old
"#;
    let succ_events = r#"---
id: decision-payment-events
type: Decision
title: Events
relations:
  - type: supersedes
    target: old-payment-polling
---
Events
"#;
    let succ_competing = r#"---
id: competing-payment-successor
type: Decision
title: Competing Successor
relations:
  - type: supersedes
    target: old-payment-polling
---
Competing
"#;
    fs::write(root.join("old.md"), old_node).unwrap();
    fs::write(root.join("events.md"), succ_events).unwrap();
    fs::write(root.join("competing.md"), succ_competing).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("conflicting_assertion"))
        .stdout(predicate::str::contains("Competing successors"));
}

#[test]
fn test_cli_lint_malformed_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let bad_note = r#"---
id: bad-yaml
title: [unclosed
---
Bad
"#;
    fs::write(root.join("bad.md"), bad_note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(root)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("malformed_file"))
        .stdout(predicate::str::contains("Malformed file 'bad.md'"));
}

#[test]
fn test_cli_lint_path_not_found() {
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg("non_existent_vault_dir_xyz")
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("Vault directory not found"));
}

#[test]
fn test_cli_lint_path_not_dir() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("a_file.md");
    fs::write(&file_path, "not a dir").unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("lint")
        .arg(&file_path)
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("Path is not a directory"));
}
