use std::path::Path;

use crate::language_contracts::{CompileContext, LanguageRegistry};

use crate::authoring::frontend::diagnostics::Diagnostic;
use crate::kir::{KirDocument, KirError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    pub path: String,
    pub content: String,
}

impl SourceDocument {
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}

pub fn compile_source_documents_with_registry(
    source_documents: Vec<SourceDocument>,
    library_context: &KirDocument,
    registry: &LanguageRegistry,
) -> Result<KirDocument, KirError> {
    let documents = source_documents
        .iter()
        .map(|source| compile_source_document_with_registry(source, library_context, registry))
        .collect::<Result<Vec<_>, _>>()?;

    KirDocument::merge(documents)
}

pub fn compile_source_document_with_registry(
    source: &SourceDocument,
    library_context: &KirDocument,
    registry: &LanguageRegistry,
) -> Result<KirDocument, KirError> {
    let service = registry
        .service_for_path(Path::new(&source.path))
        .ok_or_else(|| {
            KirError::Frontend(format!(
                "no registered language service for `{}`",
                source.path
            ))
        })?;
    let report = service.compile(
        &source.content,
        CompileContext {
            source_name: &source.path,
            library_context,
        },
    );

    report.document.ok_or_else(|| {
        let details = report
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        KirError::Frontend(format!(
            "language service `{}` did not produce KIR for `{}`{}",
            service.language_id(),
            source.path,
            if details.is_empty() {
                String::new()
            } else {
                format!(": {details}")
            }
        ))
    })
}

#[cfg(not(any(test, feature = "toy-parser")))]
pub fn compile_source_documents(
    _source_documents: Vec<SourceDocument>,
    _library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    Err(KirError::Frontend(
        "source compilation requires a registered language service".to_string(),
    ))
}

#[cfg(any(test, feature = "toy-parser"))]
pub fn compile_source_documents(
    source_documents: Vec<SourceDocument>,
    _library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    crate::authoring::test_support::toy_language::compile_documents(source_documents)
}

pub fn parse_source_module(_path: &str, _content: &str) -> Result<(), Diagnostic> {
    Err(Diagnostic::new(
        "source parsing requires a language-specific parser".to_string(),
        None,
    ))
}
