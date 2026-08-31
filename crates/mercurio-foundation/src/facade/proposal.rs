use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub project_id: String,
    pub base_commit: String,
    pub status: ProposalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PullRequestBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Draft,
    Validated,
    PrReady,
    PrOpen,
    Merged,
    Abandoned,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestBinding {
    pub provider: String,
    pub repository: String,
    pub number: u64,
    pub base_branch: String,
    pub head_branch: String,
    pub state: PullRequestState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticImpact {
    pub project_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub summary: SemanticImpactSummary,
    pub status: SemanticImpactStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticImpactSummary {
    pub added_elements: usize,
    pub removed_elements: usize,
    pub changed_elements: usize,
    pub diagnostics_added: usize,
    pub diagnostics_resolved: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SemanticImpactStatus {
    Unknown,
    Clean,
    Risky,
    Invalid,
}
