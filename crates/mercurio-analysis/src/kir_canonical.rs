//! KIR equivalence oracle: canonicalization pass feeding `diff_kir_documents`.
//!
//! Compiled KIR documents are position-sensitive in two ways: every element
//! carries a `metadata` property (`source_file` / `source_span` /
//! `source_language` / `generated` / `lowering`), and anonymous constructs
//! (connections, successions, flows, comments, imports, reference ends, ...)
//! embed `{start_line}_{start_col}` / `{start_line}.{start_col}` /
//! `{start_line}` / ordinal suffixes in their element ids. Two compiles of the
//! same model authored differently (reordered declarations, reformatted text,
//! split across files) therefore raw-diff as different documents even though
//! they are semantically identical.
//!
//! [`canonicalize_kir_document`] normalizes both artifacts in order:
//!
//! 1. strip each element's `metadata` property (and the volatile
//!    `parsed_from` document metadata entry);
//! 2. rewrite position-derived ids to content-derived keys
//!    (`{base}.c{stable-content-hash}`, with deterministic `#n` suffixing for
//!    content-identical siblings). Detection is generic — any id with a
//!    trailing `\d+_\d+`, `\d+.\d+`, or bare ordinal segment is treated as
//!    position-derived — rather than hard-coding the known emission-template
//!    prefixes;
//! 3. apply the id map to every string reference value (scalars, arrays, and
//!    `{id|element_id|qualified_name|ref}` objects, mirroring how
//!    `reference_values` in `mercurio_semantic_services::mutation` finds
//!    them), then sort order-insensitive reference-list properties;
//! 4. re-sort elements by canonical id.
//!
//! [`kir_equivalence_diff`] canonicalizes both sides and delegates to
//! [`diff_kir_documents`]; an empty diff means the documents are equivalent.

use std::collections::{BTreeMap, BTreeSet};

use mercurio_kir::{KirDocument, KirElement};
use mercurio_semantic_services::identity::stable_digest;
use mercurio_semantic_services::mutation::{SemanticDiff, diff_kir_documents};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const KIR_EQUIVALENCE_REPORT_SCHEMA_VERSION: &str = "1.0";

/// Document metadata entries that record where the document was parsed from
/// rather than what it means.
const VOLATILE_DOCUMENT_METADATA: &[&str] = &["parsed_from"];

/// Properties that anchor an element to its owner. They are re-pointed by the
/// id map but excluded from content hashing (the owner path is already part of
/// the canonical id base, and including parent links would create
/// parent/child hash cycles).
const OWNER_PROPERTIES: &[&str] = &[
    "owner",
    "owning_namespace",
    "owning_type",
    "owning_definition",
    "featuring_type",
];

/// Reference-list properties whose order carries no semantic weight (member
/// order in a namespace is declaration order, which is exactly the
/// position-sensitivity being normalized away). Sorted after id rewriting.
const ORDER_INSENSITIVE_LIST_PROPERTIES: &[&str] =
    &["members", "features", "sources", "targets", "relationships"];

/// Object fields that carry a reference value, mirroring `reference_values`
/// in `mercurio_semantic_services::mutation`.
const REFERENCE_OBJECT_FIELDS: &[&str] = &["id", "element_id", "qualified_name", "ref"];

/// A canonicalized document plus the id rewrites that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalizedKir {
    pub document: KirDocument,
    /// Original position-derived element id -> canonical content-derived id.
    pub id_map: BTreeMap<String, String>,
}

/// Serializable outcome of one equivalence check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KirEquivalenceReport {
    pub schema_version: String,
    pub equivalent: bool,
    pub diff: SemanticDiff,
    /// Position-derived id rewrites applied to the left document.
    pub left_id_map: BTreeMap<String, String>,
    /// Position-derived id rewrites applied to the right document.
    pub right_id_map: BTreeMap<String, String>,
}

/// Normalize position-sensitive artifacts out of a compiled KIR document.
pub fn canonicalize_kir_document(document: &KirDocument) -> CanonicalizedKir {
    let mut document = document.clone();

    for key in VOLATILE_DOCUMENT_METADATA {
        document.metadata.remove(*key);
    }
    for element in &mut document.elements {
        element.properties.remove("metadata");
    }

    let id_map = build_id_map(&document);

    for element in &mut document.elements {
        if let Some(canonical) = id_map.get(&element.id) {
            // `qualified_name` is synthesized from the element id during KIR
            // merge normalization, so it carries the same position-derived
            // tail; rewrite it with the same canonical suffix.
            let suffix = positional_id_base(&element.id)
                .and_then(|base| canonical.get(base.len() + 1..))
                .map(ToOwned::to_owned);
            element.id = canonical.clone();
            if let (Some(suffix), Some(Value::String(qualified_name))) = (
                suffix,
                element.properties.get_mut("qualified_name"),
            ) {
                if let Some(base) = strip_positional_tail(qualified_name) {
                    *qualified_name = format!("{base}.{suffix}");
                }
            }
        }
        for (name, value) in element.properties.iter_mut() {
            apply_id_map(value, &id_map);
            if ORDER_INSENSITIVE_LIST_PROPERTIES.contains(&name.as_str()) {
                sort_reference_list(value);
            }
        }
    }

    document.elements.sort_by(|a, b| a.id.cmp(&b.id));

    CanonicalizedKir { document, id_map }
}

/// Canonicalize both documents, then diff them with the existing
/// order-insensitive semantic diff. An empty diff means equivalence.
pub fn kir_equivalence_diff(left: &KirDocument, right: &KirDocument) -> SemanticDiff {
    let left = canonicalize_kir_document(left);
    let right = canonicalize_kir_document(right);
    diff_kir_documents(&left.document, &right.document)
}

/// True when the two documents are semantically equivalent modulo authoring
/// position (source spans, declaration order, file layout).
pub fn kir_documents_equivalent(left: &KirDocument, right: &KirDocument) -> bool {
    semantic_diff_is_empty(&kir_equivalence_diff(left, right))
}

/// Full serializable report: verdict, diff, and the id rewrites applied to
/// each side.
pub fn kir_equivalence_report(left: &KirDocument, right: &KirDocument) -> KirEquivalenceReport {
    let left = canonicalize_kir_document(left);
    let right = canonicalize_kir_document(right);
    let diff = diff_kir_documents(&left.document, &right.document);
    KirEquivalenceReport {
        schema_version: KIR_EQUIVALENCE_REPORT_SCHEMA_VERSION.to_string(),
        equivalent: semantic_diff_is_empty(&diff),
        diff,
        left_id_map: left.id_map,
        right_id_map: right.id_map,
    }
}

/// True when a semantic diff reports no changes at all.
pub fn semantic_diff_is_empty(diff: &SemanticDiff) -> bool {
    diff.added_elements.is_empty()
        && diff.removed_elements.is_empty()
        && diff.renamed_elements.is_empty()
        && diff.moved_elements.is_empty()
        && diff.retyped_usages.is_empty()
        && diff.changed_specializations.is_empty()
        && diff.changed_attributes.is_empty()
        && diff.added_relationships.is_empty()
        && diff.removed_relationships.is_empty()
}

// --- position-derived id detection -----------------------------------------

fn is_numeric_segment(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_line_col_segment(segment: &str) -> bool {
    match segment.split_once('_') {
        Some((line, col)) => is_numeric_segment(line) && is_numeric_segment(col),
        None => false,
    }
}

/// Number of trailing position-derived segments (`{line}_{col}`,
/// `{line}.{col}`, or a bare ordinal/line number).
fn positional_tail_len(segments: &[&str]) -> usize {
    let Some(last) = segments.last() else {
        return 0;
    };
    if is_line_col_segment(last) {
        1
    } else if is_numeric_segment(last) {
        if segments.len() >= 2 && is_numeric_segment(segments[segments.len() - 2]) {
            // `comment.{owner_path}.{name}.{start_line}.{start_col}`
            2
        } else {
            // `connection...{start_line}` / `import.{owner_id}.{ordinal}`
            1
        }
    } else {
        0
    }
}

/// If `value` (a dotted path, id or qualified name) carries a
/// position-derived tail, return it with the tail stripped.
fn strip_positional_tail(value: &str) -> Option<String> {
    let segments = value.split('.').collect::<Vec<_>>();
    let tail = positional_tail_len(&segments);
    if tail == 0 || segments.len() <= tail {
        return None;
    }
    let base = &segments[..segments.len() - tail];
    let head = base[0];
    if head.is_empty() || is_numeric_segment(head) || is_line_col_segment(head) {
        return None;
    }
    Some(base.join("."))
}

/// If `id` carries a position-derived tail, return the id base with the tail
/// stripped (`connection.Demo.Connection.11` -> `connection.Demo.Connection`).
/// Stricter than [`strip_positional_tail`]: an element id keeps at least a
/// template prefix plus one path segment.
fn positional_id_base(id: &str) -> Option<String> {
    let segments = id.split('.').collect::<Vec<_>>();
    if segments.len() < 3 {
        return None;
    }
    let tail = positional_tail_len(&segments);
    if tail == 0 || segments.len() - tail < 2 {
        return None;
    }
    strip_positional_tail(id)
}

// --- content-derived canonical ids -----------------------------------------

struct ContentKeyContext<'a> {
    by_id: BTreeMap<&'a str, &'a KirElement>,
    children: BTreeMap<&'a str, Vec<&'a str>>,
    positional: BTreeSet<&'a str>,
    memo: BTreeMap<String, String>,
}

fn build_id_map(document: &KirDocument) -> BTreeMap<String, String> {
    let mut positional_bases = BTreeMap::new();
    for element in &document.elements {
        if let Some(base) = positional_id_base(&element.id) {
            positional_bases.insert(element.id.as_str(), base);
        }
    }
    if positional_bases.is_empty() {
        return BTreeMap::new();
    }

    let by_id = document
        .elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<BTreeMap<_, _>>();
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for element in &document.elements {
        if let Some(owner) = owner_reference(element) {
            children.entry(owner).or_default().push(element.id.as_str());
        }
    }
    let mut context = ContentKeyContext {
        by_id,
        children,
        positional: positional_bases.keys().copied().collect(),
        memo: BTreeMap::new(),
    };

    let named_ids = document
        .elements
        .iter()
        .filter(|element| !context.positional.contains(element.id.as_str()))
        .map(|element| element.id.clone())
        .collect::<BTreeSet<_>>();

    // Candidate canonical id -> original ids that map to it. Content-identical
    // siblings collide here and receive deterministic `#n` suffixes.
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, base) in &positional_bases {
        let mut in_progress = BTreeSet::new();
        let key = content_key(id, &mut context, &mut in_progress);
        groups
            .entry(format!("{base}.c{key}"))
            .or_default()
            .push((*id).to_string());
    }

    let mut id_map = BTreeMap::new();
    for (candidate, mut ids) in groups {
        ids.sort();
        if ids.len() == 1 && !named_ids.contains(&candidate) {
            if let Some(id) = ids.pop() {
                id_map.insert(id, candidate);
            }
        } else {
            for (index, id) in ids.into_iter().enumerate() {
                id_map.insert(id, format!("{candidate}#{}", index + 1));
            }
        }
    }
    id_map
}

fn owner_reference(element: &KirElement) -> Option<&str> {
    OWNER_PROPERTIES
        .iter()
        .find_map(|key| element.properties.get(*key).and_then(Value::as_str))
}

/// Stable content hash for a position-derived element: covers the element's
/// kind, layer, and non-positional properties (type refs, source/target refs,
/// specializations, literal text, expression payloads), plus the content keys
/// of its owned children (a succession's identity lives in its end reference
/// usages, which the succession element itself does not list). References to
/// other position-derived elements contribute *their* content keys, so the
/// hash never observes line/column numbers.
fn content_key(id: &str, context: &mut ContentKeyContext<'_>, in_progress: &mut BTreeSet<String>) -> String {
    if let Some(key) = context.memo.get(id) {
        return key.clone();
    }
    let Some(element) = context.by_id.get(id).copied() else {
        // Dangling reference to an element outside this document: fall back to
        // the position-stripped id, which is identical on both sides.
        return format!("ext:{}", positional_id_base(id).unwrap_or_else(|| id.to_string()));
    };
    if !in_progress.insert(id.to_string()) {
        // Reference cycle between position-derived elements: fall back to the
        // stripped base so the computation stays deterministic. Such keys are
        // not memoized (see below).
        return format!("cycle:{}", positional_id_base(id).unwrap_or_else(|| id.to_string()));
    }

    let mut properties = serde_json::Map::new();
    for (name, value) in &element.properties {
        if OWNER_PROPERTIES.contains(&name.as_str())
            || ORDER_INSENSITIVE_LIST_PROPERTIES.contains(&name.as_str())
            // Synthesized from the (position-derived) element id during KIR
            // merge normalization; the id base already covers it.
            || name == "qualified_name"
        {
            continue;
        }
        properties.insert(name.clone(), hash_value(value, context, in_progress));
    }

    let mut child_keys = context
        .children
        .get(id)
        .map(|children| children.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|child| {
            if context.positional.contains(child) {
                format!("~{}", content_key(child, context, in_progress))
            } else {
                child.to_string()
            }
        })
        .collect::<Vec<_>>();
    child_keys.sort();

    let payload = serde_json::json!({
        "kind": element.kind,
        "layer": element.layer,
        "properties": Value::Object(properties),
        "children": child_keys,
    });
    let digest = match serde_json::to_vec(&payload) {
        Ok(bytes) => stable_digest([("kir-content-key".as_bytes(), bytes.as_slice())]),
        Err(err) => {
            let message = err.to_string();
            stable_digest([
                ("kir-content-key-error".as_bytes(), id.as_bytes()),
                ("message".as_bytes(), message.as_bytes()),
            ])
        }
    };
    let key = digest
        .rsplit(':')
        .next()
        .map(ToOwned::to_owned)
        .unwrap_or(digest);

    in_progress.remove(id);
    // Only memoize keys computed without an active cycle fallback above this
    // frame; a key observed mid-cycle would depend on traversal entry order.
    if in_progress.is_empty() {
        context.memo.insert(id.to_string(), key.clone());
    }
    key
}

/// Normalize a property value for hashing: any string equal to a
/// position-derived element id is replaced by that element's content key
/// (deep walk — nested arrays and objects included).
fn hash_value(
    value: &Value,
    context: &mut ContentKeyContext<'_>,
    in_progress: &mut BTreeSet<String>,
) -> Value {
    match value {
        Value::String(text) => {
            if context.positional.contains(text.as_str()) {
                let text = text.clone();
                Value::String(format!("~{}", content_key(&text, context, in_progress)))
            } else {
                value.clone()
            }
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| hash_value(item, context, in_progress))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, item)| (key.clone(), hash_value(item, context, in_progress)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

// --- id-map application and reference-list sorting -------------------------

/// Rewrite reference values through the id map, mirroring the shapes
/// `reference_values` recognizes: scalar strings, arrays, and
/// `{id|element_id|qualified_name|ref}` objects.
fn apply_id_map(value: &mut Value, id_map: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(canonical) = id_map.get(text.as_str()) {
                *text = canonical.clone();
            }
        }
        Value::Array(items) => {
            for item in items {
                apply_id_map(item, id_map);
            }
        }
        Value::Object(object) => {
            for field in REFERENCE_OBJECT_FIELDS {
                if let Some(Value::String(text)) = object.get_mut(*field) {
                    if let Some(canonical) = id_map.get(text.as_str()) {
                        *text = canonical.clone();
                    }
                }
            }
        }
        _ => {}
    }
}

fn sort_reference_list(value: &mut Value) {
    if let Value::Array(items) = value {
        items.sort_by_cached_key(|item| match item {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn element(id: &str, kind: &str, properties: Value) -> KirElement {
        let properties = match properties {
            Value::Object(map) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        };
        KirElement {
            id: id.to_string(),
            kind: kind.to_string(),
            layer: 2,
            properties,
        }
    }

    fn document(elements: Vec<KirElement>) -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements,
        }
    }

    fn source_metadata(file: &str, line: u64) -> Value {
        json!({
            "generated": false,
            "source_file": file,
            "source_language": "sysml",
            "source_span": {
                "start_line": line,
                "start_col": 5,
                "end_line": line,
                "end_col": 40
            }
        })
    }

    /// A hand-built compile artifact: package with two parts, one anonymous
    /// connection (position-suffixed id + end reference usages), authored
    /// with configurable source positions and member order.
    fn connection_fixture(file: &str, line: u64, members_reversed: bool) -> KirDocument {
        let connection_id = format!("connection.Demo.Connection.{line}");
        let source_id = format!("reference.Demo.Connection.source.{line}");
        let target_id = format!("reference.Demo.Connection.target.{line}");
        let mut members = vec![
            "type.Demo.Engine".to_string(),
            "type.Demo.Vehicle".to_string(),
            connection_id.clone(),
        ];
        if members_reversed {
            members.reverse();
        }
        document(vec![
            element(
                "pkg.Demo",
                "SysML::Package",
                json!({
                    "declared_name": "Demo",
                    "members": members,
                    "metadata": source_metadata(file, 1)
                }),
            ),
            element(
                "type.Demo.Vehicle",
                "SysML::Systems::PartDefinition",
                json!({
                    "declared_name": "Vehicle",
                    "owner": "pkg.Demo",
                    "metadata": source_metadata(file, line + 1)
                }),
            ),
            element(
                "type.Demo.Engine",
                "SysML::Systems::PartDefinition",
                json!({
                    "declared_name": "Engine",
                    "owner": "pkg.Demo",
                    "metadata": source_metadata(file, line + 2)
                }),
            ),
            element(
                &connection_id,
                "SysML::ConnectionUsage",
                json!({
                    "definition": "Connections::Connection",
                    "members": [source_id.clone(), target_id.clone()],
                    "features": [source_id.clone(), target_id.clone()],
                    "owner": "pkg.Demo",
                    // Synthesized from the element id by KIR merge
                    // normalization, so it carries the same position tail.
                    "qualified_name": format!("Demo.Connection.{line}"),
                    "metadata": source_metadata(file, line)
                }),
            ),
            element(
                &source_id,
                "SysML::ReferenceUsage",
                json!({
                    "declared_name": "source",
                    "definition": "type.Demo.Engine",
                    "featuring_type": connection_id.clone(),
                    "owner": connection_id.clone(),
                    "metadata": source_metadata(file, line)
                }),
            ),
            element(
                &target_id,
                "SysML::ReferenceUsage",
                json!({
                    "declared_name": "target",
                    "definition": "type.Demo.Vehicle",
                    "featuring_type": connection_id.clone(),
                    "owner": connection_id.clone(),
                    "metadata": source_metadata(file, line)
                }),
            ),
        ])
    }

    #[test]
    fn positional_id_detection_matches_known_templates() {
        for (id, base) in [
            (
                "succession-as.Demo.Drive..18_15",
                "succession-as.Demo.Drive.",
            ),
            ("comment.Demo.comment.3.5", "comment.Demo.comment"),
            ("connection.Demo.Connection.11", "connection.Demo.Connection"),
            (
                "reference.Demo.Connection.source.11",
                "reference.Demo.Connection.source",
            ),
            ("import.pkg.Demo.1", "import.pkg.Demo"),
            ("flow.Demo.Drive..7_9", "flow.Demo.Drive."),
        ] {
            assert_eq!(
                positional_id_base(id).as_deref(),
                Some(base),
                "expected {id} to be position-derived"
            );
        }
        for id in [
            "pkg.Demo",
            "type.Demo.Vehicle",
            "feature.Demo.Vehicle.mass",
            "action.Demo.Drive.first_step",
            "feature.Demo.Drive..earlierOccurrence",
        ] {
            assert_eq!(
                positional_id_base(id),
                None,
                "expected {id} to be stable as-is"
            );
        }
    }

    #[test]
    fn strips_element_metadata_and_volatile_document_metadata() {
        let mut doc = connection_fixture("a.sysml", 11, false);
        doc.metadata
            .insert("parsed_from".to_string(), json!("a.sysml"));
        doc.metadata.insert("source".to_string(), json!("sysml"));

        let canonical = canonicalize_kir_document(&doc);

        assert!(!canonical.document.metadata.contains_key("parsed_from"));
        assert!(canonical.document.metadata.contains_key("source"));
        assert!(
            canonical
                .document
                .elements
                .iter()
                .all(|element| !element.properties.contains_key("metadata"))
        );
    }

    #[test]
    fn same_model_at_different_positions_is_equivalent() {
        let left = connection_fixture("a.sysml", 11, false);
        let right = connection_fixture("b.sysml", 40, false);

        let diff = kir_equivalence_diff(&left, &right);
        assert!(
            semantic_diff_is_empty(&diff),
            "expected empty diff, got {diff:?}"
        );
        assert!(kir_documents_equivalent(&left, &right));

        // Without canonicalization the same pair raw-diffs as different.
        let raw = diff_kir_documents(&left, &right);
        assert!(!semantic_diff_is_empty(&raw));
    }

    #[test]
    fn member_order_is_insensitive() {
        let left = connection_fixture("a.sysml", 11, false);
        let right = connection_fixture("a.sysml", 11, true);

        assert!(kir_documents_equivalent(&left, &right));
    }

    #[test]
    fn canonical_ids_are_content_derived_and_applied_to_references() {
        let doc = connection_fixture("a.sysml", 11, false);
        let canonical = canonicalize_kir_document(&doc);

        assert_eq!(canonical.id_map.len(), 3);
        let connection_canonical = canonical
            .id_map
            .get("connection.Demo.Connection.11")
            .cloned()
            .unwrap_or_default();
        assert!(connection_canonical.starts_with("connection.Demo.Connection.c"));
        // The package member list now references the canonical id.
        let package = canonical
            .document
            .elements
            .iter()
            .find(|element| element.id == "pkg.Demo")
            .map(|element| element.properties.clone())
            .unwrap_or_default();
        let members = package
            .get("members")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            members
                .iter()
                .any(|member| member.as_str() == Some(connection_canonical.as_str()))
        );
        // No position-derived ids survive anywhere in the document.
        for element in &canonical.document.elements {
            assert!(positional_id_base(&element.id).is_none(), "{}", element.id);
        }
        // The synthesized qualified_name is rewritten with the same suffix.
        let connection_qualified_name = canonical
            .document
            .elements
            .iter()
            .find(|element| element.id == connection_canonical)
            .and_then(|element| element.properties.get("qualified_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        assert!(
            connection_qualified_name.starts_with("Demo.Connection.c"),
            "{connection_qualified_name}"
        );
        assert!(!connection_qualified_name.ends_with(".11"));
    }

    #[test]
    fn distinct_connections_keep_distinct_canonical_ids_across_reordering() {
        // Two connections with different targets; authored in opposite source
        // order in the two documents.
        let build = |first_line: u64, second_line: u64| {
            let mut doc = connection_fixture("a.sysml", first_line, false);
            let extra_connection = format!("connection.Demo.Connection.{second_line}");
            let extra_source = format!("reference.Demo.Connection.source.{second_line}");
            doc.elements.push(element(
                &extra_connection,
                "SysML::ConnectionUsage",
                json!({
                    "definition": "Connections::Connection",
                    "members": [extra_source.clone()],
                    "owner": "pkg.Demo",
                    "metadata": source_metadata("a.sysml", second_line)
                }),
            ));
            doc.elements.push(element(
                &extra_source,
                "SysML::ReferenceUsage",
                json!({
                    "declared_name": "source",
                    "definition": "type.Demo.Vehicle",
                    "featuring_type": extra_connection.clone(),
                    "owner": extra_connection.clone(),
                    "metadata": source_metadata("a.sysml", second_line)
                }),
            ));
            doc
        };
        let left = build(11, 20);
        let right = build(30, 5);

        assert!(kir_documents_equivalent(&left, &right));

        let canonical = canonicalize_kir_document(&left);
        let connection_ids = canonical
            .document
            .elements
            .iter()
            .filter(|element| element.kind == "SysML::ConnectionUsage")
            .map(|element| element.id.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(connection_ids.len(), 2, "{connection_ids:?}");
    }

    #[test]
    fn identical_siblings_get_deterministic_suffixes() {
        let build = |lines: [u64; 2]| {
            document(
                [
                    vec![element(
                        "pkg.Demo",
                        "SysML::Package",
                        json!({
                            "declared_name": "Demo",
                            "members": lines
                                .iter()
                                .map(|line| format!("import.pkg.Demo.{line}"))
                                .collect::<Vec<_>>(),
                            "metadata": source_metadata("a.sysml", 1)
                        }),
                    )],
                    lines
                        .iter()
                        .map(|line| {
                            element(
                                &format!("import.pkg.Demo.{line}"),
                                "SysML::Import",
                                json!({
                                    "owner": "pkg.Demo",
                                    "metadata": source_metadata("a.sysml", *line)
                                }),
                            )
                        })
                        .collect(),
                ]
                .concat(),
            )
        };
        let left = build([0, 1]);
        let right = build([1, 2]);

        let canonical = canonicalize_kir_document(&left);
        let ids = canonical.id_map.values().collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 2, "suffixing must keep ids unique: {ids:?}");
        assert!(ids.iter().all(|id| id.contains('#')), "{ids:?}");

        assert!(kir_documents_equivalent(&left, &right));
    }

    #[test]
    fn renamed_part_is_not_equivalent() {
        let left = connection_fixture("a.sysml", 11, false);
        let mut right = connection_fixture("a.sysml", 11, false);
        for element in &mut right.elements {
            if element.id == "type.Demo.Vehicle" {
                element.id = "type.Demo.Car".to_string();
                element
                    .properties
                    .insert("declared_name".to_string(), json!("Car"));
            }
        }

        let diff = kir_equivalence_diff(&left, &right);
        assert!(!semantic_diff_is_empty(&diff));
        assert!(!kir_documents_equivalent(&left, &right));
    }

    #[test]
    fn changed_connection_target_is_not_equivalent() {
        let left = connection_fixture("a.sysml", 11, false);
        let mut right = connection_fixture("a.sysml", 11, false);
        for element in &mut right.elements {
            if element.id == "reference.Demo.Connection.target.11" {
                element
                    .properties
                    .insert("definition".to_string(), json!("type.Demo.Engine"));
            }
        }

        assert!(!kir_documents_equivalent(&left, &right));
    }

    #[test]
    fn equivalence_report_serializes() {
        let left = connection_fixture("a.sysml", 11, false);
        let right = connection_fixture("b.sysml", 40, false);

        let report = kir_equivalence_report(&left, &right);
        assert!(report.equivalent);
        assert_eq!(
            report.schema_version,
            KIR_EQUIVALENCE_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.left_id_map.len(), 3);
        assert_eq!(report.right_id_map.len(), 3);

        let serialized = serde_json::to_string(&report).expect("report serializes");
        let deserialized: KirEquivalenceReport =
            serde_json::from_str(&serialized).expect("report deserializes");
        assert_eq!(deserialized, report);
    }
}
