use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use serde_json::Value;

use crate::authoring::frontend::ast::{
    AliasDecl, Declaration, GenericDefinitionDecl, GenericUsageDecl, ImportDecl, PackageDecl,
    ParsedModule, SourceSpan,
};
use crate::kir::{KirDocument, KirElement};
use crate::model::Graph;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EditorOutlineNodeDto {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    pub label: String,
    pub kind: String,
    pub start_line_number: usize,
    pub start_column: usize,
    pub end_line_number: usize,
    pub end_column: usize,
    pub properties: BTreeMap<String, Value>,
    pub children: Vec<EditorOutlineNodeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorOutlineKey {
    source_file: String,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

impl EditorOutlineKey {
    pub fn new(source_file: &str, span: &SourceSpan) -> Self {
        Self {
            source_file: normalize_source_file_key(source_file),
            start_line: span.start_line,
            start_column: span.start_col,
            end_line: span.end_line,
            end_column: span.end_col,
        }
    }

    #[cfg(test)]
    pub fn from_parts(
        source_file: impl Into<String>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            source_file: normalize_source_file_key(&source_file.into()),
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }
}

pub fn build_editor_outline(
    relative_path: &str,
    module: &ParsedModule,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> Vec<EditorOutlineNodeDto> {
    if let Some(package) = &module.package {
        return vec![package_outline_node(relative_path, package, element_index)];
    }

    let mut nodes = outline_nodes_for_declarations(relative_path, &module.members, element_index);
    if nodes.is_empty() {
        nodes.extend(
            module
                .imports
                .iter()
                .map(|import| import_outline_node(relative_path, import, element_index)),
        );
        nodes.extend(
            module
                .definition_like_declarations()
                .iter()
                .map(|definition| {
                    generic_definition_outline_node(relative_path, definition, element_index)
                }),
        );
    }

    nodes
}

pub fn build_editor_outline_index_for_graph(graph: &Graph) -> HashMap<EditorOutlineKey, String> {
    let mut index = HashMap::new();
    for element in graph.elements() {
        let Some((source_file, start_line, start_column, end_line, end_column)) =
            editor_outline_key_parts_for_properties(&element.properties.to_btree_map())
        else {
            continue;
        };
        for candidate_source_file in source_file_suffix_candidates(&source_file) {
            index
                .entry(EditorOutlineKey {
                    source_file: candidate_source_file,
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                })
                .or_insert_with(|| element.element_id.clone());
        }
    }
    index
}

pub fn build_semantic_editor_outline_from_document(
    relative_path: &str,
    document: &KirDocument,
) -> Vec<EditorOutlineNodeDto> {
    let mut items = document
        .elements
        .iter()
        .filter_map(|element| semantic_outline_item_from_kir(relative_path, element))
        .collect::<Vec<_>>();
    items.sort_by(semantic_outline_item_order);

    let items_by_id = items
        .iter()
        .cloned()
        .map(|item| (item.id.clone(), item))
        .collect::<HashMap<_, _>>();
    let parent_by_id = items
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                semantic_outline_parent_id(item, &items, &items_by_id),
            )
        })
        .collect::<HashMap<_, _>>();

    semantic_outline_nodes(None, &items, &parent_by_id)
}

fn outline_nodes_for_declarations(
    relative_path: &str,
    declarations: &[Declaration],
    element_index: &HashMap<EditorOutlineKey, String>,
) -> Vec<EditorOutlineNodeDto> {
    declarations
        .iter()
        .map(|declaration| declaration_outline_node(relative_path, declaration, element_index))
        .collect()
}

fn declaration_outline_node(
    relative_path: &str,
    declaration: &Declaration,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    if let Some(definition) = declaration.as_definition_like() {
        return generic_definition_outline_node(relative_path, &definition, element_index);
    }
    if let Some(usage) = declaration.as_usage_like() {
        return generic_usage_outline_node(relative_path, &usage, element_index);
    }

    match declaration {
        Declaration::Package(package) => {
            package_outline_node(relative_path, package, element_index)
        }
        Declaration::Import(import) => import_outline_node(relative_path, import, element_index),
        Declaration::GenericDefinition(_) | Declaration::GenericUsage(_) => {
            unreachable!("generic declarations are handled by projection helpers")
        }
        Declaration::Alias(alias) => alias_outline_node(relative_path, alias, element_index),
    }
}

fn package_outline_node(
    relative_path: &str,
    package: &PackageDecl,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    let mut children =
        outline_nodes_for_declarations(relative_path, &package.members, element_index);
    if children.is_empty() {
        children.extend(
            package
                .imports
                .iter()
                .map(|import| import_outline_node(relative_path, import, element_index)),
        );
        children.extend(
            package
                .definition_like_declarations()
                .iter()
                .map(|definition| {
                    generic_definition_outline_node(relative_path, definition, element_index)
                }),
        );
    }

    outline_node(
        relative_path,
        element_index,
        "package",
        &package.name.as_colon_string(),
        "package",
        &package.span,
        BTreeMap::from([
            (
                "declared_name".to_string(),
                Value::String(
                    package
                        .name
                        .segments
                        .last()
                        .cloned()
                        .unwrap_or_else(|| package.name.as_colon_string()),
                ),
            ),
            (
                "qualified_name".to_string(),
                Value::String(package.name.as_colon_string()),
            ),
            (
                "member_count".to_string(),
                Value::from(children.len() as u64),
            ),
            (
                "docs".to_string(),
                Value::Array(package.docs.iter().cloned().map(Value::String).collect()),
            ),
            (
                "modifiers".to_string(),
                Value::Array(
                    package
                        .modifiers
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]),
        children,
    )
}

fn import_outline_node(
    relative_path: &str,
    import: &ImportDecl,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    outline_node(
        relative_path,
        element_index,
        "import",
        &import.path.as_colon_string(),
        "import",
        &import.span,
        BTreeMap::from([
            (
                "path".to_string(),
                Value::String(import.path.as_colon_string()),
            ),
            (
                "docs".to_string(),
                Value::Array(import.docs.iter().cloned().map(Value::String).collect()),
            ),
            (
                "modifiers".to_string(),
                Value::Array(
                    import
                        .modifiers
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]),
        Vec::new(),
    )
}

fn generic_definition_outline_node(
    relative_path: &str,
    definition: &GenericDefinitionDecl,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    outline_node(
        relative_path,
        element_index,
        &format!("{}_definition", definition.keyword),
        &definition.name,
        &format!("{} def", definition.keyword),
        &definition.span,
        BTreeMap::from([
            (
                "keyword".to_string(),
                Value::String(definition.keyword.clone()),
            ),
            ("name".to_string(), Value::String(definition.name.clone())),
            (
                "specializes".to_string(),
                Value::Array(
                    definition
                        .specializes
                        .iter()
                        .map(|item| Value::String(item.as_colon_string()))
                        .collect(),
                ),
            ),
            (
                "member_count".to_string(),
                Value::from(definition.members.len() as u64),
            ),
            (
                "docs".to_string(),
                Value::Array(definition.docs.iter().cloned().map(Value::String).collect()),
            ),
            (
                "modifiers".to_string(),
                Value::Array(
                    definition
                        .modifiers
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]),
        outline_nodes_for_declarations(relative_path, &definition.members, element_index),
    )
}

fn generic_usage_outline_node(
    relative_path: &str,
    usage: &GenericUsageDecl,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    outline_node(
        relative_path,
        element_index,
        &format!("{}_usage", usage.keyword),
        &usage.name,
        &usage.keyword,
        &usage.span,
        BTreeMap::from([
            ("keyword".to_string(), Value::String(usage.keyword.clone())),
            ("name".to_string(), Value::String(usage.name.clone())),
            (
                "type".to_string(),
                usage
                    .ty
                    .as_ref()
                    .map(|item| Value::String(item.as_colon_string()))
                    .unwrap_or(Value::Null),
            ),
            (
                "additional_types".to_string(),
                Value::Array(
                    usage
                        .additional_types
                        .iter()
                        .map(|item| Value::String(item.as_colon_string()))
                        .collect(),
                ),
            ),
            (
                "specializes".to_string(),
                Value::Array(
                    usage
                        .specializes
                        .iter()
                        .map(|item| Value::String(item.as_colon_string()))
                        .collect(),
                ),
            ),
            (
                "member_count".to_string(),
                Value::from(usage.body_members.len() as u64),
            ),
            (
                "docs".to_string(),
                Value::Array(usage.docs.iter().cloned().map(Value::String).collect()),
            ),
            (
                "modifiers".to_string(),
                Value::Array(usage.modifiers.iter().cloned().map(Value::String).collect()),
            ),
        ]),
        outline_nodes_for_declarations(relative_path, &usage.body_members, element_index),
    )
}

fn alias_outline_node(
    relative_path: &str,
    alias: &AliasDecl,
    element_index: &HashMap<EditorOutlineKey, String>,
) -> EditorOutlineNodeDto {
    outline_node(
        relative_path,
        element_index,
        "alias",
        &alias.name,
        "alias",
        &alias.span,
        BTreeMap::from([
            ("name".to_string(), Value::String(alias.name.clone())),
            (
                "target".to_string(),
                Value::String(alias.target.as_colon_string()),
            ),
            (
                "docs".to_string(),
                Value::Array(alias.docs.iter().cloned().map(Value::String).collect()),
            ),
            (
                "modifiers".to_string(),
                Value::Array(alias.modifiers.iter().cloned().map(Value::String).collect()),
            ),
        ]),
        Vec::new(),
    )
}

fn outline_node(
    relative_path: &str,
    element_index: &HashMap<EditorOutlineKey, String>,
    kind_key: &str,
    label: &str,
    kind: &str,
    span: &SourceSpan,
    properties: BTreeMap<String, Value>,
    children: Vec<EditorOutlineNodeDto>,
) -> EditorOutlineNodeDto {
    let element_id = element_index
        .get(&EditorOutlineKey::new(relative_path, span))
        .cloned();
    EditorOutlineNodeDto {
        id: format!(
            "{relative_path}:{kind_key}:{}:{}:{}:{}",
            span.start_line, span.start_col, span.end_line, span.end_col
        ),
        element_id,
        label: label.to_string(),
        kind: kind.to_string(),
        start_line_number: span.start_line,
        start_column: span.start_col,
        end_line_number: span.end_line,
        end_column: span.end_col,
        properties,
        children,
    }
}

fn editor_outline_key_parts_for_properties(
    properties: &BTreeMap<String, Value>,
) -> Option<(String, usize, usize, usize, usize)> {
    let metadata = properties.get("metadata")?.as_object()?;
    let source_file = metadata.get("source_file")?.as_str()?;
    let span = metadata.get("source_span")?.as_object()?;

    Some((
        normalize_source_file_key(source_file),
        span.get("start_line")?.as_u64()? as usize,
        span.get("start_col")?.as_u64()? as usize,
        span.get("end_line")?.as_u64()? as usize,
        span.get("end_col")?.as_u64()? as usize,
    ))
}

fn normalize_source_file_key(source_file: &str) -> String {
    source_file.replace('\\', "/")
}

fn source_file_suffix_candidates(source_file: &str) -> Vec<String> {
    let normalized = normalize_source_file_key(source_file);
    let segments = normalized.split('/').collect::<Vec<_>>();
    let mut candidates = Vec::with_capacity(segments.len());
    for start in 0..segments.len() {
        candidates.push(segments[start..].join("/"));
    }
    candidates
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticOutlineItem {
    id: String,
    label: String,
    kind: String,
    properties: BTreeMap<String, Value>,
    span: EditorOutlineKey,
    owner_id: Option<String>,
}

fn semantic_outline_item_from_kir(
    relative_path: &str,
    element: &KirElement,
) -> Option<SemanticOutlineItem> {
    if element.layer != 2 {
        return None;
    }

    let (source_file, start_line, start_column, end_line, end_column) =
        editor_outline_key_parts_for_properties(&element.properties)?;
    if !source_file_matches_relative_path(&source_file, relative_path) {
        return None;
    }

    Some(SemanticOutlineItem {
        id: element.id.clone(),
        label: semantic_outline_label_from_properties(&element.id, &element.properties),
        kind: semantic_outline_kind(&element.kind),
        properties: element.properties.clone(),
        span: EditorOutlineKey {
            source_file: normalize_source_file_key(relative_path),
            start_line,
            start_column,
            end_line,
            end_column,
        },
        owner_id: element
            .properties
            .get("owner")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn semantic_outline_label_from_properties(
    element_id: &str,
    properties: &BTreeMap<String, Value>,
) -> String {
    properties
        .get("declared_name")
        .and_then(Value::as_str)
        .or_else(|| properties.get("qualified_name").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| label_for_id(element_id))
}

fn semantic_outline_kind(kind: &str) -> String {
    kind.rsplit("::").next().unwrap_or(kind).to_string()
}

fn source_file_matches_relative_path(source_file: &str, relative_path: &str) -> bool {
    let relative_path = normalize_source_file_key(relative_path);
    source_file_suffix_candidates(source_file)
        .into_iter()
        .any(|candidate| candidate == relative_path)
}

fn semantic_outline_parent_id(
    item: &SemanticOutlineItem,
    items: &[SemanticOutlineItem],
    items_by_id: &HashMap<String, SemanticOutlineItem>,
) -> Option<String> {
    if let Some(owner_id) = item.owner_id.as_ref() {
        if let Some(owner) = items_by_id.get(owner_id) {
            if owner.span != item.span && span_contains(&owner.span, &item.span) {
                return Some(owner.id.clone());
            }
        }
    }

    items
        .iter()
        .filter(|candidate| candidate.id != item.id)
        .filter(|candidate| span_contains(&candidate.span, &item.span))
        .min_by(|left, right| {
            (span_extent(&left.span), &left.id).cmp(&(span_extent(&right.span), &right.id))
        })
        .map(|candidate| candidate.id.clone())
}

fn semantic_outline_nodes(
    parent_id: Option<&str>,
    items: &[SemanticOutlineItem],
    parent_by_id: &HashMap<String, Option<String>>,
) -> Vec<EditorOutlineNodeDto> {
    let mut children = items
        .iter()
        .filter(|item| {
            parent_by_id
                .get(&item.id)
                .and_then(|value| value.as_deref())
                == parent_id
        })
        .cloned()
        .collect::<Vec<_>>();
    children.sort_by(semantic_outline_item_order);

    children
        .into_iter()
        .map(|item| EditorOutlineNodeDto {
            id: item.id.clone(),
            element_id: Some(item.id.clone()),
            label: item.label,
            kind: item.kind,
            start_line_number: item.span.start_line,
            start_column: item.span.start_column,
            end_line_number: item.span.end_line,
            end_column: item.span.end_column,
            properties: item.properties,
            children: semantic_outline_nodes(Some(&item.id), items, parent_by_id),
        })
        .collect()
}

fn semantic_outline_item_order(
    left: &SemanticOutlineItem,
    right: &SemanticOutlineItem,
) -> std::cmp::Ordering {
    (
        left.span.start_line,
        left.span.start_column,
        span_extent(&left.span),
        &left.id,
    )
        .cmp(&(
            right.span.start_line,
            right.span.start_column,
            span_extent(&right.span),
            &right.id,
        ))
}

fn span_contains(container: &EditorOutlineKey, candidate: &EditorOutlineKey) -> bool {
    if container.source_file != candidate.source_file || container == candidate {
        return false;
    }

    let starts_before_or_at = (container.start_line, container.start_column)
        <= (candidate.start_line, candidate.start_column);
    let ends_after_or_at =
        (container.end_line, container.end_column) >= (candidate.end_line, candidate.end_column);

    starts_before_or_at && ends_after_or_at
}

fn span_extent(span: &EditorOutlineKey) -> (usize, usize, usize, usize) {
    (
        span.end_line.saturating_sub(span.start_line),
        span.end_column.saturating_sub(span.start_column),
        span.end_line,
        span.end_column,
    )
}

fn label_for_id(id: &str) -> String {
    id.rsplit(['.', ':']).next().unwrap_or(id).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    fn element(
        id: &str,
        kind: &str,
        source_file: &str,
        span: (usize, usize, usize, usize),
        properties: BTreeMap<String, Value>,
    ) -> KirElement {
        let mut properties = properties;
        properties.insert(
            "metadata".to_string(),
            json!({
                "source_file": source_file,
                "source_span": {
                    "start_line": span.0,
                    "start_col": span.1,
                    "end_line": span.2,
                    "end_col": span.3
                }
            }),
        );

        KirElement {
            id: id.to_string(),
            kind: kind.to_string(),
            layer: 2,
            properties,
        }
    }

    #[test]
    fn editor_outline_index_matches_source_file_suffixes() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![element(
                "type.Demo.Vehicle",
                "Model::Systems::PartDefinition",
                "test_files/l2/minimal_vehicle.model",
                (11, 3, 13, 3),
                BTreeMap::new(),
            )],
        };
        let graph = Graph::from_document(document).unwrap();

        let index = build_editor_outline_index_for_graph(&graph);

        assert_eq!(
            index.get(&EditorOutlineKey::from_parts(
                "minimal_vehicle.model",
                11,
                3,
                13,
                3
            )),
            Some(&"type.Demo.Vehicle".to_string())
        );
    }

    #[test]
    fn semantic_outline_groups_elements_by_owner_and_source_span() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element(
                    "pkg.Demo",
                    "Model::Systems::Package",
                    "test_files/l2/minimal_vehicle.model",
                    (1, 1, 20, 2),
                    BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("Demo".to_string()),
                    )]),
                ),
                element(
                    "type.Demo.Vehicle",
                    "Model::Systems::PartDefinition",
                    "test_files/l2/minimal_vehicle.model",
                    (3, 3, 10, 4),
                    BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("Vehicle".to_string()),
                        ),
                        ("owner".to_string(), Value::String("pkg.Demo".to_string())),
                    ]),
                ),
                element(
                    "feature.Demo.Vehicle.engine",
                    "Model::Systems::PartUsage",
                    "test_files/l2/minimal_vehicle.model",
                    (5, 5, 5, 22),
                    BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("engine".to_string()),
                        ),
                        (
                            "owner".to_string(),
                            Value::String("type.Demo.Vehicle".to_string()),
                        ),
                    ]),
                ),
            ],
        };

        let outline =
            build_semantic_editor_outline_from_document("minimal_vehicle.model", &document);

        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].id, "pkg.Demo");
        assert_eq!(outline[0].label, "Demo");
        assert_eq!(outline[0].children.len(), 1);
        assert_eq!(outline[0].children[0].id, "type.Demo.Vehicle");
        assert_eq!(outline[0].children[0].label, "Vehicle");
        assert_eq!(outline[0].children[0].children.len(), 1);
        assert_eq!(
            outline[0].children[0].children[0].id,
            "feature.Demo.Vehicle.engine"
        );
        assert_eq!(outline[0].children[0].children[0].label, "engine");
    }
}
