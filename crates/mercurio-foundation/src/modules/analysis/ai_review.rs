use serde::{Deserialize, Serialize};

use crate::semantic_services::feasibility::{FeasibilityStatus, MutationFeasibilityReport};
use crate::semantic_services::mutation::MutationProposal;

pub const AI_MUTATION_REVIEW_SCHEMA_VERSION: &str = "mercurio.ai.mutation_review.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMutationProposalReviewInput {
    pub proposal: MutationProposal,
    pub feasibility: MutationFeasibilityReport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback: Vec<SemanticMutationProposalFeedback>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMutationProposalFeedback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    pub decision: SemanticMutationProposalFeedbackDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_repair_hint_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticMutationProposalFeedbackDecision {
    Accepted,
    Rejected,
    NeedsRevision,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMutationProposalReview {
    pub schema_version: String,
    pub status: SemanticMutationProposalReviewStatus,
    pub score: f64,
    pub operation_count: usize,
    pub repair_hint_count: usize,
    pub blockers: Vec<String>,
    pub recommended_next_action: String,
    pub feedback_summary: SemanticMutationFeedbackSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticMutationProposalReviewStatus {
    ReadyToApply,
    NeedsRevision,
    Blocked,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMutationFeedbackSummary {
    pub accepted: usize,
    pub rejected: usize,
    pub needs_revision: usize,
    pub applied: usize,
}

pub fn evaluate_semantic_mutation_proposal_review(
    input: &SemanticMutationProposalReviewInput,
) -> SemanticMutationProposalReview {
    let feedback_summary = semantic_mutation_feedback_summary(&input.feedback);
    let mut blockers = Vec::new();
    blockers.extend(
        input
            .feasibility
            .blocking_reasons
            .iter()
            .map(|issue| issue.message.clone()),
    );

    let status = if feedback_summary.applied > 0 {
        SemanticMutationProposalReviewStatus::Applied
    } else if feedback_summary.rejected > 0 || feedback_summary.needs_revision > 0 {
        SemanticMutationProposalReviewStatus::NeedsRevision
    } else {
        match input.feasibility.status {
            FeasibilityStatus::Allowed | FeasibilityStatus::AllowedWithWarnings => {
                SemanticMutationProposalReviewStatus::ReadyToApply
            }
            FeasibilityStatus::Blocked => SemanticMutationProposalReviewStatus::Blocked,
            FeasibilityStatus::RequiresDisambiguation
            | FeasibilityStatus::RequiresSupportingChanges
            | FeasibilityStatus::UnsupportedByAuthoringBackend => {
                SemanticMutationProposalReviewStatus::NeedsRevision
            }
        }
    };

    let score = review_score(status, &input.feasibility);
    let recommended_next_action = recommended_review_action(status, &input.feasibility);

    SemanticMutationProposalReview {
        schema_version: AI_MUTATION_REVIEW_SCHEMA_VERSION.to_string(),
        status,
        score,
        operation_count: input.proposal.operations.len(),
        repair_hint_count: input.feasibility.repair_hints.len(),
        blockers,
        recommended_next_action,
        feedback_summary,
    }
}

pub fn semantic_mutation_feedback_summary(
    feedback: &[SemanticMutationProposalFeedback],
) -> SemanticMutationFeedbackSummary {
    let mut summary = SemanticMutationFeedbackSummary {
        accepted: 0,
        rejected: 0,
        needs_revision: 0,
        applied: 0,
    };
    for entry in feedback {
        match entry.decision {
            SemanticMutationProposalFeedbackDecision::Accepted => summary.accepted += 1,
            SemanticMutationProposalFeedbackDecision::Rejected => summary.rejected += 1,
            SemanticMutationProposalFeedbackDecision::NeedsRevision => summary.needs_revision += 1,
            SemanticMutationProposalFeedbackDecision::Applied => summary.applied += 1,
        }
    }
    summary
}

fn review_score(
    status: SemanticMutationProposalReviewStatus,
    feasibility: &MutationFeasibilityReport,
) -> f64 {
    match status {
        SemanticMutationProposalReviewStatus::Applied => 1.0,
        SemanticMutationProposalReviewStatus::ReadyToApply => {
            if feasibility.warnings.is_empty() {
                0.95
            } else {
                0.8
            }
        }
        SemanticMutationProposalReviewStatus::NeedsRevision => 0.45,
        SemanticMutationProposalReviewStatus::Blocked => 0.0,
    }
}

fn recommended_review_action(
    status: SemanticMutationProposalReviewStatus,
    feasibility: &MutationFeasibilityReport,
) -> String {
    match status {
        SemanticMutationProposalReviewStatus::Applied => "No action required".to_string(),
        SemanticMutationProposalReviewStatus::ReadyToApply => {
            "Review semantic diff, then apply the checked mutation plan".to_string()
        }
        SemanticMutationProposalReviewStatus::NeedsRevision
            if !feasibility.repair_hints.is_empty() =>
        {
            "Revise the proposal using the core repair hints".to_string()
        }
        SemanticMutationProposalReviewStatus::NeedsRevision => {
            "Revise the proposal against the latest semantic context".to_string()
        }
        SemanticMutationProposalReviewStatus::Blocked if !feasibility.repair_hints.is_empty() => {
            "Resolve blocking feasibility issues using the core repair hints".to_string()
        }
        SemanticMutationProposalReviewStatus::Blocked => {
            "Resolve blocking feasibility issues before applying".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_services::feasibility::{
        FeasibilityIssue, FeasibilityIssueKind, FeasibilityRepairHint, FeasibilityRepairHintKind,
        MutationFeasibilityReport,
    };
    use crate::semantic_services::mutation::{MutationProposal, SemanticDiff, WorkspaceRevision};

    fn proposal() -> MutationProposal {
        MutationProposal {
            intent: "Evaluate proposal".to_string(),
            operations: Vec::new(),
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: WorkspaceRevision {
                fingerprint: "rev".to_string(),
            },
        }
    }

    fn feasibility(status: FeasibilityStatus) -> MutationFeasibilityReport {
        MutationFeasibilityReport {
            status,
            normalized_plan: None,
            blocking_reasons: Vec::new(),
            warnings: Vec::new(),
            required_choices: Vec::new(),
            suggested_supporting_changes: Vec::new(),
            repair_hints: Vec::new(),
            resulting_diff: Some(SemanticDiff::default()),
            checked_against: WorkspaceRevision {
                fingerprint: "rev".to_string(),
            },
        }
    }

    #[test]
    fn allowed_proposal_review_is_ready_to_apply() {
        let input = SemanticMutationProposalReviewInput {
            proposal: proposal(),
            feasibility: feasibility(FeasibilityStatus::Allowed),
            feedback: Vec::new(),
        };

        let review = evaluate_semantic_mutation_proposal_review(&input);

        assert_eq!(
            review.status,
            SemanticMutationProposalReviewStatus::ReadyToApply
        );
        assert_eq!(review.score, 0.95);
    }

    #[test]
    fn blocked_proposal_review_carries_blockers_and_repair_hints() {
        let mut feasibility = feasibility(FeasibilityStatus::Blocked);
        feasibility.blocking_reasons.push(FeasibilityIssue {
            kind: FeasibilityIssueKind::ResolutionFailure,
            operation_index: Some(0),
            message: "missing type: Demo.Part".to_string(),
        });
        feasibility.repair_hints.push(FeasibilityRepairHint {
            kind: FeasibilityRepairHintKind::UseExistingElement,
            operation_index: Some(0),
            message: "Use an existing element reference from the semantic context".to_string(),
            suggested_operation: None,
        });
        let input = SemanticMutationProposalReviewInput {
            proposal: proposal(),
            feasibility,
            feedback: Vec::new(),
        };

        let review = evaluate_semantic_mutation_proposal_review(&input);

        assert_eq!(review.status, SemanticMutationProposalReviewStatus::Blocked);
        assert_eq!(review.repair_hint_count, 1);
        assert_eq!(review.blockers, vec!["missing type: Demo.Part"]);
        assert!(review.recommended_next_action.contains("core repair hints"));
    }

    #[test]
    fn feedback_can_force_revision_status() {
        let input = SemanticMutationProposalReviewInput {
            proposal: proposal(),
            feasibility: feasibility(FeasibilityStatus::Allowed),
            feedback: vec![SemanticMutationProposalFeedback {
                proposal_id: Some("p1".to_string()),
                decision: SemanticMutationProposalFeedbackDecision::NeedsRevision,
                rationale: Some("Wrong target".to_string()),
                selected_repair_hint_indices: Vec::new(),
                tags: vec!["target".to_string()],
            }],
        };

        let review = evaluate_semantic_mutation_proposal_review(&input);

        assert_eq!(
            review.status,
            SemanticMutationProposalReviewStatus::NeedsRevision
        );
        assert_eq!(review.feedback_summary.needs_revision, 1);
    }
}
