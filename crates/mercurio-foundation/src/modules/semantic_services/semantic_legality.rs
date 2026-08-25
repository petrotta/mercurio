use serde::{Deserialize, Serialize};

use crate::runtime::{
    DatalogError, Fact, RuleDiagnostic, RuleDiagnosticSeverity, RulePack, evaluate,
    evaluate_diagnostics,
};
use crate::semantic_services::semantic_profile::{
    AttributePolicyAnswer, CapabilityAnswer, ConservativeSemanticCapabilityOracle,
    SemanticCapabilityOracle, SemanticElementAuthoring,
};

pub const SEMANTIC_LEGALITY_SCHEMA_VERSION: &str = "mercurio.semantic_legality.v1";

#[derive(Debug, Clone)]
pub struct SemanticLegalityService<O = ConservativeSemanticCapabilityOracle> {
    oracle: O,
    rulepacks: Vec<RulePack>,
}

impl SemanticLegalityService<ConservativeSemanticCapabilityOracle> {
    pub fn new() -> Self {
        Self {
            oracle: ConservativeSemanticCapabilityOracle,
            rulepacks: Vec::new(),
        }
    }
}

impl Default for SemanticLegalityService<ConservativeSemanticCapabilityOracle> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> SemanticLegalityService<O>
where
    O: SemanticCapabilityOracle,
{
    pub fn with_oracle(oracle: O) -> Self {
        Self {
            oracle,
            rulepacks: Vec::new(),
        }
    }

    pub fn with_oracle_and_rulepacks(oracle: O, rulepacks: Vec<RulePack>) -> Self {
        Self { oracle, rulepacks }
    }

    pub fn rulepacks(&self) -> &[RulePack] {
        &self.rulepacks
    }

    pub fn check(&self, request: SemanticLegalityRequest) -> SemanticLegalityReport {
        let answer = self.oracle_answer(&request.operation);
        let mut diagnostics = diagnostics_from_capability_answer(&answer);

        match self.evaluate_rule_diagnostics(&request) {
            Ok(rule_diagnostics) => diagnostics.extend(
                rule_diagnostics
                    .into_iter()
                    .map(SemanticLegalityDiagnostic::from_rule_diagnostic),
            ),
            Err(error) => diagnostics.push(SemanticLegalityDiagnostic {
                code: "semantic.legality.rule_evaluation_failed".to_string(),
                severity: RuleDiagnosticSeverity::Error,
                message: error.to_string(),
                subjects: Vec::new(),
                source: SemanticLegalityDiagnosticSource::RuleEvaluation,
                source_facts: Vec::new(),
            }),
        }

        let status = status_from_answer_and_diagnostics(&answer, &diagnostics);
        SemanticLegalityReport {
            schema_version: SEMANTIC_LEGALITY_SCHEMA_VERSION.to_string(),
            operation: request.operation,
            status,
            answer,
            diagnostics,
        }
    }

    pub fn check_containment(
        &self,
        container_kind: impl Into<String>,
        child_kind: impl Into<String>,
    ) -> SemanticLegalityReport {
        self.check(SemanticLegalityRequest::containment(
            container_kind,
            child_kind,
        ))
    }

    pub fn check_specialization(
        &self,
        source_kind: impl Into<String>,
        target_kind: impl Into<String>,
    ) -> SemanticLegalityReport {
        self.check(SemanticLegalityRequest::specialization(
            source_kind,
            target_kind,
        ))
    }

    pub fn check_usage_typing(
        &self,
        usage_kind: impl Into<String>,
        definition_kind: impl Into<String>,
    ) -> SemanticLegalityReport {
        self.check(SemanticLegalityRequest::usage_typing(
            usage_kind,
            definition_kind,
        ))
    }

    pub fn check_relationship(
        &self,
        relationship_kind: impl Into<String>,
        source_kind: impl Into<String>,
        target_kind: impl Into<String>,
    ) -> SemanticLegalityReport {
        self.check(SemanticLegalityRequest::relationship(
            relationship_kind,
            source_kind,
            target_kind,
        ))
    }

    pub fn check_attribute_write(
        &self,
        kind: impl Into<String>,
        attribute: impl Into<String>,
    ) -> SemanticLegalityReport {
        self.check(SemanticLegalityRequest::attribute_write(kind, attribute))
    }

    pub fn attribute_policy(&self, kind: &str, attribute: &str) -> AttributePolicyAnswer {
        self.oracle.attribute_policy(kind, attribute)
    }

    pub fn supporting_definition_keyword_for_usage(&self, usage_kind: &str) -> Option<String> {
        self.oracle
            .supporting_definition_keyword_for_usage(usage_kind)
    }

    pub fn normalize_definition_keyword(&self, keyword: &str) -> String {
        self.oracle.normalize_definition_keyword(keyword)
    }

    pub fn authoring_for_element_kind(&self, kind: &str) -> Option<SemanticElementAuthoring> {
        self.oracle.authoring_for_element_kind(kind)
    }

    pub fn semantic_kind_for_definition_keyword(&self, keyword: &str) -> Option<String> {
        self.oracle.semantic_kind_for_definition_keyword(keyword)
    }

    pub fn semantic_kind_for_usage_keyword(&self, keyword: &str) -> Option<String> {
        self.oracle.semantic_kind_for_usage_keyword(keyword)
    }

    fn oracle_answer(&self, operation: &SemanticLegalityOperation) -> CapabilityAnswer {
        match operation {
            SemanticLegalityOperation::Containment {
                container_kind,
                child_kind,
            } => self.oracle.can_contain(container_kind, child_kind),
            SemanticLegalityOperation::Specialization {
                source_kind,
                target_kind,
            } => self.oracle.can_specialize(source_kind, target_kind),
            SemanticLegalityOperation::UsageTyping {
                usage_kind,
                definition_kind,
            } => self.oracle.can_type_usage(usage_kind, definition_kind),
            SemanticLegalityOperation::Relationship {
                relationship_kind,
                source_kind,
                target_kind,
            } => self
                .oracle
                .can_relate(relationship_kind, source_kind, target_kind),
            SemanticLegalityOperation::AttributeWrite { kind, attribute } => {
                let policy = self.oracle.attribute_policy(kind, attribute);
                if policy.writable {
                    CapabilityAnswer::Allowed
                } else {
                    CapabilityAnswer::Denied(
                        policy
                            .reason
                            .unwrap_or_else(|| format!("attribute `{attribute}` is not writable")),
                    )
                }
            }
        }
    }

    fn evaluate_rule_diagnostics(
        &self,
        request: &SemanticLegalityRequest,
    ) -> Result<Vec<RuleDiagnostic>, DatalogError> {
        if self.rulepacks.is_empty() {
            return Ok(Vec::new());
        }

        let mut facts = request.operation.facts();
        facts.extend(request.facts.iter().cloned());

        let mut rulepacks = vec![RulePack::structural_core()];
        rulepacks.extend(self.rulepacks.iter().cloned());
        for pack in &rulepacks {
            facts.extend(pack.facts.iter().cloned());
        }
        let rules = rulepacks
            .iter()
            .flat_map(|pack| pack.rules.iter().cloned())
            .collect::<Vec<_>>();
        let diagnostics = rulepacks
            .iter()
            .flat_map(|pack| pack.diagnostics.iter().cloned())
            .collect::<Vec<_>>();
        let evaluation = evaluate(facts, &rules)?;
        evaluate_diagnostics(&evaluation, &diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLegalityRequest {
    pub operation: SemanticLegalityOperation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Fact>,
}

impl SemanticLegalityRequest {
    pub fn new(operation: SemanticLegalityOperation) -> Self {
        Self {
            operation,
            facts: Vec::new(),
        }
    }

    pub fn containment(container_kind: impl Into<String>, child_kind: impl Into<String>) -> Self {
        Self::new(SemanticLegalityOperation::Containment {
            container_kind: container_kind.into(),
            child_kind: child_kind.into(),
        })
    }

    pub fn specialization(source_kind: impl Into<String>, target_kind: impl Into<String>) -> Self {
        Self::new(SemanticLegalityOperation::Specialization {
            source_kind: source_kind.into(),
            target_kind: target_kind.into(),
        })
    }

    pub fn usage_typing(usage_kind: impl Into<String>, definition_kind: impl Into<String>) -> Self {
        Self::new(SemanticLegalityOperation::UsageTyping {
            usage_kind: usage_kind.into(),
            definition_kind: definition_kind.into(),
        })
    }

    pub fn relationship(
        relationship_kind: impl Into<String>,
        source_kind: impl Into<String>,
        target_kind: impl Into<String>,
    ) -> Self {
        Self::new(SemanticLegalityOperation::Relationship {
            relationship_kind: relationship_kind.into(),
            source_kind: source_kind.into(),
            target_kind: target_kind.into(),
        })
    }

    pub fn attribute_write(kind: impl Into<String>, attribute: impl Into<String>) -> Self {
        Self::new(SemanticLegalityOperation::AttributeWrite {
            kind: kind.into(),
            attribute: attribute.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SemanticLegalityOperation {
    Containment {
        #[serde(rename = "containerKind")]
        container_kind: String,
        #[serde(rename = "childKind")]
        child_kind: String,
    },
    Specialization {
        #[serde(rename = "sourceKind")]
        source_kind: String,
        #[serde(rename = "targetKind")]
        target_kind: String,
    },
    UsageTyping {
        #[serde(rename = "usageKind")]
        usage_kind: String,
        #[serde(rename = "definitionKind")]
        definition_kind: String,
    },
    Relationship {
        #[serde(rename = "relationshipKind")]
        relationship_kind: String,
        #[serde(rename = "sourceKind")]
        source_kind: String,
        #[serde(rename = "targetKind")]
        target_kind: String,
    },
    AttributeWrite {
        #[serde(rename = "elementKind")]
        kind: String,
        attribute: String,
    },
}

impl SemanticLegalityOperation {
    pub fn facts(&self) -> Vec<Fact> {
        match self {
            Self::Containment {
                container_kind,
                child_kind,
            } => vec![
                Fact::new("legality_operation", ["containment".to_string()]),
                Fact::new(
                    "legality_containment_request",
                    [container_kind.clone(), child_kind.clone()],
                ),
            ],
            Self::Specialization {
                source_kind,
                target_kind,
            } => vec![
                Fact::new("legality_operation", ["specialization".to_string()]),
                Fact::new(
                    "legality_specialization_request",
                    [source_kind.clone(), target_kind.clone()],
                ),
            ],
            Self::UsageTyping {
                usage_kind,
                definition_kind,
            } => vec![
                Fact::new("legality_operation", ["usage_typing".to_string()]),
                Fact::new(
                    "legality_usage_typing_request",
                    [usage_kind.clone(), definition_kind.clone()],
                ),
            ],
            Self::Relationship {
                relationship_kind,
                source_kind,
                target_kind,
            } => vec![
                Fact::new("legality_operation", ["relationship".to_string()]),
                Fact::new(
                    "legality_relationship_request",
                    [
                        relationship_kind.clone(),
                        source_kind.clone(),
                        target_kind.clone(),
                    ],
                ),
            ],
            Self::AttributeWrite { kind, attribute } => vec![
                Fact::new("legality_operation", ["attribute_write".to_string()]),
                Fact::new(
                    "legality_attribute_write_request",
                    [kind.clone(), attribute.clone()],
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLegalityReport {
    pub schema_version: String,
    pub operation: SemanticLegalityOperation,
    pub status: SemanticLegalityStatus,
    pub answer: CapabilityAnswer,
    pub diagnostics: Vec<SemanticLegalityDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticLegalityStatus {
    Allowed,
    AllowedWithWarnings,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLegalityDiagnostic {
    pub code: String,
    pub severity: RuleDiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    pub source: SemanticLegalityDiagnosticSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_facts: Vec<Fact>,
}

impl SemanticLegalityDiagnostic {
    fn from_rule_diagnostic(diagnostic: RuleDiagnostic) -> Self {
        Self {
            code: diagnostic.rule_id,
            severity: diagnostic.severity,
            message: diagnostic.message,
            subjects: diagnostic.subjects,
            source: SemanticLegalityDiagnosticSource::Rulepack,
            source_facts: diagnostic.source_facts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticLegalityDiagnosticSource {
    Oracle,
    Rulepack,
    RuleEvaluation,
}

fn diagnostics_from_capability_answer(
    answer: &CapabilityAnswer,
) -> Vec<SemanticLegalityDiagnostic> {
    match answer {
        CapabilityAnswer::Allowed => Vec::new(),
        CapabilityAnswer::Denied(message) => vec![SemanticLegalityDiagnostic {
            code: "semantic.legality.oracle_denied".to_string(),
            severity: RuleDiagnosticSeverity::Error,
            message: message.clone(),
            subjects: Vec::new(),
            source: SemanticLegalityDiagnosticSource::Oracle,
            source_facts: Vec::new(),
        }],
        CapabilityAnswer::Unknown(message) => vec![SemanticLegalityDiagnostic {
            code: "semantic.legality.oracle_unknown".to_string(),
            severity: RuleDiagnosticSeverity::Warning,
            message: message.clone(),
            subjects: Vec::new(),
            source: SemanticLegalityDiagnosticSource::Oracle,
            source_facts: Vec::new(),
        }],
    }
}

fn status_from_answer_and_diagnostics(
    answer: &CapabilityAnswer,
    diagnostics: &[SemanticLegalityDiagnostic],
) -> SemanticLegalityStatus {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == RuleDiagnosticSeverity::Error)
    {
        return SemanticLegalityStatus::Blocked;
    }
    if matches!(answer, CapabilityAnswer::Unknown(_)) {
        return SemanticLegalityStatus::Unknown;
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == RuleDiagnosticSeverity::Warning)
    {
        return SemanticLegalityStatus::AllowedWithWarnings;
    }
    SemanticLegalityStatus::Allowed
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::runtime::{Atom, DiagnosticRule, Term};

    #[test]
    fn reports_oracle_answers_as_legality_status() {
        let service = SemanticLegalityService::new();

        let report = service.check_containment("package", "part");

        assert_eq!(report.status, SemanticLegalityStatus::Allowed);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn rulepack_error_diagnostic_blocks_otherwise_allowed_operation() {
        let rulepack = RulePack {
            id: "test.legality".to_string(),
            version: "0.1.0".to_string(),
            metadata: BTreeMap::<String, Value>::new(),
            facts: vec![Fact::new(
                "forbidden_relationship_target",
                ["part".to_string()],
            )],
            rules: Vec::new(),
            diagnostics: vec![DiagnosticRule {
                id: "test.satisfy.target_requirement".to_string(),
                severity: RuleDiagnosticSeverity::Error,
                message: "satisfy must target a requirement-like element".to_string(),
                subjects: vec![Term::Var("Target".to_string())],
                when: vec![
                    Atom {
                        predicate: "legality_relationship_request".to_string(),
                        terms: vec![
                            Term::Const("satisfy".to_string()),
                            Term::Var("Source".to_string()),
                            Term::Var("Target".to_string()),
                        ],
                    },
                    Atom {
                        predicate: "forbidden_relationship_target".to_string(),
                        terms: vec![Term::Var("Target".to_string())],
                    },
                ],
            }],
        };
        let service = SemanticLegalityService::with_oracle_and_rulepacks(
            ConservativeSemanticCapabilityOracle,
            vec![rulepack],
        );

        let report = service.check_relationship("satisfy", "part", "part");

        assert_eq!(report.status, SemanticLegalityStatus::Blocked);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "test.satisfy.target_requirement"
                && diagnostic.source == SemanticLegalityDiagnosticSource::Rulepack
                && diagnostic.subjects == ["part"]
        }));
    }
}
