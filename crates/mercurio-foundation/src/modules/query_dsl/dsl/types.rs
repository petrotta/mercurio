use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use rhai::{Array, Dynamic, Engine, EvalAltResult, FnPtr, Map, NativeCallContext, Scope};
use serde::Serialize;
use serde_json::Value;

use super::{DslExtensionSpec, DslQueryResult, rhai_dynamic_to_json};
use crate::kir::{KirDocument, KirElement};
use crate::model::{Element, Graph, NodeId};
use crate::model::{collect_specialization_ancestors, element_metatype};
use crate::semantic_services::identity::workspace_revision_for_kir_document;
use crate::semantic_services::mutation::{
    ElementRef, SemanticElementKind, SemanticMutation, WorkspaceRevision, diff_kir_documents,
};
use crate::session::transaction::{
    SemanticChangeSet, SemanticTransaction, TransactionArtifact, TransactionOperation,
    TransactionStatus,
};

#[derive(Debug, Clone)]
pub struct ModelContext {
    graph: Arc<Graph>,
}

#[derive(Debug, Clone)]
pub struct DslAppContext {
    graph: Arc<Graph>,
}

impl DslAppContext {
    pub fn new(graph: Arc<Graph>) -> Self {
        Self { graph }
    }

    pub fn new_model(&mut self, name: String) -> Result<DslTransientModel, Box<EvalAltResult>> {
        DslTransientModel::new_model(name)
    }

    pub fn current_model(&mut self) -> ModelContext {
        ModelContext::new(Arc::clone(&self.graph))
    }

    pub fn capabilities(&mut self) -> Array {
        builtin_capabilities()
            .into_iter()
            .map(|capability| Dynamic::from(capability.to_map()))
            .collect()
    }

    pub fn capability(&mut self, id: String) -> DslCapability {
        DslCapability {
            id,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn requires(&mut self, id: String, version: String) -> Map {
        let available = builtin_capability(&id).is_some();
        map_from_entries([
            ("id", Dynamic::from(id)),
            ("version", Dynamic::from(version)),
            ("available", Dynamic::from(available)),
        ])
    }
}

impl ModelContext {
    pub fn new(graph: Arc<Graph>) -> Self {
        Self { graph }
    }

    pub fn parts(&mut self) -> ElementSet {
        self.user_elements()
    }

    pub fn user_elements(&mut self) -> ElementSet {
        self.elements_matching(|element| ModelLayerSelector::User.includes(element.layer))
    }

    pub fn library_elements(&mut self) -> ElementSet {
        self.elements_matching(|element| ModelLayerSelector::Library.includes(element.layer))
    }

    pub fn elements(&mut self) -> ElementSet {
        self.elements_matching(|_| true)
    }

    pub fn namespaces(&mut self) -> ElementSet {
        let mut user_elements = self.user_elements();
        user_elements.where_metatype_is(Dynamic::from(DslMetatype::new("Namespace")))
    }

    fn elements_matching(&self, mut predicate: impl FnMut(&Element) -> bool) -> ElementSet {
        let ids = self
            .graph
            .elements()
            .iter()
            .filter(|element| predicate(element))
            .map(|element| element.id)
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn element(&mut self, element_id: String) -> Dynamic {
        match self.graph.element_by_element_id(&element_id) {
            Some(element) => Dynamic::from(DslElement {
                id: element.id,
                graph: Arc::clone(&self.graph),
            }),
            None => Dynamic::UNIT,
        }
    }

    pub fn elements_where_contains_any(&mut self, field: &str, expected: &[String]) -> ElementSet {
        let ids = self
            .graph
            .elements()
            .iter()
            .filter(|element| {
                let value = element_property_json(element, field);
                expected
                    .iter()
                    .any(|expected| value_contains(&value, expected))
            })
            .map(|element| element.id)
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn match_pattern(&mut self, pattern: String) -> Result<Array, Box<EvalAltResult>> {
        let query = crate::query_dsl::query::parse_query(&pattern).map_err(|error| {
            Box::new(EvalAltResult::ErrorRuntime(
                error.to_string().into(),
                rhai::Position::NONE,
            ))
        })?;
        let document = graph_to_kir_document(&self.graph);
        let result_set = crate::query_dsl::query::QueryEngine::new(&document)
            .execute(&query)
            .map_err(|error| {
                Box::new(EvalAltResult::ErrorRuntime(
                    error.to_string().into(),
                    rhai::Position::NONE,
                ))
            })?;

        Ok(result_set
            .rows
            .into_iter()
            .map(|row| {
                let map = row
                    .into_iter()
                    .map(|(key, value)| (key.into(), serde_json_to_dynamic(value)))
                    .collect::<Map>();
                Dynamic::from(map)
            })
            .collect())
    }

    pub fn capabilities(&mut self) -> Array {
        builtin_capabilities()
            .into_iter()
            .map(|capability| Dynamic::from(capability.to_map()))
            .collect()
    }

    pub fn capability(&mut self, id: String) -> DslCapability {
        DslCapability {
            id,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn requires(&mut self, id: String, version: String) -> Map {
        let available = builtin_capability(&id).is_some();
        map_from_entries([
            ("id", Dynamic::from(id)),
            ("version", Dynamic::from(version)),
            ("available", Dynamic::from(available)),
        ])
    }

    pub fn changes(&mut self) -> DslChangeSetBuilder {
        DslChangeSetBuilder::default()
    }

    pub fn transaction(&mut self, label: String) -> DslTransactionBuilder {
        DslTransactionBuilder {
            label,
            target: DslTransactionTarget::Workspace {
                graph: Arc::clone(&self.graph),
            },
            operations: Vec::new(),
            pending_actions: Vec::new(),
            change_set_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DslTransientModel {
    state: Arc<RwLock<DslTransientModelState>>,
}

#[derive(Debug, Clone)]
struct DslTransientModelState {
    name: String,
    document: KirDocument,
    graph: Arc<Graph>,
    revision: Option<WorkspaceRevision>,
    commit_count: usize,
}

impl DslTransientModel {
    pub fn new_model(name: String) -> Result<Self, Box<EvalAltResult>> {
        let name = non_empty_string(name).ok_or_else(|| {
            Box::new(EvalAltResult::ErrorRuntime(
                "new_model requires a non-empty model name".into(),
                rhai::Position::NONE,
            ))
        })?;
        let state = DslTransientModelState::new(name).map_err(|error| {
            Box::new(EvalAltResult::ErrorRuntime(
                error.into(),
                rhai::Position::NONE,
            ))
        })?;
        Ok(Self {
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub fn get_name(&mut self) -> String {
        self.with_state(|state| state.name.clone())
    }

    pub fn get_revision(&mut self) -> String {
        self.with_state(|state| {
            state
                .revision
                .as_ref()
                .map(|revision| revision.fingerprint.clone())
                .unwrap_or_else(|| "unrevisioned".to_string())
        })
    }

    pub fn element_count(&mut self) -> i64 {
        self.with_state(|state| state.graph.elements().len() as i64)
    }

    pub fn parts(&mut self) -> ElementSet {
        self.user_elements()
    }

    pub fn user_elements(&mut self) -> ElementSet {
        let mut context = self.context();
        context.user_elements()
    }

    pub fn library_elements(&mut self) -> ElementSet {
        let mut context = self.context();
        context.library_elements()
    }

    pub fn elements(&mut self) -> ElementSet {
        let mut context = self.context();
        context.elements()
    }

    pub fn namespaces(&mut self) -> ElementSet {
        let mut context = self.context();
        context.namespaces()
    }

    pub fn element(&mut self, element_id: String) -> Dynamic {
        let mut context = self.context();
        context.element(element_id)
    }

    pub fn transaction(&mut self, label: String) -> DslTransactionBuilder {
        DslTransactionBuilder {
            label,
            target: DslTransactionTarget::Transient {
                model: self.clone(),
            },
            operations: Vec::new(),
            pending_actions: Vec::new(),
            change_set_index: 0,
        }
    }

    pub fn to_query_result(&self) -> DslQueryResult {
        let (name, revision, element_count, commit_count) = self.with_state(|state| {
            (
                state.name.clone(),
                state
                    .revision
                    .as_ref()
                    .map(|revision| revision.fingerprint.clone())
                    .unwrap_or_else(|| "unrevisioned".to_string()),
                state.graph.elements().len(),
                state.commit_count,
            )
        });
        DslQueryResult {
            columns: vec![
                "kind".into(),
                "name".into(),
                "revision".into(),
                "element_count".into(),
                "commit_count".into(),
            ],
            rows: vec![vec![
                Value::String("transient_model".into()),
                Value::String(name),
                Value::String(revision),
                Value::from(element_count),
                Value::from(commit_count),
            ]],
        }
    }

    fn context(&self) -> ModelContext {
        ModelContext::new(self.with_state(|state| Arc::clone(&state.graph)))
    }

    fn base_revision(&self) -> Option<WorkspaceRevision> {
        self.with_state(|state| state.revision.clone())
    }

    fn default_container(&self) -> String {
        self.with_state(|state| state.name.clone())
    }

    fn preview_transaction(&self, transaction: &SemanticTransaction) -> Map {
        let result = self.with_state(|state| {
            apply_transient_transaction(&state.document, &transaction.operations)
                .map(|document| diff_kir_documents(&state.document, &document))
        });
        match result {
            Ok(diff) => serializable_to_map(transaction.preview_report(diff)),
            Err(error) => serializable_to_map(
                transaction.rejected_report("DSL_TRANSIENT_TRANSACTION_PREVIEW", error),
            ),
        }
    }

    fn commit_transaction(&self, transaction: &SemanticTransaction) -> Map {
        let result = self.with_state_mut(|state| {
            let base_document = state.document.clone();
            let next_document =
                apply_transient_transaction(&base_document, &transaction.operations)?;
            let graph = Graph::from_document(next_document.clone())
                .map_err(|error| format!("could not rebuild transient model graph: {error}"))?;
            let new_revision = workspace_revision_for_kir_document(&next_document)
                .map_err(|error| format!("could not revise transient model: {error}"))?;
            let semantic_diff = diff_kir_documents(&base_document, &next_document);
            let element_count = graph.elements().len();

            state.document = next_document;
            state.graph = Arc::new(graph);
            state.revision = Some(new_revision.clone());
            state.commit_count += 1;

            Ok((
                semantic_diff,
                new_revision,
                element_count,
                state.commit_count,
            ))
        });

        match result {
            Ok((semantic_diff, new_revision, element_count, commit_count)) => {
                let mut report = transaction.preview_report(semantic_diff);
                report.status = TransactionStatus::Committed;
                report.applied = true;
                report.new_revision = Some(new_revision);
                report.artifacts.push(TransactionArtifact {
                    id: format!("artifact.{}.transient_model", transaction.id),
                    kind: "transient_model_commit".to_string(),
                    digest: None,
                    payload: transient_commit_payload(element_count, commit_count),
                });
                report.metadata.insert(
                    "target".to_string(),
                    Value::String("transient_model".to_string()),
                );
                serializable_to_map(report)
            }
            Err(error) => serializable_to_map(
                transaction.rejected_report("DSL_TRANSIENT_TRANSACTION_COMMIT", error),
            ),
        }
    }

    fn with_state<T>(&self, read: impl FnOnce(&DslTransientModelState) -> T) -> T {
        match self.state.read() {
            Ok(state) => read(&state),
            Err(error) => {
                let state = error.into_inner();
                read(&state)
            }
        }
    }

    fn with_state_mut<T>(
        &self,
        write: impl FnOnce(&mut DslTransientModelState) -> Result<T, String>,
    ) -> Result<T, String> {
        match self.state.write() {
            Ok(mut state) => write(&mut state),
            Err(error) => {
                let mut state = error.into_inner();
                write(&mut state)
            }
        }
    }
}

impl DslTransientModelState {
    fn new(name: String) -> Result<Self, String> {
        let document = KirDocument {
            metadata: BTreeMap::from([
                ("name".to_string(), Value::String(name.clone())),
                ("transient".to_string(), Value::Bool(true)),
            ]),
            elements: Vec::new(),
        };
        let graph = Graph::from_document(document.clone())
            .map_err(|error| format!("could not create transient model graph: {error}"))?;
        let revision = workspace_revision_for_kir_document(&document).ok();
        Ok(Self {
            name,
            document,
            graph: Arc::new(graph),
            revision,
            commit_count: 0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ElementSet {
    pub(crate) ids: Vec<NodeId>,
    pub(crate) graph: Arc<Graph>,
}

impl ElementSet {
    pub fn count(&mut self) -> i64 {
        self.ids.len() as i64
    }

    pub fn first(&mut self) -> Dynamic {
        match self.ids.first().copied() {
            Some(id) => Dynamic::from(DslElement {
                id,
                graph: Arc::clone(&self.graph),
            }),
            None => Dynamic::UNIT,
        }
    }

    pub fn collect_elements(&mut self) -> Array {
        self.ids
            .iter()
            .copied()
            .map(|id| {
                Dynamic::from(DslElement {
                    id,
                    graph: Arc::clone(&self.graph),
                })
            })
            .collect()
    }

    pub fn select_fields(&mut self, fields: Array) -> DslQueryResult {
        let columns = fields
            .into_iter()
            .filter_map(|field| field.try_cast::<String>())
            .collect::<Vec<_>>();
        let rows = self
            .ids
            .iter()
            .filter_map(|id| self.graph.element(*id))
            .map(|element| {
                columns
                    .iter()
                    .map(|column| element_property_json(element, column))
                    .collect()
            })
            .collect();

        DslQueryResult { columns, rows }
    }

    pub fn where_eq(&mut self, field: String, expected: Dynamic) -> ElementSet {
        let expected = rhai_dynamic_to_json(expected);
        self.filter_by_property(&field, |value| value == expected)
    }

    pub fn where_ne(&mut self, field: String, expected: Dynamic) -> ElementSet {
        let expected = rhai_dynamic_to_json(expected);
        self.filter_by_property(&field, |value| value != expected)
    }

    pub fn where_contains(&mut self, field: String, expected: String) -> ElementSet {
        self.filter_by_property(&field, |value| value_contains(&value, &expected))
    }

    pub fn where_in(&mut self, field: String, expected: Array) -> ElementSet {
        let expected = expected
            .into_iter()
            .map(rhai_dynamic_to_json)
            .collect::<Vec<_>>();
        self.filter_by_property(&field, |value| expected.iter().any(|item| item == &value))
    }

    pub fn where_model_layer(&mut self, expected: Dynamic) -> ElementSet {
        let Some(selector) = model_layer_operand(expected) else {
            return self.empty();
        };
        let ids = self
            .ids
            .iter()
            .filter_map(|id| {
                let element = self.graph.element(*id)?;
                selector.includes(element.layer).then_some(*id)
            })
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn where_metatype(&mut self, expected: Dynamic) -> ElementSet {
        let Some(expected) = metatype_operand_name(expected) else {
            return self.empty();
        };
        self.filter_by_metatypes(&[expected], IncludeMetatypeSubtypes::No)
    }

    pub fn where_metatype_is(&mut self, expected: Dynamic) -> ElementSet {
        let Some(expected) = metatype_operand_name(expected) else {
            return self.empty();
        };
        self.filter_by_metatypes(&[expected], IncludeMetatypeSubtypes::Yes)
    }

    pub fn where_metatype_in(&mut self, expected: Array) -> ElementSet {
        let expected = expected
            .into_iter()
            .filter_map(metatype_operand_name)
            .collect::<Vec<_>>();
        if expected.is_empty() {
            return self.empty();
        }
        self.filter_by_metatypes(&expected, IncludeMetatypeSubtypes::No)
    }

    pub fn where_metatype_is_any(&mut self, expected: Array) -> ElementSet {
        let expected = expected
            .into_iter()
            .filter_map(metatype_operand_name)
            .collect::<Vec<_>>();
        if expected.is_empty() {
            return self.empty();
        }
        self.filter_by_metatypes(&expected, IncludeMetatypeSubtypes::Yes)
    }

    pub fn order_by(&mut self, field: String) -> ElementSet {
        self.order_by_field(&field, false)
    }

    pub fn order_by_desc(&mut self, field: String) -> ElementSet {
        self.order_by_field(&field, true)
    }

    pub fn related(&mut self, relation: String) -> ElementSet {
        let ids = self
            .ids
            .iter()
            .flat_map(|id| self.graph.outgoing(*id, &relation).map(|edge| edge.target))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn select_related(
        &mut self,
        relation: String,
        source_fields: Array,
        target_fields: Array,
    ) -> DslQueryResult {
        self.select_related_filtered(relation, None, source_fields, target_fields)
    }

    pub fn select_related_where_eq(
        &mut self,
        relation: String,
        target_field: String,
        expected: Dynamic,
        source_fields: Array,
        target_fields: Array,
    ) -> DslQueryResult {
        self.select_related_filtered(
            relation,
            Some((target_field, rhai_dynamic_to_json(expected))),
            source_fields,
            target_fields,
        )
    }

    fn filter_by_property(
        &self,
        field: &str,
        mut predicate: impl FnMut(Value) -> bool,
    ) -> ElementSet {
        let ids = self
            .ids
            .iter()
            .filter_map(|id| {
                let element = self.graph.element(*id)?;
                predicate(element_property_json(element, field)).then_some(*id)
            })
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    fn filter_by_metatypes(
        &self,
        expected: &[String],
        include_subtypes: IncludeMetatypeSubtypes,
    ) -> ElementSet {
        let ids = self
            .ids
            .iter()
            .filter_map(|id| {
                let element = self.graph.element(*id)?;
                element_matches_metatype(&self.graph, element, expected, include_subtypes)
                    .then_some(*id)
            })
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    fn empty(&self) -> ElementSet {
        ElementSet {
            ids: Vec::new(),
            graph: Arc::clone(&self.graph),
        }
    }

    fn order_by_field(&self, field: &str, descending: bool) -> ElementSet {
        let mut ids = self.ids.clone();
        ids.sort_by(|left, right| {
            let ordering = match (self.graph.element(*left), self.graph.element(*right)) {
                (Some(left), Some(right)) => compare_json_values(
                    &element_property_json(left, field),
                    &element_property_json(right, field),
                ),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            let ordering = if descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering.then_with(|| left.cmp(right))
        });
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    fn select_related_filtered(
        &self,
        relation: String,
        target_filter: Option<(String, Value)>,
        source_fields: Array,
        target_fields: Array,
    ) -> DslQueryResult {
        let source_fields = string_array(source_fields);
        let target_fields = string_array(target_fields);
        let columns = source_fields
            .iter()
            .map(|field| format!("source.{field}"))
            .chain(target_fields.iter().map(|field| format!("target.{field}")))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();

        for source_id in &self.ids {
            let Some(source) = self.graph.element(*source_id) else {
                continue;
            };
            for edge in self.graph.outgoing(*source_id, &relation) {
                let Some(target) = self.graph.element(edge.target) else {
                    continue;
                };
                if let Some((field, expected)) = &target_filter
                    && element_property_json(target, field) != *expected
                {
                    continue;
                }

                let row = source_fields
                    .iter()
                    .map(|field| element_property_json(source, field))
                    .chain(
                        target_fields
                            .iter()
                            .map(|field| element_property_json(target, field)),
                    )
                    .collect();
                rows.push(row);
            }
        }

        DslQueryResult { columns, rows }
    }

    pub fn to_query_result(&self) -> DslQueryResult {
        let columns = vec!["element_id".into(), "kind".into(), "layer".into()];
        let rows = self
            .ids
            .iter()
            .filter_map(|id| self.graph.element(*id))
            .map(|element| {
                vec![
                    Value::String(element.element_id.clone()),
                    Value::String(element.kind.as_ref().to_string()),
                    Value::from(element.layer),
                ]
            })
            .collect();
        DslQueryResult { columns, rows }
    }
}

#[derive(Debug, Clone)]
pub struct DslElement {
    pub(crate) id: NodeId,
    pub(crate) graph: Arc<Graph>,
}

impl DslElement {
    fn elem(&self) -> Option<&Element> {
        self.graph.element(self.id)
    }

    pub fn get_id(&mut self) -> String {
        self.elem()
            .map(|element| element.element_id.clone())
            .unwrap_or_default()
    }

    pub fn get_kind(&mut self) -> String {
        self.elem()
            .map(|element| element.kind.as_ref().to_string())
            .unwrap_or_default()
    }

    pub fn get_layer(&mut self) -> i64 {
        self.elem().map_or(0, |element| i64::from(element.layer))
    }

    pub fn get_model_layer(&mut self) -> String {
        self.elem()
            .map(|element| model_layer_label(element.layer).to_string())
            .unwrap_or_default()
    }

    pub fn get_metatype(&mut self) -> String {
        self.elem()
            .and_then(|element| element_metatype_display(&self.graph, element))
            .unwrap_or_default()
    }

    pub fn is_metatype(&mut self, expected: Dynamic) -> bool {
        let Some(expected) = metatype_operand_name(expected) else {
            return false;
        };
        self.elem()
            .map(|element| {
                element_matches_metatype(
                    &self.graph,
                    element,
                    &[expected],
                    IncludeMetatypeSubtypes::Yes,
                )
            })
            .unwrap_or(false)
    }

    pub fn metatype_chain(&mut self) -> Array {
        self.elem()
            .map(|element| {
                element_metatype_label_chain(&self.graph, element)
                    .into_iter()
                    .map(Dynamic::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn property(&mut self, name: String) -> Dynamic {
        if let Some(value) = self
            .elem()
            .and_then(|element| element.properties.get(&name))
            .cloned()
        {
            return serde_json_to_dynamic(value);
        }

        self.owned_feature_property(&name).unwrap_or(Dynamic::UNIT)
    }

    fn owned_feature_property(&self, name: &str) -> Option<Dynamic> {
        for relation in ["features", "members"] {
            for edge in self.graph.outgoing(self.id, relation) {
                let Some(feature) = self.graph.element(edge.target) else {
                    continue;
                };
                if string_property(feature, "declared_name") != Some(name) {
                    continue;
                }
                if let Some(value) = literal_expression_value(feature) {
                    return Some(serde_json_to_dynamic(value));
                }
                if let Some(value) = feature.properties.get("value").cloned() {
                    return Some(serde_json_to_dynamic(value));
                }
            }
        }
        None
    }

    pub fn outgoing(&mut self, relation: String) -> ElementSet {
        let ids = self
            .graph
            .outgoing(self.id, &relation)
            .map(|edge| edge.target)
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn incoming(&mut self, relation: String) -> ElementSet {
        let ids = self
            .graph
            .incoming(self.id, &relation)
            .map(|edge| edge.source)
            .collect();
        ElementSet {
            ids,
            graph: Arc::clone(&self.graph),
        }
    }

    pub fn outgoing_edges(&mut self) -> Array {
        self.graph
            .outgoing_edges(self.id)
            .map(|edge| {
                Dynamic::from(DslEdge {
                    source: edge.source,
                    target: edge.target,
                    relation: edge.relation.as_ref().to_string(),
                    graph: Arc::clone(&self.graph),
                })
            })
            .collect()
    }

    pub fn incoming_edges(&mut self) -> Array {
        self.graph
            .incoming_edges(self.id)
            .map(|edge| {
                Dynamic::from(DslEdge {
                    source: edge.source,
                    target: edge.target,
                    relation: edge.relation.as_ref().to_string(),
                    graph: Arc::clone(&self.graph),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DslMetatype {
    name: String,
}

impl DslMetatype {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn from_name(name: String) -> Self {
        Self::new(name)
    }

    pub fn get_name(&mut self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct DslEdge {
    pub(crate) source: NodeId,
    pub(crate) target: NodeId,
    pub(crate) relation: String,
    pub(crate) graph: Arc<Graph>,
}

impl DslEdge {
    pub fn get_relation(&mut self) -> String {
        self.relation.clone()
    }

    pub fn get_source_id(&mut self) -> String {
        self.graph
            .element_id(self.source)
            .map(str::to_string)
            .unwrap_or_default()
    }

    pub fn get_target_id(&mut self) -> String {
        self.graph
            .element_id(self.target)
            .map(str::to_string)
            .unwrap_or_default()
    }

    pub fn source_element(&mut self) -> Dynamic {
        Dynamic::from(DslElement {
            id: self.source,
            graph: Arc::clone(&self.graph),
        })
    }

    pub fn target_element(&mut self) -> Dynamic {
        Dynamic::from(DslElement {
            id: self.target,
            graph: Arc::clone(&self.graph),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DslCapability {
    id: String,
    graph: Arc<Graph>,
}

impl DslCapability {
    pub fn get_id(&mut self) -> String {
        self.id.clone()
    }

    pub fn available(&mut self) -> bool {
        builtin_capability(&self.id).is_some()
    }

    pub fn readiness(&mut self) -> Map {
        match builtin_capability(&self.id) {
            Some(capability) => map_from_entries([
                ("id", Dynamic::from(capability.id)),
                ("status", Dynamic::from("ready")),
                ("deterministic", Dynamic::from(capability.deterministic)),
            ]),
            None => map_from_entries([
                ("id", Dynamic::from(self.id.clone())),
                ("status", Dynamic::from("missing")),
                ("deterministic", Dynamic::from(false)),
            ]),
        }
    }

    pub fn run(&mut self, parameters: Map) -> Map {
        let element_count = self.graph.elements().len() as i64;
        match builtin_capability(&self.id) {
            Some(capability) => map_from_entries([
                ("id", Dynamic::from(capability.id)),
                ("status", Dynamic::from("passed")),
                ("deterministic", Dynamic::from(capability.deterministic)),
                ("element_count", Dynamic::from(element_count)),
                ("parameters", Dynamic::from(parameters)),
            ]),
            None => map_from_entries([
                ("id", Dynamic::from(self.id.clone())),
                ("status", Dynamic::from("missing")),
                ("deterministic", Dynamic::from(false)),
                ("element_count", Dynamic::from(element_count)),
                ("parameters", Dynamic::from(parameters)),
            ]),
        }
    }
}

#[derive(Debug, Clone)]
struct DslCapabilityDescriptor {
    id: String,
    name: String,
    deterministic: bool,
}

impl DslCapabilityDescriptor {
    fn to_map(self) -> Map {
        map_from_entries([
            ("id", Dynamic::from(self.id)),
            ("name", Dynamic::from(self.name)),
            ("deterministic", Dynamic::from(self.deterministic)),
        ])
    }
}

#[derive(Debug, Clone, Default)]
pub struct DslChangeSetBuilder {
    actions: Vec<SemanticMutation>,
}

impl DslChangeSetBuilder {
    pub fn rename(&mut self, element: String, new_name: String) -> Self {
        self.actions.push(SemanticMutation::RenameDeclaration {
            element: ElementRef::new(element),
            new_name,
        });
        self.clone()
    }

    pub fn set_attribute(&mut self, element: String, attribute: String, value: Dynamic) -> Self {
        self.actions.push(SemanticMutation::SetAttribute {
            element: ElementRef::new(element),
            attribute,
            value: rhai_dynamic_to_json(value),
        });
        self.clone()
    }

    pub fn preview(&mut self) -> Map {
        let actions = self
            .actions
            .iter()
            .map(|action| match serde_json::to_value(action) {
                Ok(value) => serde_json_to_dynamic(value),
                Err(_) => Dynamic::UNIT,
            })
            .collect::<Array>();
        let change_set = SemanticChangeSet::new("DSL change set", self.actions.clone());
        map_from_entries([
            ("kind", Dynamic::from("change_set_preview")),
            ("action_count", Dynamic::from(self.actions.len() as i64)),
            ("applies_changes", Dynamic::from(false)),
            ("actions", Dynamic::from(actions)),
            ("change_set", Dynamic::from(serializable_to_map(change_set))),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct DslBuildPlan {
    name: String,
    tasks: BTreeMap<String, DslBuildTask>,
}

impl DslBuildPlan {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tasks: BTreeMap::new(),
        }
    }

    pub fn task(&mut self, name: String) -> Self {
        self.tasks.entry(name).or_default();
        self.clone()
    }

    pub fn depends_on(&mut self, task: String, dependency: String) -> Self {
        self.tasks
            .entry(task)
            .or_default()
            .dependencies
            .insert(dependency.clone());
        self.tasks.entry(dependency).or_default();
        self.clone()
    }

    pub fn operation(&mut self, task: String, operation: String) -> Self {
        self.tasks
            .entry(task)
            .or_default()
            .operations
            .push(operation);
        self.clone()
    }

    pub fn plan(&mut self) -> Map {
        let tasks = self
            .tasks
            .iter()
            .map(|(name, task)| {
                let dependencies = task
                    .dependencies
                    .iter()
                    .cloned()
                    .map(Dynamic::from)
                    .collect::<Array>();
                let operations = task
                    .operations
                    .iter()
                    .cloned()
                    .map(Dynamic::from)
                    .collect::<Array>();
                Dynamic::from(map_from_entries([
                    ("name", Dynamic::from(name.clone())),
                    ("dependencies", Dynamic::from(dependencies)),
                    ("operations", Dynamic::from(operations)),
                ]))
            })
            .collect::<Array>();
        map_from_entries([
            ("kind", Dynamic::from("build_plan")),
            ("name", Dynamic::from(self.name.clone())),
            ("task_count", Dynamic::from(self.tasks.len() as i64)),
            ("tasks", Dynamic::from(tasks)),
        ])
    }
}

#[derive(Debug, Clone, Default)]
struct DslBuildTask {
    dependencies: BTreeSet<String>,
    operations: Vec<String>,
}

#[derive(Debug, Clone)]
enum DslTransactionTarget {
    Workspace { graph: Arc<Graph> },
    Transient { model: DslTransientModel },
}

impl DslTransactionTarget {
    fn base_revision(&self) -> Option<WorkspaceRevision> {
        match self {
            Self::Workspace { graph } => graph_revision(graph),
            Self::Transient { model } => model.base_revision(),
        }
    }

    fn default_container(&self) -> Option<String> {
        match self {
            Self::Workspace { .. } => None,
            Self::Transient { model } => Some(model.default_container()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DslTransactionBuilder {
    label: String,
    target: DslTransactionTarget,
    operations: Vec<TransactionOperation>,
    pending_actions: Vec<SemanticMutation>,
    change_set_index: usize,
}

impl DslTransactionBuilder {
    pub fn create_package(&mut self, name: String) -> Self {
        self.pending_actions.push(SemanticMutation::AddPackage {
            target_file: "transient://model.kir.json".to_string(),
            name,
        });
        self.clone()
    }

    pub fn create_part_def(&mut self, qualified_name: String) -> Self {
        let (container, name) =
            split_child_name(&qualified_name, self.target.default_container().as_deref());
        self.pending_actions.push(SemanticMutation::AddDefinition {
            container: ElementRef::new(container),
            keyword: "part def".to_string(),
            name,
            specializes: Vec::new(),
        });
        self.clone()
    }

    pub fn create_definition(&mut self, container: String, keyword: String, name: String) -> Self {
        self.pending_actions.push(SemanticMutation::AddDefinition {
            container: ElementRef::new(container),
            keyword,
            name,
            specializes: Vec::new(),
        });
        self.clone()
    }

    pub fn create_element(&mut self, container: String, metaclass: String, name: String) -> Self {
        self.pending_actions.push(SemanticMutation::AddElement {
            container: ElementRef::new(container),
            kind: SemanticElementKind::new(metaclass),
            name,
            ty: None,
            specializes: Vec::new(),
            properties: BTreeMap::new(),
        });
        self.clone()
    }

    pub fn create_typed_element(
        &mut self,
        container: String,
        metaclass: String,
        name: String,
        ty: String,
    ) -> Self {
        self.pending_actions.push(SemanticMutation::AddElement {
            container: ElementRef::new(container),
            kind: SemanticElementKind::new(metaclass),
            name,
            ty: Some(ElementRef::new(ty)),
            specializes: Vec::new(),
            properties: BTreeMap::new(),
        });
        self.clone()
    }

    pub fn rename(&mut self, element: String, new_name: String) -> Self {
        self.pending_actions
            .push(SemanticMutation::RenameDeclaration {
                element: ElementRef::new(element),
                new_name,
            });
        self.clone()
    }

    pub fn set_attribute(&mut self, element: String, attribute: String, value: Dynamic) -> Self {
        self.pending_actions.push(SemanticMutation::SetAttribute {
            element: ElementRef::new(element),
            attribute,
            value: rhai_dynamic_to_json(value),
        });
        self.clone()
    }

    pub fn capability(&mut self, capability_id: String, parameters: Map) -> Self {
        self.flush_pending_change_set();
        self.operations.push(TransactionOperation::capability_run(
            capability_id,
            rhai_dynamic_to_json(Dynamic::from(parameters)),
        ));
        self.clone()
    }

    pub fn build_task(&mut self, task: String) -> Self {
        self.flush_pending_change_set();
        self.operations.push(TransactionOperation::build_task(
            task,
            Vec::new(),
            Vec::new(),
        ));
        self.clone()
    }

    pub fn build_operation(&mut self, task: String, operation: String) -> Self {
        self.flush_pending_change_set();
        self.operations.push(TransactionOperation::build_task(
            task,
            Vec::new(),
            vec![operation],
        ));
        self.clone()
    }

    pub fn build_depends_on(&mut self, task: String, dependency: String) -> Self {
        self.flush_pending_change_set();
        self.operations.push(TransactionOperation::build_task(
            task,
            vec![dependency],
            Vec::new(),
        ));
        self.clone()
    }

    pub fn preview(&mut self) -> Map {
        self.flush_pending_change_set();
        let transaction = self.semantic_transaction();
        match &self.target {
            DslTransactionTarget::Workspace { .. } => {
                serializable_to_map(transaction.preview_report(Default::default()))
            }
            DslTransactionTarget::Transient { model } => model.preview_transaction(&transaction),
        }
    }

    pub fn commit(&mut self) -> Map {
        self.flush_pending_change_set();
        let transaction = self.semantic_transaction();
        match &self.target {
            DslTransactionTarget::Workspace { .. } => {
                serializable_to_map(transaction.rejected_report(
                    "DSL_TRANSACTION_HOST_PERMISSION",
                    "DSL transactions require an explicit host-approved commit operation",
                ))
            }
            DslTransactionTarget::Transient { model } => model.commit_transaction(&transaction),
        }
    }

    fn semantic_transaction(&self) -> SemanticTransaction {
        SemanticTransaction::new(
            self.label.clone(),
            self.target.base_revision(),
            self.operations.clone(),
        )
    }

    fn flush_pending_change_set(&mut self) {
        if self.pending_actions.is_empty() {
            return;
        }
        self.change_set_index += 1;
        let label = format!("{} change set {}", self.label, self.change_set_index);
        let actions = std::mem::take(&mut self.pending_actions);
        self.operations
            .push(TransactionOperation::change_set(SemanticChangeSet::new(
                label, actions,
            )));
    }
}

pub fn register_types(engine: &mut Engine) {
    engine
        .register_type_with_name::<DslAppContext>("AppContext")
        .register_fn("new_model", DslAppContext::new_model)
        .register_fn("current_model", DslAppContext::current_model)
        .register_fn("capabilities", DslAppContext::capabilities)
        .register_fn("capability", DslAppContext::capability)
        .register_fn("requires", DslAppContext::requires);

    engine
        .register_type_with_name::<DslTransientModel>("TransientModel")
        .register_get("name", DslTransientModel::get_name)
        .register_get("revision", DslTransientModel::get_revision)
        .register_fn("element_count", DslTransientModel::element_count)
        .register_fn("parts", DslTransientModel::parts)
        .register_fn("elements", DslTransientModel::elements)
        .register_fn("user_elements", DslTransientModel::user_elements)
        .register_fn("library_elements", DslTransientModel::library_elements)
        .register_fn("namespaces", DslTransientModel::namespaces)
        .register_fn("element", DslTransientModel::element)
        .register_fn("transaction", DslTransientModel::transaction);

    engine
        .register_type_with_name::<ModelContext>("ModelContext")
        .register_fn("parts", ModelContext::parts)
        .register_fn("elements", ModelContext::elements)
        .register_fn("user_elements", ModelContext::user_elements)
        .register_fn("library_elements", ModelContext::library_elements)
        .register_fn("namespaces", ModelContext::namespaces)
        .register_fn("element", ModelContext::element)
        .register_fn("match_pattern", ModelContext::match_pattern)
        .register_fn("capabilities", ModelContext::capabilities)
        .register_fn("capability", ModelContext::capability)
        .register_fn("requires", ModelContext::requires)
        .register_fn("changes", ModelContext::changes)
        .register_fn("transaction", ModelContext::transaction);

    engine
        .register_type_with_name::<ElementSet>("ElementSet")
        .register_fn("count", ElementSet::count)
        .register_fn("first", ElementSet::first)
        .register_fn("collect", ElementSet::collect_elements)
        .register_fn("select", ElementSet::select_fields)
        .register_fn("where_eq", ElementSet::where_eq)
        .register_fn("where_ne", ElementSet::where_ne)
        .register_fn("where_contains", ElementSet::where_contains)
        .register_fn("where_in", ElementSet::where_in)
        .register_fn("where_model_layer", ElementSet::where_model_layer)
        .register_fn("where_metatype", ElementSet::where_metatype)
        .register_fn("where_metatype_is", ElementSet::where_metatype_is)
        .register_fn("where_metatype_in", ElementSet::where_metatype_in)
        .register_fn("where_metatype_is_any", ElementSet::where_metatype_is_any)
        .register_fn("order_by", ElementSet::order_by)
        .register_fn("order_by_desc", ElementSet::order_by_desc)
        .register_fn("related", ElementSet::related)
        .register_fn("select_related", ElementSet::select_related)
        .register_fn(
            "select_related_where_eq",
            ElementSet::select_related_where_eq,
        )
        .register_fn("where", filter_where)
        .register_fn("map", map_elements);

    engine
        .register_type_with_name::<DslElement>("Element")
        .register_get("id", DslElement::get_id)
        .register_get("kind", DslElement::get_kind)
        .register_get("layer", DslElement::get_layer)
        .register_get("model_layer", DslElement::get_model_layer)
        .register_get("metatype", DslElement::get_metatype)
        .register_fn("property", DslElement::property)
        .register_fn("is_metatype", DslElement::is_metatype)
        .register_fn("metatype_chain", DslElement::metatype_chain)
        .register_fn("outgoing", DslElement::outgoing)
        .register_fn("incoming", DslElement::incoming)
        .register_fn("outgoing_edges", DslElement::outgoing_edges)
        .register_fn("incoming_edges", DslElement::incoming_edges);

    engine
        .register_type_with_name::<DslMetatype>("Metatype")
        .register_get("name", DslMetatype::get_name)
        .register_fn("to_string", DslMetatype::get_name);
    engine.register_fn("metatype", DslMetatype::from_name);

    engine
        .register_type_with_name::<DslEdge>("Edge")
        .register_get("relation", DslEdge::get_relation)
        .register_get("source_id", DslEdge::get_source_id)
        .register_get("target_id", DslEdge::get_target_id)
        .register_fn("source", DslEdge::source_element)
        .register_fn("target", DslEdge::target_element);

    engine
        .register_type_with_name::<DslCapability>("Capability")
        .register_get("id", DslCapability::get_id)
        .register_fn("available", DslCapability::available)
        .register_fn("readiness", DslCapability::readiness)
        .register_fn("run", DslCapability::run);

    engine
        .register_type_with_name::<DslChangeSetBuilder>("ChangeSetBuilder")
        .register_fn("rename", DslChangeSetBuilder::rename)
        .register_fn("set_attribute", DslChangeSetBuilder::set_attribute)
        .register_fn("preview", DslChangeSetBuilder::preview);

    engine
        .register_type_with_name::<DslBuildPlan>("BuildPlan")
        .register_fn("task", DslBuildPlan::task)
        .register_fn("depends_on", DslBuildPlan::depends_on)
        .register_fn("operation", DslBuildPlan::operation)
        .register_fn("plan", DslBuildPlan::plan);

    engine.register_fn("build", DslBuildPlan::new);

    engine
        .register_type_with_name::<DslTransactionBuilder>("TransactionBuilder")
        .register_fn("create_package", DslTransactionBuilder::create_package)
        .register_fn("create_part_def", DslTransactionBuilder::create_part_def)
        .register_fn(
            "create_definition",
            DslTransactionBuilder::create_definition,
        )
        .register_fn("create_element", DslTransactionBuilder::create_element)
        .register_fn(
            "create_typed_element",
            DslTransactionBuilder::create_typed_element,
        )
        .register_fn("rename", DslTransactionBuilder::rename)
        .register_fn("set_attribute", DslTransactionBuilder::set_attribute)
        .register_fn("capability", DslTransactionBuilder::capability)
        .register_fn("build_task", DslTransactionBuilder::build_task)
        .register_fn("build_operation", DslTransactionBuilder::build_operation)
        .register_fn("build_depends_on", DslTransactionBuilder::build_depends_on)
        .register_fn("preview", DslTransactionBuilder::preview)
        .register_fn("commit", DslTransactionBuilder::commit);
}

pub fn register_extensions(engine: &mut Engine, extensions: &[DslExtensionSpec]) {
    for model_set in extensions
        .iter()
        .flat_map(|extension| extension.model_sets.iter())
    {
        let name = model_set.name.clone();
        let field = Arc::new(model_set.field.clone());
        let contains_any = Arc::new(model_set.contains_any.clone());
        engine.register_fn(name, move |context: &mut ModelContext| {
            context.elements_where_contains_any(field.as_str(), contains_any.as_slice())
        });
    }
}

fn filter_where(
    context: NativeCallContext,
    set: &mut ElementSet,
    predicate: FnPtr,
) -> Result<ElementSet, Box<EvalAltResult>> {
    let mut kept = Vec::new();
    for id in &set.ids {
        let element = DslElement {
            id: *id,
            graph: Arc::clone(&set.graph),
        };
        if predicate.call_within_context::<bool>(&context, (element,))? {
            kept.push(*id);
        }
    }
    Ok(ElementSet {
        ids: kept,
        graph: Arc::clone(&set.graph),
    })
}

fn map_elements(
    context: NativeCallContext,
    set: &mut ElementSet,
    mapper: FnPtr,
) -> Result<Array, Box<EvalAltResult>> {
    set.ids
        .iter()
        .map(|id| {
            let element = DslElement {
                id: *id,
                graph: Arc::clone(&set.graph),
            };
            mapper.call_within_context::<Dynamic>(&context, (element,))
        })
        .collect()
}

fn element_property_json(element: &Element, column: &str) -> Value {
    let mut segments = column.split('.');
    let Some(first) = segments.next() else {
        return Value::Null;
    };
    let Some(mut value) = element_root_property_json(element, first) else {
        return Value::Null;
    };

    for segment in segments {
        value = match value {
            Value::Object(map) => map.get(segment).cloned().unwrap_or(Value::Null),
            _ => return Value::Null,
        };
    }

    value
}

fn element_root_property_json(element: &Element, property: &str) -> Option<Value> {
    match property {
        "id" | "element_id" => Some(Value::String(element.element_id.clone())),
        "kind" => Some(Value::String(element.kind.as_ref().to_string())),
        "layer" => Some(Value::from(element.layer)),
        "layer_name" | "model_layer" => {
            Some(Value::String(model_layer_label(element.layer).into()))
        }
        "metatype_name" => fallback_metatype_display_from_element(element).map(Value::String),
        "metatype_chain" => Some(Value::Array(
            fallback_metatype_chain_from_element(element)
                .into_iter()
                .map(Value::String)
                .collect(),
        )),
        "qualified_name" => element
            .properties
            .get(property)
            .cloned()
            .or_else(|| qualified_name_from_element_id(&element.element_id).map(Value::String)),
        "metatype" => direct_metatype_property_json(element),
        property => element.properties.get(property).cloned(),
    }
}

fn direct_metatype_property_json(element: &Element) -> Option<Value> {
    element.properties.get("metatype").cloned().or_else(|| {
        element
            .properties
            .get("metadata")
            .and_then(|metadata| metadata.get("metatype"))
            .cloned()
    })
}

fn fallback_metatype_display_from_element(element: &Element) -> Option<String> {
    fallback_metatype_candidates_from_element(element)
        .first()
        .map(|name| metatype_tail(name).to_string())
}

fn fallback_metatype_chain_from_element(element: &Element) -> Vec<String> {
    let mut labels = Vec::new();
    if let Some(direct) = fallback_metatype_display_from_element(element) {
        push_unique(&mut labels, direct.clone());
        for ancestor in fallback_metatype_ancestor_names(&direct) {
            push_unique(&mut labels, ancestor);
        }
    }
    labels
}

fn fallback_metatype_candidates_from_element(element: &Element) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(value) = direct_metatype_property_json(element) {
        collect_metatype_value_candidates(&value, &mut candidates);
    }
    if candidates.is_empty() {
        push_unique(&mut candidates, element.kind.as_ref().to_string());
    }
    candidates
}

fn qualified_name_from_element_id(element_id: &str) -> Option<String> {
    let (_, qualified_name) = element_id.split_once('.')?;
    (!qualified_name.is_empty()).then(|| qualified_name.to_string())
}

fn string_array(values: Array) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| value.try_cast::<String>())
        .collect()
}

fn apply_transient_transaction(
    document: &KirDocument,
    operations: &[TransactionOperation],
) -> Result<KirDocument, String> {
    let mut next = document.clone();
    for (operation_index, operation) in operations.iter().enumerate() {
        match operation {
            TransactionOperation::ChangeSet { change_set } => {
                for action in &change_set.operations {
                    apply_transient_action(&mut next, action)
                        .map_err(|error| format!("operation {operation_index}: {error}"))?;
                }
            }
            TransactionOperation::CapabilityRun { .. }
            | TransactionOperation::BuildTask { .. }
            | TransactionOperation::DslScript { .. } => {
                return Err(format!(
                    "operation {operation_index} is not an in-memory model edit"
                ));
            }
        }
    }
    Ok(next)
}

fn apply_transient_action(
    document: &mut KirDocument,
    action: &SemanticMutation,
) -> Result<(), String> {
    match action {
        SemanticMutation::AddPackage { target_file, name } => {
            apply_transient_add_package(document, target_file, name)
        }
        SemanticMutation::AddDefinition {
            container,
            keyword,
            name,
            specializes,
        } => apply_transient_add_definition(document, container, keyword, name, specializes),
        SemanticMutation::AddElement {
            container,
            kind,
            name,
            ty,
            specializes,
            properties,
        } => apply_transient_add_element(
            document,
            container,
            &kind.metaclass,
            name,
            ty,
            specializes,
            properties,
        ),
        SemanticMutation::AddUsage {
            container,
            keyword,
            name,
            ty,
            specializes,
        } => apply_transient_add_usage(document, container, keyword, name, ty, specializes),
        SemanticMutation::RenameDeclaration { element, new_name } => {
            apply_transient_rename(document, element, new_name)
        }
        SemanticMutation::SetAttribute {
            element,
            attribute,
            value,
        } => apply_transient_set_attribute(document, element, attribute, value.clone()),
        _ => Err(format!(
            "transient model commit does not support action {} yet",
            semantic_mutation_label(action)
        )),
    }
}

fn apply_transient_add_package(
    document: &mut KirDocument,
    target_file: &str,
    name: &str,
) -> Result<(), String> {
    let qualified_name = normalize_qualified_name(name)?;
    let id = format!("pkg.{qualified_name}");
    ensure_missing_element(document, &id, &qualified_name)?;

    let declared_name = qualified_name_tail(&qualified_name).to_string();
    let owner_id = match parent_qualified_name(&qualified_name) {
        Some(parent) => Some(
            resolve_element_ref(document, &ElementRef::new(parent))?
                .id
                .clone(),
        ),
        None => None,
    };

    let mut properties = BTreeMap::from([
        ("declared_name".to_string(), Value::String(declared_name)),
        (
            "qualified_name".to_string(),
            Value::String(qualified_name.clone()),
        ),
    ]);
    if let Some(owner_id) = &owner_id {
        properties.insert("owner".to_string(), Value::String(owner_id.clone()));
    }
    if !target_file.trim().is_empty() {
        properties.insert(
            "source_file".to_string(),
            Value::String(target_file.to_string()),
        );
    }

    document.elements.push(KirElement {
        id: id.clone(),
        kind: "Package".to_string(),
        layer: 2,
        properties,
    });

    if let Some(owner_id) = owner_id {
        add_member_reference(document, &owner_id, &id)?;
    }

    Ok(())
}

fn apply_transient_add_definition(
    document: &mut KirDocument,
    container: &ElementRef,
    keyword: &str,
    name: &str,
    specializes: &[ElementRef],
) -> Result<(), String> {
    let container_id = resolve_element_ref(document, container)?.id.clone();
    let container_qualified_name =
        element_qualified_name(resolve_element_ref(document, container)?);
    let declared_name = normalize_declared_name(name)?;
    let qualified_name = format!("{container_qualified_name}.{declared_name}");
    let id = format!("{}.{qualified_name}", definition_id_prefix(keyword));
    ensure_missing_element(document, &id, &qualified_name)?;

    let mut properties = BTreeMap::from([
        ("declared_name".to_string(), Value::String(declared_name)),
        (
            "qualified_name".to_string(),
            Value::String(qualified_name.clone()),
        ),
        ("owner".to_string(), Value::String(container_id.clone())),
    ]);
    if !specializes.is_empty() {
        properties.insert(
            "specializes".to_string(),
            Value::Array(
                specializes
                    .iter()
                    .map(|element| Value::String(element.qualified_name.clone()))
                    .collect(),
            ),
        );
    }

    document.elements.push(KirElement {
        id: id.clone(),
        kind: definition_kind(keyword),
        layer: 2,
        properties,
    });
    add_member_reference(document, &container_id, &id)
}

fn apply_transient_add_element(
    document: &mut KirDocument,
    container: &ElementRef,
    metaclass: &str,
    name: &str,
    ty: &Option<ElementRef>,
    specializes: &[ElementRef],
    element_properties: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let container_id = resolve_element_ref(document, container)?.id.clone();
    let container_qualified_name =
        element_qualified_name(resolve_element_ref(document, container)?);
    let declared_name = normalize_declared_name(name)?;
    let qualified_name = format!("{container_qualified_name}.{declared_name}");
    let id = format!(
        "{}.{}",
        semantic_element_id_prefix(metaclass),
        qualified_name
    );
    ensure_missing_element(document, &id, &qualified_name)?;

    let mut properties = BTreeMap::from([
        ("declared_name".to_string(), Value::String(declared_name)),
        (
            "qualified_name".to_string(),
            Value::String(qualified_name.clone()),
        ),
        ("owner".to_string(), Value::String(container_id.clone())),
    ]);
    if let Some(ty) = ty {
        properties.insert("type".to_string(), Value::String(ty.qualified_name.clone()));
    }
    if !specializes.is_empty() {
        properties.insert(
            "specializes".to_string(),
            Value::Array(
                specializes
                    .iter()
                    .map(|element| Value::String(element.qualified_name.clone()))
                    .collect(),
            ),
        );
    }
    properties.extend(element_properties.clone());

    document.elements.push(KirElement {
        id: id.clone(),
        kind: metaclass.to_string(),
        layer: 2,
        properties,
    });
    add_member_reference(document, &container_id, &id)
}

fn apply_transient_add_usage(
    document: &mut KirDocument,
    container: &ElementRef,
    keyword: &str,
    name: &str,
    ty: &Option<ElementRef>,
    specializes: &[ElementRef],
) -> Result<(), String> {
    let container_id = resolve_element_ref(document, container)?.id.clone();
    let container_qualified_name =
        element_qualified_name(resolve_element_ref(document, container)?);
    let declared_name = normalize_declared_name(name)?;
    let qualified_name = format!("{container_qualified_name}.{declared_name}");
    let id = format!("{}.{qualified_name}", usage_id_prefix(keyword));
    ensure_missing_element(document, &id, &qualified_name)?;

    let mut properties = BTreeMap::from([
        ("declared_name".to_string(), Value::String(declared_name)),
        (
            "qualified_name".to_string(),
            Value::String(qualified_name.clone()),
        ),
        ("owner".to_string(), Value::String(container_id.clone())),
    ]);
    if let Some(ty) = ty {
        properties.insert("type".to_string(), Value::String(ty.qualified_name.clone()));
    }
    if !specializes.is_empty() {
        properties.insert(
            "specializes".to_string(),
            Value::Array(
                specializes
                    .iter()
                    .map(|element| Value::String(element.qualified_name.clone()))
                    .collect(),
            ),
        );
    }

    document.elements.push(KirElement {
        id: id.clone(),
        kind: usage_kind(keyword),
        layer: 2,
        properties,
    });
    add_member_reference(document, &container_id, &id)
}

fn apply_transient_rename(
    document: &mut KirDocument,
    element: &ElementRef,
    new_name: &str,
) -> Result<(), String> {
    let new_name = normalize_declared_name(new_name)?;
    let target = resolve_element_ref_mut(document, element)?;
    target
        .properties
        .insert("declared_name".to_string(), Value::String(new_name.clone()));

    let old_qualified_name = element_qualified_name(target);
    let new_qualified_name = match parent_qualified_name(&old_qualified_name) {
        Some(parent) => format!("{parent}.{new_name}"),
        None => new_name,
    };
    target.properties.insert(
        "qualified_name".to_string(),
        Value::String(new_qualified_name),
    );
    Ok(())
}

fn apply_transient_set_attribute(
    document: &mut KirDocument,
    element: &ElementRef,
    attribute: &str,
    value: Value,
) -> Result<(), String> {
    let attribute = non_empty_string(attribute.to_string())
        .ok_or_else(|| "set_attribute requires a non-empty attribute name".to_string())?;
    let target = resolve_element_ref_mut(document, element)?;
    target.properties.insert(attribute, value);
    Ok(())
}

fn ensure_missing_element(
    document: &KirDocument,
    id: &str,
    qualified_name: &str,
) -> Result<(), String> {
    if document.elements.iter().any(|element| {
        element.id == id
            || element
                .properties
                .get("qualified_name")
                .and_then(Value::as_str)
                == Some(qualified_name)
    }) {
        Err(format!("transient model already contains {qualified_name}"))
    } else {
        Ok(())
    }
}

fn resolve_element_ref<'a>(
    document: &'a KirDocument,
    element_ref: &ElementRef,
) -> Result<&'a KirElement, String> {
    document
        .elements
        .iter()
        .find(|element| element_matches_ref(element, element_ref))
        .ok_or_else(|| {
            format!(
                "transient model element not found: {}",
                element_ref.qualified_name
            )
        })
}

fn resolve_element_ref_mut<'a>(
    document: &'a mut KirDocument,
    element_ref: &ElementRef,
) -> Result<&'a mut KirElement, String> {
    document
        .elements
        .iter_mut()
        .find(|element| element_matches_ref(element, element_ref))
        .ok_or_else(|| {
            format!(
                "transient model element not found: {}",
                element_ref.qualified_name
            )
        })
}

fn element_matches_ref(element: &KirElement, element_ref: &ElementRef) -> bool {
    element.id == element_ref.qualified_name
        || element
            .properties
            .get("qualified_name")
            .and_then(Value::as_str)
            == Some(element_ref.qualified_name.as_str())
        || element
            .properties
            .get("declared_name")
            .and_then(Value::as_str)
            == Some(element_ref.qualified_name.as_str())
}

fn add_member_reference(
    document: &mut KirDocument,
    owner_id: &str,
    member_id: &str,
) -> Result<(), String> {
    let owner = document
        .elements
        .iter_mut()
        .find(|element| element.id == owner_id)
        .ok_or_else(|| format!("transient model owner not found: {owner_id}"))?;
    let members = owner
        .properties
        .entry("members".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    match members {
        Value::Array(items) => {
            if !items.iter().any(|item| item.as_str() == Some(member_id)) {
                items.push(Value::String(member_id.to_string()));
            }
            Ok(())
        }
        _ => Err(format!(
            "transient model owner {owner_id} has non-list members"
        )),
    }
}

fn split_child_name(value: &str, default_container: Option<&str>) -> (String, String) {
    let trimmed = value.trim();
    match trimmed.rsplit_once('.') {
        Some((container, name)) if !container.trim().is_empty() && !name.trim().is_empty() => {
            (container.trim().to_string(), name.trim().to_string())
        }
        _ => (
            default_container.unwrap_or_default().to_string(),
            trimmed.to_string(),
        ),
    }
}

fn normalize_qualified_name(value: &str) -> Result<String, String> {
    let segments = value
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err("qualified name must not be empty".to_string());
    }
    Ok(segments.join("."))
}

fn normalize_declared_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.contains('.') {
        Err(format!(
            "declared name must be a non-empty local name: {value}"
        ))
    } else {
        Ok(value.to_string())
    }
}

fn parent_qualified_name(value: &str) -> Option<&str> {
    value.rsplit_once('.').map(|(parent, _)| parent)
}

fn qualified_name_tail(value: &str) -> &str {
    value
        .rsplit_once('.')
        .map(|(_, tail)| tail)
        .unwrap_or(value)
}

fn element_qualified_name(element: &KirElement) -> String {
    element
        .properties
        .get("qualified_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            element
                .properties
                .get("declared_name")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| element.id.clone())
        })
}

fn definition_id_prefix(keyword: &str) -> &'static str {
    match normalized_keyword(keyword).as_str() {
        "package" => "pkg",
        _ => "type",
    }
}

fn usage_id_prefix(keyword: &str) -> &'static str {
    match normalized_keyword(keyword).as_str() {
        "attribute" | "attr" => "feature",
        _ => "usage",
    }
}

fn semantic_element_id_prefix(metaclass: &str) -> &'static str {
    if metaclass.ends_with("Definition") || metaclass == "Classifier" || metaclass == "Type" {
        "type"
    } else if metaclass == "Package" {
        "pkg"
    } else if metaclass.ends_with("Usage") || metaclass.ends_with("Feature") {
        "usage"
    } else {
        "element"
    }
}

fn definition_kind(keyword: &str) -> String {
    match normalized_keyword(keyword).as_str() {
        "part" | "partdef" | "partdefinition" => "PartDefinition".to_string(),
        "requirement" | "requirementdef" | "requirementdefinition" => {
            "RequirementDefinition".to_string()
        }
        "port" | "portdef" | "portdefinition" => "PortDefinition".to_string(),
        "interface" | "interfacedef" | "interfacedefinition" => "InterfaceDefinition".to_string(),
        "action" | "actiondef" | "actiondefinition" => "ActionDefinition".to_string(),
        "state" | "statedef" | "statedefinition" => "StateDefinition".to_string(),
        "attribute" | "attributedef" | "attributedefinition" => "AttributeDefinition".to_string(),
        other => pascal_definition_kind(other, "Definition"),
    }
}

fn usage_kind(keyword: &str) -> String {
    match normalized_keyword(keyword).as_str() {
        "part" | "partusage" => "PartUsage".to_string(),
        "requirement" | "requirementusage" => "RequirementUsage".to_string(),
        "port" | "portusage" => "PortUsage".to_string(),
        "interface" | "interfaceusage" => "InterfaceUsage".to_string(),
        "action" | "actionusage" => "ActionUsage".to_string(),
        "state" | "stateusage" => "StateUsage".to_string(),
        "attribute" | "attr" | "attributeusage" => "AttributeUsage".to_string(),
        other => pascal_definition_kind(other, "Usage"),
    }
}

fn normalized_keyword(keyword: &str) -> String {
    keyword
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn pascal_definition_kind(keyword: &str, suffix: &str) -> String {
    if keyword.is_empty() {
        return suffix.to_string();
    }
    let mut chars = keyword.chars();
    let first = chars
        .next()
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_default();
    let rest = chars.collect::<String>();
    format!("{first}{rest}{suffix}")
}

fn semantic_mutation_label(action: &SemanticMutation) -> &'static str {
    match action {
        SemanticMutation::AddPackage { .. } => "add_package",
        SemanticMutation::AddElement { .. } => "add_element",
        SemanticMutation::AddDefinition { .. } => "add_definition",
        SemanticMutation::AddUsage { .. } => "add_usage",
        SemanticMutation::AddRelationship { .. } => "add_relationship",
        SemanticMutation::AddMetadataAnnotation { .. } => "add_metadata_annotation",
        SemanticMutation::Remove { .. } => "remove",
        SemanticMutation::RemoveRelationship { .. } => "remove_relationship",
        SemanticMutation::RenameDeclaration { .. } => "rename_declaration",
        SemanticMutation::UpdateUsageType { .. } => "update_usage_type",
        SemanticMutation::SetExpression { .. } => "set_expression",
        SemanticMutation::UpdateSpecializations { .. } => "update_specializations",
        SemanticMutation::MoveDeclaration { .. } => "move_declaration",
        SemanticMutation::SetAttribute { .. } => "set_attribute",
    }
}

fn transient_commit_payload(element_count: usize, commit_count: usize) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("elementCount".to_string(), Value::from(element_count));
    payload.insert("commitCount".to_string(), Value::from(commit_count));
    Value::Object(payload)
}

const KERML_METATYPES: &[&str] = &[
    "Element",
    "Namespace",
    "Package",
    "Type",
    "Classifier",
    "Feature",
    "Step",
    "Behavior",
    "Function",
    "Expression",
    "Relationship",
    "Dependency",
    "Annotation",
    "Documentation",
    "Import",
    "Membership",
    "FeatureMembership",
    "Specialization",
    "Subclassification",
    "FeatureTyping",
    "Subsetting",
    "Redefinition",
    "Conjugation",
    "Association",
    "Connector",
    "Succession",
    "FeatureValue",
    "LiteralExpression",
];

const SYSML_METATYPES: &[&str] = &[
    "PartDefinition",
    "PartUsage",
    "AttributeDefinition",
    "AttributeUsage",
    "ItemDefinition",
    "ItemUsage",
    "PortDefinition",
    "PortUsage",
    "InterfaceDefinition",
    "InterfaceUsage",
    "ActionDefinition",
    "ActionUsage",
    "StateDefinition",
    "StateUsage",
    "ConstraintDefinition",
    "ConstraintUsage",
    "RequirementDefinition",
    "RequirementUsage",
    "UseCaseDefinition",
    "UseCaseUsage",
    "AnalysisCaseDefinition",
    "AnalysisCaseUsage",
    "VerificationCaseDefinition",
    "VerificationCaseUsage",
    "ViewDefinition",
    "ViewUsage",
    "ViewpointDefinition",
    "ViewpointUsage",
    "AllocationDefinition",
    "AllocationUsage",
    "ConnectionDefinition",
    "ConnectionUsage",
    "FlowDefinition",
    "FlowUsage",
    "CalculationDefinition",
    "CalculationUsage",
    "MetadataDefinition",
    "MetadataUsage",
    "ConcernDefinition",
    "ConcernUsage",
];

const GLOBAL_METATYPE_TOKENS: &[&str] = &[
    "Element",
    "Namespace",
    "Package",
    "PartDefinition",
    "PartUsage",
    "RequirementDefinition",
    "RequirementUsage",
    "PortDefinition",
    "PortUsage",
    "InterfaceDefinition",
    "InterfaceUsage",
    "ActionDefinition",
    "ActionUsage",
    "StateDefinition",
    "StateUsage",
    "AllocationDefinition",
    "AllocationUsage",
];

pub fn push_scope_constants(scope: &mut Scope<'_>) {
    scope.push("KerML", metatype_namespace(KERML_METATYPES));
    scope.push("SysML", metatype_namespace(SYSML_METATYPES));
    scope.push("ModelLayer", model_layer_namespace());
    for name in GLOBAL_METATYPE_TOKENS {
        scope.push(*name, DslMetatype::new(*name));
    }
}

fn metatype_namespace(names: &[&str]) -> Map {
    let mut map = Map::new();
    for name in names {
        map.insert((*name).into(), Dynamic::from(DslMetatype::new(*name)));
    }
    map
}

fn model_layer_namespace() -> Map {
    let mut map = Map::new();
    for name in [
        "All",
        "Foundation",
        "Library",
        "User",
        "UserModel",
        "Derived",
    ] {
        map.insert(
            name.into(),
            Dynamic::from(match name {
                "All" => "all",
                "Foundation" => "foundation",
                "Library" => "library",
                "User" | "UserModel" => "user",
                "Derived" => "derived",
                _ => name,
            }),
        );
    }
    map
}

#[derive(Debug, Clone, Copy)]
enum ModelLayerSelector {
    User,
    Library,
    Foundation,
    StandardLibrary,
    Derived,
    All,
    Exact(u8),
}

impl ModelLayerSelector {
    fn includes(self, layer: u8) -> bool {
        match self {
            Self::User => layer == 2,
            Self::Library => layer < 2,
            Self::Foundation => layer == 0,
            Self::StandardLibrary => layer == 1,
            Self::Derived => layer == 3,
            Self::All => true,
            Self::Exact(expected) => layer == expected,
        }
    }
}

fn model_layer_operand(value: Dynamic) -> Option<ModelLayerSelector> {
    if let Some(layer) = value.clone().try_cast::<i64>() {
        return u8::try_from(layer).ok().map(ModelLayerSelector::Exact);
    }
    let text = value.try_cast::<String>()?;
    parse_model_layer(&text)
}

fn parse_model_layer(text: &str) -> Option<ModelLayerSelector> {
    let key = text.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match key.as_str() {
        "user" | "user_model" | "model" | "authored" => Some(ModelLayerSelector::User),
        "library" | "libraries" | "reference" | "stdlib" | "standard_library" => {
            Some(ModelLayerSelector::Library)
        }
        "foundation" | "kernel" => Some(ModelLayerSelector::Foundation),
        "standard" | "standard_library_only" | "stdlib_only" => {
            Some(ModelLayerSelector::StandardLibrary)
        }
        "derived" | "computed" | "feature" | "features" => Some(ModelLayerSelector::Derived),
        "all" | "*" => Some(ModelLayerSelector::All),
        _ => key.parse::<u8>().ok().map(ModelLayerSelector::Exact),
    }
}

fn model_layer_label(layer: u8) -> &'static str {
    match layer {
        0 => "foundation",
        1 => "library",
        2 => "user",
        3 => "derived",
        _ => "other",
    }
}

#[derive(Debug, Clone, Copy)]
enum IncludeMetatypeSubtypes {
    No,
    Yes,
}

fn metatype_operand_name(value: Dynamic) -> Option<String> {
    if let Some(token) = value.clone().try_cast::<DslMetatype>() {
        return non_empty_string(token.name);
    }
    if let Some(text) = value.clone().try_cast::<String>() {
        return non_empty_string(text);
    }
    if let Some(map) = value.try_cast::<Map>() {
        for key in ["name", "id", "element_id", "label"] {
            if let Some(text) = map
                .get(key)
                .cloned()
                .and_then(|value| value.try_cast::<String>())
                .and_then(non_empty_string)
            {
                return Some(text);
            }
        }
    }
    None
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn element_matches_metatype(
    graph: &Graph,
    element: &Element,
    expected: &[String],
    include_subtypes: IncludeMetatypeSubtypes,
) -> bool {
    let candidates = match include_subtypes {
        IncludeMetatypeSubtypes::No => element_direct_metatype_candidates(graph, element),
        IncludeMetatypeSubtypes::Yes => element_metatype_candidates(graph, element),
    };
    expected.iter().any(|expected| {
        candidates
            .iter()
            .any(|candidate| metatype_names_match(candidate, expected))
    })
}

fn element_metatype_display(graph: &Graph, element: &Element) -> Option<String> {
    if let Some(metatype) = element_metatype(graph, element.id) {
        return Some(metamodel_element_label(metatype));
    }
    element_direct_metatype_candidates(graph, element)
        .first()
        .map(|name| metatype_tail(name).to_string())
}

fn element_metatype_label_chain(graph: &Graph, element: &Element) -> Vec<String> {
    let mut labels = Vec::new();
    if let Some(metatype) = element_metatype(graph, element.id) {
        let direct = metamodel_element_label(metatype);
        push_unique(&mut labels, direct.clone());
        for ancestor in collect_specialization_ancestors(graph, metatype.id) {
            push_unique(&mut labels, metamodel_element_label(ancestor));
        }
        for ancestor in fallback_metatype_ancestor_names(&direct) {
            push_unique(&mut labels, ancestor);
        }
        return labels;
    }

    let direct = element_direct_metatype_candidates(graph, element)
        .first()
        .map(|name| metatype_tail(name).to_string());
    if let Some(direct) = direct {
        push_unique(&mut labels, direct.clone());
        for ancestor in fallback_metatype_ancestor_names(&direct) {
            push_unique(&mut labels, ancestor);
        }
    }
    labels
}

fn element_metatype_candidates(graph: &Graph, element: &Element) -> Vec<String> {
    let mut candidates = element_direct_metatype_candidates(graph, element);

    if let Some(metatype) = element_metatype(graph, element.id) {
        for ancestor in collect_specialization_ancestors(graph, metatype.id) {
            push_unique(&mut candidates, ancestor.element_id.clone());
            push_unique(&mut candidates, metamodel_element_label(ancestor));
        }
    }

    for direct in element_direct_metatype_candidates(graph, element) {
        for ancestor in fallback_metatype_ancestor_names(&direct) {
            push_unique(&mut candidates, ancestor);
        }
    }

    candidates
}

fn element_direct_metatype_candidates(graph: &Graph, element: &Element) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(metatype) = element_metatype(graph, element.id) {
        push_unique(&mut candidates, metatype.element_id.clone());
        push_unique(&mut candidates, metamodel_element_label(metatype));
    }
    if let Some(value) = direct_metatype_property_json(element) {
        collect_metatype_value_candidates(&value, &mut candidates);
    }
    if candidates.is_empty() {
        push_unique(&mut candidates, element.kind.as_ref().to_string());
    }
    candidates
}

fn collect_metatype_value_candidates(value: &Value, candidates: &mut Vec<String>) {
    match value {
        Value::String(value) => push_unique(candidates, value.clone()),
        Value::Array(values) => {
            for value in values {
                collect_metatype_value_candidates(value, candidates);
            }
        }
        Value::Object(values) => {
            for key in [
                "id",
                "element_id",
                "qualified_name",
                "declared_name",
                "name",
                "label",
            ] {
                if let Some(value) = values.get(key).and_then(Value::as_str) {
                    push_unique(candidates, value.to_string());
                }
            }
        }
        _ => {}
    }
}

fn metamodel_element_label(element: &Element) -> String {
    string_property(element, "declared_name")
        .map(str::to_string)
        .unwrap_or_else(|| metatype_tail(&element.element_id).to_string())
}

fn fallback_metatype_ancestor_names(name: &str) -> Vec<String> {
    const NONE: &[&str] = &[];
    const ELEMENT: &[&str] = &["Element"];
    const NAMESPACE: &[&str] = &["Namespace", "Element"];
    const TYPE: &[&str] = &["Type", "Namespace", "Element"];
    const CLASSIFIER: &[&str] = &["Classifier", "Type", "Namespace", "Element"];
    const FEATURE: &[&str] = &["Feature", "Element"];
    const RELATIONSHIP: &[&str] = &["Relationship", "Element"];

    let key = normalize_metatype_key(metatype_tail(name));
    let ancestors = match key.as_str() {
        "element" => NONE,
        "namespace" => ELEMENT,
        "package" => NAMESPACE,
        "type" => NAMESPACE,
        "classifier" => TYPE,
        "class" => CLASSIFIER,
        "feature" | "step" => ELEMENT,
        "relationship" | "dependency" | "membership" | "specialization" => ELEMENT,
        _ if key.ends_with("definition") => CLASSIFIER,
        _ if key.ends_with("usage") => FEATURE,
        _ if key.ends_with("relationship") => RELATIONSHIP,
        _ if !key.is_empty() => ELEMENT,
        _ => NONE,
    };
    ancestors.iter().map(|name| (*name).to_string()).collect()
}

fn metatype_names_match(candidate: &str, expected: &str) -> bool {
    let candidate_keys = metatype_match_keys(candidate);
    let expected_keys = metatype_match_keys(expected);
    candidate_keys
        .iter()
        .any(|candidate| expected_keys.contains(candidate))
}

fn metatype_match_keys(value: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let normalized = normalize_metatype_key(value);
    if !normalized.is_empty() {
        keys.insert(normalized);
    }
    let tail = normalize_metatype_key(metatype_tail(value));
    if !tail.is_empty() {
        keys.insert(tail);
    }
    keys
}

fn normalize_metatype_key(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn metatype_tail(value: &str) -> &str {
    let trimmed = value.trim();
    trimmed
        .rsplit(|ch| matches!(ch, ':' | '.' | '/' | '#'))
        .find(|segment| !segment.is_empty())
        .unwrap_or(trimmed)
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !value.trim().is_empty() && !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn builtin_capabilities() -> Vec<DslCapabilityDescriptor> {
    vec![
        DslCapabilityDescriptor {
            id: "mercurio.dsl.analysis".to_string(),
            name: "DSL analysis".to_string(),
            deterministic: true,
        },
        DslCapabilityDescriptor {
            id: "mercurio.dsl.mutation-preview".to_string(),
            name: "DSL mutation preview".to_string(),
            deterministic: true,
        },
    ]
}

fn builtin_capability(id: &str) -> Option<DslCapabilityDescriptor> {
    builtin_capabilities()
        .into_iter()
        .find(|capability| capability.id == id)
}

fn map_from_entries<const N: usize>(entries: [(&str, Dynamic); N]) -> Map {
    entries
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect()
}

fn value_contains(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value.contains(expected),
        Value::Array(values) => values.iter().any(|value| value_contains(value, expected)),
        Value::Object(values) => values.values().any(|value| value_contains(value, expected)),
        _ => false,
    }
}

fn compare_json_values(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        (Value::Number(left), Value::Number(right)) => match (left.as_f64(), right.as_f64()) {
            (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
            _ => left.to_string().cmp(&right.to_string()),
        },
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Array(left), Value::Array(right)) => left.len().cmp(&right.len()),
        (Value::Object(left), Value::Object(right)) => left.len().cmp(&right.len()),
        (left, right) => value_kind_rank(left)
            .cmp(&value_kind_rank(right))
            .then_with(|| left.to_string().cmp(&right.to_string())),
    }
}

fn value_kind_rank(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn string_property<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element.properties.get(name).and_then(Value::as_str)
}

fn literal_expression_value(element: &Element) -> Option<Value> {
    let expression = element.properties.get("expression_ir")?;
    if expression.get("kind").and_then(Value::as_str) == Some("literal") {
        expression.get("value").cloned()
    } else {
        None
    }
}

fn graph_to_kir_document(graph: &Graph) -> KirDocument {
    KirDocument {
        metadata: BTreeMap::new(),
        elements: graph
            .elements()
            .iter()
            .map(|element| KirElement {
                id: element.element_id.clone(),
                kind: element.kind.as_ref().to_string(),
                layer: element.layer,
                properties: element.properties.to_btree_map(),
            })
            .collect(),
    }
}

fn graph_revision(graph: &Graph) -> Option<crate::semantic_services::mutation::WorkspaceRevision> {
    workspace_revision_for_kir_document(&graph_to_kir_document(graph)).ok()
}

fn serializable_to_map(value: impl Serialize) -> Map {
    match serde_json::to_value(value) {
        Ok(value) => json_to_map(value),
        Err(error) => map_from_entries([(
            "error",
            Dynamic::from(format!("could not serialize DSL value: {error}")),
        )]),
    }
}

fn json_to_map(value: Value) -> Map {
    match serde_json_to_dynamic(value).try_cast::<Map>() {
        Some(map) => map,
        None => Map::new(),
    }
}

fn serde_json_to_dynamic(value: Value) -> Dynamic {
    match value {
        Value::String(value) => Dynamic::from(value),
        Value::Number(value) => value
            .as_i64()
            .map(Dynamic::from)
            .or_else(|| value.as_f64().map(Dynamic::from))
            .unwrap_or(Dynamic::UNIT),
        Value::Bool(value) => Dynamic::from(value),
        Value::Array(values) => Dynamic::from(
            values
                .into_iter()
                .map(serde_json_to_dynamic)
                .collect::<Array>(),
        ),
        Value::Object(values) => Dynamic::from(
            values
                .into_iter()
                .map(|(key, value)| (key.into(), serde_json_to_dynamic(value)))
                .collect::<Map>(),
        ),
        Value::Null => Dynamic::UNIT,
    }
}
