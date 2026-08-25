use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kir::{KirDocument, KirElement, KirError};
use crate::semantic_services::mutation::WorkspaceRevision;

pub const SEMANTIC_ANCHOR_SCHEMA: &str = "mercurio.semantic_anchor.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationshipId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConceptId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackageId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StdlibVersion(String);

/// Canonical source location lives in the KIR module; re-exported here so the
/// existing `identity::SourceSpanRef` path keeps resolving.
pub use crate::kir::SourceSpanRef;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticAnchor {
    #[serde(default = "semantic_anchor_schema")]
    pub schema: String,
    pub element_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_revision: Option<WorkspaceRevision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAnchorResolutionStatus {
    Resolved,
    StaleRevision,
    MissingElement,
    UnsupportedSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticAnchorResolution {
    pub status: SemanticAnchorResolutionStatus,
    pub element_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<WorkspaceRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_revision: Option<WorkspaceRevision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
}

macro_rules! semantic_id {
    ($type_name:ident) => {
        impl $type_name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $type_name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $type_name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

semantic_id!(ElementId);
semantic_id!(RelationshipId);
semantic_id!(ConceptId);
semantic_id!(PackageId);
semantic_id!(ProfileId);
semantic_id!(StdlibVersion);

pub fn semantic_anchor_for_element(
    document: &KirDocument,
    element_id: &str,
) -> Result<Option<SemanticAnchor>, KirError> {
    let Some(element) = document
        .elements
        .iter()
        .find(|element| element.id == element_id)
    else {
        return Ok(None);
    };
    let workspace_revision = workspace_revision_for_kir_document(document)?;
    let qualified_name = qualified_name_for_element(element);
    Ok(Some(SemanticAnchor {
        schema: SEMANTIC_ANCHOR_SCHEMA.to_string(),
        element_id: element.id.clone(),
        semantic_path: semantic_path(qualified_name.as_deref()),
        qualified_name,
        workspace_revision: Some(workspace_revision),
        source_spans: source_span_for_element(element).into_iter().collect(),
    }))
}

pub fn resolve_semantic_anchor(
    document: &KirDocument,
    anchor: &SemanticAnchor,
) -> Result<SemanticAnchorResolution, KirError> {
    let actual_revision = workspace_revision_for_kir_document(document)?;
    if anchor.schema != SEMANTIC_ANCHOR_SCHEMA {
        return Ok(SemanticAnchorResolution {
            status: SemanticAnchorResolutionStatus::UnsupportedSchema,
            element_id: anchor.element_id.clone(),
            qualified_name: anchor.qualified_name.clone(),
            semantic_path: anchor.semantic_path.clone(),
            expected_revision: anchor.workspace_revision.clone(),
            actual_revision: Some(actual_revision),
            source_spans: anchor.source_spans.clone(),
        });
    }

    let Some(element) = document
        .elements
        .iter()
        .find(|element| element.id == anchor.element_id)
    else {
        return Ok(SemanticAnchorResolution {
            status: SemanticAnchorResolutionStatus::MissingElement,
            element_id: anchor.element_id.clone(),
            qualified_name: anchor.qualified_name.clone(),
            semantic_path: anchor.semantic_path.clone(),
            expected_revision: anchor.workspace_revision.clone(),
            actual_revision: Some(actual_revision),
            source_spans: anchor.source_spans.clone(),
        });
    };

    let qualified_name = qualified_name_for_element(element);
    let status = if anchor
        .workspace_revision
        .as_ref()
        .is_some_and(|expected| expected != &actual_revision)
    {
        SemanticAnchorResolutionStatus::StaleRevision
    } else {
        SemanticAnchorResolutionStatus::Resolved
    };

    Ok(SemanticAnchorResolution {
        status,
        element_id: element.id.clone(),
        semantic_path: semantic_path(qualified_name.as_deref()),
        qualified_name,
        expected_revision: anchor.workspace_revision.clone(),
        actual_revision: Some(actual_revision),
        source_spans: source_span_for_element(element).into_iter().collect(),
    })
}

pub fn workspace_revision_for_kir_document(
    document: &KirDocument,
) -> Result<WorkspaceRevision, KirError> {
    let bytes = serde_json::to_vec(document)?;
    Ok(WorkspaceRevision {
        fingerprint: stable_digest([("kir-document".as_bytes(), bytes.as_slice())]),
    })
}

pub fn stable_digest<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = (&'a [u8], &'a [u8])>,
{
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for (label, bytes) in chunks {
        for byte in label
            .iter()
            .chain(&(bytes.len() as u64).to_le_bytes())
            .chain(bytes)
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    format!("fnv1a64:{hash:016x}")
}

fn semantic_anchor_schema() -> String {
    SEMANTIC_ANCHOR_SCHEMA.to_string()
}

fn qualified_name_for_element(element: &KirElement) -> Option<String> {
    element
        .properties
        .get("qualified_name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn semantic_path(qualified_name: Option<&str>) -> Vec<String> {
    qualified_name
        .into_iter()
        .flat_map(|value| value.split('.'))
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn source_span_for_element(element: &KirElement) -> Option<SourceSpanRef> {
    let direct = element.properties.get("source_span");
    let metadata = element.properties.get("metadata");
    let span = direct.or_else(|| metadata.and_then(|metadata| metadata.get("source_span")))?;
    let file = metadata
        .and_then(|metadata| metadata.get("source_file"))
        .and_then(Value::as_str)
        .or_else(|| span.get("file").and_then(Value::as_str))
        .unwrap_or("");
    Some(SourceSpanRef {
        file: file.to_string(),
        start_line: span
            .get("start_line")
            .or_else(|| span.get("startLine"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        start_col: span
            .get("start_col")
            .or_else(|| span.get("startCol"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        end_line: span
            .get("end_line")
            .or_else(|| span.get("endLine"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        end_col: span
            .get("end_col")
            .or_else(|| span.get("endCol"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::kir::{KirDocument, KirElement};

    use super::*;

    #[test]
    fn kir_workspace_revision_is_stable() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo".to_string(),
                kind: "Package".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        };

        let first = workspace_revision_for_kir_document(&document).unwrap();
        let second = workspace_revision_for_kir_document(&document).unwrap();

        assert_eq!(first, second);
        assert!(first.fingerprint.starts_with("fnv1a64:"));
    }

    #[test]
    fn semantic_anchor_captures_revision_path_and_source_span() {
        let document = anchor_document("Demo.Vehicle");

        let anchor = semantic_anchor_for_element(&document, "type.Demo.Vehicle")
            .unwrap()
            .unwrap();

        assert_eq!(anchor.schema, SEMANTIC_ANCHOR_SCHEMA);
        assert_eq!(anchor.qualified_name.as_deref(), Some("Demo.Vehicle"));
        assert_eq!(anchor.semantic_path, vec!["Demo", "Vehicle"]);
        assert!(anchor.workspace_revision.is_some());
        assert_eq!(anchor.source_spans[0].file, "demo.sysml");
        assert_eq!(anchor.source_spans[0].start_line, 4);
    }

    #[test]
    fn semantic_anchor_resolution_reports_stale_revision() {
        let original = anchor_document("Demo.Vehicle");
        let anchor = semantic_anchor_for_element(&original, "type.Demo.Vehicle")
            .unwrap()
            .unwrap();
        let updated = anchor_document("Demo.RenamedVehicle");

        let resolution = resolve_semantic_anchor(&updated, &anchor).unwrap();

        assert_eq!(
            resolution.status,
            SemanticAnchorResolutionStatus::StaleRevision
        );
        assert_eq!(
            resolution.qualified_name.as_deref(),
            Some("Demo.RenamedVehicle")
        );
    }

    fn anchor_document(qualified_name: &str) -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([
                    ("qualified_name".to_string(), json!(qualified_name)),
                    (
                        "metadata".to_string(),
                        json!({
                            "source_file": "demo.sysml",
                            "source_span": {
                                "start_line": 4,
                                "start_col": 1,
                                "end_line": 4,
                                "end_col": 20
                            }
                        }),
                    ),
                ]),
            }],
        }
    }
}
