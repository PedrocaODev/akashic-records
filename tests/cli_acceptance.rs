use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const PAYMENT_VAULT_PATH: &str = "tests/fixtures/payment_vault";

const EXPECTED_FIXTURE_FILES: &[&str] = &[
    "concept-payment-status.md",
    "decision-payment-status-polling.md",
    "decision-payment-status-events.md",
    "review-incident-timeout.md",
    "incident-2026-05-status-timeout.md",
    "runbook-payment-reconciliation.md",
    "decision-payment-staging-polling.md",
    "proposal-payment-grpc.md",
    "cycle-canary-a.md",
    "cycle-canary-b.md",
    "cycle-canary-c.md",
    "competing-successor.md",
    "stale-unreplaced-batch.md",
    "external-source-stripe.md",
    "corrupted-target.md",
];

fn vault_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PAYMENT_VAULT_PATH)
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Requirement 2a:
/// All 15 fixture files exist and can be inspected (`inspect` subcommand on single note and whole vault).
#[test]
fn test_acceptance_fixture_files_and_inspect() {
    let vault = vault_path();
    assert!(vault.exists(), "Payment vault fixture directory must exist");

    // Verify all 15 files exist
    for filename in EXPECTED_FIXTURE_FILES {
        let file_path = vault.join(filename);
        assert!(
            file_path.exists(),
            "Expected fixture note '{}' does not exist",
            filename
        );
    }

    // Inspect whole vault (human output)
    let mut cmd_vault_human = Command::cargo_bin("akashic").unwrap();
    cmd_vault_human
        .arg("inspect")
        .arg(&vault)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Nodes: 15"))
        .stdout(predicate::str::contains("Total Edges: 15"))
        .stdout(predicate::str::contains("supersedes: 5"))
        .stdout(predicate::str::contains("depends_on: 5"))
        .stdout(predicate::str::contains("supported_by: 2"))
        .stdout(predicate::str::contains("caused_by: 1"))
        .stdout(predicate::str::contains("relates_to: 1"))
        .stdout(predicate::str::contains("contradicts: 1"));

    // Inspect whole vault (JSON output)
    let mut cmd_vault_json = Command::cargo_bin("akashic").unwrap();
    let assert_json = cmd_vault_json
        .arg("inspect")
        .arg(&vault)
        .arg("--json")
        .assert()
        .success();

    let stdout_json = String::from_utf8(assert_json.get_output().stdout.clone()).unwrap();
    let inv: Value = serde_json::from_str(&stdout_json).expect("Valid JSON inventory");
    assert_eq!(inv["total_nodes"], 15);
    assert_eq!(inv["total_edges"], 15);
    assert_eq!(inv["edges_by_type"]["supersedes"], 5);
    assert_eq!(inv["edges_by_type"]["depends_on"], 5);
    assert_eq!(inv["edges_by_type"]["supported_by"], 2);
    assert_eq!(inv["edges_by_type"]["caused_by"], 1);
    assert_eq!(inv["edges_by_type"]["relates_to"], 1);
    assert_eq!(inv["edges_by_type"]["contradicts"], 1);
    assert!(
        inv["malformed_files"].as_array().unwrap().is_empty(),
        "All 15 fixture files must parse without malformation"
    );

    // Inspect single note (human output)
    let single_note = vault.join("concept-payment-status.md");
    let mut cmd_single_human = Command::cargo_bin("akashic").unwrap();
    cmd_single_human
        .arg("inspect")
        .arg(&single_note)
        .assert()
        .success()
        .stdout(predicate::str::contains("Node: concept-payment-status"))
        .stdout(predicate::str::contains("Type: Concept"))
        .stdout(predicate::str::contains(
            "Title: Payment Status Domain Model",
        ))
        .stdout(predicate::str::contains("Status: stable"))
        .stdout(predicate::str::contains("Valid From: 2024-01-01"))
        .stdout(predicate::str::contains("system: payments"));

    // Inspect single note (JSON output)
    let events_note = vault.join("decision-payment-status-events.md");
    let mut cmd_single_json = Command::cargo_bin("akashic").unwrap();
    let assert_single = cmd_single_json
        .arg("inspect")
        .arg(&events_note)
        .arg("--json")
        .assert()
        .success();

    let stdout_single = String::from_utf8(assert_single.get_output().stdout.clone()).unwrap();
    let note_val: Value = serde_json::from_str(&stdout_single).expect("Valid JSON note");
    assert_eq!(note_val["id"], "decision-payment-status-events");
    assert_eq!(note_val["type"], "Decision");
    assert_eq!(note_val["title"], "Use event-driven payment status");
    assert_eq!(note_val["status"], "stable");
    assert_eq!(note_val["valid_from"], "2026-06-01");
    assert_eq!(note_val["scope"]["system"], "payments");
    assert_eq!(note_val["scope"]["env"], "production");
    assert_eq!(note_val["relations"].as_array().unwrap().len(), 3);
}

/// Requirement 2b:
/// `lint` against `tests/fixtures/payment_vault/`:
/// - Must fail (non-zero exit code 1)
/// - Tarjan SCC detects 3-node canary cycle (cycle-canary-a -> cycle-canary-b -> cycle-canary-c -> cycle-canary-a)
/// - Broken target detected for `corrupted-target` targeting `./nonexistent-target-note.md`
/// - Conflicting assertions detected:
///   * Competing successors (`competing-successor` and `decision-payment-status-events` both superseding `decision-payment-status-polling`)
///   * Active contradiction (`proposal-payment-grpc` and `decision-payment-status-events`)
/// - Verify both human-readable and `--json` outputs.
#[test]
fn test_acceptance_lint_detects_all_fixture_violations_human_and_json() {
    let vault = vault_path();

    // Human output verification
    let mut cmd_human = Command::cargo_bin("akashic").unwrap();
    cmd_human
        .arg("lint")
        .arg(&vault)
        .assert()
        .code(predicate::eq(1))
        .stdout(predicate::str::contains("Checked 15 nodes and 15 edges."))
        .stdout(predicate::str::contains("Integrity Violations (4):"))
        .stdout(predicate::str::contains(
            "[broken_target] Node 'corrupted-target' has relation 'depends_on' targeting './nonexistent-target-note.md' which does not resolve to any note in the vault",
        ))
        .stdout(predicate::str::contains(
            "[conflicting_assertion] Competing successors: 'competing-successor' and 'decision-payment-status-events' both supersede 'decision-payment-status-polling' in overlapping scopes without superseding each other",
        ))
        .stdout(predicate::str::contains(
            "[conflicting_assertion] Active contradiction between 'decision-payment-status-events' and 'proposal-payment-grpc' with overlapping scopes",
        ))
        .stdout(predicate::str::contains(
            "[supersession_cycle] Supersession cycle detected: cycle-canary-a -> cycle-canary-b -> cycle-canary-c -> cycle-canary-a",
        ))
        .stdout(predicate::str::contains(
            "Lint failed with 4 error(s) and 0 warning(s).",
        ));

    // Structured JSON output verification
    let mut cmd_json = Command::cargo_bin("akashic").unwrap();
    let assert_json = cmd_json
        .arg("lint")
        .arg(&vault)
        .arg("--json")
        .assert()
        .code(predicate::eq(1));

    let stdout_json = String::from_utf8(assert_json.get_output().stdout.clone()).unwrap();
    let report: Value = serde_json::from_str(&stdout_json).expect("Valid JSON lint report");
    assert_eq!(report["healthy"], false);
    assert_eq!(report["total_nodes"], 15);
    assert_eq!(report["total_edges"], 15);
    assert_eq!(report["errors"], 4);
    assert_eq!(report["warnings"], 0);

    let findings = report["findings"].as_array().expect("Findings array");
    assert_eq!(findings.len(), 4);

    // Finding 1: broken target
    assert_eq!(findings[0]["kind"], "broken_target");
    assert_eq!(findings[0]["severity"], "error");
    assert_eq!(findings[0]["source"], "corrupted-target");
    assert_eq!(findings[0]["target"], "./nonexistent-target-note.md");

    // Finding 2: competing successors
    assert_eq!(findings[1]["kind"], "conflicting_assertion");
    assert_eq!(findings[1]["severity"], "error");
    assert_eq!(findings[1]["source"], "competing-successor");
    assert_eq!(findings[1]["target"], "decision-payment-status-events");
    assert!(findings[1]["message"]
        .as_str()
        .unwrap()
        .contains("both supersede 'decision-payment-status-polling'"));

    // Finding 3: active contradiction
    assert_eq!(findings[2]["kind"], "conflicting_assertion");
    assert_eq!(findings[2]["severity"], "error");
    assert_eq!(findings[2]["source"], "decision-payment-status-events");
    assert_eq!(findings[2]["target"], "proposal-payment-grpc");
    assert!(findings[2]["message"]
        .as_str()
        .unwrap()
        .contains("Active contradiction"));

    // Finding 4: supersession cycle via Tarjan SCC
    assert_eq!(findings[3]["kind"], "supersession_cycle");
    assert_eq!(findings[3]["severity"], "error");
    assert_eq!(findings[3]["source"], "cycle-canary-a");
    let cycle = findings[3]["cycle"].as_array().expect("Cycle array");
    assert_eq!(
        cycle,
        &[
            Value::String("cycle-canary-a".to_string()),
            Value::String("cycle-canary-b".to_string()),
            Value::String("cycle-canary-c".to_string()),
        ]
    );
}

/// Requirement 2c:
/// `query` against `tests/fixtures/payment_vault/`:
/// - Seeded with `decision-payment-status-polling`: triggers `[UNRESOLVED_CONFLICT]` due to
///   competing successors (`competing-successor` and `decision-payment-status-events`), exiting with status code 2.
/// - Seeded with `cycle-canary-a`: triggers `[UNRESOLVED_CONFLICT]` due to supersession cycle, exiting with status code 2.
#[test]
fn test_acceptance_query_unresolved_conflicts() {
    let vault = vault_path();

    // Seeded with decision-payment-status-polling (competing successors)
    let mut cmd_polling = Command::cargo_bin("akashic").unwrap();
    cmd_polling
        .arg("query")
        .arg(&vault)
        .arg("--seed")
        .arg("decision-payment-status-polling")
        .assert()
        .code(predicate::eq(2))
        .stdout(predicate::str::contains("### [UNRESOLVED_CONFLICT]"))
        .stdout(predicate::str::contains("- **Kind**: competing_successors"))
        .stdout(predicate::str::contains(
            "- **Predecessor**: `decision-payment-status-polling`",
        ))
        .stdout(predicate::str::contains(
            "`competing-successor` (evidence: ./review-incident-timeout.md)",
        ))
        .stdout(predicate::str::contains(
            "`decision-payment-status-events` (evidence: ./review-incident-timeout.md)",
        ))
        .stdout(predicate::str::contains(
            "Alternative Webhook Payment Status (`competing-successor`) [DISPUTED]",
        ))
        .stdout(predicate::str::contains(
            "Use event-driven payment status (`decision-payment-status-events`) [DISPUTED]",
        ));

    // Seeded with cycle-canary-a (Tarjan SCC cycle)
    let mut cmd_cycle = Command::cargo_bin("akashic").unwrap();
    cmd_cycle
        .arg("query")
        .arg(&vault)
        .arg("--seed")
        .arg("cycle-canary-a")
        .assert()
        .code(predicate::eq(2))
        .stdout(predicate::str::contains("### [UNRESOLVED_CONFLICT]"))
        .stdout(predicate::str::contains("- **Kind**: supersession_cycle"))
        .stdout(predicate::str::contains(
            "- **Cycle Path**: cycle-canary-a -> cycle-canary-b -> cycle-canary-c -> cycle-canary-a",
        ))
        .stdout(predicate::str::contains("Canary Cycle Node A (`cycle-canary-a`) [DISPUTED]"))
        .stdout(predicate::str::contains("Canary Cycle Node B (`cycle-canary-b`) [DISPUTED]"))
        .stdout(predicate::str::contains("Canary Cycle Node C (`cycle-canary-c`) [DISPUTED]"));
}

/// Requirement 2d:
/// Current guidance resolution test:
/// When vault is copied to a tempdir and `competing-successor.md` is removed (resolving the competing successor conflict),
/// querying `--seed decision-payment-status-polling --mode current` cleanly resolves to active guidance
/// `decision-payment-status-events` and shows `decision-payment-status-polling [SUPERSEDED]` in Historical Rationale!
#[test]
fn test_acceptance_current_guidance_resolution() {
    let dir = tempdir().unwrap();
    copy_dir_all(&vault_path(), dir.path());

    // Remove competing-successor.md and proposal-payment-grpc.md to remove conflicting assertions on successor
    fs::remove_file(dir.path().join("competing-successor.md")).unwrap();
    fs::remove_file(dir.path().join("proposal-payment-grpc.md")).unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("query")
        .arg(dir.path())
        .arg("--seed")
        .arg("decision-payment-status-polling")
        .arg("--mode")
        .arg("current")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains(
            "- **Status**: SUPERSEDED (Resolved to successor)",
        ))
        .stdout(predicate::str::contains(
            "Resolved superseded note 'decision-payment-status-polling' along 1 supersession hop(s) to active guidance 'decision-payment-status-events'.",
        ))
        .stdout(predicate::str::contains("## Active Guidance"))
        .stdout(predicate::str::contains(
            "Use event-driven payment status (`decision-payment-status-events`)",
        ))
        .stdout(predicate::str::contains("## Historical Rationale (SUPERSEDED)"))
        .stdout(predicate::str::contains(
            "Use HTTP polling for payment status (`decision-payment-status-polling`) [SUPERSEDED]",
        ))
        .stdout(predicate::str::contains(
            "- **Superseded By**: `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains(
            "- **Evidence**: ./review-incident-timeout.md",
        ));
}

/// Requirement 2e:
/// Query multi-mode tests against the payment vault:
/// - `--mode lineage` on `decision-payment-status-events`
/// - `--mode impact` on `decision-payment-status-events` (finding `runbook-payment-reconciliation`)
/// - `--mode historical --as-of 2025-01-15` on `decision-payment-status-polling`
#[test]
fn test_acceptance_query_multimode() {
    let vault = vault_path();

    // 1. Mode impact directly on fixture vault
    let mut cmd_impact = Command::cargo_bin("akashic").unwrap();
    cmd_impact
        .arg("query")
        .arg(&vault)
        .arg("--seed")
        .arg("decision-payment-status-events")
        .arg("--mode")
        .arg("impact")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Impact Analysis Packet",
        ))
        .stdout(predicate::str::contains(
            "- **Seed**: `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains("- **Total Dependents**: 1"))
        .stdout(predicate::str::contains("- **Max Depth**: 1"))
        .stdout(predicate::str::contains("## Downstream Dependents"))
        .stdout(predicate::str::contains(
            "Payment Reconciliation Runbook (`runbook-payment-reconciliation`)",
        ))
        .stdout(predicate::str::contains("- **Depth**: 1"))
        .stdout(predicate::str::contains(
            "- **Dependency Path**: `decision-payment-status-events` -> `runbook-payment-reconciliation`",
        ));

    // 2. Mode lineage on clean resolved chain
    let dir = tempdir().unwrap();
    copy_dir_all(&vault, dir.path());
    fs::remove_file(dir.path().join("competing-successor.md")).unwrap();

    let mut cmd_lineage = Command::cargo_bin("akashic").unwrap();
    cmd_lineage
        .arg("query")
        .arg(dir.path())
        .arg("--seed")
        .arg("decision-payment-status-events")
        .arg("--mode")
        .arg("lineage")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Decision Lineage Packet",
        ))
        .stdout(predicate::str::contains(
            "- **Seed**: `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains("- **Chain Length**: 2"))
        .stdout(predicate::str::contains(
            "`decision-payment-status-polling` -> `decision-payment-status-events`",
        ))
        .stdout(predicate::str::contains(
            "1. Use HTTP polling for payment status (`decision-payment-status-polling`) [SUPERSEDED]",
        ))
        .stdout(predicate::str::contains(
            "2. Use event-driven payment status (`decision-payment-status-events`) [CURRENT GUIDANCE]",
        ))
        .stdout(predicate::str::contains("## Empirical Evidence"))
        .stdout(predicate::str::contains(
            "Post-Incident Review - Payment Status Polling Timeout (`review-incident-timeout`)",
        ));

    // 3. Mode historical with --as-of 2025-01-15 on decision-payment-status-polling
    let mut cmd_historical = Command::cargo_bin("akashic").unwrap();
    cmd_historical
        .arg("query")
        .arg(dir.path())
        .arg("--seed")
        .arg("decision-payment-status-polling")
        .arg("--mode")
        .arg("historical")
        .arg("--as-of")
        .arg("2025-01-15")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Akashic Records — Historical Context Packet",
        ))
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-payment-status-polling`",
        ))
        .stdout(predicate::str::contains("- **As Of**: `2025-01-15`"))
        .stdout(predicate::str::contains(
            "Resolved active guidance as of 2025-01-15 to 'decision-payment-status-polling'.",
        ))
        .stdout(predicate::str::contains(
            "Use HTTP polling for payment status (`decision-payment-status-polling`)",
        ));
}

/// Requirement 2f:
/// Scoped query test:
/// - Querying `decision-payment-staging-polling` in staging scope vs production advisory.
#[test]
fn test_acceptance_scoped_query() {
    let vault = vault_path();

    // Querying decision-payment-staging-polling in staging scope
    let mut cmd_staging = Command::cargo_bin("akashic").unwrap();
    cmd_staging
        .arg("query")
        .arg(&vault)
        .arg("--seed")
        .arg("decision-payment-staging-polling")
        .arg("--scope")
        .arg("env=staging")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-payment-staging-polling`",
        ))
        .stdout(predicate::str::contains(
            "- **Scope Filter**: [env=staging]",
        ))
        .stdout(predicate::str::contains(
            "Use mock HTTP polling in staging (`decision-payment-staging-polling`)",
        ));

    // Querying decision-payment-status-polling in staging scope triggers production advisories
    let mut cmd_prod_advisory = Command::cargo_bin("akashic").unwrap();
    cmd_prod_advisory
        .arg("query")
        .arg(&vault)
        .arg("--seed")
        .arg("decision-payment-status-polling")
        .arg("--scope")
        .arg("env=staging")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "- **Active Guidance**: `decision-payment-status-polling`",
        ))
        .stdout(predicate::str::contains(
            "Note 'decision-payment-status-polling' has no active successors in the requested scope",
        ))
        .stdout(predicate::str::contains("> [!NOTE]"))
        .stdout(predicate::str::contains("**Advisories:**"))
        .stdout(predicate::str::contains(
            "A successor exists in another scope: 'decision-payment-status-events' (scope: [env=production, system=payments])",
        ));
}

/// Requirement 2g:
/// Lossless OKF export test:
/// - `akashic export tests/fixtures/payment_vault --format okf --out <tempdir>`
/// - Verifies manifest `okf_manifest.json` / `manifest.json` and checks that all 15 notes are exported
///   with their frontmatter relations preserved losslessly in the `relations:` block.
#[test]
fn test_acceptance_lossless_okf_export() {
    let vault = vault_path();
    let export_dir = tempdir().unwrap();

    let mut cmd = Command::cargo_bin("akashic").unwrap();
    cmd.arg("export")
        .arg(&vault)
        .arg("--format")
        .arg("okf")
        .arg("--out")
        .arg(export_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported 15 node(s)"))
        .stdout(predicate::str::contains("Manifest written to"));

    let manifest_path = if export_dir.path().join("okf_manifest.json").exists() {
        export_dir.path().join("okf_manifest.json")
    } else {
        export_dir.path().join("manifest.json")
    };
    assert!(manifest_path.exists(), "Export manifest must exist");

    let manifest_content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: Value = serde_json::from_str(&manifest_content).expect("Valid JSON manifest");

    assert_eq!(manifest["format"], "okf-v0.2");
    assert_eq!(manifest["total_nodes"], 15);
    assert_eq!(manifest["total_relations"], 16);
    assert_eq!(manifest["relations_by_type"]["supersedes"], 5);
    assert_eq!(manifest["relations_by_type"]["depends_on"], 6);
    assert_eq!(manifest["relations_by_type"]["supported_by"], 2);
    assert_eq!(manifest["relations_by_type"]["caused_by"], 1);
    assert_eq!(manifest["relations_by_type"]["relates_to"], 1);
    assert_eq!(manifest["relations_by_type"]["contradicts"], 1);

    // Verify all 15 notes exist on disk and relations block is preserved losslessly
    for filename in EXPECTED_FIXTURE_FILES {
        let exported_file = export_dir.path().join(filename);
        assert!(
            exported_file.exists(),
            "Exported note '{}' must exist on disk",
            filename
        );

        let content = fs::read_to_string(&exported_file).unwrap();
        assert!(
            content.starts_with("---\n"),
            "Exported note '{}' must retain YAML frontmatter",
            filename
        );
    }

    // Specific relational checks on exported note frontmatter
    let events_content =
        fs::read_to_string(export_dir.path().join("decision-payment-status-events.md")).unwrap();
    assert!(events_content.contains("type: supersedes"));
    assert!(events_content.contains("target: ./decision-payment-status-polling.md"));
    assert!(events_content.contains("evidence: ./review-incident-timeout.md"));
    assert!(events_content.contains("type: supported_by"));
    assert!(events_content.contains("target: ./review-incident-timeout.md"));
    assert!(events_content.contains("type: depends_on"));
    assert!(events_content.contains("target: ./concept-payment-status.md"));

    let review_content =
        fs::read_to_string(export_dir.path().join("review-incident-timeout.md")).unwrap();
    assert!(review_content.contains("type: relates_to"));
    assert!(review_content.contains("target: ./incident-2026-05-status-timeout.md"));
    assert!(review_content.contains("type: supported_by"));
    assert!(review_content.contains("target: ./external-source-stripe.md"));

    let incident_content =
        fs::read_to_string(export_dir.path().join("incident-2026-05-status-timeout.md")).unwrap();
    assert!(incident_content.contains("type: caused_by"));
    assert!(incident_content.contains("target: ./decision-payment-status-polling.md"));

    let staging_content = fs::read_to_string(
        export_dir
            .path()
            .join("decision-payment-staging-polling.md"),
    )
    .unwrap();
    assert!(staging_content.contains("type: depends_on"));
    assert!(staging_content.contains("target: ./concept-payment-status.md"));
}

/// Requirement 2h:
/// Plan & Apply lifecycle test:
/// - Run plan and apply against a temp vault derived from payment_vault, verifying SHA-256 validation
///   and strict idempotency.
#[test]
fn test_acceptance_plan_apply_lifecycle() {
    let dir = tempdir().unwrap();
    copy_dir_all(&vault_path(), dir.path());
    let root = dir.path();

    // 1. Initial plan on normalized payment vault has 0 actions (empty actions array)
    let mut cmd_plan_clean = Command::cargo_bin("akashic").unwrap();
    cmd_plan_clean
        .arg("plan")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"actions\": []"));

    // 2. Introduce an unnormalized file needing plan actions:
    // - Missing `id:` (stem inference)
    // - Top-level `supersedes:` (relation migration)
    // - Wikilink `[[concept-payment-status]]` (markdown link migration)
    let candidate_content = r#"---
type: Decision
title: Candidate Architectural Revision
supersedes: ./decision-payment-status-events.md
scope:
  system: payments
  env: production
---
# Candidate Revision
Investigating transitions defined in [[concept-payment-status]].
"#;
    let candidate_path = root.join("decision-candidate-revision.md");
    fs::write(&candidate_path, candidate_content).unwrap();

    let plan_file = root.join("migration_plan.json");

    // 3. Generate plan
    let mut cmd_plan = Command::cargo_bin("akashic").unwrap();
    cmd_plan
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&plan_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Plan written to"))
        .stdout(predicate::str::contains("1 proposed action"));

    assert!(plan_file.exists());
    let plan_str = fs::read_to_string(&plan_file).unwrap();
    let plan_val: Value = serde_json::from_str(&plan_str).expect("Valid JSON plan");
    assert_eq!(plan_val["actions"].as_array().unwrap().len(), 1);

    let action = &plan_val["actions"][0];
    let source_sha = action["source_sha256"].as_str().unwrap();
    let expected_sha = action["expected_new_sha256"].as_str().unwrap();
    assert_eq!(source_sha.len(), 64);
    assert_eq!(expected_sha.len(), 64);
    assert_ne!(source_sha, expected_sha);

    // 4. Test SHA-256 validation: Tampering file aborts apply cleanly without partial modification
    fs::write(&candidate_path, "tampered unauthorized disk change").unwrap();

    let mut cmd_apply_tampered = Command::cargo_bin("akashic").unwrap();
    cmd_apply_tampered
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Hash mismatch for file"))
        .stderr(predicate::str::contains(
            "Aborting cleanly without modifications",
        ));

    // 5. Restore candidate content to match original planned hash
    fs::write(&candidate_path, candidate_content).unwrap();

    // 6. Apply plan successfully
    let mut cmd_apply = Command::cargo_bin("akashic").unwrap();
    cmd_apply
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied 1 change"));

    // Verify mutations took effect
    let updated_candidate = fs::read_to_string(&candidate_path).unwrap();
    assert!(updated_candidate.contains("id: decision-candidate-revision"));
    assert!(!updated_candidate.contains("supersedes: ./decision-payment-status-events.md\nscope:"));
    assert!(updated_candidate.contains("relations:"));
    assert!(updated_candidate.contains("- type: supersedes"));
    assert!(updated_candidate.contains("target: ./decision-payment-status-events.md"));
    assert!(updated_candidate.contains("[concept-payment-status](./concept-payment-status.md)"));

    // 7. Strict Idempotency: Re-applying the exact same plan modifies 0 files
    let mut cmd_reapply = Command::cargo_bin("akashic").unwrap();
    cmd_reapply
        .arg("apply")
        .arg(root)
        .arg("--plan")
        .arg(&plan_file)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Applied 0 changes (1 files already up-to-date)",
        ));

    // 8. Re-planning discovers 0 actions
    let replan_file = root.join("replan.json");
    let mut cmd_replan = Command::cargo_bin("akashic").unwrap();
    cmd_replan
        .arg("plan")
        .arg(root)
        .arg("--out")
        .arg(&replan_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 proposed actions"));

    let replan_str = fs::read_to_string(&replan_file).unwrap();
    let replan_val: Value = serde_json::from_str(&replan_str).expect("Valid JSON replan");
    assert_eq!(replan_val["actions"].as_array().unwrap().len(), 0);
}
