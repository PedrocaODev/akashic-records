use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Scope bounds where a node or relation applies (e.g. env=production, system=payments).
pub type Scope = BTreeMap<String, String>;

/// Canonical typed relation kinds supported by Akashic Records.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationType {
    Supersedes,
    SupersededBy,
    DependsOn,
    RequiredBy,
    SupportedBy,
    Supports,
    Contradicts,
    CausedBy,
    Caused,
    RelatesTo,
    Custom(String),
}

impl RelationType {
    /// Returns the canonical or custom string slice representing this relation type.
    pub fn as_str(&self) -> &str {
        match self {
            RelationType::Supersedes => "supersedes",
            RelationType::SupersededBy => "superseded_by",
            RelationType::DependsOn => "depends_on",
            RelationType::RequiredBy => "required_by",
            RelationType::SupportedBy => "supported_by",
            RelationType::Supports => "supports",
            RelationType::Contradicts => "contradicts",
            RelationType::CausedBy => "caused_by",
            RelationType::Caused => "caused",
            RelationType::RelatesTo => "relates_to",
            RelationType::Custom(s) => s.as_str(),
        }
    }

    /// Returns true if this relation type is an inverse kind dynamically projected by the engine.
    pub fn is_inverse(&self) -> bool {
        matches!(
            self,
            RelationType::SupersededBy
                | RelationType::RequiredBy
                | RelationType::Supports
                | RelationType::Caused
        )
    }

    /// Returns the inverse relation type, if one is defined.
    pub fn inverse(&self) -> Option<RelationType> {
        match self {
            RelationType::Supersedes => Some(RelationType::SupersededBy),
            RelationType::SupersededBy => Some(RelationType::Supersedes),
            RelationType::DependsOn => Some(RelationType::RequiredBy),
            RelationType::RequiredBy => Some(RelationType::DependsOn),
            RelationType::SupportedBy => Some(RelationType::Supports),
            RelationType::Supports => Some(RelationType::SupportedBy),
            RelationType::CausedBy => Some(RelationType::Caused),
            RelationType::Caused => Some(RelationType::CausedBy),
            RelationType::Contradicts => Some(RelationType::Contradicts),
            RelationType::RelatesTo => Some(RelationType::RelatesTo),
            RelationType::Custom(_) => None,
        }
    }
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for RelationType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
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
        })
    }
}

/// A malformed file encountered during vault traversal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MalformedFile {
    pub path: String,
    pub error: String,
}

/// Vault-wide inventory metrics reporting summary data about the graph index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VaultInventory {
    pub vault_path: String,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub edges_by_type: BTreeMap<String, usize>,
    pub malformed_files: Vec<MalformedFile>,
}

/// A typed, directed edge connecting a source node to a target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relation {
    #[serde(rename = "type")]
    pub relation_type: RelationType,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
}

/// Raw metadata extracted from YAML frontmatter compliant with Google OKF v0.2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeMetadata {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
}

/// Complete in-memory domain representation of a Markdown document node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl Node {
    pub fn from_metadata(meta: NodeMetadata, path: Option<String>, body: Option<String>) -> Self {
        Self {
            id: meta.id,
            node_type: meta.node_type,
            title: meta.title,
            status: meta.status,
            valid_from: meta.valid_from,
            valid_until: meta.valid_until,
            scope: meta.scope.unwrap_or_default(),
            relations: meta.relations,
            path,
            body,
        }
    }
}

/// Checks whether two scopes overlap (i.e. do not contradict on any shared key).
/// An empty scope represents global scope and overlaps with everything.
pub fn scopes_overlap(s1: &Scope, s2: &Scope) -> bool {
    for (k, v1) in s1 {
        if let Some(v2) = s2.get(k) {
            if v1 != v2 {
                return false;
            }
        }
    }
    true
}

/// Checks whether scope `sup` covers scope `sub` (i.e. all constraints in `sup` are satisfied by `sub`).
/// For example, an empty scope `{}` covers any scope. `{env: prod}` covers `{env: prod, system: payments}`.
pub fn scope_covers(sup: &Scope, sub: &Scope) -> bool {
    sup.iter().all(|(k, v)| sub.get(k) == Some(v))
}

/// Severity level of a graph integrity finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Error,
    Warning,
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingSeverity::Error => write!(f, "error"),
            FindingSeverity::Warning => write!(f, "warning"),
        }
    }
}

/// Category of graph topology finding or violation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    BrokenTarget,
    SupersessionCycle,
    ConflictingAssertion,
    ScopeMismatch,
    MalformedFile,
}

impl std::fmt::Display for FindingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingKind::BrokenTarget => write!(f, "broken_target"),
            FindingKind::SupersessionCycle => write!(f, "supersession_cycle"),
            FindingKind::ConflictingAssertion => write!(f, "conflicting_assertion"),
            FindingKind::ScopeMismatch => write!(f, "scope_mismatch"),
            FindingKind::MalformedFile => write!(f, "malformed_file"),
        }
    }
}

/// A structured graph finding representing an integrity violation or advisory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: FindingSeverity,
    pub kind: FindingKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<Vec<String>>,
}

/// Structured lint report summarizing all graph integrity findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintReport {
    pub vault_path: String,
    pub healthy: bool,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub errors: usize,
    pub warnings: usize,
    pub findings: Vec<Finding>,
}

impl LintReport {
    /// Creates a new empty `LintReport` marked as healthy.
    pub fn new(vault_path: impl Into<String>, total_nodes: usize, total_edges: usize) -> Self {
        Self {
            vault_path: vault_path.into(),
            healthy: true,
            total_nodes,
            total_edges,
            errors: 0,
            warnings: 0,
            findings: Vec::new(),
        }
    }

    /// Appends a finding to the report, updating error/warning counts and healthy status.
    pub fn add_finding(&mut self, finding: Finding) {
        match finding.severity {
            FindingSeverity::Error => {
                self.errors += 1;
                self.healthy = false;
            }
            FindingSeverity::Warning => {
                self.warnings += 1;
            }
        }
        self.findings.push(finding);
    }
}

/// A proposed file edit in a migration plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanAction {
    pub path: String,
    pub source_sha256: String,
    pub expected_new_sha256: String,
    #[serde(alias = "patch", alias = "unified_diff")]
    pub diff: String,
}

/// A machine-readable, reviewable migration plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plan {
    pub plan_version: String,
    pub vault_root: String,
    pub actions: Vec<PlanAction>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_type_as_str_and_is_inverse() {
        assert_eq!(RelationType::Supersedes.as_str(), "supersedes");
        assert_eq!(RelationType::SupersededBy.as_str(), "superseded_by");
        assert_eq!(
            RelationType::Custom("custom_kind".to_string()).as_str(),
            "custom_kind"
        );

        assert!(!RelationType::Supersedes.is_inverse());
        assert!(RelationType::SupersededBy.is_inverse());
        assert!(!RelationType::DependsOn.is_inverse());
        assert!(RelationType::RequiredBy.is_inverse());
        assert!(!RelationType::SupportedBy.is_inverse());
        assert!(RelationType::Supports.is_inverse());
        assert!(!RelationType::CausedBy.is_inverse());
        assert!(RelationType::Caused.is_inverse());
        assert!(!RelationType::Contradicts.is_inverse());
        assert!(!RelationType::RelatesTo.is_inverse());
        assert!(!RelationType::Custom("custom".to_string()).is_inverse());
    }

    #[test]
    fn test_relation_type_inverse() {
        assert_eq!(
            RelationType::Supersedes.inverse(),
            Some(RelationType::SupersededBy)
        );
        assert_eq!(
            RelationType::SupersededBy.inverse(),
            Some(RelationType::Supersedes)
        );
        assert_eq!(
            RelationType::DependsOn.inverse(),
            Some(RelationType::RequiredBy)
        );
        assert_eq!(
            RelationType::RequiredBy.inverse(),
            Some(RelationType::DependsOn)
        );
        assert_eq!(
            RelationType::SupportedBy.inverse(),
            Some(RelationType::Supports)
        );
        assert_eq!(
            RelationType::Supports.inverse(),
            Some(RelationType::SupportedBy)
        );
        assert_eq!(RelationType::CausedBy.inverse(), Some(RelationType::Caused));
        assert_eq!(RelationType::Caused.inverse(), Some(RelationType::CausedBy));
        assert_eq!(
            RelationType::Contradicts.inverse(),
            Some(RelationType::Contradicts)
        );
        assert_eq!(
            RelationType::RelatesTo.inverse(),
            Some(RelationType::RelatesTo)
        );
        assert_eq!(
            RelationType::Custom("custom_rel".to_string()).inverse(),
            None
        );
    }

    #[test]
    fn test_relation_type_serde() {
        let types = vec![
            (RelationType::Supersedes, "\"supersedes\""),
            (RelationType::SupersededBy, "\"superseded_by\""),
            (RelationType::DependsOn, "\"depends_on\""),
            (RelationType::RequiredBy, "\"required_by\""),
            (RelationType::SupportedBy, "\"supported_by\""),
            (RelationType::Supports, "\"supports\""),
            (RelationType::Contradicts, "\"contradicts\""),
            (RelationType::CausedBy, "\"caused_by\""),
            (RelationType::Caused, "\"caused\""),
            (RelationType::RelatesTo, "\"relates_to\""),
        ];

        for (rt, json_str) in types {
            assert_eq!(serde_json::to_string(&rt).unwrap(), json_str);
            let deserialized: RelationType = serde_json::from_str(json_str).unwrap();
            assert_eq!(deserialized, rt);
        }
    }

    #[test]
    fn test_scope_helpers() {
        let mut s1 = Scope::new();
        s1.insert("env".to_string(), "production".to_string());

        let mut s2 = Scope::new();
        s2.insert("env".to_string(), "staging".to_string());

        let mut s3 = Scope::new();
        s3.insert("env".to_string(), "production".to_string());
        s3.insert("system".to_string(), "payments".to_string());

        let empty = Scope::new();

        assert!(!scopes_overlap(&s1, &s2));
        assert!(scopes_overlap(&s1, &s3));
        assert!(scopes_overlap(&empty, &s1));
        assert!(scopes_overlap(&s1, &empty));

        assert!(scope_covers(&empty, &s1));
        assert!(scope_covers(&s1, &s3));
        assert!(!scope_covers(&s3, &s1));
        assert!(!scope_covers(&s1, &s2));
    }

    #[test]
    fn test_finding_and_lint_report() {
        let mut report = LintReport::new("/tmp/vault", 5, 4);
        assert!(report.healthy);
        assert_eq!(report.errors, 0);

        report.add_finding(Finding {
            severity: FindingSeverity::Error,
            kind: FindingKind::BrokenTarget,
            message: "Target not found".to_string(),
            source: Some("node-1".to_string()),
            target: Some("missing.md".to_string()),
            cycle: None,
        });

        assert!(!report.healthy);
        assert_eq!(report.errors, 1);
        assert_eq!(report.findings.len(), 1);

        let json_str = serde_json::to_string(&report).unwrap();
        assert!(json_str.contains("\"healthy\":false"));
        assert!(json_str.contains("\"kind\":\"broken_target\""));
    }

    #[test]
    fn test_plan_serde() {
        let plan = Plan {
            plan_version: "0.1.0".to_string(),
            vault_root: "/path/to/vault".to_string(),
            actions: vec![PlanAction {
                path: "note.md".to_string(),
                source_sha256: "abc".to_string(),
                expected_new_sha256: "def".to_string(),
                diff: "--- a/note.md\n+++ b/note.md\n".to_string(),
            }],
        };

        let json = serde_json::to_string(&plan).unwrap();
        let parsed: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, parsed);

        // Backward compatibility: deserialize JSON with "patch" instead of "diff"
        let legacy_json = r#"{
            "plan_version": "0.1.0",
            "vault_root": "/path/to/vault",
            "actions": [{
                "path": "note.md",
                "source_sha256": "abc",
                "expected_new_sha256": "def",
                "patch": "--- a/note.md\n+++ b/note.md\n"
            }]
        }"#;
        let legacy_parsed: Plan = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(plan, legacy_parsed);
    }
}
