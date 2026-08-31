//! The bidirectional map between a saved `view` usage and `mercurio.view.v1`.
//!
//! Save-as-View V-6.3. [`view_spec_from_usage`] reads a view out of the model;
//! [`usage_from_view_spec`] turns a spec into the draft of a view usage that
//! V-6.5 will apply as a checked mutation. Together they are what makes a saved
//! view *the model's* rather than a sidecar's.
//!
//! # What "round-trip" means here
//!
//! Not one thing, deliberately:
//!
//! - **Tier 1 and tier 2** specs round-trip exactly — `spec → usage → spec` is
//!   the identity.
//! - **A materialized save is a different spec by design.** It converts
//!   `DiagramScopeDto::Traversal` into `ExplicitElements`, freezing what the
//!   author actually curated. That is a normalization, not a loss, and the rule
//!   is `materialize(spec) → usage → spec' == materialize(spec)`.
//! - **Tier 3** returns [`NotReifiable`] naming the field that has no home. A
//!   free-text search is the worked example: it has no scope, no notation and
//!   no stable element set, so it is not a view and inventing an encoding for
//!   it would be worse than refusing.
//!
//! # The asymmetry worth knowing
//!
//! The model direction is lossy on purpose. An `expose` predicate
//! (`expose vehicle::**[@Safety]`) narrows what a view shows, but
//! `DiagramSpecDto` has no field for it, so reading such a view back gives the
//! scope without the predicate. That is not a round-trip failure — the spec
//! never carried a predicate to lose — but it does mean two views differing
//! only by predicate read back as specs differing only by title. Resolving what
//! they *show* is [`super::expose::exposed_elements`]'s job, not this module's.

use std::collections::BTreeMap;

use crate::model::{Graph, NodeId};

use super::expose::{
    inherited_filter_conditions, qualified_name, scope_base, scope_is_wildcard,
};
use super::{
    DiagramKindDto, DiagramScopeDto, DiagramSpecDto, ModelViewSpecDto, TableColumnSpecDto,
    TableKindDto, TableRowTypeDto, TableScopeDto, TableSpecDto, VIEW_SPEC_VERSION,
    ViewDocumentDto,
};

/// KIR kind of an `expose` member.
const EXPOSE_KIND: &str = "SysML::Expose";
const OWNER: &str = "owner";

/// The standard renderings, and the `mercurio.view.v1` kind each one means.
///
/// Only these four exist in the standard library
/// (`Systems Library/Views.sysml`). Everything else is a user `rendering def`
/// subtyping `GraphicalRendering`/`TabularRendering`, which is tier 2 and needs
/// the `#Mercurio` profile from V-6.4 — so it is refused here rather than
/// guessed at.
const STANDARD_RENDERINGS: &[(&str, StandardRendering)] = &[
    ("asTreeDiagram", StandardRendering::Diagram(DiagramKindDto::PackageTree)),
    (
        "asInterconnectionDiagram",
        StandardRendering::Diagram(DiagramKindDto::InternalBlock),
    ),
    ("asElementTable", StandardRendering::Table(TableKindDto::Elements)),
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum StandardRendering {
    Diagram(DiagramKindDto),
    Table(TableKindDto),
}

/// A view, or part of one, that has no home in the model.
///
/// Carries the offending field rather than a bare failure, because the caller's
/// only useful response is to tell the author which part of their view cannot
/// be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotReifiable {
    /// Dotted path of the field with no encoding, e.g. `model.query`.
    pub field: String,
    /// Why it has none, in terms an author can act on.
    pub reason: String,
}

impl NotReifiable {
    fn new(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for NotReifiable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "`{}` cannot be reified: {}", self.field, self.reason)
    }
}

/// One `expose` member of a view being drafted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposeDraft {
    /// The scope path, e.g. `vehicle::**` or `CuratedViews::Alpha`.
    pub path: String,
    /// A bracketed predicate, without the brackets, e.g. `@Safety`.
    pub predicate: Option<String>,
}

/// A `rendering` a saved view needs alongside it.
///
/// Columns are not properties of a view — they belong to its *rendering*, as
/// `columnView` subsets. So a table view with columns cannot be written as one
/// element: it needs a rendering subtyping `asElementTable` that declares them,
/// and the view renders with that. Without this the columns are silently
/// dropped on save, which is the sort of loss the whole plan exists to stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderingDraft {
    pub declared_name: String,
    /// The standard rendering it subtypes, e.g. `asElementTable`.
    pub subtypes: String,
    /// Column feature names, in order. `columnView` is `ordered`, so this
    /// sequence is model content rather than presentation.
    pub columns: Vec<String>,
}

/// Everything needed to write a `view` usage into a model.
///
/// Deliberately *not* SysML text: V-6.5 applies this through the same
/// check-then-apply mutation pipeline as any other edit, and text is only one
/// of its outputs. [`ViewUsageDraft::to_sysml`] renders it when text is what
/// you want — a fixture round-trip, or a preview.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewUsageDraft {
    pub declared_name: String,
    pub documentation: Option<String>,
    pub exposes: Vec<ExposeDraft>,
    /// Conditions for `filter` members, verbatim.
    pub filters: Vec<String>,
    /// The rendering to `render`, e.g. `asTreeDiagram`.
    pub rendering: Option<String>,
    /// A rendering to declare beside the view, when the standard ones cannot
    /// carry what the spec holds — today, columns.
    pub rendering_def: Option<RenderingDraft>,
}

impl ViewUsageDraft {
    /// The draft as SysML v2 text.
    ///
    /// Names are quoted whenever they are not plain identifiers, which is the
    /// common case for a saved view: `'vehicle structure view'` has spaces.
    pub fn to_sysml(&self) -> String {
        let mut out = String::new();
        if let Some(rendering) = &self.rendering_def {
            out.push_str(&format!(
                "rendering {} :> {} {{\n",
                quote_name(&rendering.declared_name),
                rendering.subtypes
            ));
            for column in &rendering.columns {
                out.push_str(&format!(
                    "    view {} :> columnView {{\n        render asTextualNotation;\n    }}\n",
                    quote_name(column)
                ));
            }
            out.push_str("}\n\n");
        }
        out.push_str(&format!("view {} {{\n", quote_name(&self.declared_name)));
        if let Some(documentation) = &self.documentation {
            out.push_str(&format!("    doc /* {documentation} */\n"));
        }
        for expose in &self.exposes {
            match &expose.predicate {
                Some(predicate) => {
                    out.push_str(&format!("    expose {}[{}];\n", expose.path, predicate))
                }
                None => out.push_str(&format!("    expose {};\n", expose.path)),
            }
        }
        for filter in &self.filters {
            out.push_str(&format!("    filter {filter};\n"));
        }
        if let Some(rendering) = &self.rendering {
            out.push_str(&format!("    render {rendering};\n"));
        }
        out.push_str("}\n");
        out
    }
}

/// A plain-identifier name for a generated rendering, from the view's title.
///
/// `component table` becomes `asComponentTableColumns`, following the library's
/// own `asElementTable` / `asTreeDiagram` convention. It has to be a plain
/// identifier rather than a quoted name: `render 'a b c';` does not parse the
/// quoted form, and a generated name is never read back — reading follows the
/// `render` member to whatever rendering it names — so only stability and
/// distinctness matter.
fn rendering_name_for(title: &str) -> String {
    let mut out = String::from("as");
    for word in title.split(|c: char| !c.is_alphanumeric()) {
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(characters.as_str());
        }
    }
    out.push_str("Columns");
    out
}

fn quote_name(name: &str) -> String {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if plain {
        name.to_string()
    } else {
        format!("'{name}'")
    }
}

// --------------------------------------------------------------- model → spec

/// Read a saved `view` usage back into a `mercurio.view.v1` document.
pub fn view_spec_from_usage(graph: &Graph, view: NodeId) -> Result<ViewDocumentDto, NotReifiable> {
    let element = graph
        .element(view)
        .ok_or_else(|| NotReifiable::new("view", "no such element in this graph"))?;
    let properties = element.properties.to_btree_map();

    let title = properties
        .get("declared_name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let description = owned_documentation(graph, view);

    let rendering = rendering_name(graph, view).ok_or_else(|| {
        NotReifiable::new(
            "kind",
            "the view declares no `render`, so its notation is unknown",
        )
    })?;
    // A user rendering subtypes a standard one -- `rendering asNameAndDoc :>
    // asElementTable` -- and that inheritance is what says how to render it. So
    // resolve through it rather than refusing every non-standard name: the
    // column *structure* is standard content even when the rendering is the
    // author's own.
    let (rendering, columns) = resolve_rendering(graph, &rendering);
    let standard = STANDARD_RENDERINGS
        .iter()
        .find(|(name, _)| *name == rendering)
        .map(|(_, kind)| kind.clone())
        .ok_or_else(|| {
            NotReifiable::new(
                "kind",
                format!(
                    "`{rendering}` does not resolve to a standard rendering; one that \
                     subtypes none of them is tier 2 and needs the #Mercurio profile (V-6.4)"
                ),
            )
        })?;

    let scopes = expose_scopes(graph, view);
    // The *effective* filters, inherited ones included: a usage typed by
    // `view def X { filter @SysML::PartUsage; }` really is a table of part
    // usages, even though the condition is written on the definition and not on
    // the usage.
    let filters = inherited_filter_conditions(graph, view);

    match standard {
        StandardRendering::Diagram(kind) => {
            let mut spec = DiagramSpecDto {
                version: VIEW_SPEC_VERSION,
                kind,
                title,
                description,
                root: None,
                query: Default::default(),
                layout: Default::default(),
                style: Default::default(),
            };
            match subtree_root(graph, view, &scopes) {
                Some(root) => spec.root = Some(root),
                // No wildcard anywhere means the author curated a set rather
                // than a subtree, which is exactly what ExplicitElements is for.
                None if !scopes.is_empty() => {
                    spec.query.scope = DiagramScopeDto::ExplicitElements {
                        elements: scope_names(graph, view, &scopes),
                    }
                }
                None => {}
            }
            Ok(ViewDocumentDto::diagram(spec))
        }
        StandardRendering::Table(kind) => {
            let scope = match subtree_root(graph, view, &scopes) {
                Some(root) => TableScopeDto::ContainmentSubtree { root },
                None if !scopes.is_empty() => TableScopeDto::ExplicitElements {
                    elements: scope_names(graph, view, &scopes),
                },
                None => TableScopeDto::WholeModel,
            };
            Ok(ViewDocumentDto::table(TableSpecDto {
                version: VIEW_SPEC_VERSION,
                kind,
                title,
                description,
                root: None,
                target_type: None,
                scope,
                row_type: filters.iter().find_map(|condition| metaclass_row_type(condition)),
                query: Default::default(),
                columns,
                show_affordances: false,
            }))
        }
    }
}

/// `filter @SysML::PartUsage` becomes a row type; `filter @Safety` does not.
///
/// The distinction is real and load-bearing: SysML evaluates a filter against
/// metadata features *plus the implicit metaclass feature*, so `@SysML::X` names
/// a metaclass while a bare `@X` names a metadata definition. Only the first is
/// a row type; treating the second as one would claim a table of
/// "Safety-typed elements", which is not a type at all.
fn metaclass_row_type(condition: &str) -> Option<TableRowTypeDto> {
    let path = condition.trim().strip_prefix('@')?.trim();
    let segments: Vec<&str> = path.split("::").collect();
    let namespace = *segments.first()?;
    let name = *segments.last()?;
    if !matches!(namespace, "SysML" | "KerML") || segments.len() < 2 {
        return None;
    }
    Some(TableRowTypeDto {
        type_name: name.to_string(),
        // A metaclass filter matches the metaclass exactly. `include_subtypes`
        // would mean walking the specialization hierarchy, which is precisely
        // what a SysML filter cannot see -- see the map's row-type note.
        include_subtypes: false,
    })
}

/// The subtree anchor, when the view exposes one.
fn subtree_root(graph: &Graph, view: NodeId, scopes: &[ExposeDraft]) -> Option<String> {
    scopes
        .iter()
        .find(|scope| scope_is_wildcard(&scope.path))
        .and_then(|scope| scope_base(graph, view, &scope.path))
        .and_then(|base| qualified_name(graph, base))
}

fn scope_names(graph: &Graph, view: NodeId, scopes: &[ExposeDraft]) -> Vec<String> {
    scopes
        .iter()
        .filter_map(|scope| {
            scope_base(graph, view, &scope.path).and_then(|base| qualified_name(graph, base))
        })
        .collect()
}

/// The view's `expose` members, in the order they were authored.
///
/// Source order, not element-id order: ids embed a module-wide ordinal, so
/// `expose.…​.10` sorts before `expose.…​.2` and a curated set would silently
/// reorder itself once a file grew past nine namespace queries.
fn expose_scopes(graph: &Graph, view: NodeId) -> Vec<ExposeDraft> {
    let mut scopes: Vec<(u64, u64, ExposeDraft)> = Vec::new();
    for member in owned_members(graph, view, EXPOSE_KIND) {
        let Some(element) = graph.element(member) else {
            continue;
        };
        let properties = element.properties.to_btree_map();
        let predicate = properties
            .get("filter")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let (line, column) = source_position(&properties);
        let paths = match properties.get("exposes") {
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>(),
            Some(serde_json::Value::String(single)) => vec![single.clone()],
            _ => Vec::new(),
        };
        for path in paths {
            scopes.push((
                line,
                column,
                ExposeDraft {
                    path,
                    predicate: predicate.clone(),
                },
            ));
        }
    }
    scopes.sort_by_key(|(line, column, _)| (*line, *column));
    scopes.into_iter().map(|(_, _, scope)| scope).collect()
}

fn source_position(properties: &BTreeMap<String, serde_json::Value>) -> (u64, u64) {
    let span = properties
        .get("metadata")
        .and_then(|metadata| metadata.get("source_span"));
    let read = |key: &str| {
        span.and_then(|span| span.get(key))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    (read("start_line"), read("start_col"))
}

/// Name of the rendering a view renders with.
///
/// `render` still lowers onto the fabricated `SysML::RenderUsage` metaclass --
/// the remaining half of V-6.1's rule-1 debt -- so this matches on the member's
/// metatype rather than its KIR kind, which is `KerML::Core::Feature`. When
/// `render` moves to `ViewRenderingMembership` this keeps working, because the
/// metatype is what changes.
fn rendering_name(graph: &Graph, view: NodeId) -> Option<String> {
    graph
        .incoming(view, OWNER)
        .map(|edge| edge.source)
        .filter_map(|node| graph.element(node))
        .find(|element| {
            element
                .properties
                .to_btree_map()
                .get("metatype")
                .and_then(|value| value.as_str())
                .is_some_and(|metatype| {
                    matches!(last_segment(metatype), "RenderUsage" | "ViewRenderingMembership")
                })
        })
        .and_then(|element| {
            element
                .properties
                .to_btree_map()
                .get("declared_name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

/// Follow a rendering name to the standard rendering it ultimately subtypes,
/// collecting the columns it declares on the way.
///
/// `asTreeDiagram` resolves to itself. `asNameAndDoc :> asElementTable`
/// resolves to `asElementTable`, and its `view … :> columnView` members are the
/// table's columns.
fn resolve_rendering(graph: &Graph, rendering: &str) -> (String, Vec<TableColumnSpecDto>) {
    if STANDARD_RENDERINGS.iter().any(|(name, _)| name == &rendering) {
        return (rendering.to_string(), Vec::new());
    }

    let Some(node) = graph.elements().iter().find(|element| {
        let properties = element.properties.to_btree_map();
        properties
            .get("metatype")
            .and_then(|value| value.as_str())
            .is_some_and(|metatype| last_segment(metatype) == "RenderingUsage")
            && properties.get("declared_name").and_then(|value| value.as_str()) == Some(rendering)
    }) else {
        return (rendering.to_string(), Vec::new());
    };

    let columns = rendering_columns(graph, node.id);
    let supertype = node
        .properties
        .to_btree_map()
        .get("specializes")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str())
        .map(|value| last_segment(value).to_string())
        .find(|name| STANDARD_RENDERINGS.iter().any(|(standard, _)| standard == name));

    (supertype.unwrap_or_else(|| rendering.to_string()), columns)
}

/// The `columnView` subsets a rendering declares, in the order they were
/// written -- `asElementTable::columnView` is `ordered`, so column order is
/// content rather than presentation.
///
/// Multiple columns are **subsets** of `columnView[0..*]`, not redefinitions of
/// it: `:>` adds instances to a many-valued feature while `:>>` replaces it. The
/// pilot only ever writes one column (`view :>> columnView[1]`, where `[1]` is
/// the redefining feature's multiplicity and not an index), so two `:>>` of the
/// same feature are two definitions of one feature -- which the compiler rightly
/// rejects as a duplicate id rather than reading as two columns.
///
/// `key` and `label` come from the column feature's own declared name, which is
/// real model content and therefore tier 1. `path` -- *which* attribute of the
/// row element a column shows -- has no standard encoding at all: the pilot's
/// column view carries only a `render`. It is tier 2, arrives with the
/// #Mercurio profile in V-6.4, and stays `None` here rather than being invented.
fn rendering_columns(graph: &Graph, rendering: NodeId) -> Vec<TableColumnSpecDto> {
    let mut columns: Vec<(u64, u64, TableColumnSpecDto)> = graph
        .incoming(rendering, OWNER)
        .map(|edge| edge.source)
        .filter_map(|node| graph.element(node))
        .filter(|element| {
            element
                .properties
                .to_btree_map()
                .get("specializes")
                .and_then(|value| value.as_array())
                .is_some_and(|specializes| {
                    specializes
                        .iter()
                        .filter_map(|value| value.as_str())
                        .any(|value| last_segment(value) == "columnView")
                })
        })
        .filter_map(|element| {
            let properties = element.properties.to_btree_map();
            let name = properties.get("declared_name")?.as_str()?.to_string();
            let (line, column) = source_position(&properties);
            Some((
                line,
                column,
                TableColumnSpecDto {
                    key: name.clone(),
                    label: name,
                    path: None,
                    expression: None,
                },
            ))
        })
        .collect();
    columns.sort_by_key(|(line, column, _)| (*line, *column));
    columns.into_iter().map(|(_, _, column)| column).collect()
}

fn owned_documentation(graph: &Graph, view: NodeId) -> Option<String> {
    graph
        .incoming(view, OWNER)
        .map(|edge| edge.source)
        .filter_map(|node| graph.element(node))
        .find(|element| element.kind.contains("Documentation"))
        .and_then(|element| {
            element
                .properties
                .to_btree_map()
                .get("body")
                .and_then(|value| value.as_str())
                .map(|body| body.trim().to_string())
        })
}

fn owned_members(graph: &Graph, owner: NodeId, kind: &str) -> Vec<NodeId> {
    graph
        .incoming(owner, OWNER)
        .map(|edge| edge.source)
        .filter(|node| {
            graph
                .element(*node)
                .is_some_and(|element| element.kind.as_ref() == kind)
        })
        .collect()
}

fn last_segment(qualified: &str) -> &str {
    qualified.rsplit("::").next().unwrap_or(qualified)
}

// --------------------------------------------------------------- spec → model

/// Turn a `mercurio.view.v1` document into the draft of a `view` usage.
pub fn usage_from_view_spec(spec: &ViewDocumentDto) -> Result<ViewUsageDraft, NotReifiable> {
    if let Some(model) = &spec.model {
        return model_draft(model);
    }
    if let Some(diagram) = &spec.diagram {
        return diagram_draft(diagram);
    }
    if let Some(table) = &spec.table {
        return table_draft(table);
    }
    Err(NotReifiable::new(
        "spec",
        "the document carries no diagram, table, or model view",
    ))
}

fn diagram_draft(spec: &DiagramSpecDto) -> Result<ViewUsageDraft, NotReifiable> {
    let rendering = STANDARD_RENDERINGS
        .iter()
        .find(|(_, standard)| *standard == StandardRendering::Diagram(spec.kind.clone()))
        .map(|(name, _)| (*name).to_string())
        .ok_or_else(|| {
            NotReifiable::new(
                "diagram.kind",
                "only asTreeDiagram and asInterconnectionDiagram are standard; \
                 other diagram kinds are tier 2 and need the #Mercurio profile (V-6.4)",
            )
        })?;

    // An explicit set wins over the traversal root: it is what the author
    // curated, and re-deriving it from `root` would hand back the superset the
    // materialization existed to avoid.
    let exposes = match spec.query.scope.explicit_elements() {
        Some(elements) => elements
            .iter()
            .map(|element| ExposeDraft {
                path: element.clone(),
                predicate: None,
            })
            .collect(),
        None => subtree_expose(spec.root.as_deref()),
    };

    Ok(ViewUsageDraft {
        declared_name: spec.title.clone(),
        documentation: spec.description.clone(),
        exposes,
        filters: Vec::new(),
        rendering: Some(rendering),
        rendering_def: None,
    })
}

fn table_draft(spec: &TableSpecDto) -> Result<ViewUsageDraft, NotReifiable> {
    if !matches!(spec.kind, TableKindDto::Elements) {
        return Err(NotReifiable::new(
            "table.kind",
            "only `elements` maps onto the standard asElementTable; \
             model_elements, requirements, and relationship_matrix are tier 2 (V-6.4)",
        ));
    }
    if spec.columns.iter().any(|column| column.expression.is_some()) {
        return Err(NotReifiable::new(
            "table.columns.expression",
            "a computed column has no standard encoding; `path` columns map onto \
             asElementTable::columnView, expressions are tier 2 (V-6.4)",
        ));
    }

    let exposes = match &spec.scope {
        TableScopeDto::WholeModel => Vec::new(),
        TableScopeDto::ContainmentSubtree { root } => subtree_expose(Some(root)),
        TableScopeDto::ExplicitElements { elements } => elements
            .iter()
            .map(|element| ExposeDraft {
                path: element.clone(),
                predicate: None,
            })
            .collect(),
    };

    // Columns live on the *rendering*, not the view, so a table with columns
    // needs one declared beside it. The generated name is derived from the
    // view's title and never read back -- reading follows the `render` member to
    // whatever rendering it names -- so it only has to be stable and distinct.
    let rendering_def = (!spec.columns.is_empty()).then(|| RenderingDraft {
        declared_name: rendering_name_for(&spec.title),
        subtypes: "asElementTable".to_string(),
        columns: spec.columns.iter().map(|column| column.key.clone()).collect(),
    });
    let rendering = rendering_def
        .as_ref()
        .map(|rendering| rendering.declared_name.clone())
        .unwrap_or_else(|| "asElementTable".to_string());

    Ok(ViewUsageDraft {
        declared_name: spec.title.clone(),
        documentation: spec.description.clone(),
        exposes,
        filters: spec
            .row_type
            .iter()
            .map(|row_type| format!("@SysML::{}", row_type.type_name))
            .collect(),
        rendering: Some(rendering),
        rendering_def,
    })
}

fn model_draft(spec: &ModelViewSpecDto) -> Result<ViewUsageDraft, NotReifiable> {
    // Tier 3, and the correct answer. A search has no scope, no notation, and
    // no stable element set; it is not a view, and encoding it as one would
    // produce a model element that cannot be re-rendered.
    if spec.query.is_some() {
        return Err(NotReifiable::new(
            "model.query",
            "a free-text search is not a view: it has no scope, no notation, and no \
             stable element set",
        ));
    }

    // An exploration is its result set, which is exactly a curated expose list.
    let mut elements = spec.expanded_parents.clone();
    elements.extend(spec.expanded_children.iter().cloned());
    if elements.is_empty() {
        elements.extend(spec.root.iter().cloned());
    }

    Ok(ViewUsageDraft {
        declared_name: spec.title.clone(),
        documentation: spec.description.clone(),
        exposes: elements
            .into_iter()
            .map(|element| ExposeDraft {
                path: element,
                predicate: None,
            })
            .collect(),
        filters: Vec::new(),
        rendering: Some("asTreeDiagram".to_string()),
        rendering_def: None,
    })
}

fn subtree_expose(root: Option<&str>) -> Vec<ExposeDraft> {
    root.into_iter()
        .map(|root| ExposeDraft {
            path: format!("{root}::**"),
            predicate: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_text_search_is_refused_by_name() {
        let spec = ViewDocumentDto::model(ModelViewSpecDto {
            version: VIEW_SPEC_VERSION,
            kind: super::super::ModelViewKindDto::Search,
            title: "find brake".to_string(),
            description: None,
            root: None,
            graph_scope: None,
            query: Some("brake".to_string()),
            expanded_parents: Vec::new(),
            expanded_children: Vec::new(),
            include_reference_edges: true,
        });

        let error = usage_from_view_spec(&spec).expect_err("a search is not a view");
        assert_eq!(error.field, "model.query");
    }

    #[test]
    fn a_metaclass_filter_is_a_row_type_but_a_metadata_filter_is_not() {
        assert_eq!(
            metaclass_row_type("@SysML::PartUsage"),
            Some(TableRowTypeDto {
                type_name: "PartUsage".to_string(),
                include_subtypes: false,
            })
        );
        assert_eq!(metaclass_row_type("@Safety"), None);
        assert_eq!(metaclass_row_type("@AnnotationDefinitions::Safety"), None);
    }

    /// The materialization rule: an explicit set is *not* a lossy version of a
    /// traversal, it is a different and more faithful spec, so it must win over
    /// `root`. Re-deriving the scope from `root` would hand back the superset
    /// the materialization existed to avoid -- which is the exact defect that
    /// put this plan in motion.
    #[test]
    fn an_explicit_scope_wins_over_the_traversal_root() {
        let spec = DiagramSpecDto {
            version: VIEW_SPEC_VERSION,
            kind: DiagramKindDto::PackageTree,
            title: "curated".to_string(),
            description: None,
            root: Some("P::vehicle".to_string()),
            query: super::super::DiagramQueryOptionsDto {
                scope: DiagramScopeDto::ExplicitElements {
                    elements: vec!["P::alpha".to_string(), "P::gamma".to_string()],
                },
                ..Default::default()
            },
            layout: Default::default(),
            style: Default::default(),
        };

        let draft = usage_from_view_spec(&ViewDocumentDto::diagram(spec))
            .expect("a curated diagram drafts a usage");

        assert_eq!(
            draft.exposes,
            vec![
                ExposeDraft {
                    path: "P::alpha".to_string(),
                    predicate: None
                },
                ExposeDraft {
                    path: "P::gamma".to_string(),
                    predicate: None
                },
            ],
            "the curated set must be written, not `expose P::vehicle::**`"
        );
    }

    #[test]
    fn a_saved_view_name_is_quoted_only_when_it_has_to_be() {
        assert_eq!(quote_name("curated"), "curated");
        assert_eq!(quote_name("vehicle structure view"), "'vehicle structure view'");
        assert_eq!(quote_name("2fast"), "'2fast'");
    }
}
