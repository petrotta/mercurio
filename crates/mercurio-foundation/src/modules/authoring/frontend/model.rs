use crate::authoring::frontend::ast::ParsedModule;
use crate::authoring::frontend::diagnostics::Diagnostic;
use crate::kir::KirDocument;

pub use crate::language_contracts::{SemanticCompileReport, SemanticCompileStatus};

#[cfg(not(test))]
pub fn compile_model_text(
    _input: &str,
    source_name: &str,
    _library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    Err(Diagnostic::new(
        format!("source compilation requires a language service for `{source_name}`"),
        None,
    ))
}

#[cfg(test)]
pub fn compile_model_text(
    input: &str,
    source_name: &str,
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    crate::authoring::source_set::compile_source_document_with_registry(
        &crate::authoring::source_set::SourceDocument::new(source_name, input),
        library_context,
        &crate::authoring::test_support::toy_language::registry(),
    )
    .map_err(|err| Diagnostic::new(err.to_string(), None))
}

#[cfg(not(test))]
pub fn compile_model_text_with_context_report(
    _input: &str,
    source_name: &str,
    _context_modules: &[ParsedModule],
    _library_context: &KirDocument,
) -> SemanticCompileReport<KirDocument> {
    SemanticCompileReport {
        status: SemanticCompileStatus::Failed,
        diagnostics: vec![Diagnostic::new(
            format!("source compilation requires a language service for `{source_name}`"),
            None,
        )],
        document: None,
    }
}

#[cfg(test)]
pub fn compile_model_text_with_context_report(
    input: &str,
    source_name: &str,
    _context_modules: &[ParsedModule],
    library_context: &KirDocument,
) -> SemanticCompileReport<KirDocument> {
    match compile_model_text(input, source_name, library_context) {
        Ok(document) => SemanticCompileReport {
            status: SemanticCompileStatus::Ok,
            diagnostics: Vec::new(),
            document: Some(document),
        },
        Err(diagnostic) => SemanticCompileReport {
            status: SemanticCompileStatus::Failed,
            diagnostics: vec![diagnostic],
            document: None,
        },
    }
}
