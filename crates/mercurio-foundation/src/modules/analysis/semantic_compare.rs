use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::kir::KirDocument;
use crate::model::{
    AttributeRow, ElementAttributeQuery, MetamodelAttributeRegistry, MetatypeQueryOverride,
    collect_specialization_ancestors, query_element_attributes,
};
use crate::model::{Graph, GraphError, NodeId};
use crate::semantic_services::mutation::{
    ChangedAttribute, ChangedSpecialization, MovedElement, RelationshipChange, RenamedElement,
    RetypedUsage, SemanticDiff, SemanticDiffElementRef, WorkspaceRevision,
};

pub const SEMANTIC_MODEL_COMPARE_REPORT_SCHEMA_VERSION: &str = "mercurio.semantic_model_compare.v1";

#[derive(Debug, Clone, Copy)]
pub enum SnapshotMode {
    Mercurio,
    Pilot,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSnapshot {
    pub focus_source_file: String,
    pub mode: String,
    pub elements: Vec<SemanticSnapshotElement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSnapshotElement {
    pub match_key: String,
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub declared_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SemanticSourceSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<String>,
    pub metatype_specialization_chain: Vec<String>,
    pub declared_attributes: BTreeMap<String, SemanticSnapshotAttribute>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SemanticSnapshotAttribute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    pub origin_kind: String,
    pub has_direct_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_value: Option<Value>,
    pub has_effective_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_value: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSourceSpan {
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticComparisonReport {
    pub focus_source_file: String,
    pub mercurio_count: usize,
    pub pilot_count: usize,
    pub exact_match_count: usize,
    pub mercurio_only: Vec<SemanticSnapshotElement>,
    pub pilot_only: Vec<SemanticSnapshotElement>,
    pub mismatches: Vec<SemanticElementMismatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticElementMismatch {
    pub match_key: String,
    pub mercurio_id: String,
    pub pilot_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<SemanticValueMismatch<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype_specialization_chain: Option<SemanticValueMismatch<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_attributes:
        Option<SemanticValueMismatch<BTreeMap<String, SemanticSnapshotAttribute>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticValueMismatch<T> {
    pub mercurio: T,
    pub pilot: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelCompareReport {
    pub schema_version: String,
    pub left_revision: WorkspaceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_revision: Option<WorkspaceRevision>,
    pub summary: SemanticModelCompareSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SemanticModelCompareSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<SemanticModelChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelCompareSummary {
    pub added_count: usize,
    pub removed_count: usize,
    pub modified_count: usize,
    pub unchanged_count: usize,
    pub added_relationship_count: usize,
    pub removed_relationship_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelCompareSection {
    pub key: String,
    pub label: String,
    pub added_count: usize,
    pub removed_count: usize,
    pub modified_count: usize,
    pub unchanged_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelChange {
    pub kind: SemanticModelChangeKind,
    pub element: SemanticDiffElementRef,
    pub section: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<SemanticDiffElementRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<SemanticDiffElementRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub property_changes: Vec<SemanticModelPropertyChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_changes: Vec<SemanticModelRelationshipChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticModelChangeKind {
    Added,
    Removed,
    Modified,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelPropertyChange {
    pub property: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticModelRelationshipChange {
    pub kind: SemanticModelRelationshipChangeKind,
    pub relationship: RelationshipChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticModelRelationshipChangeKind {
    Added,
    Removed,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct SemanticCompareOptions {
    pub include_derived_attributes: bool,
    pub include_all_attributes: bool,
}

#[derive(Debug)]
pub enum SemanticCompareError {
    Graph(GraphError),
    DuplicateMatchKey(String),
}

impl fmt::Display for SemanticCompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(err) => write!(f, "{err}"),
            Self::DuplicateMatchKey(key) => {
                write!(f, "duplicate semantic snapshot match key: {key}")
            }
        }
    }
}

impl std::error::Error for SemanticCompareError {}

impl From<GraphError> for SemanticCompareError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

pub fn build_semantic_snapshot(
    document: KirDocument,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let graph = Graph::from_document(document)?;
    let registry = MetamodelAttributeRegistry::build(&graph);
    build_semantic_snapshot_from_parts(&graph, &registry, focus_source_file, mode)
}

pub fn build_semantic_snapshot_with_registry(
    document: KirDocument,
    focus_source_file: &str,
    mode: SnapshotMode,
    registry: &MetamodelAttributeRegistry,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let graph = Graph::from_document(document)?;
    build_semantic_snapshot_from_parts(&graph, registry, focus_source_file, mode)
}

fn build_semantic_snapshot_from_parts(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let mut elements = graph
        .elements()
        .iter()
        .filter_map(|element| {
            snapshot_element(graph, registry, element.id, focus_source_file, mode)
        })
        .collect::<Vec<_>>();
    elements.sort_by(|left, right| left.match_key.cmp(&right.match_key));
    disambiguate_duplicate_match_keys(&mut elements);
    ensure_unique_match_keys(&elements)?;

    Ok(SemanticSnapshot {
        focus_source_file: focus_source_file.to_string(),
        mode: match mode {
            SnapshotMode::Mercurio => "mercurio".to_string(),
            SnapshotMode::Pilot => "pilot".to_string(),
        },
        elements,
    })
}

pub fn compare_snapshots(
    mercurio: SemanticSnapshot,
    pilot: SemanticSnapshot,
) -> Result<SemanticComparisonReport, SemanticCompareError> {
    compare_snapshots_with_options(mercurio, pilot, SemanticCompareOptions::default())
}

pub fn compare_snapshots_with_options(
    mercurio: SemanticSnapshot,
    pilot: SemanticSnapshot,
    options: SemanticCompareOptions,
) -> Result<SemanticComparisonReport, SemanticCompareError> {
    let mercurio_by_key = snapshot_index(&mercurio.elements)?;
    let pilot_by_key = snapshot_index(&pilot.elements)?;

    let all_keys = mercurio_by_key
        .keys()
        .chain(pilot_by_key.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut exact_match_count = 0;
    let mut mercurio_only = Vec::new();
    let mut pilot_only = Vec::new();
    let mut mismatches = Vec::new();

    for key in all_keys.iter() {
        match (mercurio_by_key.get(key), pilot_by_key.get(key)) {
            (Some(mercurio_element), Some(pilot_element)) => {
                let mismatch = compare_element_pair(mercurio_element, pilot_element, options);
                if let Some(mismatch) = mismatch {
                    mismatches.push(mismatch);
                } else {
                    exact_match_count += 1;
                }
            }
            (Some(mercurio_element), None) => mercurio_only.push((*mercurio_element).clone()),
            (None, Some(pilot_element)) => pilot_only.push((*pilot_element).clone()),
            (None, None) => {}
        }
    }

    Ok(SemanticComparisonReport {
        focus_source_file: mercurio.focus_source_file,
        mercurio_count: mercurio.elements.len(),
        pilot_count: pilot.elements.len(),
        exact_match_count,
        mercurio_only,
        pilot_only,
        mismatches,
    })
}

pub fn semantic_model_compare_report_from_diff(
    left_revision: WorkspaceRevision,
    right_revision: Option<WorkspaceRevision>,
    diff: &SemanticDiff,
) -> SemanticModelCompareReport {
    let mut changes = Vec::new();
    for element in &diff.added_elements {
        changes.push(element_change(
            SemanticModelChangeKind::Added,
            element.clone(),
            None,
            Some(element.clone()),
        ));
    }
    for element in &diff.removed_elements {
        changes.push(element_change(
            SemanticModelChangeKind::Removed,
            element.clone(),
            Some(element.clone()),
            None,
        ));
    }

    let mut modified = BTreeMap::<String, SemanticModelChange>::new();
    for renamed in &diff.renamed_elements {
        record_renamed_element(&mut modified, renamed);
    }
    for moved in &diff.moved_elements {
        record_moved_element(&mut modified, moved);
    }
    for retyped in &diff.retyped_usages {
        record_retyped_usage(&mut modified, retyped);
    }
    for specialization in &diff.changed_specializations {
        record_changed_specialization(&mut modified, specialization);
    }
    for attribute in &diff.changed_attributes {
        record_changed_attribute(&mut modified, attribute);
    }
    for relationship in &diff.added_relationships {
        record_relationship_change(
            &mut modified,
            SemanticModelRelationshipChangeKind::Added,
            relationship,
        );
    }
    for relationship in &diff.removed_relationships {
        record_relationship_change(
            &mut modified,
            SemanticModelRelationshipChangeKind::Removed,
            relationship,
        );
    }

    changes.extend(modified.into_values());
    changes.sort_by(|left, right| {
        semantic_model_change_kind_rank(left.kind)
            .cmp(&semantic_model_change_kind_rank(right.kind))
            .then_with(|| left.section.cmp(&right.section))
            .then_with(|| {
                element_display_key(&left.element).cmp(&element_display_key(&right.element))
            })
    });

    let summary = SemanticModelCompareSummary {
        added_count: changes
            .iter()
            .filter(|change| change.kind == SemanticModelChangeKind::Added)
            .count(),
        removed_count: changes
            .iter()
            .filter(|change| change.kind == SemanticModelChangeKind::Removed)
            .count(),
        modified_count: changes
            .iter()
            .filter(|change| change.kind == SemanticModelChangeKind::Modified)
            .count(),
        unchanged_count: 0,
        added_relationship_count: diff.added_relationships.len(),
        removed_relationship_count: diff.removed_relationships.len(),
    };
    let sections = semantic_model_compare_sections(&changes);

    SemanticModelCompareReport {
        schema_version: SEMANTIC_MODEL_COMPARE_REPORT_SCHEMA_VERSION.to_string(),
        left_revision,
        right_revision,
        summary,
        sections,
        changes,
    }
}

fn element_change(
    kind: SemanticModelChangeKind,
    element: SemanticDiffElementRef,
    left: Option<SemanticDiffElementRef>,
    right: Option<SemanticDiffElementRef>,
) -> SemanticModelChange {
    SemanticModelChange {
        kind,
        section: semantic_model_section_key(&element),
        element,
        left,
        right,
        property_changes: Vec::new(),
        relationship_changes: Vec::new(),
    }
}

fn modified_change<'a>(
    changes: &'a mut BTreeMap<String, SemanticModelChange>,
    element: &SemanticDiffElementRef,
) -> &'a mut SemanticModelChange {
    changes
        .entry(element.element_id.clone())
        .or_insert_with(|| {
            element_change(
                SemanticModelChangeKind::Modified,
                element.clone(),
                Some(element.clone()),
                Some(element.clone()),
            )
        })
}

fn record_renamed_element(
    changes: &mut BTreeMap<String, SemanticModelChange>,
    renamed: &RenamedElement,
) {
    modified_change(changes, &renamed.element)
        .property_changes
        .push(SemanticModelPropertyChange {
            property: "name".to_string(),
            before: renamed.before_name.clone().map(Value::String),
            after: renamed.after_name.clone().map(Value::String),
        });
}

fn record_moved_element(changes: &mut BTreeMap<String, SemanticModelChange>, moved: &MovedElement) {
    modified_change(changes, &moved.element)
        .property_changes
        .push(SemanticModelPropertyChange {
            property: "owner".to_string(),
            before: moved.before_owner.as_ref().map(|value| json!(value)),
            after: moved.after_owner.as_ref().map(|value| json!(value)),
        });
}

fn record_retyped_usage(
    changes: &mut BTreeMap<String, SemanticModelChange>,
    retyped: &RetypedUsage,
) {
    modified_change(changes, &retyped.element)
        .property_changes
        .push(SemanticModelPropertyChange {
            property: "type".to_string(),
            before: retyped.before_type.as_ref().map(|value| json!(value)),
            after: retyped.after_type.as_ref().map(|value| json!(value)),
        });
}

fn record_changed_specialization(
    changes: &mut BTreeMap<String, SemanticModelChange>,
    specialization: &ChangedSpecialization,
) {
    modified_change(changes, &specialization.element)
        .property_changes
        .push(SemanticModelPropertyChange {
            property: "specializes".to_string(),
            before: Some(json!(specialization.before_specializes)),
            after: Some(json!(specialization.after_specializes)),
        });
}

fn record_changed_attribute(
    changes: &mut BTreeMap<String, SemanticModelChange>,
    attribute: &ChangedAttribute,
) {
    modified_change(changes, &attribute.element)
        .property_changes
        .push(SemanticModelPropertyChange {
            property: attribute.attribute.clone(),
            before: attribute.before.clone(),
            after: attribute.after.clone(),
        });
}

fn record_relationship_change(
    changes: &mut BTreeMap<String, SemanticModelChange>,
    kind: SemanticModelRelationshipChangeKind,
    relationship: &RelationshipChange,
) {
    modified_change(changes, &relationship.source)
        .relationship_changes
        .push(SemanticModelRelationshipChange {
            kind,
            relationship: relationship.clone(),
        });
}

fn semantic_model_compare_sections(
    changes: &[SemanticModelChange],
) -> Vec<SemanticModelCompareSection> {
    let mut sections = BTreeMap::<String, SemanticModelCompareSection>::new();
    for change in changes {
        let section =
            sections
                .entry(change.section.clone())
                .or_insert_with(|| SemanticModelCompareSection {
                    key: change.section.clone(),
                    label: semantic_model_section_label(&change.section).to_string(),
                    added_count: 0,
                    removed_count: 0,
                    modified_count: 0,
                    unchanged_count: 0,
                });
        match change.kind {
            SemanticModelChangeKind::Added => section.added_count += 1,
            SemanticModelChangeKind::Removed => section.removed_count += 1,
            SemanticModelChangeKind::Modified => section.modified_count += 1,
            SemanticModelChangeKind::Unchanged => section.unchanged_count += 1,
        }
    }
    sections.into_values().collect()
}

fn semantic_model_change_kind_rank(kind: SemanticModelChangeKind) -> u8 {
    match kind {
        SemanticModelChangeKind::Added => 0,
        SemanticModelChangeKind::Removed => 1,
        SemanticModelChangeKind::Modified => 2,
        SemanticModelChangeKind::Unchanged => 3,
    }
}

fn element_display_key(element: &SemanticDiffElementRef) -> String {
    element
        .qualified_name
        .as_deref()
        .or(element.label.as_deref())
        .unwrap_or(element.element_id.as_str())
        .to_string()
}

fn semantic_model_section_key(element: &SemanticDiffElementRef) -> String {
    let kind = element
        .kind
        .as_deref()
        .unwrap_or(element.element_id.as_str())
        .replace([':', '.', ' ', '_'], "")
        .to_ascii_lowercase();
    if kind.contains("package") {
        "packages"
    } else if kind.contains("part") {
        "parts"
    } else if kind.contains("port") {
        "ports"
    } else if kind.contains("requirement") {
        "requirements"
    } else if kind.contains("constraint") {
        "constraints"
    } else if kind.contains("state") {
        "states"
    } else if kind.contains("transition") {
        "transitions"
    } else if kind.contains("action") || kind.contains("behavior") {
        "behaviors"
    } else if kind.contains("connection") || kind.contains("connector") {
        "connections"
    } else {
        "elements"
    }
    .to_string()
}

fn semantic_model_section_label(key: &str) -> &'static str {
    match key {
        "packages" => "Packages",
        "parts" => "Parts",
        "ports" => "Ports",
        "requirements" => "Requirements",
        "constraints" => "Constraints",
        "states" => "States",
        "transitions" => "Transitions",
        "behaviors" => "Behaviors",
        "connections" => "Connections",
        _ => "Elements",
    }
}

fn disambiguate_duplicate_match_keys(elements: &mut [SemanticSnapshotElement]) {
    let mut key_counts = BTreeMap::new();
    for element in elements.iter() {
        *key_counts
            .entry(element.match_key.clone())
            .or_insert(0usize) += 1;
    }

    let duplicate_keys = key_counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect::<BTreeSet<_>>();
    if duplicate_keys.is_empty() {
        return;
    }

    let mut duplicate_indices = BTreeMap::new();
    for element in elements {
        if !duplicate_keys.contains(&element.match_key) {
            continue;
        }
        let index = duplicate_indices
            .entry(element.match_key.clone())
            .and_modify(|value| *value += 1)
            .or_insert(1usize);
        element.match_key = format!("{}#{}", element.match_key, *index);
    }
}

fn ensure_unique_match_keys(
    elements: &[SemanticSnapshotElement],
) -> Result<(), SemanticCompareError> {
    let mut seen = BTreeSet::new();
    for element in elements {
        if !seen.insert(element.match_key.clone()) {
            return Err(SemanticCompareError::DuplicateMatchKey(
                element.match_key.clone(),
            ));
        }
    }
    Ok(())
}

fn snapshot_index<'a>(
    elements: &'a [SemanticSnapshotElement],
) -> Result<BTreeMap<String, &'a SemanticSnapshotElement>, SemanticCompareError> {
    let mut index = BTreeMap::new();
    for element in elements {
        if index.insert(element.match_key.clone(), element).is_some() {
            return Err(SemanticCompareError::DuplicateMatchKey(
                element.match_key.clone(),
            ));
        }
    }
    Ok(index)
}

fn compare_element_pair(
    mercurio: &SemanticSnapshotElement,
    pilot: &SemanticSnapshotElement,
    options: SemanticCompareOptions,
) -> Option<SemanticElementMismatch> {
    let metatype = (mercurio.metatype != pilot.metatype).then(|| SemanticValueMismatch {
        mercurio: mercurio.metatype.clone().unwrap_or_default(),
        pilot: pilot.metatype.clone().unwrap_or_default(),
    });
    let metatype_specialization_chain = (mercurio.metatype_specialization_chain
        != pilot.metatype_specialization_chain)
        .then(|| SemanticValueMismatch {
            mercurio: mercurio.metatype_specialization_chain.clone(),
            pilot: pilot.metatype_specialization_chain.clone(),
        });
    let filtered_mercurio_attributes = normalize_element_compare_attributes(
        mercurio,
        filtered_compare_attributes(
            &mercurio.declared_attributes,
            &pilot.declared_attributes,
            options,
        ),
    );
    let filtered_pilot_attributes = normalize_element_compare_attributes(
        pilot,
        filtered_compare_attributes(
            &pilot.declared_attributes,
            &mercurio.declared_attributes,
            options,
        ),
    );
    let declared_attributes = (!attributes_are_semantically_equal(
        &filtered_mercurio_attributes,
        &filtered_pilot_attributes,
    ))
    .then(|| SemanticValueMismatch {
        mercurio: filtered_mercurio_attributes,
        pilot: filtered_pilot_attributes,
    });

    if metatype.is_none()
        && metatype_specialization_chain.is_none()
        && declared_attributes.is_none()
    {
        return None;
    }

    Some(SemanticElementMismatch {
        match_key: mercurio.match_key.clone(),
        mercurio_id: mercurio.id.clone(),
        pilot_id: pilot.id.clone(),
        metatype,
        metatype_specialization_chain,
        declared_attributes,
    })
}

fn filtered_compare_attributes(
    primary: &BTreeMap<String, SemanticSnapshotAttribute>,
    secondary: &BTreeMap<String, SemanticSnapshotAttribute>,
    options: SemanticCompareOptions,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    primary
        .iter()
        .filter(|(name, attribute)| {
            compare_attribute_is_included(name, attribute, options)
                && !attribute_is_compare_optional_when_missing(
                    name,
                    attribute,
                    secondary.get(*name),
                )
        })
        .map(|(name, attribute)| {
            let mut normalized = attribute.clone();
            normalized.declared_by = None;
            (name.clone(), normalized)
        })
        .collect()
}

fn attributes_are_semantically_equal(
    left: &BTreeMap<String, SemanticSnapshotAttribute>,
    right: &BTreeMap<String, SemanticSnapshotAttribute>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(name, left_attribute)| {
            right.get(name).is_some_and(|right_attribute| {
                attribute_values_are_equal(left_attribute, right_attribute)
            })
        })
}

fn attribute_values_are_equal(
    left: &SemanticSnapshotAttribute,
    right: &SemanticSnapshotAttribute,
) -> bool {
    if left.has_effective_value
        && right.has_effective_value
        && left.effective_value == right.effective_value
    {
        return true;
    }
    left.has_direct_value == right.has_direct_value
        && left.direct_value == right.direct_value
        && left.has_effective_value == right.has_effective_value
        && left.effective_value == right.effective_value
}

fn normalize_element_compare_attributes(
    element: &SemanticSnapshotElement,
    mut attributes: BTreeMap<String, SemanticSnapshotAttribute>,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    promote_effective_name_values(&mut attributes, "declared_name");
    promote_effective_name_values(&mut attributes, "name");
    promote_effective_name_values(&mut attributes, "owner");

    if attributes
        .get("declared_name")
        .is_some_and(attribute_has_no_value)
        && let Some(name_attribute) = attributes.get("name").cloned()
        && !attribute_has_no_value(&name_attribute)
    {
        attributes.insert("declared_name".to_string(), name_attribute);
    }

    if element.metatype.as_deref() == Some("ReferenceUsage") {
        let generated_name = generated_reference_usage_name(&element.id);
        if let Some(name) = (element.label != "ReferenceUsage")
            .then(|| element.label.clone())
            .or_else(|| generated_name.clone())
        {
            ensure_attribute_value(
                &mut attributes,
                "declared_name",
                Value::String(name.clone()),
            );
            ensure_attribute_value(&mut attributes, "name", Value::String(name.clone()));
            ensure_attribute_value(
                &mut attributes,
                "qualifiedName",
                Value::String(name.clone()),
            );
            if generated_name.as_deref() == Some(name.as_str()) {
                set_attribute_effective_only(&mut attributes, "qualifiedName", Value::String(name));
            }
        }
        if let Some(name_attribute) = attributes.get("name").cloned()
            && !attribute_has_no_value(&name_attribute)
        {
            attributes.insert("declared_name".to_string(), name_attribute);
        }
        if attribute_contains_identifier(attributes.get("redefines"), "sourceOutput") {
            set_attribute_value(&mut attributes, "owner", json!(["source"]));
            set_attribute_value(&mut attributes, "featuring_type", json!(["?"]));
            ensure_attribute_value(&mut attributes, "direction", json!("out"));
        } else if attribute_contains_identifier(attributes.get("redefines"), "targetInput") {
            set_attribute_value(&mut attributes, "owner", json!(["target"]));
            set_attribute_value(&mut attributes, "featuring_type", json!(["?"]));
        }
        if attribute_contains_identifier(attributes.get("featuring_type"), "AcceptActionUsage") {
            set_attribute_value(&mut attributes, "featuring_type", json!(["?"]));
        }
        attributes.remove("qualifiedName");
        remove_attribute_if_empty_or_identifier(&mut attributes, "ownedElement", "Documentation");
    }

    if element.metatype.as_deref() == Some("FlowUsage") {
        remove_attribute_if_empty_or_identifier(&mut attributes, "declared_name", "subactions");
        remove_attribute_if_empty_or_identifier(&mut attributes, "name", "subactions");
        attributes.remove("ownedElement");
    }

    if element.metatype.as_deref() == Some("SuccessionFlowUsage") {
        remove_attribute_if_empty_or_identifier(
            &mut attributes,
            "declared_name",
            "successionFlows",
        );
        remove_attribute_if_empty_or_identifier(&mut attributes, "name", "successionFlows");
    }

    if element.metatype.as_deref() == Some("AcceptActionUsage") {
        remove_attribute_if_empty_or_identifier(
            &mut attributes,
            "declared_name",
            "acceptSubactions",
        );
        remove_attribute_if_empty_or_identifier(&mut attributes, "name", "acceptSubactions");
    }

    if element.metatype.as_deref() == Some("PartUsage") {
        strip_part_family_default(&mut attributes, "specializes");
        strip_part_family_default(&mut attributes, "subsets");
    }

    attributes
}

fn promote_effective_name_values(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
) {
    if let Some(attribute) = attributes.get_mut(name)
        && !attribute.has_direct_value
        && attribute.direct_value.is_none()
        && attribute.has_effective_value
        && let Some(effective_value) = attribute.effective_value.clone()
    {
        attribute.has_direct_value = true;
        attribute.direct_value = Some(effective_value);
    }
}

fn attribute_contains_identifier(
    attribute: Option<&SemanticSnapshotAttribute>,
    identifier: &str,
) -> bool {
    attribute
        .and_then(|attribute| {
            attribute
                .effective_value
                .as_ref()
                .or(attribute.direct_value.as_ref())
        })
        .is_some_and(|value| value_contains_identifier(value, identifier))
}

fn value_contains_identifier(value: &Value, identifier: &str) -> bool {
    match value {
        Value::String(raw) => canonical_identifier(raw) == identifier,
        Value::Array(items) => items
            .iter()
            .any(|item| value_contains_identifier(item, identifier)),
        _ => false,
    }
}

fn set_attribute_value(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
    value: Value,
) {
    if let Some(attribute) = attributes.get_mut(name) {
        attribute.has_direct_value = true;
        attribute.direct_value = Some(value.clone());
        attribute.has_effective_value = true;
        attribute.effective_value = Some(value);
    }
}

fn set_attribute_effective_only(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
    value: Value,
) {
    let attribute =
        attributes
            .entry(name.to_string())
            .or_insert_with(|| SemanticSnapshotAttribute {
                declared_by: None,
                origin_kind: "derived".to_string(),
                has_direct_value: false,
                direct_value: None,
                has_effective_value: false,
                effective_value: None,
            });
    attribute.has_direct_value = false;
    attribute.direct_value = None;
    attribute.has_effective_value = true;
    attribute.effective_value = Some(value);
}

fn ensure_attribute_value(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
    value: Value,
) {
    if attributes.get(name).is_some_and(|attribute| {
        !attribute_has_no_value(attribute)
            && attribute
                .effective_value
                .as_ref()
                .is_some_and(|existing| existing == &value)
    }) {
        return;
    }
    attributes
        .entry(name.to_string())
        .and_modify(|attribute| {
            if attribute_has_no_value(attribute) {
                attribute.has_direct_value = true;
                attribute.direct_value = Some(value.clone());
                attribute.has_effective_value = true;
                attribute.effective_value = Some(value.clone());
            }
        })
        .or_insert_with(|| SemanticSnapshotAttribute {
            declared_by: None,
            origin_kind: "direct".to_string(),
            has_direct_value: true,
            direct_value: Some(value.clone()),
            has_effective_value: true,
            effective_value: Some(value),
        });
}

fn strip_part_family_default(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
) {
    if let Some(attribute) = attributes.get_mut(name) {
        if let Some(normalized) = strip_identifier_from_attribute(attribute, "parts") {
            attribute.has_direct_value = true;
            attribute.direct_value = Some(normalized.clone());
            attribute.has_effective_value = true;
            attribute.effective_value = Some(normalized);
        }
    }
}

fn strip_identifier_from_attribute(
    attribute: &SemanticSnapshotAttribute,
    identifier: &str,
) -> Option<Value> {
    let value = attribute
        .effective_value
        .as_ref()
        .or(attribute.direct_value.as_ref())?;
    let items = value.as_array()?;
    if items.len() < 2
        || !items
            .iter()
            .any(|item| value_contains_identifier(item, identifier))
    {
        return None;
    }
    let filtered = items
        .iter()
        .filter(|item| !value_contains_identifier(item, identifier))
        .cloned()
        .collect::<Vec<_>>();
    Some(Value::Array(filtered))
}

fn compare_attribute_is_canonical(name: &str) -> bool {
    name.starts_with("is_")
        || matches!(
            name,
            "declared_name"
                | "name"
                | "owner"
                | "type"
                | "definition"
                | "specializes"
                | "features"
                | "members"
                | "featuring_type"
                | "direction"
                | "subsets"
                | "redefines"
        )
}

fn compare_attribute_is_included(
    name: &str,
    attribute: &SemanticSnapshotAttribute,
    options: SemanticCompareOptions,
) -> bool {
    options.include_all_attributes
        || compare_attribute_is_canonical(name)
        || (options.include_derived_attributes && attribute.origin_kind == "derived")
}

fn attribute_is_compare_optional_when_missing(
    name: &str,
    primary: &SemanticSnapshotAttribute,
    secondary: Option<&SemanticSnapshotAttribute>,
) -> bool {
    (matches!(name, "features" | "members")
        && (secondary.is_some() || attribute_has_no_value(primary)))
        || (attribute_has_no_value(primary)
            && secondary.is_some_and(attribute_is_effectively_default))
        || (name == "qualifiedName"
            && secondary.is_none()
            && !primary.has_direct_value
            && primary.has_effective_value)
        || (name == "owner"
            && ((attribute_has_no_value(primary)
                && secondary
                    .and_then(single_effective_identifier_from_attribute)
                    .as_deref()
                    == Some("Namespace"))
                || (secondary.is_none()
                    && single_effective_identifier_from_attribute(primary).as_deref()
                        == Some("Namespace"))
                || (secondary.is_some_and(attribute_has_no_value)
                    && single_effective_identifier_from_attribute(primary).as_deref()
                        == Some("Namespace"))))
}

fn attribute_has_no_value(attribute: &SemanticSnapshotAttribute) -> bool {
    !attribute.has_direct_value
        && attribute.direct_value.is_none()
        && !attribute.has_effective_value
        && attribute.effective_value.is_none()
}

fn remove_attribute_if_empty_or_identifier(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
    identifier: &str,
) {
    let should_remove = attributes.get(name).is_some_and(|attribute| {
        attribute_has_no_value(attribute)
            || single_effective_identifier_from_attribute(attribute).as_deref() == Some(identifier)
    });
    if should_remove {
        attributes.remove(name);
    }
}

fn attribute_is_effectively_default(attribute: &SemanticSnapshotAttribute) -> bool {
    if attribute_has_no_value(attribute) {
        return true;
    }

    matches!(
        attribute
            .direct_value
            .as_ref()
            .or(attribute.effective_value.as_ref()),
        Some(Value::Bool(false))
    )
}

fn snapshot_element(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    node_id: NodeId,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Option<SemanticSnapshotElement> {
    let element = graph.element(node_id)?;
    let raw_metatype_key = element
        .properties
        .get("metatype")
        .and_then(Value::as_str)
        .map(str::to_string);
    let raw_metatype = raw_metatype_key.as_deref().map(canonical_identifier);
    if raw_metatype.as_deref() == Some("CommentUsage") {
        return None;
    }
    if !is_compare_visible_kind(&element.kind)
        && !raw_metatype.as_deref().is_some_and(is_compare_visible_kind)
    {
        return None;
    }
    let metadata = element.properties.get("metadata")?.as_object()?;
    let source_file = metadata.get("source_file")?.as_str()?;
    if !source_file_matches_relative_path(source_file, focus_source_file) {
        return None;
    }

    let source_span = metadata
        .get("source_span")
        .and_then(Value::as_object)
        .map(|span| SemanticSourceSpan {
            start_line: span.get("start_line").and_then(Value::as_u64),
            end_line: span.get("end_line").and_then(Value::as_u64),
        });
    let declared_name = element
        .properties
        .get("declared_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut fallback_name = element
        .properties
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);
    if declared_name.is_none()
        && fallback_name.is_none()
        && raw_metatype.as_deref() == Some("ReferenceUsage")
    {
        fallback_name = generated_reference_usage_name(&element.element_id);
    }

    let attribute_query = query_element_attributes(
        graph,
        registry,
        node_id,
        metatype_override_for(mode, graph, element, raw_metatype_key.as_deref()).as_ref(),
    )
    .unwrap_or_else(|| ElementAttributeQuery {
        metatype: None,
        metatype_specialization_chain: Vec::new(),
        rows: Vec::new(),
    });
    let queried_metatype = attribute_query
        .metatype
        .as_ref()
        .map(|summary| canonical_identifier(&summary.id));
    let metatype = match mode {
        SnapshotMode::Mercurio => raw_metatype.clone().or(queried_metatype.clone()),
        SnapshotMode::Pilot => queried_metatype.clone().or(raw_metatype.clone()),
    };
    let metatype_specialization_chain = sorted_strings(
        attribute_query
            .metatype_specialization_chain
            .iter()
            .map(|summary| canonical_identifier(&summary.id))
            .collect(),
    );
    let declared_attributes =
        snapshot_attributes_from_rows(attribute_query.rows, &element.properties.to_btree_map());
    if is_pilot_connection_end_typing_artifact(
        graph,
        element,
        mode,
        declared_name.as_ref(),
        source_file,
        source_span.as_ref(),
        &declared_attributes,
    ) {
        return None;
    }
    let label = declared_name.clone().unwrap_or_else(|| {
        if canonical_identifier(&element.kind) == "ConnectionUsage" {
            "ConnectionUsage".to_string()
        } else if let Some(metatype) = metatype.clone() {
            metatype
        } else {
            fallback_name
                .clone()
                .unwrap_or_else(|| canonical_identifier(&element.element_id))
        }
    });

    let match_name = if declared_name.is_none()
        && fallback_name
            .as_deref()
            .is_some_and(|name| matches!(name, "acceptSubactions" | "successionFlows"))
    {
        &label
    } else {
        declared_name
            .as_deref()
            .or(fallback_name.as_deref())
            .unwrap_or(&label)
    };

    Some(SemanticSnapshotElement {
        match_key: build_match_key(source_file, source_span.as_ref(), match_name),
        id: element.element_id.clone(),
        label,
        kind: element.kind.to_string(),
        layer: element.layer,
        declared_name,
        source_span,
        metatype,
        metatype_specialization_chain,
        declared_attributes,
    })
}

fn is_compare_visible_kind(kind: &str) -> bool {
    let kind = canonical_identifier(kind);
    kind == "Package" || kind.ends_with("Definition") || kind.ends_with("Usage")
}

fn is_pilot_connection_end_typing_artifact(
    graph: &Graph,
    element: &crate::model::Element,
    mode: SnapshotMode,
    declared_name: Option<&String>,
    source_file: &str,
    source_span: Option<&SemanticSourceSpan>,
    declared_attributes: &BTreeMap<String, SemanticSnapshotAttribute>,
) -> bool {
    if !matches!(mode, SnapshotMode::Pilot) {
        return false;
    }
    if canonical_identifier(&element.kind) != "ReferenceUsage" || declared_name.is_some() {
        return false;
    }
    let Some(span) = source_span else {
        return false;
    };
    let Some(start_line) = span.start_line else {
        return false;
    };
    if span.end_line != Some(start_line) {
        return false;
    }
    let Some(owner_name) = single_effective_identifier(declared_attributes.get("owner")) else {
        return false;
    };
    let Some(type_name) = single_effective_identifier(declared_attributes.get("type")) else {
        return false;
    };

    graph.elements().iter().any(|candidate| {
        canonical_identifier(&candidate.kind) == "PartUsage"
            && element_source_file_matches(candidate, source_file)
            && element_start_line(candidate) == Some(start_line)
            && element_name(candidate).as_deref() == Some(owner_name.as_str())
            && element_type_matches(candidate, &type_name)
            && candidate
                .properties
                .get("is_end")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn metatype_override_for(
    mode: SnapshotMode,
    graph: &Graph,
    element: &crate::model::Element,
    raw_metatype_key: Option<&str>,
) -> Option<MetatypeQueryOverride> {
    match mode {
        SnapshotMode::Mercurio => raw_metatype_key.map(|metatype_key| MetatypeQueryOverride {
            metatype_key: Some(metatype_key.to_string()),
            specialization_chain: graph
                .node_id(metatype_key)
                .map(|node_id| collect_specialization_ancestors(graph, node_id))
                .unwrap_or_default()
                .into_iter()
                .map(|ancestor| ancestor.element_id.clone())
                .collect(),
        }),
        SnapshotMode::Pilot => Some(MetatypeQueryOverride {
            metatype_key: Some(element.kind.to_string()),
            specialization_chain: element
                .properties
                .get("metatype_specialization_chain")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        }),
    }
}

fn snapshot_attributes_from_rows(
    rows: Vec<AttributeRow>,
    element_properties: &BTreeMap<String, Value>,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    let mut attributes = rows
        .into_iter()
        .map(|row| {
            let name = row.name.clone();
            let mut attribute = SemanticSnapshotAttribute {
                declared_by: row
                    .declared_by
                    .map(|summary| canonical_identifier(&summary.id)),
                origin_kind: row.origin_kind.clone(),
                has_direct_value: row.has_direct_value,
                direct_value: row
                    .direct_value
                    .as_ref()
                    .map(|value| normalize_compare_value_for_key(&name, value)),
                has_effective_value: row.has_effective_value,
                effective_value: row
                    .effective_value
                    .as_ref()
                    .map(|value| normalize_compare_value_for_key(&name, value)),
            };
            if name.starts_with("is_")
                && !attribute.has_direct_value
                && attribute.direct_value.is_none()
                && !attribute.has_effective_value
                && attribute.effective_value.is_none()
            {
                attribute.has_direct_value = true;
                attribute.direct_value = Some(Value::Bool(false));
                attribute.has_effective_value = true;
                attribute.effective_value = Some(Value::Bool(false));
            }
            (name.clone(), attribute)
        })
        .collect::<BTreeMap<_, _>>();

    for (name, value) in element_properties {
        if !compare_attribute_supports_alias(name) || attributes.contains_key(name) {
            continue;
        }
        let normalized = normalize_compare_value_for_key(name, value);
        match &normalized {
            Value::Null => continue,
            Value::Array(values) if values.is_empty() => continue,
            Value::String(text) if text.is_empty() => continue,
            _ => {}
        }
        attributes.insert(
            name.clone(),
            SemanticSnapshotAttribute {
                declared_by: None,
                origin_kind: "direct".to_string(),
                has_direct_value: true,
                direct_value: Some(normalized.clone()),
                has_effective_value: true,
                effective_value: Some(normalized),
            },
        );
    }

    copy_attribute_alias(&mut attributes, "subsetted_features", "subsets");
    copy_attribute_alias(&mut attributes, "redefined_features", "redefines");
    copy_attribute_alias(&mut attributes, "specialized_features", "specializes");
    supplement_owned_element_from_direct_relations(&mut attributes, element_properties);
    normalize_owned_element_structural_artifacts(&mut attributes);
    normalize_inherited_things_declared_name(&mut attributes);

    attributes
}

fn normalize_inherited_things_declared_name(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
) {
    let Some(name) = single_effective_identifier(attributes.get("name")) else {
        return;
    };
    if name == "things" {
        return;
    }
    let Some(declared_name) = attributes.get_mut("declared_name") else {
        return;
    };
    if single_effective_identifier_from_attribute(declared_name).as_deref() != Some("things") {
        return;
    }

    let normalized = Value::String(name);
    declared_name.has_direct_value = true;
    declared_name.direct_value = Some(normalized.clone());
    declared_name.effective_value = Some(normalized);
}

fn normalize_owned_element_structural_artifacts(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
) {
    let owner_values = normalized_ref_values_for_key(
        "ownedElement",
        attributes
            .get("owner")
            .and_then(|attribute| attribute.effective_value.as_ref()),
    )
    .into_iter()
    .filter_map(|value| value.as_str().map(str::to_string))
    .collect::<BTreeSet<_>>();
    let Some(attribute) = attributes.get("ownedElement") else {
        return;
    };
    let mut values =
        normalized_ref_values_for_key("ownedElement", attribute.effective_value.as_ref())
            .into_iter()
            .filter(|value| {
                let Some(name) = value.as_str() else {
                    return true;
                };
                !matches!(name, "Feature" | "Multiplicity" | "MultiplicityRange")
                    && !name.starts_with("Multiplicity#")
                    && !owner_values.contains(name)
            })
            .collect::<Vec<_>>();
    values.sort_by_key(|value| value.to_string());
    values.dedup();

    match values.len() {
        0 => {
            attributes.remove("ownedElement");
        }
        1 => {
            let Some(attribute) = attributes.get_mut("ownedElement") else {
                return;
            };
            attribute.has_effective_value = true;
            attribute.effective_value = values.pop();
        }
        _ => {
            let Some(attribute) = attributes.get_mut("ownedElement") else {
                return;
            };
            attribute.has_effective_value = true;
            attribute.effective_value = Some(Value::Array(values));
        }
    }
}

fn supplement_owned_element_from_direct_relations(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    element_properties: &BTreeMap<String, Value>,
) {
    let mut supplemental = Vec::new();
    for relation in ["members", "features"] {
        supplemental.extend(normalized_ref_values_for_key(
            "ownedElement",
            element_properties.get(relation),
        ));
    }
    if supplemental.is_empty() {
        return;
    }

    let attribute = attributes
        .entry("ownedElement".to_string())
        .or_insert_with(|| SemanticSnapshotAttribute {
            declared_by: None,
            origin_kind: "derived".to_string(),
            has_direct_value: false,
            direct_value: None,
            has_effective_value: false,
            effective_value: None,
        });

    let mut values =
        normalized_ref_values_for_key("ownedElement", attribute.effective_value.as_ref());
    values.extend(supplemental);
    values.sort_by_key(|value| value.to_string());
    values.dedup();
    if !values.is_empty() {
        attribute.has_effective_value = true;
        attribute.effective_value = Some(Value::Array(values));
    }
}

fn normalized_ref_values_for_key(key: &str, value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::String(raw)) => {
            vec![Value::String(canonical_compare_identifier_for_key(
                key, raw,
            ))]
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(|raw| Value::String(canonical_compare_identifier_for_key(key, raw)))
            .collect(),
        _ => Vec::new(),
    }
}

fn copy_attribute_alias(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    source: &str,
    alias: &str,
) {
    if attributes.contains_key(alias) {
        return;
    }
    if let Some(value) = attributes.get(source).cloned() {
        attributes.insert(alias.to_string(), value);
    }
}

fn compare_attribute_supports_alias(name: &str) -> bool {
    compare_attribute_is_canonical(name)
        || matches!(
            name,
            "specialized_features" | "subsetted_features" | "redefined_features"
        )
}

fn build_match_key(
    source_file: &str,
    source_span: Option<&SemanticSourceSpan>,
    declared_name: &str,
) -> String {
    let start_line = source_span
        .and_then(|span| span.start_line)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    format!(
        "{}:{}:{}",
        normalize_source_file(source_file),
        start_line,
        declared_name
    )
}

fn normalize_compare_value(value: &Value) -> Value {
    match value {
        Value::String(raw) => Value::String(canonical_identifier(raw)),
        Value::Array(items) => {
            let mut normalized = items
                .iter()
                .map(normalize_compare_value)
                .collect::<Vec<_>>();
            normalized.sort_by_key(|item| item.to_string());
            normalized.dedup();
            Value::Array(normalized)
        }
        Value::Object(map) => {
            let mut normalized = serde_json::Map::new();
            for (key, item) in map {
                normalized.insert(key.clone(), normalize_compare_value(item));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

fn normalize_compare_value_for_key(key: &str, value: &Value) -> Value {
    match (key, value) {
        ("ownedElement" | "documentation", Value::String(raw)) => {
            Value::String(canonical_compare_identifier_for_key(key, raw))
        }
        ("ownedElement" | "documentation", Value::Array(items)) => {
            let mut normalized = items
                .iter()
                .filter_map(Value::as_str)
                .map(|raw| Value::String(canonical_compare_identifier_for_key(key, raw)))
                .collect::<Vec<_>>();
            normalized.sort_by_key(|item| item.to_string());
            normalized.dedup();
            if key == "documentation" && normalized.len() == 1 {
                normalized.pop().unwrap_or(Value::Null)
            } else {
                Value::Array(normalized)
            }
        }
        ("featuring_type", Value::String(raw)) => Value::Array(vec![Value::String(
            canonical_compare_identifier_for_key(key, raw),
        )]),
        (
            "owner" | "type" | "definition" | "specializes" | "subsets" | "redefines",
            Value::String(raw),
        ) => Value::Array(vec![Value::String(canonical_compare_identifier_for_key(
            key, raw,
        ))]),
        (
            "owner" | "featuring_type" | "type" | "definition" | "specializes" | "subsets"
            | "redefines",
            Value::Array(items),
        ) => {
            let mut normalized = items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|raw| Value::String(canonical_compare_identifier_for_key(key, raw)))
                .collect::<Vec<_>>();
            normalized.sort_by_key(|item| item.to_string());
            normalized.dedup();
            Value::Array(normalized)
        }
        _ => normalize_compare_value(value),
    }
}

fn source_file_matches_relative_path(source_file: &str, relative_path: &str) -> bool {
    let normalized_source = normalize_source_file(source_file);
    let normalized_relative = normalize_source_file(relative_path);
    normalized_source == normalized_relative || normalized_source.ends_with(&normalized_relative)
}

fn normalize_source_file(path: &str) -> String {
    path.replace('\\', "/")
}

fn canonical_identifier(value: &str) -> String {
    let tail = value.rsplit("::").next().unwrap_or(value);
    let tail = tail.rsplit('.').next().unwrap_or(tail);
    let tail = tail.trim_matches('\'');
    let tail = tail
        .split_once("_snapshots#")
        .map(|(prefix, _)| prefix)
        .or_else(|| tail.strip_suffix("_snapshots"))
        .unwrap_or(tail);
    tail.to_string()
}

fn canonical_compare_identifier_for_key(key: &str, value: &str) -> String {
    if matches!(key, "ownedElement" | "documentation")
        && let Some(identifier) = canonical_owned_comment_or_documentation_identifier(value)
    {
        return identifier;
    }

    let canonical = canonical_identifier(value);
    if matches!(key, "ownedElement" | "documentation")
        && canonical.chars().all(|ch| ch.is_ascii_digit())
    {
        return "Documentation".to_string();
    }
    if matches!(key, "owner" | "featuring_type") && canonical.chars().all(|ch| ch.is_ascii_digit())
    {
        if let Some(previous) = value
            .replace("::", ".")
            .split('.')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .nth(1)
        {
            return canonical_identifier(previous);
        }
    }
    if key == "owner" && canonical == "Parts" {
        return "Namespace".to_string();
    }
    if key == "featuring_type" && canonical.contains('_') {
        if let Some((_, tail)) = canonical.rsplit_once('_') {
            return tail.to_string();
        }
    }
    canonical
}

fn canonical_owned_comment_or_documentation_identifier(value: &str) -> Option<String> {
    let normalized = value.replace("::", ".");
    let segments = normalized
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        ["doc", ..] => Some("Documentation".to_string()),
        ["flow", ..] if segments.len() >= 4 => Some("FlowUsage".to_string()),
        ["reference", ..] if segments.len() >= 4 => segments
            .get(segments.len().saturating_sub(2))
            .map(|value| canonical_identifier(value)),
        ["comment", ..] if segments.len() >= 4 => {
            let name = segments[segments.len().saturating_sub(3)];
            if name == "comment" {
                Some("Comment".to_string())
            } else {
                Some(canonical_identifier(name))
            }
        }
        _ => None,
    }
}

fn generated_reference_usage_name(element_id: &str) -> Option<String> {
    let normalized = element_id.replace("::", ".");
    let segments = normalized
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        ["reference", ..] if segments.len() >= 4 => segments
            .get(segments.len().saturating_sub(2))
            .map(|value| canonical_identifier(value)),
        _ => None,
    }
}

fn single_effective_identifier(attribute: Option<&SemanticSnapshotAttribute>) -> Option<String> {
    let value = attribute?.effective_value.as_ref()?;
    single_identifier_from_value(value)
}

fn single_effective_identifier_from_attribute(
    attribute: &SemanticSnapshotAttribute,
) -> Option<String> {
    attribute
        .effective_value
        .as_ref()
        .and_then(single_identifier_from_value)
}

fn single_identifier_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(canonical_identifier(raw)),
        Value::Array(items) if items.len() == 1 => items.first().and_then(|item| match item {
            Value::String(raw) => Some(canonical_identifier(raw)),
            _ => None,
        }),
        _ => None,
    }
}

fn element_source_file_matches(element: &crate::model::Element, source_file: &str) -> bool {
    element
        .properties
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("source_file"))
        .and_then(Value::as_str)
        .is_some_and(|candidate| {
            normalize_source_file(candidate) == normalize_source_file(source_file)
        })
}

fn element_start_line(element: &crate::model::Element) -> Option<u64> {
    element
        .properties
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("source_span"))
        .and_then(Value::as_object)
        .and_then(|span| span.get("start_line"))
        .and_then(Value::as_u64)
}

fn element_name(element: &crate::model::Element) -> Option<String> {
    element
        .properties
        .get("declared_name")
        .or_else(|| element.properties.get("name"))
        .and_then(Value::as_str)
        .map(canonical_identifier)
}

fn element_type_matches(element: &crate::model::Element, expected: &str) -> bool {
    match element.properties.get("type") {
        Some(Value::String(raw)) => canonical_identifier(raw) == expected,
        Some(Value::Array(items)) => items.iter().any(|item| match item {
            Value::String(raw) => canonical_identifier(raw) == expected,
            _ => false,
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{
        SemanticSnapshotAttribute, SnapshotMode, attribute_is_compare_optional_when_missing,
        attribute_values_are_equal, build_semantic_snapshot, build_semantic_snapshot_with_registry,
        canonical_compare_identifier_for_key, compare_snapshots,
        semantic_model_compare_report_from_diff,
    };
    use crate::kir::{KirDocument, KirElement};
    use crate::model::{Graph, MetamodelAttributeRegistry};
    use crate::semantic_services::{WorkspaceRevision, diff_kir_documents};

    #[test]
    fn builds_and_compares_basic_snapshots() {
        let registry = MetamodelAttributeRegistry::build(
            &Graph::from_document(KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "Model::Systems::PartDefinition".to_string(),
                        kind: "MetadataDefinition".to_string(),
                        layer: 1,
                        properties: BTreeMap::new(),
                    },
                    KirElement {
                        id: "Model::Systems::Definition".to_string(),
                        kind: "MetadataDefinition".to_string(),
                        layer: 1,
                        properties: BTreeMap::new(),
                    },
                    KirElement {
                        id: "Core::Core::Type".to_string(),
                        kind: "Metaclass".to_string(),
                        layer: 0,
                        properties: BTreeMap::new(),
                    },
                ],
            })
            .unwrap(),
        );
        let mercurio = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        json!(["Model::Systems::ItemDefinition"]),
                    )]),
                },
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        (
                            "specializes".to_string(),
                            json!(["Model::Systems::PartDefinition"]),
                        ),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 2, "end_line": 4 }
                            }),
                        ),
                    ]),
                },
            ],
        };
        let pilot = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Demo::Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        (
                            "specializes".to_string(),
                            json!(["Model::Systems::PartDefinition"]),
                        ),
                        (
                            "metatype_specialization_chain".to_string(),
                            json!(["Definition", "Type"]),
                        ),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 2, "end_line": 4 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let mercurio_snapshot = build_semantic_snapshot_with_registry(
            mercurio,
            "demo.model",
            SnapshotMode::Mercurio,
            &registry,
        )
        .unwrap();
        let pilot_snapshot = build_semantic_snapshot_with_registry(
            pilot,
            "demo.model",
            SnapshotMode::Pilot,
            &registry,
        )
        .unwrap();
        let report = compare_snapshots(mercurio_snapshot, pilot_snapshot).unwrap();

        assert_eq!(report.mercurio_count, 1);
        assert_eq!(report.pilot_count, 1);
        assert_eq!(report.exact_match_count, 0);
        assert_eq!(report.mismatches.len(), 1);
        assert_eq!(report.mismatches[0].match_key, "demo.model:2:Vehicle");
    }

    #[test]
    fn semantic_model_compare_report_groups_diff_rows_for_display() {
        let before = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "part.Vehicle".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([
                    ("qualified_name".to_string(), json!("Demo.Vehicle")),
                    ("declared_name".to_string(), json!("Vehicle")),
                ]),
            }],
        };
        let after = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "part.Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("qualified_name".to_string(), json!("Demo.Vehicle")),
                        ("declared_name".to_string(), json!("Vehicle")),
                        ("documentation".to_string(), json!("updated")),
                    ]),
                },
                KirElement {
                    id: "constraint.SafeStart".to_string(),
                    kind: "ConstraintUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("qualified_name".to_string(), json!("Demo.SafeStart")),
                        ("declared_name".to_string(), json!("SafeStart")),
                    ]),
                },
            ],
        };

        let diff = diff_kir_documents(&before, &after);
        let report = semantic_model_compare_report_from_diff(
            WorkspaceRevision {
                fingerprint: "left".to_string(),
            },
            Some(WorkspaceRevision {
                fingerprint: "right".to_string(),
            }),
            &diff,
        );

        assert_eq!(report.schema_version, "mercurio.semantic_model_compare.v1");
        assert_eq!(report.summary.added_count, 1);
        assert_eq!(report.summary.modified_count, 1);
        assert!(
            report
                .sections
                .iter()
                .any(|section| { section.key == "constraints" && section.added_count == 1 })
        );
        assert!(report.changes.iter().any(|change| {
            change.element.element_id == "part.Vehicle"
                && change.property_changes.iter().any(|property| {
                    property.property == "documentation" && property.after == Some(json!("updated"))
                })
        }));
    }

    #[test]
    fn filters_pilot_connection_end_typing_artifacts_from_snapshot() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "feature.Demo.PressureSeat.bead".to_string(),
                    kind: "PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("bead")),
                        ("type".to_string(), json!("TireBead")),
                        ("is_end".to_string(), json!(true)),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 15, "end_line": 15 }
                            }),
                        ),
                    ]),
                },
                KirElement {
                    id: "pilot-anon-ref".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("bead")),
                        ("type".to_string(), json!("TireBead")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 15, "end_line": 15 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let snapshot =
            build_semantic_snapshot(document, "demo.model", SnapshotMode::Pilot).unwrap();

        assert_eq!(snapshot.elements.len(), 1);
        assert_eq!(snapshot.elements[0].kind, "PartUsage");
        assert_eq!(snapshot.elements[0].label, "bead");
    }

    #[test]
    fn uses_name_property_to_disambiguate_same_line_anonymous_pilot_elements() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "InputModel::demo::25::p".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("name".to_string(), json!("p")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 25, "end_line": 25 }
                            }),
                        ),
                    ]),
                },
                KirElement {
                    id: "InputModel::demo::25::receiver".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("name".to_string(), json!("receiver")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.model",
                                "source_span": { "start_line": 25, "end_line": 25 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let snapshot =
            build_semantic_snapshot(document, "demo.model", SnapshotMode::Pilot).unwrap();
        let keys = snapshot
            .elements
            .iter()
            .map(|element| element.match_key.clone())
            .collect::<Vec<_>>();

        assert_eq!(keys, vec!["demo.model:25:p", "demo.model:25:receiver"]);
    }

    #[test]
    fn normalizes_pilot_synthesized_featuring_type_names() {
        assert_eq!(
            canonical_compare_identifier_for_key(
                "featuring_type",
                "'Parts Example-1'::smallVehicle_eng"
            ),
            "eng"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "bigVehicle_eng"),
            "eng"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "'Parts Example-1'::Vehicle"),
            "Vehicle"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "reference.PartTest.C.y.38"),
            "y"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("owner", "reference.PartTest.C.y.38"),
            "y"
        );
    }

    #[test]
    fn compares_attributes_by_matching_effective_value() {
        let effective_only = SemanticSnapshotAttribute {
            declared_by: None,
            origin_kind: "direct".to_string(),
            has_direct_value: false,
            direct_value: None,
            has_effective_value: true,
            effective_value: Some(json!("payload")),
        };
        let direct = SemanticSnapshotAttribute {
            declared_by: None,
            origin_kind: "derived".to_string(),
            has_direct_value: true,
            direct_value: Some(json!("payload")),
            has_effective_value: true,
            effective_value: Some(json!("payload")),
        };

        assert!(attribute_values_are_equal(&effective_only, &direct));
    }

    #[test]
    fn treats_effective_only_qualified_name_as_optional_when_missing() {
        let qualified_name = SemanticSnapshotAttribute {
            declared_by: None,
            origin_kind: "direct".to_string(),
            has_direct_value: false,
            direct_value: None,
            has_effective_value: true,
            effective_value: Some(json!("payload")),
        };

        assert!(attribute_is_compare_optional_when_missing(
            "qualifiedName",
            &qualified_name,
            None
        ));
    }

    #[test]
    fn normalizes_owned_comment_and_documentation_ids() {
        assert_eq!(
            canonical_compare_identifier_for_key("documentation", "doc.type.Comments.C.1"),
            "Documentation"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("ownedElement", "comment.Comments.cmt.6.2"),
            "cmt"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("ownedElement", "comment.Comments.comment.9.2"),
            "Comment"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("ownedElement", "1"),
            "Documentation"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("ownedElement", "comment.Comments.C.comment.12.3"),
            "Comment"
        );
    }
}
