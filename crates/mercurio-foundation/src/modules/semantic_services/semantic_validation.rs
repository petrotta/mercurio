use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::kir::{Diagnostic, KirDocument, KirError};
use crate::model::Graph;
use crate::model::{MetamodelFeatureRegistry, validate_derived_metamodel_semantics};

pub const SEMANTIC_VALIDATION_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticValidationMode {
    Off,
    Warn,
    Error,
}

impl SemanticValidationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

impl Default for SemanticValidationMode {
    fn default() -> Self {
        Self::Error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticValidationPolicy {
    #[serde(default = "default_semantic_validation_policy_version")]
    pub version: u32,
    #[serde(default)]
    pub mode: SemanticValidationMode,
}

impl SemanticValidationPolicy {
    pub fn cache_key(self) -> String {
        format!(
            "semantic-validation:v{}:{}",
            self.version,
            self.mode.as_str()
        )
    }

    fn diagnostic_severity(self) -> Option<SemanticValidationSeverity> {
        match self.mode {
            SemanticValidationMode::Off => None,
            SemanticValidationMode::Warn => Some(SemanticValidationSeverity::Warning),
            SemanticValidationMode::Error => Some(SemanticValidationSeverity::Error),
        }
    }
}

impl Default for SemanticValidationPolicy {
    fn default() -> Self {
        Self {
            version: SEMANTIC_VALIDATION_POLICY_VERSION,
            mode: SemanticValidationMode::Error,
        }
    }
}

fn default_semantic_validation_policy_version() -> u32 {
    SEMANTIC_VALIDATION_POLICY_VERSION
}

/// Severity used by [`SemanticValidationReport`] diagnostics. Alias of the canonical
/// [`crate::kir::Severity`].
pub use crate::kir::Severity as SemanticValidationSeverity;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticValidationReport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl SemanticValidationReport {
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == SemanticValidationSeverity::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == SemanticValidationSeverity::Error)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

pub fn validate_kir_semantics(
    document: &KirDocument,
) -> Result<SemanticValidationReport, KirError> {
    validate_kir_semantics_with_context(document, None)
}

pub fn validate_kir_semantics_with_policy(
    document: &KirDocument,
    policy: SemanticValidationPolicy,
) -> Result<SemanticValidationReport, KirError> {
    validate_kir_semantics_with_context_and_policy(document, None, policy)
}

pub fn validate_kir_semantics_with_context(
    document: &KirDocument,
    library_context: Option<&KirDocument>,
) -> Result<SemanticValidationReport, KirError> {
    validate_kir_semantics_with_context_and_policy(
        document,
        library_context,
        SemanticValidationPolicy::default(),
    )
}

pub fn validate_kir_semantics_with_context_and_policy(
    document: &KirDocument,
    library_context: Option<&KirDocument>,
    policy: SemanticValidationPolicy,
) -> Result<SemanticValidationReport, KirError> {
    let target_element_ids = document
        .elements
        .iter()
        .map(|element| element.id.clone())
        .collect::<BTreeSet<_>>();
    let validation_document = match library_context {
        Some(library_context) => KirDocument::merge([library_context.clone(), document.clone()])?,
        None => document.clone(),
    };
    let graph = Graph::from_document(validation_document).map_err(|err| {
        KirError::Model(format!(
            "failed to build graph for semantic validation: {err}"
        ))
    })?;

    Ok(validate_kir_semantics_for_graph_targets(
        &graph,
        Some(&target_element_ids),
        policy,
    ))
}

pub fn validate_kir_semantics_for_graph(graph: &Graph) -> SemanticValidationReport {
    validate_kir_semantics_for_graph_with_policy(graph, SemanticValidationPolicy::default())
}

pub fn validate_kir_semantics_for_graph_with_policy(
    graph: &Graph,
    policy: SemanticValidationPolicy,
) -> SemanticValidationReport {
    validate_kir_semantics_for_graph_targets(graph, None, policy)
}

fn validate_kir_semantics_for_graph_targets(
    graph: &Graph,
    target_element_ids: Option<&BTreeSet<String>>,
    policy: SemanticValidationPolicy,
) -> SemanticValidationReport {
    let Some(severity) = policy.diagnostic_severity() else {
        return SemanticValidationReport::default();
    };

    let registry = MetamodelFeatureRegistry::build(graph);
    let diagnostics = validate_derived_metamodel_semantics(graph, &registry)
        .into_iter()
        .filter_map(|mut diagnostic| {
            target_element_ids
                .map(|ids| {
                    diagnostic
                        .subjects
                        .iter()
                        .any(|subject| ids.contains(subject))
                })
                .unwrap_or(true)
                .then(|| {
                    diagnostic.severity = severity;
                    diagnostic
                })
        })
        .collect();

    SemanticValidationReport { diagnostics }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::kir::{KIR_SCHEMA_VERSION, KirElement};

    #[test]
    fn metamodel_validation_reports_errors_against_document_elements() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: vec![KirElement {
                id: "transition.Demo.start".to_string(),
                kind: "SysML::Systems::TransitionUsage".to_string(),
                layer: 2,
                properties: BTreeMap::from([("source".to_string(), json!("state.Demo.Initial"))]),
            }],
        };
        let library_context = validation_library_context();

        let report =
            validate_kir_semantics_with_context(&document, Some(&library_context)).unwrap();

        assert_eq!(report.error_count(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "kir.metamodel.endpoints.incomplete"
        );
        assert_eq!(
            report.diagnostics[0].subjects,
            vec!["transition.Demo.start".to_string()]
        );
    }

    #[test]
    fn warn_policy_reports_semantic_diagnostics_as_warnings() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: vec![KirElement {
                id: "transition.Demo.start".to_string(),
                kind: "SysML::Systems::TransitionUsage".to_string(),
                layer: 2,
                properties: BTreeMap::from([("source".to_string(), json!("state.Demo.Initial"))]),
            }],
        };
        let library_context = validation_library_context();

        let report = validate_kir_semantics_with_context_and_policy(
            &document,
            Some(&library_context),
            SemanticValidationPolicy {
                mode: SemanticValidationMode::Warn,
                ..SemanticValidationPolicy::default()
            },
        )
        .unwrap();

        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.error_count(), 0);
        assert!(!report.has_errors());
    }

    #[test]
    fn off_policy_returns_empty_report() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: vec![KirElement {
                id: "transition.Demo.start".to_string(),
                kind: "SysML::Systems::TransitionUsage".to_string(),
                layer: 2,
                properties: BTreeMap::from([("source".to_string(), json!("state.Demo.Initial"))]),
            }],
        };
        let library_context = validation_library_context();

        let report = validate_kir_semantics_with_context_and_policy(
            &document,
            Some(&library_context),
            SemanticValidationPolicy {
                mode: SemanticValidationMode::Off,
                ..SemanticValidationPolicy::default()
            },
        )
        .unwrap();

        assert!(report.is_empty());
    }

    #[test]
    fn validation_context_warnings_are_filtered_out() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: Vec::new(),
        };
        let mut library_context = validation_library_context();
        library_context.elements.push(KirElement {
            id: "transition.Context.bad".to_string(),
            kind: "SysML::Systems::TransitionUsage".to_string(),
            layer: 1,
            properties: BTreeMap::from([("source".to_string(), json!("context.source"))]),
        });

        let report =
            validate_kir_semantics_with_context(&document, Some(&library_context)).unwrap();

        assert!(report.is_empty());
    }

    #[test]
    fn generic_kir_feature_ownership_without_metamodel_contract_is_quiet() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "model.Type".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["feature.Demo.Vehicle.mass"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.mass".to_string(),
                    kind: "model.Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("type.Demo.Vehicle")),
                        ("declared_name".to_string(), json!("mass")),
                    ]),
                },
            ],
        };

        let report = validate_kir_semantics(&document).unwrap();

        assert!(report.is_empty());
    }

    pub(crate) fn validation_library_context() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements: vec![
                KirElement {
                    id: "SysML::Systems::TransitionUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                metamodel_feature(
                    "metafeature.TransitionUsage.source",
                    "SysML::Systems::TransitionUsage",
                    "source",
                ),
                metamodel_feature(
                    "metafeature.TransitionUsage.target",
                    "SysML::Systems::TransitionUsage",
                    "target",
                ),
            ],
        }
    }

    fn metamodel_feature(id: &str, owner: &str, kir_property: &str) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties: BTreeMap::from([
                ("owner".to_string(), json!(owner)),
                ("kir_property".to_string(), json!(kir_property)),
                ("feature_kind".to_string(), json!("reference")),
            ]),
        }
    }
}
