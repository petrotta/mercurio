use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::{Deref, Index};
use std::sync::{Arc, OnceLock};

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::model::ir::{KirDocument, KirElement, KirFieldRegistry};

pub type NodeId = u32;

#[derive(Debug, Clone)]
pub struct Graph {
    elements: Vec<Element>,
    by_element_id: HashMap<String, NodeId>,
    edges: Vec<Edge>,
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Element {
    pub id: NodeId,
    pub element_id: String,
    pub kind: Arc<str>,
    pub layer: u8,
    pub properties: ElementProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub relation: Arc<str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphArtifact {
    pub elements: Vec<Element>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementProperties {
    element_id_value: Value,
    declared: BTreeMap<Arc<str>, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateId(String),
    UnknownElement(String),
    NodeOverflow,
    InvalidArtifact(String),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate element id: {id}"),
            Self::UnknownElement(id) => write!(f, "unknown element id: {id}"),
            Self::NodeOverflow => write!(f, "too many elements for u32 node ids"),
            Self::InvalidArtifact(message) => write!(f, "invalid graph artifact: {message}"),
        }
    }
}

impl std::error::Error for GraphError {}

impl Graph {
    pub fn from_document(document: KirDocument) -> Result<Self, GraphError> {
        let field_registry = KirFieldRegistry::from_document(&document);
        let mut by_element_id = HashMap::new();
        let mut elements = Vec::with_capacity(document.elements.len());
        let mut kind_interner = HashMap::new();
        let mut property_key_interner = HashMap::new();

        for raw in document.elements {
            if by_element_id.contains_key(&raw.id) {
                return Err(GraphError::DuplicateId(raw.id));
            }

            let id = NodeId::try_from(elements.len()).map_err(|_| GraphError::NodeOverflow)?;
            by_element_id.insert(raw.id.clone(), id);
            elements.push(Element::from_raw(
                id,
                raw,
                &mut kind_interner,
                &mut property_key_interner,
            ));
        }

        let mut graph = Self {
            outgoing: vec![Vec::new(); elements.len()],
            incoming: vec![Vec::new(); elements.len()],
            elements,
            by_element_id,
            edges: Vec::new(),
        };
        graph.build_edges(&field_registry)?;
        Ok(graph)
    }

    pub fn from_artifact(artifact: GraphArtifact) -> Result<Self, GraphError> {
        let mut by_element_id = HashMap::new();
        let mut elements = Vec::with_capacity(artifact.elements.len());
        let mut kind_interner = HashMap::new();
        let mut property_key_interner = HashMap::new();

        for raw in artifact.elements {
            if by_element_id.contains_key(&raw.element_id) {
                return Err(GraphError::DuplicateId(raw.element_id));
            }

            let id = NodeId::try_from(elements.len()).map_err(|_| GraphError::NodeOverflow)?;
            by_element_id.insert(raw.element_id.clone(), id);
            let kind = intern_string(&mut kind_interner, raw.kind.as_ref());
            elements.push(Element {
                id,
                element_id: raw.element_id.clone(),
                kind,
                layer: raw.layer,
                properties: ElementProperties::new(
                    raw.element_id,
                    raw.properties.into_declared(),
                    &mut property_key_interner,
                ),
            });
        }

        let mut graph = Self {
            outgoing: vec![Vec::new(); elements.len()],
            incoming: vec![Vec::new(); elements.len()],
            elements,
            by_element_id,
            edges: Vec::new(),
        };
        let mut relation_interner = HashMap::new();

        for mut edge in artifact.edges {
            if graph.element(edge.source).is_none() || graph.element(edge.target).is_none() {
                return Err(GraphError::InvalidArtifact(format!(
                    "edge references missing node {} -> {}",
                    edge.source, edge.target
                )));
            }
            let edge_index = graph.edges.len();
            edge.relation = intern_string(&mut relation_interner, edge.relation.as_ref());
            graph.outgoing[edge.source as usize].push(edge_index);
            graph.incoming[edge.target as usize].push(edge_index);
            graph.edges.push(edge);
        }

        Ok(graph)
    }

    pub fn artifact(&self) -> GraphArtifact {
        GraphArtifact {
            elements: self.elements.clone(),
            edges: self.edges.clone(),
        }
    }

    fn build_edges(&mut self, field_registry: &KirFieldRegistry) -> Result<(), GraphError> {
        let mut relation_interner = HashMap::new();

        for element in &self.elements {
            for (property, value) in &element.properties {
                if property.as_ref() == "element_id" {
                    continue;
                }
                for external_target in field_registry.reference_ids(property, value) {
                    let Some(&target) = self.by_element_id.get(external_target) else {
                        continue;
                    };
                    let edge = Edge {
                        source: element.id,
                        target,
                        relation: intern_string(&mut relation_interner, property),
                    };
                    let edge_index = self.edges.len();
                    self.outgoing[element.id as usize].push(edge_index);
                    self.incoming[target as usize].push(edge_index);
                    self.edges.push(edge);
                }
            }
        }

        Ok(())
    }

    pub fn element(&self, id: NodeId) -> Option<&Element> {
        self.elements.get(id as usize)
    }

    pub fn element_by_element_id(&self, element_id: &str) -> Option<&Element> {
        self.node_id(element_id).and_then(|id| self.element(id))
    }

    pub fn node_id(&self, element_id: &str) -> Option<NodeId> {
        self.by_element_id.get(element_id).copied()
    }

    pub fn element_id(&self, id: NodeId) -> Option<&str> {
        self.element(id).map(|element| element.element_id.as_str())
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn outgoing_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.outgoing
            .get(id as usize)
            .into_iter()
            .flat_map(|edges| {
                edges
                    .iter()
                    .filter_map(|edge_index| self.edges.get(*edge_index))
            })
    }

    pub fn incoming_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.incoming
            .get(id as usize)
            .into_iter()
            .flat_map(|edges| {
                edges
                    .iter()
                    .filter_map(|edge_index| self.edges.get(*edge_index))
            })
    }

    pub fn outgoing(&self, id: NodeId, relation: &str) -> impl Iterator<Item = &Edge> {
        self.outgoing_edges(id)
            .filter(move |edge| edge.relation.as_ref() == relation)
    }

    pub fn incoming(&self, id: NodeId, relation: &str) -> impl Iterator<Item = &Edge> {
        self.incoming_edges(id)
            .filter(move |edge| edge.relation.as_ref() == relation)
    }

    pub fn relation_targets(
        &self,
        element_id: &str,
        relation: &str,
    ) -> Result<Vec<&Element>, GraphError> {
        let node_id = self
            .node_id(element_id)
            .ok_or_else(|| GraphError::UnknownElement(element_id.to_string()))?;

        Ok(self
            .outgoing(node_id, relation)
            .filter_map(|edge| self.element(edge.target))
            .collect())
    }
}

fn intern_string(interner: &mut HashMap<String, Arc<str>>, value: &str) -> Arc<str> {
    if let Some(interned) = interner.get(value) {
        return Arc::clone(interned);
    }
    let interned: Arc<str> = Arc::from(value);
    interner.insert(value.to_string(), Arc::clone(&interned));
    interned
}

fn intern_property_key(interner: &mut HashMap<String, Arc<str>>, key: &str) -> Arc<str> {
    if let Some(interned) = known_property_keys().get(key) {
        return Arc::clone(interned);
    }
    intern_string(interner, key)
}

fn known_property_keys() -> &'static HashMap<&'static str, Arc<str>> {
    static KNOWN_KEYS: OnceLock<HashMap<&'static str, Arc<str>>> = OnceLock::new();
    KNOWN_KEYS.get_or_init(|| {
        [
            "allocated",
            "allocated_to",
            "annotatedElement",
            "arguments",
            "body",
            "chaining_feature",
            "conjugated",
            "declared_multiplicity",
            "declared_name",
            "declared_short_name",
            "definition",
            "dependencies",
            "direction",
            "doc",
            "documentedElement",
            "documentation",
            "effect",
            "expression",
            "expression_ir",
            "feature_kind",
            "feature_typings",
            "features",
            "featuring_type",
            "imports",
            "is_abstract",
            "is_conjugated",
            "is_derived",
            "is_end",
            "is_ordered",
            "is_readonly",
            "is_unique",
            "is_variable",
            "items",
            "kir_property",
            "language",
            "lower",
            "member",
            "memberElement",
            "members",
            "metadata",
            "metamodel_language",
            "metamodel_layer",
            "metatype",
            "multiplicity",
            "multiplicity_lower",
            "multiplicity_upper",
            "name",
            "operator",
            "operator_expression",
            "opposite",
            "original_definition",
            "ownedElement",
            "owned_feature",
            "owned_features",
            "owner",
            "owning_definition",
            "owning_namespace",
            "owning_type",
            "parameters",
            "parent_state",
            "parts",
            "payload",
            "pilot_library_group",
            "qualified_name",
            "redefined_features",
            "redefines",
            "related",
            "relatedElement",
            "relationships",
            "requirement_id",
            "result",
            "satisfy",
            "source",
            "source_feature",
            "source_file",
            "source_language",
            "source_span",
            "sources",
            "specialized_features",
            "specializes",
            "subsets",
            "subsetted_features",
            "successions",
            "target",
            "target_ref",
            "targets",
            "text",
            "trigger",
            "trigger_kind",
            "type",
            "type_label",
            "upper",
            "verify",
        ]
        .into_iter()
        .map(|key| (key, Arc::<str>::from(key)))
        .collect()
    })
}

impl Element {
    fn from_raw(
        id: NodeId,
        raw: KirElement,
        kind_interner: &mut HashMap<String, Arc<str>>,
        property_key_interner: &mut HashMap<String, Arc<str>>,
    ) -> Self {
        let kind = intern_string(kind_interner, &raw.kind);
        Self {
            id,
            element_id: raw.id.clone(),
            kind,
            layer: raw.layer,
            properties: ElementProperties::new(raw.id, raw.properties, property_key_interner),
        }
    }
}

impl ElementProperties {
    fn new(
        element_id: String,
        mut declared: BTreeMap<String, Value>,
        property_key_interner: &mut HashMap<String, Arc<str>>,
    ) -> Self {
        declared.remove("element_id");
        let declared = declared
            .into_iter()
            .map(|(key, value)| (intern_property_key(property_key_interner, &key), value))
            .collect();
        Self {
            element_id_value: Value::String(element_id),
            declared,
        }
    }

    fn into_declared(self) -> BTreeMap<String, Value> {
        self.declared
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    pub fn to_btree_map(&self) -> BTreeMap<String, Value> {
        let mut properties = self
            .declared
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        properties.insert("element_id".to_string(), self.element_id_value.clone());
        properties
    }

    pub fn from_declared_arc_for_artifact(
        element_id: String,
        mut declared: BTreeMap<Arc<str>, Value>,
    ) -> Self {
        declared.remove("element_id");
        Self {
            element_id_value: Value::String(element_id),
            declared,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        if key == "element_id" {
            return Some(&self.element_id_value);
        }
        self.declared.get(key)
    }
}

impl Deref for ElementProperties {
    type Target = BTreeMap<Arc<str>, Value>;

    fn deref(&self) -> &Self::Target {
        &self.declared
    }
}

impl Index<&str> for ElementProperties {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("no entry found for key {index:?}"))
    }
}

impl<'a> IntoIterator for &'a ElementProperties {
    type Item = (&'a Arc<str>, &'a Value);
    type IntoIter = std::collections::btree_map::Iter<'a, Arc<str>, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.declared.iter()
    }
}

impl Serialize for ElementProperties {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.declared.len() + 1))?;
        for (key, value) in &self.declared {
            map.serialize_entry(key, value)?;
        }
        map.serialize_entry("element_id", &self.element_id_value)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ElementProperties {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut declared = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let element_id_value = declared.remove("element_id").unwrap_or(Value::Null);
        let declared = declared
            .into_iter()
            .map(|(key, value)| (Arc::<str>::from(key), value))
            .collect();
        Ok(Self {
            element_id_value,
            declared,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_element_id_as_property_without_creating_self_edge() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        assert_eq!(element.element_id, "type.Demo.Vehicle");
        assert_eq!(
            element.properties.get("element_id"),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn canonical_element_id_overwrites_mismatched_property() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "element_id".to_string(),
                    Value::String("stale".to_string()),
                )]),
            }],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        assert_eq!(
            element.properties.get("element_id"),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
    }

    #[test]
    fn keeps_metatype_and_specialization_as_distinct_relations() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Camera".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "metatype".to_string(),
                            Value::String("Model::Systems::PartDefinition".to_string()),
                        ),
                        (
                            "specializes".to_string(),
                            Value::Array(vec![Value::String("type.ImagingDevice".to_string())]),
                        ),
                    ]),
                },
                KirElement {
                    id: "type.ImagingDevice".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        let camera_id = graph.node_id("type.Camera").unwrap();
        let metatype_targets = graph
            .outgoing(camera_id, "metatype")
            .filter_map(|edge| graph.element_id(edge.target))
            .collect::<Vec<_>>();
        let specialization_targets = graph
            .outgoing(camera_id, "specializes")
            .filter_map(|edge| graph.element_id(edge.target))
            .collect::<Vec<_>>();

        assert_eq!(metatype_targets, vec!["Model::Systems::PartDefinition"]);
        assert_eq!(specialization_targets, vec!["type.ImagingDevice"]);
    }

    #[test]
    fn non_reference_strings_do_not_create_edges() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "body".to_string(),
                        Value::String("type.Demo.Engine".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.Demo.Engine".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        assert!(graph.edges().is_empty());
    }

    #[test]
    fn metamodel_declared_reference_field_creates_edges() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                metamodel_feature(
                    "metafeature.Demo.Thing.related_requirement",
                    "Demo::Thing",
                    "related_requirement",
                    "reference",
                    Some(Value::String("1".to_string())),
                ),
                KirElement {
                    id: "type.Demo.Source".to_string(),
                    kind: "Demo::Thing".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "related_requirement".to_string(),
                        Value::String("requirement.Demo.R1".to_string()),
                    )]),
                },
                KirElement {
                    id: "requirement.Demo.R1".to_string(),
                    kind: "Demo::Requirement".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        let source = graph.node_id("type.Demo.Source").unwrap();
        let target = graph.node_id("requirement.Demo.R1").unwrap();

        assert!(
            graph
                .outgoing(source, "related_requirement")
                .any(|edge| edge.target == target)
        );
    }

    #[test]
    fn metamodel_declared_scalar_field_does_not_create_edges() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                metamodel_feature(
                    "metafeature.Demo.Thing.external_id",
                    "Demo::Thing",
                    "external_id",
                    "attribute",
                    None,
                ),
                KirElement {
                    id: "type.Demo.Source".to_string(),
                    kind: "Demo::Thing".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "external_id".to_string(),
                        Value::String("type.Demo.Target".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.Demo.Target".to_string(),
                    kind: "Demo::Thing".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        assert!(graph.edges().is_empty());
    }

    #[test]
    fn registered_reference_list_scalar_creates_edge() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::String("type.Demo.Machine".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.Demo.Machine".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.edges()[0].relation.as_ref(), "specializes");
    }

    #[test]
    fn representative_example_projects_core_graph_edges() {
        let graph = Graph::from_document(KirDocument::representative_example().unwrap()).unwrap();
        let package = graph.node_id("pkg.Example").unwrap();
        let activity = graph.node_id("activity.Example.Startup").unwrap();

        assert!(
            graph
                .element_by_element_id("activity.Example.Startup")
                .is_some()
        );
        assert!(
            graph
                .outgoing(activity, "owner")
                .any(|edge| edge.target == package)
        );
        assert!(
            graph
                .outgoing(package, "members")
                .any(|edge| edge.target == activity)
        );
    }

    fn metamodel_feature(
        id: &str,
        owner: &str,
        kir_property: &str,
        feature_kind: &str,
        upper: Option<Value>,
    ) -> KirElement {
        let mut properties = BTreeMap::from([
            ("owner".to_string(), Value::String(owner.to_string())),
            (
                "kir_property".to_string(),
                Value::String(kir_property.to_string()),
            ),
            (
                "feature_kind".to_string(),
                Value::String(feature_kind.to_string()),
            ),
        ]);
        if let Some(upper) = upper {
            properties.insert("upper".to_string(), upper);
        }
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties,
        }
    }
}
