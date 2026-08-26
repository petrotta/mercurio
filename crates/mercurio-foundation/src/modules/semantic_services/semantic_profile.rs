use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status", content = "message")]
pub enum CapabilityAnswer {
    Allowed,
    Denied(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributePolicyAnswer {
    pub writable: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticCapabilityProfile {
    pub containment: BTreeSet<CapabilityPair>,
    pub specializations: BTreeSet<CapabilityPair>,
    pub usage_typings: BTreeSet<CapabilityPair>,
    pub relationships: BTreeSet<RelationshipCapability>,
    pub attribute_policies: BTreeMap<AttributePolicyKey, AttributePolicyAnswer>,
    pub relationship_owner_sources: BTreeSet<String>,
    pub doc_id_attribute_aliases: Vec<&'static str>,
    pub supporting_definition_keywords: BTreeMap<String, String>,
    pub definition_keyword_aliases: BTreeMap<String, String>,
    pub element_kind_authoring: BTreeMap<String, SemanticElementAuthoring>,
    pub definition_keyword_element_kinds: BTreeMap<String, String>,
    pub usage_keyword_element_kinds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityPair {
    pub source_kind: String,
    pub target_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationshipCapability {
    pub relationship_kind: String,
    pub source_kind: String,
    pub target_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttributePolicyKey {
    pub kind: String,
    pub attribute: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticElementForm {
    Definition,
    Usage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticElementAuthoring {
    pub form: SemanticElementForm,
    pub keyword: String,
}

impl SemanticCapabilityProfile {
    pub fn allow_containment(mut self, container_kind: &str, child_kind: &str) -> Self {
        self.containment
            .insert(CapabilityPair::new(container_kind, child_kind));
        self
    }

    pub fn allow_specialization(mut self, source_kind: &str, target_kind: &str) -> Self {
        self.specializations
            .insert(CapabilityPair::new(source_kind, target_kind));
        self
    }

    pub fn allow_usage_typing(mut self, usage_kind: &str, definition_kind: &str) -> Self {
        self.usage_typings
            .insert(CapabilityPair::new(usage_kind, definition_kind));
        self
    }

    pub fn allow_relationship(
        mut self,
        relationship_kind: &str,
        source_kind: &str,
        target_kind: &str,
    ) -> Self {
        self.relationships.insert(RelationshipCapability::new(
            relationship_kind,
            source_kind,
            target_kind,
        ));
        self
    }

    pub fn attribute_policy(
        mut self,
        kind: &str,
        attribute: &str,
        answer: AttributePolicyAnswer,
    ) -> Self {
        self.attribute_policies
            .insert(AttributePolicyKey::new(kind, attribute), answer);
        self
    }

    pub fn relationship_uses_owner_as_source(mut self, relationship_kind: &str) -> Self {
        self.relationship_owner_sources
            .insert(normalize_capability_token(relationship_kind));
        self
    }

    pub fn supporting_definition_keyword(
        mut self,
        usage_kind: &str,
        definition_keyword: &str,
    ) -> Self {
        self.supporting_definition_keywords.insert(
            normalize_capability_token(usage_kind),
            normalize_capability_token(definition_keyword),
        );
        self
    }

    pub fn definition_keyword_alias(mut self, alias: &str, normalized: &str) -> Self {
        self.definition_keyword_aliases.insert(
            normalize_capability_token(alias),
            normalize_capability_token(normalized),
        );
        self
    }

    pub fn element_kind_authoring(
        mut self,
        kind: &str,
        form: SemanticElementForm,
        keyword: &str,
    ) -> Self {
        self.element_kind_authoring.insert(
            normalize_capability_token(kind),
            SemanticElementAuthoring {
                form,
                keyword: normalize_capability_token(keyword),
            },
        );
        self
    }

    pub fn definition_keyword_element_kind(mut self, keyword: &str, kind: &str) -> Self {
        self.definition_keyword_element_kinds
            .insert(normalize_capability_token(keyword), kind.trim().to_string());
        self
    }

    pub fn usage_keyword_element_kind(mut self, keyword: &str, kind: &str) -> Self {
        self.usage_keyword_element_kinds
            .insert(normalize_capability_token(keyword), kind.trim().to_string());
        self
    }
}

impl CapabilityPair {
    pub fn new(source_kind: &str, target_kind: &str) -> Self {
        Self {
            source_kind: normalize_capability_token(source_kind),
            target_kind: normalize_capability_token(target_kind),
        }
    }
}

impl RelationshipCapability {
    pub fn new(relationship_kind: &str, source_kind: &str, target_kind: &str) -> Self {
        Self {
            relationship_kind: normalize_capability_token(relationship_kind),
            source_kind: normalize_capability_token(source_kind),
            target_kind: normalize_capability_token(target_kind),
        }
    }
}

impl AttributePolicyKey {
    pub fn new(kind: &str, attribute: &str) -> Self {
        Self {
            kind: normalize_capability_token(kind),
            attribute: normalize_capability_token(attribute),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TableSemanticCapabilityOracle {
    profile: SemanticCapabilityProfile,
}

impl TableSemanticCapabilityOracle {
    pub fn new(profile: SemanticCapabilityProfile) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> &SemanticCapabilityProfile {
        &self.profile
    }

    fn pair_answer(
        &self,
        table: &BTreeSet<CapabilityPair>,
        source_kind: &str,
        target_kind: &str,
        subject: &str,
    ) -> CapabilityAnswer {
        let source = normalize_capability_token(source_kind);
        let target = normalize_capability_token(target_kind);
        if source.is_empty() || target.is_empty() {
            return CapabilityAnswer::Unknown("missing kind information".to_string());
        }
        if table.contains(&CapabilityPair {
            source_kind: source.clone(),
            target_kind: target.clone(),
        }) || table.contains(&CapabilityPair {
            source_kind: source.clone(),
            target_kind: "*".to_string(),
        }) || table.contains(&CapabilityPair {
            source_kind: "*".to_string(),
            target_kind: target.clone(),
        }) || table.contains(&CapabilityPair {
            source_kind: "*".to_string(),
            target_kind: "*".to_string(),
        }) {
            CapabilityAnswer::Allowed
        } else {
            CapabilityAnswer::Denied(format!(
                "{subject} `{source_kind}` -> `{target_kind}` is not allowed by the semantic capability profile"
            ))
        }
    }
}

impl SemanticCapabilityOracle for TableSemanticCapabilityOracle {
    fn can_contain(&self, container_kind: &str, child_kind: &str) -> CapabilityAnswer {
        self.pair_answer(
            &self.profile.containment,
            container_kind,
            child_kind,
            "containment",
        )
    }

    fn can_specialize(&self, source_kind: &str, target_kind: &str) -> CapabilityAnswer {
        self.pair_answer(
            &self.profile.specializations,
            source_kind,
            target_kind,
            "specialization",
        )
    }

    fn can_type_usage(&self, usage_kind: &str, definition_kind: &str) -> CapabilityAnswer {
        self.pair_answer(
            &self.profile.usage_typings,
            usage_kind,
            definition_kind,
            "typing",
        )
    }

    fn can_relate(
        &self,
        relationship_kind: &str,
        source_kind: &str,
        target_kind: &str,
    ) -> CapabilityAnswer {
        let relation = normalize_capability_token(relationship_kind);
        let source = normalize_capability_token(source_kind);
        let target = normalize_capability_token(target_kind);
        if relation.is_empty() || source.is_empty() || target.is_empty() {
            return CapabilityAnswer::Unknown(
                "missing relationship capability information".to_string(),
            );
        }
        let candidates = [
            RelationshipCapability::new(&relation, &source, &target),
            RelationshipCapability::new(&relation, &source, "*"),
            RelationshipCapability::new(&relation, "*", &target),
            RelationshipCapability::new(&relation, "*", "*"),
        ];
        if candidates
            .iter()
            .any(|candidate| self.profile.relationships.contains(candidate))
        {
            CapabilityAnswer::Allowed
        } else {
            CapabilityAnswer::Denied(format!(
                "relationship `{relationship_kind}` from `{source_kind}` to `{target_kind}` is not allowed by the semantic capability profile"
            ))
        }
    }

    fn attribute_policy(&self, kind: &str, attribute: &str) -> AttributePolicyAnswer {
        let kind = normalize_capability_token(kind);
        let attribute = normalize_capability_token(attribute);
        self.profile
            .attribute_policies
            .get(&AttributePolicyKey {
                kind: kind.clone(),
                attribute: attribute.clone(),
            })
            .or_else(|| {
                self.profile.attribute_policies.get(&AttributePolicyKey {
                    kind: "*".to_string(),
                    attribute: attribute.clone(),
                })
            })
            .cloned()
            .unwrap_or_else(|| AttributePolicyAnswer {
                writable: false,
                reason: Some(format!(
                    "attribute `{attribute}` is not writable on `{kind}` by the semantic capability profile"
                )),
            })
    }

    fn relationship_uses_owner_as_source(&self, relationship_kind: &str) -> bool {
        self.profile
            .relationship_owner_sources
            .contains(&normalize_capability_token(relationship_kind))
    }

    fn doc_id_attribute_aliases(&self) -> &'static [&'static str] {
        if self
            .profile
            .doc_id_attribute_aliases
            .contains(&"requirement_id")
        {
            &["id", "requirement_id"]
        } else {
            &["id"]
        }
    }

    fn supporting_definition_keyword_for_usage(&self, usage_kind: &str) -> Option<String> {
        self.profile
            .supporting_definition_keywords
            .get(&normalize_capability_token(usage_kind))
            .cloned()
    }

    fn normalize_definition_keyword(&self, keyword: &str) -> String {
        let normalized = normalize_capability_token(keyword);
        self.profile
            .definition_keyword_aliases
            .get(&normalized)
            .cloned()
            .unwrap_or(normalized)
    }

    fn authoring_for_element_kind(&self, kind: &str) -> Option<SemanticElementAuthoring> {
        self.profile
            .element_kind_authoring
            .get(&normalize_capability_token(kind))
            .cloned()
    }

    fn semantic_kind_for_definition_keyword(&self, keyword: &str) -> Option<String> {
        let normalized = self.normalize_definition_keyword(keyword);
        self.profile
            .definition_keyword_element_kinds
            .get(&normalized)
            .cloned()
    }

    fn semantic_kind_for_usage_keyword(&self, keyword: &str) -> Option<String> {
        self.profile
            .usage_keyword_element_kinds
            .get(&normalize_capability_token(keyword))
            .cloned()
    }
}

pub trait SemanticCapabilityOracle {
    fn can_contain(&self, container_kind: &str, child_kind: &str) -> CapabilityAnswer;
    fn can_specialize(&self, source_kind: &str, target_kind: &str) -> CapabilityAnswer;
    fn can_type_usage(&self, usage_kind: &str, definition_kind: &str) -> CapabilityAnswer;
    fn can_relate(
        &self,
        relationship_kind: &str,
        source_kind: &str,
        target_kind: &str,
    ) -> CapabilityAnswer;
    fn attribute_policy(&self, kind: &str, attribute: &str) -> AttributePolicyAnswer;

    fn relationship_uses_owner_as_source(&self, _relationship_kind: &str) -> bool {
        false
    }

    fn doc_id_attribute_aliases(&self) -> &'static [&'static str] {
        &["id"]
    }

    fn supporting_definition_keyword_for_usage(&self, _usage_kind: &str) -> Option<String> {
        None
    }

    fn normalize_definition_keyword(&self, keyword: &str) -> String {
        keyword.trim().to_string()
    }

    fn authoring_for_element_kind(&self, _kind: &str) -> Option<SemanticElementAuthoring> {
        None
    }

    fn semantic_kind_for_definition_keyword(&self, _keyword: &str) -> Option<String> {
        None
    }

    fn semantic_kind_for_usage_keyword(&self, _keyword: &str) -> Option<String> {
        None
    }
}

pub fn normalize_capability_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone, Default)]
pub struct ConservativeSemanticCapabilityOracle;

impl SemanticCapabilityOracle for ConservativeSemanticCapabilityOracle {
    fn can_contain(&self, container_kind: &str, child_kind: &str) -> CapabilityAnswer {
        if container_kind.is_empty() || child_kind.is_empty() {
            CapabilityAnswer::Unknown("missing kind information".to_string())
        } else if container_kind.eq_ignore_ascii_case("package")
            || container_kind.to_ascii_lowercase().contains("container")
            || container_kind.to_ascii_lowercase().contains("def")
            || container_kind.to_ascii_lowercase().contains("usage")
        {
            CapabilityAnswer::Allowed
        } else {
            CapabilityAnswer::Unknown(format!(
                "no language profile governs whether `{container_kind}` can own `{child_kind}`"
            ))
        }
    }

    fn can_specialize(&self, source_kind: &str, target_kind: &str) -> CapabilityAnswer {
        if source_kind.is_empty() || target_kind.is_empty() {
            CapabilityAnswer::Unknown("missing kind information".to_string())
        } else {
            CapabilityAnswer::Allowed
        }
    }

    fn can_type_usage(&self, usage_kind: &str, definition_kind: &str) -> CapabilityAnswer {
        if usage_kind.is_empty() || definition_kind.is_empty() {
            CapabilityAnswer::Unknown("missing kind information".to_string())
        } else if !definition_kind.to_ascii_lowercase().contains("def") {
            CapabilityAnswer::Denied(format!("`{definition_kind}` is not a definition-like type"))
        } else {
            CapabilityAnswer::Allowed
        }
    }

    fn can_relate(
        &self,
        relationship_kind: &str,
        source_kind: &str,
        target_kind: &str,
    ) -> CapabilityAnswer {
        if source_kind.trim().is_empty() {
            return CapabilityAnswer::Denied(format!(
                "relationship source `{source_kind}` is not element-like"
            ));
        }
        if target_kind.trim().is_empty() {
            return CapabilityAnswer::Denied(format!(
                "relationship target `{target_kind}` is not element-like"
            ));
        }
        if relationship_kind.trim().is_empty() {
            return CapabilityAnswer::Unknown(format!(
                "relationship kind `{relationship_kind}` is not yet governed"
            ));
        }
        CapabilityAnswer::Allowed
    }

    fn attribute_policy(&self, kind: &str, attribute: &str) -> AttributePolicyAnswer {
        let attribute = attribute.to_ascii_lowercase();
        let writable = matches!(
            attribute.as_str(),
            "declared_name"
                | "specializes"
                | "type"
                | "is_abstract"
                | "is_end"
                | "direction"
                | "target"
                | "imports"
                | "expression"
                | "doc"
        );
        AttributePolicyAnswer {
            writable,
            reason: (!writable).then(|| {
                format!("attribute `{attribute}` is not writable on `{kind}` by this service")
            }),
        }
    }
}
