use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kir::KirDocument;
use crate::model::derived_properties;
use crate::model::{
    AttributeRow, AttributeValueSource, ElementSummary, MetamodelAttributeRegistry,
    collect_specialization_ancestors, effective_element_properties_with_derived,
    query_element_attributes,
};
use crate::model::{Edge, Element, Graph};
use crate::runtime::{Runtime, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphScope {
    Model,
    ModelPlusContext,
    Full,
}

impl GraphScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::ModelPlusContext => "model_plus_context",
            Self::Full => "full",
        }
    }

    pub fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("full") => Self::Full,
            Some("model_plus_context") => Self::ModelPlusContext,
            Some("model") | None | Some(_) => Self::Model,
        }
    }

    pub fn all() -> Vec<String> {
        [Self::Model, Self::ModelPlusContext, Self::Full]
            .into_iter()
            .map(|scope| scope.as_str().to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphDto {
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphNodeDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub property_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphEdgeDto {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelMetadataDto {
    pub element_count: usize,
    pub edge_count: usize,
    pub library_element_count: usize,
    pub user_element_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_version: Option<String>,
    pub layers: Vec<u8>,
    pub relations: Vec<String>,
    pub graph_scopes: Vec<String>,
    pub default_graph_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetatypeExplorerRequestDto {
    pub seed_id: String,
    #[serde(default)]
    pub expanded_parents: Vec<String>,
    #[serde(default)]
    pub expanded_children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelExplorerRequestDto {
    pub seed_id: String,
    #[serde(default)]
    pub expanded_parents: Vec<String>,
    #[serde(default)]
    pub expanded_children: Vec<String>,
    #[serde(default = "default_true")]
    pub include_reference_edges: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExplorerAttributeDto {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MetatypeExplorerGraphDto {
    pub seed_id: String,
    pub nodes: Vec<MetatypeExplorerNodeDto>,
    pub edges: Vec<MetatypeExplorerEdgeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MetatypeExplorerNodeDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub attributes: Vec<ExplorerAttributeDto>,
    pub specializes_count: usize,
    pub specialized_by_count: usize,
    pub is_seed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MetatypeExplorerEdgeDto {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelExplorerGraphDto {
    pub seed_id: String,
    pub nodes: Vec<ModelExplorerNodeDto>,
    pub edges: Vec<ModelExplorerEdgeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelExplorerNodeDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub attributes: Vec<ExplorerAttributeDto>,
    pub specializes_count: usize,
    pub specialized_by_count: usize,
    pub is_seed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelExplorerEdgeDto {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ElementDetailsDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<ElementSummaryDto>,
    pub metatype_specialization_chain: Vec<ElementSummaryDto>,
    pub direct_properties: BTreeMap<String, Value>,
    pub inherited_properties: Vec<InheritedPropertiesDto>,
    pub effective_properties: BTreeMap<String, Value>,
    pub property_table: ElementPropertyTableDto,
    pub specialization_chain: Vec<ElementSummaryDto>,
    pub inbound: Vec<GraphEdgeDto>,
    pub outbound: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ElementSummaryDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InheritedPropertiesDto {
    pub element: ElementSummaryDto,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ElementPropertyTableDto {
    pub rows: Vec<ElementPropertyRowDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ElementPropertyRowDto {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<ElementSummaryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplicity_lower: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplicity_upper: Option<String>,
    pub origin_kind: String,
    pub has_direct_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_value: Option<Value>,
    pub has_effective_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_value: Option<Value>,
    pub inherited_values: Vec<InheritedPropertyValueDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InheritedPropertyValueDto {
    pub element: ElementSummaryDto,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LibraryTreeNodeDto {
    pub id: String,
    pub label: String,
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    pub child_count: usize,
    pub children: Vec<LibraryTreeNodeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SearchResultDto {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
}

pub fn graph_view(graph: &Graph, scope: GraphScope) -> GraphDto {
    let visible_ids = collect_graph_scope_ids(graph, scope);
    let mut nodes = graph
        .elements()
        .iter()
        .filter(|element| visible_ids.contains(&element.id))
        .map(|element| GraphNodeDto {
            id: element.element_id.clone(),
            label: label_for_id(&element.element_id),
            kind: element.kind.to_string(),
            layer: element.layer,
            property_count: element.properties.len(),
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut edges = graph
        .edges()
        .iter()
        .filter(|edge| visible_ids.contains(&edge.source) && visible_ids.contains(&edge.target))
        .filter_map(|edge| edge_view(graph, edge))
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));

    GraphDto { nodes, edges }
}

pub fn model_metadata_view(graph: &Graph, stdlib_document: &KirDocument) -> ModelMetadataDto {
    let layers = graph
        .elements()
        .iter()
        .map(|element| element.layer)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let relations = graph
        .edges()
        .iter()
        .map(|edge| edge.relation.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let library_element_count = graph
        .elements()
        .iter()
        .filter(|element| element.layer < 2)
        .count();
    let user_element_count = graph
        .elements()
        .iter()
        .filter(|element| element.layer == 2)
        .count();

    ModelMetadataDto {
        element_count: graph.elements().len(),
        edge_count: graph.edge_count(),
        library_element_count,
        user_element_count,
        library_version: metadata_string(stdlib_document, "stdlib_version"),
        layers,
        relations,
        graph_scopes: GraphScope::all(),
        default_graph_scope: GraphScope::Model.as_str().to_string(),
    }
}

pub fn document_model_metadata_view(document: &KirDocument) -> ModelMetadataDto {
    let layers = document
        .elements
        .iter()
        .map(|element| element.layer)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    ModelMetadataDto {
        element_count: document.elements.len(),
        edge_count: 0,
        library_element_count: document.elements.len(),
        user_element_count: 0,
        library_version: metadata_string(document, "stdlib_version"),
        layers,
        relations: Vec::new(),
        graph_scopes: GraphScope::all(),
        default_graph_scope: GraphScope::Model.as_str().to_string(),
    }
}

pub fn element_details(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    element_id: &str,
) -> Option<ElementDetailsDto> {
    let element = graph.element_by_element_id(element_id)?;

    let mut inbound = graph
        .incoming_edges(element.id)
        .filter_map(|edge| edge_view(graph, edge))
        .collect::<Vec<_>>();
    inbound.sort_by(|left, right| left.id.cmp(&right.id));

    let mut outbound = graph
        .outgoing_edges(element.id)
        .filter_map(|edge| edge_view(graph, edge))
        .collect::<Vec<_>>();
    outbound.sort_by(|left, right| left.id.cmp(&right.id));

    Some(build_element_details(
        graph,
        metamodel_registry,
        element,
        inbound,
        outbound,
    ))
}

pub fn search_view(graph: &Graph, query: &str) -> Vec<SearchResultDto> {
    let query = query.trim().to_ascii_lowercase();
    let mut results = graph
        .elements()
        .iter()
        .map(|element| SearchResultDto {
            id: element.element_id.clone(),
            label: label_for_id(&element.element_id),
            kind: element.kind.to_string(),
            layer: element.layer,
        })
        .filter(|entry| {
            query.is_empty()
                || entry.id.to_ascii_lowercase().contains(&query)
                || entry.kind.to_ascii_lowercase().contains(&query)
                || entry.label.to_ascii_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| left.id.cmp(&right.id));
    results
}

pub fn metatype_explorer_view(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    request: &MetatypeExplorerRequestDto,
) -> Option<MetatypeExplorerGraphDto> {
    let seed = graph.element_by_element_id(&request.seed_id)?;
    let expanded_parents = request
        .expanded_parents
        .iter()
        .filter_map(|id| graph.node_id(id))
        .collect::<BTreeSet<_>>();
    let expanded_children = request
        .expanded_children
        .iter()
        .filter_map(|id| graph.node_id(id))
        .collect::<BTreeSet<_>>();
    let mut visible_ids = BTreeSet::from([seed.id]);

    for edge in graph.outgoing(seed.id, "specializes") {
        visible_ids.insert(edge.target);
    }

    for node_id in &expanded_parents {
        visible_ids.insert(*node_id);
        for edge in graph.outgoing(*node_id, "specializes") {
            visible_ids.insert(edge.target);
        }
    }

    for node_id in &expanded_children {
        visible_ids.insert(*node_id);
        for edge in graph.incoming(*node_id, "specializes") {
            visible_ids.insert(edge.source);
        }
    }

    let mut nodes = visible_ids
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .map(|element| metatype_explorer_node(graph, metamodel_registry, element, seed.id))
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut edge_keys = BTreeSet::new();
    for node_id in &visible_ids {
        for edge in graph.outgoing(*node_id, "specializes") {
            if !visible_ids.contains(&edge.target) {
                continue;
            }

            let Some(source_id) = graph.element_id(edge.source) else {
                continue;
            };
            let Some(target_id) = graph.element_id(edge.target) else {
                continue;
            };
            edge_keys.insert((source_id.to_string(), target_id.to_string()));
        }
    }

    let edges = edge_keys
        .into_iter()
        .map(|(source, target)| MetatypeExplorerEdgeDto {
            id: format!("specializes:{source}->{target}"),
            source,
            target,
            relation: "specializes".to_string(),
        })
        .collect();

    Some(MetatypeExplorerGraphDto {
        seed_id: seed.element_id.clone(),
        nodes,
        edges,
    })
}

pub fn model_explorer_view(
    graph: &Graph,
    request: &ModelExplorerRequestDto,
) -> Option<ModelExplorerGraphDto> {
    let seed = graph.element_by_element_id(&request.seed_id)?;
    if seed.layer != 2 {
        return None;
    }

    let expanded_parents = request
        .expanded_parents
        .iter()
        .filter_map(|id| graph.node_id(id))
        .collect::<BTreeSet<_>>();
    let expanded_children = request
        .expanded_children
        .iter()
        .filter_map(|id| graph.node_id(id))
        .collect::<BTreeSet<_>>();
    let mut visible_ids = BTreeSet::from([seed.id]);

    for edge in graph.outgoing(seed.id, "specializes") {
        if graph
            .element(edge.target)
            .is_some_and(|element| element.layer == 2)
        {
            visible_ids.insert(edge.target);
        }
    }

    for node_id in &expanded_parents {
        let Some(element) = graph.element(*node_id) else {
            continue;
        };
        if element.layer != 2 {
            continue;
        }
        visible_ids.insert(*node_id);
        for edge in graph.outgoing(*node_id, "specializes") {
            if graph
                .element(edge.target)
                .is_some_and(|target| target.layer == 2)
            {
                visible_ids.insert(edge.target);
            }
        }
    }

    for node_id in &expanded_children {
        let Some(element) = graph.element(*node_id) else {
            continue;
        };
        if element.layer != 2 {
            continue;
        }
        visible_ids.insert(*node_id);
        for edge in graph.incoming(*node_id, "specializes") {
            if graph
                .element(edge.source)
                .is_some_and(|source| source.layer == 2)
            {
                visible_ids.insert(edge.source);
            }
        }
    }

    let mut nodes = visible_ids
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .map(|element| model_explorer_node(graph, element, seed.id))
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut edge_keys = BTreeSet::new();
    for node_id in &visible_ids {
        for edge in graph.outgoing(*node_id, "specializes") {
            if !visible_ids.contains(&edge.target) {
                continue;
            }
            let Some(source_id) = graph.element_id(edge.source) else {
                continue;
            };
            let Some(target_id) = graph.element_id(edge.target) else {
                continue;
            };
            edge_keys.insert((
                source_id.to_string(),
                target_id.to_string(),
                "specializes".to_string(),
            ));
        }
    }

    if request.include_reference_edges {
        for node_id in &visible_ids {
            for edge in graph.outgoing_edges(*node_id) {
                if !visible_ids.contains(&edge.target)
                    || !include_model_reference_relation(&edge.relation)
                {
                    continue;
                }
                let Some(source_id) = graph.element_id(edge.source) else {
                    continue;
                };
                let Some(target_id) = graph.element_id(edge.target) else {
                    continue;
                };
                edge_keys.insert((
                    source_id.to_string(),
                    target_id.to_string(),
                    edge.relation.to_string(),
                ));
            }
        }
    }

    let edges = edge_keys
        .into_iter()
        .map(|(source, target, relation)| ModelExplorerEdgeDto {
            id: format!("{relation}:{source}->{target}"),
            source,
            target,
            relation,
        })
        .collect();

    Some(ModelExplorerGraphDto {
        seed_id: seed.element_id.clone(),
        nodes,
        edges,
    })
}

pub fn library_tree_view(graph: &Graph) -> Vec<LibraryTreeNodeDto> {
    build_tree_from_graph(graph, |element| element.layer < 2)
}

pub fn library_tree_view_from_document(
    document: &KirDocument,
) -> Result<Vec<LibraryTreeNodeDto>, RuntimeError> {
    let runtime = Runtime::from_document(document.clone())?;
    Ok(build_tree_from_graph(runtime.graph(), |_| true))
}

fn build_element_details(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    element: &Element,
    inbound: Vec<GraphEdgeDto>,
    outbound: Vec<GraphEdgeDto>,
) -> ElementDetailsDto {
    let ancestors = collect_specialization_ancestors(graph, element.id);
    let specialization_chain = ancestors
        .iter()
        .map(|ancestor| element_summary_dto(ancestor))
        .collect::<Vec<_>>();
    let inherited_properties = ancestors
        .iter()
        .filter(|ancestor| !ancestor.properties.is_empty())
        .map(|ancestor| InheritedPropertiesDto {
            element: element_summary_dto(ancestor),
            properties: ancestor.properties.to_btree_map(),
        })
        .collect::<Vec<_>>();

    let derived_properties = derived_properties(graph, element);
    let effective_properties = effective_element_properties_with_derived(
        &ancestors,
        &element.properties,
        &derived_properties,
    );
    let attribute_query = query_element_attributes(graph, metamodel_registry, element.id, None)
        .unwrap_or_else(|| crate::model::ElementAttributeQuery {
            metatype: None,
            metatype_specialization_chain: Vec::new(),
            rows: Vec::new(),
        });

    ElementDetailsDto {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
        metatype: attribute_query.metatype.map(element_summary_from_query),
        metatype_specialization_chain: attribute_query
            .metatype_specialization_chain
            .into_iter()
            .map(element_summary_from_query)
            .collect(),
        direct_properties: element.properties.to_btree_map(),
        inherited_properties,
        effective_properties,
        property_table: ElementPropertyTableDto {
            rows: attribute_query
                .rows
                .into_iter()
                .map(property_row_from_query)
                .collect(),
        },
        specialization_chain,
        inbound,
        outbound,
    }
}

fn element_summary_from_query(summary: ElementSummary) -> ElementSummaryDto {
    ElementSummaryDto {
        id: summary.id,
        label: summary.label,
        kind: summary.kind,
        layer: summary.layer,
    }
}

fn inherited_value_from_query(value: AttributeValueSource) -> InheritedPropertyValueDto {
    InheritedPropertyValueDto {
        element: element_summary_from_query(value.element),
        value: value.value,
    }
}

fn property_row_from_query(row: AttributeRow) -> ElementPropertyRowDto {
    ElementPropertyRowDto {
        name: row.name,
        declared_by: row.declared_by.map(element_summary_from_query),
        type_label: row.type_label,
        feature_kind: row.feature_kind,
        multiplicity_lower: row.multiplicity_lower,
        multiplicity_upper: row.multiplicity_upper,
        origin_kind: row.origin_kind,
        has_direct_value: row.has_direct_value,
        direct_value: row.direct_value,
        has_effective_value: row.has_effective_value,
        effective_value: row.effective_value,
        inherited_values: row
            .inherited_values
            .into_iter()
            .map(inherited_value_from_query)
            .collect(),
    }
}

fn element_summary_dto(element: &Element) -> ElementSummaryDto {
    ElementSummaryDto {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
    }
}

fn edge_view(graph: &Graph, edge: &Edge) -> Option<GraphEdgeDto> {
    let source = graph.element_id(edge.source)?.to_string();
    let target = graph.element_id(edge.target)?.to_string();

    Some(GraphEdgeDto {
        id: format!("{source}:{}:{target}", edge.relation),
        source,
        target,
        relation: edge.relation.to_string(),
    })
}

fn label_for_id(id: &str) -> String {
    let tail = id.rsplit("::").next().unwrap_or(id);
    tail.rsplit('.').next().unwrap_or(tail).to_string()
}

fn collect_graph_scope_ids(graph: &Graph, scope: GraphScope) -> BTreeSet<u32> {
    let mut visible_ids = graph
        .elements()
        .iter()
        .filter(|element| match scope {
            GraphScope::Model | GraphScope::ModelPlusContext => element.layer == 2,
            GraphScope::Full => true,
        })
        .map(|element| element.id)
        .collect::<BTreeSet<_>>();

    if scope == GraphScope::ModelPlusContext {
        let model_ids = visible_ids.iter().copied().collect::<Vec<_>>();
        for node_id in model_ids {
            for edge in graph.outgoing_edges(node_id) {
                visible_ids.insert(edge.target);
            }
            for edge in graph.incoming_edges(node_id) {
                visible_ids.insert(edge.source);
            }
        }
    }

    visible_ids
}

fn metatype_explorer_node(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    element: &Element,
    seed_id: u32,
) -> MetatypeExplorerNodeDto {
    let mut attributes = metamodel_registry
        .declared_attributes_for(&element.element_id)
        .iter()
        .map(|declaration| ExplorerAttributeDto {
            name: declaration.name.clone(),
            type_label: declaration.type_label.clone(),
        })
        .collect::<Vec<_>>();
    attributes.sort_by(|left, right| left.name.cmp(&right.name));

    MetatypeExplorerNodeDto {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
        attributes,
        specializes_count: graph.outgoing(element.id, "specializes").count(),
        specialized_by_count: graph.incoming(element.id, "specializes").count(),
        is_seed: element.id == seed_id,
    }
}

fn model_explorer_node(graph: &Graph, element: &Element, seed_id: u32) -> ModelExplorerNodeDto {
    let mut attributes = owned_feature_attributes(graph, element);
    attributes.sort_by(|left, right| left.name.cmp(&right.name));

    ModelExplorerNodeDto {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
        attributes,
        specializes_count: graph
            .outgoing(element.id, "specializes")
            .filter(|edge| {
                graph
                    .element(edge.target)
                    .is_some_and(|target| target.layer == 2)
            })
            .count(),
        specialized_by_count: graph
            .incoming(element.id, "specializes")
            .filter(|edge| {
                graph
                    .element(edge.source)
                    .is_some_and(|source| source.layer == 2)
            })
            .count(),
        is_seed: element.id == seed_id,
    }
}

fn include_model_reference_relation(relation: &str) -> bool {
    !matches!(relation, "specializes" | "owner" | "metatype")
}

fn owned_feature_attributes(graph: &Graph, element: &Element) -> Vec<ExplorerAttributeDto> {
    element
        .properties
        .get("features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|feature_id| graph.element_by_element_id(feature_id))
        .filter_map(|feature| {
            explorer_declared_name(feature).map(|name| ExplorerAttributeDto {
                name,
                type_label: explorer_type_label(feature),
            })
        })
        .collect()
}

fn explorer_declared_name(element: &Element) -> Option<String> {
    element
        .properties
        .get("declared_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            element
                .properties
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn explorer_type_label(element: &Element) -> Option<String> {
    relation_type_label(element.properties.get("type"))
}

fn relation_type_label(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(entry)) => Some(label_for_id(entry)),
        Some(Value::Array(entries)) => entries.iter().find_map(|entry| match entry {
            Value::String(item) => Some(label_for_id(item)),
            _ => None,
        }),
        _ => None,
    }
}

fn build_tree_from_graph(
    graph: &Graph,
    include_element: impl Fn(&Element) -> bool,
) -> Vec<LibraryTreeNodeDto> {
    let mut root = TreeNode::root();
    let mut library_elements = graph
        .elements()
        .iter()
        .filter(|element| include_element(element))
        .collect::<Vec<_>>();
    library_elements.sort_by(|left, right| left.element_id.cmp(&right.element_id));

    for element in library_elements {
        root.insert(path_segments_for_tree(&element.element_id), element);
    }

    root.into_children()
}

fn path_segments_for_tree(element_id: &str) -> Vec<String> {
    if element_id.contains("::") {
        element_id.split("::").map(str::to_string).collect()
    } else {
        element_id.split('.').map(str::to_string).collect()
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Default)]
struct TreeNode {
    label: String,
    element_id: Option<String>,
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn root() -> Self {
        Self::default()
    }

    fn insert(&mut self, segments: Vec<String>, element: &Element) {
        if segments.is_empty() {
            return;
        }

        let mut current = self;
        let path_len = segments.len();
        for (index, segment) in segments.into_iter().enumerate() {
            current = current
                .children
                .entry(segment.clone())
                .or_insert_with(|| TreeNode {
                    label: segment,
                    element_id: None,
                    children: BTreeMap::new(),
                });

            if index + 1 == path_len {
                current.element_id = Some(element.element_id.clone());
            }
        }
    }

    fn into_children(self) -> Vec<LibraryTreeNodeDto> {
        self.children
            .into_iter()
            .map(|(key, child)| child.into_dto(key))
            .collect()
    }

    fn into_dto(self, id: String) -> LibraryTreeNodeDto {
        let TreeNode {
            label,
            element_id,
            children,
        } = self;
        let children = children
            .into_iter()
            .map(|(child_id, child)| child.into_dto(child_id))
            .collect::<Vec<_>>();
        let node_type = if element_id.is_some() {
            "element"
        } else {
            "namespace"
        };

        LibraryTreeNodeDto {
            id,
            label,
            node_type: node_type.to_string(),
            element_id,
            child_count: children.len(),
            children,
        }
    }
}

fn metadata_string(document: &KirDocument, key: &str) -> Option<String> {
    document
        .metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            document
                .metadata
                .get("merged_sources")
                .and_then(Value::as_array)
                .and_then(|sources| {
                    sources
                        .iter()
                        .find_map(|source| source.get(key).and_then(Value::as_str))
                })
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::kir::{KirDocument, KirElement};
    use crate::model::{Graph, MetamodelAttributeRegistry};

    use super::{
        GraphScope, MetatypeExplorerRequestDto, ModelExplorerRequestDto,
        document_model_metadata_view, element_details, graph_view, library_tree_view,
        metatype_explorer_view, model_explorer_view, model_metadata_view, search_view,
    };

    #[test]
    fn graph_view_model_plus_context_includes_connected_library_nodes() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "metatype".to_string(),
                        Value::String("Model::Systems::PartDefinition".to_string()),
                    )]),
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

        let model = graph_view(&graph, GraphScope::Model);
        let model_plus_context = graph_view(&graph, GraphScope::ModelPlusContext);

        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model_plus_context.nodes.len(), 2);
        assert_eq!(model_plus_context.edges.len(), 1);
        assert_eq!(model_plus_context.edges[0].relation, "metatype");
    }

    #[test]
    fn model_metadata_view_summarizes_graph_and_merged_source_metadata() {
        let stdlib_document = KirDocument {
            metadata: BTreeMap::from([(
                "merged_sources".to_string(),
                Value::Array(vec![serde_json::json!({"stdlib_version": "test-stdlib"})]),
            )]),
            elements: Vec::new(),
        };
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "metatype".to_string(),
                        Value::String("Model::Systems::PartDefinition".to_string()),
                    )]),
                },
            ],
        })
        .unwrap();

        let metadata = model_metadata_view(&graph, &stdlib_document);

        assert_eq!(metadata.element_count, 2);
        assert_eq!(metadata.edge_count, 1);
        assert_eq!(metadata.library_element_count, 1);
        assert_eq!(metadata.user_element_count, 1);
        assert_eq!(metadata.library_version.as_deref(), Some("test-stdlib"));
        assert_eq!(metadata.layers, vec![1, 2]);
        assert_eq!(metadata.relations, vec!["metatype"]);
        assert_eq!(metadata.default_graph_scope, GraphScope::Model.as_str());
    }

    #[test]
    fn document_model_metadata_view_summarizes_shell_document() {
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "stdlib_version".to_string(),
                Value::String("shell-stdlib".to_string()),
            )]),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Model::Systems::PartUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
            ],
        };

        let metadata = document_model_metadata_view(&document);

        assert_eq!(metadata.element_count, 2);
        assert_eq!(metadata.edge_count, 0);
        assert_eq!(metadata.library_element_count, 2);
        assert_eq!(metadata.user_element_count, 0);
        assert_eq!(metadata.library_version.as_deref(), Some("shell-stdlib"));
        assert_eq!(metadata.layers, vec![1]);
        assert!(metadata.relations.is_empty());
    }

    #[test]
    fn search_view_matches_by_id_kind_and_label() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "feature.Vehicle.engine".to_string(),
                    kind: "Model::Systems::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        let by_label = search_view(&graph, "engine");
        let by_kind = search_view(&graph, "partdefinition");
        let all = search_view(&graph, "  ");

        assert_eq!(by_label.len(), 1);
        assert_eq!(by_label[0].id, "feature.Vehicle.engine");
        assert_eq!(by_kind.len(), 1);
        assert_eq!(by_kind[0].id, "type.Vehicle");
        assert_eq!(
            all.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            vec!["feature.Vehicle.engine", "type.Vehicle"]
        );
    }

    #[test]
    fn element_details_include_effective_properties_and_edges() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "metafeature.Model.PartDefinition.related".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        (
                            "owner".to_string(),
                            Value::String("Model::Systems::PartDefinition".to_string()),
                        ),
                        (
                            "kir_property".to_string(),
                            Value::String("related".to_string()),
                        ),
                        (
                            "feature_kind".to_string(),
                            Value::String("reference".to_string()),
                        ),
                    ]),
                },
                KirElement {
                    id: "type.BaseVehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "mass".to_string(),
                        Value::String("1000 kg".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::Array(vec![Value::String("type.BaseVehicle".to_string())]),
                    )]),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelAttributeRegistry::build(&graph);

        let details = element_details(&graph, &registry, "type.Vehicle").unwrap();

        assert_eq!(details.id, "type.Vehicle");
        assert_eq!(details.label, "Vehicle");
        assert_eq!(details.specialization_chain[0].id, "type.BaseVehicle");
        assert_eq!(
            details.effective_properties.get("mass"),
            Some(&Value::String("1000 kg".to_string()))
        );
        assert_eq!(details.outbound.len(), 1);
        assert_eq!(details.outbound[0].relation, "specializes");
    }

    #[test]
    fn library_tree_view_builds_namespaces_for_library_elements() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Model::Systems::PartUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();

        let tree = library_tree_view(&graph);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, "Model");
        assert_eq!(tree[0].node_type, "namespace");
        let systems = &tree[0].children[0];
        assert_eq!(systems.id, "Systems");
        assert_eq!(systems.child_count, 2);
        assert!(systems.children.iter().any(|child| {
            child.id == "PartDefinition"
                && child.node_type == "element"
                && child.element_id.as_deref() == Some("Model::Systems::PartDefinition")
        }));
    }

    #[test]
    fn metatype_explorer_view_expands_specialization_neighborhood() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::Block".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::Array(vec![Value::String("Model::Core::Type".to_string())]),
                    )]),
                },
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::Array(vec![Value::String("Model::Systems::Block".to_string())]),
                    )]),
                },
                KirElement {
                    id: "Model::Core::Type".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelAttributeRegistry::build(&graph);

        let view = metatype_explorer_view(
            &graph,
            &registry,
            &MetatypeExplorerRequestDto {
                seed_id: "Model::Systems::PartDefinition".to_string(),
                expanded_parents: vec!["Model::Systems::Block".to_string()],
                expanded_children: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(view.seed_id, "Model::Systems::PartDefinition");
        assert!(view.nodes.iter().any(|node| node.id == "Model::Core::Type"));
        assert!(view.edges.iter().any(|edge| {
            edge.source == "Model::Systems::Block" && edge.target == "Model::Core::Type"
        }));
    }

    #[test]
    fn model_explorer_view_includes_owned_feature_attributes_and_reference_edges() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "metafeature.Model.PartDefinition.related".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        (
                            "owner".to_string(),
                            Value::String("Model::Systems::PartDefinition".to_string()),
                        ),
                        (
                            "kir_property".to_string(),
                            Value::String("related".to_string()),
                        ),
                        (
                            "feature_kind".to_string(),
                            Value::String("reference".to_string()),
                        ),
                    ]),
                },
                KirElement {
                    id: "type.BaseVehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "type.Engine".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        Value::Array(vec![Value::String("type.Vehicle".to_string())]),
                    )]),
                },
                KirElement {
                    id: "type.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "specializes".to_string(),
                            Value::Array(vec![Value::String("type.BaseVehicle".to_string())]),
                        ),
                        (
                            "features".to_string(),
                            Value::Array(vec![Value::String("feature.Vehicle.engine".to_string())]),
                        ),
                        (
                            "related".to_string(),
                            Value::String("type.Engine".to_string()),
                        ),
                    ]),
                },
                KirElement {
                    id: "feature.Vehicle.engine".to_string(),
                    kind: "Model::Systems::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("engine".to_string()),
                        ),
                        ("type".to_string(), Value::String("type.Engine".to_string())),
                    ]),
                },
            ],
        })
        .unwrap();

        let view = model_explorer_view(
            &graph,
            &ModelExplorerRequestDto {
                seed_id: "type.Vehicle".to_string(),
                expanded_parents: Vec::new(),
                expanded_children: vec!["type.Vehicle".to_string()],
                include_reference_edges: true,
            },
        )
        .unwrap();

        let vehicle = view
            .nodes
            .iter()
            .find(|node| node.id == "type.Vehicle")
            .unwrap();
        assert!(vehicle.attributes.iter().any(|attribute| {
            attribute.name == "engine" && attribute.type_label.as_deref() == Some("Engine")
        }));
        assert!(view.edges.iter().any(|edge| {
            edge.source == "type.Vehicle"
                && edge.target == "type.Engine"
                && edge.relation == "related"
        }));
    }
}
