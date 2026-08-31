use serde::{Deserialize, Serialize};

use crate::runtime::Fact;
use crate::semantic_services::mutation::{
    ElementRef, SemanticAffordanceContext, SemanticElementContext,
    SemanticMutationCapabilityContext, SemanticReasoningContext,
};
use crate::semantic_services::semantic_legality::{
    SEMANTIC_LEGALITY_SCHEMA_VERSION, SemanticLegalityBatch, SemanticLegalityOperation,
    SemanticLegalityReport, SemanticLegalityService, SemanticLegalityStatus,
};
use crate::semantic_services::semantic_profile::{
    ConservativeSemanticCapabilityOracle, SemanticCapabilityOracle,
};

pub const SEMANTIC_NEXT_ACTIONS_SCHEMA_VERSION: &str = "mercurio.semantic_next_actions.v1";

#[derive(Debug, Clone)]
pub struct SemanticNextActionsService<O = ConservativeSemanticCapabilityOracle> {
    legality: SemanticLegalityService<O>,
    capability_context: SemanticMutationCapabilityContext,
}

impl SemanticNextActionsService<ConservativeSemanticCapabilityOracle> {
    pub fn new(capability_context: SemanticMutationCapabilityContext) -> Self {
        Self {
            legality: SemanticLegalityService::new(),
            capability_context,
        }
    }
}

impl<O> SemanticNextActionsService<O>
where
    O: SemanticCapabilityOracle,
{
    pub fn with_legality(
        legality: SemanticLegalityService<O>,
        capability_context: SemanticMutationCapabilityContext,
    ) -> Self {
        Self {
            legality,
            capability_context,
        }
    }

    pub fn next_actions(&self, request: SemanticNextActionsRequest) -> SemanticNextActionsReport {
        let batch = self.legality.batch(request.facts.clone());
        self.next_actions_with_batch(request, &batch)
    }

    /// `batch` must have been prepared from the same facts as
    /// `request.facts`; callers that answer many requests over one fact
    /// context reuse the batch across them.
    fn next_actions_with_batch(
        &self,
        request: SemanticNextActionsRequest,
        batch: &SemanticLegalityBatch<'_, O>,
    ) -> SemanticNextActionsReport {
        let mut candidates = Vec::new();
        // Many candidates (e.g. every concrete target of one kind) map to the
        // same kind-level legality operation - compute each distinct
        // operation once.
        let mut memo: std::collections::BTreeMap<String, SemanticLegalityReport> =
            std::collections::BTreeMap::new();
        let element = request.element.clone();

        for target_kind in &request.candidate_target_kinds {
            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::Specialize {
                    target_kind: target_kind.clone(),
                },
                SemanticLegalityOperation::Specialization {
                    source_kind: request.element_kind.clone(),
                    target_kind: target_kind.clone(),
                },
            );

            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::TypeUsage {
                    definition_kind: target_kind.clone(),
                },
                SemanticLegalityOperation::UsageTyping {
                    usage_kind: request.element_kind.clone(),
                    definition_kind: target_kind.clone(),
                },
            );
        }

        for attribute in &request.candidate_attributes {
            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::WriteAttribute {
                    attribute: attribute.clone(),
                },
                SemanticLegalityOperation::AttributeWrite {
                    kind: request.element_kind.clone(),
                    attribute: attribute.clone(),
                },
            );
        }

        for target_kind in &request.candidate_target_kinds {
            for relationship_kind in &self.capability_context.relationship_kinds {
                self.push_checked_action(
                    &mut candidates,
                    batch,
                    &mut memo,
                    element.clone(),
                    SemanticNextActionOperation::AddRelationship {
                        relationship_kind: relationship_kind.clone(),
                        target_kind: target_kind.clone(),
                        target: None,
                    },
                    SemanticLegalityOperation::Relationship {
                        relationship_kind: relationship_kind.clone(),
                        source_kind: request.element_kind.clone(),
                        target_kind: target_kind.clone(),
                    },
                );
            }
        }

        for target in &request.candidate_targets {
            for relationship_kind in &self.capability_context.relationship_kinds {
                self.push_checked_action(
                    &mut candidates,
                    batch,
                    &mut memo,
                    element.clone(),
                    SemanticNextActionOperation::AddRelationship {
                        relationship_kind: relationship_kind.clone(),
                        target_kind: target.kind.clone(),
                        target: Some(target.element.clone()),
                    },
                    SemanticLegalityOperation::Relationship {
                        relationship_kind: relationship_kind.clone(),
                        source_kind: request.element_kind.clone(),
                        target_kind: target.kind.clone(),
                    },
                );
            }
        }

        self.push_checked_action(
            &mut candidates,
            batch,
            &mut memo,
            element.clone(),
            SemanticNextActionOperation::AddPackage {
                child_kind: "package".to_string(),
            },
            SemanticLegalityOperation::Containment {
                container_kind: request.element_kind.clone(),
                child_kind: "package".to_string(),
            },
        );

        for kind in &self.capability_context.element_kinds {
            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::AddElement {
                    child_kind: kind.clone(),
                },
                SemanticLegalityOperation::Containment {
                    container_kind: request.element_kind.clone(),
                    child_kind: kind.clone(),
                },
            );
        }

        for keyword in &self.capability_context.definition_keywords {
            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::AddDefinition {
                    child_kind: keyword.clone(),
                },
                SemanticLegalityOperation::Containment {
                    container_kind: request.element_kind.clone(),
                    child_kind: keyword.clone(),
                },
            );
        }

        for keyword in &self.capability_context.usage_keywords {
            self.push_checked_action(
                &mut candidates,
                batch,
                &mut memo,
                element.clone(),
                SemanticNextActionOperation::AddUsage {
                    child_kind: keyword.clone(),
                },
                SemanticLegalityOperation::Containment {
                    container_kind: request.element_kind.clone(),
                    child_kind: keyword.clone(),
                },
            );
        }

        candidates.sort_by_key(|candidate| {
            (
                next_action_status_rank(&candidate.action.status),
                next_action_operation_rank(&candidate.action.operation),
                candidate.ordinal,
            )
        });
        let candidate_count = candidates.len();
        let limit = request.max_actions.unwrap_or(usize::MAX);
        let truncated = candidate_count > limit;
        let actions = candidates
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(rank, mut candidate)| {
                candidate.action.rank = rank as u32;
                candidate.action
            })
            .collect();

        SemanticNextActionsReport {
            schema_version: SEMANTIC_NEXT_ACTIONS_SCHEMA_VERSION.to_string(),
            legality_schema_version: SEMANTIC_LEGALITY_SCHEMA_VERSION.to_string(),
            element: request.element,
            element_kind: request.element_kind,
            actions,
            truncated,
        }
    }

    fn push_checked_action(
        &self,
        candidates: &mut Vec<RankedSemanticNextAction>,
        batch: &SemanticLegalityBatch<'_, O>,
        memo: &mut std::collections::BTreeMap<String, SemanticLegalityReport>,
        element: Option<ElementRef>,
        operation: SemanticNextActionOperation,
        legality_operation: SemanticLegalityOperation,
    ) {
        let memo_key = format!("{legality_operation:?}");
        let legality = memo
            .entry(memo_key)
            .or_insert_with(|| batch.check(legality_operation))
            .clone();
        candidates.push(RankedSemanticNextAction {
            ordinal: candidates.len(),
            action: SemanticNextAction {
                element,
                operation,
                status: legality.status,
                rank: u32::MAX,
                summary: next_action_summary(&legality),
                legality,
            },
        });
    }
}

#[derive(Debug, Clone)]
struct RankedSemanticNextAction {
    ordinal: usize,
    action: SemanticNextAction,
}

pub fn enrich_semantic_reasoning_context_with_next_action_affordances<O>(
    context: &mut SemanticReasoningContext,
    max_affordances: usize,
    service: &SemanticNextActionsService<O>,
) where
    O: SemanticCapabilityOracle,
{
    let facts = context
        .facts
        .iter()
        .map(|fact| Fact {
            predicate: fact.predicate.clone(),
            terms: fact.terms.clone(),
        })
        .collect::<Vec<_>>();
    let focus = context
        .focus
        .iter()
        .map(|element| element.qualified_name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let focused_only = !focus.is_empty();
    let batch = service.legality.batch(facts.clone());
    let containers = context
        .elements
        .iter()
        .filter(|element| {
            (!focused_only || focus.contains(&element.element.qualified_name))
                && next_action_element_has_affordances(element, context, service, &batch)
        })
        .cloned()
        .collect::<Vec<_>>();

    for element in containers {
        if context.affordances.len() >= max_affordances {
            context.truncated = true;
            return;
        }
        let report = service.next_actions_with_batch(
            SemanticNextActionsRequest {
                element: Some(element.element.clone()),
                element_kind: next_action_element_kind(&element),
                candidate_target_kinds: Vec::new(),
                candidate_targets: next_action_candidate_targets(context, &element),
                candidate_attributes: Vec::new(),
                facts: facts.clone(),
                max_actions: Some(max_affordances.saturating_sub(context.affordances.len())),
            },
            &batch,
        );
        for action in report.actions {
            if context.affordances.len() >= max_affordances {
                context.truncated = true;
                return;
            }
            let Some(affordance) = affordance_from_next_action(action) else {
                continue;
            };
            context.affordances.push(affordance);
        }
        if report.truncated {
            context.truncated = true;
            return;
        }
    }
}

fn affordance_from_next_action(action: SemanticNextAction) -> Option<SemanticAffordanceContext> {
    let (operation, child_kind) = match action.operation {
        SemanticNextActionOperation::AddPackage { child_kind } => ("AddPackage", child_kind),
        SemanticNextActionOperation::AddElement { child_kind } => ("AddElement", child_kind),
        SemanticNextActionOperation::AddDefinition { child_kind } => ("AddDefinition", child_kind),
        SemanticNextActionOperation::AddUsage { child_kind } => ("AddUsage", child_kind),
        SemanticNextActionOperation::AddRelationship {
            relationship_kind,
            target,
            ..
        } => (
            "AddRelationship",
            target
                .map(|target| format!("{relationship_kind} -> {}", target.qualified_name))
                .unwrap_or(relationship_kind),
        ),
        _ => return None,
    };
    Some(SemanticAffordanceContext {
        element: action.element?,
        operation: operation.to_string(),
        child_kind,
        status: format!("{:?}", action.status),
        reason: Some(action.summary),
    })
}

fn next_action_element_has_affordances<O>(
    element: &SemanticElementContext,
    context: &SemanticReasoningContext,
    service: &SemanticNextActionsService<O>,
    batch: &SemanticLegalityBatch<'_, O>,
) -> bool
where
    O: SemanticCapabilityOracle,
{
    next_action_element_can_own_children(element, service, batch)
        || next_action_element_can_relate_to_candidate(element, context, service, batch)
}

fn next_action_element_can_own_children<O>(
    element: &SemanticElementContext,
    service: &SemanticNextActionsService<O>,
    batch: &SemanticLegalityBatch<'_, O>,
) -> bool
where
    O: SemanticCapabilityOracle,
{
    let container_kind = next_action_element_kind(element);
    let child_kinds = std::iter::once("package")
        .chain(
            service
                .capability_context
                .element_kinds
                .iter()
                .map(String::as_str),
        )
        .chain(
            service
                .capability_context
                .definition_keywords
                .iter()
                .map(String::as_str),
        )
        .chain(
            service
                .capability_context
                .usage_keywords
                .iter()
                .map(String::as_str),
        );
    child_kinds.into_iter().any(|child_kind| {
        matches!(
            batch
                .check(SemanticLegalityOperation::Containment {
                    container_kind: container_kind.clone(),
                    child_kind: child_kind.to_string(),
                })
                .status,
            SemanticLegalityStatus::Allowed | SemanticLegalityStatus::AllowedWithWarnings
        )
    })
}

fn next_action_element_can_relate_to_candidate<O>(
    element: &SemanticElementContext,
    context: &SemanticReasoningContext,
    service: &SemanticNextActionsService<O>,
    batch: &SemanticLegalityBatch<'_, O>,
) -> bool
where
    O: SemanticCapabilityOracle,
{
    let source_kind = next_action_element_kind(element);
    next_action_candidate_targets(context, element)
        .into_iter()
        .any(|target| {
            service
                .capability_context
                .relationship_kinds
                .iter()
                .any(|relationship_kind| {
                    matches!(
                        batch
                            .check(SemanticLegalityOperation::Relationship {
                                relationship_kind: relationship_kind.clone(),
                                source_kind: source_kind.clone(),
                                target_kind: target.kind.clone(),
                            })
                            .status,
                        SemanticLegalityStatus::Allowed
                            | SemanticLegalityStatus::AllowedWithWarnings
                    )
                })
        })
}

fn next_action_element_kind(element: &SemanticElementContext) -> String {
    element
        .attributes
        .get("keyword")
        .and_then(serde_json::Value::as_str)
        .filter(|keyword| !keyword.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| element.kind.clone())
}

fn next_action_candidate_targets(
    context: &SemanticReasoningContext,
    source: &SemanticElementContext,
) -> Vec<SemanticNextActionTarget> {
    let mut seen = std::collections::BTreeSet::new();
    context
        .elements
        .iter()
        .filter(|candidate| candidate.element != source.element)
        .filter_map(|candidate| {
            let target = SemanticNextActionTarget {
                element: candidate.element.clone(),
                kind: next_action_target_kind(candidate),
                label: Some(candidate.label.clone()).filter(|label| !label.trim().is_empty()),
            };
            if target.kind.trim().is_empty()
                || target.element.qualified_name.trim().is_empty()
                || !seen.insert(target.element.qualified_name.clone())
            {
                return None;
            }
            Some(target)
        })
        .collect()
}

fn next_action_target_kind(element: &SemanticElementContext) -> String {
    if let Some(kir_kind) = element
        .attributes
        .get("kirKind")
        .and_then(serde_json::Value::as_str)
        .filter(|kind| !kind.trim().is_empty())
    {
        return kir_kind.to_string();
    }
    if let Some(keyword) = element
        .attributes
        .get("keyword")
        .and_then(serde_json::Value::as_str)
        .filter(|keyword| !keyword.trim().is_empty())
    {
        return if element.kind.eq_ignore_ascii_case("definition") {
            format!("{keyword} def")
        } else {
            keyword.to_string()
        };
    }
    element.kind.clone()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNextActionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementRef>,
    pub element_kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_target_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_targets: Vec<SemanticNextActionTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Fact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_actions: Option<usize>,
}

impl SemanticNextActionsRequest {
    pub fn for_kind(element_kind: impl Into<String>) -> Self {
        Self {
            element: None,
            element_kind: element_kind.into(),
            candidate_target_kinds: Vec::new(),
            candidate_targets: Vec::new(),
            candidate_attributes: Vec::new(),
            facts: Vec::new(),
            max_actions: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNextActionTarget {
    pub element: ElementRef,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNextActionsReport {
    pub schema_version: String,
    pub legality_schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementRef>,
    pub element_kind: String,
    pub actions: Vec<SemanticNextAction>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNextAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementRef>,
    pub operation: SemanticNextActionOperation,
    pub status: SemanticLegalityStatus,
    #[serde(default)]
    pub rank: u32,
    pub summary: String,
    pub legality: SemanticLegalityReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SemanticNextActionOperation {
    AddPackage {
        #[serde(rename = "childKind")]
        child_kind: String,
    },
    AddElement {
        #[serde(rename = "childKind")]
        child_kind: String,
    },
    AddDefinition {
        #[serde(rename = "childKind")]
        child_kind: String,
    },
    AddUsage {
        #[serde(rename = "childKind")]
        child_kind: String,
    },
    AddRelationship {
        #[serde(rename = "relationshipKind")]
        relationship_kind: String,
        #[serde(rename = "targetKind")]
        target_kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<ElementRef>,
    },
    Specialize {
        #[serde(rename = "targetKind")]
        target_kind: String,
    },
    TypeUsage {
        #[serde(rename = "definitionKind")]
        definition_kind: String,
    },
    WriteAttribute {
        attribute: String,
    },
}

fn next_action_summary(report: &SemanticLegalityReport) -> String {
    report
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| format!("{:?}", report.status).to_ascii_lowercase())
}

fn next_action_status_rank(status: &SemanticLegalityStatus) -> u8 {
    match status {
        SemanticLegalityStatus::Allowed => 0,
        SemanticLegalityStatus::AllowedWithWarnings => 1,
        SemanticLegalityStatus::Unknown => 2,
        SemanticLegalityStatus::Blocked => 3,
    }
}

fn next_action_operation_rank(operation: &SemanticNextActionOperation) -> u8 {
    match operation {
        SemanticNextActionOperation::WriteAttribute { .. } => 0,
        SemanticNextActionOperation::AddRelationship { .. } => 1,
        SemanticNextActionOperation::Specialize { .. } => 2,
        SemanticNextActionOperation::TypeUsage { .. } => 3,
        SemanticNextActionOperation::AddPackage { .. } => 4,
        SemanticNextActionOperation::AddElement { .. } => 5,
        SemanticNextActionOperation::AddDefinition { .. } => 6,
        SemanticNextActionOperation::AddUsage { .. } => 7,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::semantic_services::mutation::{
        AI_SEMANTIC_CONTEXT_SCHEMA_VERSION, AiSemanticContextUsage, SemanticElementContext,
        SemanticFactContext, SemanticMutationCapabilityContext, SemanticReasoningContext,
        SemanticRelationshipContext, WorkspaceRevision,
    };
    use crate::semantic_services::semantic_profile::{
        AttributePolicyAnswer, SemanticCapabilityProfile, TableSemanticCapabilityOracle,
    };

    use super::*;

    fn test_ai_context_usage() -> AiSemanticContextUsage {
        AiSemanticContextUsage {
            authoritative_for_existing_elements: true,
            prefer_ranked_allowed_affordances: true,
            cite_rule_diagnostics: true,
            element_ref_format: "dot_qualified".to_string(),
        }
    }

    #[test]
    fn next_actions_attach_legality_reports_to_candidates() {
        let profile = SemanticCapabilityProfile::default()
            .allow_containment("package", "part")
            .allow_relationship("satisfy", "part", "requirement")
            .attribute_policy(
                "*",
                "text",
                AttributePolicyAnswer {
                    writable: true,
                    reason: None,
                },
            );
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: vec!["part".to_string()],
                usage_keywords: Vec::new(),
                relationship_kinds: vec!["satisfy".to_string()],
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );

        let report = service.next_actions(SemanticNextActionsRequest {
            element: Some(ElementRef::new("Vehicle")),
            element_kind: "part".to_string(),
            candidate_target_kinds: vec!["requirement".to_string(), "part".to_string()],
            candidate_targets: Vec::new(),
            candidate_attributes: vec!["text".to_string(), "private".to_string()],
            facts: Vec::new(),
            max_actions: None,
        });

        assert!(report.actions.iter().any(|action| {
            action.operation
                == SemanticNextActionOperation::AddRelationship {
                    relationship_kind: "satisfy".to_string(),
                    target_kind: "requirement".to_string(),
                    target: None,
                }
                && action.status == SemanticLegalityStatus::Allowed
        }));
        assert!(report.actions.iter().any(|action| {
            action.operation
                == SemanticNextActionOperation::AddRelationship {
                    relationship_kind: "satisfy".to_string(),
                    target_kind: "part".to_string(),
                    target: None,
                }
                && action.status == SemanticLegalityStatus::Blocked
        }));
        assert!(report.actions.iter().any(|action| {
            action.operation
                == SemanticNextActionOperation::WriteAttribute {
                    attribute: "text".to_string(),
                }
                && action.status == SemanticLegalityStatus::Allowed
        }));
        assert!(report.actions.iter().any(|action| {
            action.operation
                == SemanticNextActionOperation::WriteAttribute {
                    attribute: "private".to_string(),
                }
                && action.status == SemanticLegalityStatus::Blocked
        }));
    }

    #[test]
    fn next_actions_prioritize_allowed_explicit_candidates_before_blocked_actions() {
        let profile = SemanticCapabilityProfile::default()
            .allow_relationship("satisfy", "part", "requirement")
            .attribute_policy(
                "*",
                "text",
                AttributePolicyAnswer {
                    writable: true,
                    reason: None,
                },
            );
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: vec![
                    "part".to_string(),
                    "requirement".to_string(),
                    "action".to_string(),
                ],
                usage_keywords: vec!["part".to_string(), "satisfy".to_string()],
                relationship_kinds: vec!["satisfy".to_string()],
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );

        let report = service.next_actions(SemanticNextActionsRequest {
            element: Some(ElementRef::new("Vehicle")),
            element_kind: "part".to_string(),
            candidate_target_kinds: vec!["requirement".to_string()],
            candidate_targets: Vec::new(),
            candidate_attributes: vec!["text".to_string()],
            facts: Vec::new(),
            max_actions: Some(4),
        });

        assert_eq!(report.actions.len(), 4);
        assert_eq!(
            report
                .actions
                .iter()
                .map(|action| action.rank)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(
            report.actions[0].operation,
            SemanticNextActionOperation::WriteAttribute {
                attribute: "text".to_string(),
            }
        );
        assert_eq!(report.actions[0].status, SemanticLegalityStatus::Allowed);
        assert_eq!(
            report.actions[1].operation,
            SemanticNextActionOperation::AddRelationship {
                relationship_kind: "satisfy".to_string(),
                target_kind: "requirement".to_string(),
                target: None,
            }
        );
        assert_eq!(report.actions[1].status, SemanticLegalityStatus::Allowed);
        assert_eq!(
            report.actions[2].operation,
            SemanticNextActionOperation::Specialize {
                target_kind: "requirement".to_string(),
            }
        );
        assert_eq!(report.actions[2].status, SemanticLegalityStatus::Blocked);
        assert!(report.truncated);
    }

    #[test]
    fn next_actions_only_reports_truncated_when_candidates_exceed_limit() {
        let profile = SemanticCapabilityProfile::default().attribute_policy(
            "*",
            "text",
            AttributePolicyAnswer {
                writable: true,
                reason: None,
            },
        );
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: Vec::new(),
                usage_keywords: Vec::new(),
                relationship_kinds: Vec::new(),
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );

        let report = service.next_actions(SemanticNextActionsRequest {
            element: Some(ElementRef::new("Vehicle")),
            element_kind: "part".to_string(),
            candidate_target_kinds: Vec::new(),
            candidate_targets: Vec::new(),
            candidate_attributes: vec!["text".to_string()],
            facts: Vec::new(),
            max_actions: Some(2),
        });

        assert_eq!(report.actions.len(), 2);
        assert_eq!(report.actions[0].rank, 0);
        assert_eq!(report.actions[1].rank, 1);
        assert!(!report.truncated);
    }

    #[test]
    fn next_action_affordances_use_element_keyword_for_legality_checks() {
        let profile = SemanticCapabilityProfile::default().allow_containment("part", "part");
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: vec!["part".to_string()],
                usage_keywords: Vec::new(),
                relationship_kinds: Vec::new(),
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );
        let mut context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: vec![ElementRef::new("vehicle")],
            elements: vec![SemanticElementContext {
                element: ElementRef::new("vehicle"),
                kind: "usage".to_string(),
                label: "vehicle".to_string(),
                owner: None,
                attributes: BTreeMap::from([(
                    "keyword".to_string(),
                    Value::String("part".to_string()),
                )]),
            }],
            relationships: Vec::<SemanticRelationshipContext>::new(),
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };

        enrich_semantic_reasoning_context_with_next_action_affordances(&mut context, 8, &service);

        assert!(context.affordances.iter().any(|affordance| {
            affordance.operation == "AddDefinition"
                && affordance.child_kind == "part"
                && affordance.status == "Allowed"
        }));
    }

    #[test]
    fn next_action_affordances_include_concrete_relationship_targets() {
        let profile = SemanticCapabilityProfile::default()
            .allow_containment("part", "part")
            .allow_relationship("satisfy", "part", "requirement");
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: Vec::new(),
                usage_keywords: Vec::new(),
                relationship_kinds: vec!["satisfy".to_string()],
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );
        let mut context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: vec![ElementRef::new("vehicle")],
            elements: vec![
                SemanticElementContext {
                    element: ElementRef::new("vehicle"),
                    kind: "usage".to_string(),
                    label: "vehicle".to_string(),
                    owner: None,
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        Value::String("part".to_string()),
                    )]),
                },
                SemanticElementContext {
                    element: ElementRef::new("SafetyRequirement"),
                    kind: "usage".to_string(),
                    label: "SafetyRequirement".to_string(),
                    owner: None,
                    attributes: BTreeMap::from([(
                        "keyword".to_string(),
                        Value::String("requirement".to_string()),
                    )]),
                },
            ],
            relationships: Vec::<SemanticRelationshipContext>::new(),
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };

        enrich_semantic_reasoning_context_with_next_action_affordances(&mut context, 8, &service);

        assert!(context.affordances.iter().any(|affordance| {
            affordance.operation == "AddRelationship"
                && affordance.child_kind == "satisfy -> SafetyRequirement"
                && affordance.status == "Allowed"
        }));
    }

    #[test]
    fn next_action_affordances_do_not_treat_kind_substrings_as_containers() {
        let profile = SemanticCapabilityProfile::default().allow_containment("part", "part");
        let service = SemanticNextActionsService::with_legality(
            SemanticLegalityService::with_oracle(TableSemanticCapabilityOracle::new(profile)),
            SemanticMutationCapabilityContext {
                metamodel_version: "test".to_string(),
                supported_operations: Vec::new(),
                variant_capabilities:
                    crate::semantic_services::variant::default_semantic_variant_capability_context(),
                element_kinds: Vec::new(),
                definition_keywords: vec!["part".to_string()],
                usage_keywords: Vec::new(),
                relationship_kinds: Vec::new(),
                usage_typing_rules: Vec::new(),
                relationship_target_rules: Vec::new(),
                guidance: Vec::new(),
            },
        );
        let mut context = SemanticReasoningContext {
            schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: WorkspaceRevision::unchecked(),
            focus: vec![ElementRef::new("notAContainer")],
            elements: vec![SemanticElementContext {
                element: ElementRef::new("notAContainer"),
                kind: "kirElement".to_string(),
                label: "notAContainer".to_string(),
                owner: None,
                attributes: BTreeMap::from([(
                    "kirKind".to_string(),
                    Value::String("NonUsageThing".to_string()),
                )]),
            }],
            relationships: Vec::<SemanticRelationshipContext>::new(),
            facts: Vec::<SemanticFactContext>::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };

        enrich_semantic_reasoning_context_with_next_action_affordances(&mut context, 8, &service);

        assert!(context.affordances.is_empty());
    }
}
