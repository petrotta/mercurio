use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::kir::{KirDocument, KirElement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotExportDocument {
    #[serde(default)]
    pub metadata: Option<Value>,
    pub elements: Vec<PilotExportElement>,
    #[serde(default)]
    pub relationships: Vec<PilotExportRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotExportElement {
    pub qualified_name: String,
    pub kind: String,
    pub library_group: String,
    #[serde(default)]
    pub source: Option<PilotSource>,
    #[serde(default)]
    pub documentation: Vec<PilotDocumentationBlock>,
    #[serde(default)]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotSource {
    pub file: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotDocumentationBlock {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotExportRelationship {
    pub source: String,
    pub relation: String,
    pub target: String,
}

#[derive(Debug)]
pub enum PilotImportError {
    Io(std::io::Error),
    Json(serde_json::Error),
    DuplicateElement(String),
    UnknownElement(String),
    UnknownLibraryGroup(String),
}

impl fmt::Display for PilotImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read pilot export: {err}"),
            Self::Json(err) => write!(f, "failed to parse pilot export: {err}"),
            Self::DuplicateElement(id) => write!(f, "duplicate pilot export element: {id}"),
            Self::UnknownElement(id) => write!(f, "pilot export references unknown element: {id}"),
            Self::UnknownLibraryGroup(group) => write!(f, "unknown pilot library group: {group}"),
        }
    }
}

impl std::error::Error for PilotImportError {}

impl From<std::io::Error> for PilotImportError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for PilotImportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn load_pilot_export(path: &Path) -> Result<PilotExportDocument, PilotImportError> {
    let input = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&input)?)
}

pub fn normalize_pilot_export(
    export: PilotExportDocument,
) -> Result<KirDocument, PilotImportError> {
    let mut known_ids = BTreeSet::new();
    let mut elements = BTreeMap::new();

    for element in export.elements {
        if !known_ids.insert(element.qualified_name.clone()) {
            return Err(PilotImportError::DuplicateElement(element.qualified_name));
        }

        let mut properties = BTreeMap::new();
        let mut metadata = Map::new();
        metadata.insert(
            "pilot_library_group".to_string(),
            Value::String(element.library_group.clone()),
        );
        for (key, value) in element.properties {
            metadata.insert(key, value);
        }

        if let Some(source) = element.source {
            metadata.insert("source_file".to_string(), Value::String(source.file));
            let mut span = Map::new();
            if let Some(start_line) = source.start_line {
                span.insert("start_line".to_string(), json!(start_line));
            }
            if let Some(end_line) = source.end_line {
                span.insert("end_line".to_string(), json!(end_line));
            }
            if !span.is_empty() {
                metadata.insert("source_span".to_string(), Value::Object(span));
            }
        }

        if !element.documentation.is_empty() {
            let blocks = element
                .documentation
                .into_iter()
                .map(|block| {
                    json!({
                        "kind": block.kind,
                        "text": block.text,
                    })
                })
                .collect::<Vec<_>>();
            properties.insert(
                "doc".to_string(),
                json!({
                    "source": "pilot",
                    "blocks": blocks,
                }),
            );
        }

        if !metadata.is_empty() {
            properties.insert("metadata".to_string(), Value::Object(metadata));
        }

        elements.insert(
            element.qualified_name.clone(),
            KirElement {
                id: element.qualified_name,
                kind: element.kind,
                layer: layer_for_group(&element.library_group)?,
                properties,
            },
        );
    }

    for relationship in export.relationships {
        if !known_ids.contains(relationship.source.as_str()) {
            return Err(PilotImportError::UnknownElement(relationship.source));
        }
        if !known_ids.contains(relationship.target.as_str()) {
            return Err(PilotImportError::UnknownElement(relationship.target));
        }

        let element = elements
            .get_mut(&relationship.source)
            .ok_or_else(|| PilotImportError::UnknownElement(relationship.source.clone()))?;
        push_relation(
            &mut element.properties,
            &relationship.relation,
            relationship.target,
        );
    }

    synthesize_metamodel_features(&mut elements);

    Ok(KirDocument {
        metadata: BTreeMap::new(),
        elements: elements.into_values().collect(),
    })
}

pub fn normalize_pilot_export_for_compare(
    export: PilotExportDocument,
) -> Result<KirDocument, PilotImportError> {
    let mut known_ids = BTreeSet::new();
    let mut elements = BTreeMap::new();

    for element in export.elements {
        if !known_ids.insert(element.qualified_name.clone()) {
            return Err(PilotImportError::DuplicateElement(element.qualified_name));
        }

        let mut properties = element.properties;
        if let Some(source) = element.source {
            let mut metadata = Map::new();
            metadata.insert("source_file".to_string(), Value::String(source.file));
            let mut span = Map::new();
            if let Some(start_line) = source.start_line {
                span.insert("start_line".to_string(), json!(start_line));
            }
            if let Some(end_line) = source.end_line {
                span.insert("end_line".to_string(), json!(end_line));
            }
            if !span.is_empty() {
                metadata.insert("source_span".to_string(), Value::Object(span));
            }
            properties.insert("metadata".to_string(), Value::Object(metadata));
        }

        if !element.documentation.is_empty() {
            let blocks = element
                .documentation
                .into_iter()
                .map(|block| {
                    json!({
                        "kind": block.kind,
                        "text": block.text,
                    })
                })
                .collect::<Vec<_>>();
            properties.insert(
                "doc".to_string(),
                json!({
                    "source": "pilot",
                    "blocks": blocks,
                }),
            );
        }

        elements.insert(
            element.qualified_name.clone(),
            KirElement {
                id: element.qualified_name,
                kind: element.kind,
                layer: compare_layer_for_group(&element.library_group)?,
                properties,
            },
        );
    }

    for relationship in export.relationships {
        if !known_ids.contains(relationship.source.as_str()) {
            return Err(PilotImportError::UnknownElement(relationship.source));
        }
        if !known_ids.contains(relationship.target.as_str()) {
            return Err(PilotImportError::UnknownElement(relationship.target));
        }

        let element = elements
            .get_mut(&relationship.source)
            .ok_or_else(|| PilotImportError::UnknownElement(relationship.source.clone()))?;
        push_relation(
            &mut element.properties,
            &relationship.relation,
            relationship.target,
        );
    }

    lift_compare_relationship_semantics(&mut elements);

    Ok(KirDocument {
        metadata: BTreeMap::new(),
        elements: elements.into_values().collect(),
    })
}

fn lift_compare_relationship_semantics(elements: &mut BTreeMap<String, KirElement>) {
    let relationship_elements = elements.values().cloned().collect::<Vec<_>>();
    for relationship in relationship_elements {
        match relationship.kind.as_str() {
            "FeatureTyping" => {
                let feature_ids = relation_targets(
                    &relationship.properties,
                    &["typed_feature", "owning_feature"],
                );
                let general_ids = relation_targets(&relationship.properties, &["general"]);
                for feature_id in feature_ids {
                    if let Some(feature) = elements.get_mut(&feature_id) {
                        for general_id in &general_ids {
                            push_relation(&mut feature.properties, "type", general_id.clone());
                            push_relation(
                                &mut feature.properties,
                                "specializes",
                                general_id.clone(),
                            );
                        }
                    }
                }
            }
            "Redefinition" => {
                let redefining_ids = relation_targets(
                    &relationship.properties,
                    &["redefining_feature", "owning_feature"],
                );
                let redefined_ids =
                    relation_targets(&relationship.properties, &["redefined_feature", "general"]);
                for redefining_id in redefining_ids {
                    if let Some(feature) = elements.get_mut(&redefining_id) {
                        for redefined_id in &redefined_ids {
                            push_relation(
                                &mut feature.properties,
                                "redefines",
                                redefined_id.clone(),
                            );
                            push_relation(
                                &mut feature.properties,
                                "specializes",
                                redefined_id.clone(),
                            );
                        }
                    }
                }
            }
            "Subsetting" => {
                let subsetting_ids = relation_targets(
                    &relationship.properties,
                    &["subsetting_feature", "owning_feature"],
                );
                let subsetted_ids =
                    relation_targets(&relationship.properties, &["subsetted_feature", "general"]);
                for subsetting_id in subsetting_ids {
                    if let Some(feature) = elements.get_mut(&subsetting_id) {
                        for subsetted_id in &subsetted_ids {
                            push_relation(&mut feature.properties, "subsets", subsetted_id.clone());
                            push_relation(
                                &mut feature.properties,
                                "specializes",
                                subsetted_id.clone(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn relation_targets(properties: &BTreeMap<String, Value>, keys: &[&str]) -> Vec<String> {
    let mut targets = Vec::new();
    for key in keys {
        let Some(value) = properties.get(*key) else {
            continue;
        };
        match value {
            Value::String(single) => targets.push(single.clone()),
            Value::Array(values) => {
                for value in values {
                    if let Value::String(target) = value {
                        targets.push(target.clone());
                    }
                }
            }
            _ => {}
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

fn synthesize_metamodel_features(elements: &mut BTreeMap<String, KirElement>) {
    let owner_feature_pairs = elements
        .iter()
        .filter(|(_, element)| element.layer < 2)
        .flat_map(|(owner_id, element)| {
            relation_targets(&element.properties, &["features"])
                .into_iter()
                .map(|feature_id| (owner_id.clone(), feature_id))
        })
        .collect::<Vec<_>>();

    for (owner_id, feature_id) in owner_feature_pairs {
        let Some(feature) = elements.get(&feature_id) else {
            continue;
        };
        let Some(raw_name) = declared_name_from_properties(&feature.properties) else {
            continue;
        };
        let kir_property = metamodel_feature_property_name(&raw_name);
        let id = format!("metafeature.{owner_id}.{kir_property}");
        if elements.contains_key(&id) {
            continue;
        }

        let feature_type = first_relation_target(&feature.properties, "type");
        let mut properties = BTreeMap::from([
            ("declared_name".to_string(), Value::String(raw_name)),
            ("owner".to_string(), Value::String(owner_id.clone())),
            (
                "source_feature".to_string(),
                Value::String(feature_id.clone()),
            ),
            (
                "kir_property".to_string(),
                Value::String(kir_property.clone()),
            ),
            (
                "feature_kind".to_string(),
                Value::String(metamodel_feature_kind(&feature.kind).to_string()),
            ),
        ]);
        if let Some(group) = pilot_library_group(feature) {
            properties.insert(
                "pilot_library_group".to_string(),
                Value::String(group.to_string()),
            );
            properties.insert(
                "metamodel_language".to_string(),
                Value::String(metamodel_language_for_group(group).to_string()),
            );
            properties.insert(
                "metamodel_layer".to_string(),
                Value::String(metamodel_layer_for_group(group).to_string()),
            );
        }
        if let Some(feature_type) = feature_type {
            properties.insert("type".to_string(), Value::String(feature_type.clone()));
            properties.insert(
                "type_label".to_string(),
                Value::String(label_for_id(&feature_type)),
            );
        }
        copy_if_present(
            &feature.properties,
            &mut properties,
            "multiplicity_lower",
            "lower",
        );
        copy_if_present(
            &feature.properties,
            &mut properties,
            "multiplicity_upper",
            "upper",
        );
        copy_if_present(
            &feature.properties,
            &mut properties,
            "is_derived",
            "is_derived",
        );
        copy_if_present(
            &feature.properties,
            &mut properties,
            "is_ordered",
            "is_ordered",
        );
        copy_if_present(
            &feature.properties,
            &mut properties,
            "is_unique",
            "is_unique",
        );

        elements.insert(
            id.clone(),
            KirElement {
                id,
                kind: "MetamodelFeature".to_string(),
                layer: feature.layer,
                properties,
            },
        );
    }
}

fn declared_name_from_properties(properties: &BTreeMap<String, Value>) -> Option<String> {
    properties
        .get("declared_name")
        .and_then(Value::as_str)
        .or_else(|| properties.get("name").and_then(Value::as_str))
        .or_else(|| {
            properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("declared_name"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("name"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn first_relation_target(properties: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    match properties.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Array(values)) => values.iter().find_map(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn pilot_library_group(element: &KirElement) -> Option<&str> {
    element
        .properties
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("pilot_library_group"))
        .and_then(Value::as_str)
}

fn metamodel_language_for_group(group: &str) -> &'static str {
    match group {
        "Kernel Libraries" => "core",
        "Systems Library" | "Domain Libraries" => "model",
        _ => "unknown",
    }
}

fn metamodel_layer_for_group(group: &str) -> &'static str {
    match group {
        "Kernel Libraries" => "kernel",
        "Systems Library" => "systems",
        "Domain Libraries" => "domain",
        _ => "unknown",
    }
}

fn metamodel_feature_kind(element_kind: &str) -> &'static str {
    match element_kind {
        "AttributeUsage" | "AttributeDefinition" => "attribute",
        _ => "reference",
    }
}

fn metamodel_feature_property_name(declared_name: &str) -> String {
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

fn copy_if_present(
    source: &BTreeMap<String, Value>,
    target: &mut BTreeMap<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = source.get(source_key).or_else(|| {
        source
            .get("metadata")
            .and_then(Value::as_object)?
            .get(source_key)
    }) {
        target.insert(target_key.to_string(), value.clone());
    }
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

fn label_for_id(value: &str) -> String {
    value
        .rsplit("::")
        .next()
        .unwrap_or(value)
        .rsplit('.')
        .next()
        .unwrap_or(value)
        .trim_matches('\'')
        .to_string()
}

fn layer_for_group(group: &str) -> Result<u8, PilotImportError> {
    match group {
        "Kernel Libraries" => Ok(0),
        "Systems Library" | "Domain Libraries" => Ok(1),
        other => Err(PilotImportError::UnknownLibraryGroup(other.to_string())),
    }
}

fn compare_layer_for_group(group: &str) -> Result<u8, PilotImportError> {
    match group {
        "Input Model" => Ok(2),
        other => layer_for_group(other),
    }
}

fn push_relation(properties: &mut BTreeMap<String, Value>, relation: &str, target: String) {
    match properties.get_mut(relation) {
        Some(Value::Array(values)) => values.push(Value::String(target)),
        Some(existing) => {
            let previous = existing.take();
            *existing = Value::Array(vec![previous, Value::String(target)]);
        }
        None => {
            properties.insert(
                relation.to_string(),
                Value::Array(vec![Value::String(target)]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{
        PilotDocumentationBlock, PilotExportDocument, PilotExportElement, PilotExportRelationship,
        PilotSource, normalize_pilot_export, normalize_pilot_export_for_compare,
    };
    fn sample_export() -> PilotExportDocument {
        PilotExportDocument {
            metadata: None,
            elements: vec![PilotExportElement {
                qualified_name: "Anything".to_string(),
                kind: "Type".to_string(),
                library_group: "Kernel Libraries".to_string(),
                source: Some(PilotSource {
                    file: "model.library/Kernel Libraries/Root.model".to_string(),
                    start_line: Some(1),
                    end_line: Some(3),
                }),
                documentation: vec![PilotDocumentationBlock {
                    kind: "comment".to_string(),
                    text: "top level generalized type".to_string(),
                }],
                properties: BTreeMap::new(),
            }],
            relationships: Vec::new(),
        }
    }

    #[test]
    fn normalizes_pilot_export_into_kir_elements() {
        let normalized = normalize_pilot_export(sample_export()).unwrap();

        assert_eq!(normalized.elements.len(), 1);
        assert_eq!(normalized.elements[0].id, "Anything");
        assert_eq!(normalized.elements[0].layer, 0);
    }

    #[test]
    fn preserves_documentation_as_non_semantic_metadata() {
        let normalized = normalize_pilot_export(sample_export()).unwrap();
        let anything = normalized
            .elements
            .iter()
            .find(|element| element.id == "Anything")
            .unwrap();

        assert_eq!(anything.layer, 0);
        assert_eq!(
            anything.properties["metadata"]["pilot_library_group"],
            "Kernel Libraries"
        );
        assert_eq!(anything.properties["doc"]["source"], "pilot");
        assert!(
            anything.properties["doc"]["blocks"][0]["text"]
                .as_str()
                .unwrap()
                .contains("top level generalized type")
        );
    }

    #[test]
    fn normalizes_owned_features_as_explicit_metamodel_feature_facts() {
        let export = PilotExportDocument {
            metadata: None,
            elements: vec![
                PilotExportElement {
                    qualified_name: "Kernel::Element".to_string(),
                    kind: "Metaclass".to_string(),
                    library_group: "Kernel Libraries".to_string(),
                    source: None,
                    documentation: vec![],
                    properties: BTreeMap::from([("declared_name".to_string(), json!("Element"))]),
                },
                PilotExportElement {
                    qualified_name: "Kernel::Element::declaredName".to_string(),
                    kind: "AttributeUsage".to_string(),
                    library_group: "Kernel Libraries".to_string(),
                    source: None,
                    documentation: vec![],
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("declaredName")),
                        ("type".to_string(), json!("Kernel::String")),
                        ("multiplicity_lower".to_string(), json!(0)),
                        ("multiplicity_upper".to_string(), json!(1)),
                    ]),
                },
                PilotExportElement {
                    qualified_name: "Kernel::String".to_string(),
                    kind: "DataType".to_string(),
                    library_group: "Kernel Libraries".to_string(),
                    source: None,
                    documentation: vec![],
                    properties: BTreeMap::from([("declared_name".to_string(), json!("String"))]),
                },
            ],
            relationships: vec![
                PilotExportRelationship {
                    source: "Kernel::Element".to_string(),
                    relation: "features".to_string(),
                    target: "Kernel::Element::declaredName".to_string(),
                },
                PilotExportRelationship {
                    source: "Kernel::Element::declaredName".to_string(),
                    relation: "type".to_string(),
                    target: "Kernel::String".to_string(),
                },
            ],
        };

        let normalized = normalize_pilot_export(export).unwrap();
        let feature = normalized
            .elements
            .iter()
            .find(|element| element.id == "metafeature.Kernel::Element.declared_name")
            .unwrap();

        assert_eq!(feature.kind, "MetamodelFeature");
        assert_eq!(feature.properties["owner"], "Kernel::Element");
        assert_eq!(
            feature.properties["source_feature"],
            "Kernel::Element::declaredName"
        );
        assert_eq!(feature.properties["kir_property"], "declared_name");
        assert_eq!(feature.properties["feature_kind"], "attribute");
        assert_eq!(
            feature.properties["pilot_library_group"],
            "Kernel Libraries"
        );
        assert_eq!(feature.properties["metamodel_language"], "core");
        assert_eq!(feature.properties["metamodel_layer"], "kernel");
        assert_eq!(feature.properties["type"], "Kernel::String");
        assert_eq!(feature.properties["type_label"], "String");
        assert_eq!(feature.properties["lower"], 0);
        assert_eq!(feature.properties["upper"], 1);
    }

    #[test]
    fn compare_normalization_preserves_direct_properties_and_user_layer() {
        let export = PilotExportDocument {
            metadata: None,
            elements: vec![PilotExportElement {
                qualified_name: "Demo::Vehicle".to_string(),
                kind: "PartDefinition".to_string(),
                library_group: "Input Model".to_string(),
                source: Some(PilotSource {
                    file: "model/src/training/02. Part Definitions/Part Definition Example.model"
                        .to_string(),
                    start_line: Some(2),
                    end_line: Some(4),
                }),
                documentation: vec![PilotDocumentationBlock {
                    kind: "comment".to_string(),
                    text: "vehicle".to_string(),
                }],
                properties: BTreeMap::from([
                    ("declared_name".to_string(), json!("Vehicle")),
                    (
                        "metatype_specialization_chain".to_string(),
                        json!(["Definition", "Type", "Namespace", "Element"]),
                    ),
                ]),
            }],
            relationships: Vec::new(),
        };

        let normalized = normalize_pilot_export_for_compare(export).unwrap();
        let element = normalized.elements.first().unwrap();

        assert_eq!(element.layer, 2);
        assert_eq!(element.properties["declared_name"], "Vehicle");
        assert_eq!(
            element.properties["metatype_specialization_chain"][0],
            "Definition"
        );
        assert_eq!(
            element.properties["metadata"]["source_file"],
            "model/src/training/02. Part Definitions/Part Definition Example.model"
        );
        assert_eq!(element.properties["doc"]["source"], "pilot");
    }

    #[test]
    fn compare_normalization_lifts_relationship_elements_into_canonical_feature_relations() {
        let export = PilotExportDocument {
            metadata: None,
            elements: vec![
                PilotExportElement {
                    qualified_name: "Demo::eng".to_string(),
                    kind: "PartUsage".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(4),
                        end_line: Some(4),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::typed".to_string(),
                    kind: "FeatureTyping".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(4),
                        end_line: Some(4),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::redef".to_string(),
                    kind: "Redefinition".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(4),
                        end_line: Some(4),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::subset".to_string(),
                    kind: "Subsetting".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(4),
                        end_line: Some(4),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::Engine".to_string(),
                    kind: "PartDefinition".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(1),
                        end_line: Some(1),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::BaseEng".to_string(),
                    kind: "PartUsage".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(2),
                        end_line: Some(2),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
                PilotExportElement {
                    qualified_name: "Demo::subparts".to_string(),
                    kind: "PartUsage".to_string(),
                    library_group: "Input Model".to_string(),
                    source: Some(PilotSource {
                        file: "demo.model".to_string(),
                        start_line: Some(3),
                        end_line: Some(3),
                    }),
                    documentation: vec![],
                    properties: BTreeMap::new(),
                },
            ],
            relationships: vec![
                PilotExportRelationship {
                    source: "Demo::typed".to_string(),
                    relation: "typed_feature".to_string(),
                    target: "Demo::eng".to_string(),
                },
                PilotExportRelationship {
                    source: "Demo::typed".to_string(),
                    relation: "general".to_string(),
                    target: "Demo::Engine".to_string(),
                },
                PilotExportRelationship {
                    source: "Demo::redef".to_string(),
                    relation: "redefining_feature".to_string(),
                    target: "Demo::eng".to_string(),
                },
                PilotExportRelationship {
                    source: "Demo::redef".to_string(),
                    relation: "redefined_feature".to_string(),
                    target: "Demo::BaseEng".to_string(),
                },
                PilotExportRelationship {
                    source: "Demo::subset".to_string(),
                    relation: "subsetting_feature".to_string(),
                    target: "Demo::eng".to_string(),
                },
                PilotExportRelationship {
                    source: "Demo::subset".to_string(),
                    relation: "subsetted_feature".to_string(),
                    target: "Demo::subparts".to_string(),
                },
            ],
        };

        let normalized = normalize_pilot_export_for_compare(export).unwrap();
        let eng = normalized
            .elements
            .iter()
            .find(|element| element.id == "Demo::eng")
            .unwrap();

        assert_eq!(eng.properties["type"][0], "Demo::Engine");
        assert_eq!(eng.properties["redefines"][0], "Demo::BaseEng");
        assert_eq!(eng.properties["subsets"][0], "Demo::subparts");
        let specializes = eng.properties["specializes"].as_array().unwrap();
        assert!(specializes.contains(&json!("Demo::Engine")));
        assert!(specializes.contains(&json!("Demo::BaseEng")));
        assert!(specializes.contains(&json!("Demo::subparts")));
    }
}
