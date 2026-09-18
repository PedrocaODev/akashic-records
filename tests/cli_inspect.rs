use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

const VALID_NOTE: &str = r#"---
id: decision-payment-status-events
type: Decision
title: Use event-driven payment status
status: stable
valid_from: "2026-06-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./review-incident-timeout.md
  - type: supported_by
    target: ./review-incident-timeout.md
---
# Architecture Decision

We decide to replace polling with event-driven notifications.
"#;

#[test]
fn test_inspect_valid_note_human() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(VALID_NOTE.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Node: decision-payment-status-events",
        ))
        .stdout(predicate::str::contains("Type: Decision"))
        .stdout(predicate::str::contains(
            "Title: Use event-driven payment status",
        ))
        .stdout(predicate::str::contains("Status: stable"))
        .stdout(predicate::str::contains("Valid From: 2026-06-01"))
        .stdout(predicate::str::contains("system: payments"))
        .stdout(predicate::str::contains("env: production"))
        .stdout(predicate::str::contains(
            "supersedes -> ./decision-payment-status-polling.md",
        ))
        .stdout(predicate::str::contains(
            "evidence: ./review-incident-timeout.md",
        ))
        .stdout(predicate::str::contains(
            "supported_by -> ./review-incident-timeout.md",
        ));
}

#[test]
fn test_inspect_valid_note_json() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(VALID_NOTE.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("inspect")
        .arg(file.path())
        .arg("--json")
        .assert()
        .success();

    let output = assert.get_output();
    let stdout_str = String::from_utf8(output.stdout.clone()).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&stdout_str).expect("Valid JSON");
    assert_eq!(parsed["id"], "decision-payment-status-events");
    assert_eq!(parsed["type"], "Decision");
    assert_eq!(parsed["title"], "Use event-driven payment status");
    assert_eq!(parsed["status"], "stable");
    assert_eq!(parsed["valid_from"], "2026-06-01");
    assert_eq!(parsed["scope"]["system"], "payments");
    assert_eq!(parsed["scope"]["env"], "production");
    assert_eq!(parsed["relations"][0]["type"], "supersedes");
    assert_eq!(
        parsed["relations"][0]["target"],
        "./decision-payment-status-polling.md"
    );
    assert_eq!(
        parsed["relations"][0]["evidence"],
        "./review-incident-timeout.md"
    );
    assert_eq!(parsed["relations"][1]["type"], "supported_by");
    assert!(parsed["body"]
        .as_str()
        .unwrap()
        .contains("# Architecture Decision"));
}

#[test]
fn test_inspect_file_not_found() {
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg("non_existent_file_path_12345.md")
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("Error:"));
}

#[test]
fn test_inspect_invalid_yaml() {
    let bad_yaml = r#"---
id: [unclosed list
title: broken
---
# Content
"#;
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(bad_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(file.path())
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("Invalid YAML frontmatter"));
}

#[test]
fn test_inspect_missing_frontmatter() {
    let no_fm = r#"# Regular Markdown

Just some text without any YAML frontmatter.
"#;
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(no_fm.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(file.path())
        .assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains(
            "Missing or malformed YAML frontmatter delimiter",
        ));
}

#[test]
fn test_inspect_standard_gfm() {
    let gfm_doc = r#"---
id: concept-gfm-check
type: Concept
title: GFM Compatibility Test
---
# Tables and Lists

| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |

- [x] Task list item
- [ ] Incomplete item

~~Strikethrough~~ and `inline code`.

```rust
fn main() {
    println!("Hello, World!");
}
```
"#;
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(gfm_doc.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("inspect")
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Node: concept-gfm-check"))
        .stdout(predicate::str::contains("Type: Concept"));
}
