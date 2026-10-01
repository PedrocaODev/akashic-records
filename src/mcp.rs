use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use chrono::Local;
use fs2::FileExt;
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, ErrorData},
    tool, tool_router, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::VaultGraph;
use crate::model::{NodeMetadata, Relation, RelationType};
use crate::query::{query_current, query_historical, query_impact, query_lineage, QueryError};

/// Permitted OKF note types.
pub const PERMITTED_OKF_TYPES: &[&str] = &[
    "Concept",
    "Decision",
    "Incident",
    "Runbook",
    "Evidence",
    "Proposal",
    "Review",
    "Source",
    "Note",
    "Architecture",
    "Infrastructure",
    "Service",
    "Application",
];

/// Parameters for `akashic_query` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AkashicQueryParams {
    /// Note identifier or title to query around.
    pub seed: String,
    /// Query mode: 'current', 'lineage', 'impact', or 'historical' (default: 'current').
    #[serde(default)]
    pub mode: Option<String>,
    /// Optional context scope key-value pairs (e.g. env=prod, phase=2).
    #[serde(default)]
    pub scope: Option<BTreeMap<String, String>>,
    /// Historical cutoff date (YYYY-MM-DD), valid only for historical mode.
    #[serde(default)]
    pub as_of: Option<String>,
}

/// Parameters for `akashic_inspect` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AkashicInspectParams {
    /// Note identifier or relative path. If omitted, returns vault-wide inventory.
    #[serde(default)]
    pub target: Option<String>,
}

/// Parameters for `akashic_lint` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AkashicLintParams {}

/// Parameters for `vault_read` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VaultReadParams {
    /// Note identifier or relative path within the vault.
    pub node_id: String,
}

/// Relation specification for `vault_write_canonical`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteRelationParam {
    /// Canonical relation type: `supersedes`, `depends_on`, `supported_by`, `caused_by`, `relates_to`, `contradicts`.
    #[serde(rename = "type")]
    pub relation_type: String,
    /// Target note identifier.
    pub target: String,
    /// Optional citation or rationale evidence.
    #[serde(default)]
    pub evidence: Option<String>,
    /// Optional scope qualification key-value pairs.
    #[serde(default)]
    pub scope: Option<BTreeMap<String, String>>,
}

/// Parameters for `vault_write_canonical` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VaultWriteCanonicalParams {
    /// Canonical note identifier (e.g. decision-use-rust).
    pub id: String,
    /// Note title.
    pub title: String,
    /// OKF note type (e.g. Decision, Concept, Note, etc.).
    #[serde(rename = "type")]
    pub node_type: String,
    /// Forward relations to other notes.
    #[serde(default)]
    pub relations: Vec<WriteRelationParam>,
    /// Note Markdown body (excluding frontmatter).
    pub body: String,
    /// Summary line to append to today's daily log note.
    pub daily_summary: String,
}

struct AdvisoryLockGuard(std::fs::File);

impl Drop for AdvisoryLockGuard {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn parse_relation_type(s: &str) -> RelationType {
    match s {
        "supersedes" => RelationType::Supersedes,
        "superseded_by" => RelationType::SupersededBy,
        "depends_on" => RelationType::DependsOn,
        "required_by" => RelationType::RequiredBy,
        "supported_by" => RelationType::SupportedBy,
        "supports" => RelationType::Supports,
        "contradicts" => RelationType::Contradicts,
        "caused_by" => RelationType::CausedBy,
        "caused" => RelationType::Caused,
        "relates_to" => RelationType::RelatesTo,
        other => RelationType::Custom(other.to_string()),
    }
}

/// Akashic Model Context Protocol (MCP) server.
#[derive(Debug, Clone)]
pub struct AkashicMcpServer {
    /// Path to the vault root directory.
    pub vault_path: PathBuf,
}

impl AkashicMcpServer {
    /// Creates a new `AkashicMcpServer` for the specified vault path.
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            vault_path: vault_path.into(),
        }
    }

    /// Verifies that the configured vault directory exists on disk.
    pub fn ensure_vault_exists(&self) -> Result<(), ErrorData> {
        if !self.vault_path.exists() || !self.vault_path.is_dir() {
            Err(ErrorData::invalid_params(
                format!(
                    "Vault directory does not exist: {}",
                    self.vault_path.display()
                ),
                None,
            ))
        } else {
            Ok(())
        }
    }
}

#[tool_router]
impl AkashicMcpServer {
    /// Query the knowledge vault graph across current guidance, decision lineage, impact analysis, or historical states.
    #[tool(
        name = "akashic_query",
        description = "Query the knowledge vault graph across current guidance, decision lineage, impact analysis, or historical states."
    )]
    pub async fn akashic_query(
        &self,
        Parameters(params): Parameters<AkashicQueryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.ensure_vault_exists()?;

        let vault = VaultGraph::from_dir(&self.vault_path).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to load vault graph: {e}"), None)
        })?;

        let mode_str = params.mode.as_deref().unwrap_or("current").to_lowercase();
        let scope = params.scope.unwrap_or_default();

        let markdown_res: Result<String, QueryError> = match mode_str.as_str() {
            "current" => {
                query_current(&vault, &params.seed, &scope).map(|packet| packet.to_markdown())
            }
            "lineage" => {
                query_lineage(&vault, &params.seed, &scope).map(|packet| packet.to_markdown())
            }
            "impact" => {
                query_impact(&vault, &params.seed, &scope).map(|packet| packet.to_markdown())
            }
            "historical" => {
                let as_of = params.as_of.as_deref();
                query_historical(&vault, &params.seed, as_of, &scope)
                    .map(|packet| packet.to_markdown())
            }
            other => {
                return Err(ErrorData::invalid_params(
                    format!(
                        "Invalid query mode: '{other}'. Valid modes are 'current', 'lineage', 'impact', 'historical'."
                    ),
                    None,
                ));
            }
        };

        match markdown_res {
            Ok(markdown) => Ok(CallToolResult::success(vec![ContentBlock::text(markdown)])),
            Err(QueryError::UnresolvedConflict(msg)) | Err(QueryError::SupersessionCycle(msg)) => {
                let markdown = format!(
                    "# Akashic Context Packet: Conflict Detected\n\n[UNRESOLVED_CONFLICT] {}\n",
                    msg
                );
                Ok(CallToolResult::success(vec![ContentBlock::text(markdown)]))
            }
            Err(e) => Err(ErrorData::invalid_params(e.to_string(), None)),
        }
    }

    /// Inspect vault topology. When target is omitted, returns vault-wide inventory and relation counts. When target is provided, returns complete metadata, forward relations, and computed inverse edges.
    #[tool(
        name = "akashic_inspect",
        description = "Inspect vault topology. When target is omitted, returns vault-wide inventory and relation counts. When target is provided, returns complete metadata, forward relations, and computed inverse edges."
    )]
    pub async fn akashic_inspect(
        &self,
        Parameters(params): Parameters<AkashicInspectParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.ensure_vault_exists()?;

        let vault = VaultGraph::from_dir(&self.vault_path).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to load vault graph: {e}"), None)
        })?;

        match params.target.as_deref() {
            None => {
                let inventory = vault.inventory();
                let mut out = String::new();
                out.push_str("# Vault Inventory\n\n");
                out.push_str(&format!("- **Total Nodes**: {}\n", inventory.total_nodes));
                out.push_str(&format!("- **Total Edges**: {}\n", inventory.total_edges));
                out.push_str("\n## Relations by Type\n");
                for (rel_type, count) in &inventory.edges_by_type {
                    out.push_str(&format!("- {}: {}\n", rel_type, count));
                }
                if !inventory.malformed_files.is_empty() {
                    out.push_str("\n## Malformed Files\n");
                    for malformed in &inventory.malformed_files {
                        out.push_str(&format!("- {}: {}\n", malformed.path, malformed.error));
                    }
                }
                Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
            }
            Some(target) => {
                let node_idx = vault.resolve_seed(target).ok_or_else(|| {
                    ErrorData::invalid_params(format!("Node not found: '{target}'"), None)
                })?;
                let node = &vault.graph[node_idx];
                let inverse_edges = vault.inverse_relations(&node.id).unwrap_or_default();

                let mut out = String::new();
                out.push_str(&format!("# Akashic Inspect: {}\n\n", node.id));
                out.push_str(&format!("- **Title**: {}\n", node.title));
                out.push_str(&format!("- **Type**: {}\n", node.node_type));
                if let Some(status) = &node.status {
                    out.push_str(&format!("- **Status**: {}\n", status));
                }
                if let Some(path) = &node.path {
                    out.push_str(&format!("- **Path**: {}\n", path));
                }
                if let Some(valid_from) = &node.valid_from {
                    out.push_str(&format!("- **Valid From**: {}\n", valid_from));
                }
                if let Some(valid_until) = &node.valid_until {
                    out.push_str(&format!("- **Valid Until**: {}\n", valid_until));
                }
                if !node.scope.is_empty() {
                    let scope_pairs: Vec<String> =
                        node.scope.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    out.push_str(&format!("- **Scope**: {}\n", scope_pairs.join(", ")));
                }

                out.push_str(&format!(
                    "\n## Forward Relations ({})\n",
                    node.relations.len()
                ));
                if node.relations.is_empty() {
                    out.push_str("_None_\n");
                } else {
                    for rel in &node.relations {
                        let mut extra = Vec::new();
                        if let Some(ev) = &rel.evidence {
                            extra.push(format!("evidence: \"{ev}\""));
                        }
                        if let Some(sc) = &rel.scope {
                            if !sc.is_empty() {
                                let sc_strs: Vec<String> =
                                    sc.iter().map(|(k, v)| format!("{k}={v}")).collect();
                                extra.push(format!("scope: [{}]", sc_strs.join(", ")));
                            }
                        }
                        if extra.is_empty() {
                            out.push_str(&format!(
                                "- {} -> [[{}]]\n",
                                rel.relation_type, rel.target
                            ));
                        } else {
                            out.push_str(&format!(
                                "- {} -> [[{}]] ({})\n",
                                rel.relation_type,
                                rel.target,
                                extra.join(", ")
                            ));
                        }
                    }
                }

                out.push_str(&format!(
                    "\n## Inverse Relations ({})\n",
                    inverse_edges.len()
                ));
                if inverse_edges.is_empty() {
                    out.push_str("_None_\n");
                } else {
                    for rel in &inverse_edges {
                        out.push_str(&format!("- {} -> [[{}]]\n", rel.relation_type, rel.target));
                    }
                }

                Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
            }
        }
    }

    /// Run topological knowledge graph linting to detect broken targets, supersession cycles, and integrity violations.
    #[tool(
        name = "akashic_lint",
        description = "Run topological knowledge graph linting to detect broken targets, supersession cycles, and integrity violations."
    )]
    pub async fn akashic_lint(
        &self,
        Parameters(_): Parameters<AkashicLintParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.ensure_vault_exists()?;

        let vault = VaultGraph::from_dir(&self.vault_path).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to load vault graph: {e}"), None)
        })?;

        let report = vault.lint();
        let formatted = crate::linter::format_lint_human(&report);
        Ok(CallToolResult::success(vec![ContentBlock::text(formatted)]))
    }

    /// Read frontmatter and raw Markdown body of a canonical note within the vault. Rejects path traversal escaping the vault.
    #[tool(
        name = "vault_read",
        description = "Read frontmatter and raw Markdown body of a canonical note within the vault. Rejects path traversal escaping the vault."
    )]
    pub async fn vault_read(
        &self,
        Parameters(params): Parameters<VaultReadParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.ensure_vault_exists()?;

        let node_id = params.node_id.trim();

        if node_id.contains("..") || node_id.starts_with('/') || node_id.starts_with('\\') {
            return Err(ErrorData::invalid_params(
                format!("Path traversal forbidden: '{node_id}' contains '..' or leading slash"),
                None,
            ));
        }

        let canonical_vault = self.vault_path.canonicalize().map_err(|e| {
            ErrorData::invalid_params(format!("Failed to canonicalize vault path: {e}"), None)
        })?;

        let read_sandboxed_file = |path: &std::path::Path| -> Result<CallToolResult, ErrorData> {
            let canonical_target = path.canonicalize().map_err(|e| {
                ErrorData::invalid_params(format!("Failed to resolve path '{node_id}': {e}"), None)
            })?;

            if !canonical_target.starts_with(&canonical_vault) {
                return Err(ErrorData::invalid_params(
                    format!("Access denied: path '{node_id}' escapes vault directory"),
                    None,
                ));
            }

            let content = fs::read_to_string(&canonical_target).map_err(|e| {
                ErrorData::invalid_params(format!("Failed to read file: {e}"), None)
            })?;

            Ok(CallToolResult::success(vec![ContentBlock::text(content)]))
        };

        // Direct candidate check on disk before doing expensive VaultGraph::from_dir
        let direct_candidate = if node_id.ends_with(".md") || node_id.ends_with(".markdown") {
            self.vault_path.join(node_id)
        } else {
            let with_md = self.vault_path.join(format!("{node_id}.md"));
            if with_md.is_file() {
                with_md
            } else {
                self.vault_path.join(node_id)
            }
        };

        if direct_candidate.is_file() {
            return read_sandboxed_file(&direct_candidate);
        }

        // Fall back to VaultGraph resolution if direct candidate file doesn't exist
        let vault = VaultGraph::from_dir(&self.vault_path).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to load vault graph: {e}"), None)
        })?;

        let resolved_file_path = if let Some(idx) = vault.resolve_seed(node_id) {
            let node = &vault.graph[idx];
            node.path.as_ref().map(|p| self.vault_path.join(p))
        } else {
            None
        };

        let target_path = resolved_file_path.ok_or_else(|| {
            ErrorData::invalid_params(format!("Note not found: '{node_id}'"), None)
        })?;

        read_sandboxed_file(&target_path)
    }

    /// Atomically writes a canonical note and updates the daily log under advisory lock.
    #[tool(
        name = "vault_write_canonical",
        description = "Atomically write a canonical note with strict OKF schema validation and append a linked summary to today's daily log note under an advisory lock."
    )]
    pub async fn vault_write_canonical(
        &self,
        Parameters(params): Parameters<VaultWriteCanonicalParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.ensure_vault_exists()?;

        // a) Advisory lock on .vault.lock
        let git_dir = self.vault_path.join(".git");
        let lock_path = if git_dir.is_dir() {
            git_dir.join(".vault.lock")
        } else {
            self.vault_path.join(".vault.lock")
        };

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| {
                ErrorData::invalid_params(format!("Failed to open lock file: {e}"), None)
            })?;

        lock_file.lock_exclusive().map_err(|e| {
            ErrorData::invalid_params(
                format!("Failed to acquire advisory lock on vault: {e}"),
                None,
            )
        })?;

        let _guard = AdvisoryLockGuard(lock_file);

        // b) Validate OKF type against permitted types
        if !PERMITTED_OKF_TYPES.contains(&params.node_type.as_str()) {
            return Err(ErrorData::invalid_params(
                format!(
                    "Invalid OKF type '{}'. Permitted types: {}",
                    params.node_type,
                    PERMITTED_OKF_TYPES.join(", ")
                ),
                None,
            ));
        }

        // c) Reject inverse relations
        let mut parsed_relations = Vec::new();
        for r in &params.relations {
            let rel_type = parse_relation_type(&r.relation_type);
            if rel_type.is_inverse() {
                return Err(ErrorData::invalid_params(
                    format!(
                        "Invalid relation type '{}'. Inverse relations are dynamic and cannot be stored in note frontmatter.",
                        r.relation_type
                    ),
                    None,
                ));
            }
            parsed_relations.push(Relation {
                relation_type: rel_type,
                target: r.target.clone(),
                evidence: r.evidence.clone(),
                scope: r.scope.clone(),
            });
        }

        let note_id = params.id.strip_suffix(".md").unwrap_or(&params.id).trim();
        if note_id.is_empty()
            || note_id.contains('/')
            || note_id.contains('\\')
            || note_id.contains("..")
        {
            return Err(ErrorData::invalid_params(
                format!("Invalid note id: '{note_id}' cannot contain path separators or '..'"),
                None,
            ));
        }

        // Preflight step 3: Validate prospective relations against in-memory graph
        for r in &parsed_relations {
            if r.relation_type == RelationType::Supersedes && r.target == note_id {
                return Err(ErrorData::invalid_params(
                    format!(
                        "Supersession cycle detected: note '{note_id}' cannot supersede itself"
                    ),
                    None,
                ));
            }
        }

        if !parsed_relations.is_empty() {
            let vault = VaultGraph::from_dir(&self.vault_path).map_err(|e| {
                ErrorData::invalid_params(format!("Failed to load vault graph: {e}"), None)
            })?;

            // Validate relation targets: verify that targets exist in graph (or on disk)
            for r in &parsed_relations {
                let target_clean = r.target.trim();
                let source_path = format!("{note_id}.md");
                let target_exists = vault.resolve_target(&source_path, target_clean).is_some()
                    || vault.resolve_seed(target_clean).is_some()
                    || self.vault_path.join(target_clean).is_file()
                    || self.vault_path.join(format!("{target_clean}.md")).is_file();

                if !target_exists {
                    return Err(ErrorData::invalid_params(
                        format!(
                            "Write rejected: relation target '{}' does not resolve to any note in the vault",
                            r.target
                        ),
                        None,
                    ));
                }
            }

            // Validate prospective relations don't introduce supersession cycles
            let has_supersedes = parsed_relations
                .iter()
                .any(|r| r.relation_type == RelationType::Supersedes);

            if has_supersedes {
                let mut scc_graph = DiGraph::<String, ()>::new();
                let mut node_indices = HashMap::<String, NodeIndex>::new();

                for edge in vault.graph.edge_references() {
                    if edge.weight().relation_type == RelationType::Supersedes {
                        let src = &vault.graph[edge.source()].id;
                        let dst = &vault.graph[edge.target()].id;
                        if src != note_id {
                            let s_idx = *node_indices
                                .entry(src.clone())
                                .or_insert_with(|| scc_graph.add_node(src.clone()));
                            let d_idx = *node_indices
                                .entry(dst.clone())
                                .or_insert_with(|| scc_graph.add_node(dst.clone()));
                            scc_graph.add_edge(s_idx, d_idx, ());
                        }
                    }
                }

                for r in &parsed_relations {
                    if r.relation_type == RelationType::Supersedes {
                        let s_idx = *node_indices
                            .entry(note_id.to_string())
                            .or_insert_with(|| scc_graph.add_node(note_id.to_string()));
                        let d_idx = *node_indices
                            .entry(r.target.clone())
                            .or_insert_with(|| scc_graph.add_node(r.target.clone()));
                        scc_graph.add_edge(s_idx, d_idx, ());
                    }
                }

                let sccs = tarjan_scc(&scc_graph);
                for scc in sccs {
                    if scc.len() > 1 {
                        let cycle_nodes: Vec<String> =
                            scc.iter().map(|idx| scc_graph[*idx].clone()).collect();
                        return Err(ErrorData::invalid_params(
                            format!(
                                "Write rejected: prospective relations introduce a supersession cycle: {}",
                                cycle_nodes.join(" -> ")
                            ),
                            None,
                        ));
                    } else if scc.len() == 1 && scc_graph.contains_edge(scc[0], scc[0]) {
                        return Err(ErrorData::invalid_params(
                            format!(
                                "Write rejected: prospective relations introduce a self-cycle on '{}'",
                                scc_graph[scc[0]]
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        // e) Atomic write of canonical note (<vault>/<id>.md) via temp file rename
        let meta = NodeMetadata {
            id: note_id.to_string(),
            node_type: params.node_type.clone(),
            title: params.title.clone(),
            status: None,
            valid_from: None,
            valid_until: None,
            scope: None,
            relations: parsed_relations,
        };

        let frontmatter = serde_yaml::to_string(&meta).map_err(|e| {
            ErrorData::invalid_params(
                format!("Failed to serialize note metadata to YAML: {e}"),
                None,
            )
        })?;

        let canonical_content = format!("---\n{}---\n\n{}\n", frontmatter, params.body.trim());
        let target_file_path = self.vault_path.join(format!("{note_id}.md"));

        // Capture previous canonical content for atomic rollback
        let previous_canonical_content = if target_file_path.exists() {
            fs::read(&target_file_path).ok()
        } else {
            None
        };

        let rollback_canonical = || {
            if let Some(ref prev) = previous_canonical_content {
                let _ = fs::write(&target_file_path, prev);
            } else {
                let _ = fs::remove_file(&target_file_path);
            }
        };

        let temp_file_path = self
            .vault_path
            .join(format!(".{note_id}.tmp.{}", std::process::id()));
        fs::write(&temp_file_path, canonical_content).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to write temporary file: {e}"), None)
        })?;

        if let Err(e) = fs::rename(&temp_file_path, &target_file_path) {
            let _ = fs::remove_file(&temp_file_path);
            return Err(ErrorData::invalid_params(
                format!(
                    "Failed to atomically write file to {}: {e}",
                    target_file_path.display()
                ),
                None,
            ));
        }

        // f) Ensure today's Daily note exists (Daily/YYYY-MM-DD.md)
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        let daily_dir = self.vault_path.join("Daily");
        if let Err(e) = fs::create_dir_all(&daily_dir) {
            rollback_canonical();
            return Err(ErrorData::invalid_params(
                format!("Failed to create Daily directory: {e}"),
                None,
            ));
        }

        let daily_file_path = daily_dir.join(format!("{today_str}.md"));
        let daily_entry = format!("- [[{note_id}]]: {}\n", params.daily_summary.trim());

        let daily_result = if daily_file_path.exists() {
            match fs::read_to_string(&daily_file_path) {
                Ok(mut existing) => {
                    if !existing.is_empty() && !existing.ends_with('\n') {
                        existing.push('\n');
                    }
                    existing.push_str(&daily_entry);
                    fs::write(&daily_file_path, existing)
                }
                Err(e) => Err(e),
            }
        } else {
            let template = format!("# {today_str}\n\n## Log\n\n{daily_entry}");
            fs::write(&daily_file_path, template)
        };

        if let Err(e) = daily_result {
            rollback_canonical();
            return Err(ErrorData::invalid_params(
                format!("Failed to update daily log file: {e}"),
                None,
            ));
        }

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Successfully wrote canonical note '{note_id}' and updated daily log Daily/{today_str}.md"
        ))]))
    }
}

#[rmcp::tool_handler(
    name = "akashic",
    version = "0.2.0",
    instructions = "Akashic Records knowledge graph server"
)]
impl rmcp::ServerHandler for AkashicMcpServer {}

/// Runs the Akashic MCP server on stdio for the specified vault path until shutdown signal.
pub async fn run_mcp_server(
    vault_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = AkashicMcpServer::new(vault_path);
    let running = server.serve(rmcp::transport::io::stdio()).await?;

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            res = running.waiting() => {
                if let Err(e) = res {
                    eprintln!("MCP server stopped with join error: {e}");
                }
            }
            _ = sigterm.recv() => {
                eprintln!("Akashic MCP server received SIGTERM, shutting down.");
            }
            _ = sigint.recv() => {
                eprintln!("Akashic MCP server received SIGINT, shutting down.");
            }
        }
    }

    #[cfg(not(unix))]
    {
        tokio::select! {
            res = running.waiting() => {
                if let Err(e) = res {
                    eprintln!("MCP server stopped with join error: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Akashic MCP server received Ctrl+C, shutting down.");
            }
        }
    }

    Ok(())
}
