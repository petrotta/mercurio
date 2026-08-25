use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::Graph;

pub const CORE_RULEPACK_ID: &str = "mercurio.core";
pub const CORE_RULEPACK_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RulePack {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Fact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DiagnosticRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub predicate: String,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub head: Atom,
    pub body: Vec<Atom>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRule {
    pub id: String,
    pub severity: RuleDiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<Term>,
    pub when: Vec<Atom>,
}

/// Severity for a datalog rule diagnostic. Alias of the canonical
/// [`crate::kir::Severity`]; retained as a name for rule-facing call sites.
pub use crate::kir::Severity as RuleDiagnosticSeverity;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleDiagnostic {
    pub rule_id: String,
    pub severity: RuleDiagnosticSeverity,
    pub message: String,
    pub subjects: Vec<String>,
    pub source_facts: Vec<Fact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Atom {
    pub predicate: String,
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum Term {
    Var(String),
    Const(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    facts: BTreeSet<Fact>,
    explanations: BTreeMap<Fact, Explanation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Explanation {
    pub rule_id: String,
    pub source_facts: Vec<Fact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedIndexes {
    pub subtypes: BTreeSet<(String, String)>,
    pub ownership: BTreeSet<(String, String)>,
    pub inherited_features: BTreeSet<(String, String)>,
    pub requirements: BTreeSet<String>,
    pub satisfied_by: BTreeMap<String, BTreeSet<String>>,
    pub verified_by: BTreeMap<String, BTreeSet<String>>,
    #[serde(
        default,
        serialize_with = "serialize_explanations",
        deserialize_with = "deserialize_explanations"
    )]
    explanations: BTreeMap<Fact, Explanation>,
}

impl DerivedIndexes {
    pub fn explanations(&self) -> &BTreeMap<Fact, Explanation> {
        &self.explanations
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExplanationEntry {
    fact: Fact,
    explanation: Explanation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatalogError {
    Io(String),
    Json(String),
    EmptyRuleBody(String),
    UnsafeVariable {
        rule_id: String,
        variable: String,
    },
    ArityMismatch {
        predicate: String,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for DatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read rule pack: {err}"),
            Self::Json(err) => write!(f, "failed to parse rule pack: {err}"),
            Self::EmptyRuleBody(rule_id) => write!(f, "rule {rule_id} has an empty body"),
            Self::UnsafeVariable { rule_id, variable } => {
                write!(f, "rule {rule_id} has unsafe head variable {variable}")
            }
            Self::ArityMismatch {
                predicate,
                expected,
                actual,
            } => write!(
                f,
                "predicate {predicate} expected arity {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for DatalogError {}

impl RulePack {
    pub fn from_str(input: &str) -> Result<Self, DatalogError> {
        serde_json::from_str(input).map_err(|err| DatalogError::Json(err.to_string()))
    }

    pub fn from_path(path: &Path) -> Result<Self, DatalogError> {
        let input =
            std::fs::read_to_string(path).map_err(|err| DatalogError::Io(err.to_string()))?;
        Self::from_str(&input)
    }

    pub fn write_pretty_to_path(&self, path: &Path) -> Result<(), DatalogError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| DatalogError::Io(err.to_string()))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|err| DatalogError::Json(err.to_string()))?;
        std::fs::write(path, content).map_err(|err| DatalogError::Io(err.to_string()))
    }

    pub fn structural_core() -> Self {
        Self {
            id: CORE_RULEPACK_ID.to_string(),
            version: CORE_RULEPACK_VERSION.to_string(),
            metadata: BTreeMap::from([(
                "description".to_string(),
                Value::String("Structural Mercurio derived semantic reasoning rules".to_string()),
            )]),
            facts: Vec::new(),
            rules: vec![
                rule(
                    "core.subtype.direct",
                    atom("subtype", [var("A"), var("B")]),
                    [atom("edge", [var("A"), constant("specializes"), var("B")])],
                ),
                rule(
                    "core.subtype.transitive",
                    atom("subtype", [var("A"), var("C")]),
                    [
                        atom("subtype", [var("A"), var("B")]),
                        atom("subtype", [var("B"), var("C")]),
                    ],
                ),
                rule(
                    "core.owns.features",
                    atom("owns", [var("Owner"), var("Child")]),
                    [atom(
                        "edge",
                        [var("Owner"), constant("features"), var("Child")],
                    )],
                ),
                rule(
                    "core.owns.members",
                    atom("owns", [var("Owner"), var("Child")]),
                    [atom(
                        "edge",
                        [var("Owner"), constant("members"), var("Child")],
                    )],
                ),
                rule(
                    "core.owns.owned_element",
                    atom("owns", [var("Owner"), var("Child")]),
                    [atom(
                        "edge",
                        [var("Owner"), constant("owned_element"), var("Child")],
                    )],
                ),
                rule(
                    "core.inherited_feature.direct",
                    atom("inherited_feature", [var("Type"), var("Feature")]),
                    [atom("owns", [var("Type"), var("Feature")])],
                ),
                rule(
                    "core.inherited_feature.specialized",
                    atom("inherited_feature", [var("Type"), var("Feature")]),
                    [
                        atom("subtype", [var("Type"), var("Parent")]),
                        atom("owns", [var("Parent"), var("Feature")]),
                    ],
                ),
            ],
            diagnostics: Vec::new(),
        }
    }

    #[deprecated(
        note = "use structural_core(); language trace semantics belong in language rulepacks"
    )]
    pub fn core() -> Self {
        Self::structural_core()
    }

    #[deprecated(note = "language-specific metamodel adapters belong in language crates")]
    pub fn metamodel_adapter_from_graph(graph: &Graph) -> Self {
        Self {
            id: "mercurio.metamodel.adapter".to_string(),
            version: CORE_RULEPACK_VERSION.to_string(),
            metadata: BTreeMap::from([
                (
                    "description".to_string(),
                    Value::String(
                        "Generated metamodel adapter facts for stable Mercurio predicates"
                            .to_string(),
                    ),
                ),
                ("elementCount".to_string(), json!(graph.elements().len())),
            ]),
            facts: Vec::new(),
            rules: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

pub fn load_default_rulepacks() -> Result<Vec<RulePack>, DatalogError> {
    #[cfg(target_arch = "wasm32")]
    {
        return Ok(vec![RulePack::structural_core()]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = default_stdlib_rulepack_path();
        if !path.exists() {
            return Ok(vec![RulePack::structural_core()]);
        }

        Ok(vec![RulePack::from_path(&path)?])
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn default_stdlib_rulepack_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_MODEL_RULEPACK_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(path) = std::env::var("MERCURIO_STDLIB_RULEPACK_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("foundation")
        .join("core.rulepack.json")
}

impl Fact {
    pub fn new<const N: usize>(predicate: &str, terms: [String; N]) -> Self {
        Self {
            predicate: predicate.to_string(),
            terms: terms.into_iter().collect(),
        }
    }
}

impl Evaluation {
    pub fn facts(&self) -> &BTreeSet<Fact> {
        &self.facts
    }

    pub fn contains(&self, predicate: &str, terms: &[&str]) -> bool {
        self.facts.contains(&Fact {
            predicate: predicate.to_string(),
            terms: terms.iter().map(|term| (*term).to_string()).collect(),
        })
    }

    pub fn explanation(&self, fact: &Fact) -> Option<&Explanation> {
        self.explanations.get(fact)
    }
}

impl DerivedIndexes {
    pub fn from_evaluation(evaluation: Evaluation) -> Self {
        let mut indexes = Self {
            explanations: evaluation.explanations.clone(),
            ..Self::default()
        };

        for fact in evaluation.facts {
            match (fact.predicate.as_str(), fact.terms.as_slice()) {
                ("subtype", [subtype, supertype]) => {
                    indexes
                        .subtypes
                        .insert((subtype.to_string(), supertype.to_string()));
                }
                ("owns", [owner, child]) => {
                    indexes
                        .ownership
                        .insert((owner.to_string(), child.to_string()));
                }
                ("inherited_feature", [owner, feature]) => {
                    indexes
                        .inherited_features
                        .insert((owner.to_string(), feature.to_string()));
                }
                ("requirement", [requirement]) => {
                    indexes.requirements.insert(requirement.to_string());
                }
                ("satisfies", [source, requirement]) => {
                    indexes
                        .satisfied_by
                        .entry(requirement.to_string())
                        .or_default()
                        .insert(source.to_string());
                }
                ("verifies", [source, requirement]) => {
                    indexes
                        .verified_by
                        .entry(requirement.to_string())
                        .or_default()
                        .insert(source.to_string());
                }
                _ => {}
            }
        }

        indexes
    }

    pub fn explanation_for(&self, predicate: &str, terms: &[&str]) -> Option<&Explanation> {
        let fact = Fact {
            predicate: predicate.to_string(),
            terms: terms.iter().map(|term| (*term).to_string()).collect(),
        };
        self.explanations.get(&fact)
    }
}

fn serialize_explanations<S>(
    explanations: &BTreeMap<Fact, Explanation>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    explanations
        .iter()
        .map(|(fact, explanation)| ExplanationEntry {
            fact: fact.clone(),
            explanation: explanation.clone(),
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

fn deserialize_explanations<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<Fact, Explanation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let entries = Vec::<ExplanationEntry>::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .map(|entry| (entry.fact, entry.explanation))
        .collect())
}

pub fn extract_graph_facts(graph: &Graph) -> Vec<Fact> {
    let mut facts = BTreeSet::new();

    for element in graph.elements() {
        facts.insert(Fact::new("element", [element.element_id.clone()]));
        facts.insert(Fact::new(
            "kind",
            [element.element_id.clone(), element.kind.to_string()],
        ));
        facts.insert(Fact::new(
            "layer",
            [element.element_id.clone(), element.layer.to_string()],
        ));

        if let Some(metadata) = element.properties.get("metadata") {
            if let Some(file) = metadata.get("source_file").and_then(Value::as_str) {
                facts.insert(Fact::new(
                    "source_file",
                    [element.element_id.clone(), file.to_string()],
                ));
            }
            if let Some(span) = metadata.get("source_span") {
                let values = [
                    span.get("start_line").and_then(Value::as_u64),
                    span.get("start_col").and_then(Value::as_u64),
                    span.get("end_line").and_then(Value::as_u64),
                    span.get("end_col").and_then(Value::as_u64),
                ];
                if let [
                    Some(start_line),
                    Some(start_col),
                    Some(end_line),
                    Some(end_col),
                ] = values
                {
                    facts.insert(Fact::new(
                        "source_span",
                        [
                            element.element_id.clone(),
                            start_line.to_string(),
                            start_col.to_string(),
                            end_line.to_string(),
                            end_col.to_string(),
                        ],
                    ));
                }
            }
        }
    }

    for edge in graph.edges() {
        let Some(source) = graph.element_id(edge.source) else {
            continue;
        };
        let Some(target) = graph.element_id(edge.target) else {
            continue;
        };
        facts.insert(Fact::new(
            "edge",
            [
                source.to_string(),
                edge.relation.to_string(),
                target.to_string(),
            ],
        ));
    }

    facts.into_iter().collect()
}

pub fn materialize_core_indexes(
    graph: &Graph,
    rulepacks: &[RulePack],
) -> Result<DerivedIndexes, DatalogError> {
    if rulepacks.iter().all(|pack| pack.rules.is_empty()) {
        return Ok(materialize_builtin_indexes(graph, rulepacks));
    }

    let mut facts = extract_graph_facts(graph);
    let mut packs = vec![RulePack::structural_core()];
    packs.extend(rulepacks.iter().cloned());
    for pack in &packs {
        facts.extend(pack.facts.iter().cloned());
    }
    let rules = packs
        .iter()
        .flat_map(|pack| pack.rules.iter().cloned())
        .collect::<Vec<_>>();

    evaluate(facts, &rules).map(DerivedIndexes::from_evaluation)
}

fn materialize_builtin_indexes(graph: &Graph, rulepacks: &[RulePack]) -> DerivedIndexes {
    let mut indexes = DerivedIndexes::default();
    let mut requirement_kinds = BTreeSet::new();
    let mut relationship_kinds = BTreeMap::<String, BTreeSet<String>>::new();

    for fact in rulepacks
        .iter()
        .flat_map(|pack| pack.facts.iter())
        .chain(extract_graph_facts(graph).iter())
    {
        match (fact.predicate.as_str(), fact.terms.as_slice()) {
            ("requirement_kind", [kind]) => {
                requirement_kinds.insert(kind.to_string());
            }
            ("relationship_kind", [kind, relation]) => {
                relationship_kinds
                    .entry(relation.to_string())
                    .or_default()
                    .insert(kind.to_string());
            }
            _ => {}
        }
    }

    for element in graph.elements() {
        for relation in ["features", "members", "owned_element"] {
            for edge in graph.outgoing(element.id, relation) {
                if let Some(child) = graph.element_id(edge.target) {
                    indexes
                        .ownership
                        .insert((element.element_id.clone(), child.to_string()));
                    indexes.explanations.insert(
                        Fact::new("owns", [element.element_id.clone(), child.to_string()]),
                        Explanation {
                            rule_id: format!("core.owns.{relation}"),
                            source_facts: vec![Fact::new(
                                "edge",
                                [
                                    element.element_id.clone(),
                                    relation.to_string(),
                                    child.to_string(),
                                ],
                            )],
                        },
                    );
                }
            }
        }
    }

    let mut ownership_by_owner = BTreeMap::<String, Vec<String>>::new();
    for (owner, child) in &indexes.ownership {
        ownership_by_owner
            .entry(owner.clone())
            .or_default()
            .push(child.clone());
    }

    for element in graph.elements() {
        for edge in graph.outgoing(element.id, "specializes") {
            let current = edge.target;
            let Some(supertype) = graph.element_id(current) else {
                continue;
            };
            indexes
                .subtypes
                .insert((element.element_id.clone(), supertype.to_string()));
            indexes
                .explanations
                .entry(Fact::new(
                    "subtype",
                    [element.element_id.clone(), supertype.to_string()],
                ))
                .or_insert_with(|| Explanation {
                    rule_id: "core.subtype.direct".to_string(),
                    source_facts: vec![Fact::new(
                        "edge",
                        [
                            element.element_id.clone(),
                            "specializes".to_string(),
                            supertype.to_string(),
                        ],
                    )],
                });
        }
    }

    for (owner, children) in &ownership_by_owner {
        for child in children {
            indexes
                .inherited_features
                .insert((owner.clone(), child.clone()));
            indexes
                .explanations
                .entry(Fact::new(
                    "inherited_feature",
                    [owner.clone(), child.clone()],
                ))
                .or_insert_with(|| Explanation {
                    rule_id: "core.inherited_feature.direct".to_string(),
                    source_facts: vec![Fact::new("owns", [owner.clone(), child.clone()])],
                });
        }
    }

    for (subtype, supertype) in &indexes.subtypes {
        if let Some(children) = ownership_by_owner.get(supertype) {
            for child in children {
                indexes
                    .inherited_features
                    .insert((subtype.clone(), child.clone()));
                indexes
                    .explanations
                    .entry(Fact::new(
                        "inherited_feature",
                        [subtype.clone(), child.clone()],
                    ))
                    .or_insert_with(|| Explanation {
                        rule_id: "core.inherited_feature.specialized".to_string(),
                        source_facts: vec![
                            Fact::new("subtype", [subtype.clone(), supertype.clone()]),
                            Fact::new("owns", [supertype.clone(), child.clone()]),
                        ],
                    });
            }
        }
    }

    for element in graph.elements() {
        if requirement_kinds.contains(element.kind.as_ref()) {
            indexes.requirements.insert(element.element_id.clone());
            indexes.explanations.insert(
                Fact::new("requirement", [element.element_id.clone()]),
                Explanation {
                    rule_id: "core.requirement.kind".to_string(),
                    source_facts: vec![Fact::new(
                        "kind",
                        [element.element_id.clone(), element.kind.to_string()],
                    )],
                },
            );
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (element, parent) in &indexes.subtypes {
            if indexes.requirements.contains(parent) && indexes.requirements.insert(element.clone())
            {
                indexes.explanations.insert(
                    Fact::new("requirement", [element.clone()]),
                    Explanation {
                        rule_id: "core.requirement.specialization".to_string(),
                        source_facts: vec![Fact::new("subtype", [element.clone(), parent.clone()])],
                    },
                );
                changed = true;
            }
        }
    }

    for edge in graph.edges() {
        let Some(source) = graph.element_id(edge.source) else {
            continue;
        };
        let Some(target) = graph.element_id(edge.target) else {
            continue;
        };
        match edge.relation.as_ref() {
            "satisfy" | "satisfies" => {
                indexes
                    .satisfied_by
                    .entry(target.to_string())
                    .or_default()
                    .insert(source.to_string());
            }
            "verify" | "verifies" => {
                indexes
                    .verified_by
                    .entry(target.to_string())
                    .or_default()
                    .insert(source.to_string());
            }
            _ => {}
        }
    }

    for relationship in graph.elements() {
        let relation = if relationship_kinds
            .get("satisfy")
            .is_some_and(|kinds| kinds.contains(relationship.kind.as_ref()))
        {
            Some("satisfy")
        } else if relationship_kinds
            .get("verify")
            .is_some_and(|kinds| kinds.contains(relationship.kind.as_ref()))
        {
            Some("verify")
        } else {
            None
        };
        let Some(relation) = relation else {
            continue;
        };
        let source = graph
            .outgoing(relationship.id, "source")
            .filter_map(|edge| graph.element_id(edge.target))
            .next();
        let target = graph
            .outgoing(relationship.id, "target")
            .filter_map(|edge| graph.element_id(edge.target))
            .next();
        let (Some(source), Some(target)) = (source, target) else {
            continue;
        };
        match relation {
            "satisfy" => {
                indexes
                    .satisfied_by
                    .entry(target.to_string())
                    .or_default()
                    .insert(source.to_string());
            }
            "verify" => {
                indexes
                    .verified_by
                    .entry(target.to_string())
                    .or_default()
                    .insert(source.to_string());
            }
            _ => {}
        }
    }

    indexes
}

pub fn evaluate<I>(facts: I, rules: &[Rule]) -> Result<Evaluation, DatalogError>
where
    I: IntoIterator<Item = Fact>,
{
    validate_rules(rules)?;
    let mut known = facts.into_iter().collect::<BTreeSet<_>>();
    let mut explanations = BTreeMap::new();
    let mut changed = true;

    while changed {
        changed = false;
        for rule in rules {
            for (derived, source_facts) in derive_rule(rule, &known)? {
                if known.insert(derived.clone()) {
                    explanations.insert(
                        derived,
                        Explanation {
                            rule_id: rule.id.clone(),
                            source_facts,
                        },
                    );
                    changed = true;
                }
            }
        }
    }

    Ok(Evaluation {
        facts: known,
        explanations,
    })
}

pub fn evaluate_diagnostics(
    evaluation: &Evaluation,
    diagnostics: &[DiagnosticRule],
) -> Result<Vec<RuleDiagnostic>, DatalogError> {
    diagnostics
        .iter()
        .map(|diagnostic| evaluate_diagnostic_rule(evaluation, diagnostic))
        .collect::<Result<Vec<_>, _>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn evaluate_diagnostic_rule(
    evaluation: &Evaluation,
    diagnostic: &DiagnosticRule,
) -> Result<Vec<RuleDiagnostic>, DatalogError> {
    let rule = Rule {
        id: diagnostic.id.clone(),
        head: Atom {
            predicate: format!("__diagnostic_{}", diagnostic.id),
            terms: diagnostic.subjects.clone(),
        },
        body: diagnostic.when.clone(),
    };
    validate_rules(std::slice::from_ref(&rule))?;
    derive_rule(&rule, evaluation.facts()).map(|matches| {
        matches
            .into_iter()
            .map(|(fact, source_facts)| RuleDiagnostic {
                rule_id: diagnostic.id.clone(),
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                subjects: fact.terms,
                source_facts,
            })
            .collect()
    })
}

fn validate_rules(rules: &[Rule]) -> Result<(), DatalogError> {
    for rule in rules {
        if rule.body.is_empty() {
            return Err(DatalogError::EmptyRuleBody(rule.id.clone()));
        }
        let body_vars = rule
            .body
            .iter()
            .flat_map(|atom| atom.terms.iter())
            .filter_map(|term| match term {
                Term::Var(name) => Some(name.as_str()),
                Term::Const(_) => None,
            })
            .collect::<BTreeSet<_>>();
        for term in &rule.head.terms {
            if let Term::Var(name) = term
                && !body_vars.contains(name.as_str())
            {
                return Err(DatalogError::UnsafeVariable {
                    rule_id: rule.id.clone(),
                    variable: name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn derive_rule(
    rule: &Rule,
    known: &BTreeSet<Fact>,
) -> Result<Vec<(Fact, Vec<Fact>)>, DatalogError> {
    let mut bindings = vec![(HashMap::<String, String>::new(), Vec::<Fact>::new())];

    for atom in &rule.body {
        let candidates = known
            .iter()
            .filter(|fact| fact.predicate == atom.predicate)
            .collect::<Vec<_>>();
        let mut next = Vec::new();

        for (binding, source_facts) in bindings {
            for fact in &candidates {
                if fact.terms.len() != atom.terms.len() {
                    return Err(DatalogError::ArityMismatch {
                        predicate: atom.predicate.clone(),
                        expected: atom.terms.len(),
                        actual: fact.terms.len(),
                    });
                }
                if let Some(next_binding) = unify(atom, fact, &binding) {
                    let mut next_sources = source_facts.clone();
                    if !next_sources.iter().any(|existing| existing == *fact) {
                        next_sources.push((*fact).clone());
                    }
                    next.push((next_binding, next_sources));
                }
            }
        }

        bindings = next;
        if bindings.is_empty() {
            break;
        }
    }

    bindings
        .into_iter()
        .map(|(binding, source_facts)| {
            instantiate(&rule.head, &binding).map(|fact| (fact, source_facts))
        })
        .collect()
}

fn unify(
    atom: &Atom,
    fact: &Fact,
    binding: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
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

fn instantiate(atom: &Atom, binding: &HashMap<String, String>) -> Result<Fact, DatalogError> {
    let terms = atom
        .terms
        .iter()
        .map(|term| match term {
            Term::Const(value) => Ok(value.clone()),
            Term::Var(name) => {
                binding
                    .get(name)
                    .cloned()
                    .ok_or_else(|| DatalogError::UnsafeVariable {
                        rule_id: atom.predicate.clone(),
                        variable: name.clone(),
                    })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Fact {
        predicate: atom.predicate.clone(),
        terms,
    })
}

fn rule<const N: usize>(id: &str, head: Atom, body: [Atom; N]) -> Rule {
    Rule {
        id: id.to_string(),
        head,
        body: body.into_iter().collect(),
    }
}

fn atom<const N: usize>(predicate: &str, terms: [Term; N]) -> Atom {
    Atom {
        predicate: predicate.to_string(),
        terms: terms.into_iter().collect(),
    }
}

fn var(name: &str) -> Term {
    Term::Var(name.to_string())
}

fn constant(value: &str) -> Term {
    Term::Const(value.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        Atom, DiagnosticRule, Fact, RuleDiagnosticSeverity, RulePack, Term, evaluate,
        evaluate_diagnostics, extract_graph_facts, load_default_rulepacks,
        materialize_core_indexes,
    };
    use crate::model::Graph;
    use crate::model::{KirDocument, KirElement};

    #[test]
    fn extracts_base_graph_facts() {
        let graph = Graph::from_document(KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [("features".to_string(), json!(["feature.engine"]))]
                        .into_iter()
                        .collect(),
                },
                KirElement {
                    id: "feature.engine".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: Default::default(),
                },
            ],
        })
        .unwrap();

        let facts = extract_graph_facts(&graph);

        assert!(facts.contains(&Fact::new("element", ["type.Vehicle".to_string()])));
        assert!(facts.contains(&Fact::new(
            "edge",
            [
                "type.Vehicle".to_string(),
                "features".to_string(),
                "feature.engine".to_string()
            ]
        )));
    }

    #[test]
    fn evaluates_diagnostic_rules_with_subjects_and_source_facts() {
        let evaluation = evaluate(
            vec![Fact::new(
                "legality_relationship_request",
                [
                    "satisfy".to_string(),
                    "part".to_string(),
                    "part".to_string(),
                ],
            )],
            &[],
        )
        .unwrap();

        let diagnostics = evaluate_diagnostics(
            &evaluation,
            &[DiagnosticRule {
                id: "sysml.satisfy.target_requirement".to_string(),
                severity: RuleDiagnosticSeverity::Error,
                message: "satisfy must target a requirement-like element".to_string(),
                subjects: vec![Term::Var("Target".to_string())],
                when: vec![Atom {
                    predicate: "legality_relationship_request".to_string(),
                    terms: vec![
                        Term::Const("satisfy".to_string()),
                        Term::Var("Source".to_string()),
                        Term::Var("Target".to_string()),
                    ],
                }],
            }],
        )
        .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].subjects, ["part"]);
        assert_eq!(diagnostics[0].source_facts.len(), 1);
    }

    #[test]
    fn materializes_core_closures_and_traceability() {
        let graph = Graph::from_document(KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Parent".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [("features".to_string(), json!(["feature.engine"]))]
                        .into_iter()
                        .collect(),
                },
                KirElement {
                    id: "type.Child".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [("specializes".to_string(), json!(["type.Parent"]))]
                        .into_iter()
                        .collect(),
                },
                KirElement {
                    id: "feature.engine".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: Default::default(),
                },
                KirElement {
                    id: "req.Braking".to_string(),
                    kind: "Model::Requirements::RequirementUsage".to_string(),
                    layer: 2,
                    properties: Default::default(),
                },
                KirElement {
                    id: "case.BrakeTest".to_string(),
                    kind: "Model::Verification::VerificationCaseUsage".to_string(),
                    layer: 2,
                    properties: [("verify".to_string(), json!("req.Braking"))]
                        .into_iter()
                        .collect(),
                },
            ],
        })
        .unwrap();

        let indexes = materialize_core_indexes(&graph, &[]).unwrap();

        assert!(
            indexes
                .subtypes
                .contains(&("type.Child".to_string(), "type.Parent".to_string()))
        );
        assert!(
            indexes
                .inherited_features
                .contains(&("type.Child".to_string(), "feature.engine".to_string()))
        );
        assert!(indexes.requirements.is_empty());
        assert!(indexes.verified_by.is_empty());
    }

    #[test]
    #[allow(deprecated)]
    fn deprecated_generated_adapter_rulepack_is_neutral() {
        let graph = Graph::from_document(KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "req.Braking".to_string(),
                kind: "Model::Requirements::RequirementUsage".to_string(),
                layer: 2,
                properties: Default::default(),
            }],
        })
        .unwrap();

        let rulepack = RulePack::metamodel_adapter_from_graph(&graph);

        assert!(rulepack.facts.is_empty());
    }

    #[test]
    fn loads_default_stdlib_rulepack() {
        let rulepacks = load_default_rulepacks().unwrap();

        assert_eq!(rulepacks.len(), 1);
        assert_eq!(rulepacks[0].id, "mercurio.core");
        assert!(rulepacks[0].facts.is_empty());
    }
}
