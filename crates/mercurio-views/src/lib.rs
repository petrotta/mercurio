//! View specification DTOs and render entrypoints.
//!
//! This crate exposes serializable diagram/table specs, validation diagnostics,
//! and render functions for model-backed views.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// `std::time::Instant::now` panics on `wasm32-unknown-unknown` (no clock).
/// Render timing is diagnostics-only, so on wasm this shim reports zero
/// elapsed time and the slow-phase warnings simply never fire — keeping the
/// crate wasm32-capable (no wall-clock in kernel crates).
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct Instant;

#[cfg(target_arch = "wasm32")]
impl Instant {
    fn now() -> Self {
        Instant
    }

    fn elapsed(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;

use mercurio_model::{
    Element, Graph, MetamodelAttributeRegistry, NodeId, collect_specialization_ancestors,
    effective_properties, element_metatype, metadata_annotations_named, query_element_attributes,
};

pub mod element_view;
pub mod model_views;

pub use element_view::ElementView;
pub use model_views::{
    ElementDetailsDto, ElementPropertyRowDto, ElementPropertyTableDto, ElementSummaryDto,
    ExplorerAttributeDto, GraphDto, GraphEdgeDto, GraphNodeDto, GraphScope, InheritedPropertiesDto,
    InheritedPropertyValueDto, LibraryTreeNodeDto, MetatypeExplorerEdgeDto,
    MetatypeExplorerGraphDto, MetatypeExplorerNodeDto, MetatypeExplorerRequestDto,
    ModelExplorerEdgeDto, ModelExplorerGraphDto, ModelExplorerNodeDto, ModelExplorerRequestDto,
    ModelMetadataDto, SearchResultDto, document_model_metadata_view, element_details, graph_view,
    library_tree_view, library_tree_view_from_document, metatype_explorer_view,
    model_explorer_view, model_metadata_view, search_view,
};

const DEFAULT_MAX_DEPTH: usize = 8;
const DEFAULT_MAX_NODES: usize = 350;
const DEFAULT_MAX_EDGES: usize = 900;
const MAX_RELATION_FANOUT_PER_NODE: usize = 250;
const TIMING_WARNING_THRESHOLD_MS: u128 = 250;
pub const VIEW_SCHEMA: &str = "mercurio.view.v1";
pub const VIEW_SPEC_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewFamilyDto {
    Diagram,
    Table,
    Model,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewRootRequirementDto {
    Required,
    Optional,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewScopeDto {
    WholeModel,
    Subtree,
    Explicit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewKindDescriptor {
    pub id: String,
    pub family: ViewFamilyDto,
    pub title: String,
    pub summary: String,
    pub root: ViewRootRequirementDto,
    #[serde(default)]
    pub root_metatypes: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<ViewScopeDto>,
    pub animatable: bool,
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewCatalogReport {
    pub entries: Vec<ViewCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewCatalogEntry {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub label: String,
    pub spec: ViewDocumentDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewAffordanceDto {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub label: String,
    pub spec: ViewDocumentDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagramKindDto {
    Structure,
    Bdd,
    InternalBlock,
    Requirement,
    Activity,
    StateMachine,
    PackageTree,
    CompositionGraph,
    ReferenceGraph,
    DependencyGraph,
    MetatypeInstanceMap,
    ImpactView,
    PropertyInheritance,
    ValidationView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagramDirectionDto {
    Parents,
    Children,
    Both,
}

impl Default for DiagramDirectionDto {
    fn default() -> Self {
        Self::Children
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramQueryOptionsDto {
    #[serde(default = "default_diagram_relations")]
    pub relations: Vec<String>,
    #[serde(default)]
    pub direction: DiagramDirectionDto,
    #[serde(default = "default_diagram_depth")]
    pub depth: usize,
    #[serde(default = "default_true")]
    pub include_libraries: bool,
    #[serde(default = "default_true")]
    pub include_user_model: bool,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_max_edges")]
    pub max_edges: usize,
}

impl Default for DiagramQueryOptionsDto {
    fn default() -> Self {
        Self {
            relations: default_diagram_relations(),
            direction: DiagramDirectionDto::default(),
            depth: default_diagram_depth(),
            include_libraries: true,
            include_user_model: true,
            max_nodes: default_max_nodes(),
            max_edges: default_max_edges(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramLayoutOptionsDto {
    #[serde(default = "default_layout_engine")]
    pub engine: String,
    #[serde(default = "default_layout_direction")]
    pub direction: String,
}

impl Default for DiagramLayoutOptionsDto {
    fn default() -> Self {
        Self {
            engine: default_layout_engine(),
            direction: default_layout_direction(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramStyleOptionsDto {
    #[serde(default = "default_true")]
    pub show_attributes: bool,
    #[serde(default = "default_true")]
    pub show_edge_labels: bool,
    #[serde(default)]
    pub group_by_layer: bool,
    #[serde(default, alias = "showAffordances")]
    pub show_affordances: bool,
}

impl Default for DiagramStyleOptionsDto {
    fn default() -> Self {
        Self {
            show_attributes: true,
            show_edge_labels: true,
            group_by_layer: false,
            show_affordances: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramSpecDto {
    pub version: u8,
    pub kind: DiagramKindDto,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default)]
    pub query: DiagramQueryOptionsDto,
    #[serde(default)]
    pub layout: DiagramLayoutOptionsDto,
    #[serde(default)]
    pub style: DiagramStyleOptionsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramRenderRequestDto {
    pub spec: DiagramSpecDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelViewKindDto {
    Metadata,
    Graph,
    Search,
    ElementDetails,
    LibraryTree,
    ModelExplorer,
    MetatypeExplorer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelViewSpecDto {
    pub version: u8,
    pub kind: ModelViewKindDto,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default)]
    pub expanded_parents: Vec<String>,
    #[serde(default)]
    pub expanded_children: Vec<String>,
    #[serde(default = "default_true")]
    pub include_reference_edges: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewModeDto {
    Visualization,
    Creation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewDocumentDto {
    pub schema: String,
    pub version: u8,
    pub kind: String,
    pub mode: ViewModeDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagram: Option<DiagramSpecDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<TableSpecDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelViewSpecDto>,
}

impl ViewDocumentDto {
    pub fn diagram(spec: DiagramSpecDto) -> Self {
        Self {
            schema: VIEW_SCHEMA.to_string(),
            version: VIEW_SPEC_VERSION,
            kind: format!("diagram.{}", diagram_kind_name(&spec.kind)),
            mode: ViewModeDto::Visualization,
            diagram: Some(spec),
            table: None,
            model: None,
        }
    }

    pub fn table(spec: TableSpecDto) -> Self {
        Self {
            schema: VIEW_SCHEMA.to_string(),
            version: VIEW_SPEC_VERSION,
            kind: "table".to_string(),
            mode: ViewModeDto::Visualization,
            diagram: None,
            table: Some(spec),
            model: None,
        }
    }

    pub fn model(spec: ModelViewSpecDto) -> Self {
        Self {
            schema: VIEW_SCHEMA.to_string(),
            version: VIEW_SPEC_VERSION,
            kind: model_view_kind_name(&spec.kind).to_string(),
            mode: ViewModeDto::Visualization,
            diagram: None,
            table: None,
            model: Some(spec),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TableKindDto {
    ModelElements,
    Elements,
    Requirements,
    /// DA-4: rows x columns pivot over a relation set. The serde name
    /// (`relationship_matrix`) matches the existing TS `ViewKind` entry.
    RelationshipMatrix,
}

/// DA-4 matrix presets. Each expands into default relations plus row/column
/// element filters inside `render_relationship_matrix`; explicit spec fields
/// always win over the preset expansion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatrixPresetDto {
    Allocation,
    Dsm,
    RequirementsCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableScopeDto {
    WholeModel,
    ContainmentSubtree { root: String },
    ExplicitElements { elements: Vec<String> },
}

impl Default for TableScopeDto {
    fn default() -> Self {
        Self::WholeModel
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableRowTypeDto {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default = "default_true")]
    pub include_subtypes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableColumnSpecDto {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableSpecDto {
    pub version: u8,
    pub kind: TableKindDto,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    #[serde(default)]
    pub scope: TableScopeDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_type: Option<TableRowTypeDto>,
    /// DA-4 (matrix kinds only): scope for the column axis. `None` mirrors the
    /// row axis (the DSM default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_scope: Option<TableScopeDto>,
    /// DA-4 (matrix kinds only): type filter for the column axis, same shape
    /// as the row-type filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_type: Option<TableRowTypeDto>,
    /// DA-4 (matrix kinds only): the relation set that fills cells. Empty
    /// falls back to the preset expansion (or the full known relation set).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<String>,
    /// DA-4 (matrix kinds only): preset expansion for allocation, DSM, and
    /// requirements-coverage matrices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix_preset: Option<MatrixPresetDto>,
    #[serde(default)]
    pub query: DiagramQueryOptionsDto,
    #[serde(default)]
    pub columns: Vec<TableColumnSpecDto>,
    #[serde(default, alias = "showAffordances")]
    pub show_affordances: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableRenderRequestDto {
    pub spec: TableSpecDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableViewDto {
    pub spec: TableSpecDto,
    pub columns: Vec<TableColumnSpecDto>,
    #[serde(default)]
    pub available_columns: Vec<TableColumnSpecDto>,
    pub rows: Vec<TableRowDto>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableRowDto {
    pub id: String,
    pub element: String,
    pub cells: Vec<TableCellDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affordances: Vec<ViewAffordanceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableCellDto {
    pub key: String,
    pub value: String,
    /// DA-4: the relationship element id a matrix cell cites (`None` for
    /// plain table cells, matrix row-header cells, and empty matrix cells).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewValidationDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagramViewDto {
    pub spec: DiagramSpecDto,
    pub symbols: Vec<DiagramSymbolDto>,
    pub nodes: Vec<DiagramNodeDto>,
    pub edges: Vec<DiagramEdgeDto>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewOverlayDto {
    pub version: u8,
    #[serde(default)]
    pub frames: Vec<ViewOverlayFrameDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewOverlayFrameDto {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_s: Option<f64>,
    #[serde(default)]
    pub node_marks: Vec<ViewNodeMarkDto>,
    #[serde(default)]
    pub edge_marks: Vec<ViewEdgeMarkDto>,
    #[serde(default)]
    pub node_values: Vec<ViewNodeValueDto>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewNodeMarkDto {
    pub element: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewEdgeMarkDto {
    pub element: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewNodeValueDto {
    pub element: String,
    pub key: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramSymbolDto {
    pub id: String,
    pub element: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagramNodeDto {
    pub id: String,
    pub symbol: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub badges: Vec<String>,
    pub attributes: Vec<DiagramAttributeDto>,
    pub properties: serde_json::Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affordances: Vec<ViewAffordanceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramAttributeDto {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramEdgeDto {
    pub id: String,
    pub symbol: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagramError {
    UnsupportedKind(DiagramKindDto),
    UnsupportedVersion(u8),
    MissingRoot,
    RootNotFound(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    UnsupportedKind(TableKindDto),
    UnsupportedVersion(u8),
    RootNotFound(String),
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedKind(kind) => write!(f, "table kind is not implemented: {kind:?}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported table spec version: {version}")
            }
            Self::RootNotFound(root) => write!(f, "table root not found: {root}"),
        }
    }
}

impl std::error::Error for TableError {}

impl std::fmt::Display for DiagramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedKind(kind) => write!(f, "diagram kind is not implemented: {kind:?}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported diagram spec version: {version}")
            }
            Self::MissingRoot => write!(f, "diagram root is required"),
            Self::RootNotFound(root) => write!(f, "diagram root not found: {root}"),
        }
    }
}

impl std::error::Error for DiagramError {}

pub fn list_diagram_kinds() -> Vec<DiagramKindDto> {
    vec![
        DiagramKindDto::Structure,
        DiagramKindDto::Bdd,
        DiagramKindDto::InternalBlock,
        DiagramKindDto::Requirement,
        DiagramKindDto::Activity,
        DiagramKindDto::StateMachine,
        DiagramKindDto::PackageTree,
        DiagramKindDto::CompositionGraph,
        DiagramKindDto::ReferenceGraph,
        DiagramKindDto::DependencyGraph,
        DiagramKindDto::MetatypeInstanceMap,
        DiagramKindDto::ImpactView,
        DiagramKindDto::PropertyInheritance,
        DiagramKindDto::ValidationView,
    ]
}

pub fn list_table_kinds() -> Vec<TableKindDto> {
    vec![
        TableKindDto::ModelElements,
        TableKindDto::Elements,
        TableKindDto::Requirements,
        TableKindDto::RelationshipMatrix,
    ]
}

pub fn list_view_kinds() -> Vec<ViewKindDescriptor> {
    let mut descriptors = Vec::new();
    descriptors.extend(
        list_diagram_kinds()
            .into_iter()
            .map(diagram_kind_descriptor),
    );
    descriptors.extend(list_table_kinds().into_iter().map(table_kind_descriptor));
    descriptors.extend(
        [
            ModelViewKindDto::Metadata,
            ModelViewKindDto::Graph,
            ModelViewKindDto::Search,
            ModelViewKindDto::ElementDetails,
            ModelViewKindDto::LibraryTree,
            ModelViewKindDto::ModelExplorer,
            ModelViewKindDto::MetatypeExplorer,
        ]
        .into_iter()
        .map(model_view_kind_descriptor),
    );
    descriptors
}

pub fn view_catalog(graph: &Graph) -> ViewCatalogReport {
    let mut entries = Vec::new();

    entries.push(ViewCatalogEntry {
        kind: "model.metadata".to_string(),
        subject: None,
        label: "Model Metadata".to_string(),
        spec: ViewDocumentDto::model(model_view_spec(
            ModelViewKindDto::Metadata,
            "Model Metadata",
            None,
        )),
    });
    entries.push(ViewCatalogEntry {
        kind: "model.graph".to_string(),
        subject: None,
        label: "Model Graph".to_string(),
        spec: ViewDocumentDto::model(ModelViewSpecDto {
            version: VIEW_SPEC_VERSION,
            kind: ModelViewKindDto::Graph,
            title: "Model Graph".to_string(),
            description: Some("Whole-model graph view.".to_string()),
            root: None,
            graph_scope: Some(GraphScope::ModelPlusContext.as_str().to_string()),
            query: None,
            expanded_parents: Vec::new(),
            expanded_children: Vec::new(),
            include_reference_edges: true,
        }),
    });
    entries.push(ViewCatalogEntry {
        kind: "model.library_tree".to_string(),
        subject: None,
        label: "Library Tree".to_string(),
        spec: ViewDocumentDto::model(model_view_spec(
            ModelViewKindDto::LibraryTree,
            "Library Tree",
            None,
        )),
    });

    if graph.elements().iter().any(|element| element.layer >= 2) {
        entries.push(ViewCatalogEntry {
            kind: "diagram.structure".to_string(),
            subject: None,
            label: "Structure: Whole Model".to_string(),
            spec: ViewDocumentDto::diagram(diagram_spec(
                DiagramKindDto::Structure,
                "Whole Model Structure",
                None,
                default_catalog_relations(),
            )),
        });
        entries.push(ViewCatalogEntry {
            kind: "table.model_elements".to_string(),
            subject: None,
            label: "Table: Model Elements".to_string(),
            spec: ViewDocumentDto::table(TableSpecDto {
                version: VIEW_SPEC_VERSION,
                kind: TableKindDto::ModelElements,
                title: "Model Elements".to_string(),
                description: Some("Whole-model element table.".to_string()),
                root: None,
                target_type: None,
                scope: TableScopeDto::WholeModel,
                row_type: None,
                column_scope: None,
                column_type: None,
                relations: Vec::new(),
                matrix_preset: None,
                query: catalog_query(default_catalog_relations()),
                columns: Vec::new(),
                show_affordances: false,
            }),
        });
    }

    if graph.elements().iter().any(is_requirement_element) {
        entries.push(ViewCatalogEntry {
            kind: "table.requirements".to_string(),
            subject: None,
            label: "Table: Requirements".to_string(),
            spec: ViewDocumentDto::table(TableSpecDto {
                version: VIEW_SPEC_VERSION,
                kind: TableKindDto::Requirements,
                title: "Requirements".to_string(),
                description: Some("Whole-model requirements table.".to_string()),
                root: None,
                target_type: None,
                scope: TableScopeDto::WholeModel,
                row_type: None,
                column_scope: None,
                column_type: None,
                relations: Vec::new(),
                matrix_preset: None,
                query: catalog_query(vec![
                    "owner".to_string(),
                    "satisfy".to_string(),
                    "verify".to_string(),
                ]),
                columns: Vec::new(),
                show_affordances: false,
            }),
        });

        // DA-4: the coverage matrix is offered whenever requirements exist,
        // mirroring the requirements-table guard.
        entries.push(matrix_catalog_entry(
            MatrixPresetDto::RequirementsCoverage,
            "Matrix: Requirements Coverage",
            "Requirements Coverage",
            "Requirement rows against satisfying and verifying elements.",
        ));
    }

    if has_matrix_allocation_content(graph) {
        entries.push(matrix_catalog_entry(
            MatrixPresetDto::Allocation,
            "Matrix: Allocation",
            "Allocation",
            "Action and activity rows allocated onto part columns.",
        ));
    }

    if has_matrix_dsm_content(graph) {
        entries.push(matrix_catalog_entry(
            MatrixPresetDto::Dsm,
            "Matrix: Design Structure (DSM)",
            "Design Structure Matrix",
            "Part-to-part connection, flow, and dependency structure.",
        ));
    }

    for element in graph.elements().iter().filter(|element| element.layer >= 2) {
        if is_package_element(element) {
            entries.push(ViewCatalogEntry {
                kind: "diagram.bdd".to_string(),
                subject: Some(element.element_id.clone()),
                label: format!("BDD: {}", catalog_element_label(element)),
                spec: ViewDocumentDto::diagram(diagram_spec(
                    DiagramKindDto::Bdd,
                    &format!("{} BDD", catalog_element_label(element)),
                    Some(element.element_id.clone()),
                    vec![
                        "owner".to_string(),
                        "part".to_string(),
                        "specializes".to_string(),
                    ],
                )),
            });
        }

        if is_block_definition_element(element) {
            entries.push(ViewCatalogEntry {
                kind: "diagram.bdd".to_string(),
                subject: Some(element.element_id.clone()),
                label: format!("BDD: {}", catalog_element_label(element)),
                spec: ViewDocumentDto::diagram(diagram_spec(
                    DiagramKindDto::Bdd,
                    &format!("{} BDD", catalog_element_label(element)),
                    Some(element.element_id.clone()),
                    vec![
                        "owner".to_string(),
                        "part".to_string(),
                        "specializes".to_string(),
                    ],
                )),
            });

            if has_internal_block_structure(graph, element) {
                entries.push(ViewCatalogEntry {
                    kind: "diagram.internal_block".to_string(),
                    subject: Some(element.element_id.clone()),
                    label: format!("IBD: {}", catalog_element_label(element)),
                    spec: ViewDocumentDto::diagram(diagram_spec(
                        DiagramKindDto::InternalBlock,
                        &format!("{} Internal Block", catalog_element_label(element)),
                        Some(element.element_id.clone()),
                        vec![
                            "owner".to_string(),
                            "owning_type".to_string(),
                            "source".to_string(),
                            "target".to_string(),
                        ],
                    )),
                });
            }
        }

        if is_activity_element(element) {
            entries.push(ViewCatalogEntry {
                kind: "diagram.activity".to_string(),
                subject: Some(element.element_id.clone()),
                label: format!("Activity: {}", catalog_element_label(element)),
                spec: ViewDocumentDto::diagram(diagram_spec(
                    DiagramKindDto::Activity,
                    &format!("{} Activity", catalog_element_label(element)),
                    Some(element.element_id.clone()),
                    vec![
                        "owner".to_string(),
                        "source".to_string(),
                        "target".to_string(),
                    ],
                )),
            });
        }

        if is_state_machine_root_element(graph, element) {
            entries.push(ViewCatalogEntry {
                kind: "diagram.state_machine".to_string(),
                subject: Some(element.element_id.clone()),
                label: format!("State Machine: {}", catalog_element_label(element)),
                spec: ViewDocumentDto::diagram(diagram_spec(
                    DiagramKindDto::StateMachine,
                    &format!("{} State Machine", catalog_element_label(element)),
                    Some(element.element_id.clone()),
                    vec![
                        "owner".to_string(),
                        "source".to_string(),
                        "target".to_string(),
                    ],
                )),
            });
        }

        if is_package_element(element) && package_contains_requirement(graph, element) {
            entries.push(ViewCatalogEntry {
                kind: "diagram.requirement".to_string(),
                subject: Some(element.element_id.clone()),
                label: format!("Requirement Diagram: {}", catalog_element_label(element)),
                spec: ViewDocumentDto::diagram(diagram_spec(
                    DiagramKindDto::Requirement,
                    &format!("{} Requirements", catalog_element_label(element)),
                    Some(element.element_id.clone()),
                    requirement_diagram_relations(),
                )),
            });
        }
    }

    entries.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.subject.cmp(&right.subject))
            .then_with(|| left.label.cmp(&right.label))
    });
    // Label participates in the identity so the three preset matrices (same
    // kind, no subject) survive; same-kind/same-subject duplicates always
    // carry identical labels, so this stays a pure duplicate guard.
    entries.dedup_by(|left, right| {
        left.kind == right.kind && left.subject == right.subject && left.label == right.label
    });

    ViewCatalogReport { entries }
}

fn matrix_catalog_entry(
    preset: MatrixPresetDto,
    label: &str,
    title: &str,
    description: &str,
) -> ViewCatalogEntry {
    ViewCatalogEntry {
        kind: "table.relationship_matrix".to_string(),
        subject: None,
        label: label.to_string(),
        spec: ViewDocumentDto::table(TableSpecDto {
            version: VIEW_SPEC_VERSION,
            kind: TableKindDto::RelationshipMatrix,
            title: title.to_string(),
            description: Some(description.to_string()),
            root: None,
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: Some(preset),
            query: catalog_query(Vec::new()),
            columns: Vec::new(),
            show_affordances: false,
        }),
    }
}

/// Catalog guard: any allocation edge or reified allocation element.
fn has_matrix_allocation_content(graph: &Graph) -> bool {
    graph
        .edges()
        .iter()
        .any(|edge| edge.relation.to_ascii_lowercase().contains("allocat"))
        || graph
            .elements()
            .iter()
            .any(|element| matrix_relationship_kind(element) == Some("allocate"))
}

/// Catalog guard: a connection/flow/dependency relationship whose endpoints
/// both resolve to part-family elements.
fn has_matrix_dsm_content(graph: &Graph) -> bool {
    graph.elements().iter().any(|element| {
        matches!(
            matrix_relationship_kind(element),
            Some("connection") | Some("flow") | Some("dependency")
        ) && matrix_link_endpoints_are_parts(graph, element)
    })
}

fn matrix_link_endpoints_are_parts(graph: &Graph, element: &Element) -> bool {
    let source_is_part = endpoint_values(graph, element, &["source", "sources", "source_feature"])
        .iter()
        .any(|element_id| {
            graph
                .element_by_element_id(element_id)
                .is_some_and(is_matrix_part_element)
        });
    let target_is_part = endpoint_values(
        graph,
        element,
        &["target", "targets", "target_ref", "target_feature"],
    )
    .iter()
    .any(|element_id| {
        graph
            .element_by_element_id(element_id)
            .is_some_and(is_matrix_part_element)
    });
    source_is_part && target_is_part
}

fn diagram_kind_descriptor(kind: DiagramKindDto) -> ViewKindDescriptor {
    let id = format!("diagram.{}", diagram_kind_name(&kind));
    let (title, summary, root, root_metatypes, scopes, animatable) = match kind {
        DiagramKindDto::Structure => (
            "Structure Diagram",
            "Generic relation graph over model elements.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::Bdd => (
            "Block Definition Diagram",
            "Definition-level composition (has-a-part rolled up from part usages) and specialization between block definitions.",
            ViewRootRequirementDto::Optional,
            vec!["Package", "PartDefinition", "ItemDefinition"],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::InternalBlock => (
            "Internal Block Diagram",
            "Internal parts, ports, and connection/interface usages for a block.",
            ViewRootRequirementDto::Required,
            vec!["PartDefinition", "PartUsage", "ItemDefinition"],
            vec![ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::Requirement => (
            "Requirement Diagram",
            "Requirement definitions/usages and derive, satisfy, verify, refine relationships.",
            ViewRootRequirementDto::Optional,
            vec!["Package", "RequirementDefinition", "RequirementUsage"],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::Activity => (
            "Activity Diagram",
            "Activity actions and control or object flows.",
            ViewRootRequirementDto::Required,
            vec!["ActivityDefinition", "ActivityUsage"],
            vec![ViewScopeDto::Subtree],
            true,
        ),
        DiagramKindDto::StateMachine => (
            "State Machine Diagram",
            "States, transitions, and pseudostate markers.",
            ViewRootRequirementDto::Required,
            vec!["StateDefinition", "StateUsage"],
            vec![ViewScopeDto::Subtree],
            true,
        ),
        DiagramKindDto::PackageTree => (
            "Package Tree",
            "Package containment hierarchy.",
            ViewRootRequirementDto::Optional,
            vec!["Package"],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::CompositionGraph => (
            "Composition Graph",
            "Whole-model composition and ownership graph.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::ReferenceGraph => (
            "Reference Graph",
            "Reference relationships between model elements.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::DependencyGraph => (
            "Dependency Graph",
            "Dependency relationships between model elements.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::MetatypeInstanceMap => (
            "Metatype Instance Map",
            "Model elements grouped by metaclass.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::ImpactView => (
            "Impact View",
            "Impact relationships around a selected model element.",
            ViewRootRequirementDto::Required,
            vec![],
            vec![ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::PropertyInheritance => (
            "Property Inheritance",
            "Inherited and overriding properties for a selected element.",
            ViewRootRequirementDto::Required,
            vec![],
            vec![ViewScopeDto::Subtree],
            false,
        ),
        DiagramKindDto::ValidationView => (
            "Validation View",
            "Validation findings attached to model elements.",
            ViewRootRequirementDto::Optional,
            vec![],
            vec![ViewScopeDto::WholeModel, ViewScopeDto::Subtree],
            false,
        ),
    };

    ViewKindDescriptor {
        id,
        family: ViewFamilyDto::Diagram,
        title: title.to_string(),
        summary: summary.to_string(),
        root,
        root_metatypes: root_metatypes.into_iter().map(str::to_string).collect(),
        scopes,
        animatable,
        editable: false,
    }
}

fn table_kind_descriptor(kind: TableKindDto) -> ViewKindDescriptor {
    let id = format!("table.{}", table_kind_name(&kind));
    // `editable` is honest per DA-3: true only where cell-edit gestures are
    // wired in the workbench (Requirements name/text/status, Elements name).
    let (title, summary, root_metatypes, editable) = match kind {
        TableKindDto::ModelElements => (
            "Model Elements Table",
            "Tabular model element inventory.",
            vec![],
            false,
        ),
        TableKindDto::Elements => (
            "Elements Table",
            "Tabular view over a selected element type.",
            vec![],
            true,
        ),
        TableKindDto::Requirements => (
            "Requirements Table",
            "Requirement rows with standard requirement columns.",
            vec!["RequirementDefinition", "RequirementUsage"],
            true,
        ),
        TableKindDto::RelationshipMatrix => (
            "Relationship Matrix",
            "Rows and columns from scoped element sets with cells from a \
             relation set (allocation, DSM, and requirements-coverage presets).",
            vec![],
            // Descriptor honesty (DA-4): the matrix renders read-only; matrix
            // cell editing waits for its gesture wiring.
            false,
        ),
    };

    ViewKindDescriptor {
        id,
        family: ViewFamilyDto::Table,
        title: title.to_string(),
        summary: summary.to_string(),
        root: ViewRootRequirementDto::Optional,
        root_metatypes: root_metatypes.into_iter().map(str::to_string).collect(),
        scopes: vec![
            ViewScopeDto::WholeModel,
            ViewScopeDto::Subtree,
            ViewScopeDto::Explicit,
        ],
        animatable: false,
        editable,
    }
}

fn model_view_kind_descriptor(kind: ModelViewKindDto) -> ViewKindDescriptor {
    let id = model_view_kind_name(&kind).to_string();
    let (title, summary, root, scopes) = match kind {
        ModelViewKindDto::Metadata => (
            "Model Metadata",
            "Model counts, source, and revision metadata.",
            ViewRootRequirementDto::None,
            vec![ViewScopeDto::WholeModel],
        ),
        ModelViewKindDto::Graph => (
            "Model Graph",
            "Whole-model graph DTO for product exploration.",
            ViewRootRequirementDto::None,
            vec![ViewScopeDto::WholeModel],
        ),
        ModelViewKindDto::Search => (
            "Model Search",
            "Search results over model elements.",
            ViewRootRequirementDto::None,
            vec![ViewScopeDto::WholeModel],
        ),
        ModelViewKindDto::ElementDetails => (
            "Element Details",
            "Detailed properties for a selected element.",
            ViewRootRequirementDto::Required,
            vec![ViewScopeDto::Subtree],
        ),
        ModelViewKindDto::LibraryTree => (
            "Library Tree",
            "Mounted library hierarchy.",
            ViewRootRequirementDto::None,
            vec![ViewScopeDto::WholeModel],
        ),
        ModelViewKindDto::ModelExplorer => (
            "Model Explorer",
            "Expandable neighborhood around a selected element.",
            ViewRootRequirementDto::Required,
            vec![ViewScopeDto::Subtree],
        ),
        ModelViewKindDto::MetatypeExplorer => (
            "Metatype Explorer",
            "Metatype-focused neighborhood around a selected element.",
            ViewRootRequirementDto::Required,
            vec![ViewScopeDto::Subtree],
        ),
    };

    ViewKindDescriptor {
        id,
        family: ViewFamilyDto::Model,
        title: title.to_string(),
        summary: summary.to_string(),
        root,
        root_metatypes: Vec::new(),
        scopes,
        animatable: false,
        editable: false,
    }
}

fn diagram_spec(
    kind: DiagramKindDto,
    title: &str,
    root: Option<String>,
    relations: Vec<String>,
) -> DiagramSpecDto {
    DiagramSpecDto {
        version: VIEW_SPEC_VERSION,
        kind,
        title: title.to_string(),
        description: None,
        root,
        query: catalog_query(relations),
        layout: DiagramLayoutOptionsDto::default(),
        style: DiagramStyleOptionsDto::default(),
    }
}

fn model_view_spec(kind: ModelViewKindDto, title: &str, root: Option<String>) -> ModelViewSpecDto {
    ModelViewSpecDto {
        version: VIEW_SPEC_VERSION,
        kind,
        title: title.to_string(),
        description: None,
        root,
        graph_scope: None,
        query: None,
        expanded_parents: Vec::new(),
        expanded_children: Vec::new(),
        include_reference_edges: true,
    }
}

fn catalog_query(relations: Vec<String>) -> DiagramQueryOptionsDto {
    DiagramQueryOptionsDto {
        relations,
        direction: DiagramDirectionDto::Children,
        depth: default_diagram_depth(),
        include_libraries: false,
        include_user_model: true,
        max_nodes: default_max_nodes(),
        max_edges: default_max_edges(),
    }
}

fn default_catalog_relations() -> Vec<String> {
    vec![
        "owner".to_string(),
        "members".to_string(),
        "owning_type".to_string(),
        "specializes".to_string(),
    ]
}

fn catalog_element_label(element: &Element) -> String {
    property_text(element, "qualified_name")
        .or_else(|| property_text(element, "declared_name"))
        .unwrap_or_else(|| label_for_id(&element.element_id))
}

fn is_package_element(element: &Element) -> bool {
    kind_matches(element, &["Package"])
}

fn is_block_definition_element(element: &Element) -> bool {
    kind_matches(
        element,
        &[
            "PartDefinition",
            "BlockDefinition",
            "ItemDefinition",
            "ComponentDefinition",
        ],
    )
}

fn is_activity_element(element: &Element) -> bool {
    kind_matches(element, &["ActivityDefinition", "ActivityUsage"])
}

fn is_requirement_element(element: &Element) -> bool {
    if requirement_relationship_kind(element).is_some() {
        return false;
    }
    let semantic_text = element_semantic_text(element);
    kind_matches(
        element,
        &["RequirementDefinition", "RequirementUsage", "Requirement"],
    ) || semantic_text.contains("requirementdefinition")
        || semantic_text.contains("requirementusage")
}

fn has_internal_block_structure(graph: &Graph, element: &Element) -> bool {
    graph.elements().iter().any(|candidate| {
        internal_block_owned_by(candidate, &element.element_id)
            && (is_internal_block_part_or_port(candidate) || is_internal_block_connector(candidate))
    })
}

fn package_contains_requirement(graph: &Graph, element: &Element) -> bool {
    let subtree = collect_containment_subtree_ids(graph, element.id);
    graph
        .elements()
        .iter()
        .any(|candidate| subtree.contains(&candidate.id) && is_requirement_element(candidate))
}

fn internal_block_owned_by(element: &Element, root_id: &str) -> bool {
    diagram_string_property_values(
        element,
        &[
            "owner",
            "owning_type",
            "owningType",
            "owning_definition",
            "owningDefinition",
            "owning_namespace",
            "owningNamespace",
        ],
    )
    .iter()
    .any(|owner| owner == root_id)
}

fn is_internal_block_part_or_port(element: &Element) -> bool {
    let role = internal_block_node_role(element);
    matches!(role.as_deref(), Some("part") | Some("port"))
}

fn is_internal_block_connector(element: &Element) -> bool {
    kind_matches(
        element,
        &[
            "ConnectionUsage",
            "Connector",
            "InterfaceUsage",
            "BindingConnector",
            "SuccessionFlow",
        ],
    ) || element_semantic_text(element).contains("connection")
        || element_semantic_text(element).contains("interfaceusage")
}

fn internal_block_node_role(element: &Element) -> Option<String> {
    let text = element_semantic_text(element);
    if text.contains("port") {
        Some("port".to_string())
    } else if text.contains("partusage")
        || text.contains("itemusage")
        || text.contains("attributeusage")
        || text.contains("referencusage")
        || text.contains("referenceusage")
    {
        Some("part".to_string())
    } else if is_block_definition_element(element) {
        Some("block".to_string())
    } else {
        None
    }
}

fn internal_block_node_symbol(
    element: &Element,
) -> Option<(String, serde_json::Map<String, Value>)> {
    let role = internal_block_node_role(element)?;
    let mut properties = serde_json::Map::new();
    let shape = match role.as_str() {
        "block" => "block",
        "port" => "port",
        _ => "part",
    };
    properties.insert("shape".to_string(), Value::String(shape.to_string()));
    Some((role, properties))
}

fn internal_block_node_badges(element: &Element) -> Vec<String> {
    internal_block_node_role(element)
        .map(|role| vec![role])
        .unwrap_or_else(|| vec![format!("L{}", element.layer)])
}

fn requirement_diagram_relations() -> Vec<String> {
    [
        "owner",
        "derive",
        "derived_from",
        "satisfy",
        "verify",
        "refine",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn requirement_relation(relation: &str) -> Option<&'static str> {
    let normalized = relation.to_ascii_lowercase();
    if normalized.contains("satisfy") {
        Some("satisfy")
    } else if normalized.contains("verify") {
        Some("verify")
    } else if normalized.contains("refine") {
        Some("refine")
    } else if normalized.contains("derive") || normalized.contains("derived") {
        Some("derive")
    } else {
        None
    }
}

fn requirement_relationship_kind(element: &Element) -> Option<&'static str> {
    let text = element_semantic_text(element);
    ["satisfy", "verify", "refine", "derive"]
        .into_iter()
        .find(|relation| text.contains(relation))
}

fn relationship_endpoints(graph: &Graph, element: &Element) -> Option<(String, String)> {
    let sources = endpoint_values(graph, element, &["source", "sources", "source_feature"]);
    let targets = endpoint_values(
        graph,
        element,
        &["target", "targets", "target_ref", "target_feature"],
    );
    sources.into_iter().zip(targets).next()
}

fn endpoint_values(graph: &Graph, element: &Element, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| element.properties.get(*key))
        .flat_map(|value| value_reference_ids(value))
        .flat_map(|id| resolve_endpoint_reference_ids(graph, &id))
        .collect()
}

fn resolve_endpoint_reference_ids(graph: &Graph, id: &str) -> Vec<String> {
    if graph.element_by_element_id(id).is_some() {
        return vec![id.to_string()];
    }

    let Some((prefix, qualified_name)) = id.split_once('.') else {
        return Vec::new();
    };
    if prefix != "feature" {
        return Vec::new();
    }

    ["requirement", "req"]
        .into_iter()
        .map(|candidate_prefix| format!("{candidate_prefix}.{qualified_name}"))
        .filter(|candidate| graph.element_by_element_id(candidate).is_some())
        .collect()
}

fn value_reference_ids(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => values.iter().flat_map(value_reference_ids).collect(),
        Value::Object(object) => ["element_id", "id", "target", "source"]
            .iter()
            .filter_map(|key| object.get(*key))
            .flat_map(value_reference_ids)
            .collect(),
        _ => Vec::new(),
    }
}

fn is_state_machine_root_element(graph: &Graph, element: &Element) -> bool {
    kind_matches(element, &["StateDefinition", "StateUsage"])
        && graph.incoming(element.id, "owner").any(|edge| {
            graph
                .element(edge.source)
                .is_some_and(|child| kind_matches(child, &["StateDefinition", "StateUsage"]))
        })
}

fn kind_matches(element: &Element, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| {
        element.kind.as_ref() == *candidate
            || element.kind.as_ref().ends_with(&format!("::{candidate}"))
    })
}

pub fn validate_view_document(
    document: &ViewDocumentDto,
) -> Result<(), Vec<ViewValidationDiagnostic>> {
    let mut diagnostics = Vec::new();
    if document.schema != VIEW_SCHEMA {
        diagnostics.push(view_diagnostic(
            "view.schema",
            "/schema",
            format!("expected schema `{VIEW_SCHEMA}`"),
        ));
    }
    if document.version != VIEW_SPEC_VERSION {
        diagnostics.push(view_diagnostic(
            "view.version",
            "/version",
            format!("expected version {VIEW_SPEC_VERSION}"),
        ));
    }
    match (&document.diagram, &document.table, &document.model) {
        (Some(diagram), None, None) => {
            let expected = format!("diagram.{}", diagram_kind_name(&diagram.kind));
            if document.kind != expected {
                diagnostics.push(view_diagnostic(
                    "view.kind",
                    "/kind",
                    format!("expected kind `{expected}` for diagram payload"),
                ));
            }
            validate_diagram_spec(diagram, "/diagram", &mut diagnostics);
        }
        (None, Some(table), None) => {
            let expected = "table";
            if document.kind != expected {
                diagnostics.push(view_diagnostic(
                    "view.kind",
                    "/kind",
                    format!("expected kind `{expected}` for table payload"),
                ));
            }
            validate_table_spec(table, "/table", &mut diagnostics);
        }
        (None, None, Some(model)) => {
            let expected = model_view_kind_name(&model.kind);
            if document.kind != expected {
                diagnostics.push(view_diagnostic(
                    "view.kind",
                    "/kind",
                    format!("expected kind `{expected}` for model view payload"),
                ));
            }
            validate_model_view_spec(model, "/model", &mut diagnostics);
        }
        (None, None, None) => diagnostics.push(view_diagnostic(
            "view.payload",
            "/",
            "view document must contain exactly one payload".to_string(),
        )),
        _ => diagnostics.push(view_diagnostic(
            "view.payload",
            "/",
            "view document must contain exactly one payload".to_string(),
        )),
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_model_view_spec(
    spec: &ModelViewSpecDto,
    path: &str,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    if spec.version != VIEW_SPEC_VERSION {
        diagnostics.push(view_diagnostic(
            "view.spec.version",
            format!("{path}/version"),
            format!("expected spec version {VIEW_SPEC_VERSION}"),
        ));
    }
    if spec.title.trim().is_empty() {
        diagnostics.push(view_diagnostic(
            "view.spec.title",
            format!("{path}/title"),
            "title is required".to_string(),
        ));
    }
    match spec.kind {
        ModelViewKindDto::Search if spec.query.as_deref().unwrap_or_default().trim().is_empty() => {
            diagnostics.push(view_diagnostic(
                "view.model.query",
                format!("{path}/query"),
                "search view query is required".to_string(),
            ));
        }
        ModelViewKindDto::ElementDetails
        | ModelViewKindDto::ModelExplorer
        | ModelViewKindDto::MetatypeExplorer
            if spec.root.as_deref().unwrap_or_default().trim().is_empty() =>
        {
            diagnostics.push(view_diagnostic(
                "view.model.root",
                format!("{path}/root"),
                "model view root is required".to_string(),
            ));
        }
        _ => {}
    }
    if matches!(spec.kind, ModelViewKindDto::Graph) {
        let scope = spec
            .graph_scope
            .as_deref()
            .unwrap_or(GraphScope::Model.as_str());
        if !matches!(scope, "model" | "model_plus_context" | "full") {
            diagnostics.push(view_diagnostic(
                "view.model.graph_scope",
                format!("{path}/graph_scope"),
                "graph scope must be one of model, model_plus_context, full".to_string(),
            ));
        }
    }
}

fn validate_diagram_spec(
    spec: &DiagramSpecDto,
    path: &str,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    validate_common_spec(spec.version, &spec.title, &spec.query, path, diagnostics);
    if spec.layout.engine.trim().is_empty() {
        diagnostics.push(view_diagnostic(
            "view.diagram.layout.engine",
            format!("{path}/layout/engine"),
            "layout engine is required".to_string(),
        ));
    }
    let direction = spec.layout.direction.to_ascii_uppercase();
    if !matches!(direction.as_str(), "LR" | "RL" | "TB" | "BT") {
        diagnostics.push(view_diagnostic(
            "view.diagram.layout.direction",
            format!("{path}/layout/direction"),
            "layout direction must be one of LR, RL, TB, BT".to_string(),
        ));
    }
}

fn validate_table_spec(
    spec: &TableSpecDto,
    path: &str,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    validate_common_spec(spec.version, &spec.title, &spec.query, path, diagnostics);
    if let TableScopeDto::ContainmentSubtree { root } = &spec.scope {
        if root.trim().is_empty() {
            diagnostics.push(view_diagnostic(
                "view.table.scope.root",
                format!("{path}/scope/root"),
                "containment subtree scope root is required".to_string(),
            ));
        }
    }
    if let Some(row_type) = &spec.row_type {
        if row_type.type_name.trim().is_empty() {
            diagnostics.push(view_diagnostic(
                "view.table.row_type.type",
                format!("{path}/row_type/type"),
                "row type is required".to_string(),
            ));
        }
    }
    let mut column_keys = BTreeSet::new();
    for (index, column) in spec.columns.iter().enumerate() {
        if column.key.trim().is_empty() {
            diagnostics.push(view_diagnostic(
                "view.table.column.key",
                format!("{path}/columns/{index}/key"),
                "column key is required".to_string(),
            ));
        }
        if !column_keys.insert(column.key.clone()) {
            diagnostics.push(view_diagnostic(
                "view.table.column.key.duplicate",
                format!("{path}/columns/{index}/key"),
                format!("duplicate column key `{}`", column.key),
            ));
        }
        if column.label.trim().is_empty() {
            diagnostics.push(view_diagnostic(
                "view.table.column.label",
                format!("{path}/columns/{index}/label"),
                "column label is required".to_string(),
            ));
        }
        if let Some(path_value) = &column.path {
            validate_column_path(
                path_value,
                format!("{path}/columns/{index}/path"),
                diagnostics,
            );
        }
        if let Some(expression) = &column.expression {
            validate_column_expression(
                expression,
                format!("{path}/columns/{index}/expression"),
                diagnostics,
            );
        } else {
            validate_column_path(
                &column.key,
                format!("{path}/columns/{index}/key"),
                diagnostics,
            );
        }
    }
}

fn validate_column_expression(
    expression: &str,
    path: String,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    let expression = expression.trim();
    if expression.is_empty() {
        diagnostics.push(view_diagnostic(
            "view.table.column.expression",
            path,
            "column expression must not be empty".to_string(),
        ));
        return;
    }
    let navigation = expression
        .strip_prefix("row.")
        .or_else(|| expression.strip_prefix("<row_element>."))
        .or_else(|| expression.strip_prefix("self."))
        .unwrap_or(expression);
    validate_column_path(navigation, path, diagnostics);
}

fn validate_column_path(
    path_value: &str,
    path: String,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    if path_value
        .split('.')
        .any(|segment| segment.trim().is_empty())
    {
        diagnostics.push(view_diagnostic(
            "view.table.column.path",
            path,
            "column path segments must not be empty".to_string(),
        ));
    }
}

fn validate_common_spec(
    version: u8,
    title: &str,
    query: &DiagramQueryOptionsDto,
    path: &str,
    diagnostics: &mut Vec<ViewValidationDiagnostic>,
) {
    if version != VIEW_SPEC_VERSION {
        diagnostics.push(view_diagnostic(
            "view.spec.version",
            format!("{path}/version"),
            format!("expected spec version {VIEW_SPEC_VERSION}"),
        ));
    }
    if title.trim().is_empty() {
        diagnostics.push(view_diagnostic(
            "view.spec.title",
            format!("{path}/title"),
            "title is required".to_string(),
        ));
    }
    if query.max_nodes == 0 {
        diagnostics.push(view_diagnostic(
            "view.query.max_nodes",
            format!("{path}/query/max_nodes"),
            "max_nodes must be greater than zero".to_string(),
        ));
    }
    if query.max_edges == 0 {
        diagnostics.push(view_diagnostic(
            "view.query.max_edges",
            format!("{path}/query/max_edges"),
            "max_edges must be greater than zero".to_string(),
        ));
    }
}

fn view_diagnostic(
    code: &'static str,
    path: impl Into<String>,
    message: String,
) -> ViewValidationDiagnostic {
    ViewValidationDiagnostic {
        code,
        path: path.into(),
        message,
    }
}

pub fn render_table(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    mut spec: TableSpecDto,
) -> Result<TableViewDto, TableError> {
    if spec.version != 1 {
        return Err(TableError::UnsupportedVersion(spec.version));
    }

    match spec.kind.clone() {
        TableKindDto::ModelElements | TableKindDto::Elements => {
            render_elements_table(graph, metamodel_registry, &mut spec)
        }
        TableKindDto::Requirements => {
            render_requirements_table(graph, metamodel_registry, &mut spec)
        }
        TableKindDto::RelationshipMatrix => render_relationship_matrix(graph, &mut spec),
    }
}

fn render_elements_table(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: &mut TableSpecDto,
) -> Result<TableViewDto, TableError> {
    let target_type = table_row_type_name(spec);
    let available_columns =
        available_table_columns(graph, metamodel_registry, target_type.as_deref());
    let columns = if spec.columns.is_empty() {
        default_table_columns(&available_columns)
    } else {
        spec.columns.clone()
    };
    let visible_ids = table_scope_ids(graph, spec, &default_diagram_relations())?;

    let mut rows = visible_ids
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| table_target_matches(graph, element, spec, target_type.as_deref()))
        .map(|element| table_row(graph, element, &columns))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    if spec.show_affordances {
        attach_table_affordances(graph, &mut rows);
    }

    let mut warnings = Vec::new();
    if rows.is_empty() {
        warnings.push("No elements matched the requested filters.".to_string());
    }

    Ok(TableViewDto {
        spec: spec.clone(),
        columns,
        available_columns,
        rows,
        warnings,
    })
}

fn render_requirements_table(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: &mut TableSpecDto,
) -> Result<TableViewDto, TableError> {
    let target_type = table_row_type_name(spec).or_else(|| Some("Requirement".to_string()));
    let available_columns =
        available_table_columns(graph, metamodel_registry, target_type.as_deref());
    let columns = if spec.columns.is_empty() {
        default_requirements_columns()
    } else {
        spec.columns.clone()
    };
    let visible_ids = table_scope_ids(
        graph,
        spec,
        &[
            "owner".to_string(),
            "satisfy".to_string(),
            "verify".to_string(),
        ],
    )?;

    let mut rows = visible_ids
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| table_target_matches(graph, element, spec, target_type.as_deref()))
        .map(|element| table_row(graph, element, &columns))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    if spec.show_affordances {
        attach_table_affordances(graph, &mut rows);
    }

    let mut warnings = Vec::new();
    if rows.is_empty() {
        warnings.push("No requirements matched the requested filters.".to_string());
    }

    Ok(TableViewDto {
        spec: spec.clone(),
        columns,
        available_columns,
        rows,
        warnings,
    })
}

/// Column key of the matrix row-header column (the pivoted grid's first
/// column, carrying each row element's label).
pub const MATRIX_ROW_HEADER_KEY: &str = "row";

/// A resolved row/column relationship: either a direct property edge
/// (`element: None`) or a reified relationship element whose id the matrix
/// cell can cite.
struct MatrixLink {
    source: String,
    target: String,
    relation: String,
    element: Option<String>,
}

/// DA-4: render a `relationship_matrix` table — rows from `scope`+`row_type`,
/// columns from `column_scope`+`column_type` (default: mirror the rows, the
/// DSM shape), cells from the `relations` set over direct property edges and
/// reified relationship elements. Presets fill relations and axis filters
/// without overriding anything the spec sets explicitly.
fn render_relationship_matrix(
    graph: &Graph,
    spec: &mut TableSpecDto,
) -> Result<TableViewDto, TableError> {
    apply_matrix_preset(spec);
    let mut warnings = Vec::new();
    let relations = matrix_relations(spec);
    let links = collect_matrix_links(graph, &relations);
    let max_axis = effective_max_nodes(&spec.query);

    let row_filter = matrix_preset_row_filter(spec.matrix_preset.as_ref());
    let row_type = spec.row_type.clone();
    let mut rows = matrix_scope_ids(graph, &spec.scope, spec.root.as_deref())?
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| matrix_axis_matches(graph, element, row_type.as_ref(), row_filter))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.element_id.cmp(&right.element_id));
    if rows.len() > max_axis {
        warnings.push(format!(
            "Matrix row limit reached; showing {max_axis} of {} rows.",
            rows.len()
        ));
        rows.truncate(max_axis);
    }

    let mut columns = matrix_column_elements(graph, spec, &rows, &links)?;
    columns.sort_by(|left, right| left.element_id.cmp(&right.element_id));
    if columns.len() > max_axis {
        warnings.push(format!(
            "Matrix column limit reached; showing {max_axis} of {} columns.",
            columns.len()
        ));
        columns.truncate(max_axis);
    }

    let mut links_by_pair: BTreeMap<(&str, &str), Vec<&MatrixLink>> = BTreeMap::new();
    for link in &links {
        links_by_pair
            .entry((link.source.as_str(), link.target.as_str()))
            .or_default()
            .push(link);
    }

    let max_cells = effective_max_edges(&spec.query);
    let mut filled = 0usize;
    let mut cells_truncated = false;
    let mut row_dtos = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut cells = Vec::with_capacity(columns.len() + 1);
        cells.push(TableCellDto {
            key: MATRIX_ROW_HEADER_KEY.to_string(),
            value: matrix_element_label(graph, row),
            element: None,
        });
        for column in &columns {
            let mut cell_links = links_by_pair
                .get(&(row.element_id.as_str(), column.element_id.as_str()))
                .map(|links| links.clone())
                .unwrap_or_default();
            if row.element_id != column.element_id {
                if let Some(reverse) =
                    links_by_pair.get(&(column.element_id.as_str(), row.element_id.as_str()))
                {
                    cell_links.extend(reverse.iter().copied());
                }
            }
            if !cell_links.is_empty() && filled >= max_cells {
                cells_truncated = true;
                cell_links.clear();
            }
            if cell_links.is_empty() {
                cells.push(TableCellDto {
                    key: column.element_id.clone(),
                    value: String::new(),
                    element: None,
                });
                continue;
            }
            filled += 1;
            let mut relation_labels = cell_links
                .iter()
                .map(|link| link.relation.as_str())
                .collect::<Vec<_>>();
            relation_labels.sort_unstable();
            relation_labels.dedup();
            cells.push(TableCellDto {
                key: column.element_id.clone(),
                value: relation_labels.join(", "),
                element: cell_links.iter().find_map(|link| link.element.clone()),
            });
        }
        row_dtos.push(TableRowDto {
            id: row.element_id.clone(),
            element: row.element_id.clone(),
            cells,
            affordances: Vec::new(),
        });
    }
    if cells_truncated {
        warnings.push(format!(
            "Matrix cell limit reached; showing first {max_cells} filled cells."
        ));
    }
    if row_dtos.is_empty() {
        warnings.push("No matrix rows matched the requested filters.".to_string());
    }
    if columns.is_empty() {
        warnings.push("No matrix columns matched the requested filters.".to_string());
    }
    if spec.show_affordances {
        attach_table_affordances(graph, &mut row_dtos);
    }

    let mut column_specs = Vec::with_capacity(columns.len() + 1);
    column_specs.push(TableColumnSpecDto {
        key: MATRIX_ROW_HEADER_KEY.to_string(),
        label: matrix_row_axis_label(spec).to_string(),
        path: None,
        expression: None,
    });
    for column in &columns {
        column_specs.push(TableColumnSpecDto {
            key: column.element_id.clone(),
            label: matrix_element_label(graph, column),
            path: None,
            expression: None,
        });
    }

    Ok(TableViewDto {
        spec: spec.clone(),
        columns: column_specs,
        available_columns: Vec::new(),
        rows: row_dtos,
        warnings,
    })
}

/// Preset expansion: fill the relation set the preset implies. Axis element
/// filters stay code-side (`matrix_preset_row_filter`/`matrix_column_elements`)
/// so the spec keeps only what the author actually wrote.
fn apply_matrix_preset(spec: &mut TableSpecDto) {
    let Some(preset) = spec.matrix_preset.clone() else {
        return;
    };
    if spec.relations.is_empty() {
        spec.relations = match preset {
            MatrixPresetDto::Allocation => vec!["allocate".to_string()],
            MatrixPresetDto::Dsm => vec![
                "connection".to_string(),
                "flow".to_string(),
                "dependency".to_string(),
            ],
            MatrixPresetDto::RequirementsCoverage => {
                vec!["satisfy".to_string(), "verify".to_string()]
            }
        };
    }
}

fn matrix_relations(spec: &TableSpecDto) -> Vec<String> {
    if spec.relations.is_empty() {
        // No preset and no explicit relations: accept every relation family a
        // preset knows about rather than rendering an unconditionally empty
        // matrix.
        [
            "allocate",
            "connection",
            "dependency",
            "flow",
            "satisfy",
            "verify",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    } else {
        spec.relations.clone()
    }
}

fn matrix_row_axis_label(spec: &TableSpecDto) -> &'static str {
    if spec.row_type.is_some() || spec.target_type.is_some() {
        return "Element";
    }
    match spec.matrix_preset {
        Some(MatrixPresetDto::Allocation) => "Action",
        Some(MatrixPresetDto::Dsm) => "Part",
        Some(MatrixPresetDto::RequirementsCoverage) => "Requirement",
        None => "Element",
    }
}

/// Column axis resolution: explicit `column_scope`/`column_type` wins; absent
/// that, the Allocation preset targets part elements, RequirementsCoverage
/// derives columns from the relation endpoints opposite the rows, and DSM (or
/// no preset) mirrors the row axis.
fn matrix_column_elements<'a>(
    graph: &'a Graph,
    spec: &TableSpecDto,
    rows: &[&'a Element],
    links: &[MatrixLink],
) -> Result<Vec<&'a Element>, TableError> {
    if spec.column_scope.is_some() || spec.column_type.is_some() {
        let scope = spec.column_scope.clone().unwrap_or_default();
        let column_filter = matrix_preset_column_filter(spec.matrix_preset.as_ref())
            .filter(|_| spec.column_type.is_none());
        return Ok(matrix_scope_ids(graph, &scope, None)?
            .iter()
            .filter_map(|node_id| graph.element(*node_id))
            .filter(|element| include_element(element, &spec.query))
            .filter(|element| {
                matrix_axis_matches(graph, element, spec.column_type.as_ref(), column_filter)
            })
            .collect());
    }

    match spec.matrix_preset {
        Some(MatrixPresetDto::Allocation) => Ok(graph
            .elements()
            .iter()
            .filter(|element| include_element(element, &spec.query))
            .filter(|element| matrix_axis_matches(graph, element, None, Some(is_matrix_part_element)))
            .collect()),
        Some(MatrixPresetDto::RequirementsCoverage) => {
            let row_ids = rows
                .iter()
                .map(|element| element.element_id.as_str())
                .collect::<BTreeSet<_>>();
            let mut column_ids = BTreeSet::new();
            for link in links {
                if row_ids.contains(link.target.as_str()) && !row_ids.contains(link.source.as_str())
                {
                    column_ids.insert(link.source.as_str());
                }
                if row_ids.contains(link.source.as_str()) && !row_ids.contains(link.target.as_str())
                {
                    column_ids.insert(link.target.as_str());
                }
            }
            Ok(column_ids
                .iter()
                .filter_map(|element_id| graph.element_by_element_id(element_id))
                .filter(|element| include_element(element, &spec.query))
                .filter(|element| matrix_axis_matches(graph, element, None, None))
                .collect())
        }
        // DSM default: columns mirror the rows.
        _ => Ok(rows.to_vec()),
    }
}

fn matrix_axis_matches(
    graph: &Graph,
    element: &Element,
    type_filter: Option<&TableRowTypeDto>,
    preset_filter: Option<fn(&Element) -> bool>,
) -> bool {
    // Reified relationships are cells, never axis entries.
    if matrix_relationship_kind(element).is_some() {
        return false;
    }
    if let Some(type_filter) = type_filter {
        return element_matches_type(
            graph,
            element,
            &type_filter.type_name,
            type_filter.include_subtypes,
        );
    }
    preset_filter.is_none_or(|filter| filter(element))
}

fn matrix_preset_row_filter(preset: Option<&MatrixPresetDto>) -> Option<fn(&Element) -> bool> {
    match preset {
        Some(MatrixPresetDto::Allocation) => Some(is_matrix_action_element),
        Some(MatrixPresetDto::Dsm) => Some(is_matrix_part_element),
        Some(MatrixPresetDto::RequirementsCoverage) => Some(is_requirement_element),
        None => None,
    }
}

fn matrix_preset_column_filter(preset: Option<&MatrixPresetDto>) -> Option<fn(&Element) -> bool> {
    match preset {
        Some(MatrixPresetDto::Allocation) | Some(MatrixPresetDto::Dsm) => {
            Some(is_matrix_part_element)
        }
        _ => None,
    }
}

fn is_matrix_action_element(element: &Element) -> bool {
    is_activity_element(element) || kind_matches(element, &["ActionUsage", "ActionDefinition"])
}

fn is_matrix_part_element(element: &Element) -> bool {
    kind_matches(
        element,
        &[
            "PartUsage",
            "PartDefinition",
            "ItemUsage",
            "ItemDefinition",
            "BlockDefinition",
            "ComponentDefinition",
            "PortUsage",
            "PortDefinition",
        ],
    )
}

/// The canonical relation label a reified relationship element carries, or
/// `None` for elements that are not relationships.
fn matrix_relationship_kind(element: &Element) -> Option<&'static str> {
    if let Some(kind) = requirement_relationship_kind(element) {
        return Some(kind);
    }
    let text = element_semantic_text(element);
    if text.contains("allocat") {
        return Some("allocate");
    }
    if text.contains("connection") || text.contains("connector") || text.contains("interfaceusage")
    {
        return Some("connection");
    }
    if text.contains("flow") {
        return Some("flow");
    }
    if text.contains("dependency") {
        return Some("dependency");
    }
    None
}

fn matrix_relation_match(candidate: &str, relations: &[String]) -> Option<String> {
    let normalized = candidate.trim().to_ascii_lowercase();
    relations.iter().find_map(|relation| {
        let relation_normalized = relation.trim().to_ascii_lowercase();
        if !relation_normalized.is_empty() && normalized.contains(&relation_normalized) {
            Some(relation_normalized)
        } else {
            None
        }
    })
}

/// Gather every row/column relationship the relation set names: direct
/// property edges first, then reified relationship elements
/// (connection/allocation/flow/satisfy/... usages with explicit endpoints)
/// whose element id a matrix cell can cite.
fn collect_matrix_links(graph: &Graph, relations: &[String]) -> Vec<MatrixLink> {
    let mut links = Vec::new();
    for edge in graph.edges() {
        let Some(relation) = matrix_relation_match(edge.relation.as_ref(), relations) else {
            continue;
        };
        let (Some(source), Some(target)) =
            (graph.element_id(edge.source), graph.element_id(edge.target))
        else {
            continue;
        };
        links.push(MatrixLink {
            source: source.to_string(),
            target: target.to_string(),
            relation,
            element: None,
        });
    }

    for element in graph.elements() {
        let Some(kind) = matrix_relationship_kind(element) else {
            continue;
        };
        let Some(relation) = matrix_relation_match(kind, relations) else {
            continue;
        };
        let sources = endpoint_values(graph, element, &["source", "sources", "source_feature"]);
        let targets = endpoint_values(
            graph,
            element,
            &["target", "targets", "target_ref", "target_feature"],
        );
        for source in &sources {
            for target in &targets {
                links.push(MatrixLink {
                    source: source.clone(),
                    target: target.clone(),
                    relation: relation.clone(),
                    element: Some(element.element_id.clone()),
                });
            }
        }
    }
    links
}

fn matrix_scope_ids(
    graph: &Graph,
    scope: &TableScopeDto,
    root: Option<&str>,
) -> Result<BTreeSet<NodeId>, TableError> {
    match scope {
        TableScopeDto::WholeModel => {
            if let Some(root) = root.filter(|root| !root.trim().is_empty()) {
                let root = resolve_root(graph, root)
                    .ok_or_else(|| TableError::RootNotFound(root.to_string()))?;
                Ok(collect_containment_subtree_ids(graph, root.id))
            } else {
                Ok(graph.elements().iter().map(|element| element.id).collect())
            }
        }
        TableScopeDto::ContainmentSubtree { root } => {
            let root = resolve_root(graph, root)
                .ok_or_else(|| TableError::RootNotFound(root.to_string()))?;
            Ok(collect_containment_subtree_ids(graph, root.id))
        }
        TableScopeDto::ExplicitElements { elements } => Ok(elements
            .iter()
            .filter_map(|element_id| graph.element_by_element_id(element_id))
            .map(|element| element.id)
            .collect()),
    }
}

fn matrix_element_label(graph: &Graph, element: &Element) -> String {
    effective_property_text(graph, element, "declared_name")
        .or_else(|| effective_property_text(graph, element, "name"))
        .unwrap_or_else(|| label_for_id(&element.element_id))
}

fn default_elements_columns() -> Vec<TableColumnSpecDto> {
    [
        ("id", "ID"),
        ("name", "Name"),
        ("kind", "Kind"),
        ("owner", "Owner"),
        ("source_file", "Source"),
    ]
    .into_iter()
    .map(|(key, label)| TableColumnSpecDto {
        key: key.to_string(),
        label: label.to_string(),
        path: None,
        expression: None,
    })
    .collect()
}

fn default_requirements_columns() -> Vec<TableColumnSpecDto> {
    [
        ("requirement_id", "ID"),
        ("name", "Name"),
        ("text", "Text"),
        ("status", "Status"),
        ("owner", "Owner"),
    ]
    .into_iter()
    .map(|(key, label)| TableColumnSpecDto {
        key: key.to_string(),
        label: label.to_string(),
        path: None,
        expression: None,
    })
    .collect()
}

fn normalized_target_type(target_type: Option<&str>) -> Option<String> {
    target_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn table_row_type_name(spec: &TableSpecDto) -> Option<String> {
    spec.row_type
        .as_ref()
        .map(|row_type| row_type.type_name.as_str())
        .and_then(|value| normalized_target_type(Some(value)))
        .or_else(|| normalized_target_type(spec.target_type.as_deref()))
}

fn table_row_type_includes_subtypes(spec: &TableSpecDto) -> bool {
    spec.row_type
        .as_ref()
        .map(|row_type| row_type.include_subtypes)
        .unwrap_or(true)
}

fn table_scope_ids(
    graph: &Graph,
    spec: &TableSpecDto,
    legacy_root_relations: &[String],
) -> Result<BTreeSet<u32>, TableError> {
    match &spec.scope {
        TableScopeDto::WholeModel => {
            if let Some(root) = spec.root.as_deref().filter(|root| !root.trim().is_empty()) {
                let root = resolve_root(graph, root)
                    .ok_or_else(|| TableError::RootNotFound(root.to_string()))?;
                Ok(
                    collect_structure_ids(graph, root.id, &spec.query, legacy_root_relations)
                        .visible_ids,
                )
            } else {
                Ok(graph.elements().iter().map(|element| element.id).collect())
            }
        }
        TableScopeDto::ContainmentSubtree { root } => {
            let root = resolve_root(graph, root)
                .ok_or_else(|| TableError::RootNotFound(root.to_string()))?;
            Ok(collect_containment_subtree_ids(graph, root.id))
        }
        TableScopeDto::ExplicitElements { elements } => Ok(elements
            .iter()
            .filter_map(|element_id| graph.element_by_element_id(element_id))
            .map(|element| element.id)
            .collect()),
    }
}

fn collect_containment_subtree_ids(graph: &Graph, root_id: u32) -> BTreeSet<u32> {
    let mut visible = BTreeSet::new();
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
        if !visible.insert(node_id) {
            continue;
        }
        let Some(parent) = graph.element(node_id) else {
            continue;
        };
        for edge in graph.incoming(node_id, "owner") {
            stack.push(edge.source);
        }
        for child in graph.elements().iter().filter(|candidate| {
            element_contains_member(parent, candidate)
                || diagram_string_property_values(
                    candidate,
                    &[
                        "owner",
                        "owning_type",
                        "owningType",
                        "owning_namespace",
                        "owningNamespace",
                    ],
                )
                .iter()
                .any(|owner| owner == &parent.element_id)
        }) {
            stack.push(child.id);
        }
    }
    visible
}

fn element_contains_member(parent: &Element, child: &Element) -> bool {
    diagram_string_property_values(parent, &["members", "owned_members", "ownedMembers"])
        .iter()
        .any(|member| member == &child.element_id)
}

fn default_table_columns(available_columns: &[TableColumnSpecDto]) -> Vec<TableColumnSpecDto> {
    let preferred = ["id", "name", "kind", "owner", "source_file"];
    let mut columns = preferred
        .iter()
        .filter_map(|key| {
            available_columns
                .iter()
                .find(|column| column.key == *key)
                .cloned()
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        columns = default_elements_columns();
    }
    columns
}

fn available_table_columns(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    target_type: Option<&str>,
) -> Vec<TableColumnSpecDto> {
    let mut keys = BTreeSet::new();
    let mut columns = Vec::new();
    for column in default_elements_columns() {
        if keys.insert(column.key.clone()) {
            columns.push(column);
        }
    }

    if let Some(target_type) = target_type {
        for element in graph.elements() {
            if !table_type_identifier_matches(element, target_type) {
                continue;
            }
            for declaration in metamodel_registry.declared_attributes_for(&element.element_id) {
                if keys.insert(declaration.name.clone()) {
                    columns.push(TableColumnSpecDto {
                        key: declaration.name.clone(),
                        label: title_from_column_key(&declaration.name),
                        path: Some(declaration.name.clone()),
                        expression: None,
                    });
                }
            }
            let Some(query) = query_element_attributes(graph, metamodel_registry, element.id, None)
            else {
                continue;
            };
            for row in query.rows {
                if keys.insert(row.name.clone()) {
                    columns.push(TableColumnSpecDto {
                        key: row.name.clone(),
                        label: title_from_column_key(&row.name),
                        path: Some(row.name),
                        expression: None,
                    });
                }
            }
        }
    }

    columns.sort_by(|left, right| {
        column_sort_rank(&left.key)
            .cmp(&column_sort_rank(&right.key))
            .then_with(|| left.label.cmp(&right.label))
    });
    columns
}

fn column_sort_rank(key: &str) -> usize {
    match key {
        "id" => 0,
        "name" => 1,
        "kind" => 2,
        "owner" => 3,
        "source_file" => 4,
        _ => 10,
    }
}

fn title_from_column_key(key: &str) -> String {
    key.split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn table_target_matches(
    graph: &Graph,
    element: &Element,
    spec: &TableSpecDto,
    target_type: Option<&str>,
) -> bool {
    let Some(target_type) = target_type else {
        return true;
    };
    element_matches_type(
        graph,
        element,
        target_type,
        table_row_type_includes_subtypes(spec),
    )
}

fn element_matches_type(
    graph: &Graph,
    element: &Element,
    target_type: &str,
    include_subtypes: bool,
) -> bool {
    if table_type_is_element(target_type) {
        return true;
    }
    if table_type_is_requirement(target_type) && is_requirement_element(element) {
        return true;
    }
    table_type_identifier_matches(element, target_type)
        || element_metatype(graph, element.id)
            .is_some_and(|metatype| table_type_identifier_matches(metatype, target_type))
        || (include_subtypes
            && collect_specialization_ancestors(graph, element.id)
                .into_iter()
                .any(|ancestor| table_type_identifier_matches(ancestor, target_type)))
}

fn table_type_is_element(target_type: &str) -> bool {
    let normalized = canonical_table_type(target_type);
    normalized == "element" || normalized.ends_with("::element")
}

fn table_type_is_requirement(target_type: &str) -> bool {
    let normalized = canonical_table_type(target_type);
    normalized == "requirement"
        || normalized == "requirementusage"
        || normalized == "requirementdefinition"
        || normalized.ends_with("::requirement")
        || normalized.ends_with("::requirementusage")
        || normalized.ends_with("::requirementdefinition")
}

fn table_type_identifier_matches(element: &Element, target_type: &str) -> bool {
    let target = canonical_table_type(target_type);
    [element.element_id.as_str(), element.kind.as_ref()]
        .into_iter()
        .any(|candidate| {
            let candidate = canonical_table_type(candidate);
            candidate == target
                || candidate.ends_with(&format!("::{target}"))
                || (!target.contains("::") && label_for_id(&candidate).contains(&target))
        })
}

fn canonical_table_type(value: &str) -> String {
    let normalized = value
        .trim()
        .replace('.', "::")
        .replace(' ', "")
        .to_ascii_lowercase();
    normalized
        .strip_suffix("def")
        .map(|stem| format!("{stem}definition"))
        .unwrap_or(normalized)
}

fn table_row(graph: &Graph, element: &Element, columns: &[TableColumnSpecDto]) -> TableRowDto {
    TableRowDto {
        id: element.element_id.clone(),
        element: element.element_id.clone(),
        cells: columns
            .iter()
            .map(|column| TableCellDto {
                key: column.key.clone(),
                value: table_cell_value(graph, element, column),
                element: None,
            })
            .collect(),
        affordances: Vec::new(),
    }
}

fn table_cell_value(graph: &Graph, element: &Element, column: &TableColumnSpecDto) -> String {
    if let Some(expression) = column
        .expression
        .as_deref()
        .map(str::trim)
        .filter(|expression| !expression.is_empty())
    {
        return resolve_table_expression(graph, element, expression).unwrap_or_default();
    }

    let path = column.path.as_deref().unwrap_or(&column.key);
    if path.starts_with("metadata[") {
        return resolve_metadata_path(element, path).unwrap_or_default();
    }
    if path.contains('.') {
        return resolve_element_path(graph, element, path).unwrap_or_default();
    }

    match path {
        "id" | "element" => element.element_id.clone(),
        "kind" => element.kind.to_string(),
        "owner" => effective_property_text(graph, element, "owner")
            .or_else(|| owner_label(graph, element))
            .unwrap_or_default(),
        "name" => effective_property_text(graph, element, "declared_name")
            .or_else(|| effective_property_text(graph, element, "name"))
            .unwrap_or_else(|| label_for_id(&element.element_id)),
        "text" => effective_property_text(graph, element, "text")
            .or_else(|| effective_property_text(graph, element, "body"))
            .or_else(|| effective_property_text(graph, element, "doc"))
            .or_else(|| owned_documentation_text(graph, element))
            .unwrap_or_default(),
        other => effective_property_text(graph, element, other).unwrap_or_default(),
    }
}

fn resolve_table_expression(graph: &Graph, element: &Element, expression: &str) -> Option<String> {
    let path = expression
        .strip_prefix("row.")
        .or_else(|| expression.strip_prefix("<row_element>."))
        .or_else(|| expression.strip_prefix("self."))
        .unwrap_or(expression);
    if matches!(path, "row" | "<row_element>" | "self") {
        return Some(element.element_id.clone());
    }
    resolve_table_path(graph, element, path)
}

fn resolve_table_path(graph: &Graph, element: &Element, path: &str) -> Option<String> {
    if path.starts_with("metadata[") {
        return resolve_metadata_path(element, path);
    }
    if path.contains('.') {
        return resolve_element_path(graph, element, path);
    }
    Some(match path {
        "id" | "element" => element.element_id.clone(),
        "kind" => element.kind.to_string(),
        "owner" | "parent" => effective_property_text(graph, element, "owner")
            .or_else(|| owner_label(graph, element))
            .unwrap_or_default(),
        "name" => effective_property_text(graph, element, "declared_name")
            .or_else(|| effective_property_text(graph, element, "name"))
            .unwrap_or_else(|| label_for_id(&element.element_id)),
        "text" => effective_property_text(graph, element, "text")
            .or_else(|| effective_property_text(graph, element, "body"))
            .or_else(|| effective_property_text(graph, element, "doc"))
            .or_else(|| owned_documentation_text(graph, element))
            .unwrap_or_default(),
        other => effective_property_text(graph, element, other).unwrap_or_default(),
    })
}

/// A declaration's `doc /* ... */` text compiles to owned `Documentation`
/// elements (`body` + `owner`), not to a direct property on the documented
/// element, so the `text` column falls back to joining those bodies.
fn owned_documentation_text(graph: &Graph, element: &Element) -> Option<String> {
    let bodies = graph
        .elements()
        .iter()
        .filter(|candidate| kind_matches(candidate, &["Documentation"]))
        .filter(|candidate| {
            candidate
                .properties
                .get("owner")
                .and_then(Value::as_str)
                .is_some_and(|owner| owner == element.element_id)
        })
        .filter_map(|candidate| candidate.properties.get("body").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!bodies.is_empty()).then(|| bodies.join("\n"))
}

fn resolve_metadata_path(element: &Element, path: &str) -> Option<String> {
    let remainder = path.strip_prefix("metadata[")?;
    let (type_name, field_path) = remainder.split_once("].")?;
    if type_name.trim().is_empty() || field_path.trim().is_empty() {
        return None;
    }
    metadata_annotations_named(&element.properties.to_btree_map(), type_name)
        .into_iter()
        .find_map(|annotation| value_path_text(&annotation.properties, field_path))
}

fn value_path_text(value: &Value, path: &str) -> Option<String> {
    let mut current = value;
    for segment in path.split('.') {
        if segment.trim().is_empty() {
            return None;
        }
        current = current.get(segment)?;
    }
    Some(value_to_text(current))
}

fn resolve_element_path(graph: &Graph, element: &Element, path: &str) -> Option<String> {
    let mut segments = path.split('.');
    let first = segments.next()?;
    let mut current = resolve_element_reference(graph, element, first)?;
    let mut tail = segments.peekable();
    while let Some(segment) = tail.next() {
        if tail.peek().is_none() {
            return match segment {
                "id" | "element" => Some(current.element_id.clone()),
                "kind" => Some(current.kind.to_string()),
                "name" => property_text(current, "declared_name")
                    .or_else(|| property_text(current, "name"))
                    .or_else(|| Some(label_for_id(&current.element_id))),
                other => property_text(current, other),
            };
        }
        current = resolve_element_reference(graph, current, segment)?;
    }
    Some(label_for_id(&current.element_id))
}

fn resolve_element_reference<'a>(
    graph: &'a Graph,
    element: &'a Element,
    key: &str,
) -> Option<&'a Element> {
    if key == "self" {
        return Some(element);
    }
    if matches!(key, "parent" | "owner") {
        return graph
            .outgoing(element.id, "owner")
            .next()
            .and_then(|edge| graph.element(edge.target));
    }
    property_text(element, key)
        .and_then(|id| graph.element_by_element_id(&id))
        .or_else(|| {
            graph
                .outgoing(element.id, key)
                .next()
                .and_then(|edge| graph.element(edge.target))
        })
}

fn owner_label(graph: &Graph, element: &Element) -> Option<String> {
    graph
        .outgoing(element.id, "owner")
        .next()
        .and_then(|edge| graph.element(edge.target))
        .map(|owner| label_for_id(&owner.element_id))
}

fn property_text(element: &Element, key: &str) -> Option<String> {
    element.properties.get(key).map(value_to_text)
}

fn effective_property_text(graph: &Graph, element: &Element, key: &str) -> Option<String> {
    property_text(element, key).or_else(|| {
        let ancestors = collect_specialization_ancestors(graph, element.id);
        effective_properties(&ancestors, &element.properties.to_btree_map())
            .get(key)
            .map(value_to_text)
    })
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(value_to_text)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => value.to_string(),
    }
}

pub fn render_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    if spec.version != 1 {
        return Err(DiagramError::UnsupportedVersion(spec.version));
    }

    let mut view = match spec.kind.clone() {
        DiagramKindDto::Structure => render_structure_diagram(graph, metamodel_registry, spec),
        DiagramKindDto::Bdd => render_bdd_diagram(graph, metamodel_registry, spec),
        DiagramKindDto::InternalBlock => {
            render_internal_block_diagram(graph, metamodel_registry, spec)
        }
        DiagramKindDto::Requirement => render_requirement_diagram(graph, metamodel_registry, spec),
        DiagramKindDto::Activity => render_activity_diagram(graph, metamodel_registry, spec),
        DiagramKindDto::StateMachine => {
            render_state_machine_diagram(graph, metamodel_registry, spec)
        }
        _ => Err(DiagramError::UnsupportedKind(spec.kind)),
    }?;

    if view.spec.style.show_affordances {
        attach_diagram_affordances(graph, &mut view);
    }

    Ok(view)
}

fn attach_diagram_affordances(graph: &Graph, view: &mut DiagramViewDto) {
    let affordances_by_subject = catalog_affordances_by_subject(graph);
    for node in &mut view.nodes {
        node.affordances = affordances_by_subject
            .get(&node.id)
            .cloned()
            .unwrap_or_default();
    }
}

fn attach_table_affordances(graph: &Graph, rows: &mut [TableRowDto]) {
    let affordances_by_subject = catalog_affordances_by_subject(graph);
    for row in rows {
        row.affordances = affordances_by_subject
            .get(&row.element)
            .cloned()
            .unwrap_or_default();
    }
}

fn catalog_affordances_by_subject(graph: &Graph) -> BTreeMap<String, Vec<ViewAffordanceDto>> {
    let mut by_subject: BTreeMap<String, Vec<ViewAffordanceDto>> = BTreeMap::new();
    for entry in view_catalog(graph).entries {
        let Some(subject) = entry.subject.as_deref() else {
            continue;
        };
        by_subject
            .entry(subject.to_string())
            .or_default()
            .push(view_affordance_from_catalog_entry(&entry));
    }

    for affordances in by_subject.values_mut() {
        affordances.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.label.cmp(&right.label))
                .then_with(|| left.subject.cmp(&right.subject))
        });
        affordances
            .dedup_by(|left, right| left.kind == right.kind && left.subject == right.subject);
    }

    by_subject
}

fn view_affordance_from_catalog_entry(entry: &ViewCatalogEntry) -> ViewAffordanceDto {
    ViewAffordanceDto {
        kind: entry.kind.clone(),
        subject: entry.subject.clone(),
        label: entry.label.clone(),
        spec: entry.spec.clone(),
    }
}

pub fn merge_view_overlay(
    view: &DiagramViewDto,
    overlay: &ViewOverlayDto,
    frame_index: usize,
) -> DiagramViewDto {
    let mut decorated = view.clone();
    let Some(frame) = overlay
        .frames
        .iter()
        .find(|frame| frame.index == frame_index)
        .or_else(|| overlay.frames.get(frame_index))
    else {
        decorated
            .warnings
            .push(format!("Overlay frame {frame_index} was not found."));
        return decorated;
    };

    decorated
        .warnings
        .extend(frame.warnings.iter().map(|warning| {
            format!(
                "Overlay frame {}{}: {warning}",
                frame.index,
                frame
                    .time_s
                    .map(|time_s| format!(" @ {time_s:.3}s"))
                    .unwrap_or_default()
            )
        }));

    for mark in &frame.node_marks {
        let Some(node) = decorated
            .nodes
            .iter_mut()
            .find(|node| node.id == mark.element)
        else {
            decorated.warnings.push(format!(
                "Overlay node mark `{}` references unknown element `{}`.",
                mark.kind, mark.element
            ));
            continue;
        };
        push_unique_badge(node, mark.label.as_deref().unwrap_or(&mark.kind));
        node.properties
            .insert("overlay_frame".to_string(), overlay_frame_value(frame));
        push_property_array_value(
            &mut node.properties,
            "overlay_marks",
            serde_json::to_value(mark)
                .unwrap_or_else(|_| Value::String(format!("{}:{}", mark.kind, mark.element))),
        );
    }

    for value in &frame.node_values {
        let Some(node) = decorated
            .nodes
            .iter_mut()
            .find(|node| node.id == value.element)
        else {
            decorated.warnings.push(format!(
                "Overlay value `{}` references unknown element `{}`.",
                value.key, value.element
            ));
            continue;
        };
        push_unique_badge(node, &overlay_value_badge(value));
        node.properties
            .insert("overlay_frame".to_string(), overlay_frame_value(frame));
        push_property_array_value(
            &mut node.properties,
            "overlay_values",
            serde_json::to_value(value).unwrap_or_else(|_| {
                Value::String(format!("{}={}", value.key, value_to_text(&value.value)))
            }),
        );
    }

    for mark in &frame.edge_marks {
        let Some(edge) = decorated
            .edges
            .iter_mut()
            .find(|edge| edge.id == mark.element || edge.symbol == mark.element)
        else {
            decorated.warnings.push(format!(
                "Overlay edge mark `{}` references unknown edge `{}`.",
                mark.kind, mark.element
            ));
            continue;
        };
        let label = mark.label.as_deref().unwrap_or(&mark.kind);
        if !edge.label.contains(label) {
            edge.label = format!("{} [{}]", edge.label, label);
        }
    }

    decorated
}

fn push_unique_badge(node: &mut DiagramNodeDto, badge: &str) {
    let trimmed = badge.trim();
    if trimmed.is_empty() {
        return;
    }
    if !node.badges.iter().any(|existing| existing == trimmed) {
        node.badges.push(trimmed.to_string());
    }
}

fn overlay_value_badge(value: &ViewNodeValueDto) -> String {
    let label = value.label.as_deref().unwrap_or(&value.key);
    let mut badge = format!("{label}={}", value_to_text(&value.value));
    if let Some(unit) = value.unit.as_deref().filter(|unit| !unit.trim().is_empty()) {
        badge.push(' ');
        badge.push_str(unit);
    }
    badge
}

fn overlay_frame_value(frame: &ViewOverlayFrameDto) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("index".to_string(), Value::from(frame.index));
    if let Some(time_s) = frame.time_s {
        object.insert("time_s".to_string(), Value::from(time_s));
    }
    Value::Object(object)
}

fn push_property_array_value(
    properties: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Value,
) {
    match properties.get_mut(key) {
        Some(Value::Array(values)) => values.push(value),
        Some(existing) => {
            let previous = std::mem::take(existing);
            *existing = Value::Array(vec![previous, value]);
        }
        None => {
            properties.insert(key.to_string(), Value::Array(vec![value]));
        }
    }
}

fn render_bdd_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    mut spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    if spec.query.relations.is_empty() {
        spec.query.relations = vec![
            "owner".to_string(),
            "part".to_string(),
            "specializes".to_string(),
        ];
    }

    let mut view = render_structure_diagram(graph, metamodel_registry, spec)?;
    add_bdd_block_nodes(graph, metamodel_registry, &mut view);
    add_derived_bdd_edges(graph, &mut view);
    retain_bdd_display_nodes(&mut view);
    add_bdd_attribute_rows(graph, &mut view);
    normalize_bdd_edges(&mut view);
    sync_diagram_symbols(&mut view);
    Ok(view)
}

fn add_bdd_block_nodes(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    view: &mut DiagramViewDto,
) {
    let root_id = view.spec.root.as_deref();
    let root_element = root_id.and_then(|id| graph.element_by_element_id(id));
    let mut block_ids = view
        .nodes
        .iter()
        .filter(|node| bdd_node_symbol(node).0 == "block")
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    if root_element.is_some_and(is_bdd_block_definition) {
        if let Some(root_id) = root_id {
            block_ids.insert(root_id.to_string());
        }
    }

    if let Some(root_id) = root_id.filter(|_| root_element.is_some_and(is_bdd_package)) {
        block_ids.extend(
            graph
                .elements()
                .iter()
                .filter(|element| is_bdd_block_definition(element))
                .filter(|element| state_diagram_owner_id(element).as_deref() == Some(root_id))
                .map(|element| element.element_id.clone()),
        );
    }

    let mut changed = true;
    while changed {
        changed = false;
        for element in graph.elements() {
            if block_ids.contains(&element.element_id) {
                for target_id in diagram_string_property_values(
                    element,
                    &["specializes", "specialization", "generalizes"],
                ) {
                    if graph
                        .element_by_element_id(&target_id)
                        .is_some_and(is_bdd_block_definition)
                        && block_ids.insert(target_id)
                    {
                        changed = true;
                    }
                }
            }

            if !is_bdd_part_usage(element)
                || !bdd_enclosing_definition_id(graph, element)
                    .is_some_and(|owner_id| block_ids.contains(&owner_id))
            {
                continue;
            }
            for target_id in bdd_usage_definition_ids(element) {
                if graph
                    .element_by_element_id(&target_id)
                    .is_some_and(is_bdd_block_definition)
                    && block_ids.insert(target_id)
                {
                    changed = true;
                }
            }
        }
    }

    let mut existing_ids = view
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    for block_id in block_ids {
        if !existing_ids.insert(block_id.clone()) {
            continue;
        }
        if let Some(element) = graph.element_by_element_id(&block_id) {
            view.nodes
                .push(diagram_node(graph, metamodel_registry, element));
        }
    }
    view.nodes.sort_by(|left, right| left.id.cmp(&right.id));
}

fn add_derived_bdd_edges(graph: &Graph, view: &mut DiagramViewDto) {
    let retained_ids = view
        .nodes
        .iter()
        .filter(|node| bdd_node_symbol(node).0 == "block")
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    // Roll part usages up to their nearest enclosing definition: every part
    // usage owned (transitively through the definition body) by definition A
    // and typed by definition B contributes to a single A -> B composition.
    let mut compositions: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for part in graph
        .elements()
        .iter()
        .filter(|element| is_bdd_part_usage(element))
    {
        let Some(source_id) = bdd_enclosing_definition_id(graph, part)
            .filter(|owner_id| retained_ids.contains(owner_id))
        else {
            continue;
        };
        for target_id in bdd_usage_definition_ids(part) {
            if !retained_ids.contains(&target_id) {
                continue;
            }
            compositions
                .entry((source_id.clone(), target_id))
                .or_default()
                .push(bdd_usage_display(part));
        }
    }

    // The aggregated definition-level edges are authoritative for their pairs;
    // drop any usage-graph `part` edges the structure pass emitted for them.
    view.edges.retain(|edge| {
        edge.relation != "part"
            || !compositions.contains_key(&(edge.source.clone(), edge.target.clone()))
    });

    let mut existing_edge_ids = view
        .edges
        .iter()
        .map(|edge| edge.id.clone())
        .collect::<BTreeSet<_>>();
    let mut existing_semantic_edges = view
        .edges
        .iter()
        .map(|edge| {
            (
                edge.relation.clone(),
                edge.source.clone(),
                edge.target.clone(),
            )
        })
        .collect::<BTreeSet<_>>();

    for source_id in &retained_ids {
        let Some(source_element) = graph.element_by_element_id(source_id) else {
            continue;
        };
        for target_id in diagram_string_property_values(
            source_element,
            &["specializes", "specialization", "generalizes"],
        ) {
            if !retained_ids.contains(&target_id) {
                continue;
            }
            let edge_id = format!("{source_id}:specializes:{target_id}");
            if existing_edge_ids.insert(edge_id.clone())
                && existing_semantic_edges.insert((
                    "specializes".to_string(),
                    source_id.clone(),
                    target_id.clone(),
                ))
            {
                view.edges.push(DiagramEdgeDto {
                    id: edge_id,
                    symbol: symbol_id_for_edge("specializes", source_id, &target_id),
                    source: source_id.clone(),
                    target: target_id,
                    relation: "specializes".to_string(),
                    label: ":>".to_string(),
                });
            }
        }
    }

    for ((source_id, target_id), mut usages) in compositions {
        let count = usages.len();
        usages.sort();
        usages.dedup();
        let mut label = usages.join(", ");
        if count > 1 {
            label.push_str(&format!(" ({count})"));
        }
        let edge_id = format!("{source_id}:part:{target_id}");
        if !existing_edge_ids.insert(edge_id.clone())
            || !existing_semantic_edges.insert((
                "part".to_string(),
                source_id.clone(),
                target_id.clone(),
            ))
        {
            continue;
        }
        view.edges.push(DiagramEdgeDto {
            id: edge_id,
            symbol: symbol_id_for_edge("part", &source_id, &target_id),
            source: source_id,
            target: target_id,
            relation: "part".to_string(),
            label,
        });
    }
    view.edges.sort_by(|left, right| left.id.cmp(&right.id));
}

fn retain_bdd_display_nodes(view: &mut DiagramViewDto) {
    view.nodes.retain(|node| bdd_node_symbol(node).0 == "block");
    let retained_ids = view
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    view.edges.retain(|edge| {
        matches!(edge.relation.as_str(), "part" | "specializes")
            && retained_ids.contains(edge.source.as_str())
            && retained_ids.contains(edge.target.as_str())
    });
}

fn normalize_bdd_edges(view: &mut DiagramViewDto) {
    for edge in &mut view.edges {
        if edge.relation == "specializes" {
            edge.label = ":>".to_string();
        }
    }
    view.edges.sort_by(|left, right| {
        (
            left.relation.as_str(),
            left.source.as_str(),
            left.target.as_str(),
            left.label.as_str(),
        )
            .cmp(&(
                right.relation.as_str(),
                right.source.as_str(),
                right.target.as_str(),
                right.label.as_str(),
            ))
    });
    view.edges.dedup_by(|left, right| {
        left.relation == right.relation
            && left.source == right.source
            && left.target == right.target
    });
}

fn is_bdd_package(element: &Element) -> bool {
    element_semantic_text(element).contains("package")
}

fn is_bdd_block_definition(element: &Element) -> bool {
    let text = element_semantic_text(element);
    text.contains("partdefinition") || text.contains("blockdefinition")
}

fn is_bdd_part_usage(element: &Element) -> bool {
    let text = element_semantic_text(element);
    text.contains("partusage") || element.element_id.starts_with("part.")
}

fn bdd_usage_definition_ids(element: &Element) -> Vec<String> {
    diagram_string_property_values(element, &["definition", "type", "typed_by", "typedBy"])
}

/// Walk the owner chain of a usage up to the nearest enclosing block
/// definition, so usages nested anywhere inside a definition body roll up to
/// that definition. Returns `None` for usages outside any block definition
/// (or on an ownership cycle).
fn bdd_enclosing_definition_id(graph: &Graph, element: &Element) -> Option<String> {
    let mut visited = BTreeSet::new();
    let mut current = state_diagram_owner_id(element)?;
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let owner = graph.element_by_element_id(&current)?;
        if is_bdd_block_definition(owner) {
            return Some(current);
        }
        current = state_diagram_owner_id(owner)?;
    }
}

fn bdd_usage_display(element: &Element) -> String {
    let mut display = state_diagram_label(element);
    if let Some(multiplicity) = state_diagram_string_property(element, &["multiplicity"]) {
        display.push(' ');
        if multiplicity.starts_with('[') {
            display.push_str(&multiplicity);
        } else {
            display.push('[');
            display.push_str(&multiplicity);
            display.push(']');
        }
    }
    display
}

fn is_bdd_attribute_usage(element: &Element) -> bool {
    element_semantic_text(element).contains("attributeusage")
}

/// Fold attribute usages onto the attribute rows of their enclosing block
/// definition node (BDDs render attributes as compartment rows, never as
/// standalone nodes), carrying declared default values where present.
fn add_bdd_attribute_rows(graph: &Graph, view: &mut DiagramViewDto) {
    let mut rows_by_definition: BTreeMap<String, Vec<DiagramAttributeDto>> = BTreeMap::new();
    for attribute in graph
        .elements()
        .iter()
        .filter(|element| is_bdd_attribute_usage(element))
    {
        let Some(definition_id) = bdd_enclosing_definition_id(graph, attribute) else {
            continue;
        };
        rows_by_definition
            .entry(definition_id)
            .or_default()
            .push(DiagramAttributeDto {
                name: state_diagram_label(attribute),
                type_label: bdd_attribute_type_label(attribute),
            });
    }
    for node in &mut view.nodes {
        let Some(mut rows) = rows_by_definition.remove(&node.id) else {
            continue;
        };
        rows.sort_by(|left, right| left.name.cmp(&right.name));
        for row in rows {
            if !node.attributes.iter().any(|existing| existing.name == row.name) {
                node.attributes.push(row);
            }
        }
    }
}

fn bdd_attribute_type_label(element: &Element) -> Option<String> {
    let type_name = bdd_usage_definition_ids(element)
        .into_iter()
        .next()
        .map(|id| label_for_id(&id));
    let value = ["value", "declared_value", "default_value", "default"]
        .iter()
        .find_map(|key| element.properties.get(*key))
        .filter(|value| !value.is_null())
        .map(value_to_text)
        .filter(|text| !text.trim().is_empty());
    match (type_name, value) {
        (Some(type_name), Some(value)) => Some(format!("{type_name} = {value}")),
        (Some(type_name), None) => Some(type_name),
        (None, Some(value)) => Some(format!("= {value}")),
        (None, None) => None,
    }
}

fn bdd_node_symbol(node: &DiagramNodeDto) -> (String, serde_json::Map<String, Value>) {
    let text = node_semantic_text(node);
    let mut properties = serde_json::Map::new();
    let (role, shape) = if text.contains("partdefinition") || text.contains("blockdefinition") {
        ("block", "block")
    } else {
        ("element", "node")
    };
    properties.insert("shape".to_string(), Value::String(shape.to_string()));
    (role.to_string(), properties)
}

fn element_semantic_text(element: &Element) -> String {
    let mut text = element.kind.to_ascii_lowercase();
    let mut values = Vec::new();
    if let Some(value) = state_diagram_string_property(element, &["metatype"]) {
        values.push(value);
    }
    for value in [
        element
            .properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("construct"))
            .and_then(Value::as_str),
        element
            .properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("metaclass"))
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    {
        values.push(value.to_string());
    }
    for value in values {
        text.push(' ');
        text.push_str(&value.to_ascii_lowercase());
    }
    text
}

fn node_semantic_text(node: &DiagramNodeDto) -> String {
    let mut text = node.kind.to_ascii_lowercase();
    for value in [
        node.properties.get("metatype").and_then(Value::as_str),
        node.properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("construct"))
            .and_then(Value::as_str),
        node.properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("metaclass"))
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    {
        text.push(' ');
        text.push_str(&value.to_ascii_lowercase());
    }
    text
}

fn diagram_string_property_values(element: &Element, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| element.properties.get(*key))
        .flat_map(|value| match value {
            Value::String(value) => vec![value.clone()],
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn render_internal_block_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    let root = spec
        .root
        .as_deref()
        .filter(|root| !root.trim().is_empty())
        .ok_or(DiagramError::MissingRoot)?;
    let root = resolve_root(graph, root).ok_or_else(|| DiagramError::RootNotFound(root.into()))?;
    let root_id = root.element_id.clone();
    let mut node_ids = BTreeSet::from([root_id.clone()]);

    for element in graph.elements().iter().filter(|element| {
        include_element(element, &spec.query) && internal_block_owned_by(element, &root_id)
    }) {
        if internal_block_node_role(element).is_some() {
            node_ids.insert(element.element_id.clone());
        }
    }

    let mut warnings = Vec::new();
    let mut edges = Vec::new();
    let max_edges = effective_max_edges(&spec.query);
    for element in graph
        .elements()
        .iter()
        .filter(|element| include_element(element, &spec.query))
    {
        if internal_block_owned_by(element, &root_id) && is_internal_block_part_or_port(element) {
            edges.push(DiagramEdgeDto {
                id: format!("part:{}:{}", root_id, element.element_id),
                symbol: symbol_id_for_edge("part", &root_id, &element.element_id),
                source: root_id.clone(),
                target: element.element_id.clone(),
                relation: "part".to_string(),
                label: "part".to_string(),
            });
        }

        if !is_internal_block_connector(element) {
            continue;
        }
        let Some((source, target)) = relationship_endpoints(graph, element) else {
            continue;
        };
        if !node_ids.contains(&source) || !node_ids.contains(&target) {
            continue;
        }
        edges.push(DiagramEdgeDto {
            id: element.element_id.clone(),
            symbol: symbol_id_for_edge("connection", &source, &target),
            source,
            target,
            relation: "connection".to_string(),
            label: catalog_element_label(element),
        });
        if edges.len() >= max_edges {
            warnings.push(format!(
                "Internal block diagram edge limit reached; showing first {max_edges} edges."
            ));
            break;
        }
    }

    let mut nodes = node_ids
        .iter()
        .filter_map(|node_id| graph.element_by_element_id(node_id))
        .map(|element| {
            let mut node = diagram_node(graph, metamodel_registry, element);
            node.badges = internal_block_node_badges(element);
            node
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);

    let symbols = nodes
        .iter()
        .map(|node| {
            let (role, properties) = graph
                .element_by_element_id(&node.id)
                .and_then(internal_block_node_symbol)
                .unwrap_or_else(|| ("part".to_string(), serde_json::Map::new()));
            DiagramSymbolDto {
                id: node.symbol.clone(),
                element: node.id.clone(),
                role,
                source: None,
                target: None,
                relation: None,
                properties,
            }
        })
        .chain(edges.iter().map(|edge| DiagramSymbolDto {
            id: edge.symbol.clone(),
            element: edge.id.clone(),
            role: "edge".to_string(),
            source: Some(symbol_id_for_element(&edge.source)),
            target: Some(symbol_id_for_element(&edge.target)),
            relation: Some(edge.relation.clone()),
            properties: edge_symbol_properties(edge.relation.as_str()),
        }))
        .collect();

    Ok(DiagramViewDto {
        spec,
        symbols,
        nodes,
        edges,
        warnings,
    })
}

fn render_requirement_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    let scoped_ids = if let Some(root) = spec.root.as_deref().filter(|root| !root.trim().is_empty())
    {
        let root = resolve_root(graph, root)
            .ok_or_else(|| DiagramError::RootNotFound(root.to_string()))?;
        Some(collect_containment_subtree_ids(graph, root.id))
    } else {
        None
    };

    let mut node_ids = graph
        .elements()
        .iter()
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| {
            scoped_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&element.id))
        })
        .filter(|element| is_requirement_element(element))
        .map(|element| element.element_id.clone())
        .collect::<BTreeSet<_>>();

    let mut edges = Vec::new();
    let max_edges = effective_max_edges(&spec.query);
    for edge in graph.edges() {
        let Some(relation) = requirement_relation(edge.relation.as_ref()) else {
            continue;
        };
        let Some(source) = graph.element(edge.source) else {
            continue;
        };
        let Some(target) = graph.element(edge.target) else {
            continue;
        };
        if !include_element(source, &spec.query) || !include_element(target, &spec.query) {
            continue;
        }
        let in_scope = scoped_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&source.id) || ids.contains(&target.id));
        if !in_scope || (!is_requirement_element(target) && !is_requirement_element(source)) {
            continue;
        }
        node_ids.insert(source.element_id.clone());
        node_ids.insert(target.element_id.clone());
        edges.push(DiagramEdgeDto {
            id: format!("{relation}:{}:{}", source.element_id, target.element_id),
            symbol: symbol_id_for_edge(relation, &source.element_id, &target.element_id),
            source: source.element_id.clone(),
            target: target.element_id.clone(),
            relation: relation.to_string(),
            label: relation.to_string(),
        });
        if edges.len() >= max_edges {
            break;
        }
    }

    for relationship in graph
        .elements()
        .iter()
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| requirement_relationship_kind(element).is_some())
    {
        let Some((source, target)) = relationship_endpoints(graph, relationship) else {
            continue;
        };
        let Some(source_element) = graph.element_by_element_id(&source) else {
            continue;
        };
        let Some(target_element) = graph.element_by_element_id(&target) else {
            continue;
        };
        let in_scope = scoped_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&source_element.id) || ids.contains(&target_element.id));
        if !in_scope
            || (!is_requirement_element(target_element) && !is_requirement_element(source_element))
        {
            continue;
        }
        node_ids.insert(source.clone());
        node_ids.insert(target.clone());
        let relation = requirement_relationship_kind(relationship).unwrap_or("requirement");
        edges.push(DiagramEdgeDto {
            id: relationship.element_id.clone(),
            symbol: symbol_id_for_edge(relation, &source, &target),
            source,
            target,
            relation: relation.to_string(),
            label: catalog_element_label(relationship),
        });
        if edges.len() >= max_edges {
            break;
        }
    }

    let mut warnings = Vec::new();
    if node_ids.is_empty() {
        warnings.push("No requirements matched the requested filters.".to_string());
    }
    if edges.len() >= max_edges {
        warnings.push(format!(
            "Requirement diagram edge limit reached; showing first {max_edges} edges."
        ));
    }

    let mut nodes = node_ids
        .iter()
        .filter_map(|node_id| graph.element_by_element_id(node_id))
        .map(|element| {
            let mut node = diagram_node(graph, metamodel_registry, element);
            if is_requirement_element(element) {
                node.badges = vec!["requirement".to_string()];
            }
            node
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);

    let symbols = nodes
        .iter()
        .map(|node| {
            let role = graph
                .element_by_element_id(&node.id)
                .filter(|element| is_requirement_element(element))
                .map(|_| "requirement")
                .unwrap_or("element");
            let mut properties = serde_json::Map::new();
            if role == "requirement" {
                properties.insert(
                    "shape".to_string(),
                    Value::String("requirement".to_string()),
                );
            }
            DiagramSymbolDto {
                id: node.symbol.clone(),
                element: node.id.clone(),
                role: role.to_string(),
                source: None,
                target: None,
                relation: None,
                properties,
            }
        })
        .chain(edges.iter().map(|edge| DiagramSymbolDto {
            id: edge.symbol.clone(),
            element: edge.id.clone(),
            role: "edge".to_string(),
            source: Some(symbol_id_for_element(&edge.source)),
            target: Some(symbol_id_for_element(&edge.target)),
            relation: Some(edge.relation.clone()),
            properties: edge_symbol_properties(edge.relation.as_str()),
        }))
        .collect();

    Ok(DiagramViewDto {
        spec,
        symbols,
        nodes,
        edges,
        warnings,
    })
}

fn render_activity_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    mut spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    if spec.query.relations.is_empty() {
        spec.query.relations = vec![
            "owner".to_string(),
            "control_flow".to_string(),
            "object_flow".to_string(),
        ];
    }

    let mut view = render_structure_diagram(graph, metamodel_registry, spec)?;
    add_owned_activity_nodes(graph, metamodel_registry, &mut view);
    add_derived_activity_flow_edges(graph, &mut view);
    retain_activity_display_nodes(&mut view);
    if view
        .edges
        .iter()
        .any(|edge| is_activity_flow_relation(&edge.relation))
    {
        view.edges.retain(|edge| edge.relation != "owner");
    }
    apply_activity_symbol_defaults(&mut view);
    sync_diagram_symbols(&mut view);
    Ok(view)
}

fn add_owned_activity_nodes(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    view: &mut DiagramViewDto,
) {
    let mut existing_ids = view
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let activity_owner_ids = view
        .nodes
        .iter()
        .filter(|node| activity_node_symbol(node).0 == "frame")
        .map(|node| node.id.clone())
        .chain(view.spec.root.iter().cloned())
        .collect::<BTreeSet<_>>();

    for element in graph.elements().iter().filter(|element| {
        activity_owner_ids.contains(&state_diagram_owner_id(element).unwrap_or_default())
            || activity_owner_ids.contains(
                &state_diagram_string_property(element, &["owning_type", "owningType"])
                    .unwrap_or_default(),
            )
            || activity_owner_ids.contains(
                &state_diagram_string_property(element, &["owning_definition", "owningDefinition"])
                    .unwrap_or_default(),
            )
    }) {
        if existing_ids.contains(&element.element_id) {
            continue;
        }
        let node = diagram_node(graph, metamodel_registry, element);
        let role = activity_node_symbol(&node).0;
        if role == "element" || role == "frame" {
            continue;
        }
        existing_ids.insert(node.id.clone());
        view.nodes.push(node);
    }
    view.nodes.sort_by(|left, right| left.id.cmp(&right.id));
}

fn retain_activity_display_nodes(view: &mut DiagramViewDto) {
    view.nodes.retain(|node| {
        activity_node_symbol(node).0 != "element" || Some(&node.id) == view.spec.root.as_ref()
    });
    let retained_ids = view
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    view.edges.retain(|edge| {
        retained_ids.contains(edge.source.as_str()) && retained_ids.contains(edge.target.as_str())
    });
}

fn add_derived_activity_flow_edges(graph: &Graph, view: &mut DiagramViewDto) {
    let retained_ids = view
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut existing_edge_ids = view
        .edges
        .iter()
        .map(|edge| edge.id.clone())
        .collect::<BTreeSet<_>>();

    for flow in graph.elements().iter().filter(|element| {
        is_activity_succession_flow(element) || is_activity_flow_with_explicit_ends(element)
    }) {
        let Some((source, target)) = activity_flow_endpoints(graph, flow) else {
            continue;
        };
        if !retained_ids.contains(source.as_str()) || !retained_ids.contains(target.as_str()) {
            continue;
        }
        let relation = activity_flow_relation(flow);
        let edge_id = format!("{}:{}:{}:{}", flow.element_id, relation, source, target);
        if !existing_edge_ids.insert(edge_id.clone()) {
            continue;
        }
        view.edges.push(DiagramEdgeDto {
            id: edge_id,
            symbol: symbol_id_for_transition_edge(&flow.element_id),
            source,
            target,
            relation: relation.to_string(),
            label: activity_flow_label(flow),
        });
    }

    view.edges.sort_by(|left, right| left.id.cmp(&right.id));
}

fn sync_diagram_symbols(view: &mut DiagramViewDto) {
    let node_symbols = view.nodes.iter().map(|node| {
        let (role, properties) = match view.spec.kind {
            DiagramKindDto::Activity => activity_node_symbol(node),
            DiagramKindDto::Bdd => bdd_node_symbol(node),
            _ => ("element".to_string(), serde_json::Map::new()),
        };
        DiagramSymbolDto {
            id: node.symbol.clone(),
            element: node.id.clone(),
            role,
            source: None,
            target: None,
            relation: None,
            properties,
        }
    });
    let edge_symbols = view.edges.iter().map(|edge| DiagramSymbolDto {
        id: edge.symbol.clone(),
        element: edge.id.clone(),
        role: if edge.relation == "transition" {
            "transition".to_string()
        } else {
            "edge".to_string()
        },
        source: Some(symbol_id_for_element(&edge.source)),
        target: Some(symbol_id_for_element(&edge.target)),
        relation: Some(edge.relation.to_string()),
        properties: edge_symbol_properties(edge.relation.as_str()),
    });
    view.symbols = node_symbols.chain(edge_symbols).collect();
}

fn is_activity_flow_relation(relation: &str) -> bool {
    matches!(
        relation,
        "control_flow" | "object_flow" | "source" | "target" | "transition"
    )
}

fn is_activity_succession_flow(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("succession")
        || state_diagram_string_property(element, &["metatype", "type", "definition"])
            .is_some_and(|value| value.to_ascii_lowercase().contains("succession"))
}

fn is_activity_flow_with_explicit_ends(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("flow")
        && state_diagram_string_property(element, &["source", "from"]).is_some()
        && state_diagram_string_property(element, &["target", "to"]).is_some()
}

fn activity_flow_endpoints(graph: &Graph, flow: &Element) -> Option<(String, String)> {
    if let Some(source) = state_diagram_string_property(flow, &["source", "from"]) {
        let target = state_diagram_string_property(flow, &["target", "to"])?;
        return Some((
            resolve_activity_endpoint(graph, flow, &source)?,
            resolve_activity_endpoint(graph, flow, &target)?,
        ));
    }

    let mut source = None;
    let mut target = None;
    for endpoint in graph
        .elements()
        .iter()
        .filter(|element| state_diagram_owner_id(element).as_deref() == Some(&flow.element_id))
    {
        let redefined = endpoint
            .properties
            .get("redefined_features")
            .or_else(|| endpoint.properties.get("redefinedFeatures"))
            .and_then(Value::as_array)?;
        let local_name = redefined
            .iter()
            .filter_map(Value::as_str)
            .find(|value| !value.contains("::"))?;
        if redefined
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.ends_with("sourceOutput"))
        {
            source = resolve_activity_endpoint(graph, flow, local_name);
        } else if redefined
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.ends_with("targetInput"))
        {
            target = resolve_activity_endpoint(graph, flow, local_name);
        }
    }

    source.zip(target)
}

fn resolve_activity_endpoint(graph: &Graph, flow: &Element, endpoint: &str) -> Option<String> {
    let owner = state_diagram_owner_id(flow)?;
    let endpoint = endpoint.trim();
    if endpoint == owner {
        return Some(owner);
    }
    if let Some(element) = graph.elements().iter().find(|element| {
        element.element_id == endpoint
            || state_diagram_string_property(element, &["qualified_name", "qualifiedName"])
                .is_some_and(|qualified_name| qualified_name.eq_ignore_ascii_case(endpoint))
    }) {
        return Some(element.element_id.clone());
    }
    let local_endpoint = endpoint.split('.').next().unwrap_or(endpoint).trim();
    graph
        .elements()
        .iter()
        .filter(|element| state_diagram_owner_id(element).as_deref() == Some(owner.as_str()))
        .find(|element| {
            state_diagram_label(element).eq_ignore_ascii_case(local_endpoint)
                || state_diagram_string_property(element, &["qualified_name", "qualifiedName"])
                    .is_some_and(|qualified_name| {
                        qualified_name.eq_ignore_ascii_case(local_endpoint)
                    })
                || label_for_id(&element.element_id).eq_ignore_ascii_case(local_endpoint)
                || element.element_id.ends_with(&format!(".{local_endpoint}"))
        })
        .map(|element| element.element_id.clone())
}

fn activity_flow_relation(flow: &Element) -> &'static str {
    let kind = flow.kind.to_ascii_lowercase();
    if kind.contains("object")
        || state_diagram_string_property(flow, &["metatype", "type", "definition"])
            .is_some_and(|value| value.to_ascii_lowercase().contains("object"))
    {
        "object_flow"
    } else {
        "control_flow"
    }
}

fn activity_flow_label(flow: &Element) -> String {
    state_diagram_string_property(flow, &["declared_name", "name"])
        .or_else(|| {
            let parts = flow.element_id.split('.').collect::<Vec<_>>();
            parts
                .iter()
                .rev()
                .skip_while(|part| part.chars().all(|ch| ch.is_ascii_digit() || ch == '_'))
                .find(|part| !part.is_empty())
                .map(|part| (*part).to_string())
        })
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| "flow".to_string())
}

fn apply_activity_symbol_defaults(view: &mut DiagramViewDto) {
    let node_roles = view
        .nodes
        .iter()
        .map(|node| (node.symbol.clone(), activity_node_symbol(node)))
        .collect::<std::collections::BTreeMap<_, _>>();

    for symbol in &mut view.symbols {
        if symbol.role == "element" {
            if let Some((role, properties)) = node_roles.get(&symbol.id) {
                symbol.role = role.clone();
                symbol.properties = properties.clone();
            }
        } else if symbol.role == "edge" {
            let relation = symbol.relation.as_deref().unwrap_or_default();
            symbol.properties = edge_symbol_properties(relation);
        }
    }
}

fn activity_node_symbol(node: &DiagramNodeDto) -> (String, serde_json::Map<String, Value>) {
    let mut kind = node.kind.to_ascii_lowercase();
    for value in [
        node.properties.get("metatype").and_then(Value::as_str),
        node.properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("construct"))
            .and_then(Value::as_str),
        node.properties
            .get("metadata")
            .and_then(|metadata| metadata.get("lowering"))
            .and_then(|lowering| lowering.get("metaclass"))
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    {
        kind.push(' ');
        kind.push_str(&value.to_ascii_lowercase());
    }
    let mut properties = serde_json::Map::new();
    let (role, shape) = if kind.contains("activity") || kind.contains("actiondefinition") {
        ("frame", "activity_frame")
    } else if kind.contains("object") {
        ("object_node", "object_node")
    } else if kind.contains("actionusage") || kind.ends_with("::action") {
        ("action", "action")
    } else if kind.contains("decision") {
        ("decision", "decision")
    } else if kind.contains("merge") {
        ("merge", "decision")
    } else if kind.contains("initial") {
        ("initial", "initial")
    } else if kind.contains("final") {
        ("activity_final", "activity_final")
    } else if kind.contains("parameter") {
        ("parameter", "parameter")
    } else {
        ("element", "node")
    };
    properties.insert("shape".to_string(), Value::String(shape.to_string()));
    if role == "object_node" {
        properties.insert("streaming".to_string(), Value::Bool(true));
    }
    (role.to_string(), properties)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateDiagramState {
    id: String,
    label: String,
    owner_id: Option<String>,
    parent_state_id: Option<String>,
    is_initial: bool,
    is_final: bool,
    is_orthogonal: bool,
    is_history: bool,
}

fn render_state_machine_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    let mut warnings = Vec::new();
    let initial_state_ids = graph
        .elements()
        .iter()
        .filter_map(state_diagram_initial_marker_target)
        .collect::<BTreeSet<_>>();
    let state_index = graph
        .elements()
        .iter()
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| is_state_diagram_state(element))
        .map(|element| {
            let id = element.element_id.clone();
            (
                id.clone(),
                StateDiagramState {
                    id,
                    label: state_diagram_label(element),
                    owner_id: state_diagram_owner_id(element),
                    parent_state_id: state_diagram_parent_state_id(element),
                    is_initial: initial_state_ids.contains(&element.element_id)
                        || state_diagram_bool_property(element, &["is_initial", "initial"])
                        || state_diagram_string_property(
                            element,
                            &["purpose", "state_kind", "kind_role"],
                        )
                        .is_some_and(|value| value.eq_ignore_ascii_case("initial")),
                    is_final: state_diagram_bool_property(element, &["is_final", "final"])
                        || state_diagram_string_property(
                            element,
                            &["purpose", "state_kind", "kind_role"],
                        )
                        .is_some_and(|value| value.eq_ignore_ascii_case("final")),
                    is_orthogonal: state_diagram_bool_property(
                        element,
                        &["is_orthogonal", "orthogonal"],
                    ) || state_diagram_string_property(
                        element,
                        &["state_kind", "kind_role"],
                    )
                    .is_some_and(|value| value.eq_ignore_ascii_case("orthogonal")),
                    is_history: state_diagram_bool_property(element, &["is_history", "history"])
                        || state_diagram_string_property(
                            element,
                            &["purpose", "state_kind", "kind_role"],
                        )
                        .is_some_and(|value| value.eq_ignore_ascii_case("history")),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let root_id = if let Some(root) = spec.root.as_deref().filter(|root| !root.trim().is_empty()) {
        Some(
            resolve_root(graph, root)
                .ok_or_else(|| DiagramError::RootNotFound(root.to_string()))?
                .element_id
                .clone(),
        )
    } else {
        None
    };

    let mut selected_ids = state_index
        .values()
        .filter(|state| {
            root_id
                .as_deref()
                .map(|root_id| {
                    state_diagram_state_matches_root(graph, &state_index, state, root_id)
                })
                .unwrap_or(true)
        })
        .map(|state| state.id.clone())
        .collect::<Vec<_>>();
    selected_ids.sort();
    selected_ids.dedup();

    let max_nodes = effective_max_nodes(&spec.query);
    let selected_total = selected_ids.len();
    selected_ids.truncate(max_nodes);
    if selected_total > selected_ids.len() {
        warnings.push(format!(
            "State-machine diagram node limit reached; showing {} of {selected_total} states.",
            selected_ids.len()
        ));
    }
    let selected_state_ids = selected_ids.into_iter().collect::<BTreeSet<_>>();

    let mut nodes = selected_state_ids
        .iter()
        .filter_map(|state_id| {
            graph
                .element_by_element_id(state_id)
                .zip(state_index.get(state_id))
        })
        .map(|(element, state)| state_diagram_node(graph, metamodel_registry, element, state))
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));

    if nodes.is_empty() {
        warnings.push("No state-machine states matched the requested filters.".to_string());
    }

    let mut transitions_seen = 0usize;
    let mut edges = Vec::new();
    let max_edges = effective_max_edges(&spec.query);
    for transition in graph
        .elements()
        .iter()
        .filter(|element| include_element(element, &spec.query))
        .filter(|element| is_state_diagram_transition(element))
    {
        let Some(source_ref) = state_diagram_transition_source(transition) else {
            continue;
        };
        let Some(target_ref) = state_diagram_transition_target(transition) else {
            continue;
        };
        transitions_seen += 1;
        let Some(source) =
            resolve_state_diagram_reference(&source_ref, &selected_state_ids, &state_index)
        else {
            continue;
        };
        let Some(target) =
            resolve_state_diagram_reference(&target_ref, &selected_state_ids, &state_index)
        else {
            continue;
        };
        edges.push(DiagramEdgeDto {
            id: transition.element_id.clone(),
            symbol: symbol_id_for_transition_edge(&transition.element_id),
            source,
            target,
            relation: "transition".to_string(),
            label: state_diagram_transition_label(transition),
        });
        if edges.len() >= max_edges {
            warnings.push(format!(
                "State-machine diagram edge limit reached; showing first {max_edges} transitions."
            ));
            break;
        }
    }
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);
    if edges.is_empty() && transitions_seen > 0 && !nodes.is_empty() {
        warnings.push("No transitions connected the selected state nodes.".to_string());
    }

    let symbols = nodes
        .iter()
        .map(|node| {
            let mut properties = serde_json::Map::new();
            if let Some(state) = state_index.get(&node.id) {
                properties.insert("shape".to_string(), Value::String("state".to_string()));
                properties.insert("is_initial".to_string(), Value::Bool(state.is_initial));
                properties.insert("is_final".to_string(), Value::Bool(state.is_final));
                properties.insert("is_history".to_string(), Value::Bool(state.is_history));
                if let Some(parent_state_id) = &state.parent_state_id {
                    properties.insert(
                        "parent_state_id".to_string(),
                        Value::String(parent_state_id.clone()),
                    );
                }
            }
            DiagramSymbolDto {
                id: node.symbol.clone(),
                element: node.id.clone(),
                role: "state".to_string(),
                source: None,
                target: None,
                relation: None,
                properties,
            }
        })
        .chain(edges.iter().map(|edge| {
            let transition = graph.element_by_element_id(&edge.id);
            DiagramSymbolDto {
                id: edge.symbol.clone(),
                element: edge.id.clone(),
                role: "transition".to_string(),
                source: Some(symbol_id_for_element(&edge.source)),
                target: Some(symbol_id_for_element(&edge.target)),
                relation: Some(edge.relation.clone()),
                properties: transition
                    .map(state_diagram_transition_symbol_properties)
                    .unwrap_or_else(|| edge_symbol_properties("transition")),
            }
        }))
        .collect();

    Ok(DiagramViewDto {
        spec,
        symbols,
        nodes,
        edges,
        warnings,
    })
}

fn state_diagram_node(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    element: &Element,
    state: &StateDiagramState,
) -> DiagramNodeDto {
    let mut node = diagram_node(graph, metamodel_registry, element);
    node.label = state.label.clone();
    node.badges = Vec::new();
    if state.is_initial {
        node.badges.push("initial".to_string());
    }
    if state.is_final {
        node.badges.push("final".to_string());
    }
    if state.is_orthogonal {
        node.badges.push("orthogonal".to_string());
    }
    if state.is_history {
        node.badges.push("history".to_string());
    }
    if node.badges.is_empty() {
        node.badges.push("state".to_string());
    }
    node.properties
        .insert("is_initial".to_string(), Value::Bool(state.is_initial));
    node.properties
        .insert("is_final".to_string(), Value::Bool(state.is_final));
    node.properties.insert(
        "is_orthogonal".to_string(),
        Value::Bool(state.is_orthogonal),
    );
    node.properties
        .insert("is_history".to_string(), Value::Bool(state.is_history));
    if let Some(owner_id) = &state.owner_id {
        node.properties.insert(
            "state_machine_owner".to_string(),
            Value::String(owner_id.clone()),
        );
    }
    if let Some(parent_state_id) = &state.parent_state_id {
        node.properties.insert(
            "parent_state_id".to_string(),
            Value::String(parent_state_id.clone()),
        );
    }
    node
}

fn is_state_diagram_state(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("stateusage")
        || kind.contains("stateaction")
        || state_diagram_string_property(element, &["type", "definition"])
            .is_some_and(|value| value.contains("States::StateAction"))
        || state_diagram_string_property(element, &["metatype"])
            .is_some_and(|value| value.contains("StateUsage"))
}

fn is_state_diagram_transition(element: &Element) -> bool {
    let kind = element.kind.to_ascii_lowercase();
    kind.contains("transition")
        || kind.contains("succession")
        || (kind.contains("acceptaction") && state_diagram_transition_target(element).is_some())
        || (state_diagram_string_property(element, &["metatype", "type", "definition"])
            .is_some_and(|value| {
                value.contains("AcceptAction") || value.contains("SuccessionFlow")
            })
            && state_diagram_transition_target(element).is_some())
        || element.element_id.starts_with("transition.")
}

fn state_diagram_initial_marker_target(element: &Element) -> Option<String> {
    if state_diagram_string_property(element, &["source_is_initial", "sourceIsInitial"])
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        || state_diagram_bool_property(element, &["source_is_initial", "sourceIsInitial"])
    {
        return state_diagram_transition_target(element)
            .or_else(|| state_diagram_transition_source(element));
    }

    let kind = element.kind.to_ascii_lowercase();
    let initial_completion = (kind.contains("succession")
        || state_diagram_string_property(element, &["metatype", "type", "definition"])
            .is_some_and(|value| value.contains("SuccessionFlow")))
        && state_diagram_string_property(element, &["trigger_kind", "triggerKind"])
            .is_some_and(|value| value.eq_ignore_ascii_case("completion"))
        && state_diagram_transition_source(element).is_none();
    if initial_completion {
        state_diagram_transition_target(element)
    } else {
        None
    }
}

fn state_diagram_state_matches_root(
    graph: &Graph,
    state_index: &BTreeMap<String, StateDiagramState>,
    state: &StateDiagramState,
    root_id: &str,
) -> bool {
    if state.id == root_id {
        return true;
    }

    let mut seen = BTreeSet::new();
    let mut queue = state_diagram_container_ids(state)
        .into_iter()
        .collect::<VecDeque<_>>();
    while let Some(container_id) = queue.pop_front() {
        if container_id == root_id {
            return true;
        }
        if !seen.insert(container_id.clone()) {
            continue;
        }
        if let Some(container_state) = state_index.get(&container_id) {
            queue.extend(state_diagram_container_ids(container_state));
        }
        if let Some(container_element) = graph.element_by_element_id(&container_id) {
            queue.extend(state_diagram_owner_ids(container_element));
        }
    }

    false
}

fn state_diagram_container_ids(state: &StateDiagramState) -> Vec<String> {
    state
        .parent_state_id
        .iter()
        .chain(state.owner_id.iter())
        .cloned()
        .collect()
}

fn state_diagram_owner_ids(element: &Element) -> Vec<String> {
    [
        "owner",
        "owning_type",
        "owningType",
        "owning_definition",
        "owningDefinition",
        "owning_namespace",
        "owningNamespace",
    ]
    .iter()
    .filter_map(|key| state_diagram_string_property(element, &[*key]))
    .collect()
}

fn state_diagram_owner_id(element: &Element) -> Option<String> {
    state_diagram_string_property(
        element,
        &[
            "owner",
            "owning_type",
            "owningType",
            "owning_definition",
            "owningDefinition",
            "owning_namespace",
            "owningNamespace",
        ],
    )
}

fn state_diagram_parent_state_id(element: &Element) -> Option<String> {
    state_diagram_string_property(
        element,
        &[
            "parent_state",
            "parentState",
            "owning_state",
            "owningState",
            "enclosing_state",
            "enclosingState",
        ],
    )
}

fn state_diagram_transition_source(element: &Element) -> Option<String> {
    state_diagram_string_property(
        element,
        &[
            "source",
            "source_state",
            "sourceState",
            "from",
            "transition_source",
            "transitionSource",
        ],
    )
}

fn state_diagram_transition_target(element: &Element) -> Option<String> {
    state_diagram_string_property(
        element,
        &[
            "target",
            "target_state",
            "targetState",
            "to",
            "transition_target",
            "transitionTarget",
        ],
    )
}

fn resolve_state_diagram_reference(
    reference: &str,
    selected_state_ids: &BTreeSet<String>,
    state_index: &BTreeMap<String, StateDiagramState>,
) -> Option<String> {
    let reference = reference.trim();
    if selected_state_ids.contains(reference) {
        return Some(reference.to_string());
    }
    selected_state_ids.iter().find_map(|state_id| {
        let state = state_index.get(state_id)?;
        if state.label.eq_ignore_ascii_case(reference)
            || label_for_id(&state.id).eq_ignore_ascii_case(reference)
            || state.id.ends_with(&format!(".{reference}"))
            || state.id.ends_with(&format!("::{reference}"))
        {
            Some(state.id.clone())
        } else {
            None
        }
    })
}

fn state_diagram_transition_label(element: &Element) -> String {
    state_diagram_string_property(element, &["trigger", "event"])
        .or_else(|| state_diagram_string_property(element, &["trigger_kind", "triggerKind"]))
        .or_else(|| {
            element
                .properties
                .get("guard")
                .map(|guard| format!("[{}]", value_to_text(guard)))
        })
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| label_for_id(&element.element_id))
}

fn state_diagram_transition_symbol_properties(element: &Element) -> serde_json::Map<String, Value> {
    let mut properties = edge_symbol_properties("transition");
    properties.insert("shape".to_string(), Value::String("transition".to_string()));
    if let Some(trigger) = state_diagram_string_property(element, &["trigger", "event"]) {
        properties.insert("trigger".to_string(), Value::String(trigger));
    }
    if let Some(trigger_kind) =
        state_diagram_string_property(element, &["trigger_kind", "triggerKind"])
    {
        properties.insert("trigger_kind".to_string(), Value::String(trigger_kind));
    }
    if let Some(guard) = element.properties.get("guard") {
        properties.insert("guard".to_string(), guard.clone());
    }
    if let Some(effect) =
        state_diagram_string_property(element, &["effect", "effect_action", "effectAction"])
    {
        properties.insert("effect".to_string(), Value::String(effect));
    }
    properties
}

fn state_diagram_label(element: &Element) -> String {
    state_diagram_string_property(
        element,
        &[
            "declared_name",
            "declaredName",
            "name",
            "qualified_name",
            "qualifiedName",
        ],
    )
    .map(|label| {
        label
            .rsplit(['.', ':'])
            .find(|part| !part.is_empty())
            .unwrap_or(&label)
            .to_string()
    })
    .unwrap_or_else(|| label_for_id(&element.element_id))
}

fn state_diagram_string_property(element: &Element, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        element
            .properties
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn state_diagram_bool_property(element: &Element, keys: &[&str]) -> bool {
    keys.iter().any(|key| match element.properties.get(*key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
    })
}

fn symbol_id_for_transition_edge(id: &str) -> String {
    format!("symbol.transition.{}", sanitize_symbol_segment(id))
}

fn render_structure_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    mut spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError> {
    let total_start = Instant::now();
    let mut timings = Vec::new();
    let mut warnings = Vec::new();

    let relation_start = Instant::now();
    let relations = if spec.query.relations.is_empty() {
        default_diagram_relations()
    } else {
        spec.query.relations.clone()
    };
    timings.push(("relations", relation_start.elapsed()));

    let traversal_start = Instant::now();
    let requested_root = spec
        .root
        .as_deref()
        .map(str::trim)
        .filter(|root| !root.is_empty())
        .map(str::to_string);
    let traversal = if let Some(root_query) = requested_root {
        let root_start = Instant::now();
        let root = resolve_root(graph, &root_query)
            .ok_or_else(|| DiagramError::RootNotFound(root_query.clone()))?;
        timings.push(("root", root_start.elapsed()));
        let root_node_id = root.id;
        // Canonicalize the requested root so kind-specific passes that look
        // the root up again by exact element id agree with the fuzzy
        // resolution used for the traversal.
        spec.root = Some(root.element_id.clone());
        collect_structure_ids(graph, root_node_id, &spec.query, &relations)
    } else {
        collect_unrooted_structure_ids(graph, &spec.query)
    };
    timings.push(("traversal", traversal_start.elapsed()));
    warnings.extend(traversal.warnings);

    let node_start = Instant::now();
    let mut nodes = traversal
        .visible_ids
        .iter()
        .filter_map(|node_id| graph.element(*node_id))
        .filter(|element| include_element(element, &spec.query))
        .take(effective_max_nodes(&spec.query))
        .map(|element| diagram_node(graph, metamodel_registry, element))
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    timings.push(("nodes", node_start.elapsed()));

    if nodes.is_empty() {
        warnings.push("No diagram nodes matched the requested filters.".to_string());
    }
    if traversal.visible_ids.len() > nodes.len() {
        warnings.push(format!(
            "Diagram node limit reached; showing {} of {} traversed nodes.",
            nodes.len(),
            traversal.visible_ids.len()
        ));
    }

    let edge_start = Instant::now();
    let retained_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut edges = Vec::new();
    let max_edges = effective_max_edges(&spec.query);
    'node_edges: for node_id in &traversal.visible_ids {
        for edge in graph.outgoing_edges(*node_id) {
            if !relations
                .iter()
                .any(|relation| relation.as_str() == edge.relation.as_ref())
            {
                continue;
            }
            let Some(source) = graph.element_id(edge.source) else {
                continue;
            };
            let Some(target) = graph.element_id(edge.target) else {
                continue;
            };
            if retained_ids.contains(source) && retained_ids.contains(target) {
                edges.push(DiagramEdgeDto {
                    id: format!("{}:{}:{}", edge.relation, source, target),
                    symbol: symbol_id_for_edge(&edge.relation, source, target),
                    source: source.to_string(),
                    target: target.to_string(),
                    relation: edge.relation.to_string(),
                    label: edge.relation.to_string(),
                });
                if edges.len() >= max_edges {
                    warnings.push(format!(
                        "Diagram edge limit reached; showing first {max_edges} matching edges."
                    ));
                    break 'node_edges;
                }
            }
        }
    }
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);
    timings.push(("edges", edge_start.elapsed()));

    timings.push(("total", total_start.elapsed()));
    let slow_phases = timings
        .iter()
        .filter(|(_, elapsed)| elapsed.as_millis() >= TIMING_WARNING_THRESHOLD_MS)
        .map(|(phase, elapsed)| format!("{phase}={}ms", elapsed.as_millis()))
        .collect::<Vec<_>>();
    if !slow_phases.is_empty() {
        warnings.push(format!(
            "Diagram render timing: {}.",
            slow_phases.join(", ")
        ));
    }

    Ok(DiagramViewDto {
        spec,
        symbols: nodes
            .iter()
            .map(|node| DiagramSymbolDto {
                id: node.symbol.clone(),
                element: node.id.clone(),
                role: "element".to_string(),
                source: None,
                target: None,
                relation: None,
                properties: serde_json::Map::new(),
            })
            .chain(edges.iter().map(|edge| DiagramSymbolDto {
                id: edge.symbol.clone(),
                element: edge.id.clone(),
                role: "edge".to_string(),
                source: Some(symbol_id_for_element(&edge.source)),
                target: Some(symbol_id_for_element(&edge.target)),
                relation: Some(edge.relation.to_string()),
                properties: edge_symbol_properties(edge.relation.as_str()),
            }))
            .collect(),
        nodes,
        edges,
        warnings,
    })
}

struct StructureTraversal {
    visible_ids: BTreeSet<NodeId>,
    warnings: Vec<String>,
}

fn collect_structure_ids(
    graph: &Graph,
    root_id: NodeId,
    query: &DiagramQueryOptionsDto,
    relations: &[String],
) -> StructureTraversal {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([(root_id, 0usize)]);
    let mut warnings = Vec::new();
    let max_depth = query.depth.min(DEFAULT_MAX_DEPTH);
    let max_nodes = effective_max_nodes(query);
    if query.depth > max_depth {
        warnings.push(format!(
            "Diagram depth limit reached; requested depth {} capped at {max_depth}.",
            query.depth
        ));
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if !visited.insert(node_id) {
            continue;
        }
        if visited.len() >= max_nodes {
            warnings.push(format!(
                "Diagram traversal node limit reached at {max_nodes} nodes."
            ));
            break;
        }
        if depth >= max_depth {
            continue;
        }

        if matches!(
            query.direction,
            DiagramDirectionDto::Parents | DiagramDirectionDto::Both
        ) {
            for relation in relations {
                let adjacent = parent_node_ids(graph, node_id, relation).collect::<Vec<_>>();
                for adjacent_id in adjacent.iter().take(MAX_RELATION_FANOUT_PER_NODE) {
                    queue.push_back((*adjacent_id, depth + 1));
                }
                if adjacent.len() > MAX_RELATION_FANOUT_PER_NODE {
                    warnings.push(format!(
                        "Diagram relation fan-out limit reached for `{relation}`."
                    ));
                }
            }
        }

        if matches!(
            query.direction,
            DiagramDirectionDto::Children | DiagramDirectionDto::Both
        ) {
            for relation in relations {
                let adjacent = child_node_ids(graph, node_id, relation).collect::<Vec<_>>();
                for adjacent_id in adjacent.iter().take(MAX_RELATION_FANOUT_PER_NODE) {
                    queue.push_back((*adjacent_id, depth + 1));
                }
                if adjacent.len() > MAX_RELATION_FANOUT_PER_NODE {
                    warnings.push(format!(
                        "Diagram relation fan-out limit reached for incoming `{relation}`."
                    ));
                }
            }
        }
    }

    StructureTraversal {
        visible_ids: visited,
        warnings,
    }
}

fn parent_node_ids<'a>(
    graph: &'a Graph,
    node_id: NodeId,
    relation: &'a str,
) -> Box<dyn Iterator<Item = NodeId> + 'a> {
    if relation == "part" {
        Box::new(graph.incoming(node_id, relation).map(|edge| edge.source))
    } else {
        Box::new(graph.outgoing(node_id, relation).map(|edge| edge.target))
    }
}

fn child_node_ids<'a>(
    graph: &'a Graph,
    node_id: NodeId,
    relation: &'a str,
) -> Box<dyn Iterator<Item = NodeId> + 'a> {
    if relation == "part" {
        Box::new(graph.outgoing(node_id, relation).map(|edge| edge.target))
    } else {
        Box::new(graph.incoming(node_id, relation).map(|edge| edge.source))
    }
}

fn collect_unrooted_structure_ids(
    graph: &Graph,
    query: &DiagramQueryOptionsDto,
) -> StructureTraversal {
    let max_nodes = effective_max_nodes(query);
    let matching_elements = graph
        .elements()
        .iter()
        .filter(|element| include_element(element, query))
        .collect::<Vec<_>>();
    // Unrooted diagrams describe the authored model, so seed from user-layer
    // top-level packages when any exist: seeding from every top-level package
    // lets a large library base (e.g. a prewarmed stdlib graph) exhaust the
    // node budget before the first user element is reached.
    let top_level_packages = matching_elements
        .iter()
        .copied()
        .filter(|element| is_top_level_package(graph, element))
        .collect::<Vec<_>>();
    let user_packages = top_level_packages
        .iter()
        .copied()
        .filter(|element| element.layer >= 2)
        .collect::<Vec<_>>();
    let seeds = if user_packages.is_empty() {
        &top_level_packages
    } else {
        &user_packages
    };
    let mut visible_ids = seeds
        .iter()
        .take(max_nodes)
        .map(|element| element.id)
        .collect::<BTreeSet<_>>();
    if !visible_ids.is_empty() {
        collect_owned_descendant_ids(graph, query, &mut visible_ids, max_nodes);
    }
    if visible_ids.is_empty() {
        visible_ids = matching_elements
            .iter()
            .copied()
            .filter(|element| element.layer >= 2)
            .chain(
                matching_elements
                    .iter()
                    .copied()
                    .filter(|element| element.layer < 2),
            )
            .take(max_nodes)
            .map(|element| element.id)
            .collect::<BTreeSet<_>>();
    }
    let mut warnings = Vec::new();
    if matching_elements.len() > visible_ids.len() {
        warnings.push(format!(
            "Diagram node limit reached; showing first {} matching nodes.",
            visible_ids.len()
        ));
    }

    StructureTraversal {
        visible_ids,
        warnings,
    }
}

fn collect_owned_descendant_ids(
    graph: &Graph,
    query: &DiagramQueryOptionsDto,
    visible_ids: &mut BTreeSet<NodeId>,
    max_nodes: usize,
) {
    let max_depth = query.depth.min(DEFAULT_MAX_DEPTH);
    let ownership_relations = ["owner", "ownedElement", "ownedMember"];
    let mut queue = visible_ids
        .iter()
        .copied()
        .map(|node_id| (node_id, 0usize))
        .collect::<VecDeque<_>>();

    while let Some((node_id, depth)) = queue.pop_front() {
        if visible_ids.len() >= max_nodes || depth >= max_depth {
            continue;
        }

        for relation in ownership_relations {
            for edge in graph
                .incoming(node_id, relation)
                .take(MAX_RELATION_FANOUT_PER_NODE)
            {
                let child_id = edge.source;
                if visible_ids.len() >= max_nodes {
                    return;
                }
                if graph
                    .element(child_id)
                    .is_some_and(|element| include_element(element, query))
                    && visible_ids.insert(child_id)
                {
                    queue.push_back((child_id, depth + 1));
                }
            }
            for edge in graph
                .outgoing(node_id, relation)
                .take(MAX_RELATION_FANOUT_PER_NODE)
            {
                let child_id = edge.target;
                if visible_ids.len() >= max_nodes {
                    return;
                }
                if graph
                    .element(child_id)
                    .is_some_and(|element| include_element(element, query))
                    && visible_ids.insert(child_id)
                {
                    queue.push_back((child_id, depth + 1));
                }
            }
        }
    }
}

fn is_top_level_package(graph: &Graph, element: &Element) -> bool {
    if !element.kind.to_ascii_lowercase().contains("package") {
        return false;
    }
    owner_ids(element).all(|owner| graph.element_by_element_id(owner).is_none())
}

fn owner_ids(element: &Element) -> impl Iterator<Item = &str> {
    element
        .properties
        .get("owner")
        .into_iter()
        .flat_map(|value| match value {
            Value::String(owner) => vec![owner.as_str()],
            Value::Array(values) => values
                .iter()
                .filter_map(|entry| entry.as_str())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
}

fn resolve_root<'a>(graph: &'a Graph, root: &str) -> Option<&'a Element> {
    if let Some(element) = graph.element_by_element_id(root) {
        return Some(element);
    }

    let normalized_root = root.trim().to_ascii_lowercase();
    // Qualified names ("RoverParts::Wheel") address dotted element ids whose
    // leading segment is a sort tag ("type.RoverParts.Wheel"), so compare the
    // tag-stripped id in qualified spelling as well.
    let qualified_root = normalized_root.replace("::", ".");
    graph
        .elements()
        .iter()
        .find(|element| {
            element
                .element_id
                .split_once('.')
                .is_some_and(|(_, path)| path.eq_ignore_ascii_case(&qualified_root))
        })
        .or_else(|| {
            graph.elements().iter().find(|element| {
                label_for_id(&element.element_id).to_ascii_lowercase() == normalized_root
                    || element
                        .element_id
                        .rsplit("::")
                        .next()
                        .is_some_and(|name| name.eq_ignore_ascii_case(root))
                    || element
                        .element_id
                        .rsplit('.')
                        .next()
                        .is_some_and(|name| name.eq_ignore_ascii_case(root))
            })
        })
}

fn include_element(element: &Element, query: &DiagramQueryOptionsDto) -> bool {
    if element.layer < 2 {
        return query.include_libraries;
    }

    query.include_user_model
}

fn diagram_node(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    element: &Element,
) -> DiagramNodeDto {
    let attributes = query_element_attributes(graph, metamodel_registry, element.id, None)
        .map(|query| query.rows)
        .unwrap_or_default()
        .into_iter()
        .map(|attribute| DiagramAttributeDto {
            name: attribute.name,
            type_label: attribute
                .effective_value
                .as_ref()
                .map(|value| value_type_label(value).to_string()),
        })
        .collect();

    DiagramNodeDto {
        id: element.element_id.clone(),
        symbol: symbol_id_for_element(&element.element_id),
        label: label_for_id(&element.element_id),
        kind: element.kind.to_string(),
        layer: element.layer,
        badges: vec![format!("L{}", element.layer)],
        attributes,
        properties: element
            .properties
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
        affordances: Vec::new(),
    }
}

fn symbol_id_for_element(id: &str) -> String {
    format!(
        "symbol.{}",
        id.chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>()
    )
}

fn symbol_id_for_edge(relation: &str, source: &str, target: &str) -> String {
    format!(
        "symbol.edge.{}.{}.{}",
        sanitize_symbol_segment(relation),
        sanitize_symbol_segment(source),
        sanitize_symbol_segment(target)
    )
}

fn sanitize_symbol_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn edge_symbol_properties(relation: &str) -> serde_json::Map<String, Value> {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "route".to_string(),
        Value::String(default_route(relation).to_string()),
    );
    properties.insert(
        "source_decoration".to_string(),
        Value::String(default_source_decoration(relation).to_string()),
    );
    properties.insert(
        "target_decoration".to_string(),
        Value::String(default_target_decoration(relation).to_string()),
    );
    properties.insert(
        "label_placement".to_string(),
        Value::String("above".to_string()),
    );
    properties
}

fn default_route(relation: &str) -> &'static str {
    match relation {
        "part" | "control_flow" | "object_flow" => "orthogonal",
        "source" | "target" | "transition" => "straight",
        _ => "straight",
    }
}

fn default_source_decoration(relation: &str) -> &'static str {
    match relation {
        "part" => "filled_diamond",
        _ => "none",
    }
}

fn default_target_decoration(relation: &str) -> &'static str {
    match relation {
        "specializes" => "hollow_triangle",
        "part" | "source" | "target" | "transition" | "control_flow" | "object_flow" => {
            "open_arrow"
        }
        _ => "open_arrow",
    }
}

fn label_for_id(id: &str) -> String {
    id.rsplit("::")
        .next()
        .and_then(|segment| segment.rsplit('.').next())
        .filter(|segment| !segment.is_empty())
        .unwrap_or(id)
        .to_string()
}

fn default_diagram_relations() -> Vec<String> {
    vec!["specializes".to_string()]
}

fn default_diagram_depth() -> usize {
    3
}

fn default_max_nodes() -> usize {
    DEFAULT_MAX_NODES
}

fn default_max_edges() -> usize {
    DEFAULT_MAX_EDGES
}

fn effective_max_nodes(query: &DiagramQueryOptionsDto) -> usize {
    query.max_nodes.clamp(1, DEFAULT_MAX_NODES)
}

fn effective_max_edges(query: &DiagramQueryOptionsDto) -> usize {
    query.max_edges.clamp(1, DEFAULT_MAX_EDGES)
}

fn default_layout_engine() -> String {
    "dagre".to_string()
}

fn default_layout_direction() -> String {
    "LR".to_string()
}

/// Render a diagram view DTO to a deterministic, lossless SVG artifact.
///
/// This is intentionally a small built-in renderer for harnesses, exports, and
/// smoke tests. Product surfaces can still apply richer interactive layout, but
/// they should start from the same `DiagramViewDto`.
pub fn render_diagram_svg(view: &DiagramViewDto) -> String {
    let node_width = 230usize;
    let node_height = 74usize;
    let gap_x = 86usize;
    let gap_y = 62usize;
    let margin = 34usize;
    let title_height = 54usize;
    let auto_layout = svg_auto_layout_positions(
        view,
        node_width,
        node_height,
        gap_x,
        gap_y,
        margin,
        title_height,
    );
    let title_width = view.spec.title.chars().count() * 9 + margin * 2;
    let width = auto_layout.width.max(title_width);
    let height = auto_layout.height;
    let positions = auto_layout.positions;
    let symbols_by_id = view
        .symbols
        .iter()
        .map(|symbol| (symbol.id.as_str(), symbol))
        .collect::<BTreeMap<_, _>>();

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-label="{}">
<rect width="100%" height="100%" fill="#f8fafc"/>
<text x="{margin}" y="34" font-family="Segoe UI, Arial, sans-serif" font-size="20" font-weight="700" fill="#0f172a">{}</text>
"##,
        svg_escape(&view.spec.title),
        svg_escape(&view.spec.title)
    );

    let mut rendered_edges = Vec::new();
    for (edge_index, edge) in view.edges.iter().enumerate() {
        let Some((source_x, source_y)) = positions.get(edge.source.as_str()) else {
            continue;
        };
        let Some((target_x, target_y)) = positions.get(edge.target.as_str()) else {
            continue;
        };
        let source_center = (source_x + node_width / 2, source_y + node_height / 2);
        let target_center = (target_x + node_width / 2, target_y + node_height / 2);
        let (x1, y1) = svg_rectangle_boundary_point(
            source_center,
            target_center,
            node_width / 2,
            node_height / 2,
        );
        let (x2, y2) = svg_rectangle_boundary_point(
            target_center,
            source_center,
            node_width / 2,
            node_height / 2,
        );
        let symbol = symbols_by_id.get(edge.symbol.as_str()).copied();
        let route = svg_symbol_property(symbol, "route")
            .unwrap_or_else(|| default_route(edge.relation.as_str()).to_string());
        let path = svg_routed_path(&route, x1, y1, x2, y2);
        rendered_edges.push((edge.relation.clone(), edge.symbol.clone(), x1, y1, x2, y2));
        svg.push_str(&format!(
            r##"<path d="{}" fill="none" stroke="#334155" stroke-width="1.8"/>
<text x="{}" y="{}" font-family="Segoe UI, Arial, sans-serif" font-size="12" font-weight="600" fill="#334155">{}</text>
"##,
            path,
            (x1 + x2) / 2 + 8,
            (y1 + y2) / 2 - 14 + ((edge_index % 4) as isize * 12),
            svg_escape(&edge.label)
        ));
    }

    for node in &view.nodes {
        let Some((x, y)) = positions.get(node.id.as_str()) else {
            continue;
        };
        let symbol = symbols_by_id.get(node.symbol.as_str()).copied();
        let role = symbol
            .map(|symbol| symbol.role.as_str())
            .unwrap_or("element");
        let shape = svg_symbol_property(symbol, "shape").unwrap_or_else(|| "node".to_string());
        svg.push_str(&svg_node_shape(
            role,
            &shape,
            *x,
            *y,
            node_width,
            node_height,
        ));
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Segoe UI, Arial, sans-serif" font-size="15" font-weight="700" fill="#0f172a">{}</text>
<text x="{}" y="{}" font-family="Segoe UI, Arial, sans-serif" font-size="11" fill="#475569">{}</text>
<text x="{}" y="{}" font-family="Segoe UI, Arial, sans-serif" font-size="10" fill="#64748b">{}</text>
"##,
            x + 14,
            y + 26,
            svg_truncate(&node.label, 25),
            x + 14,
            y + 48,
            svg_truncate(&node.kind, 31),
            x + 14,
            y + 64,
            svg_escape(&node.badges.join(" "))
        ));
    }

    for (relation, symbol_id, x1, y1, x2, y2) in rendered_edges {
        let symbol = symbols_by_id.get(symbol_id.as_str()).copied();
        let target_decoration = svg_symbol_property(symbol, "target_decoration")
            .unwrap_or_else(|| default_target_decoration(relation.as_str()).to_string());
        let source_decoration = svg_symbol_property(symbol, "source_decoration")
            .unwrap_or_else(|| default_source_decoration(relation.as_str()).to_string());
        svg.push_str(&svg_target_decoration(&target_decoration, x1, y1, x2, y2));
        svg.push_str(&svg_source_decoration(&source_decoration, x1, y1, x2, y2));
    }

    svg.push_str("</svg>\n");
    svg
}

struct SvgAutoLayout {
    positions: BTreeMap<String, (usize, usize)>,
    width: usize,
    height: usize,
}

fn svg_auto_layout_positions(
    view: &DiagramViewDto,
    node_width: usize,
    node_height: usize,
    gap_x: usize,
    gap_y: usize,
    margin: usize,
    title_height: usize,
) -> SvgAutoLayout {
    if matches!(view.spec.kind, DiagramKindDto::StateMachine) && !view.nodes.is_empty() {
        return svg_state_machine_layout_positions(
            view,
            node_width,
            node_height,
            gap_x,
            gap_y,
            margin,
            title_height,
        );
    }

    let levels = svg_layout_levels(view);
    let mut by_level = BTreeMap::<usize, Vec<String>>::new();
    for node in &view.nodes {
        by_level
            .entry(*levels.get(&node.id).unwrap_or(&0))
            .or_default()
            .push(node.id.clone());
    }
    for ids in by_level.values_mut() {
        ids.sort_by_key(|id| {
            view.nodes
                .iter()
                .find(|node| node.id == *id)
                .map(|node| (node.kind.clone(), node.label.clone(), node.id.clone()))
        });
    }

    let direction = view.spec.layout.direction.to_ascii_uppercase();
    let horizontal = direction != "TB" && direction != "BT";
    let max_lanes = by_level.values().map(Vec::len).max().unwrap_or(1);
    let level_count = by_level.len().max(1);
    let mut positions = BTreeMap::new();

    for (level, ids) in &by_level {
        for (lane, id) in ids.iter().enumerate() {
            let logical_level = if direction == "BT" || direction == "RL" {
                level_count.saturating_sub(1).saturating_sub(*level)
            } else {
                *level
            };
            let (x, y) = if horizontal {
                (
                    margin + logical_level * (node_width + gap_x),
                    title_height + margin + lane * (node_height + gap_y),
                )
            } else {
                (
                    margin + lane * (node_width + gap_x),
                    title_height + margin + logical_level * (node_height + gap_y),
                )
            };
            positions.insert(id.clone(), (x, y));
        }
    }

    let width = if horizontal {
        margin * 2 + level_count * node_width + level_count.saturating_sub(1) * gap_x
    } else {
        margin * 2 + max_lanes * node_width + max_lanes.saturating_sub(1) * gap_x
    };
    let height = if horizontal {
        title_height + margin * 2 + max_lanes * node_height + max_lanes.saturating_sub(1) * gap_y
    } else {
        title_height
            + margin * 2
            + level_count * node_height
            + level_count.saturating_sub(1) * gap_y
    };

    SvgAutoLayout {
        positions,
        width,
        height,
    }
}

fn svg_state_machine_layout_positions(
    view: &DiagramViewDto,
    node_width: usize,
    node_height: usize,
    gap_x: usize,
    gap_y: usize,
    margin: usize,
    title_height: usize,
) -> SvgAutoLayout {
    let ordered_ids = svg_state_machine_node_order(view);
    let node_count = ordered_ids.len().max(1);
    let columns = if node_count == 1 {
        1
    } else {
        node_count.min(3)
    };
    let rows = node_count.div_ceil(columns);
    let mut positions = BTreeMap::new();

    for (index, id) in ordered_ids.into_iter().enumerate() {
        let row = index / columns;
        let column = index % columns;
        positions.insert(
            id,
            (
                margin + column * (node_width + gap_x),
                title_height + margin + row * (node_height + gap_y),
            ),
        );
    }

    SvgAutoLayout {
        positions,
        width: margin * 2 + columns * node_width + columns.saturating_sub(1) * gap_x,
        height: title_height + margin * 2 + rows * node_height + rows.saturating_sub(1) * gap_y,
    }
}

fn svg_state_machine_node_order(view: &DiagramViewDto) -> Vec<String> {
    let node_ids = view
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let node_sort_key = |id: &String| {
        view.nodes
            .iter()
            .find(|node| node.id == *id)
            .map(|node| (node.label.clone(), node.id.clone()))
            .unwrap_or_else(|| (id.clone(), id.clone()))
    };
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for edge in &view.edges {
        if node_ids.contains(&edge.source) && node_ids.contains(&edge.target) {
            outgoing
                .entry(edge.source.clone())
                .or_default()
                .push(edge.target.clone());
        }
    }
    for targets in outgoing.values_mut() {
        targets.sort_by_key(&node_sort_key);
    }

    let mut starts = view
        .nodes
        .iter()
        .filter(|node| node.badges.iter().any(|badge| badge == "initial"))
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    if starts.is_empty() {
        starts = node_ids.iter().cloned().collect();
    }
    starts.sort_by_key(&node_sort_key);

    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from(starts);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        ordered.push(id.clone());
        for target in outgoing.get(&id).into_iter().flatten() {
            if !seen.contains(target) {
                queue.push_back(target.clone());
            }
        }
    }

    let mut remaining = node_ids
        .into_iter()
        .filter(|id| !seen.contains(id))
        .collect::<Vec<_>>();
    remaining.sort_by_key(&node_sort_key);
    ordered.extend(remaining);
    ordered
}

fn svg_layout_levels(view: &DiagramViewDto) -> BTreeMap<String, usize> {
    let node_ids = view
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let mut children_by_parent = BTreeMap::<String, Vec<String>>::new();
    let mut child_ids = BTreeSet::new();

    for edge in &view.edges {
        if !node_ids.contains(&edge.source) || !node_ids.contains(&edge.target) {
            continue;
        }
        children_by_parent
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        child_ids.insert(edge.target.clone());
    }

    let mut roots = node_ids
        .difference(&child_ids)
        .cloned()
        .collect::<Vec<String>>();
    if roots.is_empty() {
        roots = node_ids.iter().cloned().collect();
    }
    roots.sort();

    let mut levels = BTreeMap::new();
    let mut queue = VecDeque::new();
    for root in roots {
        levels.insert(root.clone(), 0);
        queue.push_back(root);
    }

    while let Some(parent) = queue.pop_front() {
        let parent_level = *levels.get(&parent).unwrap_or(&0);
        for child in children_by_parent.get(&parent).into_iter().flatten() {
            if !levels.contains_key(child) {
                levels.insert(child.clone(), parent_level + 1);
                queue.push_back(child.clone());
            }
        }
    }

    for id in node_ids {
        levels.entry(id).or_insert(0);
    }
    levels
}

fn svg_node_shape(
    role: &str,
    shape: &str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> String {
    let (fill, stroke, radius) = match (role, shape) {
        ("state", _) => ("#ecfeff", "#0e7490", 12),
        ("action", _) => ("#fff7ed", "#c2410c", 12),
        ("block", _) => ("#f0fdf4", "#15803d", 6),
        (_, "object") => ("#eff6ff", "#1d4ed8", 4),
        (_, _) => ("#ffffff", "#334155", 8),
    };
    format!(
        r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>
"##
    )
}

fn svg_routed_path(route: &str, x1: isize, y1: isize, x2: isize, y2: isize) -> String {
    if route == "orthogonal" || route == "elbow" {
        let mid_x = (x1 + x2) / 2;
        format!("M {x1} {y1} L {mid_x} {y1} L {mid_x} {y2} L {x2} {y2}")
    } else {
        format!("M {x1} {y1} L {x2} {y2}")
    }
}

fn svg_rectangle_boundary_point(
    center: (usize, usize),
    toward: (usize, usize),
    half_width: usize,
    half_height: usize,
) -> (isize, isize) {
    let dx = toward.0 as f64 - center.0 as f64;
    let dy = toward.1 as f64 - center.1 as f64;
    if dx.abs() < f64::EPSILON && dy.abs() < f64::EPSILON {
        return (center.0 as isize, center.1 as isize);
    }

    let scale_x = if dx.abs() < f64::EPSILON {
        f64::INFINITY
    } else {
        half_width as f64 / dx.abs()
    };
    let scale_y = if dy.abs() < f64::EPSILON {
        f64::INFINITY
    } else {
        half_height as f64 / dy.abs()
    };
    let scale = scale_x.min(scale_y);

    (
        (center.0 as f64 + dx * scale).round() as isize,
        (center.1 as f64 + dy * scale).round() as isize,
    )
}

fn svg_target_decoration(decoration: &str, x1: isize, y1: isize, x2: isize, y2: isize) -> String {
    if decoration == "none" {
        return String::new();
    }

    let dx = x2 as f64 - x1 as f64;
    let dy = y2 as f64 - y1 as f64;
    let length = (dx * dx + dy * dy).sqrt();
    if length < f64::EPSILON {
        return String::new();
    }

    let ux = dx / length;
    let uy = dy / length;
    let px = -uy;
    let py = ux;
    let size = if decoration == "hollow_triangle" {
        20.0
    } else {
        14.0
    };
    let spread = if decoration == "hollow_triangle" {
        12.0
    } else {
        7.0
    };
    let tip_x = x2 as f64;
    let tip_y = y2 as f64;
    let left_x = tip_x - ux * size + px * spread;
    let left_y = tip_y - uy * size + py * spread;
    let right_x = tip_x - ux * size - px * spread;
    let right_y = tip_y - uy * size - py * spread;

    if decoration == "hollow_triangle" {
        return format!(
            r##"<path d="M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z" fill="#f8fafc" stroke="#0f172a" stroke-width="3" stroke-linejoin="miter"/>
"##,
            tip_x, tip_y, left_x, left_y, right_x, right_y
        );
    }

    format!(
        r##"<path d="M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1}" fill="none" stroke="#0f172a" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
"##,
        left_x, left_y, tip_x, tip_y, right_x, right_y
    )
}

fn svg_source_decoration(decoration: &str, x1: isize, y1: isize, x2: isize, y2: isize) -> String {
    if decoration != "filled_diamond" {
        return String::new();
    }

    let dx = x2 as f64 - x1 as f64;
    let dy = y2 as f64 - y1 as f64;
    let length = (dx * dx + dy * dy).sqrt();
    if length < f64::EPSILON {
        return String::new();
    }

    let ux = dx / length;
    let uy = dy / length;
    let px = -uy;
    let py = ux;
    let size = 10.0;
    let half_width = 5.5;
    let tip_x = x1 as f64;
    let tip_y = y1 as f64;
    let center_x = tip_x + ux * size;
    let center_y = tip_y + uy * size;
    let tail_x = tip_x + ux * size * 2.0;
    let tail_y = tip_y + uy * size * 2.0;
    let side_a_x = center_x + px * half_width;
    let side_a_y = center_y + py * half_width;
    let side_b_x = center_x - px * half_width;
    let side_b_y = center_y - py * half_width;

    format!(
        r##"<path d="M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z" fill="#0f172a" stroke="#0f172a" stroke-width="1"/>
"##,
        tip_x, tip_y, side_a_x, side_a_y, tail_x, tail_y, side_b_x, side_b_y
    )
}

fn svg_symbol_property(symbol: Option<&DiagramSymbolDto>, key: &str) -> Option<String> {
    symbol?
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn svg_truncate(value: &str, max_chars: usize) -> String {
    let mut output = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    svg_escape(&output)
}

fn svg_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mercurio_kir::{KirDocument, KirElement};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn sample_graph() -> (Graph, MetamodelAttributeRegistry) {
        let document = view_fixture_document();
        let graph = Graph::from_document(document).expect("sample graph should be valid");
        let registry = MetamodelAttributeRegistry::build(&graph);
        (graph, registry)
    }

    fn view_fixture_document() -> KirDocument {
        fn element(
            id: &str,
            kind: &str,
            layer: u8,
            properties: BTreeMap<String, Value>,
        ) -> KirElement {
            KirElement {
                id: id.to_string(),
                kind: kind.to_string(),
                layer,
                properties,
            }
        }

        KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element(
                    "Comment",
                    "Metaclass",
                    1,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Comment")),
                        (
                            "features".to_string(),
                            json!(["metafeature.Comment.body", "metafeature.Comment.locale"]),
                        ),
                    ]),
                ),
                element(
                    "metafeature.Comment.body",
                    "MetamodelFeature",
                    1,
                    BTreeMap::from([
                        ("owner".to_string(), json!("Comment")),
                        ("kir_property".to_string(), json!("body")),
                        ("type_label".to_string(), json!("String")),
                    ]),
                ),
                element(
                    "metafeature.Comment.locale",
                    "MetamodelFeature",
                    1,
                    BTreeMap::from([
                        ("owner".to_string(), json!("Comment")),
                        ("kir_property".to_string(), json!("locale")),
                        ("type_label".to_string(), json!("String")),
                    ]),
                ),
                element(
                    "Model::Kernel::ComponentDefinition",
                    "PartDefinition",
                    1,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("ComponentDefinition")),
                        (
                            "qualified_name".to_string(),
                            json!("Model::Kernel::ComponentDefinition"),
                        ),
                    ]),
                ),
                element(
                    "pkg.Example",
                    "Package",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Example")),
                        ("qualified_name".to_string(), json!("Example")),
                        (
                            "members".to_string(),
                            json!([
                                "type.Example.Vehicle",
                                "port.Example.Vehicle.power",
                                "connection.Example.Vehicle.ControllerPower",
                                "satisfy.Example.Vehicle.ControllerSafeStart",
                                "state.Example.DriveMode",
                                "activity.Example.Startup",
                                "req.Example.SafeStart",
                                "comment.Example.Note",
                                "comment.Example.LocalizedNote"
                            ]),
                        ),
                    ]),
                ),
                element(
                    "type.Example.Vehicle",
                    "PartDefinition",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        ("qualified_name".to_string(), json!("Example.Vehicle")),
                        ("owner".to_string(), json!("pkg.Example")),
                        (
                            "specializes".to_string(),
                            json!(["Model::Kernel::ComponentDefinition"]),
                        ),
                    ]),
                ),
                element(
                    "feature.Example.Vehicle.controller",
                    "PartUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("controller")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.Vehicle.controller"),
                        ),
                        ("owner".to_string(), json!("type.Example.Vehicle")),
                        ("owning_type".to_string(), json!("type.Example.Vehicle")),
                        ("satisfy".to_string(), json!(["req.Example.SafeStart"])),
                    ]),
                ),
                element(
                    "port.Example.Vehicle.power",
                    "PortUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("power")),
                        ("qualified_name".to_string(), json!("Example.Vehicle.power")),
                        ("owner".to_string(), json!("type.Example.Vehicle")),
                        ("owning_type".to_string(), json!("type.Example.Vehicle")),
                    ]),
                ),
                element(
                    "connection.Example.Vehicle.ControllerPower",
                    "ConnectionUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("ControllerPower")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.Vehicle.ControllerPower"),
                        ),
                        ("owner".to_string(), json!("type.Example.Vehicle")),
                        (
                            "source".to_string(),
                            json!("feature.Example.Vehicle.controller"),
                        ),
                        ("target".to_string(), json!("port.Example.Vehicle.power")),
                    ]),
                ),
                element(
                    "satisfy.Example.Vehicle.ControllerSafeStart",
                    "SatisfyRequirementUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("ControllerSafeStart")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.Vehicle.ControllerSafeStart"),
                        ),
                        ("owner".to_string(), json!("pkg.Example")),
                        (
                            "source".to_string(),
                            json!("feature.Example.Vehicle.controller"),
                        ),
                        ("target".to_string(), json!("feature.Example.SafeStart")),
                    ]),
                ),
                element(
                    "state.Example.DriveMode",
                    "StateUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("DriveMode")),
                        ("qualified_name".to_string(), json!("Example.DriveMode")),
                        ("owner".to_string(), json!("pkg.Example")),
                        ("source".to_string(), json!("state.Example.Parked")),
                        ("target".to_string(), json!("state.Example.Driving")),
                    ]),
                ),
                element(
                    "state.Example.Parked",
                    "StateUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Parked")),
                        ("qualified_name".to_string(), json!("Example.Parked")),
                        ("owner".to_string(), json!("state.Example.DriveMode")),
                        ("is_initial".to_string(), json!(true)),
                    ]),
                ),
                element(
                    "state.Example.Driving",
                    "StateUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Driving")),
                        ("qualified_name".to_string(), json!("Example.Driving")),
                        ("owner".to_string(), json!("state.Example.DriveMode")),
                        ("is_final".to_string(), json!(true)),
                    ]),
                ),
                element(
                    "transition.Example.DriveMode.ParkedToDriving",
                    "TransitionUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("ParkedToDriving")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.DriveMode.ParkedToDriving"),
                        ),
                        ("owner".to_string(), json!("state.Example.DriveMode")),
                        ("source".to_string(), json!("state.Example.Parked")),
                        ("target".to_string(), json!("state.Example.Driving")),
                        ("trigger".to_string(), json!("drive")),
                        ("trigger_kind".to_string(), json!("event")),
                    ]),
                ),
                element(
                    "activity.Example.Startup",
                    "ActivityUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Startup")),
                        ("qualified_name".to_string(), json!("Example.Startup")),
                        ("owner".to_string(), json!("pkg.Example")),
                    ]),
                ),
                element(
                    "action.Example.Startup.Validate",
                    "ActionUsage",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Validate")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.Startup.Validate"),
                        ),
                        ("owner".to_string(), json!("activity.Example.Startup")),
                    ]),
                ),
                element(
                    "flow.Example.Startup.Validate",
                    "ControlFlow",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("ValidateFlow")),
                        (
                            "qualified_name".to_string(),
                            json!("Example.Startup.ValidateFlow"),
                        ),
                        ("owner".to_string(), json!("activity.Example.Startup")),
                        ("source".to_string(), json!("activity.Example.Startup")),
                        (
                            "target".to_string(),
                            json!("action.Example.Startup.Validate"),
                        ),
                    ]),
                ),
                element(
                    "req.Example.SafeStart",
                    "KerML::Core::Feature",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("SafeStart")),
                        ("qualified_name".to_string(), json!("Example.SafeStart")),
                        ("metatype".to_string(), json!("SysML::RequirementUsage")),
                        ("owner".to_string(), json!("pkg.Example")),
                        ("requirement_id".to_string(), json!("REQ-001")),
                        (
                            "text".to_string(),
                            json!("The vehicle shall prevent unsafe starts."),
                        ),
                        (
                            "metadata".to_string(),
                            json!({
                                "Review": {
                                    "properties": {
                                        "status": "approved",
                                        "owner": "Foundation Team",
                                        "reviewDate": "2026-06-03"
                                    }
                                }
                            }),
                        ),
                    ]),
                ),
                element(
                    "comment.Example.Note",
                    "Comment",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Note")),
                        ("qualified_name".to_string(), json!("Example.Note")),
                        ("owner".to_string(), json!("pkg.Example")),
                        ("metatype".to_string(), json!("Comment")),
                        ("body".to_string(), json!("Primary model note.")),
                        ("locale".to_string(), json!("en-US")),
                    ]),
                ),
                element(
                    "comment.Example.LocalizedNote",
                    "Comment",
                    2,
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("LocalizedNote")),
                        ("qualified_name".to_string(), json!("Example.LocalizedNote")),
                        ("owner".to_string(), json!("pkg.Example")),
                        ("metatype".to_string(), json!("Comment")),
                        ("body".to_string(), json!("Localized model note.")),
                        ("locale".to_string(), json!("fr-FR")),
                    ]),
                ),
            ],
        }
    }

    fn render_sample(spec: DiagramSpecDto) -> DiagramViewDto {
        let (graph, registry) = sample_graph();
        render_diagram(&graph, &registry, spec).expect("sample diagram should render")
    }

    fn render_table_sample(spec: TableSpecDto) -> TableViewDto {
        let (graph, registry) = sample_graph();
        render_table(&graph, &registry, spec).expect("sample table should render")
    }

    #[test]
    fn merge_view_overlay_decorates_copy_without_mutating_base() {
        let base = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::StateMachine,
            title: "Drive Mode".to_string(),
            description: None,
            root: Some("state.Example.DriveMode".to_string()),
            query: DiagramQueryOptionsDto {
                relations: Vec::new(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });
        let base_svg = render_diagram_svg(&base);
        let overlay = ViewOverlayDto {
            version: 1,
            frames: vec![ViewOverlayFrameDto {
                index: 2,
                time_s: Some(2.0),
                node_marks: vec![ViewNodeMarkDto {
                    element: "state.Example.Driving".to_string(),
                    kind: "active".to_string(),
                    label: Some("active".to_string()),
                    properties: serde_json::Map::new(),
                }],
                edge_marks: vec![ViewEdgeMarkDto {
                    element: "transition.Example.DriveMode.ParkedToDriving".to_string(),
                    kind: "visited".to_string(),
                    label: Some("visited".to_string()),
                    properties: serde_json::Map::new(),
                }],
                node_values: vec![ViewNodeValueDto {
                    element: "state.Example.Driving".to_string(),
                    key: "mode".to_string(),
                    value: json!("Running"),
                    label: Some("mode".to_string()),
                    unit: None,
                }],
                warnings: Vec::new(),
            }],
        };

        let decorated = merge_view_overlay(&base, &overlay, 2);
        let running = decorated
            .nodes
            .iter()
            .find(|node| node.id == "state.Example.Driving")
            .expect("driving state node");
        assert!(running.badges.iter().any(|badge| badge == "active"));
        assert!(running.badges.iter().any(|badge| badge == "mode=Running"));
        assert!(running.properties.contains_key("overlay_marks"));
        assert!(decorated.edges.iter().any(|edge| {
            edge.id == "transition.Example.DriveMode.ParkedToDriving"
                && edge.label.contains("visited")
        }));
        assert_eq!(render_diagram_svg(&base), base_svg);
        assert_ne!(render_diagram_svg(&decorated), base_svg);
    }

    #[test]
    fn merge_view_overlay_reports_missing_frame_and_targets() {
        let base = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::Activity,
            title: "Startup".to_string(),
            description: None,
            root: Some("activity.Example.Startup".to_string()),
            query: DiagramQueryOptionsDto {
                relations: Vec::new(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });
        let overlay = ViewOverlayDto {
            version: 1,
            frames: vec![ViewOverlayFrameDto {
                index: 0,
                time_s: None,
                node_marks: vec![ViewNodeMarkDto {
                    element: "missing.node".to_string(),
                    kind: "active".to_string(),
                    label: None,
                    properties: serde_json::Map::new(),
                }],
                edge_marks: Vec::new(),
                node_values: Vec::new(),
                warnings: Vec::new(),
            }],
        };

        let missing_target = merge_view_overlay(&base, &overlay, 0);
        assert!(
            missing_target
                .warnings
                .iter()
                .any(|warning| { warning.contains("references unknown element `missing.node`") })
        );
        let missing_frame = merge_view_overlay(&base, &overlay, 99);
        assert!(
            missing_frame
                .warnings
                .iter()
                .any(|warning| warning.contains("Overlay frame 99 was not found"))
        );
    }

    fn structure_spec(root: Option<&str>, relations: Vec<&str>) -> DiagramSpecDto {
        DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::Structure,
            title: "Sample".to_string(),
            description: None,
            root: root.map(str::to_string),
            query: DiagramQueryOptionsDto {
                relations: relations.into_iter().map(str::to_string).collect(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        }
    }

    fn bdd_spec(root: &str) -> DiagramSpecDto {
        DiagramSpecDto {
            kind: DiagramKindDto::Bdd,
            ..structure_spec(Some(root), Vec::new())
        }
    }

    fn bdd_vehicle_fixture_graph() -> (Graph, MetamodelAttributeRegistry) {
        fn element(id: &str, kind: &str, properties: BTreeMap<String, Value>) -> KirElement {
            KirElement {
                id: id.to_string(),
                kind: kind.to_string(),
                layer: 2,
                properties,
            }
        }

        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element(
                    "pkg.Fleet",
                    "Package",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Fleet")),
                        ("qualified_name".to_string(), json!("Fleet")),
                    ]),
                ),
                element(
                    "type.Fleet.Platform",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Platform")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                    ]),
                ),
                element(
                    "type.Fleet.Vehicle",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                        ("specializes".to_string(), json!(["type.Fleet.Platform"])),
                    ]),
                ),
                element(
                    "type.Fleet.Chassis",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Chassis")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                    ]),
                ),
                element(
                    "type.Fleet.Battery",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Battery")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                    ]),
                ),
                element(
                    "type.Fleet.Motor",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Motor")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                    ]),
                ),
                element(
                    "type.Fleet.Wheel",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Wheel")),
                        ("owner".to_string(), json!("pkg.Fleet")),
                    ]),
                ),
                element(
                    "part.Fleet.Vehicle.chassis",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("chassis")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                        ("definition".to_string(), json!("type.Fleet.Chassis")),
                    ]),
                ),
                element(
                    "part.Fleet.Vehicle.battery",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("battery")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                        ("definition".to_string(), json!("type.Fleet.Battery")),
                        ("multiplicity".to_string(), json!("2")),
                    ]),
                ),
                // Untyped grouping usage: the nested motor usage below must
                // still roll up to Vehicle through it.
                element(
                    "part.Fleet.Vehicle.drivetrain",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("drivetrain")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                    ]),
                ),
                element(
                    "part.Fleet.Vehicle.drivetrain.motor",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("motor")),
                        ("owner".to_string(), json!("part.Fleet.Vehicle.drivetrain")),
                        ("definition".to_string(), json!("type.Fleet.Motor")),
                    ]),
                ),
                // Two usages of the same definition: must merge into a single
                // composition edge with a combined label and count.
                element(
                    "part.Fleet.Vehicle.frontWheel",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("frontWheel")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                        ("definition".to_string(), json!("type.Fleet.Wheel")),
                    ]),
                ),
                element(
                    "part.Fleet.Vehicle.rearWheel",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("rearWheel")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                        ("definition".to_string(), json!("type.Fleet.Wheel")),
                    ]),
                ),
                element(
                    "attr.Fleet.Vehicle.mass",
                    "AttributeUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("mass")),
                        ("owner".to_string(), json!("type.Fleet.Vehicle")),
                        ("type".to_string(), json!("Real")),
                        ("value".to_string(), json!(1200)),
                    ]),
                ),
            ],
        };
        let graph = Graph::from_document(document).expect("bdd fixture graph should be valid");
        let registry = MetamodelAttributeRegistry::build(&graph);
        (graph, registry)
    }

    #[test]
    fn bdd_diagram_rolls_up_definition_level_composition() {
        let (graph, registry) = bdd_vehicle_fixture_graph();
        let view = render_diagram(&graph, &registry, bdd_spec("pkg.Fleet"))
            .expect("bdd diagram should render");

        let node_ids = view
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            node_ids,
            vec![
                "type.Fleet.Battery",
                "type.Fleet.Chassis",
                "type.Fleet.Motor",
                "type.Fleet.Platform",
                "type.Fleet.Vehicle",
                "type.Fleet.Wheel",
            ],
            "bdd shows only block definitions: no package, usage, or attribute nodes"
        );

        assert!(
            view.edges.iter().all(|edge| edge.relation != "owner"),
            "bdd must not emit ownership/containment edges"
        );

        let part_edges = view
            .edges
            .iter()
            .filter(|edge| edge.relation == "part")
            .map(|edge| {
                (
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.label.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            part_edges,
            vec![
                ("type.Fleet.Vehicle", "type.Fleet.Battery", "battery [2]"),
                ("type.Fleet.Vehicle", "type.Fleet.Chassis", "chassis"),
                (
                    "type.Fleet.Vehicle",
                    "type.Fleet.Motor",
                    "motor"
                ),
                (
                    "type.Fleet.Vehicle",
                    "type.Fleet.Wheel",
                    "frontWheel, rearWheel (2)"
                ),
            ],
            "composition rolls up to definitions, including nested usages, one edge per pair"
        );

        let specialization_edges = view
            .edges
            .iter()
            .filter(|edge| edge.relation == "specializes")
            .map(|edge| {
                (
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.label.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            specialization_edges,
            vec![("type.Fleet.Vehicle", "type.Fleet.Platform", ":>")],
            "definition-level specialization edges are preserved"
        );

        let vehicle = view
            .nodes
            .iter()
            .find(|node| node.id == "type.Fleet.Vehicle")
            .expect("vehicle node is present");
        let mass_row = vehicle
            .attributes
            .iter()
            .find(|attribute| attribute.name == "mass")
            .expect("attribute usages ride the definition node as attribute rows");
        assert_eq!(mass_row.type_label.as_deref(), Some("Real = 1200"));
    }

    #[test]
    fn bdd_diagram_resolves_fuzzy_and_qualified_root_names() {
        let (graph, registry) = bdd_vehicle_fixture_graph();
        for (root, canonical) in [
            ("Fleet", "pkg.Fleet"),
            ("Fleet::Vehicle", "type.Fleet.Vehicle"),
            ("Vehicle", "type.Fleet.Vehicle"),
        ] {
            let view = render_diagram(&graph, &registry, bdd_spec(root))
                .unwrap_or_else(|error| panic!("root `{root}` should render: {error:?}"));
            assert_eq!(
                view.spec.root.as_deref(),
                Some(canonical),
                "root `{root}` should canonicalize to `{canonical}`"
            );
            assert!(
                view.nodes.iter().any(|node| node.id == "type.Fleet.Vehicle"),
                "root `{root}` should surface the Vehicle block, got {:?}",
                view.nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn unrooted_diagram_seeds_user_model_before_large_library_base() {
        // A library base larger than the traversal budget must not starve the
        // user model out of an unrooted render.
        let mut elements = (0..2 * DEFAULT_MAX_NODES)
            .map(|index| KirElement {
                id: format!("LibraryPackage{index:04}"),
                kind: "Package".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            })
            .collect::<Vec<_>>();
        elements.push(KirElement {
            id: "pkg.Rover".to_string(),
            kind: "Package".to_string(),
            layer: 2,
            properties: BTreeMap::from([("declared_name".to_string(), json!("Rover"))]),
        });
        elements.push(KirElement {
            id: "type.Rover.Rover".to_string(),
            kind: "PartDefinition".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), json!("Rover")),
                ("owner".to_string(), json!("pkg.Rover")),
            ]),
        });
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements,
        };
        let graph = Graph::from_document(document).expect("graph should build");
        let registry = MetamodelAttributeRegistry::build(&graph);

        let mut spec = structure_spec(None, Vec::new());
        spec.query.include_libraries = true;
        let view = render_diagram(&graph, &registry, spec.clone())
            .expect("unrooted structure diagram should render");
        assert_eq!(
            view.nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["pkg.Rover", "type.Rover.Rover"],
            "unrooted structure render seeds from the user model"
        );

        spec.kind = DiagramKindDto::Bdd;
        let view =
            render_diagram(&graph, &registry, spec).expect("unrooted bdd diagram should render");
        assert!(
            view.nodes.iter().any(|node| node.id == "type.Rover.Rover"),
            "unrooted bdd render keeps the user blocks, got {:?}",
            view.nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn bdd_diagram_composition_edges_use_part_symbol_records() {
        let (graph, registry) = bdd_vehicle_fixture_graph();
        let view = render_diagram(&graph, &registry, bdd_spec("pkg.Fleet"))
            .expect("bdd diagram should render");

        let wheel_edges = view
            .edges
            .iter()
            .filter(|edge| edge.relation == "part" && edge.target == "type.Fleet.Wheel")
            .collect::<Vec<_>>();
        assert_eq!(
            wheel_edges.len(),
            1,
            "duplicate part usages of one definition merge into a single edge"
        );
        let wheel_edge = wheel_edges[0];
        assert_eq!(wheel_edge.id, "type.Fleet.Vehicle:part:type.Fleet.Wheel");

        let symbol = view
            .symbols
            .iter()
            .find(|symbol| symbol.id == wheel_edge.symbol)
            .expect("composition edge has a symbol record");
        assert_eq!(symbol.role, "edge");
        assert_eq!(symbol.relation.as_deref(), Some("part"));
        assert_eq!(
            symbol
                .properties
                .get("source_decoration")
                .and_then(Value::as_str),
            Some("filled_diamond"),
            "composition symbol carries the filled-diamond decoration for the client resolver"
        );

        for node in &view.nodes {
            let node_symbol = view
                .symbols
                .iter()
                .find(|symbol| symbol.id == node.symbol)
                .expect("definition node has a symbol record");
            assert_eq!(node_symbol.role, "block");
        }
    }

    #[test]
    fn list_view_kinds_covers_existing_view_families() {
        let descriptors = list_view_kinds();
        let ids = descriptors
            .iter()
            .map(|descriptor| descriptor.id.as_str())
            .collect::<BTreeSet<_>>();

        assert!(ids.contains("diagram.state_machine"));
        assert!(ids.contains("diagram.activity"));
        assert!(ids.contains("diagram.bdd"));
        assert!(ids.contains("table.requirements"));
        assert!(ids.contains("model.graph"));
        assert!(ids.contains("explorer.model"));
        assert_eq!(
            ids.len(),
            list_diagram_kinds().len() + list_table_kinds().len() + 7
        );

        let state_machine = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "diagram.state_machine")
            .expect("state machine descriptor is present");
        assert_eq!(state_machine.family, ViewFamilyDto::Diagram);
        assert_eq!(state_machine.root, ViewRootRequirementDto::Required);
        assert!(state_machine.animatable);
    }

    #[test]
    fn table_descriptors_pin_per_kind_editable_flags() {
        let editable_by_id = list_view_kinds()
            .into_iter()
            .map(|descriptor| (descriptor.id, descriptor.editable))
            .collect::<std::collections::BTreeMap<_, _>>();

        // DA-3: editable is true only where cell-edit gestures are wired.
        assert_eq!(editable_by_id.get("table.requirements"), Some(&true));
        assert_eq!(editable_by_id.get("table.elements"), Some(&true));
        assert_eq!(editable_by_id.get("table.model_elements"), Some(&false));
    }

    #[test]
    fn view_catalog_returns_descriptor_backed_renderable_offers() {
        let (graph, registry) = sample_graph();
        let catalog = view_catalog(&graph);
        let descriptor_ids = list_view_kinds()
            .into_iter()
            .map(|descriptor| descriptor.id)
            .collect::<BTreeSet<_>>();

        assert!(
            catalog
                .entries
                .iter()
                .all(|entry| descriptor_ids.contains(&entry.kind)),
            "{catalog:#?}"
        );
        assert!(catalog.entries.iter().any(|entry| {
            entry.kind == "diagram.state_machine"
                && entry.subject.as_deref() == Some("state.Example.DriveMode")
        }));
        assert!(catalog.entries.iter().any(|entry| {
            entry.kind == "diagram.activity"
                && entry.subject.as_deref() == Some("activity.Example.Startup")
        }));
        assert!(catalog.entries.iter().any(|entry| {
            entry.kind == "diagram.internal_block"
                && entry.subject.as_deref() == Some("type.Example.Vehicle")
        }));
        assert!(catalog.entries.iter().any(|entry| {
            entry.kind == "diagram.requirement" && entry.subject.as_deref() == Some("pkg.Example")
        }));
        assert!(
            catalog
                .entries
                .iter()
                .any(|entry| entry.kind == "table.requirements")
        );

        for entry in &catalog.entries {
            validate_view_document(&entry.spec).expect("catalog spec should validate");
            if let Some(spec) = entry.spec.diagram.clone() {
                render_diagram(&graph, &registry, spec).unwrap_or_else(|err| {
                    panic!("catalog diagram `{}` should render: {err}", entry.kind)
                });
            }
            if let Some(spec) = entry.spec.table.clone() {
                render_table(&graph, &registry, spec).unwrap_or_else(|err| {
                    panic!("catalog table `{}` should render: {err}", entry.kind)
                });
            }
        }
    }

    #[test]
    fn diagram_affordances_are_opt_in_and_catalog_backed() {
        let lean = render_sample(structure_spec(Some("pkg.Example"), vec!["owner"]));
        assert!(lean.nodes.iter().all(|node| node.affordances.is_empty()));
        let lean_json = serde_json::to_value(&lean).expect("lean view should serialize");
        assert!(
            lean_json["nodes"]
                .as_array()
                .expect("nodes should be an array")
                .iter()
                .all(|node| node.get("affordances").is_none())
        );

        let mut spec = structure_spec(Some("pkg.Example"), vec!["owner"]);
        spec.style.show_affordances = true;
        let view = render_sample(spec);
        let drive_mode = view
            .nodes
            .iter()
            .find(|node| node.id == "state.Example.DriveMode")
            .expect("state machine root should render");
        let affordance = drive_mode
            .affordances
            .iter()
            .find(|affordance| affordance.kind == "diagram.state_machine")
            .expect("state machine root should expose state machine affordance");

        assert_eq!(
            affordance.subject.as_deref(),
            Some("state.Example.DriveMode")
        );
        assert!(affordance.spec.diagram.as_ref().is_some_and(|spec| {
            spec.root.as_deref() == Some("state.Example.DriveMode")
                && spec.kind == DiagramKindDto::StateMachine
        }));
    }

    #[test]
    fn table_affordances_are_opt_in_and_catalog_backed() {
        let mut spec = TableSpecDto {
            version: 1,
            kind: TableKindDto::ModelElements,
            title: "Model Elements".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::ContainmentSubtree {
                root: "pkg.Example".to_string(),
            },
            row_type: Some(TableRowTypeDto {
                type_name: "Element".to_string(),
                include_subtypes: true,
            }),
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: Vec::new(),
            show_affordances: false,
        };
        let lean = render_table_sample(spec.clone());
        assert!(lean.rows.iter().all(|row| row.affordances.is_empty()));

        spec.show_affordances = true;
        let view = render_table_sample(spec);
        let vehicle = view
            .rows
            .iter()
            .find(|row| row.element == "type.Example.Vehicle")
            .expect("vehicle row should render");
        assert!(vehicle.affordances.iter().any(|affordance| {
            affordance.kind == "diagram.internal_block"
                && affordance.subject.as_deref() == Some("type.Example.Vehicle")
                && affordance.spec.diagram.as_ref().is_some_and(|spec| {
                    spec.kind == DiagramKindDto::InternalBlock
                        && spec.root.as_deref() == Some("type.Example.Vehicle")
                })
        }));
    }

    #[test]
    fn affordance_flags_accept_camel_case_input() {
        let diagram: DiagramSpecDto = serde_json::from_value(json!({
            "version": 1,
            "kind": "structure",
            "title": "Overview",
            "style": {
                "showAffordances": true
            }
        }))
        .expect("diagram spec should accept camelCase affordance flag");
        assert!(diagram.style.show_affordances);

        let table: TableSpecDto = serde_json::from_value(json!({
            "version": 1,
            "kind": "model_elements",
            "title": "Elements",
            "showAffordances": true
        }))
        .expect("table spec should accept camelCase affordance flag");
        assert!(table.show_affordances);
    }

    #[test]
    fn internal_block_diagram_renders_parts_ports_and_connections() {
        let view = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::InternalBlock,
            title: "Vehicle IBD".to_string(),
            description: None,
            root: Some("type.Example.Vehicle".to_string()),
            query: DiagramQueryOptionsDto {
                relations: vec![],
                direction: DiagramDirectionDto::Children,
                depth: 2,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });

        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "type.Example.Vehicle")
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "feature.Example.Vehicle.controller")
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "port.Example.Vehicle.power")
        );
        assert!(view.edges.iter().any(|edge| {
            edge.relation == "connection"
                && edge.source == "feature.Example.Vehicle.controller"
                && edge.target == "port.Example.Vehicle.power"
        }));
        assert!(view.symbols.iter().any(|symbol| {
            symbol.element == "port.Example.Vehicle.power" && symbol.role == "port"
        }));
    }

    #[test]
    fn requirement_diagram_renders_satisfy_edges() {
        let view = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::Requirement,
            title: "Requirements".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            query: DiagramQueryOptionsDto {
                relations: requirement_diagram_relations(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });

        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "req.Example.SafeStart")
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "feature.Example.Vehicle.controller")
        );
        assert!(view.edges.iter().any(|edge| {
            edge.relation == "satisfy"
                && edge.source == "feature.Example.Vehicle.controller"
                && edge.target == "req.Example.SafeStart"
        }));
        assert!(view.symbols.iter().any(|symbol| {
            symbol.element == "req.Example.SafeStart" && symbol.role == "requirement"
        }));
    }

    #[test]
    fn requirement_relationship_endpoints_resolve_feature_requirement_aliases() {
        let (graph, _) = sample_graph();
        let relationship = graph
            .element_by_element_id("satisfy.Example.Vehicle.ControllerSafeStart")
            .expect("sample relationship");

        assert_eq!(
            relationship_endpoints(&graph, relationship),
            Some((
                "feature.Example.Vehicle.controller".to_string(),
                "req.Example.SafeStart".to_string()
            ))
        );
    }

    #[test]
    fn structure_diagram_renders_sample_package_containment() {
        let view = render_sample(structure_spec(Some("pkg.Example"), vec!["owner"]));

        assert!(view.nodes.iter().any(|node| node.id == "pkg.Example"));
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "type.Example.Vehicle")
        );
        assert!(
            view.symbols
                .iter()
                .any(|symbol| symbol.element == "type.Example.Vehicle"
                    && symbol.id == "symbol.type_Example_Vehicle")
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "state.Example.DriveMode")
        );
        assert!(view.edges.iter().any(|edge| {
            edge.source == "type.Example.Vehicle"
                && edge.target == "pkg.Example"
                && edge.relation == "owner"
        }));
        assert!(view.warnings.is_empty());
    }

    #[test]
    fn structure_diagram_preserves_feature_relationship() {
        let view = render_sample(structure_spec(
            Some("type.Example.Vehicle"),
            vec!["owning_type"],
        ));

        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "type.Example.Vehicle")
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "feature.Example.Vehicle.controller")
        );
        let feature_edge = view
            .edges
            .iter()
            .find(|edge| {
                edge.source == "feature.Example.Vehicle.controller"
                    && edge.target == "type.Example.Vehicle"
                    && edge.relation == "owning_type"
            })
            .expect("feature edge should render");
        let feature_symbol = view
            .symbols
            .iter()
            .find(|symbol| symbol.id == feature_edge.symbol)
            .expect("feature edge should have a symbol");
        assert_eq!(feature_symbol.role, "edge");
        assert_eq!(
            feature_symbol.source.as_deref(),
            Some("symbol.feature_Example_Vehicle_controller")
        );
        assert_eq!(
            feature_symbol.target.as_deref(),
            Some("symbol.type_Example_Vehicle")
        );
        assert_eq!(feature_symbol.relation.as_deref(), Some("owning_type"));
        assert_eq!(
            feature_symbol
                .properties
                .get("route")
                .and_then(|value| value.as_str()),
            Some("straight")
        );
        assert_eq!(
            feature_symbol
                .properties
                .get("target_decoration")
                .and_then(|value| value.as_str()),
            Some("open_arrow")
        );
    }

    #[test]
    fn structure_diagram_validates_unknown_root() {
        let (graph, registry) = sample_graph();
        let error = render_diagram(
            &graph,
            &registry,
            structure_spec(Some("Vehicle::Missing"), vec!["owner"]),
        )
        .expect_err("unknown root should fail");

        assert_eq!(
            error,
            DiagramError::RootNotFound("Vehicle::Missing".to_string())
        );
    }

    #[test]
    fn structure_diagram_honors_include_library_filter() {
        let mut spec = structure_spec(
            Some("Model::Kernel::ComponentDefinition"),
            vec!["specializes"],
        );
        spec.query.include_libraries = true;
        spec.query.include_user_model = true;
        let with_libraries = render_sample(spec.clone());
        assert!(
            with_libraries
                .nodes
                .iter()
                .any(|node| node.id == "Model::Kernel::ComponentDefinition")
        );

        spec.query.include_libraries = false;
        let without_libraries = render_sample(spec);
        assert!(
            without_libraries
                .nodes
                .iter()
                .all(|node| node.id != "Model::Kernel::ComponentDefinition")
        );
    }

    #[test]
    fn structure_diagram_reports_edge_limit() {
        let mut spec = structure_spec(
            Some("state.Example.DriveMode"),
            vec!["owner", "source", "target"],
        );
        spec.query.max_edges = 2;
        let view = render_sample(spec);

        assert_eq!(view.edges.len(), 2);
        assert!(
            view.warnings
                .iter()
                .any(|warning| warning.contains("Diagram edge limit reached"))
        );
    }

    #[test]
    fn activity_diagram_assigns_activity_symbol_roles() {
        let view = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::Activity,
            title: "Startup Activity".to_string(),
            description: None,
            root: Some("activity.Example.Startup".to_string()),
            query: DiagramQueryOptionsDto {
                relations: vec![
                    "owner".to_string(),
                    "source".to_string(),
                    "target".to_string(),
                ],
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });

        let action = view
            .symbols
            .iter()
            .find(|symbol| symbol.element == "action.Example.Startup.Validate")
            .expect("activity action should have a symbol");
        assert_eq!(action.role, "action");
        assert_eq!(
            action
                .properties
                .get("shape")
                .and_then(|value| value.as_str()),
            Some("action")
        );

        let flow = view
            .symbols
            .iter()
            .find(|symbol| symbol.relation.as_deref() == Some("control_flow"))
            .expect("activity control flow should have a symbol");
        assert_eq!(flow.role, "edge");
        assert_eq!(
            flow.properties
                .get("route")
                .and_then(|value| value.as_str()),
            Some("orthogonal")
        );
        assert_eq!(
            flow.properties
                .get("target_decoration")
                .and_then(|value| value.as_str()),
            Some("open_arrow")
        );
    }

    #[test]
    fn state_machine_diagram_renders_states_and_transitions() {
        let view = render_sample(DiagramSpecDto {
            version: 1,
            kind: DiagramKindDto::StateMachine,
            title: "Drive Mode".to_string(),
            description: None,
            root: Some("state.Example.DriveMode".to_string()),
            query: DiagramQueryOptionsDto {
                relations: Vec::new(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            layout: DiagramLayoutOptionsDto::default(),
            style: DiagramStyleOptionsDto::default(),
        });

        assert_eq!(view.spec.kind, DiagramKindDto::StateMachine);
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "state.Example.Parked"
                    && node.label == "Parked"
                    && node.badges.contains(&"initial".to_string()))
        );
        assert!(
            view.nodes
                .iter()
                .any(|node| node.id == "state.Example.Driving"
                    && node.badges.contains(&"final".to_string()))
        );
        let transition = view
            .edges
            .iter()
            .find(|edge| edge.id == "transition.Example.DriveMode.ParkedToDriving")
            .expect("transition edge should render");
        assert_eq!(transition.source, "state.Example.Parked");
        assert_eq!(transition.target, "state.Example.Driving");
        assert_eq!(transition.relation, "transition");
        assert_eq!(transition.label, "drive");

        let state_symbol = view
            .symbols
            .iter()
            .find(|symbol| symbol.element == "state.Example.Parked")
            .expect("state symbol should render");
        assert_eq!(state_symbol.role, "state");
        assert_eq!(
            state_symbol
                .properties
                .get("shape")
                .and_then(|value| value.as_str()),
            Some("state")
        );
        assert_eq!(
            state_symbol
                .properties
                .get("is_initial")
                .and_then(|value| value.as_bool()),
            Some(true)
        );

        let transition_symbol = view
            .symbols
            .iter()
            .find(|symbol| symbol.element == "transition.Example.DriveMode.ParkedToDriving")
            .expect("transition symbol should render");
        assert_eq!(transition_symbol.role, "transition");
        assert_eq!(
            transition_symbol
                .properties
                .get("target_decoration")
                .and_then(|value| value.as_str()),
            Some("open_arrow")
        );
        assert_eq!(
            transition_symbol
                .properties
                .get("trigger_kind")
                .and_then(|value| value.as_str()),
            Some("event")
        );
    }

    #[test]
    fn state_machine_diagram_resolves_shorthand_transition_endpoints() {
        let shorthand = KirElement {
            id: "transition.Example.DriveMode.Short".to_string(),
            kind: "TransitionUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), json!("Short")),
                ("owner".to_string(), json!("state.Example.DriveMode")),
                ("transitionSource".to_string(), json!("Parked")),
                ("transitionTarget".to_string(), json!("Driving")),
                ("trigger".to_string(), json!("short")),
            ]),
        };
        let mut document = view_fixture_document();
        document.elements.push(shorthand);
        let graph = Graph::from_document(document).expect("sample graph should rebuild");
        let registry = MetamodelAttributeRegistry::build(&graph);

        let view = render_diagram(
            &graph,
            &registry,
            DiagramSpecDto {
                version: 1,
                kind: DiagramKindDto::StateMachine,
                title: "Drive Mode".to_string(),
                description: None,
                root: Some("state.Example.DriveMode".to_string()),
                query: DiagramQueryOptionsDto {
                    relations: Vec::new(),
                    direction: DiagramDirectionDto::Children,
                    depth: 3,
                    include_libraries: false,
                    include_user_model: true,
                    max_nodes: 350,
                    max_edges: 900,
                },
                layout: DiagramLayoutOptionsDto::default(),
                style: DiagramStyleOptionsDto::default(),
            },
        )
        .expect("state-machine diagram should render");

        assert!(view.edges.iter().any(|edge| {
            edge.id == "transition.Example.DriveMode.Short"
                && edge.source == "state.Example.Parked"
                && edge.target == "state.Example.Driving"
                && edge.label == "short"
        }));
    }

    #[test]
    fn requirements_table_renders_requirement_rows() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::Requirements,
            title: "Requirements".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto {
                relations: vec!["owner".to_string()],
                direction: DiagramDirectionDto::Children,
                depth: 2,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            columns: Vec::new(),
            show_affordances: false,
        });

        assert_eq!(view.columns.len(), 5);
        let row = view
            .rows
            .iter()
            .find(|row| row.element == "req.Example.SafeStart")
            .expect("requirement row should render");
        assert!(
            row.cells
                .iter()
                .any(|cell| { cell.key == "requirement_id" && cell.value == "REQ-001" })
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "text" && cell.value.contains("unsafe starts"))
        );
    }

    /// Requirement text authored as `doc /* ... */` compiles to an owned
    /// `Documentation` element, not to a `text` property on the requirement;
    /// the TEXT column must fall back to that documentation body.
    #[test]
    fn requirements_table_text_falls_back_to_owned_documentation_body() {
        let requirement = KirElement {
            id: "req.Example.SoftStop".to_string(),
            kind: "SysML::Requirements::RequirementUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), json!("SoftStop")),
                ("qualified_name".to_string(), json!("Example.SoftStop")),
                ("owner".to_string(), json!("pkg.Example")),
            ]),
        };
        let documentation = KirElement {
            id: "doc.req.Example.SoftStop.1".to_string(),
            kind: "KerML::Root::Documentation".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("owner".to_string(), json!("req.Example.SoftStop")),
                ("body".to_string(), json!("The vehicle shall stop smoothly.")),
            ]),
        };
        let mut document = view_fixture_document();
        document.elements.push(requirement);
        document.elements.push(documentation);
        let graph = Graph::from_document(document).expect("sample graph should rebuild");
        let registry = MetamodelAttributeRegistry::build(&graph);

        let view = render_table(
            &graph,
            &registry,
            TableSpecDto {
                version: 1,
                kind: TableKindDto::Requirements,
                title: "Requirements".to_string(),
                description: None,
                root: Some("pkg.Example".to_string()),
                target_type: None,
                scope: TableScopeDto::WholeModel,
                row_type: None,
                column_scope: None,
                column_type: None,
                relations: Vec::new(),
                matrix_preset: None,
                query: DiagramQueryOptionsDto {
                    relations: vec!["owner".to_string()],
                    direction: DiagramDirectionDto::Children,
                    depth: 2,
                    include_libraries: false,
                    include_user_model: true,
                    max_nodes: 350,
                    max_edges: 900,
                },
                columns: Vec::new(),
                show_affordances: false,
            },
        )
        .expect("requirements table should render");

        let row = view
            .rows
            .iter()
            .find(|row| row.element == "req.Example.SoftStop")
            .expect("documented requirement row should render");
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "text"
                    && cell.value == "The vehicle shall stop smoothly."),
            "{row:#?}"
        );
    }

    #[test]
    fn view_document_round_trips_diagram_spec() {
        let document = ViewDocumentDto::diagram(structure_spec(Some("pkg.Example"), vec!["owner"]));

        validate_view_document(&document).expect("document should validate");
        let json = serde_json::to_string_pretty(&document).unwrap();
        let decoded: ViewDocumentDto = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.schema, VIEW_SCHEMA);
        assert_eq!(decoded.version, VIEW_SPEC_VERSION);
        assert_eq!(decoded.kind, "diagram.structure");
        assert!(decoded.diagram.is_some());
        assert!(decoded.table.is_none());
        assert_eq!(decoded, document);
    }

    #[test]
    fn view_document_round_trips_table_spec() {
        let document = ViewDocumentDto::table(TableSpecDto {
            version: 1,
            kind: TableKindDto::Requirements,
            title: "Requirements".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![TableColumnSpecDto {
                key: "requirement_id".to_string(),
                label: "ID".to_string(),
                path: None,
                expression: None,
            }],
            show_affordances: false,
        });

        validate_view_document(&document).expect("document should validate");
        let decoded: ViewDocumentDto =
            serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();

        assert_eq!(decoded.kind, "table");
        assert!(decoded.diagram.is_none());
        assert!(decoded.table.is_some());
        assert_eq!(decoded, document);
    }

    #[test]
    fn view_document_round_trips_model_view() {
        let document = ViewDocumentDto::model(ModelViewSpecDto {
            version: 1,
            kind: ModelViewKindDto::MetatypeExplorer,
            title: "Metatype Explorer".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            graph_scope: None,
            query: None,
            expanded_parents: Vec::new(),
            expanded_children: Vec::new(),
            include_reference_edges: true,
        });

        validate_view_document(&document).expect("document should validate");
        let decoded: ViewDocumentDto =
            serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();

        assert_eq!(decoded.kind, "explorer.metatype");
        assert!(decoded.model.is_some());
        assert!(decoded.diagram.is_none());
        assert!(decoded.table.is_none());
        assert_eq!(decoded, document);
    }

    #[test]
    fn view_document_round_trips_every_model_view_kind() {
        let cases = [
            (
                ModelViewKindDto::Metadata,
                "model.metadata",
                None,
                None,
                None,
            ),
            (
                ModelViewKindDto::Graph,
                "model.graph",
                None,
                Some("model_plus_context".to_string()),
                None,
            ),
            (
                ModelViewKindDto::Search,
                "model.search",
                None,
                None,
                Some("vehicle".to_string()),
            ),
            (
                ModelViewKindDto::ElementDetails,
                "model.element_details",
                Some("pkg.Vehicle".to_string()),
                None,
                None,
            ),
            (
                ModelViewKindDto::LibraryTree,
                "model.library_tree",
                None,
                None,
                None,
            ),
            (
                ModelViewKindDto::ModelExplorer,
                "explorer.model",
                Some("pkg.Vehicle".to_string()),
                None,
                None,
            ),
            (
                ModelViewKindDto::MetatypeExplorer,
                "explorer.metatype",
                Some("pkg.Vehicle".to_string()),
                None,
                None,
            ),
        ];

        for (kind, expected_document_kind, root, graph_scope, query) in cases {
            let document = ViewDocumentDto::model(ModelViewSpecDto {
                version: 1,
                kind,
                title: expected_document_kind.to_string(),
                description: None,
                root,
                graph_scope,
                query,
                expanded_parents: Vec::new(),
                expanded_children: Vec::new(),
                include_reference_edges: true,
            });

            validate_view_document(&document).expect("model document should validate");
            let decoded: ViewDocumentDto =
                serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();

            assert_eq!(decoded.kind, expected_document_kind);
            assert_eq!(decoded, document);
        }
    }

    #[test]
    fn view_document_validation_rejects_multiple_primary_payloads() {
        let mut document =
            ViewDocumentDto::diagram(structure_spec(Some("pkg.Example"), vec!["owner"]));
        document.model = Some(ModelViewSpecDto {
            version: 1,
            kind: ModelViewKindDto::Metadata,
            title: "Model Metadata".to_string(),
            description: None,
            root: None,
            graph_scope: None,
            query: None,
            expanded_parents: Vec::new(),
            expanded_children: Vec::new(),
            include_reference_edges: true,
        });

        let diagnostics =
            validate_view_document(&document).expect_err("multiple payloads should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "view.payload")
        );
    }

    #[test]
    fn view_document_validation_rejects_old_model_public_inputs() {
        let explorer: ViewDocumentDto = serde_json::from_value(json!({
            "schema": "mercurio.view.v1",
            "version": 1,
            "kind": "explorer.l2",
            "mode": "visualization",
            "model": {
                "version": 1,
                "kind": "model_explorer",
                "title": "Model Explorer",
                "root": "pkg.Vehicle"
            }
        }))
        .unwrap();
        let diagnostics =
            validate_view_document(&explorer).expect_err("old explorer kind should fail");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "view.kind")
        );

        let graph: ViewDocumentDto = serde_json::from_value(json!({
            "schema": "mercurio.view.v1",
            "version": 1,
            "kind": "model.graph",
            "mode": "visualization",
            "model": {
                "version": 1,
                "kind": "graph",
                "title": "Model Graph",
                "graph_scope": "l2"
            }
        }))
        .unwrap();
        let diagnostics = validate_view_document(&graph).expect_err("old graph scope should fail");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "view.model.graph_scope")
        );
    }

    #[test]
    fn view_document_validation_rejects_mismatched_payload() {
        let mut document =
            ViewDocumentDto::diagram(structure_spec(Some("pkg.Example"), vec!["owner"]));
        document.kind = "table".to_string();

        let diagnostics =
            validate_view_document(&document).expect_err("mismatched kind should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "view.kind")
        );
    }

    #[test]
    fn view_document_validation_rejects_bad_table_columns() {
        let document = ViewDocumentDto::table(TableSpecDto {
            version: 1,
            kind: TableKindDto::Requirements,
            title: "Requirements".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![
                TableColumnSpecDto {
                    key: "id".to_string(),
                    label: "ID".to_string(),
                    path: None,
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "id".to_string(),
                    label: String::new(),
                    path: None,
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        let diagnostics =
            validate_view_document(&document).expect_err("invalid columns should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "view.table.column.key.duplicate" })
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "view.table.column.label")
        );
    }

    #[test]
    fn requirements_table_supports_attribute_path_columns() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::Requirements,
            title: "Requirements".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto {
                relations: vec!["owner".to_string()],
                direction: DiagramDirectionDto::Children,
                depth: 2,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            columns: vec![
                TableColumnSpecDto {
                    key: "id".to_string(),
                    label: "ID".to_string(),
                    path: Some("requirement_id".to_string()),
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "owner_name".to_string(),
                    label: "Owner Name".to_string(),
                    path: Some("owner.declared_name".to_string()),
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        let row = view.rows.first().expect("requirement row should render");
        assert!(
            row.cells
                .iter()
                .any(|cell| { cell.key == "id" && cell.value == "REQ-001" })
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| { cell.key == "owner_name" && cell.value == "Example" })
        );
    }

    #[test]
    fn requirements_table_supports_metadata_path_columns() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::Requirements,
            title: "Requirement Lifecycle".to_string(),
            description: None,
            root: Some("pkg.Example".to_string()),
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto {
                relations: vec!["owner".to_string()],
                direction: DiagramDirectionDto::Children,
                depth: 2,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            columns: vec![
                TableColumnSpecDto {
                    key: "status".to_string(),
                    label: "Status".to_string(),
                    path: Some("metadata[Review].status".to_string()),
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "owner".to_string(),
                    label: "Owner".to_string(),
                    path: Some("metadata[Review].owner".to_string()),
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "review_date".to_string(),
                    label: "Review Date".to_string(),
                    path: Some("metadata[Review].reviewDate".to_string()),
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        let row = view.rows.first().expect("requirement row should render");
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "status" && cell.value == "approved")
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "owner" && cell.value == "Foundation Team")
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "review_date" && cell.value == "2026-06-03")
        );
    }

    #[test]
    fn element_table_filters_by_type_and_subtypes() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::Elements,
            title: "Comments".to_string(),
            description: None,
            root: None,
            target_type: Some("Comment".to_string()),
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![
                TableColumnSpecDto {
                    key: "name".to_string(),
                    label: "Name".to_string(),
                    path: None,
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "body".to_string(),
                    label: "Body".to_string(),
                    path: Some("body".to_string()),
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "locale".to_string(),
                    label: "Locale".to_string(),
                    path: Some("locale".to_string()),
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        assert!(
            view.rows
                .iter()
                .any(|row| row.element == "comment.Example.Note")
        );
        assert!(
            view.rows
                .iter()
                .any(|row| row.element == "comment.Example.LocalizedNote")
        );
        assert!(
            view.rows
                .iter()
                .all(|row| row.element.contains("Comment") || row.element.contains("comment."))
        );
        assert!(
            view.available_columns
                .iter()
                .any(|column| column.key == "body")
        );
        assert!(
            view.available_columns
                .iter()
                .any(|column| column.key == "locale")
        );
    }

    #[test]
    fn model_elements_table_scopes_rows_to_containment_subtree() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::ModelElements,
            title: "Vehicle Subtree".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::ContainmentSubtree {
                root: "type.Example.Vehicle".to_string(),
            },
            row_type: Some(TableRowTypeDto {
                type_name: "Element".to_string(),
                include_subtypes: true,
            }),
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![
                TableColumnSpecDto {
                    key: "name".to_string(),
                    label: "Name".to_string(),
                    path: None,
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "parent_name".to_string(),
                    label: "Parent".to_string(),
                    path: Some("parent.name".to_string()),
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        assert_eq!(
            view.rows
                .iter()
                .map(|row| row.element.as_str())
                .collect::<Vec<_>>(),
            vec![
                "connection.Example.Vehicle.ControllerPower",
                "feature.Example.Vehicle.controller",
                "port.Example.Vehicle.power",
                "type.Example.Vehicle",
            ]
        );
        let controller = view
            .rows
            .iter()
            .find(|row| row.element == "feature.Example.Vehicle.controller")
            .expect("controller row should render");
        assert!(
            controller
                .cells
                .iter()
                .any(|cell| cell.key == "parent_name" && cell.value == "Vehicle")
        );
    }

    #[test]
    fn model_elements_table_supports_direct_navigation_columns() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::ModelElements,
            title: "Requirements Direct".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::ContainmentSubtree {
                root: "pkg.Example".to_string(),
            },
            row_type: Some(TableRowTypeDto {
                type_name: "Requirement".to_string(),
                include_subtypes: true,
            }),
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![
                TableColumnSpecDto {
                    key: "id".to_string(),
                    label: "ID".to_string(),
                    path: Some("requirement_id".to_string()),
                    expression: None,
                },
                TableColumnSpecDto {
                    key: "parent_name".to_string(),
                    label: "Parent".to_string(),
                    path: Some("parent.name".to_string()),
                    expression: None,
                },
            ],
            show_affordances: false,
        });

        let row = view.rows.first().expect("requirement row should render");
        assert_eq!(row.element, "req.Example.SafeStart");
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "id" && cell.value == "REQ-001")
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "parent_name" && cell.value == "Example")
        );
    }

    #[test]
    fn model_elements_table_supports_dsl_style_row_expressions() {
        let view = render_table_sample(TableSpecDto {
            version: 1,
            kind: TableKindDto::ModelElements,
            title: "Requirements Expressions".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::ContainmentSubtree {
                root: "pkg.Example".to_string(),
            },
            row_type: Some(TableRowTypeDto {
                type_name: "Requirement".to_string(),
                include_subtypes: true,
            }),
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: None,
            query: DiagramQueryOptionsDto::default(),
            columns: vec![
                TableColumnSpecDto {
                    key: "name".to_string(),
                    label: "Name".to_string(),
                    path: None,
                    expression: Some("row.name".to_string()),
                },
                TableColumnSpecDto {
                    key: "parent_name".to_string(),
                    label: "Parent".to_string(),
                    path: None,
                    expression: Some("row.parent.name".to_string()),
                },
                TableColumnSpecDto {
                    key: "status".to_string(),
                    label: "Status".to_string(),
                    path: None,
                    expression: Some("row.metadata[Review].status".to_string()),
                },
            ],
            show_affordances: false,
        });

        let row = view.rows.first().expect("requirement row should render");
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "name" && cell.value == "SafeStart")
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "parent_name" && cell.value == "Example")
        );
        assert!(
            row.cells
                .iter()
                .any(|cell| cell.key == "status" && cell.value == "approved")
        );
    }

    fn matrix_spec(preset: Option<MatrixPresetDto>) -> TableSpecDto {
        TableSpecDto {
            version: 1,
            kind: TableKindDto::RelationshipMatrix,
            title: "Matrix".to_string(),
            description: None,
            root: None,
            target_type: None,
            scope: TableScopeDto::WholeModel,
            row_type: None,
            column_scope: None,
            column_type: None,
            relations: Vec::new(),
            matrix_preset: preset,
            query: DiagramQueryOptionsDto {
                relations: Vec::new(),
                direction: DiagramDirectionDto::Children,
                depth: 3,
                include_libraries: false,
                include_user_model: true,
                max_nodes: 350,
                max_edges: 900,
            },
            columns: Vec::new(),
            show_affordances: false,
        }
    }

    fn matrix_row_ids(view: &TableViewDto) -> Vec<&str> {
        view.rows.iter().map(|row| row.element.as_str()).collect()
    }

    fn matrix_column_keys(view: &TableViewDto) -> Vec<&str> {
        view.columns
            .iter()
            .map(|column| column.key.as_str())
            .collect()
    }

    fn matrix_cell<'a>(view: &'a TableViewDto, row_id: &str, column_key: &str) -> &'a TableCellDto {
        view.rows
            .iter()
            .find(|row| row.id == row_id)
            .and_then(|row| row.cells.iter().find(|cell| cell.key == column_key))
            .unwrap_or_else(|| panic!("matrix cell {row_id} x {column_key} should exist"))
    }

    #[test]
    fn relationship_matrix_descriptor_is_registered_and_read_only() {
        assert_eq!(
            serde_json::to_value(TableKindDto::RelationshipMatrix)
                .expect("table kind should serialize"),
            json!("relationship_matrix")
        );
        assert!(list_table_kinds().contains(&TableKindDto::RelationshipMatrix));

        let descriptor = list_view_kinds()
            .into_iter()
            .find(|descriptor| descriptor.id == "table.relationship_matrix")
            .expect("relationship matrix descriptor is present");
        assert_eq!(descriptor.family, ViewFamilyDto::Table);
        assert_eq!(descriptor.root, ViewRootRequirementDto::Optional);
        assert_eq!(
            descriptor.scopes,
            vec![
                ViewScopeDto::WholeModel,
                ViewScopeDto::Subtree,
                ViewScopeDto::Explicit,
            ]
        );
        assert!(!descriptor.animatable);
        // Descriptor honesty: matrix editing is not wired yet.
        assert!(!descriptor.editable);
    }

    #[test]
    fn requirements_coverage_matrix_cites_satisfy_relationships() {
        let view = render_table_sample(matrix_spec(Some(MatrixPresetDto::RequirementsCoverage)));

        assert_eq!(matrix_row_ids(&view), vec!["req.Example.SafeStart"]);
        assert_eq!(
            matrix_column_keys(&view),
            vec![MATRIX_ROW_HEADER_KEY, "feature.Example.Vehicle.controller"]
        );
        assert_eq!(view.columns[0].label, "Requirement");
        assert_eq!(view.columns[1].label, "controller");

        let header = matrix_cell(&view, "req.Example.SafeStart", MATRIX_ROW_HEADER_KEY);
        assert_eq!(header.value, "SafeStart");
        assert_eq!(header.element, None);

        let cell = matrix_cell(
            &view,
            "req.Example.SafeStart",
            "feature.Example.Vehicle.controller",
        );
        assert_eq!(cell.value, "satisfy");
        assert_eq!(
            cell.element.as_deref(),
            Some("satisfy.Example.Vehicle.ControllerSafeStart")
        );
        assert!(view.warnings.is_empty(), "{:?}", view.warnings);
    }

    #[test]
    fn allocation_matrix_preset_maps_actions_onto_parts() {
        let mut document = view_fixture_document();
        document.elements.push(KirElement {
            id: "allocation.Example.ValidateToController".to_string(),
            kind: "AllocationUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), json!("ValidateToController")),
                ("owner".to_string(), json!("pkg.Example")),
                (
                    "source".to_string(),
                    json!("action.Example.Startup.Validate"),
                ),
                (
                    "target".to_string(),
                    json!("feature.Example.Vehicle.controller"),
                ),
            ]),
        });
        let graph = Graph::from_document(document).expect("sample graph should rebuild");
        let registry = MetamodelAttributeRegistry::build(&graph);

        let view = render_table(&graph, &registry, matrix_spec(Some(MatrixPresetDto::Allocation)))
            .expect("allocation matrix should render");

        assert_eq!(
            matrix_row_ids(&view),
            vec!["action.Example.Startup.Validate", "activity.Example.Startup"]
        );
        assert_eq!(
            matrix_column_keys(&view),
            vec![
                MATRIX_ROW_HEADER_KEY,
                "feature.Example.Vehicle.controller",
                "port.Example.Vehicle.power",
                "type.Example.Vehicle",
            ]
        );

        let cell = matrix_cell(
            &view,
            "action.Example.Startup.Validate",
            "feature.Example.Vehicle.controller",
        );
        assert_eq!(cell.value, "allocate");
        assert_eq!(
            cell.element.as_deref(),
            Some("allocation.Example.ValidateToController")
        );

        let filled_cells = view
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().skip(1))
            .filter(|cell| !cell.value.is_empty())
            .count();
        assert_eq!(filled_cells, 1);
    }

    #[test]
    fn dsm_matrix_mirrors_part_axis_and_cites_connections() {
        let view = render_table_sample(matrix_spec(Some(MatrixPresetDto::Dsm)));

        let part_ids = vec![
            "feature.Example.Vehicle.controller",
            "port.Example.Vehicle.power",
            "type.Example.Vehicle",
        ];
        assert_eq!(matrix_row_ids(&view), part_ids);
        let mut expected_columns = vec![MATRIX_ROW_HEADER_KEY];
        expected_columns.extend(part_ids.iter().copied());
        assert_eq!(matrix_column_keys(&view), expected_columns);

        let forward = matrix_cell(
            &view,
            "feature.Example.Vehicle.controller",
            "port.Example.Vehicle.power",
        );
        assert_eq!(forward.value, "connection");
        assert_eq!(
            forward.element.as_deref(),
            Some("connection.Example.Vehicle.ControllerPower")
        );

        // The undirected mirror of the same connection is filled too.
        let mirrored = matrix_cell(
            &view,
            "port.Example.Vehicle.power",
            "feature.Example.Vehicle.controller",
        );
        assert_eq!(mirrored.value, "connection");

        // The diagonal and unrelated pairs stay empty.
        assert!(
            matrix_cell(&view, "type.Example.Vehicle", "type.Example.Vehicle")
                .value
                .is_empty()
        );
        assert!(
            matrix_cell(
                &view,
                "type.Example.Vehicle",
                "feature.Example.Vehicle.controller"
            )
            .value
            .is_empty()
        );
    }

    #[test]
    fn matrix_axis_and_cell_limits_truncate_with_warnings() {
        let mut spec = matrix_spec(Some(MatrixPresetDto::Dsm));
        spec.query.max_nodes = 2;
        spec.query.max_edges = 1;
        let view = render_table_sample(spec);

        assert_eq!(view.rows.len(), 2);
        assert!(
            view.warnings
                .iter()
                .any(|warning| warning.contains("Matrix row limit reached")),
            "{:?}",
            view.warnings
        );
        assert!(
            view.warnings
                .iter()
                .any(|warning| warning.contains("Matrix cell limit reached")),
            "{:?}",
            view.warnings
        );
        let filled_cells = view
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().skip(1))
            .filter(|cell| !cell.value.is_empty())
            .count();
        assert_eq!(filled_cells, 1);
    }

    /// DA-4 acceptance: a synthetic document mirroring
    /// `mercurio-examples/desktop/vehicle-mass-compliance`'s requirement/
    /// satisfy shape (one requirement definition satisfied by the baseline
    /// vehicle part, plus an untraced requirement pinning the empty row).
    #[test]
    fn coverage_matrix_matches_vehicle_mass_compliance_requirement_shape() {
        fn element(
            id: &str,
            kind: &str,
            properties: BTreeMap<String, Value>,
        ) -> KirElement {
            KirElement {
                id: id.to_string(),
                kind: kind.to_string(),
                layer: 2,
                properties,
            }
        }

        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element(
                    "pkg.VehicleMassCompliance",
                    "Package",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("VehicleMassCompliance")),
                        (
                            "qualified_name".to_string(),
                            json!("VehicleMassCompliance"),
                        ),
                    ]),
                ),
                element(
                    "type.VehicleMassCompliance.Vehicle",
                    "PartDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        ("owner".to_string(), json!("pkg.VehicleMassCompliance")),
                    ]),
                ),
                element(
                    "req.VehicleMassCompliance.VehicleMassRequirement",
                    "RequirementDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("VehicleMassRequirement")),
                        ("owner".to_string(), json!("pkg.VehicleMassCompliance")),
                    ]),
                ),
                element(
                    "req.VehicleMassCompliance.UntracedRequirement",
                    "RequirementDefinition",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("UntracedRequirement")),
                        ("owner".to_string(), json!("pkg.VehicleMassCompliance")),
                    ]),
                ),
                element(
                    "feature.VehicleMassCompliance.baselineVehicle",
                    "PartUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("baselineVehicle")),
                        ("owner".to_string(), json!("pkg.VehicleMassCompliance")),
                        (
                            "satisfy".to_string(),
                            json!(["req.VehicleMassCompliance.VehicleMassRequirement"]),
                        ),
                    ]),
                ),
                element(
                    "satisfy.VehicleMassCompliance.BaselineVehicleMass",
                    "SatisfyRequirementUsage",
                    BTreeMap::from([
                        ("declared_name".to_string(), json!("BaselineVehicleMass")),
                        ("owner".to_string(), json!("pkg.VehicleMassCompliance")),
                        (
                            "source".to_string(),
                            json!("feature.VehicleMassCompliance.baselineVehicle"),
                        ),
                        (
                            "target".to_string(),
                            json!("req.VehicleMassCompliance.VehicleMassRequirement"),
                        ),
                    ]),
                ),
            ],
        };
        let graph = Graph::from_document(document).expect("coverage graph should build");
        let registry = MetamodelAttributeRegistry::build(&graph);

        let view = render_table(
            &graph,
            &registry,
            matrix_spec(Some(MatrixPresetDto::RequirementsCoverage)),
        )
        .expect("coverage matrix should render");

        assert_eq!(
            matrix_row_ids(&view),
            vec![
                "req.VehicleMassCompliance.UntracedRequirement",
                "req.VehicleMassCompliance.VehicleMassRequirement",
            ]
        );
        assert_eq!(
            matrix_column_keys(&view),
            vec![
                MATRIX_ROW_HEADER_KEY,
                "feature.VehicleMassCompliance.baselineVehicle",
            ]
        );

        let satisfied = matrix_cell(
            &view,
            "req.VehicleMassCompliance.VehicleMassRequirement",
            "feature.VehicleMassCompliance.baselineVehicle",
        );
        assert_eq!(satisfied.value, "satisfy");
        assert_eq!(
            satisfied.element.as_deref(),
            Some("satisfy.VehicleMassCompliance.BaselineVehicleMass")
        );

        let untraced = matrix_cell(
            &view,
            "req.VehicleMassCompliance.UntracedRequirement",
            "feature.VehicleMassCompliance.baselineVehicle",
        );
        assert!(untraced.value.is_empty());
        assert_eq!(untraced.element, None);
    }

    #[test]
    fn view_catalog_offers_content_guarded_matrices() {
        let (graph, _) = sample_graph();
        let catalog = view_catalog(&graph);
        let matrix_labels = catalog
            .entries
            .iter()
            .filter(|entry| entry.kind == "table.relationship_matrix")
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>();
        // The fixture has requirements and a part connection but no
        // allocation content, so the allocation matrix is not offered.
        assert_eq!(
            matrix_labels,
            vec![
                "Matrix: Design Structure (DSM)",
                "Matrix: Requirements Coverage",
            ]
        );

        let mut document = view_fixture_document();
        document.elements.push(KirElement {
            id: "allocation.Example.ValidateToController".to_string(),
            kind: "AllocationUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("owner".to_string(), json!("pkg.Example")),
                (
                    "source".to_string(),
                    json!("action.Example.Startup.Validate"),
                ),
                (
                    "target".to_string(),
                    json!("feature.Example.Vehicle.controller"),
                ),
            ]),
        });
        let graph = Graph::from_document(document).expect("sample graph should rebuild");
        let catalog = view_catalog(&graph);
        assert!(catalog.entries.iter().any(|entry| {
            entry.kind == "table.relationship_matrix" && entry.label == "Matrix: Allocation"
        }));
    }

    #[test]
    fn matrix_spec_round_trips_and_legacy_table_json_parses() {
        let mut spec = matrix_spec(Some(MatrixPresetDto::Dsm));
        spec.relations = vec!["satisfy".to_string()];
        spec.column_scope = Some(TableScopeDto::ContainmentSubtree {
            root: "pkg.Example".to_string(),
        });
        spec.column_type = Some(TableRowTypeDto {
            type_name: "PartUsage".to_string(),
            include_subtypes: true,
        });
        let document = ViewDocumentDto::table(spec);

        validate_view_document(&document).expect("matrix document should validate");
        let json = serde_json::to_value(&document).expect("matrix document should serialize");
        assert_eq!(json["kind"], json!("table"));
        assert_eq!(json["table"]["kind"], json!("relationship_matrix"));
        assert_eq!(json["table"]["matrix_preset"], json!("dsm"));
        assert_eq!(json["table"]["relations"], json!(["satisfy"]));
        assert_eq!(
            json["table"]["column_scope"]["kind"],
            json!("containment_subtree")
        );
        assert_eq!(json["table"]["column_type"]["type"], json!("PartUsage"));
        let decoded: ViewDocumentDto =
            serde_json::from_value(json).expect("matrix document should deserialize");
        assert_eq!(decoded, document);

        // Pre-DA-4 table JSON (no matrix fields) still parses to defaults.
        let legacy: TableSpecDto = serde_json::from_value(json!({
            "version": 1,
            "kind": "requirements",
            "title": "Requirements"
        }))
        .expect("legacy table spec should parse");
        assert!(legacy.relations.is_empty());
        assert!(legacy.matrix_preset.is_none());
        assert!(legacy.column_scope.is_none());
        assert!(legacy.column_type.is_none());
    }
}

fn default_true() -> bool {
    true
}

fn diagram_kind_name(kind: &DiagramKindDto) -> &'static str {
    match kind {
        DiagramKindDto::Structure => "structure",
        DiagramKindDto::Bdd => "bdd",
        DiagramKindDto::InternalBlock => "internal_block",
        DiagramKindDto::Requirement => "requirement",
        DiagramKindDto::Activity => "activity",
        DiagramKindDto::StateMachine => "state_machine",
        DiagramKindDto::PackageTree => "package_tree",
        DiagramKindDto::CompositionGraph => "composition_graph",
        DiagramKindDto::ReferenceGraph => "reference_graph",
        DiagramKindDto::DependencyGraph => "dependency_graph",
        DiagramKindDto::MetatypeInstanceMap => "metatype_instance_map",
        DiagramKindDto::ImpactView => "impact_view",
        DiagramKindDto::PropertyInheritance => "property_inheritance",
        DiagramKindDto::ValidationView => "validation_view",
    }
}

fn table_kind_name(kind: &TableKindDto) -> &'static str {
    match kind {
        TableKindDto::ModelElements => "model_elements",
        TableKindDto::Elements => "elements",
        TableKindDto::Requirements => "requirements",
        TableKindDto::RelationshipMatrix => "relationship_matrix",
    }
}

fn model_view_kind_name(kind: &ModelViewKindDto) -> &'static str {
    match kind {
        ModelViewKindDto::Metadata => "model.metadata",
        ModelViewKindDto::Graph => "model.graph",
        ModelViewKindDto::Search => "model.search",
        ModelViewKindDto::ElementDetails => "model.element_details",
        ModelViewKindDto::LibraryTree => "model.library_tree",
        ModelViewKindDto::ModelExplorer => "explorer.model",
        ModelViewKindDto::MetatypeExplorer => "explorer.metatype",
    }
}

fn value_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
