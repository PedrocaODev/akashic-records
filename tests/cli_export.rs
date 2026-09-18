use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_export_okf_basic() {
    let vault_dir = tempdir().unwrap();
    let export_dir = tempdir().unwrap();
    let root = vault_dir.path();

    let decisions = root.join("decisions");
    let reviews = root.join("reviews");
    fs::create_dir_all(&decisions).unwrap();
    fs::create_dir_all(&reviews).unwrap();

    let note1 = r#"---
id: decision-events
type: Decision
title: Use Event-Driven
status: stable
valid_from: "2026-06-01"
valid_until: "2027-01-01"
scope:
  env: production
  system: payments
relations:
  - type: supersedes
    target: ./decision-polling.md
    evidence: rev-timeout
  - type: supported_by
    target: rev-timeout
---
# Events Decision
See [[rev-timeout|Timeout Review]] and [Polling](./decision-polling.md).
"#;
    fs::write(decisions.join("decision-events.md"), note1).unwrap();

    let note2 = r#"---
id: decision-polling
type: Decision
title: Use Polling
status: superseded
---
# Polling Decision
Older approach.
"#;
    fs::write(decisions.join("decision-polling.md"), note2).unwrap();

    let note3 = r#"---
id: rev-timeout
type: Review
title: Timeout Incident Review
status: stable
---
# Timeout Review
Empirical review.
"#;
    fs::write(reviews.join("rev-timeout.md"), note3).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg(root)
        .arg("--format")
        .arg("okf")
        .arg("--out")
        .arg(export_dir.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Exported 3 node(s)"))
        .stdout(predicate::str::contains("Manifest written to"));

    let manifest_file = export_dir.path().join("manifest.json");
    assert!(manifest_file.exists());
    let manifest_str = fs::read_to_string(&manifest_file).unwrap();
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();
    assert_eq!(manifest_json["format"], "okf-v0.2");
    assert_eq!(manifest_json["total_nodes"], 3);
    assert_eq!(manifest_json["total_relations"], 2);

    let exp_events = export_dir.path().join("decisions/decision-events.md");
    assert!(exp_events.exists());
    let content = fs::read_to_string(&exp_events).unwrap();
    assert!(content.contains("id: decision-events"));
    assert!(content.contains("type: Decision"));
    assert!(content.contains("status: stable"));
    assert!(content.contains("env: production"));
    assert!(content.contains("type: supersedes"));
    assert!(content.contains("[Timeout Review](../reviews/rev-timeout.md)"));
    assert!(content.contains("[Polling](./decision-polling.md)"));
}

#[test]
fn test_cli_export_okf_json() {
    let vault_dir = tempdir().unwrap();
    let export_dir = tempdir().unwrap();
    let root = vault_dir.path();

    let note = r#"---
id: my-note
type: Note
title: Note Title
---
Content
"#;
    fs::write(root.join("note.md"), note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg(root)
        .arg("--format")
        .arg("okf")
        .arg("--out")
        .arg(export_dir.path())
        .arg("--json");

    let assert_out = cmd.assert().success();
    let stdout = String::from_utf8(assert_out.get_output().stdout.clone()).unwrap();
    let val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(val["format"], "okf-v0.2");
    assert_eq!(val["total_nodes"], 1);
}

#[test]
fn test_cli_export_default_format() {
    let vault_dir = tempdir().unwrap();
    let export_dir = tempdir().unwrap();
    let root = vault_dir.path();

    let note = r#"---
id: default-note
type: Note
title: Default Format Test
---
Testing default format.
"#;
    fs::write(root.join("note.md"), note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg(root)
        .arg("--out")
        .arg(export_dir.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Exported 1 node(s)"))
        .stdout(predicate::str::contains("format: okf-v0.2"));
}

#[test]
fn test_cli_export_unsupported_format() {
    let vault_dir = tempdir().unwrap();
    let export_dir = tempdir().unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg(vault_dir.path())
        .arg("--format")
        .arg("xml")
        .arg("--out")
        .arg(export_dir.path());

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported export format 'xml'"));
}

#[test]
fn test_cli_export_nonexistent_vault() {
    let export_dir = tempdir().unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg("/path/definitely/does/not/exist")
        .arg("--format")
        .arg("okf")
        .arg("--out")
        .arg(export_dir.path());

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Vault directory not found"));
}

#[test]
fn test_cli_export_roundtrip_inspection() {
    let vault_dir = tempdir().unwrap();
    let export_dir = tempdir().unwrap();
    let root = vault_dir.path();

    let decisions = root.join("decisions");
    fs::create_dir_all(&decisions).unwrap();

    let note1 = r#"---
id: adr-1
type: Decision
title: ADR 1
relations:
  - type: supersedes
    target: ./adr-2.md
---
# ADR 1
"#;
    fs::write(decisions.join("adr-1.md"), note1).unwrap();

    let note2 = r#"---
id: adr-2
type: Decision
title: ADR 2
---
# ADR 2
"#;
    fs::write(decisions.join("adr-2.md"), note2).unwrap();

    // 1. Export vault
    let mut export_cmd = Command::cargo_bin("akashic").unwrap();
    export_cmd
        .arg("export")
        .arg(root)
        .arg("--format")
        .arg("okf")
        .arg("--out")
        .arg(export_dir.path());
    export_cmd.assert().success();

    // 2. Inspect original vault
    let mut inspect_orig = Command::cargo_bin("akashic").unwrap();
    inspect_orig.arg("inspect").arg(root).arg("--json");
    let orig_out = inspect_orig.assert().success();
    let orig_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(orig_out.get_output().stdout.clone()).unwrap())
            .unwrap();

    // 3. Inspect exported vault
    let mut inspect_exp = Command::cargo_bin("akashic").unwrap();
    inspect_exp
        .arg("inspect")
        .arg(export_dir.path())
        .arg("--json");
    let exp_out = inspect_exp.assert().success();
    let exp_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(exp_out.get_output().stdout.clone()).unwrap())
            .unwrap();

    assert_eq!(orig_json["total_nodes"], exp_json["total_nodes"]);
    assert_eq!(orig_json["total_edges"], exp_json["total_edges"]);
    assert_eq!(orig_json["edges_by_type"], exp_json["edges_by_type"]);
}
