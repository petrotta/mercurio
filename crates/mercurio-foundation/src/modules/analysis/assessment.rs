use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::Graph;
use crate::runtime::{
    Atom, DatalogError, Evaluation, Fact, RulePack, Term, evaluate, extract_graph_facts,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentSpec {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<AssessmentAssertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentAssertion {
    pub id: String,
    pub description: String,
    pub query: AssessmentQuery,
    pub expect: AssessmentExpectation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentQuery {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub find: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub where_atoms: Vec<Atom>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AssessmentExpectation {
    Exists,
    CountEq { value: usize },
    CountAtLeast { value: usize },
    ContainsBinding { variable: String, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentReport {
    pub id: String,
    pub title: String,
    pub status: AssessmentStatus,
    pub assertions: Vec<AssessmentAssertionReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssessmentRequest {
    pub spec: AssessmentSpec,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rulepacks: Vec<RulePack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Fact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssessmentResult {
    pub report: AssessmentReport,
    pub facts: Vec<Fact>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssessmentStatus {
    Pass,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentAssertionReport {
    pub id: String,
    pub description: String,
    pub status: AssessmentStatus,
    pub binding_count: usize,
    pub bindings: Vec<BTreeMap<String, String>>,
    pub message: String,
}

#[derive(Debug)]
pub enum AssessmentError {
    Datalog(DatalogError),
}

impl fmt::Display for AssessmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Datalog(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AssessmentError {}

impl From<DatalogError> for AssessmentError {
    fn from(value: DatalogError) -> Self {
        Self::Datalog(value)
    }
}

pub fn run_graph_assessment(
    graph: &Graph,
    rulepacks: &[RulePack],
    spec: &AssessmentSpec,
) -> Result<AssessmentReport, AssessmentError> {
    let mut facts = extract_graph_facts(graph);
    for pack in rulepacks {
        facts.extend(pack.facts.iter().cloned());
    }
    let rules = rulepacks
        .iter()
        .flat_map(|pack| pack.rules.iter().cloned())
        .collect::<Vec<_>>();
    let evaluation = evaluate(facts, &rules)?;
    run_evaluation_assessment(&evaluation, spec)
}

pub fn run_runtime_assessment(
    request: RuntimeAssessmentRequest,
) -> Result<RuntimeAssessmentResult, AssessmentError> {
    let mut facts = request.facts;
    for pack in &request.rulepacks {
        facts.extend(pack.facts.iter().cloned());
    }
    let rules = request
        .rulepacks
        .iter()
        .flat_map(|pack| pack.rules.iter().cloned())
        .collect::<Vec<_>>();
    let evaluation = evaluate(facts, &rules)?;
    let report = run_evaluation_assessment(&evaluation, &request.spec)?;
    Ok(RuntimeAssessmentResult {
        report,
        facts: evaluation.facts().iter().cloned().collect(),
    })
}

pub fn run_evaluation_assessment(
    evaluation: &Evaluation,
    spec: &AssessmentSpec,
) -> Result<AssessmentReport, AssessmentError> {
    let assertions = spec
        .assertions
        .iter()
        .map(|assertion| {
            let bindings = query_evaluation(evaluation, &assertion.query)?;
            Ok(report_assertion(assertion, bindings))
        })
        .collect::<Result<Vec<_>, AssessmentError>>()?;
    let status = if assertions
        .iter()
        .all(|assertion| assertion.status == AssessmentStatus::Pass)
    {
        AssessmentStatus::Pass
    } else {
        AssessmentStatus::Failed
    };

    Ok(AssessmentReport {
        id: spec.id.clone(),
        title: spec.title.clone(),
        status,
        assertions,
    })
}

pub fn query_evaluation(
    evaluation: &Evaluation,
    query: &AssessmentQuery,
) -> Result<Vec<BTreeMap<String, String>>, AssessmentError> {
    let mut bindings = vec![BTreeMap::<String, String>::new()];

    for atom in &query.where_atoms {
        let candidates = evaluation
            .facts()
            .iter()
            .filter(|fact| fact.predicate == atom.predicate)
            .collect::<Vec<_>>();
        let mut next = Vec::new();

        for binding in bindings {
            for fact in &candidates {
                if fact.terms.len() != atom.terms.len() {
                    return Err(DatalogError::ArityMismatch {
                        predicate: atom.predicate.clone(),
                        expected: atom.terms.len(),
                        actual: fact.terms.len(),
                    }
                    .into());
                }
                if let Some(next_binding) = unify_query_atom(atom, fact, &binding) {
                    next.push(project_binding(next_binding, &query.find));
                }
            }
        }

        bindings = dedupe_bindings(next);
        if bindings.is_empty() {
            break;
        }
    }

    Ok(dedupe_bindings(bindings))
}

fn report_assertion(
    assertion: &AssessmentAssertion,
    bindings: Vec<BTreeMap<String, String>>,
) -> AssessmentAssertionReport {
    let binding_count = bindings.len();
    let passed = match &assertion.expect {
        AssessmentExpectation::Exists => binding_count > 0,
        AssessmentExpectation::CountEq { value } => binding_count == *value,
        AssessmentExpectation::CountAtLeast { value } => binding_count >= *value,
        AssessmentExpectation::ContainsBinding { variable, value } => bindings
            .iter()
            .any(|binding| binding.get(variable).is_some_and(|actual| actual == value)),
    };

    AssessmentAssertionReport {
        id: assertion.id.clone(),
        description: assertion.description.clone(),
        status: if passed {
            AssessmentStatus::Pass
        } else {
            AssessmentStatus::Failed
        },
        binding_count,
        bindings,
        message: assertion_message(&assertion.expect, binding_count, passed),
    }
}

fn assertion_message(
    expectation: &AssessmentExpectation,
    binding_count: usize,
    passed: bool,
) -> String {
    let prefix = if passed { "pass" } else { "failed" };
    match expectation {
        AssessmentExpectation::Exists => {
            format!("{prefix}: expected at least one binding; found {binding_count}")
        }
        AssessmentExpectation::CountEq { value } => {
            format!("{prefix}: expected {value} binding(s); found {binding_count}")
        }
        AssessmentExpectation::CountAtLeast { value } => {
            format!("{prefix}: expected at least {value} binding(s); found {binding_count}")
        }
        AssessmentExpectation::ContainsBinding { variable, value } => {
            format!(
                "{prefix}: expected binding {variable}={value}; found {binding_count} binding(s)"
            )
        }
    }
}

fn unify_query_atom(
    atom: &Atom,
    fact: &Fact,
    binding: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, String>> {
    let mut next = binding.clone();
    for (term, value) in atom.terms.iter().zip(&fact.terms) {
        match term {
            Term::Const(expected) if expected != value => return None,
            Term::Const(_) => {}
            Term::Var(name) => {
                if let Some(existing) = next.get(name) {
                    if existing != value {
                        return None;
                    }
                } else {
                    next.insert(name.clone(), value.clone());
                }
            }
        }
    }
    Some(next)
}

fn project_binding(binding: BTreeMap<String, String>, find: &[String]) -> BTreeMap<String, String> {
    if find.is_empty() {
        return binding;
    }
    find.iter()
        .filter_map(|variable| {
            binding
                .get(variable)
                .map(|value| (variable.clone(), value.clone()))
        })
        .collect()
}

fn dedupe_bindings(bindings: Vec<BTreeMap<String, String>>) -> Vec<BTreeMap<String, String>> {
    let mut deduped = Vec::new();
    for binding in bindings {
        if !deduped.iter().any(|existing| existing == &binding) {
            deduped.push(binding);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_runtime_assessment_from_supplied_facts_rules_and_queries() {
        let request = RuntimeAssessmentRequest {
            facts: vec![Fact::new("package", ["Demo".to_string()])],
            rulepacks: vec![RulePack {
                id: "test.derived".to_string(),
                version: "0.1.0".to_string(),
                metadata: BTreeMap::new(),
                facts: vec![],
                rules: vec![crate::runtime::Rule {
                    id: "top-level-package-from-package".to_string(),
                    head: Atom {
                        predicate: "top_level_package".to_string(),
                        terms: vec![Term::Var("P".to_string())],
                    },
                    body: vec![Atom {
                        predicate: "package".to_string(),
                        terms: vec![Term::Var("P".to_string())],
                    }],
                }],
                diagnostics: vec![],
            }],
            spec: AssessmentSpec {
                id: "runtime.package.demo".to_string(),
                title: "Runtime package check".to_string(),
                assertions: vec![AssessmentAssertion {
                    id: "has-demo-package".to_string(),
                    description: "A derived top-level package exists".to_string(),
                    query: AssessmentQuery {
                        find: vec!["P".to_string()],
                        where_atoms: vec![Atom {
                            predicate: "top_level_package".to_string(),
                            terms: vec![Term::Var("P".to_string())],
                        }],
                    },
                    expect: AssessmentExpectation::ContainsBinding {
                        variable: "P".to_string(),
                        value: "Demo".to_string(),
                    },
                }],
            },
        };

        let result = run_runtime_assessment(request).unwrap();

        assert_eq!(result.report.status, AssessmentStatus::Pass);
        assert!(
            result
                .facts
                .iter()
                .any(|fact| fact.predicate == "top_level_package" && fact.terms == ["Demo"])
        );
    }
}
