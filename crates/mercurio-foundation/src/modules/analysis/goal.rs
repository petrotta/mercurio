#![allow(deprecated)]

use serde::{Deserialize, Serialize};

use crate::semantic_services::mutation::{
    ElementRef, SemanticReasoningContext, SemanticRelationshipContext, WorkspaceRevision,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticGoalSpec {
    pub policy: GoalPolicy,
    pub checks: Vec<SemanticGoalCheck>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticGoalProfile {
    pub id: String,
    pub name: String,
    pub kind: SemanticGoalProfileKind,
    pub goal: SemanticGoalSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticGoalProfileKind {
    Task,
    Quality,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GoalPolicy {
    All,
    Any,
    ScoreAtLeast(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticGoalCheck {
    AllOf {
        checks: Vec<SemanticGoalCheck>,
    },
    AnyOf {
        checks: Vec<SemanticGoalCheck>,
    },
    ElementExists {
        element: ElementRef,
        kind: Option<String>,
    },
    NamedElementExists {
        name: String,
        kind: Option<String>,
    },
    RelationshipExists {
        source: ElementRef,
        kind: String,
        target: ElementRef,
    },
    NamedRelationshipExists {
        source_name: String,
        kind: String,
        target_name: String,
    },
    #[deprecated(
        note = "requirement quality checks are domain-specific; use mercurio-requirements"
    )]
    RequirementsHaveFields {
        fields: Vec<String>,
    },
    TypedUsages {
        usage_kinds: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalEvaluation {
    pub satisfied: bool,
    pub score: f64,
    pub policy: GoalPolicy,
    pub checked_against: WorkspaceRevision,
    pub results: Vec<GoalCheckEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalCheckEvaluation {
    pub check: SemanticGoalCheck,
    pub satisfied: bool,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticGoalExplanation {
    pub policy: String,
    pub instructions: Vec<String>,
}

pub fn explain_semantic_goal(goal: &SemanticGoalSpec) -> SemanticGoalExplanation {
    SemanticGoalExplanation {
        policy: explain_goal_policy(goal.policy, goal.checks.len()),
        instructions: goal.checks.iter().flat_map(explain_goal_check).collect(),
    }
}

fn explain_goal_policy(policy: GoalPolicy, check_count: usize) -> String {
    match policy {
        GoalPolicy::All => format!("All {check_count} checks must be satisfied."),
        GoalPolicy::Any => format!("At least one of {check_count} checks must be satisfied."),
        GoalPolicy::ScoreAtLeast(threshold) => {
            format!("The satisfied check score must be at least {threshold:.2}.")
        }
    }
}

fn explain_goal_check(check: &SemanticGoalCheck) -> Vec<String> {
    match check {
        SemanticGoalCheck::AllOf { checks } => checks.iter().flat_map(explain_goal_check).collect(),
        SemanticGoalCheck::AnyOf { checks } => {
            let options = checks
                .iter()
                .flat_map(explain_goal_check)
                .collect::<Vec<_>>()
                .join(" OR ");
            vec![format!("Satisfy at least one alternative: {options}")]
        }
        SemanticGoalCheck::ElementExists { element, kind } => vec![format!(
            "Ensure element `{}` exists{}.",
            element.qualified_name,
            kind.as_ref()
                .map(|kind| format!(" as a `{kind}`"))
                .unwrap_or_default()
        )],
        SemanticGoalCheck::NamedElementExists { name, kind } => vec![format!(
            "Ensure a model element named `{name}` exists{}.",
            kind.as_ref()
                .map(|kind| format!(" as a `{kind}`"))
                .unwrap_or_default()
        )],
        SemanticGoalCheck::RelationshipExists {
            source,
            kind,
            target,
        } => vec![format!(
            "Ensure `{}` has a `{kind}` relationship to `{}`.",
            source.qualified_name, target.qualified_name
        )],
        SemanticGoalCheck::NamedRelationshipExists {
            source_name,
            kind,
            target_name,
        } => vec![format!(
            "Ensure `{source_name}` has a `{kind}` relationship to `{target_name}`."
        )],
        SemanticGoalCheck::RequirementsHaveFields { fields } => vec![format!(
            "Every requirement element must have non-empty semantic field(s): {}. If an existing requirement is missing one, propose SetAttribute for that field instead of creating a duplicate requirement.",
            fields.join(", ")
        )],
        SemanticGoalCheck::TypedUsages { usage_kinds } => vec![format!(
            "Every usage with kind(s) {} must be explicitly typed by an appropriate definition. Prefer AddDefinition first, then AddUsage with ty set to that definition.",
            usage_kinds.join(", ")
        )],
    }
}

pub fn evaluate_semantic_goal(
    context: &SemanticReasoningContext,
    goal: &SemanticGoalSpec,
) -> GoalEvaluation {
    let results = goal
        .checks
        .iter()
        .cloned()
        .map(|check| evaluate_goal_check(context, check))
        .collect::<Vec<_>>();
    let satisfied_count = results.iter().filter(|result| result.satisfied).count();
    let score = if results.is_empty() {
        1.0
    } else {
        satisfied_count as f64 / results.len() as f64
    };
    let satisfied = match goal.policy {
        GoalPolicy::All => results.iter().all(|result| result.satisfied),
        GoalPolicy::Any => results.iter().any(|result| result.satisfied),
        GoalPolicy::ScoreAtLeast(threshold) => score >= threshold,
    };

    GoalEvaluation {
        satisfied,
        score,
        policy: goal.policy,
        checked_against: context.workspace_revision.clone(),
        results,
    }
}

fn evaluate_goal_check(
    context: &SemanticReasoningContext,
    check: SemanticGoalCheck,
) -> GoalCheckEvaluation {
    match &check {
        SemanticGoalCheck::AllOf { checks } => {
            let nested = checks
                .iter()
                .cloned()
                .map(|check| evaluate_goal_check(context, check))
                .collect::<Vec<_>>();
            let satisfied = nested.iter().all(|result| result.satisfied);
            let evidence = nested
                .iter()
                .flat_map(|result| result.evidence.iter().cloned())
                .collect();
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::AnyOf { checks } => {
            let nested = checks
                .iter()
                .cloned()
                .map(|check| evaluate_goal_check(context, check))
                .collect::<Vec<_>>();
            let satisfied = nested.iter().any(|result| result.satisfied);
            let evidence = nested
                .iter()
                .flat_map(|result| result.evidence.iter().cloned())
                .collect();
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::ElementExists { element, kind } => {
            let mut evidence = Vec::new();
            let satisfied = context.elements.iter().any(|candidate| {
                if candidate.element != *element {
                    return false;
                }
                evidence.push(format!("found element `{}`", element.qualified_name));
                kind.as_ref().is_none_or(|expected_kind| {
                    let direct_kind_matches = candidate.kind.eq_ignore_ascii_case(expected_kind);
                    let keyword_matches = candidate
                        .attributes
                        .get("keyword")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|keyword| keyword.eq_ignore_ascii_case(expected_kind));
                    let kir_kind_matches = candidate
                        .attributes
                        .get("kirKind")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|kir_kind| kir_kind.eq_ignore_ascii_case(expected_kind));
                    direct_kind_matches || keyword_matches || kir_kind_matches
                })
            });
            if !satisfied {
                evidence.push(format!("missing element `{}`", element.qualified_name));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::NamedElementExists { name, kind } => {
            let mut evidence = Vec::new();
            let satisfied = context.elements.iter().any(|candidate| {
                if !element_name_matches(candidate, name) {
                    return false;
                }
                let kind_matches = kind
                    .as_ref()
                    .is_none_or(|expected_kind| element_kind_matches(candidate, expected_kind));
                if kind_matches {
                    evidence.push(format!(
                        "found element named `{name}` at `{}`",
                        candidate.element.qualified_name
                    ));
                }
                kind_matches
            });
            if !satisfied {
                evidence.push(format!("missing element named `{name}`"));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::RelationshipExists {
            source,
            kind,
            target,
        } => {
            let wanted = SemanticRelationshipContext {
                kind: kind.clone(),
                source: source.clone(),
                target: target.clone(),
            };
            let mut evidence = Vec::new();
            let satisfied = context.relationships.iter().any(|relationship| {
                relationship.source == wanted.source
                    && relationship.target == wanted.target
                    && relationship_kind_matches(&relationship.kind, &wanted.kind)
            });
            if satisfied {
                evidence.push(format!(
                    "found relationship `{}` --{}--> `{}`",
                    source.qualified_name, kind, target.qualified_name
                ));
            } else {
                evidence.push(format!(
                    "missing relationship `{}` --{}--> `{}`",
                    source.qualified_name, kind, target.qualified_name
                ));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::NamedRelationshipExists {
            source_name,
            kind,
            target_name,
        } => {
            let mut evidence = Vec::new();
            let satisfied = context.relationships.iter().any(|relationship| {
                let matches = relationship_kind_matches(&relationship.kind, kind)
                    && element_ref_name_matches(context, &relationship.source, source_name)
                    && element_ref_name_matches(context, &relationship.target, target_name);
                if matches {
                    evidence.push(format!(
                        "found relationship `{}` --{}--> `{}`",
                        relationship.source.qualified_name,
                        relationship.kind,
                        relationship.target.qualified_name
                    ));
                }
                matches
            });
            if !satisfied {
                evidence.push(format!(
                    "missing relationship named `{source_name}` --{kind}--> `{target_name}`"
                ));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::RequirementsHaveFields { fields } => {
            let requirements = context
                .elements
                .iter()
                .filter(|element| element_kind_matches(element, "requirement"))
                .collect::<Vec<_>>();
            let mut evidence = Vec::new();
            for requirement in &requirements {
                for field in fields {
                    if !element_has_required_field(requirement, field) {
                        evidence.push(format!(
                            "requirement `{}` is missing `{field}`",
                            requirement.element.qualified_name
                        ));
                    }
                }
            }
            let satisfied = evidence.is_empty();
            if satisfied {
                evidence.push(format!(
                    "all {} requirement element(s) have required fields: {}",
                    requirements.len(),
                    fields.join(", ")
                ));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
        SemanticGoalCheck::TypedUsages { usage_kinds } => {
            let mut evidence = Vec::new();
            for element in &context.elements {
                if element.kind != "usage" {
                    continue;
                }
                let keyword = element
                    .attributes
                    .get("keyword")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if !usage_kinds
                    .iter()
                    .any(|usage_kind| keyword.eq_ignore_ascii_case(usage_kind))
                {
                    continue;
                }
                if !element.attributes.contains_key("type") {
                    evidence.push(format!(
                        "usage `{}` with kind `{keyword}` is missing type",
                        element.element.qualified_name
                    ));
                }
            }
            let satisfied = evidence.is_empty();
            if satisfied {
                evidence.push(format!(
                    "all usage kinds have types where required: {}",
                    usage_kinds.join(", ")
                ));
            }
            GoalCheckEvaluation {
                check,
                satisfied,
                evidence,
            }
        }
    }
}

pub fn default_model_quality_profile() -> SemanticGoalProfile {
    SemanticGoalProfile {
        id: "default-model-quality".to_string(),
        name: "Default Model Quality".to_string(),
        kind: SemanticGoalProfileKind::Quality,
        goal: SemanticGoalSpec {
            policy: GoalPolicy::All,
            checks: vec![
                SemanticGoalCheck::RequirementsHaveFields {
                    fields: vec!["id".to_string(), "text".to_string()],
                },
                SemanticGoalCheck::TypedUsages {
                    usage_kinds: vec!["part".to_string()],
                },
            ],
        },
    }
}

fn element_kind_matches(
    element: &crate::semantic_services::mutation::SemanticElementContext,
    expected: &str,
) -> bool {
    element.kind.eq_ignore_ascii_case(expected)
        || element
            .attributes
            .get("keyword")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|keyword| keyword.eq_ignore_ascii_case(expected))
        || element
            .attributes
            .get("kirKind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kir_kind| kir_kind.eq_ignore_ascii_case(expected))
}

fn element_name_matches(
    element: &crate::semantic_services::mutation::SemanticElementContext,
    expected: &str,
) -> bool {
    element.label.eq_ignore_ascii_case(expected)
        || qualified_name_leaf_matches(&element.element.qualified_name, expected)
        || element
            .attributes
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case(expected))
        || element
            .attributes
            .get("declaredName")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}

fn element_ref_name_matches(
    context: &SemanticReasoningContext,
    element: &ElementRef,
    expected: &str,
) -> bool {
    context
        .elements
        .iter()
        .find(|candidate| candidate.element == *element)
        .is_some_and(|candidate| element_name_matches(candidate, expected))
        || qualified_name_leaf_matches(&element.qualified_name, expected)
}

fn qualified_name_leaf_matches(qualified_name: &str, expected: &str) -> bool {
    qualified_name
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())
        .is_some_and(|leaf| leaf.eq_ignore_ascii_case(expected))
}

fn element_has_required_field(
    element: &crate::semantic_services::mutation::SemanticElementContext,
    field: &str,
) -> bool {
    match field {
        "text" => {
            string_attribute_present(element, "text")
                || string_attribute_present(element, "body")
                || string_attribute_present(element, "doc")
                || string_attribute_present(element, "documentation")
                || !element
                    .attributes
                    .get("docs")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(Vec::is_empty)
        }
        "id" => {
            string_attribute_present(element, "id")
                || string_attribute_present(element, "requirementId")
                || string_attribute_present(element, "requirement_id")
        }
        other => element
            .attributes
            .get(other)
            .is_some_and(|value| match value {
                serde_json::Value::String(text) => !text.trim().is_empty(),
                serde_json::Value::Array(items) => !items.is_empty(),
                serde_json::Value::Null => false,
                _ => true,
            }),
    }
}

fn string_attribute_present(
    element: &crate::semantic_services::mutation::SemanticElementContext,
    attribute: &str,
) -> bool {
    element
        .attributes
        .get(attribute)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

fn relationship_kind_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
        || actual
            .strip_prefix("kir.")
            .is_some_and(|kind| kind.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_services::mutation::{
        AI_SEMANTIC_CONTEXT_SCHEMA_VERSION, AiSemanticContextUsage, SemanticElementContext,
        SemanticFactContext, SemanticRelationshipContext,
    };
    use std::collections::BTreeMap;

    fn test_ai_context_usage() -> AiSemanticContextUsage {
        AiSemanticContextUsage {
            authoritative_for_existing_elements: true,
            prefer_ranked_allowed_affordances: true,
            cite_rule_diagnostics: true,
            element_ref_format: "dot_qualified".to_string(),
        }
    }

    #[test]
    fn evaluates_required_elements_and_relationships() {
        let context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: Vec::new(),
            elements: vec![
                SemanticElementContext {
                    element: ElementRef::new("HybridVehicle.HybridVehicle"),
                    kind: "definition".to_string(),
                    label: "HybridVehicle".to_string(),
                    owner: Some(ElementRef::new("HybridVehicle")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("part".to_string()),
                    )]),
                },
                SemanticElementContext {
                    element: ElementRef::new("HybridVehicle.ImproveEfficiency"),
                    kind: "definition".to_string(),
                    label: "ImproveEfficiency".to_string(),
                    owner: Some(ElementRef::new("HybridVehicle")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("requirement".to_string()),
                    )]),
                },
            ],
            relationships: vec![SemanticRelationshipContext {
                kind: "satisfy".to_string(),
                source: ElementRef::new("HybridVehicle.HybridVehicle"),
                target: ElementRef::new("HybridVehicle.ImproveEfficiency"),
            }],
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };
        let goal = SemanticGoalSpec {
            policy: GoalPolicy::All,
            checks: vec![
                SemanticGoalCheck::ElementExists {
                    element: ElementRef::new("HybridVehicle.HybridVehicle"),
                    kind: Some("part".to_string()),
                },
                SemanticGoalCheck::RelationshipExists {
                    source: ElementRef::new("HybridVehicle.HybridVehicle"),
                    kind: "satisfy".to_string(),
                    target: ElementRef::new("HybridVehicle.ImproveEfficiency"),
                },
            ],
        };

        let evaluation = evaluate_semantic_goal(&context, &goal);

        assert!(evaluation.satisfied);
        assert_eq!(evaluation.score, 1.0);
        assert_eq!(evaluation.results.len(), 2);
    }

    #[test]
    fn evaluates_named_elements_and_relationships() {
        let context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: Vec::new(),
            elements: vec![
                SemanticElementContext {
                    element: ElementRef::new("HybridVehicle.Vehicle.RegenerativeBraking"),
                    kind: "definition".to_string(),
                    label: "RegenerativeBraking".to_string(),
                    owner: Some(ElementRef::new("HybridVehicle.Vehicle")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("action".to_string()),
                    )]),
                },
                SemanticElementContext {
                    element: ElementRef::new("HybridVehicle.EfficiencyRequirement"),
                    kind: "definition".to_string(),
                    label: "EfficiencyRequirement".to_string(),
                    owner: Some(ElementRef::new("HybridVehicle")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("requirement".to_string()),
                    )]),
                },
            ],
            relationships: vec![SemanticRelationshipContext {
                kind: "satisfy".to_string(),
                source: ElementRef::new("HybridVehicle.Vehicle.RegenerativeBraking"),
                target: ElementRef::new("HybridVehicle.EfficiencyRequirement"),
            }],
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };
        let goal = SemanticGoalSpec {
            policy: GoalPolicy::All,
            checks: vec![
                SemanticGoalCheck::AnyOf {
                    checks: vec![
                        SemanticGoalCheck::NamedElementExists {
                            name: "RegenerativeBrakingSystem".to_string(),
                            kind: Some("part".to_string()),
                        },
                        SemanticGoalCheck::NamedElementExists {
                            name: "RegenerativeBraking".to_string(),
                            kind: Some("action".to_string()),
                        },
                    ],
                },
                SemanticGoalCheck::NamedRelationshipExists {
                    source_name: "RegenerativeBraking".to_string(),
                    kind: "satisfy".to_string(),
                    target_name: "EfficiencyRequirement".to_string(),
                },
            ],
        };

        let evaluation = evaluate_semantic_goal(&context, &goal);

        assert!(evaluation.satisfied);
        assert_eq!(evaluation.score, 1.0);
    }

    #[test]
    fn default_quality_profile_flags_requirements_without_id_text_and_untyped_part_usages() {
        let context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: Vec::new(),
            elements: vec![
                SemanticElementContext {
                    element: ElementRef::new("Demo.NeedRange"),
                    kind: "definition".to_string(),
                    label: "NeedRange".to_string(),
                    owner: Some(ElementRef::new("Demo")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("requirement".to_string()),
                    )]),
                },
                SemanticElementContext {
                    element: ElementRef::new("Demo.Vehicle.engine"),
                    kind: "usage".to_string(),
                    label: "engine".to_string(),
                    owner: Some(ElementRef::new("Demo.Vehicle")),
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        serde_json::Value::String("part".to_string()),
                    )]),
                },
            ],
            relationships: Vec::new(),
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };

        let evaluation = evaluate_semantic_goal(&context, &default_model_quality_profile().goal);

        assert!(!evaluation.satisfied);
        assert_eq!(evaluation.score, 0.0);
        assert!(evaluation.results.iter().any(|result| {
            !result.satisfied
                && matches!(
                    result.check,
                    SemanticGoalCheck::RequirementsHaveFields { .. }
                )
        }));
        assert!(evaluation.results.iter().any(|result| {
            !result.satisfied && matches!(result.check, SemanticGoalCheck::TypedUsages { .. })
        }));
    }

    #[test]
    fn explains_default_quality_profile_for_ai_guidance() {
        let explanation = explain_semantic_goal(&default_model_quality_profile().goal);

        assert_eq!(explanation.policy, "All 2 checks must be satisfied.");
        assert!(explanation.instructions.iter().any(|instruction| {
            instruction.contains("Every requirement element must have non-empty semantic field")
                && instruction.contains("id, text")
                && instruction.contains("SetAttribute")
        }));
        assert!(explanation.instructions.iter().any(|instruction| {
            instruction.contains("Every usage with kind(s) part must be explicitly typed")
        }));
    }

    #[test]
    fn default_quality_profile_accepts_requirement_id_text_and_typed_part_usage() {
        let context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: Vec::new(),
            elements: vec![
                SemanticElementContext {
                    element: ElementRef::new("Demo.NeedRange"),
                    kind: "definition".to_string(),
                    label: "NeedRange".to_string(),
                    owner: Some(ElementRef::new("Demo")),
                    attributes: BTreeMap::from([
                        (
                            "keyword".to_string(),
                            serde_json::Value::String("requirement".to_string()),
                        ),
                        (
                            "id".to_string(),
                            serde_json::Value::String("REQ-001".to_string()),
                        ),
                        (
                            "text".to_string(),
                            serde_json::Value::String(
                                "The vehicle shall exceed 200 miles.".to_string(),
                            ),
                        ),
                    ]),
                },
                SemanticElementContext {
                    element: ElementRef::new("Demo.Vehicle.engine"),
                    kind: "usage".to_string(),
                    label: "engine".to_string(),
                    owner: Some(ElementRef::new("Demo.Vehicle")),
                    attributes: BTreeMap::from([
                        (
                            "keyword".to_string(),
                            serde_json::Value::String("part".to_string()),
                        ),
                        (
                            "type".to_string(),
                            serde_json::Value::String("Engine".to_string()),
                        ),
                    ]),
                },
            ],
            relationships: Vec::new(),
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };

        let evaluation = evaluate_semantic_goal(&context, &default_model_quality_profile().goal);

        assert!(evaluation.satisfied);
        assert_eq!(evaluation.score, 1.0);
    }
}
