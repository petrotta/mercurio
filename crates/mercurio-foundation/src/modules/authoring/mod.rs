pub mod authoring;
pub mod frontend;
pub mod outline;
pub mod source_set;
pub mod syntax_compare;

#[doc(hidden)]
pub mod test_support;

pub use authoring::{
    Alias, AttributeWritePolicy, AuthoringError, AuthoringModule, AuthoringProject,
    AuthoringRenderProfile, ContainerSelector, Declaration, Definition, Import, Mutation,
    MutationResult, Package, QualifiedName, RenderedSpan, SemanticAttribute, SemanticEdit, Usage,
    ValidationReport, WriteBackMode, WriteBackResult, create_empty_model,
    load_authoring_project_from_kir, load_authoring_project_from_model,
    textual_model_authoring_render_profile,
};
pub use frontend::pilot::{
    PilotDocumentationBlock, PilotExportDocument, PilotExportElement, PilotExportRelationship,
    PilotImportError, PilotSource, load_pilot_export, normalize_pilot_export,
    normalize_pilot_export_for_compare,
};
pub use outline::{
    EditorOutlineKey, EditorOutlineNodeDto, build_editor_outline,
    build_editor_outline_index_for_graph, build_semantic_editor_outline_from_document,
};
pub use source_set::{
    SourceDocument, compile_source_document_with_registry, compile_source_documents,
    compile_source_documents_with_registry, parse_source_module,
};
pub use syntax_compare::{
    SyntaxComparisonReport, SyntaxNodeMismatch, SyntaxSnapshot, SyntaxSnapshotNode,
    SyntaxSourceSpan, build_rust_syntax_snapshot, compare_syntax_snapshots,
};
