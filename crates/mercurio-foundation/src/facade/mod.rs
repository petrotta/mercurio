//! Curated facade for Mercurio foundation APIs.
//!
//! This crate is the primary integration surface for tools that need to load
//! KIR, build model graphs, run semantic/runtime queries, produce view DTOs,
//! and work with workspace/library configuration. Prefer the root-level
//! re-exports in this crate over depending on implementation modules directly.
//!
//! Module paths are kept public for existing callers, but most implementation
//! modules are hidden from generated documentation so the rustdoc surface
//! reflects the intended API contract.

pub mod ai_review {
    pub use crate::analysis::ai_review::*;
}
#[doc(hidden)]
pub mod assessment {
    pub use crate::analysis::assessment::*;
}
#[doc(hidden)]
pub mod capability {
    pub use crate::analysis::capability::*;
}
#[doc(hidden)]
pub mod cognitive {
    pub use crate::analysis::cognitive::*;
}
#[doc(hidden)]
pub mod datalog {
    pub use crate::runtime::{
        Atom, CORE_RULEPACK_ID, CORE_RULEPACK_VERSION, DatalogError, DerivedIndexes,
        DiagnosticRule, Evaluation, Explanation, Fact, Rule, RuleDiagnostic,
        RuleDiagnosticSeverity, RulePack, Term, evaluate, evaluate_diagnostics,
        extract_graph_facts, load_default_rulepacks, materialize_core_indexes,
    };
}
#[doc(hidden)]
pub mod derived {
    pub use crate::model::{
        DerivedFeatureCache, DerivedFeatureManifest, DerivedFeatureManifestError,
        DerivedFeatureRegistry, DerivedFeatureRule, DerivedFeatureSpec, DerivedPropertySource,
        DerivedPropertyValue, builtin_core_derived_feature_manifest, derived_properties,
        derived_property, manifest_from_metadata,
    };
}
#[doc(hidden)]
pub mod expression {
    pub use crate::model::{
        BinaryExpressionOp, ExpressionEvaluationContext, ExpressionEvaluationError, ExpressionIr,
        ExpressionIrError, ExpressionPathRoot, ExpressionPathSegment, ExpressionValidationError,
        UnaryExpressionOp,
    };
}
#[doc(hidden)]
pub mod feasibility {
    pub use crate::semantic_services::feasibility::*;
}
#[doc(hidden)]
#[allow(unused_imports)]
pub mod frontend {
    pub use crate::authoring::frontend::*;
}
#[allow(deprecated)]
#[doc(hidden)]
pub mod goal {
    pub use crate::analysis::goal::*;
}
#[doc(hidden)]
pub mod graph {
    pub use crate::model::{
        Edge, Element, ElementProperties, Graph, GraphArtifact, GraphError, NodeId,
    };
}
#[doc(hidden)]
pub mod identity {
    pub use crate::semantic_services::identity::*;
}
#[doc(hidden)]
pub mod ir;
#[doc(hidden)]
pub mod language {
    pub use crate::codegen::language::*;
}
#[doc(hidden)]
pub mod library {
    pub use crate::workspace::library::*;
}
#[doc(hidden)]
pub mod logging;
#[doc(hidden)]
pub mod metadata {
    pub use crate::model::{
        ElementMetadataView, KirMetadataAnnotation, MetadataView, metadata_annotations,
        metadata_annotations_named, metadata_string_property,
    };
}
#[doc(hidden)]
pub mod metamodel {
    pub use crate::model::{
        AttributeRow, AttributeValueSource, DerivedMetamodelCapabilities, ElementAttributeQuery,
        ElementSummary, MetamodelAttributeDeclaration, MetamodelAttributeRegistry,
        MetamodelClassView, MetamodelFeatureRegistry, MetamodelFeatureView,
        MetamodelValidationDiagnostic, MetatypeQueryOverride, collect_specialization_ancestors,
        derive_metamodel_capabilities, effective_element_properties_with_derived,
        effective_properties, effective_properties_with_derived, element_metatype,
        query_element_attributes, validate_derived_metamodel_semantics,
    };
}
pub mod dsl {
    pub use crate::query_dsl::dsl::*;
}
pub mod model_state {
    pub use crate::workspace::model_state::*;
}
#[doc(hidden)]
pub mod mpack {
    pub use crate::workspace::mpack::*;
}
#[doc(hidden)]
pub mod mutation {
    pub use crate::semantic_services::mutation::*;
}
#[doc(hidden)]
pub mod outline {
    pub use crate::authoring::outline::*;
}
#[doc(hidden)]
pub mod paths {
    pub use crate::workspace::paths::*;
}
#[doc(hidden)]
pub mod performance {
    pub use crate::workspace::performance::*;
}
#[doc(hidden)]
pub mod plugin_registry {
    pub use crate::workspace::plugin_registry::*;
}
#[doc(hidden)]
pub mod proposal;
#[doc(hidden)]
pub mod project_sources {
    pub use crate::workspace::project_sources::*;
}
#[doc(hidden)]
pub mod python_codegen {
    pub use crate::codegen::python_codegen::*;
}
#[doc(hidden)]
pub mod query {
    pub use crate::query_dsl::query::*;
}
#[doc(hidden)]
pub mod semantic_compare {
    pub use crate::analysis::semantic_compare::*;
}
pub mod semantic_legality {
    pub use crate::semantic_services::semantic_legality::*;
}
pub mod semantic_next_actions {
    pub use crate::semantic_services::semantic_next_actions::*;
}
#[doc(hidden)]
pub mod semantic_profile {
    pub use crate::semantic_services::semantic_profile::*;
}
#[doc(hidden)]
pub mod semantic_target;
#[doc(hidden)]
pub mod semantic_validation {
    pub use crate::semantic_services::semantic_validation::*;
}
#[doc(hidden)]
pub mod source_set {
    pub use crate::authoring::source_set::*;
}
#[doc(hidden)]
pub mod syntax_compare {
    pub use crate::authoring::syntax_compare::*;
}
pub mod transaction {
    pub use crate::session::transaction::*;
}
pub mod variant {
    pub use crate::semantic_services::variant::*;
}
#[doc(hidden)]
pub mod workspace_cache {
    pub use crate::workspace::workspace_cache::*;
}

pub use crate::analysis::{
    AnalysisCapabilityDescriptor, AnalysisCapabilityEffect, AnalysisCapabilityProviderKind,
    AnalysisCapabilitySelector, AnalysisCaseModel, AnalysisElementRef, AnalysisInventory,
    AnalysisOpportunity, AnalysisOpportunityKind, AnalysisOpportunityReadiness,
    AnalysisOpportunityReport, AnalysisStructuralPredicate, AnalysisTechniqueKind,
    AnalysisWorkflow, AnalysisWorkflowStep, AnalysisWorkflowStepKind, RequirementEvaluationModel,
    builtin_analysis_capability_descriptors,
};
pub use crate::authoring::{
    Alias, AttributeWritePolicy, AuthoringError, AuthoringModule, AuthoringProject,
    AuthoringRenderProfile, ContainerSelector, Declaration, Definition, Import, Mutation,
    MutationResult, Package, QualifiedName, RenderedSpan, SemanticAttribute, SemanticEdit, Usage,
    ValidationReport, WriteBackMode, WriteBackResult, create_empty_model,
    load_authoring_project_from_kir, textual_model_authoring_render_profile,
};
pub use crate::language_contracts::{
    CompileContext, LanguageRegistry, LanguageService, ProjectSourceSelectionError,
    select_project_source_paths,
};
pub use crate::runtime::{
    ExecutionContext, LayeredRuntime, LayeredRuntimeAssembly, QueryResult, Runtime,
    RuntimeArtifact, RuntimeBase, RuntimeError, RuntimeOverlay, RuntimeProfile,
    RuntimeProfileTimings,
};
pub use crate::session::{
    CAPABILITY_RUN_MIME_TYPE, CellKind, CellLanguage, CellOutput, CellOutputKind, CellRunHandlers,
    CellRunReport, CellRunRequest, CellRunStatus, CommitMode, CommitResult, CommitStrategy,
    DSL_ACTION_PREVIEW_MIME_TYPE, DSL_QUERY_MIME_TYPE, DslCellAnalysisRequest, ForkElement,
    ForkElementSpec, KirOverlay, ModelFork, ModelSession, ModelWorkspace, PythonCellScriptOutput,
    PythonCellScriptRequest, SessionError, UnsupportedCellRun, WorkspaceSnapshot,
    cell_status_from_capability, default_cell_id, run_cell_with_handlers, string_parameter,
    u64_parameter,
};
pub use crate::workspace::{
    ProjectDescriptor, ProjectExtensionDescriptor, ProjectModelConfig, ResolvedWorkspaceContext,
    ResolvedWorkspaceLibrary, WorkspaceConfig, WorkspaceConfigError, WorkspaceContextOptions,
    WorkspaceLibraryConfig, WorkspaceLibraryRole, WorkspacePluginConfig,
    discover_project_extension_descriptor_path, discover_workspace_config_path,
    resolve_project_descriptor_context, resolve_workspace_context,
    resolve_workspace_context_from_config_path, resolve_workspace_context_with_options,
};
pub use ai_review::{
    AI_MUTATION_REVIEW_SCHEMA_VERSION, SemanticMutationFeedbackSummary,
    SemanticMutationProposalFeedback, SemanticMutationProposalFeedbackDecision,
    SemanticMutationProposalReview, SemanticMutationProposalReviewInput,
    SemanticMutationProposalReviewStatus, evaluate_semantic_mutation_proposal_review,
    semantic_mutation_feedback_summary,
};
pub use assessment::{
    AssessmentAssertion, AssessmentAssertionReport, AssessmentError, AssessmentExpectation,
    AssessmentQuery, AssessmentReport, AssessmentSpec, AssessmentStatus, RuntimeAssessmentRequest,
    RuntimeAssessmentResult, query_evaluation, run_evaluation_assessment, run_graph_assessment,
    run_runtime_assessment,
};
pub use capability::{
    AnalysisScope, CapabilityCostClass, CapabilityDescriptor, CapabilityError, CapabilityKind,
    CapabilityMaturity, CapabilityModelPatch, CapabilityReadinessReport, CapabilityReadinessStatus,
    CapabilityRegistry, CapabilityRunReport, CapabilityRunRequest, CapabilityRunStatus,
    CapabilityTarget, DecisionAssessment, DecisionContext, EvidenceEdge, EvidenceGraph,
    EvidenceNode, EvidenceNodeKind, EvidenceRelation, GenericImpactCapability,
    GenericModelInspectionCapability, InsightConfidence, InsightKind, InsightPolarity,
    InsightScope, InsightSeverity, PatchConfidence, SemanticArtifact, SemanticCapability,
    SemanticDiagnostic, SemanticDiagnosticSeverity, SemanticInsight, SemanticWorkspaceSnapshot,
    assess_decision_context,
};
pub use cognitive::{
    CognitiveCandidate, CognitiveCitation, CognitiveConfidence, CognitiveContext,
    CognitiveDiagnostic, CognitiveDiagnosticSeverity, CognitiveElement, CognitiveError,
    CognitiveFocus, CognitiveInferenceRequest, CognitiveInferenceResponse, CognitiveOperation,
    CognitiveProvider, CognitiveProviderStatus, CognitiveRelationship, DesignDecision,
    DesignIntent, HeuristicCognitiveProvider, SemanticWorkspaceRef, analyze, critique,
    design_intent_to_assessment_spec, design_intent_to_semantic_goal_spec, explore, propose,
};
pub use datalog::{
    Atom, CORE_RULEPACK_ID, CORE_RULEPACK_VERSION, DatalogError, DerivedIndexes, DiagnosticRule,
    Evaluation, Explanation, Fact, Rule, RuleDiagnostic, RuleDiagnosticSeverity, RulePack, Term,
    evaluate, evaluate_diagnostics, extract_graph_facts, load_default_rulepacks,
    materialize_core_indexes,
};
pub use derived::{
    DerivedFeatureCache, DerivedFeatureManifest, DerivedFeatureManifestError,
    DerivedFeatureRegistry, DerivedFeatureRule, DerivedFeatureSpec, DerivedPropertySource,
    DerivedPropertyValue, builtin_core_derived_feature_manifest, derived_properties,
    derived_property, manifest_from_metadata,
};
pub use dsl::{
    DSL_ANALYSIS_RUN_ARTIFACT_KIND, DSL_QUERY_ARTIFACT_KIND, DslAnalysisRunReport,
    DslAnalysisRunRequest, DslAnalysisRunSpec, DslDiagnostic, DslDiagnosticCategory, DslEngine,
    DslError, DslExecutionLimits, DslExtensionSpec, DslFieldSchema, DslModelSetFunction,
    DslQueryReport, DslQueryRequest, DslQueryResult, DslSchema, RhaiEngine,
};
pub use expression::{
    BinaryExpressionOp, ExpressionEvaluationContext, ExpressionEvaluationError, ExpressionIr,
    ExpressionIrError, ExpressionPathRoot, ExpressionPathSegment, ExpressionValidationError,
    UnaryExpressionOp,
};
pub use feasibility::{
    CoreMutationFeasibilityService, FeasibilityIssue, FeasibilityIssueKind, FeasibilityRepairHint,
    FeasibilityRepairHintKind, FeasibilityStatus, MutationContext, MutationFeasibilityReport,
    MutationFeasibilityService, RequiredChoice, workspace_revision_for_project,
};
pub use frontend::pilot::{
    PilotDocumentationBlock, PilotExportDocument, PilotExportElement, PilotExportRelationship,
    PilotImportError, PilotSource, load_pilot_export, normalize_pilot_export,
    normalize_pilot_export_for_compare,
};
pub use goal::{
    GoalCheckEvaluation, GoalEvaluation, GoalPolicy, SemanticGoalCheck, SemanticGoalExplanation,
    SemanticGoalProfile, SemanticGoalProfileKind, SemanticGoalSpec, default_model_quality_profile,
    evaluate_semantic_goal, explain_semantic_goal,
};
pub use graph::{Edge, Element, Graph, GraphError, NodeId};
pub use identity::{
    ConceptId, ElementId, PackageId, ProfileId, RelationshipId, SEMANTIC_ANCHOR_SCHEMA,
    SemanticAnchor, SemanticAnchorResolution, SemanticAnchorResolutionStatus, SourceSpanRef,
    StdlibVersion, resolve_semantic_anchor, semantic_anchor_for_element, stable_digest,
    workspace_revision_for_kir_document,
};
pub use ir::{
    Diagnostic, DiagnosticKind, KIR_PROP_MEMBERS, KIR_PROP_NAME, KIR_PROP_OWNER,
    KIR_PROP_SPECIALIZES, KIR_PROP_TYPE, KIR_SCHEMA_VERSION, KirDocument, KirElement, KirError,
    KirFieldKind, KirFieldRegistry, KirFieldSpec, REPRESENTATIVE_KIR_JSON, Severity,
    load_model_stack, load_model_stack_with_registry,
};
pub use language::{
    BaselineLibrary, CURRENT_DEFAULT_PROFILE_ID, Concept, LanguageId, LanguageProfile,
    LanguageProfileError, LibraryContext, MetamodelConceptRegistry, default_language_profile,
    default_metamodel_registry, load_language_profile,
};
pub use library::{
    BaselineLibraryConfig, KparLocator, KparPackageBuild, KparPackageSource, LibraryCacheMetadata,
    LibraryProviderConfig, LocalPackageManifest, LocalPackageRecord, LocalPackageRepository,
    LocalPackageSource, MercurioLockFile, MercurioLockedPackage, MercurioPackageBuildProvenance,
    MercurioPackageDependency, MercurioPackageManifest, MercurioPackageSourceProvenance,
    PackageKirCache, PackageKirCacheManifest, PackageReference, ResolvedLibraryArtifact,
    load_baseline_library_document, package_bytes_digest, parse_package_reference,
    write_kpar_package,
};
pub use metadata::{
    ElementMetadataView, KirMetadataAnnotation, MetadataView, metadata_annotations,
    metadata_annotations_named, metadata_string_property,
};
pub use metamodel::{
    AttributeRow, AttributeValueSource, DerivedMetamodelCapabilities, ElementAttributeQuery,
    ElementSummary, MetamodelAttributeDeclaration, MetamodelAttributeRegistry, MetamodelClassView,
    MetamodelFeatureRegistry, MetamodelFeatureView, MetamodelValidationDiagnostic,
    MetatypeQueryOverride, collect_specialization_ancestors, derive_metamodel_capabilities,
    effective_properties, effective_properties_with_derived, element_metatype,
    query_element_attributes, validate_derived_metamodel_semantics,
};
pub use model_state::{
    InputSource, InputSourceKind, InputSourceSet, MODEL_SERVICE_API_VERSION, ModelArtifact,
    ModelBuildRecord, ModelRevision, ModelRevisionDescriptor, ModelRevisionEnvelope,
    ModelRevisionId, ModelRevisionProducer, ModelRevisionPushMode, ModelServicePullRequest,
    ModelServicePullResponse, ModelServicePushRevisionRequest, ModelServicePushRevisionResponse,
    ModelServicePushStatus, ModelState, ModelStateDescriptor, ModelStateError, ModelStateId,
    RemoteModelRef,
};
pub use mpack::{
    MpackLanguageProfile, MpackLibrary, MpackManifest, MpackPythonPackage,
    MpackPythonWrapperBinding, MpackRequirements, MpackRulepack, MpackService,
    MpackValidationError, validate_mpack_manifest,
};
pub use mutation::{
    AI_SEMANTIC_CONTEXT_SCHEMA_VERSION, AiSemanticContextUsage, ChangedAttribute,
    ChangedSpecialization, ElementRef, MovedElement, MutationApplicationResult, MutationEvidence,
    MutationPlan, MutationProposal, RelationshipChange, RenamedElement, RetypedUsage,
    SemanticAffordanceContext, SemanticDiff, SemanticDiffElementRef, SemanticElementContext,
    SemanticElementKind, SemanticElementRef, SemanticExpression, SemanticFactContext,
    SemanticMutation, SemanticMutationCapabilityContext, SemanticReasoningContext,
    SemanticRelationshipContext, SemanticRelationshipTargetRuleContext,
    SemanticUsageTypingRuleContext, WorkspaceRevision,
    default_semantic_mutation_capability_context, diff_kir_documents,
    enrich_semantic_reasoning_context_with_child_affordances,
    enrich_semantic_reasoning_context_with_child_affordances_for_capability,
    enrich_semantic_reasoning_context_with_child_affordances_for_capability_and_oracle,
    enrich_semantic_reasoning_context_with_graph, mutation_application_digest,
    mutation_proposal_digest, semantic_reasoning_context_from_authoring_project,
    semantic_reasoning_context_from_authoring_project_with_oracle,
};
pub use outline::{
    EditorOutlineKey, EditorOutlineNodeDto, build_editor_outline,
    build_editor_outline_index_for_graph, build_semantic_editor_outline_from_document,
};
pub use paths::{
    bundled_extension_repo_path, bundled_package_repo_path, bundled_stdlib_package_set_path,
    default_kernel_library_path, default_package_kir_cache_path, default_package_repo_path,
    default_stdlib_path, default_stdlib_rulepack_path, default_user_config_path,
    default_workspace_root, repo_path, repo_root,
};
pub use performance::{
    CachePerformanceConfig, CachePerformanceReport, CachePerformanceScenarioReport,
    CachePerformanceTimings, CoreScalabilityCreationStrategy, CoreScalabilityMetricConfig,
    CoreScalabilityReport, CoreScalabilityScenarioReport, CoreScalabilityTimings,
    EmfComparisonReport, KirPerformanceConfig, KirPerformanceMemory, KirPerformanceReport,
    KirPerformanceScenarioReport, KirPerformanceTimings, MemoryMetric, SemanticDiffSummary,
    TimingMetric, run_cache_performance, run_core_scalability_metric, run_kir_performance,
};
pub use plugin_registry::{
    InstalledMpack, MpackActivationIndex, MpackAssetRef,
    PluginInstallSource as RegistryPluginInstallSource, PluginRegistryError,
    default_plugin_registry_root, install_plugin_manifest, installed_mpack_manifests,
    installed_plugin_manifest_paths, mpack_activation_index, plugin_manifest_dir,
    plugin_package_digest, plugin_registry_root, publish_plugin_package,
    read_plugin_install_source, read_plugin_manifest as read_registry_plugin_manifest,
};
pub use project_sources::{
    PROJECT_DESCRIPTOR_FILE_NAME, ProjectSourceFile, ProjectSourceMode, ProjectSourceSet,
    resolve_project_sources,
};
pub use proposal::{
    Proposal, ProposalStatus, PullRequestBinding, PullRequestState, SemanticImpact,
    SemanticImpactStatus, SemanticImpactSummary,
};
pub use python_codegen::{
    PythonWrapperGeneration, generate_python_wrappers, generate_rust_stdlib_consts,
};
pub use query::{
    FilterExpr, OrderBy, Projection, Query, QueryEngine, QueryError, QueryResultSet, QuerySource,
    SortDirection, TermPattern, TriplePattern, elements_with_metadata, parse_query,
};
pub use semantic_compare::{
    SEMANTIC_MODEL_COMPARE_REPORT_SCHEMA_VERSION, SemanticCompareError, SemanticCompareOptions,
    SemanticComparisonReport, SemanticElementMismatch, SemanticModelChange,
    SemanticModelChangeKind, SemanticModelCompareReport, SemanticModelCompareSection,
    SemanticModelCompareSummary, SemanticModelPropertyChange, SemanticModelRelationshipChange,
    SemanticModelRelationshipChangeKind, SemanticSnapshot, SemanticSnapshotAttribute,
    SemanticSnapshotElement, SemanticSourceSpan, SemanticValueMismatch, SnapshotMode,
    build_semantic_snapshot, build_semantic_snapshot_with_registry, compare_snapshots,
    compare_snapshots_with_options, semantic_model_compare_report_from_diff,
};
pub use semantic_legality::{
    SEMANTIC_LEGALITY_SCHEMA_VERSION, SemanticLegalityDiagnostic, SemanticLegalityDiagnosticSource,
    SemanticLegalityOperation, SemanticLegalityReport, SemanticLegalityRequest,
    SemanticLegalityService, SemanticLegalityStatus,
};
pub use semantic_next_actions::{
    SEMANTIC_NEXT_ACTIONS_SCHEMA_VERSION, SemanticNextAction, SemanticNextActionOperation,
    SemanticNextActionTarget, SemanticNextActionsReport, SemanticNextActionsRequest,
    SemanticNextActionsService, enrich_semantic_reasoning_context_with_next_action_affordances,
};
pub use semantic_profile::{
    AttributePolicyAnswer, AttributePolicyKey, CapabilityAnswer, CapabilityPair,
    ConservativeSemanticCapabilityOracle, RelationshipCapability, SemanticCapabilityOracle,
    SemanticCapabilityProfile, SemanticElementAuthoring, SemanticElementForm,
    TableSemanticCapabilityOracle, normalize_capability_token,
};
pub use semantic_target::{
    IncludeSubtypes, ResolvedSemanticTarget, SemanticTarget, SemanticTargetError,
    SemanticTargetResolver, TargetLayers,
};
pub use semantic_validation::{
    SEMANTIC_VALIDATION_POLICY_VERSION, SemanticValidationMode, SemanticValidationPolicy,
    SemanticValidationReport, SemanticValidationSeverity, validate_kir_semantics,
    validate_kir_semantics_for_graph, validate_kir_semantics_for_graph_with_policy,
    validate_kir_semantics_with_context, validate_kir_semantics_with_context_and_policy,
    validate_kir_semantics_with_policy,
};
pub use source_set::{
    SourceDocument, compile_source_document_with_registry, compile_source_documents,
    compile_source_documents_with_registry, parse_source_module,
};
pub use syntax_compare::{
    SyntaxComparisonReport, SyntaxNodeMismatch, SyntaxSnapshot, SyntaxSnapshotNode,
    SyntaxSourceSpan, build_rust_syntax_snapshot, compare_syntax_snapshots,
};
pub use transaction::{
    SEMANTIC_CHANGE_SET_SCHEMA, SEMANTIC_TRANSACTION_SCHEMA, SemanticChangeSet,
    SemanticTransaction, SemanticTransactionReport, TransactionArtifact, TransactionDiagnostic,
    TransactionDiagnosticSeverity, TransactionIsolation, TransactionOperation, TransactionStatus,
};
pub use variant::{
    CoreSemanticVariantService, SEMANTIC_VARIANT_SCHEMA_VERSION, SemanticVariantAuthority,
    SemanticVariantCapabilityContext, SemanticVariantPreview, SemanticVariantRequest,
    SemanticVariantService, SemanticVariantStatus, default_semantic_variant_capability_context,
};
pub use workspace_cache::{
    PersistentCacheStatus, PersistentCompileResult, PersistentWorkspaceCache,
    PersistentWorkspaceCacheOptions, RUNTIME_CACHE_FORMAT_VERSION, RuntimeCacheManifest,
    RuntimeCachePolicy, WorkspaceCompileArtifactKey, WorkspaceCompileCacheManifest,
    WorkspaceCompileCacheOutputs, WorkspaceSourceFileFingerprint, load_runtime_artifact_cache,
    load_runtime_artifact_cache_with_rejection_reason, runtime_artifact_from_binary_bytes,
    runtime_artifact_to_binary_bytes, source_file_fingerprints, workspace_compile_artifact_key,
    write_runtime_artifact_cache,
};
