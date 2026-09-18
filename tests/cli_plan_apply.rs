use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

use akashic::model::Plan;

#[test]
fn test_cli_plan_output_to_file_and_stdout() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Note 1: Missing ID
    let n1 = r#"---
type: Decision
title: Decision One
---
Content 1
"#;
    fs::write(root.join("decision-1.md"), n1).unwrap();

    // Note 2: Top-level supersedes
    let n2 = r#"---
id: decision-2
type: Decision
title: Decision Two
supersedes: ./decision-1.md
---
Content 2
"#;
    fs::write(root.join("decision-2.md"), n2).unwrap();

    // Note 3: Wikilinks in body
    let n3 = r#"---
id: concept-3
type: Concept
title: Concept Three
---
See [[decision-2]] and [[decision-2|Decision 2]].
"#;
    fs::write(root.join("concept-3.md"), n3).unwrap();

    let plan_path = root.join("plan.json");

    // Run akashic plan with --out
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Plan written to"))
        .stdout(predicate::str::contains("3 proposed actions"));

    assert!(plan_path.exists());
    let plan_content = fs::read_to_string(&plan_path).unwrap();
    let plan: Plan = serde_json::from_str(&plan_content).unwrap();
    assert_eq!(plan.plan_version, "0.1.0");
    assert_eq!(plan.actions.len(), 3);

    for action in &plan.actions {
        assert!(!action.path.is_empty());
        assert_eq!(action.source_sha256.len(), 64);
        assert_eq!(action.expected_new_sha256.len(), 64);
        assert_ne!(action.source_sha256, action.expected_new_sha256);
        assert!(action.diff.contains("@@"));
    }

    // Run akashic plan without --out to verify stdout JSON
    let mut cmd_stdout = Command::cargo_bin("akashic").unwrap();
    let assert_out = cmd_stdout.arg("plan").arg(root).assert().success();

    let stdout_str = String::from_utf8(assert_out.get_output().stdout.clone()).unwrap();
    let plan_from_stdout: Plan = serde_json::from_str(&stdout_str).unwrap();
    assert_eq!(plan_from_stdout.actions.len(), 3);
}

#[test]
fn test_cli_apply_dry_run_leaves_files_untouched() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note = r#"---
type: Decision
title: Legacy Note
supersedes: ./target.md
---
Content with [[target]].
"#;
    let note_path = root.join("legacy.md");
    fs::write(&note_path, note).unwrap();

    let plan_path = root.join("plan.json");

    // Plan
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan_path)
        .assert()
        .success();

    // Dry-run apply
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_path)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Validation successful"))
        .stdout(predicate::str::contains("[dry-run]"));

    // File should be completely untouched
    let note_after_dry = fs::read_to_string(&note_path).unwrap();
    assert_eq!(note_after_dry, note);
}

#[test]
fn test_cli_apply_and_strict_idempotency() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Note 1: Missing ID
    fs::write(
        root.join("missing-id.md"),
        "---\ntype: Concept\ntitle: No ID\n---\nHello",
    )
    .unwrap();

    // Note 2: Top-level supersedes and wikilink
    fs::write(
        root.join("superseder.md"),
        "---\nid: superseder\ntype: Decision\ntitle: Superseder\nsupersedes: ./missing-id.md\n---\nCheck [[missing-id]].",
    )
    .unwrap();

    let plan_path = root.join("plan.json");

    // Generate plan
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan_path)
        .assert()
        .success();

    // Apply plan
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied 2 changes"));

    // Verify mutations
    let n1 = fs::read_to_string(root.join("missing-id.md")).unwrap();
    assert!(n1.contains("id: missing-id"));

    let n2 = fs::read_to_string(root.join("superseder.md")).unwrap();
    assert!(!n2.contains("supersedes: ./missing-id.md"));
    assert!(n2.contains("relations:"));
    assert!(n2.contains("- type: supersedes"));
    assert!(n2.contains("target: ./missing-id.md"));
    assert!(n2.contains("[missing-id](./missing-id.md)"));

    // Re-apply the EXACT same plan: must produce 0 modifications and 0 file churn
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Applied 0 changes (2 files already up-to-date)",
        ));

    // Re-plan: must find 0 proposed actions
    let plan2_path = root.join("plan2.json");
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan2_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 proposed actions"));

    let plan2_content = fs::read_to_string(&plan2_path).unwrap();
    let plan2: Plan = serde_json::from_str(&plan2_content).unwrap();
    assert_eq!(plan2.actions.len(), 0);
}

#[test]
fn test_cli_apply_hash_mismatch_aborts_cleanly_without_partial_mutations() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let n1_orig = "---\ntype: Concept\ntitle: Note One\n---\nFirst note.";
    let n2_orig = "---\ntype: Concept\ntitle: Note Two\n---\nSecond note.";

    fs::write(root.join("note1.md"), n1_orig).unwrap();
    fs::write(root.join("note2.md"), n2_orig).unwrap();

    let plan_path = root.join("plan.json");

    // Generate plan for both notes
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan_path)
        .assert()
        .success();

    // Tamper note2 on disk
    fs::write(root.join("note2.md"), "tampered content after plan").unwrap();

    // Apply must abort with error
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Hash mismatch for file 'note2.md'",
        ))
        .stderr(predicate::str::contains(
            "Aborting cleanly without modifications",
        ));

    // CRITICAL: note1 must NOT have been partially modified
    let n1_current = fs::read_to_string(root.join("note1.md")).unwrap();
    assert_eq!(n1_current, n1_orig);
}

#[test]
fn test_cli_plan_nonexistent_vault() {
    Command::cargo_bin("akashic")
        .unwrap()
        .arg("plan")
        .arg("/nonexistent/directory/path/that/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Vault directory not found"));
}

#[test]
fn test_cli_apply_nonexistent_plan() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    Command::cargo_bin("akashic")
        .unwrap()
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg("/nonexistent/plan.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Plan file not found"));
}
