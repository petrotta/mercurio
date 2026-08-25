use std::collections::{BTreeSet, VecDeque};
use std::fmt;

use crate::graph::{Graph, NodeId};
use crate::language::{Concept, MetamodelConceptRegistry};
use crate::metamodel::element_metatype;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeSubtypes {
    No,
    Yes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLayers {
    UserModel,
    Library,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTarget {
    pub concept: Concept,
    pub include_subtypes: IncludeSubtypes,
    pub layers: TargetLayers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSemanticTarget {
    pub concept: Concept,
    pub anchor_id: String,
    pub matching_metatypes: BTreeSet<String>,
    pub matching_elements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticTargetError {
    MissingConcept(Concept),
    MissingAnchor(String),
}

impl fmt::Display for SemanticTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConcept(concept) => {
                write!(f, "language profile does not define concept {concept}")
            }
            Self::MissingAnchor(anchor) => {
                write!(f, "semantic anchor not found in graph: {anchor}")
            }
        }
    }
}

impl std::error::Error for SemanticTargetError {}

pub struct SemanticTargetResolver<'a> {
    graph: &'a Graph,
    concepts: &'a MetamodelConceptRegistry,
}

impl<'a> SemanticTargetResolver<'a> {
    pub fn new(graph: &'a Graph, concepts: &'a MetamodelConceptRegistry) -> Self {
        Self { graph, concepts }
    }

    pub fn resolve(
        &self,
        target: &SemanticTarget,
    ) -> Result<ResolvedSemanticTarget, SemanticTargetError> {
        let anchor_id = self
            .concepts
            .canonical_kind(&target.concept)
            .ok_or_else(|| SemanticTargetError::MissingConcept(target.concept.clone()))?;
        let anchor_node = self
            .graph
            .node_id(anchor_id)
            .ok_or_else(|| SemanticTargetError::MissingAnchor(anchor_id.to_string()))?;
        let matching_metatypes = match target.include_subtypes {
            IncludeSubtypes::No => BTreeSet::from([anchor_id.to_string()]),
            IncludeSubtypes::Yes => specialization_descendants(self.graph, anchor_node),
        };
        let matching_elements = self
            .graph
            .elements()
            .iter()
            .filter(|element| target.layers.includes(element.layer))
            .filter(|element| {
                element_metatype(self.graph, element.id)
                    .map(|metatype| matching_metatypes.contains(&metatype.element_id))
                    .unwrap_or_else(|| matching_metatypes.contains(element.kind.as_ref()))
            })
            .map(|element| element.element_id.clone())
            .collect::<Vec<_>>();

        Ok(ResolvedSemanticTarget {
            concept: target.concept.clone(),
            anchor_id: anchor_id.to_string(),
            matching_metatypes,
            matching_elements,
        })
    }
}

impl TargetLayers {
    fn includes(self, layer: u8) -> bool {
        match self {
            Self::UserModel => layer == 2,
            Self::Library => layer < 2,
            Self::All => true,
        }
    }
}

fn specialization_descendants(graph: &Graph, anchor: NodeId) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([anchor]);
    while let Some(node_id) = queue.pop_front() {
        if !seen.insert(node_id) {
            continue;
        }
        if let Some(element_id) = graph.element_id(node_id) {
            result.insert(element_id.to_string());
        }
        for edge in graph.incoming(node_id, "specializes") {
            queue.push_back(edge.source);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::Value;

    use crate::{
        Concept, Graph, KirDocument, KirElement, LanguageId, LanguageProfile,
        MetamodelConceptRegistry,
    };

    use super::*;

    #[test]
    fn resolves_concept_target_with_subtypes() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Package".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Custom::Package".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::Array(vec![Value::String("Model::Package".to_string())]),
                    )]),
                },
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Model::Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "metatype".to_string(),
                        Value::String("Custom::Package".to_string()),
                    )]),
                },
            ],
        };
        let graph = Graph::from_document(document).unwrap();
        let profile = LanguageProfile {
            id: "test".to_string(),
            language: LanguageId::from("model"),
            language_version: "2.0".to_string(),
            metamodel_version: "2.0".to_string(),
            stdlib_version: "test".to_string(),
            stdlib_path: "stdlib.kir.json".to_string(),
            kir_schema_version: "0.2".to_string(),
            canonical_kinds: BTreeMap::from([(Concept::PACKAGE, "Model::Package".to_string())]),
            semantic_anchors: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        let registry = MetamodelConceptRegistry::from_profile(&profile);
        let resolved = SemanticTargetResolver::new(&graph, &registry)
            .resolve(&SemanticTarget {
                concept: Concept::PACKAGE,
                include_subtypes: IncludeSubtypes::Yes,
                layers: TargetLayers::UserModel,
            })
            .unwrap();

        assert_eq!(
            resolved.matching_metatypes,
            BTreeSet::from(["Custom::Package".to_string(), "Model::Package".to_string()])
        );
        assert_eq!(resolved.matching_elements, vec!["pkg.Demo"]);
    }
}
