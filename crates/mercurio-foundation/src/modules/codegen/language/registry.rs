use std::collections::BTreeMap;

use crate::codegen::language::concepts::Concept;
use crate::codegen::language::profile::{
    LanguageProfile, LanguageProfileError, default_language_profile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetamodelConceptRegistry {
    canonical_kinds: BTreeMap<Concept, String>,
    semantic_anchors: BTreeMap<String, String>,
    aliases: BTreeMap<String, String>,
}

impl MetamodelConceptRegistry {
    pub fn from_profile(profile: &LanguageProfile) -> Self {
        Self {
            canonical_kinds: profile.canonical_kinds.clone(),
            semantic_anchors: profile.semantic_anchors.clone(),
            aliases: profile.aliases.clone(),
        }
    }

    pub fn canonical_kind(&self, concept: &Concept) -> Option<&str> {
        self.canonical_kinds.get(concept).map(String::as_str)
    }

    pub fn normalize_kind<'a>(&'a self, kind: &'a str) -> &'a str {
        self.aliases.get(kind).map(String::as_str).unwrap_or(kind)
    }

    pub fn is_kind(&self, kind: &str, concept: &Concept) -> bool {
        self.canonical_kind(concept)
            .map(|canonical| self.normalize_kind(kind) == canonical)
            .unwrap_or(false)
    }

    pub fn semantic_anchor(&self, concept: &str) -> Option<&str> {
        self.semantic_anchors.get(concept).map(String::as_str)
    }
}

pub fn default_metamodel_registry() -> Result<MetamodelConceptRegistry, LanguageProfileError> {
    default_language_profile().map(|profile| MetamodelConceptRegistry::from_profile(&profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_is_core_only() {
        let registry = default_metamodel_registry().unwrap();

        assert!(registry.is_kind("model.Package", &Concept::PACKAGE));
        assert!(registry.semantic_anchor("part_definition").is_none());
    }

    #[test]
    fn registry_accepts_profile_defined_concepts() {
        let concept = Concept::new("domain_specific");
        let profile = LanguageProfile {
            id: "test".to_string(),
            language: crate::codegen::language::LanguageId::from("test"),
            language_version: "1".to_string(),
            metamodel_version: "1".to_string(),
            stdlib_version: "none".to_string(),
            stdlib_path: "empty.kir.json".to_string(),
            kir_schema_version: crate::kir::KIR_SCHEMA_VERSION.to_string(),
            canonical_kinds: BTreeMap::from([(concept.clone(), "Domain::Specific".to_string())]),
            semantic_anchors: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        let registry = MetamodelConceptRegistry::from_profile(&profile);

        assert_eq!(registry.canonical_kind(&concept), Some("Domain::Specific"));
    }
}
