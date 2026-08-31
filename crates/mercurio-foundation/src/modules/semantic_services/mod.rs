pub mod feasibility;
pub mod identity;
pub mod mutation;
pub mod semantic_legality;
pub mod semantic_next_actions;
pub mod semantic_profile;
pub mod semantic_validation;
pub mod variant;

pub use feasibility::{
    CoreMutationFeasibilityService, FeasibilityIssue, FeasibilityIssueKind, FeasibilityRepairHint,
    FeasibilityRepairHintKind, FeasibilityStatus, MutationContext, MutationFeasibilityReport,
    MutationFeasibilityService, RequiredChoice, workspace_revision_for_project,
};
pub use identity::{
    ConceptId, ElementId, PackageId, ProfileId, RelationshipId, SEMANTIC_ANCHOR_SCHEMA,
    SemanticAnchor, SemanticAnchorResolution, SemanticAnchorResolutionStatus, SourceSpanRef,
    StdlibVersion, resolve_semantic_anchor, semantic_anchor_for_element, stable_digest,
    workspace_revision_for_kir_document,
};
pub use mutation::{
    AI_SEMANTIC_CONTEXT_SCHEMA_VERSION, AiSemanticContextUsage, ChangedAttribute,
    ChangedSpecialization, ElementRef, ModelChangeEvent, ModelChangeProvenance, MovedElement,
    MutationApplicationResult, MutationEvidence, MutationPlan, MutationProposal,
    RelationshipChange, RenamedElement, RetypedUsage, SemanticAffordanceContext, SemanticDiff,
    SemanticDiffElementRef, SemanticElementContext, SemanticElementKind, SemanticElementRef,
    SemanticExpression, SemanticFactContext, SemanticMutation, SemanticMutationCapabilityContext,
    SemanticReasoningContext, SemanticRelationshipContext, SemanticRelationshipTargetRuleContext,
    SemanticUsageTypingRuleContext, WorkspaceRevision,
    default_semantic_mutation_capability_context, diff_kir_documents,
    enrich_semantic_reasoning_context_with_child_affordances,
    enrich_semantic_reasoning_context_with_child_affordances_for_capability,
    enrich_semantic_reasoning_context_with_child_affordances_for_capability_and_oracle,
    enrich_semantic_reasoning_context_with_graph, mutation_application_digest,
    mutation_proposal_digest, semantic_reasoning_context_from_authoring_project,
    semantic_reasoning_context_from_authoring_project_with_oracle,
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
pub use semantic_validation::{
    SEMANTIC_VALIDATION_POLICY_VERSION, SemanticValidationMode, SemanticValidationPolicy,
    SemanticValidationReport, SemanticValidationSeverity, validate_kir_semantics,
    validate_kir_semantics_for_graph, validate_kir_semantics_for_graph_with_policy,
    validate_kir_semantics_with_context, validate_kir_semantics_with_context_and_policy,
    validate_kir_semantics_with_policy,
};
pub use variant::{
    CoreSemanticVariantService, SEMANTIC_VARIANT_SCHEMA_VERSION, SemanticVariantAuthority,
    SemanticVariantCapabilityContext, SemanticVariantPreview, SemanticVariantRequest,
    SemanticVariantService, SemanticVariantStatus, default_semantic_variant_capability_context,
};
