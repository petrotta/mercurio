use serde::{Deserialize, Serialize};

use crate::runtime::{
    DatalogError, Fact, FactIndex, Rule, RuleDiagnostic, RuleDiagnosticSeverity, RulePack,
    evaluate, evaluate_diagnostics, evaluate_diagnostics_with_overlay,
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
    engine: Option<LegalityRuleEngine>,
}

/// The rulepack contents merged once at service construction so per-check
/// evaluation does not re-clone every rulepack.
#[derive(Debug, Clone)]
struct LegalityRuleEngine {
    base_facts: Vec<Fact>,
    rules: Vec<Rule>,
    diagnostics: Vec<crate::runtime::DiagnosticRule>,
    rule_body_predicates: std::collections::BTreeSet<String>,
}

impl LegalityRuleEngine {
    fn from_rulepacks(rulepacks: &[RulePack]) -> Option<Self> {
        if rulepacks.is_empty() {
            return None;
        }
        let mut packs = vec![RulePack::structural_core()];
        packs.extend(rulepacks.iter().cloned());
        let base_facts = packs
            .iter()
            .flat_map(|pack| pack.facts.iter().cloned())
            .collect();
        let rules = packs
            .iter()
            .flat_map(|pack| pack.rules.iter().cloned())
            .collect::<Vec<_>>();
        let diagnostics = packs
            .iter()
            .flat_map(|pack| pack.diagnostics.iter().cloned())
            .collect();
        let rule_body_predicates = rules
            .iter()
            .flat_map(|rule| rule.body.iter())
            .map(|atom| atom.predicate.clone())
            .collect();
        Some(Self {
            base_facts,
            rules,
            diagnostics,
            rule_body_predicates,
        })
    }
}

impl SemanticLegalityService<ConservativeSemanticCapabilityOracle> {
    pub fn new() -> Self {
        Self::with_oracle(ConservativeSemanticCapabilityOracle)
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
        Self::with_oracle_and_rulepacks(oracle, Vec::new())
    }

    pub fn with_oracle_and_rulepacks(oracle: O, rulepacks: Vec<RulePack>) -> Self {
        let engine = LegalityRuleEngine::from_rulepacks(&rulepacks);
        Self {
            oracle,
            rulepacks,
            engine,
        }
    }

    pub fn rulepacks(&self) -> &[RulePack] {
        &self.rulepacks
    }

    pub fn check(&self, request: SemanticLegalityRequest) -> SemanticLegalityReport {
        let answer = self.oracle_answer(&request.operation);
        let mut diagnostics = diagnostics_from_capability_answer(&answer);
        append_rule_diagnostics(&mut diagnostics, self.evaluate_rule_diagnostics(&request));

        let status = status_from_answer_and_diagnostics(&answer, &diagnostics);
        SemanticLegalityReport {
            schema_version: SEMANTIC_LEGALITY_SCHEMA_VERSION.to_string(),
            operation: request.operation,
            status,
            answer,
            diagnostics,
        }
    }

    /// Prepares a batch evaluator that runs the rulepack fixpoint once over
    /// `shared_facts` and answers many per-operation checks against it.
    ///
    /// Equivalent to calling [`Self::check`] with the same `shared_facts` for
    /// every operation, but the (expensive) rule evaluation happens once
    /// instead of once per check.
    pub fn batch(&self, shared_facts: Vec<Fact>) -> SemanticLegalityBatch<'_, O> {
        let base = self.engine.as_ref().map(|engine| {
            let facts = shared_facts
                .iter()
                .cloned()
                .chain(engine.base_facts.iter().cloned());
            evaluate(facts, &engine.rules).map(|evaluation| FactIndex::from_evaluation(&evaluation))
        });
        SemanticLegalityBatch {
            service: self,
            shared_facts,
            base,
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
        let Some(engine) = &self.engine else {
            return Ok(Vec::new());
        };

        let mut facts = request.operation.facts();
        facts.extend(request.facts.iter().cloned());
        facts.extend(engine.base_facts.iter().cloned());
        let evaluation = evaluate(facts, &engine.rules)?;
        evaluate_diagnostics(&evaluation, &engine.diagnostics)
    }
}

/// Answers many legality checks that share one fact context, evaluating the
/// rulepack fixpoint once instead of once per check. Produced by
/// [`SemanticLegalityService::batch`].
#[derive(Debug)]
pub struct SemanticLegalityBatch<'a, O = ConservativeSemanticCapabilityOracle> {
    service: &'a SemanticLegalityService<O>,
    shared_facts: Vec<Fact>,
    base: Option<Result<FactIndex, DatalogError>>,
}

impl<O> SemanticLegalityBatch<'_, O>
where
    O: SemanticCapabilityOracle,
{
    /// Equivalent to `service.check(SemanticLegalityRequest { operation,
    /// facts: shared_facts })`.
    pub fn check(&self, operation: SemanticLegalityOperation) -> SemanticLegalityReport {
        let op_facts = operation.facts();
        if let Some(engine) = &self.service.engine
            && op_facts
                .iter()
                .any(|fact| engine.rule_body_predicates.contains(&fact.predicate))
        {
            // An operation fact could feed a derivation rule, so the shared
            // fixpoint is not reusable for this operation - run the full
            // per-check evaluation instead.
            return self.service.check(SemanticLegalityRequest {
                operation,
                facts: self.shared_facts.clone(),
            });
        }

        let answer = self.service.oracle_answer(&operation);
        let mut diagnostics = diagnostics_from_capability_answer(&answer);
        match (&self.service.engine, &self.base) {
            (Some(engine), Some(Ok(index))) => append_rule_diagnostics(
                &mut diagnostics,
                evaluate_diagnostics_with_overlay(index, &op_facts, &engine.diagnostics),
            ),
            (_, Some(Err(error))) => append_rule_diagnostics(&mut diagnostics, Err(error.clone())),
            _ => {}
        }

        let status = status_from_answer_and_diagnostics(&answer, &diagnostics);
        SemanticLegalityReport {
            schema_version: SEMANTIC_LEGALITY_SCHEMA_VERSION.to_string(),
            operation,
            status,
            answer,
            diagnostics,
        }
    }
}

fn append_rule_diagnostics(
    diagnostics: &mut Vec<SemanticLegalityDiagnostic>,
    result: Result<Vec<RuleDiagnostic>, DatalogError>,
) {
    match result {
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

    fn parity_rulepack(rule_on_legality_predicate: bool) -> RulePack {
        let mut rules = vec![crate::runtime::Rule {
            id: "test.derive_forbidden".to_string(),
            head: Atom {
                predicate: "forbidden_relationship_target".to_string(),
                terms: vec![Term::Var("Kind".to_string())],
            },
            body: vec![Atom {
                predicate: "shared_forbidden".to_string(),
                terms: vec![Term::Var("Kind".to_string())],
            }],
        }];
        if rule_on_legality_predicate {
            rules.push(crate::runtime::Rule {
                id: "test.derive_from_operation".to_string(),
                head: Atom {
                    predicate: "operation_seen".to_string(),
                    terms: vec![Term::Var("Operation".to_string())],
                },
                body: vec![Atom {
                    predicate: "legality_operation".to_string(),
                    terms: vec![Term::Var("Operation".to_string())],
                }],
            });
        }
        let mut diagnostics = vec![DiagnosticRule {
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
        }];
        if rule_on_legality_predicate {
            diagnostics.push(DiagnosticRule {
                id: "test.operation_seen".to_string(),
                severity: RuleDiagnosticSeverity::Warning,
                message: "operation observed by rules".to_string(),
                subjects: vec![Term::Var("Operation".to_string())],
                when: vec![Atom {
                    predicate: "operation_seen".to_string(),
                    terms: vec![Term::Var("Operation".to_string())],
                }],
            });
        }
        RulePack {
            id: "test.parity".to_string(),
            version: "0.1.0".to_string(),
            metadata: BTreeMap::<String, Value>::new(),
            facts: Vec::new(),
            rules,
            diagnostics,
        }
    }

    fn parity_operations() -> Vec<SemanticLegalityOperation> {
        vec![
            SemanticLegalityOperation::Relationship {
                relationship_kind: "satisfy".to_string(),
                source_kind: "part".to_string(),
                target_kind: "part".to_string(),
            },
            SemanticLegalityOperation::Relationship {
                relationship_kind: "satisfy".to_string(),
                source_kind: "part".to_string(),
                target_kind: "requirement".to_string(),
            },
            SemanticLegalityOperation::Containment {
                container_kind: "package".to_string(),
                child_kind: "part".to_string(),
            },
            SemanticLegalityOperation::AttributeWrite {
                kind: "requirement".to_string(),
                attribute: "text".to_string(),
            },
        ]
    }

    #[test]
    fn batch_check_matches_per_check_reports() {
        for rule_on_legality_predicate in [false, true] {
            let service = SemanticLegalityService::with_oracle_and_rulepacks(
                ConservativeSemanticCapabilityOracle,
                vec![parity_rulepack(rule_on_legality_predicate)],
            );
            let shared_facts = vec![Fact::new("shared_forbidden", ["part".to_string()])];
            let batch = service.batch(shared_facts.clone());

            for operation in parity_operations() {
                let batched = batch.check(operation.clone());
                let direct = service.check(SemanticLegalityRequest {
                    operation,
                    facts: shared_facts.clone(),
                });
                assert_eq!(
                    batched, direct,
                    "batch and per-check reports diverged (rule_on_legality_predicate={rule_on_legality_predicate})"
                );
            }
        }
    }

    #[test]
    fn batch_check_reports_shared_fact_driven_diagnostics() {
        let service = SemanticLegalityService::with_oracle_and_rulepacks(
            ConservativeSemanticCapabilityOracle,
            vec![parity_rulepack(false)],
        );
        let batch = service.batch(vec![Fact::new("shared_forbidden", ["part".to_string()])]);

        let report = batch.check(SemanticLegalityOperation::Relationship {
            relationship_kind: "satisfy".to_string(),
            source_kind: "part".to_string(),
            target_kind: "part".to_string(),
        });

        assert_eq!(report.status, SemanticLegalityStatus::Blocked);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "test.satisfy.target_requirement"
                && diagnostic.source == SemanticLegalityDiagnosticSource::Rulepack
                && diagnostic.subjects == ["part"]
        }));
    }
}
