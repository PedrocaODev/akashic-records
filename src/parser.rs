use pulldown_cmark::{Options, Parser};
use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::model::{Node, NodeMetadata};

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("I/O error reading note: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "Missing or malformed YAML frontmatter delimiter (expected opening and closing '---')"
    )]
    MissingFrontmatter,

    #[error("Invalid YAML frontmatter: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),

    #[error("Markdown AST error: {0}")]
    Markdown(String),

    #[error("Inverse relation type '{0}' cannot be declared in frontmatter (inverse relations are dynamically derived)")]
    InverseRelationDeclared(String),
}

/// Parses the string content of a Markdown note, extracting YAML frontmatter and parsing the body AST.
pub fn parse_note_content(content: &str, path: Option<&str>) -> Result<Node, ParseError> {
    let trimmed = content.trim_start_matches('\u{feff}'); // Handle UTF-8 BOM if present

    if !trimmed.starts_with("---") {
        return Err(ParseError::MissingFrontmatter);
    }

    // Find the end of the opening delimiter line
    let rest = &trimmed[3..];
    let after_first_line = match rest.find('\n') {
        Some(pos) => &rest[pos + 1..],
        None => return Err(ParseError::MissingFrontmatter),
    };

    // Find closing delimiter line ("---" or "...")
    let mut closing_pos = None;
    let mut current_offset = 0;

    for line in after_first_line.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]).trim();
        if trimmed_line == "---" || trimmed_line == "..." {
            closing_pos = Some(current_offset);
            break;
        }
        current_offset += line.len();
    }

    let closing_offset = match closing_pos {
        Some(pos) => pos,
        None => return Err(ParseError::MissingFrontmatter),
    };

    let yaml_str = &after_first_line[..closing_offset];
    let body_start = match after_first_line[closing_offset..].find('\n') {
        Some(newline_pos) => closing_offset + newline_pos + 1,
        None => after_first_line.len(),
    };
    let body_str = &after_first_line[body_start..];

    // Deserialize YAML frontmatter into NodeMetadata
    let metadata: NodeMetadata = serde_yaml::from_str(yaml_str)?;

    // Validate that frontmatter does not declare inverse relations (ADR-0002)
    for rel in &metadata.relations {
        if rel.relation_type.is_inverse() {
            return Err(ParseError::InverseRelationDeclared(
                rel.relation_type.as_str().to_string(),
            ));
        }
    }

    // Stream through Markdown body events using pulldown-cmark
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(body_str, options);
    // Exhaust the streaming iterator to ensure valid AST event processing
    for _event in parser {
        // Event inspection could extract headers or inline relative links in future slices
    }

    Ok(Node::from_metadata(
        metadata,
        path.map(|s| s.to_string()),
        Some(body_str.to_string()),
    ))
}

/// Reads a Markdown note from the filesystem and parses its frontmatter and body.
pub fn parse_note_file<P: AsRef<Path>>(path: P) -> Result<Node, ParseError> {
    let p = path.as_ref();
    let content = fs::read_to_string(p)?;
    parse_note_content(&content, Some(p.to_string_lossy().as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationType;

    #[test]
    fn test_parse_valid_frontmatter_and_markdown() {
        let doc = r#"---
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
# Heading

Markdown content with **bold** and [link](./target.md).
"#;

        let node = parse_note_content(doc, Some("decision-payment-status-events.md")).unwrap();
        assert_eq!(node.id, "decision-payment-status-events");
        assert_eq!(node.node_type, "Decision");
        assert_eq!(node.title, "Use event-driven payment status");
        assert_eq!(node.status.as_deref(), Some("stable"));
        assert_eq!(node.valid_from.as_deref(), Some("2026-06-01"));
        assert_eq!(
            node.scope.get("system").map(|s| s.as_str()),
            Some("payments")
        );
        assert_eq!(
            node.scope.get("env").map(|s| s.as_str()),
            Some("production")
        );
        assert_eq!(node.relations.len(), 2);
        assert_eq!(node.relations[0].relation_type, RelationType::Supersedes);
        assert_eq!(
            node.relations[0].target,
            "./decision-payment-status-polling.md"
        );
        assert_eq!(
            node.relations[0].evidence.as_deref(),
            Some("./review-incident-timeout.md")
        );
        assert_eq!(node.relations[1].relation_type, RelationType::SupportedBy);
        assert!(node.body.unwrap().contains("# Heading"));
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let doc = "# Plain Markdown\nWithout frontmatter.";
        let err = parse_note_content(doc, None).unwrap_err();
        assert!(matches!(err, ParseError::MissingFrontmatter));
    }

    #[test]
    fn test_parse_inverse_relations_rejected() {
        let inverse_types = ["superseded_by", "required_by", "supports", "caused"];

        for inv in inverse_types {
            let doc = format!(
                r#"---
id: test-node
type: Decision
title: Test
relations:
  - type: {}
    target: ./target.md
---
Body
"#,
                inv
            );

            let err = parse_note_content(&doc, None).unwrap_err();
            match err {
                ParseError::InverseRelationDeclared(ref s) => assert_eq!(s, inv),
                other => panic!("Expected InverseRelationDeclared, got {:?}", other),
            }
        }
    }
}
