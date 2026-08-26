use std::collections::{BTreeMap, BTreeSet};

use crate::kir::{KirDocument, KirElement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Concept, Diagnostic, SemanticCompileReport, SemanticCompileStatus, TextRange, ast::SourceSpan,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDocument {
    pub source_name: String,
    pub revision: u64,
    pub text: String,
}

impl SourceDocument {
    pub fn new(source_name: impl Into<String>, revision: u64, text: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            revision,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolDescriptor {
    pub qualified_name: String,
    pub concept: Concept,
    pub span: TextRange,
    pub element_id: String,
    pub source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSymbols {
    pub source_name: String,
    pub revision: u64,
    pub symbols: Vec<SymbolDescriptor>,
}

impl DocumentSymbols {
    pub fn empty(source_name: impl Into<String>, revision: u64) -> Self {
        Self {
            source_name: source_name.into(),
            revision,
            symbols: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceDescriptor {
    pub source_name: String,
    pub span: TextRange,
    pub target_element_id: String,
    pub property: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_source_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_element_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementAtPosition {
    pub element_id: String,
    pub source_name: String,
    pub span: TextRange,
}

pub trait ScopeProvider {
    fn visible_symbols(&self, source_name: &str, byte_offset: usize) -> Vec<SymbolDescriptor>;
    fn resolve_reference(
        &self,
        source_name: &str,
        byte_offset: usize,
    ) -> Option<ReferenceDescriptor>;
    fn references(&self, source_name: &str) -> Vec<ReferenceDescriptor>;
    fn element_at(&self, source_name: &str, byte_offset: usize) -> Option<ElementAtPosition>;
}

#[derive(Debug, Clone)]
pub struct LanguageAnalysis {
    pub status: SemanticCompileStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub document: Option<KirDocument>,
    pub symbols: DocumentSymbols,
    pub references: Vec<ReferenceDescriptor>,
    pub visible_symbols: Vec<SymbolDescriptor>,
}

impl ScopeProvider for LanguageAnalysis {
    fn visible_symbols(&self, _source_name: &str, _byte_offset: usize) -> Vec<SymbolDescriptor> {
        self.visible_symbols.clone()
    }

    fn resolve_reference(
        &self,
        source_name: &str,
        byte_offset: usize,
    ) -> Option<ReferenceDescriptor> {
        self.references
            .iter()
            .filter(|reference| {
                reference.source_name == source_name && contains_offset(reference.span, byte_offset)
            })
            .min_by_key(|reference| {
                (
                    reference
                        .span
                        .end_byte
                        .saturating_sub(reference.span.start_byte),
                    reference_priority(&reference.property),
                )
            })
            .cloned()
    }

    fn references(&self, source_name: &str) -> Vec<ReferenceDescriptor> {
        self.references
            .iter()
            .filter(|reference| reference.source_name == source_name)
            .cloned()
            .collect()
    }

    fn element_at(&self, source_name: &str, byte_offset: usize) -> Option<ElementAtPosition> {
        self.symbols
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.source_name == source_name && contains_offset(symbol.span, byte_offset)
            })
            .min_by_key(|symbol| symbol.span.end_byte.saturating_sub(symbol.span.start_byte))
            .map(|symbol| ElementAtPosition {
                element_id: symbol.element_id.clone(),
                source_name: symbol.source_name.clone(),
                span: symbol.span,
            })
    }
}

pub fn analysis_from_compile_report(
    source: &str,
    source_name: &str,
    revision: u64,
    library_context: &KirDocument,
    report: SemanticCompileReport<KirDocument>,
) -> LanguageAnalysis {
    let symbols = report
        .document
        .as_ref()
        .map(|document| document_symbols(source, source_name, revision, document))
        .unwrap_or_else(|| DocumentSymbols::empty(source_name, revision));
    let references = report
        .document
        .as_ref()
        .map(|document| document_references(source, source_name, document, library_context))
        .unwrap_or_default();
    let mut visible_symbols = symbols.symbols.clone();
    visible_symbols
        .extend(document_symbols("", "mercurio-stdlib://library", 0, library_context).symbols);
    LanguageAnalysis {
        status: report.status,
        diagnostics: report.diagnostics,
        document: report.document,
        symbols,
        references,
        visible_symbols,
    }
}

pub fn document_symbols(
    source: &str,
    source_name: &str,
    revision: u64,
    document: &KirDocument,
) -> DocumentSymbols {
    let mut symbols = document
        .elements
        .iter()
        .filter_map(|element| symbol_from_element(source, source_name, element))
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| {
        left.qualified_name
            .cmp(&right.qualified_name)
            .then_with(|| left.element_id.cmp(&right.element_id))
    });
    DocumentSymbols {
        source_name: source_name.to_string(),
        revision,
        symbols,
    }
}

fn symbol_from_element(
    source: &str,
    default_source_name: &str,
    element: &KirElement,
) -> Option<SymbolDescriptor> {
    let qualified_name = element
        .properties
        .get("qualified_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            element
                .id
                .split_once('.')
                .map(|(_, qualified)| qualified.to_string())
        })
        .or_else(|| {
            element
                .properties
                .get("declared_name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })?;
    let (source_name, span) = element_source_span(element)
        .map(|(file, span)| {
            let text_range = source_span_to_text_range(
                (file == default_source_name)
                    .then_some(source)
                    .unwrap_or(""),
                &span,
            );
            (file, text_range)
        })
        .unwrap_or_else(|| {
            (
                default_source_name.to_string(),
                TextRange::new(0, source.len()),
            )
        });
    let concept = element
        .properties
        .get("metatype")
        .and_then(Value::as_str)
        .unwrap_or(element.kind.as_str());
    Some(SymbolDescriptor {
        qualified_name,
        concept: Concept::new(concept),
        span,
        element_id: element.id.clone(),
        source_name,
    })
}

fn document_references(
    source: &str,
    source_name: &str,
    document: &KirDocument,
    library_context: &KirDocument,
) -> Vec<ReferenceDescriptor> {
    let local_ids = document
        .elements
        .iter()
        .map(|element| element.id.as_str())
        .collect::<BTreeSet<_>>();
    let library_sources = library_context
        .elements
        .iter()
        .map(|element| {
            let source = element_source_span(element)
                .map(|(file, _)| file)
                .unwrap_or_else(|| "mercurio-stdlib://library".to_string());
            (element.id.as_str(), source)
        })
        .collect::<BTreeMap<_, _>>();
    let mut references = Vec::new();
    for element in &document.elements {
        let span = element_source_span(element)
            .map(|(_, span)| source_span_to_text_range(source, &span))
            .unwrap_or_else(|| TextRange::new(0, source.len()));
        for (property, value) in &element.properties {
            if is_non_reference_property(property) {
                continue;
            }
            collect_reference_values(
                value,
                property,
                &local_ids,
                &library_sources,
                source_name,
                span,
                &element.id,
                &mut references,
            );
        }
    }
    references.sort_by(|left, right| {
        left.span
            .start_byte
            .cmp(&right.span.start_byte)
            .then_with(|| left.target_element_id.cmp(&right.target_element_id))
    });
    references.dedup();
    references
}

fn collect_reference_values(
    value: &Value,
    property: &str,
    local_ids: &BTreeSet<&str>,
    library_sources: &BTreeMap<&str, String>,
    source_name: &str,
    span: TextRange,
    owner_element_id: &str,
    output: &mut Vec<ReferenceDescriptor>,
) {
    match value {
        Value::String(candidate) => {
            if local_ids.contains(candidate.as_str())
                || library_sources.contains_key(candidate.as_str())
            {
                output.push(ReferenceDescriptor {
                    source_name: source_name.to_string(),
                    span,
                    target_element_id: candidate.clone(),
                    property: property.to_string(),
                    target_source_name: library_sources.get(candidate.as_str()).cloned(),
                    owner_element_id: Some(owner_element_id.to_string()),
                });
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_reference_values(
                    value,
                    property,
                    local_ids,
                    library_sources,
                    source_name,
                    span,
                    owner_element_id,
                    output,
                );
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_reference_values(
                    value,
                    property,
                    local_ids,
                    library_sources,
                    source_name,
                    span,
                    owner_element_id,
                    output,
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_non_reference_property(property: &str) -> bool {
    matches!(
        property,
        "declared_name"
            | "name"
            | "qualified_name"
            | "metatype"
            | "source_span"
            | "metadata"
            | "body"
            | "doc"
    )
}

fn element_source_span(element: &KirElement) -> Option<(String, SourceSpan)> {
    let direct = element.properties.get("source_span");
    let metadata = element
        .properties
        .get("metadata")
        .and_then(Value::as_object);
    let value = direct.or_else(|| metadata.and_then(|values| values.get("source_span")))?;
    let file = value
        .get("file")
        .and_then(Value::as_str)
        .or_else(|| {
            metadata
                .and_then(|values| values.get("source_file"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_string();
    Some((
        file,
        SourceSpan {
            start_line: value.get("start_line")?.as_u64()? as usize,
            start_col: value.get("start_col")?.as_u64()? as usize,
            end_line: value.get("end_line")?.as_u64()? as usize,
            end_col: value.get("end_col")?.as_u64()? as usize,
        },
    ))
}

pub fn source_span_to_text_range(source: &str, span: &SourceSpan) -> TextRange {
    TextRange::new(
        line_col_to_byte(source, span.start_line, span.start_col),
        line_col_to_byte(source, span.end_line, span.end_col),
    )
}

pub fn line_col_to_byte(source: &str, one_based_line: usize, one_based_col: usize) -> usize {
    let target_line = one_based_line.saturating_sub(1);
    let target_col = one_based_col.saturating_sub(1);
    let mut offset = 0;
    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        if line_index == target_line {
            return offset
                + line
                    .char_indices()
                    .map(|(index, _)| index)
                    .nth(target_col)
                    .unwrap_or_else(|| line.trim_end_matches(['\r', '\n']).len());
        }
        offset += line.len();
    }
    source.len()
}

fn reference_priority(property: &str) -> u8 {
    match property {
        "type" | "reference_target" => 0,
        "target" | "imported_namespace" => 1,
        "specializes" | "redefines" | "subsets" => 2,
        _ => 10,
    }
}
fn contains_offset(range: TextRange, offset: usize) -> bool {
    range.start_byte <= offset && offset <= range.end_byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_symbols_and_compiler_resolved_references() {
        let source = "package Demo { part def Vehicle; part car : Vehicle; }";
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                element("type.Demo.Vehicle", "Demo.Vehicle", 1, 16, 1, 33, None),
                element(
                    "feature.Demo.car",
                    "Demo.car",
                    1,
                    34,
                    1,
                    53,
                    Some(("type", "type.Demo.Vehicle")),
                ),
            ],
        };
        let report = SemanticCompileReport {
            status: SemanticCompileStatus::Ok,
            diagnostics: Vec::new(),
            document: Some(document),
        };
        let analysis = analysis_from_compile_report(
            source,
            "demo.sysml",
            7,
            &KirDocument {
                metadata: BTreeMap::new(),
                elements: Vec::new(),
            },
            report,
        );
        assert_eq!(analysis.symbols.revision, 7);
        assert_eq!(analysis.symbols.symbols.len(), 2);
        assert_eq!(analysis.references.len(), 1);
        assert_eq!(
            analysis.references[0].target_element_id,
            "type.Demo.Vehicle"
        );
    }

    fn element(
        id: &str,
        qualified_name: &str,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        reference: Option<(&str, &str)>,
    ) -> KirElement {
        let mut properties = BTreeMap::from([
            ("qualified_name".to_string(), json!(qualified_name)),
            (
                "source_span".to_string(),
                json!({
                    "file": "demo.sysml",
                    "start_line": start_line,
                    "start_col": start_col,
                    "end_line": end_line,
                    "end_col": end_col
                }),
            ),
        ]);
        if let Some((name, target)) = reference {
            properties.insert(name.to_string(), json!(target));
        }
        KirElement {
            id: id.to_string(),
            kind: "PartDefinition".to_string(),
            layer: 2,
            properties,
        }
    }
}
