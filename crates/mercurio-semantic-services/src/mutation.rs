use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identity::{SemanticAnchor, stable_digest};
use crate::semantic_profile::{
    CapabilityAnswer, ConservativeSemanticCapabilityOracle, SemanticCapabilityOracle,
};
use crate::variant::{
    SemanticVariantCapabilityContext, default_semantic_variant_capability_context,
};
use mercurio_authoring::authoring::{
    AuthoringModule, AuthoringProject, Declaration, MutationResult, QualifiedName,
};
pub use mercurio_authoring::authoring::WriteBackMode;
use mercurio_kir::{
    KIR_PROP_NAME, KIR_PROP_OWNER, KIR_PROP_SPECIALIZES, KIR_PROP_TYPE, KirDocument, KirElement,
};
use mercurio_model::{Element, Graph};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementRef {
    pub qualified_name: String,
}

impl ElementRef {
    pub fn new(qualified_name: impl Into<String>) -> Self {
        Self {
            qualified_name: qualified_name.into(),
        }
    }

    pub fn as_qualified_name(&self) -> QualifiedName {
        QualifiedName::parse(&self.qualified_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceRevision {
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticElementRef {
    pub element_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_anchor: Option<SemanticAnchor>,
}

impl SemanticElementRef {
    pub fn new(element_id: impl Into<String>) -> Self {
        Self {
            element_id: element_id.into(),
            qualified_name: None,
            label: None,
            kind: None,
            semantic_anchor: None,
        }
    }

    pub fn unresolved(value: impl Into<String>) -> Self {
        let value = value.into();
        Self::new(value.clone()).with_qualified_name(value)
    }

    pub fn from_element(element: &KirElement) -> Self {
        Self::new(element.id.clone())
            .with_qualified_name_optional(string_property(&element.properties, "qualified_name"))
            .with_label_optional(kir_element_label(element))
            .with_kind(element.kind.clone())
    }

    pub fn from_graph_element(element: &Element) -> Self {
        Self::new(element.element_id.clone())
            .with_qualified_name_optional(graph_string_property(element, "qualified_name"))
            .with_label_optional(graph_element_label(element))
            .with_kind(element.kind.to_string())
    }

    pub fn with_qualified_name(mut self, qualified_name: impl Into<String>) -> Self {
        self.qualified_name = Some(qualified_name.into());
        self
    }

    pub fn with_qualified_name_optional(mut self, qualified_name: Option<String>) -> Self {
        self.qualified_name = qualified_name;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_label_optional(mut self, label: Option<String>) -> Self {
        self.label = label;
        self
    }

    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn with_kind_optional(mut self, kind: Option<String>) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_anchor(mut self, anchor: SemanticAnchor) -> Self {
        self.semantic_anchor = Some(anchor);
        self
    }

    pub fn with_anchor_optional(mut self, anchor: Option<SemanticAnchor>) -> Self {
        self.semantic_anchor = anchor;
        self
    }
}

impl WorkspaceRevision {
    pub fn unchecked() -> Self {
        Self {
            fingerprint: "unchecked".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationProposal {
    pub intent: String,
    pub operations: Vec<SemanticMutation>,
    pub evidence: Vec<MutationEvidence>,
    pub rationale: Option<String>,
    pub workspace_revision: WorkspaceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvidence {
    pub element: Option<ElementRef>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationPlan {
    pub proposal_id: String,
    pub normalized_operations: Vec<SemanticMutation>,
    pub required_supporting_changes: Vec<SemanticMutation>,
    pub checked_against: WorkspaceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticElementKind {
    pub metaclass: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl SemanticElementKind {
    pub fn new(metaclass: impl Into<String>) -> Self {
        Self {
            metaclass: metaclass.into(),
            profile: None,
        }
    }

    pub fn with_profile(metaclass: impl Into<String>, profile: impl Into<String>) -> Self {
        Self {
            metaclass: metaclass.into(),
            profile: Some(profile.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SemanticMutation {
    AddPackage {
        target_file: String,
        name: String,
    },
    AddElement {
        container: ElementRef,
        kind: SemanticElementKind,
        name: String,
        ty: Option<ElementRef>,
        specializes: Vec<ElementRef>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        properties: BTreeMap<String, Value>,
    },
    AddDefinition {
        container: ElementRef,
        keyword: String,
        name: String,
        specializes: Vec<ElementRef>,
    },
    AddUsage {
        container: ElementRef,
        keyword: String,
        name: String,
        ty: Option<ElementRef>,
        specializes: Vec<ElementRef>,
    },
    AddRelationship {
        kind: String,
        source: ElementRef,
        target: ElementRef,
    },
    AddMetadataAnnotation {
        element: ElementRef,
        metadata_type: String,
        properties: BTreeMap<String, String>,
    },
    Remove {
        element: ElementRef,
    },
    RemoveRelationship {
        kind: String,
        source: ElementRef,
        target: ElementRef,
    },
    RenameDeclaration {
        element: ElementRef,
        new_name: String,
    },
    UpdateUsageType {
        element: ElementRef,
        ty: Option<ElementRef>,
    },
    SetExpression {
        element: ElementRef,
        expression: Option<SemanticExpression>,
    },
    UpdateSpecializations {
        element: ElementRef,
        specializes: Vec<ElementRef>,
    },
    MoveDeclaration {
        element: ElementRef,
        destination: ElementRef,
    },
    SetAttribute {
        element: ElementRef,
        attribute: String,
        value: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMutationCapabilityContext {
    pub metamodel_version: String,
    pub supported_operations: Vec<String>,
    #[serde(default)]
    pub variant_capabilities: SemanticVariantCapabilityContext,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_kinds: Vec<String>,
    pub definition_keywords: Vec<String>,
    pub usage_keywords: Vec<String>,
    pub relationship_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usage_typing_rules: Vec<SemanticUsageTypingRuleContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_target_rules: Vec<SemanticRelationshipTargetRuleContext>,
    pub guidance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticUsageTypingRuleContext {
    pub usage_keyword: String,
    pub expected_definition_keyword: String,
    pub expected_definition_kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticRelationshipTargetRuleContext {
    pub relationship_kind: String,
    pub expected_target_kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticReasoningContext {
    pub schema_version: String,
    pub metamodel_version: String,
    pub workspace_revision: WorkspaceRevision,
    pub focus: Vec<ElementRef>,
    pub elements: Vec<SemanticElementContext>,
    pub relationships: Vec<SemanticRelationshipContext>,
    pub facts: Vec<SemanticFactContext>,
    pub affordances: Vec<SemanticAffordanceContext>,
    pub source_files: Vec<String>,
    pub truncated: bool,
    pub usage: AiSemanticContextUsage,
}

pub const AI_SEMANTIC_CONTEXT_SCHEMA_VERSION: &str = "mercurio.ai.semantic_context.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSemanticContextUsage {
    pub authoritative_for_existing_elements: bool,
    pub prefer_ranked_allowed_affordances: bool,
    pub cite_rule_diagnostics: bool,
    pub element_ref_format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticElementContext {
    pub element: ElementRef,
    pub kind: String,
    pub label: String,
    pub owner: Option<ElementRef>,
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticRelationshipContext {
    pub kind: String,
    pub source: ElementRef,
    pub target: ElementRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFactContext {
    pub predicate: String,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticAffordanceContext {
    pub element: ElementRef,
    pub operation: String,
    pub child_kind: String,
    pub status: String,
    pub reason: Option<String>,
}

pub fn semantic_reasoning_context_from_authoring_project(
    project: &AuthoringProject,
    workspace_revision: WorkspaceRevision,
    focus: Vec<ElementRef>,
    max_elements: usize,
) -> SemanticReasoningContext {
    semantic_reasoning_context_from_authoring_project_with_oracle(
        project,
        workspace_revision,
        focus,
        max_elements,
        &crate::semantic_profile::ConservativeSemanticCapabilityOracle,
    )
}

pub fn semantic_reasoning_context_from_authoring_project_with_oracle(
    project: &AuthoringProject,
    workspace_revision: WorkspaceRevision,
    focus: Vec<ElementRef>,
    max_elements: usize,
    oracle: &impl SemanticCapabilityOracle,
) -> SemanticReasoningContext {
    let mut all_elements = Vec::new();
    let mut all_relationships = Vec::new();
    let mut facts = Vec::new();
    let mut source_files = Vec::new();
    let mut truncated = false;

    for (path, module) in project.files() {
        source_files.push(path.to_string());
        collect_module_semantic_context(
            module,
            path,
            None,
            usize::MAX,
            &mut all_elements,
            &mut all_relationships,
            &mut truncated,
            oracle,
        );
        collect_module_semantic_facts(module, None, &mut facts);
    }
    truncated = all_elements.len() > max_elements;
    let (elements, relationships) =
        select_reasoning_context_elements(all_elements, all_relationships, &focus, max_elements);

    SemanticReasoningContext {
        schema_version: AI_SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
        metamodel_version: "model-v2-authoring-context-v1".to_string(),
        workspace_revision,
        focus,
        elements,
        relationships,
        facts,
        affordances: Vec::new(),
        source_files,
        truncated,
        usage: AiSemanticContextUsage {
            authoritative_for_existing_elements: true,
            prefer_ranked_allowed_affordances: true,
            cite_rule_diagnostics: true,
            element_ref_format: "dot_qualified".to_string(),
        },
    }
}

pub fn enrich_semantic_reasoning_context_with_facts(
    context: &mut SemanticReasoningContext,
    facts: impl IntoIterator<Item = SemanticFactContext>,
    max_facts: usize,
) {
    for fact in facts {
        if context.facts.len() >= max_facts {
            context.truncated = true;
            return;
        }
        if !context.facts.contains(&fact) {
            context.facts.push(fact);
        }
    }
}

pub fn enrich_semantic_reasoning_context_with_child_affordances(
    context: &mut SemanticReasoningContext,
    max_affordances: usize,
) {
    let capability_context = default_semantic_mutation_capability_context();
    enrich_semantic_reasoning_context_with_child_affordances_for_capability(
        context,
        max_affordances,
        &capability_context,
    );
}

pub fn enrich_semantic_reasoning_context_with_child_affordances_for_capability(
    context: &mut SemanticReasoningContext,
    max_affordances: usize,
    capability_context: &SemanticMutationCapabilityContext,
) {
    enrich_semantic_reasoning_context_with_child_affordances_for_capability_and_oracle(
        context,
        max_affordances,
        capability_context,
        &ConservativeSemanticCapabilityOracle,
    );
}

pub fn enrich_semantic_reasoning_context_with_child_affordances_for_capability_and_oracle(
    context: &mut SemanticReasoningContext,
    max_affordances: usize,
    capability_context: &SemanticMutationCapabilityContext,
    oracle: &impl SemanticCapabilityOracle,
) {
    let focus = context
        .focus
        .iter()
        .map(|element| element.qualified_name.clone())
        .collect::<BTreeSet<_>>();
    let focused_only = !focus.is_empty();
    let containers = context
        .elements
        .iter()
        .filter(|element| {
            (!focused_only || focus.contains(&element.element.qualified_name))
                && semantic_element_can_own_children(element, capability_context, oracle)
        })
        .map(|element| element.element.clone())
        .collect::<Vec<_>>();

    for element in containers {
        push_child_affordance(
            context,
            max_affordances,
            SemanticAffordanceContext {
                element: element.clone(),
                operation: "AddPackage".to_string(),
                child_kind: "package".to_string(),
                status: "candidate".to_string(),
                reason: Some(
                    "container-like elements can conservatively own nested packages".to_string(),
                ),
            },
        );
        for keyword in &capability_context.definition_keywords {
            push_child_affordance(
                context,
                max_affordances,
                SemanticAffordanceContext {
                    element: element.clone(),
                    operation: "AddDefinition".to_string(),
                    child_kind: keyword.clone(),
                    status: "candidate".to_string(),
                    reason: Some(
                        "candidate from the active language mutation profile; feasibility remains authoritative"
                            .to_string(),
                    ),
                },
            );
        }
        for keyword in &capability_context.usage_keywords {
            push_child_affordance(
                context,
                max_affordances,
                SemanticAffordanceContext {
                    element: element.clone(),
                    operation: "AddUsage".to_string(),
                    child_kind: keyword.clone(),
                    status: "candidate".to_string(),
                    reason: Some(
                        "candidate from the active language mutation profile; feasibility remains authoritative"
                            .to_string(),
                    ),
                },
            );
        }
    }
}

fn push_child_affordance(
    context: &mut SemanticReasoningContext,
    max_affordances: usize,
    affordance: SemanticAffordanceContext,
) {
    if context.affordances.len() >= max_affordances {
        context.truncated = true;
        return;
    }
    context.affordances.push(affordance);
}

fn semantic_element_can_own_children(
    element: &SemanticElementContext,
    capability_context: &SemanticMutationCapabilityContext,
    oracle: &impl SemanticCapabilityOracle,
) -> bool {
    let container_kinds = semantic_element_container_kind_candidates(element);
    if container_kinds.is_empty() {
        return false;
    }

    ["package"]
        .into_iter()
        .chain(
            capability_context
                .definition_keywords
                .iter()
                .map(String::as_str),
        )
        .chain(capability_context.usage_keywords.iter().map(String::as_str))
        .any(|child_kind| {
            container_kinds.iter().any(|container_kind| {
                matches!(
                    oracle.can_contain(container_kind, child_kind),
                    CapabilityAnswer::Allowed
                )
            })
        })
}

fn semantic_element_container_kind_candidates(element: &SemanticElementContext) -> Vec<&str> {
    let mut candidates = Vec::new();
    push_nonempty_candidate(&mut candidates, element.kind.as_str());
    if let Some(kir_kind) = element.attributes.get("kirKind").and_then(Value::as_str) {
        push_nonempty_candidate(&mut candidates, kir_kind);
    }
    if let Some(keyword) = element.attributes.get("keyword").and_then(Value::as_str) {
        push_nonempty_candidate(&mut candidates, keyword);
    }
    candidates
}

fn push_nonempty_candidate<'a>(candidates: &mut Vec<&'a str>, value: &'a str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() && !candidates.contains(&trimmed) {
        candidates.push(trimmed);
    }
}

pub fn enrich_semantic_reasoning_context_with_graph(
    context: &mut SemanticReasoningContext,
    graph: &Graph,
    max_elements: usize,
    max_facts: usize,
) {
    for element in graph.elements() {
        if context.elements.len() >= max_elements {
            context.truncated = true;
            break;
        }
        if context
            .elements
            .iter()
            .any(|item| item.element.qualified_name == element.element_id)
        {
            continue;
        }
        let mut attributes = element.properties.to_btree_map();
        attributes.insert(
            "kirKind".to_string(),
            Value::String(element.kind.to_string()),
        );
        attributes.insert("kirLayer".to_string(), Value::from(element.layer));
        context.elements.push(SemanticElementContext {
            element: ElementRef::new(element.element_id.clone()),
            kind: "kirElement".to_string(),
            label: element
                .properties
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&element.element_id)
                .to_string(),
            owner: element
                .properties
                .get("owner")
                .and_then(Value::as_str)
                .map(ElementRef::new),
            attributes,
        });
    }

    for edge in graph.edges() {
        let Some(source) = graph.element_id(edge.source) else {
            continue;
        };
        let Some(target) = graph.element_id(edge.target) else {
            continue;
        };
        context.relationships.push(SemanticRelationshipContext {
            kind: format!("kir.{}", edge.relation),
            source: ElementRef::new(source),
            target: ElementRef::new(target),
        });
        if context.facts.len() < max_facts {
            context.facts.push(SemanticFactContext {
                predicate: edge.relation.to_string(),
                terms: vec![source.to_string(), target.to_string()],
            });
        } else {
            context.truncated = true;
        }
    }
}

fn collect_module_semantic_context(
    module: &AuthoringModule,
    source_file: &str,
    owner: Option<String>,
    max_elements: usize,
    elements: &mut Vec<SemanticElementContext>,
    relationships: &mut Vec<SemanticRelationshipContext>,
    truncated: &mut bool,
    oracle: &impl SemanticCapabilityOracle,
) {
    if let Some(package) = &module.package {
        let package_name = package.name.as_dot_string();
        push_semantic_element(
            elements,
            max_elements,
            truncated,
            SemanticElementContext {
                element: ElementRef::new(package_name.clone()),
                kind: "package".to_string(),
                label: package_name.clone(),
                owner: owner.as_ref().map(ElementRef::new),
                attributes: context_attributes([
                    ("sourceFile", Value::String(source_file.to_string())),
                    ("memberCount", Value::from(package.members.len())),
                ]),
            },
        );
        for member in &package.members {
            collect_declaration_semantic_context(
                member,
                source_file,
                Some(package_name.clone()),
                max_elements,
                elements,
                relationships,
                truncated,
                oracle,
            );
        }
    }

    for member in &module.members {
        collect_declaration_semantic_context(
            member,
            source_file,
            owner.clone(),
            max_elements,
            elements,
            relationships,
            truncated,
            oracle,
        );
    }
}

fn collect_declaration_semantic_context(
    declaration: &Declaration,
    source_file: &str,
    owner: Option<String>,
    max_elements: usize,
    elements: &mut Vec<SemanticElementContext>,
    relationships: &mut Vec<SemanticRelationshipContext>,
    truncated: &mut bool,
    oracle: &impl SemanticCapabilityOracle,
) {
    match declaration {
        Declaration::Package(package) => {
            let qname = qualify_context_name(owner.as_deref(), &package.name.as_dot_string());
            push_semantic_element(
                elements,
                max_elements,
                truncated,
                SemanticElementContext {
                    element: ElementRef::new(qname.clone()),
                    kind: "package".to_string(),
                    label: package.name.as_dot_string(),
                    owner: owner.as_ref().map(ElementRef::new),
                    attributes: context_attributes([
                        ("sourceFile", Value::String(source_file.to_string())),
                        ("memberCount", Value::from(package.members.len())),
                    ]),
                },
            );
            for member in &package.members {
                collect_declaration_semantic_context(
                    member,
                    source_file,
                    Some(qname.clone()),
                    max_elements,
                    elements,
                    relationships,
                    truncated,
                    oracle,
                );
            }
        }
        Declaration::Definition(definition) => {
            let qname = qualify_context_name(owner.as_deref(), &definition.name);
            let mut attributes = context_attributes([
                ("sourceFile", Value::String(source_file.to_string())),
                ("keyword", Value::String(definition.keyword.clone())),
                ("memberCount", Value::from(definition.members.len())),
            ]);
            insert_doc_context_attributes(&mut attributes, &definition.docs);
            if !definition.specializes.is_empty() {
                attributes.insert(
                    "specializes".to_string(),
                    Value::Array(
                        definition
                            .specializes
                            .iter()
                            .map(|item| Value::String(item.as_dot_string()))
                            .collect(),
                    ),
                );
            }
            push_semantic_element(
                elements,
                max_elements,
                truncated,
                SemanticElementContext {
                    element: ElementRef::new(qname.clone()),
                    kind: "definition".to_string(),
                    label: definition.name.clone(),
                    owner: owner.as_ref().map(ElementRef::new),
                    attributes,
                },
            );
            for target in &definition.specializes {
                relationships.push(SemanticRelationshipContext {
                    kind: "specializes".to_string(),
                    source: ElementRef::new(qname.clone()),
                    target: ElementRef::new(target.as_dot_string()),
                });
            }
            for member in &definition.members {
                collect_declaration_semantic_context(
                    member,
                    source_file,
                    Some(qname.clone()),
                    max_elements,
                    elements,
                    relationships,
                    truncated,
                    oracle,
                );
            }
        }
        Declaration::Usage(usage) => {
            let qname = qualify_context_name(owner.as_deref(), &usage.name);
            let mut attributes = context_attributes([
                ("sourceFile", Value::String(source_file.to_string())),
                ("keyword", Value::String(usage.keyword.clone())),
                ("memberCount", Value::from(usage.members.len())),
            ]);
            insert_doc_context_attributes(&mut attributes, &usage.docs);
            if let Some(ty) = &usage.ty {
                attributes.insert("type".to_string(), Value::String(ty.as_dot_string()));
                relationships.push(SemanticRelationshipContext {
                    kind: "typedBy".to_string(),
                    source: ElementRef::new(qname.clone()),
                    target: ElementRef::new(ty.as_dot_string()),
                });
            }
            if let Some(target) = &usage.reference_target {
                attributes.insert(
                    "referenceTarget".to_string(),
                    Value::String(target.as_dot_string()),
                );
                relationships.push(SemanticRelationshipContext {
                    kind: usage.keyword.clone(),
                    source: ElementRef::new(qname.clone()),
                    target: ElementRef::new(target.as_dot_string()),
                });
                if oracle.relationship_uses_owner_as_source(&usage.keyword) {
                    if let Some(owner) = &owner {
                        relationships.push(SemanticRelationshipContext {
                            kind: usage.keyword.clone(),
                            source: ElementRef::new(owner.clone()),
                            target: ElementRef::new(target.as_dot_string()),
                        });
                    }
                }
            } else if oracle.relationship_uses_owner_as_source(&usage.keyword) {
                if let Some(owner) = &owner {
                    relationships.push(SemanticRelationshipContext {
                        kind: usage.keyword.clone(),
                        source: ElementRef::new(owner.clone()),
                        target: ElementRef::new(usage.name.clone()),
                    });
                }
            }
            if let Some(expression) = &usage.expression {
                attributes.insert("expression".to_string(), Value::String(expression.clone()));
            }
            push_semantic_element(
                elements,
                max_elements,
                truncated,
                SemanticElementContext {
                    element: ElementRef::new(qname.clone()),
                    kind: "usage".to_string(),
                    label: usage.name.clone(),
                    owner: owner.as_ref().map(ElementRef::new),
                    attributes,
                },
            );
            for member in &usage.members {
                collect_declaration_semantic_context(
                    member,
                    source_file,
                    Some(qname.clone()),
                    max_elements,
                    elements,
                    relationships,
                    truncated,
                    oracle,
                );
            }
        }
        Declaration::Import(import) => {
            if let Some(owner) = &owner {
                relationships.push(SemanticRelationshipContext {
                    kind: "imports".to_string(),
                    source: ElementRef::new(owner.clone()),
                    target: ElementRef::new(import.path.as_dot_string()),
                });
            }
        }
        Declaration::Alias(alias) => {
            let qname = qualify_context_name(owner.as_deref(), &alias.name);
            push_semantic_element(
                elements,
                max_elements,
                truncated,
                SemanticElementContext {
                    element: ElementRef::new(qname.clone()),
                    kind: "alias".to_string(),
                    label: alias.name.clone(),
                    owner: owner.as_ref().map(ElementRef::new),
                    attributes: context_attributes([
                        ("sourceFile", Value::String(source_file.to_string())),
                        ("target", Value::String(alias.target.as_dot_string())),
                    ]),
                },
            );
            relationships.push(SemanticRelationshipContext {
                kind: "aliases".to_string(),
                source: ElementRef::new(qname),
                target: ElementRef::new(alias.target.as_dot_string()),
            });
        }
    }
}

fn push_semantic_element(
    elements: &mut Vec<SemanticElementContext>,
    max_elements: usize,
    truncated: &mut bool,
    element: SemanticElementContext,
) {
    if elements.len() >= max_elements {
        *truncated = true;
        return;
    }
    elements.push(element);
}

fn collect_module_semantic_facts(
    module: &AuthoringModule,
    owner: Option<String>,
    facts: &mut Vec<SemanticFactContext>,
) {
    if let Some(package) = &module.package {
        let package_name = qualify_context_name(owner.as_deref(), &package.name.as_dot_string());
        push_fact(facts, "package", [package_name.clone()]);
        push_fact(
            facts,
            "name",
            [package_name.clone(), package.name.as_dot_string()],
        );
        if let Some(owner) = &owner {
            push_fact(facts, "owns", [owner.clone(), package_name.clone()]);
        } else {
            push_fact(facts, "top_level_package", [package_name.clone()]);
        }
        for member in &package.members {
            collect_declaration_semantic_facts(member, Some(package_name.clone()), facts);
        }
    }
    for member in &module.members {
        collect_declaration_semantic_facts(member, owner.clone(), facts);
    }
}

fn collect_declaration_semantic_facts(
    declaration: &Declaration,
    owner: Option<String>,
    facts: &mut Vec<SemanticFactContext>,
) {
    match declaration {
        Declaration::Package(package) => {
            let id = qualify_context_name(owner.as_deref(), &package.name.as_dot_string());
            push_fact(facts, "package", [id.clone()]);
            push_fact(facts, "name", [id.clone(), package.name.as_dot_string()]);
            if let Some(owner) = &owner {
                push_fact(facts, "owns", [owner.clone(), id.clone()]);
            } else {
                push_fact(facts, "top_level_package", [id.clone()]);
            }
            for member in &package.members {
                collect_declaration_semantic_facts(member, Some(id.clone()), facts);
            }
        }
        Declaration::Definition(definition) => {
            let id = qualify_context_name(owner.as_deref(), &definition.name);
            push_fact(facts, "definition", [id.clone()]);
            push_fact(
                facts,
                "definition_keyword",
                [id.clone(), definition.keyword.clone()],
            );
            push_fact(facts, "name", [id.clone(), definition.name.clone()]);
            if let Some(owner) = &owner {
                push_fact(facts, "owns", [owner.clone(), id.clone()]);
            }
            for target in &definition.specializes {
                push_fact(facts, "specializes", [id.clone(), target.as_dot_string()]);
            }
            for member in &definition.members {
                collect_declaration_semantic_facts(member, Some(id.clone()), facts);
            }
        }
        Declaration::Usage(usage) => {
            let id = qualify_context_name(owner.as_deref(), &usage.name);
            push_fact(facts, "usage", [id.clone()]);
            push_fact(facts, "usage_keyword", [id.clone(), usage.keyword.clone()]);
            push_fact(facts, "name", [id.clone(), usage.name.clone()]);
            if let Some(owner) = &owner {
                push_fact(facts, "owns", [owner.clone(), id.clone()]);
            }
            for modifier in &usage.modifiers {
                push_fact(facts, "modifier", [id.clone(), modifier.clone()]);
            }
            if matches!(
                usage.keyword.as_str(),
                "connect" | "connection" | "interface"
            ) {
                push_fact(facts, "connection_usage", [id.clone()]);
            }
            if usage.keyword == "interface" {
                push_fact(facts, "interface_usage", [id.clone()]);
            }
            if let Some(ty) = &usage.ty {
                push_fact(facts, "type", [id.clone(), ty.as_dot_string()]);
            }
            if let Some(target) = &usage.reference_target {
                push_fact(
                    facts,
                    "reference_target",
                    [id.clone(), target.as_dot_string()],
                );
            }
            for target in &usage.specializes {
                push_fact(facts, "specializes", [id.clone(), target.as_dot_string()]);
            }
            for member in &usage.members {
                collect_declaration_semantic_facts(member, Some(id.clone()), facts);
            }
        }
        Declaration::Import(import) => {
            if let Some(owner) = &owner {
                push_fact(
                    facts,
                    "imports",
                    [owner.clone(), import.path.as_dot_string()],
                );
            }
        }
        Declaration::Alias(alias) => {
            let id = qualify_context_name(owner.as_deref(), &alias.name);
            push_fact(facts, "alias", [id.clone()]);
            push_fact(facts, "aliases", [id.clone(), alias.target.as_dot_string()]);
            if let Some(owner) = &owner {
                push_fact(facts, "owns", [owner.clone(), id]);
            }
        }
    }
}

fn push_fact<const N: usize>(
    facts: &mut Vec<SemanticFactContext>,
    predicate: &str,
    terms: [String; N],
) {
    facts.push(SemanticFactContext {
        predicate: predicate.to_string(),
        terms: terms.into_iter().collect(),
    });
}

fn select_reasoning_context_elements(
    mut elements: Vec<SemanticElementContext>,
    relationships: Vec<SemanticRelationshipContext>,
    focus: &[ElementRef],
    max_elements: usize,
) -> (
    Vec<SemanticElementContext>,
    Vec<SemanticRelationshipContext>,
) {
    if max_elements == 0 {
        return (Vec::new(), Vec::new());
    }
    if !focus.is_empty() {
        let focus_names = focus
            .iter()
            .map(|item| item.qualified_name.clone())
            .collect::<BTreeSet<_>>();
        elements.sort_by_key(|element| focus_selection_rank(element, &relationships, &focus_names));
    }
    elements.truncate(max_elements);
    let selected = elements
        .iter()
        .map(|element| element.element.qualified_name.clone())
        .collect::<BTreeSet<_>>();
    let relationships = relationships
        .into_iter()
        .filter(|relationship| {
            selected.contains(&relationship.source.qualified_name)
                || selected.contains(&relationship.target.qualified_name)
        })
        .collect();
    (elements, relationships)
}

fn focus_selection_rank(
    element: &SemanticElementContext,
    relationships: &[SemanticRelationshipContext],
    focus_names: &BTreeSet<String>,
) -> (usize, String) {
    let name = &element.element.qualified_name;
    if focus_names.contains(name) {
        return (0, name.clone());
    }
    if element
        .owner
        .as_ref()
        .is_some_and(|owner| focus_names.contains(&owner.qualified_name))
        || focus_names
            .iter()
            .any(|focus| name.starts_with(&format!("{focus}.")))
    {
        return (1, name.clone());
    }
    if relationships.iter().any(|relationship| {
        (relationship.source.qualified_name == *name
            && focus_names.contains(&relationship.target.qualified_name))
            || (relationship.target.qualified_name == *name
                && focus_names.contains(&relationship.source.qualified_name))
    }) {
        return (2, name.clone());
    }
    if focus_names
        .iter()
        .any(|focus| focus.starts_with(&format!("{name}.")))
    {
        return (3, name.clone());
    }
    (10, name.clone())
}

fn context_attributes(
    attributes: impl IntoIterator<Item = (&'static str, Value)>,
) -> BTreeMap<String, Value> {
    attributes
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn insert_doc_context_attributes(attributes: &mut BTreeMap<String, Value>, docs: &[String]) {
    if docs.is_empty() {
        return;
    }
    attributes.insert(
        "docs".to_string(),
        Value::Array(docs.iter().map(|doc| Value::String(doc.clone())).collect()),
    );
    if let Some(id) = id_from_docs(docs) {
        attributes.insert("id".to_string(), Value::String(id));
    }
    if let Some(text) = text_from_docs(docs) {
        attributes.insert("text".to_string(), Value::String(text));
    }
}

fn id_from_docs(docs: &[String]) -> Option<String> {
    docs.iter().find_map(|doc| {
        let trimmed = doc.trim();
        trimmed
            .strip_prefix("id:")
            .or_else(|| trimmed.strip_prefix("ID:"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn text_from_docs(docs: &[String]) -> Option<String> {
    docs.iter()
        .find(|doc| !is_id_doc(doc) && !doc.trim().is_empty())
        .cloned()
}

fn is_id_doc(doc: &str) -> bool {
    let trimmed = doc.trim();
    trimmed.starts_with("id:") || trimmed.starts_with("ID:")
}

fn qualify_context_name(owner: Option<&str>, name: &str) -> String {
    if name.contains('.') || name.contains("::") {
        return QualifiedName::parse(name).as_dot_string();
    }
    owner
        .filter(|owner| !owner.is_empty())
        .map(|owner| format!("{owner}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

pub fn default_semantic_mutation_capability_context() -> SemanticMutationCapabilityContext {
    SemanticMutationCapabilityContext {
        metamodel_version: "language-neutral-mutation-v1".to_string(),
        supported_operations: vec![
            "AddPackage".to_string(),
            "AddElement".to_string(),
            "AddDefinition".to_string(),
            "AddUsage".to_string(),
            "AddRelationship".to_string(),
            "AddMetadataAnnotation".to_string(),
            "Remove".to_string(),
            "RemoveRelationship".to_string(),
            "RenameDeclaration".to_string(),
            "UpdateUsageType".to_string(),
            "SetExpression".to_string(),
            "UpdateSpecializations".to_string(),
            "MoveDeclaration".to_string(),
            "SetAttribute".to_string(),
        ],
        variant_capabilities: default_semantic_variant_capability_context(),
        element_kinds: Vec::new(),
        definition_keywords: Vec::new(),
        usage_keywords: Vec::new(),
        relationship_kinds: Vec::new(),
        usage_typing_rules: Vec::new(),
        relationship_target_rules: Vec::new(),
        guidance: vec![
            "Use a language-specific mutation profile for concrete keywords and relationships."
                .to_string(),
            "Return semantic mutations, not source text edits.".to_string(),
            "Core feasibility remains authoritative for contextual legality.".to_string(),
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SemanticExpression {
    Text(String),
}

impl SemanticExpression {
    pub fn as_text(&self) -> &str {
        match self {
            Self::Text(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationApplicationResult {
    pub changed_files: BTreeSet<String>,
    pub edited_files: BTreeMap<String, String>,
    pub changed_declarations: BTreeSet<String>,
    pub semantic_diff: SemanticDiff,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_back_mode: Option<WriteBackMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SemanticDiff {
    pub added_elements: Vec<SemanticDiffElementRef>,
    pub removed_elements: Vec<SemanticDiffElementRef>,
    pub renamed_elements: Vec<RenamedElement>,
    pub moved_elements: Vec<MovedElement>,
    pub retyped_usages: Vec<RetypedUsage>,
    pub changed_specializations: Vec<ChangedSpecialization>,
    pub changed_attributes: Vec<ChangedAttribute>,
    pub added_relationships: Vec<RelationshipChange>,
    pub removed_relationships: Vec<RelationshipChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChangeProvenance {
    pub mutation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelChangeEvent {
    pub sequence: u64,
    pub revision_before: WorkspaceRevision,
    pub revision_after: WorkspaceRevision,
    pub provenance: ModelChangeProvenance,
    pub diff: SemanticDiff,
}

impl ModelChangeEvent {
    pub fn new(
        revision_before: WorkspaceRevision,
        revision_after: WorkspaceRevision,
        provenance: ModelChangeProvenance,
        diff: SemanticDiff,
    ) -> Self {
        Self {
            sequence: 0,
            revision_before,
            revision_after,
            provenance,
            diff,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamedElement {
    pub element: SemanticDiffElementRef,
    pub before_name: Option<String>,
    pub after_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovedElement {
    pub element: SemanticDiffElementRef,
    pub before_owner: Option<SemanticDiffElementRef>,
    pub after_owner: Option<SemanticDiffElementRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetypedUsage {
    pub element: SemanticDiffElementRef,
    pub before_type: Option<SemanticDiffElementRef>,
    pub after_type: Option<SemanticDiffElementRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSpecialization {
    pub element: SemanticDiffElementRef,
    pub before_specializes: Vec<SemanticDiffElementRef>,
    pub after_specializes: Vec<SemanticDiffElementRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangedAttribute {
    pub element: SemanticDiffElementRef,
    pub attribute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationshipChange {
    pub kind: String,
    pub source: SemanticDiffElementRef,
    pub target: SemanticDiffElementRef,
}

pub type SemanticDiffElementRef = SemanticElementRef;

pub fn mutation_proposal_digest(proposal: &MutationProposal) -> String {
    stable_json_digest("semantic-mutation-proposal", proposal)
}

pub fn mutation_application_digest(application: &MutationApplicationResult) -> String {
    let payload = serde_json::json!({
        "changed_files": &application.changed_files,
        "edited_files": &application.edited_files,
        "changed_declarations": &application.changed_declarations,
        "semantic_diff": &application.semantic_diff,
    });
    stable_json_digest("semantic-mutation-application", &payload)
}

fn stable_json_digest<T>(tag: &'static str, value: &T) -> String
where
    T: Serialize,
{
    match serde_json::to_vec(value) {
        Ok(bytes) => stable_digest([(tag.as_bytes(), bytes.as_slice())]),
        Err(err) => {
            let message = err.to_string();
            stable_digest([
                (tag.as_bytes(), "serialization-error".as_bytes()),
                ("message".as_bytes(), message.as_bytes()),
            ])
        }
    }
}

pub fn diff_kir_documents(before: &KirDocument, after: &KirDocument) -> SemanticDiff {
    let mut diff = SemanticDiff::default();
    let before_elements = before
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<BTreeMap<_, _>>();
    let after_elements = after
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<BTreeMap<_, _>>();

    for id in before_elements.keys() {
        if !after_elements.contains_key(id) {
            if let Some(element) = before_elements.get(id) {
                diff.removed_elements
                    .push(SemanticDiffElementRef::from_element(element));
            }
        }
    }
    for (id, after_element) in &after_elements {
        let Some(before_element) = before_elements.get(id) else {
            diff.added_elements
                .push(SemanticDiffElementRef::from_element(after_element));
            continue;
        };
        collect_element_property_diff(&mut diff, before_element, after_element);
    }

    collect_relationship_diff(before, after, &mut diff);
    diff
}

fn collect_element_property_diff(diff: &mut SemanticDiff, before: &KirElement, after: &KirElement) {
    let element = SemanticDiffElementRef::from_element(after);
    if before.kind != after.kind {
        diff.changed_attributes.push(ChangedAttribute {
            element: element.clone(),
            attribute: "kind".to_string(),
            before: Some(Value::String(before.kind.clone())),
            after: Some(Value::String(after.kind.clone())),
        });
    }
    if before.layer != after.layer {
        diff.changed_attributes.push(ChangedAttribute {
            element: element.clone(),
            attribute: "layer".to_string(),
            before: Some(Value::from(before.layer)),
            after: Some(Value::from(after.layer)),
        });
    }

    collect_renamed_element(diff, before, after, &element);
    collect_moved_element(diff, before, after, &element);
    collect_retyped_usage(diff, before, after, &element);
    collect_changed_specializations(diff, before, after, &element);

    let property_names = before
        .properties
        .keys()
        .chain(after.properties.keys())
        .collect::<BTreeSet<_>>();
    for name in property_names {
        if classified_property(name) {
            continue;
        }
        if before.properties.get(name) != after.properties.get(name) {
            diff.changed_attributes.push(ChangedAttribute {
                element: element.clone(),
                attribute: name.clone(),
                before: before.properties.get(name).cloned(),
                after: after.properties.get(name).cloned(),
            });
        }
    }
}

fn collect_renamed_element(
    diff: &mut SemanticDiff,
    before: &KirElement,
    after: &KirElement,
    element: &SemanticDiffElementRef,
) {
    let before_name = element_name(before);
    let after_name = element_name(after);
    if before_name != after_name {
        diff.renamed_elements.push(RenamedElement {
            element: element.clone(),
            before_name,
            after_name,
        });
    }
}

fn collect_moved_element(
    diff: &mut SemanticDiff,
    before: &KirElement,
    after: &KirElement,
    element: &SemanticDiffElementRef,
) {
    let before_owner = owner_ref(before);
    let after_owner = owner_ref(after);
    if before_owner != after_owner {
        diff.moved_elements.push(MovedElement {
            element: element.clone(),
            before_owner,
            after_owner,
        });
    }
}

fn collect_retyped_usage(
    diff: &mut SemanticDiff,
    before: &KirElement,
    after: &KirElement,
    element: &SemanticDiffElementRef,
) {
    let before_type = type_ref(before);
    let after_type = type_ref(after);
    if before_type != after_type {
        diff.retyped_usages.push(RetypedUsage {
            element: element.clone(),
            before_type,
            after_type,
        });
    }
}

fn collect_changed_specializations(
    diff: &mut SemanticDiff,
    before: &KirElement,
    after: &KirElement,
    element: &SemanticDiffElementRef,
) {
    let before_specializes = specialization_refs(before);
    let after_specializes = specialization_refs(after);
    if before_specializes != after_specializes {
        diff.changed_specializations.push(ChangedSpecialization {
            element: element.clone(),
            before_specializes,
            after_specializes,
        });
    }
}

fn collect_relationship_diff(before: &KirDocument, after: &KirDocument, diff: &mut SemanticDiff) {
    let before_relationships = document_relationships(before);
    let after_relationships = document_relationships(after);

    for relationship in before_relationships.difference(&after_relationships) {
        diff.removed_relationships.push(relationship.clone());
    }
    for relationship in after_relationships.difference(&before_relationships) {
        diff.added_relationships.push(relationship.clone());
    }
}

fn document_relationships(document: &KirDocument) -> BTreeSet<RelationshipChange> {
    let Ok(graph) = Graph::from_document(document.clone()) else {
        return BTreeSet::new();
    };
    graph
        .edges()
        .iter()
        .filter_map(|edge| {
            let source_element = graph.element(edge.source)?;
            let target_element = graph.element(edge.target)?;
            Some(RelationshipChange {
                kind: edge.relation.to_string(),
                source: SemanticDiffElementRef::from_graph_element(source_element),
                target: SemanticDiffElementRef::from_graph_element(target_element),
            })
        })
        .collect()
}

pub(crate) fn diff_for_operation(
    operation: &SemanticMutation,
    result: Option<&MutationResult>,
) -> SemanticDiff {
    let mut diff = SemanticDiff::default();
    match operation {
        SemanticMutation::AddPackage { name, .. } => {
            diff.added_elements
                .push(SemanticDiffElementRef::unresolved(name.clone()));
        }
        SemanticMutation::AddElement {
            container, name, ..
        }
        | SemanticMutation::AddDefinition {
            container, name, ..
        }
        | SemanticMutation::AddUsage {
            container, name, ..
        } => {
            diff.added_elements
                .push(SemanticDiffElementRef::unresolved(format!(
                    "{}.{name}",
                    container.qualified_name
                )));
        }
        SemanticMutation::AddRelationship {
            kind,
            source,
            target,
        } => diff.added_relationships.push(RelationshipChange {
            kind: kind.clone(),
            source: diff_ref_from_element_ref(source),
            target: diff_ref_from_element_ref(target),
        }),
        SemanticMutation::AddMetadataAnnotation { element, .. } => {
            diff.changed_attributes.push(ChangedAttribute {
                element: diff_ref_from_element_ref(element),
                attribute: "metadata".to_string(),
                before: None,
                after: None,
            });
        }
        SemanticMutation::Remove { element } => {
            diff.removed_elements
                .push(diff_ref_from_element_ref(element));
        }
        SemanticMutation::RemoveRelationship {
            kind,
            source,
            target,
        } => diff.removed_relationships.push(RelationshipChange {
            kind: kind.clone(),
            source: diff_ref_from_element_ref(source),
            target: diff_ref_from_element_ref(target),
        }),
        SemanticMutation::RenameDeclaration { element, new_name } => {
            diff.renamed_elements.push(RenamedElement {
                element: diff_ref_from_element_ref(element),
                before_name: element
                    .qualified_name
                    .rsplit_once('.')
                    .map(|(_, name)| name.to_string())
                    .or_else(|| Some(element.qualified_name.clone())),
                after_name: Some(new_name.clone()),
            });
        }
        SemanticMutation::UpdateUsageType { element, ty } => {
            diff.retyped_usages.push(RetypedUsage {
                element: diff_ref_from_element_ref(element),
                before_type: None,
                after_type: ty.as_ref().map(diff_ref_from_element_ref),
            });
        }
        SemanticMutation::SetExpression { element, .. } => {
            diff.changed_attributes.push(ChangedAttribute {
                element: diff_ref_from_element_ref(element),
                attribute: "expression".to_string(),
                before: None,
                after: None,
            });
        }
        SemanticMutation::UpdateSpecializations {
            element,
            specializes,
        } => diff.changed_specializations.push(ChangedSpecialization {
            element: diff_ref_from_element_ref(element),
            before_specializes: Vec::new(),
            after_specializes: specializes.iter().map(diff_ref_from_element_ref).collect(),
        }),
        SemanticMutation::MoveDeclaration {
            element,
            destination,
        } => diff.moved_elements.push(MovedElement {
            element: diff_ref_from_element_ref(element),
            before_owner: None,
            after_owner: Some(diff_ref_from_element_ref(destination)),
        }),
        SemanticMutation::SetAttribute {
            element, attribute, ..
        } => diff.changed_attributes.push(ChangedAttribute {
            element: diff_ref_from_element_ref(element),
            attribute: attribute.clone(),
            before: None,
            after: None,
        }),
    }

    if let Some(result) = result {
        for declaration in &result.changed_declarations {
            let element = SemanticDiffElementRef::unresolved(declaration.clone());
            if !diff.added_elements.contains(&element)
                && !diff
                    .changed_attributes
                    .iter()
                    .any(|item| item.element == element)
                && !diff
                    .changed_specializations
                    .iter()
                    .any(|item| item.element == element)
                && !diff
                    .retyped_usages
                    .iter()
                    .any(|item| item.element == element)
            {
                diff.changed_attributes.push(ChangedAttribute {
                    element,
                    attribute: "declaration".to_string(),
                    before: None,
                    after: None,
                });
            }
        }
    }

    diff
}

fn diff_ref_from_element_ref(element: &ElementRef) -> SemanticDiffElementRef {
    SemanticDiffElementRef::unresolved(element.qualified_name.clone()).with_label_optional(
        element
            .qualified_name
            .rsplit('.')
            .next()
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    )
}

fn element_name(element: &KirElement) -> Option<String> {
    string_property(&element.properties, KIR_PROP_NAME)
        .or_else(|| string_property(&element.properties, "name"))
        .or_else(|| string_property(&element.properties, "qualified_name"))
}

fn owner_ref(element: &KirElement) -> Option<SemanticDiffElementRef> {
    for key in [
        KIR_PROP_OWNER,
        "owning_namespace",
        "owning_type",
        "owning_definition",
        "featuring_type",
    ] {
        if let Some(owner) = element.properties.get(key).and_then(Value::as_str) {
            return Some(SemanticDiffElementRef::unresolved(owner));
        }
    }
    None
}

fn type_ref(element: &KirElement) -> Option<SemanticDiffElementRef> {
    for key in [KIR_PROP_TYPE, "definition", "feature_typings"] {
        if let Some(value) = element.properties.get(key) {
            if let Some(reference) = first_reference_value(value) {
                return Some(SemanticDiffElementRef::unresolved(reference));
            }
        }
    }
    None
}

fn specialization_refs(element: &KirElement) -> Vec<SemanticDiffElementRef> {
    [
        KIR_PROP_SPECIALIZES,
        "specialized_features",
        "subsets",
        "redefines",
    ]
    .into_iter()
    .filter_map(|key| element.properties.get(key))
    .flat_map(reference_values)
    .map(SemanticDiffElementRef::unresolved)
    .collect()
}

fn first_reference_value(value: &Value) -> Option<String> {
    reference_values(value).into_iter().next()
}

fn reference_values(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => values.iter().flat_map(reference_values).collect::<Vec<_>>(),
        Value::Object(object) => object
            .get("id")
            .or_else(|| object.get("element_id"))
            .or_else(|| object.get("qualified_name"))
            .or_else(|| object.get("ref"))
            .and_then(Value::as_str)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn string_property(properties: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn graph_string_property(element: &Element, key: &str) -> Option<String> {
    element
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn graph_element_label(element: &Element) -> Option<String> {
    graph_string_property(element, "declared_name")
        .or_else(|| graph_string_property(element, "name"))
        .or_else(|| graph_string_property(element, "qualified_name"))
        .or_else(|| {
            element
                .element_id
                .rsplit('.')
                .next()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn kir_element_label(element: &KirElement) -> Option<String> {
    string_property(&element.properties, "declared_name")
        .or_else(|| string_property(&element.properties, "name"))
        .or_else(|| string_property(&element.properties, "qualified_name"))
        .or_else(|| {
            element
                .id
                .rsplit('.')
                .next()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn classified_property(name: &str) -> bool {
    matches!(
        name,
        KIR_PROP_NAME
            | "name"
            | "qualified_name"
            | KIR_PROP_OWNER
            | "owning_namespace"
            | "owning_type"
            | "owning_definition"
            | "featuring_type"
            | KIR_PROP_TYPE
            | "definition"
            | "feature_typings"
            | KIR_PROP_SPECIALIZES
            | "specialized_features"
            | "subsets"
            | "redefines"
    )
}

pub(crate) fn merge_diff(target: &mut SemanticDiff, source: SemanticDiff) {
    target.added_elements.extend(source.added_elements);
    target.removed_elements.extend(source.removed_elements);
    target.renamed_elements.extend(source.renamed_elements);
    target.moved_elements.extend(source.moved_elements);
    target.retyped_usages.extend(source.retyped_usages);
    target
        .changed_specializations
        .extend(source.changed_specializations);
    target.changed_attributes.extend(source.changed_attributes);
    target
        .added_relationships
        .extend(source.added_relationships);
    target
        .removed_relationships
        .extend(source.removed_relationships);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ElementRef, MutationApplicationResult, MutationProposal, WorkspaceRevision,
        default_semantic_mutation_capability_context, diff_kir_documents,
        enrich_semantic_reasoning_context_with_child_affordances,
        enrich_semantic_reasoning_context_with_graph, mutation_application_digest,
        mutation_proposal_digest, semantic_reasoning_context_from_authoring_project,
    };
    use mercurio_authoring::authoring::AuthoringProject;
    use mercurio_kir::{KirDocument, KirElement};
    use mercurio_model::Graph;

    #[test]
    fn default_capability_context_is_language_neutral() {
        let context = default_semantic_mutation_capability_context();

        assert_eq!(context.metamodel_version, "language-neutral-mutation-v1");
        assert!(
            context
                .supported_operations
                .contains(&"AddDefinition".to_string())
        );
        assert!(context.definition_keywords.is_empty());
        assert!(context.usage_keywords.is_empty());
        assert!(context.relationship_kinds.is_empty());
        assert_eq!(
            context.variant_capabilities.schema_version,
            crate::SEMANTIC_VARIANT_SCHEMA_VERSION
        );
        assert!(
            context
                .variant_capabilities
                .supported_operations
                .contains(&"PreviewVariant".to_string())
        );
        assert!(
            context
                .guidance
                .iter()
                .any(|item| item.contains("language-specific mutation profile"))
        );
    }

    #[test]
    fn mutation_application_telemetry_is_additive() {
        let legacy = serde_json::json!({
            "changed_files": ["model.sysml"],
            "edited_files": {"model.sysml": "package Demo;"},
            "changed_declarations": ["Demo"],
            "semantic_diff": {
                "added_elements": [],
                "removed_elements": [],
                "renamed_elements": [],
                "moved_elements": [],
                "retyped_usages": [],
                "changed_specializations": [],
                "changed_attributes": [],
                "added_relationships": [],
                "removed_relationships": []
            }
        });
        let application: MutationApplicationResult =
            serde_json::from_value(legacy).expect("legacy application result should deserialize");

        assert_eq!(application.proposed_digest, None);
        assert_eq!(application.applied_digest, None);
        assert_eq!(application.decided_at, None);

        let proposal = MutationProposal {
            intent: "Add demo package".to_string(),
            operations: Vec::new(),
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: WorkspaceRevision::unchecked(),
        };
        let proposed_digest = mutation_proposal_digest(&proposal);
        assert_eq!(proposed_digest, mutation_proposal_digest(&proposal));

        let mut stamped = application.clone();
        stamped.proposed_digest = Some(proposed_digest);
        stamped.applied_digest = Some(mutation_application_digest(&application));
        stamped.decided_at = Some(42);
        stamped.supersedes = Some("proposal:previous".to_string());
        stamped.actor = Some("human".to_string());

        let value = serde_json::to_value(&stamped).expect("application result should serialize");
        assert_eq!(value["decided_at"], 42);
        assert_eq!(value["supersedes"], "proposal:previous");
        assert_eq!(value["actor"], "human");
        assert_ne!(value["proposed_digest"], value["applied_digest"]);
    }

    #[test]
    fn semantic_diff_compares_kir_documents() {
        let before = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "members".to_string(),
                        serde_json::json!(["req.startup"]),
                    )]),
                },
                KirElement {
                    id: "req.startup".to_string(),
                    kind: "RequirementUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        };
        let after = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "members".to_string(),
                        serde_json::json!(["req.startup", "case.verifyStartup"]),
                    )]),
                },
                KirElement {
                    id: "req.startup".to_string(),
                    kind: "RequirementUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "metadata".to_string(),
                        serde_json::json!([{"type": "ReviewTag"}]),
                    )]),
                },
                KirElement {
                    id: "case.verifyStartup".to_string(),
                    kind: "VerificationCaseUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        };

        let diff = diff_kir_documents(&before, &after);

        assert!(
            diff.added_elements
                .iter()
                .any(|element| element.element_id == "case.verifyStartup")
        );
        assert!(diff.changed_attributes.iter().any(|change| {
            change.element.element_id == "req.startup"
                && change.attribute == "metadata"
                && change.before.is_none()
                && change.after == Some(serde_json::json!([{"type": "ReviewTag"}]))
        }));
        assert!(diff.added_relationships.iter().any(|relationship| {
            relationship.kind == "members"
                && relationship.source.element_id == "pkg.Demo"
                && relationship.target.element_id == "case.verifyStartup"
        }));
    }

    #[test]
    fn semantic_diff_classifies_stable_id_changes() {
        let before = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "part.Vehicle".to_string(),
                    kind: "PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), serde_json::json!("vehicle")),
                        ("owner".to_string(), serde_json::json!("pkg.Demo")),
                        ("type".to_string(), serde_json::json!("def.Vehicle")),
                        ("specializes".to_string(), serde_json::json!(["def.Asset"])),
                        ("mass".to_string(), serde_json::json!(1000)),
                    ]),
                },
                KirElement {
                    id: "def.Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "def.Asset".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        };
        let after = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "pkg.Other".to_string(),
                    kind: "Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "part.Vehicle".to_string(),
                    kind: "PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), serde_json::json!("car")),
                        ("owner".to_string(), serde_json::json!("pkg.Other")),
                        ("type".to_string(), serde_json::json!("def.Car")),
                        (
                            "specializes".to_string(),
                            serde_json::json!(["def.MobileAsset"]),
                        ),
                        ("mass".to_string(), serde_json::json!(950)),
                    ]),
                },
                KirElement {
                    id: "def.Car".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "def.MobileAsset".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        };

        let diff = diff_kir_documents(&before, &after);

        assert!(
            diff.added_elements
                .iter()
                .any(|element| element.element_id == "pkg.Other")
        );
        assert!(
            diff.removed_elements
                .iter()
                .any(|element| element.element_id == "def.Vehicle")
        );
        assert!(diff.renamed_elements.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change.before_name.as_deref() == Some("vehicle")
                && change.after_name.as_deref() == Some("car")
        }));
        assert!(diff.moved_elements.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change
                    .before_owner
                    .as_ref()
                    .is_some_and(|owner| owner.element_id == "pkg.Demo")
                && change
                    .after_owner
                    .as_ref()
                    .is_some_and(|owner| owner.element_id == "pkg.Other")
        }));
        assert!(diff.retyped_usages.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change
                    .before_type
                    .as_ref()
                    .is_some_and(|ty| ty.element_id == "def.Vehicle")
                && change
                    .after_type
                    .as_ref()
                    .is_some_and(|ty| ty.element_id == "def.Car")
        }));
        assert!(diff.changed_specializations.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change.before_specializes[0].element_id == "def.Asset"
                && change.after_specializes[0].element_id == "def.MobileAsset"
        }));
        assert!(diff.changed_attributes.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change.attribute == "mass"
                && change.before == Some(serde_json::json!(1000))
                && change.after == Some(serde_json::json!(950))
        }));
    }

    #[test]
    fn semantic_reasoning_context_summarizes_authoring_project() {
        let files = BTreeMap::from([(
            "hybrid.model".to_string(),
            r#"
package HybridVehicle {
    part HybridVehicle {
        part battery : BatteryPack;
        attribute efficiency;
    }

    part BatteryPack;
}
"#
            .to_string(),
        )]);
        let project = AuthoringProject::from_model_files(files).expect("project parses");

        let context = semantic_reasoning_context_from_authoring_project(
            &project,
            WorkspaceRevision::unchecked(),
            vec![ElementRef::new("HybridVehicle.HybridVehicle")],
            64,
        );

        assert_eq!(context.metamodel_version, "model-v2-authoring-context-v1");
        assert_eq!(context.source_files, vec!["hybrid.model".to_string()]);
        assert!(!context.truncated);
        assert!(
            context
                .elements
                .iter()
                .any(|item| item.element.qualified_name == "HybridVehicle.HybridVehicle")
        );
        assert!(context.elements.iter().any(|item| {
            item.element.qualified_name == "HybridVehicle.HybridVehicle.battery"
                && item.attributes.get("type").and_then(|value| value.as_str())
                    == Some("BatteryPack")
        }));
        assert!(context.relationships.iter().any(|relationship| {
            relationship.kind == "typedBy"
                && relationship.source.qualified_name == "HybridVehicle.HybridVehicle.battery"
                && relationship.target.qualified_name == "BatteryPack"
        }));
        assert!(context.facts.iter().any(|fact| {
            fact.predicate == "usage" && fact.terms == vec!["HybridVehicle.BatteryPack".to_string()]
        }));
        assert!(context.facts.iter().any(|fact| {
            fact.predicate == "type"
                && fact.terms
                    == vec![
                        "HybridVehicle.HybridVehicle.battery".to_string(),
                        "BatteryPack".to_string(),
                    ]
        }));
        assert!(context.facts.iter().any(|fact| {
            fact.predicate == "owns"
                && fact.terms
                    == vec![
                        "HybridVehicle.HybridVehicle".to_string(),
                        "HybridVehicle.HybridVehicle.battery".to_string(),
                    ]
        }));
    }

    #[test]
    fn semantic_reasoning_context_truncates_with_focus_bias() {
        let files = BTreeMap::from([(
            "large.model".to_string(),
            r#"
package Demo {
    part unrelatedA;
    part unrelatedB;
    part focus {
        part child;
    }
    part unrelatedC;
}
"#
            .to_string(),
        )]);
        let project = AuthoringProject::from_model_files(files).expect("project parses");

        let context = semantic_reasoning_context_from_authoring_project(
            &project,
            WorkspaceRevision::unchecked(),
            vec![ElementRef::new("Demo.focus")],
            2,
        );

        assert!(context.truncated);
        assert!(
            context
                .elements
                .iter()
                .any(|item| item.element.qualified_name == "Demo.focus")
        );
        assert!(
            context
                .elements
                .iter()
                .any(|item| item.element.qualified_name == "Demo.focus.child")
        );
    }

    #[test]
    fn neutral_semantic_reasoning_context_keeps_trace_relationship_usage_source() {
        let files = BTreeMap::from([(
            "hybrid.model".to_string(),
            r#"
package HybridVehicle {
    part def Vehicle {
        action def RegenerativeBraking {
            trace EfficiencyRequirement references EfficiencyRequirement;
        }
    }

    requirement def EfficiencyRequirement;
}
"#
            .to_string(),
        )]);
        let project = AuthoringProject::from_model_files(files).expect("project parses");

        let context = semantic_reasoning_context_from_authoring_project(
            &project,
            WorkspaceRevision::unchecked(),
            Vec::new(),
            64,
        );

        assert!(context.relationships.iter().any(|relationship| {
            relationship.kind == "trace"
                && relationship.source.qualified_name
                    == "HybridVehicle.Vehicle.RegenerativeBraking.EfficiencyRequirement"
                && relationship
                    .target
                    .qualified_name
                    .ends_with("EfficiencyRequirement")
        }));
    }

    #[test]
    fn semantic_reasoning_context_exposes_focus_child_affordances() {
        let files = BTreeMap::from([(
            "hybrid.model".to_string(),
            r#"
package HybridVehicle {
    part HybridVehicle;
}
"#
            .to_string(),
        )]);
        let project = AuthoringProject::from_model_files(files).expect("project parses");
        let mut context = semantic_reasoning_context_from_authoring_project(
            &project,
            WorkspaceRevision::unchecked(),
            vec![ElementRef::new("HybridVehicle.HybridVehicle")],
            64,
        );

        enrich_semantic_reasoning_context_with_child_affordances(&mut context, 64);

        assert!(
            context
                .affordances
                .iter()
                .all(|affordance| affordance.child_kind != "part")
        );
    }

    #[test]
    fn semantic_reasoning_context_can_include_kir_graph_facts() {
        let mut context = semantic_reasoning_context_from_authoring_project(
            &AuthoringProject::default(),
            WorkspaceRevision::unchecked(),
            vec![ElementRef::new("type.Vehicle")],
            64,
        );
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "part_definition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "owned_feature".to_string(),
                        serde_json::Value::String("feature.Vehicle.battery".to_string()),
                    )]),
                },
                KirElement {
                    id: "feature.Vehicle.battery".to_string(),
                    kind: "part_usage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "owner".to_string(),
                        serde_json::Value::String("type.Vehicle".to_string()),
                    )]),
                },
            ],
        })
        .expect("graph builds");

        enrich_semantic_reasoning_context_with_graph(&mut context, &graph, 64, 64);

        assert!(
            context
                .elements
                .iter()
                .any(|item| item.element.qualified_name == "type.Vehicle"
                    && item.kind == "kirElement")
        );
        assert!(context.relationships.iter().any(|relationship| {
            relationship.kind == "kir.owned_feature"
                && relationship.source.qualified_name == "type.Vehicle"
                && relationship.target.qualified_name == "feature.Vehicle.battery"
        }));
        assert!(context.facts.iter().any(|fact| {
            fact.predicate == "owned_feature"
                && fact.terms
                    == vec![
                        "type.Vehicle".to_string(),
                        "feature.Vehicle.battery".to_string(),
                    ]
        }));
    }
}
