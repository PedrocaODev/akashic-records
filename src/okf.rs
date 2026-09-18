use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::graph::VaultGraph;
use crate::model::{NodeMetadata, Relation, Scope};

/// Summary of an exported node in the export manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportedNodeSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scope: Scope,
    pub relations_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
}

/// Summary of OKF extension profile attributes used across the exported vault.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExtensionAttributesSummary {
    pub profile: String,
    pub total_scoped_nodes: usize,
    pub total_relations: usize,
    pub total_evidence_references: usize,
    pub total_scoped_relations: usize,
}

/// The export manifest summarizing all exported nodes and extensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportManifest {
    pub format: String,
    pub vault_path: String,
    pub export_path: String,
    pub total_nodes: usize,
    pub total_relations: usize,
    pub relations_by_type: BTreeMap<String, usize>,
    pub extension_attributes: ExtensionAttributesSummary,
    pub nodes: Vec<ExportedNodeSummary>,
}

/// Computes the relative POSIX path from a source directory (relative to vault root)
/// to a target file (relative to vault root).
pub fn compute_relative_path(source_dir: &str, target_path: &str) -> String {
    let source_dir_clean = source_dir.trim_matches('/');
    let target_path_clean = target_path.trim_matches('/');

    let source_parts: Vec<&str> = if source_dir_clean.is_empty() {
        Vec::new()
    } else {
        source_dir_clean
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect()
    };

    let target_parts: Vec<&str> = target_path_clean
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();

    let mut common_len = 0;
    while common_len < source_parts.len()
        && common_len < target_parts.len()
        && source_parts[common_len] == target_parts[common_len]
    {
        common_len += 1;
    }

    let mut result_parts = Vec::new();
    let ups = source_parts.len() - common_len;
    for _ in 0..ups {
        result_parts.push("..");
    }

    for part in &target_parts[common_len..] {
        result_parts.push(part);
    }

    if ups == 0 {
        format!("./{}", result_parts.join("/"))
    } else {
        result_parts.join("/")
    }
}

/// Resolves a link destination string into a navigable relative path in the exported directory.
pub fn resolve_link_destination(
    vault: &VaultGraph,
    source_rel_path: &str,
    raw_dest: &str,
) -> Option<String> {
    if raw_dest.starts_with('#')
        || raw_dest.contains("://")
        || raw_dest.starts_with("mailto:")
        || raw_dest.starts_with("data:")
    {
        return None;
    }

    let (target_part, anchor_str) = if let Some((t, a)) = raw_dest.split_once('#') {
        (t, format!("#{}", a))
    } else {
        (raw_dest, String::new())
    };

    if target_part.is_empty() {
        return None;
    }

    let source_dir = Path::new(source_rel_path)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let source_dir = if source_dir == "." { "" } else { &source_dir };

    let target_idx = vault.resolve_target(source_rel_path, target_part)?;
    let target_node = &vault.graph[target_idx];
    let target_posix = target_node.path.as_deref()?;

    let rel_path = compute_relative_path(source_dir, target_posix);
    Some(format!("{}{}", rel_path, anchor_str))
}

/// Converts Markdown body links (both standard markdown links and wikilinks)
/// so they remain navigable and valid in the exported directory.
/// Preserves code blocks and inline code unchanged.
pub fn convert_markdown_body_links(
    vault: &VaultGraph,
    source_rel_path: &str,
    body: &str,
) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(body, options).into_offset_iter();

    let mut code_spans: Vec<std::ops::Range<usize>> = Vec::new();
    let mut standard_link_spans: Vec<(std::ops::Range<usize>, String)> = Vec::new();

    let mut in_code_block: Option<usize> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = Some(range.start);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = in_code_block.take() {
                    code_spans.push(start..range.end);
                }
            }
            Event::Code(_) => {
                code_spans.push(range);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                if in_code_block.is_none() {
                    standard_link_spans.push((range, dest_url.to_string()));
                }
            }
            _ => {}
        }
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    // 1. Process standard markdown links
    for (range, dest_url) in &standard_link_spans {
        if let Some(new_dest) = resolve_link_destination(vault, source_rel_path, dest_url) {
            let link_text = &body[range.clone()];
            if let Some(bracket_close) = link_text.rfind("](") {
                let text_prefix = &link_text[..=bracket_close]; // includes ']'
                let inside_parens =
                    &link_text[bracket_close + 2..link_text.len().saturating_sub(1)];

                // Check if inside_parens had title
                let new_inside = if let Some(title_start) =
                    inside_parens.find(|c: char| c == '"' || c == '\'')
                {
                    format!("{} {}", new_dest, &inside_parens[title_start..])
                } else {
                    new_dest
                };

                let new_link = format!("{}({})", text_prefix, new_inside);
                replacements.push((range.start, range.end, new_link));
            }
        }
    }

    // 2. Process wikilinks: [[target]] or [[target|display]]
    let mut search_idx = 0;
    while let Some(start_pos) = body[search_idx..].find("[[") {
        let abs_start = search_idx + start_pos;
        search_idx = abs_start + 2;

        let is_in_code = code_spans
            .iter()
            .any(|r| r.start <= abs_start && abs_start < r.end);
        let is_in_std_link = standard_link_spans
            .iter()
            .any(|(r, _)| r.start <= abs_start && abs_start < r.end);

        if is_in_code || is_in_std_link {
            continue;
        }

        if let Some(end_offset) = body[abs_start + 2..].find("]]") {
            let abs_end = abs_start + 2 + end_offset + 2;
            let wiki_content = &body[abs_start + 2..abs_start + 2 + end_offset];

            if wiki_content.contains('\n') || wiki_content.contains("[[") {
                continue;
            }

            let (target_full, display_text) = if let Some((t, d)) = wiki_content.split_once('|') {
                (t.trim(), d.trim().to_string())
            } else {
                let t = wiki_content.trim();
                (t, t.to_string())
            };

            let (target_part, anchor_str) = if let Some((t, a)) = target_full.split_once('#') {
                (t.trim(), format!("#{}", a.trim()))
            } else {
                (target_full, String::new())
            };

            let source_dir = Path::new(source_rel_path)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let source_dir = if source_dir == "." { "" } else { &source_dir };

            let new_dest =
                if let Some(target_idx) = vault.resolve_target(source_rel_path, target_part) {
                    let target_node = &vault.graph[target_idx];
                    if let Some(target_posix) = &target_node.path {
                        let rel = compute_relative_path(source_dir, target_posix);
                        format!("{}{}", rel, anchor_str)
                    } else {
                        format!("{}{}", target_part, anchor_str)
                    }
                } else if target_part.starts_with("./")
                    || target_part.starts_with("../")
                    || target_part.ends_with(".md")
                {
                    format!("{}{}", target_part, anchor_str)
                } else {
                    format!("./{}.md{}", target_part, anchor_str)
                };

            let converted = format!("[{}]({})", display_text, new_dest);
            replacements.push((abs_start, abs_end, converted));
            search_idx = abs_end;
        }
    }

    replacements.sort_by_key(|(start, _, _)| *start);

    let mut result = String::with_capacity(body.len());
    let mut last_idx = 0;
    for (start, end, rep) in replacements {
        if start >= last_idx {
            result.push_str(&body[last_idx..start]);
            result.push_str(&rep);
            last_idx = end;
        }
    }
    result.push_str(&body[last_idx..]);

    result
}

/// Exports a vault to Google OKF v0.2 nodes in the given output directory.
/// Emits `manifest.json` in the output directory summarizing exported nodes and extensions.
pub fn export_okf(
    vault_path: &Path,
    out_dir: &Path,
) -> Result<ExportManifest, Box<dyn std::error::Error>> {
    let vault = VaultGraph::from_dir(vault_path)?;

    fs::create_dir_all(out_dir)?;

    let mut exported_nodes = Vec::new();
    let mut relations_by_type = BTreeMap::new();
    let mut total_relations = 0;
    let mut total_evidence_references = 0;
    let mut total_scoped_relations = 0;
    let mut total_scoped_nodes = 0;

    let mut nodes: Vec<_> = vault.graph.node_weights().cloned().collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    for node in &nodes {
        let rel_path = node.path.as_deref().unwrap_or("unnamed.md");
        let dest_file_path = out_dir.join(rel_path);

        if let Some(parent) = dest_file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !node.scope.is_empty() {
            total_scoped_nodes += 1;
        }

        for rel in &node.relations {
            total_relations += 1;
            let type_str = rel.relation_type.as_str().to_string();
            *relations_by_type.entry(type_str).or_insert(0) += 1;

            if rel.evidence.is_some() {
                total_evidence_references += 1;
            }
            if rel.scope.is_some() {
                total_scoped_relations += 1;
            }
        }

        let meta = NodeMetadata {
            id: node.id.clone(),
            node_type: node.node_type.clone(),
            title: node.title.clone(),
            status: node.status.clone(),
            valid_from: node.valid_from.clone(),
            valid_until: node.valid_until.clone(),
            scope: if node.scope.is_empty() {
                None
            } else {
                Some(node.scope.clone())
            },
            relations: node.relations.clone(),
        };

        let converted_body = if let Some(ref body) = node.body {
            convert_markdown_body_links(&vault, rel_path, body)
        } else {
            String::new()
        };

        let yaml_str = serde_yaml::to_string(&meta)?;
        let file_content = format!(
            "---\n{}---\n{}",
            yaml_str.trim_start_matches("---\n"),
            converted_body
        );

        fs::write(&dest_file_path, file_content)?;

        exported_nodes.push(ExportedNodeSummary {
            id: node.id.clone(),
            node_type: node.node_type.clone(),
            title: node.title.clone(),
            path: rel_path.to_string(),
            status: node.status.clone(),
            valid_from: node.valid_from.clone(),
            valid_until: node.valid_until.clone(),
            scope: node.scope.clone(),
            relations_count: node.relations.len(),
            relations: node.relations.clone(),
        });
    }

    let extension_summary = ExtensionAttributesSummary {
        profile: "https://github.com/akashic-records/specs/okf-extension-v1".to_string(),
        total_scoped_nodes,
        total_relations,
        total_evidence_references,
        total_scoped_relations,
    };

    let manifest = ExportManifest {
        format: "okf-v0.2".to_string(),
        vault_path: vault_path.display().to_string(),
        export_path: out_dir.display().to_string(),
        total_nodes: exported_nodes.len(),
        total_relations,
        relations_by_type,
        extension_attributes: extension_summary,
        nodes: exported_nodes,
    };

    let manifest_path = out_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(manifest_path, manifest_json)?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compute_relative_path() {
        assert_eq!(
            compute_relative_path("decisions", "decisions/adr-2.md"),
            "./adr-2.md"
        );
        assert_eq!(compute_relative_path("", "concept.md"), "./concept.md");
        assert_eq!(
            compute_relative_path("", "decisions/adr-1.md"),
            "./decisions/adr-1.md"
        );
        assert_eq!(
            compute_relative_path("decisions", "concept.md"),
            "../concept.md"
        );
        assert_eq!(
            compute_relative_path("decisions", "reviews/rev-1.md"),
            "../reviews/rev-1.md"
        );
        assert_eq!(compute_relative_path("a/b/c", "a/d/e.md"), "../../d/e.md");
        assert_eq!(compute_relative_path("a/b", "a/b/sub/c.md"), "./sub/c.md");
    }

    #[test]
    fn test_convert_markdown_body_links_and_code_preservation() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let decisions_dir = root.join("decisions");
        let reviews_dir = root.join("reviews");
        fs::create_dir_all(&decisions_dir).unwrap();
        fs::create_dir_all(&reviews_dir).unwrap();

        let rev_content = r#"---
id: rev-001
type: Review
title: Benchmark Review
---
# Review Details
"#;
        fs::write(reviews_dir.join("rev-1.md"), rev_content).unwrap();

        let adr_content = r#"---
id: adr-001
type: Decision
title: Use Rust
---
# Body
"#;
        fs::write(decisions_dir.join("adr-1.md"), adr_content).unwrap();

        let vault = VaultGraph::from_dir(root).unwrap();

        let body = r#"Check this [Benchmark](rev-001) and [Benchmark Path](../reviews/rev-1.md#sec).
Also see [[rev-001|Display Rev]] and plain [[rev-001]].
External [Google](https://google.com) and internal [Anchor](#sec).

`inline [code](rev-001) and [[rev-001]]`

```rust
// code block
let x = "[not a link](rev-001)";
let y = "[[rev-001]]";
```
"#;

        let converted = convert_markdown_body_links(&vault, "decisions/adr-1.md", body);

        // Standard link by ID converted to relative path
        assert!(converted.contains("[Benchmark](../reviews/rev-1.md)"));
        // Standard link with anchor preserved
        assert!(converted.contains("[Benchmark Path](../reviews/rev-1.md#sec)"));
        // Wikilink with display text converted
        assert!(converted.contains("[Display Rev](../reviews/rev-1.md)"));
        // Plain wikilink converted
        assert!(converted.contains("[rev-001](../reviews/rev-1.md)"));
        // External link untouched
        assert!(converted.contains("[Google](https://google.com)"));
        // Internal anchor untouched
        assert!(converted.contains("[Anchor](#sec)"));
        // Inline code untouched
        assert!(converted.contains("`inline [code](rev-001) and [[rev-001]]`"));
        // Fenced code untouched
        assert!(converted.contains("let x = \"[not a link](rev-001)\";"));
        assert!(converted.contains("let y = \"[[rev-001]]\";"));
    }

    #[test]
    fn test_export_okf_end_to_end() {
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

        let manifest = export_okf(root, export_dir.path()).unwrap();

        // 1. Check manifest fields
        assert_eq!(manifest.format, "okf-v0.2");
        assert_eq!(manifest.total_nodes, 3);
        assert_eq!(manifest.total_relations, 2);
        assert_eq!(manifest.relations_by_type.get("supersedes"), Some(&1));
        assert_eq!(manifest.relations_by_type.get("supported_by"), Some(&1));
        assert_eq!(manifest.extension_attributes.total_scoped_nodes, 1);
        assert_eq!(manifest.extension_attributes.total_evidence_references, 1);

        // 2. Check manifest.json on disk
        let manifest_file = export_dir.path().join("manifest.json");
        assert!(manifest_file.exists());
        let manifest_disk: ExportManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_file).unwrap()).unwrap();
        assert_eq!(manifest_disk.total_nodes, 3);

        // 3. Check exported files exist on disk
        let exp_events = export_dir.path().join("decisions/decision-events.md");
        let exp_polling = export_dir.path().join("decisions/decision-polling.md");
        let exp_review = export_dir.path().join("reviews/rev-timeout.md");
        assert!(exp_events.exists());
        assert!(exp_polling.exists());
        assert!(exp_review.exists());

        // 4. Verify frontmatter and lifecycle mapping in exported files
        let events_text = fs::read_to_string(&exp_events).unwrap();
        assert!(events_text.contains("id: decision-events"));
        assert!(events_text.contains("type: Decision"));
        assert!(events_text.contains("title: Use Event-Driven"));
        assert!(events_text.contains("status: stable"));
        assert!(
            events_text.contains("valid_from: 2026-06-01")
                || events_text.contains("valid_from: '2026-06-01'")
                || events_text.contains("valid_from: \"2026-06-01\"")
        );
        assert!(
            events_text.contains("valid_until: 2027-01-01")
                || events_text.contains("valid_until: '2027-01-01'")
                || events_text.contains("valid_until: \"2027-01-01\"")
        );
        assert!(events_text.contains("env: production"));
        assert!(events_text.contains("system: payments"));

        // 5. Verify relations extension preserved without data loss
        assert!(events_text.contains("type: supersedes"));
        assert!(events_text.contains("target: ./decision-polling.md"));
        assert!(events_text.contains("evidence: rev-timeout"));
        assert!(events_text.contains("type: supported_by"));

        // 6. Verify converted Markdown body links
        assert!(events_text.contains("[Timeout Review](../reviews/rev-timeout.md)"));
        assert!(events_text.contains("[Polling](./decision-polling.md)"));

        // 7. Verify exported directory is a valid vault that can be indexed cleanly
        let exported_vault = VaultGraph::from_dir(export_dir.path()).unwrap();
        assert_eq!(exported_vault.graph.node_count(), 3);
        assert_eq!(exported_vault.graph.edge_count(), 2);
        assert!(exported_vault.get_node("decision-events").is_some());
        assert!(exported_vault.get_node("decision-polling").is_some());
        assert!(exported_vault.get_node("rev-timeout").is_some());
    }
}
