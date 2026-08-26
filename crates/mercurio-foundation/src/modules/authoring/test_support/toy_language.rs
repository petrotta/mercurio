use std::collections::BTreeMap;

use crate::language_contracts::{
    CompileContext, LanguageRegistry, LanguageService, SemanticCompileReport, SemanticCompileStatus,
};
use serde_json::json;

use crate::authoring::frontend::diagnostics::Diagnostic;
use crate::authoring::source_set::SourceDocument;
use crate::kir::{KIR_SCHEMA_VERSION, KirDocument, KirElement, KirError};

pub const TOY_LANGUAGE_ID: &str = "toy";
pub const TOY_EXTENSIONS: &[&str] = &["toy", "model"];

pub struct ToyLanguageService;

impl LanguageService for ToyLanguageService {
    fn language_id(&self) -> &str {
        TOY_LANGUAGE_ID
    }

    fn extensions(&self) -> &[&str] {
        TOY_EXTENSIONS
    }

    fn compile(
        &self,
        source: &str,
        context: CompileContext<'_>,
    ) -> SemanticCompileReport<KirDocument> {
        compile_report(source, context.source_name)
    }
}

pub fn registry() -> LanguageRegistry {
    let mut registry = LanguageRegistry::new();
    registry.register(ToyLanguageService);
    registry
}

pub fn compile_documents(source_documents: Vec<SourceDocument>) -> Result<KirDocument, KirError> {
    let documents = source_documents
        .iter()
        .map(|source| compile_document(source))
        .collect::<Result<Vec<_>, _>>()?;
    KirDocument::merge(documents)
}

pub fn compile_document(source: &SourceDocument) -> Result<KirDocument, KirError> {
    match compile_report(&source.content, &source.path) {
        SemanticCompileReport {
            document: Some(document),
            ..
        } => Ok(document),
        SemanticCompileReport { diagnostics, .. } => Err(KirError::Frontend(
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )),
    }
}

pub fn compile_report(source: &str, source_name: &str) -> SemanticCompileReport<KirDocument> {
    match parse(source, source_name) {
        Ok(model) => SemanticCompileReport {
            status: SemanticCompileStatus::Ok,
            diagnostics: Vec::new(),
            document: Some(model.into_kir()),
        },
        Err(diagnostics) => SemanticCompileReport {
            status: SemanticCompileStatus::Failed,
            diagnostics,
            document: None,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToyModel {
    package: String,
    source_name: String,
    declarations: Vec<ToyDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ToyDeclaration {
    Type { name: String },
    Definition { name: String },
    FeatureDefinition { name: String },
    Usage { name: String, type_name: String },
}

impl ToyModel {
    fn into_kir(self) -> KirDocument {
        let member_ids = self
            .declarations
            .iter()
            .map(|declaration| declaration.element_id(&self.package))
            .collect::<Vec<_>>();
        let mut elements = vec![KirElement {
            id: format!("pkg.{}", self.package),
            kind: "model.Package".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                ("qualified_name".to_string(), json!(self.package)),
                ("declared_name".to_string(), json!(self.package)),
                ("members".to_string(), json!(member_ids)),
                ("source_file".to_string(), json!(self.source_name)),
            ]),
        }];

        elements.extend(
            self.declarations
                .into_iter()
                .map(|declaration| declaration.into_kir(&self.package, &self.source_name)),
        );

        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements,
        }
    }
}

impl ToyDeclaration {
    fn element_id(&self, package: &str) -> String {
        match self {
            ToyDeclaration::Type { name } => format!("type.{package}.{name}"),
            ToyDeclaration::Definition { name } => format!("type.{package}.{name}"),
            ToyDeclaration::FeatureDefinition { name } => format!("type.{package}.{name}"),
            ToyDeclaration::Usage { name, .. } => format!("part.{package}.{name}"),
        }
    }

    fn into_kir(self, package: &str, source_name: &str) -> KirElement {
        match self {
            ToyDeclaration::Type { name } => KirElement {
                id: format!("type.{package}.{name}"),
                kind: "model.Type".to_string(),
                layer: 2,
                properties: common_properties(package, &name, source_name),
            },
            ToyDeclaration::Definition { name } => KirElement {
                id: format!("type.{package}.{name}"),
                kind: "model.PartDefinition".to_string(),
                layer: 2,
                properties: common_properties(package, &name, source_name),
            },
            ToyDeclaration::FeatureDefinition { name } => KirElement {
                id: format!("type.{package}.{name}"),
                kind: "model.FeatureDefinition".to_string(),
                layer: 2,
                properties: common_properties(package, &name, source_name),
            },
            ToyDeclaration::Usage { name, type_name } => {
                let mut properties = common_properties(package, &name, source_name);
                properties.insert(
                    "type".to_string(),
                    json!(qualified_type_id(package, &type_name)),
                );
                KirElement {
                    id: format!("part.{package}.{name}"),
                    kind: "model.PartUsage".to_string(),
                    layer: 2,
                    properties,
                }
            }
        }
    }
}

fn common_properties(
    package: &str,
    name: &str,
    source_name: &str,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        (
            "qualified_name".to_string(),
            json!(format!("{package}.{name}")),
        ),
        ("declared_name".to_string(), json!(name)),
        ("owner".to_string(), json!(format!("pkg.{package}"))),
        ("source_file".to_string(), json!(source_name)),
    ])
}

fn qualified_type_id(package: &str, type_name: &str) -> String {
    if type_name.contains('.') {
        format!("type.{type_name}")
    } else {
        format!("type.{package}.{type_name}")
    }
}

fn parse(source: &str, source_name: &str) -> Result<ToyModel, Vec<Diagnostic>> {
    let tokens = tokenize(source);
    if tokens.is_empty() {
        return Err(vec![Diagnostic::new("toy source is empty", None)]);
    }

    let mut index = 0;
    let mut package = None;
    let mut declarations = Vec::new();
    let mut diagnostics = Vec::new();

    while index < tokens.len() {
        match tokens[index].as_str() {
            "package" => {
                index += 1;
                match tokens.get(index) {
                    Some(name) if is_identifier(name) => {
                        package = Some(name.clone());
                        index += 1;
                    }
                    _ => {
                        diagnostics.push(Diagnostic::new("toy package requires a name", None));
                        break;
                    }
                }
            }
            "type" => {
                index += 1;
                match tokens.get(index) {
                    Some(name) if is_identifier(name) => {
                        declarations.push(ToyDeclaration::Type { name: name.clone() });
                        index += 1;
                    }
                    _ => {
                        diagnostics.push(Diagnostic::new("toy type requires a name", None));
                        break;
                    }
                }
            }
            "part" => {
                index += 1;
                match tokens.get(index).map(String::as_str) {
                    Some("def") => {
                        index += 1;
                        match tokens.get(index) {
                            Some(name) if is_identifier(name) => {
                                declarations
                                    .push(ToyDeclaration::Definition { name: name.clone() });
                                index += 1;
                            }
                            _ => {
                                diagnostics.push(Diagnostic::new(
                                    "toy part definition requires a name",
                                    None,
                                ));
                                break;
                            }
                        }
                    }
                    Some(name) if is_identifier(name) => {
                        let part_name = name.to_string();
                        index += 1;
                        match (tokens.get(index).map(String::as_str), tokens.get(index + 1)) {
                            (Some(":"), Some(type_name)) if is_qualified_identifier(type_name) => {
                                declarations.push(ToyDeclaration::Usage {
                                    name: part_name,
                                    type_name: type_name.clone(),
                                });
                                index += 2;
                            }
                            _ => {
                                diagnostics.push(Diagnostic::new(
                                    "toy part usage must be `part <name> : <type>`",
                                    None,
                                ));
                                break;
                            }
                        }
                    }
                    _ => {
                        diagnostics.push(Diagnostic::new(
                            "toy part declaration must be `part def <name>` or `part <name> : <type>`",
                            None,
                        ));
                        break;
                    }
                }
            }
            "feature" => {
                index += 1;
                match (tokens.get(index).map(String::as_str), tokens.get(index + 1)) {
                    (Some("def"), Some(name)) if is_identifier(name) => {
                        declarations.push(ToyDeclaration::FeatureDefinition { name: name.clone() });
                        index += 2;
                    }
                    _ => {
                        diagnostics.push(Diagnostic::new(
                            "toy feature definition requires a name",
                            None,
                        ));
                        break;
                    }
                }
            }
            token => {
                diagnostics.push(Diagnostic::new(
                    format!("unsupported toy declaration `{token}`"),
                    None,
                ));
                break;
            }
        }

        while matches!(
            tokens.get(index).map(String::as_str),
            Some(";") | Some("{") | Some("}")
        ) {
            index += 1;
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(ToyModel {
        package: package.unwrap_or_else(|| package_name_from_source_name(source_name)),
        source_name: source_name.to_string(),
        declarations: if declarations.is_empty() {
            vec![ToyDeclaration::Type {
                name: "Thing".to_string(),
            }]
        } else {
            declarations
        },
    })
}

fn tokenize(source: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if matches!(ch, ':' | ';' | '{' | '}') {
                tokens.push(ch.to_string());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_qualified_identifier(value: &str) -> bool {
    value.split('.').all(is_identifier)
}

fn package_name_from_source_name(source_name: &str) -> String {
    std::path::Path::new(source_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| is_identifier(value))
        .unwrap_or("Model")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::kir::KirDocument;

    use super::{compile_report, registry};
    use crate::language_contracts::SemanticCompileStatus;

    #[test]
    fn compiles_package_type_and_part_usage() {
        let report = compile_report(
            "package Demo type Vehicle part vehicle : Vehicle",
            "demo.toy",
        );

        assert_eq!(report.status, SemanticCompileStatus::Ok);
        let document = report.document.unwrap();
        assert_eq!(document.elements.len(), 3);
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "pkg.Demo")
        );
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "type.Demo.Vehicle")
        );
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "part.Demo.vehicle")
        );
        document.validate().unwrap();
    }

    #[test]
    fn reports_unsupported_declaration() {
        let report = compile_report("package Demo relation Link", "demo.toy");

        assert_eq!(report.status, SemanticCompileStatus::Failed);
        assert!(report.document.is_none());
        assert_eq!(report.diagnostics.len(), 1);
        assert!(
            report.diagnostics[0]
                .message
                .contains("unsupported toy declaration")
        );
    }

    #[test]
    fn registry_dispatches_by_toy_extension() {
        let registry = registry();
        let report = registry.compile_path(
            Path::new("demo.toy"),
            "package Demo type Vehicle",
            &KirDocument {
                metadata: Default::default(),
                elements: Vec::new(),
            }
            .with_schema_version(),
        );

        assert_eq!(report.status, SemanticCompileStatus::Ok);
        assert!(report.document.is_some());
    }
}
