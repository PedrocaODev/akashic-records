use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::cargo_bin;
use chrono::Local;
use fs2::FileExt;
use serde_json::{json, Value};
use tempfile::tempdir;

const PAYMENT_VAULT_PATH: &str = "tests/fixtures/payment_vault";

fn vault_fixture_path() -> PathBuf {
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

struct McpTestClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpTestClient {
    fn spawn(vault_path: &Path) -> Self {
        let bin = cargo_bin("akashic");
        let mut child = Command::new(bin)
            .arg("mcp")
            .arg("--vault")
            .arg(vault_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to spawn akashic mcp process");

        let stdin = child.stdin.take().expect("Child stdin unavailable");
        let stdout = child.stdout.take().expect("Child stdout unavailable");
        let reader = BufReader::new(stdout);

        Self {
            child,
            stdin,
            reader,
            next_id: 1,
        }
    }

    fn send_request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut raw = serde_json::to_string(&req).unwrap();
        raw.push('\n');
        self.stdin
            .write_all(raw.as_bytes())
            .expect("Failed to write to stdin");
        self.stdin.flush().expect("Failed to flush stdin");

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .expect("Failed to read line from child stdout");

        serde_json::from_str(&line).unwrap_or_else(|e| {
            panic!("Failed to parse JSON response: {e}. Raw stdout: {line}");
        })
    }

    fn send_notification(&mut self, method: &str, params: Value) {
        let notif = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let mut raw = serde_json::to_string(&notif).unwrap();
        raw.push('\n');
        self.stdin
            .write_all(raw.as_bytes())
            .expect("Failed to write to stdin");
        self.stdin.flush().expect("Failed to flush stdin");
    }

    fn initialize(&mut self) -> Value {
        let init_params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        });

        let resp = self.send_request("initialize", init_params);
        self.send_notification("notifications/initialized", json!({}));
        resp
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.send_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }
}

impl Drop for McpTestClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 1. Handshake (initialize request, response, notifications/initialized)
#[test]
fn test_mcp_handshake() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);

    let resp = client.initialize();
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(resp["error"].is_null(), "Handshake should not error");

    let result = &resp["result"];
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["serverInfo"]["name"], "akashic");
}

/// 2. Tool discovery (tools/list schema validation for all 5 tools)
#[test]
fn test_mcp_tool_discovery() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);
    client.initialize();

    let resp = client.send_request("tools/list", json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(resp["error"].is_null());

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools must be an array");

    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(
        tool_names.contains(&"akashic_query"),
        "Missing akashic_query"
    );
    assert!(
        tool_names.contains(&"akashic_inspect"),
        "Missing akashic_inspect"
    );
    assert!(tool_names.contains(&"akashic_lint"), "Missing akashic_lint");
    assert!(tool_names.contains(&"vault_read"), "Missing vault_read");
    assert!(
        tool_names.contains(&"vault_write_canonical"),
        "Missing vault_write_canonical"
    );
    assert_eq!(tools.len(), 5, "Expected exactly 5 tools");

    for tool in tools {
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object");
        if let Some(props) = schema.get("properties") {
            assert!(props.is_object());
        }
    }
}

/// 3. Query execution across modes against test fixture
#[test]
fn test_mcp_query_execution_across_modes() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);
    client.initialize();

    // Mode: current
    let current_res = client.call_tool(
        "akashic_query",
        json!({
            "seed": "decision-payment-status-events",
            "mode": "current"
        }),
    );
    assert!(current_res["error"].is_null());
    let current_text = current_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        current_text.contains("decision-payment-status-events"),
        "Current query must include seed note"
    );

    // Mode: lineage
    let lineage_res = client.call_tool(
        "akashic_query",
        json!({
            "seed": "decision-payment-status-events",
            "mode": "lineage"
        }),
    );
    assert!(lineage_res["error"].is_null());
    let lineage_text = lineage_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        lineage_text.contains("Lineage") || lineage_text.contains("decision-payment-status-events"),
        "Lineage query must return lineage packet"
    );

    // Mode: impact
    let impact_res = client.call_tool(
        "akashic_query",
        json!({
            "seed": "concept-payment-status",
            "mode": "impact"
        }),
    );
    assert!(impact_res["error"].is_null());
    let impact_text = impact_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        impact_text.contains("Impact") || impact_text.contains("concept-payment-status"),
        "Impact query must return impact packet"
    );

    // Mode: historical
    let historical_res = client.call_tool(
        "akashic_query",
        json!({
            "seed": "decision-payment-status-events",
            "mode": "historical",
            "as_of": "2026-06-01"
        }),
    );
    assert!(historical_res["error"].is_null());
    let historical_text = historical_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        historical_text.contains("Historical")
            || historical_text.contains("decision-payment-status-events"),
        "Historical query must return historical packet"
    );
}

/// 4. Conflict abstention for cycles
#[test]
fn test_mcp_query_conflict_abstention() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);
    client.initialize();

    // Query on cycle canary
    let cycle_res = client.call_tool(
        "akashic_query",
        json!({
            "seed": "cycle-canary-a",
            "mode": "current"
        }),
    );

    assert!(
        cycle_res["error"].is_null(),
        "Query must not crash on conflict"
    );
    let cycle_text = cycle_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        cycle_text.contains("[UNRESOLVED_CONFLICT]"),
        "Expected [UNRESOLVED_CONFLICT] marker in output for cycle: {cycle_text}"
    );
}

/// 5. Structural inspection and linting
#[test]
fn test_mcp_structural_inspection_and_linting() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);
    client.initialize();

    // Inspect vault-wide inventory (target omitted)
    let inv_res = client.call_tool("akashic_inspect", json!({}));
    assert!(inv_res["error"].is_null());
    let inv_text = inv_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(inv_text.contains("Vault Inventory"));
    assert!(inv_text.contains("Total Nodes"));
    assert!(inv_text.contains("Total Edges"));

    // Inspect specific node (target provided)
    let node_res = client.call_tool(
        "akashic_inspect",
        json!({
            "target": "decision-payment-status-events"
        }),
    );
    assert!(node_res["error"].is_null());
    let node_text = node_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(node_text.contains("decision-payment-status-events"));
    assert!(node_text.contains("Forward Relations"));
    assert!(node_text.contains("Inverse Relations"));

    // Lint
    let lint_res = client.call_tool("akashic_lint", json!({}));
    assert!(lint_res["error"].is_null());
    let lint_text = lint_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(
        lint_text.contains("violations found")
            || lint_text.to_lowercase().contains("violations")
            || lint_text.contains("Vault is healthy")
            || lint_text.contains("Integrity Violations"),
        "Lint report expected: {lint_text}"
    );
}

/// 6. vault_read with valid note and path traversal rejection
#[test]
fn test_mcp_vault_read_and_path_traversal() {
    let vault = vault_fixture_path();
    let mut client = McpTestClient::spawn(&vault);
    client.initialize();

    // Valid read
    let read_res = client.call_tool(
        "vault_read",
        json!({
            "node_id": "concept-payment-status"
        }),
    );
    assert!(read_res["error"].is_null());
    let read_text = read_res["result"]["content"][0]["text"]
        .as_str()
        .expect("Text content");
    assert!(read_text.contains("concept-payment-status"));
    assert!(read_text.contains("Payment Status Domain Model"));

    // Path traversal with ".."
    let traversal_res = client.call_tool(
        "vault_read",
        json!({
            "node_id": "../../etc/passwd"
        }),
    );
    assert!(
        !traversal_res["error"].is_null() || traversal_res["result"]["isError"] == true,
        "Path traversal must be rejected"
    );

    // Absolute path traversal
    let abs_res = client.call_tool(
        "vault_read",
        json!({
            "node_id": "/etc/passwd"
        }),
    );
    assert!(
        !abs_res["error"].is_null() || abs_res["result"]["isError"] == true,
        "Absolute path outside vault must be rejected"
    );
}

/// 7. vault_write_canonical atomic write and daily note append
#[test]
fn test_mcp_vault_write_canonical_and_daily_append() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "decision-mcp-canonical",
            "title": "Adopt MCP Stdio Server",
            "type": "Decision",
            "relations": [
                {
                    "type": "relates_to",
                    "target": "concept-payment-status"
                }
            ],
            "body": "## Context\nWe adopt standard MCP JSON-RPC protocol over stdio.",
            "daily_summary": "Documented decision to adopt MCP stdio interface"
        }),
    );

    assert!(
        write_res["error"].is_null(),
        "Write canonical must succeed: {:?}",
        write_res
    );

    // Verify canonical note on disk
    let note_path = temp.path().join("decision-mcp-canonical.md");
    assert!(note_path.exists(), "Canonical note file must exist on disk");
    let content = fs::read_to_string(&note_path).unwrap();
    assert!(content.contains("decision-mcp-canonical"));
    assert!(content.contains("Adopt MCP Stdio Server"));
    assert!(content.contains("relates_to"));
    assert!(content.contains("We adopt standard MCP JSON-RPC protocol over stdio"));

    // Verify daily note append
    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let daily_path = temp.path().join("Daily").join(format!("{today_str}.md"));
    assert!(daily_path.exists(), "Daily log note must exist on disk");
    let daily_content = fs::read_to_string(&daily_path).unwrap();
    assert!(
        daily_content.contains(
            "- [[decision-mcp-canonical]]: Documented decision to adopt MCP stdio interface"
        ),
        "Daily log must contain formatted summary entry: {daily_content}"
    );
}

/// 8. Rejection of inverse relations
#[test]
fn test_mcp_rejection_of_inverse_relations() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "decision-illegal-inverse",
            "title": "Illegal Inverse Relation",
            "type": "Decision",
            "relations": [
                {
                    "type": "superseded_by",
                    "target": "decision-payment-status-events"
                }
            ],
            "body": "This write must fail because superseded_by is an inverse relation.",
            "daily_summary": "Attempt illegal inverse write"
        }),
    );

    assert!(
        !write_res["error"].is_null() || write_res["result"]["isError"] == true,
        "Inverse relations must be rejected: {:?}",
        write_res
    );

    let note_path = temp.path().join("decision-illegal-inverse.md");
    assert!(
        !note_path.exists(),
        "Illegal note must not be written to disk"
    );
}

/// 9. Rejection of invalid OKF types
#[test]
fn test_mcp_rejection_of_invalid_okf_types() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "invalid-type-note",
            "title": "Invalid Type Note",
            "type": "NonExistentType",
            "relations": [],
            "body": "This write must fail because of invalid OKF type.",
            "daily_summary": "Attempt invalid type write"
        }),
    );

    assert!(
        !write_res["error"].is_null() || write_res["result"]["isError"] == true,
        "Invalid OKF type must be rejected: {:?}",
        write_res
    );

    let note_path = temp.path().join("invalid-type-note.md");
    assert!(
        !note_path.exists(),
        "Invalid type note must not be written to disk"
    );
}

/// 10. Rejection of supersession cycles
#[test]
fn test_mcp_rejection_of_supersession_cycles() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    // Self-supersession cycle
    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "self-superseder",
            "title": "Self Superseding Note",
            "type": "Decision",
            "relations": [
                {
                    "type": "supersedes",
                    "target": "self-superseder"
                }
            ],
            "body": "This write must fail because of self supersession.",
            "daily_summary": "Attempt self supersession write"
        }),
    );

    assert!(
        !write_res["error"].is_null() || write_res["result"]["isError"] == true,
        "Self supersession cycle must be rejected: {:?}",
        write_res
    );

    let note_path = temp.path().join("self-superseder.md");
    assert!(
        !note_path.exists(),
        "Self-cycle note must not be written to disk"
    );
}

/// 11. Concurrent write locking serialization
#[test]
fn test_mcp_concurrent_write_locking() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let lock_path = temp.path().join(".vault.lock");
    let lock_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();

    // Acquire lock externally to simulate concurrent write
    lock_file.lock_exclusive().unwrap();

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let (tx, rx) = std::sync::mpsc::channel();
    let temp_path = temp.path().to_path_buf();

    // Spawn thread that releases the lock after a short delay
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        lock_file.unlock().unwrap();
        tx.send(()).unwrap();
    });

    // Write should succeed after lock is released
    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "decision-serialized-lock",
            "title": "Serialized Lock Decision",
            "type": "Decision",
            "relations": [],
            "body": "Body for serialized lock test.",
            "daily_summary": "Serialized lock test"
        }),
    );

    assert!(
        write_res["error"].is_null(),
        "Write canonical must serialize and succeed: {:?}",
        write_res
    );
    rx.recv_timeout(Duration::from_secs(2))
        .expect("Lock release signal received");

    let note_path = temp_path.join("decision-serialized-lock.md");
    assert!(note_path.exists(), "Serialized note must exist on disk");
}

/// 12. Rejection of broken relation targets
#[test]
fn test_mcp_rejection_of_broken_targets() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "decision-broken-target",
            "title": "Broken Target Decision",
            "type": "Decision",
            "relations": [
                {
                    "type": "relates_to",
                    "target": "non-existent-note-target"
                }
            ],
            "body": "This write must fail because the target does not exist.",
            "daily_summary": "Attempt broken target write"
        }),
    );

    assert!(
        !write_res["error"].is_null() || write_res["result"]["isError"] == true,
        "Broken relation targets must be rejected: {:?}",
        write_res
    );

    let note_path = temp.path().join("decision-broken-target.md");
    assert!(
        !note_path.exists(),
        "Note with broken target must not be written to disk"
    );
}

/// 13. Atomic rollback when daily note creation fails
#[test]
fn test_mcp_write_canonical_atomic_rollback_on_daily_failure() {
    let temp = tempdir().unwrap();
    copy_dir_all(&vault_fixture_path(), temp.path());

    // Create a regular file named "Daily" so that creating Daily/ directory fails
    let daily_blocker = temp.path().join("Daily");
    fs::write(&daily_blocker, "regular file blocking directory creation").unwrap();

    let mut client = McpTestClient::spawn(temp.path());
    client.initialize();

    let write_res = client.call_tool(
        "vault_write_canonical",
        json!({
            "id": "decision-rollback-test",
            "title": "Rollback Test Decision",
            "type": "Decision",
            "relations": [],
            "body": "This write must roll back because Daily cannot be created.",
            "daily_summary": "Attempt write that triggers rollback"
        }),
    );

    assert!(
        !write_res["error"].is_null() || write_res["result"]["isError"] == true,
        "Write must return error when daily note creation fails: {:?}",
        write_res
    );

    let note_path = temp.path().join("decision-rollback-test.md");
    assert!(
        !note_path.exists(),
        "Canonical note must be rolled back (deleted) when daily write fails"
    );
}
