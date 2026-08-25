use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::analysis::assessment::{
    AssessmentAssertion, AssessmentExpectation, AssessmentQuery, AssessmentSpec,
};
use crate::analysis::capability::{
    SemanticArtifact, SemanticElementRef, SemanticWorkspaceSnapshot,
};
use crate::analysis::goal::{GoalPolicy, SemanticGoalCheck, SemanticGoalSpec};
use crate::kir::KirDocument;
use crate::model::MetamodelAttributeRegistry;
use crate::model::{Element, Graph, GraphError};
use crate::runtime::{Atom, Term};
use crate::semantic_services::identity::{SourceSpanRef, stable_digest};
use crate::semantic_services::mutation::WorkspaceRevision;

pub trait CognitiveProvider {
    fn status(&self) -> CognitiveProviderStatus;

    fn infer(
        &self,
        request: CognitiveInferenceRequest,
    ) -> Result<CognitiveInferenceResponse, CognitiveError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CognitiveProviderStatus {
    pub provider_id: String,
    pub label: String,
    pub available: bool,
    pub deterministic: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_operations: Vec<CognitiveOperation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveInferenceRequest {
    pub operation: CognitiveOperation,
    pub intent: DesignIntent,
    pub context: CognitiveContext,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveInferenceResponse {
    pub operation: CognitiveOperation,
    pub provider_id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<CognitiveCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<CognitiveDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<CognitiveCitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<SemanticWorkspaceRef>,
    pub focus: CognitiveFocus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<CognitiveElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<CognitiveRelationship>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<CognitiveDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SemanticArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<DesignDecision>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticWorkspaceRef {
    pub revision: WorkspaceRevision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignIntent {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

pub fn design_intent_to_semantic_goal_spec(intent: &DesignIntent) -> SemanticGoalSpec {
    let mut checks = intent
        .goals
        .iter()
        .map(|statement| SemanticGoalCheck::NamedElementExists {
            name: intent_statement_element_name(statement),
            kind: None,
        })
        .collect::<Vec<_>>();

    if checks.is_empty() {
        checks.push(SemanticGoalCheck::NamedElementExists {
            name: intent_statement_element_name(&intent.summary),
            kind: None,
        });
    }

    SemanticGoalSpec {
        policy: GoalPolicy::Any,
        checks,
    }
}

fn intent_statement_element_name(statement: &str) -> String {
    let name = statement
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_ascii_uppercase().to_string();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<String>();

    if name.is_empty() {
        "DesignIntent".to_string()
    } else {
        name
    }
}

pub fn design_intent_to_assessment_spec(intent: &DesignIntent) -> AssessmentSpec {
    let mut assertions = vec![AssessmentAssertion {
        id: "intent-summary-present".to_string(),
        description: "The assessment evidence is tied to the design intent summary.".to_string(),
        query: AssessmentQuery {
            find: vec!["summary".to_string()],
            where_atoms: vec![Atom {
                predicate: "design_intent_summary".to_string(),
                terms: vec![Term::Var("summary".to_string())],
            }],
        },
        expect: AssessmentExpectation::ContainsBinding {
            variable: "summary".to_string(),
            value: intent.summary.clone(),
        },
    }];
    assertions.extend(
        intent
            .goals
            .iter()
            .enumerate()
            .map(|(index, goal)| AssessmentAssertion {
                id: format!("intent-goal-{}", index + 1),
                description: format!("The assessment evidence includes design intent goal: {goal}"),
                query: AssessmentQuery {
                    find: vec!["goal".to_string()],
                    where_atoms: vec![Atom {
                        predicate: "design_intent_goal".to_string(),
                        terms: vec![Term::Var("goal".to_string())],
                    }],
                },
                expect: AssessmentExpectation::ContainsBinding {
                    variable: "goal".to_string(),
                    value: goal.clone(),
                },
            }),
    );
    AssessmentSpec {
        id: stable_digest([("design-intent".as_bytes(), intent.summary.as_bytes())]),
        title: intent.summary.clone(),
        assertions,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveFocus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<SemanticElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl CognitiveFocus {
    pub fn workspace() -> Self {
        Self {
            elements: Vec::new(),
            relationship_ids: Vec::new(),
            source_spans: Vec::new(),
            description: Some("workspace".to_string()),
        }
    }

    pub fn elements(elements: Vec<SemanticElementRef>) -> Self {
        Self {
            elements,
            relationship_ids: Vec::new(),
            source_spans: Vec::new(),
            description: None,
        }
    }
}

impl Default for CognitiveFocus {
    fn default() -> Self {
        Self::workspace()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveOperation {
    Explore,
    Analyze,
    Critique,
    Propose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveElement {
    pub element: SemanticElementRef,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<String>,
    pub layer: u8,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CognitiveRelationship {
    pub id: String,
    pub kind: String,
    pub source: SemanticElementRef,
    pub target: SemanticElementRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveCandidate {
    pub id: String,
    pub operation: CognitiveOperation,
    pub title: String,
    pub summary: String,
    pub confidence: CognitiveConfidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_refs: Vec<SemanticElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SemanticArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

/// A cognitive-provider diagnostic. Alias of the canonical
/// [`crate::kir::Diagnostic`].
pub use crate::kir::Diagnostic as CognitiveDiagnostic;

/// Severity for a [`CognitiveDiagnostic`]. Alias of the canonical
/// [`crate::kir::Severity`].
pub use crate::kir::Severity as CognitiveDiagnosticSeverity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CognitiveCitation {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_refs: Vec<SemanticElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignDecision {
    pub id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_refs: Vec<SemanticElementRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CognitiveError {
    ProviderUnavailable(String),
    UnsupportedOperation(CognitiveOperation),
    InvalidRequest(String),
    ContextBuild(String),
}

impl fmt::Display for CognitiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderUnavailable(message) => {
                write!(f, "cognitive provider unavailable: {message}")
            }
            Self::UnsupportedOperation(operation) => {
                write!(f, "cognitive operation is not supported: {operation:?}")
            }
            Self::InvalidRequest(message) => write!(f, "invalid cognitive request: {message}"),
            Self::ContextBuild(message) => {
                write!(f, "failed to build cognitive context: {message}")
            }
        }
    }
}

impl std::error::Error for CognitiveError {}

impl From<GraphError> for CognitiveError {
    fn from(value: GraphError) -> Self {
        Self::ContextBuild(value.to_string())
    }
}

impl CognitiveContext {
    pub fn from_workspace(workspace: &SemanticWorkspaceSnapshot, focus: CognitiveFocus) -> Self {
        Self::from_graph_with_registry(
            workspace.graph.as_ref(),
            workspace.metamodel_registry.as_ref(),
            Some(SemanticWorkspaceRef {
                revision: workspace.revision.clone(),
                profile_id: workspace.profile_id.clone(),
            }),
            focus,
        )
    }

    pub fn from_document(
        document: KirDocument,
        focus: CognitiveFocus,
    ) -> Result<Self, CognitiveError> {
        let graph = Graph::from_document(document)?;
        let registry = MetamodelAttributeRegistry::build(&graph);
        Ok(Self::from_graph_with_registry(
            &graph, &registry, None, focus,
        ))
    }

    pub fn from_graph(graph: &Graph, focus: CognitiveFocus) -> Self {
        let registry = MetamodelAttributeRegistry::build(graph);
        Self::from_graph_with_registry(graph, &registry, None, focus)
    }

    pub fn from_graph_with_registry(
        graph: &Graph,
        _metamodel_registry: &MetamodelAttributeRegistry,
        workspace: Option<SemanticWorkspaceRef>,
        focus: CognitiveFocus,
    ) -> Self {
        let focused_ids = focus
            .elements
            .iter()
            .map(|element| element.element_id.as_str())
            .collect::<BTreeSet<_>>();
        let focused_only = !focused_ids.is_empty();
        let elements = graph
            .elements()
            .iter()
            .filter(|element| !focused_only || focused_ids.contains(element.element_id.as_str()))
            .map(cognitive_element)
            .collect::<Vec<_>>();
        let in_context = elements
            .iter()
            .map(|element| element.element.element_id.as_str())
            .collect::<BTreeSet<_>>();
        let relationships = graph
            .edges()
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                let source = graph.element(edge.source)?;
                let target = graph.element(edge.target)?;
                if focused_only
                    && !in_context.contains(source.element_id.as_str())
                    && !in_context.contains(target.element_id.as_str())
                {
                    return None;
                }
                Some(CognitiveRelationship {
                    id: format!("rel.{index}"),
                    kind: edge.relation.to_string(),
                    source: semantic_element_ref(source),
                    target: semantic_element_ref(target),
                })
            })
            .collect::<Vec<_>>();
        let mut source_files = BTreeSet::new();
        for element in &elements {
            for span in &element.source_spans {
                if !span.file.is_empty() {
                    source_files.insert(span.file.clone());
                }
            }
        }
        Self {
            workspace,
            focus,
            elements,
            relationships,
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
            source_files: source_files.into_iter().collect(),
            history: Vec::new(),
            truncated: false,
        }
    }

    pub fn with_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = CognitiveDiagnostic>,
    ) -> Self {
        self.diagnostics.extend(diagnostics);
        self
    }

    pub fn with_artifacts(mut self, artifacts: impl IntoIterator<Item = SemanticArtifact>) -> Self {
        self.artifacts.extend(artifacts);
        self
    }
}

pub fn explore<P: CognitiveProvider>(
    provider: &P,
    intent: DesignIntent,
    context: CognitiveContext,
) -> Result<CognitiveInferenceResponse, CognitiveError> {
    provider.infer(CognitiveInferenceRequest {
        operation: CognitiveOperation::Explore,
        intent,
        context,
        parameters: BTreeMap::new(),
    })
}

pub fn analyze<P: CognitiveProvider>(
    provider: &P,
    intent: DesignIntent,
    context: CognitiveContext,
) -> Result<CognitiveInferenceResponse, CognitiveError> {
    provider.infer(CognitiveInferenceRequest {
        operation: CognitiveOperation::Analyze,
        intent,
        context,
        parameters: BTreeMap::new(),
    })
}

pub fn critique<P: CognitiveProvider>(
    provider: &P,
    intent: DesignIntent,
    context: CognitiveContext,
) -> Result<CognitiveInferenceResponse, CognitiveError> {
    provider.infer(CognitiveInferenceRequest {
        operation: CognitiveOperation::Critique,
        intent,
        context,
        parameters: BTreeMap::new(),
    })
}

pub fn propose<P: CognitiveProvider>(
    provider: &P,
    intent: DesignIntent,
    context: CognitiveContext,
) -> Result<CognitiveInferenceResponse, CognitiveError> {
    provider.infer(CognitiveInferenceRequest {
        operation: CognitiveOperation::Propose,
        intent,
        context,
        parameters: BTreeMap::new(),
    })
}

#[derive(Debug, Clone)]
pub struct HeuristicCognitiveProvider {
    provider_id: String,
}

impl Default for HeuristicCognitiveProvider {
    fn default() -> Self {
        Self::new("foundation.heuristic")
    }
}

impl HeuristicCognitiveProvider {
    pub fn new(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
        }
    }
}

impl CognitiveProvider for HeuristicCognitiveProvider {
    fn status(&self) -> CognitiveProviderStatus {
        CognitiveProviderStatus {
            provider_id: self.provider_id.clone(),
            label: "Foundation heuristic cognitive provider".to_string(),
            available: true,
            deterministic: true,
            supported_operations: vec![
                CognitiveOperation::Explore,
                CognitiveOperation::Analyze,
                CognitiveOperation::Critique,
                CognitiveOperation::Propose,
            ],
            limitations: vec![
                "Uses deterministic structural heuristics only; no external model is called."
                    .to_string(),
            ],
        }
    }

    fn infer(
        &self,
        request: CognitiveInferenceRequest,
    ) -> Result<CognitiveInferenceResponse, CognitiveError> {
        if !self
            .status()
            .supported_operations
            .contains(&request.operation)
        {
            return Err(CognitiveError::UnsupportedOperation(request.operation));
        }
        let candidates = match request.operation {
            CognitiveOperation::Explore => explore_candidates(&request),
            CognitiveOperation::Analyze => analyze_candidates(&request),
            CognitiveOperation::Critique => critique_candidates(&request),
            CognitiveOperation::Propose => propose_candidates(&request),
        };
        let citations = request
            .context
            .elements
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, element)| CognitiveCitation {
                id: format!("citation.{}", index + 1),
                label: element
                    .element
                    .label
                    .clone()
                    .unwrap_or_else(|| element.element.element_id.clone()),
                element_refs: vec![element.element.clone()],
                source_spans: element.source_spans.clone(),
            })
            .collect::<Vec<_>>();
        Ok(CognitiveInferenceResponse {
            operation: request.operation,
            provider_id: self.provider_id.clone(),
            summary: heuristic_summary(request.operation, &request.context, &candidates),
            candidates,
            diagnostics: request.context.diagnostics.clone(),
            citations,
            limitations: self.status().limitations,
        })
    }
}

fn explore_candidates(request: &CognitiveInferenceRequest) -> Vec<CognitiveCandidate> {
    let mut candidates = Vec::new();
    for element in prioritized_elements(&request.context).into_iter().take(3) {
        candidates.push(base_candidate(
            request.operation,
            format!("explore.{}", candidates.len() + 1),
            format!("Explore {}", element_label(element)),
            format!(
                "Inspect `{}` and its adjacent semantic relationships for intent: {}",
                element_label(element),
                request.intent.summary
            ),
            vec![element.element.clone()],
            vec![
                "Element is in the requested focus or appears early in the semantic context."
                    .to_string(),
                format!(
                    "Context includes {} relationships for structural follow-up.",
                    request.context.relationships.len()
                ),
            ],
        ));
    }
    if candidates.is_empty() {
        candidates.push(workspace_candidate(
            request.operation,
            "explore.workspace",
            "Explore workspace",
            "No focused elements were supplied; start by inspecting the workspace outline.",
        ));
    }
    candidates
}

fn analyze_candidates(request: &CognitiveInferenceRequest) -> Vec<CognitiveCandidate> {
    let impacted = request
        .context
        .relationships
        .iter()
        .flat_map(|relationship| [relationship.source.clone(), relationship.target.clone()])
        .collect::<Vec<_>>();
    vec![CognitiveCandidate {
        id: "analyze.impact".to_string(),
        operation: request.operation,
        title: "Analyze semantic impact".to_string(),
        summary: format!(
            "Structural analysis found {} elements and {} relationships relevant to `{}`.",
            request.context.elements.len(),
            request.context.relationships.len(),
            request.intent.summary
        ),
        confidence: CognitiveConfidence::Medium,
        element_refs: dedupe_refs(impacted),
        artifacts: Vec::new(),
        rationale: vec![
            "Impact is approximated from graph adjacency in the supplied cognitive context."
                .to_string(),
        ],
        risks: Vec::new(),
        metadata: BTreeMap::from([
            (
                "elementCount".to_string(),
                Value::from(request.context.elements.len()),
            ),
            (
                "relationshipCount".to_string(),
                Value::from(request.context.relationships.len()),
            ),
        ]),
    }]
}

fn critique_candidates(request: &CognitiveInferenceRequest) -> Vec<CognitiveCandidate> {
    let missing_spans = request
        .context
        .elements
        .iter()
        .filter(|element| element.source_spans.is_empty())
        .count();
    let mut risks = Vec::new();
    if request.intent.assumptions.is_empty() {
        risks.push("Intent has no explicit assumptions.".to_string());
    }
    if request.intent.constraints.is_empty() {
        risks.push("Intent has no explicit constraints.".to_string());
    }
    if missing_spans > 0 {
        risks.push(format!(
            "{missing_spans} context elements have no source span evidence."
        ));
    }
    if request.context.diagnostics.is_empty() {
        risks.push("No diagnostics were supplied for cross-checking.".to_string());
    }
    vec![CognitiveCandidate {
        id: "critique.context".to_string(),
        operation: request.operation,
        title: "Critique context readiness".to_string(),
        summary: if risks.is_empty() {
            "The supplied context has explicit intent constraints, assumptions, and evidence."
                .to_string()
        } else {
            format!("Found {} critique points before proposing changes.", risks.len())
        },
        confidence: CognitiveConfidence::High,
        element_refs: request
            .context
            .elements
            .iter()
            .take(5)
            .map(|element| element.element.clone())
            .collect(),
        artifacts: Vec::new(),
        rationale: vec![
            "Critique is deterministic and checks for missing assumptions, constraints, diagnostics, and evidence."
                .to_string(),
        ],
        risks,
        metadata: BTreeMap::new(),
    }]
}

fn propose_candidates(request: &CognitiveInferenceRequest) -> Vec<CognitiveCandidate> {
    let target = prioritized_elements(&request.context)
        .into_iter()
        .next()
        .map(|element| element.element.clone());
    let element_refs = target.clone().into_iter().collect::<Vec<_>>();
    let payload = json!({
        "intent": request.intent.summary,
        "focus": element_refs,
        "operation": "proposal_draft",
        "note": "Heuristic provider emits a reviewable draft artifact, not an authoritative model mutation."
    });
    vec![CognitiveCandidate {
        id: "propose.draft".to_string(),
        operation: request.operation,
        title: "Draft reviewable proposal".to_string(),
        summary: "Create a typed proposal draft that can be reviewed by product or AI adapters before applying authority-backed changes.".to_string(),
        confidence: CognitiveConfidence::Low,
        element_refs: element_refs.clone(),
        artifacts: vec![SemanticArtifact {
            id: "artifact.proposal_draft".to_string(),
            kind: "proposal_draft".to_string(),
            schema: "mercurio.cognitive.proposal_draft.v1".to_string(),
            digest: stable_payload_digest(&payload),
            element_refs,
            payload,
        }],
        rationale: vec![
            "Core cognitive providers may propose typed artifacts, but accepted changes still flow through workspace authority."
                .to_string(),
        ],
        risks: vec![
            "Heuristic proposal requires semantic feasibility and user review before application."
                .to_string(),
        ],
        metadata: BTreeMap::new(),
    }]
}

fn base_candidate(
    operation: CognitiveOperation,
    id: String,
    title: String,
    summary: String,
    element_refs: Vec<SemanticElementRef>,
    rationale: Vec<String>,
) -> CognitiveCandidate {
    CognitiveCandidate {
        id,
        operation,
        title,
        summary,
        confidence: CognitiveConfidence::Medium,
        element_refs,
        artifacts: Vec::new(),
        rationale,
        risks: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn workspace_candidate(
    operation: CognitiveOperation,
    id: &str,
    title: &str,
    summary: &str,
) -> CognitiveCandidate {
    base_candidate(
        operation,
        id.to_string(),
        title.to_string(),
        summary.to_string(),
        Vec::new(),
        vec!["No element focus was available in the cognitive context.".to_string()],
    )
}

fn prioritized_elements(context: &CognitiveContext) -> Vec<&CognitiveElement> {
    let focus_ids = context
        .focus
        .elements
        .iter()
        .map(|element| element.element_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut elements = context.elements.iter().collect::<Vec<_>>();
    elements.sort_by_key(|element| {
        (
            !focus_ids.contains(element.element.element_id.as_str()),
            element.layer,
            element.element.element_id.clone(),
        )
    });
    elements
}

fn heuristic_summary(
    operation: CognitiveOperation,
    context: &CognitiveContext,
    candidates: &[CognitiveCandidate],
) -> String {
    format!(
        "{operation:?} produced {} candidate(s) from {} semantic element(s), {} relationship(s), and {} diagnostic(s).",
        candidates.len(),
        context.elements.len(),
        context.relationships.len(),
        context.diagnostics.len()
    )
}

fn cognitive_element(element: &Element) -> CognitiveElement {
    CognitiveElement {
        element: semantic_element_ref(element),
        kind: element.kind.to_string(),
        metatype: string_property(element, "metatype").or_else(|| string_property(element, "type")),
        layer: element.layer,
        attributes: element.properties.to_btree_map(),
        source_spans: source_span_for_element(element).into_iter().collect(),
    }
}

fn semantic_element_ref(element: &Element) -> SemanticElementRef {
    SemanticElementRef::from_graph_element(element)
}

fn source_span_for_element(element: &Element) -> Option<SourceSpanRef> {
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

fn string_property(element: &Element, key: &str) -> Option<String> {
    element
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn element_label(element: &CognitiveElement) -> String {
    element
        .element
        .label
        .clone()
        .or_else(|| element.element.qualified_name.clone())
        .unwrap_or_else(|| element.element.element_id.clone())
}

fn dedupe_refs(refs: Vec<SemanticElementRef>) -> Vec<SemanticElementRef> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for element_ref in refs {
        if seen.insert(element_ref.element_id.clone()) {
            deduped.push(element_ref);
        }
    }
    deduped
}

fn stable_payload_digest(payload: &Value) -> String {
    let serialized = serde_json::to_string(payload).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325u64;
    for byte in serialized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ai_context_usage() -> crate::semantic_services::mutation::AiSemanticContextUsage {
        crate::semantic_services::mutation::AiSemanticContextUsage {
            authoritative_for_existing_elements: true,
            prefer_ranked_allowed_affordances: true,
            cite_rule_diagnostics: true,
            element_ref_format: "dot_qualified".to_string(),
        }
    }
    use crate::kir::KirElement;

    fn test_document() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 0,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        ("qualified_name".to_string(), json!("Vehicle")),
                        ("owned_features".to_string(), json!(["Vehicle.engine"])),
                        (
                            "source_span".to_string(),
                            json!({
                                "file": "vehicle.sysml",
                                "start_line": 1,
                                "start_col": 1,
                                "end_line": 3,
                                "end_col": 2
                            }),
                        ),
                    ]),
                },
                KirElement {
                    id: "Vehicle.engine".to_string(),
                    kind: "PartUsage".to_string(),
                    layer: 0,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("engine")),
                        ("qualified_name".to_string(), json!("Vehicle.engine")),
                        ("owner".to_string(), json!("Vehicle")),
                    ]),
                },
            ],
        }
    }

    fn intent() -> DesignIntent {
        DesignIntent {
            summary: "Improve vehicle efficiency".to_string(),
            goals: vec!["reduce losses".to_string()],
            constraints: vec!["preserve existing interfaces".to_string()],
            assumptions: vec!["current model is preliminary".to_string()],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn cognitive_context_extracts_semantic_elements_and_relationships() {
        let context =
            CognitiveContext::from_document(test_document(), CognitiveFocus::workspace()).unwrap();

        assert_eq!(context.elements.len(), 2);
        assert_eq!(context.relationships.len(), 2);
        assert_eq!(context.source_files, vec!["vehicle.sysml"]);
        assert_eq!(
            context.elements[0].element.label.as_deref(),
            Some("Vehicle")
        );
        assert_eq!(context.elements[0].source_spans[0].file, "vehicle.sysml");
    }

    #[test]
    fn focused_context_limits_elements_but_keeps_adjacent_relationships() {
        let focus = CognitiveFocus::elements(vec![
            SemanticElementRef::new("Vehicle.engine")
                .with_qualified_name("Vehicle.engine")
                .with_label("engine"),
        ]);
        let context = CognitiveContext::from_document(test_document(), focus).unwrap();

        assert_eq!(context.elements.len(), 1);
        assert_eq!(context.elements[0].element.element_id, "Vehicle.engine");
        assert_eq!(context.relationships.len(), 2);
    }

    #[test]
    fn design_intent_adapters_share_one_intent_source() {
        let intent = intent();
        let goal = design_intent_to_semantic_goal_spec(&intent);
        let assessment = design_intent_to_assessment_spec(&intent);

        assert!(!goal.checks.is_empty());
        assert_eq!(assessment.title, "Improve vehicle efficiency");
        assert_eq!(assessment.assertions.len(), 2);
        assert!(assessment.id.starts_with("sha256:") || assessment.id.starts_with("fnv"));
    }

    #[test]
    fn design_intent_drives_assessment_goal_and_quality_gate_contracts() {
        let intent = intent();
        let assessment = design_intent_to_assessment_spec(&intent);
        let assessment_result = crate::analysis::assessment::run_runtime_assessment(
            crate::analysis::assessment::RuntimeAssessmentRequest {
                spec: assessment,
                rulepacks: Vec::new(),
                facts: vec![
                    crate::runtime::Fact::new("design_intent_summary", [intent.summary.clone()]),
                    crate::runtime::Fact::new("design_intent_goal", [intent.goals[0].clone()]),
                ],
            },
        )
        .unwrap();

        assert_eq!(
            assessment_result.report.status,
            crate::analysis::assessment::AssessmentStatus::Pass
        );

        let context = crate::semantic_services::mutation::SemanticReasoningContext {
            schema_version: crate::semantic_services::mutation::AI_SEMANTIC_CONTEXT_SCHEMA_VERSION
                .to_string(),
            metamodel_version: "test".to_string(),
            workspace_revision: crate::semantic_services::mutation::WorkspaceRevision::unchecked(),
            focus: Vec::new(),
            elements: vec![
                crate::semantic_services::mutation::SemanticElementContext {
                    element: crate::semantic_services::mutation::ElementRef::new(
                        "Demo.ReduceLosses",
                    ),
                    kind: "definition".to_string(),
                    label: "ReduceLosses".to_string(),
                    owner: Some(crate::semantic_services::mutation::ElementRef::new("Demo")),
                    attributes: BTreeMap::from([
                        (
                            "keyword".to_string(),
                            serde_json::Value::String("requirement".to_string()),
                        ),
                        (
                            "id".to_string(),
                            serde_json::Value::String("REQ-EFF-001".to_string()),
                        ),
                        (
                            "text".to_string(),
                            serde_json::Value::String(
                                "The design shall reduce losses.".to_string(),
                            ),
                        ),
                    ]),
                },
                crate::semantic_services::mutation::SemanticElementContext {
                    element: crate::semantic_services::mutation::ElementRef::new(
                        "Demo.Vehicle.engine",
                    ),
                    kind: "usage".to_string(),
                    label: "engine".to_string(),
                    owner: Some(crate::semantic_services::mutation::ElementRef::new(
                        "Demo.Vehicle",
                    )),
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
            facts: Vec::new(),
            affordances: Vec::new(),
            source_files: Vec::new(),
            truncated: false,
            usage: test_ai_context_usage(),
        };
        let goal = design_intent_to_semantic_goal_spec(&intent);
        let goal_evaluation = crate::analysis::goal::evaluate_semantic_goal(&context, &goal);
        let quality_gate_evaluation = crate::analysis::goal::evaluate_semantic_goal(
            &context,
            &crate::analysis::goal::default_model_quality_profile().goal,
        );

        assert!(goal_evaluation.satisfied);
        assert_eq!(goal_evaluation.score, 1.0);
        assert!(quality_gate_evaluation.satisfied);
        assert_eq!(quality_gate_evaluation.score, 1.0);
    }

    #[test]
    fn heuristic_provider_supports_all_core_operations() {
        let provider = HeuristicCognitiveProvider::default();
        let context =
            CognitiveContext::from_document(test_document(), CognitiveFocus::workspace()).unwrap();

        for operation in [
            CognitiveOperation::Explore,
            CognitiveOperation::Analyze,
            CognitiveOperation::Critique,
            CognitiveOperation::Propose,
        ] {
            let response = provider
                .infer(CognitiveInferenceRequest {
                    operation,
                    intent: intent(),
                    context: context.clone(),
                    parameters: BTreeMap::new(),
                })
                .unwrap();
            assert_eq!(response.operation, operation);
            assert_eq!(response.provider_id, "foundation.heuristic");
            assert!(!response.candidates.is_empty());
        }
    }

    #[test]
    fn operation_helpers_delegate_to_provider() {
        let provider = HeuristicCognitiveProvider::default();
        let context =
            CognitiveContext::from_document(test_document(), CognitiveFocus::workspace()).unwrap();

        let response = propose(&provider, intent(), context).unwrap();

        assert_eq!(response.operation, CognitiveOperation::Propose);
        assert_eq!(response.candidates[0].artifacts[0].kind, "proposal_draft");
    }
}
