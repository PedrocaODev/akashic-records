use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::{HashMap, HashSet};

use crate::graph::VaultGraph;
use crate::model::{
    scope_covers, scopes_overlap, Finding, FindingKind, FindingSeverity, LintReport, Node,
    RelationType,
};

/// Orders the nodes in a strongly connected component along a directed cycle path.
/// Starts deterministically at the lexicographically smallest node ID.
fn order_cycle_nodes(graph: &DiGraph<String, ()>, scc: &[NodeIndex]) -> Vec<String> {
    let scc_set: HashSet<NodeIndex> = scc.iter().copied().collect();
    let min_idx = *scc.iter().min_by_key(|&&idx| &graph[idx]).unwrap();

    let mut path = vec![min_idx];
    let mut current = min_idx;
    let mut visited = HashSet::new();
    visited.insert(current);

    while let Some(next) = graph
        .neighbors_directed(current, Direction::Outgoing)
        .find(|n| scc_set.contains(n) && (!visited.contains(n) || *n == min_idx))
    {
        if next == min_idx {
            break;
        }
        visited.insert(next);
        path.push(next);
        current = next;
    }

    // In case there are nodes in SCC not in simple cycle path, append them deterministically
    if path.len() < scc.len() {
        let path_set: HashSet<NodeIndex> = path.iter().copied().collect();
        let mut remaining: Vec<NodeIndex> = scc
            .iter()
            .copied()
            .filter(|idx| !path_set.contains(idx))
            .collect();
        remaining.sort_by_key(|&idx| &graph[idx]);
        path.extend(remaining);
    }

    path.into_iter().map(|idx| graph[idx].clone()).collect()
}

/// Identifies arbitrary N-node cycles in `supersedes` relations using Tarjan's SCC algorithm.
/// Returns a list of cycles, each represented as an ordered vector of note IDs.
pub fn find_supersession_cycles(vault: &VaultGraph) -> Vec<Vec<String>> {
    let mut scc_graph = DiGraph::<String, ()>::new();
    let mut node_indices = HashMap::<String, NodeIndex>::new();

    // Build directed graph of only `supersedes` relations
    for edge in vault.graph.edge_references() {
        if edge.weight().relation_type == RelationType::Supersedes {
            let source_id = vault.graph[edge.source()].id.clone();
            let target_id = vault.graph[edge.target()].id.clone();

            let s_idx = *node_indices
                .entry(source_id.clone())
                .or_insert_with(|| scc_graph.add_node(source_id));
            let t_idx = *node_indices
                .entry(target_id.clone())
                .or_insert_with(|| scc_graph.add_node(target_id));

            scc_graph.add_edge(s_idx, t_idx, ());
        }
    }

    let sccs = tarjan_scc(&scc_graph);
    let mut cycles = Vec::new();

    for scc in sccs {
        if scc.len() > 1 {
            let cycle = order_cycle_nodes(&scc_graph, &scc);
            cycles.push(cycle);
        } else if scc.len() == 1 {
            let node_idx = scc[0];
            if scc_graph.contains_edge(node_idx, node_idx) {
                cycles.push(vec![scc_graph[node_idx].clone()]);
            }
        }
    }

    cycles.sort_by(|a, b| a.first().cmp(&b.first()));
    cycles
}

/// Detects supersession cycles and formats them as graph findings.
pub fn detect_supersession_cycles(vault: &VaultGraph) -> Vec<Finding> {
    let cycles = find_supersession_cycles(vault);
    let mut findings = Vec::new();

    for cycle in cycles {
        let cycle_str = if cycle.len() == 1 {
            format!("{} -> {}", cycle[0], cycle[0])
        } else {
            format!("{} -> {}", cycle.join(" -> "), cycle[0])
        };

        findings.push(Finding {
            severity: FindingSeverity::Error,
            kind: FindingKind::SupersessionCycle,
            message: format!("Supersession cycle detected: {}", cycle_str),
            source: cycle.first().cloned(),
            target: None,
            cycle: Some(cycle),
        });
    }

    findings
}

/// Detects broken relation targets where a relation target does not resolve to an existing note in the vault.
pub fn detect_broken_targets(vault: &VaultGraph) -> Vec<Finding> {
    let mut findings = Vec::new();

    for node_idx in vault.graph.node_indices() {
        let node = &vault.graph[node_idx];
        let source_path = node.path.as_deref().unwrap_or(&node.id);

        for rel in &node.relations {
            if vault.resolve_target(source_path, &rel.target).is_none() {
                findings.push(Finding {
                    severity: FindingSeverity::Error,
                    kind: FindingKind::BrokenTarget,
                    message: format!(
                        "Node '{}' has relation '{}' targeting '{}' which does not resolve to any note in the vault",
                        node.id, rel.relation_type, rel.target
                    ),
                    source: Some(node.id.clone()),
                    target: Some(rel.target.clone()),
                    cycle: None,
                });
            }
        }
    }

    findings.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.message.cmp(&b.message))
    });

    findings
}

/// Determines whether a node is currently active (not explicitly deprecated/archived/superseded).
pub fn is_node_active(vault: &VaultGraph, node: &Node) -> bool {
    if let Some(ref status) = node.status {
        let s = status.to_lowercase();
        if s == "deprecated"
            || s == "superseded"
            || s == "archived"
            || s == "retired"
            || s == "obsolete"
        {
            return false;
        }
    }

    if let Some(&node_idx) = vault.node_by_id.get(&node.id) {
        for edge_ref in vault.graph.edges_directed(node_idx, Direction::Incoming) {
            if edge_ref.weight().relation_type == RelationType::Supersedes {
                let succ_node = &vault.graph[edge_ref.source()];
                let succ_scope = edge_ref.weight().scope.as_ref().unwrap_or(&succ_node.scope);
                if scopes_overlap(succ_scope, &node.scope) {
                    return false;
                }
            }
        }
    }

    true
}

/// Flags conflicting assertions, including:
/// 1. Active `contradicts` edges between notes with overlapping scopes.
/// 2. Competing successors superseding the same predecessor in overlapping scopes without superseding each other.
pub fn detect_conflicting_assertions(vault: &VaultGraph) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen_pairs = HashSet::new();

    // 1. Active contradicts edges in overlapping scopes
    for edge in vault.graph.edge_references() {
        if edge.weight().relation_type == RelationType::Contradicts {
            let node_a = &vault.graph[edge.source()];
            let node_b = &vault.graph[edge.target()];

            let mut pair = [node_a.id.clone(), node_b.id.clone()];
            pair.sort();
            if !seen_pairs.insert((pair[0].clone(), pair[1].clone())) {
                continue;
            }

            if !is_node_active(vault, node_a) || !is_node_active(vault, node_b) {
                continue;
            }

            let scope_a = edge.weight().scope.as_ref().unwrap_or(&node_a.scope);
            let scope_b = &node_b.scope;

            if scopes_overlap(scope_a, scope_b) {
                findings.push(Finding {
                    severity: FindingSeverity::Error,
                    kind: FindingKind::ConflictingAssertion,
                    message: format!(
                        "Active contradiction between '{}' and '{}' with overlapping scopes",
                        pair[0], pair[1]
                    ),
                    source: Some(pair[0].clone()),
                    target: Some(pair[1].clone()),
                    cycle: None,
                });
            }
        }
    }

    // 2. Competing successors in overlapping scopes
    for node_idx in vault.graph.node_indices() {
        let pred_node = &vault.graph[node_idx];
        let mut successors = Vec::new();

        for edge in vault.graph.edges_directed(node_idx, Direction::Incoming) {
            if edge.weight().relation_type == RelationType::Supersedes {
                let succ_node = &vault.graph[edge.source()];
                if is_node_active(vault, succ_node) {
                    let succ_scope = edge.weight().scope.as_ref().unwrap_or(&succ_node.scope);
                    successors.push((edge.source(), succ_node, succ_scope));
                }
            }
        }

        if successors.len() > 1 {
            for i in 0..successors.len() {
                for j in (i + 1)..successors.len() {
                    let (idx1, s1, scope1) = successors[i];
                    let (idx2, s2, scope2) = successors[j];

                    let s1_supersedes_s2 = vault
                        .graph
                        .edges_connecting(idx1, idx2)
                        .any(|e| e.weight().relation_type == RelationType::Supersedes);
                    let s2_supersedes_s1 = vault
                        .graph
                        .edges_connecting(idx2, idx1)
                        .any(|e| e.weight().relation_type == RelationType::Supersedes);

                    if !s1_supersedes_s2 && !s2_supersedes_s1 && scopes_overlap(scope1, scope2) {
                        let mut pair = [s1.id.clone(), s2.id.clone()];
                        pair.sort();
                        if seen_pairs.insert((format!("competing:{}", pair[0]), pair[1].clone())) {
                            findings.push(Finding {
                                severity: FindingSeverity::Error,
                                kind: FindingKind::ConflictingAssertion,
                                message: format!(
                                    "Competing successors: '{}' and '{}' both supersede '{}' in overlapping scopes without superseding each other",
                                    pair[0], pair[1], pred_node.id
                                ),
                                source: Some(pair[0].clone()),
                                target: Some(pair[1].clone()),
                                cycle: None,
                            });
                        }
                    }
                }
            }
        }
    }

    findings.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.message.cmp(&b.message))
    });

    findings
}

/// Detects supersession relations where successor and predecessor scopes contradict each other.
pub fn detect_scope_mismatches(vault: &VaultGraph) -> Vec<Finding> {
    let mut findings = Vec::new();

    for edge in vault.graph.edge_references() {
        if edge.weight().relation_type == RelationType::Supersedes {
            let succ_node = &vault.graph[edge.source()];
            let pred_node = &vault.graph[edge.target()];
            let succ_scope = edge.weight().scope.as_ref().unwrap_or(&succ_node.scope);
            let pred_scope = &pred_node.scope;

            if !scope_covers(succ_scope, pred_scope) {
                findings.push(Finding {
                    severity: FindingSeverity::Error,
                    kind: FindingKind::ScopeMismatch,
                    message: format!(
                        "Node '{}' cannot supersede '{}': conflicting scopes ({:?} vs {:?})",
                        succ_node.id, pred_node.id, succ_scope, pred_scope
                    ),
                    source: Some(succ_node.id.clone()),
                    target: Some(pred_node.id.clone()),
                    cycle: None,
                });
            }
        }
    }

    findings.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.message.cmp(&b.message))
    });

    findings
}

/// Collects findings for malformed files encountered during traversal.
pub fn detect_malformed_files(vault: &VaultGraph) -> Vec<Finding> {
    let mut findings = Vec::new();

    for malformed in &vault.malformed_files {
        findings.push(Finding {
            severity: FindingSeverity::Error,
            kind: FindingKind::MalformedFile,
            message: format!("Malformed file '{}': {}", malformed.path, malformed.error),
            source: Some(malformed.path.clone()),
            target: None,
            cycle: None,
        });
    }

    findings.sort_by(|a, b| a.source.cmp(&b.source));
    findings
}

/// Lints the vault graph for all integrity violations, returning a complete `LintReport`.
pub fn lint_vault(vault: &VaultGraph) -> LintReport {
    let mut report = LintReport::new(
        &vault.vault_path,
        vault.graph.node_count(),
        vault.graph.edge_count(),
    );

    // 1. Broken targets
    for finding in detect_broken_targets(vault) {
        report.add_finding(finding);
    }

    // 2. Supersession cycles (Tarjan SCC)
    for finding in detect_supersession_cycles(vault) {
        report.add_finding(finding);
    }

    // 3. Conflicting assertions
    for finding in detect_conflicting_assertions(vault) {
        report.add_finding(finding);
    }

    // 4. Scope mismatches
    for finding in detect_scope_mismatches(vault) {
        report.add_finding(finding);
    }

    // 5. Malformed files
    for finding in detect_malformed_files(vault) {
        report.add_finding(finding);
    }

    // Deterministic sort
    report.findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.kind.to_string().cmp(&b.kind.to_string()))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.message.cmp(&b.message))
    });

    report
}

/// Formats a LintReport into a human-readable summary string.
pub fn format_lint_human(report: &LintReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Vault: {}\n", report.vault_path));
    out.push_str(&format!(
        "Checked {} nodes and {} edges.\n",
        report.total_nodes, report.total_edges
    ));

    if report.healthy {
        out.push_str("✓ Graph is healthy. No integrity violations found.\n");
    } else {
        out.push_str(&format!(
            "\nIntegrity Violations ({}):\n",
            report.findings.len()
        ));
        for finding in &report.findings {
            out.push_str(&format!(
                "  - [{}] [{}] {}\n",
                finding.severity, finding.kind, finding.message
            ));
        }
        out.push_str(&format!(
            "\nLint failed with {} error(s) and {} warning(s).\n",
            report.errors, report.warnings
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_2_node_cycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: node-a
type: Decision
title: Node A
relations:
  - type: supersedes
    target: node-b
---
A
"#;
        let note_b = r#"---
id: node-b
type: Decision
title: Node B
relations:
  - type: supersedes
    target: node-a
---
B
"#;
        fs::write(root.join("a.md"), note_a).unwrap();
        fs::write(root.join("b.md"), note_b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let cycles = find_supersession_cycles(&vault);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["node-a", "node-b"]);

        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::SupersessionCycle);
        assert!(report.findings[0]
            .message
            .contains("node-a -> node-b -> node-a"));
    }

    #[test]
    fn test_detect_3_node_cycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: canary-a
type: Decision
title: Canary A
relations:
  - type: supersedes
    target: canary-b
---
A
"#;
        let note_b = r#"---
id: canary-b
type: Decision
title: Canary B
relations:
  - type: supersedes
    target: canary-c
---
B
"#;
        let note_c = r#"---
id: canary-c
type: Decision
title: Canary C
relations:
  - type: supersedes
    target: canary-a
---
C
"#;
        fs::write(root.join("canary-a.md"), note_a).unwrap();
        fs::write(root.join("canary-b.md"), note_b).unwrap();
        fs::write(root.join("canary-c.md"), note_c).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let cycles = find_supersession_cycles(&vault);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["canary-a", "canary-b", "canary-c"]);

        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::SupersessionCycle);
        assert!(report.findings[0]
            .message
            .contains("canary-a -> canary-b -> canary-c -> canary-a"));
    }

    #[test]
    fn test_detect_1_node_self_cycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: loop-node
type: Decision
title: Loop Node
relations:
  - type: supersedes
    target: loop-node
---
Loop
"#;
        fs::write(root.join("loop.md"), note_a).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let cycles = find_supersession_cycles(&vault);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["loop-node"]);

        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::SupersessionCycle);
    }

    #[test]
    fn test_acyclic_supersedes_dag_is_healthy() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note1 = r#"---
id: dec-1
type: Decision
title: Decision 1
---
1
"#;
        let note2 = r#"---
id: dec-2
type: Decision
title: Decision 2
relations:
  - type: supersedes
    target: dec-1
---
2
"#;
        let note3 = r#"---
id: dec-3
type: Decision
title: Decision 3
relations:
  - type: supersedes
    target: dec-2
---
3
"#;
        fs::write(root.join("1.md"), note1).unwrap();
        fs::write(root.join("2.md"), note2).unwrap();
        fs::write(root.join("3.md"), note3).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let cycles = find_supersession_cycles(&vault);
        assert!(cycles.is_empty());

        let report = lint_vault(&vault);
        assert!(report.healthy);
        assert_eq!(report.errors, 0);
    }

    #[test]
    fn test_non_supersedes_cycles_not_reported_as_supersession_cycles() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Bidirectional relates_to or circular depends_on should not be flagged as supersession cycles
        let note_a = r#"---
id: doc-a
type: Concept
title: Doc A
relations:
  - type: relates_to
    target: doc-b
  - type: depends_on
    target: doc-b
---
A
"#;
        let note_b = r#"---
id: doc-b
type: Concept
title: Doc B
relations:
  - type: relates_to
    target: doc-a
  - type: depends_on
    target: doc-a
---
B
"#;
        fs::write(root.join("a.md"), note_a).unwrap();
        fs::write(root.join("b.md"), note_b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let cycles = find_supersession_cycles(&vault);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_detect_broken_target() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note = r#"---
id: broken-node
type: Decision
title: Broken Target Note
relations:
  - type: supersedes
    target: ./missing-file.md
  - type: supported_by
    target: non-existent-id
---
Content
"#;
        fs::write(root.join("broken.md"), note).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 2);
        assert_eq!(report.findings[0].kind, FindingKind::BrokenTarget);
        assert_eq!(report.findings[1].kind, FindingKind::BrokenTarget);
    }

    #[test]
    fn test_detect_active_contradiction_overlapping_scopes() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: dec-event
type: Decision
title: Event Decision
status: stable
scope:
  env: production
  system: payments
relations:
  - type: contradicts
    target: dec-grpc
---
Events
"#;
        let note_b = r#"---
id: dec-grpc
type: Proposal
title: gRPC Decision
status: draft
scope:
  env: production
  system: payments
---
gRPC
"#;
        fs::write(root.join("a.md"), note_a).unwrap();
        fs::write(root.join("b.md"), note_b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::ConflictingAssertion);
        assert!(report.findings[0]
            .message
            .contains("Active contradiction between 'dec-event' and 'dec-grpc'"));
    }

    #[test]
    fn test_contradiction_disjoint_scopes_not_flagged() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note_a = r#"---
id: dec-prod
type: Decision
title: Prod Decision
scope:
  env: production
relations:
  - type: contradicts
    target: dec-staging
---
Prod
"#;
        let note_b = r#"---
id: dec-staging
type: Decision
title: Staging Decision
scope:
  env: staging
---
Staging
"#;
        fs::write(root.join("a.md"), note_a).unwrap();
        fs::write(root.join("b.md"), note_b).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let report = lint_vault(&vault);
        assert!(report.healthy);
        assert_eq!(report.errors, 0);
    }

    #[test]
    fn test_competing_successors_overlapping_scopes() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let pred = r#"---
id: old-arch
type: Decision
title: Old Architecture
---
Old
"#;
        let succ1 = r#"---
id: succ-1
type: Decision
title: Successor 1
relations:
  - type: supersedes
    target: old-arch
---
1
"#;
        let succ2 = r#"---
id: succ-2
type: Decision
title: Successor 2
relations:
  - type: supersedes
    target: old-arch
---
2
"#;
        fs::write(root.join("old.md"), pred).unwrap();
        fs::write(root.join("succ1.md"), succ1).unwrap();
        fs::write(root.join("succ2.md"), succ2).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::ConflictingAssertion);
        assert!(report.findings[0]
            .message
            .contains("Competing successors: 'succ-1' and 'succ-2' both supersede 'old-arch'"));
    }

    #[test]
    fn test_scope_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let pred = r#"---
id: prod-policy
type: Decision
title: Prod Policy
scope:
  env: production
---
Prod
"#;
        let succ = r#"---
id: staging-policy
type: Decision
title: Staging Policy
scope:
  env: staging
relations:
  - type: supersedes
    target: prod-policy
---
Staging
"#;
        fs::write(root.join("prod.md"), pred).unwrap();
        fs::write(root.join("staging.md"), succ).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();
        let report = lint_vault(&vault);
        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings[0].kind, FindingKind::ScopeMismatch);
    }

    #[test]
    fn test_scope_mismatch_narrower_and_covering() {
        // Case 1: Narrower successor scope triggers ScopeMismatch
        {
            let dir = tempdir().unwrap();
            let root = dir.path();

            let pred = r#"---
id: broad-policy
type: Decision
title: Broad Policy
scope:
  env: production
---
Broad
"#;
            let succ = r#"---
id: narrow-policy
type: Decision
title: Narrow Policy
scope:
  env: production
  region: us-east
relations:
  - type: supersedes
    target: broad-policy
---
Narrow
"#;
            fs::write(root.join("broad.md"), pred).unwrap();
            fs::write(root.join("narrow.md"), succ).unwrap();

            let vault = VaultGraph::from_dir(root).unwrap();
            let findings = detect_scope_mismatches(&vault);
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].kind, FindingKind::ScopeMismatch);
            assert_eq!(findings[0].source.as_deref(), Some("narrow-policy"));
            assert_eq!(findings[0].target.as_deref(), Some("broad-policy"));
        }

        // Case 2: Identical successor scope does not trigger ScopeMismatch
        {
            let dir = tempdir().unwrap();
            let root = dir.path();

            let pred = r#"---
id: base-policy
type: Decision
title: Base Policy
scope:
  env: production
  region: us-east
---
Base
"#;
            let succ = r#"---
id: identical-policy
type: Decision
title: Identical Policy
scope:
  env: production
  region: us-east
relations:
  - type: supersedes
    target: base-policy
---
Identical
"#;
            fs::write(root.join("base.md"), pred).unwrap();
            fs::write(root.join("identical.md"), succ).unwrap();

            let vault = VaultGraph::from_dir(root).unwrap();
            let findings = detect_scope_mismatches(&vault);
            assert!(findings.is_empty());
        }

        // Case 3: Covering (broader) successor scope does not trigger ScopeMismatch
        {
            let dir = tempdir().unwrap();
            let root = dir.path();

            let pred = r#"---
id: regional-policy
type: Decision
title: Regional Policy
scope:
  env: production
  region: us-east
---
Regional
"#;
            let succ = r#"---
id: global-policy
type: Decision
title: Global Policy
scope:
  env: production
relations:
  - type: supersedes
    target: regional-policy
---
Global
"#;
            fs::write(root.join("regional.md"), pred).unwrap();
            fs::write(root.join("global.md"), succ).unwrap();

            let vault = VaultGraph::from_dir(root).unwrap();
            let findings = detect_scope_mismatches(&vault);
            assert!(findings.is_empty());
        }
    }
}
