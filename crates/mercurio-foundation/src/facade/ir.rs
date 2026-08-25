pub use crate::kir::{
    Diagnostic, DiagnosticKind, KIR_PROP_MEMBERS, KIR_PROP_NAME, KIR_PROP_OWNER,
    KIR_PROP_SPECIALIZES, KIR_PROP_TYPE, KIR_SCHEMA_VERSION, KirDocument, KirElement, KirError,
    KirFieldKind, KirFieldRegistry, KirFieldSpec, REPRESENTATIVE_KIR_JSON, Severity,
};

use std::path::Path;

use crate::language_contracts::LanguageRegistry;

pub fn load_model_stack(model_path: &Path) -> Result<KirDocument, KirError> {
    KirDocument::from_path(model_path)
}

pub fn load_model_stack_with_registry(
    model_path: &Path,
    library_context: &KirDocument,
    registry: &LanguageRegistry,
) -> Result<KirDocument, KirError> {
    if is_kir_json(model_path) {
        return KirDocument::from_path(model_path);
    }

    let source = std::fs::read_to_string(model_path)?;
    let report = registry.compile_path(model_path, &source, library_context);
    let document = report.document.ok_or_else(|| {
        let details = report
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        KirError::Frontend(if details.is_empty() {
            format!(
                "registered language service did not produce KIR for `{}`",
                model_path.display()
            )
        } else {
            format!(
                "registered language service did not produce KIR for `{}`: {details}",
                model_path.display()
            )
        })
    })?;
    KirDocument::merge([library_context.clone(), document])
}

fn is_kir_json(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|file_name| file_name.ends_with(".kir.json"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn load_model_stack_accepts_kir_documents() {
        let document = super::load_model_stack(&crate::paths::repo_path(
            "resources/foundation/empty.kir.json",
        ));

        assert!(document.is_ok());
    }
}
