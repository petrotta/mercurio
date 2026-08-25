pub mod ai_review;
pub mod analysis;
pub mod assessment;
pub mod capability;
pub mod cognitive;
pub mod goal;
pub mod semantic_compare;

pub use ai_review::{
    AI_MUTATION_REVIEW_SCHEMA_VERSION, SemanticMutationFeedbackSummary,
    SemanticMutationProposalFeedback, SemanticMutationProposalFeedbackDecision,
    SemanticMutationProposalReview, SemanticMutationProposalReviewInput,
    SemanticMutationProposalReviewStatus, evaluate_semantic_mutation_proposal_review,
    semantic_mutation_feedback_summary,
};
pub use analysis::{
    AnalysisCapabilityDescriptor, AnalysisCapabilityEffect, AnalysisCapabilityProviderKind,
    AnalysisCapabilitySelector, AnalysisCaseModel, AnalysisElementRef, AnalysisInventory,
    AnalysisOpportunity, AnalysisOpportunityKind, AnalysisOpportunityReadiness,
    AnalysisOpportunityReport, AnalysisStructuralPredicate, AnalysisTechniqueKind,
    AnalysisWorkflow, AnalysisWorkflowStep, AnalysisWorkflowStepKind, RequirementEvaluationModel,
    builtin_analysis_capability_descriptors,
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
    SemanticDiagnostic, SemanticDiagnosticSeverity, SemanticElementRef, SemanticInsight,
    SemanticWorkspaceSnapshot, assess_decision_context,
};
pub use cognitive::{
    CognitiveCandidate, CognitiveCitation, CognitiveConfidence, CognitiveContext,
    CognitiveDiagnostic, CognitiveDiagnosticSeverity, CognitiveElement, CognitiveError,
    CognitiveFocus, CognitiveInferenceRequest, CognitiveInferenceResponse, CognitiveOperation,
    CognitiveProvider, CognitiveProviderStatus, CognitiveRelationship, DesignDecision,
    DesignIntent, HeuristicCognitiveProvider, SemanticWorkspaceRef, analyze, critique,
    design_intent_to_assessment_spec, design_intent_to_semantic_goal_spec, explore, propose,
};
pub use goal::{
    GoalCheckEvaluation, GoalEvaluation, GoalPolicy, SemanticGoalCheck, SemanticGoalExplanation,
    SemanticGoalProfile, SemanticGoalProfileKind, SemanticGoalSpec, default_model_quality_profile,
    evaluate_semantic_goal, explain_semantic_goal,
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
