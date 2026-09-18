use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

use crate::graph::VaultGraph;
use crate::model::{Node, Plan, VaultInventory};
use crate::parser::parse_note_file;
use crate::planner::{apply_plan, plan_vault};
use crate::query::{
    parse_scope_args, query_current, query_historical, query_impact, query_lineage, QueryError,
};

#[derive(Parser, Debug)]
#[command(
    name = "akashic",
    about = "Reader-agnostic Markdown knowledge graph engine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect a single Markdown note or directory
    Inspect {
        /// Path to the Markdown file or directory to inspect
        path: PathBuf,

        /// Output structured JSON representation
        #[arg(long)]
        json: bool,
    },
    /// Lint knowledge graph topology for integrity violations and cycles
    Lint {
        /// Path to the vault directory to lint
        path: PathBuf,

        /// Output structured JSON representation
        #[arg(long)]
        json: bool,
    },

    /// Query the knowledge graph for current guidance, decision lineage, impact analysis, or historical state
    Query {
        /// Path to the vault directory
        vault: PathBuf,

        /// Seed note ID or path
        #[arg(long)]
        seed: String,

        /// Query mode ('current', 'lineage', 'impact', 'historical')
        #[arg(long, default_value = "current", value_parser = parse_query_mode)]
        mode: QueryMode,

        /// Point-in-time date (YYYY-MM-DD) for historical query mode
        #[arg(long = "as-of")]
        as_of: Option<String>,

        /// Scope filter key-value pair (e.g. --scope env=production)
        #[arg(long = "scope", action = clap::ArgAction::Append)]
        scope: Vec<String>,

        /// Output structured JSON representation
        #[arg(long)]
        json: bool,
    },

    /// Inspect vault and generate an idempotent migration plan with unified diffs
    Plan {
        /// Path to the vault directory
        vault: PathBuf,

        /// Optional output file path for the reviewable JSON plan
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Apply an approved migration plan to a vault idempotently
    Apply {
        /// Path to the vault directory
        vault: PathBuf,

        /// Path to the plan JSON file to apply
        #[arg(long)]
        plan: PathBuf,

        /// Validate hashes and display planned changes without modifying any files
        #[arg(long)]
        dry_run: bool,
    },

    /// Export vault to an external format
    Export {
        /// Path to the vault directory to export
        vault: PathBuf,

        /// Export format (currently 'okf')
        #[arg(long, default_value = "okf")]
        format: String,

        /// Output directory for exported files
        #[arg(long)]
        out: PathBuf,

        /// Output manifest JSON to stdout
        #[arg(long)]
        json: bool,
    },
}

/// Formats a Node into a human-readable summary string.
pub fn format_human(node: &Node) -> String {
    let mut out = String::new();
    out.push_str(&format!("Node: {}\n", node.id));
    out.push_str(&format!("Type: {}\n", node.node_type));
    out.push_str(&format!("Title: {}\n", node.title));

    if let Some(ref status) = node.status {
        out.push_str(&format!("Status: {}\n", status));
    }
    if let Some(ref vf) = node.valid_from {
        out.push_str(&format!("Valid From: {}\n", vf));
    }
    if let Some(ref vu) = node.valid_until {
        out.push_str(&format!("Valid Until: {}\n", vu));
    }

    if !node.scope.is_empty() {
        out.push_str("Scope:\n");
        for (k, v) in &node.scope {
            out.push_str(&format!("  {}: {}\n", k, v));
        }
    }

    if !node.relations.is_empty() {
        out.push_str(&format!("Relations ({}):\n", node.relations.len()));
        for rel in &node.relations {
            let mut parts = Vec::new();
            if let Some(ref ev) = rel.evidence {
                parts.push(format!("evidence: {}", ev));
            }
            if let Some(ref sc) = rel.scope {
                let sc_str = sc
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                parts.push(format!("scope: [{}]", sc_str));
            }

            let detail = if parts.is_empty() {
                String::new()
            } else {
                format!(" ({})", parts.join(", "))
            };

            out.push_str(&format!(
                "  - {} -> {}{}\n",
                rel.relation_type, rel.target, detail
            ));
        }
    }

    out
}

/// Formats a VaultInventory into a human-readable summary string.
pub fn format_inventory_human(inv: &VaultInventory) -> String {
    let mut out = String::new();
    out.push_str(&format!("Vault: {}\n", inv.vault_path));
    out.push_str(&format!("Total Nodes: {}\n", inv.total_nodes));
    out.push_str(&format!("Total Edges: {}\n", inv.total_edges));

    if !inv.edges_by_type.is_empty() {
        out.push_str("Edges by Type:\n");
        for (rel_type, count) in &inv.edges_by_type {
            out.push_str(&format!("  - {}: {}\n", rel_type, count));
        }
    }

    if !inv.malformed_files.is_empty() {
        out.push_str(&format!(
            "Malformed Files ({}):\n",
            inv.malformed_files.len()
        ));
        for malformed in &inv.malformed_files {
            out.push_str(&format!("  - {}: {}\n", malformed.path, malformed.error));
        }
    }

    out
}

/// Executes the inspect subcommand against a given path (note file or vault directory).
pub fn run_inspect(path: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("File or directory not found: {}", path.display()).into());
    }

    if path.is_dir() {
        let vault = VaultGraph::from_dir(path)?;
        let inventory = vault.inventory();

        if json {
            let serialized = serde_json::to_string_pretty(&inventory)?;
            println!("{}", serialized);
        } else {
            print!("{}", format_inventory_human(&inventory));
        }
    } else {
        let node = parse_note_file(path)?;

        if json {
            let serialized = serde_json::to_string_pretty(&node)?;
            println!("{}", serialized);
        } else {
            print!("{}", format_human(&node));
        }
    }

    Ok(())
}

/// Executes the lint subcommand against a given vault directory.
/// Returns Ok(true) if the vault is healthy, Ok(false) if integrity violations exist.
pub fn run_lint(path: &Path, json: bool) -> Result<bool, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("Vault directory not found: {}", path.display()).into());
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", path.display()).into());
    }

    let vault = VaultGraph::from_dir(path)?;
    let report = vault.lint();

    if json {
        let serialized = serde_json::to_string_pretty(&report)?;
        println!("{}", serialized);
    } else {
        print!("{}", crate::linter::format_lint_human(&report));
    }

    Ok(report.healthy)
}

fn parse_query_mode(s: &str) -> Result<QueryMode, String> {
    s.parse::<QueryMode>().map_err(|e| e.to_string())
}

/// Supported graph query modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryMode {
    Current,
    Lineage,
    Impact,
    Historical,
}

impl std::str::FromStr for QueryMode {
    type Err = QueryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "current" => Ok(QueryMode::Current),
            "lineage" => Ok(QueryMode::Lineage),
            "impact" => Ok(QueryMode::Impact),
            "historical" => Ok(QueryMode::Historical),
            _ => Err(QueryError::UnsupportedMode(s.to_string())),
        }
    }
}

impl std::fmt::Display for QueryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryMode::Current => write!(f, "current"),
            QueryMode::Lineage => write!(f, "lineage"),
            QueryMode::Impact => write!(f, "impact"),
            QueryMode::Historical => write!(f, "historical"),
        }
    }
}

/// Helper to format and print query context packets in JSON or Markdown.
fn print_query_output<T: serde::Serialize>(
    packet: &T,
    markdown: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(packet)?);
    } else {
        print!("{}", markdown);
    }
    Ok(())
}

/// Executes the query subcommand against a given vault directory.
pub fn run_query(
    vault_path: &Path,
    seed: &str,
    mode: QueryMode,
    as_of: Option<&str>,
    scope_args: &[String],
    json: bool,
) -> Result<i32, String> {
    if !vault_path.exists() {
        return Err(format!(
            "Vault directory not found: {}",
            vault_path.display()
        ));
    }
    if !vault_path.is_dir() {
        return Err(format!("Path is not a directory: {}", vault_path.display()));
    }

    let scope = parse_scope_args(scope_args)?;
    let vault = VaultGraph::from_dir(vault_path).map_err(|e| e.to_string())?;

    let exit_code = match mode {
        QueryMode::Current => {
            let packet = query_current(&vault, seed, &scope).map_err(|e| e.to_string())?;
            print_query_output(&packet, &packet.to_markdown(), json).map_err(|e| e.to_string())?;
            if packet.status == "unresolved_conflict" || packet.unresolved_conflict.is_some() {
                2
            } else {
                0
            }
        }
        QueryMode::Lineage => {
            let packet = query_lineage(&vault, seed, &scope).map_err(|e| e.to_string())?;
            print_query_output(&packet, &packet.to_markdown(), json).map_err(|e| e.to_string())?;
            0
        }
        QueryMode::Impact => {
            let packet = query_impact(&vault, seed, &scope).map_err(|e| e.to_string())?;
            print_query_output(&packet, &packet.to_markdown(), json).map_err(|e| e.to_string())?;
            0
        }
        QueryMode::Historical => {
            let packet =
                query_historical(&vault, seed, as_of, &scope).map_err(|e| e.to_string())?;
            print_query_output(&packet, &packet.to_markdown(), json).map_err(|e| e.to_string())?;
            0
        }
    };

    Ok(exit_code)
}

/// Executes the plan subcommand across a vault directory.
pub fn run_plan(vault: &Path, out: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let plan = plan_vault(vault)?;
    let serialized = serde_json::to_string_pretty(&plan)?;

    if let Some(out_path) = out {
        fs::write(out_path, serialized)?;
        println!(
            "Plan written to {} ({} proposed actions)",
            out_path.display(),
            plan.actions.len()
        );
    } else {
        println!("{}", serialized);
    }

    Ok(())
}

/// Executes the apply subcommand against an approved migration plan.
pub fn run_apply(
    vault: &Path,
    plan_path: &Path,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !plan_path.exists() {
        return Err(format!("Plan file not found: {}", plan_path.display()).into());
    }

    let plan_str = fs::read_to_string(plan_path)?;
    let plan: Plan = serde_json::from_str(&plan_str)
        .map_err(|e| format!("Invalid plan JSON in '{}': {}", plan_path.display(), e))?;

    let res = apply_plan(vault, &plan, dry_run)?;

    if dry_run {
        println!(
            "Validation successful. {} files would be modified ({} already up-to-date). [dry-run]",
            res.files_modified, res.files_already_up_to_date
        );
    } else {
        println!(
            "Applied {} changes ({} files already up-to-date).",
            res.files_modified, res.files_already_up_to_date
        );
    }

    Ok(())
}

/// Executes the export subcommand against a vault directory.
pub fn run_export(
    vault: &Path,
    format: &str,
    out: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !vault.exists() {
        return Err(format!("Vault directory not found: {}", vault.display()).into());
    }
    if !vault.is_dir() {
        return Err(format!("Vault path is not a directory: {}", vault.display()).into());
    }

    if !format.eq_ignore_ascii_case("okf") {
        return Err(format!(
            "Unsupported export format '{}'. Only 'okf' is currently supported.",
            format
        )
        .into());
    }

    let manifest = crate::okf::export_okf(vault, out)?;

    if json {
        let serialized = serde_json::to_string_pretty(&manifest)?;
        println!("{}", serialized);
    } else {
        println!(
            "Exported {} node(s) to {} (format: {})",
            manifest.total_nodes,
            out.display(),
            manifest.format
        );
        println!(
            "Manifest written to {}",
            out.join("manifest.json").display()
        );
    }

    Ok(())
}
