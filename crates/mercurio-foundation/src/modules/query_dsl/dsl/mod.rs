mod stdlib;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rhai::{Engine, EvalAltResult, Scope};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value, json};

use crate::analysis::capability::{
    CapabilityRunReport, CapabilityRunStatus, CapabilityTarget, EvidenceEdge, EvidenceGraph,
    EvidenceNode, EvidenceNodeKind, EvidenceRelation, InsightConfidence, InsightKind,
    InsightPolarity, InsightScope, InsightSeverity, SemanticArtifact, SemanticElementRef,
    SemanticInsight,
};
use crate::kir::{KirFieldKind, KirFieldRegistry};
use crate::model::Graph;
use crate::semantic_services::identity::stable_digest;
use crate::workspace::model_state::{ModelArtifact, ModelRevision};
use types::{DslAppContext, ModelContext};

pub use types::{DslEdge, DslElement, ElementSet};

pub const DSL_QUERY_ARTIFACT_KIND: &str = "mercurio.artifact.dsl/query-report";
pub const DSL_ANALYSIS_RUN_ARTIFACT_KIND: &str = "mercurio.artifact.dsl/analysis-run-report";
pub const DSL_QUERY_CAPABILITY_ID: &str = "mercurio.dsl.query";
pub const DSL_CAPABILITY_VERSION: &str = "0.1.0";
pub const DSL_QUERY_RESULT_SCHEMA: &str = "mercurio.dsl.query_result.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslDiagnosticCategory {
    Parse,
    Runtime,
    Limit,
    HostPermission,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslDiagnostic {
    pub code: String,
    pub category: DslDiagnosticCategory,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslError {
    pub diagnostic: DslDiagnostic,
}

impl DslError {
    pub fn diagnostic(&self) -> &DslDiagnostic {
        &self.diagnostic
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            diagnostic: DslDiagnostic {
                code: "DSL_INTERNAL".to_string(),
                category: DslDiagnosticCategory::Internal,
                message: message.into(),
                script_name: None,
                line: None,
                column: None,
            },
        }
    }

    pub fn with_script_name(mut self, script_name: Option<String>) -> Self {
        if self.diagnostic.script_name.is_none() {
            self.diagnostic.script_name = script_name;
        }
        self
    }
}

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (
            self.diagnostic.script_name.as_deref(),
            self.diagnostic.line,
            self.diagnostic.column,
        ) {
            (Some(script_name), Some(line), Some(column)) => write!(
                f,
                "{} at {script_name}:{line}:{column}: {}",
                self.diagnostic.code, self.diagnostic.message
            ),
            (Some(script_name), Some(line), None) => write!(
                f,
                "{} at {script_name}:{line}: {}",
                self.diagnostic.code, self.diagnostic.message
            ),
            (Some(script_name), None, _) => {
                write!(
                    f,
                    "{} in {script_name}: {}",
                    self.diagnostic.code, self.diagnostic.message
                )
            }
            (None, Some(line), Some(column)) => write!(
                f,
                "{} at {line}:{column}: {}",
                self.diagnostic.code, self.diagnostic.message
            ),
            (None, Some(line), None) => {
                write!(
                    f,
                    "{} at {line}: {}",
                    self.diagnostic.code, self.diagnostic.message
                )
            }
            (None, None, _) => write!(f, "{}: {}", self.diagnostic.code, self.diagnostic.message),
        }
    }
}

impl std::error::Error for DslError {}

impl From<Box<EvalAltResult>> for DslError {
    fn from(error: Box<EvalAltResult>) -> Self {
        let position = error.position();
        let inner = error.unwrap_inner();
        let (code, category) = match inner {
            EvalAltResult::ErrorParsing(..) => ("DSL_PARSE", DslDiagnosticCategory::Parse),
            EvalAltResult::ErrorTooManyOperations(..) => {
                ("DSL_LIMIT_OPERATIONS", DslDiagnosticCategory::Limit)
            }
            EvalAltResult::ErrorTooManyVariables(..) => {
                ("DSL_LIMIT_VARIABLES", DslDiagnosticCategory::Limit)
            }
            EvalAltResult::ErrorTooManyModules(..) => {
                ("DSL_LIMIT_MODULES", DslDiagnosticCategory::Limit)
            }
            EvalAltResult::ErrorStackOverflow(..) => {
                ("DSL_LIMIT_STACK", DslDiagnosticCategory::Limit)
            }
            EvalAltResult::ErrorDataTooLarge(..) => {
                ("DSL_LIMIT_DATA", DslDiagnosticCategory::Limit)
            }
            EvalAltResult::ErrorSystem(..) => ("DSL_INTERNAL", DslDiagnosticCategory::Internal),
            _ => ("DSL_RUNTIME", DslDiagnosticCategory::Runtime),
        };
        Self {
            diagnostic: DslDiagnostic {
                code: code.to_string(),
                category,
                message: error.to_string(),
                script_name: None,
                line: position.line(),
                column: position.position(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslFieldSchema {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSchema {
    pub element_kinds: Vec<String>,
    pub fields: Vec<DslFieldSchema>,
    pub stdlib_functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslAnalysisRunSpec {
    pub run_id: String,
    pub capability_id: String,
    pub script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_element_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslExecutionLimits {
    pub max_operations: u64,
    pub max_string_size: usize,
    pub max_array_size: usize,
    pub max_map_size: usize,
    pub max_call_levels: usize,
}

impl Default for DslExecutionLimits {
    fn default() -> Self {
        Self {
            max_operations: 500_000,
            max_string_size: 1_000_000,
            max_array_size: 50_000,
            max_map_size: 50_000,
            max_call_levels: 32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslQueryRequest {
    pub script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<DslExecutionLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslQueryReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_name: Option<String>,
    pub result: DslQueryResult,
}

impl DslQueryReport {
    pub fn to_capability_run_report(
        &self,
        run_id: impl Into<String>,
    ) -> Result<CapabilityRunReport, DslError> {
        let payload = serde_json::to_value(&self.result)
            .map_err(|error| DslError::internal(error.to_string()))?;
        let payload_digest_input = payload.to_string();
        let digest = stable_digest([(
            DSL_QUERY_RESULT_SCHEMA.as_bytes(),
            payload_digest_input.as_bytes(),
        )]);
        Ok(CapabilityRunReport {
            run_id: run_id.into(),
            capability_id: DSL_QUERY_CAPABILITY_ID.to_string(),
            capability_version: Some(DSL_CAPABILITY_VERSION.to_string()),
            status: CapabilityRunStatus::Passed,
            target: CapabilityTarget::Workspace,
            insights: Vec::new(),
            artifacts: vec![SemanticArtifact {
                id: self
                    .script_name
                    .as_ref()
                    .map(|name| format!("dsl_query_result.{name}"))
                    .unwrap_or_else(|| "dsl_query_result".to_string()),
                kind: DSL_QUERY_ARTIFACT_KIND.to_string(),
                schema: DSL_QUERY_RESULT_SCHEMA.to_string(),
                digest,
                element_refs: Vec::new(),
                payload,
            }],
            evidence: EvidenceGraph::default(),
            diagnostics: Vec::new(),
            limitations: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslAnalysisRunRequest {
    #[serde(flatten)]
    pub spec: DslAnalysisRunSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<DslExecutionLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslAnalysisRunReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_name: Option<String>,
    pub report: CapabilityRunReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslExtensionSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_sets: Vec<DslModelSetFunction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schema_functions: Vec<String>,
}

impl DslExtensionSpec {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            model_sets: Vec::new(),
            schema_functions: Vec::new(),
        }
    }

    pub fn with_model_set_contains_any(
        mut self,
        name: impl Into<String>,
        field: impl Into<String>,
        contains_any: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let name = name.into();
        self.schema_functions.push(format!("ModelContext.{name}"));
        self.model_sets.push(DslModelSetFunction {
            name,
            field: field.into(),
            contains_any: contains_any.into_iter().map(Into::into).collect(),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslModelSetFunction {
    pub name: String,
    pub field: String,
    pub contains_any: Vec<String>,
}

pub struct RhaiEngine {
    engine: Engine,
}

pub struct DslEngine {
    rhai: RhaiEngine,
    extensions: Vec<DslExtensionSpec>,
}

impl DslEngine {
    pub fn new() -> Self {
        Self::with_extensions(Vec::new())
    }

    pub fn with_limits(limits: DslExecutionLimits) -> Self {
        Self::with_limits_and_extensions(limits, Vec::new())
    }

    pub fn with_extensions(extensions: Vec<DslExtensionSpec>) -> Self {
        Self::with_limits_and_extensions(DslExecutionLimits::default(), extensions)
    }

    pub fn with_limits_and_extensions(
        limits: DslExecutionLimits,
        extensions: Vec<DslExtensionSpec>,
    ) -> Self {
        Self {
            rhai: RhaiEngine::with_limits_and_extensions(limits, &extensions),
            extensions,
        }
    }

    pub fn eval_query(&self, graph: Arc<Graph>, script: &str) -> Result<DslQueryResult, DslError> {
        self.rhai.eval_query(graph, script)
    }

    pub fn eval_query_on_revision(
        &self,
        revision: &ModelRevision,
        script: &str,
    ) -> Result<DslQueryResult, DslError> {
        self.eval_query(revision.graph(), script)
    }

    pub fn execute_query(
        &self,
        graph: Arc<Graph>,
        request: DslQueryRequest,
    ) -> Result<DslQueryReport, DslError> {
        let script_name = request.script_name.clone();
        let result = if let Some(limits) = request.limits {
            Self::with_limits_and_extensions(limits, self.extensions.clone())
                .eval_query(graph, &request.script)
        } else {
            self.eval_query(graph, &request.script)
        }
        .map_err(|err| err.with_script_name(script_name.clone()))?;
        Ok(DslQueryReport {
            script_name: request.script_name,
            result,
        })
    }

    pub fn execute_query_on_revision(
        &self,
        revision: &ModelRevision,
        request: DslQueryRequest,
    ) -> Result<DslQueryReport, DslError> {
        self.execute_query(revision.graph(), request)
    }

    pub fn execute_query_artifact_on_revision(
        &self,
        revision: &ModelRevision,
        request: DslQueryRequest,
    ) -> Result<ModelArtifact, DslError> {
        let label = request.script_name.clone();
        let report = self.execute_query_on_revision(revision, request)?;
        let payload =
            serde_json::to_value(report).map_err(|error| DslError::internal(error.to_string()))?;
        ModelArtifact::new(
            revision.id().clone(),
            DSL_QUERY_ARTIFACT_KIND,
            label,
            payload,
        )
        .map_err(|error| DslError::internal(error.to_string()))
    }

    pub fn eval_analysis_run(
        &self,
        graph: Arc<Graph>,
        spec: DslAnalysisRunSpec,
    ) -> Result<CapabilityRunReport, DslError> {
        self.rhai.eval_analysis_run(graph, spec)
    }

    pub fn eval_analysis_run_on_revision(
        &self,
        revision: &ModelRevision,
        spec: DslAnalysisRunSpec,
    ) -> Result<CapabilityRunReport, DslError> {
        self.eval_analysis_run(revision.graph(), spec)
    }

    pub fn execute_analysis_run(
        &self,
        graph: Arc<Graph>,
        request: DslAnalysisRunRequest,
    ) -> Result<DslAnalysisRunReport, DslError> {
        let script_name = request.script_name.clone();
        let report = if let Some(limits) = request.limits {
            Self::with_limits_and_extensions(limits, self.extensions.clone())
                .eval_analysis_run(graph, request.spec)
        } else {
            self.eval_analysis_run(graph, request.spec)
        }
        .map_err(|err| err.with_script_name(script_name.clone()))?;
        Ok(DslAnalysisRunReport {
            script_name: request.script_name,
            report,
        })
    }

    pub fn execute_analysis_run_on_revision(
        &self,
        revision: &ModelRevision,
        request: DslAnalysisRunRequest,
    ) -> Result<DslAnalysisRunReport, DslError> {
        self.execute_analysis_run(revision.graph(), request)
    }

    pub fn execute_analysis_run_artifact_on_revision(
        &self,
        revision: &ModelRevision,
        request: DslAnalysisRunRequest,
    ) -> Result<ModelArtifact, DslError> {
        let label = request.script_name.clone();
        let report = self.execute_analysis_run_on_revision(revision, request)?;
        let payload =
            serde_json::to_value(report).map_err(|error| DslError::internal(error.to_string()))?;
        ModelArtifact::new(
            revision.id().clone(),
            DSL_ANALYSIS_RUN_ARTIFACT_KIND,
            label,
            payload,
        )
        .map_err(|error| DslError::internal(error.to_string()))
    }

    pub fn schema(graph: &Graph) -> DslSchema {
        Self::schema_with_extensions(graph, &[])
    }

    pub fn schema_with_extensions(graph: &Graph, extensions: &[DslExtensionSpec]) -> DslSchema {
        let mut schema = RhaiEngine::schema(graph);
        schema.stdlib_functions.extend(
            extensions
                .iter()
                .flat_map(|extension| extension.schema_functions.iter().cloned()),
        );
        schema.stdlib_functions.sort();
        schema.stdlib_functions.dedup();
        schema
    }

    pub fn schema_for(&self, graph: &Graph) -> DslSchema {
        Self::schema_with_extensions(graph, &self.extensions)
    }

    pub fn schema_for_revision(&self, revision: &ModelRevision) -> DslSchema {
        Self::schema_with_extensions(&revision.graph(), &self.extensions)
    }
}

impl RhaiEngine {
    pub fn new() -> Self {
        Self::with_limits(DslExecutionLimits::default())
    }

    pub fn with_limits(limits: DslExecutionLimits) -> Self {
        Self::with_limits_and_extensions(limits, &[])
    }

    pub fn with_limits_and_extensions(
        limits: DslExecutionLimits,
        extensions: &[DslExtensionSpec],
    ) -> Self {
        let mut engine = Engine::new();

        engine.set_max_operations(limits.max_operations);
        engine.set_max_string_size(limits.max_string_size);
        engine.set_max_array_size(limits.max_array_size);
        engine.set_max_map_size(limits.max_map_size);
        engine.set_max_call_levels(limits.max_call_levels);
        engine.disable_symbol("print");
        engine.disable_symbol("debug");

        types::register_types(&mut engine);
        types::register_extensions(&mut engine, extensions);
        stdlib::register_stdlib(&mut engine);

        Self { engine }
    }

    pub fn eval_query(&self, graph: Arc<Graph>, script: &str) -> Result<DslQueryResult, DslError> {
        let mut scope = Scope::new();
        scope.push("app", DslAppContext::new(Arc::clone(&graph)));
        scope.push("model", ModelContext::new(graph));
        types::push_scope_constants(&mut scope);

        let result: rhai::Dynamic = self.engine.eval_with_scope(&mut scope, script)?;
        Ok(dynamic_to_query_result(result))
    }

    pub fn eval_analysis_run(
        &self,
        graph: Arc<Graph>,
        spec: DslAnalysisRunSpec,
    ) -> Result<CapabilityRunReport, DslError> {
        let result = self.eval_query(Arc::clone(&graph), &spec.script)?;
        Ok(query_result_to_analysis_report(&graph, spec, result))
    }

    pub fn schema(graph: &Graph) -> DslSchema {
        let mut kinds = BTreeSet::new();
        let mut field_names = BTreeSet::from([
            "element_id".to_string(),
            "id".to_string(),
            "kind".to_string(),
            "layer".to_string(),
            "layer_name".to_string(),
            "model_layer".to_string(),
            "metatype_name".to_string(),
            "metatype_chain".to_string(),
        ]);

        for element in graph.elements() {
            kinds.insert(element.kind.as_ref().to_string());
            field_names.extend(element.properties.to_btree_map().into_keys());
        }

        let registry = KirFieldRegistry::structural();
        let fields = field_names
            .into_iter()
            .map(|name| DslFieldSchema {
                kind: field_kind_label(&registry, &name).to_string(),
                name,
            })
            .collect();

        DslSchema {
            element_kinds: kinds.into_iter().collect(),
            fields,
            stdlib_functions: vec![
                "AppContext.capabilities".into(),
                "AppContext.capability".into(),
                "AppContext.current_model".into(),
                "AppContext.new_model".into(),
                "AppContext.requires".into(),
                "ElementSet.order_by".into(),
                "ElementSet.order_by_desc".into(),
                "ElementSet.related".into(),
                "ElementSet.select_related".into(),
                "ElementSet.select_related_where_eq".into(),
                "ElementSet.where_contains".into(),
                "ElementSet.where_eq".into(),
                "ElementSet.where_in".into(),
                "ElementSet.where_metatype".into(),
                "ElementSet.where_metatype_in".into(),
                "ElementSet.where_metatype_is".into(),
                "ElementSet.where_metatype_is_any".into(),
                "ElementSet.where_model_layer".into(),
                "ElementSet.where_ne".into(),
                "Element.is_metatype".into(),
                "Element.metatype_chain".into(),
                "BuildPlan.depends_on".into(),
                "BuildPlan.operation".into(),
                "BuildPlan.plan".into(),
                "BuildPlan.task".into(),
                "TransactionBuilder.build_depends_on".into(),
                "TransactionBuilder.build_operation".into(),
                "TransactionBuilder.build_task".into(),
                "TransactionBuilder.capability".into(),
                "TransactionBuilder.commit".into(),
                "TransactionBuilder.create_definition".into(),
                "TransactionBuilder.create_package".into(),
                "TransactionBuilder.create_part_def".into(),
                "TransactionBuilder.preview".into(),
                "TransactionBuilder.rename".into(),
                "TransactionBuilder.set_attribute".into(),
                "TransientModel.element".into(),
                "TransientModel.element_count".into(),
                "TransientModel.elements".into(),
                "TransientModel.library_elements".into(),
                "TransientModel.namespaces".into(),
                "TransientModel.transaction".into(),
                "TransientModel.user_elements".into(),
                "ModelContext.capabilities".into(),
                "ModelContext.capability".into(),
                "ModelContext.changes".into(),
                "ModelContext.library_elements".into(),
                "ModelContext.namespaces".into(),
                "ModelContext.requires".into(),
                "ModelContext.transaction".into(),
                "ModelContext.user_elements".into(),
                "all_parts".into(),
                "build".into(),
                "count_by_kind".into(),
                "metatype".into(),
                "reachable".into(),
                "specialization_depth".into(),
                "max".into(),
                "min".into(),
                "sum".into(),
            ],
        }
    }
}

fn query_result_to_analysis_report(
    graph: &Graph,
    spec: DslAnalysisRunSpec,
    result: DslQueryResult,
) -> CapabilityRunReport {
    let status = status_from_result(&result);
    let subject = spec
        .subject_element_id
        .as_deref()
        .map(|element_id| semantic_element_ref(graph, element_id));
    let target = subject
        .as_ref()
        .map(|element| CapabilityTarget::Element {
            element_id: element.element_id.clone(),
        })
        .unwrap_or(CapabilityTarget::Workspace);
    let evidence_id = format!("evidence.{}.analysis_run", spec.run_id);
    let artifact_id = format!("artifact.{}.dsl_result", spec.run_id);
    let payload = json!({
        "columns": result.columns,
        "rows": result.rows,
        "script": spec.script,
    });
    let artifact = SemanticArtifact {
        id: artifact_id.clone(),
        kind: "dsl_analysis_result".to_string(),
        schema: "mercurio.dsl.analysis_result.v1".to_string(),
        digest: value_digest(&payload),
        element_refs: subject.clone().into_iter().collect(),
        payload,
    };
    let insight = insight_from_result(
        &spec.run_id,
        subject.clone(),
        status,
        &evidence_id,
        &artifact,
    );

    CapabilityRunReport {
        run_id: spec.run_id,
        capability_id: spec.capability_id,
        capability_version: Some(DSL_CAPABILITY_VERSION.to_string()),
        status,
        target,
        insights: insight.into_iter().collect(),
        artifacts: vec![artifact],
        evidence: EvidenceGraph {
            nodes: vec![
                EvidenceNode {
                    id: evidence_id.clone(),
                    kind: EvidenceNodeKind::AnalysisRun,
                    label: "DSL analysis run".to_string(),
                    element_refs: subject.into_iter().collect(),
                    source_spans: Vec::new(),
                    properties: BTreeMap::new(),
                },
                EvidenceNode {
                    id: artifact_id.clone(),
                    kind: EvidenceNodeKind::Artifact,
                    label: "DSL analysis result".to_string(),
                    element_refs: Vec::new(),
                    source_spans: Vec::new(),
                    properties: BTreeMap::new(),
                },
            ],
            edges: vec![EvidenceEdge {
                source_id: artifact_id,
                target_id: evidence_id,
                relation: EvidenceRelation::ProducedBy,
            }],
        },
        diagnostics: Vec::new(),
        limitations: Vec::new(),
    }
}

fn status_from_result(result: &DslQueryResult) -> CapabilityRunStatus {
    match verdict_value(result).and_then(Value::as_str) {
        Some("pass") | Some("passed") | Some("satisfied") => CapabilityRunStatus::Passed,
        Some("fail") | Some("failed") | Some("violated") => CapabilityRunStatus::Failed,
        Some("inconclusive") | Some("unknown") => CapabilityRunStatus::Inconclusive,
        _ => CapabilityRunStatus::Inconclusive,
    }
}

fn insight_from_result(
    run_id: &str,
    subject: Option<SemanticElementRef>,
    status: CapabilityRunStatus,
    evidence_id: &str,
    artifact: &SemanticArtifact,
) -> Option<SemanticInsight> {
    let (kind, polarity, severity, claim) = match status {
        CapabilityRunStatus::Passed => (
            InsightKind::CriterionPass,
            InsightPolarity::Supports,
            InsightSeverity::Info,
            "Analysis criterion passed",
        ),
        CapabilityRunStatus::Failed => (
            InsightKind::CriterionFail,
            InsightPolarity::Weakens,
            InsightSeverity::Error,
            "Analysis criterion failed",
        ),
        _ => return None,
    };
    Some(SemanticInsight {
        id: format!("insight.{run_id}.verdict"),
        kind,
        subject: subject
            .unwrap_or_else(|| SemanticElementRef::new("workspace").with_label("Workspace")),
        claim: claim.to_string(),
        polarity,
        severity,
        confidence: InsightConfidence::High,
        scope: InsightScope::Revision {
            revision: crate::semantic_services::mutation::WorkspaceRevision {
                fingerprint: artifact.digest.clone(),
            },
        },
        evidence_ids: vec![evidence_id.to_string(), artifact.id.clone()],
        source_spans: Vec::new(),
        metrics: first_row_metrics(&artifact.payload),
        assumptions: Vec::new(),
        limitations: Vec::new(),
    })
}

fn verdict_value(result: &DslQueryResult) -> Option<&Value> {
    let column_index = result
        .columns
        .iter()
        .position(|column| column == "verdict")?;
    result.rows.first()?.get(column_index)
}

fn first_row_metrics(payload: &Value) -> BTreeMap<String, Value> {
    let Some(columns) = payload.get("columns").and_then(Value::as_array) else {
        return BTreeMap::new();
    };
    let Some(row) = payload
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
    else {
        return BTreeMap::new();
    };
    columns
        .iter()
        .zip(row)
        .filter_map(|(column, value)| {
            column
                .as_str()
                .map(|column| (column.to_string(), value.clone()))
        })
        .collect()
}

fn semantic_element_ref(graph: &Graph, element_id: &str) -> SemanticElementRef {
    graph
        .element_by_element_id(element_id)
        .map(SemanticElementRef::from_graph_element)
        .unwrap_or_else(|| SemanticElementRef::new(element_id))
}

fn value_digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    stable_digest([("dsl-analysis-artifact".as_bytes(), bytes.as_slice())])
}

impl Default for RhaiEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DslEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn dynamic_to_query_result(result: rhai::Dynamic) -> DslQueryResult {
    if let Some(query_result) = result.clone().try_cast::<DslQueryResult>() {
        return query_result;
    }
    if let Some(model) = result.clone().try_cast::<types::DslTransientModel>() {
        return model.to_query_result();
    }
    if let Some(set) = result.clone().try_cast::<ElementSet>() {
        return set.to_query_result();
    }
    if let Some(value) = result.clone().try_cast::<i64>() {
        return single_value_result(Value::from(value));
    }
    if let Some(value) = result.clone().try_cast::<f64>() {
        return single_value_result(float_to_json(value));
    }
    if let Some(value) = result.clone().try_cast::<bool>() {
        return single_value_result(Value::Bool(value));
    }
    if let Some(value) = result.clone().try_cast::<String>() {
        return single_value_result(Value::String(value));
    }
    if let Some(map) = result.clone().try_cast::<rhai::Map>() {
        return map_to_query_result(map);
    }
    if let Some(array) = result.clone().try_cast::<rhai::Array>() {
        return array_to_query_result(array);
    }
    single_value_result(rhai_dynamic_to_json(result))
}

fn single_value_result(value: Value) -> DslQueryResult {
    DslQueryResult {
        columns: vec!["value".into()],
        rows: vec![vec![value]],
    }
}

fn field_kind_label(registry: &KirFieldRegistry, name: &str) -> &'static str {
    match registry.field(name).map(|spec| spec.kind) {
        Some(KirFieldKind::Reference) => "reference",
        Some(KirFieldKind::ReferenceList) => "list",
        _ => "scalar",
    }
}

fn map_to_query_result(map: rhai::Map) -> DslQueryResult {
    let columns = map.keys().map(ToString::to_string).collect::<Vec<_>>();
    let row = columns
        .iter()
        .map(|column| {
            map.get(column.as_str())
                .cloned()
                .map(rhai_dynamic_to_json)
                .unwrap_or(Value::Null)
        })
        .collect();
    DslQueryResult {
        columns,
        rows: vec![row],
    }
}

fn array_to_query_result(array: rhai::Array) -> DslQueryResult {
    if let Some(first) = array.first() {
        if first.is::<rhai::Map>() {
            let first_map = first.clone().cast::<rhai::Map>();
            let columns = first_map
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let rows = array
                .into_iter()
                .filter_map(|value| value.try_cast::<rhai::Map>())
                .map(|map| {
                    columns
                        .iter()
                        .map(|column| {
                            map.get(column.as_str())
                                .cloned()
                                .map(rhai_dynamic_to_json)
                                .unwrap_or(Value::Null)
                        })
                        .collect()
                })
                .collect();
            return DslQueryResult { columns, rows };
        }
    }

    DslQueryResult {
        columns: vec!["value".into()],
        rows: array
            .into_iter()
            .map(|value| vec![rhai_dynamic_to_json(value)])
            .collect(),
    }
}

fn rhai_dynamic_to_json(value: rhai::Dynamic) -> Value {
    if value.is::<i64>() {
        return Value::from(value.cast::<i64>());
    }
    if value.is::<f64>() {
        return float_to_json(value.cast::<f64>());
    }
    if value.is::<bool>() {
        return Value::Bool(value.cast::<bool>());
    }
    if value.is::<String>() {
        return Value::String(value.cast::<String>());
    }
    if value.is::<rhai::Array>() {
        return Value::Array(
            value
                .cast::<rhai::Array>()
                .into_iter()
                .map(rhai_dynamic_to_json)
                .collect(),
        );
    }
    if value.is::<rhai::Map>() {
        let object = value
            .cast::<rhai::Map>()
            .into_iter()
            .map(|(key, value)| (key.to_string(), rhai_dynamic_to_json(value)))
            .collect();
        return Value::Object(object);
    }
    if value.is_unit() {
        return Value::Null;
    }
    Value::String(format!("{value:?}"))
}

fn float_to_json(value: f64) -> Value {
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::kir::{KirDocument, KirElement};

    use super::*;

    fn sample_graph() -> Arc<Graph> {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".into(),
                    kind: "PartDefinition".into(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("Vehicle")),
                        ("mass_kg".into(), serde_json::json!(10.0)),
                        (
                            "features".into(),
                            serde_json::json!(["feature.Demo.Vehicle.payloadMass"]),
                        ),
                        (
                            "members".into(),
                            serde_json::json!(["type.Demo.Vehicle.wheel"]),
                        ),
                    ]),
                },
                KirElement {
                    id: "type.Demo.Vehicle.wheel".into(),
                    kind: "PartUsage".into(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("wheel")),
                        ("mass_kg".into(), serde_json::json!(2.5)),
                        ("owner".into(), serde_json::json!("type.Demo.Vehicle")),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.payloadMass".into(),
                    kind: "AttributeUsage".into(),
                    layer: 3,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("payload_mass_kg")),
                        ("owner".into(), serde_json::json!("type.Demo.Vehicle")),
                        (
                            "expression_ir".into(),
                            serde_json::json!({"kind": "literal", "value": 3.0}),
                        ),
                    ]),
                },
                KirElement {
                    id: "type.Demo.Animal".into(),
                    kind: "PartDefinition".into(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("Animal")),
                        ("mass_kg".into(), serde_json::json!(4.0)),
                    ]),
                },
            ],
        };
        Arc::new(Graph::from_document(document).unwrap())
    }

    fn sample_revision() -> ModelRevision {
        ModelRevision::from_kir_document(
            KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![KirElement {
                    id: "type.Demo.Vehicle".into(),
                    kind: "PartDefinition".into(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".into(),
                        serde_json::json!("Vehicle"),
                    )]),
                }],
            },
            crate::workspace::model_state::ModelBuildRecord::new(
                crate::workspace::model_state::ModelRevisionProducer::KirImport,
            ),
        )
        .unwrap()
    }

    fn metatype_graph() -> Arc<Graph> {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "KerML::Core::Element".into(),
                    kind: "Metaclass".into(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "declared_name".into(),
                        serde_json::json!("Element"),
                    )]),
                },
                KirElement {
                    id: "KerML::Core::Namespace".into(),
                    kind: "Metaclass".into(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("Namespace")),
                        (
                            "specializes".into(),
                            serde_json::json!(["KerML::Core::Element"]),
                        ),
                    ]),
                },
                KirElement {
                    id: "KerML::Core::Package".into(),
                    kind: "Metaclass".into(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("Package")),
                        (
                            "specializes".into(),
                            serde_json::json!(["KerML::Core::Namespace"]),
                        ),
                    ]),
                },
                KirElement {
                    id: "pkg.Demo".into(),
                    kind: "Package".into(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".into(), serde_json::json!("Demo")),
                        ("metatype".into(), serde_json::json!("KerML::Core::Package")),
                    ]),
                },
            ],
        };
        Arc::new(Graph::from_document(document).unwrap())
    }

    fn first_row_value<'a>(result: &'a DslQueryResult, column: &str) -> &'a Value {
        let index = result
            .columns
            .iter()
            .position(|candidate| candidate == column)
            .expect("column exists");
        &result.rows[0][index]
    }

    #[test]
    fn count_all_parts() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(sample_graph(), "model.parts().count()")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], serde_json::json!(3));
    }

    #[test]
    fn dsl_engine_facade_runs_query() {
        let engine = DslEngine::new();
        let result = engine
            .eval_query(sample_graph(), "model.parts().count()")
            .unwrap();
        assert_eq!(result.columns, vec!["value"]);
        assert_eq!(result.rows[0][0], serde_json::json!(3));
    }

    #[test]
    fn dsl_engine_report_preserves_script_name() {
        let engine = DslEngine::new();
        let report = engine
            .execute_query(
                sample_graph(),
                DslQueryRequest {
                    script: "model.parts().count()".into(),
                    script_name: Some("queries/count_parts.mercurio-query.dsl".into()),
                    limits: None,
                },
            )
            .unwrap();

        assert_eq!(
            report.script_name.as_deref(),
            Some("queries/count_parts.mercurio-query.dsl")
        );
        assert_eq!(report.result.rows[0][0], serde_json::json!(3));
    }

    #[test]
    fn dsl_query_report_converts_to_capability_run_report() {
        let report = DslQueryReport {
            script_name: Some("queries/count_parts.mercurio-query.dsl".to_string()),
            result: DslQueryResult {
                columns: vec!["value".to_string()],
                rows: vec![vec![serde_json::json!(3)]],
            },
        };

        let capability_report = report.to_capability_run_report("run.dsl.query").unwrap();

        assert_eq!(capability_report.run_id, "run.dsl.query");
        assert_eq!(capability_report.capability_id, DSL_QUERY_CAPABILITY_ID);
        assert_eq!(capability_report.status, CapabilityRunStatus::Passed);
        assert_eq!(capability_report.artifacts.len(), 1);
        assert_eq!(capability_report.artifacts[0].kind, DSL_QUERY_ARTIFACT_KIND);
        assert_eq!(
            capability_report.artifacts[0].schema,
            DSL_QUERY_RESULT_SCHEMA
        );
        assert_eq!(
            capability_report.artifacts[0].payload["rows"][0][0],
            serde_json::json!(3)
        );
    }

    #[test]
    fn dsl_engine_runs_query_against_model_revision() {
        let engine = DslEngine::new();
        let revision = sample_revision();
        let report = engine
            .execute_query_on_revision(
                &revision,
                DslQueryRequest {
                    script: "model.parts().count()".into(),
                    script_name: Some("queries/revision_count.mercurio-query.dsl".into()),
                    limits: None,
                },
            )
            .unwrap();

        assert_eq!(report.result.columns, vec!["value"]);
        assert_eq!(report.result.rows[0][0], serde_json::json!(1));
    }

    #[test]
    fn dsl_engine_query_artifact_is_scoped_to_model_revision() {
        let engine = DslEngine::new();
        let revision = sample_revision();
        let artifact = engine
            .execute_query_artifact_on_revision(
                &revision,
                DslQueryRequest {
                    script: "model.parts().count()".into(),
                    script_name: Some("queries/revision_count.mercurio-query.dsl".into()),
                    limits: None,
                },
            )
            .unwrap();

        assert_eq!(artifact.revision_id, revision.id().clone());
        assert_eq!(artifact.kind, DSL_QUERY_ARTIFACT_KIND);
        assert_eq!(
            artifact.payload["script_name"],
            serde_json::json!("queries/revision_count.mercurio-query.dsl")
        );
        assert_eq!(
            artifact.payload["result"]["rows"][0][0],
            serde_json::json!(1)
        );
    }

    #[test]
    fn dsl_engine_request_limits_operations() {
        let engine = DslEngine::new();
        let error = engine
            .execute_query(
                sample_graph(),
                DslQueryRequest {
                    script: "let x = 0; while x < 1000 { x += 1; } x".into(),
                    script_name: Some("queries/too_many_ops.mercurio-query.dsl".into()),
                    limits: Some(DslExecutionLimits {
                        max_operations: 10,
                        ..DslExecutionLimits::default()
                    }),
                },
            )
            .unwrap_err();

        assert_eq!(error.diagnostic().code, "DSL_LIMIT_OPERATIONS");
        assert_eq!(error.diagnostic().category, DslDiagnosticCategory::Limit);
        assert_eq!(
            error.diagnostic().script_name.as_deref(),
            Some("queries/too_many_ops.mercurio-query.dsl")
        );
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn dsl_engine_parse_error_has_position() {
        let engine = DslEngine::new();
        let error = engine
            .execute_query(
                sample_graph(),
                DslQueryRequest {
                    script: "let = 1;".into(),
                    script_name: Some("queries/bad.mercurio-query.dsl".into()),
                    limits: None,
                },
            )
            .unwrap_err();

        assert_eq!(error.diagnostic().code, "DSL_PARSE");
        assert_eq!(error.diagnostic().category, DslDiagnosticCategory::Parse);
        assert_eq!(
            error.diagnostic().script_name.as_deref(),
            Some("queries/bad.mercurio-query.dsl")
        );
        assert_eq!(error.diagnostic().line, Some(1));
    }

    #[test]
    fn filter_by_kind() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.parts().where(|p| p.kind == "PartDefinition").count()"#,
            )
            .unwrap();
        assert_eq!(result.rows[0][0], serde_json::json!(2));
    }

    #[test]
    fn select_names() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.parts()
                       .where(|p| p.kind == "PartDefinition")
                       .select(["declared_name"])"#,
            )
            .unwrap();
        assert_eq!(result.columns, vec!["declared_name"]);
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn native_property_helpers_filter_and_order() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.parts()
                       .where_in("kind", ["PartDefinition"])
                       .where_ne("declared_name", "Animal")
                       .order_by_desc("declared_name")
                       .select(["declared_name"])"#,
            )
            .unwrap();

        assert_eq!(result.columns, vec!["declared_name"]);
        assert_eq!(result.rows, vec![vec![json!("Vehicle")]]);
    }

    #[test]
    fn named_model_layer_helpers_filter_user_and_library_elements() {
        let engine = RhaiEngine::new();
        let user_result = engine
            .eval_query(metatype_graph(), r#"model.user_elements().count()"#)
            .unwrap();
        let library_result = engine
            .eval_query(
                metatype_graph(),
                r#"model.elements().where_model_layer(ModelLayer.Library).count()"#,
            )
            .unwrap();

        assert_eq!(user_result.rows[0][0], json!(1));
        assert_eq!(library_result.rows[0][0], json!(3));
    }

    #[test]
    fn metatype_helpers_distinguish_exact_from_subtype_matching() {
        let engine = RhaiEngine::new();
        let exact_result = engine
            .eval_query(
                metatype_graph(),
                r#"model.user_elements().where_metatype("Element").count()"#,
            )
            .unwrap();
        let subtype_result = engine
            .eval_query(
                metatype_graph(),
                r#"model.user_elements().where_metatype_is("Element").count()"#,
            )
            .unwrap();
        let package_result = engine
            .eval_query(
                metatype_graph(),
                r#"model.user_elements().where_metatype(KerML.Package).count()"#,
            )
            .unwrap();

        assert_eq!(exact_result.rows[0][0], json!(0));
        assert_eq!(subtype_result.rows[0][0], json!(1));
        assert_eq!(package_result.rows[0][0], json!(1));
    }

    #[test]
    fn metatype_tokens_support_namespace_queries_and_element_inspection() {
        let engine = RhaiEngine::new();
        let namespace_result = engine
            .eval_query(metatype_graph(), r#"model.namespaces().count()"#)
            .unwrap();
        let element_result = engine
            .eval_query(
                metatype_graph(),
                r#"let demo = model.element("pkg.Demo");
                   #{is_namespace: demo.is_metatype(Namespace),
                     metatype: demo.metatype,
                     layer: demo.model_layer,
                     chain: demo.metatype_chain()}"#,
            )
            .unwrap();

        assert_eq!(namespace_result.rows[0][0], json!(1));
        assert_eq!(
            first_row_value(&element_result, "is_namespace"),
            &json!(true)
        );
        assert_eq!(
            first_row_value(&element_result, "metatype"),
            &json!("Package")
        );
        assert_eq!(first_row_value(&element_result, "layer"), &json!("user"));
        assert_eq!(
            first_row_value(&element_result, "chain"),
            &json!(["Package", "Namespace", "Element"])
        );
    }

    #[test]
    fn native_relationship_projection_selects_source_and_target_fields() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.parts()
                       .where_eq("declared_name", "Vehicle")
                       .select_related_where_eq(
                           "features",
                           "kind",
                           "AttributeUsage",
                           ["declared_name"],
                           ["declared_name"]
                       )"#,
            )
            .unwrap();

        assert_eq!(
            result.columns,
            vec!["source.declared_name", "target.declared_name"]
        );
        assert_eq!(
            result.rows,
            vec![vec![json!("Vehicle"), json!("payload_mass_kg")]]
        );
    }

    #[test]
    fn dsl_engine_registers_model_set_extensions() {
        let extension = DslExtensionSpec::new("test").with_model_set_contains_any(
            "vehicle_named",
            "declared_name",
            ["Vehicle"],
        );
        let engine = DslEngine::with_extensions(vec![extension]);
        let result = engine
            .eval_query(
                sample_graph(),
                "model.vehicle_named().select([\"declared_name\"])",
            )
            .unwrap();

        assert_eq!(result.columns, vec!["declared_name"]);
        assert_eq!(result.rows, vec![vec![json!("Vehicle")]]);
        assert!(
            engine
                .schema_for(&sample_graph())
                .stdlib_functions
                .contains(&"ModelContext.vehicle_named".to_string())
        );
    }

    #[test]
    fn capability_binding_reports_readiness_and_run_status() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"let cap = model.capability("mercurio.dsl.analysis");
                   cap.run(#{subject: "Demo.Vehicle"})"#,
            )
            .unwrap();

        assert_eq!(
            result.columns,
            vec![
                "deterministic",
                "element_count",
                "id",
                "parameters",
                "status"
            ]
        );
        assert_eq!(result.rows[0][2], json!("mercurio.dsl.analysis"));
        assert_eq!(result.rows[0][4], json!("passed"));
    }

    #[test]
    fn change_set_preview_does_not_apply_mutation() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.changes()
                       .rename("Demo.Vehicle", "VehicleRenamed")
                       .set_attribute("Demo.Vehicle", "mass_kg", 12.0)
                       .preview()"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "action_count"), &json!(2));
        assert_eq!(first_row_value(&result, "applies_changes"), &json!(false));
        assert_eq!(
            first_row_value(&result, "kind"),
            &json!("change_set_preview")
        );
        assert_eq!(
            first_row_value(&result, "change_set")["schema"],
            json!("mercurio.semantic_change_set.v1")
        );
    }

    #[test]
    fn build_plan_declares_deterministic_task_graph() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"build("check")
                       .task("compile")
                       .task("query")
                       .depends_on("query", "compile")
                       .operation("compile", "compile")
                       .operation("query", "dsl-run")
                       .plan()"#,
            )
            .unwrap();

        assert_eq!(result.rows[0][0], json!("build_plan"));
        assert_eq!(result.rows[0][1], json!("check"));
        assert_eq!(result.rows[0][2], json!(2));
    }

    #[test]
    fn transaction_preview_collects_change_set_capability_and_build_operations() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.transaction("vehicle check")
                       .rename("Demo.Vehicle", "VehicleRenamed")
                       .set_attribute("Demo.Vehicle", "mass_kg", 12.0)
                       .capability("mercurio.dsl.analysis", #{scope: "vehicle"})
                       .build_task("check")
                       .build_operation("check", "compile")
                       .build_depends_on("publish", "check")
                       .preview()"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "applied"), &json!(false));
        assert_eq!(first_row_value(&result, "label"), &json!("vehicle check"));
        assert_eq!(first_row_value(&result, "operation_count"), &json!(5));
        assert_eq!(first_row_value(&result, "status"), &json!("previewed"));
        assert_eq!(
            first_row_value(&result, "operations")[0]["kind"],
            json!("change_set")
        );
        assert!(
            first_row_value(&result, "transaction_id")
                .as_str()
                .is_some_and(|value| value.starts_with("txn.fnv1a64_"))
        );
    }

    #[test]
    fn transaction_commit_is_host_permission_gated_by_default() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.transaction("rename vehicle")
                       .rename("Demo.Vehicle", "VehicleRenamed")
                       .commit()"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "applied"), &json!(false));
        assert_eq!(first_row_value(&result, "status"), &json!("rejected"));
        assert!(
            first_row_value(&result, "diagnostics")
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[test]
    fn transient_model_transaction_commit_applies_in_memory() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"let scratch = app.new_model("Demo");
                   let report = scratch.transaction("seed scratch")
                       .create_package("Demo")
                       .create_part_def("Demo.Vehicle")
                       .commit();
                   #{status: report.status,
                     applied: report.applied,
                     element_count: scratch.user_elements().count(),
                     package_count: scratch.user_elements().where_metatype(KerML.Package).count(),
                     part_count: scratch.user_elements().where_metatype_is(SysML.PartDefinition).count(),
                     revision: scratch.revision}"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "status"), &json!("committed"));
        assert_eq!(first_row_value(&result, "applied"), &json!(true));
        assert_eq!(first_row_value(&result, "element_count"), &json!(2));
        assert_eq!(first_row_value(&result, "package_count"), &json!(1));
        assert_eq!(first_row_value(&result, "part_count"), &json!(1));
        assert!(
            first_row_value(&result, "revision")
                .as_str()
                .is_some_and(|value| value != "unrevisioned")
        );
    }

    #[test]
    fn transient_model_transaction_commits_semantic_add_element() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"let scratch = app.new_model("Demo");
                   let report = scratch.transaction("seed scratch")
                       .create_package("Demo")
                       .create_element("Demo", "PartDefinition", "Vehicle")
                       .create_typed_element("Demo.Vehicle", "PartUsage", "engine", "Demo.Engine")
                       .preview();
                   #{status: report.status,
                     applied: report.applied,
                     added_count: report.semantic_diff.added_elements.len}"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "status"), &json!("previewed"));
        assert_eq!(first_row_value(&result, "applied"), &json!(false));
        assert_eq!(first_row_value(&result, "added_count"), &json!(3));
    }

    #[test]
    fn transient_model_transaction_preview_does_not_apply() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"let scratch = app.new_model("Demo");
                   let report = scratch.transaction("preview scratch")
                       .create_package("Demo")
                       .preview();
                   #{status: report.status,
                     applied: report.applied,
                     element_count: scratch.user_elements().count(),
                     added_count: report.semantic_diff.added_elements.len}"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "status"), &json!("previewed"));
        assert_eq!(first_row_value(&result, "applied"), &json!(false));
        assert_eq!(first_row_value(&result, "element_count"), &json!(0));
        assert_eq!(first_row_value(&result, "added_count"), &json!(1));
    }

    #[test]
    fn transient_model_commit_rejects_non_model_edit_operations() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"let scratch = app.new_model("Demo");
                   scratch.transaction("run capability")
                       .capability("mercurio.dsl.analysis", #{scope: "scratch"})
                       .commit()"#,
            )
            .unwrap();

        assert_eq!(first_row_value(&result, "status"), &json!("rejected"));
        assert_eq!(first_row_value(&result, "applied"), &json!(false));
        assert!(
            first_row_value(&result, "diagnostics")
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[test]
    fn app_current_model_references_injected_model() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"app.current_model().user_elements().count()"#,
            )
            .unwrap();

        assert_eq!(result.rows[0][0], json!(3));
    }

    #[test]
    fn traverse_outgoing() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.element("type.Demo.Vehicle").outgoing("members").count()"#,
            )
            .unwrap();
        assert_eq!(result.rows[0][0], serde_json::json!(1));
    }

    #[test]
    fn stdlib_count_by_kind() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(sample_graph(), "count_by_kind(model.parts())")
            .unwrap();
        assert_eq!(result.columns, vec!["PartDefinition", "PartUsage"]);
        assert_eq!(result.rows.len(), 1);
    }

    #[test]
    fn stdlib_sum_numeric_properties() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"sum(model.parts().map(|p| p.property("mass_kg")))"#,
            )
            .unwrap();
        assert_eq!(result.columns, vec!["value"]);
        assert_eq!(result.rows[0][0], serde_json::json!(16.5));
    }

    #[test]
    fn stdlib_max_and_min_numeric_properties() {
        let engine = RhaiEngine::new();
        let max_result = engine
            .eval_query(
                sample_graph(),
                r#"max(model.parts().map(|p| p.property("mass_kg")))"#,
            )
            .unwrap();
        let min_result = engine
            .eval_query(
                sample_graph(),
                r#"min(model.parts().map(|p| p.property("mass_kg")))"#,
            )
            .unwrap();

        assert_eq!(max_result.columns, vec!["value"]);
        assert_eq!(max_result.rows[0][0], serde_json::json!(10.0));
        assert_eq!(min_result.columns, vec!["value"]);
        assert_eq!(min_result.rows[0][0], serde_json::json!(2.5));
    }

    #[test]
    fn stdlib_max_and_min_binary_values() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"#{larger: max(12.0, 7.0), smaller: min(12.0, 7.0)}"#,
            )
            .unwrap();

        assert_eq!(result.columns, vec!["larger", "smaller"]);
        assert_eq!(
            result.rows[0],
            vec![serde_json::json!(12.0), serde_json::json!(7.0)]
        );
    }

    #[test]
    fn property_resolves_owned_literal_feature() {
        let engine = RhaiEngine::new();
        let result = engine
            .eval_query(
                sample_graph(),
                r#"model.element("type.Demo.Vehicle").property("payload_mass_kg")"#,
            )
            .unwrap();
        assert_eq!(result.columns, vec!["value"]);
        assert_eq!(result.rows[0][0], serde_json::json!(3.0));
    }

    #[test]
    fn analysis_run_wraps_dsl_result_with_evidence() {
        let engine = RhaiEngine::new();
        let report = engine
            .eval_analysis_run(
                sample_graph(),
                DslAnalysisRunSpec {
                    run_id: "vehicle-mass".into(),
                    capability_id: "mercurio.dsl.analysis".into(),
                    subject_element_id: Some("type.Demo.Vehicle".into()),
                    script: r#"
                        let total = sum(model.parts().map(|p| p.property("mass_kg")));
                        #{total_mass_kg: total, verdict: "pass"}
                    "#
                    .into(),
                },
            )
            .unwrap();

        assert_eq!(report.status, CapabilityRunStatus::Passed);
        assert_eq!(report.artifacts.len(), 1);
        assert_eq!(report.insights.len(), 1);
        assert_eq!(report.insights[0].kind, InsightKind::CriterionPass);
        assert_eq!(report.evidence.nodes.len(), 2);
        assert!(
            report
                .evidence
                .nodes
                .iter()
                .any(|node| node.kind == EvidenceNodeKind::AnalysisRun)
        );
        assert_eq!(
            report.artifacts[0].payload["rows"][0][0],
            serde_json::json!(16.5)
        );
    }

    #[test]
    fn unknown_element_returns_unit() {
        let engine = RhaiEngine::new();
        let result = engine.eval_query(sample_graph(), r#"model.element("nonexistent")"#);
        assert!(result.is_ok());
    }
}
