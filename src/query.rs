use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};

use crate::graph::VaultGraph;
use crate::model::{Node, RelationType, Scope};

/// Errors that can occur during graph queries.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Seed note '{0}' not found in vault '{1}'")]
    SeedNotFound(String, String),

    #[error("Unsupported query mode '{0}'. Currently supported modes: current, lineage, impact, historical")]
    UnsupportedMode(String),

    #[error("Missing required '--as-of <YYYY-MM-DD>' parameter for historical query mode")]
    MissingAsOf,

    #[error("Invalid date format '{0}'. Expected YYYY-MM-DD")]
    InvalidDateFormat(String),

    #[error("Unresolved conflict: {0}")]
    UnresolvedConflict(String),

    #[error("Supersession cycle detected: {0}")]
    SupersessionCycle(String),

    #[error("Invalid scope: {0}")]
    InvalidScope(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A single step in a supersession path from predecessor to successor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupersessionStep {
    pub predecessor: String,
    pub successor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
}

/// A historical predecessor note retained for decision rationale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoricalRationaleNote {
    pub node: Node,
    pub superseded_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
}

/// A competing or cycle candidate involved in an unresolved conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictCandidate {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
}

/// Structured details for an unresolved graph conflict (competing successors or cycle).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedConflict {
    pub alert: String, // "[UNRESOLVED_CONFLICT]"
    pub kind: String,  // "competing_successors" or "supersession_cycle"
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<Vec<String>>,
    pub candidates: Vec<ConflictCandidate>,
}

/// Alias for UnresolvedConflict report.
pub type UnresolvedConflictReport = UnresolvedConflict;

/// Alias for Note/Node.
pub type Note = Node;

/// An evidence link or candidate reference in an unresolved conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictEvidenceLink {
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Structured result packet for current guidance queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CurrentGuidancePacket {
    pub mode: String,
    pub seed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_guidance_id: Option<String>,
    pub is_current: bool,
    pub status: String,
    pub query_scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_note: Option<Note>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_conflict: Option<UnresolvedConflictReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersession_chain: Vec<SupersessionStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub historical_rationale: Vec<HistoricalRationaleNote>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<String>,
    pub message: String,
}

impl CurrentGuidancePacket {
    /// Formats the packet as bounded, copy-pasteable Markdown suitable for LLM prompts.
    pub fn to_markdown(&self) -> String {
        format_markdown_packet(self)
    }
}

/// The resolved supersession lineage from root ancestor to active leaf guidance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersessionLineage {
    pub chain_indices: Vec<petgraph::graph::NodeIndex>,
    pub steps: Vec<SupersessionStep>,
    pub advisories: Vec<String>,
}

/// A single entry in a chronological lineage replacement chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineageEntry {
    pub node: Node,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopted_evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersession_evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_by: Vec<String>,
    pub is_current: bool,
}

/// Structured result packet for lineage queries tracing replacement chains.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineagePacket {
    pub mode: String,
    pub seed: String,
    pub active_guidance_id: String,
    pub query_scope: Scope,
    pub chain: Vec<LineageEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<SupersessionStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub empirical_evidence: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<String>,
    pub message: String,
}

impl LineagePacket {
    /// Formats the packet as bounded, copy-pasteable Markdown suitable for LLM prompts.
    pub fn to_markdown(&self) -> String {
        format_lineage_markdown(self)
    }
}

/// A downstream component dependent on the queried seed note.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactItem {
    pub id: String,
    pub title: String,
    pub node_type: String,
    pub depth: usize,
    pub path: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

/// Structured result packet for impact analysis queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactPacket {
    pub mode: String,
    pub seed: String,
    pub seed_title: String,
    pub query_scope: Scope,
    pub total_dependents: usize,
    pub max_depth: usize,
    pub dependents: Vec<ImpactItem>,
    pub message: String,
}

impl ImpactPacket {
    /// Formats the packet as bounded, copy-pasteable Markdown suitable for LLM prompts.
    pub fn to_markdown(&self) -> String {
        format_impact_markdown(self)
    }
}

/// Structured result packet for point-in-time historical guidance queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoricalPacket {
    pub mode: String,
    pub seed: String,
    pub as_of: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_guidance_id: Option<String>,
    pub is_active: bool,
    pub status: String,
    pub query_scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_note: Option<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisories: Vec<String>,
    pub message: String,
}

impl HistoricalPacket {
    /// Formats the packet as bounded, copy-pasteable Markdown suitable for LLM prompts.
    pub fn to_markdown(&self) -> String {
        format_historical_markdown(self)
    }
}

/// Validates that a date string strictly conforms to `YYYY-MM-DD` format.
pub fn validate_date_format(date_str: &str) -> Result<(), QueryError> {
    let bytes = date_str.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(QueryError::InvalidDateFormat(date_str.to_string()));
    }

    let year = &date_str[0..4];
    let month = &date_str[5..7];
    let day = &date_str[8..10];

    if !year.chars().all(|c| c.is_ascii_digit())
        || !month.chars().all(|c| c.is_ascii_digit())
        || !day.chars().all(|c| c.is_ascii_digit())
    {
        return Err(QueryError::InvalidDateFormat(date_str.to_string()));
    }

    let m: u32 = month
        .parse()
        .map_err(|_| QueryError::InvalidDateFormat(date_str.to_string()))?;
    let d: u32 = day
        .parse()
        .map_err(|_| QueryError::InvalidDateFormat(date_str.to_string()))?;

    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(QueryError::InvalidDateFormat(date_str.to_string()));
    }

    Ok(())
}

/// Checks whether an effective scope matches the requested query scope.
pub fn scope_matches(query_scope: &Scope, effective_scope: &Scope) -> bool {
    for (k, q_val) in query_scope {
        if let Some(e_val) = effective_scope.get(k) {
            if e_val != q_val {
                return false;
            }
        }
    }
    true
}

/// Parses command-line scope arguments (e.g. `["env=production", "system=payments"]`) into a `Scope`.
pub fn parse_scope_args(args: &[String]) -> Result<Scope, String> {
    let mut scope = BTreeMap::new();
    for arg in args {
        for part in arg.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((k, v)) = part.split_once('=') {
                let k = k.trim().to_string();
                let v = v.trim().to_string();
                if k.is_empty() || v.is_empty() {
                    return Err(format!(
                        "Invalid scope pair '{}': key and value cannot be empty",
                        part
                    ));
                }
                scope.insert(k, v);
            } else {
                return Err(format!(
                    "Invalid scope format '{}'. Expected KEY=VALUE",
                    part
                ));
            }
        }
    }
    Ok(scope)
}

/// Determines whether candidate `idx_b` transitively supersedes candidate `idx_a`
/// along a directed path of `supersedes` edges with pairwise overlapping scopes,
/// all overlapping with `scope`.
pub fn transitively_supersedes(
    vault: &VaultGraph,
    idx_b: NodeIndex,
    idx_a: NodeIndex,
    scope: &Scope,
) -> bool {
    if idx_b == idx_a {
        return false;
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    queue.push_back((idx_b, None::<Scope>));
    visited.insert(idx_b);

    while let Some((curr_idx, prev_scope)) = queue.pop_front() {
        for edge in vault.graph.edges_directed(curr_idx, Direction::Outgoing) {
            if edge.weight().relation_type == RelationType::Supersedes {
                let target_idx = edge.target();
                let edge_sc = edge
                    .weight()
                    .scope
                    .as_ref()
                    .unwrap_or(&vault.graph[curr_idx].scope);

                let scopes_pairwise_overlap = match prev_scope {
                    Some(ref prev_sc) => crate::model::scopes_overlap(edge_sc, prev_sc),
                    None => true,
                };

                if scopes_pairwise_overlap && crate::model::scopes_overlap(edge_sc, scope) {
                    if target_idx == idx_a {
                        return true;
                    }

                    if visited.insert(target_idx) {
                        queue.push_back((target_idx, Some(edge_sc.clone())));
                    }
                }
            }
        }
    }

    false
}

fn order_supersession_cycle(
    vault: &VaultGraph,
    start_idx: NodeIndex,
    cycle_set: &HashSet<NodeIndex>,
) -> Vec<String> {
    let mut path = vec![start_idx];
    let mut current = start_idx;
    let mut visited = HashSet::new();
    visited.insert(current);

    while let Some(next) = {
        let mut edges: Vec<_> = vault
            .graph
            .edges_directed(current, Direction::Outgoing)
            .filter(|e| {
                e.weight().relation_type == RelationType::Supersedes
                    && cycle_set.contains(&e.target())
            })
            .collect();
        edges.sort_by_key(|e| &vault.graph[e.target()].id);
        edges.first().map(|e| e.target())
    } {
        if next == start_idx || visited.contains(&next) {
            break;
        }
        visited.insert(next);
        path.push(next);
        current = next;
    }

    if path.len() < cycle_set.len() {
        let path_set: HashSet<NodeIndex> = path.iter().copied().collect();
        let mut remaining: Vec<NodeIndex> = cycle_set
            .iter()
            .copied()
            .filter(|idx| !path_set.contains(idx))
            .collect();
        remaining.sort_by_key(|&idx| &vault.graph[idx].id);
        path.extend(remaining);
    }

    path.into_iter()
        .map(|idx| vault.graph[idx].id.clone())
        .collect()
}

/// Traces forward from a starting node to current guidance along matching supersession edges.
pub fn resolve_forward_supersession(
    vault: &VaultGraph,
    start_idx: petgraph::graph::NodeIndex,
    query_scope: &Scope,
) -> Result<SupersessionLineage, QueryError> {
    let mut chain_indices: Vec<petgraph::graph::NodeIndex> = Vec::new();
    let mut steps = Vec::new();
    let mut advisories = Vec::new();
    let mut visited = HashSet::new();

    let mut curr_idx = start_idx;

    loop {
        if !visited.insert(curr_idx) {
            let cycle_path = chain_indices
                .iter()
                .map(|idx| vault.graph[*idx].id.clone())
                .chain(std::iter::once(vault.graph[curr_idx].id.clone()))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(QueryError::SupersessionCycle(cycle_path));
        }
        chain_indices.push(curr_idx);

        let mut matching_successors = Vec::new();
        for edge_ref in vault.graph.edges_directed(curr_idx, Direction::Incoming) {
            let edge_data = edge_ref.weight();
            if edge_data.relation_type == RelationType::Supersedes {
                let source_idx = edge_ref.source();
                let succ_node = &vault.graph[source_idx];
                let effective_scope = edge_data.scope.as_ref().unwrap_or(&succ_node.scope);
                if scope_matches(query_scope, effective_scope) {
                    matching_successors.push((
                        source_idx,
                        edge_data.clone(),
                        effective_scope.clone(),
                    ));
                } else {
                    let sc_str = if effective_scope.is_empty() {
                        "unscoped".to_string()
                    } else {
                        effective_scope
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let adv = format!(
                        "A successor exists in another scope: '{}' (scope: [{}])",
                        succ_node.id, sc_str
                    );
                    if !advisories.contains(&adv) {
                        advisories.push(adv);
                    }
                }
            }
        }

        if matching_successors.is_empty() {
            break;
        }

        if matching_successors.len() > 1 {
            let candidate_descriptions = matching_successors
                .iter()
                .map(|(idx, edge, _)| {
                    let id = &vault.graph[*idx].id;
                    if let Some(ref ev) = edge.evidence {
                        format!("'{}' (evidence: {})", id, ev)
                    } else {
                        format!("'{}'", id)
                    }
                })
                .collect::<Vec<_>>();
            return Err(QueryError::UnresolvedConflict(format!(
                "[UNRESOLVED_CONFLICT] Competing successors found for '{}': {}. Strictly abstaining from selecting a winner.",
                vault.graph[curr_idx].id,
                candidate_descriptions.join(", ")
            )));
        }

        let (next_idx, edge_data, _eff_scope) = matching_successors.remove(0);
        steps.push(SupersessionStep {
            predecessor: vault.graph[curr_idx].id.clone(),
            successor: vault.graph[next_idx].id.clone(),
            evidence: edge_data.evidence.clone(),
            scope: edge_data.scope.clone(),
        });
        curr_idx = next_idx;
    }

    Ok(SupersessionLineage {
        chain_indices,
        steps,
        advisories,
    })
}

/// Traces the full supersession chain containing the given seed note,
/// navigating backwards to the earliest ancestor and then forwards to the active leaf.
pub fn trace_supersession_lineage(
    vault: &VaultGraph,
    seed_idx: petgraph::graph::NodeIndex,
    query_scope: &Scope,
) -> Result<SupersessionLineage, QueryError> {
    // Phase 1: Trace backward from seed to find root ancestor
    let mut curr_idx = seed_idx;
    let mut backward_visited = HashSet::new();
    backward_visited.insert(curr_idx);

    loop {
        let mut matching_preds = Vec::new();
        for edge_ref in vault.graph.edges_directed(curr_idx, Direction::Outgoing) {
            let edge_data = edge_ref.weight();
            if edge_data.relation_type == RelationType::Supersedes {
                let target_idx = edge_ref.target();
                let pred_node = &vault.graph[target_idx];
                let effective_scope = edge_data.scope.as_ref().unwrap_or(&pred_node.scope);
                if scope_matches(query_scope, effective_scope) {
                    matching_preds.push(target_idx);
                }
            }
        }

        if matching_preds.is_empty() || matching_preds.len() > 1 {
            break;
        }

        let pred_idx = matching_preds[0];
        if !backward_visited.insert(pred_idx) {
            return Err(QueryError::SupersessionCycle(format!(
                "{} -> {}",
                vault.graph[curr_idx].id, vault.graph[pred_idx].id
            )));
        }
        curr_idx = pred_idx;
    }

    let root_idx = curr_idx;

    // Phase 2: Trace forward from root ancestor to current guidance
    resolve_forward_supersession(vault, root_idx, query_scope)
}

/// Resolves the current guidance note starting from a seed note, traversing supersession chains.
pub fn query_current(
    vault: &VaultGraph,
    seed: &str,
    query_scope: &Scope,
) -> Result<CurrentGuidancePacket, QueryError> {
    let seed_idx = vault
        .resolve_seed(seed)
        .ok_or_else(|| QueryError::SeedNotFound(seed.to_string(), vault.vault_path.clone()))?;

    let seed_node = &vault.graph[seed_idx];
    let mut current_idx = seed_idx;
    let mut visited_order: Vec<NodeIndex> = Vec::new();
    let mut chain = Vec::new();
    let mut historical = Vec::new();
    let mut advisories = Vec::new();

    loop {
        if let Some(cycle_start_pos) = visited_order.iter().position(|&idx| idx == current_idx) {
            let cycle_indices = &visited_order[cycle_start_pos..];
            let cycle_set: HashSet<NodeIndex> = cycle_indices.iter().copied().collect();
            let cycle_ids = order_supersession_cycle(vault, current_idx, &cycle_set);

            let cycle_path = if cycle_ids.len() == 1 {
                format!("{} -> {}", cycle_ids[0], cycle_ids[0])
            } else {
                format!("{} -> {}", cycle_ids.join(" -> "), cycle_ids[0])
            };

            let mut candidates: Vec<ConflictCandidate> = cycle_indices
                .iter()
                .map(|&cand_idx| {
                    let cand_node = &vault.graph[cand_idx];
                    let cand_evidence = vault
                        .graph
                        .edges_directed(cand_idx, Direction::Outgoing)
                        .find(|e| {
                            e.weight().relation_type == RelationType::Supersedes
                                && cycle_indices.contains(&e.target())
                        })
                        .and_then(|e| e.weight().evidence.clone())
                        .or_else(|| {
                            cand_node
                                .relations
                                .iter()
                                .find(|r| r.relation_type == RelationType::Supersedes)
                                .and_then(|r| r.evidence.clone())
                        });

                    let cand_scope = vault
                        .graph
                        .edges_directed(cand_idx, Direction::Outgoing)
                        .find(|e| {
                            e.weight().relation_type == RelationType::Supersedes
                                && cycle_indices.contains(&e.target())
                        })
                        .and_then(|e| e.weight().scope.clone())
                        .or_else(|| {
                            if cand_node.scope.is_empty() {
                                None
                            } else {
                                Some(cand_node.scope.clone())
                            }
                        });

                    ConflictCandidate {
                        id: cand_node.id.clone(),
                        title: Some(cand_node.title.clone()),
                        path: cand_node.path.clone(),
                        evidence: cand_evidence,
                        scope: cand_scope,
                    }
                })
                .collect();

            candidates.sort_by(|a, b| a.id.cmp(&b.id));

            let message = format!(
                "[UNRESOLVED_CONFLICT] Supersession cycle detected: {}. Strictly abstaining from selecting a winner.",
                cycle_path
            );

            let conflict = UnresolvedConflict {
                alert: "[UNRESOLVED_CONFLICT]".to_string(),
                kind: "supersession_cycle".to_string(),
                message: message.clone(),
                predecessor: None,
                cycle: Some(cycle_ids),
                candidates,
            };

            historical.truncate(cycle_start_pos);
            chain.truncate(cycle_start_pos);

            return Ok(CurrentGuidancePacket {
                mode: "current".to_string(),
                seed: seed.to_string(),
                active_guidance_id: None,
                is_current: false,
                status: "unresolved_conflict".to_string(),
                query_scope: query_scope.clone(),
                active_note: None,
                unresolved_conflict: Some(conflict),
                supersession_chain: chain,
                historical_rationale: historical,
                advisories,
                message,
            });
        }

        visited_order.push(current_idx);

        // Find incoming edges with relation_type == Supersedes
        let mut matching_successors = Vec::new();
        for edge_ref in vault.graph.edges_directed(current_idx, Direction::Incoming) {
            let edge_data = edge_ref.weight();
            if edge_data.relation_type == RelationType::Supersedes {
                let source_idx = edge_ref.source();
                let succ_node = &vault.graph[source_idx];
                let effective_scope = edge_data.scope.as_ref().unwrap_or(&succ_node.scope);
                if scope_matches(query_scope, effective_scope) {
                    matching_successors.push((
                        source_idx,
                        edge_data.clone(),
                        effective_scope.clone(),
                    ));
                } else {
                    let sc_str = if effective_scope.is_empty() {
                        "unscoped".to_string()
                    } else {
                        effective_scope
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let adv = format!(
                        "A successor exists in another scope: '{}' (scope: [{}])",
                        succ_node.id, sc_str
                    );
                    if !advisories.contains(&adv) {
                        advisories.push(adv);
                    }
                }
            }
        }

        if matching_successors.is_empty() {
            // No successors in scope; current_idx is active guidance candidate
            break;
        }

        let next_idx;
        let edge_data;

        if matching_successors.len() > 1 {
            // Filter matching successors to active candidates
            let active_candidates: Vec<(
                petgraph::graph::NodeIndex,
                crate::graph::EdgeData,
                Scope,
            )> = matching_successors
                .iter()
                .filter(|(idx, _, _)| crate::linter::is_node_active(vault, &vault.graph[*idx]))
                .cloned()
                .collect();

            // Check if any candidate transitively supersedes another candidate
            let mut survivor_candidates = Vec::new();
            for i in 0..active_candidates.len() {
                let (idx_a, ref e_a, ref sc_a) = active_candidates[i];
                let mut superseded_by_another = false;

                for (j, &(idx_b, _, _)) in active_candidates.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    if transitively_supersedes(vault, idx_b, idx_a, sc_a) {
                        superseded_by_another = true;
                        break;
                    }
                }

                if !superseded_by_another {
                    survivor_candidates.push((idx_a, e_a.clone(), sc_a.clone()));
                }
            }

            if survivor_candidates.is_empty() {
                let pred_id = vault.graph[current_idx].id.clone();

                // Check if any matching successors participate in a supersession cycle
                let all_cycles = crate::linter::find_supersession_cycles(vault);
                let succ_ids: HashSet<String> = matching_successors
                    .iter()
                    .map(|(idx, _, _)| vault.graph[*idx].id.clone())
                    .collect();
                let matching_cycle = all_cycles
                    .into_iter()
                    .find(|cyc| cyc.iter().any(|id| succ_ids.contains(id)));

                if let Some(cycle_ids) = matching_cycle {
                    let cycle_path = if cycle_ids.len() == 1 {
                        format!("{} -> {}", cycle_ids[0], cycle_ids[0])
                    } else {
                        format!("{} -> {}", cycle_ids.join(" -> "), cycle_ids[0])
                    };

                    let mut candidates: Vec<ConflictCandidate> = matching_successors
                        .into_iter()
                        .map(|(idx, edge_d, effective_scope)| {
                            let node = &vault.graph[idx];
                            let scope = if effective_scope.is_empty() {
                                None
                            } else {
                                Some(effective_scope)
                            };
                            ConflictCandidate {
                                id: node.id.clone(),
                                title: Some(node.title.clone()),
                                path: node.path.clone(),
                                evidence: edge_d.evidence,
                                scope,
                            }
                        })
                        .collect();
                    candidates.sort_by(|a, b| a.id.cmp(&b.id));

                    let message = format!(
                        "[UNRESOLVED_CONFLICT] Supersession cycle detected among successors of '{}': {}. Strictly abstaining from selecting a winner.",
                        pred_id, cycle_path
                    );

                    let conflict = UnresolvedConflict {
                        alert: "[UNRESOLVED_CONFLICT]".to_string(),
                        kind: "supersession_cycle".to_string(),
                        message: message.clone(),
                        predecessor: Some(pred_id),
                        cycle: Some(cycle_ids),
                        candidates,
                    };

                    return Ok(CurrentGuidancePacket {
                        mode: "current".to_string(),
                        seed: seed.to_string(),
                        active_guidance_id: None,
                        is_current: false,
                        status: "unresolved_conflict".to_string(),
                        query_scope: query_scope.clone(),
                        active_note: None,
                        unresolved_conflict: Some(conflict),
                        supersession_chain: chain,
                        historical_rationale: historical,
                        advisories,
                        message,
                    });
                } else if !active_candidates.is_empty() {
                    let mut candidates: Vec<ConflictCandidate> = active_candidates
                        .into_iter()
                        .map(|(idx, edge_d, effective_scope)| {
                            let node = &vault.graph[idx];
                            let scope = if effective_scope.is_empty() {
                                None
                            } else {
                                Some(effective_scope)
                            };
                            ConflictCandidate {
                                id: node.id.clone(),
                                title: Some(node.title.clone()),
                                path: node.path.clone(),
                                evidence: edge_d.evidence,
                                scope,
                            }
                        })
                        .collect();
                    candidates.sort_by(|a, b| a.id.cmp(&b.id));
                    let candidate_ids: Vec<String> =
                        candidates.iter().map(|c| c.id.clone()).collect();
                    let message = format!(
                        "[UNRESOLVED_CONFLICT] Competing successors found for '{}': {}. Strictly abstaining from selecting a winner.",
                        pred_id,
                        candidate_ids.join(", ")
                    );

                    let conflict = UnresolvedConflict {
                        alert: "[UNRESOLVED_CONFLICT]".to_string(),
                        kind: "competing_successors".to_string(),
                        message: message.clone(),
                        predecessor: Some(pred_id),
                        cycle: None,
                        candidates,
                    };

                    return Ok(CurrentGuidancePacket {
                        mode: "current".to_string(),
                        seed: seed.to_string(),
                        active_guidance_id: None,
                        is_current: false,
                        status: "unresolved_conflict".to_string(),
                        query_scope: query_scope.clone(),
                        active_note: None,
                        unresolved_conflict: Some(conflict),
                        supersession_chain: chain,
                        historical_rationale: historical,
                        advisories,
                        message,
                    });
                } else {
                    let mut candidates: Vec<ConflictCandidate> = matching_successors
                        .into_iter()
                        .map(|(idx, edge_d, effective_scope)| {
                            let node = &vault.graph[idx];
                            let scope = if effective_scope.is_empty() {
                                None
                            } else {
                                Some(effective_scope)
                            };
                            ConflictCandidate {
                                id: node.id.clone(),
                                title: Some(node.title.clone()),
                                path: node.path.clone(),
                                evidence: edge_d.evidence,
                                scope,
                            }
                        })
                        .collect();
                    candidates.sort_by(|a, b| a.id.cmp(&b.id));

                    let message = format!(
                        "[UNRESOLVED_CONFLICT] Note '{}' has been superseded, but all successors are inactive in the requested scope.",
                        pred_id
                    );

                    let conflict = UnresolvedConflict {
                        alert: "[UNRESOLVED_CONFLICT]".to_string(),
                        kind: "superseded_without_active_successor".to_string(),
                        message: message.clone(),
                        predecessor: Some(pred_id),
                        cycle: None,
                        candidates,
                    };

                    return Ok(CurrentGuidancePacket {
                        mode: "current".to_string(),
                        seed: seed.to_string(),
                        active_guidance_id: None,
                        is_current: false,
                        status: "unresolved_conflict".to_string(),
                        query_scope: query_scope.clone(),
                        active_note: None,
                        unresolved_conflict: Some(conflict),
                        supersession_chain: chain,
                        historical_rationale: historical,
                        advisories,
                        message,
                    });
                }
            } else if survivor_candidates.len() == 1 {
                let (s_idx, s_edge, _) = survivor_candidates.remove(0);
                next_idx = s_idx;
                edge_data = s_edge;
            } else {
                let pred_id = vault.graph[current_idx].id.clone();
                let mut candidates: Vec<ConflictCandidate> = survivor_candidates
                    .into_iter()
                    .map(|(idx, edge_d, effective_scope)| {
                        let node = &vault.graph[idx];
                        let scope = if effective_scope.is_empty() {
                            None
                        } else {
                            Some(effective_scope)
                        };
                        ConflictCandidate {
                            id: node.id.clone(),
                            title: Some(node.title.clone()),
                            path: node.path.clone(),
                            evidence: edge_d.evidence,
                            scope,
                        }
                    })
                    .collect();

                candidates.sort_by(|a, b| a.id.cmp(&b.id));
                let candidate_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();
                let message = format!(
                    "[UNRESOLVED_CONFLICT] Competing successors found for '{}': {}. Strictly abstaining from selecting a winner.",
                    pred_id,
                    candidate_ids.join(", ")
                );

                let conflict = UnresolvedConflict {
                    alert: "[UNRESOLVED_CONFLICT]".to_string(),
                    kind: "competing_successors".to_string(),
                    message: message.clone(),
                    predecessor: Some(pred_id),
                    cycle: None,
                    candidates,
                };

                return Ok(CurrentGuidancePacket {
                    mode: "current".to_string(),
                    seed: seed.to_string(),
                    active_guidance_id: None,
                    is_current: false,
                    status: "unresolved_conflict".to_string(),
                    query_scope: query_scope.clone(),
                    active_note: None,
                    unresolved_conflict: Some(conflict),
                    supersession_chain: chain,
                    historical_rationale: historical,
                    advisories,
                    message,
                });
            }
        } else {
            let (s_idx, s_edge, _) = matching_successors.remove(0);
            next_idx = s_idx;
            edge_data = s_edge;
        }

        let curr_node = &vault.graph[current_idx];
        let succ_node = &vault.graph[next_idx];

        historical.push(HistoricalRationaleNote {
            node: curr_node.clone(),
            superseded_by: succ_node.id.clone(),
            evidence: edge_data.evidence.clone(),
            scope: edge_data.scope.clone(),
        });

        chain.push(SupersessionStep {
            predecessor: curr_node.id.clone(),
            successor: succ_node.id.clone(),
            evidence: edge_data.evidence.clone(),
            scope: edge_data.scope.clone(),
        });

        current_idx = next_idx;
    }

    let current_node = &vault.graph[current_idx];

    // Check for active contradicts relations connected to current_idx
    let mut contradicting_edges = Vec::new();

    for edge in vault.graph.edges_directed(current_idx, Direction::Outgoing) {
        if edge.weight().relation_type == RelationType::Contradicts {
            let other_idx = edge.target();
            if other_idx == current_idx {
                continue;
            }
            let other_node = &vault.graph[other_idx];
            if crate::linter::is_node_active(vault, other_node) {
                let scope_current = edge.weight().scope.as_ref().unwrap_or(&current_node.scope);
                let scope_other = &other_node.scope;
                if crate::model::scopes_overlap(scope_current, scope_other)
                    && scope_matches(query_scope, scope_current)
                    && scope_matches(query_scope, scope_other)
                {
                    contradicting_edges.push((other_idx, edge.weight().evidence.clone(), true));
                }
            }
        }
    }

    for edge in vault.graph.edges_directed(current_idx, Direction::Incoming) {
        if edge.weight().relation_type == RelationType::Contradicts {
            let other_idx = edge.source();
            if other_idx == current_idx {
                continue;
            }
            let other_node = &vault.graph[other_idx];
            if crate::linter::is_node_active(vault, other_node) {
                let scope_other = edge.weight().scope.as_ref().unwrap_or(&other_node.scope);
                let scope_current = &current_node.scope;
                if crate::model::scopes_overlap(scope_current, scope_other)
                    && scope_matches(query_scope, scope_current)
                    && scope_matches(query_scope, scope_other)
                {
                    contradicting_edges.push((other_idx, edge.weight().evidence.clone(), false));
                }
            }
        }
    }

    if !contradicting_edges.is_empty() {
        contradicting_edges.sort_by(|a, b| vault.graph[a.0].id.cmp(&vault.graph[b.0].id));
        let (other_idx, edge_evidence, is_outgoing) = contradicting_edges.remove(0);
        let other_node = &vault.graph[other_idx];

        let (current_evidence, other_evidence) = if is_outgoing {
            let other_ev = vault
                .graph
                .edges_connecting(other_idx, current_idx)
                .find(|e| e.weight().relation_type == RelationType::Contradicts)
                .and_then(|e| e.weight().evidence.clone())
                .or_else(|| {
                    other_node
                        .relations
                        .iter()
                        .find(|r| r.relation_type == RelationType::Contradicts)
                        .and_then(|r| r.evidence.clone())
                });
            (edge_evidence, other_ev)
        } else {
            let curr_ev = vault
                .graph
                .edges_connecting(current_idx, other_idx)
                .find(|e| e.weight().relation_type == RelationType::Contradicts)
                .and_then(|e| e.weight().evidence.clone())
                .or_else(|| {
                    current_node
                        .relations
                        .iter()
                        .find(|r| r.relation_type == RelationType::Contradicts)
                        .and_then(|r| r.evidence.clone())
                });
            (curr_ev, edge_evidence)
        };

        let current_candidate = ConflictCandidate {
            id: current_node.id.clone(),
            title: Some(current_node.title.clone()),
            path: current_node.path.clone(),
            evidence: current_evidence,
            scope: if current_node.scope.is_empty() {
                None
            } else {
                Some(current_node.scope.clone())
            },
        };

        let other_candidate = ConflictCandidate {
            id: other_node.id.clone(),
            title: Some(other_node.title.clone()),
            path: other_node.path.clone(),
            evidence: other_evidence,
            scope: if other_node.scope.is_empty() {
                None
            } else {
                Some(other_node.scope.clone())
            },
        };

        let message = format!(
            "[UNRESOLVED_CONFLICT] Active contradiction between '{}' and '{}' with overlapping scopes.",
            current_node.id, other_node.id
        );

        let conflict = UnresolvedConflict {
            alert: "[UNRESOLVED_CONFLICT]".to_string(),
            kind: "contradiction".to_string(),
            message: message.clone(),
            predecessor: None,
            cycle: None,
            candidates: vec![current_candidate, other_candidate],
        };

        return Ok(CurrentGuidancePacket {
            mode: "current".to_string(),
            seed: seed.to_string(),
            active_guidance_id: None,
            is_current: false,
            status: "unresolved_conflict".to_string(),
            query_scope: query_scope.clone(),
            active_note: None,
            unresolved_conflict: Some(conflict),
            supersession_chain: chain,
            historical_rationale: historical,
            advisories,
            message,
        });
    }

    let is_current = current_idx == seed_idx;
    let active_node = vault.graph[current_idx].clone();

    let (status, message) = if is_current {
        if advisories.is_empty() {
            (
                "current".to_string(),
                format!(
                    "Note '{}' has no successors and is itself current guidance.",
                    seed_node.id
                ),
            )
        } else {
            (
                "current_in_scope".to_string(),
                format!(
                    "Note '{}' has no active successors in the requested scope and is itself current guidance.",
                    seed_node.id
                ),
            )
        }
    } else {
        (
            "superseded".to_string(),
            format!(
                "Resolved superseded note '{}' along {} supersession hop(s) to active guidance '{}'.",
                seed_node.id,
                chain.len(),
                active_node.id
            ),
        )
    };

    Ok(CurrentGuidancePacket {
        mode: "current".to_string(),
        seed: seed.to_string(),
        active_guidance_id: Some(active_node.id.clone()),
        is_current,
        status,
        query_scope: query_scope.clone(),
        active_note: Some(active_node),
        unresolved_conflict: None,
        supersession_chain: chain,
        historical_rationale: historical,
        advisories,
        message,
    })
}

/// Traces the chronological replacement chain ($A \rightarrow B \rightarrow C$), displaying why
/// decisions changed and surfacing supporting evidence.
pub fn query_lineage(
    vault: &VaultGraph,
    seed: &str,
    query_scope: &Scope,
) -> Result<LineagePacket, QueryError> {
    let seed_idx = vault
        .resolve_seed(seed)
        .ok_or_else(|| QueryError::SeedNotFound(seed.to_string(), vault.vault_path.clone()))?;

    let seed_node = &vault.graph[seed_idx];
    let lineage = trace_supersession_lineage(vault, seed_idx, query_scope)?;

    let active_guidance_id = vault.graph[*lineage.chain_indices.last().unwrap()]
        .id
        .clone();

    let mut chain = Vec::new();
    let mut empirical_evidence = Vec::new();
    let mut seen_evidence_ids = HashSet::new();

    for (i, &idx) in lineage.chain_indices.iter().enumerate() {
        let node = vault.graph[idx].clone();
        let is_current = i == lineage.chain_indices.len() - 1;
        let supersedes = if i > 0 {
            Some(vault.graph[lineage.chain_indices[i - 1]].id.clone())
        } else {
            None
        };
        let superseded_by = if i + 1 < lineage.chain_indices.len() {
            Some(vault.graph[lineage.chain_indices[i + 1]].id.clone())
        } else {
            None
        };

        // Evidence attribution:
        // - adopted_evidence: why this decision replaced its predecessor (from transition step i-1)
        // - supersession_evidence: why this decision was superseded by its successor (from transition step i)
        let adopted_evidence = if i > 0 {
            lineage.steps[i - 1].evidence.clone()
        } else {
            None
        };

        let supersession_evidence = if i < lineage.steps.len() {
            lineage.steps[i].evidence.clone()
        } else {
            None
        };

        // Collect empirical evidence targets for this note
        let mut supported_by = Vec::new();
        for edge_ref in vault.graph.edges_directed(idx, Direction::Outgoing) {
            let edge_data = edge_ref.weight();
            if edge_data.relation_type == RelationType::SupportedBy {
                let target_idx = edge_ref.target();
                let ev_node = &vault.graph[target_idx];
                supported_by.push(ev_node.id.clone());
                if seen_evidence_ids.insert(ev_node.id.clone()) {
                    empirical_evidence.push(ev_node.clone());
                }
            }
        }
        for rel in &node.relations {
            if rel.relation_type == RelationType::SupportedBy {
                let resolved_id = if let Some(target_idx) = vault.resolve_seed(&rel.target) {
                    vault.graph[target_idx].id.clone()
                } else {
                    rel.target.clone()
                };
                if !supported_by.contains(&resolved_id) {
                    supported_by.push(resolved_id);
                }
            }
        }
        supported_by.sort();

        chain.push(LineageEntry {
            node,
            supersedes,
            superseded_by,
            adopted_evidence,
            supersession_evidence,
            supported_by,
            is_current,
        });
    }

    let message = if chain.len() == 1 {
        format!(
            "Note '{}' has no supersession lineage and is itself current guidance.",
            seed_node.id
        )
    } else {
        let chain_str = chain
            .iter()
            .map(|e| format!("'{}'", e.node.id))
            .collect::<Vec<_>>()
            .join(" -> ");
        format!(
            "Traced chronological replacement chain across {} node(s): {}.",
            chain.len(),
            chain_str
        )
    };

    Ok(LineagePacket {
        mode: "lineage".to_string(),
        seed: seed.to_string(),
        active_guidance_id,
        query_scope: query_scope.clone(),
        chain,
        transitions: lineage.steps,
        empirical_evidence,
        advisories: lineage.advisories,
        message,
    })
}

/// Traverses dynamic inverse dependencies (projected from `depends_on`) to list all downstream
/// components that transitively depend on the seed note. Dependencies are structural across scopes.
pub fn query_impact(
    vault: &VaultGraph,
    seed: &str,
    query_scope: &Scope,
) -> Result<ImpactPacket, QueryError> {
    let seed_idx = vault
        .resolve_seed(seed)
        .ok_or_else(|| QueryError::SeedNotFound(seed.to_string(), vault.vault_path.clone()))?;

    let seed_node = &vault.graph[seed_idx];
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent_map: std::collections::HashMap<
        petgraph::graph::NodeIndex,
        petgraph::graph::NodeIndex,
    > = std::collections::HashMap::new();

    visited.insert(seed_idx);
    queue.push_back((seed_idx, 0));

    let mut dependents = Vec::new();
    let mut max_depth = 0;

    while let Some((curr_idx, curr_depth)) = queue.pop_front() {
        let mut direct_dependents = Vec::new();

        // Dynamically project inverse relation from incoming forward depends_on edges.
        // Downstream components are structural dependencies; they are included across scopes.
        for edge_ref in vault.graph.edges_directed(curr_idx, Direction::Incoming) {
            let edge_data = edge_ref.weight();
            if edge_data.relation_type == RelationType::DependsOn {
                let dep_idx = edge_ref.source();
                let dep_node = &vault.graph[dep_idx];
                let effective_scope = edge_data.scope.as_ref().unwrap_or(&dep_node.scope);
                direct_dependents.push((dep_idx, effective_scope.clone()));
            }
        }

        // Sort deterministically by dependent node ID
        direct_dependents.sort_by(|a, b| vault.graph[a.0].id.cmp(&vault.graph[b.0].id));

        for (dep_idx, eff_scope) in direct_dependents {
            if visited.insert(dep_idx) {
                parent_map.insert(dep_idx, curr_idx);
                let dep_node = &vault.graph[dep_idx];
                let next_depth = curr_depth + 1;

                if next_depth > max_depth {
                    max_depth = next_depth;
                }

                // Reconstruct path once per emitted ImpactItem from parent_map
                let mut path_indices = Vec::new();
                let mut walk = dep_idx;
                path_indices.push(walk);
                while let Some(&p) = parent_map.get(&walk) {
                    path_indices.push(p);
                    if p == seed_idx {
                        break;
                    }
                    walk = p;
                }
                path_indices.reverse();
                let path: Vec<String> = path_indices
                    .iter()
                    .map(|idx| vault.graph[*idx].id.clone())
                    .collect();

                dependents.push(ImpactItem {
                    id: dep_node.id.clone(),
                    title: dep_node.title.clone(),
                    node_type: dep_node.node_type.clone(),
                    depth: next_depth,
                    path,
                    scope: eff_scope,
                    file_path: dep_node.path.clone(),
                });

                queue.push_back((dep_idx, next_depth));
            }
        }
    }

    let total_dependents = dependents.len();
    let message = if total_dependents == 0 {
        format!("No downstream components depend on '{}'.", seed_node.id)
    } else {
        format!(
            "Found {} downstream component(s) dependent on '{}' across {} dependency level(s).",
            total_dependents, seed_node.id, max_depth
        )
    };

    Ok(ImpactPacket {
        mode: "impact".to_string(),
        seed: seed.to_string(),
        seed_title: seed_node.title.clone(),
        query_scope: query_scope.clone(),
        total_dependents,
        max_depth,
        dependents,
        message,
    })
}

/// Reconstructs the active guidance as of a given historical point-in-time date (YYYY-MM-DD).
pub fn query_historical(
    vault: &VaultGraph,
    seed: &str,
    as_of: Option<&str>,
    query_scope: &Scope,
) -> Result<HistoricalPacket, QueryError> {
    let as_of_raw = match as_of {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return Err(QueryError::MissingAsOf),
    };

    validate_date_format(as_of_raw)?;
    let as_of_date = as_of_raw;

    let seed_idx = vault
        .resolve_seed(seed)
        .ok_or_else(|| QueryError::SeedNotFound(seed.to_string(), vault.vault_path.clone()))?;

    let seed_node = &vault.graph[seed_idx];
    let lineage = trace_supersession_lineage(vault, seed_idx, query_scope)?;

    let nodes: Vec<Node> = lineage
        .chain_indices
        .iter()
        .map(|idx| vault.graph[*idx].clone())
        .collect();

    let mut warnings = Vec::new();
    for n in &nodes {
        if n.valid_from.is_none() && n.valid_until.is_none() {
            warnings.push(format!(
                "Note '{}' lacks lifecycle dates ('valid_from' / 'valid_until'); unable to strictly verify interval bounds.",
                n.id
            ));
        } else if n.valid_from.is_none() {
            warnings.push(format!(
                "Note '{}' is missing 'valid_from' date; unable to strictly verify start of validity.",
                n.id
            ));
        }
    }

    // Check if as_of is prior to earliest valid_from
    let earliest_start = nodes.iter().filter_map(|n| n.valid_from.as_deref()).min();
    if let Some(first_start) = earliest_start {
        if as_of_date < first_start {
            if nodes[0].valid_from.is_none() {
                return Ok(HistoricalPacket {
                    mode: "historical".to_string(),
                    seed: seed.to_string(),
                    as_of: as_of_date.to_string(),
                    active_guidance_id: None,
                    is_active: false,
                    status: "indeterminate".to_string(),
                    query_scope: query_scope.clone(),
                    active_note: None,
                    warnings,
                    advisories: lineage.advisories,
                    message: format!(
                        "As of {}, guidance for '{}' is indeterminate: earliest note '{}' lacks 'valid_from' and cannot be verified as active prior to known date '{}'.",
                        as_of_date, seed_node.id, nodes[0].id, first_start
                    ),
                });
            } else {
                return Ok(HistoricalPacket {
                    mode: "historical".to_string(),
                    seed: seed.to_string(),
                    as_of: as_of_date.to_string(),
                    active_guidance_id: None,
                    is_active: false,
                    status: "not_yet_valid".to_string(),
                    query_scope: query_scope.clone(),
                    active_note: None,
                    warnings,
                    advisories: lineage.advisories,
                    message: format!(
                        "As of {}, guidance for '{}' was not yet valid (earliest valid_from is '{}').",
                        as_of_date, seed_node.id, first_start
                    ),
                });
            }
        }
    } else {
        // No notes in the chain have valid_from
        return Ok(HistoricalPacket {
            mode: "historical".to_string(),
            seed: seed.to_string(),
            as_of: as_of_date.to_string(),
            active_guidance_id: None,
            is_active: false,
            status: "indeterminate".to_string(),
            query_scope: query_scope.clone(),
            active_note: None,
            warnings,
            advisories: lineage.advisories,
            message: format!(
                "As of {}, guidance for '{}' is indeterminate due to missing lifecycle dates ('valid_from').",
                as_of_date, seed_node.id
            ),
        });
    }

    // Evaluate validity intervals across the chronological chain.
    // Spec invariants:
    // 1. Do NOT default undated notes to true.
    // 2. When successor B supersedes A and arrives at B's valid_from, A's active period terminates at B's valid_from.
    // 3. An expired successor must NEVER resurrect an older predecessor.
    // 4. If multiple competing active notes match as_of without a determinable supersession ordering,
    //    strictly abstain by returning QueryError::UnresolvedConflict with [UNRESOLVED_CONFLICT].
    let mut active_matches = Vec::new();
    let mut has_indeterminate_in_chain = false;

    for i in 0..nodes.len() {
        let node = &nodes[i];
        let start = match node.valid_from.as_deref() {
            Some(s) => s,
            None => {
                has_indeterminate_in_chain = true;
                continue;
            }
        };

        if as_of_date < start {
            continue;
        }

        // Check if a successor exists in the chain
        let succ_start = if i + 1 < nodes.len() {
            nodes[i + 1].valid_from.as_deref()
        } else {
            None
        };

        // If successor has arrived, node i has terminated at successor's valid_from
        if let Some(succ_date) = succ_start {
            if as_of_date >= succ_date {
                continue;
            }
        } else if i + 1 < nodes.len() {
            // Successor exists but lacks valid_from; cannot determine termination date
            has_indeterminate_in_chain = true;
        }

        // Check explicit expiration
        if let Some(ref until) = node.valid_until {
            if as_of_date > until.as_str() {
                continue;
            }
        }

        active_matches.push(node.clone());
    }

    // Strict abstention if multiple competing active notes match
    if active_matches.len() > 1 {
        let competing_items: Vec<String> = active_matches
            .iter()
            .map(|n| {
                let evidence_items: Vec<String> = n
                    .relations
                    .iter()
                    .filter_map(|r| {
                        r.evidence
                            .as_ref()
                            .map(|e| format!("{}: {}", r.relation_type, e))
                    })
                    .collect();
                if evidence_items.is_empty() {
                    format!("'{}'", n.id)
                } else {
                    format!("'{}' (evidence: [{}])", n.id, evidence_items.join(", "))
                }
            })
            .collect();

        return Err(QueryError::UnresolvedConflict(format!(
            "[UNRESOLVED_CONFLICT] Multiple competing active notes match as_of '{}': {}. Strictly abstaining from selecting a winner.",
            as_of_date,
            competing_items.join(", ")
        )));
    }

    if let Some(active) = active_matches.pop() {
        let active_id = active.id.clone();
        let message = format!(
            "Resolved active guidance as of {} to '{}'.",
            as_of_date, active_id
        );

        return Ok(HistoricalPacket {
            mode: "historical".to_string(),
            seed: seed.to_string(),
            as_of: as_of_date.to_string(),
            active_guidance_id: Some(active_id),
            is_active: true,
            status: "active".to_string(),
            query_scope: query_scope.clone(),
            active_note: Some(active),
            warnings,
            advisories: lineage.advisories,
            message,
        });
    }

    // No note is actively matching. Determine why:
    // Check if the most recent candidate note that was valid has expired.
    // An expired successor must NEVER resurrect an older predecessor.
    for i in (0..nodes.len()).rev() {
        let node = &nodes[i];
        if let Some(start) = node.valid_from.as_deref() {
            if as_of_date >= start {
                if let Some(ref until) = node.valid_until {
                    if as_of_date > until.as_str() {
                        return Ok(HistoricalPacket {
                            mode: "historical".to_string(),
                            seed: seed.to_string(),
                            as_of: as_of_date.to_string(),
                            active_guidance_id: None,
                            is_active: false,
                            status: "expired".to_string(),
                            query_scope: query_scope.clone(),
                            active_note: None,
                            warnings,
                            advisories: lineage.advisories,
                            message: format!(
                                "As of {}, guidance for '{}' had expired (note '{}' valid_until was '{}').",
                                as_of_date, seed_node.id, node.id, until
                            ),
                        });
                    }
                }
            }
        }
    }

    if has_indeterminate_in_chain {
        return Ok(HistoricalPacket {
            mode: "historical".to_string(),
            seed: seed.to_string(),
            as_of: as_of_date.to_string(),
            active_guidance_id: None,
            is_active: false,
            status: "indeterminate".to_string(),
            query_scope: query_scope.clone(),
            active_note: None,
            warnings,
            advisories: lineage.advisories,
            message: format!(
                "As of {}, guidance for '{}' is indeterminate due to missing lifecycle dates in the supersession chain.",
                as_of_date, seed_node.id
            ),
        });
    }

    Ok(HistoricalPacket {
        mode: "historical".to_string(),
        seed: seed.to_string(),
        as_of: as_of_date.to_string(),
        active_guidance_id: None,
        is_active: false,
        status: "no_active_guidance".to_string(),
        query_scope: query_scope.clone(),
        active_note: None,
        warnings,
        advisories: lineage.advisories,
        message: format!(
            "No guidance was active for '{}' as of {}.",
            seed_node.id, as_of_date
        ),
    })
}

/// Formats a `CurrentGuidancePacket` into a bounded Markdown context packet.
pub fn format_markdown_packet(packet: &CurrentGuidancePacket) -> String {
    let mut out = String::new();

    out.push_str("# Akashic Records — Agent Context Packet\n\n");
    out.push_str(&format!("- **Mode**: {}\n", packet.mode));
    out.push_str(&format!("- **Seed**: `{}`\n", packet.seed));

    if let Some(ref conflict) = packet.unresolved_conflict {
        out.push_str("- **Active Guidance**: None (Strict abstention)\n");
        out.push_str("- **Status**: UNRESOLVED CONFLICT (Strict abstention)\n");

        if !packet.query_scope.is_empty() {
            let sc_str = packet
                .query_scope
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Scope Filter**: [{}]\n", sc_str));
        }

        out.push_str(&format!("- **Resolution**: {}\n\n", packet.message));

        if !packet.advisories.is_empty() {
            out.push_str("> [!NOTE]\n");
            out.push_str("> **Advisories:**\n");
            for adv in &packet.advisories {
                out.push_str(&format!("> - {}\n", adv));
            }
            out.push('\n');
        }

        out.push_str("> [!WARNING]\n");
        out.push_str("> ### [UNRESOLVED_CONFLICT]\n");
        out.push_str(&format!("> - **Kind**: {}\n", conflict.kind));
        if let Some(ref pred) = conflict.predecessor {
            out.push_str(&format!("> - **Predecessor**: `{}`\n", pred));
        }
        if let Some(ref cyc) = conflict.cycle {
            let cycle_str = if cyc.len() == 1 {
                format!("{} -> {}", cyc[0], cyc[0])
            } else {
                format!("{} -> {}", cyc.join(" -> "), cyc[0])
            };
            out.push_str(&format!("> - **Cycle Path**: {}\n", cycle_str));
        }
        out.push_str("> - **Competing Candidates**:\n");
        for cand in &conflict.candidates {
            if let Some(ref ev) = cand.evidence {
                out.push_str(&format!(">   - `{}` (evidence: {})\n", cand.id, ev));
            } else {
                out.push_str(&format!(">   - `{}`\n", cand.id));
            }
        }
        out.push('\n');

        out.push_str("---\n\n");
        out.push_str("## Competing Candidates\n\n");
        for cand in &conflict.candidates {
            let title_str = cand.title.as_deref().unwrap_or(&cand.id);
            out.push_str(&format!("### {} (`{}`) [DISPUTED]\n\n", title_str, cand.id));
            out.push_str(&format!("- **ID**: `{}`\n", cand.id));
            if let Some(ref path) = cand.path {
                out.push_str(&format!("- **Path**: {}\n", path));
            }
            if let Some(ref ev) = cand.evidence {
                out.push_str(&format!("- **Evidence**: {}\n", ev));
            }
            if let Some(ref sc) = cand.scope {
                let sc_str = sc
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("- **Scope**: [{}]\n", sc_str));
            }
            out.push('\n');
        }

        if !packet.historical_rationale.is_empty() {
            out.push_str("---\n\n");
            out.push_str("## Historical Rationale (SUPERSEDED)\n\n");
            out.push_str("> [!WARNING]\n");
            out.push_str(
                "> The following note(s) are SUPERSEDED and retained solely for historical rationale and decision provenance.\n",
            );
            out.push_str("> DO NOT treat this content as current architectural guidance.\n\n");

            for item in &packet.historical_rationale {
                out.push_str(&format!(
                    "### {} (`{}`) [SUPERSEDED]\n\n",
                    item.node.title, item.node.id
                ));
                out.push_str(&format!("- **Type**: {}\n", item.node.node_type));
                out.push_str(&format!("- **Superseded By**: `{}`\n", item.superseded_by));
                if let Some(ref ev) = item.evidence {
                    out.push_str(&format!("- **Evidence**: {}\n", ev));
                }
                if let Some(ref sc) = item.scope {
                    let sc_str = sc
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!("- **Supersession Scope**: [{}]\n", sc_str));
                }
                if let Some(ref body) = item.node.body {
                    let trimmed = body.trim();
                    if !trimmed.is_empty() {
                        out.push_str("\n#### Content\n\n");
                        out.push_str(trimmed);
                        out.push('\n');
                    }
                }
                out.push('\n');
            }
        }

        return out;
    }

    let active_id = packet.active_guidance_id.as_deref().unwrap_or("none");
    out.push_str(&format!("- **Active Guidance**: `{}`\n", active_id));

    let status_str = if packet.is_current {
        "CURRENT GUIDANCE"
    } else {
        "SUPERSEDED (Resolved to successor)"
    };
    out.push_str(&format!("- **Status**: {}\n", status_str));

    if !packet.query_scope.is_empty() {
        let sc_str = packet
            .query_scope
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- **Scope Filter**: [{}]\n", sc_str));
    }

    out.push_str(&format!("- **Resolution**: {}\n\n", packet.message));

    if !packet.advisories.is_empty() {
        out.push_str("> [!NOTE]\n");
        out.push_str("> **Advisories:**\n");
        for adv in &packet.advisories {
            out.push_str(&format!("> - {}\n", adv));
        }
        out.push('\n');
    }

    if let Some(ref active_note) = packet.active_note {
        out.push_str("---\n\n");
        out.push_str("## Active Guidance\n\n");
        out.push_str(&format!(
            "### {} (`{}`)\n\n",
            active_note.title, active_note.id
        ));
        out.push_str(&format!("- **Type**: {}\n", active_note.node_type));

        if let Some(ref st) = active_note.status {
            out.push_str(&format!("- **Status**: {}\n", st));
        }
        if let Some(ref vf) = active_note.valid_from {
            out.push_str(&format!("- **Valid From**: {}\n", vf));
        }
        if let Some(ref vu) = active_note.valid_until {
            out.push_str(&format!("- **Valid Until**: {}\n", vu));
        }
        if !active_note.scope.is_empty() {
            let sc_str = active_note
                .scope
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Scope**: [{}]\n", sc_str));
        }
        if let Some(ref path) = active_note.path {
            out.push_str(&format!("- **Path**: {}\n", path));
        }

        if !active_note.relations.is_empty() {
            out.push_str(&format!(
                "- **Relations** ({}):\n",
                active_note.relations.len()
            ));
            for rel in &active_note.relations {
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

        if let Some(ref body) = active_note.body {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                out.push_str("\n#### Content\n\n");
                out.push_str(trimmed);
                out.push('\n');
            }
        }
    }

    if !packet.historical_rationale.is_empty() {
        out.push_str("\n---\n\n");
        out.push_str("## Historical Rationale (SUPERSEDED)\n\n");
        out.push_str("> [!WARNING]\n");
        out.push_str(
            "> The following note(s) are SUPERSEDED and retained solely for historical rationale and decision provenance.\n",
        );
        out.push_str("> DO NOT treat this content as current architectural guidance.\n\n");

        for item in &packet.historical_rationale {
            out.push_str(&format!(
                "### {} (`{}`) [SUPERSEDED]\n\n",
                item.node.title, item.node.id
            ));
            out.push_str(&format!("- **Type**: {}\n", item.node.node_type));
            out.push_str(&format!("- **Superseded By**: `{}`\n", item.superseded_by));
            if let Some(ref ev) = item.evidence {
                out.push_str(&format!("- **Evidence**: {}\n", ev));
            }
            if let Some(ref sc) = item.scope {
                let sc_str = sc
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("- **Supersession Scope**: [{}]\n", sc_str));
            }
            if let Some(ref body) = item.node.body {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    out.push_str("\n#### Content\n\n");
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
            out.push('\n');
        }
    }

    out
}

/// Formats a `LineagePacket` into a bounded Markdown context packet.
pub fn format_lineage_markdown(packet: &LineagePacket) -> String {
    let mut out = String::new();
    out.push_str("# Akashic Records — Decision Lineage Packet\n\n");
    out.push_str(&format!("- **Mode**: {}\n", packet.mode));
    out.push_str(&format!("- **Seed**: `{}`\n", packet.seed));
    out.push_str(&format!(
        "- **Active Guidance**: `{}`\n",
        packet.active_guidance_id
    ));
    out.push_str(&format!("- **Chain Length**: {}\n", packet.chain.len()));

    if !packet.query_scope.is_empty() {
        let sc_str = packet
            .query_scope
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- **Scope Filter**: [{}]\n", sc_str));
    }

    out.push_str(&format!("- **Resolution**: {}\n\n", packet.message));

    if !packet.advisories.is_empty() {
        out.push_str("> [!NOTE]\n");
        out.push_str("> **Advisories:**\n");
        for adv in &packet.advisories {
            out.push_str(&format!("> - {}\n", adv));
        }
        out.push_str("\n");
    }

    out.push_str("---\n\n");
    out.push_str("## Chronological Replacement Chain\n\n");

    let chain_flow = packet
        .chain
        .iter()
        .map(|entry| format!("`{}`", entry.node.id))
        .collect::<Vec<_>>()
        .join(" -> ");
    out.push_str(&format!("{}\n\n", chain_flow));

    for (i, entry) in packet.chain.iter().enumerate() {
        let status_label = if entry.is_current {
            "[CURRENT GUIDANCE]"
        } else {
            "[SUPERSEDED]"
        };
        out.push_str(&format!(
            "### {}. {} (`{}`) {}\n\n",
            i + 1,
            entry.node.title,
            entry.node.id,
            status_label
        ));
        out.push_str(&format!("- **Type**: {}\n", entry.node.node_type));
        if let Some(ref st) = entry.node.status {
            out.push_str(&format!("- **Status**: {}\n", st));
        }
        if let Some(ref vf) = entry.node.valid_from {
            out.push_str(&format!("- **Valid From**: {}\n", vf));
        }
        if let Some(ref vu) = entry.node.valid_until {
            out.push_str(&format!("- **Valid Until**: {}\n", vu));
        }
        if let Some(ref pred) = entry.supersedes {
            out.push_str(&format!("- **Supersedes**: `{}`\n", pred));
        }
        if let Some(ref ev) = entry.adopted_evidence {
            out.push_str(&format!("- **Adopted Evidence**: {}\n", ev));
        }
        if let Some(ref succ) = entry.superseded_by {
            out.push_str(&format!("- **Superseded By**: `{}`\n", succ));
        }
        if let Some(ref ev) = entry.supersession_evidence {
            out.push_str(&format!("- **Supersession Evidence**: {}\n", ev));
        }
        if !entry.supported_by.is_empty() {
            let supp_str = entry
                .supported_by
                .iter()
                .map(|s| format!("`{}`", s))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Supported By**: [{}]\n", supp_str));
        }
        if !entry.node.scope.is_empty() {
            let sc_str = entry
                .node
                .scope
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Scope**: [{}]\n", sc_str));
        }
        if let Some(ref path) = entry.node.path {
            out.push_str(&format!("- **Path**: {}\n", path));
        }
        if let Some(ref body) = entry.node.body {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                out.push_str("\n#### Content\n\n");
                out.push_str(trimmed);
                out.push('\n');
            }
        }
        out.push('\n');
    }

    if !packet.empirical_evidence.is_empty() {
        out.push_str("---\n\n");
        out.push_str("## Empirical Evidence\n\n");
        for ev_node in &packet.empirical_evidence {
            out.push_str(&format!("### {} (`{}`)\n\n", ev_node.title, ev_node.id));
            out.push_str(&format!("- **Type**: {}\n", ev_node.node_type));
            if let Some(ref st) = ev_node.status {
                out.push_str(&format!("- **Status**: {}\n", st));
            }
            if let Some(ref path) = ev_node.path {
                out.push_str(&format!("- **Path**: {}\n", path));
            }
            if let Some(ref body) = ev_node.body {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    out.push_str("\n#### Content\n\n");
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
            out.push('\n');
        }
    }

    out
}

/// Formats an `ImpactPacket` into a bounded Markdown context packet.
pub fn format_impact_markdown(packet: &ImpactPacket) -> String {
    let mut out = String::new();
    out.push_str("# Akashic Records — Impact Analysis Packet\n\n");
    out.push_str(&format!("- **Mode**: {}\n", packet.mode));
    out.push_str(&format!(
        "- **Seed**: `{}` (\"{}\")\n",
        packet.seed, packet.seed_title
    ));
    out.push_str(&format!(
        "- **Total Dependents**: {}\n",
        packet.total_dependents
    ));
    out.push_str(&format!("- **Max Depth**: {}\n", packet.max_depth));

    if !packet.query_scope.is_empty() {
        let sc_str = packet
            .query_scope
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- **Scope Filter**: [{}]\n", sc_str));
    }

    out.push_str(&format!("- **Resolution**: {}\n\n", packet.message));
    out.push_str("---\n\n");
    out.push_str("## Downstream Dependents\n\n");

    if packet.dependents.is_empty() {
        out.push_str("No downstream components depend on this note.\n");
    } else {
        for (i, dep) in packet.dependents.iter().enumerate() {
            out.push_str(&format!("### {}. {} (`{}`)\n\n", i + 1, dep.title, dep.id));
            out.push_str(&format!("- **Type**: {}\n", dep.node_type));
            out.push_str(&format!("- **Depth**: {}\n", dep.depth));
            let path_str = dep
                .path
                .iter()
                .map(|p| format!("`{}`", p))
                .collect::<Vec<_>>()
                .join(" -> ");
            out.push_str(&format!("- **Dependency Path**: {}\n", path_str));
            if !dep.scope.is_empty() {
                let sc_str = dep
                    .scope
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("- **Scope**: [{}]\n", sc_str));
            }
            if let Some(ref path) = dep.file_path {
                out.push_str(&format!("- **Path**: {}\n", path));
            }
            out.push('\n');
        }
    }

    out
}

/// Formats a `HistoricalPacket` into a bounded Markdown context packet.
pub fn format_historical_markdown(packet: &HistoricalPacket) -> String {
    let mut out = String::new();
    out.push_str("# Akashic Records — Historical Context Packet\n\n");
    out.push_str(&format!("- **Mode**: {}\n", packet.mode));
    out.push_str(&format!("- **Seed**: `{}`\n", packet.seed));
    out.push_str(&format!("- **As Of**: `{}`\n", packet.as_of));

    let active_id_str = packet
        .active_guidance_id
        .as_deref()
        .map(|id| format!("`{}`", id))
        .unwrap_or_else(|| "None".to_string());
    out.push_str(&format!("- **Active Guidance**: {}\n", active_id_str));
    out.push_str(&format!("- **Status**: {}\n", packet.status));

    if !packet.query_scope.is_empty() {
        let sc_str = packet
            .query_scope
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- **Scope Filter**: [{}]\n", sc_str));
    }

    out.push_str(&format!("- **Resolution**: {}\n\n", packet.message));

    if !packet.warnings.is_empty() {
        out.push_str("> [!WARNING]\n");
        out.push_str("> **Warnings:**\n");
        for warn in &packet.warnings {
            out.push_str(&format!("> - {}\n", warn));
        }
        out.push_str("\n");
    }

    if !packet.advisories.is_empty() {
        out.push_str("> [!NOTE]\n");
        out.push_str("> **Advisories:**\n");
        for adv in &packet.advisories {
            out.push_str(&format!("> - {}\n", adv));
        }
        out.push_str("\n");
    }

    out.push_str("---\n\n");
    out.push_str(&format!("## Active Guidance (as of {})\n\n", packet.as_of));

    if let Some(ref note) = packet.active_note {
        out.push_str(&format!("### {} (`{}`)\n\n", note.title, note.id));
        out.push_str(&format!("- **Type**: {}\n", note.node_type));
        if let Some(ref st) = note.status {
            out.push_str(&format!("- **Status**: {}\n", st));
        }
        if let Some(ref vf) = note.valid_from {
            out.push_str(&format!("- **Valid From**: {}\n", vf));
        }
        if let Some(ref vu) = note.valid_until {
            out.push_str(&format!("- **Valid Until**: {}\n", vu));
        }
        if !note.scope.is_empty() {
            let sc_str = note
                .scope
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("- **Scope**: [{}]\n", sc_str));
        }
        if let Some(ref path) = note.path {
            out.push_str(&format!("- **Path**: {}\n", path));
        }
        if let Some(ref body) = note.body {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                out.push_str("\n#### Content\n\n");
                out.push_str(trimmed);
                out.push('\n');
            }
        }
    } else {
        out.push_str(&format!(
            "No active guidance found for `{}` as of `{}`.\n",
            packet.seed, packet.as_of
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_scope_args_valid_and_invalid() {
        let args = vec!["env=production".to_string(), "system=payments".to_string()];
        let scope = parse_scope_args(&args).unwrap();
        assert_eq!(scope.get("env").map(|s| s.as_str()), Some("production"));
        assert_eq!(scope.get("system").map(|s| s.as_str()), Some("payments"));

        let comma_args = vec!["env=staging,system=checkout".to_string()];
        let scope_comma = parse_scope_args(&comma_args).unwrap();
        assert_eq!(scope_comma.get("env").map(|s| s.as_str()), Some("staging"));
        assert_eq!(
            scope_comma.get("system").map(|s| s.as_str()),
            Some("checkout")
        );

        assert!(parse_scope_args(&["invalid_format".to_string()]).is_err());
        assert!(parse_scope_args(&["=missing_key".to_string()]).is_err());
        assert!(parse_scope_args(&["missing_val=".to_string()]).is_err());
    }

    #[test]
    fn test_scope_matching() {
        let mut query_scope = Scope::new();
        let mut cand_scope = Scope::new();

        // Empty query matches anything
        assert!(scope_matches(&query_scope, &cand_scope));

        query_scope.insert("env".to_string(), "production".to_string());
        // Candidate without env restriction matches
        assert!(scope_matches(&query_scope, &cand_scope));

        // Candidate with same env matches
        cand_scope.insert("env".to_string(), "production".to_string());
        assert!(scope_matches(&query_scope, &cand_scope));

        // Candidate with different env fails
        cand_scope.insert("env".to_string(), "staging".to_string());
        assert!(!scope_matches(&query_scope, &cand_scope));
    }

    #[test]
    fn test_validate_date_format() {
        assert!(validate_date_format("2026-09-18").is_ok());
        assert!(validate_date_format("2020-01-01").is_ok());
        assert!(validate_date_format("2025-12-31").is_ok());

        assert!(validate_date_format("2026-9-18").is_err());
        assert!(validate_date_format("2026/09/18").is_err());
        assert!(validate_date_format("2026-13-01").is_err());
        assert!(validate_date_format("2026-00-01").is_err());
        assert!(validate_date_format("2026-02-32").is_err());
        assert!(validate_date_format("invalid-date").is_err());
        assert!(validate_date_format("").is_err());
    }

    #[test]
    fn test_query_seed_not_found() {
        let dir = tempdir().unwrap();
        let vault = VaultGraph::from_dir(dir.path()).unwrap();
        let err = query_current(&vault, "nonexistent", &Scope::new()).unwrap_err();
        match err {
            QueryError::SeedNotFound(seed, _) => assert_eq!(seed, "nonexistent"),
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_query_seed_current_and_superseded_chains() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Note A (obsolete)
        let note_a = r#"---
id: note-a
type: Decision
title: Architecture A
---
Body A
"#;
        // Note B (supersedes A, superseded by C)
        let note_b = r#"---
id: note-b
type: Decision
title: Architecture B
relations:
  - type: supersedes
    target: ./note-a.md
    evidence: ./reviews/perf.md
---
Body B
"#;
        // Note C (current guidance)
        let note_c = r#"---
id: note-c
type: Decision
title: Architecture C
relations:
  - type: supersedes
    target: ./note-b.md
---
Body C
"#;
        std::fs::write(root.join("note-a.md"), note_a).unwrap();
        std::fs::write(root.join("note-b.md"), note_b).unwrap();
        std::fs::write(root.join("note-c.md"), note_c).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let empty_scope = Scope::new();

        // Query current note C
        let res_c = query_current(&vault, "note-c", &empty_scope).unwrap();
        assert!(res_c.is_current);
        assert_eq!(res_c.active_guidance_id.as_deref(), Some("note-c"));
        assert!(res_c.historical_rationale.is_empty());
        assert!(res_c
            .message
            .contains("has no successors and is itself current guidance"));

        // Query superseded note A: multi-hop chain to C
        let res_a = query_current(&vault, "note-a", &empty_scope).unwrap();
        assert!(!res_a.is_current);
        assert_eq!(res_a.active_guidance_id.as_deref(), Some("note-c"));
        assert_eq!(res_a.historical_rationale.len(), 2);
        assert_eq!(res_a.historical_rationale[0].node.id, "note-a");
        assert_eq!(res_a.historical_rationale[1].node.id, "note-b");
        assert_eq!(
            res_a.historical_rationale[0].evidence.as_deref(),
            Some("./reviews/perf.md")
        );

        // Markdown packet output contains expected sections
        let md = res_a.to_markdown();
        assert!(md.contains("# Akashic Records — Agent Context Packet"));
        assert!(md.contains("## Active Guidance"));
        assert!(md.contains("Architecture C (`note-c`)"));
        assert!(md.contains("## Historical Rationale (SUPERSEDED)"));
        assert!(md.contains("> [!WARNING]"));
        assert!(md.contains("Architecture A (`note-a`) [SUPERSEDED]"));
    }

    #[test]
    fn test_query_lineage_replacement_chain_evidence_and_seed_variants() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: dec-a
type: Decision
title: Monolith
status: deprecated
valid_from: "2024-01-01"
---
Initial monolithic architecture.
"#;
        let note_b = r#"---
id: dec-b
type: Decision
title: Microservices
status: deprecated
valid_from: "2025-01-01"
relations:
  - type: supersedes
    target: ./dec-a.md
    evidence: ./reviews/scaling-issues.md
---
Transitioned to microservices for scale.
"#;
        let note_c = r#"---
id: dec-c
type: Decision
title: Modular Monolith
status: stable
valid_from: "2026-01-01"
relations:
  - type: supersedes
    target: ./dec-b.md
    evidence: ./reviews/network-overhead.md
---
Consolidated into modular monolith.
"#;
        std::fs::write(root.join("dec-a.md"), note_a).unwrap();
        std::fs::write(root.join("dec-b.md"), note_b).unwrap();
        std::fs::write(root.join("dec-c.md"), note_c).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let empty_scope = Scope::new();

        // Querying from any note in the chain (A, B, or C) should trace the entire chain
        for seed_id in &["dec-a", "dec-b", "dec-c"] {
            let lineage = query_lineage(&vault, seed_id, &empty_scope).unwrap();
            assert_eq!(lineage.mode, "lineage");
            assert_eq!(lineage.active_guidance_id, "dec-c");
            assert_eq!(lineage.chain.len(), 3);

            assert_eq!(lineage.chain[0].node.id, "dec-a");
            assert_eq!(lineage.chain[0].superseded_by.as_deref(), Some("dec-b"));
            assert_eq!(
                lineage.chain[0].supersession_evidence.as_deref(),
                Some("./reviews/scaling-issues.md")
            );
            assert_eq!(lineage.chain[0].adopted_evidence, None);
            assert!(!lineage.chain[0].is_current);

            assert_eq!(lineage.chain[1].node.id, "dec-b");
            assert_eq!(lineage.chain[1].supersedes.as_deref(), Some("dec-a"));
            assert_eq!(lineage.chain[1].superseded_by.as_deref(), Some("dec-c"));
            assert_eq!(
                lineage.chain[1].adopted_evidence.as_deref(),
                Some("./reviews/scaling-issues.md")
            );
            assert_eq!(
                lineage.chain[1].supersession_evidence.as_deref(),
                Some("./reviews/network-overhead.md")
            );
            assert!(!lineage.chain[1].is_current);

            assert_eq!(lineage.chain[2].node.id, "dec-c");
            assert_eq!(lineage.chain[2].supersedes.as_deref(), Some("dec-b"));
            assert_eq!(
                lineage.chain[2].adopted_evidence.as_deref(),
                Some("./reviews/network-overhead.md")
            );
            assert_eq!(lineage.chain[2].supersession_evidence, None);
            assert!(lineage.chain[2].is_current);

            assert_eq!(lineage.transitions.len(), 2);
            assert_eq!(lineage.transitions[0].predecessor, "dec-a");
            assert_eq!(lineage.transitions[0].successor, "dec-b");
            assert_eq!(
                lineage.transitions[0].evidence.as_deref(),
                Some("./reviews/scaling-issues.md")
            );

            assert_eq!(lineage.transitions[1].predecessor, "dec-b");
            assert_eq!(lineage.transitions[1].successor, "dec-c");
            assert_eq!(
                lineage.transitions[1].evidence.as_deref(),
                Some("./reviews/network-overhead.md")
            );

            let md = lineage.to_markdown();
            assert!(md.contains("# Akashic Records — Decision Lineage Packet"));
            assert!(md.contains("`dec-a` -> `dec-b` -> `dec-c`"));
            assert!(md.contains("**Supersession Evidence**: ./reviews/scaling-issues.md"));
            assert!(md.contains("**Adopted Evidence**: ./reviews/scaling-issues.md"));
        }
    }

    #[test]
    fn test_query_lineage_cycle_and_conflict_handling() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Cycle notes
        let cyc1 = r#"---
id: cyc-1
type: Decision
title: Cyc 1
relations:
  - type: supersedes
    target: ./cyc-2.md
---
C1
"#;
        let cyc2 = r#"---
id: cyc-2
type: Decision
title: Cyc 2
relations:
  - type: supersedes
    target: ./cyc-1.md
---
C2
"#;
        std::fs::write(root.join("cyc-1.md"), cyc1).unwrap();
        std::fs::write(root.join("cyc-2.md"), cyc2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let err_cyc = query_lineage(&vault, "cyc-1", &Scope::new()).unwrap_err();
        match err_cyc {
            QueryError::SupersessionCycle(c) => {
                assert!(c.contains("cyc-1"));
                assert!(c.contains("cyc-2"));
            }
            other => panic!("Expected SupersessionCycle, got {:?}", other),
        }

        // Competing successors
        let dir_comp = tempdir().unwrap();
        let root_comp = dir_comp.path();
        let base = r#"---
id: base
type: Decision
title: Base
---
Base
"#;
        let s1 = r#"---
id: s1
type: Decision
title: S1
relations:
  - type: supersedes
    target: ./base.md
---
S1
"#;
        let s2 = r#"---
id: s2
type: Decision
title: S2
relations:
  - type: supersedes
    target: ./base.md
---
S2
"#;
        std::fs::write(root_comp.join("base.md"), base).unwrap();
        std::fs::write(root_comp.join("s1.md"), s1).unwrap();
        std::fs::write(root_comp.join("s2.md"), s2).unwrap();

        let vault_comp = VaultGraph::from_dir(root_comp).unwrap();
        let err_comp = query_lineage(&vault_comp, "base", &Scope::new()).unwrap_err();
        match err_comp {
            QueryError::UnresolvedConflict(msg) => {
                assert!(msg.contains("Competing successors found for 'base'"));
            }
            other => panic!("Expected UnresolvedConflict, got {:?}", other),
        }
    }

    #[test]
    fn test_query_impact_transitive_dependents_depth_and_paths() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let db = r#"---
id: core-db
type: Infrastructure
title: Core Database
---
Postgres cluster.
"#;
        let auth = r#"---
id: auth-service
type: Service
title: Authentication Service
relations:
  - type: depends_on
    target: ./core-db.md
---
Auth JWT service.
"#;
        let billing = r#"---
id: billing-service
type: Service
title: Billing Service
relations:
  - type: depends_on
    target: ./core-db.md
---
Stripe billing integration.
"#;
        let web = r#"---
id: web-portal
type: Application
title: Customer Web Portal
relations:
  - type: depends_on
    target: ./auth-service.md
---
React web portal.
"#;
        let payment_job = r#"---
id: payment-job
type: CronJob
title: Nightly Payment Reconciliation
relations:
  - type: depends_on
    target: ./billing-service.md
---
Reconciliation job.
"#;
        std::fs::write(root.join("core-db.md"), db).unwrap();
        std::fs::write(root.join("auth-service.md"), auth).unwrap();
        std::fs::write(root.join("billing-service.md"), billing).unwrap();
        std::fs::write(root.join("web-portal.md"), web).unwrap();
        std::fs::write(root.join("payment-job.md"), payment_job).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let empty_scope = Scope::new();

        let impact = query_impact(&vault, "core-db", &empty_scope).unwrap();
        assert_eq!(impact.mode, "impact");
        assert_eq!(impact.seed, "core-db");
        assert_eq!(impact.total_dependents, 4);
        assert_eq!(impact.max_depth, 2);

        // Check depth 1 dependents
        let d1: Vec<_> = impact.dependents.iter().filter(|d| d.depth == 1).collect();
        assert_eq!(d1.len(), 2);
        assert_eq!(d1[0].id, "auth-service");
        assert_eq!(d1[0].path, vec!["core-db", "auth-service"]);
        assert_eq!(d1[1].id, "billing-service");
        assert_eq!(d1[1].path, vec!["core-db", "billing-service"]);

        // Check depth 2 dependents
        let d2: Vec<_> = impact.dependents.iter().filter(|d| d.depth == 2).collect();
        assert_eq!(d2.len(), 2);
        assert_eq!(d2[0].id, "web-portal");
        assert_eq!(d2[0].path, vec!["core-db", "auth-service", "web-portal"]);
        assert_eq!(d2[1].id, "payment-job");
        assert_eq!(
            d2[1].path,
            vec!["core-db", "billing-service", "payment-job"]
        );

        let md = impact.to_markdown();
        assert!(md.contains("# Akashic Records — Impact Analysis Packet"));
        assert!(md.contains("- **Total Dependents**: 4"));
        assert!(md.contains("- **Max Depth**: 2"));
        assert!(md.contains("Customer Web Portal (`web-portal`)"));

        // Query leaf note: no dependents
        let impact_leaf = query_impact(&vault, "web-portal", &empty_scope).unwrap();
        assert_eq!(impact_leaf.total_dependents, 0);
        assert_eq!(impact_leaf.max_depth, 0);
        assert!(impact_leaf.dependents.is_empty());
        assert!(impact_leaf
            .message
            .contains("No downstream components depend"));
    }

    #[test]
    fn test_query_historical_past_intermediate_future_and_lifecycle_intervals() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: dec-2024
type: Decision
title: Architecture 2024
valid_from: "2024-01-01"
valid_until: "2024-12-31"
---
2024 guidance
"#;
        let note_b = r#"---
id: dec-2025
type: Decision
title: Architecture 2025
valid_from: "2025-01-01"
valid_until: "2025-12-31"
relations:
  - type: supersedes
    target: ./dec-2024.md
---
2025 guidance
"#;
        let note_c = r#"---
id: dec-2026
type: Decision
title: Architecture 2026
valid_from: "2026-01-01"
relations:
  - type: supersedes
    target: ./dec-2025.md
---
2026 guidance
"#;
        std::fs::write(root.join("dec-2024.md"), note_a).unwrap();
        std::fs::write(root.join("dec-2025.md"), note_b).unwrap();
        std::fs::write(root.join("dec-2026.md"), note_c).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let scope = Scope::new();

        // 1. Date before valid_from
        let res_early = query_historical(&vault, "dec-2024", Some("2023-06-01"), &scope).unwrap();
        assert!(!res_early.is_active);
        assert_eq!(res_early.status, "not_yet_valid");
        assert!(res_early.active_guidance_id.is_none());
        assert!(res_early.active_note.is_none());
        assert!(res_early.message.contains("not yet valid"));

        // 2. Date in the past (2024)
        let res_past = query_historical(&vault, "dec-2024", Some("2024-06-15"), &scope).unwrap();
        assert!(res_past.is_active);
        assert_eq!(res_past.status, "active");
        assert_eq!(res_past.active_guidance_id.as_deref(), Some("dec-2024"));
        assert_eq!(res_past.active_note.as_ref().unwrap().id, "dec-2024");

        // 3. Intermediate date (2025) queried on seed dec-2024
        let res_mid = query_historical(&vault, "dec-2024", Some("2025-07-01"), &scope).unwrap();
        assert!(res_mid.is_active);
        assert_eq!(res_mid.status, "active");
        assert_eq!(res_mid.active_guidance_id.as_deref(), Some("dec-2025"));
        assert_eq!(res_mid.active_note.as_ref().unwrap().id, "dec-2025");

        // 4. Future / Current date (2026) queried on seed dec-2024
        let res_future = query_historical(&vault, "dec-2024", Some("2026-06-01"), &scope).unwrap();
        assert!(res_future.is_active);
        assert_eq!(res_future.status, "active");
        assert_eq!(res_future.active_guidance_id.as_deref(), Some("dec-2026"));
        assert_eq!(res_future.active_note.as_ref().unwrap().id, "dec-2026");

        // Missing as_of error
        let err_missing = query_historical(&vault, "dec-2024", None, &scope).unwrap_err();
        match err_missing {
            QueryError::MissingAsOf => {}
            other => panic!("Expected MissingAsOf, got {:?}", other),
        }

        // Invalid date format error
        let err_bad_date =
            query_historical(&vault, "dec-2024", Some("2026-9-1"), &scope).unwrap_err();
        match err_bad_date {
            QueryError::InvalidDateFormat(d) => assert_eq!(d, "2026-9-1"),
            other => panic!("Expected InvalidDateFormat, got {:?}", other),
        }
    }

    #[test]
    fn test_query_historical_missing_dates_warning_indicator() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note = r#"---
id: note-no-dates
type: Decision
title: Undated Decision
---
Undated content
"#;
        std::fs::write(root.join("note-no-dates.md"), note).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let scope = Scope::new();

        let res = query_historical(&vault, "note-no-dates", Some("2026-06-01"), &scope).unwrap();
        assert!(!res.is_active);
        assert_eq!(res.status, "indeterminate");
        assert_eq!(res.active_guidance_id, None);
        assert!(!res.warnings.is_empty());
        assert!(res.warnings[0].contains("lacks lifecycle dates"));

        let md = res.to_markdown();
        assert!(md.contains("# Akashic Records — Historical Context Packet"));
        assert!(md.contains("> [!WARNING]"));
        assert!(md.contains("lacks lifecycle dates"));
    }

    #[test]
    fn test_query_scoped_supersession_matching_and_advisory() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let base_note = r#"---
id: dec-base
type: Decision
title: Base Decision
---
Base Content
"#;
        let prod_note = r#"---
id: dec-prod
type: Decision
title: Production Decision
scope:
  env: production
relations:
  - type: supersedes
    target: ./dec-base.md
---
Prod Content
"#;
        std::fs::write(root.join("dec-base.md"), base_note).unwrap();
        std::fs::write(root.join("dec-prod.md"), prod_note).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();

        // Query with matching scope
        let mut prod_scope = Scope::new();
        prod_scope.insert("env".to_string(), "production".to_string());
        let res_prod = query_current(&vault, "dec-base", &prod_scope).unwrap();
        assert_eq!(res_prod.active_guidance_id.as_deref(), Some("dec-prod"));
        assert_eq!(res_prod.historical_rationale.len(), 1);

        // Query with non-matching scope
        let mut staging_scope = Scope::new();
        staging_scope.insert("env".to_string(), "staging".to_string());
        let res_staging = query_current(&vault, "dec-base", &staging_scope).unwrap();
        assert_eq!(res_staging.active_guidance_id.as_deref(), Some("dec-base"));
        assert!(res_staging.is_current);
        assert_eq!(res_staging.historical_rationale.len(), 0);
        assert_eq!(res_staging.advisories.len(), 1);
        assert!(
            res_staging.advisories[0].contains("A successor exists in another scope: 'dec-prod'")
        );

        let md_staging = res_staging.to_markdown();
        assert!(md_staging.contains("> [!NOTE]"));
        assert!(md_staging.contains("dec-prod"));
    }

    #[test]
    fn test_query_supersession_cycle_abstention() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note1 = r#"---
id: cyc-1
type: Decision
title: Cycle 1
relations:
  - type: supersedes
    target: ./cyc-2.md
    evidence: ./evidence1.md
---
Content 1
"#;
        let note2 = r#"---
id: cyc-2
type: Decision
title: Cycle 2
relations:
  - type: supersedes
    target: ./cyc-1.md
    evidence: ./evidence2.md
---
Content 2
"#;
        std::fs::write(root.join("cyc-1.md"), note1).unwrap();
        std::fs::write(root.join("cyc-2.md"), note2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let packet = query_current(&vault, "cyc-1", &Scope::new()).unwrap();
        assert_eq!(packet.status, "unresolved_conflict");
        assert!(packet.active_guidance_id.is_none());
        assert!(packet.active_note.is_none());
        assert!(packet.unresolved_conflict.is_some());
        let conflict = packet.unresolved_conflict.as_ref().unwrap();
        assert_eq!(conflict.alert, "[UNRESOLVED_CONFLICT]");
        assert_eq!(conflict.kind, "supersession_cycle");
        assert!(conflict.message.contains("Supersession cycle detected"));
        assert!(conflict.cycle.is_some());
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.candidates[0].id, "cyc-1");
        assert_eq!(conflict.candidates[1].id, "cyc-2");

        let md = packet.to_markdown();
        assert!(md.contains("[UNRESOLVED_CONFLICT]"));
        assert!(md.contains("## Competing Candidates"));
        assert!(md.contains("[DISPUTED]"));
    }

    #[test]
    fn test_query_competing_successors_abstention() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_base = r#"---
id: comp-base
type: Decision
title: Competing Base
---
Base
"#;
        let note_s1 = r#"---
id: comp-succ1
type: Decision
title: Successor 1
relations:
  - type: supersedes
    target: ./comp-base.md
    evidence: ./reviews/s1.md
---
Succ 1
"#;
        let note_s2 = r#"---
id: comp-succ2
type: Decision
title: Successor 2
relations:
  - type: supersedes
    target: ./comp-base.md
    evidence: ./reviews/s2.md
---
Succ 2
"#;
        std::fs::write(root.join("comp-base.md"), note_base).unwrap();
        std::fs::write(root.join("comp-succ1.md"), note_s1).unwrap();
        std::fs::write(root.join("comp-succ2.md"), note_s2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let packet = query_current(&vault, "comp-base", &Scope::new()).unwrap();
        assert_eq!(packet.status, "unresolved_conflict");
        assert!(packet.active_guidance_id.is_none());
        assert!(packet.active_note.is_none());
        assert!(packet.unresolved_conflict.is_some());
        let conflict = packet.unresolved_conflict.as_ref().unwrap();
        assert_eq!(conflict.alert, "[UNRESOLVED_CONFLICT]");
        assert_eq!(conflict.kind, "competing_successors");
        assert_eq!(conflict.predecessor.as_deref(), Some("comp-base"));
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.candidates[0].id, "comp-succ1");
        assert_eq!(
            conflict.candidates[0].evidence.as_deref(),
            Some("./reviews/s1.md")
        );
        assert_eq!(conflict.candidates[1].id, "comp-succ2");
        assert_eq!(
            conflict.candidates[1].evidence.as_deref(),
            Some("./reviews/s2.md")
        );

        let md = packet.to_markdown();
        assert!(md.contains("[UNRESOLVED_CONFLICT]"));
        assert!(md.contains("## Competing Candidates"));
        assert!(md.contains("comp-succ1"));
        assert!(md.contains("comp-succ2"));
        assert!(md.contains("[DISPUTED]"));
    }

    #[test]
    fn test_query_active_contradiction_abstention() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: contra-x
type: Decision
title: Option X
status: stable
scope:
  tier: backend
relations:
  - type: contradicts
    target: ./contra-y.md
    evidence: ./reviews/divergence.md
---
Option X
"#;
        let note_y = r#"---
id: contra-y
type: Decision
title: Option Y
status: stable
scope:
  tier: backend
relations:
  - type: contradicts
    target: ./contra-x.md
    evidence: ./reviews/divergence.md
---
Option Y
"#;

        std::fs::write(root.join("contra-x.md"), note_a).unwrap();
        std::fs::write(root.join("contra-y.md"), note_y).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();

        // Query matching scope
        let mut scope = Scope::new();
        scope.insert("tier".to_string(), "backend".to_string());

        let packet = query_current(&vault, "contra-x", &scope).unwrap();
        assert_eq!(packet.status, "unresolved_conflict");
        assert!(packet.active_guidance_id.is_none());
        assert!(packet.active_note.is_none());
        assert!(packet.unresolved_conflict.is_some());

        let conflict = packet.unresolved_conflict.unwrap();
        assert_eq!(conflict.kind, "contradiction");
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.candidates[0].id, "contra-x");
        assert_eq!(conflict.candidates[1].id, "contra-y");
        assert_eq!(
            conflict.candidates[0].evidence.as_deref(),
            Some("./reviews/divergence.md")
        );

        // If queried in a disjoint scope, contradiction is not in scope
        let mut disjoint_scope = Scope::new();
        disjoint_scope.insert("tier".to_string(), "frontend".to_string());
        let res_disjoint = query_current(&vault, "contra-x", &disjoint_scope).unwrap();
        assert_eq!(res_disjoint.status, "current");
        assert_eq!(res_disjoint.active_guidance_id.as_deref(), Some("contra-x"));
    }

    #[test]
    fn test_query_transitive_supersession_resolution() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let base = r#"---
id: base
type: Decision
title: Base
---
Base
"#;
        let a = r#"---
id: a
type: Decision
title: A
relations:
  - type: supersedes
    target: ./base.md
---
A
"#;
        let x = r#"---
id: x
type: Decision
title: X
relations:
  - type: supersedes
    target: ./a.md
---
X
"#;
        let b = r#"---
id: b
type: Decision
title: B
relations:
  - type: supersedes
    target: ./base.md
  - type: supersedes
    target: ./x.md
---
B
"#;

        std::fs::write(root.join("base.md"), base).unwrap();
        std::fs::write(root.join("a.md"), a).unwrap();
        std::fs::write(root.join("x.md"), x).unwrap();
        std::fs::write(root.join("b.md"), b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();

        let idx_a = vault.resolve_seed("a").unwrap();
        let idx_b = vault.resolve_seed("b").unwrap();
        let scope = Scope::new();

        assert!(transitively_supersedes(&vault, idx_b, idx_a, &scope));
        assert!(!transitively_supersedes(&vault, idx_a, idx_b, &scope));

        let packet = query_current(&vault, "base", &scope).unwrap();
        assert_eq!(packet.status, "superseded");
        assert_eq!(packet.active_guidance_id.as_deref(), Some("b"));
    }

    #[test]
    fn test_query_predecessor_cyclic_successors_abstains() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let pred = r#"---
id: pred
type: Decision
title: Predecessor
---
Pred
"#;
        let s1 = r#"---
id: s1
type: Decision
title: S1
relations:
  - type: supersedes
    target: ./pred.md
  - type: supersedes
    target: ./s2.md
---
S1
"#;
        let s2 = r#"---
id: s2
type: Decision
title: S2
relations:
  - type: supersedes
    target: ./pred.md
  - type: supersedes
    target: ./s1.md
---
S2
"#;

        std::fs::write(root.join("pred.md"), pred).unwrap();
        std::fs::write(root.join("s1.md"), s1).unwrap();
        std::fs::write(root.join("s2.md"), s2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let packet = query_current(&vault, "pred", &Scope::new()).unwrap();

        assert_eq!(packet.status, "unresolved_conflict");
        assert!(packet.active_guidance_id.is_none());
        let conflict = packet.unresolved_conflict.unwrap();
        assert_eq!(conflict.kind, "supersession_cycle");
        assert_eq!(conflict.predecessor.as_deref(), Some("pred"));
        assert_eq!(conflict.candidates.len(), 2);
    }

    #[test]
    fn test_query_historical_overlapping_validity_strict_abstention() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Two notes both valid on 2026-06-01 where one supersedes the other without a valid_from boundary to terminate it cleanly
        // Or directly competing active notes matching as_of
        let note1 = r#"---
id: hist-conf-1
type: Decision
title: Conflict 1
valid_from: 2026-01-01
valid_until: 2026-12-31
relations:
  - type: supersedes
    target: ./hist-conf-2.md
    evidence: benchmark-1.md
---
Content 1
"#;
        let note2 = r#"---
id: hist-conf-2
type: Decision
title: Conflict 2
valid_from: 2026-01-01
valid_until: 2026-12-31
relations:
  - type: supersedes
    target: ./hist-conf-1.md
    evidence: benchmark-2.md
---
Content 2
"#;
        std::fs::write(root.join("hist-conf-1.md"), note1).unwrap();
        std::fs::write(root.join("hist-conf-2.md"), note2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let err =
            query_historical(&vault, "hist-conf-1", Some("2026-06-01"), &Scope::new()).unwrap_err();
        match err {
            QueryError::SupersessionCycle(_) => {
                // Detected cycle
            }
            QueryError::UnresolvedConflict(msg) => {
                assert!(msg.contains("[UNRESOLVED_CONFLICT]"));
            }
            other => panic!("Expected conflict or cycle, got {:?}", other),
        }
    }

    #[test]
    fn test_query_lineage_empirical_evidence_supported_by() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let evidence_note = r#"---
id: ev-benchmarks
type: Benchmark
title: Benchmark Findings 2026
---
Throughput results.
"#;
        let dec_note = r#"---
id: dec-supported
type: Decision
title: High Throughput Queue
relations:
  - type: supported_by
    target: ./ev-benchmarks.md
---
Architecture definition.
"#;
        std::fs::write(root.join("ev-benchmarks.md"), evidence_note).unwrap();
        std::fs::write(root.join("dec-supported.md"), dec_note).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let packet = query_lineage(&vault, "dec-supported", &Scope::new()).unwrap();
        assert_eq!(packet.empirical_evidence.len(), 1);
        assert_eq!(packet.empirical_evidence[0].id, "ev-benchmarks");
        assert_eq!(packet.chain[0].supported_by, vec!["ev-benchmarks"]);

        let md = packet.to_markdown();
        assert!(md.contains("## Empirical Evidence"));
        assert!(md.contains("Benchmark Findings 2026 (`ev-benchmarks`)"));
        assert!(md.contains("- **Supported By**: [`ev-benchmarks`]"));
    }

    #[test]
    fn test_query_historical_successor_terminates_predecessor_and_never_resurrects() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: dec-old
type: Decision
title: Old Decision
valid_from: 2024-01-01
valid_until: 2026-12-31
---
Old
"#;
        let note_b = r#"---
id: dec-new
type: Decision
title: New Decision
valid_from: 2025-01-01
valid_until: 2025-06-01
relations:
  - type: supersedes
    target: ./dec-old.md
---
New
"#;
        std::fs::write(root.join("dec-old.md"), note_a).unwrap();
        std::fs::write(root.join("dec-new.md"), note_b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let scope = Scope::new();

        // 1. In 2024: dec-old is active
        let res_2024 = query_historical(&vault, "dec-old", Some("2024-06-01"), &scope).unwrap();
        assert_eq!(res_2024.status, "active");
        assert_eq!(res_2024.active_guidance_id.as_deref(), Some("dec-old"));

        // 2. In early 2025: dec-new is active (dec-old terminated at dec-new's valid_from)
        let res_early_2025 =
            query_historical(&vault, "dec-old", Some("2025-03-01"), &scope).unwrap();
        assert_eq!(res_early_2025.status, "active");
        assert_eq!(
            res_early_2025.active_guidance_id.as_deref(),
            Some("dec-new")
        );

        // 3. In late 2025: dec-new expired on 2025-06-01. dec-old MUST NEVER be resurrected!
        let res_late_2025 =
            query_historical(&vault, "dec-old", Some("2025-08-01"), &scope).unwrap();
        assert_eq!(res_late_2025.status, "expired");
        assert!(!res_late_2025.is_active);
        assert_eq!(res_late_2025.active_guidance_id, None);
    }
}
