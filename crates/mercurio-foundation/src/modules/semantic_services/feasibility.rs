use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::authoring::authoring::{
    AttributeWritePolicy, AuthoringModule, AuthoringProject, ContainerSelector, Declaration,
    Mutation, QualifiedName, SemanticEdit,
};
use crate::runtime::RulePack;
use crate::semantic_services::mutation::{
    ElementRef, MutationApplicationResult, MutationPlan, MutationProposal, SemanticDiff,
    SemanticElementKind, SemanticMutation, WorkspaceRevision, diff_for_operation, merge_diff,
};
use crate::semantic_services::semantic_legality::{
    SemanticLegalityReport, SemanticLegalityService, SemanticLegalityStatus,
};
pub use crate::semantic_services::semantic_profile::{
    CapabilityAnswer, ConservativeSemanticCapabilityOracle, SemanticCapabilityOracle,
    SemanticElementForm,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MutationContext {
    pub project: AuthoringProject,
    pub workspace_revision: WorkspaceRevision,
}

impl MutationContext {
    pub fn from_project(project: AuthoringProject) -> Self {
        let workspace_revision = workspace_revision_for_project(&project);
        Self {
            project,
            workspace_revision,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationFeasibilityReport {
    pub status: FeasibilityStatus,
    pub normalized_plan: Option<MutationPlan>,
    pub blocking_reasons: Vec<FeasibilityIssue>,
    pub warnings: Vec<FeasibilityIssue>,
    pub required_choices: Vec<RequiredChoice>,
    pub suggested_supporting_changes: Vec<SemanticMutation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repair_hints: Vec<FeasibilityRepairHint>,
    pub resulting_diff: Option<SemanticDiff>,
    pub checked_against: WorkspaceRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeasibilityStatus {
    Allowed,
    AllowedWithWarnings,
    Blocked,
    RequiresDisambiguation,
    RequiresSupportingChanges,
    UnsupportedByAuthoringBackend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeasibilityIssue {
    pub kind: FeasibilityIssueKind,
    pub operation_index: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeasibilityIssueKind {
    SemanticRuleViolation,
    MetamodelViolation,
    ResolutionFailure,
    NameCollision,
    RequiresImport,
    RequiresSupportingChange,
    UnsupportedByAuthoringBackend,
    StaleWorkspaceRevision,
    ValidationFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredChoice {
    pub operation_index: usize,
    pub message: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeasibilityRepairHint {
    pub kind: FeasibilityRepairHintKind,
    pub operation_index: Option<usize>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_operation: Option<SemanticMutation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeasibilityRepairHintKind {
    UseExistingElement,
    AddSupportingDefinition,
    ReplaceDeprecatedVocabulary,
    UseAllowedRelationshipTarget,
    UseAllowedUsageType,
    RemoveUnsupportedOperation,
    RefreshWorkspaceRevision,
    ReviseProposal,
}

pub trait MutationFeasibilityService {
    fn check(
        &self,
        context: &MutationContext,
        proposal: &MutationProposal,
    ) -> MutationFeasibilityReport;
}

#[derive(Debug, Clone)]
pub struct CoreMutationFeasibilityService<O = ConservativeSemanticCapabilityOracle> {
    legality: SemanticLegalityService<O>,
}

impl CoreMutationFeasibilityService<ConservativeSemanticCapabilityOracle> {
    pub fn new() -> Self {
        Self {
            legality: SemanticLegalityService::new(),
        }
    }
}

impl Default for CoreMutationFeasibilityService<ConservativeSemanticCapabilityOracle> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> CoreMutationFeasibilityService<O>
where
    O: SemanticCapabilityOracle,
{
    pub fn with_oracle(oracle: O) -> Self {
        Self {
            legality: SemanticLegalityService::with_oracle(oracle),
        }
    }

    pub fn with_oracle_and_rulepacks(oracle: O, rulepacks: Vec<RulePack>) -> Self {
        Self {
            legality: SemanticLegalityService::with_oracle_and_rulepacks(oracle, rulepacks),
        }
    }

    pub fn authoring_mutation_for_operation(
        &self,
        project: &AuthoringProject,
        operation: &SemanticMutation,
    ) -> Option<Mutation> {
        self.authoring_mutation_for(project, operation)
    }

    pub fn apply_checked_plan(
        &self,
        context: &MutationContext,
        plan: &MutationPlan,
    ) -> Result<MutationApplicationResult, FeasibilityIssue> {
        if context.workspace_revision != plan.checked_against {
            return Err(FeasibilityIssue {
                kind: FeasibilityIssueKind::StaleWorkspaceRevision,
                operation_index: None,
                message: "workspace changed after feasibility was checked".to_string(),
            });
        }

        let before_kir = context.project.compile_kir_document().ok();
        let mut project = context.project.clone();
        let mut changed_files = BTreeSet::new();
        let mut changed_declarations = BTreeSet::new();
        let mut semantic_diff = SemanticDiff::default();

        for (index, operation) in plan.normalized_operations.iter().enumerate() {
            let result = match operation {
                SemanticMutation::SetAttribute {
                    element,
                    attribute,
                    value,
                } => project.apply_semantic_edit(SemanticEdit::SetAttribute {
                    element: element.as_qualified_name(),
                    attribute: attribute.clone(),
                    value: value.clone(),
                    policy: AttributeWritePolicy::UpsertDirect,
                }),
                _ => {
                    let Some(mutation) = self.authoring_mutation_for(&project, operation) else {
                        return Err(FeasibilityIssue {
                            kind: FeasibilityIssueKind::UnsupportedByAuthoringBackend,
                            operation_index: Some(index),
                            message: "operation is semantically represented but not yet writable"
                                .to_string(),
                        });
                    };
                    project.apply_mutation(mutation)
                }
            }
            .map_err(|err| FeasibilityIssue {
                kind: FeasibilityIssueKind::ValidationFailure,
                operation_index: Some(index),
                message: err.to_string(),
            })?;
            changed_files.extend(result.changed_files.iter().cloned());
            changed_declarations.extend(result.changed_declarations.iter().cloned());
            let (property_files, property_declarations) =
                apply_add_element_properties(&mut project, operation).map_err(|err| {
                    FeasibilityIssue {
                        kind: FeasibilityIssueKind::ValidationFailure,
                        operation_index: Some(index),
                        message: err.to_string(),
                    }
                })?;
            changed_files.extend(property_files);
            changed_declarations.extend(property_declarations);
            merge_diff(
                &mut semantic_diff,
                diff_for_operation(operation, Some(&result)),
            );
        }

        let write_back = project
            .write_back_changed_files(&changed_files)
            .map_err(|err| FeasibilityIssue {
                kind: FeasibilityIssueKind::ValidationFailure,
                operation_index: None,
                message: err.to_string(),
            })?;
        if let (Some(before), Ok(after)) = (before_kir, project.compile_kir_document()) {
            semantic_diff = crate::semantic_services::mutation::diff_kir_documents(&before, &after);
        }

        Ok(MutationApplicationResult {
            changed_files,
            edited_files: write_back.edited_files,
            changed_declarations,
            semantic_diff,
            proposed_digest: None,
            applied_digest: None,
            decided_at: None,
            supersedes: None,
            reverts: None,
            actor: None,
        })
    }
}

impl<O> MutationFeasibilityService for CoreMutationFeasibilityService<O>
where
    O: SemanticCapabilityOracle,
{
    fn check(
        &self,
        context: &MutationContext,
        proposal: &MutationProposal,
    ) -> MutationFeasibilityReport {
        let mut blocking_reasons = Vec::new();
        let mut warnings = Vec::new();
        let required_choices = Vec::new();
        let mut suggested_supporting_changes = Vec::new();
        let mut resulting_diff = SemanticDiff::default();
        let mut changed_files = BTreeSet::new();

        if context.workspace_revision != proposal.workspace_revision {
            blocking_reasons.push(FeasibilityIssue {
                kind: FeasibilityIssueKind::StaleWorkspaceRevision,
                operation_index: None,
                message: "proposal was produced for a different workspace revision".to_string(),
            });
        }

        let before_kir = context.project.compile_kir_document().ok();
        let mut project = context.project.clone();
        let mut unsupported_backend = false;
        let mut requires_supporting_changes = false;
        let mut normalized_operations = Vec::with_capacity(proposal.operations.len());

        for (index, operation) in proposal.operations.iter().enumerate() {
            let operation = self.normalize_operation(operation);
            normalized_operations.push(operation.clone());
            self.check_references(
                &project,
                &operation,
                index,
                &mut blocking_reasons,
                &mut warnings,
                &mut suggested_supporting_changes,
                &mut requires_supporting_changes,
            );
            if operation_requires_supporting_change(&project, &operation) {
                merge_diff(&mut resulting_diff, diff_for_operation(&operation, None));
                continue;
            }

            let result = match &operation {
                SemanticMutation::SetAttribute {
                    element,
                    attribute,
                    value,
                } => project.apply_semantic_edit(SemanticEdit::SetAttribute {
                    element: element.as_qualified_name(),
                    attribute: attribute.clone(),
                    value: value.clone(),
                    policy: AttributeWritePolicy::UpsertDirect,
                }),
                _ => {
                    let Some(mutation) = self.authoring_mutation_for(&project, &operation) else {
                        unsupported_backend = true;
                        warnings.push(FeasibilityIssue {
                            kind: FeasibilityIssueKind::UnsupportedByAuthoringBackend,
                            operation_index: Some(index),
                            message: "operation is represented semantically but has no authoring write-back path yet".to_string(),
                        });
                        merge_diff(&mut resulting_diff, diff_for_operation(&operation, None));
                        continue;
                    };
                    project.apply_mutation(mutation)
                }
            };

            match result {
                Ok(result) => {
                    changed_files.extend(result.changed_files.iter().cloned());
                    match apply_add_element_properties(&mut project, &operation) {
                        Ok((property_files, _)) => {
                            changed_files.extend(property_files);
                        }
                        Err(err) => {
                            blocking_reasons.push(FeasibilityIssue {
                                kind: FeasibilityIssueKind::ValidationFailure,
                                operation_index: Some(index),
                                message: err.to_string(),
                            });
                        }
                    }
                    merge_diff(
                        &mut resulting_diff,
                        diff_for_operation(&operation, Some(&result)),
                    );
                }
                Err(err) => {
                    blocking_reasons.push(FeasibilityIssue {
                        kind: FeasibilityIssueKind::ValidationFailure,
                        operation_index: Some(index),
                        message: err.to_string(),
                    });
                }
            }
        }

        if !changed_files.is_empty()
            && !unsupported_backend
            && !requires_supporting_changes
            && blocking_reasons.is_empty()
        {
            match project.write_back_changed_files(&changed_files) {
                Ok(_) => {
                    if let (Some(before), Ok(after)) = (&before_kir, project.compile_kir_document())
                    {
                        resulting_diff =
                            crate::semantic_services::mutation::diff_kir_documents(before, &after);
                    }
                }
                Err(err) => {
                    blocking_reasons.push(FeasibilityIssue {
                        kind: FeasibilityIssueKind::ValidationFailure,
                        operation_index: None,
                        message: err.to_string(),
                    });
                }
            }
        }

        let status = if !blocking_reasons.is_empty() {
            FeasibilityStatus::Blocked
        } else if requires_supporting_changes {
            FeasibilityStatus::RequiresSupportingChanges
        } else if unsupported_backend {
            FeasibilityStatus::UnsupportedByAuthoringBackend
        } else if !warnings.is_empty() {
            FeasibilityStatus::AllowedWithWarnings
        } else {
            FeasibilityStatus::Allowed
        };

        let normalized_plan = if matches!(
            status,
            FeasibilityStatus::Allowed
                | FeasibilityStatus::AllowedWithWarnings
                | FeasibilityStatus::UnsupportedByAuthoringBackend
        ) {
            Some(MutationPlan {
                proposal_id: proposal_id(proposal),
                normalized_operations,
                required_supporting_changes: suggested_supporting_changes.clone(),
                checked_against: context.workspace_revision.clone(),
            })
        } else {
            None
        };

        let repair_hints =
            feasibility_repair_hints(&blocking_reasons, &warnings, &suggested_supporting_changes);

        MutationFeasibilityReport {
            status,
            normalized_plan,
            blocking_reasons,
            warnings,
            required_choices,
            suggested_supporting_changes,
            repair_hints,
            resulting_diff: Some(resulting_diff),
            checked_against: context.workspace_revision.clone(),
        }
    }
}

fn apply_add_element_properties(
    project: &mut AuthoringProject,
    operation: &SemanticMutation,
) -> Result<(BTreeSet<String>, BTreeSet<String>), crate::authoring::authoring::AuthoringError> {
    let SemanticMutation::AddElement {
        container,
        name,
        properties,
        ..
    } = operation
    else {
        return Ok((BTreeSet::new(), BTreeSet::new()));
    };
    let element = ElementRef::new(format!("{}.{}", container.qualified_name, name));
    let mut changed_files = BTreeSet::new();
    let mut changed_declarations = BTreeSet::new();
    for (attribute, value) in properties
        .iter()
        .filter(|(attribute, _)| !is_structural_add_element_property(attribute))
    {
        let result = project.apply_semantic_edit(SemanticEdit::SetAttribute {
            element: element.as_qualified_name(),
            attribute: attribute.clone(),
            value: value.clone(),
            policy: AttributeWritePolicy::UpsertDirect,
        })?;
        changed_files.extend(result.changed_files);
        changed_declarations.extend(result.changed_declarations);
    }
    Ok((changed_files, changed_declarations))
}

fn is_structural_add_element_property(attribute: &str) -> bool {
    matches!(
        attribute,
        "declared_name" | "qualified_name" | "owner" | "type" | "specializes"
    )
}

impl<O> CoreMutationFeasibilityService<O>
where
    O: SemanticCapabilityOracle,
{
    fn normalize_operation(&self, operation: &SemanticMutation) -> SemanticMutation {
        match operation {
            SemanticMutation::AddDefinition {
                container,
                keyword,
                name,
                specializes,
            } => self
                .legality
                .semantic_kind_for_definition_keyword(keyword)
                .map(|metaclass| SemanticMutation::AddElement {
                    container: container.clone(),
                    kind: SemanticElementKind::new(metaclass),
                    name: name.clone(),
                    ty: None,
                    specializes: specializes.clone(),
                    properties: Default::default(),
                })
                .unwrap_or_else(|| SemanticMutation::AddDefinition {
                    container: container.clone(),
                    keyword: self.legality.normalize_definition_keyword(keyword),
                    name: name.clone(),
                    specializes: specializes.clone(),
                }),
            SemanticMutation::AddUsage {
                container,
                keyword,
                name,
                ty,
                specializes,
            } => self
                .legality
                .semantic_kind_for_usage_keyword(keyword)
                .map(|metaclass| SemanticMutation::AddElement {
                    container: container.clone(),
                    kind: SemanticElementKind::new(metaclass),
                    name: name.clone(),
                    ty: ty.clone(),
                    specializes: specializes.clone(),
                    properties: Default::default(),
                })
                .unwrap_or_else(|| operation.clone()),
            _ => operation.clone(),
        }
    }

    fn authoring_mutation_for(
        &self,
        project: &AuthoringProject,
        operation: &SemanticMutation,
    ) -> Option<Mutation> {
        match operation {
            SemanticMutation::AddPackage { target_file, name } => Some(Mutation::AddPackage {
                target_file: target_file.clone(),
                package_name: QualifiedName::parse(name),
            }),
            SemanticMutation::AddElement {
                container,
                kind,
                name,
                ty,
                specializes,
                ..
            } => {
                let authoring = self.legality.authoring_for_element_kind(&kind.metaclass)?;
                match authoring.form {
                    SemanticElementForm::Definition => Some(Mutation::AddDefinition {
                        container: container_selector_for(project, container),
                        keyword: authoring.keyword,
                        name: name.clone(),
                        specializes: specializes
                            .iter()
                            .map(ElementRef::as_qualified_name)
                            .collect(),
                    }),
                    SemanticElementForm::Usage => Some(Mutation::AddUsage {
                        container: container_selector_for(project, container),
                        keyword: authoring.keyword,
                        name: name.clone(),
                        ty: ty.as_ref().map(ElementRef::as_qualified_name),
                        specializes: specializes
                            .iter()
                            .map(ElementRef::as_qualified_name)
                            .collect(),
                    }),
                }
            }
            SemanticMutation::AddDefinition {
                container,
                keyword,
                name,
                specializes,
            } => Some(Mutation::AddDefinition {
                container: container_selector_for(project, container),
                keyword: self.legality.normalize_definition_keyword(keyword),
                name: name.clone(),
                specializes: specializes
                    .iter()
                    .map(ElementRef::as_qualified_name)
                    .collect(),
            }),
            SemanticMutation::AddUsage {
                container,
                keyword,
                name,
                ty,
                specializes,
            } => Some(Mutation::AddUsage {
                container: container_selector_for(project, container),
                keyword: keyword.clone(),
                name: name.clone(),
                ty: ty.as_ref().map(ElementRef::as_qualified_name),
                specializes: specializes
                    .iter()
                    .map(ElementRef::as_qualified_name)
                    .collect(),
            }),
            SemanticMutation::RenameDeclaration { element, new_name } => {
                Some(Mutation::RenameDeclaration {
                    qualified_name: element.as_qualified_name(),
                    new_name: new_name.clone(),
                })
            }
            SemanticMutation::UpdateUsageType { element, ty } => Some(Mutation::UpdateUsageType {
                qualified_name: element.as_qualified_name(),
                ty: ty.as_ref().map(ElementRef::as_qualified_name),
            }),
            SemanticMutation::SetExpression {
                element,
                expression,
            } => Some(Mutation::SetExpression {
                qualified_name: element.as_qualified_name(),
                expression: expression.as_ref().map(|expr| expr.as_text().to_string()),
            }),
            SemanticMutation::UpdateSpecializations {
                element,
                specializes,
            } => Some(Mutation::UpdateSpecializations {
                qualified_name: element.as_qualified_name(),
                specializes: specializes
                    .iter()
                    .map(ElementRef::as_qualified_name)
                    .collect(),
            }),
            SemanticMutation::MoveDeclaration {
                element,
                destination,
            } => Some(Mutation::MoveDeclaration {
                qualified_name: element.as_qualified_name(),
                destination: container_selector_for(project, destination),
            }),
            SemanticMutation::AddRelationship {
                kind,
                source,
                target,
            } => Some(Mutation::AddRelationship {
                container: container_selector_for(project, source),
                kind: kind.clone(),
                source: source.as_qualified_name(),
                target: target.as_qualified_name(),
            }),
            SemanticMutation::AddMetadataAnnotation {
                element,
                metadata_type,
                properties,
            } => Some(Mutation::AddMetadataAnnotation {
                element: element.as_qualified_name(),
                metadata_type: metadata_type.clone(),
                properties: properties.clone(),
            }),
            SemanticMutation::Remove { element } => Some(Mutation::RemoveDeclaration {
                qualified_name: element.as_qualified_name(),
            }),
            SemanticMutation::RemoveRelationship { .. } => None,
            SemanticMutation::SetAttribute { .. } => None,
        }
    }

    fn check_references(
        &self,
        project: &AuthoringProject,
        operation: &SemanticMutation,
        index: usize,
        blocking_reasons: &mut Vec<FeasibilityIssue>,
        warnings: &mut Vec<FeasibilityIssue>,
        suggested_supporting_changes: &mut Vec<SemanticMutation>,
        requires_supporting_changes: &mut bool,
    ) {
        match operation {
            SemanticMutation::AddPackage { .. } => {}
            SemanticMutation::AddElement {
                container,
                kind,
                ty,
                specializes,
                ..
            } => {
                self.require_existing(project, container, index, "container", blocking_reasons);
                let container_kind = self
                    .semantic_declaration_kind_label(project, container)
                    .unwrap_or_else(|| "container".to_string());
                let child_kind = kind.metaclass.clone();
                self.apply_legality_report(
                    self.legality
                        .check_containment(&container_kind, &child_kind),
                    index,
                    "container capability",
                    warnings,
                    blocking_reasons,
                );
                if let Some(ty) = ty {
                    if !exists(project, ty) {
                        if let Some(supporting_change) = self
                            .supporting_definition_for_missing_usage_type(
                                container,
                                &kind.metaclass,
                                ty,
                            )
                        {
                            *requires_supporting_changes = true;
                            suggested_supporting_changes.push(supporting_change);
                        } else {
                            blocking_reasons.push(FeasibilityIssue {
                                kind: FeasibilityIssueKind::ResolutionFailure,
                                operation_index: Some(index),
                                message: format!("missing type: {}", ty.qualified_name),
                            });
                        }
                    } else {
                        let definition_kind = self
                            .semantic_declaration_kind_label(project, ty)
                            .unwrap_or_else(|| "definition".to_string());
                        self.apply_legality_report(
                            self.legality
                                .check_usage_typing(&child_kind, &definition_kind),
                            index,
                            "typing capability",
                            warnings,
                            blocking_reasons,
                        );
                    }
                }
                for target in specializes {
                    self.require_existing(
                        project,
                        target,
                        index,
                        "specialization",
                        blocking_reasons,
                    );
                    let target_kind = self
                        .semantic_declaration_kind_label(project, target)
                        .unwrap_or_else(|| "specialization".to_string());
                    self.apply_legality_report(
                        self.legality
                            .check_specialization(&child_kind, &target_kind),
                        index,
                        "specialization capability",
                        warnings,
                        blocking_reasons,
                    );
                }
            }
            SemanticMutation::AddDefinition {
                container,
                keyword,
                name: _,
                specializes,
            } => {
                self.require_existing(project, container, index, "container", blocking_reasons);
                let container_kind = declaration_kind_label(project, container)
                    .unwrap_or_else(|| "container".to_string());
                self.apply_legality_report(
                    self.legality.check_containment(&container_kind, keyword),
                    index,
                    "container capability",
                    warnings,
                    blocking_reasons,
                );
                for target in specializes {
                    self.require_existing(
                        project,
                        target,
                        index,
                        "specialization",
                        blocking_reasons,
                    );
                    let target_kind = declaration_kind_label(project, target)
                        .unwrap_or_else(|| "specialization".to_string());
                    self.apply_legality_report(
                        self.legality.check_specialization(keyword, &target_kind),
                        index,
                        "specialization capability",
                        warnings,
                        blocking_reasons,
                    );
                }
            }
            SemanticMutation::AddUsage {
                container,
                keyword,
                ty,
                specializes,
                ..
            } => {
                self.require_existing(project, container, index, "container", blocking_reasons);
                let container_kind = declaration_kind_label(project, container)
                    .unwrap_or_else(|| "container".to_string());
                self.apply_legality_report(
                    self.legality.check_containment(&container_kind, keyword),
                    index,
                    "container capability",
                    warnings,
                    blocking_reasons,
                );
                if let Some(ty) = ty {
                    if !exists(project, ty) {
                        if let Some(definition_keyword) = self
                            .legality
                            .supporting_definition_keyword_for_usage(keyword)
                        {
                            *requires_supporting_changes = true;
                            suggested_supporting_changes.push(SemanticMutation::AddDefinition {
                                container: parent_ref(ty).unwrap_or_else(|| container.clone()),
                                keyword: definition_keyword,
                                name: ty
                                    .qualified_name
                                    .rsplit('.')
                                    .next()
                                    .unwrap_or(&ty.qualified_name)
                                    .to_string(),
                                specializes: Vec::new(),
                            });
                        } else {
                            self.require_existing(project, ty, index, "type", blocking_reasons);
                        }
                    } else {
                        let definition_kind = declaration_kind_label(project, ty)
                            .unwrap_or_else(|| "definition".to_string());
                        self.apply_legality_report(
                            self.legality.check_usage_typing(keyword, &definition_kind),
                            index,
                            "typing capability",
                            warnings,
                            blocking_reasons,
                        );
                    }
                }
                for target in specializes {
                    self.require_existing(
                        project,
                        target,
                        index,
                        "specialization",
                        blocking_reasons,
                    );
                    let target_kind = declaration_kind_label(project, target)
                        .unwrap_or_else(|| "specialization".to_string());
                    self.apply_legality_report(
                        self.legality.check_specialization(keyword, &target_kind),
                        index,
                        "specialization capability",
                        warnings,
                        blocking_reasons,
                    );
                }
            }
            SemanticMutation::AddRelationship {
                kind,
                source,
                target,
            } => {
                self.require_existing(
                    project,
                    source,
                    index,
                    "relationship source",
                    blocking_reasons,
                );
                self.require_existing(
                    project,
                    target,
                    index,
                    "relationship target",
                    blocking_reasons,
                );
                let target_kind = declaration_kind_label(project, target)
                    .unwrap_or_else(|| target.qualified_name.clone());
                let source_kind = declaration_kind_label(project, source)
                    .unwrap_or_else(|| source.qualified_name.clone());
                self.apply_legality_report(
                    self.legality
                        .check_relationship(kind, &source_kind, &target_kind),
                    index,
                    "relationship capability",
                    warnings,
                    blocking_reasons,
                );
            }
            SemanticMutation::AddMetadataAnnotation {
                element,
                metadata_type,
                ..
            } => {
                self.require_existing(project, element, index, "metadata target", blocking_reasons);
                if metadata_type.trim().is_empty() {
                    blocking_reasons.push(FeasibilityIssue {
                        kind: FeasibilityIssueKind::ValidationFailure,
                        operation_index: Some(index),
                        message: "metadata annotation type must not be empty".to_string(),
                    });
                }
            }
            SemanticMutation::Remove { element } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
            }
            SemanticMutation::RemoveRelationship { source, target, .. } => {
                self.require_existing(
                    project,
                    source,
                    index,
                    "relationship source",
                    blocking_reasons,
                );
                self.require_existing(
                    project,
                    target,
                    index,
                    "relationship target",
                    blocking_reasons,
                );
            }
            SemanticMutation::SetExpression { element, .. } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
                let kind = declaration_kind_label(project, element)
                    .unwrap_or_else(|| "element".to_string());
                self.apply_legality_report(
                    self.legality.check_attribute_write(&kind, "expression"),
                    index,
                    "attribute capability",
                    warnings,
                    blocking_reasons,
                );
            }
            SemanticMutation::SetAttribute {
                element, attribute, ..
            } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
                let kind = declaration_kind_label(project, element)
                    .unwrap_or_else(|| "element".to_string());
                self.apply_legality_report(
                    self.legality.check_attribute_write(&kind, attribute),
                    index,
                    "attribute capability",
                    warnings,
                    blocking_reasons,
                );
            }
            SemanticMutation::UpdateUsageType { element, ty } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
                if let Some(ty) = ty {
                    self.require_existing(project, ty, index, "type", blocking_reasons);
                    let usage_kind = declaration_kind_label(project, element)
                        .unwrap_or_else(|| "usage".to_string());
                    let definition_kind = declaration_kind_label(project, ty)
                        .unwrap_or_else(|| "definition".to_string());
                    self.apply_legality_report(
                        self.legality
                            .check_usage_typing(&usage_kind, &definition_kind),
                        index,
                        "typing capability",
                        warnings,
                        blocking_reasons,
                    );
                }
            }
            SemanticMutation::UpdateSpecializations {
                element,
                specializes,
            } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
                let source_kind = declaration_kind_label(project, element)
                    .unwrap_or_else(|| "element".to_string());
                for target in specializes {
                    self.require_existing(
                        project,
                        target,
                        index,
                        "specialization",
                        blocking_reasons,
                    );
                    let target_kind = declaration_kind_label(project, target)
                        .unwrap_or_else(|| "specialization".to_string());
                    self.apply_legality_report(
                        self.legality
                            .check_specialization(&source_kind, &target_kind),
                        index,
                        "specialization capability",
                        warnings,
                        blocking_reasons,
                    );
                }
            }
            SemanticMutation::RenameDeclaration { element, .. }
            | SemanticMutation::MoveDeclaration { element, .. } => {
                self.require_existing(project, element, index, "element", blocking_reasons);
            }
        }
    }

    fn supporting_definition_for_missing_usage_type(
        &self,
        container: &ElementRef,
        usage_metaclass: &str,
        ty: &ElementRef,
    ) -> Option<SemanticMutation> {
        let authoring = self.legality.authoring_for_element_kind(usage_metaclass)?;
        if authoring.form != SemanticElementForm::Usage {
            return None;
        }

        let definition_keyword = self
            .legality
            .supporting_definition_keyword_for_usage(&authoring.keyword)?;
        let container = parent_ref(ty).unwrap_or_else(|| container.clone());
        let name = ty
            .qualified_name
            .rsplit('.')
            .next()
            .unwrap_or(&ty.qualified_name)
            .to_string();

        Some(
            self.legality
                .semantic_kind_for_definition_keyword(&definition_keyword)
                .map(|metaclass| SemanticMutation::AddElement {
                    container: container.clone(),
                    kind: SemanticElementKind::new(metaclass),
                    name: name.clone(),
                    ty: None,
                    specializes: Vec::new(),
                    properties: Default::default(),
                })
                .unwrap_or_else(|| SemanticMutation::AddDefinition {
                    container,
                    keyword: definition_keyword,
                    name,
                    specializes: Vec::new(),
                }),
        )
    }

    fn require_existing(
        &self,
        project: &AuthoringProject,
        element: &ElementRef,
        index: usize,
        role: &str,
        blocking_reasons: &mut Vec<FeasibilityIssue>,
    ) {
        if !exists(project, element) {
            blocking_reasons.push(FeasibilityIssue {
                kind: FeasibilityIssueKind::ResolutionFailure,
                operation_index: Some(index),
                message: format!("missing {role}: {}", element.qualified_name),
            });
        }
    }

    fn semantic_declaration_kind_label(
        &self,
        project: &AuthoringProject,
        element: &ElementRef,
    ) -> Option<String> {
        let label = declaration_kind_label(project, element)?;
        if label == "package" {
            return Some("Package".to_string());
        }
        if let Some(keyword) = label.strip_suffix(" def") {
            return self
                .legality
                .semantic_kind_for_definition_keyword(keyword)
                .or(Some(label));
        }
        self.legality
            .semantic_kind_for_usage_keyword(&label)
            .or(Some(label))
    }

    fn apply_legality_report(
        &self,
        report: SemanticLegalityReport,
        index: usize,
        subject: &str,
        warnings: &mut Vec<FeasibilityIssue>,
        blocking_reasons: &mut Vec<FeasibilityIssue>,
    ) {
        if report.diagnostics.is_empty() {
            if report.status == SemanticLegalityStatus::Unknown {
                warnings.push(FeasibilityIssue {
                    kind: FeasibilityIssueKind::MetamodelViolation,
                    operation_index: Some(index),
                    message: format!("{subject}: semantic legality is unknown"),
                });
            }
            return;
        }

        for diagnostic in report.diagnostics {
            let issue = FeasibilityIssue {
                kind: FeasibilityIssueKind::MetamodelViolation,
                operation_index: Some(index),
                message: format!("{subject}: {}", diagnostic.message),
            };
            match diagnostic.severity {
                crate::runtime::RuleDiagnosticSeverity::Error => blocking_reasons.push(issue),
                crate::runtime::RuleDiagnosticSeverity::Warning
                | crate::runtime::RuleDiagnosticSeverity::Info => warnings.push(issue),
            }
        }
    }
}

fn exists(project: &AuthoringProject, element: &ElementRef) -> bool {
    project
        .semantic_attributes(&QualifiedName::parse(&element.qualified_name))
        .is_ok()
}

fn container_selector_for(project: &AuthoringProject, element: &ElementRef) -> ContainerSelector {
    let qualified_name = element.as_qualified_name();
    if is_package(project, element) {
        ContainerSelector::Package { qualified_name }
    } else {
        ContainerSelector::Declaration { qualified_name }
    }
}

fn is_package(project: &AuthoringProject, element: &ElementRef) -> bool {
    project.files().any(|(_, module)| {
        module
            .package
            .as_ref()
            .is_some_and(|package| package.name.as_dot_string() == element.qualified_name)
    })
}

fn operation_requires_supporting_change(
    project: &AuthoringProject,
    operation: &SemanticMutation,
) -> bool {
    matches!(
        operation,
        SemanticMutation::AddUsage { ty: Some(ty), .. }
        | SemanticMutation::AddElement { ty: Some(ty), .. } if !exists(project, ty)
    )
}

fn parent_ref(element: &ElementRef) -> Option<ElementRef> {
    element
        .qualified_name
        .rsplit_once('.')
        .map(|(parent, _)| ElementRef::new(parent.to_string()))
}

fn feasibility_repair_hints(
    blocking_reasons: &[FeasibilityIssue],
    warnings: &[FeasibilityIssue],
    suggested_supporting_changes: &[SemanticMutation],
) -> Vec<FeasibilityRepairHint> {
    let mut hints = Vec::new();
    for issue in blocking_reasons.iter().chain(warnings.iter()) {
        let message = issue.message.to_ascii_lowercase();
        let kind = match issue.kind {
            FeasibilityIssueKind::StaleWorkspaceRevision => {
                FeasibilityRepairHintKind::RefreshWorkspaceRevision
            }
            FeasibilityIssueKind::ResolutionFailure => {
                FeasibilityRepairHintKind::UseExistingElement
            }
            FeasibilityIssueKind::UnsupportedByAuthoringBackend => {
                FeasibilityRepairHintKind::RemoveUnsupportedOperation
            }
            FeasibilityIssueKind::MetamodelViolation
                if message.contains("block") && message.contains("part") =>
            {
                FeasibilityRepairHintKind::ReplaceDeprecatedVocabulary
            }
            FeasibilityIssueKind::MetamodelViolation
                if message.contains("relationship") || message.contains("target") =>
            {
                FeasibilityRepairHintKind::UseAllowedRelationshipTarget
            }
            FeasibilityIssueKind::MetamodelViolation
                if message.contains("typing") || message.contains("typed") =>
            {
                FeasibilityRepairHintKind::UseAllowedUsageType
            }
            FeasibilityIssueKind::RequiresSupportingChange
            | FeasibilityIssueKind::RequiresImport => {
                FeasibilityRepairHintKind::AddSupportingDefinition
            }
            _ => FeasibilityRepairHintKind::ReviseProposal,
        };
        hints.push(FeasibilityRepairHint {
            kind,
            operation_index: issue.operation_index,
            message: repair_hint_message(kind, issue),
            suggested_operation: None,
        });
    }
    for operation in suggested_supporting_changes {
        hints.push(FeasibilityRepairHint {
            kind: FeasibilityRepairHintKind::AddSupportingDefinition,
            operation_index: None,
            message: "Add the supporting semantic definition before applying this proposal"
                .to_string(),
            suggested_operation: Some(operation.clone()),
        });
    }
    hints
}

fn repair_hint_message(kind: FeasibilityRepairHintKind, issue: &FeasibilityIssue) -> String {
    match kind {
        FeasibilityRepairHintKind::UseExistingElement => {
            "Use an existing element reference from the semantic context".to_string()
        }
        FeasibilityRepairHintKind::AddSupportingDefinition => {
            "Add or import the missing supporting definition first".to_string()
        }
        FeasibilityRepairHintKind::ReplaceDeprecatedVocabulary => {
            "Replace deprecated SysML v1 vocabulary with the SysML v2 term `part`".to_string()
        }
        FeasibilityRepairHintKind::UseAllowedRelationshipTarget => {
            "Choose a relationship target allowed by the core semantic legality service".to_string()
        }
        FeasibilityRepairHintKind::UseAllowedUsageType => {
            "Choose a usage type allowed by the core semantic legality service".to_string()
        }
        FeasibilityRepairHintKind::RemoveUnsupportedOperation => {
            "Remove or revise the operation until an authoring write-back path exists".to_string()
        }
        FeasibilityRepairHintKind::RefreshWorkspaceRevision => {
            "Refresh semantic context and regenerate the proposal for the current workspace revision"
                .to_string()
        }
        FeasibilityRepairHintKind::ReviseProposal => issue.message.clone(),
    }
}

fn declaration_kind_label(project: &AuthoringProject, element: &ElementRef) -> Option<String> {
    for (_, module) in project.files() {
        if module
            .package
            .as_ref()
            .is_some_and(|package| package.name.as_dot_string() == element.qualified_name)
        {
            return Some("package".to_string());
        }
        if let Some(kind) = declaration_kind_label_in_module(module, &element.qualified_name) {
            return Some(kind);
        }
    }
    None
}

fn declaration_kind_label_in_module(
    module: &AuthoringModule,
    qualified_name: &str,
) -> Option<String> {
    for member in &module.members {
        if let Some(kind) = declaration_kind_label_in_declaration(member, "", qualified_name) {
            return Some(kind);
        }
    }
    if let Some(package) = &module.package {
        let package_name = package.name.as_dot_string();
        for member in &package.members {
            if let Some(kind) =
                declaration_kind_label_in_declaration(member, &package_name, qualified_name)
            {
                return Some(kind);
            }
        }
    }
    None
}

fn declaration_kind_label_in_declaration(
    declaration: &Declaration,
    parent: &str,
    qualified_name: &str,
) -> Option<String> {
    match declaration {
        Declaration::Package(package) => {
            let current = package.name.as_dot_string();
            if current == qualified_name {
                return Some("package".to_string());
            }
            for member in &package.members {
                if let Some(kind) =
                    declaration_kind_label_in_declaration(member, &current, qualified_name)
                {
                    return Some(kind);
                }
            }
        }
        Declaration::Definition(definition) => {
            let current = join_ref(parent, &definition.name);
            if current == qualified_name {
                return Some(format!("{} def", definition.keyword));
            }
            for member in &definition.members {
                if let Some(kind) =
                    declaration_kind_label_in_declaration(member, &current, qualified_name)
                {
                    return Some(kind);
                }
            }
        }
        Declaration::Usage(usage) => {
            let current = join_ref(parent, &usage.name);
            if current == qualified_name {
                return Some(usage.keyword.clone());
            }
            for member in &usage.members {
                if let Some(kind) =
                    declaration_kind_label_in_declaration(member, &current, qualified_name)
                {
                    return Some(kind);
                }
            }
        }
        Declaration::Alias(alias) => {
            if join_ref(parent, &alias.name) == qualified_name {
                return Some("alias".to_string());
            }
        }
        Declaration::Import(_) => {}
    }
    None
}

fn join_ref(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}.{name}")
    }
}

pub fn workspace_revision_for_project(project: &AuthoringProject) -> WorkspaceRevision {
    let mut hasher = DefaultHasher::new();
    for (path, _) in project.files() {
        path.hash(&mut hasher);
        if let Ok(rendered) = project.render_new_file(path) {
            rendered.hash(&mut hasher);
        }
    }
    WorkspaceRevision {
        fingerprint: format!("{:016x}", hasher.finish()),
    }
}

fn proposal_id(proposal: &MutationProposal) -> String {
    let mut hasher = DefaultHasher::new();
    proposal.intent.hash(&mut hasher);
    proposal.workspace_revision.fingerprint.hash(&mut hasher);
    for operation in &proposal.operations {
        format!("{operation:?}").hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::authoring::authoring::load_authoring_project_from_model;
    use crate::semantic_services::mutation::{
        MutationEvidence, MutationProposal, SemanticExpression,
    };
    use crate::semantic_services::semantic_profile::AttributePolicyAnswer;

    #[derive(Debug, Clone)]
    struct TypingOracle(CapabilityAnswer);

    impl SemanticCapabilityOracle for TypingOracle {
        fn can_contain(&self, _container_kind: &str, _child_kind: &str) -> CapabilityAnswer {
            CapabilityAnswer::Allowed
        }

        fn can_specialize(&self, _source_kind: &str, _target_kind: &str) -> CapabilityAnswer {
            CapabilityAnswer::Allowed
        }

        fn can_type_usage(&self, _usage_kind: &str, _definition_kind: &str) -> CapabilityAnswer {
            self.0.clone()
        }

        fn can_relate(
            &self,
            _relationship_kind: &str,
            _source_kind: &str,
            _target_kind: &str,
        ) -> CapabilityAnswer {
            CapabilityAnswer::Allowed
        }

        fn attribute_policy(&self, _kind: &str, _attribute: &str) -> AttributePolicyAnswer {
            AttributePolicyAnswer {
                writable: true,
                reason: None,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct SupportingDefinitionOracle;

    impl SemanticCapabilityOracle for SupportingDefinitionOracle {
        fn can_contain(&self, container_kind: &str, child_kind: &str) -> CapabilityAnswer {
            ConservativeSemanticCapabilityOracle.can_contain(container_kind, child_kind)
        }

        fn can_specialize(&self, source_kind: &str, target_kind: &str) -> CapabilityAnswer {
            ConservativeSemanticCapabilityOracle.can_specialize(source_kind, target_kind)
        }

        fn can_type_usage(&self, usage_kind: &str, definition_kind: &str) -> CapabilityAnswer {
            ConservativeSemanticCapabilityOracle.can_type_usage(usage_kind, definition_kind)
        }

        fn can_relate(
            &self,
            relationship_kind: &str,
            source_kind: &str,
            target_kind: &str,
        ) -> CapabilityAnswer {
            ConservativeSemanticCapabilityOracle.can_relate(
                relationship_kind,
                source_kind,
                target_kind,
            )
        }

        fn attribute_policy(&self, kind: &str, attribute: &str) -> AttributePolicyAnswer {
            ConservativeSemanticCapabilityOracle.attribute_policy(kind, attribute)
        }

        fn supporting_definition_keyword_for_usage(&self, usage_kind: &str) -> Option<String> {
            usage_kind
                .eq_ignore_ascii_case("component")
                .then(|| "component".to_string())
        }
    }

    fn neutral_model_project() -> AuthoringProject {
        load_authoring_project_from_model(BTreeMap::from([(
            "system.model".to_string(),
            r#"
package SystemModel {
    component def System {
        component processor : Processor;
        component actuator : Actuator;
        component sensor : Sensor;
        component dataStore : DataStore;
        component controller : Controller;
    }

    component def Processor {
        value throughput : Real;
        value latency : Real;
    }

    component def Actuator {
        value responseTime : Real;
        value maxLoad : Real;
    }

    component def Sensor {
        value sampleRate : Real;
        value mass : Real;
    }

    component def DataStore;

    component def Controller {
        value strategy : String;
    }

    objective def ReliabilityObjective {
        value targetAvailability : Real;
    }
}
"#
            .to_string(),
        )]))
        .unwrap()
    }

    #[test]
    fn component_update_proposal_is_feasible_for_supported_operations() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Add diagnostics to the system model".to_string(),
            operations: vec![
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "component".to_string(),
                    name: "DiagnosticsModule".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddUsage {
                    container: ElementRef::new("SystemModel.System"),
                    keyword: "component".to_string(),
                    name: "diagnostics".to_string(),
                    ty: Some(ElementRef::new("SystemModel.DiagnosticsModule")),
                    specializes: Vec::new(),
                },
            ],
            evidence: vec![MutationEvidence {
                element: Some(ElementRef::new("SystemModel.Sensor")),
                summary: "Existing sensors can provide diagnostics data.".to_string(),
            }],
            rationale: Some("Diagnostics improve model observability.".to_string()),
            workspace_revision: context.workspace_revision.clone(),
        };

        let service = CoreMutationFeasibilityService::new();
        let report = service.check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Allowed, "{report:#?}");
        assert!(report.blocking_reasons.is_empty());
        let diff = report.resulting_diff.unwrap();
        assert!(
            diff.added_elements
                .iter()
                .any(|element| element.element_id == "type.SystemModel.DiagnosticsModule")
        );
        assert!(
            diff.added_elements
                .iter()
                .any(|element| element.element_id == "feature.SystemModel.System.diagnostics")
        );

        let application = service
            .apply_checked_plan(&context, &report.normalized_plan.unwrap())
            .unwrap();
        assert!(
            application
                .changed_declarations
                .contains("SystemModel.DiagnosticsModule")
        );
        assert!(
            application
                .changed_declarations
                .contains("SystemModel.System.diagnostics")
        );
        assert!(
            application
                .semantic_diff
                .added_elements
                .iter()
                .any(|element| element.element_id == "feature.SystemModel.System.diagnostics")
        );
    }

    #[test]
    fn feasibility_blocks_custom_oracle_denied_typing() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Add policy-blocked diagnostics usage".to_string(),
            operations: vec![SemanticMutation::AddUsage {
                container: ElementRef::new("SystemModel.System"),
                keyword: "component".to_string(),
                name: "diagnostics".to_string(),
                ty: Some(ElementRef::new("SystemModel.Sensor")),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::with_oracle(TypingOracle(
            CapabilityAnswer::Denied("test oracle denied typing".to_string()),
        ))
        .check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Blocked);
        assert!(report.blocking_reasons.iter().any(|issue| {
            issue.kind == FeasibilityIssueKind::MetamodelViolation
                && issue.message.contains("test oracle denied typing")
        }));
    }

    #[test]
    fn feasibility_warns_on_custom_oracle_unknown_typing() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Add policy-unknown diagnostics usage".to_string(),
            operations: vec![SemanticMutation::AddUsage {
                container: ElementRef::new("SystemModel.System"),
                keyword: "component".to_string(),
                name: "diagnostics".to_string(),
                ty: Some(ElementRef::new("SystemModel.Sensor")),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::with_oracle(TypingOracle(
            CapabilityAnswer::Unknown("test oracle cannot prove typing".to_string()),
        ))
        .check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::AllowedWithWarnings);
        assert!(report.warnings.iter().any(|issue| {
            issue.kind == FeasibilityIssueKind::MetamodelViolation
                && issue.message.contains("test oracle cannot prove typing")
        }));
    }

    #[test]
    fn neutral_feasibility_blocks_missing_usage_type_without_language_profile() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Add diagnostics usage before its definition exists".to_string(),
            operations: vec![SemanticMutation::AddUsage {
                container: ElementRef::new("SystemModel.System"),
                keyword: "component".to_string(),
                name: "diagnostics".to_string(),
                ty: Some(ElementRef::new("SystemModel.DiagnosticsModule")),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Blocked, "{report:#?}");
        assert!(report.suggested_supporting_changes.is_empty());
        assert!(report.blocking_reasons.iter().any(|issue| {
            issue.kind == FeasibilityIssueKind::ResolutionFailure
                && issue
                    .message
                    .contains("missing type: SystemModel.DiagnosticsModule")
        }));
        assert!(report.repair_hints.iter().any(|hint| {
            hint.kind == FeasibilityRepairHintKind::UseExistingElement
                && hint.operation_index == Some(0)
        }));
    }

    #[test]
    fn feasibility_suggests_supporting_definition_repair_hint_when_profile_can_supply_type() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Add diagnostics usage before its definition exists".to_string(),
            operations: vec![SemanticMutation::AddUsage {
                container: ElementRef::new("SystemModel.System"),
                keyword: "component".to_string(),
                name: "diagnostics".to_string(),
                ty: Some(ElementRef::new("SystemModel.DiagnosticsModule")),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::with_oracle(SupportingDefinitionOracle)
            .check(&context, &proposal);

        assert_eq!(
            report.status,
            FeasibilityStatus::RequiresSupportingChanges,
            "{report:#?}"
        );
        assert!(
            report
                .suggested_supporting_changes
                .iter()
                .any(|operation| matches!(
                    operation,
                    SemanticMutation::AddDefinition { name, .. } if name == "DiagnosticsModule"
                ))
        );
        assert!(report.repair_hints.iter().any(|hint| {
            hint.kind == FeasibilityRepairHintKind::AddSupportingDefinition
                && matches!(
                    &hint.suggested_operation,
                    Some(SemanticMutation::AddDefinition { name, .. })
                        if name == "DiagnosticsModule"
                )
        }));
    }

    #[test]
    fn relationship_candidate_is_semantically_checked_and_writable() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Relate the system to its objective".to_string(),
            operations: vec![SemanticMutation::AddRelationship {
                kind: "relate".to_string(),
                source: ElementRef::new("SystemModel.System"),
                target: ElementRef::new("SystemModel.ReliabilityObjective"),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Allowed, "{report:#?}");
        assert!(report.warnings.is_empty());
        let diff = report.resulting_diff.unwrap();
        assert!(diff.added_elements.iter().any(|element| {
            element.element_id == "relationship.SystemModel.System.ReliabilityObjective"
        }));
        assert!(diff.added_relationships.iter().any(|relationship| {
            relationship.kind == "target"
                && relationship.source.element_id
                    == "relationship.SystemModel.System.ReliabilityObjective"
                && relationship.target.element_id == "type.SystemModel.ReliabilityObjective"
        }));

        let application = CoreMutationFeasibilityService::new()
            .apply_checked_plan(&context, &report.normalized_plan.unwrap())
            .unwrap();
        assert!(
            application
                .changed_declarations
                .contains("SystemModel.System.ReliabilityObjective")
        );
        let mut project = context.project.clone();
        let result = project
            .apply_mutation(crate::authoring::authoring::Mutation::AddRelationship {
                container: crate::authoring::authoring::ContainerSelector::Declaration {
                    qualified_name: QualifiedName::parse("SystemModel.System"),
                },
                kind: "relate".to_string(),
                source: QualifiedName::parse("SystemModel.System"),
                target: QualifiedName::parse("SystemModel.ReliabilityObjective"),
            })
            .unwrap();
        let edited = project.write_back_mutation(&result).unwrap();
        let source = edited.edited_files.get("system.model").unwrap();
        assert!(
            source.contains(
                "relate ReliabilityObjective references SystemModel.ReliabilityObjective;"
            )
        );
    }

    #[test]
    fn stale_workspace_revision_blocks_feasibility() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Stale proposal".to_string(),
            operations: vec![SemanticMutation::AddDefinition {
                container: ElementRef::new("SystemModel"),
                keyword: "component".to_string(),
                name: "AuditModule".to_string(),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: WorkspaceRevision {
                fingerprint: "stale".to_string(),
            },
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Blocked);
        assert!(
            report
                .blocking_reasons
                .iter()
                .any(|issue| { issue.kind == FeasibilityIssueKind::StaleWorkspaceRevision })
        );
        assert!(report.repair_hints.iter().any(|hint| {
            hint.kind == FeasibilityRepairHintKind::RefreshWorkspaceRevision
                && hint.operation_index.is_none()
        }));
    }

    #[test]
    fn mutation_plan_can_generate_model_from_empty_project() {
        let context = MutationContext::from_project(AuthoringProject::default());
        let proposal = MutationProposal {
            intent: "Generate a minimal system model".to_string(),
            operations: vec![
                SemanticMutation::AddPackage {
                    target_file: "system.model".to_string(),
                    name: "SystemModel".to_string(),
                },
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "component".to_string(),
                    name: "System".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "component".to_string(),
                    name: "Processor".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "component".to_string(),
                    name: "Actuator".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "component".to_string(),
                    name: "Sensor".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddDefinition {
                    container: ElementRef::new("SystemModel"),
                    keyword: "objective".to_string(),
                    name: "ReliabilityObjective".to_string(),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddUsage {
                    container: ElementRef::new("SystemModel.System"),
                    keyword: "component".to_string(),
                    name: "processor".to_string(),
                    ty: Some(ElementRef::new("SystemModel.Processor")),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddUsage {
                    container: ElementRef::new("SystemModel.System"),
                    keyword: "component".to_string(),
                    name: "actuator".to_string(),
                    ty: Some(ElementRef::new("SystemModel.Actuator")),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddUsage {
                    container: ElementRef::new("SystemModel.System"),
                    keyword: "component".to_string(),
                    name: "sensor".to_string(),
                    ty: Some(ElementRef::new("SystemModel.Sensor")),
                    specializes: Vec::new(),
                },
                SemanticMutation::AddRelationship {
                    kind: "relate".to_string(),
                    source: ElementRef::new("SystemModel.System"),
                    target: ElementRef::new("SystemModel.ReliabilityObjective"),
                },
            ],
            evidence: Vec::new(),
            rationale: Some(
                "Create a semantic model from typed construction operations.".to_string(),
            ),
            workspace_revision: context.workspace_revision.clone(),
        };

        let service = CoreMutationFeasibilityService::new();
        let report = service.check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Allowed, "{report:#?}");
        let application = service
            .apply_checked_plan(&context, &report.normalized_plan.unwrap())
            .unwrap();
        assert!(application.changed_files.contains("system.model"));
        assert!(
            application
                .changed_declarations
                .contains("SystemModel.System.processor")
        );
        assert!(
            application
                .changed_declarations
                .contains("SystemModel.System.ReliabilityObjective")
        );
    }

    #[test]
    fn mutation_plan_can_set_expression_and_emit_expression_ir() {
        let project = load_authoring_project_from_model(BTreeMap::from([(
            "score.model".to_string(),
            r#"
package Demo {
    component element {
        value score : Real;
    }
}
"#
            .to_string(),
        )]))
        .unwrap();
        let context = MutationContext::from_project(project);
        let proposal = MutationProposal {
            intent: "Set element score expression".to_string(),
            operations: vec![SemanticMutation::SetExpression {
                element: ElementRef::new("Demo.element.score"),
                expression: Some(SemanticExpression::Text("0.42".to_string())),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let service = CoreMutationFeasibilityService::new();
        let report = service.check(&context, &proposal);
        assert_eq!(report.status, FeasibilityStatus::Allowed, "{report:#?}");
        assert!(
            report
                .resulting_diff
                .unwrap()
                .changed_attributes
                .iter()
                .any(
                    |change| change.element.element_id == "feature.Demo.element.score"
                        && change.attribute == "expression_ir"
                        && change.after == Some(serde_json::json!("0.42"))
                )
        );

        let mut project = context.project.clone();
        let result = project
            .apply_mutation(crate::authoring::authoring::Mutation::SetExpression {
                qualified_name: QualifiedName::parse("Demo.element.score"),
                expression: Some("0.42".to_string()),
            })
            .unwrap();
        let edited = project.write_back_mutation(&result).unwrap();
        let source = edited.edited_files.get("score.model").unwrap();
        assert!(source.contains("value score: Real = 0.42;"));

        assert!(source.contains("value score: Real = 0.42;"));
    }

    #[test]
    fn neutral_feasibility_allows_profile_specific_relationship_without_language_profile() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Profile-specific relationship target".to_string(),
            operations: vec![SemanticMutation::AddRelationship {
                kind: "profile-link".to_string(),
                source: ElementRef::new("SystemModel.System"),
                target: ElementRef::new("SystemModel.Sensor"),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Allowed);
        assert!(report.blocking_reasons.is_empty());
    }

    #[test]
    fn neutral_feasibility_allows_profile_specific_usage_typing_without_language_profile() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Profile-specific usage typing".to_string(),
            operations: vec![SemanticMutation::AddUsage {
                container: ElementRef::new("SystemModel.System"),
                keyword: "component".to_string(),
                name: "objectiveComponent".to_string(),
                ty: Some(ElementRef::new("SystemModel.ReliabilityObjective")),
                specializes: Vec::new(),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Allowed);
        assert!(report.blocking_reasons.is_empty());
    }

    #[test]
    fn feasibility_blocks_unwritable_semantic_attribute() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Invalid attribute write".to_string(),
            operations: vec![SemanticMutation::SetAttribute {
                element: ElementRef::new("SystemModel.System"),
                attribute: "owner".to_string(),
                value: serde_json::json!("pkg.Other"),
            }],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let report = CoreMutationFeasibilityService::new().check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Blocked);
        assert!(report.blocking_reasons.iter().any(|issue| {
            issue.kind == FeasibilityIssueKind::MetamodelViolation
                && issue.message.contains("not writable")
        }));
    }

    #[test]
    fn neutral_feasibility_blocks_profile_specific_attributes_without_language_profile() {
        let context = MutationContext::from_project(neutral_model_project());
        let proposal = MutationProposal {
            intent: "Fill objective metadata".to_string(),
            operations: vec![
                SemanticMutation::SetAttribute {
                    element: ElementRef::new("SystemModel.ReliabilityObjective"),
                    attribute: "id".to_string(),
                    value: serde_json::json!("OBJ-REL-001"),
                },
                SemanticMutation::SetAttribute {
                    element: ElementRef::new("SystemModel.ReliabilityObjective"),
                    attribute: "text".to_string(),
                    value: serde_json::json!("The system model should improve observability."),
                },
            ],
            evidence: Vec::new(),
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };

        let service = CoreMutationFeasibilityService::new();
        let report = service.check(&context, &proposal);

        assert_eq!(report.status, FeasibilityStatus::Blocked);
        assert!(report.blocking_reasons.iter().any(|issue| {
            issue.kind == FeasibilityIssueKind::MetamodelViolation
                && issue.message.contains("not writable")
        }));
    }
}
