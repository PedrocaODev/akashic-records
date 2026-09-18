use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_query_authoritative_current_guidance() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note = r#"---
id: decision-cloud-native
type: Decision
title: Cloud Native Architecture
status: stable
valid_from: "2026-01-01"
scope:
  env: production
---
Use Kubernetes and managed Postgres.
"#;
    fs::write(root.join("decision-cloud-native.md"), note).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("query")
        .arg(root)
        .arg("--seed")
        .arg("decision-cloud-native")
        .arg("--mode")
        .arg("current")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Agent Context Packet",
        ))
        .stdout(predicate::str::contains(
            "- **Seed**: `decision-cloud-native`",
        ))
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-cloud-native`",
        ))
        .stdout(predicate::str::contains("- **Status**: CURRENT GUIDANCE"))
        .stdout(predicate::str::contains(
            "Note 'decision-cloud-native' has no successors and is itself current guidance.",
        ))
        .stdout(predicate::str::contains("## Active Guidance"))
        .stdout(predicate::str::contains(
            "Cloud Native Architecture (`decision-cloud-native`)",
        ))
        .stdout(predicate::str::contains(
            "Use Kubernetes and managed Postgres.",
        ))
        .stdout(predicate::str::contains("## Historical Rationale").not());
}

#[test]
fn test_cli_query_superseded_predecessor_to_successor() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let decisions = root.join("decisions");
    fs::create_dir_all(&decisions).unwrap();

    let old_note = r#"---
id: decision-payment-status-polling
type: Decision
title: Use HTTP polling for payments
status: deprecated
valid_from: "2025-01-01"
---
Poll payments endpoint every 2 seconds.
"#;

    let new_note = r#"---
id: decision-payment-status-events
type: Decision
title: Use event-driven payment status
status: stable
valid_from: "2026-06-01"
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./reviews/incident-timeout.md
---
Publish PaymentCompleted domain events to Kafka.
"#;

    fs::write(
        decisions.join("decision-payment-status-polling.md"),
        old_note,
    )
    .unwrap();
    fs::write(
        decisions.join("decision-payment-status-events.md"),
        new_note,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("query")
        .arg(root)
        .arg("--seed")
        .arg("decision-payment-status-polling")
        .arg("--mode")
        .arg("current")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Akashic Records — Agent Context Packet"))
        .stdout(predicate::str::contains("- **Seed**: `decision-payment-status-polling`"))
        .stdout(predicate::str::contains("- **Active Guidance**: `decision-payment-status-events`"))
        .stdout(predicate::str::contains("- **Status**: SUPERSEDED (Resolved to successor)"))
        .stdout(predicate::str::contains(
            "Resolved superseded note 'decision-payment-status-polling' along 1 supersession hop(s) to active guidance 'decision-payment-status-events'.",
        ))
        .stdout(predicate::str::contains("## Active Guidance"))
        .stdout(predicate::str::contains("Use event-driven payment status (`decision-payment-status-events`)"))
        .stdout(predicate::str::contains("Publish PaymentCompleted domain events to Kafka."))
        .stdout(predicate::str::contains("## Historical Rationale (SUPERSEDED)"))
        .stdout(predicate::str::contains("> [!WARNING]"))
        .stdout(predicate::str::contains("Use HTTP polling for payments (`decision-payment-status-polling`) [SUPERSEDED]"))
        .stdout(predicate::str::contains("Poll payments endpoint every 2 seconds."));
}

#[test]
fn test_cli_query_with_scope_filter() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let base_note = r#"---
id: adr-base
type: Decision
title: Base Architecture
---
Base implementation
"#;

    let prod_note = r#"---
id: adr-prod
type: Decision
title: Production Architecture
scope:
  env: production
relations:
  - type: supersedes
    target: ./adr-base.md
---
Production implementation
"#;

    fs::write(root.join("adr-base.md"), base_note).unwrap();
    fs::write(root.join("adr-prod.md"), prod_note).unwrap();

    // When querying with --scope env=production, adr-base resolves to adr-prod
    let mut cmd_prod = Command::cargo_bin("akashic").unwrap();
    cmd_prod
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("adr-base")
        .arg("--mode")
        .arg("current")
        .arg("--scope")
        .arg("env=production")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `adr-prod`",
        ))
        .stdout(predicate::str::contains(
            "## Historical Rationale (SUPERSEDED)",
        ));

    // When querying with --scope env=staging, adr-prod does not apply; adr-base remains active with advisory
    let mut cmd_staging = Command::cargo_bin("akashic").unwrap();
    cmd_staging
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("adr-base")
        .arg("--mode")
        .arg("current")
        .arg("--scope")
        .arg("env=staging")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `adr-base`",
        ))
        .stdout(predicate::str::contains("Advisories:"))
        .stdout(predicate::str::contains(
            "A successor exists in another scope: 'adr-prod' (scope: [env=production])",
        ))
        .stdout(predicate::str::contains("Base implementation"));
}

#[test]
fn test_cli_query_seed_as_relative_path() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("architecture").join("decisions");
    fs::create_dir_all(&sub).unwrap();

    let note_a = r#"---
id: adr-100
type: Decision
title: Legacy System
---
Legacy details
"#;
    let note_b = r#"---
id: adr-200
type: Decision
title: Modern System
relations:
  - type: supersedes
    target: ./adr-100.md
---
Modern details
"#;
    fs::write(sub.join("adr-100.md"), note_a).unwrap();
    fs::write(sub.join("adr-200.md"), note_b).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("query")
        .arg(root)
        .arg("--seed")
        .arg("architecture/decisions/adr-100.md")
        .arg("--mode")
        .arg("current")
        .assert()
        .success()
        .stdout(predicate::str::contains("- **Active Guidance**: `adr-200`"));
}

#[test]
fn test_cli_query_json_format() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note_a = r#"---
id: dec-old
type: Decision
title: Old
---
Old Body
"#;
    let note_b = r#"---
id: dec-new
type: Decision
title: New
relations:
  - type: supersedes
    target: ./dec-old.md
---
New Body
"#;
    fs::write(root.join("dec-old.md"), note_a).unwrap();
    fs::write(root.join("dec-new.md"), note_b).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-old")
        .arg("--json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON");

    assert_eq!(parsed["mode"], "current");
    assert_eq!(parsed["seed"], "dec-old");
    assert_eq!(parsed["active_guidance_id"], "dec-new");
    assert_eq!(parsed["is_current"], false);
    assert_eq!(parsed["status"], "superseded");
    assert_eq!(parsed["active_note"]["id"], "dec-new");
    assert_eq!(parsed["historical_rationale"][0]["node"]["id"], "dec-old");
    assert_eq!(parsed["supersession_chain"][0]["predecessor"], "dec-old");
    assert_eq!(parsed["supersession_chain"][0]["successor"], "dec-new");
}

#[test]
fn test_cli_query_errors() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note = r#"---
id: note-single
type: Decision
title: Single Note
---
Content
"#;
    fs::write(root.join("note-single.md"), note).unwrap();

    // Seed not found error
    let mut cmd_missing = Command::cargo_bin("akashic").unwrap();
    cmd_missing
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("does-not-exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Seed note 'does-not-exist' not found",
        ));

    // Invalid scope format error
    let mut cmd_bad_scope = Command::cargo_bin("akashic").unwrap();
    cmd_bad_scope
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("note-single")
        .arg("--scope")
        .arg("malformed_scope_without_equals")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid scope format"));

    // Unsupported query mode
    let mut cmd_bad_mode = Command::cargo_bin("akashic").unwrap();
    cmd_bad_mode
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("note-single")
        .arg("--mode")
        .arg("unknown_mode")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Unsupported query mode 'unknown_mode'. Currently supported modes: current, lineage, impact, historical",
        ));

    // Historical query missing --as-of
    let mut cmd_missing_as_of = Command::cargo_bin("akashic").unwrap();
    cmd_missing_as_of
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("note-single")
        .arg("--mode")
        .arg("historical")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Missing required '--as-of <YYYY-MM-DD>' parameter for historical query mode",
        ));

    // Historical query with invalid date format
    let mut cmd_invalid_date = Command::cargo_bin("akashic").unwrap();
    cmd_invalid_date
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("note-single")
        .arg("--mode")
        .arg("historical")
        .arg("--as-of")
        .arg("2026-13-99")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Invalid date format '2026-13-99'. Expected YYYY-MM-DD",
        ));
}

#[test]
fn test_cli_query_mode_lineage() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note_a = r#"---
id: dec-a
type: Decision
title: SQLite Storage
status: deprecated
valid_from: "2024-01-01"
---
Initial embedded storage.
"#;
    let note_b = r#"---
id: dec-b
type: Decision
title: PostgreSQL Storage
status: deprecated
valid_from: "2025-01-01"
relations:
  - type: supersedes
    target: ./dec-a.md
    evidence: ./reviews/concurrency.md
---
Migrated to PostgreSQL for multi-user concurrency.
"#;
    let note_c = r#"---
id: dec-c
type: Decision
title: CockroachDB Storage
status: stable
valid_from: "2026-01-01"
relations:
  - type: supersedes
    target: ./dec-b.md
    evidence: ./reviews/geo-replication.md
---
Migrated to CockroachDB for multi-region active-active.
"#;
    fs::write(root.join("dec-a.md"), note_a).unwrap();
    fs::write(root.join("dec-b.md"), note_b).unwrap();
    fs::write(root.join("dec-c.md"), note_c).unwrap();

    // 1. Markdown output
    let mut cmd_md = Command::cargo_bin("akashic").unwrap();
    cmd_md
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-a")
        .arg("--mode")
        .arg("lineage")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Decision Lineage Packet",
        ))
        .stdout(predicate::str::contains(
            "## Chronological Replacement Chain",
        ))
        .stdout(predicate::str::contains("`dec-a` -> `dec-b` -> `dec-c`"))
        .stdout(predicate::str::contains(
            "**Supersession Evidence**: ./reviews/concurrency.md",
        ))
        .stdout(predicate::str::contains(
            "**Supersession Evidence**: ./reviews/geo-replication.md",
        ))
        .stdout(predicate::str::contains(
            "SQLite Storage (`dec-a`) [SUPERSEDED]",
        ))
        .stdout(predicate::str::contains(
            "PostgreSQL Storage (`dec-b`) [SUPERSEDED]",
        ))
        .stdout(predicate::str::contains(
            "CockroachDB Storage (`dec-c`) [CURRENT GUIDANCE]",
        ));

    // 2. JSON output
    let mut cmd_json = Command::cargo_bin("akashic").unwrap();
    let assert_json = cmd_json
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-b")
        .arg("--mode")
        .arg("lineage")
        .arg("--json")
        .assert()
        .success();

    let json_output = String::from_utf8(assert_json.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(parsed["mode"], "lineage");
    assert_eq!(parsed["seed"], "dec-b");
    assert_eq!(parsed["active_guidance_id"], "dec-c");
    assert_eq!(parsed["chain"].as_array().unwrap().len(), 3);
    assert_eq!(parsed["chain"][0]["node"]["id"], "dec-a");
    assert_eq!(parsed["chain"][1]["node"]["id"], "dec-b");
    assert_eq!(parsed["chain"][2]["node"]["id"], "dec-c");
    assert_eq!(parsed["transitions"].as_array().unwrap().len(), 2);
    assert_eq!(
        parsed["transitions"][0]["evidence"],
        "./reviews/concurrency.md"
    );
    assert_eq!(
        parsed["transitions"][1]["evidence"],
        "./reviews/geo-replication.md"
    );
}

#[test]
fn test_cli_query_mode_impact() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let db_note = r#"---
id: db-postgres
type: Infrastructure
title: PostgreSQL Database Cluster
---
Primary relational DB.
"#;
    let auth_note = r#"---
id: svc-auth
type: Service
title: User Authentication Service
relations:
  - type: depends_on
    target: ./db-postgres.md
---
Handles OAuth and session management.
"#;
    let ui_note = r#"---
id: app-frontend
type: Application
title: Customer Web Frontend
relations:
  - type: depends_on
    target: ./svc-auth.md
---
Next.js client interface.
"#;
    fs::write(root.join("db-postgres.md"), db_note).unwrap();
    fs::write(root.join("svc-auth.md"), auth_note).unwrap();
    fs::write(root.join("app-frontend.md"), ui_note).unwrap();

    // 1. Markdown output
    let mut cmd_md = Command::cargo_bin("akashic").unwrap();
    cmd_md
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("db-postgres")
        .arg("--mode")
        .arg("impact")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Impact Analysis Packet",
        ))
        .stdout(predicate::str::contains("- **Total Dependents**: 2"))
        .stdout(predicate::str::contains("- **Max Depth**: 2"))
        .stdout(predicate::str::contains(
            "User Authentication Service (`svc-auth`)",
        ))
        .stdout(predicate::str::contains(
            "Customer Web Frontend (`app-frontend`)",
        ))
        .stdout(predicate::str::contains("`db-postgres` -> `svc-auth`"))
        .stdout(predicate::str::contains(
            "`db-postgres` -> `svc-auth` -> `app-frontend`",
        ));

    // 2. JSON output
    let mut cmd_json = Command::cargo_bin("akashic").unwrap();
    let assert_json = cmd_json
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("db-postgres")
        .arg("--mode")
        .arg("impact")
        .arg("--json")
        .assert()
        .success();

    let json_output = String::from_utf8(assert_json.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(parsed["mode"], "impact");
    assert_eq!(parsed["seed"], "db-postgres");
    assert_eq!(parsed["total_dependents"], 2);
    assert_eq!(parsed["max_depth"], 2);
    let deps = parsed["dependents"].as_array().unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0]["id"], "svc-auth");
    assert_eq!(deps[0]["depth"], 1);
    assert_eq!(deps[1]["id"], "app-frontend");
    assert_eq!(deps[1]["depth"], 2);
}

#[test]
fn test_cli_query_mode_historical() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note_2024 = r#"---
id: dec-cache-2024
type: Decision
title: Redis Single Node
status: deprecated
valid_from: "2024-01-01"
valid_until: "2024-12-31"
---
Use standalone Redis instance.
"#;
    let note_2025 = r#"---
id: dec-cache-2025
type: Decision
title: Redis Cluster
status: stable
valid_from: "2025-01-01"
relations:
  - type: supersedes
    target: ./dec-cache-2024.md
---
Use Redis Cluster with sharding.
"#;
    let note_undated = r#"---
id: dec-undated
type: Decision
title: Undated Architecture Note
---
Undated content.
"#;
    fs::write(root.join("dec-cache-2024.md"), note_2024).unwrap();
    fs::write(root.join("dec-cache-2025.md"), note_2025).unwrap();
    fs::write(root.join("dec-undated.md"), note_undated).unwrap();

    // 1. Historical query evaluating 2024 date
    let mut cmd_2024 = Command::cargo_bin("akashic").unwrap();
    cmd_2024
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-cache-2024")
        .arg("--mode")
        .arg("historical")
        .arg("--as-of")
        .arg("2024-08-15")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Historical Context Packet",
        ))
        .stdout(predicate::str::contains("- **As Of**: `2024-08-15`"))
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `dec-cache-2024`",
        ))
        .stdout(predicate::str::contains(
            "Redis Single Node (`dec-cache-2024`)",
        ));

    // 2. Historical query evaluating 2025 date with --json
    let mut cmd_2025 = Command::cargo_bin("akashic").unwrap();
    let assert_2025 = cmd_2025
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-cache-2024")
        .arg("--mode")
        .arg("historical")
        .arg("--as-of")
        .arg("2025-06-01")
        .arg("--json")
        .assert()
        .success();

    let json_output = String::from_utf8(assert_2025.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(parsed["mode"], "historical");
    assert_eq!(parsed["as_of"], "2025-06-01");
    assert_eq!(parsed["is_active"], true);
    assert_eq!(parsed["active_guidance_id"], "dec-cache-2025");
    assert_eq!(parsed["active_note"]["id"], "dec-cache-2025");

    // 3. Historical query on undated note triggers warning indicator
    let mut cmd_undated = Command::cargo_bin("akashic").unwrap();
    cmd_undated
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-undated")
        .arg("--mode")
        .arg("historical")
        .arg("--as-of")
        .arg("2026-01-01")
        .assert()
        .success()
        .stdout(predicate::str::contains("> [!WARNING]"))
        .stdout(predicate::str::contains("lacks lifecycle dates"));
}

#[test]
fn test_cli_query_competing_successors_unresolved_conflict_human() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let base = r#"---
id: dec-base
type: Decision
title: Base Decision
---
Base content
"#;
    let s1 = r#"---
id: dec-succ1
type: Decision
title: Successor 1
relations:
  - type: supersedes
    target: ./dec-base.md
    evidence: ./reviews/s1.md
---
Succ 1 content
"#;
    let s2 = r#"---
id: dec-succ2
type: Decision
title: Successor 2
relations:
  - type: supersedes
    target: ./dec-base.md
    evidence: ./reviews/s2.md
---
Succ 2 content
"#;

    fs::write(root.join("dec-base.md"), base).unwrap();
    fs::write(root.join("dec-succ1.md"), s1).unwrap();
    fs::write(root.join("dec-succ2.md"), s2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-base")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("- **Active Guidance**: None (Strict abstention)"));
    assert!(stdout.contains("- **Status**: UNRESOLVED CONFLICT (Strict abstention)"));
    assert!(stdout.contains("> [!WARNING]"));
    assert!(stdout.contains("### [UNRESOLVED_CONFLICT]"));
    assert!(stdout.contains("- **Kind**: competing_successors"));
    assert!(stdout.contains("- **Predecessor**: `dec-base`"));
    assert!(stdout.contains("- **Competing Candidates**:"));
    assert!(stdout.contains("`dec-succ1` (evidence: ./reviews/s1.md)"));
    assert!(stdout.contains("`dec-succ2` (evidence: ./reviews/s2.md)"));
    assert!(stdout.contains("## Competing Candidates"));
    assert!(stdout.contains("### Successor 1 (`dec-succ1`) [DISPUTED]"));
    assert!(stdout.contains("### Successor 2 (`dec-succ2`) [DISPUTED]"));
}

#[test]
fn test_cli_query_competing_successors_unresolved_conflict_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let base = r#"---
id: dec-base
type: Decision
title: Base Decision
---
Base content
"#;
    let s1 = r#"---
id: dec-succ1
type: Decision
title: Successor 1
relations:
  - type: supersedes
    target: ./dec-base.md
    evidence: ./reviews/s1.md
---
Succ 1 content
"#;
    let s2 = r#"---
id: dec-succ2
type: Decision
title: Successor 2
relations:
  - type: supersedes
    target: ./dec-base.md
    evidence: ./reviews/s2.md
---
Succ 2 content
"#;

    fs::write(root.join("dec-base.md"), base).unwrap();
    fs::write(root.join("dec-succ1.md"), s1).unwrap();
    fs::write(root.join("dec-succ2.md"), s2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("dec-base")
        .arg("--json")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON");

    assert_eq!(parsed["status"], "unresolved_conflict");
    assert!(parsed.get("active_guidance_id").is_none() || parsed["active_guidance_id"].is_null());
    assert!(parsed.get("active_note").is_none() || parsed["active_note"].is_null());

    let conflict = &parsed["unresolved_conflict"];
    assert_eq!(conflict["alert"], "[UNRESOLVED_CONFLICT]");
    assert_eq!(conflict["kind"], "competing_successors");
    assert_eq!(conflict["predecessor"], "dec-base");
    assert_eq!(conflict["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(conflict["candidates"][0]["id"], "dec-succ1");
    assert_eq!(conflict["candidates"][0]["evidence"], "./reviews/s1.md");
    assert_eq!(conflict["candidates"][1]["id"], "dec-succ2");
    assert_eq!(conflict["candidates"][1]["evidence"], "./reviews/s2.md");
}

#[test]
fn test_cli_query_supersession_cycle_unresolved_conflict_human() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let n1 = r#"---
id: node-1
type: Decision
title: Node One
relations:
  - type: supersedes
    target: ./node-2.md
    evidence: ./ev1.md
---
Body 1
"#;
    let n2 = r#"---
id: node-2
type: Decision
title: Node Two
relations:
  - type: supersedes
    target: ./node-3.md
    evidence: ./ev2.md
---
Body 2
"#;
    let n3 = r#"---
id: node-3
type: Decision
title: Node Three
relations:
  - type: supersedes
    target: ./node-1.md
    evidence: ./ev3.md
---
Body 3
"#;

    fs::write(root.join("node-1.md"), n1).unwrap();
    fs::write(root.join("node-2.md"), n2).unwrap();
    fs::write(root.join("node-3.md"), n3).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("node-1")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("- **Active Guidance**: None (Strict abstention)"));
    assert!(stdout.contains("- **Status**: UNRESOLVED CONFLICT (Strict abstention)"));
    assert!(stdout.contains("> [!WARNING]"));
    assert!(stdout.contains("### [UNRESOLVED_CONFLICT]"));
    assert!(stdout.contains("- **Kind**: supersession_cycle"));
    assert!(stdout.contains("- **Cycle Path**:"));
    assert!(stdout.contains("- **Competing Candidates**:"));
    assert!(stdout.contains("## Competing Candidates"));
    assert!(stdout.contains("### Node One (`node-1`) [DISPUTED]"));
    assert!(stdout.contains("### Node Two (`node-2`) [DISPUTED]"));
    assert!(stdout.contains("### Node Three (`node-3`) [DISPUTED]"));
}

#[test]
fn test_cli_query_supersession_cycle_unresolved_conflict_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let n1 = r#"---
id: node-1
type: Decision
title: Node One
relations:
  - type: supersedes
    target: ./node-2.md
    evidence: ./ev1.md
---
Body 1
"#;
    let n2 = r#"---
id: node-2
type: Decision
title: Node Two
relations:
  - type: supersedes
    target: ./node-3.md
    evidence: ./ev2.md
---
Body 2
"#;
    let n3 = r#"---
id: node-3
type: Decision
title: Node Three
relations:
  - type: supersedes
    target: ./node-1.md
    evidence: ./ev3.md
---
Body 3
"#;

    fs::write(root.join("node-1.md"), n1).unwrap();
    fs::write(root.join("node-2.md"), n2).unwrap();
    fs::write(root.join("node-3.md"), n3).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("node-1")
        .arg("--json")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON");

    assert_eq!(parsed["status"], "unresolved_conflict");
    assert!(parsed.get("active_guidance_id").is_none() || parsed["active_guidance_id"].is_null());

    let conflict = &parsed["unresolved_conflict"];
    assert_eq!(conflict["alert"], "[UNRESOLVED_CONFLICT]");
    assert_eq!(conflict["kind"], "supersession_cycle");
    assert!(conflict["cycle"].is_array());
    assert_eq!(conflict["candidates"].as_array().unwrap().len(), 3);
}

#[test]
fn test_cli_query_competing_successors_resolved_when_one_supersedes_other() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let base = r#"---
id: base-note
type: Decision
title: Base Note
---
Base
"#;
    let succ_b = r#"---
id: succ-b
type: Decision
title: Intermediate Successor B
relations:
  - type: supersedes
    target: ./base-note.md
---
B
"#;
    let succ_c = r#"---
id: succ-c
type: Decision
title: Final Successor C
relations:
  - type: supersedes
    target: ./base-note.md
  - type: supersedes
    target: ./succ-b.md
---
C
"#;

    fs::write(root.join("base-note.md"), base).unwrap();
    fs::write(root.join("succ-b.md"), succ_b).unwrap();
    fs::write(root.join("succ-c.md"), succ_c).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("base-note")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("- **Active Guidance**: `succ-c`"));
    assert!(stdout.contains("Final Successor C (`succ-c`)"));
}

#[test]
fn test_cli_query_active_contradiction_unresolved_conflict() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let note_a = r#"---
id: contra-a
type: Decision
title: SQLite Storage Engine
status: stable
relations:
  - type: contradicts
    target: ./contra-b.md
    evidence: ./analysis/storage-divergence.md
---
Use embedded SQLite for local storage.
"#;

    let note_b = r#"---
id: contra-b
type: Decision
title: PostgreSQL Storage Engine
status: stable
relations:
  - type: contradicts
    target: ./contra-a.md
    evidence: ./analysis/storage-divergence.md
---
Use client-server PostgreSQL for all persistence.
"#;

    fs::write(root.join("contra-a.md"), note_a).unwrap();
    fs::write(root.join("contra-b.md"), note_b).unwrap();

    // Human format: exits with 2 and renders conflict banner
    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("contra-a")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[UNRESOLVED_CONFLICT]"));
    assert!(stdout.contains("- **Kind**: contradiction"));
    assert!(stdout.contains("Active contradiction between 'contra-a' and 'contra-b'"));
    assert!(stdout.contains("## Competing Candidates"));
    assert!(stdout.contains("### SQLite Storage Engine (`contra-a`) [DISPUTED]"));
    assert!(stdout.contains("### PostgreSQL Storage Engine (`contra-b`) [DISPUTED]"));

    // JSON format: exits with 2 and contains structured packet
    let mut json_cmd = Command::cargo_bin("akashic").unwrap();
    let json_assert = json_cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("contra-a")
        .arg("--json")
        .assert()
        .code(2);

    let json_str = String::from_utf8(json_assert.get_output().stdout.clone()).unwrap();
    let val: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["status"], "unresolved_conflict");
    assert!(val["active_guidance_id"].is_null());
    assert_eq!(val["unresolved_conflict"]["kind"], "contradiction");
    assert_eq!(
        val["unresolved_conflict"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn test_cli_query_transitive_candidate_supersession_resolved() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let base = r#"---
id: base-note
type: Decision
title: Base Note
---
Base
"#;

    // succ-a directly supersedes base-note
    let succ_a = r#"---
id: succ-a
type: Decision
title: Candidate A
relations:
  - type: supersedes
    target: ./base-note.md
---
Candidate A
"#;

    // succ-x supersedes succ-a
    let succ_x = r#"---
id: succ-x
type: Decision
title: Intermediate X
relations:
  - type: supersedes
    target: ./succ-a.md
---
Intermediate X
"#;

    // succ-b supersedes base-note AND succ-x (so B transitively supersedes A via X)
    let succ_b = r#"---
id: succ-b
type: Decision
title: Candidate B
relations:
  - type: supersedes
    target: ./base-note.md
  - type: supersedes
    target: ./succ-x.md
---
Candidate B
"#;

    fs::write(root.join("base-note.md"), base).unwrap();
    fs::write(root.join("succ-a.md"), succ_a).unwrap();
    fs::write(root.join("succ-x.md"), succ_x).unwrap();
    fs::write(root.join("succ-b.md"), succ_b).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("base-note")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("- **Active Guidance**: `succ-b`"));
    assert!(stdout.contains("Candidate B (`succ-b`)"));
}

#[test]
fn test_cli_query_predecessor_not_returned_when_successors_in_cycle() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let pred = r#"---
id: pred-note
type: Decision
title: Predecessor Note
---
Predecessor
"#;

    let cyc_1 = r#"---
id: cyc-succ-1
type: Decision
title: Cyclic Successor 1
relations:
  - type: supersedes
    target: ./pred-note.md
  - type: supersedes
    target: ./cyc-succ-2.md
---
Successor 1
"#;

    let cyc_2 = r#"---
id: cyc-succ-2
type: Decision
title: Cyclic Successor 2
relations:
  - type: supersedes
    target: ./pred-note.md
  - type: supersedes
    target: ./cyc-succ-1.md
---
Successor 2
"#;

    fs::write(root.join("pred-note.md"), pred).unwrap();
    fs::write(root.join("cyc-succ-1.md"), cyc_1).unwrap();
    fs::write(root.join("cyc-succ-2.md"), cyc_2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("pred-note")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[UNRESOLVED_CONFLICT]"));
    assert!(stdout.contains("- **Kind**: supersession_cycle"));
    assert!(stdout.contains("- **Predecessor**: `pred-note`"));
    assert!(!stdout.contains("- **Active Guidance**: `pred-note`"));

    // JSON format verification
    let mut json_cmd = Command::cargo_bin("akashic").unwrap();
    let json_assert = json_cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("pred-note")
        .arg("--json")
        .assert()
        .code(2);

    let json_str = String::from_utf8(json_assert.get_output().stdout.clone()).unwrap();
    let val: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["status"], "unresolved_conflict");
    assert!(val["active_guidance_id"].is_null());
    assert_eq!(val["unresolved_conflict"]["kind"], "supersession_cycle");
    assert_eq!(val["unresolved_conflict"]["predecessor"], "pred-note");
}

#[test]
fn test_cli_query_predecessor_not_returned_when_all_successors_inactive() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let pred = r#"---
id: pred-note
type: Decision
title: Predecessor Note
---
Predecessor
"#;

    let dep_1 = r#"---
id: dep-succ-1
type: Decision
title: Deprecated Successor 1
status: deprecated
relations:
  - type: supersedes
    target: ./pred-note.md
---
Deprecated 1
"#;

    let dep_2 = r#"---
id: dep-succ-2
type: Decision
title: Deprecated Successor 2
status: deprecated
relations:
  - type: supersedes
    target: ./pred-note.md
---
Deprecated 2
"#;

    fs::write(root.join("pred-note.md"), pred).unwrap();
    fs::write(root.join("dep-succ-1.md"), dep_1).unwrap();
    fs::write(root.join("dep-succ-2.md"), dep_2).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    let assert = cmd
        .arg("query")
        .arg(root)
        .arg("--seed")
        .arg("pred-note")
        .assert()
        .code(2);

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[UNRESOLVED_CONFLICT]"));
    assert!(stdout.contains("- **Kind**: superseded_without_active_successor"));
    assert!(stdout.contains("- **Predecessor**: `pred-note`"));
    assert!(!stdout.contains("- **Active Guidance**: `pred-note`"));
}
