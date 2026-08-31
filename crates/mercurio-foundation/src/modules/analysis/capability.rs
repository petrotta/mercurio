use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

use crate::analysis::AnalysisInventory;
use crate::kir::{KirDocument, KirElement, KirError};
use crate::model::{Edge, Graph};
use crate::model::{
    MetamodelAttributeDeclaration, MetamodelAttributeRegistry, collect_specialization_ancestors,
};
use crate::semantic_services::identity::{
    SemanticAnchor, SemanticAnchorResolution, SourceSpanRef, resolve_semantic_anchor,
    semantic_anchor_for_element, workspace_revision_for_kir_document,
};
pub use crate::semantic_services::mutation::SemanticElementRef;
use crate::semantic_services::mutation::WorkspaceRevision;
use crate::semantic_services::semantic_validation::{
    SemanticValidationReport, validate_kir_semantics, validate_kir_semantics_for_graph,
};
use crate::workspace::model_state::{ModelBuildRecord, ModelRevision, ModelRevisionProducer};

#[derive(Debug, Clone)]
pub struct SemanticWorkspaceSnapshot {
    pub revision: WorkspaceRevision,
    pub kir: Arc<KirDocument>,
    pub graph: Arc<Graph>,
    pub metamodel_registry: Arc<MetamodelAttributeRegistry>,
    pub validation_report: SemanticValidationReport,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub name: String,
    pub kind: CapabilityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_artifact_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub produced_insight_kinds: Vec<InsightKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub produced_artifact_kinds: Vec<String>,
    pub deterministic: bool,
    pub cost_class: CapabilityCostClass,
    pub maturity: CapabilityMaturity,
}

pub const CAPABILITY_KIND_REQUIREMENT_ANALYSIS: &str =
    "mercurio.capability.kind/requirement-analysis";
pub const CAPABILITY_KIND_TRACEABILITY: &str = "mercurio.capability.kind/traceability";
pub const CAPABILITY_KIND_IMPACT_ANALYSIS: &str = "mercurio.capability.kind/impact-analysis";
pub const CAPABILITY_KIND_DYNAMIC_BEHAVIOR: &str = "mercurio.capability.kind/dynamic-behavior";
pub const CAPABILITY_KIND_CONSTRAINT_ANALYSIS: &str =
    "mercurio.capability.kind/constraint-analysis";
pub const CAPABILITY_KIND_CONTRACT_ANALYSIS: &str = "mercurio.capability.kind/contract-analysis";
pub const CAPABILITY_KIND_MUTATION_PREVIEW: &str = "mercurio.capability.kind/mutation-preview";
pub const CAPABILITY_KIND_SEMANTIC_COMPARISON: &str =
    "mercurio.capability.kind/semantic-comparison";
pub const CAPABILITY_KIND_DECISION_ASSESSMENT: &str =
    "mercurio.capability.kind/decision-assessment";
pub const CAPABILITY_KIND_CUSTOM: &str = "mercurio.capability.kind/custom";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityKind(pub Cow<'static, str>);

impl CapabilityKind {
    pub fn new(kind: impl Into<String>) -> Self {
        let kind = kind.into();
        match canonical_capability_kind(&kind) {
            Some(canonical) => Self(Cow::Borrowed(canonical)),
            None => Self(Cow::Owned(kind)),
        }
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    #[allow(non_upper_case_globals)]
    pub const RequirementAnalysis: Self = Self(Cow::Borrowed(CAPABILITY_KIND_REQUIREMENT_ANALYSIS));
    #[allow(non_upper_case_globals)]
    pub const Traceability: Self = Self(Cow::Borrowed(CAPABILITY_KIND_TRACEABILITY));
    #[allow(non_upper_case_globals)]
    pub const ImpactAnalysis: Self = Self(Cow::Borrowed(CAPABILITY_KIND_IMPACT_ANALYSIS));
    #[allow(non_upper_case_globals)]
    pub const DynamicBehavior: Self = Self(Cow::Borrowed(CAPABILITY_KIND_DYNAMIC_BEHAVIOR));
    #[allow(non_upper_case_globals)]
    pub const ConstraintAnalysis: Self = Self(Cow::Borrowed(CAPABILITY_KIND_CONSTRAINT_ANALYSIS));
    #[allow(non_upper_case_globals)]
    pub const ContractAnalysis: Self = Self(Cow::Borrowed(CAPABILITY_KIND_CONTRACT_ANALYSIS));
    #[allow(non_upper_case_globals)]
    pub const MutationPreview: Self = Self(Cow::Borrowed(CAPABILITY_KIND_MUTATION_PREVIEW));
    #[allow(non_upper_case_globals)]
    pub const SemanticComparison: Self = Self(Cow::Borrowed(CAPABILITY_KIND_SEMANTIC_COMPARISON));
    #[allow(non_upper_case_globals)]
    pub const DecisionAssessment: Self = Self(Cow::Borrowed(CAPABILITY_KIND_DECISION_ASSESSMENT));
    #[allow(non_upper_case_globals)]
    pub const Custom: Self = Self(Cow::Borrowed(CAPABILITY_KIND_CUSTOM));
}

impl Serialize for CapabilityKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_ref())
    }
}

impl<'de> Deserialize<'de> for CapabilityKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::new(raw))
    }
}

impl Default for CapabilityKind {
    fn default() -> Self {
        Self::Custom
    }
}

fn canonical_capability_kind(raw: &str) -> Option<&'static str> {
    match raw {
        "requirement_analysis" | CAPABILITY_KIND_REQUIREMENT_ANALYSIS => {
            Some(CAPABILITY_KIND_REQUIREMENT_ANALYSIS)
        }
        "traceability" | CAPABILITY_KIND_TRACEABILITY => Some(CAPABILITY_KIND_TRACEABILITY),
        "impact_analysis" | CAPABILITY_KIND_IMPACT_ANALYSIS => {
            Some(CAPABILITY_KIND_IMPACT_ANALYSIS)
        }
        "dynamic_behavior" | CAPABILITY_KIND_DYNAMIC_BEHAVIOR => {
            Some(CAPABILITY_KIND_DYNAMIC_BEHAVIOR)
        }
        "constraint_analysis" | CAPABILITY_KIND_CONSTRAINT_ANALYSIS => {
            Some(CAPABILITY_KIND_CONSTRAINT_ANALYSIS)
        }
        "contract_analysis" | CAPABILITY_KIND_CONTRACT_ANALYSIS => {
            Some(CAPABILITY_KIND_CONTRACT_ANALYSIS)
        }
        "mutation_preview" | CAPABILITY_KIND_MUTATION_PREVIEW => {
            Some(CAPABILITY_KIND_MUTATION_PREVIEW)
        }
        "semantic_comparison" | CAPABILITY_KIND_SEMANTIC_COMPARISON => {
            Some(CAPABILITY_KIND_SEMANTIC_COMPARISON)
        }
        "decision_assessment" | CAPABILITY_KIND_DECISION_ASSESSMENT => {
            Some(CAPABILITY_KIND_DECISION_ASSESSMENT)
        }
        "custom" | CAPABILITY_KIND_CUSTOM => Some(CAPABILITY_KIND_CUSTOM),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityCostClass {
    Cheap,
    Moderate,
    Expensive,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMaturity {
    Experimental,
    Prototype,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisScope {
    AuthoredModel,
    Stdlib,
    Metamodel,
    All,
}

impl AnalysisScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthoredModel => "authored_model",
            Self::Stdlib => "stdlib",
            Self::Metamodel => "metamodel",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRunRequest {
    pub run_id: String,
    pub capability_id: String,
    #[serde(default)]
    pub target: CapabilityTarget,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_artifacts: Vec<SemanticArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityTarget {
    Workspace,
    Element { element_id: String },
    Scope { scope_id: String },
}

impl Default for CapabilityTarget {
    fn default() -> Self {
        Self::Workspace
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityReadinessReport {
    pub capability_id: String,
    pub target: CapabilityTarget,
    pub status: CapabilityReadinessStatus,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReadinessStatus {
    Ready,
    Partial,
    NotApplicable,
    Blocked,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRunReport {
    pub run_id: String,
    pub capability_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_version: Option<String>,
    pub status: CapabilityRunStatus,
    pub target: CapabilityTarget,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub insights: Vec<SemanticInsight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SemanticArtifact>,
    #[serde(default)]
    pub evidence: EvidenceGraph,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SemanticDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

pub const FOUNDATION_CAPABILITY_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRunStatus {
    Passed,
    Failed,
    Inconclusive,
    Partial,
    NotApplicable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticInsight {
    pub id: String,
    pub kind: InsightKind,
    pub subject: SemanticElementRef,
    pub claim: String,
    pub polarity: InsightPolarity,
    pub severity: InsightSeverity,
    pub confidence: InsightConfidence,
    pub scope: InsightScope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metrics: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    CoverageGap,
    VerificationGap,
    SatisfactionEvidence,
    RequirementRisk,
    RequirementConflict,
    TraceCompleteness,
    ImpactHotspot,
    DependencyClosure,
    AffectedElement,
    ChangeRisk,
    BehaviorObserved,
    ScenarioFailure,
    RequirementViolation,
    ReachabilityFinding,
    RuntimeMetric,
    ConstraintViolation,
    SatisfiedConstraint,
    UnknownVariable,
    FeasibleRegion,
    DerivedMetric,
    AssumptionGap,
    GuaranteeViolation,
    InterfaceRisk,
    ContractCompatibility,
    ProposedEdit,
    FeasibilityIssue,
    RequiredChoice,
    ValidationDelta,
    SemanticDiff,
    ModelDelta,
    ConformanceGap,
    Regression,
    Improvement,
    CriterionPass,
    CriterionFail,
    MissingEvidence,
    DecisionRisk,
    RecommendedNextAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightPolarity {
    Supports,
    Weakens,
    Neutral,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InsightScope {
    Workspace,
    Element { element_id: String },
    Scenario { scenario_id: String },
    Alternative { alternative_id: String },
    Revision { revision: WorkspaceRevision },
}

impl Default for InsightScope {
    fn default() -> Self {
        Self::Workspace
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticArtifact {
    pub id: String,
    pub kind: String,
    pub schema: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_refs: Vec<SemanticElementRef>,
    pub payload: Value,
}

/// A capability/analysis diagnostic. Alias of the canonical
/// [`crate::kir::Diagnostic`]; element references live in `subjects` and
/// locations in `source_spans`.
pub use crate::kir::Diagnostic as SemanticDiagnostic;

/// Severity for a [`SemanticDiagnostic`]. Alias of the canonical
/// [`crate::kir::Severity`].
pub use crate::kir::Severity as SemanticDiagnosticSeverity;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EvidenceGraph {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<EvidenceNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<EvidenceEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceNode {
    pub id: String,
    pub kind: EvidenceNodeKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_refs: Vec<SemanticElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceNodeKind {
    KirElement,
    SourceSpan,
    Fact,
    Rule,
    AnalysisRun,
    Capability,
    Artifact,
    HumanDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation: EvidenceRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRelation {
    Supports,
    DerivedFrom,
    ProducedBy,
    Consumed,
    Affects,
    Explains,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionContext {
    pub id: String,
    pub question: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub criteria: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenarios: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_elements: Vec<SemanticElementRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionAssessment {
    pub context: DecisionContext,
    pub status: CapabilityRunStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub insights: Vec<SemanticInsight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended_next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    DuplicateCapability(String),
    MissingCapability(String),
    InvalidRequest(String),
    Workspace(String),
    Execution(String),
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCapability(id) => write!(f, "duplicate capability `{id}`"),
            Self::MissingCapability(id) => write!(f, "missing capability `{id}`"),
            Self::InvalidRequest(message) => write!(f, "invalid capability request: {message}"),
            Self::Workspace(message) => write!(f, "workspace capability error: {message}"),
            Self::Execution(message) => write!(f, "capability execution error: {message}"),
        }
    }
}

impl std::error::Error for CapabilityError {}

impl From<KirError> for CapabilityError {
    fn from(value: KirError) -> Self {
        Self::Workspace(value.to_string())
    }
}

pub trait SemanticCapability: Send + Sync {
    fn descriptor(&self) -> CapabilityDescriptor;

    fn readiness(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        target: &CapabilityTarget,
    ) -> CapabilityReadinessReport;

    fn run(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        request: CapabilityRunRequest,
    ) -> Result<CapabilityRunReport, CapabilityError>;
}

#[derive(Default)]
pub struct CapabilityRegistry {
    capabilities: BTreeMap<String, Box<dyn SemanticCapability>>,
}

#[derive(Debug, Clone, Default)]
pub struct GenericImpactCapability;

#[derive(Debug, Clone, Default)]
pub struct GenericModelInspectionCapability;

#[derive(Debug, Clone, Default)]
pub struct GenericAnalysisOpportunityCapability;

impl SemanticWorkspaceSnapshot {
    pub fn from_document(kir: KirDocument) -> Result<Self, CapabilityError> {
        Self::from_document_with_profile(kir, None)
    }

    pub fn from_document_with_profile(
        kir: KirDocument,
        profile_id: Option<String>,
    ) -> Result<Self, CapabilityError> {
        kir.validate_persisted()?;
        let validation_report = validate_kir_semantics(&kir)
            .map_err(|err| CapabilityError::Workspace(err.to_string()))?;
        let revision = workspace_revision_for_kir_document(&kir)?;
        let graph = Graph::from_document(kir.clone())
            .map_err(|err| CapabilityError::Workspace(err.to_string()))?;
        let metamodel_registry = MetamodelAttributeRegistry::build(&graph);
        Ok(Self {
            revision,
            kir: Arc::new(kir),
            graph: Arc::new(graph),
            metamodel_registry: Arc::new(metamodel_registry),
            validation_report,
            profile_id,
        })
    }

    pub fn from_graph_with_profile(
        graph: Graph,
        profile_id: Option<String>,
    ) -> Result<Self, CapabilityError> {
        let kir = KirDocument {
            metadata: BTreeMap::new(),
            elements: graph
                .elements()
                .iter()
                .map(|element| KirElement {
                    id: element.element_id.clone(),
                    kind: element.kind.to_string(),
                    layer: element.layer,
                    properties: element.properties.to_btree_map(),
                })
                .collect(),
        };
        let revision = workspace_revision_for_kir_document(&kir)?;
        let metamodel_registry = MetamodelAttributeRegistry::build(&graph);
        let validation_report = validate_kir_semantics_for_graph(&graph);
        Ok(Self {
            revision,
            kir: Arc::new(kir),
            graph: Arc::new(graph),
            metamodel_registry: Arc::new(metamodel_registry),
            validation_report,
            profile_id,
        })
    }

    pub fn from_model_revision(revision: &ModelRevision) -> Self {
        let graph = revision.graph();
        let validation_report = validate_kir_semantics_for_graph(&graph);
        Self {
            revision: revision.workspace_revision().clone(),
            kir: revision.kir(),
            graph,
            metamodel_registry: revision.metamodel_registry(),
            validation_report,
            profile_id: revision.profile_id().map(ToOwned::to_owned),
        }
    }

    pub fn model_revision(&self) -> ModelRevision {
        ModelRevision::from_parts(
            Arc::clone(&self.kir),
            Arc::clone(&self.graph),
            Arc::clone(&self.metamodel_registry),
            self.revision.clone(),
            self.profile_id.clone(),
            ModelBuildRecord::new(ModelRevisionProducer::SemanticSnapshot),
        )
    }

    pub fn element(&self, element_id: &str) -> Option<&crate::model::Element> {
        self.graph.element_by_element_id(element_id)
    }

    pub fn element_ref(&self, element_id: &str) -> SemanticElementRef {
        self.element(element_id)
            .map(|element| {
                let mut reference = element_ref(element);
                reference.semantic_anchor = self.anchor_for_element(element_id).ok().flatten();
                reference
            })
            .unwrap_or_else(|| SemanticElementRef::new(element_id))
    }

    pub fn source_spans(&self, element_id: &str) -> Vec<SourceSpanRef> {
        self.element(element_id)
            .and_then(source_span_for_element)
            .into_iter()
            .collect()
    }

    pub fn anchor_for_element(
        &self,
        element_id: &str,
    ) -> Result<Option<SemanticAnchor>, CapabilityError> {
        semantic_anchor_for_element(&self.kir, element_id)
            .map_err(|err| CapabilityError::Workspace(err.to_string()))
    }

    pub fn resolve_anchor(
        &self,
        anchor: &SemanticAnchor,
    ) -> Result<SemanticAnchorResolution, CapabilityError> {
        resolve_semantic_anchor(&self.kir, anchor)
            .map_err(|err| CapabilityError::Workspace(err.to_string()))
    }
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_foundation_builtins() -> Self {
        let mut registry = Self::new();
        registry
            .register(GenericImpactCapability)
            .expect("foundation impact capability id is unique");
        registry
            .register(GenericModelInspectionCapability)
            .expect("foundation inspection capability id is unique");
        registry
            .register(GenericAnalysisOpportunityCapability)
            .expect("foundation analysis opportunity capability id is unique");
        registry
    }

    pub fn register(
        &mut self,
        capability: impl SemanticCapability + 'static,
    ) -> Result<(), CapabilityError> {
        let descriptor = capability.descriptor();
        if self.capabilities.contains_key(&descriptor.id) {
            return Err(CapabilityError::DuplicateCapability(descriptor.id));
        }
        self.capabilities
            .insert(descriptor.id, Box::new(capability));
        Ok(())
    }

    pub fn list(&self) -> Vec<CapabilityDescriptor> {
        self.capabilities
            .values()
            .map(|capability| capability.descriptor())
            .collect()
    }

    pub fn readiness(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        capability_id: &str,
        target: &CapabilityTarget,
    ) -> Result<CapabilityReadinessReport, CapabilityError> {
        Ok(self
            .capabilities
            .get(capability_id)
            .ok_or_else(|| CapabilityError::MissingCapability(capability_id.to_string()))?
            .readiness(workspace, target))
    }

    pub fn run(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        request: CapabilityRunRequest,
    ) -> Result<CapabilityRunReport, CapabilityError> {
        self.capabilities
            .get(&request.capability_id)
            .ok_or_else(|| CapabilityError::MissingCapability(request.capability_id.clone()))?
            .run(workspace, request)
    }
}

pub fn assess_decision_context(
    context: DecisionContext,
    readiness: &[CapabilityReadinessReport],
    reports: &[CapabilityRunReport],
) -> DecisionAssessment {
    let insights = reports
        .iter()
        .flat_map(|report| report.insights.iter().cloned())
        .collect::<Vec<_>>();
    let missing_evidence = missing_evidence(readiness, reports);
    let status = decision_status(readiness, reports, &insights, &missing_evidence);
    let recommended_next_actions =
        recommended_next_actions(readiness, reports, &insights, &missing_evidence);

    DecisionAssessment {
        context,
        status,
        insights,
        missing_evidence,
        recommended_next_actions,
    }
}

fn missing_evidence(
    readiness: &[CapabilityReadinessReport],
    reports: &[CapabilityRunReport],
) -> Vec<String> {
    let reported = reports
        .iter()
        .map(|report| report.capability_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut missing = Vec::new();

    for readiness in readiness {
        match readiness.status {
            CapabilityReadinessStatus::NotApplicable => {}
            CapabilityReadinessStatus::Ready | CapabilityReadinessStatus::Partial
                if reported.contains(readiness.capability_id.as_str()) => {}
            _ => push_unique(
                &mut missing,
                format!("{}: {}", readiness.capability_id, readiness.message),
            ),
        }
    }

    for report in reports {
        if report.status == CapabilityRunStatus::Inconclusive && report.insights.is_empty() {
            push_unique(
                &mut missing,
                format!(
                    "{}: produced no decision-relevant insights",
                    report.capability_id
                ),
            );
        }
    }

    missing
}

fn decision_status(
    readiness: &[CapabilityReadinessReport],
    reports: &[CapabilityRunReport],
    insights: &[SemanticInsight],
    missing_evidence: &[String],
) -> CapabilityRunStatus {
    if readiness
        .iter()
        .any(|report| report.status == CapabilityReadinessStatus::Error)
        || reports
            .iter()
            .any(|report| report.status == CapabilityRunStatus::Error)
    {
        return CapabilityRunStatus::Error;
    }

    if reports
        .iter()
        .any(|report| report.status == CapabilityRunStatus::Failed)
        || insights.iter().any(is_failure_insight)
    {
        return CapabilityRunStatus::Failed;
    }

    if reports.is_empty() || insights.is_empty() {
        return CapabilityRunStatus::Inconclusive;
    }

    if !missing_evidence.is_empty()
        || reports
            .iter()
            .any(|report| report.status == CapabilityRunStatus::Partial)
        || insights.iter().any(is_cautionary_insight)
    {
        return CapabilityRunStatus::Partial;
    }

    CapabilityRunStatus::Passed
}

fn recommended_next_actions(
    readiness: &[CapabilityReadinessReport],
    reports: &[CapabilityRunReport],
    insights: &[SemanticInsight],
    missing_evidence: &[String],
) -> Vec<String> {
    let mut actions = Vec::new();

    let mut prioritized_insights = insights.iter().collect::<Vec<_>>();
    prioritized_insights.sort_by(|left, right| {
        action_priority(left)
            .cmp(&action_priority(right))
            .then_with(|| severity_rank(right.severity).cmp(&severity_rank(left.severity)))
    });

    for insight in prioritized_insights {
        match insight.kind {
            InsightKind::CoverageGap => push_unique(
                &mut actions,
                "Add satisfy evidence for uncovered requirements, then rerun requirement analysis.",
            ),
            InsightKind::VerificationGap => push_unique(
                &mut actions,
                "Add or link verification cases for requirements with verification gaps, then rerun requirement analysis.",
            ),
            InsightKind::RequirementRisk | InsightKind::RequirementConflict => push_unique(
                &mut actions,
                "Resolve the requirement risk or conflict before accepting the decision.",
            ),
            InsightKind::ScenarioFailure
            | InsightKind::RequirementViolation
            | InsightKind::ReachabilityFinding => push_unique(
                &mut actions,
                "Make the behavior scenario executable and rerun dynamic behavior analysis.",
            ),
            InsightKind::ConstraintViolation
            | InsightKind::UnknownVariable
            | InsightKind::FeasibilityIssue => push_unique(
                &mut actions,
                "Resolve constraint violations or bind unknown variables, then rerun constraint analysis.",
            ),
            InsightKind::AssumptionGap
            | InsightKind::GuaranteeViolation
            | InsightKind::InterfaceRisk => push_unique(
                &mut actions,
                "Resolve contract assumptions, guarantees, or interface risks before committing the design.",
            ),
            InsightKind::ImpactHotspot | InsightKind::AffectedElement | InsightKind::ChangeRisk => {
                push_unique(
                    &mut actions,
                    "Review impact hotspots before selecting a model edit.",
                )
            }
            InsightKind::RequiredChoice => push_unique(
                &mut actions,
                "Capture the required engineering choice explicitly so downstream analyses can evaluate it.",
            ),
            InsightKind::Regression | InsightKind::ConformanceGap => push_unique(
                &mut actions,
                "Compare the candidate against the baseline and address regressions or conformance gaps.",
            ),
            InsightKind::DecisionRisk => push_unique(
                &mut actions,
                "Resolve the highest-severity decision risk before accepting an alternative.",
            ),
            InsightKind::MissingEvidence => push_unique(
                &mut actions,
                "Add or run the missing evidence capability before making the decision.",
            ),
            _ => {}
        }
    }

    for report in reports {
        if report.status == CapabilityRunStatus::Error {
            push_unique(
                &mut actions,
                format!(
                    "Fix capability execution for `{}` before relying on its result.",
                    report.capability_id
                ),
            );
        }
    }

    for readiness in readiness.iter().filter(|readiness| {
        matches!(
            readiness.status,
            CapabilityReadinessStatus::Blocked | CapabilityReadinessStatus::Error
        )
    }) {
        push_unique(
            &mut actions,
            format!(
                "Unblock `{}`: {}",
                readiness.capability_id, readiness.message
            ),
        );
    }

    if !missing_evidence.is_empty() {
        push_unique(
            &mut actions,
            format!(
                "Add or unblock missing capability evidence: {}.",
                missing_evidence
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        );
    }

    if actions.is_empty() {
        if insights.is_empty() {
            actions.push(
                "Run at least one applicable semantic capability before committing to a design decision."
                    .to_string(),
            );
        } else {
            actions.push(
                "Record the decision rationale and rerun capability analysis after the next model change."
                    .to_string(),
            );
        }
    }

    actions
}

fn action_priority(insight: &SemanticInsight) -> u8 {
    if is_failure_insight(insight) {
        return 0;
    }

    match insight.kind {
        InsightKind::ScenarioFailure
        | InsightKind::RequirementViolation
        | InsightKind::ConstraintViolation
        | InsightKind::GuaranteeViolation
        | InsightKind::Regression => 1,
        InsightKind::CoverageGap
        | InsightKind::VerificationGap
        | InsightKind::RequirementRisk
        | InsightKind::RequirementConflict
        | InsightKind::UnknownVariable
        | InsightKind::FeasibilityIssue
        | InsightKind::AssumptionGap
        | InsightKind::InterfaceRisk
        | InsightKind::ConformanceGap
        | InsightKind::DecisionRisk
        | InsightKind::MissingEvidence => 2,
        InsightKind::RequiredChoice => 3,
        InsightKind::ImpactHotspot | InsightKind::AffectedElement | InsightKind::ChangeRisk => 4,
        _ => 5,
    }
}

fn severity_rank(severity: InsightSeverity) -> u8 {
    match severity {
        InsightSeverity::Critical => 4,
        InsightSeverity::Error => 3,
        InsightSeverity::Warning => 2,
        InsightSeverity::Info => 1,
    }
}

fn is_failure_insight(insight: &SemanticInsight) -> bool {
    insight.severity == InsightSeverity::Critical
        || (insight.polarity == InsightPolarity::Weakens
            && matches!(
                insight.severity,
                InsightSeverity::Error | InsightSeverity::Critical
            ))
}

fn is_cautionary_insight(insight: &SemanticInsight) -> bool {
    insight.polarity == InsightPolarity::Weakens
        && matches!(
            insight.severity,
            InsightSeverity::Warning | InsightSeverity::Error | InsightSeverity::Critical
        )
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !values.contains(&value) {
        values.push(value);
    }
}

impl SemanticCapability for GenericModelInspectionCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: "foundation.inspect.model".to_string(),
            name: "Generic Model and Metamodel Inspection".to_string(),
            kind: CapabilityKind::Custom,
            profile_id: None,
            target_kinds: Vec::new(),
            relationship_kinds: Vec::new(),
            input_artifact_kinds: Vec::new(),
            produced_insight_kinds: vec![
                InsightKind::AffectedElement,
                InsightKind::TraceCompleteness,
            ],
            produced_artifact_kinds: vec![
                "model_inspection_summary".to_string(),
                "model_inspection_result".to_string(),
            ],
            deterministic: true,
            cost_class: CapabilityCostClass::Cheap,
            maturity: CapabilityMaturity::Prototype,
        }
    }

    fn readiness(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        target: &CapabilityTarget,
    ) -> CapabilityReadinessReport {
        if workspace.graph.elements().is_empty() {
            return readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::NotApplicable,
                "workspace has no semantic elements",
            );
        }
        match target {
            CapabilityTarget::Element { element_id } if workspace.element(element_id).is_none() => {
                readiness(
                    self.descriptor().id,
                    target.clone(),
                    CapabilityReadinessStatus::Blocked,
                    format!("target element `{element_id}` does not exist"),
                )
            }
            _ => readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::Ready,
                "model and metamodel inspection can run over the selected scope",
            ),
        }
    }

    fn run(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        request: CapabilityRunRequest,
    ) -> Result<CapabilityRunReport, CapabilityError> {
        let readiness = self.readiness(workspace, &request.target);
        if readiness.status == CapabilityReadinessStatus::Blocked
            || readiness.status == CapabilityReadinessStatus::NotApplicable
        {
            return Ok(CapabilityRunReport {
                run_id: request.run_id,
                capability_id: request.capability_id,
                capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
                status: match readiness.status {
                    CapabilityReadinessStatus::NotApplicable => CapabilityRunStatus::NotApplicable,
                    _ => CapabilityRunStatus::Error,
                },
                target: request.target,
                insights: Vec::new(),
                artifacts: Vec::new(),
                evidence: EvidenceGraph::default(),
                diagnostics: Vec::new(),
                limitations: vec![readiness.message],
            });
        }

        let analysis_scope = parameter_analysis_scope(&request);
        let query = parameter_string(&request, "query");
        let limit = parameter_usize(&request, "limit", 8);
        let (insights, payload, evidence) = match &request.target {
            CapabilityTarget::Element { element_id } => {
                inspect_element(workspace, element_id, analysis_scope, &request.run_id)?
            }
            _ => match query.as_deref().filter(|value| !value.trim().is_empty()) {
                Some(query) => {
                    inspect_query(workspace, query, analysis_scope, limit, &request.run_id)
                }
                None => {
                    inspect_workspace_summary(workspace, analysis_scope, limit, &request.run_id)
                }
            },
        };
        let artifact = SemanticArtifact {
            id: format!("artifact.{}.inspection", request.run_id),
            kind: if query.is_some() || matches!(request.target, CapabilityTarget::Element { .. }) {
                "model_inspection_result".to_string()
            } else {
                "model_inspection_summary".to_string()
            },
            schema: "mercurio.capability.model_inspection.v1".to_string(),
            digest: value_digest(&payload),
            element_refs: insights
                .iter()
                .map(|insight| insight.subject.clone())
                .collect(),
            payload,
        };

        Ok(CapabilityRunReport {
            run_id: request.run_id,
            capability_id: request.capability_id,
            capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
            status: CapabilityRunStatus::Passed,
            target: request.target,
            insights,
            artifacts: vec![artifact],
            evidence,
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        })
    }
}

impl SemanticCapability for GenericAnalysisOpportunityCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: "foundation.analysis.opportunities".to_string(),
            name: "Semantic Analysis Opportunity Discovery".to_string(),
            kind: CapabilityKind::Custom,
            profile_id: None,
            target_kinds: Vec::new(),
            relationship_kinds: Vec::new(),
            input_artifact_kinds: Vec::new(),
            produced_insight_kinds: vec![InsightKind::RecommendedNextAction],
            produced_artifact_kinds: vec!["analysis_opportunity_report".to_string()],
            deterministic: true,
            cost_class: CapabilityCostClass::Cheap,
            maturity: CapabilityMaturity::Prototype,
        }
    }

    fn readiness(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        target: &CapabilityTarget,
    ) -> CapabilityReadinessReport {
        if workspace.graph.elements().is_empty() {
            return readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::NotApplicable,
                "workspace has no semantic elements",
            );
        }

        let report = AnalysisInventory::from_graph(&workspace.graph).opportunities();
        if report.opportunities.is_empty() {
            readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::NotApplicable,
                "workspace has no analysis cases, constraints, requirements, or executable behavior opportunities",
            )
        } else {
            readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::Ready,
                format!(
                    "{} semantic analysis opportunity(s) are available",
                    report.opportunities.len()
                ),
            )
        }
    }

    fn run(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        request: CapabilityRunRequest,
    ) -> Result<CapabilityRunReport, CapabilityError> {
        let readiness = self.readiness(workspace, &request.target);
        let report = AnalysisInventory::from_graph(&workspace.graph).opportunities();
        let payload = serde_json::to_value(&report)
            .map_err(|err| CapabilityError::Execution(err.to_string()))?;
        let mut element_refs = Vec::new();
        let mut seen_elements = BTreeSet::new();
        for opportunity in &report.opportunities {
            for element in &opportunity.elements {
                if seen_elements.insert(element.element_id.clone()) {
                    element_refs.push(workspace.element_ref(&element.element_id));
                }
            }
        }
        let insights = report
            .opportunities
            .iter()
            .take(12)
            .map(|opportunity| {
                let subject = opportunity
                    .elements
                    .first()
                    .map(|element| workspace.element_ref(&element.element_id))
                    .unwrap_or_else(|| {
                        SemanticElementRef::new("workspace").with_label("workspace")
                    });
                SemanticInsight {
                    id: format!("{}.{}", request.run_id, opportunity.id),
                    kind: InsightKind::RecommendedNextAction,
                    subject,
                    claim: format!("{}: {}", opportunity.label, opportunity.description),
                    polarity: InsightPolarity::Supports,
                    severity: InsightSeverity::Info,
                    confidence: InsightConfidence::High,
                    scope: InsightScope::Workspace,
                    evidence_ids: Vec::new(),
                    source_spans: Vec::new(),
                    metrics: BTreeMap::from([(
                        "runnable".to_string(),
                        Value::Bool(opportunity.runnable),
                    )]),
                    assumptions: Vec::new(),
                    limitations: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        let artifact = SemanticArtifact {
            id: format!("artifact.{}.analysis_opportunities", request.run_id),
            kind: "analysis_opportunity_report".to_string(),
            schema: "mercurio.capability.analysis_opportunities.v1".to_string(),
            digest: value_digest(&payload),
            element_refs,
            payload,
        };

        Ok(CapabilityRunReport {
            run_id: request.run_id,
            capability_id: request.capability_id,
            capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
            status: match readiness.status {
                CapabilityReadinessStatus::NotApplicable => CapabilityRunStatus::NotApplicable,
                CapabilityReadinessStatus::Blocked | CapabilityReadinessStatus::Error => {
                    CapabilityRunStatus::Error
                }
                _ if report.opportunities.is_empty() => CapabilityRunStatus::Inconclusive,
                _ => CapabilityRunStatus::Passed,
            },
            target: request.target,
            insights,
            artifacts: vec![artifact],
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: if readiness.status == CapabilityReadinessStatus::Ready {
                Vec::new()
            } else {
                vec![readiness.message]
            },
        })
    }
}

impl SemanticCapability for GenericImpactCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: "foundation.impact.graph".to_string(),
            name: "Generic Graph Impact".to_string(),
            kind: CapabilityKind::ImpactAnalysis,
            profile_id: None,
            target_kinds: Vec::new(),
            relationship_kinds: Vec::new(),
            input_artifact_kinds: Vec::new(),
            produced_insight_kinds: vec![
                InsightKind::ImpactHotspot,
                InsightKind::AffectedElement,
                InsightKind::DependencyClosure,
            ],
            produced_artifact_kinds: vec!["graph_impact_summary".to_string()],
            deterministic: true,
            cost_class: CapabilityCostClass::Cheap,
            maturity: CapabilityMaturity::Prototype,
        }
    }

    fn readiness(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        target: &CapabilityTarget,
    ) -> CapabilityReadinessReport {
        if workspace.graph.elements().is_empty() {
            return readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::NotApplicable,
                "workspace has no semantic elements",
            );
        }
        match target {
            CapabilityTarget::Element { element_id } if workspace.element(element_id).is_none() => {
                readiness(
                    self.descriptor().id,
                    target.clone(),
                    CapabilityReadinessStatus::Blocked,
                    format!("target element `{element_id}` does not exist"),
                )
            }
            _ => readiness(
                self.descriptor().id,
                target.clone(),
                CapabilityReadinessStatus::Ready,
                "graph impact can run over the selected scope",
            ),
        }
    }

    fn run(
        &self,
        workspace: &SemanticWorkspaceSnapshot,
        request: CapabilityRunRequest,
    ) -> Result<CapabilityRunReport, CapabilityError> {
        let readiness = self.readiness(workspace, &request.target);
        if readiness.status == CapabilityReadinessStatus::Blocked
            || readiness.status == CapabilityReadinessStatus::NotApplicable
        {
            return Ok(CapabilityRunReport {
                run_id: request.run_id,
                capability_id: request.capability_id,
                capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
                status: match readiness.status {
                    CapabilityReadinessStatus::NotApplicable => CapabilityRunStatus::NotApplicable,
                    _ => CapabilityRunStatus::Error,
                },
                target: request.target,
                insights: Vec::new(),
                artifacts: Vec::new(),
                evidence: EvidenceGraph::default(),
                diagnostics: Vec::new(),
                limitations: vec![readiness.message],
            });
        }

        let analysis_scope = parameter_analysis_scope_or(&request, AnalysisScope::AuthoredModel);
        let (insights, payload, evidence) = match &request.target {
            CapabilityTarget::Element { element_id } => {
                graph_impact_for_element(workspace, element_id, analysis_scope)?
            }
            _ => graph_impact_for_workspace(
                workspace,
                analysis_scope,
                parameter_usize(&request, "limit", 5),
            ),
        };
        let artifact = SemanticArtifact {
            id: format!("artifact.{}.impact", request.run_id),
            kind: "graph_impact_summary".to_string(),
            schema: "mercurio.capability.graph_impact.v1".to_string(),
            digest: value_digest(&payload),
            element_refs: insights
                .iter()
                .map(|insight| insight.subject.clone())
                .collect(),
            payload,
        };

        Ok(CapabilityRunReport {
            run_id: request.run_id,
            capability_id: request.capability_id,
            capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
            status: if insights.is_empty() {
                CapabilityRunStatus::Inconclusive
            } else {
                CapabilityRunStatus::Partial
            },
            target: request.target,
            insights,
            artifacts: vec![artifact],
            evidence,
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        })
    }
}

fn readiness(
    capability_id: String,
    target: CapabilityTarget,
    status: CapabilityReadinessStatus,
    message: impl Into<String>,
) -> CapabilityReadinessReport {
    CapabilityReadinessReport {
        capability_id,
        target,
        status,
        message: message.into(),
        required_inputs: Vec::new(),
        limitations: Vec::new(),
    }
}

fn graph_impact_for_workspace(
    workspace: &SemanticWorkspaceSnapshot,
    analysis_scope: AnalysisScope,
    limit: usize,
) -> (Vec<SemanticInsight>, Value, EvidenceGraph) {
    let mut degree_by_node = BTreeMap::<u32, usize>::new();
    for edge in workspace.graph.edges() {
        *degree_by_node.entry(edge.source).or_default() += 1;
        *degree_by_node.entry(edge.target).or_default() += 1;
    }
    let mut ranked = degree_by_node
        .into_iter()
        .filter_map(|(node_id, degree)| {
            let element = workspace.graph.element(node_id)?;
            if !element_matches_analysis_scope(element, analysis_scope) {
                return None;
            }
            Some((element.element_id.clone(), degree))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ranked.truncate(limit);

    let mut evidence = EvidenceGraph::default();
    let insights = ranked
        .iter()
        .enumerate()
        .map(|(index, (element_id, degree))| {
            let evidence_id = format!("evidence.impact.hotspot.{element_id}");
            let subject = workspace.element_ref(element_id);
            evidence.nodes.push(EvidenceNode {
                id: evidence_id.clone(),
                kind: EvidenceNodeKind::Fact,
                label: format!("{element_id} has {degree} incident graph edges"),
                element_refs: vec![subject.clone()],
                source_spans: workspace.source_spans(element_id),
                properties: BTreeMap::from([("degree".to_string(), Value::from(*degree))]),
            });
            SemanticInsight {
                id: format!("insight.impact.hotspot.{}", index + 1),
                kind: InsightKind::ImpactHotspot,
                subject,
                claim: format!(
                    "`{element_id}` is a graph impact hotspot with {degree} incident edges."
                ),
                polarity: InsightPolarity::Neutral,
                severity: if *degree >= 10 {
                    InsightSeverity::Warning
                } else {
                    InsightSeverity::Info
                },
                confidence: InsightConfidence::Medium,
                scope: InsightScope::Workspace,
                evidence_ids: vec![evidence_id],
                source_spans: workspace.source_spans(element_id),
                metrics: BTreeMap::from([("degree".to_string(), Value::from(*degree))]),
                assumptions: Vec::new(),
                limitations: vec![
                    "generic graph impact does not apply profile-specific semantics".to_string(),
                ],
            }
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema": "mercurio.capability.graph_impact.v1",
        "scope": "workspace",
        "analysisScope": analysis_scope.as_str(),
        "filteredOutOfScopeElementCount": workspace
            .graph
            .elements()
            .iter()
            .filter(|element| !element_matches_analysis_scope(element, analysis_scope))
            .count(),
        "filteredLibraryElementCount": workspace
            .graph
            .elements()
            .iter()
            .filter(|element| !is_authored_model_element(element))
            .count(),
        "hotspots": ranked
            .iter()
            .map(|(element_id, degree)| json!({ "elementId": element_id, "degree": degree }))
            .collect::<Vec<_>>(),
    });

    (insights, payload, evidence)
}

fn graph_impact_for_element(
    workspace: &SemanticWorkspaceSnapshot,
    element_id: &str,
    analysis_scope: AnalysisScope,
) -> Result<(Vec<SemanticInsight>, Value, EvidenceGraph), CapabilityError> {
    let node = workspace.graph.node_id(element_id).ok_or_else(|| {
        CapabilityError::InvalidRequest(format!("unknown element `{element_id}`"))
    })?;
    let element = workspace.graph.element(node).ok_or_else(|| {
        CapabilityError::InvalidRequest(format!("unknown element `{element_id}`"))
    })?;
    if !element_matches_analysis_scope(element, analysis_scope) {
        return Err(CapabilityError::InvalidRequest(format!(
            "element `{element_id}` is outside analysis scope `{}`",
            analysis_scope.as_str()
        )));
    }
    let outgoing = workspace.graph.outgoing_edges(node).collect::<Vec<_>>();
    let incoming = workspace.graph.incoming_edges(node).collect::<Vec<_>>();
    let degree = outgoing.len() + incoming.len();
    let subject = workspace.element_ref(element_id);
    let evidence_id = format!("evidence.impact.element.{element_id}");
    let insight = SemanticInsight {
        id: format!("insight.impact.element.{element_id}"),
        kind: InsightKind::AffectedElement,
        subject: subject.clone(),
        claim: format!("`{element_id}` has {degree} direct semantic graph connections."),
        polarity: InsightPolarity::Neutral,
        severity: if degree >= 10 {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        },
        confidence: InsightConfidence::Medium,
        scope: InsightScope::Element {
            element_id: element_id.to_string(),
        },
        evidence_ids: vec![evidence_id.clone()],
        source_spans: workspace.source_spans(element_id),
        metrics: BTreeMap::from([
            ("incoming_edges".to_string(), Value::from(incoming.len())),
            ("outgoing_edges".to_string(), Value::from(outgoing.len())),
            ("degree".to_string(), Value::from(degree)),
        ]),
        assumptions: Vec::new(),
        limitations: vec!["generic graph impact reports direct KIR references only".to_string()],
    };
    let payload = json!({
        "schema": "mercurio.capability.graph_impact.v1",
        "scope": "element",
        "analysisScope": analysis_scope.as_str(),
        "elementId": element_id,
        "incoming": edge_payload(workspace, &incoming),
        "outgoing": edge_payload(workspace, &outgoing),
    });
    let evidence = EvidenceGraph {
        nodes: vec![EvidenceNode {
            id: evidence_id,
            kind: EvidenceNodeKind::Fact,
            label: format!("{element_id} direct graph connections"),
            element_refs: vec![subject],
            source_spans: workspace.source_spans(element_id),
            properties: BTreeMap::from([
                ("incoming_edges".to_string(), Value::from(incoming.len())),
                ("outgoing_edges".to_string(), Value::from(outgoing.len())),
            ]),
        }],
        edges: Vec::new(),
    };

    Ok((vec![insight], payload, evidence))
}

fn inspect_workspace_summary(
    workspace: &SemanticWorkspaceSnapshot,
    analysis_scope: AnalysisScope,
    limit: usize,
    run_id: &str,
) -> (Vec<SemanticInsight>, Value, EvidenceGraph) {
    let authored_count = workspace
        .graph
        .elements()
        .iter()
        .filter(|element| is_authored_model_element(element))
        .count();
    let library_count = workspace
        .graph
        .elements()
        .len()
        .saturating_sub(authored_count);
    let metamodel_count = workspace
        .graph
        .elements()
        .iter()
        .filter(|element| {
            string_property(element, "metamodel_layer").is_some()
                || string_property(element, "metamodel_language").is_some()
                || matches!(element.kind.as_ref(), "Metaclass" | "MetamodelFeature")
        })
        .count();
    let mut authored_kind_counts = BTreeMap::<String, usize>::new();
    for element in workspace.graph.elements() {
        if is_authored_model_element(element) {
            *authored_kind_counts
                .entry(element.kind.to_string())
                .or_default() += 1;
        }
    }
    let scoped_count = workspace
        .graph
        .elements()
        .iter()
        .filter(|element| element_matches_analysis_scope(element, analysis_scope))
        .count();
    let mut scoped_kind_counts = BTreeMap::<String, usize>::new();
    for element in workspace.graph.elements() {
        if element_matches_analysis_scope(element, analysis_scope) {
            *scoped_kind_counts
                .entry(element.kind.to_string())
                .or_default() += 1;
        }
    }
    let top_kinds = authored_kind_counts
        .into_iter()
        .map(|(kind, count)| json!({ "kind": kind, "count": count }))
        .take(limit)
        .collect::<Vec<_>>();
    let top_scoped_kinds = scoped_kind_counts
        .into_iter()
        .map(|(kind, count)| json!({ "kind": kind, "count": count }))
        .take(limit)
        .collect::<Vec<_>>();
    let payload = json!({
        "schema": "mercurio.capability.model_inspection.v1",
        "scope": "workspace",
        "analysisScope": analysis_scope.as_str(),
        "elementCount": workspace.graph.elements().len(),
        "scopedElementCount": scoped_count,
        "authoredElementCount": authored_count,
        "libraryElementCount": library_count,
        "metamodelElementCount": metamodel_count,
        "relationshipCount": workspace.graph.edges().len(),
        "topAuthoredKinds": top_kinds,
        "topScopedKinds": top_scoped_kinds,
    });
    let evidence_id = format!("evidence.{run_id}.inspection.workspace");
    let evidence = EvidenceGraph {
        nodes: vec![EvidenceNode {
            id: evidence_id.clone(),
            kind: EvidenceNodeKind::Fact,
            label: "Workspace model inspection inventory".to_string(),
            element_refs: Vec::new(),
            source_spans: Vec::new(),
            properties: BTreeMap::from([
                (
                    "elementCount".to_string(),
                    Value::from(workspace.graph.elements().len()),
                ),
                ("scopedElementCount".to_string(), Value::from(scoped_count)),
                (
                    "authoredElementCount".to_string(),
                    Value::from(authored_count),
                ),
                (
                    "libraryElementCount".to_string(),
                    Value::from(library_count),
                ),
                (
                    "metamodelElementCount".to_string(),
                    Value::from(metamodel_count),
                ),
            ]),
        }],
        edges: Vec::new(),
    };
    let insight = SemanticInsight {
        id: format!("insight.{run_id}.inspection.workspace"),
        kind: InsightKind::TraceCompleteness,
        subject: SemanticElementRef::new("workspace").with_label("Workspace"),
        claim: format!(
            "Workspace inspection found {scoped_count} elements in analysis scope `{}` from {authored_count} authored elements, {library_count} library elements, and {metamodel_count} metamodel elements.",
            analysis_scope.as_str()
        ),
        polarity: InsightPolarity::Neutral,
        severity: InsightSeverity::Info,
        confidence: InsightConfidence::High,
        scope: InsightScope::Workspace,
        evidence_ids: vec![evidence_id],
        source_spans: Vec::new(),
        metrics: BTreeMap::from([
            (
                "scoped_element_count".to_string(),
                Value::from(scoped_count),
            ),
            (
                "analysis_scope".to_string(),
                Value::String(analysis_scope.as_str().to_string()),
            ),
            (
                "authored_element_count".to_string(),
                Value::from(authored_count),
            ),
            (
                "library_element_count".to_string(),
                Value::from(library_count),
            ),
            (
                "metamodel_element_count".to_string(),
                Value::from(metamodel_count),
            ),
        ]),
        assumptions: Vec::new(),
        limitations: Vec::new(),
    };
    (vec![insight], payload, evidence)
}

fn inspect_query(
    workspace: &SemanticWorkspaceSnapshot,
    query: &str,
    analysis_scope: AnalysisScope,
    limit: usize,
    run_id: &str,
) -> (Vec<SemanticInsight>, Value, EvidenceGraph) {
    let terms = inspection_query_terms(query);
    let mut matches = Vec::new();
    for term in &terms {
        matches.extend(inspection_name_matches(workspace, term));
    }
    matches.sort_by(|left, right| {
        inspection_match_rank(left, &terms)
            .cmp(&inspection_match_rank(right, &terms))
            .then_with(|| left.element_id.cmp(&right.element_id))
    });
    matches.dedup_by(|left, right| left.element_id == right.element_id);
    matches.retain(|element| element_matches_analysis_scope(element, analysis_scope));
    matches.truncate(limit);

    let mut evidence = EvidenceGraph::default();
    let mut insights = Vec::new();
    let mut result_payloads = Vec::new();
    for (index, element) in matches.iter().enumerate() {
        let evidence_id = format!("evidence.{run_id}.inspection.match.{}", index + 1);
        let subject = element_ref(element);
        let inspection = element_inspection_payload(workspace, element);
        evidence.nodes.push(EvidenceNode {
            id: evidence_id.clone(),
            kind: EvidenceNodeKind::KirElement,
            label: format!("Inspection match {}", element.element_id),
            element_refs: vec![subject.clone()],
            source_spans: workspace.source_spans(&element.element_id),
            properties: BTreeMap::from([
                ("query".to_string(), Value::String(query.to_string())),
                (
                    "elementId".to_string(),
                    Value::String(element.element_id.clone()),
                ),
                (
                    "analysisScope".to_string(),
                    Value::String(analysis_scope.as_str().to_string()),
                ),
            ]),
        });
        insights.push(SemanticInsight {
            id: format!("insight.{run_id}.inspection.match.{}", index + 1),
            kind: InsightKind::AffectedElement,
            subject,
            claim: format!(
                "Inspection query `{query}` matched `{}` from the compiled KIR graph.",
                element.element_id
            ),
            polarity: InsightPolarity::Neutral,
            severity: InsightSeverity::Info,
            confidence: InsightConfidence::High,
            scope: InsightScope::Workspace,
            evidence_ids: vec![evidence_id],
            source_spans: workspace.source_spans(&element.element_id),
            metrics: BTreeMap::new(),
            assumptions: Vec::new(),
            limitations: Vec::new(),
        });
        result_payloads.push(inspection);
    }

    if insights.is_empty() {
        let evidence_id = format!("evidence.{run_id}.inspection.no_match");
        evidence.nodes.push(EvidenceNode {
            id: evidence_id.clone(),
            kind: EvidenceNodeKind::Fact,
            label: format!("Inspection query `{query}` matched no elements"),
            element_refs: Vec::new(),
            source_spans: Vec::new(),
            properties: BTreeMap::from([("query".to_string(), Value::String(query.to_string()))]),
        });
        insights.push(SemanticInsight {
            id: format!("insight.{run_id}.inspection.no_match"),
            kind: InsightKind::MissingEvidence,
            subject: SemanticElementRef::new("workspace").with_label("Workspace"),
            claim: format!("Inspection query `{query}` matched no compiled KIR elements."),
            polarity: InsightPolarity::Neutral,
            severity: InsightSeverity::Info,
            confidence: InsightConfidence::High,
            scope: InsightScope::Workspace,
            evidence_ids: vec![evidence_id],
            source_spans: Vec::new(),
            metrics: BTreeMap::new(),
            assumptions: Vec::new(),
            limitations: Vec::new(),
        });
    }

    let payload = json!({
        "schema": "mercurio.capability.model_inspection.v1",
        "scope": "query",
        "analysisScope": analysis_scope.as_str(),
        "query": query,
        "terms": terms,
        "matchCount": result_payloads.len(),
        "matches": result_payloads,
    });
    (insights, payload, evidence)
}

fn inspect_element(
    workspace: &SemanticWorkspaceSnapshot,
    element_id: &str,
    analysis_scope: AnalysisScope,
    run_id: &str,
) -> Result<(Vec<SemanticInsight>, Value, EvidenceGraph), CapabilityError> {
    let element = workspace.element(element_id).ok_or_else(|| {
        CapabilityError::InvalidRequest(format!("unknown element `{element_id}`"))
    })?;
    if !element_matches_analysis_scope(element, analysis_scope) {
        return Err(CapabilityError::InvalidRequest(format!(
            "element `{element_id}` is outside analysis scope `{}`",
            analysis_scope.as_str()
        )));
    }
    let payload = json!({
        "schema": "mercurio.capability.model_inspection.v1",
        "scope": "element",
        "analysisScope": analysis_scope.as_str(),
        "element": element_inspection_payload(workspace, element),
    });
    let subject = element_ref(element);
    let evidence_id = format!("evidence.{run_id}.inspection.element.{element_id}");
    let evidence = EvidenceGraph {
        nodes: vec![EvidenceNode {
            id: evidence_id.clone(),
            kind: EvidenceNodeKind::KirElement,
            label: format!("Inspection for {element_id}"),
            element_refs: vec![subject.clone()],
            source_spans: workspace.source_spans(element_id),
            properties: BTreeMap::from([(
                "elementId".to_string(),
                Value::String(element_id.to_string()),
            )]),
        }],
        edges: Vec::new(),
    };
    let insight = SemanticInsight {
        id: format!("insight.{run_id}.inspection.element.{element_id}"),
        kind: InsightKind::AffectedElement,
        subject,
        claim: format!("Inspection resolved `{element_id}` in the compiled KIR graph."),
        polarity: InsightPolarity::Neutral,
        severity: InsightSeverity::Info,
        confidence: InsightConfidence::High,
        scope: InsightScope::Element {
            element_id: element_id.to_string(),
        },
        evidence_ids: vec![evidence_id],
        source_spans: workspace.source_spans(element_id),
        metrics: BTreeMap::new(),
        assumptions: Vec::new(),
        limitations: Vec::new(),
    };
    Ok((vec![insight], payload, evidence))
}

fn element_inspection_payload(
    workspace: &SemanticWorkspaceSnapshot,
    element: &crate::model::Element,
) -> Value {
    let ancestors = collect_specialization_ancestors(&workspace.graph, element.id);
    let declared_attributes = workspace
        .metamodel_registry
        .declared_attributes_for(&element.element_id)
        .iter()
        .map(metamodel_attribute_payload)
        .collect::<Vec<_>>();
    let inherited_attributes = ancestors
        .iter()
        .flat_map(|ancestor| {
            workspace
                .metamodel_registry
                .declared_attributes_for(&ancestor.element_id)
                .iter()
                .map(metamodel_attribute_payload)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let incoming = workspace
        .graph
        .incoming_edges(element.id)
        .take(12)
        .collect::<Vec<_>>();
    let outgoing = workspace
        .graph
        .outgoing_edges(element.id)
        .take(12)
        .collect::<Vec<_>>();
    json!({
        "id": element.element_id,
        "label": element_ref(element).label,
        "qualifiedName": string_property(element, "qualified_name"),
        "kind": element.kind.as_ref(),
        "layer": element.layer,
        "scope": if is_authored_model_element(element) { "authored_model" } else { "library_or_metamodel" },
        "declaredAttributes": declared_attributes,
        "inheritedAttributes": inherited_attributes,
        "specializationAncestors": ancestors
            .iter()
            .map(|ancestor| json!({
                "id": ancestor.element_id,
                "label": element_ref(ancestor).label,
                "kind": ancestor.kind.as_ref(),
            }))
            .collect::<Vec<_>>(),
        "incoming": edge_payload(workspace, &incoming),
        "outgoing": edge_payload(workspace, &outgoing),
    })
}

fn metamodel_attribute_payload(attribute: &MetamodelAttributeDeclaration) -> Value {
    json!({
        "name": attribute.name,
        "declaredBy": attribute.declared_by.id,
        "typeLabel": attribute.type_label,
        "featureKind": attribute.feature_kind,
        "multiplicityLower": attribute.multiplicity_lower,
        "multiplicityUpper": attribute.multiplicity_upper,
    })
}

fn edge_payload(workspace: &SemanticWorkspaceSnapshot, edges: &[&Edge]) -> Vec<Value> {
    edges
        .iter()
        .filter_map(|edge| {
            Some(json!({
                "source": workspace.graph.element_id(edge.source)?,
                "relation": edge.relation.as_ref(),
                "target": workspace.graph.element_id(edge.target)?,
            }))
        })
        .collect()
}

fn parameter_usize(request: &CapabilityRunRequest, key: &str, default: usize) -> usize {
    request
        .parameters
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(default)
}

fn parameter_string(request: &CapabilityRunRequest, key: &str) -> Option<String> {
    request
        .parameters
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parameter_analysis_scope(request: &CapabilityRunRequest) -> AnalysisScope {
    parameter_analysis_scope_or(request, AnalysisScope::All)
}

fn parameter_analysis_scope_or(
    request: &CapabilityRunRequest,
    default: AnalysisScope,
) -> AnalysisScope {
    parameter_string(request, "analysis_scope")
        .or_else(|| parameter_string(request, "analysisScope"))
        .and_then(|value| analysis_scope_from_str(&value))
        .unwrap_or(default)
}

fn analysis_scope_from_str(value: &str) -> Option<AnalysisScope> {
    match value.trim().to_ascii_lowercase().as_str() {
        "authored_model" | "authored" | "model" | "user_model" => {
            Some(AnalysisScope::AuthoredModel)
        }
        "stdlib" | "library" | "libraries" | "standard_library" => Some(AnalysisScope::Stdlib),
        "metamodel" | "meta_model" => Some(AnalysisScope::Metamodel),
        "all" | "workspace" => Some(AnalysisScope::All),
        _ => None,
    }
}

fn inspection_query_terms(query: &str) -> Vec<String> {
    let mut terms = query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != ':' && ch != '.' && ch != '_')
        .map(str::trim)
        .filter(|term| term.len() >= 3)
        .filter(|term| {
            term.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
                || term.contains("::")
                || term.contains('.')
                || term.contains('_')
        })
        .map(|term| term.trim_matches('.').to_string())
        .collect::<Vec<_>>();
    if query.to_ascii_lowercase().contains("element") && !terms.iter().any(|term| term == "Element")
    {
        terms.push("Element".to_string());
    }
    terms.sort();
    terms.dedup();
    terms.truncate(12);
    terms
}

fn inspection_name_matches<'a>(
    workspace: &'a SemanticWorkspaceSnapshot,
    term: &str,
) -> Vec<&'a crate::model::Element> {
    let normalized = normalize_inspection_name(term);
    workspace
        .graph
        .elements()
        .iter()
        .filter(|element| {
            let id = normalize_inspection_name(&element.element_id);
            let label = element_ref(element).label.unwrap_or_default();
            id == normalized
                || id.ends_with(&format!(".{normalized}"))
                || label == term
                || string_property(element, "qualified_name").is_some_and(|qualified_name| {
                    let qualified = normalize_inspection_name(&qualified_name);
                    qualified == normalized || qualified.ends_with(&format!(".{normalized}"))
                })
        })
        .collect()
}

fn inspection_match_rank(element: &crate::model::Element, terms: &[String]) -> u8 {
    if terms
        .iter()
        .any(|term| element.element_id == format!("KerML::Root::{term}"))
    {
        0
    } else if terms.iter().any(|term| element.element_id == *term) {
        1
    } else if element.element_id.contains("::Root::") {
        2
    } else if !is_authored_model_element(element) {
        3
    } else {
        4
    }
}

fn normalize_inspection_name(value: &str) -> String {
    value.trim().replace("::", ".")
}

fn element_ref(element: &crate::model::Element) -> SemanticElementRef {
    SemanticElementRef::from_graph_element(element)
}

fn source_span_for_element(element: &crate::model::Element) -> Option<SourceSpanRef> {
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

fn string_property(element: &crate::model::Element, key: &str) -> Option<String> {
    element
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(|metadata| metadata.get(key))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn bool_property(element: &crate::model::Element, key: &str) -> Option<bool> {
    element
        .properties
        .get(key)
        .and_then(Value::as_bool)
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(|metadata| metadata.get(key))
                .and_then(Value::as_bool)
        })
}

fn is_authored_model_element(element: &crate::model::Element) -> bool {
    if bool_property(element, "is_library_element")
        .or_else(|| bool_property(element, "isLibraryElement"))
        .unwrap_or(false)
    {
        return false;
    }

    if string_property(element, "pilot_library_group").is_some()
        || string_property(element, "library_group").is_some()
        || string_property(element, "metamodel_layer").is_some()
        || string_property(element, "metamodel_language").is_some()
    {
        return false;
    }

    if let Some(source_file) = string_property(element, "source_file") {
        let normalized = source_file.replace('\\', "/");
        if normalized.starts_with("Kernel Libraries/")
            || normalized.starts_with("System Libraries/")
            || normalized.starts_with("Domain Libraries/")
        {
            return false;
        }
    }

    true
}

fn is_metamodel_element(element: &crate::model::Element) -> bool {
    string_property(element, "metamodel_layer").is_some()
        || string_property(element, "metamodel_language").is_some()
        || matches!(element.kind.as_ref(), "Metaclass" | "MetamodelFeature")
        || element.element_id.starts_with("KerML::Root::")
}

fn is_stdlib_element(element: &crate::model::Element) -> bool {
    !is_authored_model_element(element) && !is_metamodel_element(element)
}

fn element_matches_analysis_scope(
    element: &crate::model::Element,
    analysis_scope: AnalysisScope,
) -> bool {
    match analysis_scope {
        AnalysisScope::AuthoredModel => is_authored_model_element(element),
        AnalysisScope::Stdlib => is_stdlib_element(element),
        AnalysisScope::Metamodel => is_metamodel_element(element),
        AnalysisScope::All => true,
    }
}

fn value_digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    crate::semantic_services::stable_digest([("semantic-artifact".as_bytes(), bytes.as_slice())])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchConfidence {
    High,
    Medium,
    Low,
}

/// Output of a mutate-effect capability run.
/// Apply by passing `proposal` to CoreMutationFeasibilityService::apply_checked_plan()
/// then write_back_mutation() in mercurio-authoring/src/authoring.rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityModelPatch {
    pub capability_id: String,
    pub run_id: String,
    pub summary: String,
    pub rationale: String,
    pub confidence: PatchConfidence,
    /// Serialized MutationProposal — kept as Value to avoid dependency cycles.
    pub proposal: serde_json::Value,
    /// Pre-computed SemanticDiff for UI review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_diff: Option<serde_json::Value>,
    #[serde(default)]
    pub reversible: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::kir::{KirDocument, KirElement};

    use super::*;

    #[test]
    fn registry_lists_and_runs_graph_impact() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "pkg.Demo".to_string(),
                        kind: "Model::Package".to_string(),
                        layer: 2,
                        properties: BTreeMap::from([(
                            "members".to_string(),
                            Value::Array(vec![Value::String("type.Vehicle".to_string())]),
                        )]),
                    },
                    KirElement {
                        id: "type.Vehicle".to_string(),
                        kind: "Model::PartDefinition".to_string(),
                        layer: 2,
                        properties: BTreeMap::new(),
                    },
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        assert!(
            registry
                .list()
                .iter()
                .any(|descriptor| descriptor.id == "foundation.impact.graph")
        );
        assert!(
            registry
                .list()
                .iter()
                .any(|descriptor| descriptor.id == "foundation.inspect.model")
        );

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.impact".to_string(),
                    capability_id: "foundation.impact.graph".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::new(),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert!(!report.insights.is_empty());
        assert_eq!(report.artifacts[0].kind, "graph_impact_summary");
    }

    #[test]
    fn model_inspection_resolves_metamodel_element_attributes() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "KerML::Root::Element".to_string(),
                        kind: "Metaclass".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([(
                            "declared_name".to_string(),
                            Value::String("Element".to_string()),
                        )]),
                    },
                    metamodel_feature(
                        "KerML::Root::Element::declaredName",
                        "KerML::Root::Element",
                        "declaredName",
                        "declared_name",
                        Some("String"),
                    ),
                    metamodel_feature(
                        "KerML::Root::Element::ownedElement",
                        "KerML::Root::Element",
                        "ownedElement",
                        "ownedElement",
                        None,
                    ),
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.inspect".to_string(),
                    capability_id: "foundation.inspect.model".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::from([
                        (
                            "query".to_string(),
                            Value::String("What are Element's attributes?".to_string()),
                        ),
                        ("limit".to_string(), Value::from(4)),
                    ]),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(report.status, CapabilityRunStatus::Passed);
        assert_eq!(
            report.artifacts[0].schema,
            "mercurio.capability.model_inspection.v1"
        );
        assert_eq!(
            report.artifacts[0].payload["matches"][0]["id"],
            "KerML::Root::Element"
        );
        let attributes = report.artifacts[0].payload["matches"][0]["declaredAttributes"]
            .as_array()
            .unwrap();
        assert!(
            attributes
                .iter()
                .any(|attribute| attribute["name"] == "declared_name")
        );
        assert!(
            attributes
                .iter()
                .any(|attribute| attribute["name"] == "ownedElement")
        );
    }

    #[test]
    fn model_inspection_analysis_scope_filters_workspace_inventory() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "Demo::Vehicle".to_string(),
                        kind: "PartDefinition".to_string(),
                        layer: 0,
                        properties: BTreeMap::from([(
                            "declared_name".to_string(),
                            Value::String("Vehicle".to_string()),
                        )]),
                    },
                    KirElement {
                        id: "ScalarValues::Real".to_string(),
                        kind: "DataType".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([
                            (
                                "declared_name".to_string(),
                                Value::String("Real".to_string()),
                            ),
                            ("is_library_element".to_string(), Value::Bool(true)),
                            (
                                "pilot_library_group".to_string(),
                                Value::String("Kernel Libraries".to_string()),
                            ),
                        ]),
                    },
                    KirElement {
                        id: "KerML::Root::Element".to_string(),
                        kind: "Metaclass".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([
                            (
                                "declared_name".to_string(),
                                Value::String("Element".to_string()),
                            ),
                            (
                                "metamodel_layer".to_string(),
                                Value::String("kernel".to_string()),
                            ),
                        ]),
                    },
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.inspect.authored".to_string(),
                    capability_id: "foundation.inspect.model".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::from([(
                        "analysis_scope".to_string(),
                        Value::String("authored_model".to_string()),
                    )]),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(
            report.artifacts[0].payload["analysisScope"],
            "authored_model"
        );
        assert_eq!(report.artifacts[0].payload["elementCount"], Value::from(3));
        assert_eq!(
            report.artifacts[0].payload["scopedElementCount"],
            Value::from(1)
        );
        assert_eq!(
            report.artifacts[0].payload["topScopedKinds"][0],
            json!({ "kind": "PartDefinition", "count": 1 })
        );
    }

    #[test]
    fn model_inspection_query_scope_can_target_metamodel_only() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "Demo::ElementAdapter".to_string(),
                        kind: "PartDefinition".to_string(),
                        layer: 0,
                        properties: BTreeMap::from([(
                            "declared_name".to_string(),
                            Value::String("ElementAdapter".to_string()),
                        )]),
                    },
                    KirElement {
                        id: "KerML::Root::Element".to_string(),
                        kind: "Metaclass".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([
                            (
                                "declared_name".to_string(),
                                Value::String("Element".to_string()),
                            ),
                            (
                                "metamodel_layer".to_string(),
                                Value::String("kernel".to_string()),
                            ),
                        ]),
                    },
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.inspect.metamodel".to_string(),
                    capability_id: "foundation.inspect.model".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::from([
                        (
                            "query".to_string(),
                            Value::String("What is Element?".to_string()),
                        ),
                        (
                            "analysis_scope".to_string(),
                            Value::String("metamodel".to_string()),
                        ),
                    ]),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(report.artifacts[0].payload["analysisScope"], "metamodel");
        assert_eq!(report.artifacts[0].payload["matchCount"], Value::from(1));
        assert_eq!(
            report.artifacts[0].payload["matches"][0]["id"],
            "KerML::Root::Element"
        );
    }

    #[test]
    fn graph_impact_workspace_hotspots_ignore_library_elements() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "ScalarValues::Real".to_string(),
                        kind: "DataType".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([
                            (
                                "declared_name".to_string(),
                                Value::String("Real".to_string()),
                            ),
                            ("is_library_element".to_string(), Value::Bool(true)),
                            (
                                "pilot_library_group".to_string(),
                                Value::String("Kernel Libraries".to_string()),
                            ),
                        ]),
                    },
                    typed_usage("part.engine.power", "ScalarValues::Real"),
                    typed_usage("part.engine.torque", "ScalarValues::Real"),
                    typed_usage("part.engine.speed", "ScalarValues::Real"),
                    typed_usage("part.engine.temperature", "ScalarValues::Real"),
                    typed_usage("part.engine.efficiency", "ScalarValues::Real"),
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.impact".to_string(),
                    capability_id: "foundation.impact.graph".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::from([("limit".to_string(), Value::from(3))]),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert!(
            report
                .insights
                .iter()
                .all(|insight| insight.subject.element_id != "ScalarValues::Real")
        );
        assert_eq!(
            report.artifacts[0].payload["filteredLibraryElementCount"],
            Value::from(1)
        );
        assert_eq!(
            report.artifacts[0].payload["filteredOutOfScopeElementCount"],
            Value::from(1)
        );
    }

    #[test]
    fn graph_impact_workspace_scope_can_target_stdlib_elements() {
        let workspace = SemanticWorkspaceSnapshot::from_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "ScalarValues::Real".to_string(),
                        kind: "DataType".to_string(),
                        layer: 1,
                        properties: BTreeMap::from([
                            (
                                "declared_name".to_string(),
                                Value::String("Real".to_string()),
                            ),
                            ("is_library_element".to_string(), Value::Bool(true)),
                            (
                                "pilot_library_group".to_string(),
                                Value::String("Kernel Libraries".to_string()),
                            ),
                        ]),
                    },
                    typed_usage("part.engine.power", "ScalarValues::Real"),
                    typed_usage("part.engine.torque", "ScalarValues::Real"),
                    typed_usage("part.engine.speed", "ScalarValues::Real"),
                ],
            }
            .normalized_for_persistence(),
        )
        .unwrap();
        let registry = CapabilityRegistry::with_foundation_builtins();

        let report = registry
            .run(
                &workspace,
                CapabilityRunRequest {
                    run_id: "run.impact.stdlib".to_string(),
                    capability_id: "foundation.impact.graph".to_string(),
                    target: CapabilityTarget::Workspace,
                    parameters: BTreeMap::from([
                        (
                            "analysis_scope".to_string(),
                            Value::String("stdlib".to_string()),
                        ),
                        ("limit".to_string(), Value::from(3)),
                    ]),
                    input_artifacts: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(report.artifacts[0].payload["analysisScope"], "stdlib");
        assert_eq!(
            report.artifacts[0].payload["hotspots"][0]["elementId"],
            "ScalarValues::Real"
        );
        assert_eq!(
            report.artifacts[0].payload["filteredOutOfScopeElementCount"],
            Value::from(3)
        );
        assert!(
            report
                .insights
                .iter()
                .all(|insight| insight.subject.element_id == "ScalarValues::Real")
        );
    }

    #[test]
    fn decision_composition_reports_blocked_missing_evidence() {
        let readiness = vec![CapabilityReadinessReport {
            capability_id: "sysml.constraint.analysis".to_string(),
            target: CapabilityTarget::Workspace,
            status: CapabilityReadinessStatus::Blocked,
            message: "no supported constraint usages were found".to_string(),
            required_inputs: Vec::new(),
            limitations: Vec::new(),
        }];

        let assessment = assess_decision_context(test_decision_context(), &readiness, &[]);

        assert_eq!(assessment.status, CapabilityRunStatus::Inconclusive);
        assert_eq!(
            assessment.missing_evidence,
            vec!["sysml.constraint.analysis: no supported constraint usages were found"]
        );
        assert!(
            assessment
                .recommended_next_actions
                .iter()
                .any(|action| action.contains("Unblock `sysml.constraint.analysis`"))
        );
    }

    #[test]
    fn capability_kind_accepts_legacy_and_open_ids() {
        let legacy: CapabilityDescriptor = serde_json::from_str(
            r#"{
                "id": "foundation.impact.graph",
                "name": "Impact",
                "kind": "impact_analysis",
                "deterministic": true,
                "cost_class": "cheap",
                "maturity": "prototype"
            }"#,
        )
        .expect("legacy kind should deserialize");
        assert_eq!(legacy.kind, CapabilityKind::ImpactAnalysis);

        let custom = CapabilityKind::new("org.example.capability.kind/thermal-analysis");
        assert_eq!(
            custom.as_str(),
            "org.example.capability.kind/thermal-analysis"
        );

        let encoded = serde_json::to_string(&legacy).expect("descriptor serializes");
        assert!(encoded.contains(CAPABILITY_KIND_IMPACT_ANALYSIS));
    }

    #[test]
    fn decision_composition_turns_weakening_insights_into_actions() {
        let report = CapabilityRunReport {
            run_id: "run.requirements".to_string(),
            capability_id: "sysml.requirement.analysis".to_string(),
            capability_version: Some("test".to_string()),
            status: CapabilityRunStatus::Passed,
            target: CapabilityTarget::Workspace,
            insights: vec![test_insight(
                InsightKind::CoverageGap,
                InsightPolarity::Weakens,
                InsightSeverity::Warning,
            )],
            artifacts: Vec::new(),
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        };

        let assessment = assess_decision_context(test_decision_context(), &[], &[report]);

        assert_eq!(assessment.status, CapabilityRunStatus::Partial);
        assert_eq!(assessment.insights.len(), 1);
        assert!(
            assessment
                .recommended_next_actions
                .iter()
                .any(|action| action.contains("satisfy evidence"))
        );
    }

    #[test]
    fn capability_run_report_versions_are_additive() {
        let legacy: CapabilityRunReport = serde_json::from_str(
            r#"{
                "run_id": "run.legacy",
                "capability_id": "foundation.inspect.model",
                "status": "passed",
                "target": { "kind": "workspace" }
            }"#,
        )
        .expect("legacy report without capability_version should deserialize");

        assert_eq!(legacy.capability_version, None);

        let encoded = serde_json::to_value(CapabilityRunReport {
            run_id: "run.current".to_string(),
            capability_id: "foundation.inspect.model".to_string(),
            capability_version: Some(FOUNDATION_CAPABILITY_VERSION.to_string()),
            status: CapabilityRunStatus::Passed,
            target: CapabilityTarget::Workspace,
            insights: Vec::new(),
            artifacts: Vec::new(),
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        })
        .expect("report should serialize");

        assert_eq!(
            encoded["capability_version"],
            serde_json::json!(FOUNDATION_CAPABILITY_VERSION)
        );
    }

    #[test]
    fn decision_composition_fails_on_critical_weakening_evidence() {
        let report = CapabilityRunReport {
            run_id: "run.behavior".to_string(),
            capability_id: "sysml.behavior.dynamic".to_string(),
            capability_version: Some("test".to_string()),
            status: CapabilityRunStatus::Passed,
            target: CapabilityTarget::Workspace,
            insights: vec![test_insight(
                InsightKind::ScenarioFailure,
                InsightPolarity::Weakens,
                InsightSeverity::Critical,
            )],
            artifacts: Vec::new(),
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        };

        let assessment = assess_decision_context(test_decision_context(), &[], &[report]);

        assert_eq!(assessment.status, CapabilityRunStatus::Failed);
        assert!(
            assessment
                .recommended_next_actions
                .iter()
                .any(|action| action.contains("dynamic behavior analysis"))
        );
    }

    #[test]
    fn decision_composition_prioritizes_severe_weakening_evidence() {
        let report = CapabilityRunReport {
            run_id: "run.mixed".to_string(),
            capability_id: "centaur.mixed".to_string(),
            capability_version: Some("test".to_string()),
            status: CapabilityRunStatus::Passed,
            target: CapabilityTarget::Workspace,
            insights: vec![
                test_insight(
                    InsightKind::ImpactHotspot,
                    InsightPolarity::Neutral,
                    InsightSeverity::Warning,
                ),
                test_insight(
                    InsightKind::ScenarioFailure,
                    InsightPolarity::Weakens,
                    InsightSeverity::Error,
                ),
            ],
            artifacts: Vec::new(),
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        };

        let assessment = assess_decision_context(test_decision_context(), &[], &[report]);

        assert_eq!(assessment.status, CapabilityRunStatus::Failed);
        assert_eq!(
            assessment.recommended_next_actions[0],
            "Make the behavior scenario executable and rerun dynamic behavior analysis."
        );
    }

    fn test_decision_context() -> DecisionContext {
        DecisionContext {
            id: "decision.test".to_string(),
            question: "What should the next model edit be?".to_string(),
            alternatives: Vec::new(),
            criteria: Vec::new(),
            assumptions: Vec::new(),
            scenarios: Vec::new(),
            target_elements: Vec::new(),
        }
    }

    fn test_insight(
        kind: InsightKind,
        polarity: InsightPolarity,
        severity: InsightSeverity,
    ) -> SemanticInsight {
        SemanticInsight {
            id: "insight.test".to_string(),
            kind,
            subject: SemanticElementRef::new("element.test").with_label("Test Element"),
            claim: "test insight".to_string(),
            polarity,
            severity,
            confidence: InsightConfidence::Medium,
            scope: InsightScope::Workspace,
            evidence_ids: Vec::new(),
            source_spans: Vec::new(),
            metrics: BTreeMap::new(),
            assumptions: Vec::new(),
            limitations: Vec::new(),
        }
    }

    fn typed_usage(id: &str, type_id: &str) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "PartUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), Value::String(id.to_string())),
                ("type".to_string(), Value::String(type_id.to_string())),
            ]),
        }
    }

    fn metamodel_feature(
        id: &str,
        owner: &str,
        declared_name: &str,
        kir_property: &str,
        type_label: Option<&str>,
    ) -> KirElement {
        let mut properties = BTreeMap::from([
            ("owner".to_string(), Value::String(owner.to_string())),
            (
                "declared_name".to_string(),
                Value::String(declared_name.to_string()),
            ),
            (
                "kir_property".to_string(),
                Value::String(kir_property.to_string()),
            ),
        ]);
        if let Some(type_label) = type_label {
            properties.insert(
                "type_label".to_string(),
                Value::String(type_label.to_string()),
            );
        }
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties,
        }
    }
}
