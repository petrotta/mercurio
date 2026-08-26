use serde::{Deserialize, Serialize};

use crate::semantic_services::feasibility::{
    CoreMutationFeasibilityService, FeasibilityStatus, MutationContext, MutationFeasibilityReport,
    MutationFeasibilityService,
};
use crate::semantic_services::identity::stable_digest;
use crate::semantic_services::mutation::{MutationProposal, SemanticDiff, WorkspaceRevision};
use crate::semantic_services::semantic_profile::{
    ConservativeSemanticCapabilityOracle, SemanticCapabilityOracle,
};

pub const SEMANTIC_VARIANT_SCHEMA_VERSION: &str = "mercurio.semantic_variant.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVariantCapabilityContext {
    pub schema_version: String,
    pub supported_operations: Vec<String>,
    pub authority: String,
    pub guidance: Vec<String>,
}

impl Default for SemanticVariantCapabilityContext {
    fn default() -> Self {
        default_semantic_variant_capability_context()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVariantRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub proposal: MutationProposal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVariantPreview {
    pub schema_version: String,
    pub variant_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub base_revision: WorkspaceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_revision: Option<WorkspaceRevision>,
    pub status: SemanticVariantStatus,
    pub feasibility: MutationFeasibilityReport,
    pub diff: SemanticDiff,
    pub authority: SemanticVariantAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticVariantStatus {
    ReadyToReview,
    NeedsRevision,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVariantAuthority {
    pub baseline_unchanged: bool,
    pub apply_requires_checked_plan: bool,
    pub discard_is_noop: bool,
}

pub trait SemanticVariantService {
    fn preview_variant(
        &self,
        context: &MutationContext,
        request: &SemanticVariantRequest,
    ) -> SemanticVariantPreview;
}

#[derive(Debug, Clone)]
pub struct CoreSemanticVariantService<O = ConservativeSemanticCapabilityOracle> {
    feasibility: CoreMutationFeasibilityService<O>,
}

impl CoreSemanticVariantService<ConservativeSemanticCapabilityOracle> {
    pub fn new() -> Self {
        Self {
            feasibility: CoreMutationFeasibilityService::new(),
        }
    }
}

impl Default for CoreSemanticVariantService<ConservativeSemanticCapabilityOracle> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> CoreSemanticVariantService<O>
where
    O: SemanticCapabilityOracle,
{
    pub fn with_oracle(oracle: O) -> Self {
        Self {
            feasibility: CoreMutationFeasibilityService::with_oracle(oracle),
        }
    }
}

impl<O> SemanticVariantService for CoreSemanticVariantService<O>
where
    O: SemanticCapabilityOracle,
{
    fn preview_variant(
        &self,
        context: &MutationContext,
        request: &SemanticVariantRequest,
    ) -> SemanticVariantPreview {
        let feasibility = self.feasibility.check(context, &request.proposal);
        let diff = feasibility.resulting_diff.clone().unwrap_or_default();
        let status = variant_status(feasibility.status);
        let variant_id = resolved_variant_id(request, &context.workspace_revision);
        let variant_revision =
            matches!(status, SemanticVariantStatus::ReadyToReview).then(|| WorkspaceRevision {
                fingerprint: variant_fingerprint(&variant_id, &context.workspace_revision, &diff),
            });

        SemanticVariantPreview {
            schema_version: SEMANTIC_VARIANT_SCHEMA_VERSION.to_string(),
            variant_id,
            label: request.label.clone(),
            goal: request.goal.clone(),
            base_revision: context.workspace_revision.clone(),
            variant_revision,
            status,
            feasibility,
            diff,
            authority: SemanticVariantAuthority {
                baseline_unchanged: true,
                apply_requires_checked_plan: true,
                discard_is_noop: true,
            },
        }
    }
}

pub fn default_semantic_variant_capability_context() -> SemanticVariantCapabilityContext {
    SemanticVariantCapabilityContext {
        schema_version: SEMANTIC_VARIANT_SCHEMA_VERSION.to_string(),
        supported_operations: vec![
            "CreateExplorationVariant".to_string(),
            "PreviewVariant".to_string(),
            "CompareVariantToBase".to_string(),
        ],
        authority: "Variants are semantic previews over a base revision; creating or previewing a variant does not change the accepted baseline.".to_string(),
        guidance: vec![
            "Use a semantic variant for exploration, trade studies, alternatives, and what-if design work.".to_string(),
            "Apply semantic mutations inside the variant preview before asking the user to accept baseline changes.".to_string(),
            "Accepting a variant requires the checked mutation plan from core feasibility; discarding a variant is a no-op.".to_string(),
        ],
    }
}

fn variant_status(status: FeasibilityStatus) -> SemanticVariantStatus {
    match status {
        FeasibilityStatus::Allowed | FeasibilityStatus::AllowedWithWarnings => {
            SemanticVariantStatus::ReadyToReview
        }
        FeasibilityStatus::Blocked => SemanticVariantStatus::Blocked,
        FeasibilityStatus::RequiresDisambiguation
        | FeasibilityStatus::RequiresSupportingChanges
        | FeasibilityStatus::UnsupportedByAuthoringBackend => SemanticVariantStatus::NeedsRevision,
    }
}

fn resolved_variant_id(
    request: &SemanticVariantRequest,
    base_revision: &WorkspaceRevision,
) -> String {
    request
        .variant_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "variant.{}",
                stable_digest([
                    ("label".as_bytes(), request.label.as_bytes()),
                    (
                        "goal".as_bytes(),
                        request.goal.as_deref().unwrap_or_default().as_bytes(),
                    ),
                    ("base".as_bytes(), base_revision.fingerprint.as_bytes()),
                ])
            )
        })
}

fn variant_fingerprint(
    variant_id: &str,
    base_revision: &WorkspaceRevision,
    diff: &SemanticDiff,
) -> String {
    let diff_json = serde_json::to_string(diff).unwrap_or_else(|error| error.to_string());
    stable_digest([
        ("variant".as_bytes(), variant_id.as_bytes()),
        ("base".as_bytes(), base_revision.fingerprint.as_bytes()),
        ("diff".as_bytes(), diff_json.as_bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::authoring::authoring::load_authoring_project_from_model;
    use crate::semantic_services::mutation::{ElementRef, SemanticMutation};

    fn context() -> MutationContext {
        let project = load_authoring_project_from_model(BTreeMap::from([(
            "vehicle.model".to_string(),
            "package Vehicle { part def Vehicle; }".to_string(),
        )]))
        .unwrap();
        MutationContext::from_project(project)
    }

    #[test]
    fn semantic_variant_preview_does_not_change_base_revision() {
        let context = context();
        let proposal = MutationProposal {
            intent: "Explore electric motor alternative".to_string(),
            operations: vec![SemanticMutation::AddDefinition {
                container: ElementRef::new("Vehicle"),
                keyword: "part".to_string(),
                name: "ElectricMotor".to_string(),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };
        let request = SemanticVariantRequest {
            variant_id: Some("variant.electric".to_string()),
            label: "Electric motor alternative".to_string(),
            goal: Some("Explore propulsion alternatives".to_string()),
            proposal,
        };

        let preview = CoreSemanticVariantService::new().preview_variant(&context, &request);

        assert_eq!(preview.status, SemanticVariantStatus::ReadyToReview);
        assert_eq!(preview.base_revision, context.workspace_revision);
        assert_ne!(
            preview.variant_revision.as_ref(),
            Some(&preview.base_revision)
        );
        assert!(preview.authority.baseline_unchanged);
        assert!(preview.diff.added_elements.iter().any(|element| {
            element.element_id == "type.Vehicle.ElectricMotor"
                || element.label.as_deref() == Some("ElectricMotor")
        }));
    }

    #[test]
    fn semantic_variant_preview_carries_blocked_feasibility() {
        let context = context();
        let proposal = MutationProposal {
            intent: "Explore impossible nested type".to_string(),
            operations: vec![SemanticMutation::AddDefinition {
                container: ElementRef::new("Vehicle.Missing"),
                keyword: "part".to_string(),
                name: "Nested".to_string(),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };
        let request = SemanticVariantRequest {
            variant_id: None,
            label: "Impossible alternative".to_string(),
            goal: None,
            proposal,
        };

        let preview = CoreSemanticVariantService::new().preview_variant(&context, &request);

        assert_eq!(preview.status, SemanticVariantStatus::Blocked);
        assert!(preview.variant_revision.is_none());
        assert!(!preview.feasibility.blocking_reasons.is_empty());
        assert!(preview.authority.baseline_unchanged);
    }
}
