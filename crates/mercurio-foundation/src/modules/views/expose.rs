//! `expose` resolution — what a saved view actually shows.
//!
//! Save-as-View SV-2. A `view` usage declares its scope with `expose` members
//! and narrows it with `filter` members. Both reach KIR as elements owned by
//! the view (`SysML::Expose` and `SysML::ElementFilterMembership`), and both
//! carry their predicate as **verbatim text**, because the AST's `Expr` cannot
//! represent the metadata-application (`@X`) or cast (`as X`) operators a
//! condition is built from. Turning that text into a set of elements is this
//! module's job.
//!
//! The result is a *set*, not a namespace binding. `expose` shares its syntax
//! with `import`, but an import contributes names to a scope while an expose
//! answers a different question: which elements does this view render?
//!
//! Three properties this module owes its callers:
//!
//! - **Deterministic.** Resolving the same view twice against the same
//!   revision returns the same ids in the same order. Every intermediate set
//!   is a `BTreeSet` keyed by element id, and name lookup breaks ties by id
//!   rather than by graph order.
//! - **`wasm32`-clean.** No filesystem, no clock, no threads.
//! - **Honest about failure.** A scope that names nothing is reported in
//!   `unresolved` rather than silently contributing an empty set. A view whose
//!   expose is a typo should look broken, not empty.
//!
//! # Filter semantics
//!
//! Per the SysML v2 spec, an element filter tests **metadata features and the
//! implicit metaclass feature only** — it does not consult the specialization
//! hierarchy. `@Safety` asks "does this element carry a Safety metadata
//! annotation", not "is this element a kind of Safety". This module implements
//! exactly that, which is why it reaches for [`metadata_annotations_named`] and
//! never for `collect_specialization_ancestors`.
//!
//! Filters compose by conjunction and are **inherited**: a view collects its
//! own filter members plus those of every view it is typed by (`:`) or
//! specializes (`:>`), transitively. That inheritance is what makes the
//! pilot's `11b-Safety and Security Feature Views.sysml` views diverge —
//! `vehicleMandatorySafetyFeatureView` gets `@Safety` from the view definition
//! it descends from and adds `(as Safety).isMandatory` of its own.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::model::{Graph, NodeId, metadata_annotations_named, metadata_string_property};

/// KIR kind of an `expose` member.
const EXPOSE_KIND: &str = "SysML::Expose";
/// KIR kind of a `filter` member.
const FILTER_KIND: &str = "SysML::ElementFilterMembership";
/// Property naming the owner of a member; also the graph relation built from it.
const OWNER: &str = "owner";

/// What a view exposes, and what it failed to resolve.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExposeResolution {
    /// Element ids the view renders, ordered deterministically.
    pub elements: Vec<String>,
    /// Scope paths that resolved to no element at all, in declaration order.
    /// Non-empty means the view is broken, not empty — surface it.
    pub unresolved: Vec<String>,
    /// Filter conditions that could not be parsed, in declaration order. A
    /// condition that does not parse is *not* treated as vacuously true: it
    /// excludes everything, so a malformed filter shows an empty view rather
    /// than a silently unfiltered one.
    pub invalid_conditions: Vec<String>,
}

/// The elements a `view` usage exposes.
///
/// `view` is the node id of a `SysML::ViewUsage` (or view definition — the
/// mechanics are identical, and a definition's filters are what usages
/// inherit).
pub fn exposed_elements(graph: &Graph, view: NodeId) -> Vec<String> {
    resolve_exposed_elements(graph, view).elements
}

/// [`exposed_elements`] with the diagnostics kept.
pub fn resolve_exposed_elements(graph: &Graph, view: NodeId) -> ExposeResolution {
    let mut resolution = ExposeResolution::default();

    let filters = inherited_filter_conditions(graph, view);
    let mut view_conditions = Vec::new();
    for text in &filters {
        match parse_condition(text) {
            Some(condition) => view_conditions.push(condition),
            None => resolution.invalid_conditions.push(text.clone()),
        }
    }

    let mut selected: BTreeSet<String> = BTreeSet::new();

    for expose in owned_members(graph, view, EXPOSE_KIND) {
        let element = match graph.element(expose) {
            Some(element) => element,
            None => continue,
        };

        // The expose's own bracketed predicate, e.g. `[@Safety]`. It narrows
        // only this scope; the view's filters narrow every scope.
        let mut scope_conditions = view_conditions.clone();
        if let Some(text) = string_property(element.properties.to_btree_map().get("filter")) {
            match parse_condition(&text) {
                Some(condition) => scope_conditions.push(condition),
                None => resolution.invalid_conditions.push(text),
            }
        }

        for path in scope_paths(&element.properties.to_btree_map()) {
            let candidates = resolve_scope(graph, view, &path);
            if candidates.is_empty() {
                resolution.unresolved.push(path);
                continue;
            }
            for candidate in candidates {
                if is_exposable(graph, candidate)
                    && scope_conditions
                        .iter()
                        .all(|condition| condition.matches(graph, candidate))
                    && let Some(id) = graph.element_id(candidate)
                {
                    selected.insert(id.to_string());
                }
            }
        }
    }

    resolution.elements = selected.into_iter().collect();
    resolution
}

// ------------------------------------------------------------------ members

/// Members of `owner` with the given KIR kind, in element-id order.
///
/// Ownership is the `owner` property, which the graph turns into an edge of
/// the same name pointing child -> owner, so members are *incoming* edges.
fn owned_members(graph: &Graph, owner: NodeId, kind: &str) -> Vec<NodeId> {
    let mut members: Vec<NodeId> = graph
        .incoming(owner, OWNER)
        .map(|edge| edge.source)
        .filter(|node| {
            graph
                .element(*node)
                .is_some_and(|element| element.kind.as_ref() == kind)
        })
        .collect();
    members.sort_by_key(|node| graph.element_id(*node).unwrap_or_default().to_string());
    members.dedup();
    members
}

/// Every filter condition that applies to `view`: its own, plus those of every
/// view it is typed by or specializes, transitively.
///
/// This is the *effective* set, which is what reading a view back needs — a
/// usage typed by `view def X { filter @SysML::PartUsage; }` really is a table
/// of part usages, even though the condition is not written on the usage.
/// Writing a view back out derives its filters from the spec's own `row_type`
/// instead, so a save never copies a definition's filters onto the usage.
///
/// Returned in a stable order (breadth-first from the view, each level sorted
/// by element id) and de-duplicated, so an inherited condition repeated on a
/// subtype is not applied twice.
pub fn inherited_filter_conditions(graph: &Graph, view: NodeId) -> Vec<String> {
    let mut conditions = Vec::new();
    let mut seen_conditions = BTreeSet::new();
    let mut seen_views = BTreeSet::new();
    let mut queue = VecDeque::from([view]);
    seen_views.insert(view);

    while let Some(current) = queue.pop_front() {
        for filter in owned_members(graph, current, FILTER_KIND) {
            let Some(element) = graph.element(filter) else {
                continue;
            };
            if let Some(text) = string_property(element.properties.to_btree_map().get("condition"))
                && seen_conditions.insert(text.clone())
            {
                conditions.push(text);
            }
        }

        // A view inherits filters through both typing (`view v : Def`) and
        // specialization (`view v :> other`). The spec treats a view usage's
        // definition as its supertype, so the two are one walk here.
        let mut supertypes: Vec<NodeId> = ["type", "specializes", "subsets", "redefines"]
            .iter()
            .flat_map(|relation| graph.outgoing(current, relation))
            .map(|edge| edge.target)
            .collect();
        supertypes.sort_by_key(|node| graph.element_id(*node).unwrap_or_default().to_string());
        for supertype in supertypes {
            if seen_views.insert(supertype) {
                queue.push_back(supertype);
            }
        }
    }

    conditions
}

/// The scope paths an expose declares. `exposes` is an array because one
/// membership expose may name several targets.
fn scope_paths(properties: &BTreeMap<String, Value>) -> Vec<String> {
    match properties.get("exposes") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::to_string)
            .collect(),
        Some(Value::String(single)) => vec![single.clone()],
        _ => Vec::new(),
    }
}

fn string_property(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

// ------------------------------------------------------------------- scopes

/// Resolve one scope path to the elements it selects.
///
/// A path is `::`-separated and may end in wildcard segments:
///
/// | Path | Selects |
/// | --- | --- |
/// | `vehicle` | `vehicle` and every element under it |
/// | `vehicle::*` | the direct children of `vehicle` |
/// | `vehicle::**` | every element under `vehicle`, at any depth |
/// | `vehicle::*::**` | everything under each direct child |
///
/// A membership expose (`expose vehicle`) selecting the subtree rather than the
/// single element is a deliberate reading: a view renders a containment tree,
/// and the pilot's `SafetyFeatureView` — `expose vehicle` plus `filter @Safety`
/// — is meaningless under the narrow reading, because `vehicle` itself carries
/// no `@Safety` and the view would always be empty.
fn resolve_scope(graph: &Graph, view: NodeId, path: &str) -> Vec<NodeId> {
    let segments: Vec<&str> = path.split("::").map(str::trim).collect();
    let wildcard_start = segments
        .iter()
        .position(|segment| is_wildcard(segment))
        .unwrap_or(segments.len());
    let (base_segments, wildcards) = segments.split_at(wildcard_start);

    if base_segments.is_empty() {
        return Vec::new();
    }

    let Some(base) = resolve_base(graph, view, base_segments) else {
        return Vec::new();
    };

    let mut current: BTreeSet<NodeId> = BTreeSet::from([base]);
    if wildcards.is_empty() {
        // A bare membership expose takes the element and its subtree.
        current.extend(descendants(graph, base));
        return current.into_iter().collect();
    }

    for wildcard in wildcards {
        let mut next = BTreeSet::new();
        for node in &current {
            match *wildcard {
                "*" => next.extend(children(graph, *node)),
                "**" => next.extend(descendants(graph, *node)),
                _ => {}
            }
        }
        current = next;
    }

    current.into_iter().collect()
}

/// The element a scope path is anchored at — the non-wildcard head of
/// `vehicle::**`, or the whole path when there is no wildcard.
///
/// The reverse mapping (V-6.3) needs this: a saved view records its scope as
/// `expose <root>::**`, and reading it back into `DiagramSpecDto.root` means
/// binding that path the same way resolution does, from the same place.
pub fn scope_base(graph: &Graph, view: NodeId, path: &str) -> Option<NodeId> {
    let segments: Vec<&str> = path.split("::").map(str::trim).collect();
    let head_len = segments
        .iter()
        .position(|segment| is_wildcard(segment))
        .unwrap_or(segments.len());
    let head = &segments[..head_len];
    if head.is_empty() {
        return None;
    }
    resolve_base(graph, view, head)
}

/// Does this scope path end in a wildcard, i.e. does it name a subtree rather
/// than one element?
pub fn scope_is_wildcard(path: &str) -> bool {
    path.split("::").map(str::trim).any(is_wildcard)
}

/// `::`-qualified name of an element, built from the declared names on its
/// owner chain — `feature.VehicleViews.vehicle` becomes
/// `VehicleViews::vehicle`.
///
/// Derived from ownership rather than by string-munging the element id: id
/// templates are an emission detail that varies per metaclass, and one of them
/// changing should not silently change what a saved view claims to expose.
pub fn qualified_name(graph: &Graph, node: NodeId) -> Option<String> {
    let mut segments = vec![declared_name(graph, node)?];
    for owner in ancestors(graph, node) {
        match declared_name(graph, owner) {
            Some(name) => segments.push(name),
            None => break,
        }
    }
    segments.reverse();
    Some(segments.join("::"))
}

fn is_wildcard(segment: &str) -> bool {
    segment == "*" || segment == "**"
}

/// Resolve the non-wildcard head of a scope path to one element.
///
/// The head is usually written relative to where the view sits — the pilot
/// writes `expose vehicle` in a package that imports `PartsTree::vehicle` — so
/// resolution walks outward from the view's owner and takes the nearest match.
/// Nearest-first is what makes the answer stable under an unrelated element
/// elsewhere in the model sharing a name.
fn resolve_base(graph: &Graph, view: NodeId, segments: &[&str]) -> Option<NodeId> {
    let path = segments.join("::");

    // Already an element id: the resolver upstream managed to bind it.
    if let Some(node) = graph.node_id(&path) {
        return Some(node);
    }

    let (first, rest) = segments.split_first()?;

    // Nearest enclosing scope first, then outward, then the whole model.
    let mut scopes: Vec<Option<NodeId>> = ancestors(graph, view).into_iter().map(Some).collect();
    scopes.push(None);

    for scope in scopes {
        let mut matches: Vec<NodeId> = match scope {
            Some(scope) => descendants(graph, scope)
                .into_iter()
                .filter(|node| declared_name(graph, *node).as_deref() == Some(*first))
                .collect(),
            None => graph
                .elements()
                .iter()
                .filter(|element| {
                    element.properties.to_btree_map().get("declared_name")
                        == Some(&Value::String((*first).to_string()))
                })
                .map(|element| element.id)
                .collect(),
        };
        matches.sort_by_key(|node| graph.element_id(*node).unwrap_or_default().to_string());

        for candidate in matches {
            if let Some(resolved) = walk_named_path(graph, candidate, rest) {
                return Some(resolved);
            }
        }
    }

    None
}

/// Walk the remaining `::` segments down through owned members by name.
fn walk_named_path(graph: &Graph, start: NodeId, segments: &[&str]) -> Option<NodeId> {
    let mut current = start;
    for segment in segments {
        let mut next: Vec<NodeId> = children(graph, current)
            .into_iter()
            .filter(|node| declared_name(graph, *node).as_deref() == Some(*segment))
            .collect();
        next.sort_by_key(|node| graph.element_id(*node).unwrap_or_default().to_string());
        current = next.into_iter().next()?;
    }
    Some(current)
}

fn declared_name(graph: &Graph, node: NodeId) -> Option<String> {
    graph
        .element(node)
        .and_then(|element| string_property(element.properties.to_btree_map().get("declared_name")))
}

/// Owners of `node`, nearest first.
fn ancestors(graph: &Graph, node: NodeId) -> Vec<NodeId> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::from([node]);
    let mut current = node;
    while let Some(owner) = graph.outgoing(current, OWNER).map(|edge| edge.target).next() {
        if !seen.insert(owner) {
            break;
        }
        chain.push(owner);
        current = owner;
    }
    chain
}

fn children(graph: &Graph, node: NodeId) -> Vec<NodeId> {
    graph.incoming(node, OWNER).map(|edge| edge.source).collect()
}

/// Every element under `node`, at any depth. Cycle-safe: a malformed graph
/// with an ownership loop terminates rather than hanging.
fn descendants(graph: &Graph, node: NodeId) -> BTreeSet<NodeId> {
    let mut found = BTreeSet::new();
    let mut queue = VecDeque::from([node]);
    let mut seen = BTreeSet::from([node]);

    while let Some(current) = queue.pop_front() {
        for child in children(graph, current) {
            if seen.insert(child) {
                found.insert(child);
                queue.push_back(child);
            }
        }
    }

    found
}

// ---------------------------------------------------------------- conditions

/// A parsed filter predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Condition {
    /// `@Safety` — the element carries that metadata annotation.
    Metadata(String),
    /// `(as Safety).isMandatory` — an attribute of an applied annotation,
    /// optionally compared against a literal. Without a comparison the
    /// attribute is read as a boolean.
    Attribute {
        metadata: String,
        attribute: String,
        expected: Option<(bool, String)>,
    },
    Not(Box<Condition>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
}

impl Condition {
    fn matches(&self, graph: &Graph, node: NodeId) -> bool {
        let Some(element) = graph.element(node) else {
            return false;
        };
        let properties = element.properties.to_btree_map();

        match self {
            // Metadata features and the implicit metaclass feature only — the
            // specialization hierarchy is deliberately not consulted.
            Condition::Metadata(name) => {
                has_metadata(graph, node, &properties, name)
                    || element.kind.as_ref() == name
                    || short_name(element.kind.as_ref()) == name
            }
            Condition::Attribute {
                metadata,
                attribute,
                expected,
            } => metadata_annotations_named(&properties, metadata)
                .iter()
                .filter_map(|annotation| metadata_string_property(annotation, attribute))
                .any(|value| match expected {
                    Some((equal, literal)) => (&value == literal) == *equal,
                    None => matches!(value.as_str(), "true" | "True" | "TRUE"),
                }),
            Condition::Not(inner) => !inner.matches(graph, node),
            Condition::And(left, right) => left.matches(graph, node) && right.matches(graph, node),
            Condition::Or(left, right) => left.matches(graph, node) || right.matches(graph, node),
        }
    }
}

/// Is this element *content* a view can show, rather than apparatus attached
/// to content?
///
/// A subtree contains more than the model: the annotation elements behind
/// `@Safety`, the view's own `expose` and `filter` members, imports, and
/// documentation are all owned children. None of them is something a diagram
/// draws, and including them corrupts the answer in a specific and misleading
/// way — a `not (@Safety)` filter would happily select the very
/// `MetadataUsage` element that records the `@Safety` application, because the
/// annotation does not itself carry the annotation.
fn is_exposable(graph: &Graph, node: NodeId) -> bool {
    let Some(element) = graph.element(node) else {
        return false;
    };
    let metatype = string_property(element.properties.to_btree_map().get("metatype"))
        .unwrap_or_else(|| element.kind.to_string());

    !matches!(
        short_name(&metatype),
        "MetadataUsage"
            | "Expose"
            | "Import"
            | "ElementFilterMembership"
            | "ViewRenderingMembership"
            | "RenderUsage"
            | "Documentation"
            | "OwningMembership"
            | "CommentUsage"
    )
}

/// Does `node` carry the metadata annotation `name`?
///
/// Lowering writes an applied annotation **two different ways**, and a filter
/// has to see both:
///
/// - `@Safety { isMandatory = true; }` writes an inline entry into the target's
///   own `metadata` property, because the application carries values.
/// - `@Safety;` writes **no inline entry at all** — only an owned
///   `MetadataUsage` child element named `Safety`. The action that populates
///   the inline form requires metadata properties, and a bare application has
///   none.
///
/// Checking only the inline form silently misses every valueless annotation,
/// which is the common case and exactly what the pilot's safety views filter
/// on.
fn has_metadata(
    graph: &Graph,
    node: NodeId,
    properties: &BTreeMap<String, Value>,
    name: &str,
) -> bool {
    if !metadata_annotations_named(properties, name).is_empty() {
        return true;
    }

    children(graph, node).into_iter().any(|child| {
        graph.element(child).is_some_and(|element| {
            let child_properties = element.properties.to_btree_map();
            let is_metadata_usage = string_property(child_properties.get("metatype"))
                .is_some_and(|metatype| short_name(&metatype) == "MetadataUsage");
            is_metadata_usage
                && string_property(child_properties.get("declared_name")).as_deref() == Some(name)
        })
    })
}

fn short_name(qualified: &str) -> &str {
    qualified
        .rsplit("::")
        .next()
        .unwrap_or(qualified)
        .rsplit('.')
        .next()
        .unwrap_or(qualified)
}

/// Parse a filter condition. `None` means the text is not a shape this
/// resolver understands — the caller reports it rather than ignoring it.
fn parse_condition(text: &str) -> Option<Condition> {
    let tokens = tokenize(text);
    let mut parser = ConditionParser { tokens, index: 0 };
    let condition = parser.parse_or()?;
    parser.at_end().then_some(condition)
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in text.chars() {
        match character {
            '@' | '(' | ')' | '.' | '|' | '&' => {
                push_token(&mut tokens, &mut current);
                tokens.push(character.to_string());
            }
            '=' | '!' => {
                // `==` and `!=` are the only two-character operators here.
                push_token(&mut tokens, &mut current);
                match tokens.last().map(String::as_str) {
                    Some("=") if character == '=' => {
                        tokens.pop();
                        tokens.push("==".to_string());
                    }
                    _ => tokens.push(character.to_string()),
                }
            }
            c if c.is_whitespace() => push_token(&mut tokens, &mut current),
            c => current.push(c),
        }
    }
    push_token(&mut tokens, &mut current);

    // Fold `! =` into `!=` after the fact; the loop above only pairs `==`.
    let mut folded: Vec<String> = Vec::new();
    for token in tokens {
        if token == "=" && folded.last().map(String::as_str) == Some("!") {
            folded.pop();
            folded.push("!=".to_string());
        } else {
            folded.push(token);
        }
    }
    folded
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

struct ConditionParser {
    tokens: Vec<String>,
    index: usize,
}

impl ConditionParser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.index).map(String::as_str)
    }

    fn advance(&mut self) -> Option<String> {
        let token = self.tokens.get(self.index).cloned();
        self.index += 1;
        token
    }

    fn eat(&mut self, token: &str) -> bool {
        if self.peek() == Some(token) {
            self.index += 1;
            return true;
        }
        false
    }

    fn at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn parse_or(&mut self) -> Option<Condition> {
        let mut left = self.parse_and()?;
        loop {
            let is_or = matches!(self.peek(), Some("or") | Some("|") | Some("||"));
            if !is_or {
                return Some(left);
            }
            self.advance();
            let right = self.parse_and()?;
            left = Condition::Or(Box::new(left), Box::new(right));
        }
    }

    fn parse_and(&mut self) -> Option<Condition> {
        let mut left = self.parse_unary()?;
        loop {
            let is_and = matches!(self.peek(), Some("and") | Some("&") | Some("&&"));
            if !is_and {
                return Some(left);
            }
            self.advance();
            let right = self.parse_unary()?;
            left = Condition::And(Box::new(left), Box::new(right));
        }
    }

    fn parse_unary(&mut self) -> Option<Condition> {
        if matches!(self.peek(), Some("not") | Some("!")) {
            self.advance();
            return Some(Condition::Not(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Condition> {
        if self.eat("@") {
            return Some(Condition::Metadata(self.parse_qualified_name()?));
        }

        if self.peek() == Some("(") {
            // Either a cast — `(as Safety).attribute` — or a parenthesised
            // sub-expression. The `as` keyword is what tells them apart.
            let checkpoint = self.index;
            self.advance();
            if self.eat("as") {
                let metadata = self.parse_qualified_name()?;
                if !self.eat(")") || !self.eat(".") {
                    return None;
                }
                let attribute = self.parse_qualified_name()?;
                let expected = match self.peek() {
                    Some("==") => {
                        self.advance();
                        Some((true, self.parse_literal()?))
                    }
                    Some("!=") => {
                        self.advance();
                        Some((false, self.parse_literal()?))
                    }
                    _ => None,
                };
                return Some(Condition::Attribute {
                    metadata,
                    attribute,
                    expected,
                });
            }
            self.index = checkpoint;
            self.advance();
            let inner = self.parse_or()?;
            return self.eat(")").then_some(inner);
        }

        None
    }

    /// A name, possibly `::`-qualified. Only the last segment is kept:
    /// annotation matching is by simple name, mirroring
    /// `metadata_annotations_named`.
    fn parse_qualified_name(&mut self) -> Option<String> {
        let mut name = self.advance()?;
        if !is_name(&name) {
            return None;
        }
        while self.peek() == Some("::") {
            self.advance();
            name = self.advance()?;
        }
        Some(short_name(&name).to_string())
    }

    fn parse_literal(&mut self) -> Option<String> {
        let token = self.advance()?;
        Some(token.trim_matches(|c| c == '\'' || c == '"').to_string())
    }
}

fn is_name(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition(text: &str) -> Condition {
        parse_condition(text).unwrap_or_else(|| panic!("`{text}` should parse"))
    }

    #[test]
    fn parses_the_pilot_condition_shapes() {
        assert_eq!(condition("@Safety"), Condition::Metadata("Safety".into()));
        assert_eq!(
            condition("not (@Safety)"),
            Condition::Not(Box::new(Condition::Metadata("Safety".into())))
        );
        assert_eq!(
            condition("@Safety | @Security"),
            Condition::Or(
                Box::new(Condition::Metadata("Safety".into())),
                Box::new(Condition::Metadata("Security".into()))
            )
        );
        assert_eq!(
            condition("@Safety and (as Safety).isMandatory"),
            Condition::And(
                Box::new(Condition::Metadata("Safety".into())),
                Box::new(Condition::Attribute {
                    metadata: "Safety".into(),
                    attribute: "isMandatory".into(),
                    expected: None,
                })
            )
        );
    }

    #[test]
    fn a_qualified_metadata_name_matches_on_its_last_segment() {
        assert_eq!(
            condition("@AnnotationDefinitions::Safety"),
            Condition::Metadata("Safety".into())
        );
    }

    /// `and` binds tighter than `or`, so this is `a or (b and c)`.
    #[test]
    fn and_binds_tighter_than_or() {
        assert_eq!(
            condition("@A | @B and @C"),
            Condition::Or(
                Box::new(Condition::Metadata("A".into())),
                Box::new(Condition::And(
                    Box::new(Condition::Metadata("B".into())),
                    Box::new(Condition::Metadata("C".into()))
                ))
            )
        );
    }

    /// A condition this resolver cannot represent must fail to parse, so the
    /// caller can report it. Returning "true" would silently widen the view.
    #[test]
    fn an_unsupported_condition_does_not_parse() {
        assert_eq!(parse_condition("@Safety and"), None);
        assert_eq!(parse_condition("size > 3"), None);
        assert_eq!(parse_condition(""), None);
    }
}
