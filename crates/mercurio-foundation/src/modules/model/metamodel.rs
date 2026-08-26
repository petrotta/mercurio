use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

use crate::kir::Severity;

use crate::model::derived::derived_properties;
use crate::model::graph::{Graph, NodeId};
use crate::model::ir::{Diagnostic, DiagnosticKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementSummary {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeValueSource {
    pub element: ElementSummary,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeRow {
    pub name: String,
    pub declared_by: Option<ElementSummary>,
    pub type_label: Option<String>,
    pub feature_kind: Option<String>,
    pub multiplicity_lower: Option<String>,
    pub multiplicity_upper: Option<String>,
    pub origin_kind: String,
    pub has_direct_value: bool,
    pub direct_value: Option<Value>,
    pub has_effective_value: bool,
    pub effective_value: Option<Value>,
    pub inherited_values: Vec<AttributeValueSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementAttributeQuery {
    pub metatype: Option<ElementSummary>,
    pub metatype_specialization_chain: Vec<ElementSummary>,
    pub rows: Vec<AttributeRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetatypeQueryOverride {
    pub metatype_key: Option<String>,
    pub specialization_chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetamodelAttributeDeclaration {
    pub name: String,
    pub declared_by: ElementSummary,
    pub type_label: Option<String>,
    pub feature_kind: Option<String>,
    pub multiplicity_lower: Option<String>,
    pub multiplicity_upper: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetamodelClassView {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub metamodel_language: Option<String>,
    pub metamodel_layer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetamodelFeatureView {
    pub id: String,
    pub owner: String,
    pub declared_by: ElementSummary,
    pub declared_name: Option<String>,
    pub kir_property: String,
    pub source_feature: Option<String>,
    pub feature_kind: Option<String>,
    pub type_id: Option<String>,
    pub type_label: Option<String>,
    pub multiplicity_lower: Option<String>,
    pub multiplicity_upper: Option<String>,
    pub metamodel_language: Option<String>,
    pub metamodel_layer: Option<String>,
    pub source_kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct MetamodelFeatureRegistry {
    class_lookup: HashMap<String, String>,
    classes: HashMap<String, MetamodelClassView>,
    declared_features: HashMap<String, Vec<MetamodelFeatureView>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivedMetamodelCapabilities {
    pub class_id: Option<String>,
    pub classifier_like: bool,
    pub feature_like: bool,
    pub namespace_like: bool,
    pub relationship_like: bool,
    pub can_own_features: bool,
    pub can_have_endpoints: bool,
}

pub type MetamodelValidationDiagnostic = Diagnostic;

impl MetamodelFeatureRegistry {
    pub fn build(graph: &Graph) -> Self {
        let mut registry = Self::default();

        for element in graph.elements() {
            let class = metamodel_class_view(element);
            for key in metatype_lookup_keys(&class.id, &class.label) {
                registry
                    .class_lookup
                    .entry(key)
                    .or_insert_with(|| class.id.clone());
            }
            registry.classes.insert(class.id.clone(), class);
        }

        for element in graph.elements() {
            if element.kind.as_ref() == "MetamodelFeature" {
                let Some(feature) = explicit_metamodel_feature_view(graph, element) else {
                    continue;
                };
                registry.insert_feature(feature, true);
                continue;
            }

            for feature_id in owned_feature_ids(element) {
                let Some(feature_node_id) = graph.node_id(&feature_id) else {
                    continue;
                };
                let Some(feature) = graph.element(feature_node_id) else {
                    continue;
                };
                let Some(view) = inferred_metamodel_feature_view(element, feature) else {
                    continue;
                };
                registry.insert_feature(view, false);
            }
        }

        registry
    }

    pub fn class(&self, class_id: &str) -> Option<&MetamodelClassView> {
        let resolved = self.resolve_class_id(class_id)?;
        self.classes.get(&resolved)
    }

    pub fn declared_features_for(&self, class_id: &str) -> &[MetamodelFeatureView] {
        self.resolve_class_id(class_id)
            .and_then(|resolved| self.declared_features.get(&resolved))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn all_features_for(&self, graph: &Graph, class_id: &str) -> Vec<MetamodelFeatureView> {
        let Some(resolved) = self.resolve_class_id(class_id) else {
            return Vec::new();
        };
        let Some(node_id) = graph.node_id(&resolved) else {
            return self.declared_features_for(&resolved).to_vec();
        };

        let mut by_property = BTreeMap::new();
        for feature in self.declared_features_for(&resolved) {
            by_property.insert(feature.kir_property.clone(), feature.clone());
        }
        for ancestor in collect_specialization_ancestors(graph, node_id) {
            for feature in self.declared_features_for(&ancestor.element_id) {
                by_property
                    .entry(feature.kir_property.clone())
                    .or_insert_with(|| feature.clone());
            }
        }
        by_property.into_values().collect()
    }

    pub fn feature_by_property(
        &self,
        graph: &Graph,
        class_id: &str,
        kir_property: &str,
    ) -> Option<MetamodelFeatureView> {
        self.all_features_for(graph, class_id)
            .into_iter()
            .find(|feature| feature.kir_property == kir_property)
    }

    fn resolve_class_id(&self, class_id: &str) -> Option<String> {
        self.class_lookup
            .get(class_id)
            .cloned()
            .or_else(|| {
                self.class_lookup
                    .get(&canonical_identifier(class_id))
                    .cloned()
            })
            .or_else(|| self.class_lookup.get(&label_for_id(class_id)).cloned())
            .or_else(|| {
                self.classes
                    .contains_key(class_id)
                    .then(|| class_id.to_string())
            })
    }

    fn insert_feature(&mut self, feature: MetamodelFeatureView, replace_existing: bool) {
        let entry = self
            .declared_features
            .entry(feature.owner.clone())
            .or_default();
        if let Some(existing) = entry
            .iter_mut()
            .find(|existing| existing.kir_property == feature.kir_property)
        {
            if replace_existing {
                *existing = feature;
            }
        } else {
            entry.push(feature);
        }
    }
}

pub fn derive_metamodel_capabilities(
    graph: &Graph,
    registry: &MetamodelFeatureRegistry,
    node_id: NodeId,
) -> DerivedMetamodelCapabilities {
    let Some(element) = graph.element(node_id) else {
        return DerivedMetamodelCapabilities::default();
    };
    let Some(class_id) = metamodel_class_id_for_element(graph, registry, element) else {
        return DerivedMetamodelCapabilities::default();
    };

    let feature_properties = metamodel_feature_properties(graph, registry, &class_id);
    DerivedMetamodelCapabilities {
        class_id: Some(class_id),
        classifier_like: feature_properties.contains("features")
            || feature_properties.contains("specializes"),
        feature_like: feature_properties.contains("owner")
            || feature_properties.contains("type")
            || feature_properties.contains("definition")
            || feature_properties.contains("featuring_type"),
        namespace_like: feature_properties.contains("members"),
        relationship_like: has_endpoint_feature(&feature_properties),
        can_own_features: feature_properties.contains("features"),
        can_have_endpoints: has_endpoint_feature(&feature_properties),
    }
}

pub fn validate_derived_metamodel_semantics(
    graph: &Graph,
    registry: &MetamodelFeatureRegistry,
) -> Vec<MetamodelValidationDiagnostic> {
    let mut diagnostics = Vec::new();

    for element in graph.elements() {
        if is_metamodel_declaration_element(element) {
            continue;
        }
        let Some(class_id) = metamodel_class_id_for_element(graph, registry, element) else {
            continue;
        };
        if !metamodel_contract_loaded_for_class(graph, registry, &class_id) {
            continue;
        }

        let capabilities = derive_metamodel_capabilities(graph, registry, element.id);
        if element.properties.get("features").is_some() && !capabilities.can_own_features {
            diagnostics.push(
                MetamodelValidationDiagnostic::new(
                    DiagnosticKind::Validation,
                    Severity::Error,
                    "kir.metamodel.features.unsupported",
                    format!(
                    "KIR element {} declares `features`, but its derived metamodel capabilities do not allow feature ownership",
                    element.element_id
                ),
                )
                .with_subject(element.element_id.clone()),
            );
        }

        let declares_source = element.properties.get("source").is_some()
            || element.properties.get("sources").is_some();
        let declares_target = element.properties.get("target").is_some()
            || element.properties.get("targets").is_some();
        if (declares_source || declares_target) && !capabilities.can_have_endpoints {
            diagnostics.push(
                MetamodelValidationDiagnostic::new(
                    DiagnosticKind::Validation,
                    Severity::Error,
                    "kir.metamodel.endpoints.unsupported",
                    format!(
                    "KIR element {} declares relationship endpoints, but its derived metamodel capabilities do not allow endpoints",
                    element.element_id
                ),
                )
                .with_subject(element.element_id.clone()),
            );
        } else if capabilities.can_have_endpoints && declares_source != declares_target {
            diagnostics.push(
                MetamodelValidationDiagnostic::new(
                    DiagnosticKind::Validation,
                    Severity::Error,
                    "kir.metamodel.endpoints.incomplete",
                    format!(
                        "KIR element {} declares only one side of a relationship endpoint pair",
                        element.element_id
                    ),
                )
                .with_subject(element.element_id.clone()),
            );
        }
    }

    diagnostics
}

#[derive(Debug, Clone, Default)]
pub struct MetamodelAttributeRegistry {
    metatype_lookup: HashMap<String, String>,
    metatype_summaries: HashMap<String, ElementSummary>,
    declared_attributes: HashMap<String, Vec<MetamodelAttributeDeclaration>>,
}

impl MetamodelAttributeRegistry {
    pub fn build(graph: &Graph) -> Self {
        let mut registry = Self::default();

        for element in graph.elements() {
            let summary = element_summary(element);
            registry
                .metatype_summaries
                .insert(element.element_id.clone(), summary.clone());
            for key in metatype_lookup_keys(&element.element_id, &summary.label) {
                registry
                    .metatype_lookup
                    .entry(key)
                    .or_insert_with(|| element.element_id.clone());
            }
        }

        for element in graph.elements() {
            if element.kind.as_ref() == "MetamodelFeature" {
                let Some(owner_id) = string_property(element, "owner") else {
                    continue;
                };
                let Some(name) = string_property(element, "kir_property").or_else(|| {
                    string_property(element, "declared_name").map(|name| attribute_query_key(&name))
                }) else {
                    continue;
                };
                let Some(owner) = graph.element_by_element_id(&owner_id) else {
                    continue;
                };
                registry.insert_declaration(
                    owner_id,
                    MetamodelAttributeDeclaration {
                        name,
                        declared_by: element_summary(owner),
                        type_label: string_property(element, "type_label")
                            .or_else(|| relation_value_label(element.properties.get("type"))),
                        feature_kind: string_property(element, "feature_kind"),
                        multiplicity_lower: value_text(element.properties.get("lower")),
                        multiplicity_upper: value_text(element.properties.get("upper")),
                    },
                    true,
                );
                continue;
            }

            let mut declarations = BTreeMap::new();
            for feature_id in owned_feature_ids(element) {
                let Some(feature_node_id) = graph.node_id(&feature_id) else {
                    continue;
                };
                let Some(feature) = graph.element(feature_node_id) else {
                    continue;
                };
                let Some(declared_name) = declared_name(feature) else {
                    continue;
                };
                let query_key = attribute_query_key(&declared_name);
                declarations.entry(query_key.clone()).or_insert_with(|| {
                    MetamodelAttributeDeclaration {
                        name: query_key,
                        declared_by: element_summary(element),
                        type_label: feature_type_label(feature),
                        feature_kind: None,
                        multiplicity_lower: value_text(
                            feature.properties.get("multiplicity_lower"),
                        ),
                        multiplicity_upper: value_text(
                            feature.properties.get("multiplicity_upper"),
                        ),
                    }
                });
            }

            if !declarations.is_empty() {
                for declaration in declarations.into_values() {
                    registry.insert_declaration(element.element_id.clone(), declaration, false);
                }
            }
        }

        registry
    }

    fn resolve_metatype_summary(&self, key: &str) -> Option<ElementSummary> {
        let resolved = self
            .metatype_lookup
            .get(key)
            .cloned()
            .or_else(|| {
                self.metatype_lookup
                    .get(&canonical_identifier(key))
                    .cloned()
            })
            .or_else(|| self.metatype_lookup.get(&label_for_id(key)).cloned())
            .unwrap_or_else(|| key.to_string());
        self.metatype_summaries.get(&resolved).cloned()
    }

    pub fn declared_attributes_for(&self, metatype_id: &str) -> &[MetamodelAttributeDeclaration] {
        self.declared_attributes
            .get(metatype_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn declared_attribute_names_for(&self, metatype_id: &str) -> Vec<String> {
        self.declared_attributes_for(metatype_id)
            .iter()
            .map(|declaration| declaration.name.clone())
            .collect()
    }

    fn insert_declaration(
        &mut self,
        metatype_id: String,
        declaration: MetamodelAttributeDeclaration,
        replace_existing: bool,
    ) {
        let entry = self.declared_attributes.entry(metatype_id).or_default();
        if let Some(existing) = entry
            .iter_mut()
            .find(|existing| existing.name == declaration.name)
        {
            if replace_existing {
                *existing = declaration;
            }
        } else {
            entry.push(declaration);
        }
    }
}

pub fn query_element_attributes(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    node_id: NodeId,
    metatype_override: Option<&MetatypeQueryOverride>,
) -> Option<ElementAttributeQuery> {
    let element = graph.element(node_id)?;
    let ancestors = collect_specialization_ancestors(graph, node_id);
    let derived_properties = derived_properties(graph, element);
    let effective_properties = effective_element_properties_with_derived(
        &ancestors,
        &element.properties,
        &derived_properties,
    );

    let (metatype, metatype_specialization_chain) = if let Some(override_query) = metatype_override
    {
        let metatype = override_query
            .metatype_key
            .as_deref()
            .and_then(|key| registry.resolve_metatype_summary(key));
        let metatype_specialization_chain = override_query
            .specialization_chain
            .iter()
            .filter_map(|key| registry.resolve_metatype_summary(key))
            .collect::<Vec<_>>();
        (metatype, metatype_specialization_chain)
    } else {
        let metatype_element = element_metatype(graph, node_id)
            .or_else(|| ancestors.first().copied())
            .or_else(|| is_metamodel_type(element).then_some(element));
        let metatype = metatype_element.map(element_summary);
        let metatype_specialization_chain = metatype_element
            .map(|item| collect_specialization_ancestors(graph, item.id))
            .unwrap_or_default()
            .into_iter()
            .map(element_summary)
            .collect::<Vec<_>>();
        (metatype, metatype_specialization_chain)
    };

    let mut declarations = BTreeMap::new();
    if let Some(summary) = &metatype {
        for declaration in registry.declared_attributes_for(&summary.id) {
            declarations
                .entry(declaration.name.clone())
                .or_insert_with(|| declaration.clone());
        }
    }
    for summary in &metatype_specialization_chain {
        for declaration in registry.declared_attributes_for(&summary.id) {
            declarations
                .entry(declaration.name.clone())
                .or_insert_with(|| declaration.clone());
        }
    }
    for name in derived_properties.keys() {
        declarations
            .entry(name.clone())
            .or_insert_with(|| MetamodelAttributeDeclaration {
                name: name.clone(),
                declared_by: metatype.clone().unwrap_or_else(|| element_summary(element)),
                type_label: None,
                feature_kind: Some("derived".to_string()),
                multiplicity_lower: None,
                multiplicity_upper: None,
            });
    }

    let rows = declarations
        .into_values()
        .map(|declaration| {
            let direct_value = element.properties.get(&declaration.name).cloned();
            let effective_value = effective_properties.get(&declaration.name).cloned();
            let inherited_values = ancestors
                .iter()
                .filter_map(|ancestor| {
                    ancestor
                        .properties
                        .get(&declaration.name)
                        .cloned()
                        .map(|value| AttributeValueSource {
                            element: element_summary(ancestor),
                            value,
                        })
                })
                .collect::<Vec<_>>();
            let origin_kind = if derived_properties.contains_key(&declaration.name) {
                "derived"
            } else if direct_value.is_some() {
                "direct"
            } else if !inherited_values.is_empty() && effective_value.is_some() {
                "inherited"
            } else if effective_value.is_some() {
                "derived"
            } else {
                "declared"
            };

            AttributeRow {
                name: declaration.name,
                declared_by: Some(declaration.declared_by),
                type_label: declaration.type_label,
                feature_kind: declaration.feature_kind,
                multiplicity_lower: declaration.multiplicity_lower,
                multiplicity_upper: declaration.multiplicity_upper,
                origin_kind: origin_kind.to_string(),
                has_direct_value: direct_value.is_some(),
                direct_value,
                has_effective_value: effective_value.is_some(),
                effective_value,
                inherited_values,
            }
        })
        .collect::<Vec<_>>();

    Some(ElementAttributeQuery {
        metatype,
        metatype_specialization_chain,
        rows,
    })
}

pub fn element_metatype(graph: &Graph, node_id: NodeId) -> Option<&crate::model::graph::Element> {
    graph
        .outgoing(node_id, "metatype")
        .next()
        .and_then(|edge| graph.element(edge.target))
}

fn metamodel_class_id_for_element(
    graph: &Graph,
    registry: &MetamodelFeatureRegistry,
    element: &crate::model::graph::Element,
) -> Option<String> {
    element_metatype(graph, element.id)
        .map(|metatype| metatype.element_id.clone())
        .or_else(|| {
            registry
                .class(element.kind.as_ref())
                .map(|class| class.id.clone())
        })
        .or_else(|| {
            registry
                .class(&element.element_id)
                .map(|class| class.id.clone())
        })
}

fn metamodel_feature_properties(
    graph: &Graph,
    registry: &MetamodelFeatureRegistry,
    class_id: &str,
) -> BTreeSet<String> {
    registry
        .all_features_for(graph, class_id)
        .into_iter()
        .map(|feature| feature.kir_property)
        .collect()
}

fn has_endpoint_feature(feature_properties: &BTreeSet<String>) -> bool {
    (feature_properties.contains("source") && feature_properties.contains("target"))
        || (feature_properties.contains("sources") && feature_properties.contains("targets"))
}

fn metamodel_contract_loaded_for_class(
    graph: &Graph,
    registry: &MetamodelFeatureRegistry,
    class_id: &str,
) -> bool {
    graph
        .element_by_element_id(class_id)
        .is_some_and(is_metamodel_type)
        || registry
            .all_features_for(graph, class_id)
            .iter()
            .any(|feature| feature.source_kind == "explicit")
}

fn is_metamodel_declaration_element(element: &crate::model::graph::Element) -> bool {
    is_metamodel_type(element) || element.kind.as_ref() == "MetamodelFeature"
}

pub fn collect_specialization_ancestors(
    graph: &Graph,
    node_id: NodeId,
) -> Vec<&crate::model::graph::Element> {
    let mut seen = BTreeSet::new();
    let mut ancestors = Vec::new();
    collect_specialization_ancestors_into(graph, node_id, &mut seen, &mut ancestors);
    ancestors
}

pub fn effective_properties(
    ancestors: &[&crate::model::graph::Element],
    direct_properties: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    effective_properties_with_derived(ancestors, direct_properties, &BTreeMap::new())
}

pub fn effective_properties_with_derived(
    ancestors: &[&crate::model::graph::Element],
    direct_properties: &BTreeMap<String, Value>,
    derived_properties: &BTreeMap<String, crate::model::derived::DerivedPropertyValue>,
) -> BTreeMap<String, Value> {
    let mut effective = BTreeMap::new();
    for ancestor in ancestors.iter().rev() {
        for (key, value) in &ancestor.properties {
            effective.insert(key.to_string(), value.clone());
        }
    }
    for (key, value) in direct_properties {
        effective.insert(key.clone(), value.clone());
    }
    for (key, value) in derived_properties {
        effective.insert(key.clone(), value.value.clone());
    }
    effective
}

pub fn effective_element_properties_with_derived(
    ancestors: &[&crate::model::graph::Element],
    direct_properties: &crate::model::graph::ElementProperties,
    derived_properties: &BTreeMap<String, crate::model::derived::DerivedPropertyValue>,
) -> BTreeMap<String, Value> {
    let mut effective = BTreeMap::new();
    for ancestor in ancestors.iter().rev() {
        for (key, value) in &ancestor.properties {
            effective.insert(key.to_string(), value.clone());
        }
    }
    for (key, value) in direct_properties {
        effective.insert(key.to_string(), value.clone());
    }
    for (key, value) in derived_properties {
        effective.insert(key.clone(), value.value.clone());
    }
    effective
}

fn collect_specialization_ancestors_into<'a>(
    graph: &'a Graph,
    node_id: NodeId,
    seen: &mut BTreeSet<NodeId>,
    ancestors: &mut Vec<&'a crate::model::graph::Element>,
) {
    let parent_ids = graph
        .outgoing(node_id, "specializes")
        .map(|edge| edge.target)
        .collect::<Vec<_>>();

    for parent_id in parent_ids {
        if !seen.insert(parent_id) {
            continue;
        }

        if let Some(parent) = graph.element(parent_id) {
            ancestors.push(parent);
            collect_specialization_ancestors_into(graph, parent_id, seen, ancestors);
        }
    }
}

fn owned_feature_ids(element: &crate::model::graph::Element) -> Vec<String> {
    element
        .properties
        .get("features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn metamodel_class_view(element: &crate::model::graph::Element) -> MetamodelClassView {
    MetamodelClassView {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
        metamodel_language: string_property(element, "metamodel_language")
            .or_else(|| pilot_library_group(element).map(metamodel_language_for_group)),
        metamodel_layer: string_property(element, "metamodel_layer")
            .or_else(|| pilot_library_group(element).map(metamodel_layer_for_group)),
    }
}

fn explicit_metamodel_feature_view(
    graph: &Graph,
    element: &crate::model::graph::Element,
) -> Option<MetamodelFeatureView> {
    let owner = string_property(element, "owner")?;
    let owner_element = graph.element_by_element_id(&owner)?;
    let kir_property = string_property(element, "kir_property").or_else(|| {
        string_property(element, "declared_name").map(|name| attribute_query_key(&name))
    })?;
    let type_id = string_property(element, "type");

    Some(MetamodelFeatureView {
        id: element.element_id.clone(),
        owner,
        declared_by: element_summary(owner_element),
        declared_name: string_property(element, "declared_name"),
        kir_property,
        source_feature: string_property(element, "source_feature"),
        feature_kind: string_property(element, "feature_kind"),
        type_label: string_property(element, "type_label")
            .or_else(|| type_id.as_deref().map(label_for_id)),
        type_id,
        multiplicity_lower: value_text(element.properties.get("lower")),
        multiplicity_upper: value_text(element.properties.get("upper")),
        metamodel_language: string_property(element, "metamodel_language"),
        metamodel_layer: string_property(element, "metamodel_layer"),
        source_kind: "explicit".to_string(),
    })
}

fn inferred_metamodel_feature_view(
    owner: &crate::model::graph::Element,
    feature: &crate::model::graph::Element,
) -> Option<MetamodelFeatureView> {
    let declared_name = declared_name(feature)?;
    let kir_property = attribute_query_key(&declared_name);
    let type_id = relation_value_string(feature.properties.get("type"));

    Some(MetamodelFeatureView {
        id: format!("inferred.{}.{}", owner.element_id, kir_property),
        owner: owner.element_id.clone(),
        declared_by: element_summary(owner),
        declared_name: Some(declared_name),
        kir_property,
        source_feature: Some(feature.element_id.clone()),
        feature_kind: None,
        type_label: feature_type_label(feature),
        type_id,
        multiplicity_lower: value_text(feature.properties.get("multiplicity_lower")),
        multiplicity_upper: value_text(feature.properties.get("multiplicity_upper")),
        metamodel_language: pilot_library_group(feature)
            .or_else(|| pilot_library_group(owner))
            .map(metamodel_language_for_group),
        metamodel_layer: pilot_library_group(feature)
            .or_else(|| pilot_library_group(owner))
            .map(metamodel_layer_for_group),
        source_kind: "inferred".to_string(),
    })
}

fn declared_name(element: &crate::model::graph::Element) -> Option<String> {
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
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("declared_name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn pilot_library_group(element: &crate::model::graph::Element) -> Option<&str> {
    element
        .properties
        .get("pilot_library_group")
        .and_then(Value::as_str)
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("pilot_library_group"))
                .and_then(Value::as_str)
        })
}

fn metamodel_language_for_group(group: &str) -> String {
    match group {
        "Kernel Libraries" => "core",
        "Systems Library" | "Domain Libraries" => "model",
        _ => "unknown",
    }
    .to_string()
}

fn metamodel_layer_for_group(group: &str) -> String {
    match group {
        "Kernel Libraries" => "kernel",
        "Systems Library" => "systems",
        "Domain Libraries" => "domain",
        _ => "unknown",
    }
    .to_string()
}

fn feature_type_label(element: &crate::model::graph::Element) -> Option<String> {
    relation_value_label(element.properties.get("type"))
}

fn string_property(element: &crate::model::graph::Element, key: &str) -> Option<String> {
    element
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn relation_value_label(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(entry)) => Some(label_for_id(entry)),
        Some(Value::Array(entries)) => entries.iter().find_map(|entry| match entry {
            Value::String(item) => Some(label_for_id(item)),
            _ => None,
        }),
        _ => None,
    }
}

fn relation_value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(entry)) => Some(entry.clone()),
        Some(Value::Array(entries)) => entries.iter().find_map(|entry| match entry {
            Value::String(item) => Some(item.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn metatype_lookup_keys(id: &str, label: &str) -> Vec<String> {
    let mut keys = BTreeSet::new();
    keys.insert(id.to_string());
    keys.insert(canonical_identifier(id));
    keys.insert(label.to_string());
    keys.into_iter().collect()
}

fn attribute_query_key(declared_name: &str) -> String {
    match declared_name {
        "ownedFeature" => "features".to_string(),
        "ownedMember" => "members".to_string(),
        "ownedSpecialization" => "specializes".to_string(),
        "documentation" => "doc".to_string(),
        "declaredName" => "declared_name".to_string(),
        "declaredShortName" => "declared_short_name".to_string(),
        "shortName" => "short_name".to_string(),
        "isLibraryElement" => "is_library_element".to_string(),
        "isAbstract" => "is_abstract".to_string(),
        "isDerived" => "is_derived".to_string(),
        "isEnd" => "is_end".to_string(),
        "isOrdered" => "is_ordered".to_string(),
        "isUnique" => "is_unique".to_string(),
        "isVariable" => "is_variable".to_string(),
        "isImplied" => "is_implied".to_string(),
        "featuringType" => "featuring_type".to_string(),
        "chainingFeature" => "chaining_feature".to_string(),
        other => to_snake_case(other),
    }
}

fn is_metamodel_type(element: &crate::model::graph::Element) -> bool {
    matches!(element.kind.as_ref(), "Metaclass" | "MetadataDefinition")
}

fn to_snake_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

fn element_summary(element: &crate::model::graph::Element) -> ElementSummary {
    ElementSummary {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
    }
}

fn canonical_identifier(value: &str) -> String {
    let tail = value.rsplit("::").next().unwrap_or(value);
    let tail = tail.rsplit('.').next().unwrap_or(tail);
    tail.trim_matches('\'').to_string()
}

fn label_for_id(value: &str) -> String {
    canonical_identifier(value)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::*;
    use crate::model::ir::{KirDocument, KirElement};

    #[test]
    fn derived_properties_are_effective_rows_without_direct_values() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.generated.1".to_string(),
                    kind: "Model::Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("Demo".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.generated.2".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("Vehicle".to_string()),
                        ),
                        (
                            "owner".to_string(),
                            Value::String("pkg.generated.1".to_string()),
                        ),
                    ]),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelAttributeRegistry::build(&graph);
        let node_id = graph.node_id("type.generated.2").unwrap();
        let query = query_element_attributes(&graph, &registry, node_id, None).unwrap();

        let qualified_name = query
            .rows
            .iter()
            .find(|row| row.name == "qualifiedName")
            .unwrap();
        assert_eq!(qualified_name.origin_kind, "derived");
        assert!(!qualified_name.has_direct_value);
        assert_eq!(
            qualified_name.effective_value,
            Some(Value::String("Demo::Vehicle".to_string()))
        );

        let name = query.rows.iter().find(|row| row.name == "name").unwrap();
        assert_eq!(name.origin_kind, "derived");
        assert_eq!(name.direct_value, None);
        assert_eq!(
            name.effective_value,
            Some(Value::String("Vehicle".to_string()))
        );

        let effective = effective_element_properties_with_derived(
            &collect_specialization_ancestors(&graph, node_id),
            &graph.element(node_id).unwrap().properties,
            &crate::model::derived::derived_properties(&graph, graph.element(node_id).unwrap()),
        );
        assert_eq!(
            effective.get("qualifiedName"),
            Some(&Value::String("Demo::Vehicle".to_string()))
        );
        assert!(
            !graph
                .element(node_id)
                .unwrap()
                .properties
                .contains_key("qualifiedName")
        );
    }

    #[test]
    fn direct_values_win_over_derived_values_in_effective_view() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.generated.1".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([
                    (
                        "declared_name".to_string(),
                        Value::String("Vehicle".to_string()),
                    ),
                    ("name".to_string(), Value::String("Stale".to_string())),
                ]),
            }],
        })
        .unwrap();
        let element = graph.element_by_element_id("type.generated.1").unwrap();
        let effective = effective_element_properties_with_derived(
            &[],
            &element.properties,
            &crate::model::derived::derived_properties(&graph, element),
        );

        assert_eq!(effective.get("name"), Some(&json!("Stale")));
    }

    #[test]
    fn registry_prefers_explicit_kir_metamodel_feature_facts() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["Model::Systems::PartUsage::partDefinition"]),
                    )]),
                },
                KirElement {
                    id: "Model::Systems::PartUsage::partDefinition".to_string(),
                    kind: "Feature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("partDefinition")),
                        ("type".to_string(), json!("Model::Systems::PartDefinition")),
                    ]),
                },
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "metafeature.Model::Systems::PartUsage.part_definition".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("Model::Systems::PartUsage")),
                        ("kir_property".to_string(), json!("part_definition")),
                        ("feature_kind".to_string(), json!("reference")),
                        ("type".to_string(), json!("Model::Systems::PartDefinition")),
                        ("type_label".to_string(), json!("PartDefinition")),
                        ("lower".to_string(), json!(0)),
                        ("upper".to_string(), json!(1)),
                    ]),
                },
            ],
        })
        .unwrap();

        let registry = MetamodelAttributeRegistry::build(&graph);
        let declarations = registry.declared_attributes_for("Model::Systems::PartUsage");
        let part_definition = declarations
            .iter()
            .find(|declaration| declaration.name == "part_definition")
            .unwrap();

        assert_eq!(
            part_definition.type_label.as_deref(),
            Some("PartDefinition")
        );
        assert_eq!(part_definition.feature_kind.as_deref(), Some("reference"));
        assert_eq!(part_definition.multiplicity_lower.as_deref(), Some("0"));
        assert_eq!(part_definition.multiplicity_upper.as_deref(), Some("1"));
    }

    #[test]
    fn feature_registry_traverses_kernel_and_systems_layers() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Core::Core::Feature".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 0,
                    properties: BTreeMap::from([(
                        "pilot_library_group".to_string(),
                        json!("Kernel Libraries"),
                    )]),
                },
                KirElement {
                    id: "Model::Systems::Usage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("pilot_library_group".to_string(), json!("Systems Library")),
                        ("specializes".to_string(), json!(["Core::Core::Feature"])),
                    ]),
                },
                KirElement {
                    id: "metafeature.Core::Core::Feature.type".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 0,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("Core::Core::Feature")),
                        ("declared_name".to_string(), json!("type")),
                        ("kir_property".to_string(), json!("type")),
                        (
                            "source_feature".to_string(),
                            json!("Core::Core::Feature::type"),
                        ),
                        ("feature_kind".to_string(), json!("reference")),
                        ("type".to_string(), json!("Core::Core::Type")),
                        ("type_label".to_string(), json!("Type")),
                        ("metamodel_language".to_string(), json!("core")),
                        ("metamodel_layer".to_string(), json!("kernel")),
                    ]),
                },
                KirElement {
                    id: "metafeature.Model::Systems::Usage.definition".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("Model::Systems::Usage")),
                        ("declared_name".to_string(), json!("definition")),
                        ("kir_property".to_string(), json!("definition")),
                        (
                            "source_feature".to_string(),
                            json!("Model::Systems::Usage::definition"),
                        ),
                        ("feature_kind".to_string(), json!("reference")),
                        ("type".to_string(), json!("Core::Core::Classifier")),
                        ("type_label".to_string(), json!("Classifier")),
                        ("metamodel_language".to_string(), json!("model")),
                        ("metamodel_layer".to_string(), json!("systems")),
                    ]),
                },
            ],
        })
        .unwrap();

        let registry = MetamodelFeatureRegistry::build(&graph);
        let usage = registry.class("Usage").unwrap();
        assert_eq!(usage.id, "Model::Systems::Usage");
        assert_eq!(usage.metamodel_language.as_deref(), Some("model"));
        assert_eq!(usage.metamodel_layer.as_deref(), Some("systems"));

        let features = registry.all_features_for(&graph, "Model::Systems::Usage");
        let definition = features
            .iter()
            .find(|feature| feature.kir_property == "definition")
            .unwrap();
        let inherited_type = features
            .iter()
            .find(|feature| feature.kir_property == "type")
            .unwrap();

        assert_eq!(definition.owner, "Model::Systems::Usage");
        assert_eq!(definition.metamodel_language.as_deref(), Some("model"));
        assert_eq!(inherited_type.owner, "Core::Core::Feature");
        assert_eq!(inherited_type.metamodel_language.as_deref(), Some("core"));
        assert_eq!(inherited_type.metamodel_layer.as_deref(), Some("kernel"));
    }

    #[test]
    fn derived_metamodel_capabilities_detect_incomplete_relationship_endpoints() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "SysML::Systems::TransitionUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                metamodel_feature(
                    "metafeature.TransitionUsage.source",
                    "SysML::Systems::TransitionUsage",
                    "source",
                ),
                metamodel_feature(
                    "metafeature.TransitionUsage.target",
                    "SysML::Systems::TransitionUsage",
                    "target",
                ),
                KirElement {
                    id: "transition.Demo.start".to_string(),
                    kind: "SysML::Systems::TransitionUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "source".to_string(),
                        json!("state.Demo.Initial"),
                    )]),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelFeatureRegistry::build(&graph);
        let transition = graph.node_id("transition.Demo.start").unwrap();

        let capabilities = derive_metamodel_capabilities(&graph, &registry, transition);
        assert_eq!(
            capabilities.class_id.as_deref(),
            Some("SysML::Systems::TransitionUsage")
        );
        assert!(capabilities.relationship_like);
        assert!(capabilities.can_have_endpoints);

        let diagnostics = validate_derived_metamodel_semantics(&graph, &registry);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "kir.metamodel.endpoints.incomplete");
        assert_eq!(diagnostics[0].subjects, ["transition.Demo.start"]);
    }

    #[test]
    fn derived_metamodel_validation_rejects_feature_ownership_without_capability() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PlainElement".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "plain.Demo.Element".to_string(),
                    kind: "Model::Systems::PlainElement".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["feature.Demo.Element.mass"]),
                    )]),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelFeatureRegistry::build(&graph);

        let diagnostics = validate_derived_metamodel_semantics(&graph, &registry);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "kir.metamodel.features.unsupported");
        assert_eq!(diagnostics[0].subjects, ["plain.Demo.Element"]);
    }

    fn metamodel_feature(id: &str, owner: &str, kir_property: &str) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties: BTreeMap::from([
                ("owner".to_string(), json!(owner)),
                ("kir_property".to_string(), json!(kir_property)),
                ("feature_kind".to_string(), json!("reference")),
            ]),
        }
    }
}
