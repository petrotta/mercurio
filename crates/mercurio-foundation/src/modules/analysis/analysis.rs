use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::analysis::capability::SemanticElementRef;
use crate::kir::{Diagnostic, DiagnosticKind, Severity};
use crate::model::{Element, Graph};

pub type AnalysisElementRef = SemanticElementRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisTechniqueKind {
    Query,
    Calculation,
    ConstraintEvaluation,
    Simulation,
    Verification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisWorkflowStepKind {
    ScopeSubject,
    BindAssumptions,
    ResolveInputs,
    ExecuteTechnique,
    EvaluateConstraints,
    EvaluateRequirements,
    ProduceViews,
    RecordResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkflowStep {
    pub kind: AnalysisWorkflowStepKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<AnalysisTechniqueKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkflow {
    pub case_id: String,
    pub steps: Vec<AnalysisWorkflowStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCaseModel {
    pub element: AnalysisElementRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculations: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_cases: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub simulations: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concerns: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<AnalysisTechniqueKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementEvaluationModel {
    pub requirement: AnalysisElementRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formal_constraints: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_cases: Vec<AnalysisElementRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisInventory {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authored_elements: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub analysis_cases: Vec<AnalysisCaseModel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculation_definitions: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculation_usages: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraint_definitions: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraint_usages: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_evaluations: Vec<RequirementEvaluationModel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_cases: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concerns: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub simulations: Vec<AnalysisElementRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisOpportunityKind {
    AnalysisCase,
    ConstraintEvaluation,
    RequirementEvaluation,
    Simulation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCapabilityProviderKind {
    Builtin,
    Plugin,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCapabilityDescriptor {
    pub id: String,
    #[serde(default)]
    pub selector: AnalysisCapabilitySelector,
    pub effect: AnalysisCapabilityEffect,
    pub provider_kind: AnalysisCapabilityProviderKind,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCapabilitySelector {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predicates: Vec<AnalysisStructuralPredicate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStructuralPredicate {
    AnalysisCase,
    ConstraintEvaluation,
    RequirementEvaluation,
    Simulation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCapabilityEffect {
    pub kind: AnalysisOpportunityKind,
    pub label: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<AnalysisTechniqueKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AnalysisOpportunityReadiness {
    Runnable,
    Blocked { diagnostic: Diagnostic },
}

impl Default for AnalysisOpportunityReadiness {
    fn default() -> Self {
        Self::Runnable
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisOpportunity {
    pub id: String,
    pub kind: AnalysisOpportunityKind,
    pub label: String,
    pub description: String,
    pub runnable: bool,
    #[serde(default)]
    pub readiness: AnalysisOpportunityReadiness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<AnalysisElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<AnalysisTechniqueKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_hint: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisOpportunityReport {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opportunities: Vec<AnalysisOpportunity>,
}

impl AnalysisInventory {
    pub fn from_graph(graph: &Graph) -> Self {
        let authored_elements = graph
            .elements()
            .iter()
            .filter(|element| is_authored_model_element(element))
            .map(element_ref)
            .collect();
        let calculation_definitions =
            collect_by_kind(graph, |kind| kind_contains(kind, "calculationdefinition"));
        let calculation_usages =
            collect_by_kind(graph, |kind| kind_contains(kind, "calculationusage"));
        let constraint_definitions =
            collect_by_kind(graph, |kind| kind_contains(kind, "constraintdefinition"));
        let constraint_usages = collect_by_kind(graph, is_constraint_usage_kind);
        let verification_cases = collect_by_kind(graph, is_verification_case_kind);
        let views = collect_by_kind(graph, is_view_kind);
        let concerns = collect_by_kind(graph, is_concern_or_viewpoint_kind);
        let simulations = collect_by_kind(graph, is_simulation_kind);

        let analysis_cases = graph
            .elements()
            .iter()
            .filter(|element| is_authored_model_element(element))
            .filter(|element| is_analysis_case_kind(element.kind.as_ref()))
            .map(|element| AnalysisCaseModel::from_element(graph, element))
            .collect();

        let requirement_evaluations = graph
            .elements()
            .iter()
            .filter(|element| is_authored_model_element(element))
            .filter(|element| is_requirement_kind(element.kind.as_ref()))
            .map(|element| requirement_evaluation_from_element(graph, element))
            .collect();

        Self {
            authored_elements,
            analysis_cases,
            calculation_definitions,
            calculation_usages,
            constraint_definitions,
            constraint_usages,
            requirement_evaluations,
            verification_cases,
            views,
            concerns,
            simulations,
        }
    }

    pub fn opportunities(&self) -> AnalysisOpportunityReport {
        self.opportunities_for_descriptors(&builtin_analysis_capability_descriptors())
    }

    pub fn opportunities_for_descriptors(
        &self,
        descriptors: &[AnalysisCapabilityDescriptor],
    ) -> AnalysisOpportunityReport {
        let mut opportunities = Vec::new();

        for descriptor in descriptors {
            opportunities.extend(self.opportunities_for_descriptor(descriptor));
        }

        AnalysisOpportunityReport {
            schema: "mercurio.analysis.opportunities.v1".to_string(),
            opportunities,
        }
    }

    fn opportunities_for_descriptor(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Vec<AnalysisOpportunity> {
        if descriptor.selector.predicates.is_empty() {
            return self
                .type_selected_opportunity(descriptor)
                .into_iter()
                .collect();
        }

        let mut opportunities = Vec::new();
        for predicate in &descriptor.selector.predicates {
            opportunities.extend(self.structural_opportunities(descriptor, predicate));
        }
        opportunities
    }

    fn type_selected_opportunity(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Option<AnalysisOpportunity> {
        if descriptor.selector.types.is_empty() {
            return None;
        }
        let selected_types = descriptor
            .selector
            .types
            .iter()
            .map(|kind| canonical_kind(kind))
            .collect::<BTreeSet<_>>();
        let elements = self
            .authored_elements
            .iter()
            .filter(|element| {
                element
                    .kind
                    .as_deref()
                    .is_some_and(|kind| selected_types.contains(&canonical_kind(kind)))
            })
            .cloned()
            .collect::<Vec<_>>();
        if elements.is_empty() {
            return None;
        }

        let mut metadata = descriptor_metadata(descriptor);
        metadata.insert("matchedTypeCount".to_string(), json!(selected_types.len()));
        metadata.insert("subjectCount".to_string(), json!(elements.len()));

        Some(opportunity_from_descriptor(
            descriptor,
            format!("{}.workspace", descriptor.id),
            elements,
            AnalysisOpportunityReadiness::Runnable,
            metadata,
        ))
    }

    fn structural_opportunities(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
        predicate: &AnalysisStructuralPredicate,
    ) -> Vec<AnalysisOpportunity> {
        match predicate {
            AnalysisStructuralPredicate::AnalysisCase => {
                self.analysis_case_opportunities(descriptor)
            }
            AnalysisStructuralPredicate::ConstraintEvaluation => self
                .constraint_evaluation_opportunity(descriptor)
                .into_iter()
                .collect(),
            AnalysisStructuralPredicate::RequirementEvaluation => self
                .requirement_evaluation_opportunity(descriptor)
                .into_iter()
                .collect(),
            AnalysisStructuralPredicate::Simulation => self
                .simulation_opportunity(descriptor)
                .into_iter()
                .collect(),
        }
    }

    fn analysis_case_opportunities(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Vec<AnalysisOpportunity> {
        self.analysis_cases
            .iter()
            .map(|case| {
                let mut metadata = descriptor_metadata(descriptor);
                metadata.insert("caseId".to_string(), json!(case.element.element_id));
                metadata.insert("workflow".to_string(), json!(case.workflow()));
                metadata.insert("subjectCount".to_string(), json!(case.subjects.len()));
                metadata.insert("constraintCount".to_string(), json!(case.constraints.len()));
                metadata.insert(
                    "requirementCount".to_string(),
                    json!(case.requirements.len()),
                );
                metadata.insert("simulationCount".to_string(), json!(case.simulations.len()));
                let label = format!(
                    "Run analysis case {}",
                    case.element
                        .label
                        .as_deref()
                        .unwrap_or(case.element.element_id.as_str())
                );
                let mut opportunity = opportunity_from_descriptor_with_label(
                    descriptor,
                    format!("analysis_case.{}", case.element.element_id),
                    label,
                    vec![case.element.clone()],
                    AnalysisOpportunityReadiness::Runnable,
                    metadata,
                );
                opportunity.techniques = case.techniques.clone();
                opportunity
            })
            .collect()
    }

    fn constraint_evaluation_opportunity(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Option<AnalysisOpportunity> {
        if self.constraint_usages.is_empty() && self.constraint_definitions.is_empty() {
            return None;
        }
        let elements = if self.constraint_usages.is_empty() {
            self.constraint_definitions.clone()
        } else {
            self.constraint_usages.clone()
        };
        let readiness = if self.constraint_usages.is_empty() {
            AnalysisOpportunityReadiness::Blocked {
                diagnostic: readiness_diagnostic(
                    "analysis.constraint.unbound",
                    "constraint definitions are present but no constraint usages are bound",
                    self.constraint_definitions
                        .iter()
                        .map(|element| element.element_id.clone()),
                ),
            }
        } else {
            AnalysisOpportunityReadiness::Runnable
        };
        let mut metadata = descriptor_metadata(descriptor);
        metadata.insert(
            "constraintDefinitionCount".to_string(),
            json!(self.constraint_definitions.len()),
        );
        metadata.insert(
            "constraintUsageCount".to_string(),
            json!(self.constraint_usages.len()),
        );
        Some(opportunity_from_descriptor(
            descriptor,
            "constraint_evaluation.workspace".to_string(),
            elements,
            readiness,
            metadata,
        ))
    }

    fn requirement_evaluation_opportunity(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Option<AnalysisOpportunity> {
        let requirement_evaluations = self
            .requirement_evaluations
            .iter()
            .filter(|evaluation| {
                !evaluation.formal_constraints.is_empty()
                    || !evaluation.verification_cases.is_empty()
            })
            .collect::<Vec<_>>();
        if requirement_evaluations.is_empty() {
            return None;
        }
        let mut metadata = descriptor_metadata(descriptor);
        metadata.insert(
            "requirementCount".to_string(),
            json!(requirement_evaluations.len()),
        );
        Some(opportunity_from_descriptor(
            descriptor,
            "requirement_evaluation.workspace".to_string(),
            requirement_evaluations
                .iter()
                .map(|evaluation| evaluation.requirement.clone())
                .collect(),
            AnalysisOpportunityReadiness::Runnable,
            metadata,
        ))
    }

    fn simulation_opportunity(
        &self,
        descriptor: &AnalysisCapabilityDescriptor,
    ) -> Option<AnalysisOpportunity> {
        if self.simulations.is_empty() {
            return None;
        }
        let mut metadata = descriptor_metadata(descriptor);
        metadata.insert(
            "simulationElementCount".to_string(),
            json!(self.simulations.len()),
        );
        Some(opportunity_from_descriptor(
            descriptor,
            "simulation.workspace".to_string(),
            self.simulations.clone(),
            AnalysisOpportunityReadiness::Runnable,
            metadata,
        ))
    }
}

pub fn builtin_analysis_capability_descriptors() -> Vec<AnalysisCapabilityDescriptor> {
    vec![
        AnalysisCapabilityDescriptor {
            id: "sysml.analysis.case".to_string(),
            selector: AnalysisCapabilitySelector {
                types: Vec::new(),
                predicates: vec![AnalysisStructuralPredicate::AnalysisCase],
            },
            effect: AnalysisCapabilityEffect {
                kind: AnalysisOpportunityKind::AnalysisCase,
                label: "Run analysis case".to_string(),
                description: "Authored analysis case with a semantic execution workflow."
                    .to_string(),
                techniques: Vec::new(),
                action_id: Some("run_analysis_case".to_string()),
                route_hint: Some("/api/analysis/cases/run".to_string()),
            },
            provider_kind: AnalysisCapabilityProviderKind::Builtin,
            version: "1".to_string(),
        },
        AnalysisCapabilityDescriptor {
            id: "sysml.constraint.analysis".to_string(),
            selector: AnalysisCapabilitySelector {
                types: Vec::new(),
                predicates: vec![AnalysisStructuralPredicate::ConstraintEvaluation],
            },
            effect: AnalysisCapabilityEffect {
                kind: AnalysisOpportunityKind::ConstraintEvaluation,
                label: "Evaluate model constraints".to_string(),
                description: "Constraint definitions or usages are present and can be evaluated against bound model values.".to_string(),
                techniques: vec![AnalysisTechniqueKind::ConstraintEvaluation],
                action_id: Some("solve_constraints".to_string()),
                route_hint: Some("/api/constraints/solve".to_string()),
            },
            provider_kind: AnalysisCapabilityProviderKind::Builtin,
            version: "1".to_string(),
        },
        AnalysisCapabilityDescriptor {
            id: "sysml.requirement.analysis".to_string(),
            selector: AnalysisCapabilitySelector {
                types: Vec::new(),
                predicates: vec![AnalysisStructuralPredicate::RequirementEvaluation],
            },
            effect: AnalysisCapabilityEffect {
                kind: AnalysisOpportunityKind::RequirementEvaluation,
                label: "Evaluate requirements".to_string(),
                description: "Requirements have formal constraints or verification cases that can produce satisfaction evidence.".to_string(),
                techniques: vec![AnalysisTechniqueKind::Verification],
                action_id: Some("analyze_requirement_coverage".to_string()),
                route_hint: Some(
                    "/api/reasoning/capabilities/sysml.requirement.analysis/run".to_string(),
                ),
            },
            provider_kind: AnalysisCapabilityProviderKind::Builtin,
            version: "1".to_string(),
        },
        AnalysisCapabilityDescriptor {
            id: "sysml.behavior.dynamic".to_string(),
            selector: AnalysisCapabilitySelector {
                types: Vec::new(),
                predicates: vec![AnalysisStructuralPredicate::Simulation],
            },
            effect: AnalysisCapabilityEffect {
                kind: AnalysisOpportunityKind::Simulation,
                label: "Explore executable behavior".to_string(),
                description: "State or behavior elements are present and may support dynamic behavior analysis or simulation.".to_string(),
                techniques: vec![AnalysisTechniqueKind::Simulation],
                action_id: Some("analyze_state_machine".to_string()),
                route_hint: Some(
                    "/api/reasoning/capabilities/sysml.behavior.dynamic/run".to_string(),
                ),
            },
            provider_kind: AnalysisCapabilityProviderKind::Builtin,
            version: "1".to_string(),
        },
    ]
}

fn opportunity_from_descriptor(
    descriptor: &AnalysisCapabilityDescriptor,
    id: String,
    elements: Vec<AnalysisElementRef>,
    readiness: AnalysisOpportunityReadiness,
    metadata: serde_json::Map<String, Value>,
) -> AnalysisOpportunity {
    opportunity_from_descriptor_with_label(
        descriptor,
        id,
        descriptor.effect.label.clone(),
        elements,
        readiness,
        metadata,
    )
}

fn opportunity_from_descriptor_with_label(
    descriptor: &AnalysisCapabilityDescriptor,
    id: String,
    label: String,
    elements: Vec<AnalysisElementRef>,
    readiness: AnalysisOpportunityReadiness,
    metadata: serde_json::Map<String, Value>,
) -> AnalysisOpportunity {
    let runnable = matches!(readiness, AnalysisOpportunityReadiness::Runnable);
    AnalysisOpportunity {
        id,
        kind: descriptor.effect.kind,
        label,
        description: descriptor.effect.description.clone(),
        runnable,
        readiness,
        elements,
        techniques: descriptor.effect.techniques.clone(),
        capability_id: Some(descriptor.id.clone()),
        action_id: descriptor.effect.action_id.clone(),
        route_hint: descriptor.effect.route_hint.clone(),
        metadata,
    }
}

fn descriptor_metadata(
    descriptor: &AnalysisCapabilityDescriptor,
) -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([
        ("providerKind".to_string(), json!(descriptor.provider_kind)),
        ("capabilityVersion".to_string(), json!(descriptor.version)),
    ])
}

fn readiness_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    subjects: impl IntoIterator<Item = String>,
) -> Diagnostic {
    let mut diagnostic =
        Diagnostic::new(DiagnosticKind::Readiness, Severity::Warning, code, message);
    diagnostic.subjects = subjects.into_iter().collect();
    diagnostic
}

impl AnalysisCaseModel {
    fn from_element(graph: &Graph, element: &Element) -> Self {
        let calculations = related_elements(graph, element, &["calculation", "calculations"])
            .into_iter()
            .chain(owned_elements_matching(
                graph,
                element,
                is_calculation_usage_kind,
            ))
            .collect::<Vec<_>>();
        let constraints = related_elements(graph, element, &["constraint", "constraints"])
            .into_iter()
            .chain(owned_elements_matching(
                graph,
                element,
                is_constraint_usage_kind,
            ))
            .collect::<Vec<_>>();
        let requirements = related_elements(graph, element, &["requirement", "requirements"]);
        let verification_cases = related_elements(
            graph,
            element,
            &["verification_case", "verification_cases", "verification"],
        );
        let simulations = related_elements(graph, element, &["simulation", "simulations"])
            .into_iter()
            .chain(owned_elements_matching(graph, element, is_simulation_kind))
            .collect::<Vec<_>>();
        let views = related_elements(graph, element, &["view", "views"]);
        let concerns = related_elements(
            graph,
            element,
            &["concern", "concerns", "viewpoint", "viewpoints"],
        );

        let mut techniques = BTreeSet::new();
        if !calculations.is_empty() {
            techniques.insert(AnalysisTechniqueKind::Calculation);
        }
        if !constraints.is_empty() {
            techniques.insert(AnalysisTechniqueKind::ConstraintEvaluation);
        }
        if !simulations.is_empty() {
            techniques.insert(AnalysisTechniqueKind::Simulation);
        }
        if !requirements.is_empty() || !verification_cases.is_empty() {
            techniques.insert(AnalysisTechniqueKind::Verification);
        }
        if has_text_property(element, &["query", "dsl", "script"]) {
            techniques.insert(AnalysisTechniqueKind::Query);
        }

        Self {
            element: element_ref(element),
            subjects: related_elements(graph, element, &["subject", "subjects"]),
            assumptions: related_elements(graph, element, &["assumption", "assumptions"]),
            inputs: related_elements(graph, element, &["input", "inputs"]),
            outputs: related_elements(graph, element, &["output", "outputs"]),
            calculations: dedup_refs(calculations),
            constraints: dedup_refs(constraints),
            requirements,
            verification_cases,
            simulations: dedup_refs(simulations),
            views,
            concerns,
            techniques: techniques.into_iter().collect(),
        }
    }

    pub fn workflow(&self) -> AnalysisWorkflow {
        let mut steps = Vec::new();
        steps.push(AnalysisWorkflowStep {
            kind: AnalysisWorkflowStepKind::ScopeSubject,
            label: "Scope analysis subject".to_string(),
            elements: self.subjects.clone(),
            techniques: Vec::new(),
        });

        if !self.assumptions.is_empty() {
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::BindAssumptions,
                label: "Bind assumptions".to_string(),
                elements: self.assumptions.clone(),
                techniques: Vec::new(),
            });
        }

        if !(self.inputs.is_empty() && self.outputs.is_empty()) {
            let mut elements = self.inputs.clone();
            elements.extend(self.outputs.clone());
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::ResolveInputs,
                label: "Resolve inputs and expected outputs".to_string(),
                elements,
                techniques: Vec::new(),
            });
        }

        let execution_elements = self
            .calculations
            .iter()
            .chain(&self.simulations)
            .cloned()
            .collect::<Vec<_>>();
        if !execution_elements.is_empty() {
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::ExecuteTechnique,
                label: "Execute calculations and simulations".to_string(),
                elements: execution_elements,
                techniques: self
                    .techniques
                    .iter()
                    .copied()
                    .filter(|technique| {
                        matches!(
                            technique,
                            AnalysisTechniqueKind::Calculation
                                | AnalysisTechniqueKind::Simulation
                                | AnalysisTechniqueKind::Query
                        )
                    })
                    .collect(),
            });
        }

        if !self.constraints.is_empty() {
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::EvaluateConstraints,
                label: "Evaluate constraints".to_string(),
                elements: self.constraints.clone(),
                techniques: vec![AnalysisTechniqueKind::ConstraintEvaluation],
            });
        }

        if !(self.requirements.is_empty() && self.verification_cases.is_empty()) {
            let mut elements = self.requirements.clone();
            elements.extend(self.verification_cases.clone());
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::EvaluateRequirements,
                label: "Evaluate requirements and verification cases".to_string(),
                elements,
                techniques: vec![AnalysisTechniqueKind::Verification],
            });
        }

        if !(self.views.is_empty() && self.concerns.is_empty()) {
            let mut elements = self.views.clone();
            elements.extend(self.concerns.clone());
            steps.push(AnalysisWorkflowStep {
                kind: AnalysisWorkflowStepKind::ProduceViews,
                label: "Produce stakeholder views".to_string(),
                elements,
                techniques: Vec::new(),
            });
        }

        steps.push(AnalysisWorkflowStep {
            kind: AnalysisWorkflowStepKind::RecordResult,
            label: "Record analysis result and evidence".to_string(),
            elements: vec![self.element.clone()],
            techniques: self.techniques.clone(),
        });

        AnalysisWorkflow {
            case_id: self.element.element_id.clone(),
            steps,
        }
    }
}

fn requirement_evaluation_from_element(
    graph: &Graph,
    element: &Element,
) -> RequirementEvaluationModel {
    let formal_constraints = related_elements(
        graph,
        element,
        &[
            "constraint",
            "constraints",
            "formal_constraint",
            "formal_constraints",
        ],
    )
    .into_iter()
    .chain(owned_elements_matching(
        graph,
        element,
        is_constraint_usage_kind,
    ))
    .collect::<Vec<_>>();

    RequirementEvaluationModel {
        requirement: element_ref(element),
        formal_constraints: dedup_refs(formal_constraints),
        verification_cases: related_elements(
            graph,
            element,
            &["verification_case", "verification_cases", "verification"],
        ),
    }
}

fn collect_by_kind<F>(graph: &Graph, predicate: F) -> Vec<AnalysisElementRef>
where
    F: Fn(&str) -> bool,
{
    graph
        .elements()
        .iter()
        .filter(|element| is_authored_model_element(element))
        .filter(|element| predicate(element.kind.as_ref()))
        .map(element_ref)
        .collect()
}

fn related_elements(
    graph: &Graph,
    element: &Element,
    properties: &[&str],
) -> Vec<AnalysisElementRef> {
    let mut refs = Vec::new();
    for property in properties {
        if let Some(value) = element.properties.get(*property) {
            collect_refs_from_value(graph, value, &mut refs);
        }
    }
    dedup_refs(refs)
}

fn collect_refs_from_value(graph: &Graph, value: &Value, refs: &mut Vec<AnalysisElementRef>) {
    match value {
        Value::String(element_id) => {
            if let Some(element) = graph.element_by_element_id(element_id) {
                refs.push(element_ref(element));
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_refs_from_value(graph, value, refs);
            }
        }
        _ => {}
    }
}

fn owned_elements_matching<F>(
    graph: &Graph,
    owner: &Element,
    predicate: F,
) -> Vec<AnalysisElementRef>
where
    F: Fn(&str) -> bool,
{
    graph
        .incoming(owner.id, "owner")
        .filter_map(|edge| graph.element(edge.source))
        .filter(|element| predicate(element.kind.as_ref()))
        .map(element_ref)
        .collect()
}

fn element_ref(element: &Element) -> AnalysisElementRef {
    AnalysisElementRef::from_graph_element(element)
}

fn string_property(element: &Element, name: &str) -> Option<String> {
    element
        .properties
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn has_text_property(element: &Element, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| string_property(element, name).is_some())
}

fn dedup_refs(refs: Vec<AnalysisElementRef>) -> Vec<AnalysisElementRef> {
    let mut seen = BTreeSet::new();
    refs.into_iter()
        .filter(|reference| seen.insert(reference.element_id.clone()))
        .collect()
}

fn is_analysis_case_kind(kind: &str) -> bool {
    kind_contains(kind, "analysiscase")
}

fn is_calculation_usage_kind(kind: &str) -> bool {
    kind_contains(kind, "calculationusage")
}

fn is_constraint_usage_kind(kind: &str) -> bool {
    kind_contains(kind, "constraintusage")
        || canonical_kind(kind) == "constraint"
        || kind_contains(kind, "assertconstraintusage")
}

fn is_requirement_kind(kind: &str) -> bool {
    kind_contains(kind, "requirement")
}

fn is_verification_case_kind(kind: &str) -> bool {
    kind_contains(kind, "verificationcase")
}

fn is_view_kind(kind: &str) -> bool {
    let kind = canonical_kind(kind);
    kind == "view" || kind.contains("viewusage") || kind.contains("viewdefinition")
}

fn is_concern_or_viewpoint_kind(kind: &str) -> bool {
    kind_contains(kind, "concern") || kind_contains(kind, "viewpoint")
}

fn is_simulation_kind(kind: &str) -> bool {
    kind_contains(kind, "simulation")
        || kind_contains(kind, "stateusage")
        || kind_contains(kind, "statedefinition")
        || kind_contains(kind, "behavior")
}

fn kind_contains(kind: &str, needle: &str) -> bool {
    canonical_kind(kind).contains(needle)
}

fn is_authored_model_element(element: &Element) -> bool {
    element.layer >= 2
}

fn canonical_kind(kind: &str) -> String {
    kind.replace([':', '.', ' ', '_'], "").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::{KirDocument, KirElement};
    use crate::model::Graph;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn element(id: &str, kind: &str, properties: BTreeMap<String, Value>) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: kind.to_string(),
            layer: 2,
            properties,
        }
    }

    fn analysis_graph() -> Graph {
        Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element(
                    "part.Vehicle",
                    "PartUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("Vehicle"))]),
                ),
                element(
                    "assumption.StartupNominal",
                    "RequirementUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("StartupNominal"))]),
                ),
                element(
                    "input.coolantTemp",
                    "AttributeUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("coolantTemp"))]),
                ),
                element(
                    "output.verdict",
                    "AttributeUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("verdict"))]),
                ),
                element(
                    "calcdef.StartMargin",
                    "CalculationDefinition",
                    BTreeMap::from([("declared_name".to_string(), json!("StartMargin"))]),
                ),
                element(
                    "calc.StartMargin.Run",
                    "CalculationUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("RunStartMargin")),
                        ("definition".to_string(), json!("calcdef.StartMargin")),
                    ]),
                ),
                element(
                    "constraintdef.SafeStart",
                    "ConstraintDefinition",
                    BTreeMap::from([("declared_name".to_string(), json!("SafeStart"))]),
                ),
                element(
                    "constraint.SafeStart.Applied",
                    "ConstraintUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("AppliedSafeStart")),
                        ("definition".to_string(), json!("constraintdef.SafeStart")),
                    ]),
                ),
                element(
                    "req.SafeStart",
                    "RequirementUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("SafeStart")),
                        (
                            "constraints".to_string(),
                            json!(["constraint.SafeStart.Applied"]),
                        ),
                    ]),
                ),
                element(
                    "verify.SafeStart",
                    "VerificationCaseUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("VerifySafeStart")),
                        ("requirement".to_string(), json!("req.SafeStart")),
                    ]),
                ),
                element(
                    "state.StartupSimulation",
                    "StateUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("StartupSimulation"))]),
                ),
                element(
                    "concern.Safety",
                    "ConcernUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("Safety"))]),
                ),
                element(
                    "view.StartupSafety",
                    "ViewUsage",
                    BTreeMap::from([("declared_name".to_string(), json!("StartupSafety"))]),
                ),
                element(
                    "analysis.StartupSafety",
                    "AnalysisCaseUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("StartupSafety")),
                        ("subject".to_string(), json!("part.Vehicle")),
                        (
                            "assumptions".to_string(),
                            json!(["assumption.StartupNominal"]),
                        ),
                        ("inputs".to_string(), json!(["input.coolantTemp"])),
                        ("outputs".to_string(), json!(["output.verdict"])),
                        ("calculations".to_string(), json!(["calc.StartMargin.Run"])),
                        (
                            "constraints".to_string(),
                            json!(["constraint.SafeStart.Applied"]),
                        ),
                        ("requirements".to_string(), json!(["req.SafeStart"])),
                        (
                            "verification_cases".to_string(),
                            json!(["verify.SafeStart"]),
                        ),
                        (
                            "simulations".to_string(),
                            json!(["state.StartupSimulation"]),
                        ),
                        ("concerns".to_string(), json!(["concern.Safety"])),
                        ("views".to_string(), json!(["view.StartupSafety"])),
                        ("script".to_string(), json!("model.all_parts().len")),
                    ]),
                ),
            ],
        })
        .expect("analysis fixture should build a graph")
    }

    fn constraint_definition_only_graph() -> Graph {
        Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![element(
                "constraintdef.SafeStart",
                "ConstraintDefinition",
                BTreeMap::from([("declared_name".to_string(), json!("SafeStart"))]),
            )],
        })
        .expect("constraint definition fixture should build a graph")
    }

    #[test]
    fn inventory_extracts_sysml_analysis_nouns() {
        let graph = analysis_graph();
        let inventory = AnalysisInventory::from_graph(&graph);

        assert_eq!(inventory.analysis_cases.len(), 1);
        assert_eq!(inventory.calculation_definitions.len(), 1);
        assert_eq!(inventory.calculation_usages.len(), 1);
        assert_eq!(inventory.constraint_definitions.len(), 1);
        assert_eq!(inventory.constraint_usages.len(), 1);
        assert_eq!(inventory.requirement_evaluations.len(), 2);
        assert_eq!(inventory.verification_cases.len(), 1);
        assert_eq!(inventory.views.len(), 1);
        assert_eq!(inventory.concerns.len(), 1);
        assert_eq!(inventory.simulations.len(), 1);
        assert!(inventory.authored_elements.len() >= 14);

        let case = &inventory.analysis_cases[0];
        assert_eq!(case.element.element_id, "analysis.StartupSafety");
        assert_eq!(case.subjects[0].element_id, "part.Vehicle");
        assert_eq!(case.calculations[0].element_id, "calc.StartMargin.Run");
        assert_eq!(
            case.constraints[0].element_id,
            "constraint.SafeStart.Applied"
        );
        assert_eq!(case.requirements[0].element_id, "req.SafeStart");
        assert_eq!(case.verification_cases[0].element_id, "verify.SafeStart");
        assert_eq!(case.simulations[0].element_id, "state.StartupSimulation");
        assert!(
            case.techniques
                .contains(&AnalysisTechniqueKind::Calculation)
        );
        assert!(
            case.techniques
                .contains(&AnalysisTechniqueKind::ConstraintEvaluation)
        );
        assert!(case.techniques.contains(&AnalysisTechniqueKind::Simulation));
        assert!(
            case.techniques
                .contains(&AnalysisTechniqueKind::Verification)
        );
        assert!(case.techniques.contains(&AnalysisTechniqueKind::Query));
    }

    #[test]
    fn requirement_evaluation_links_formal_constraints() {
        let graph = analysis_graph();
        let inventory = AnalysisInventory::from_graph(&graph);
        let requirement = inventory
            .requirement_evaluations
            .iter()
            .find(|evaluation| evaluation.requirement.element_id == "req.SafeStart")
            .expect("safe-start requirement should be inventoried");

        assert_eq!(
            requirement.formal_constraints[0].element_id,
            "constraint.SafeStart.Applied"
        );
    }

    #[test]
    fn workflow_orders_analysis_execution_from_case_context() {
        let graph = analysis_graph();
        let inventory = AnalysisInventory::from_graph(&graph);
        let workflow = inventory.analysis_cases[0].workflow();
        let step_kinds = workflow
            .steps
            .iter()
            .map(|step| step.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            step_kinds,
            vec![
                AnalysisWorkflowStepKind::ScopeSubject,
                AnalysisWorkflowStepKind::BindAssumptions,
                AnalysisWorkflowStepKind::ResolveInputs,
                AnalysisWorkflowStepKind::ExecuteTechnique,
                AnalysisWorkflowStepKind::EvaluateConstraints,
                AnalysisWorkflowStepKind::EvaluateRequirements,
                AnalysisWorkflowStepKind::ProduceViews,
                AnalysisWorkflowStepKind::RecordResult,
            ]
        );
        assert!(
            workflow.steps[3]
                .techniques
                .contains(&AnalysisTechniqueKind::Simulation)
        );
    }

    #[test]
    fn opportunities_surface_analysis_cases_and_executable_semantics() {
        let graph = analysis_graph();
        let inventory = AnalysisInventory::from_graph(&graph);
        let report = inventory.opportunities();

        assert_eq!(report.schema, "mercurio.analysis.opportunities.v1");
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == AnalysisOpportunityKind::AnalysisCase
                && opportunity.action_id.as_deref() == Some("run_analysis_case")
        }));
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == AnalysisOpportunityKind::ConstraintEvaluation
                && opportunity.capability_id.as_deref() == Some("sysml.constraint.analysis")
        }));
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == AnalysisOpportunityKind::RequirementEvaluation
                && opportunity.capability_id.as_deref() == Some("sysml.requirement.analysis")
        }));
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == AnalysisOpportunityKind::Simulation
                && opportunity.capability_id.as_deref() == Some("sysml.behavior.dynamic")
                && opportunity.elements[0].element_id == "state.StartupSimulation"
        }));
    }

    #[test]
    fn descriptor_type_join_surfaces_plugin_capability_opportunities() {
        let graph = analysis_graph();
        let inventory = AnalysisInventory::from_graph(&graph);
        let descriptor = AnalysisCapabilityDescriptor {
            id: "plugin.state.review".to_string(),
            selector: AnalysisCapabilitySelector {
                types: vec!["StateDefinition".to_string(), "StateUsage".to_string()],
                predicates: Vec::new(),
            },
            effect: AnalysisCapabilityEffect {
                kind: AnalysisOpportunityKind::Simulation,
                label: "Review state behavior".to_string(),
                description: "Plugin-provided state behavior review.".to_string(),
                techniques: vec![AnalysisTechniqueKind::Simulation],
                action_id: Some("run_plugin_capability".to_string()),
                route_hint: Some("/api/reasoning/capabilities/plugin.state.review/run".to_string()),
            },
            provider_kind: AnalysisCapabilityProviderKind::Plugin,
            version: "2026.7".to_string(),
        };

        let report = inventory.opportunities_for_descriptors(&[descriptor]);

        assert_eq!(report.opportunities.len(), 1);
        let opportunity = &report.opportunities[0];
        assert_eq!(
            opportunity.capability_id.as_deref(),
            Some("plugin.state.review")
        );
        assert!(opportunity.runnable);
        assert_eq!(
            opportunity.readiness,
            AnalysisOpportunityReadiness::Runnable
        );
        assert_eq!(
            opportunity.elements[0].element_id,
            "state.StartupSimulation"
        );
        assert_eq!(
            opportunity.metadata.get("providerKind"),
            Some(&json!("plugin"))
        );
        assert_eq!(
            opportunity.metadata.get("capabilityVersion"),
            Some(&json!("2026.7"))
        );
    }

    #[test]
    fn constraint_definitions_without_usages_surface_blocked_readiness() {
        let graph = constraint_definition_only_graph();
        let inventory = AnalysisInventory::from_graph(&graph);
        let report = inventory.opportunities();
        let opportunity = report
            .opportunities
            .iter()
            .find(|opportunity| opportunity.kind == AnalysisOpportunityKind::ConstraintEvaluation)
            .expect("constraint definitions should surface a readiness-blocked opportunity");

        assert!(!opportunity.runnable);
        match &opportunity.readiness {
            AnalysisOpportunityReadiness::Blocked { diagnostic } => {
                assert_eq!(diagnostic.kind, DiagnosticKind::Readiness);
                assert_eq!(diagnostic.code, "analysis.constraint.unbound");
                assert_eq!(diagnostic.subjects, vec!["constraintdef.SafeStart"]);
            }
            AnalysisOpportunityReadiness::Runnable => {
                panic!("constraint definition without usage should be blocked")
            }
        }
    }
}
