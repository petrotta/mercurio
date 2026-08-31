use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(any(test, feature = "toy-parser"))]
use serde_json::json;

use crate::authoring::frontend::ast::{
    BinaryOp, CommentKind, CommentNote, Declaration as AstDeclaration, Expr, LiteralExpr,
    MultiplicityRange, PackageDecl, ParsedModule, SourceSpan, UnaryOp,
};
use crate::authoring::frontend::diagnostics::Diagnostic;
#[cfg(any(test, feature = "toy-parser"))]
use crate::kir::KIR_SCHEMA_VERSION;
use crate::kir::{KirDocument, KirElement, KirError};

pub type SourceCompiler = fn(&BTreeMap<String, String>) -> Result<KirDocument, AuthoringError>;
pub type ModuleRenderer = fn(&AuthoringModule) -> String;
pub type PackageRenderer = fn(&Package, usize) -> String;
pub type DeclarationRenderer = fn(&Declaration, usize) -> String;

#[derive(Clone, Copy)]
pub struct AuthoringRenderProfile {
    pub render_module: ModuleRenderer,
    pub render_package: PackageRenderer,
    pub render_declaration: DeclarationRenderer,
}

impl Default for AuthoringRenderProfile {
    fn default() -> Self {
        textual_model_authoring_render_profile()
    }
}

impl fmt::Debug for AuthoringRenderProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthoringRenderProfile")
            .finish_non_exhaustive()
    }
}

pub fn textual_model_authoring_render_profile() -> AuthoringRenderProfile {
    AuthoringRenderProfile {
        render_module: render_textual_module,
        render_package: render_textual_package,
        render_declaration: render_textual_declaration,
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthoringProject {
    files: BTreeMap<String, FileModel>,
    source_compiler: Option<SourceCompiler>,
    render_profile: AuthoringRenderProfile,
}

impl PartialEq for AuthoringProject {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files
    }
}

impl Eq for AuthoringProject {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileModel {
    path: String,
    module: AuthoringModule,
    original_text: Option<String>,
    source_map: Option<FileSourceMap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthoringModule {
    pub package: Option<Package>,
    pub members: Vec<Declaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: QualifiedName,
    pub members: Vec<Declaration>,
    /// Leading own-line `//` and non-doc `/* */` comments. Rendered by the
    /// declaration's *container* (never by `render(..)` itself) so a
    /// localized splice — whose span starts at the declaration keyword,
    /// below any leading comments kept in the original text — cannot
    /// duplicate them.
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub path: QualifiedName,
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub keyword: String,
    pub name: String,
    pub specializes: Vec<QualifiedName>,
    pub members: Vec<Declaration>,
    pub raw_body: Option<String>,
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub keyword: String,
    pub name: String,
    pub is_implicit_name: bool,
    pub ty: Option<QualifiedName>,
    pub reference_target: Option<QualifiedName>,
    pub metadata_properties: BTreeMap<String, String>,
    pub multiplicity: Option<MultiplicityRange>,
    pub expression: Option<String>,
    pub additional_types: Vec<QualifiedName>,
    pub specializes: Vec<QualifiedName>,
    pub subsets: Vec<QualifiedName>,
    pub redefines: Vec<QualifiedName>,
    pub members: Vec<Declaration>,
    pub raw_body: Option<String>,
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    pub target: QualifiedName,
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    Package(Package),
    Import(Import),
    Definition(Definition),
    Usage(Usage),
    Alias(Alias),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QualifiedName(pub Vec<String>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    AddPackage {
        target_file: String,
        package_name: QualifiedName,
    },
    AddImport {
        target_file: String,
        package_name: Option<QualifiedName>,
        path: QualifiedName,
    },
    RemoveImport {
        target_file: String,
        package_name: Option<QualifiedName>,
        path: QualifiedName,
    },
    AddDefinition {
        container: ContainerSelector,
        keyword: String,
        name: String,
        specializes: Vec<QualifiedName>,
    },
    AddUsage {
        container: ContainerSelector,
        keyword: String,
        name: String,
        ty: Option<QualifiedName>,
        specializes: Vec<QualifiedName>,
    },
    AddRelationship {
        container: ContainerSelector,
        kind: String,
        source: QualifiedName,
        target: QualifiedName,
    },
    AddMetadataAnnotation {
        element: QualifiedName,
        metadata_type: String,
        properties: BTreeMap<String, String>,
    },
    RemoveDeclaration {
        qualified_name: QualifiedName,
    },
    RenameDeclaration {
        qualified_name: QualifiedName,
        new_name: String,
    },
    UpdateSpecializations {
        qualified_name: QualifiedName,
        specializes: Vec<QualifiedName>,
    },
    UpdateUsageType {
        qualified_name: QualifiedName,
        ty: Option<QualifiedName>,
    },
    SetExpression {
        qualified_name: QualifiedName,
        expression: Option<String>,
    },
    MoveDeclaration {
        qualified_name: QualifiedName,
        destination: ContainerSelector,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticEdit {
    SetAttribute {
        element: QualifiedName,
        attribute: String,
        value: Value,
        policy: AttributeWritePolicy,
    },
    ClearAttribute {
        element: QualifiedName,
        attribute: String,
        policy: AttributeWritePolicy,
    },
    AddAttributeValue {
        element: QualifiedName,
        attribute: String,
        value: Value,
        policy: AttributeWritePolicy,
    },
    RemoveAttributeValue {
        element: QualifiedName,
        attribute: String,
        value: Value,
        policy: AttributeWritePolicy,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeWritePolicy {
    DirectOnly,
    UpsertDirect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticAttribute {
    pub name: String,
    pub origin_kind: String,
    pub direct_value: Option<Value>,
    pub effective_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerSelector {
    File { target_file: String },
    Package { qualified_name: QualifiedName },
    Declaration { qualified_name: QualifiedName },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MutationResult {
    pub changed_files: BTreeSet<String>,
    pub changed_declarations: BTreeSet<String>,
    pub affected_element_ids: BTreeSet<String>,
    rewrite_plan: Vec<RewriteInstruction>,
}

impl MutationResult {
    /// Folds another mutation's outcome into this one so a multi-operation
    /// plan can be written back in a single pass. Rewrite instructions keep
    /// their application order; write-back normalization dedupes them.
    pub fn merge(&mut self, other: MutationResult) {
        self.changed_files.extend(other.changed_files);
        self.changed_declarations.extend(other.changed_declarations);
        self.affected_element_ids.extend(other.affected_element_ids);
        self.rewrite_plan.extend(other.rewrite_plan);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteBackResult {
    pub edited_files: BTreeMap<String, String>,
    pub mode: WriteBackMode,
    pub changed_spans: BTreeMap<String, Vec<RenderedSpan>>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteBackMode {
    LocalizedPatch,
    CanonicalRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub ok: bool,
    pub expected_element_count: usize,
    pub actual_element_count: usize,
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum AuthoringError {
    Parse(Diagnostic),
    Kir(KirError),
    MissingFile(String),
    MissingPackage(String),
    MissingDeclaration(String),
    InvalidMutation(String),
    Unsupported(String),
    Validation(String),
    Io(std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSourceMap {
    package: Option<SourceNode>,
    declarations: BTreeMap<String, SourceNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceNode {
    span: SourceSpan,
    indent: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RewriteInstruction {
    FullFile {
        file: String,
    },
    ReplaceNode {
        file: String,
        anchor_qname: String,
        render_qname: String,
    },
    ReplaceContainer {
        file: String,
        anchor_qname: Option<String>,
        render_qname: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeclarationKind {
    Package,
    Definition,
    Usage,
    Alias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocatedDeclaration {
    file: String,
    kind: DeclarationKind,
    parent_qname: Option<String>,
}

impl fmt::Display for AuthoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Kir(err) => write!(f, "{err}"),
            Self::MissingFile(path) => write!(f, "missing authoring file: {path}"),
            Self::MissingPackage(name) => write!(f, "missing package: {name}"),
            Self::MissingDeclaration(name) => write!(f, "missing declaration: {name}"),
            Self::InvalidMutation(message) => write!(f, "{message}"),
            Self::Unsupported(message) => write!(f, "{message}"),
            Self::Validation(message) => write!(f, "{message}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for AuthoringError {}

impl From<Diagnostic> for AuthoringError {
    fn from(value: Diagnostic) -> Self {
        Self::Parse(value)
    }
}

impl From<KirError> for AuthoringError {
    fn from(value: KirError) -> Self {
        Self::Kir(value)
    }
}

impl From<std::io::Error> for AuthoringError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl QualifiedName {
    pub fn new(segments: Vec<String>) -> Self {
        Self(segments)
    }

    pub fn parse(value: &str) -> Self {
        let segments = value
            .split(['.', ':'])
            .filter(|segment| !segment.is_empty())
            .map(unquote_sysml_name)
            .collect::<Vec<_>>();
        if value.contains("::") {
            return Self(
                value
                    .split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(unquote_sysml_name)
                    .collect(),
            );
        }
        Self(segments)
    }

    pub fn as_dot_string(&self) -> String {
        self.0.join(".")
    }

    pub fn as_colon_string(&self) -> String {
        self.0.join("::")
    }

    fn tail(&self) -> Option<&str> {
        self.0.last().map(String::as_str)
    }
}

pub fn create_empty_model() -> AuthoringProject {
    AuthoringProject::default()
}

pub fn load_authoring_project_from_kir(
    document: &KirDocument,
) -> Result<AuthoringProject, AuthoringError> {
    AuthoringProject::from_kir_document(document)
}

#[cfg(not(any(test, feature = "toy-parser")))]
pub fn load_authoring_project_from_model(
    _files: BTreeMap<String, String>,
) -> Result<AuthoringProject, AuthoringError> {
    Err(AuthoringError::Unsupported(
        "loading source text requires a language-specific parser".to_string(),
    ))
}

#[cfg(any(test, feature = "toy-parser"))]
pub fn load_authoring_project_from_model(
    files: BTreeMap<String, String>,
) -> Result<AuthoringProject, AuthoringError> {
    let mut project = AuthoringProject::default();
    for (path, source) in files {
        project.files.insert(
            path.clone(),
            FileModel {
                path,
                module: parse_fake_model_source(&source)?,
                original_text: None,
                source_map: None,
            },
        );
    }
    Ok(project)
}

impl AuthoringProject {
    pub fn from_parsed_modules(
        modules: BTreeMap<String, ParsedModule>,
        original_texts: BTreeMap<String, String>,
    ) -> Result<Self, AuthoringError> {
        let mut project = Self::default();
        for (path, parsed) in modules {
            let module = AuthoringModule::from_ast(&parsed);
            let source_map =
                FileSourceMap::from_ast(&parsed, original_texts.get(&path).map(String::as_str));
            project.files.insert(
                path.clone(),
                FileModel {
                    original_text: original_texts.get(&path).cloned(),
                    source_map: Some(source_map),
                    path,
                    module,
                },
            );
        }
        Ok(project)
    }

    pub fn with_source_compiler(mut self, compiler: SourceCompiler) -> Self {
        self.source_compiler = Some(compiler);
        self
    }

    pub fn with_render_profile(mut self, render_profile: AuthoringRenderProfile) -> Self {
        self.render_profile = render_profile;
        self
    }

    pub fn from_model_files(files: BTreeMap<String, String>) -> Result<Self, AuthoringError> {
        load_authoring_project_from_model(files)
    }

    pub fn from_kir_document(document: &KirDocument) -> Result<Self, AuthoringError> {
        let grouped = group_kir_by_source_file(document);
        let mut project = Self::default();

        for (path, elements) in grouped {
            let module = module_from_kir_elements(&elements)?;
            project.files.insert(
                path.clone(),
                FileModel {
                    path,
                    module,
                    original_text: None,
                    source_map: None,
                },
            );
        }

        Ok(project)
    }

    pub fn files(&self) -> impl Iterator<Item = (&str, &AuthoringModule)> {
        self.files
            .iter()
            .map(|(path, file)| (path.as_str(), &file.module))
    }

    pub fn render_new_file(&self, path: &str) -> Result<String, AuthoringError> {
        let file = self
            .files
            .get(path)
            .ok_or_else(|| AuthoringError::MissingFile(path.to_string()))?;
        Ok(self.render_module(&file.module))
    }

    pub fn semantic_attributes(
        &self,
        element: &QualifiedName,
    ) -> Result<Vec<SemanticAttribute>, AuthoringError> {
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;

        if matches!(located.kind, DeclarationKind::Package) {
            let package = locate_package_ref(&file.module, element)
                .ok_or_else(|| AuthoringError::MissingPackage(element.as_dot_string()))?;
            return Ok(semantic_attributes_for_package(package));
        }

        let declaration = locate_declaration_ref(&file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        Ok(semantic_attributes_for_declaration(declaration))
    }

    pub fn apply_semantic_edit(
        &mut self,
        edit: SemanticEdit,
    ) -> Result<MutationResult, AuthoringError> {
        match edit {
            SemanticEdit::SetAttribute {
                element,
                attribute,
                value,
                policy,
            } => {
                let attribute = normalize_attribute_name(&attribute);
                self.ensure_attribute_policy(&element, &attribute, policy)?;
                match attribute.as_str() {
                    "declared_name" => self.apply_mutation(Mutation::RenameDeclaration {
                        qualified_name: element,
                        new_name: value_as_string(&value, "declared_name")?,
                    }),
                    "is_language_extension_keyword" => {
                        let (modifier, enabled) = modifier_edit_for_attribute(
                            &attribute,
                            value_as_bool(&value, &attribute)?,
                        );
                        self.apply_modifier_flag_edit(&element, &attribute, modifier, enabled)
                    }
                    "language_extensions" => self.apply_language_extensions_edit(
                        &element,
                        value_as_string_list(&value, "language_extensions")?,
                    ),
                    "transition_source" | "transition_target" | "trigger" | "trigger_kind" => self
                        .apply_usage_modifier_value_edit(
                            &element,
                            &attribute,
                            &attribute,
                            Some(value_as_string(&value, &attribute)?),
                        ),
                    "source_is_initial" => {
                        let enabled = value_as_bool(&value, &attribute)?;
                        self.apply_modifier_flag_edit(
                            &element,
                            &attribute,
                            "source_is_initial",
                            enabled,
                        )
                    }
                    "declared_short_name" | "short_name" => self.apply_short_name_edit(
                        &element,
                        &attribute,
                        Some(value_as_string(&value, &attribute)?),
                    ),
                    "raw_body" | "body" => self.apply_raw_body_edit(
                        &element,
                        &attribute,
                        Some(value_as_string(&value, &attribute)?),
                    ),
                    "specializes" => self.apply_mutation(Mutation::UpdateSpecializations {
                        qualified_name: element,
                        specializes: value_as_qname_list(&value, "specializes")?,
                    }),
                    "annotated_elements" => self.apply_annotated_elements_edit(
                        &element,
                        value_as_qname_list(&value, "annotated_elements")?,
                    ),
                    "additional_types" | "subsets" | "redefines" => self
                        .apply_usage_qname_list_edit(
                            &element,
                            &attribute,
                            value_as_qname_list(&value, &attribute)?,
                        ),
                    "type" => self.apply_mutation(Mutation::UpdateUsageType {
                        qualified_name: element,
                        ty: Some(value_as_qname(&value, "type")?),
                    }),
                    "is_abstract" | "is_end" | "is_derived" | "is_individual" | "is_ordered"
                    | "is_unique" | "is_variable" => {
                        let (modifier, enabled) = modifier_edit_for_attribute(
                            &attribute,
                            value_as_bool(&value, &attribute)?,
                        );
                        self.apply_modifier_flag_edit(&element, &attribute, modifier, enabled)
                    }
                    "direction" => {
                        self.apply_direction_edit(&element, Some(value_as_direction(&value)?))
                    }
                    "multiplicity" => self.apply_usage_multiplicity_edit(
                        &element,
                        Some(value_as_multiplicity(&value)?),
                    ),
                    "reference_target" => {
                        if value.is_array() {
                            self.apply_usage_reference_targets_edit(
                                &element,
                                value_as_qname_list(&value, "reference_target")?,
                            )
                        } else {
                            self.apply_usage_reference_target_edit(
                                &element,
                                Some(value_as_qname(&value, "reference_target")?),
                            )
                        }
                    }
                    "reference_targets" => self.apply_usage_reference_targets_edit(
                        &element,
                        value_as_qname_list(&value, "reference_targets")?,
                    ),
                    "target" => {
                        self.apply_target_edit(&element, Some(value_as_qname(&value, "target")?))
                    }
                    "imports" => self
                        .apply_imports_replace(&element, value_as_qname_list(&value, "imports")?),
                    "doc" | "text" => self.apply_doc_edit(
                        &element,
                        DocEdit::Text(value_as_string(&value, &attribute)?),
                    ),
                    "id" => self.apply_doc_edit(
                        &element,
                        DocEdit::Id(value_as_string(&value, &attribute)?),
                    ),
                    other => Err(AuthoringError::Unsupported(format!(
                        "semantic set is not supported for attribute `{other}`"
                    ))),
                }
            }
            SemanticEdit::ClearAttribute {
                element,
                attribute,
                policy,
            } => {
                let attribute = normalize_attribute_name(&attribute);
                self.ensure_attribute_policy(&element, &attribute, policy)?;
                match attribute.as_str() {
                    "specializes" => self.apply_mutation(Mutation::UpdateSpecializations {
                        qualified_name: element,
                        specializes: Vec::new(),
                    }),
                    "annotated_elements" => {
                        self.apply_annotated_elements_edit(&element, Vec::new())
                    }
                    "is_language_extension_keyword" => {
                        let (modifier, enabled) = modifier_clear_for_attribute(&attribute);
                        self.apply_modifier_flag_edit(&element, &attribute, modifier, enabled)
                    }
                    "language_extensions" => {
                        self.apply_language_extensions_edit(&element, Vec::new())
                    }
                    "transition_source" | "transition_target" | "trigger" | "trigger_kind" => {
                        self.apply_usage_modifier_value_edit(&element, &attribute, &attribute, None)
                    }
                    "source_is_initial" => self.apply_modifier_flag_edit(
                        &element,
                        &attribute,
                        "source_is_initial",
                        false,
                    ),
                    "declared_short_name" | "short_name" => {
                        self.apply_short_name_edit(&element, &attribute, None)
                    }
                    "raw_body" | "body" => self.apply_raw_body_edit(&element, &attribute, None),
                    "additional_types" | "subsets" | "redefines" => {
                        self.apply_usage_qname_list_edit(&element, &attribute, Vec::new())
                    }
                    "type" => self.apply_mutation(Mutation::UpdateUsageType {
                        qualified_name: element,
                        ty: None,
                    }),
                    "is_abstract" | "is_end" | "is_derived" | "is_individual" | "is_ordered"
                    | "is_unique" | "is_variable" => {
                        let (modifier, enabled) = modifier_clear_for_attribute(&attribute);
                        self.apply_modifier_flag_edit(&element, &attribute, modifier, enabled)
                    }
                    "direction" => self.apply_direction_edit(&element, None),
                    "multiplicity" => self.apply_usage_multiplicity_edit(&element, None),
                    "reference_target" | "reference_targets" => {
                        self.apply_usage_reference_targets_edit(&element, Vec::new())
                    }
                    "target" => self.apply_target_edit(&element, None),
                    "imports" => self.apply_imports_replace(&element, Vec::new()),
                    "doc" | "text" => self.apply_doc_edit(&element, DocEdit::ClearText),
                    "id" => self.apply_doc_edit(&element, DocEdit::ClearId),
                    other => Err(AuthoringError::Unsupported(format!(
                        "semantic clear is not supported for attribute `{other}`"
                    ))),
                }
            }
            SemanticEdit::AddAttributeValue {
                element,
                attribute,
                value,
                policy,
            } => {
                let attribute = normalize_attribute_name(&attribute);
                self.ensure_attribute_policy(&element, &attribute, policy)?;
                match attribute.as_str() {
                    "specializes" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        for item in value_as_qname_list(&value, "specializes")? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_mutation(Mutation::UpdateSpecializations {
                            qualified_name: element,
                            specializes: values,
                        })
                    }
                    "annotated_elements" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        for item in value_as_qname_list(&value, "annotated_elements")? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_annotated_elements_edit(&element, values)
                    }
                    "language_extensions" => {
                        let mut values = self.string_list_attribute_values(&element, &attribute)?;
                        for item in value_as_string_list(&value, "language_extensions")? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_language_extensions_edit(&element, values)
                    }
                    "additional_types" | "subsets" | "redefines" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        for item in value_as_qname_list(&value, &attribute)? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_usage_qname_list_edit(&element, &attribute, values)
                    }
                    "reference_target" | "reference_targets" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        for item in value_as_qname_list(&value, &attribute)? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_usage_reference_targets_edit(&element, values)
                    }
                    "imports" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        for item in value_as_qname_list(&value, "imports")? {
                            if !values.contains(&item) {
                                values.push(item);
                            }
                        }
                        self.apply_imports_replace(&element, values)
                    }
                    other => Err(AuthoringError::Unsupported(format!(
                        "semantic add is not supported for attribute `{other}`"
                    ))),
                }
            }
            SemanticEdit::RemoveAttributeValue {
                element,
                attribute,
                value,
                policy,
            } => {
                let attribute = normalize_attribute_name(&attribute);
                self.ensure_attribute_policy(&element, &attribute, policy)?;
                match attribute.as_str() {
                    "specializes" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_qname_list(&value, "specializes")?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_mutation(Mutation::UpdateSpecializations {
                            qualified_name: element,
                            specializes: values,
                        })
                    }
                    "annotated_elements" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_qname_list(&value, "annotated_elements")?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_annotated_elements_edit(&element, values)
                    }
                    "language_extensions" => {
                        let mut values = self.string_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_string_list(&value, "language_extensions")?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_language_extensions_edit(&element, values)
                    }
                    "additional_types" | "subsets" | "redefines" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_qname_list(&value, &attribute)?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_usage_qname_list_edit(&element, &attribute, values)
                    }
                    "reference_target" | "reference_targets" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_qname_list(&value, &attribute)?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_usage_reference_targets_edit(&element, values)
                    }
                    "imports" => {
                        let mut values = self.qname_list_attribute_values(&element, &attribute)?;
                        let removals = value_as_qname_list(&value, "imports")?;
                        values.retain(|item| !removals.contains(item));
                        self.apply_imports_replace(&element, values)
                    }
                    other => Err(AuthoringError::Unsupported(format!(
                        "semantic remove is not supported for attribute `{other}`"
                    ))),
                }
            }
        }
    }

    pub fn apply_mutation(&mut self, mutation: Mutation) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let mut changed_files = BTreeSet::new();
        let mut changed_declarations = BTreeSet::new();
        let rewrite_plan = match mutation {
            Mutation::AddPackage {
                target_file,
                package_name,
            } => {
                let file = self.ensure_file_mut(&target_file);
                if file.module.package.is_some() {
                    return Err(AuthoringError::InvalidMutation(format!(
                        "file `{target_file}` already has a package"
                    )));
                }
                file.module.package = Some(Package {
                    name: package_name.clone(),
                    members: Vec::new(),
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers: Vec::new(),
                });
                changed_files.insert(target_file.clone());
                changed_declarations.insert(package_name.as_dot_string());
                vec![RewriteInstruction::FullFile { file: target_file }]
            }
            Mutation::AddImport {
                target_file,
                package_name,
                path,
            } => {
                let file = self.ensure_file_mut(&target_file);
                let import = Declaration::Import(Import {
                    path: path.clone(),
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers: Vec::new(),
                });
                let instruction = if let Some(package_name) = package_name {
                    let package =
                        locate_package_mut(&mut file.module, &package_name).ok_or_else(|| {
                            AuthoringError::MissingPackage(package_name.as_dot_string())
                        })?;
                    package.members.push(import);
                    RewriteInstruction::ReplaceContainer {
                        file: target_file.clone(),
                        anchor_qname: Some(package_name.as_dot_string()),
                        render_qname: Some(package_name.as_dot_string()),
                    }
                } else {
                    file.module.members.push(import);
                    RewriteInstruction::FullFile {
                        file: target_file.clone(),
                    }
                };
                changed_files.insert(target_file);
                vec![instruction]
            }
            Mutation::RemoveImport {
                target_file,
                package_name,
                path,
            } => {
                let file = self
                    .files
                    .get_mut(&target_file)
                    .ok_or_else(|| AuthoringError::MissingFile(target_file.clone()))?;
                let removed = if let Some(package_name) = &package_name {
                    let package =
                        locate_package_mut(&mut file.module, package_name).ok_or_else(|| {
                            AuthoringError::MissingPackage(package_name.as_dot_string())
                        })?;
                    remove_import(&mut package.members, &path)
                } else {
                    remove_import(&mut file.module.members, &path)
                };
                if !removed {
                    return Err(AuthoringError::InvalidMutation(format!(
                        "missing import `{}` in `{target_file}`",
                        path.as_colon_string()
                    )));
                }
                changed_files.insert(target_file.clone());
                vec![if let Some(package_name) = package_name {
                    RewriteInstruction::ReplaceContainer {
                        file: target_file,
                        anchor_qname: Some(package_name.as_dot_string()),
                        render_qname: Some(package_name.as_dot_string()),
                    }
                } else {
                    RewriteInstruction::FullFile { file: target_file }
                }]
            }
            Mutation::AddDefinition {
                container,
                keyword,
                name,
                specializes,
            } => {
                let name = unquote_sysml_name(&name);
                let definition = Declaration::Definition(Definition {
                    keyword,
                    name: name.clone(),
                    specializes,
                    members: Vec::new(),
                    raw_body: None,
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers: Vec::new(),
                });
                let (file, owner_qname, instruction) =
                    self.push_into_container(container, definition)?;
                changed_files.insert(file.clone());
                let owner = owner_qname.unwrap_or_default();
                changed_declarations.insert(join_qname(&owner, &name));
                vec![instruction]
            }
            Mutation::AddUsage {
                container,
                keyword,
                name,
                ty,
                specializes,
            } => {
                let name = unquote_sysml_name(&name);
                let modifiers = if keyword == "metadata" {
                    vec!["metadata_usage".to_string()]
                } else {
                    Vec::new()
                };
                let usage = Declaration::Usage(Usage {
                    keyword,
                    name: name.clone(),
                    is_implicit_name: false,
                    ty,
                    reference_target: None,
                    metadata_properties: BTreeMap::new(),
                    multiplicity: None,
                    expression: None,
                    additional_types: Vec::new(),
                    specializes,
                    subsets: Vec::new(),
                    redefines: Vec::new(),
                    members: Vec::new(),
                    raw_body: None,
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers,
                });
                let (file, owner_qname, instruction) =
                    self.push_into_container(container, usage)?;
                changed_files.insert(file.clone());
                let owner = owner_qname.unwrap_or_default();
                changed_declarations.insert(join_qname(&owner, &name));
                vec![instruction]
            }
            Mutation::AddRelationship {
                container,
                kind,
                source,
                target,
            } => {
                let usage = relationship_usage(&kind, &source, &target)?;
                let relationship_name = usage.name.clone();
                let (file, owner_qname, instruction) =
                    self.push_into_container(container, Declaration::Usage(usage))?;
                changed_files.insert(file.clone());
                let owner = owner_qname.unwrap_or_default();
                changed_declarations.insert(join_qname(&owner, &relationship_name));
                vec![instruction]
            }
            Mutation::AddMetadataAnnotation {
                element,
                metadata_type,
                properties,
            } => {
                if metadata_type.trim().is_empty() {
                    return Err(AuthoringError::InvalidMutation(
                        "metadata annotation type must not be empty".to_string(),
                    ));
                }
                let usage = Declaration::Usage(Usage {
                    keyword: "metadata".to_string(),
                    name: metadata_type.clone(),
                    is_implicit_name: false,
                    ty: None,
                    reference_target: None,
                    metadata_properties: properties,
                    multiplicity: None,
                    expression: None,
                    additional_types: Vec::new(),
                    specializes: Vec::new(),
                    subsets: Vec::new(),
                    redefines: Vec::new(),
                    members: Vec::new(),
                    raw_body: None,
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers: Vec::new(),
                });
                let (file, _, instruction) = self.push_into_container(
                    ContainerSelector::Declaration {
                        qualified_name: element.clone(),
                    },
                    usage,
                )?;
                changed_files.insert(file);
                changed_declarations.insert(element.as_dot_string());
                vec![instruction]
            }
            Mutation::RemoveDeclaration { qualified_name } => {
                let located = self.locate_declaration(&qualified_name)?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                match located.parent_qname.clone() {
                    Some(parent_qname) => {
                        let parent = locate_members_mut(
                            &mut file.module,
                            &QualifiedName::parse(&parent_qname),
                        )
                        .ok_or_else(|| AuthoringError::MissingDeclaration(parent_qname.clone()))?;
                        remove_declaration(parent, &qualified_name).ok_or_else(|| {
                            AuthoringError::MissingDeclaration(qualified_name.as_dot_string())
                        })?;
                        changed_files.insert(located.file.clone());
                        vec![RewriteInstruction::ReplaceContainer {
                            file: located.file,
                            anchor_qname: Some(parent_qname.clone()),
                            render_qname: Some(parent_qname),
                        }]
                    }
                    None => {
                        match located.kind {
                            DeclarationKind::Package => {
                                file.module.package = None;
                            }
                            _ => {
                                remove_declaration(&mut file.module.members, &qualified_name)
                                    .ok_or_else(|| {
                                        AuthoringError::MissingDeclaration(
                                            qualified_name.as_dot_string(),
                                        )
                                    })?;
                            }
                        }
                        changed_files.insert(located.file.clone());
                        vec![RewriteInstruction::FullFile { file: located.file }]
                    }
                }
            }
            Mutation::RenameDeclaration {
                qualified_name,
                new_name,
            } => {
                let new_name = unquote_sysml_name(&new_name);
                let located = self.locate_declaration(&qualified_name)?;
                let old_qname = qualified_name.as_dot_string();
                let new_qname = if let Some(parent) = &located.parent_qname {
                    join_qname(parent, &new_name)
                } else {
                    new_name.clone()
                };
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                rename_declaration(&mut file.module, &qualified_name, &new_name)?;
                changed_files.insert(located.file.clone());
                changed_declarations.insert(new_qname.clone());
                vec![RewriteInstruction::ReplaceNode {
                    file: located.file,
                    anchor_qname: old_qname,
                    render_qname: new_qname,
                }]
            }
            Mutation::UpdateSpecializations {
                qualified_name,
                specializes,
            } => {
                let located = self.locate_declaration(&qualified_name)?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                update_specializations(&mut file.module, &qualified_name, specializes)?;
                changed_files.insert(located.file.clone());
                changed_declarations.insert(qualified_name.as_dot_string());
                vec![RewriteInstruction::ReplaceNode {
                    file: located.file,
                    anchor_qname: qualified_name.as_dot_string(),
                    render_qname: qualified_name.as_dot_string(),
                }]
            }
            Mutation::UpdateUsageType { qualified_name, ty } => {
                let located = self.locate_declaration(&qualified_name)?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                update_usage_type(&mut file.module, &qualified_name, ty)?;
                changed_files.insert(located.file.clone());
                changed_declarations.insert(qualified_name.as_dot_string());
                vec![RewriteInstruction::ReplaceNode {
                    file: located.file,
                    anchor_qname: qualified_name.as_dot_string(),
                    render_qname: qualified_name.as_dot_string(),
                }]
            }
            Mutation::SetExpression {
                qualified_name,
                expression,
            } => {
                let located = self.locate_declaration(&qualified_name)?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                set_usage_expression(&mut file.module, &qualified_name, expression)?;
                changed_files.insert(located.file.clone());
                changed_declarations.insert(qualified_name.as_dot_string());
                vec![RewriteInstruction::ReplaceNode {
                    file: located.file,
                    anchor_qname: qualified_name.as_dot_string(),
                    render_qname: qualified_name.as_dot_string(),
                }]
            }
            Mutation::MoveDeclaration {
                qualified_name,
                destination,
            } => {
                let located = self.locate_declaration(&qualified_name)?;
                let source_file_path = located.file.clone();
                let moved = {
                    let source_file = self
                        .files
                        .get_mut(&source_file_path)
                        .ok_or_else(|| AuthoringError::MissingFile(source_file_path.clone()))?;
                    extract_declaration(&mut source_file.module, &qualified_name)?
                };
                let source_instruction = match located.parent_qname.clone() {
                    Some(parent_qname) => RewriteInstruction::ReplaceContainer {
                        file: source_file_path.clone(),
                        anchor_qname: Some(parent_qname.clone()),
                        render_qname: Some(parent_qname),
                    },
                    None => RewriteInstruction::FullFile {
                        file: source_file_path.clone(),
                    },
                };
                let (dest_file, _, dest_instruction) =
                    self.push_into_container(destination, moved)?;
                changed_files.insert(source_file_path.clone());
                changed_files.insert(dest_file.clone());
                vec![source_instruction, dest_instruction]
            }
        };
        self.finalize_change(before, changed_files, changed_declarations, rewrite_plan)
    }

    pub fn write_back_mutation(
        &mut self,
        mutation: &MutationResult,
    ) -> Result<WriteBackResult, AuthoringError> {
        let localized = self.try_localized_writeback(mutation);
        let (edited_files, mode, changed_spans, validation) = match localized {
            Ok((files, spans, report)) if report.ok => {
                (files, WriteBackMode::LocalizedPatch, spans, report)
            }
            Ok((_, _, _report)) => {
                let (files, spans) = self.canonical_rewrite(&mutation.changed_files)?;
                let report = self.validate_rendered_files(&files)?;
                if !report.ok {
                    return Err(AuthoringError::Validation(
                        report
                            .message
                            .clone()
                            .unwrap_or_else(|| "write-back validation failed".to_string()),
                    ));
                }
                (files, WriteBackMode::CanonicalRewrite, spans, report)
            }
            Err(_) => {
                let (files, spans) = self.canonical_rewrite(&mutation.changed_files)?;
                let report = self.validate_rendered_files(&files)?;
                if !report.ok {
                    return Err(AuthoringError::Validation(
                        report
                            .message
                            .clone()
                            .unwrap_or_else(|| "write-back validation failed".to_string()),
                    ));
                }
                (files, WriteBackMode::CanonicalRewrite, spans, report)
            }
        };

        self.accept_write_back_files(&edited_files)?;

        Ok(WriteBackResult {
            edited_files,
            mode,
            changed_spans,
            validation,
        })
    }

    pub fn write_back_changed_files(
        &self,
        changed_files: &BTreeSet<String>,
    ) -> Result<WriteBackResult, AuthoringError> {
        let (edited_files, changed_spans) = self.canonical_rewrite(changed_files)?;
        let validation = self.validate_rendered_files(&edited_files)?;
        Ok(WriteBackResult {
            edited_files,
            mode: WriteBackMode::CanonicalRewrite,
            changed_spans,
            validation,
        })
    }

    pub fn write_back_changed_files_and_update(
        &mut self,
        changed_files: &BTreeSet<String>,
    ) -> Result<WriteBackResult, AuthoringError> {
        let write_back = self.write_back_changed_files(changed_files)?;
        self.accept_write_back_files(&write_back.edited_files)?;
        Ok(write_back)
    }

    pub fn validate_rendered_files_public(
        &self,
        rendered: &BTreeMap<String, String>,
    ) -> Result<ValidationReport, AuthoringError> {
        self.validate_rendered_files(rendered)
    }

    pub fn compile_kir_document(&self) -> Result<KirDocument, AuthoringError> {
        self.compile_user_kir()
    }

    fn accept_write_back_files(
        &mut self,
        edited_files: &BTreeMap<String, String>,
    ) -> Result<(), AuthoringError> {
        for (path, content) in edited_files {
            let file = self.ensure_file_mut(path);
            file.original_text = Some(content.clone());
            // The parse-time spans no longer describe the edited text; keeping
            // them would let a later localized write-back splice with stale
            // offsets. A fresh parse is required to re-enable localization.
            file.source_map = None;
        }
        Ok(())
    }

    fn try_localized_writeback(
        &self,
        mutation: &MutationResult,
    ) -> Result<
        (
            BTreeMap<String, String>,
            BTreeMap<String, Vec<RenderedSpan>>,
            ValidationReport,
        ),
        AuthoringError,
    > {
        let mut edited = BTreeMap::new();
        let mut changed_spans = BTreeMap::new();
        let instructions_by_file = group_rewrites_by_file(&mutation.rewrite_plan);

        for (file_path, instructions) in instructions_by_file {
            let file = self
                .files
                .get(&file_path)
                .ok_or_else(|| AuthoringError::MissingFile(file_path.clone()))?;
            let original = file.original_text.as_ref().ok_or_else(|| {
                AuthoringError::Unsupported(format!(
                    "localized write-back requires original source text for `{file_path}`"
                ))
            })?;
            let source_map = file.source_map.as_ref().ok_or_else(|| {
                AuthoringError::Unsupported(format!(
                    "localized write-back requires source provenance for `{file_path}`"
                ))
            })?;
            let instructions =
                normalize_rewrite_instructions(instructions, source_map, &file.module)?;

            let mut patches = Vec::new();
            let mut spans = Vec::new();
            for instruction in instructions {
                let (span, replacement) = match instruction {
                    RewriteInstruction::FullFile { .. } => {
                        return Err(AuthoringError::Unsupported(
                            "full-file rewrite is not localized".to_string(),
                        ));
                    }
                    RewriteInstruction::ReplaceNode {
                        anchor_qname,
                        render_qname,
                        ..
                    } => {
                        let node = source_map.declarations.get(&anchor_qname).ok_or_else(|| {
                            AuthoringError::Unsupported(format!(
                                "missing source span for `{anchor_qname}`"
                            ))
                        })?;
                        let declaration = render_declaration_at_qname(
                            &file.module,
                            &QualifiedName::parse(&render_qname),
                            self.render_profile,
                        )
                        .ok_or_else(|| AuthoringError::MissingDeclaration(render_qname.clone()))?;
                        (
                            node.span.clone(),
                            render_for_splice(&declaration, node.indent),
                        )
                    }
                    RewriteInstruction::ReplaceContainer {
                        anchor_qname,
                        render_qname,
                        ..
                    } => {
                        if let Some(anchor_qname) = anchor_qname {
                            let node = if let Some(package) = &source_map.package {
                                if render_qname.as_deref() == Some(&anchor_qname)
                                    && file.module.package.as_ref().is_some_and(|package_model| {
                                        package_model.name.as_dot_string() == anchor_qname
                                    })
                                {
                                    package
                                } else {
                                    source_map.declarations.get(&anchor_qname).ok_or_else(|| {
                                        AuthoringError::Unsupported(format!(
                                            "missing source span for `{anchor_qname}`"
                                        ))
                                    })?
                                }
                            } else {
                                source_map.declarations.get(&anchor_qname).ok_or_else(|| {
                                    AuthoringError::Unsupported(format!(
                                        "missing source span for `{anchor_qname}`"
                                    ))
                                })?
                            };
                            let replacement = if let Some(render_qname) = render_qname {
                                if file.module.package.as_ref().is_some_and(|package_model| {
                                    package_model.name.as_dot_string() == render_qname
                                }) {
                                    render_for_splice(
                                        &(self.render_profile.render_package)(
                                            file.module.package.as_ref().expect("package exists"),
                                            0,
                                        ),
                                        node.indent,
                                    )
                                } else if let Some(declaration) = render_declaration_at_qname(
                                    &file.module,
                                    &QualifiedName::parse(&render_qname),
                                    self.render_profile,
                                ) {
                                    render_for_splice(&declaration, node.indent)
                                } else {
                                    return Err(AuthoringError::MissingDeclaration(render_qname));
                                }
                            } else {
                                String::new()
                            };
                            (node.span.clone(), replacement)
                        } else {
                            return Err(AuthoringError::Unsupported(
                                "module-level localized replacement is unsupported".to_string(),
                            ));
                        }
                    }
                };
                spans.push(RenderedSpan {
                    start_line: span.start_line,
                    start_col: span.start_col,
                    end_line: span.end_line,
                    end_col: span.end_col,
                });
                patches.push((span_to_offsets(original, &span)?, replacement));
            }

            patches.sort_by(|left, right| right.0.0.cmp(&left.0.0));
            validate_non_overlapping_patches(&patches)?;
            let mut updated = original.clone();
            for ((start, end), replacement) in patches {
                updated.replace_range(start..end, &replacement);
            }
            edited.insert(file_path.clone(), updated);
            changed_spans.insert(file_path, spans);
        }

        let validation = self.validate_rendered_files(&edited)?;
        Ok((edited, changed_spans, validation))
    }

    fn canonical_rewrite(
        &self,
        changed_files: &BTreeSet<String>,
    ) -> Result<
        (
            BTreeMap<String, String>,
            BTreeMap<String, Vec<RenderedSpan>>,
        ),
        AuthoringError,
    > {
        let mut edited = BTreeMap::new();
        let mut spans = BTreeMap::new();
        for file_path in changed_files {
            let file = self
                .files
                .get(file_path)
                .ok_or_else(|| AuthoringError::MissingFile(file_path.clone()))?;
            let rendered = self.render_module(&file.module);
            let span = rendered_span_for_text(&rendered);
            edited.insert(file_path.clone(), rendered);
            spans.insert(file_path.clone(), vec![span]);
        }
        Ok((edited, spans))
    }

    fn validate_rendered_files(
        &self,
        edited_files: &BTreeMap<String, String>,
    ) -> Result<ValidationReport, AuthoringError> {
        let mut final_texts = self.current_texts();
        for (path, content) in edited_files {
            final_texts.insert(path.clone(), content.clone());
        }

        let Some(compiler) = self.source_compiler else {
            // Without a compiler only the project shape can be checked.
            let expected_count = self.render_all_files().len();
            let actual_count = final_texts.len();
            let ok = expected_count == actual_count;
            return Ok(ValidationReport {
                ok,
                expected_element_count: expected_count,
                actual_element_count: actual_count,
                message: (!ok)
                    .then(|| "rendered files do not match the authoring project shape".to_string()),
            });
        };

        // The canonical render of the in-memory model is the semantic
        // authority; the edited texts must compile to the same elements for
        // the files they touch. A mismatch (or an edited text that fails to
        // compile) reports !ok so write_back_mutation can fall back to the
        // canonical rewrite instead of shipping a mis-spliced patch.
        let canonical = match compiler(&self.render_all_files()) {
            Ok(document) => document,
            Err(err) => {
                return Ok(ValidationReport {
                    ok: false,
                    expected_element_count: 0,
                    actual_element_count: 0,
                    message: Some(format!("canonical render failed to compile: {err}")),
                });
            }
        };
        let edited = match compiler(&final_texts) {
            Ok(document) => document,
            Err(err) => {
                return Ok(ValidationReport {
                    ok: false,
                    expected_element_count: canonical.elements.len(),
                    actual_element_count: 0,
                    message: Some(format!("edited text failed to compile: {err}")),
                });
            }
        };

        let edited_paths = edited_files.keys().cloned().collect::<BTreeSet<_>>();
        let expected = normalized_element_ids_for_files(&canonical, &edited_paths);
        let actual = normalized_element_ids_for_files(&edited, &edited_paths);
        let expected_element_count = expected.values().sum();
        let actual_element_count = actual.values().sum();
        let ok = expected == actual;
        Ok(ValidationReport {
            ok,
            expected_element_count,
            actual_element_count,
            message: (!ok).then(|| {
                let mismatched = expected
                    .keys()
                    .filter(|id| expected.get(*id) != actual.get(*id))
                    .chain(actual.keys().filter(|id| !expected.contains_key(*id)))
                    .cloned()
                    .collect::<BTreeSet<_>>();
                format!(
                    "edited text diverges from the canonical model on: {}",
                    mismatched.into_iter().collect::<Vec<_>>().join(", ")
                )
            }),
        })
    }

    fn compile_user_kir(&self) -> Result<KirDocument, AuthoringError> {
        if let Some(compiler) = self.source_compiler {
            return compiler(&self.render_all_files());
        }

        #[cfg(any(test, feature = "toy-parser"))]
        {
            return Ok(fake_authoring_project_to_kir(self));
        }

        #[cfg(not(any(test, feature = "toy-parser")))]
        Err(AuthoringError::Unsupported(
            "compiling authoring source requires a language-specific compiler".to_string(),
        ))
    }

    fn render_all_files(&self) -> BTreeMap<String, String> {
        self.files
            .iter()
            .map(|(path, file)| (path.clone(), self.render_module(&file.module)))
            .collect()
    }

    fn current_texts(&self) -> BTreeMap<String, String> {
        self.files
            .iter()
            .map(|(path, file)| {
                (
                    path.clone(),
                    file.original_text
                        .clone()
                        .unwrap_or_else(|| self.render_module(&file.module)),
                )
            })
            .collect()
    }

    fn render_module(&self, module: &AuthoringModule) -> String {
        (self.render_profile.render_module)(module)
    }

    fn ensure_file_mut(&mut self, path: &str) -> &mut FileModel {
        self.files
            .entry(path.to_string())
            .or_insert_with(|| FileModel {
                path: path.to_string(),
                module: AuthoringModule::default(),
                original_text: None,
                source_map: None,
            })
    }

    fn finalize_change(
        &self,
        before: KirDocument,
        changed_files: BTreeSet<String>,
        changed_declarations: BTreeSet<String>,
        rewrite_plan: Vec<RewriteInstruction>,
    ) -> Result<MutationResult, AuthoringError> {
        let after = self.compile_user_kir()?;
        let affected_element_ids = diff_element_ids(&before, &after);

        Ok(MutationResult {
            changed_files,
            changed_declarations,
            affected_element_ids,
            rewrite_plan,
        })
    }

    fn ensure_attribute_policy(
        &self,
        element: &QualifiedName,
        attribute: &str,
        policy: AttributeWritePolicy,
    ) -> Result<(), AuthoringError> {
        let attributes = self.semantic_attributes(element)?;
        let Some(row) = attributes.iter().find(|row| row.name == attribute) else {
            if matches!(attribute, "reference_target" | "reference_targets")
                && self
                    .files
                    .values()
                    .any(|file| locate_usage_ref(&file.module, element).is_some())
            {
                return Ok(());
            }
            return Err(AuthoringError::Unsupported(format!(
                "attribute `{attribute}` is not supported on `{}`",
                element.as_dot_string()
            )));
        };
        if matches!(policy, AttributeWritePolicy::DirectOnly) && row.origin_kind != "direct" {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is `{}`; use UpsertDirect to create a direct value",
                element.as_dot_string(),
                row.origin_kind
            )));
        }
        Ok(())
    }

    fn qname_list_attribute_values(
        &self,
        element: &QualifiedName,
        attribute: &str,
    ) -> Result<Vec<QualifiedName>, AuthoringError> {
        let attributes = self.semantic_attributes(element)?;
        let row = attributes
            .into_iter()
            .find(|row| row.name == attribute)
            .ok_or_else(|| {
                AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is not supported on `{}`",
                    element.as_dot_string()
                ))
            })?;
        let values = row
            .direct_value
            .or(row.effective_value)
            .unwrap_or_else(|| Value::Array(Vec::new()));
        value_as_qname_list(&values, attribute)
    }

    fn string_list_attribute_values(
        &self,
        element: &QualifiedName,
        attribute: &str,
    ) -> Result<Vec<String>, AuthoringError> {
        let attributes = self.semantic_attributes(element)?;
        let row = attributes
            .into_iter()
            .find(|row| row.name == attribute)
            .ok_or_else(|| {
                AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is not supported on `{}`",
                    element.as_dot_string()
                ))
            })?;
        let values = row
            .direct_value
            .or(row.effective_value)
            .unwrap_or_else(|| Value::Array(Vec::new()));
        value_as_string_list(&values, attribute)
    }

    fn apply_modifier_flag_edit(
        &mut self,
        element: &QualifiedName,
        attribute: &str,
        modifier: &str,
        enabled: bool,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let changed = match declaration {
            Declaration::Definition(definition) => {
                set_modifier_flag(&mut definition.modifiers, modifier, enabled)
            }
            Declaration::Usage(usage) => set_modifier_flag(&mut usage.modifiers, modifier, enabled),
            other => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is not supported on `{}`",
                    declaration_kind_label(other)
                )));
            }
        };
        if !changed {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is already set to `{enabled}`",
                element.as_dot_string()
            )));
        }
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_short_name_edit(
        &mut self,
        element: &QualifiedName,
        attribute: &str,
        short_name: Option<String>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let changed = match declaration {
            Declaration::Definition(definition) => {
                set_short_name_modifier(&mut definition.modifiers, short_name.as_deref())
            }
            Declaration::Usage(usage) => {
                set_short_name_modifier(&mut usage.modifiers, short_name.as_deref())
            }
            Declaration::Alias(alias) => {
                set_short_name_modifier(&mut alias.modifiers, short_name.as_deref())
            }
            other => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is not supported on `{}`",
                    declaration_kind_label(other)
                )));
            }
        };
        if !changed {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is already set to `{}`",
                element.as_dot_string(),
                short_name.as_deref().unwrap_or("<none>")
            )));
        }
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_raw_body_edit(
        &mut self,
        element: &QualifiedName,
        attribute: &str,
        raw_body: Option<String>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let target = match declaration {
            Declaration::Definition(definition) => &mut definition.raw_body,
            Declaration::Usage(usage) => &mut usage.raw_body,
            other => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is not supported on `{}`",
                    declaration_kind_label(other)
                )));
            }
        };
        let normalized = raw_body.map(|body| body.trim().to_string());
        if *target == normalized {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        *target = normalized;
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_direction_edit(
        &mut self,
        element: &QualifiedName,
        direction: Option<String>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let usage = match declaration {
            Declaration::Usage(usage) => usage,
            _ => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `direction` is only supported on usages"
                )));
            }
        };
        let changed = set_direction(&mut usage.modifiers, direction.as_deref());
        if !changed {
            return Err(AuthoringError::InvalidMutation(format!(
                "direction on `{}` is already `{}`",
                element.as_dot_string(),
                direction.as_deref().unwrap_or("<none>")
            )));
        }
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_usage_qname_list_edit(
        &mut self,
        element: &QualifiedName,
        attribute: &str,
        values: Vec<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let usage = match declaration {
            Declaration::Usage(usage) => usage,
            _ => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is only supported on usages"
                )));
            }
        };
        let target = match attribute {
            "additional_types" => &mut usage.additional_types,
            "subsets" => &mut usage.subsets,
            "redefines" => &mut usage.redefines,
            other => {
                return Err(AuthoringError::Unsupported(format!(
                    "semantic list edit is not supported for attribute `{other}`"
                )));
            }
        };
        if *target == values {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        *target = values;
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_annotated_elements_edit(
        &mut self,
        element: &QualifiedName,
        values: Vec<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let definition = match declaration {
            Declaration::Definition(definition) if definition.keyword == "metadata" => definition,
            Declaration::Definition(_) => {
                return Err(AuthoringError::Unsupported(
                    "attribute `annotated_elements` is only supported on metadata definitions"
                        .to_string(),
                ));
            }
            _ => {
                return Err(AuthoringError::Unsupported(
                    "attribute `annotated_elements` is only supported on definitions".to_string(),
                ));
            }
        };
        if annotated_elements_from_modifiers(&definition.modifiers) == values {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `annotated_elements` on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        set_annotated_elements(&mut definition.modifiers, &values);
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_language_extensions_edit(
        &mut self,
        element: &QualifiedName,
        values: Vec<String>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let modifiers = declaration_modifiers_mut(declaration);
        if language_extensions_from_modifiers(modifiers) == values {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `language_extensions` on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        set_language_extensions(modifiers, &values);
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_usage_reference_target_edit(
        &mut self,
        element: &QualifiedName,
        target: Option<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        self.apply_usage_reference_targets_edit(element, target.into_iter().collect())
    }

    fn apply_usage_reference_targets_edit(
        &mut self,
        element: &QualifiedName,
        targets: Vec<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let usage = locate_usage_mut(&mut file.module, element).ok_or_else(|| {
            AuthoringError::Unsupported(
                "attribute `reference_target` is only supported on usages".to_string(),
            )
        })?;
        if usage_reference_targets(usage) == targets {
            return Err(AuthoringError::InvalidMutation(format!(
                "reference target on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        usage.reference_target = targets.first().cloned();
        set_reference_targets(&mut usage.modifiers, &targets);
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_usage_modifier_value_edit(
        &mut self,
        element: &QualifiedName,
        attribute: &str,
        key: &str,
        value: Option<String>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let usage = match declaration {
            Declaration::Usage(usage) => usage,
            _ => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `{attribute}` is only supported on usages"
                )));
            }
        };
        if modifier_value(&usage.modifiers, key).map(str::to_string) == value {
            return Err(AuthoringError::InvalidMutation(format!(
                "attribute `{attribute}` on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        set_modifier_value(&mut usage.modifiers, key, value.as_deref());
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_usage_multiplicity_edit(
        &mut self,
        element: &QualifiedName,
        multiplicity: Option<MultiplicityRange>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let usage = match declaration {
            Declaration::Usage(usage) => usage,
            _ => {
                return Err(AuthoringError::Unsupported(
                    "attribute `multiplicity` is only supported on usages".to_string(),
                ));
            }
        };
        if usage.multiplicity == multiplicity {
            return Err(AuthoringError::InvalidMutation(format!(
                "multiplicity on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        usage.multiplicity = multiplicity;
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_target_edit(
        &mut self,
        element: &QualifiedName,
        target: Option<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let declaration = locate_declaration_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
        let alias = match declaration {
            Declaration::Alias(alias) => alias,
            _ => {
                return Err(AuthoringError::Unsupported(format!(
                    "attribute `target` is only supported on aliases"
                )));
            }
        };
        if alias.target
            == target
                .clone()
                .unwrap_or_else(|| QualifiedName::new(Vec::new()))
        {
            return Err(AuthoringError::InvalidMutation(format!(
                "target on `{}` is already unchanged",
                element.as_dot_string()
            )));
        }
        alias.target = target.ok_or_else(|| {
            AuthoringError::InvalidMutation("alias target cannot be cleared".to_string())
        })?;
        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceNode {
                file: located.file,
                anchor_qname: element.as_dot_string(),
                render_qname: element.as_dot_string(),
            }],
        )
    }

    fn apply_imports_replace(
        &mut self,
        element: &QualifiedName,
        imports: Vec<QualifiedName>,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        if !matches!(located.kind, DeclarationKind::Package) {
            return Err(AuthoringError::Unsupported(
                "attribute `imports` is only supported on packages".to_string(),
            ));
        }
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
        let package = locate_package_mut(&mut file.module, element)
            .ok_or_else(|| AuthoringError::MissingPackage(element.as_dot_string()))?;
        let mut non_imports = package
            .members
            .iter()
            .filter(|member| !matches!(member, Declaration::Import(_)))
            .cloned()
            .collect::<Vec<_>>();
        let mut new_members = imports
            .into_iter()
            .map(|path| {
                Declaration::Import(Import {
                    path,
                    comments: Vec::new(),
                    docs: Vec::new(),
                    modifiers: Vec::new(),
                })
            })
            .collect::<Vec<_>>();
        new_members.append(&mut non_imports);
        package.members = new_members;

        let mut changed_files = BTreeSet::new();
        changed_files.insert(located.file.clone());
        let mut changed_declarations = BTreeSet::new();
        changed_declarations.insert(element.as_dot_string());
        self.finalize_change(
            before,
            changed_files,
            changed_declarations,
            vec![RewriteInstruction::ReplaceContainer {
                file: located.file,
                anchor_qname: Some(element.as_dot_string()),
                render_qname: Some(element.as_dot_string()),
            }],
        )
    }

    fn apply_doc_edit(
        &mut self,
        element: &QualifiedName,
        edit: DocEdit,
    ) -> Result<MutationResult, AuthoringError> {
        let before = self.compile_user_kir()?;
        let located = self.locate_declaration(element)?;
        let changed_file = located.file.clone();
        let file = self
            .files
            .get_mut(&located.file)
            .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;

        if matches!(located.kind, DeclarationKind::Package) {
            let package = locate_package_mut(&mut file.module, element)
                .ok_or_else(|| AuthoringError::MissingPackage(element.as_dot_string()))?;
            apply_doc_value_edit(&mut package.docs, edit);
        } else {
            let declaration = locate_declaration_mut(&mut file.module, element)
                .ok_or_else(|| AuthoringError::MissingDeclaration(element.as_dot_string()))?;
            apply_doc_value_edit(declaration_docs_mut(declaration), edit);
        }

        self.finalize_change(
            before,
            BTreeSet::from([changed_file.clone()]),
            BTreeSet::from([element.as_dot_string()]),
            vec![RewriteInstruction::ReplaceContainer {
                file: changed_file,
                anchor_qname: Some(element.as_dot_string()),
                render_qname: Some(element.as_dot_string()),
            }],
        )
    }

    fn locate_declaration(
        &self,
        qualified_name: &QualifiedName,
    ) -> Result<LocatedDeclaration, AuthoringError> {
        let key = qualified_name.as_dot_string();
        let mut found = Vec::new();
        for (file_path, file) in &self.files {
            if let Some((kind, parent)) = locate_declaration_in_module(&file.module, qualified_name)
            {
                found.push(LocatedDeclaration {
                    file: file_path.clone(),
                    kind,
                    parent_qname: parent,
                });
            }
        }
        match found.len() {
            0 => Err(AuthoringError::MissingDeclaration(key)),
            1 => Ok(found.remove(0)),
            _ => Err(AuthoringError::InvalidMutation(format!(
                "ambiguous declaration ownership for `{key}`"
            ))),
        }
    }

    fn push_into_container(
        &mut self,
        container: ContainerSelector,
        declaration: Declaration,
    ) -> Result<(String, Option<String>, RewriteInstruction), AuthoringError> {
        match container {
            ContainerSelector::File { target_file } => {
                let file = self.ensure_file_mut(&target_file);
                file.module.members.push(declaration);
                Ok((
                    target_file.clone(),
                    None,
                    RewriteInstruction::FullFile { file: target_file },
                ))
            }
            ContainerSelector::Package { qualified_name } => {
                let located = self.locate_declaration(&qualified_name).or_else(|_| {
                    self.file_for_package(&qualified_name)
                        .map(|file| LocatedDeclaration {
                            file,
                            kind: DeclarationKind::Package,
                            parent_qname: None,
                        })
                        .ok_or_else(|| {
                            AuthoringError::MissingPackage(qualified_name.as_dot_string())
                        })
                })?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                let package =
                    locate_package_mut(&mut file.module, &qualified_name).ok_or_else(|| {
                        AuthoringError::MissingPackage(qualified_name.as_dot_string())
                    })?;
                package.members.push(declaration);
                Ok((
                    located.file.clone(),
                    Some(qualified_name.as_dot_string()),
                    RewriteInstruction::ReplaceContainer {
                        file: located.file,
                        anchor_qname: Some(qualified_name.as_dot_string()),
                        render_qname: Some(qualified_name.as_dot_string()),
                    },
                ))
            }
            ContainerSelector::Declaration { qualified_name } => {
                let located = self.locate_declaration(&qualified_name)?;
                let file = self
                    .files
                    .get_mut(&located.file)
                    .ok_or_else(|| AuthoringError::MissingFile(located.file.clone()))?;
                let members =
                    locate_members_mut(&mut file.module, &qualified_name).ok_or_else(|| {
                        AuthoringError::MissingDeclaration(qualified_name.as_dot_string())
                    })?;
                members.push(declaration);
                Ok((
                    located.file.clone(),
                    Some(qualified_name.as_dot_string()),
                    RewriteInstruction::ReplaceContainer {
                        file: located.file,
                        anchor_qname: Some(qualified_name.as_dot_string()),
                        render_qname: Some(qualified_name.as_dot_string()),
                    },
                ))
            }
        }
    }

    fn file_for_package(&self, package_name: &QualifiedName) -> Option<String> {
        self.files.iter().find_map(|(path, file)| {
            file.module
                .package
                .as_ref()
                .filter(|package| package.name == *package_name)
                .map(|_| path.clone())
        })
    }
}

impl AuthoringModule {
    fn from_ast(module: &ParsedModule) -> Self {
        let members = if module.package.is_some() {
            module
                .members
                .iter()
                .filter(|member| !matches!(member, AstDeclaration::Package(_)))
                .map(Declaration::from_ast)
                .collect()
        } else {
            module.members.iter().map(Declaration::from_ast).collect()
        };
        Self {
            package: module.package.as_ref().map(Package::from_ast),
            members,
        }
    }

    fn render(&self) -> String {
        let mut sections = Vec::new();
        if let Some(package) = &self.package {
            sections.push(render_with_leading_comments(
                &package.comments,
                package.render(0),
                0,
            ));
        }
        sections.extend(
            self.members
                .iter()
                .map(|member| render_declaration_with_comments(member, 0)),
        );
        if sections.is_empty() {
            String::new()
        } else {
            format!("{}\n", sections.join("\n\n"))
        }
    }
}

impl Package {
    fn from_ast(package: &PackageDecl) -> Self {
        Self {
            name: QualifiedName(package.name.segments.clone()),
            members: package.members.iter().map(Declaration::from_ast).collect(),
            comments: package.comments.clone(),
            docs: package.docs.clone(),
            modifiers: package.modifiers.clone(),
        }
    }

    fn render(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        let mut lines = render_docs(&self.docs, indent);
        let mut header = String::new();
        if !self.modifiers.is_empty() {
            header.push_str(&self.modifiers.join(" "));
            header.push(' ');
        }
        header.push_str("package ");
        header.push_str(&render_qname(&self.name));
        header.push_str(" {");
        lines.push(format!("{prefix}{header}"));
        if !self.members.is_empty() {
            let body = self
                .members
                .iter()
                .map(|member| render_declaration_with_comments(member, indent + 2))
                .collect::<Vec<_>>()
                .join("\n\n");
            lines.push(body);
        }
        lines.push(format!("{prefix}}}"));
        lines.join("\n")
    }
}

impl Definition {
    fn render(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        let mut lines = render_docs(&self.docs, indent);
        let mut header = render_modifier_prefix(&self.modifiers);
        header.push_str(&render_keyword(&self.keyword, &self.modifiers));
        header.push_str(" def ");
        header.push_str(&render_angle_adornment_prefix(&self.modifiers));
        header.push_str(&render_name_segment(&self.name));
        if !self.specializes.is_empty() {
            header.push_str(" specializes ");
            header.push_str(
                &self
                    .specializes
                    .iter()
                    .map(render_qname)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        let annotated_elements = annotated_elements_from_modifiers(&self.modifiers);
        if self.members.is_empty() && self.raw_body.is_none() && annotated_elements.is_empty() {
            header.push(';');
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }

        header.push_str(" {");
        lines.push(format!("{prefix}{header}"));
        let body = if self.keyword == "metadata" && !annotated_elements.is_empty() {
            render_metadata_definition_body(
                &annotated_elements,
                &self.members,
                self.raw_body.as_deref(),
                indent + 2,
            )
        } else {
            render_member_and_raw_body(&self.members, self.raw_body.as_deref(), indent + 2)
        };
        if !body.is_empty() {
            lines.push(body);
        }
        lines.push(format!("{prefix}}}"));
        lines.join("\n")
    }
}

impl Usage {
    fn render(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        let mut lines = render_docs(&self.docs, indent);
        let mut header = render_modifier_prefix(&self.modifiers);
        if self.keyword == "metadata" && is_metadata_usage_modifier(&self.modifiers) {
            header.push_str("metadata ");
            header.push_str(&render_angle_adornment_prefix(&self.modifiers));
            if !self.is_implicit_name {
                header.push_str(&render_name_segment(&self.name));
            }
            if let Some(ty) = &self.ty {
                if !self.is_implicit_name {
                    header.push_str(": ");
                } else {
                    header.push_str(": ");
                }
                header.push_str(&render_qname(ty));
            }
            let reference_targets = usage_reference_targets(self);
            if !reference_targets.is_empty() {
                header.push_str(" about ");
                header.push_str(
                    &reference_targets
                        .iter()
                        .map(render_qname)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            append_usage_relations(&mut header, self);
            if self.members.is_empty() && self.raw_body.is_none() {
                header.push(';');
                lines.push(format!("{prefix}{header}"));
                return lines.join("\n");
            }
            header.push_str(" {");
            lines.push(format!("{prefix}{header}"));
            let body =
                render_member_and_raw_body(&self.members, self.raw_body.as_deref(), indent + 2);
            if !body.is_empty() {
                lines.push(body);
            }
            lines.push(format!("{prefix}}}"));
            return lines.join("\n");
        }
        if self.keyword == "metadata" {
            header.push('@');
            header.push_str(&self.name.replace('.', "::"));
            if self.metadata_properties.is_empty() {
                header.push(';');
                lines.push(format!("{prefix}{header}"));
                return lines.join("\n");
            }
            header.push_str(" {");
            lines.push(format!("{prefix}{header}"));
            for (key, value) in &self.metadata_properties {
                lines.push(format!(
                    "{}  {key} = {};",
                    prefix,
                    render_metadata_property_value(value)
                ));
            }
            lines.push(format!("{prefix}}}"));
            return lines.join("\n");
        }
        if self.keyword == "satisfy"
            && let Some(reference_target) = &self.reference_target
        {
            header.push_str("satisfy requirement ");
            header.push_str(reference_target.tail().unwrap_or("target"));
            header.push(';');
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }
        if let Some(rendered) = render_relationship_shorthand(self) {
            header.push_str(&rendered);
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }
        if let Some(rendered) = render_transition_shorthand(self) {
            header.push_str(&rendered);
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }
        if self.keyword == "perform" && self.members.is_empty() && self.ty.is_none() {
            header.push_str("perform action ");
            if !self.is_implicit_name {
                header.push_str(&render_name_segment(&self.name));
            }
            header.push(';');
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }
        header.push_str(&render_keyword(&self.keyword, &self.modifiers));
        header.push(' ');
        header.push_str(&render_angle_adornment_prefix(&self.modifiers));
        if !self.is_implicit_name {
            header.push_str(&render_name_segment(&self.name));
        }
        if let Some(ty) = &self.ty {
            if !self.is_implicit_name {
                header.push_str(": ");
            } else {
                header.push_str(": ");
            }
            header.push_str(&render_qname(ty));
        }
        if let Some(multiplicity) = &self.multiplicity {
            header.push('[');
            header.push_str(&multiplicity.raw);
            header.push(']');
        }
        if !self.additional_types.is_empty() {
            header.push_str(" :> ");
            header.push_str(
                &self
                    .additional_types
                    .iter()
                    .map(render_qname)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !self.specializes.is_empty() {
            header.push_str(" specializes ");
            header.push_str(
                &self
                    .specializes
                    .iter()
                    .map(render_qname)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !self.subsets.is_empty() {
            header.push_str(" subsets ");
            header.push_str(
                &self
                    .subsets
                    .iter()
                    .map(render_qname)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !self.redefines.is_empty() {
            header.push_str(" redefines ");
            header.push_str(
                &self
                    .redefines
                    .iter()
                    .map(render_qname)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(reference_target) = &self.reference_target {
            header.push_str(" references ");
            header.push_str(&reference_target.as_dot_string());
        }
        if let Some(expression) = &self.expression {
            header.push_str(" = ");
            header.push_str(expression);
        }
        if self.members.is_empty() && self.raw_body.is_none() {
            header.push(';');
            lines.push(format!("{prefix}{header}"));
            return lines.join("\n");
        }

        header.push_str(" {");
        lines.push(format!("{prefix}{header}"));
        let body = render_member_and_raw_body(&self.members, self.raw_body.as_deref(), indent + 2);
        if !body.is_empty() {
            lines.push(body);
        }
        lines.push(format!("{prefix}}}"));
        lines.join("\n")
    }
}

impl Alias {
    fn render(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        let mut lines = render_docs(&self.docs, indent);
        let mut header = render_modifier_prefix(&self.modifiers);
        header.push_str("alias ");
        header.push_str(&render_angle_adornment_prefix(&self.modifiers));
        header.push_str(&render_name_segment(&self.name));
        header.push_str(" = ");
        header.push_str(&render_qname(&self.target));
        header.push(';');
        lines.push(format!("{prefix}{header}"));
        lines.join("\n")
    }
}

impl Declaration {
    fn from_ast(declaration: &AstDeclaration) -> Self {
        if let Some(definition) = declaration.as_definition_like() {
            return Self::Definition(definition_from_ast_like(&definition));
        }
        if let Some(usage) = declaration.as_usage_like() {
            return Self::Usage(usage_from_ast_like(&usage));
        }

        match declaration {
            AstDeclaration::Package(package) => Self::Package(Package::from_ast(package)),
            AstDeclaration::Import(import) => Self::Import(Import {
                path: QualifiedName(import.path.segments.clone()),
                comments: import.comments.clone(),
                docs: import.docs.clone(),
                modifiers: import.modifiers.clone(),
            }),
            AstDeclaration::Alias(alias) => Self::Alias(Alias {
                name: alias.name.clone(),
                target: QualifiedName(alias.target.segments.clone()),
                comments: alias.comments.clone(),
                docs: alias.docs.clone(),
                modifiers: alias.modifiers.clone(),
            }),
            _ => unreachable!("definition-like and usage-like declarations are handled above"),
        }
    }

    /// Leading own-line comments preserved from the source. Rendered by the
    /// container (see `render_declaration_with_comments`), never by
    /// `render(..)` itself, so localized splices cannot duplicate them.
    pub fn comments(&self) -> &[CommentNote] {
        match self {
            Self::Package(package) => &package.comments,
            Self::Import(import) => &import.comments,
            Self::Definition(definition) => &definition.comments,
            Self::Usage(usage) => &usage.comments,
            Self::Alias(alias) => &alias.comments,
        }
    }

    fn render(&self, indent: usize) -> String {
        match self {
            Self::Package(package) => package.render(indent),
            Self::Import(import) => {
                let prefix = " ".repeat(indent);
                let mut lines = render_docs(&import.docs, indent);
                let mut header = render_modifier_prefix(&import.modifiers);
                header.push_str("import ");
                header.push_str(&render_qname(&import.path));
                header.push(';');
                lines.push(format!("{prefix}{header}"));
                lines.join("\n")
            }
            Self::Definition(definition) => definition.render(indent),
            Self::Usage(usage) => usage.render(indent),
            Self::Alias(alias) => alias.render(indent),
        }
    }
}

fn definition_from_ast_like(
    definition: &crate::authoring::frontend::ast::GenericDefinitionDecl,
) -> Definition {
    Definition {
        keyword: definition.keyword.clone(),
        name: definition.name.clone(),
        specializes: definition
            .specializes
            .iter()
            .map(|name| QualifiedName(name.segments.clone()))
            .collect(),
        members: definition
            .members
            .iter()
            .map(Declaration::from_ast)
            .collect(),
        raw_body: None,
        comments: definition.comments.clone(),
        docs: definition.docs.clone(),
        modifiers: definition.modifiers.clone(),
    }
}

fn usage_from_ast_like(usage: &crate::authoring::frontend::ast::GenericUsageDecl) -> Usage {
    let mut modifiers = usage.modifiers.clone();
    if usage.keyword == "metadata"
        && usage.metadata_properties.is_empty()
        && (usage.ty.is_some()
            || usage.reference_target.is_some()
            || !usage.body_members.is_empty()
            || modifiers
                .iter()
                .any(|modifier| modifier == "metadata_usage"))
        && !modifiers
            .iter()
            .any(|modifier| modifier == "metadata_usage")
    {
        modifiers.push("metadata_usage".to_string());
    }
    Usage {
        keyword: usage.keyword.clone(),
        name: usage.name.clone(),
        is_implicit_name: usage.is_implicit_name,
        ty: usage
            .ty
            .as_ref()
            .map(|ty| QualifiedName(ty.segments.clone())),
        reference_target: usage
            .reference_target
            .as_ref()
            .map(|target| QualifiedName(target.segments.clone())),
        metadata_properties: usage.metadata_properties.clone(),
        multiplicity: usage.multiplicity.clone(),
        expression: usage.expression.as_ref().map(render_expr),
        additional_types: usage
            .additional_types
            .iter()
            .map(|name| QualifiedName(name.segments.clone()))
            .collect(),
        specializes: usage
            .specializes
            .iter()
            .map(|name| QualifiedName(name.segments.clone()))
            .collect(),
        subsets: usage
            .subsets
            .iter()
            .map(|name| QualifiedName(name.segments.clone()))
            .collect(),
        redefines: usage
            .redefines
            .iter()
            .map(|name| QualifiedName(name.segments.clone()))
            .collect(),
        members: usage
            .body_members
            .iter()
            .map(Declaration::from_ast)
            .collect(),
        raw_body: None,
        comments: usage.comments.clone(),
        docs: usage.docs.clone(),
        modifiers,
    }
}

impl FileSourceMap {
    fn from_ast(module: &ParsedModule, original_text: Option<&str>) -> Self {
        let lines = original_text.map(|text| text.lines().collect::<Vec<_>>());
        let lines = lines.as_deref();
        let mut map = Self {
            package: module
                .package
                .as_ref()
                .map(|package| source_node_covering_docs(&package.span, package.docs.len(), lines)),
            declarations: BTreeMap::new(),
        };
        if let Some(package) = &module.package {
            collect_source_nodes(
                &package.members,
                &package.name.segments.join("."),
                Some(&package.name.segments.join(".")),
                lines,
                &mut map.declarations,
            );
        }
        if module.package.is_none() {
            collect_source_nodes(&module.members, "", None, lines, &mut map.declarations);
        }
        map
    }
}

fn collect_source_nodes(
    declarations: &[AstDeclaration],
    owner: &str,
    parent_qname: Option<&str>,
    lines: Option<&[&str]>,
    nodes: &mut BTreeMap<String, SourceNode>,
) {
    for declaration in declarations {
        if let Some(definition) = declaration.as_definition_like() {
            let qname = qualify_name(owner, &definition.name);
            nodes.insert(
                qname.clone(),
                source_node_covering_docs(&definition.span, definition.docs.len(), lines),
            );
            collect_source_nodes(
                &definition.members,
                &qname,
                Some(parent_or_self(parent_qname, &qname)),
                lines,
                nodes,
            );
            continue;
        }
        if let Some(usage) = declaration.as_usage_like() {
            let qname = qualify_name(owner, &usage.name);
            nodes.insert(
                qname.clone(),
                source_node_covering_docs(&usage.span, usage.docs.len(), lines),
            );
            collect_source_nodes(
                &usage.body_members,
                &qname,
                Some(parent_or_self(parent_qname, &qname)),
                lines,
                nodes,
            );
            continue;
        }

        match declaration {
            AstDeclaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.segments.join("."));
                nodes.insert(
                    qname.clone(),
                    source_node_covering_docs(&package.span, package.docs.len(), lines),
                );
                collect_source_nodes(&package.members, &qname, Some(&qname), lines, nodes);
            }
            AstDeclaration::Import(_) => {}
            AstDeclaration::Alias(alias) => {
                let qname = qualify_name(owner, &alias.name);
                nodes.insert(
                    qname,
                    source_node_covering_docs(&alias.span, alias.docs.len(), lines),
                );
            }
            _ => unreachable!("definition-like and usage-like declarations are handled above"),
        }
    }
}

/// Builds the source node for a declaration, widening the recorded span
/// upward over the declaration's own leading `doc /* ... */` lines. The
/// canonical printer emits docs immediately above the declaration keyword
/// while the parse-time span starts at the keyword itself; a localized
/// splice re-renders the declaration *including* its docs, so the span must
/// cover the doc bytes already in the source or the splice would duplicate
/// them. When the source layout cannot be proven to be exactly `docs`
/// adjacent doc annotations, the span is left unwidened and write-back
/// falls back to the canonical rewrite exactly as before.
fn source_node_covering_docs(span: &SourceSpan, docs: usize, lines: Option<&[&str]>) -> SourceNode {
    let indent = span.start_col.saturating_sub(1);
    let mut span = span.clone();
    if docs > 0
        && let Some(lines) = lines
        && let Some((start_line, start_col)) = doc_run_start(lines, span.start_line, docs)
    {
        span.start_line = start_line;
        span.start_col = start_col;
    }
    SourceNode { span, indent }
}

/// 1-based start position of the run of exactly `docs` doc annotations
/// sitting directly above `decl_start_line`, or `None` when those lines do
/// not form one.
fn doc_run_start(lines: &[&str], decl_start_line: usize, docs: usize) -> Option<(usize, usize)> {
    let mut line = decl_start_line;
    for _ in 0..docs {
        line = doc_block_start_above(lines, line)?;
    }
    let text = lines.get(line.checked_sub(1)?)?;
    let col = text.len() - text.trim_start().len() + 1;
    Some((line, col))
}

/// First line of the `doc /* ... */` annotation whose last line sits
/// directly above `below_line`, or `None` if those lines don't form exactly
/// one doc annotation.
fn doc_block_start_above(lines: &[&str], below_line: usize) -> Option<usize> {
    let end_line = below_line.checked_sub(1)?;
    if end_line == 0 || !lines.get(end_line - 1)?.trim_end().ends_with("*/") {
        return None;
    }
    let mut start_line = end_line;
    loop {
        if lines.get(start_line - 1)?.trim_start().starts_with("doc") {
            let block = lines[start_line - 1..end_line].join("\n");
            return is_single_doc_annotation(&block).then_some(start_line);
        }
        start_line -= 1;
        if start_line == 0 {
            return None;
        }
    }
}

/// True when `block` is exactly one `doc /* ... */` annotation — the `doc`
/// keyword, one block comment, and nothing else.
fn is_single_doc_annotation(block: &str) -> bool {
    let Some(rest) = block.trim_start().strip_prefix("doc") else {
        return false;
    };
    let Some(interior) = rest.trim_start().strip_prefix("/*") else {
        return false;
    };
    let Some(close) = interior.find("*/") else {
        return false;
    };
    interior[close + 2..].trim().is_empty()
}

fn parent_or_self<'a>(parent_qname: Option<&'a str>, fallback: &'a str) -> &'a str {
    parent_qname.unwrap_or(fallback)
}

fn locate_declaration_in_module(
    module: &AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<(DeclarationKind, Option<String>)> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return Some((DeclarationKind::Package, None));
    }
    if let Some(package) = &module.package
        && let Some(result) = locate_declaration_in_members(
            &package.members,
            &package.name.as_dot_string(),
            qualified_name,
        )
    {
        return Some(result);
    }
    locate_declaration_in_members(&module.members, "", qualified_name)
}

fn locate_declaration_in_members(
    declarations: &[Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<(DeclarationKind, Option<String>)> {
    for declaration in declarations {
        match declaration {
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if qname == qualified_name.as_dot_string() {
                    return Some((
                        DeclarationKind::Package,
                        (!owner.is_empty()).then(|| owner.to_string()),
                    ));
                }
                if let Some(result) =
                    locate_declaration_in_members(&package.members, &qname, qualified_name)
                {
                    return Some(result);
                }
            }
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if qname == qualified_name.as_dot_string() {
                    return Some((
                        DeclarationKind::Definition,
                        (!owner.is_empty()).then(|| owner.to_string()),
                    ));
                }
                if let Some(result) =
                    locate_declaration_in_members(&definition.members, &qname, qualified_name)
                {
                    return Some(result);
                }
            }
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    return Some((
                        DeclarationKind::Usage,
                        (!owner.is_empty()).then(|| owner.to_string()),
                    ));
                }
                if let Some(result) =
                    locate_declaration_in_members(&usage.members, &qname, qualified_name)
                {
                    return Some(result);
                }
            }
            Declaration::Alias(alias) => {
                let qname = qualify_name(owner, &alias.name);
                if qname == qualified_name.as_dot_string() {
                    return Some((
                        DeclarationKind::Alias,
                        (!owner.is_empty()).then(|| owner.to_string()),
                    ));
                }
            }
            Declaration::Import(_) => {}
        }
    }
    None
}

fn locate_package_mut<'a>(
    module: &'a mut AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Package> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return module.package.as_mut();
    }
    if let Some(package) = &mut module.package {
        return locate_package_in_members_mut(
            &mut package.members,
            &package.name.as_dot_string(),
            qualified_name,
        );
    }
    locate_package_in_members_mut(&mut module.members, "", qualified_name)
}

fn locate_package_ref<'a>(
    module: &'a AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a Package> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return module.package.as_ref();
    }
    if let Some(package) = &module.package
        && let Some(found) = locate_package_in_members_ref(
            &package.members,
            &package.name.as_dot_string(),
            qualified_name,
        )
    {
        return Some(found);
    }
    locate_package_in_members_ref(&module.members, "", qualified_name)
}

fn locate_package_in_members_mut<'a>(
    declarations: &'a mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Package> {
    for declaration in declarations {
        if let Declaration::Package(package) = declaration {
            let qname = qualify_name(owner, &package.name.as_dot_string());
            if qname == qualified_name.as_dot_string() {
                return Some(package);
            }
            if let Some(found) =
                locate_package_in_members_mut(&mut package.members, &qname, qualified_name)
            {
                return Some(found);
            }
        }
    }
    None
}

fn locate_package_in_members_ref<'a>(
    declarations: &'a [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a Package> {
    for declaration in declarations {
        if let Declaration::Package(package) = declaration {
            let qname = qualify_name(owner, &package.name.as_dot_string());
            if qname == qualified_name.as_dot_string() {
                return Some(package);
            }
            if let Some(found) =
                locate_package_in_members_ref(&package.members, &qname, qualified_name)
            {
                return Some(found);
            }
        }
    }
    None
}

fn locate_members_mut<'a>(
    module: &'a mut AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Vec<Declaration>> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return module.package.as_mut().map(|package| &mut package.members);
    }
    if let Some(package) = &mut module.package
        && let Some(found) = locate_members_in_declarations_mut(
            &mut package.members,
            &package.name.as_dot_string(),
            qualified_name,
        )
    {
        return Some(found);
    }
    locate_members_in_declarations_mut(&mut module.members, "", qualified_name)
}

fn locate_members_in_declarations_mut<'a>(
    declarations: &'a mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Vec<Declaration>> {
    for declaration in declarations {
        match declaration {
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if qname == qualified_name.as_dot_string() {
                    return Some(&mut package.members);
                }
                if let Some(found) =
                    locate_members_in_declarations_mut(&mut package.members, &qname, qualified_name)
                {
                    return Some(found);
                }
            }
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if qname == qualified_name.as_dot_string() {
                    return Some(&mut definition.members);
                }
                if let Some(found) = locate_members_in_declarations_mut(
                    &mut definition.members,
                    &qname,
                    qualified_name,
                ) {
                    return Some(found);
                }
            }
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    return Some(&mut usage.members);
                }
                if let Some(found) =
                    locate_members_in_declarations_mut(&mut usage.members, &qname, qualified_name)
                {
                    return Some(found);
                }
            }
            Declaration::Import(_) | Declaration::Alias(_) => {}
        }
    }
    None
}

fn locate_declaration_ref<'a>(
    module: &'a AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a Declaration> {
    if let Some(package) = &module.package
        && let Some(found) = locate_declaration_in_members_ref(
            &package.members,
            &package.name.as_dot_string(),
            qualified_name,
        )
    {
        return Some(found);
    }
    locate_declaration_in_members_ref(&module.members, "", qualified_name)
}

fn locate_declaration_in_members_ref<'a>(
    declarations: &'a [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a Declaration> {
    for declaration in declarations {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string() {
            return Some(declaration);
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                locate_declaration_in_members_ref(&package.members, &qname, qualified_name)
            }
            Declaration::Definition(definition) => {
                locate_declaration_in_members_ref(&definition.members, &qname, qualified_name)
            }
            Declaration::Usage(usage) => {
                locate_declaration_in_members_ref(&usage.members, &qname, qualified_name)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if let Some(found) = nested {
            return Some(found);
        }
    }
    None
}

fn locate_usage_ref<'a>(
    module: &'a AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a Usage> {
    if let Some(package) = &module.package
        && let Some(found) = locate_usage_in_members_ref(
            &package.members,
            &package.name.as_dot_string(),
            qualified_name,
        )
    {
        return Some(found);
    }
    locate_usage_in_members_ref(&module.members, "", qualified_name)
}

fn locate_usage_in_members_ref<'a>(
    declarations: &'a [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a Usage> {
    for declaration in declarations {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string()
            && let Declaration::Usage(usage) = declaration
        {
            return Some(usage);
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                locate_usage_in_members_ref(&package.members, &qname, qualified_name)
            }
            Declaration::Definition(definition) => {
                locate_usage_in_members_ref(&definition.members, &qname, qualified_name)
            }
            Declaration::Usage(usage) => {
                locate_usage_in_members_ref(&usage.members, &qname, qualified_name)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if let Some(found) = nested {
            return Some(found);
        }
    }
    None
}

fn locate_declaration_mut<'a>(
    module: &'a mut AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Declaration> {
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && let Some(found) =
            locate_declaration_in_members_mut(&mut package.members, &package_owner, qualified_name)
    {
        return Some(found);
    }
    locate_declaration_in_members_mut(&mut module.members, "", qualified_name)
}

fn locate_declaration_in_members_mut<'a>(
    declarations: &'a mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Declaration> {
    for declaration in declarations {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string() {
            return Some(declaration);
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                locate_declaration_in_members_mut(&mut package.members, &qname, qualified_name)
            }
            Declaration::Definition(definition) => {
                locate_declaration_in_members_mut(&mut definition.members, &qname, qualified_name)
            }
            Declaration::Usage(usage) => {
                locate_declaration_in_members_mut(&mut usage.members, &qname, qualified_name)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if nested.is_some() {
            return nested;
        }
    }
    None
}

fn locate_usage_mut<'a>(
    module: &'a mut AuthoringModule,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Usage> {
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && let Some(found) =
            locate_usage_in_members_mut(&mut package.members, &package_owner, qualified_name)
    {
        return Some(found);
    }
    locate_usage_in_members_mut(&mut module.members, "", qualified_name)
}

fn locate_usage_in_members_mut<'a>(
    declarations: &'a mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<&'a mut Usage> {
    for declaration in declarations {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string()
            && let Declaration::Usage(usage) = declaration
        {
            return Some(usage);
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                locate_usage_in_members_mut(&mut package.members, &qname, qualified_name)
            }
            Declaration::Definition(definition) => {
                locate_usage_in_members_mut(&mut definition.members, &qname, qualified_name)
            }
            Declaration::Usage(usage) => {
                locate_usage_in_members_mut(&mut usage.members, &qname, qualified_name)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if nested.is_some() {
            return nested;
        }
    }
    None
}

fn semantic_attributes_for_package(package: &Package) -> Vec<SemanticAttribute> {
    vec![
        SemanticAttribute {
            name: "declared_name".to_string(),
            origin_kind: "direct".to_string(),
            direct_value: Some(Value::String(
                package.name.tail().unwrap_or_default().to_string(),
            )),
            effective_value: Some(Value::String(
                package.name.tail().unwrap_or_default().to_string(),
            )),
        },
        SemanticAttribute {
            name: "imports".to_string(),
            origin_kind: if package
                .members
                .iter()
                .any(|member| matches!(member, Declaration::Import(_)))
            {
                "direct".to_string()
            } else {
                "declared".to_string()
            },
            direct_value: Some(Value::Array(
                package
                    .members
                    .iter()
                    .filter_map(|member| match member {
                        Declaration::Import(import) => {
                            Some(Value::String(import.path.as_colon_string()))
                        }
                        _ => None,
                    })
                    .collect(),
            )),
            effective_value: Some(Value::Array(
                package
                    .members
                    .iter()
                    .filter_map(|member| match member {
                        Declaration::Import(import) => {
                            Some(Value::String(import.path.as_colon_string()))
                        }
                        _ => None,
                    })
                    .collect(),
            )),
        },
    ]
}

fn semantic_attributes_for_declaration(declaration: &Declaration) -> Vec<SemanticAttribute> {
    match declaration {
        Declaration::Package(package) => semantic_attributes_for_package(package),
        Declaration::Definition(definition) => vec![
            semantic_scalar_attribute(
                "declared_name",
                Some(Value::String(definition.name.clone())),
                true,
            ),
            semantic_short_name_attribute(&definition.modifiers),
            semantic_effective_short_name_attribute(&definition.modifiers, &definition.name),
            semantic_doc_attribute("doc", &definition.docs),
            semantic_doc_attribute(
                "text",
                &text_from_docs(&definition.docs)
                    .into_iter()
                    .collect::<Vec<_>>(),
            ),
            semantic_doc_attribute(
                "id",
                &id_from_docs(&definition.docs)
                    .into_iter()
                    .collect::<Vec<_>>(),
            ),
            semantic_language_extensions_attribute(&definition.modifiers),
            semantic_language_extension_keyword_attribute(&definition.modifiers),
            semantic_raw_body_attribute(&definition.raw_body),
            semantic_annotated_elements_attribute(&definition.modifiers),
            semantic_list_attribute("specializes", &definition.specializes),
            semantic_scalar_attribute(
                "is_abstract",
                Some(Value::Bool(
                    definition
                        .modifiers
                        .iter()
                        .any(|modifier| modifier == "abstract"),
                )),
                definition
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "abstract"),
            ),
            semantic_modifier_attribute("is_derived", "derived", &definition.modifiers),
            semantic_modifier_attribute("is_individual", "individual", &definition.modifiers),
        ],
        Declaration::Usage(usage) => vec![
            semantic_scalar_attribute(
                "declared_name",
                Some(Value::String(usage.name.clone())),
                true,
            ),
            semantic_short_name_attribute(&usage.modifiers),
            semantic_effective_short_name_attribute(&usage.modifiers, &usage.name),
            semantic_doc_attribute("doc", &usage.docs),
            semantic_doc_attribute(
                "text",
                &text_from_docs(&usage.docs).into_iter().collect::<Vec<_>>(),
            ),
            semantic_doc_attribute(
                "id",
                &id_from_docs(&usage.docs).into_iter().collect::<Vec<_>>(),
            ),
            semantic_language_extensions_attribute(&usage.modifiers),
            semantic_language_extension_keyword_attribute(&usage.modifiers),
            semantic_raw_body_attribute(&usage.raw_body),
            SemanticAttribute {
                name: "type".to_string(),
                origin_kind: if usage.ty.is_some() {
                    "direct".to_string()
                } else {
                    "declared".to_string()
                },
                direct_value: usage
                    .ty
                    .as_ref()
                    .map(|value| Value::String(value.as_colon_string())),
                effective_value: usage
                    .ty
                    .as_ref()
                    .map(|value| Value::String(value.as_colon_string())),
            },
            semantic_scalar_attribute(
                "multiplicity",
                usage
                    .multiplicity
                    .as_ref()
                    .map(|multiplicity| Value::String(multiplicity.raw.clone())),
                usage.multiplicity.is_some(),
            ),
            semantic_list_attribute("specializes", &usage.specializes),
            semantic_list_attribute("additional_types", &usage.additional_types),
            semantic_list_attribute("subsets", &usage.subsets),
            semantic_list_attribute("redefines", &usage.redefines),
            semantic_scalar_attribute(
                "reference_target",
                usage
                    .reference_target
                    .as_ref()
                    .map(|target| Value::String(target.as_colon_string())),
                usage.reference_target.is_some(),
            ),
            semantic_list_attribute("reference_targets", &usage_reference_targets(usage)),
            semantic_scalar_attribute(
                "is_abstract",
                Some(Value::Bool(
                    usage
                        .modifiers
                        .iter()
                        .any(|modifier| modifier == "abstract"),
                )),
                usage
                    .modifiers
                    .iter()
                    .any(|modifier| modifier == "abstract"),
            ),
            semantic_scalar_attribute(
                "is_end",
                Some(Value::Bool(
                    usage.modifiers.iter().any(|modifier| modifier == "end"),
                )),
                usage.modifiers.iter().any(|modifier| modifier == "end"),
            ),
            semantic_modifier_attribute("is_derived", "derived", &usage.modifiers),
            semantic_modifier_attribute("is_individual", "individual", &usage.modifiers),
            semantic_modifier_attribute("is_ordered", "ordered", &usage.modifiers),
            semantic_modifier_attribute("is_variable", "variable", &usage.modifiers),
            semantic_unique_attribute(&usage.modifiers),
            semantic_modifier_value_attribute(
                "transition_source",
                "transition_source",
                &usage.modifiers,
            ),
            semantic_modifier_value_attribute(
                "transition_target",
                "transition_target",
                &usage.modifiers,
            ),
            semantic_modifier_value_attribute("trigger", "trigger", &usage.modifiers),
            semantic_modifier_value_attribute("trigger_kind", "trigger_kind", &usage.modifiers),
            semantic_modifier_attribute("source_is_initial", "source_is_initial", &usage.modifiers),
            SemanticAttribute {
                name: "direction".to_string(),
                origin_kind: usage
                    .modifiers
                    .iter()
                    .find(|modifier| matches!(modifier.as_str(), "in" | "out" | "inout"))
                    .map(|_| "direct".to_string())
                    .unwrap_or_else(|| "declared".to_string()),
                direct_value: usage
                    .modifiers
                    .iter()
                    .find(|modifier| matches!(modifier.as_str(), "in" | "out" | "inout"))
                    .map(|value| Value::String(value.clone())),
                effective_value: usage
                    .modifiers
                    .iter()
                    .find(|modifier| matches!(modifier.as_str(), "in" | "out" | "inout"))
                    .map(|value| Value::String(value.clone())),
            },
        ],
        Declaration::Alias(alias) => vec![
            semantic_scalar_attribute(
                "declared_name",
                Some(Value::String(alias.name.clone())),
                true,
            ),
            semantic_short_name_attribute(&alias.modifiers),
            semantic_effective_short_name_attribute(&alias.modifiers, &alias.name),
            semantic_scalar_attribute(
                "target",
                Some(Value::String(alias.target.as_colon_string())),
                true,
            ),
        ],
        Declaration::Import(import) => vec![semantic_scalar_attribute(
            "imports",
            Some(Value::String(import.path.as_colon_string())),
            true,
        )],
    }
}

fn semantic_doc_attribute(name: &str, docs: &[String]) -> SemanticAttribute {
    let value = match docs {
        [] => None,
        [single] => Some(Value::String(single.clone())),
        many => Some(Value::Array(
            many.iter().map(|doc| Value::String(doc.clone())).collect(),
        )),
    };
    SemanticAttribute {
        name: name.to_string(),
        origin_kind: if value.is_some() {
            "direct".to_string()
        } else {
            "declared".to_string()
        },
        direct_value: value.clone(),
        effective_value: value,
    }
}

fn semantic_raw_body_attribute(raw_body: &Option<String>) -> SemanticAttribute {
    semantic_scalar_attribute(
        "raw_body",
        raw_body.as_ref().map(|body| Value::String(body.clone())),
        raw_body.is_some(),
    )
}

fn semantic_language_extensions_attribute(modifiers: &[String]) -> SemanticAttribute {
    let values = language_extensions_from_modifiers(modifiers);
    SemanticAttribute {
        name: "language_extensions".to_string(),
        origin_kind: if values.is_empty() {
            "declared".to_string()
        } else {
            "direct".to_string()
        },
        direct_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        )),
        effective_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        )),
    }
}

fn semantic_language_extension_keyword_attribute(modifiers: &[String]) -> SemanticAttribute {
    let enabled = modifiers
        .iter()
        .any(|modifier| modifier == "hashed_keyword");
    semantic_scalar_attribute(
        "is_language_extension_keyword",
        Some(Value::Bool(enabled)),
        enabled,
    )
}

fn semantic_annotated_elements_attribute(modifiers: &[String]) -> SemanticAttribute {
    let values = annotated_elements_from_modifiers(modifiers);
    SemanticAttribute {
        name: "annotated_elements".to_string(),
        origin_kind: if values.is_empty() {
            "declared".to_string()
        } else {
            "direct".to_string()
        },
        direct_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.as_colon_string()))
                .collect(),
        )),
        effective_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.as_colon_string()))
                .collect(),
        )),
    }
}

fn semantic_modifier_attribute(
    name: &str,
    modifier: &str,
    modifiers: &[String],
) -> SemanticAttribute {
    let enabled = modifiers.iter().any(|existing| existing == modifier);
    semantic_scalar_attribute(name, Some(Value::Bool(enabled)), enabled)
}

fn semantic_modifier_value_attribute(
    name: &str,
    key: &str,
    modifiers: &[String],
) -> SemanticAttribute {
    semantic_scalar_attribute(
        name,
        modifier_value(modifiers, key).map(|value| Value::String(value.to_string())),
        modifier_value(modifiers, key).is_some(),
    )
}

fn semantic_short_name_attribute(modifiers: &[String]) -> SemanticAttribute {
    semantic_scalar_attribute(
        "declared_short_name",
        declared_short_name_from_modifiers(modifiers).map(|value| Value::String(value.to_string())),
        declared_short_name_from_modifiers(modifiers).is_some(),
    )
}

fn semantic_effective_short_name_attribute(
    modifiers: &[String],
    declared_name: &str,
) -> SemanticAttribute {
    let short_name = declared_short_name_from_modifiers(modifiers).unwrap_or(declared_name);
    SemanticAttribute {
        name: "short_name".to_string(),
        origin_kind: if declared_short_name_from_modifiers(modifiers).is_some() {
            "direct".to_string()
        } else {
            "derived".to_string()
        },
        direct_value: declared_short_name_from_modifiers(modifiers)
            .map(|value| Value::String(value.to_string())),
        effective_value: Some(Value::String(short_name.to_string())),
    }
}

fn semantic_unique_attribute(modifiers: &[String]) -> SemanticAttribute {
    let is_nonunique = modifiers.iter().any(|modifier| modifier == "nonunique");
    semantic_scalar_attribute("is_unique", Some(Value::Bool(!is_nonunique)), is_nonunique)
}

fn semantic_scalar_attribute(
    name: &str,
    value: Option<Value>,
    is_direct: bool,
) -> SemanticAttribute {
    SemanticAttribute {
        name: name.to_string(),
        origin_kind: if is_direct {
            "direct".to_string()
        } else {
            "declared".to_string()
        },
        direct_value: value.clone().filter(|_| is_direct),
        effective_value: value,
    }
}

fn semantic_list_attribute(name: &str, values: &[QualifiedName]) -> SemanticAttribute {
    SemanticAttribute {
        name: name.to_string(),
        origin_kind: if values.is_empty() {
            "declared".to_string()
        } else {
            "direct".to_string()
        },
        direct_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.as_colon_string()))
                .collect(),
        )),
        effective_value: Some(Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.as_colon_string()))
                .collect(),
        )),
    }
}

fn multiplicity_range_from_raw(raw: &str) -> MultiplicityRange {
    let (lower, upper) = raw
        .split_once("..")
        .map(|(lower, upper)| (lower.to_string(), upper.to_string()))
        .unwrap_or_else(|| (raw.to_string(), raw.to_string()));
    MultiplicityRange {
        lower,
        upper,
        raw: raw.to_string(),
        span: SourceSpan {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
        },
    }
}

fn value_as_multiplicity(value: &Value) -> Result<MultiplicityRange, AuthoringError> {
    let raw = value_as_string(value, "multiplicity")?;
    let normalized = raw
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or_else(|| raw.trim())
        .trim();
    if normalized.is_empty() {
        return Err(AuthoringError::InvalidMutation(
            "multiplicity cannot be empty".to_string(),
        ));
    }
    Ok(multiplicity_range_from_raw(normalized))
}

fn unquote_sysml_name(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('\'')
        .and_then(|inner| inner.strip_suffix('\''))
        .unwrap_or(trimmed)
        .to_string()
}

fn render_qname(value: &QualifiedName) -> String {
    value
        .0
        .iter()
        .map(|segment| render_name_segment(segment))
        .collect::<Vec<_>>()
        .join("::")
}

fn render_name_segment(value: &str) -> String {
    let unquoted = unquote_sysml_name(value);
    if is_plain_sysml_identifier(&unquoted) {
        unquoted
    } else {
        format!("'{}'", unquoted.replace('\'', "\\'"))
    }
}

fn is_plain_sysml_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn normalize_attribute_name(name: &str) -> String {
    match name {
        "ownedFeature" => "features".to_string(),
        "ownedMember" => "members".to_string(),
        "ownedSpecialization" => "specializes".to_string(),
        "documentation" => "doc".to_string(),
        "id" => "id".to_string(),
        "text" => "text".to_string(),
        "declaredName" => "declared_name".to_string(),
        "declaredShortName" => "declared_short_name".to_string(),
        "shortName" => "short_name".to_string(),
        "languageExtension"
        | "languageExtensions"
        | "language_extension"
        | "language_extensions" => "language_extensions".to_string(),
        "isLanguageExtensionKeyword" | "languageExtensionKeyword" => {
            "is_language_extension_keyword".to_string()
        }
        "isAbstract" => "is_abstract".to_string(),
        "isDerived" => "is_derived".to_string(),
        "isEnd" => "is_end".to_string(),
        "isIndividual" => "is_individual".to_string(),
        "isOrdered" => "is_ordered".to_string(),
        "isUnique" => "is_unique".to_string(),
        "isVariable" => "is_variable".to_string(),
        "rawBody" | "raw_body" | "body" => "raw_body".to_string(),
        "annotatedElement" | "annotatedElements" | "annotated_elements" => {
            "annotated_elements".to_string()
        }
        "featuringType" => "featuring_type".to_string(),
        "imports" => "imports".to_string(),
        "type" => "type".to_string(),
        "additionalTypes" | "additionalType" => "additional_types".to_string(),
        "subsettedFeature" | "subsettedFeatures" | "subsets" => "subsets".to_string(),
        "redefinedFeature" | "redefinedFeatures" | "redefines" => "redefines".to_string(),
        "referenceTarget" | "reference_target" => "reference_target".to_string(),
        "referenceTargets" | "reference_targets" | "about" | "aboutTargets" | "about_targets" => {
            "reference_targets".to_string()
        }
        "transitionSource" | "transition_source" => "transition_source".to_string(),
        "transitionTarget" | "transition_target" => "transition_target".to_string(),
        "triggerKind" | "trigger_kind" => "trigger_kind".to_string(),
        "sourceIsInitial" | "source_is_initial" | "initial" => "source_is_initial".to_string(),
        "target" => "target".to_string(),
        "direction" => "direction".to_string(),
        "name" => "declared_name".to_string(),
        other => {
            let mut result = String::with_capacity(other.len());
            for (index, ch) in other.chars().enumerate() {
                if ch.is_ascii_uppercase() && index > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            }
            result
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocEdit {
    Id(String),
    Text(String),
    ClearId,
    ClearText,
}

fn declaration_docs_mut(declaration: &mut Declaration) -> &mut Vec<String> {
    match declaration {
        Declaration::Package(package) => &mut package.docs,
        Declaration::Import(import) => &mut import.docs,
        Declaration::Definition(definition) => &mut definition.docs,
        Declaration::Usage(usage) => &mut usage.docs,
        Declaration::Alias(alias) => &mut alias.docs,
    }
}

fn declaration_modifiers_mut(declaration: &mut Declaration) -> &mut Vec<String> {
    match declaration {
        Declaration::Package(package) => &mut package.modifiers,
        Declaration::Import(import) => &mut import.modifiers,
        Declaration::Definition(definition) => &mut definition.modifiers,
        Declaration::Usage(usage) => &mut usage.modifiers,
        Declaration::Alias(alias) => &mut alias.modifiers,
    }
}

fn apply_doc_value_edit(docs: &mut Vec<String>, edit: DocEdit) {
    match edit {
        DocEdit::Id(value) => {
            docs.retain(|doc| !is_id_doc(doc));
            let value = value.trim();
            if !value.is_empty() {
                docs.insert(0, format!("id: {value}"));
            }
        }
        DocEdit::Text(value) => {
            docs.retain(|doc| is_id_doc(doc));
            let value = value.trim();
            if !value.is_empty() {
                docs.push(value.to_string());
            }
        }
        DocEdit::ClearId => docs.retain(|doc| !is_id_doc(doc)),
        DocEdit::ClearText => docs.retain(|doc| is_id_doc(doc)),
    }
}

fn id_from_docs(docs: &[String]) -> Option<String> {
    docs.iter().find_map(|doc| {
        let trimmed = doc.trim();
        trimmed
            .strip_prefix("id:")
            .or_else(|| trimmed.strip_prefix("ID:"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn text_from_docs(docs: &[String]) -> Option<String> {
    docs.iter()
        .find(|doc| !is_id_doc(doc) && !doc.trim().is_empty())
        .cloned()
}

fn is_id_doc(doc: &str) -> bool {
    let trimmed = doc.trim();
    trimmed.starts_with("id:") || trimmed.starts_with("ID:")
}

fn value_as_string(value: &Value, attribute: &str) -> Result<String, AuthoringError> {
    value.as_str().map(str::to_string).ok_or_else(|| {
        AuthoringError::InvalidMutation(format!("attribute `{attribute}` expects a string value"))
    })
}

fn value_as_string_list(value: &Value, attribute: &str) -> Result<Vec<String>, AuthoringError> {
    match value {
        Value::String(_) => Ok(vec![value_as_string(value, attribute)?]),
        Value::Array(items) => items
            .iter()
            .map(|item| value_as_string(item, attribute))
            .collect(),
        _ => Err(AuthoringError::InvalidMutation(format!(
            "attribute `{attribute}` expects a string or string array"
        ))),
    }
}

fn value_as_qname(value: &Value, attribute: &str) -> Result<QualifiedName, AuthoringError> {
    Ok(QualifiedName::parse(&value_as_string(value, attribute)?))
}

fn value_as_qname_list(
    value: &Value,
    attribute: &str,
) -> Result<Vec<QualifiedName>, AuthoringError> {
    match value {
        Value::String(_) => Ok(vec![value_as_qname(value, attribute)?]),
        Value::Array(items) => items
            .iter()
            .map(|item| value_as_qname(item, attribute))
            .collect(),
        _ => Err(AuthoringError::InvalidMutation(format!(
            "attribute `{attribute}` expects a string or string array"
        ))),
    }
}

fn value_as_bool(value: &Value, attribute: &str) -> Result<bool, AuthoringError> {
    value.as_bool().ok_or_else(|| {
        AuthoringError::InvalidMutation(format!("attribute `{attribute}` expects a boolean value"))
    })
}

fn value_as_direction(value: &Value) -> Result<String, AuthoringError> {
    let value = value_as_string(value, "direction")?;
    match value.as_str() {
        "in" | "out" | "inout" => Ok(value),
        _ => Err(AuthoringError::InvalidMutation(
            "direction must be one of `in`, `out`, or `inout`".to_string(),
        )),
    }
}

fn set_modifier_flag(modifiers: &mut Vec<String>, modifier: &str, enabled: bool) -> bool {
    let had_modifier = modifiers.iter().any(|existing| existing == modifier);
    if enabled {
        if had_modifier {
            return false;
        }
        modifiers.push(modifier.to_string());
        true
    } else if had_modifier {
        modifiers.retain(|existing| existing != modifier);
        true
    } else {
        false
    }
}

fn modifier_value<'a>(modifiers: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    modifiers
        .iter()
        .find_map(|modifier| modifier.strip_prefix(&prefix))
}

fn set_modifier_value(modifiers: &mut Vec<String>, key: &str, value: Option<&str>) {
    let prefix = format!("{key}=");
    modifiers.retain(|modifier| !modifier.starts_with(&prefix));
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        modifiers.push(format!("{key}={value}"));
    }
}

fn set_short_name_modifier(modifiers: &mut Vec<String>, short_name: Option<&str>) -> bool {
    let current = declared_short_name_from_modifiers(modifiers).map(str::to_string);
    if current.as_deref() == short_name {
        return false;
    }
    modifiers.retain(|modifier| !is_angle_adornment_modifier(modifier));
    if let Some(short_name) = short_name {
        modifiers.push(short_name.to_string());
    }
    true
}

fn annotated_elements_from_modifiers(modifiers: &[String]) -> Vec<QualifiedName> {
    modifiers
        .iter()
        .filter_map(|modifier| modifier.strip_prefix("annotated_element="))
        .map(QualifiedName::parse)
        .collect()
}

fn set_annotated_elements(modifiers: &mut Vec<String>, values: &[QualifiedName]) {
    modifiers.retain(|modifier| !modifier.starts_with("annotated_element="));
    modifiers.extend(
        values
            .iter()
            .map(|value| format!("annotated_element={}", value.as_dot_string())),
    );
}

fn reference_targets_from_modifiers(modifiers: &[String]) -> Vec<QualifiedName> {
    modifiers
        .iter()
        .filter_map(|modifier| modifier.strip_prefix("reference_target="))
        .map(QualifiedName::parse)
        .collect()
}

fn set_reference_targets(modifiers: &mut Vec<String>, values: &[QualifiedName]) {
    modifiers.retain(|modifier| !modifier.starts_with("reference_target="));
    modifiers.extend(
        values
            .iter()
            .map(|value| format!("reference_target={}", value.as_dot_string())),
    );
}

fn usage_reference_targets(usage: &Usage) -> Vec<QualifiedName> {
    let targets = reference_targets_from_modifiers(&usage.modifiers);
    if targets.is_empty() {
        usage.reference_target.iter().cloned().collect()
    } else {
        targets
    }
}

fn language_extensions_from_modifiers(modifiers: &[String]) -> Vec<String> {
    modifiers
        .iter()
        .filter_map(|modifier| modifier.strip_prefix("language_extension="))
        .map(str::to_string)
        .collect()
}

fn set_language_extensions(modifiers: &mut Vec<String>, values: &[String]) {
    modifiers.retain(|modifier| !modifier.starts_with("language_extension="));
    modifiers.extend(
        values
            .iter()
            .map(|value| format!("language_extension={}", value.trim()))
            .filter(|value| value != "language_extension="),
    );
}

fn modifier_edit_for_attribute(attribute: &str, value: bool) -> (&'static str, bool) {
    match attribute {
        "is_language_extension_keyword" => ("hashed_keyword", value),
        "is_abstract" => ("abstract", value),
        "is_derived" => ("derived", value),
        "is_end" => ("end", value),
        "is_individual" => ("individual", value),
        "is_ordered" => ("ordered", value),
        "is_unique" => ("nonunique", !value),
        "is_variable" => ("variable", value),
        _ => unreachable!("unsupported modifier attribute `{attribute}`"),
    }
}

fn modifier_clear_for_attribute(attribute: &str) -> (&'static str, bool) {
    match attribute {
        "is_unique" => ("nonunique", false),
        _ => modifier_edit_for_attribute(attribute, false),
    }
}

fn set_direction(modifiers: &mut Vec<String>, direction: Option<&str>) -> bool {
    let current = modifiers
        .iter()
        .find(|modifier| matches!(modifier.as_str(), "in" | "out" | "inout"))
        .cloned();
    if current.as_deref() == direction {
        return false;
    }
    modifiers.retain(|modifier| !matches!(modifier.as_str(), "in" | "out" | "inout"));
    if let Some(direction) = direction {
        modifiers.push(direction.to_string());
    }
    true
}

fn declaration_kind_label(declaration: &Declaration) -> &'static str {
    match declaration {
        Declaration::Package(_) => "package",
        Declaration::Import(_) => "import",
        Declaration::Definition(_) => "definition",
        Declaration::Usage(_) => "usage",
        Declaration::Alias(_) => "alias",
    }
}

fn remove_import(declarations: &mut Vec<Declaration>, path: &QualifiedName) -> bool {
    let original = declarations.len();
    declarations.retain(
        |declaration| !matches!(declaration, Declaration::Import(import) if import.path == *path),
    );
    original != declarations.len()
}

fn remove_declaration(
    declarations: &mut Vec<Declaration>,
    qualified_name: &QualifiedName,
) -> Option<Declaration> {
    let mut index = None;
    for (idx, declaration) in declarations.iter().enumerate() {
        if declaration_name(declaration) == Some(qualified_name.tail().unwrap_or_default()) {
            index = Some(idx);
            break;
        }
    }
    index.map(|idx| declarations.remove(idx))
}

fn declaration_name(declaration: &Declaration) -> Option<&str> {
    match declaration {
        Declaration::Package(package) => package.name.tail(),
        Declaration::Definition(definition) => Some(definition.name.as_str()),
        Declaration::Usage(usage) => Some(usage.name.as_str()),
        Declaration::Alias(alias) => Some(alias.name.as_str()),
        Declaration::Import(_) => None,
    }
}

fn rename_declaration(
    module: &mut AuthoringModule,
    qualified_name: &QualifiedName,
    new_name: &str,
) -> Result<(), AuthoringError> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        if let Some(package) = &mut module.package {
            if let Some(last) = package.name.0.last_mut() {
                *last = new_name.to_string();
                return Ok(());
            }
        }
    }
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && rename_in_members(
            &mut package.members,
            &package_owner,
            qualified_name,
            new_name,
        )
        .is_some()
    {
        return Ok(());
    }
    rename_in_members(&mut module.members, "", qualified_name, new_name)
        .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()))
}

fn rename_in_members(
    declarations: &mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
    new_name: &str,
) -> Option<()> {
    for declaration in declarations {
        match declaration {
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if qname == qualified_name.as_dot_string() {
                    if let Some(last) = package.name.0.last_mut() {
                        *last = new_name.to_string();
                        return Some(());
                    }
                }
                if rename_in_members(&mut package.members, &qname, qualified_name, new_name)
                    .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if qname == qualified_name.as_dot_string() {
                    definition.name = new_name.to_string();
                    return Some(());
                }
                if rename_in_members(&mut definition.members, &qname, qualified_name, new_name)
                    .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    usage.name = new_name.to_string();
                    usage.is_implicit_name = false;
                    return Some(());
                }
                if rename_in_members(&mut usage.members, &qname, qualified_name, new_name).is_some()
                {
                    return Some(());
                }
            }
            Declaration::Alias(alias) => {
                let qname = qualify_name(owner, &alias.name);
                if qname == qualified_name.as_dot_string() {
                    alias.name = new_name.to_string();
                    return Some(());
                }
            }
            Declaration::Import(_) => {}
        }
    }
    None
}

fn update_specializations(
    module: &mut AuthoringModule,
    qualified_name: &QualifiedName,
    specializes: Vec<QualifiedName>,
) -> Result<(), AuthoringError> {
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && update_specializations_in_members(
            &mut package.members,
            &package_owner,
            qualified_name,
            &specializes,
        )
        .is_some()
    {
        return Ok(());
    }
    update_specializations_in_members(&mut module.members, "", qualified_name, &specializes)
        .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()))
}

fn update_specializations_in_members(
    declarations: &mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
    specializes: &[QualifiedName],
) -> Option<()> {
    for declaration in declarations {
        match declaration {
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if qname == qualified_name.as_dot_string() {
                    definition.specializes = specializes.to_vec();
                    return Some(());
                }
                if update_specializations_in_members(
                    &mut definition.members,
                    &qname,
                    qualified_name,
                    specializes,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    usage.specializes = specializes.to_vec();
                    return Some(());
                }
                if update_specializations_in_members(
                    &mut usage.members,
                    &qname,
                    qualified_name,
                    specializes,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if update_specializations_in_members(
                    &mut package.members,
                    &qname,
                    qualified_name,
                    specializes,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Import(_) | Declaration::Alias(_) => {}
        }
    }
    None
}

fn update_usage_type(
    module: &mut AuthoringModule,
    qualified_name: &QualifiedName,
    ty: Option<QualifiedName>,
) -> Result<(), AuthoringError> {
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && update_usage_type_in_members(
            &mut package.members,
            &package_owner,
            qualified_name,
            ty.as_ref(),
        )
        .is_some()
    {
        return Ok(());
    }
    update_usage_type_in_members(&mut module.members, "", qualified_name, ty.as_ref())
        .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()))
}

fn update_usage_type_in_members(
    declarations: &mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
    ty: Option<&QualifiedName>,
) -> Option<()> {
    for declaration in declarations {
        match declaration {
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    usage.ty = ty.cloned();
                    return Some(());
                }
                if update_usage_type_in_members(&mut usage.members, &qname, qualified_name, ty)
                    .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if update_usage_type_in_members(&mut definition.members, &qname, qualified_name, ty)
                    .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if update_usage_type_in_members(&mut package.members, &qname, qualified_name, ty)
                    .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Import(_) | Declaration::Alias(_) => {}
        }
    }
    None
}

fn set_usage_expression(
    module: &mut AuthoringModule,
    qualified_name: &QualifiedName,
    expression: Option<String>,
) -> Result<(), AuthoringError> {
    if let Some(package_owner) = module
        .package
        .as_ref()
        .map(|package| package.name.as_dot_string())
        && let Some(package) = module.package.as_mut()
        && set_usage_expression_in_members(
            &mut package.members,
            &package_owner,
            qualified_name,
            expression.as_deref(),
        )
        .is_some()
    {
        return Ok(());
    }
    set_usage_expression_in_members(
        &mut module.members,
        "",
        qualified_name,
        expression.as_deref(),
    )
    .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()))
}

fn set_usage_expression_in_members(
    declarations: &mut [Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
    expression: Option<&str>,
) -> Option<()> {
    for declaration in declarations {
        match declaration {
            Declaration::Usage(usage) => {
                let qname = qualify_name(owner, &usage.name);
                if qname == qualified_name.as_dot_string() {
                    usage.expression = expression.map(str::to_string);
                    return Some(());
                }
                if set_usage_expression_in_members(
                    &mut usage.members,
                    &qname,
                    qualified_name,
                    expression,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Definition(definition) => {
                let qname = qualify_name(owner, &definition.name);
                if set_usage_expression_in_members(
                    &mut definition.members,
                    &qname,
                    qualified_name,
                    expression,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Package(package) => {
                let qname = qualify_name(owner, &package.name.as_dot_string());
                if set_usage_expression_in_members(
                    &mut package.members,
                    &qname,
                    qualified_name,
                    expression,
                )
                .is_some()
                {
                    return Some(());
                }
            }
            Declaration::Import(_) | Declaration::Alias(_) => {}
        }
    }
    None
}

fn relationship_usage(
    kind: &str,
    source: &QualifiedName,
    target: &QualifiedName,
) -> Result<Usage, AuthoringError> {
    let keyword = kind.trim().to_ascii_lowercase();
    if keyword.is_empty() {
        return Err(AuthoringError::Unsupported(
            "relationship kind is required by authoring write-back".to_string(),
        ));
    }
    Ok(Usage {
        keyword,
        name: target.tail().unwrap_or("target").to_string(),
        is_implicit_name: false,
        ty: None,
        reference_target: Some(target.clone()),
        metadata_properties: BTreeMap::new(),
        multiplicity: None,
        expression: None,
        additional_types: Vec::new(),
        specializes: Vec::new(),
        subsets: Vec::new(),
        redefines: Vec::new(),
        members: Vec::new(),
        raw_body: None,
        comments: Vec::new(),
        docs: Vec::new(),
        modifiers: vec![format!("relationship_source={}", source.as_dot_string())],
    })
}

fn extract_declaration(
    module: &mut AuthoringModule,
    qualified_name: &QualifiedName,
) -> Result<Declaration, AuthoringError> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return module
            .package
            .take()
            .map(Declaration::Package)
            .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()));
    }
    if let Some(package) = &mut module.package {
        if let Some(removed) = extract_from_members(
            &mut package.members,
            &package.name.as_dot_string(),
            qualified_name,
        ) {
            return Ok(removed);
        }
    }
    extract_from_members(&mut module.members, "", qualified_name)
        .ok_or_else(|| AuthoringError::MissingDeclaration(qualified_name.as_dot_string()))
}

fn extract_from_members(
    declarations: &mut Vec<Declaration>,
    owner: &str,
    qualified_name: &QualifiedName,
) -> Option<Declaration> {
    let mut index = None;
    for (idx, declaration) in declarations.iter_mut().enumerate() {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string() {
            index = Some(idx);
            break;
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                extract_from_members(&mut package.members, &qname, qualified_name)
            }
            Declaration::Definition(definition) => {
                extract_from_members(&mut definition.members, &qname, qualified_name)
            }
            Declaration::Usage(usage) => {
                extract_from_members(&mut usage.members, &qname, qualified_name)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if nested.is_some() {
            return nested;
        }
    }
    index.map(|idx| declarations.remove(idx))
}

fn render_declaration_at_qname(
    module: &AuthoringModule,
    qualified_name: &QualifiedName,
    render_profile: AuthoringRenderProfile,
) -> Option<String> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name == *qualified_name)
    {
        return module
            .package
            .as_ref()
            .map(|package| (render_profile.render_package)(package, 0));
    }
    if let Some(package) = &module.package
        && let Some(rendered) = render_decl_in_members(
            &package.members,
            &package.name.as_dot_string(),
            qualified_name,
            render_profile,
        )
    {
        return Some(rendered);
    }
    render_decl_in_members(&module.members, "", qualified_name, render_profile)
}

fn render_decl_in_members(
    declarations: &[Declaration],
    owner: &str,
    qualified_name: &QualifiedName,
    render_profile: AuthoringRenderProfile,
) -> Option<String> {
    for declaration in declarations {
        let qname = match declaration {
            Declaration::Package(package) => qualify_name(owner, &package.name.as_dot_string()),
            Declaration::Definition(definition) => qualify_name(owner, &definition.name),
            Declaration::Usage(usage) => qualify_name(owner, &usage.name),
            Declaration::Alias(alias) => qualify_name(owner, &alias.name),
            Declaration::Import(_) => continue,
        };
        if qname == qualified_name.as_dot_string() {
            return Some((render_profile.render_declaration)(declaration, 0));
        }
        let nested = match declaration {
            Declaration::Package(package) => {
                render_decl_in_members(&package.members, &qname, qualified_name, render_profile)
            }
            Declaration::Definition(definition) => {
                render_decl_in_members(&definition.members, &qname, qualified_name, render_profile)
            }
            Declaration::Usage(usage) => {
                render_decl_in_members(&usage.members, &qname, qualified_name, render_profile)
            }
            Declaration::Alias(_) | Declaration::Import(_) => None,
        };
        if nested.is_some() {
            return nested;
        }
    }
    None
}

fn render_textual_module(module: &AuthoringModule) -> String {
    module.render()
}

fn render_textual_package(package: &Package, indent: usize) -> String {
    package.render(indent)
}

fn render_textual_declaration(declaration: &Declaration, indent: usize) -> String {
    declaration.render(indent)
}

fn render_docs(docs: &[String], indent: usize) -> Vec<String> {
    let prefix = " ".repeat(indent);
    docs.iter()
        .map(|doc| format!("{prefix}doc /* {} */", doc.replace("*/", "* /")))
        .collect()
}

/// Renders preserved comments verbatim: `//{text}` per line comment and
/// `/*{text}*/` for block comments, with block interiors (including any
/// newlines) kept byte-for-byte. Emitted before docs.
fn render_comments(comments: &[CommentNote], indent: usize) -> Vec<String> {
    let prefix = " ".repeat(indent);
    comments
        .iter()
        .map(|comment| match comment.kind {
            CommentKind::Line => format!("{prefix}//{}", comment.text),
            CommentKind::Block => format!("{prefix}/*{}*/", comment.text),
        })
        .collect()
}

fn render_with_leading_comments(
    comments: &[CommentNote],
    rendered: String,
    indent: usize,
) -> String {
    if comments.is_empty() {
        return rendered;
    }
    let mut lines = render_comments(comments, indent);
    lines.push(rendered);
    lines.join("\n")
}

/// Container-side render of a member declaration: leading comments first,
/// then the declaration itself (which starts with its docs). Only containers
/// render leading comments — a localized splice replaces the declaration
/// span alone, leaving the original comment bytes above it untouched.
fn render_declaration_with_comments(declaration: &Declaration, indent: usize) -> String {
    render_with_leading_comments(declaration.comments(), declaration.render(indent), indent)
}

fn render_member_and_raw_body(
    members: &[Declaration],
    raw_body: Option<&str>,
    indent: usize,
) -> String {
    let mut blocks = members
        .iter()
        .map(|member| render_declaration_with_comments(member, indent))
        .collect::<Vec<_>>();
    if let Some(raw_body) = raw_body {
        let prefix = " ".repeat(indent);
        let rendered = raw_body
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    format!("{prefix}{}", line.trim_end())
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !rendered.trim().is_empty() {
            blocks.push(rendered);
        }
    }
    blocks.join("\n\n")
}

fn render_metadata_definition_body(
    annotated_elements: &[QualifiedName],
    members: &[Declaration],
    raw_body: Option<&str>,
    indent: usize,
) -> String {
    let mut blocks = annotated_elements
        .iter()
        .map(|target| {
            format!(
                "{}:> annotatedElement : {};",
                " ".repeat(indent),
                render_qname(target)
            )
        })
        .collect::<Vec<_>>();
    let rest = render_member_and_raw_body(members, raw_body, indent);
    if !rest.is_empty() {
        blocks.push(rest);
    }
    blocks.join("\n")
}

fn render_modifier_prefix(modifiers: &[String]) -> String {
    let rendered = modifiers
        .iter()
        .filter(|modifier| !is_angle_adornment_modifier(modifier))
        .filter(|modifier| !is_internal_render_modifier(modifier))
        .map(|modifier| {
            if let Some(extension) = modifier.strip_prefix("language_extension=") {
                format!("#{}", render_language_extension_keyword(extension))
            } else if is_angle_adornment_modifier(modifier) {
                format!("<{}>", render_angle_adornment(modifier))
            } else {
                modifier.clone()
            }
        })
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        String::new()
    } else {
        format!("{} ", rendered.join(" "))
    }
}

fn render_keyword(keyword: &str, modifiers: &[String]) -> String {
    let rendered_keyword = match keyword {
        "use-case" => "use case",
        other => other,
    };
    if modifiers
        .iter()
        .any(|modifier| modifier == "hashed_keyword")
    {
        format!("#{}", render_language_extension_keyword(rendered_keyword))
    } else {
        rendered_keyword.to_string()
    }
}

fn render_language_extension_keyword(value: &str) -> String {
    value.trim().trim_start_matches('#').to_string()
}

fn render_relationship_shorthand(usage: &Usage) -> Option<String> {
    let source = relationship_source_from_modifiers(&usage.modifiers)?;
    let target = usage.reference_target.as_ref()?;
    let target = target.as_dot_string();
    match usage.keyword.as_str() {
        "flow" => Some(format!("flow from {source} to {target};")),
        "succession" => Some(format!("succession flow from {source} to {target};")),
        "allocation" | "allocate" => Some(format!(
            "allocation {} allocate {source} to {target};",
            render_name_segment(&usage.name)
        )),
        _ => None,
    }
}

fn render_transition_shorthand(usage: &Usage) -> Option<String> {
    if usage.keyword != "transition" {
        return None;
    }
    let source = modifier_value(&usage.modifiers, "transition_source");
    let target = modifier_value(&usage.modifiers, "transition_target");
    let trigger = modifier_value(&usage.modifiers, "trigger");
    let source_is_initial = usage
        .modifiers
        .iter()
        .any(|modifier| modifier == "source_is_initial");
    if source.is_none() && target.is_none() && trigger.is_none() {
        return None;
    }
    if source_is_initial && trigger.is_none() {
        let mut rendered = String::new();
        if let Some(source) = source {
            rendered.push_str("first ");
            rendered.push_str(&render_qname(&QualifiedName::parse(source)));
        }
        if let Some(target) = target {
            if !rendered.is_empty() {
                rendered.push(' ');
            }
            rendered.push_str("then ");
            rendered.push_str(&render_qname(&QualifiedName::parse(target)));
        }
        rendered.push(';');
        return Some(rendered);
    }
    let mut rendered = String::from("transition");
    if !usage.is_implicit_name && usage.name != "transition" {
        rendered.push(' ');
        rendered.push_str(&render_name_segment(&usage.name));
    }
    if let Some(source) = source {
        rendered.push_str(" first ");
        rendered.push_str(&render_qname(&QualifiedName::parse(source)));
    }
    if let Some(trigger) = trigger {
        rendered.push_str(" accept ");
        // Guarded triggers (`accept when …`, `accept after …`, `accept at …`)
        // store the guard keyword separately as `trigger_kind`; rendering it
        // back is required for the shorthand to re-parse. Event triggers
        // (`trigger_kind=event`) render as the bare trigger name.
        if let Some(kind) = modifier_value(&usage.modifiers, "trigger_kind")
            && matches!(kind, "after" | "when" | "at")
        {
            rendered.push_str(kind);
            rendered.push(' ');
        }
        rendered.push_str(trigger);
    }
    if let Some(target) = target {
        rendered.push_str(" then ");
        rendered.push_str(&render_qname(&QualifiedName::parse(target)));
    }
    rendered.push(';');
    Some(rendered)
}

fn append_usage_relations(header: &mut String, usage: &Usage) {
    if let Some(multiplicity) = &usage.multiplicity {
        header.push('[');
        header.push_str(&multiplicity.raw);
        header.push(']');
    }
    if !usage.additional_types.is_empty() {
        header.push_str(" :> ");
        header.push_str(
            &usage
                .additional_types
                .iter()
                .map(render_qname)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !usage.specializes.is_empty() {
        header.push_str(" specializes ");
        header.push_str(
            &usage
                .specializes
                .iter()
                .map(render_qname)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !usage.subsets.is_empty() {
        header.push_str(" subsets ");
        header.push_str(
            &usage
                .subsets
                .iter()
                .map(render_qname)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !usage.redefines.is_empty() {
        header.push_str(" redefines ");
        header.push_str(
            &usage
                .redefines
                .iter()
                .map(render_qname)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(expression) = &usage.expression {
        header.push_str(" = ");
        header.push_str(expression);
    }
}

fn relationship_source_from_modifiers(modifiers: &[String]) -> Option<&str> {
    modifiers
        .iter()
        .find_map(|modifier| modifier.strip_prefix("relationship_source="))
}

fn is_metadata_usage_modifier(modifiers: &[String]) -> bool {
    modifiers
        .iter()
        .any(|modifier| modifier == "metadata_usage")
}

fn is_internal_render_modifier(modifier: &str) -> bool {
    modifier.starts_with("relationship_source=")
        || modifier.starts_with("annotated_element=")
        || modifier.starts_with("reference_target=")
        || modifier.starts_with("transition_source=")
        || modifier.starts_with("transition_target=")
        || modifier.starts_with("trigger=")
        || modifier.starts_with("trigger_kind=")
        || modifier == "source_is_initial"
        || modifier == "hashed_keyword"
        || modifier == "metadata_usage"
}

fn render_angle_adornment_prefix(modifiers: &[String]) -> String {
    let rendered = modifiers
        .iter()
        .filter(|modifier| is_angle_adornment_modifier(modifier))
        .map(|modifier| format!("<{}>", render_angle_adornment(modifier)))
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        String::new()
    } else {
        format!("{} ", rendered.join(" "))
    }
}

fn declared_short_name_from_modifiers(modifiers: &[String]) -> Option<&str> {
    modifiers
        .iter()
        .find(|modifier| is_angle_adornment_modifier(modifier))
        .map(String::as_str)
}

fn is_angle_adornment_modifier(modifier: &str) -> bool {
    !is_internal_render_modifier(modifier)
        && !is_keyword_modifier(modifier)
        && !modifier.contains('=')
        && !modifier.contains(':')
        && !modifier.contains(' ')
}

fn is_keyword_modifier(modifier: &str) -> bool {
    matches!(
        modifier,
        "abstract"
            | "all"
            | "composite"
            | "derived"
            | "do"
            | "end"
            | "entry"
            | "exit"
            | "in"
            | "individual"
            | "inout"
            | "nonunique"
            | "ordered"
            | "out"
            | "private"
            | "protected"
            | "public"
            | "readonly"
            | "ref"
            | "variation"
            | "variable"
    )
}

fn render_angle_adornment(value: &str) -> String {
    value.replace('<', "").replace('>', "")
}

fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(literal) => match literal {
            LiteralExpr::Integer(value) => value.to_string(),
            LiteralExpr::Real(value) => value.clone(),
            LiteralExpr::Boolean(value) => value.to_string(),
            LiteralExpr::String(value) => format!("{value:?}"),
        },
        Expr::Name(name) => name.as_colon_string(),
        Expr::SelfRef(_) => "self".to_string(),
        Expr::Tuple { items, .. } => format!(
            "({})",
            items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::Unary { op, expr, .. } => match op {
            UnaryOp::Negate => format!("-{}", render_expr(expr)),
            UnaryOp::Not => format!("not {}", render_expr(expr)),
        },
        Expr::Binary {
            left, op, right, ..
        } => format!(
            "{} {} {}",
            render_expr(left),
            render_binary_op(op),
            render_expr(right)
        ),
        Expr::Path { root, segment, .. } => format!("{}.{}", render_expr(root), segment),
        Expr::Call { function, args, .. } => format!(
            "{function}({})",
            args.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn render_binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Power => "**",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
    }
}

fn render_metadata_property_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }
    if is_unquoted_metadata_value(trimmed) {
        trimmed.to_string()
    } else {
        format!("{trimmed:?}")
    }
}

fn is_unquoted_metadata_value(value: &str) -> bool {
    value.split("::").all(|segment| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
            && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

/// Indents a rendered declaration for splicing into original text. The
/// splice range starts at the declaration's own start column, so the text
/// before it already carries the indentation: the first line stays bare and
/// only continuation lines receive the prefix.
fn render_for_splice(rendered: &str, indent: usize) -> String {
    let mut lines = rendered.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let prefix = " ".repeat(indent);
    let mut spliced = first.to_string();
    for line in lines {
        spliced.push('\n');
        if !line.is_empty() {
            spliced.push_str(&prefix);
            spliced.push_str(line);
        }
    }
    spliced
}

fn qualify_name(owner: &str, name: &str) -> String {
    if owner.is_empty() {
        name.to_string()
    } else {
        format!("{owner}.{name}")
    }
}

fn join_qname(owner: &str, name: &str) -> String {
    if owner.is_empty() {
        name.to_string()
    } else {
        format!("{owner}.{name}")
    }
}

fn group_rewrites_by_file(
    rewrites: &[RewriteInstruction],
) -> BTreeMap<String, Vec<RewriteInstruction>> {
    let mut grouped = BTreeMap::new();
    for rewrite in rewrites {
        let file = match rewrite {
            RewriteInstruction::FullFile { file }
            | RewriteInstruction::ReplaceNode { file, .. }
            | RewriteInstruction::ReplaceContainer { file, .. } => file.clone(),
        };
        grouped
            .entry(file)
            .or_insert_with(Vec::new)
            .push(rewrite.clone());
    }
    grouped
}

/// Prepares one file's rewrite instructions for span splicing: drops exact
/// duplicates, drops instructions whose anchor span is covered by another
/// instruction's re-render, and drops instructions for declarations created
/// after parse time (no source span) when an ancestor container instruction
/// re-renders them anyway. Any instruction that survives without a source
/// span — and any full-file rewrite — is not localizable, so the caller's
/// canonical fallback takes over via the returned error.
fn normalize_rewrite_instructions(
    instructions: Vec<RewriteInstruction>,
    source_map: &FileSourceMap,
    module: &AuthoringModule,
) -> Result<Vec<RewriteInstruction>, AuthoringError> {
    let mut deduped: Vec<RewriteInstruction> = Vec::new();
    for instruction in instructions {
        if !deduped.contains(&instruction) {
            deduped.push(instruction);
        }
    }

    let mut anchors = Vec::with_capacity(deduped.len());
    for instruction in &deduped {
        let anchor_qname = match instruction {
            RewriteInstruction::FullFile { .. } => {
                return Err(AuthoringError::Unsupported(
                    "full-file rewrite is not localized".to_string(),
                ));
            }
            RewriteInstruction::ReplaceNode { anchor_qname, .. } => anchor_qname.clone(),
            RewriteInstruction::ReplaceContainer {
                anchor_qname: Some(anchor_qname),
                ..
            } => anchor_qname.clone(),
            RewriteInstruction::ReplaceContainer {
                anchor_qname: None, ..
            } => {
                return Err(AuthoringError::Unsupported(
                    "module-level localized replacement is unsupported".to_string(),
                ));
            }
        };
        let span = anchor_span(&anchor_qname, source_map, module);
        anchors.push((anchor_qname, span));
    }

    let mut kept = Vec::with_capacity(deduped.len());
    for (index, instruction) in deduped.iter().enumerate() {
        let (anchor_qname, span) = &anchors[index];
        let Some(span) = span else {
            let covered_by_ancestor = anchors.iter().any(|(other_qname, other_span)| {
                other_span.is_some() && is_qname_ancestor(other_qname, anchor_qname)
            });
            if covered_by_ancestor {
                continue;
            }
            return Err(AuthoringError::Unsupported(format!(
                "missing source span for `{anchor_qname}`"
            )));
        };
        let subsumed = anchors
            .iter()
            .enumerate()
            .any(|(other_index, (_, other_span))| {
                other_index != index
                    && other_span.as_ref().is_some_and(|other_span| {
                        span_contains(other_span, span)
                            && (other_span != span || other_index < index)
                    })
            });
        if subsumed {
            continue;
        }
        kept.push(instruction.clone());
    }
    Ok(kept)
}

fn anchor_span(
    anchor_qname: &str,
    source_map: &FileSourceMap,
    module: &AuthoringModule,
) -> Option<SourceSpan> {
    if module
        .package
        .as_ref()
        .is_some_and(|package| package.name.as_dot_string() == anchor_qname)
    {
        if let Some(package) = &source_map.package {
            return Some(package.span.clone());
        }
    }
    source_map
        .declarations
        .get(anchor_qname)
        .map(|node| node.span.clone())
}

fn is_qname_ancestor(ancestor: &str, descendant: &str) -> bool {
    descendant.len() > ancestor.len()
        && descendant.starts_with(ancestor)
        && descendant.as_bytes()[ancestor.len()] == b'.'
}

fn span_contains(outer: &SourceSpan, inner: &SourceSpan) -> bool {
    (outer.start_line, outer.start_col) <= (inner.start_line, inner.start_col)
        && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
}

fn validate_non_overlapping_patches(
    patches: &[((usize, usize), String)],
) -> Result<(), AuthoringError> {
    let mut previous_start = usize::MAX;
    for ((start, end), _) in patches {
        if *end > previous_start {
            return Err(AuthoringError::Unsupported(
                "localized rewrite produced overlapping source patches".to_string(),
            ));
        }
        previous_start = *start;
    }
    Ok(())
}

fn span_to_offsets(text: &str, span: &SourceSpan) -> Result<(usize, usize), AuthoringError> {
    let starts = line_start_offsets(text);
    let start_line = span
        .start_line
        .checked_sub(1)
        .ok_or_else(|| AuthoringError::Validation("invalid source span start".to_string()))?;
    let end_line = span
        .end_line
        .checked_sub(1)
        .ok_or_else(|| AuthoringError::Validation("invalid source span end".to_string()))?;
    let start = starts
        .get(start_line)
        .copied()
        .unwrap_or(text.len())
        .saturating_add(span.start_col.saturating_sub(1));
    let end = starts
        .get(end_line)
        .copied()
        .unwrap_or(text.len())
        .saturating_add(span.end_col);
    Ok((start.min(text.len()), end.min(text.len())))
}

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn rendered_span_for_text(text: &str) -> RenderedSpan {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return RenderedSpan {
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        };
    }
    let end_line = lines.len();
    let end_col = lines.last().map_or(1, |line| line.len() + 1);
    RenderedSpan {
        start_line: 1,
        start_col: 1,
        end_line,
        end_col,
    }
}

/// Counts elements per normalized id for the given source files. Trailing
/// id segments that only encode source position (`{line}`, `{line}_{col}`,
/// `{line}.{col}`, ordinals) are stripped so a localized patch is not
/// penalized for preserving the author's layout while the canonical render
/// reflows it; content differences still surface as count mismatches.
fn normalized_element_ids_for_files(
    document: &KirDocument,
    files: &BTreeSet<String>,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for element in &document.elements {
        let source_file = element
            .properties
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("source_file"))
            .and_then(Value::as_str)
            .map(|path| path.replace('\\', "/"));
        if source_file.is_some_and(|path| files.contains(&path)) {
            *counts
                .entry(normalize_positional_id(&element.id))
                .or_default() += 1;
        }
    }
    counts
}

fn normalize_positional_id(id: &str) -> String {
    let mut segments = id.split('.').collect::<Vec<_>>();
    while segments.len() > 1 {
        let last = segments[segments.len() - 1];
        let positional = last
            .split('_')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
        if positional {
            segments.pop();
        } else {
            break;
        }
    }
    segments.join(".")
}

fn diff_element_ids(before: &KirDocument, after: &KirDocument) -> BTreeSet<String> {
    let before_ids = before
        .elements
        .iter()
        .map(|element| element.id.clone())
        .collect::<BTreeSet<_>>();
    let after_ids = after
        .elements
        .iter()
        .map(|element| element.id.clone())
        .collect::<BTreeSet<_>>();
    before_ids
        .symmetric_difference(&after_ids)
        .cloned()
        .collect()
}

fn group_kir_by_source_file(document: &KirDocument) -> BTreeMap<String, Vec<KirElement>> {
    let mut grouped = BTreeMap::new();
    for element in &document.elements {
        let source_file = element
            .properties
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("source_file"))
            .and_then(Value::as_str);
        if let Some(source_file) = source_file {
            grouped
                .entry(source_file.replace('\\', "/"))
                .or_insert_with(Vec::new)
                .push(element.clone());
        }
    }
    grouped
}

fn module_from_kir_elements(elements: &[KirElement]) -> Result<AuthoringModule, AuthoringError> {
    let mut by_id = HashMap::new();
    for element in elements {
        by_id.insert(element.id.clone(), element.clone());
    }

    let mut module = AuthoringModule::default();
    let mut consumed = BTreeSet::new();
    let package_id = elements
        .iter()
        .filter(|element| element.kind.contains("Package") && element.id.starts_with("pkg."))
        .min_by_key(|element| element.id.matches('.').count())
        .map(|element| element.id.clone());

    if let Some(package_id) = package_id {
        if let Some(package) = build_package_from_kir(&package_id, &by_id, &mut consumed)? {
            module.package = Some(package);
        }
    }

    let mut top_level = Vec::new();
    for element in elements {
        if consumed.contains(&element.id) {
            continue;
        }
        if let Some(declaration) = build_declaration_from_kir(&element.id, &by_id, &mut consumed)? {
            top_level.push(declaration);
        }
    }
    top_level.sort_by_key(|declaration| declaration_name_for_sort(declaration));
    module.members = top_level;
    Ok(module)
}

fn build_package_from_kir(
    id: &str,
    by_id: &HashMap<String, KirElement>,
    consumed: &mut BTreeSet<String>,
) -> Result<Option<Package>, AuthoringError> {
    let Some(element) = by_id.get(id) else {
        return Ok(None);
    };
    consumed.insert(id.to_string());
    let name = declared_name_from_properties(&element.properties)
        .or_else(|| id.strip_prefix("pkg.").map(QualifiedName::parse))
        .unwrap_or_else(|| QualifiedName::new(vec!["Package".to_string()]));
    let member_ids = element
        .properties
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut members = Vec::new();
    for member_id in member_ids {
        if let Some(member) = build_declaration_from_kir(&member_id, by_id, consumed)? {
            members.push(member);
        }
    }
    Ok(Some(Package {
        name,
        members,
        comments: Vec::new(),
        docs: docs_from_properties(&element.properties),
        modifiers: Vec::new(),
    }))
}

fn build_declaration_from_kir(
    id: &str,
    by_id: &HashMap<String, KirElement>,
    consumed: &mut BTreeSet<String>,
) -> Result<Option<Declaration>, AuthoringError> {
    let Some(element) = by_id.get(id) else {
        return Ok(None);
    };
    if !consumed.insert(id.to_string()) {
        return Ok(None);
    }

    if element.kind.contains("Import") || id.starts_with("import.") {
        let path = element
            .properties
            .get("imports")
            .and_then(Value::as_str)
            .map(QualifiedName::parse)
            .ok_or_else(|| {
                AuthoringError::Unsupported(format!("cannot reconstruct import `{id}` from KIR"))
            })?;
        return Ok(Some(Declaration::Import(Import {
            path,
            comments: Vec::new(),
            docs: docs_from_properties(&element.properties),
            modifiers: Vec::new(),
        })));
    }

    if element.kind.contains("Package") && id.starts_with("pkg.") {
        return Ok(build_package_from_kir(id, by_id, consumed)?.map(Declaration::Package));
    }

    let members = element
        .properties
        .get("members")
        .or_else(|| element.properties.get("features"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let built_members = members
        .iter()
        .filter_map(|member_id| build_declaration_from_kir(member_id, by_id, consumed).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    if id.starts_with("type.") {
        let keyword = keyword_from_kind(&element.kind, true);
        let name = declared_name_from_properties(&element.properties)
            .and_then(|name| name.tail().map(str::to_string))
            .unwrap_or_else(|| tail_from_id(id));
        return Ok(Some(Declaration::Definition(Definition {
            keyword,
            name,
            specializes: specializations_from_properties(&element.properties, None),
            members: built_members,
            raw_body: None,
            comments: Vec::new(),
            docs: docs_from_properties(&element.properties),
            modifiers: Vec::new(),
        })));
    }

    if id.starts_with("feature.")
        || id.starts_with("relationship.")
        || element.properties.contains_key("owner")
    {
        let reference_target =
            if id.starts_with("relationship.") || element.properties.contains_key("target") {
                element
                    .properties
                    .get("target")
                    .and_then(Value::as_str)
                    .map(qualified_name_from_element_id)
            } else {
                None
            };
        let name = reference_target
            .as_ref()
            .and_then(QualifiedName::tail)
            .map(str::to_string)
            .or_else(|| {
                element
                    .properties
                    .get("declared_name")
                    .and_then(Value::as_str)
                    .or_else(|| element.properties.get("name").and_then(Value::as_str))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| tail_from_id(id));
        let ty = element
            .properties
            .get("type")
            .and_then(Value::as_str)
            .map(QualifiedName::parse);
        return Ok(Some(Declaration::Usage(Usage {
            keyword: keyword_from_kind(&element.kind, false),
            name,
            is_implicit_name: element.properties.get("declared_name").is_none(),
            ty: ty.clone(),
            reference_target,
            metadata_properties: BTreeMap::new(),
            multiplicity: element
                .properties
                .get("multiplicity")
                .and_then(Value::as_str)
                .map(multiplicity_range_from_raw),
            expression: element
                .properties
                .get("expression_ir")
                .and_then(Value::as_str)
                .map(str::to_string),
            additional_types: Vec::new(),
            specializes: specializations_from_properties(&element.properties, ty.as_ref()),
            subsets: property_qnames(&element.properties, "subsetted_features"),
            redefines: property_qnames(&element.properties, "redefined_features"),
            members: built_members,
            raw_body: None,
            comments: Vec::new(),
            docs: docs_from_properties(&element.properties),
            modifiers: usage_modifiers_from_properties(&element.properties),
        })));
    }

    Ok(None)
}

fn declared_name_from_properties(properties: &BTreeMap<String, Value>) -> Option<QualifiedName> {
    properties
        .get("declared_name")
        .and_then(Value::as_str)
        .or_else(|| properties.get("name").and_then(Value::as_str))
        .map(QualifiedName::parse)
}

fn docs_from_properties(properties: &BTreeMap<String, Value>) -> Vec<String> {
    properties
        .get("doc")
        .and_then(Value::as_object)
        .and_then(|doc| doc.get("blocks"))
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn property_qnames(properties: &BTreeMap<String, Value>, key: &str) -> Vec<QualifiedName> {
    properties
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(QualifiedName::parse)
        .collect()
}

fn specializations_from_properties(
    properties: &BTreeMap<String, Value>,
    ty: Option<&QualifiedName>,
) -> Vec<QualifiedName> {
    property_qnames(properties, "specializes")
        .into_iter()
        .filter(|name| Some(name) != ty)
        .collect()
}

fn usage_modifiers_from_properties(properties: &BTreeMap<String, Value>) -> Vec<String> {
    let mut modifiers = Vec::new();
    if properties
        .get("is_end")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        modifiers.push("end".to_string());
    }
    if let Some(direction) = properties.get("direction").and_then(Value::as_str) {
        modifiers.push(direction.to_string());
    }
    modifiers
}

fn keyword_from_kind(kind: &str, is_definition: bool) -> String {
    let tail = kind.rsplit("::").next().unwrap_or(kind);
    let keyword = if is_definition {
        tail.strip_suffix("Definition").unwrap_or(tail)
    } else {
        tail.strip_suffix("Usage")
            .or_else(|| tail.strip_suffix("Relationship"))
            .unwrap_or(tail)
    };
    keyword
        .chars()
        .enumerate()
        .fold(String::new(), |mut acc, (index, ch)| {
            if ch.is_ascii_uppercase() && index > 0 {
                acc.push('-');
            }
            acc.push(ch.to_ascii_lowercase());
            acc
        })
}

fn qualified_name_from_element_id(id: &str) -> QualifiedName {
    let without_prefix = id
        .strip_prefix("type.")
        .or_else(|| id.strip_prefix("feature."))
        .or_else(|| id.strip_prefix("relationship."))
        .or_else(|| id.strip_prefix("pkg."))
        .unwrap_or(id);
    QualifiedName::parse(without_prefix)
}

fn declaration_name_for_sort(declaration: &Declaration) -> String {
    match declaration {
        Declaration::Package(package) => package.name.as_dot_string(),
        Declaration::Import(import) => import.path.as_colon_string(),
        Declaration::Definition(definition) => definition.name.clone(),
        Declaration::Usage(usage) => usage.name.clone(),
        Declaration::Alias(alias) => alias.name.clone(),
    }
}

fn tail_from_id(id: &str) -> String {
    id.rsplit('.').next().unwrap_or(id).to_string()
}

#[cfg(any(test, feature = "toy-parser"))]
fn parse_fake_model_source(source: &str) -> Result<AuthoringModule, AuthoringError> {
    let tokens = fake_model_lines(source);
    let mut index = 0;
    let members = parse_fake_model_members(&tokens, &mut index)?;
    let mut module = AuthoringModule::default();
    for member in members {
        match member {
            Declaration::Package(package) if module.package.is_none() => {
                module.package = Some(package);
            }
            other => module.members.push(other),
        }
    }
    Ok(module)
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_model_lines(source: &str) -> Vec<String> {
    source
        .replace('{', "{\n")
        .replace('}', "\n}\n")
        .replace(';', ";\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(any(test, feature = "toy-parser"))]
fn parse_fake_model_members(
    lines: &[String],
    index: &mut usize,
) -> Result<Vec<Declaration>, AuthoringError> {
    let mut members = Vec::new();
    while *index < lines.len() {
        let line = lines[*index].trim();
        if line == "}" {
            *index += 1;
            break;
        }
        if line.ends_with('{') {
            let header = line.trim_end_matches('{').trim();
            *index += 1;
            let nested = parse_fake_model_members(lines, index)?;
            members.push(fake_declaration_from_header(header, nested)?);
        } else {
            let header = line.trim_end_matches(';').trim();
            *index += 1;
            if !header.is_empty() {
                members.push(fake_declaration_from_header(header, Vec::new())?);
            }
        }
    }
    Ok(members)
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_declaration_from_header(
    header: &str,
    members: Vec<Declaration>,
) -> Result<Declaration, AuthoringError> {
    let header = header.trim();
    if let Some(rest) = header.strip_prefix("package ") {
        return Ok(Declaration::Package(Package {
            name: QualifiedName::parse(rest.trim()),
            members,
            comments: Vec::new(),
            docs: Vec::new(),
            modifiers: Vec::new(),
        }));
    }

    if let Some((keyword, name)) = header.split_once(" def ") {
        return Ok(Declaration::Definition(Definition {
            keyword: keyword
                .split_whitespace()
                .last()
                .unwrap_or(keyword)
                .to_string(),
            name: clean_fake_name(name),
            specializes: Vec::new(),
            members,
            raw_body: None,
            comments: Vec::new(),
            docs: Vec::new(),
            modifiers: Vec::new(),
        }));
    }

    let mut words = header.split_whitespace();
    let keyword = words.next().unwrap_or("part").to_string();
    let rest = words.collect::<Vec<_>>().join(" ");
    let (rest, reference_target) = rest
        .split_once(" references ")
        .map(|(left, right)| {
            (
                left.trim().to_string(),
                Some(QualifiedName::parse(right.trim())),
            )
        })
        .unwrap_or((rest, None));
    let (name_part, expression) = rest
        .split_once('=')
        .map(|(left, right)| (left.trim(), Some(right.trim().to_string())))
        .unwrap_or((rest.trim(), None));
    let (name, ty) = name_part
        .split_once(':')
        .map(|(left, right)| {
            (
                clean_fake_name(left),
                Some(QualifiedName::parse(right.trim())),
            )
        })
        .unwrap_or((clean_fake_name(name_part), None));

    Ok(Declaration::Usage(Usage {
        keyword,
        name,
        is_implicit_name: false,
        ty,
        reference_target,
        metadata_properties: BTreeMap::new(),
        multiplicity: None,
        expression,
        additional_types: Vec::new(),
        specializes: Vec::new(),
        subsets: Vec::new(),
        redefines: Vec::new(),
        members,
        raw_body: None,
        comments: Vec::new(),
        docs: Vec::new(),
        modifiers: Vec::new(),
    }))
}

#[cfg(any(test, feature = "toy-parser"))]
fn clean_fake_name(value: &str) -> String {
    value
        .split_whitespace()
        .next()
        .unwrap_or(value)
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .to_string()
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_authoring_project_to_kir(project: &AuthoringProject) -> KirDocument {
    let mut elements = Vec::new();
    for (path, file) in &project.files {
        if let Some(package) = &file.module.package {
            fake_emit_package(package, path, &mut elements);
        }
        for member in &file.module.members {
            fake_emit_declaration(member, None, path, &mut elements);
        }
    }
    KirDocument {
        metadata: BTreeMap::from([("kir_schema_version".to_string(), json!(KIR_SCHEMA_VERSION))]),
        elements,
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_emit_package(package: &Package, source_file: &str, elements: &mut Vec<KirElement>) {
    let package_qname = package.name.as_dot_string();
    let member_ids = package
        .members
        .iter()
        .map(|member| fake_declaration_id(member, &package_qname))
        .collect::<Vec<_>>();
    elements.push(KirElement {
        id: format!("pkg.{package_qname}"),
        kind: "model.Package".to_string(),
        layer: 2,
        properties: BTreeMap::from([
            ("qualified_name".to_string(), json!(package_qname)),
            (
                "declared_name".to_string(),
                json!(package.name.tail().unwrap_or("Package")),
            ),
            ("members".to_string(), json!(member_ids)),
            (
                "metadata".to_string(),
                json!({ "source_file": source_file }),
            ),
        ]),
    });
    for member in &package.members {
        fake_emit_declaration(member, Some(&package_qname), source_file, elements);
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_emit_declaration(
    declaration: &Declaration,
    owner: Option<&str>,
    source_file: &str,
    elements: &mut Vec<KirElement>,
) {
    match declaration {
        Declaration::Package(package) => fake_emit_package(package, source_file, elements),
        Declaration::Definition(definition) => {
            let qname = owner
                .map(|owner| format!("{owner}.{}", definition.name))
                .unwrap_or_else(|| definition.name.clone());
            let member_ids = definition
                .members
                .iter()
                .map(|member| fake_declaration_id(member, &qname))
                .collect::<Vec<_>>();
            elements.push(KirElement {
                id: format!("type.{qname}"),
                kind: fake_definition_kind(&definition.keyword),
                layer: 2,
                properties: BTreeMap::from([
                    ("qualified_name".to_string(), json!(qname)),
                    ("declared_name".to_string(), json!(definition.name)),
                    ("members".to_string(), json!(member_ids)),
                    (
                        "metadata".to_string(),
                        json!({ "source_file": source_file }),
                    ),
                ]),
            });
            for member in &definition.members {
                fake_emit_declaration(member, Some(&qname), source_file, elements);
            }
        }
        Declaration::Usage(usage) => {
            let owner = owner.unwrap_or("root");
            let qname = format!("{owner}.{}", usage.name);
            let id = if usage.reference_target.is_some() {
                format!("relationship.{qname}")
            } else {
                format!("feature.{qname}")
            };
            let mut properties = BTreeMap::from([
                ("qualified_name".to_string(), json!(qname)),
                ("declared_name".to_string(), json!(usage.name)),
                ("owner".to_string(), json!(format!("type.{owner}"))),
                (
                    "metadata".to_string(),
                    json!({ "source_file": source_file }),
                ),
            ]);
            if let Some(ty) = &usage.ty {
                properties.insert("type".to_string(), json!(ty.as_dot_string()));
            }
            if let Some(expression) = &usage.expression {
                properties.insert("expression_ir".to_string(), json!(expression));
            }
            if usage.reference_target.is_some() {
                properties.insert("source".to_string(), json!(format!("type.{owner}")));
                let target = usage
                    .reference_target
                    .as_ref()
                    .map(QualifiedName::as_dot_string)
                    .unwrap_or_else(|| usage.name.clone());
                properties.insert("target".to_string(), json!(format!("type.{target}")));
            }
            elements.push(KirElement {
                id,
                kind: fake_usage_kind(&usage.keyword),
                layer: 2,
                properties,
            });
            for member in &usage.members {
                fake_emit_declaration(member, Some(&qname), source_file, elements);
            }
        }
        Declaration::Import(_) | Declaration::Alias(_) => {}
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_declaration_id(declaration: &Declaration, owner: &str) -> String {
    match declaration {
        Declaration::Package(package) => format!("pkg.{}", package.name.as_dot_string()),
        Declaration::Definition(definition) => format!("type.{owner}.{}", definition.name),
        Declaration::Usage(usage) if usage.reference_target.is_some() => {
            format!("relationship.{owner}.{}", usage.name)
        }
        Declaration::Usage(usage) => format!("feature.{owner}.{}", usage.name),
        Declaration::Import(import) => format!("import.{}", import.path.as_dot_string()),
        Declaration::Alias(alias) => format!("alias.{owner}.{}", alias.name),
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_definition_kind(keyword: &str) -> String {
    match keyword {
        "action" => "model.ActionDefinition".to_string(),
        "metadata" => "model.MetadataDefinition".to_string(),
        "attribute" => "model.AttributeDefinition".to_string(),
        other => return format!("model.{}Definition", pascal_case(other)),
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn fake_usage_kind(keyword: &str) -> String {
    match keyword {
        "attribute" => "model.AttributeUsage".to_string(),
        "action" => "model.ActionUsage".to_string(),
        other => return format!("model.{}Usage", pascal_case(other)),
    }
}

#[cfg(any(test, feature = "toy-parser"))]
fn pascal_case(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    chars.as_str().to_ascii_lowercase()
                ),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AttributeWritePolicy, ContainerSelector, Mutation, QualifiedName, SemanticEdit,
        create_empty_model, load_authoring_project_from_kir,
    };
    use crate::kir::{KIR_SCHEMA_VERSION, KirDocument, KirElement};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    fn qname(value: &str) -> QualifiedName {
        QualifiedName::parse(value)
    }

    fn kir_document(elements: Vec<KirElement>) -> KirDocument {
        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                json!(KIR_SCHEMA_VERSION),
            )]),
            elements,
        }
    }

    fn kir_element(
        id: &str,
        kind: &str,
        source_file: &str,
        properties: impl IntoIterator<Item = (&'static str, Value)>,
    ) -> KirElement {
        let mut properties = properties
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<BTreeMap<_, _>>();
        properties.insert(
            "metadata".to_string(),
            json!({ "source_file": source_file }),
        );
        KirElement {
            id: id.to_string(),
            kind: kind.to_string(),
            layer: 2,
            properties,
        }
    }

    #[test]
    fn empty_project_can_emit_new_model_file_after_mutation() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();

        let definition_result = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "part".to_string(),
                name: "Vehicle".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&definition_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::CanonicalRewrite);
        assert!(text.contains("package Demo {"));
        assert!(text.contains("part def Vehicle;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn rename_rewrites_existing_definition_locally() {
        let mut project = load_authoring_project_from_kir(&kir_document(vec![
            kir_element(
                "pkg.Demo",
                "model.Package",
                "model.model",
                [
                    ("qualified_name", json!("Demo")),
                    ("declared_name", json!("Demo")),
                    ("members", json!(["type.Demo.Vehicle"])),
                ],
            ),
            kir_element(
                "type.Demo.Vehicle",
                "model.PartDefinition",
                "model.model",
                [
                    ("qualified_name", json!("Demo.Vehicle")),
                    ("declared_name", json!("Vehicle")),
                ],
            ),
        ]))
        .unwrap();

        let mutation = project
            .apply_mutation(Mutation::RenameDeclaration {
                qualified_name: qname("Demo.Vehicle"),
                new_name: "Car".to_string(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&mutation).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::CanonicalRewrite);
        assert!(text.contains("part def Car;"));
        assert!(!text.contains("part def Vehicle;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn adding_nested_usage_rewrites_only_owner_declaration() {
        let mut project = load_authoring_project_from_kir(&kir_document(vec![
            kir_element(
                "pkg.Demo",
                "model.Package",
                "model.model",
                [
                    ("qualified_name", json!("Demo")),
                    ("declared_name", json!("Demo")),
                    ("members", json!(["type.Demo.Engine", "type.Demo.Vehicle"])),
                ],
            ),
            kir_element(
                "type.Demo.Engine",
                "model.PartDefinition",
                "model.model",
                [
                    ("qualified_name", json!("Demo.Engine")),
                    ("declared_name", json!("Engine")),
                ],
            ),
            kir_element(
                "type.Demo.Vehicle",
                "model.PartDefinition",
                "model.model",
                [
                    ("qualified_name", json!("Demo.Vehicle")),
                    ("declared_name", json!("Vehicle")),
                ],
            ),
        ]))
        .unwrap();

        let mutation = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.Vehicle"),
                },
                keyword: "part".to_string(),
                name: "engine".to_string(),
                ty: Some(qname("Engine")),
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&mutation).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::CanonicalRewrite);
        assert!(text.contains("part engine: Engine;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn semantic_usage_list_edits_render_relationship_clauses() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        for name in ["Vehicle", "BaseFeature", "CrossFeature", "RedefinedFeature"] {
            let result = project
                .apply_mutation(Mutation::AddDefinition {
                    container: ContainerSelector::Package {
                        qualified_name: qname("Demo"),
                    },
                    keyword: "part".to_string(),
                    name: name.to_string(),
                    specializes: Vec::new(),
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }
        let usage_result = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "part".to_string(),
                name: "vehicle".to_string(),
                ty: Some(qname("Vehicle")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&usage_result).unwrap();

        let set_result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.vehicle"),
                attribute: "additionalTypes".to_string(),
                value: json!("BaseFeature"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        project.write_back_mutation(&set_result).unwrap();
        let add_subset_result = project
            .apply_semantic_edit(SemanticEdit::AddAttributeValue {
                element: qname("Demo.vehicle"),
                attribute: "subsettedFeatures".to_string(),
                value: json!("CrossFeature"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        project.write_back_mutation(&add_subset_result).unwrap();
        let add_redefines_result = project
            .apply_semantic_edit(SemanticEdit::AddAttributeValue {
                element: qname("Demo.vehicle"),
                attribute: "redefinedFeatures".to_string(),
                value: json!("RedefinedFeature"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&add_redefines_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert!(text.contains(
            "part vehicle: Vehicle :> BaseFeature subsets CrossFeature redefines RedefinedFeature;"
        ));
        assert!(write_back.validation.ok);

        let reference_result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.vehicle"),
                attribute: "referenceTarget".to_string(),
                value: json!("CrossFeature"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&reference_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("references CrossFeature"));
        assert!(write_back.validation.ok);

        let clear_reference_result = project
            .apply_semantic_edit(SemanticEdit::ClearAttribute {
                element: qname("Demo.vehicle"),
                attribute: "referenceTarget".to_string(),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        project
            .write_back_mutation(&clear_reference_result)
            .unwrap();

        let remove_result = project
            .apply_semantic_edit(SemanticEdit::RemoveAttributeValue {
                element: qname("Demo.vehicle"),
                attribute: "subsettedFeature".to_string(),
                value: json!("CrossFeature"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&remove_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(!text.contains("subsets CrossFeature"));
        assert!(text.contains("redefines RedefinedFeature"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn semantic_multiplicity_edit_renders_usage_bounds() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        for name in ["Vehicle", "Wheel"] {
            let result = project
                .apply_mutation(Mutation::AddDefinition {
                    container: ContainerSelector::Package {
                        qualified_name: qname("Demo"),
                    },
                    keyword: "part".to_string(),
                    name: name.to_string(),
                    specializes: Vec::new(),
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }
        let vehicle_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "part".to_string(),
                name: "vehicle".to_string(),
                ty: Some(qname("Vehicle")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&vehicle_usage).unwrap();
        let wheel_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.vehicle"),
                },
                keyword: "part".to_string(),
                name: "wheel".to_string(),
                ty: Some(qname("Wheel")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&wheel_usage).unwrap();

        let set_result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.vehicle.wheel"),
                attribute: "multiplicity".to_string(),
                value: json!("[4]"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&set_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("part wheel: Wheel[4];"));
        assert!(write_back.validation.ok);

        let clear_result = project
            .apply_semantic_edit(SemanticEdit::ClearAttribute {
                element: qname("Demo.vehicle.wheel"),
                attribute: "multiplicity".to_string(),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&clear_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("part wheel: Wheel;"));
        assert!(!text.contains("Wheel[4]"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn semantic_modifier_edits_render_advanced_usage_modifiers() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let attribute_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "attribute".to_string(),
                name: "Mass".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&attribute_def).unwrap();
        let mass_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "attribute".to_string(),
                name: "mass".to_string(),
                ty: Some(qname("Mass")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&mass_usage).unwrap();

        for (attribute, value) in [
            ("isOrdered", json!(true)),
            ("isUnique", json!(false)),
            ("isDerived", json!(true)),
            ("isVariable", json!(true)),
        ] {
            let result = project
                .apply_semantic_edit(SemanticEdit::SetAttribute {
                    element: qname("Demo.mass"),
                    attribute: attribute.to_string(),
                    value,
                    policy: AttributeWritePolicy::UpsertDirect,
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }

        let text = project.render_new_file("model.model").unwrap();
        assert!(text.contains("ordered nonunique derived variable attribute mass: Mass;"));
        let attributes = project.semantic_attributes(&qname("Demo.mass")).unwrap();
        assert!(attributes.iter().any(|row| {
            row.name == "is_unique" && row.effective_value == Some(Value::Bool(false))
        }));

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.mass"),
                attribute: "isUnique".to_string(),
                value: json!(true),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("ordered derived variable attribute mass: Mass;"));
        assert!(!text.contains("nonunique"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn semantic_short_name_edit_renders_angle_adornment() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let attribute_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "attribute".to_string(),
                name: "Mass".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&attribute_def).unwrap();
        let mass_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "attribute".to_string(),
                name: "mass".to_string(),
                ty: Some(qname("Mass")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&mass_usage).unwrap();

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.mass"),
                attribute: "declaredShortName".to_string(),
                value: json!("m"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("attribute <m> mass: Mass;"));
        assert!(write_back.validation.ok);

        let attributes = project.semantic_attributes(&qname("Demo.mass")).unwrap();
        assert!(attributes.iter().any(|row| {
            row.name == "declared_short_name"
                && row.effective_value == Some(Value::String("m".to_string()))
        }));

        let clear_result = project
            .apply_semantic_edit(SemanticEdit::ClearAttribute {
                element: qname("Demo.mass"),
                attribute: "declaredShortName".to_string(),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&clear_result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("attribute mass: Mass;"));
        assert!(!text.contains("<m>"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn add_relationship_renders_flow_succession_and_allocation_shorthands() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        for name in ["Source", "Target"] {
            let result = project
                .apply_mutation(Mutation::AddDefinition {
                    container: ContainerSelector::Package {
                        qualified_name: qname("Demo"),
                    },
                    keyword: "part".to_string(),
                    name: name.to_string(),
                    specializes: Vec::new(),
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }

        for kind in ["flow", "succession", "allocation"] {
            let result = project
                .apply_mutation(Mutation::AddRelationship {
                    container: ContainerSelector::Package {
                        qualified_name: qname("Demo"),
                    },
                    kind: kind.to_string(),
                    source: qname("Source"),
                    target: qname("Target"),
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }

        let text = project.render_new_file("model.model").unwrap();
        assert!(text.contains("flow from Source to Target;"));
        assert!(text.contains("succession flow from Source to Target;"));
        assert!(text.contains("allocation Target allocate Source to Target;"));
        assert!(project.compile_kir_document().is_ok());
    }

    #[test]
    fn perform_usage_renders_action_shorthand() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let action_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "action".to_string(),
                name: "ProvidePower".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&action_def).unwrap();
        let perform = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "perform".to_string(),
                name: "providePower".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&perform).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("perform action providePower;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn raw_action_body_edit_renders_and_validates() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let action_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "action".to_string(),
                name: "ProvidePower".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&action_def).unwrap();

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.ProvidePower"),
                attribute: "rawBody".to_string(),
                value: json!("first start;\nthen done;"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("action def ProvidePower {"));
        assert!(text.contains("first start;"));
        assert!(text.contains("then done;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn raw_constraint_body_edit_renders_and_validates() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let constraint_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "constraint".to_string(),
                name: "MassConstraint".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&constraint_def).unwrap();
        let constraint_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "constraint".to_string(),
                name: "massConstraint".to_string(),
                ty: Some(qname("MassConstraint")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&constraint_usage).unwrap();

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.MassConstraint"),
                attribute: "rawBody".to_string(),
                value: json!("in totalMass;\nin componentMasses;\ntotalMass == componentMasses"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        project.write_back_mutation(&result).unwrap();
        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.massConstraint"),
                attribute: "rawBody".to_string(),
                value: json!("in totalMass = mass;\nin componentMasses = engine.mass;"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("constraint def MassConstraint {"));
        assert!(text.contains("totalMass == componentMasses"));
        assert!(text.contains("constraint massConstraint: MassConstraint {"));
        assert!(text.contains("in totalMass = mass;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn metadata_usage_renders_keyword_form_with_about_target() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let metadata_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "metadata".to_string(),
                name: "Safety".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&metadata_def).unwrap();
        let part_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "part".to_string(),
                name: "Vehicle".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&part_def).unwrap();
        let metadata_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "metadata".to_string(),
                name: "vehicleSafety".to_string(),
                ty: Some(qname("Safety")),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&metadata_usage).unwrap();

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.vehicleSafety"),
                attribute: "referenceTarget".to_string(),
                value: json!("Vehicle"),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("metadata def Safety;"));
        assert!(
            text.contains("metadata vehicleSafety: Safety about Vehicle;"),
            "{text}"
        );
        assert!(write_back.validation.ok);
    }

    #[test]
    fn metadata_usage_renders_multiple_about_targets() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let metadata_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "metadata".to_string(),
                name: "Safety".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&metadata_def).unwrap();
        let metadata_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "metadata".to_string(),
                name: "Safety".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&metadata_usage).unwrap();
        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.Safety"),
                attribute: "about".to_string(),
                value: json!(["vehicle::seatBelt", "vehicle::airBag", "vehicle::bumper"]),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert!(text.contains(
            "metadata Safety about vehicle::seatBelt, vehicle::airBag, vehicle::bumper;"
        ));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn metadata_definition_renders_structured_annotated_elements() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let metadata_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "metadata".to_string(),
                name: "SecurityFeature".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&metadata_def).unwrap();

        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.SecurityFeature"),
                attribute: "annotatedElements".to_string(),
                value: json!(["SysML::PartDefinition", "SysML::PartUsage"]),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("metadata def SecurityFeature {"));
        assert!(text.contains(":> annotatedElement : SysML::PartDefinition;"));
        assert!(text.contains(":> annotatedElement : SysML::PartUsage;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn language_extension_keywords_render_hash_forms() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let scenario = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "scenario".to_string(),
                name: "DeviceFailure".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&scenario).unwrap();
        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.DeviceFailure"),
                attribute: "isLanguageExtensionKeyword".to_string(),
                value: json!(true),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        project.write_back_mutation(&result).unwrap();
        let service_port = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "port".to_string(),
                name: "ServiceDiscovery".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&service_port).unwrap();
        let result = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.ServiceDiscovery"),
                attribute: "languageExtensions".to_string(),
                value: json!(["service"]),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        let write_back = project.write_back_mutation(&result).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("#scenario def DeviceFailure;"));
        assert!(text.contains("#service port def ServiceDiscovery;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn use_case_keyword_renders_two_word_source_form() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let use_case_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "use-case".to_string(),
                name: "TransportPassenger".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&use_case_def).unwrap();
        let use_case_usage = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "use-case".to_string(),
                name: "transportPassenger".to_string(),
                ty: Some(qname("TransportPassenger")),
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&use_case_usage).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();
        assert!(text.contains("use case def TransportPassenger;"));
        assert!(text.contains("use case transportPassenger: TransportPassenger;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn transition_usage_renders_state_shorthand() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let state_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "state".to_string(),
                name: "VehicleStates".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&state_def).unwrap();
        let transition = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.VehicleStates"),
                },
                keyword: "transition".to_string(),
                name: "off_to_starting".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&transition).unwrap();
        for (attribute, value) in [
            ("transitionSource", "off"),
            ("trigger", "VehicleStartSignal"),
            ("transitionTarget", "starting"),
        ] {
            let result = project
                .apply_semantic_edit(SemanticEdit::SetAttribute {
                    element: qname("Demo.VehicleStates.off_to_starting"),
                    attribute: attribute.to_string(),
                    value: json!(value),
                    policy: AttributeWritePolicy::UpsertDirect,
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }
        let text = project.render_new_file("model.model").unwrap();
        assert!(text.contains(
            "transition off_to_starting first off accept VehicleStartSignal then starting;"
        ));
        assert!(project.compile_user_kir().is_ok());
    }

    #[test]
    fn transition_shorthand_renders_guarded_trigger_kinds() {
        // Guarded triggers keep their guard keyword (`when` / `after` / `at`)
        // in `trigger_kind`; the rendered shorthand must restore it or the
        // text does not re-parse (DA-11 Tier-T write-back idempotence).
        let mut guarded = super::Usage {
            keyword: "transition".to_string(),
            name: "heating_ready".to_string(),
            is_implicit_name: false,
            ty: None,
            reference_target: None,
            metadata_properties: BTreeMap::new(),
            multiplicity: None,
            expression: None,
            additional_types: Vec::new(),
            specializes: Vec::new(),
            subsets: Vec::new(),
            redefines: Vec::new(),
            members: Vec::new(),
            raw_body: None,
            comments: Vec::new(),
            docs: Vec::new(),
            modifiers: vec![
                "transition_source=Heating".to_string(),
                "trigger_kind=when".to_string(),
                "trigger=temperature >= targetTemp".to_string(),
                "transition_target=Ready".to_string(),
            ],
        };
        assert_eq!(
            super::render_transition_shorthand(&guarded),
            Some(
                "transition heating_ready first Heating accept when temperature >= targetTemp then Ready;"
                    .to_string()
            )
        );

        guarded.modifiers = vec![
            "transition_source=Cold".to_string(),
            "trigger_kind=after".to_string(),
            "trigger=0.5".to_string(),
            "transition_target=Heating".to_string(),
        ];
        assert_eq!(
            super::render_transition_shorthand(&guarded),
            Some("transition heating_ready first Cold accept after 0.5 then Heating;".to_string())
        );

        // Event triggers (bare signal names) stay unadorned.
        guarded.modifiers = vec![
            "transition_source=off".to_string(),
            "trigger_kind=event".to_string(),
            "trigger=VehicleStartSignal".to_string(),
            "transition_target=starting".to_string(),
        ];
        assert_eq!(
            super::render_transition_shorthand(&guarded),
            Some(
                "transition heating_ready first off accept VehicleStartSignal then starting;"
                    .to_string()
            )
        );
    }

    #[test]
    fn quoted_names_render_for_packages_definitions_and_usages() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("'Subsetting Example'"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let vehicle_part = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Subsetting Example"),
                },
                keyword: "part".to_string(),
                name: "Vehicle Part".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&vehicle_part).unwrap();
        let vehicle = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Subsetting Example"),
                },
                keyword: "part".to_string(),
                name: "'Vehicle Definition'".to_string(),
                specializes: vec![qname("Subsetting Example.Vehicle Part")],
            })
            .unwrap();
        project.write_back_mutation(&vehicle).unwrap();
        let wheel = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Subsetting Example.Vehicle Definition"),
                },
                keyword: "part".to_string(),
                name: "front wheel".to_string(),
                ty: Some(qname("Vehicle Part")),
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&wheel).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert!(text.contains("package 'Subsetting Example'"));
        assert!(text.contains("part def 'Vehicle Part';"));
        assert!(text.contains(
            "part def 'Vehicle Definition' specializes 'Subsetting Example'::'Vehicle Part'"
        ));
        assert!(text.contains("part 'front wheel': 'Vehicle Part';"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn transition_usage_renders_initial_state_shorthand() {
        let mut project = create_empty_model();
        let package_result = project
            .apply_mutation(Mutation::AddPackage {
                target_file: "model.model".to_string(),
                package_name: qname("Demo"),
            })
            .unwrap();
        project.write_back_mutation(&package_result).unwrap();
        let state_def = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("Demo"),
                },
                keyword: "state".to_string(),
                name: "VehicleStates".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&state_def).unwrap();
        let transition = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.VehicleStates"),
                },
                keyword: "transition".to_string(),
                name: "start".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        project.write_back_mutation(&transition).unwrap();
        for (attribute, value) in [
            ("transitionSource", json!("start")),
            ("sourceIsInitial", json!(true)),
            ("transitionTarget", json!("off")),
        ] {
            let result = project
                .apply_semantic_edit(SemanticEdit::SetAttribute {
                    element: qname("Demo.VehicleStates.start"),
                    attribute: attribute.to_string(),
                    value,
                    policy: AttributeWritePolicy::UpsertDirect,
                })
                .unwrap();
            project.write_back_mutation(&result).unwrap();
        }
        let text = project.render_all_files().remove("model.model").unwrap();

        assert!(text.contains("first start then off;"));
        assert!(!text.contains("transition start first start then off;"));
    }

    #[test]
    fn adding_metadata_annotation_round_trips_through_source() {
        let mut project = load_authoring_project_from_kir(&kir_document(vec![
            kir_element(
                "pkg.Demo",
                "model.Package",
                "model.model",
                [
                    ("qualified_name", json!("Demo")),
                    ("declared_name", json!("Demo")),
                    ("members", json!(["type.Demo.safeStart"])),
                ],
            ),
            kir_element(
                "type.Demo.safeStart",
                "model.Requirement",
                "model.model",
                [
                    ("qualified_name", json!("Demo.safeStart")),
                    ("declared_name", json!("safeStart")),
                ],
            ),
        ]))
        .unwrap();

        let mutation = project
            .apply_mutation(Mutation::AddMetadataAnnotation {
                element: qname("Demo.safeStart"),
                metadata_type: "ReviewTag".to_string(),
                properties: [
                    ("owner".to_string(), "Safety Team".to_string()),
                    ("status".to_string(), "draft".to_string()),
                ]
                .into_iter()
                .collect(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&mutation).unwrap();
        let text = write_back.edited_files.get("model.model").unwrap();

        assert!(text.contains("@ReviewTag"));
        assert!(text.contains("owner = \"Safety Team\";"));
        assert!(text.contains("status = draft;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn multi_file_top_level_addition_requires_target_file_and_edits_only_that_file() {
        let mut project = load_authoring_project_from_kir(&kir_document(vec![
            kir_element(
                "pkg.A",
                "model.Package",
                "a.model",
                [
                    ("qualified_name", json!("A")),
                    ("declared_name", json!("A")),
                    ("members", json!(["type.A.Vehicle"])),
                ],
            ),
            kir_element(
                "type.A.Vehicle",
                "model.PartDefinition",
                "a.model",
                [
                    ("qualified_name", json!("A.Vehicle")),
                    ("declared_name", json!("Vehicle")),
                ],
            ),
            kir_element(
                "pkg.B",
                "model.Package",
                "b.model",
                [
                    ("qualified_name", json!("B")),
                    ("declared_name", json!("B")),
                    ("members", json!(["type.B.Engine"])),
                ],
            ),
            kir_element(
                "type.B.Engine",
                "model.PartDefinition",
                "b.model",
                [
                    ("qualified_name", json!("B.Engine")),
                    ("declared_name", json!("Engine")),
                ],
            ),
        ]))
        .unwrap();

        let mutation = project
            .apply_mutation(Mutation::AddDefinition {
                container: ContainerSelector::Package {
                    qualified_name: qname("B"),
                },
                keyword: "part".to_string(),
                name: "Brake".to_string(),
                specializes: Vec::new(),
            })
            .unwrap();
        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert!(write_back.edited_files.contains_key("b.model"));
        assert!(!write_back.edited_files.contains_key("a.model"));
        assert!(write_back.edited_files["b.model"].contains("part def Brake;"));
        assert!(write_back.validation.ok);
    }

    #[test]
    fn moving_declaration_between_files_updates_source_and_destination() {
        let mut project = load_authoring_project_from_kir(&kir_document(vec![
            kir_element(
                "pkg.A",
                "model.Package",
                "a.model",
                [
                    ("qualified_name", json!("A")),
                    ("declared_name", json!("A")),
                    ("members", json!(["type.A.Vehicle"])),
                ],
            ),
            kir_element(
                "type.A.Vehicle",
                "model.PartDefinition",
                "a.model",
                [
                    ("qualified_name", json!("A.Vehicle")),
                    ("declared_name", json!("Vehicle")),
                ],
            ),
            kir_element(
                "pkg.B",
                "model.Package",
                "b.model",
                [
                    ("qualified_name", json!("B")),
                    ("declared_name", json!("B")),
                    ("members", json!([])),
                ],
            ),
        ]))
        .unwrap();

        let mutation = project
            .apply_mutation(Mutation::MoveDeclaration {
                qualified_name: qname("A.Vehicle"),
                destination: ContainerSelector::Package {
                    qualified_name: qname("B"),
                },
            })
            .unwrap();
        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert!(write_back.edited_files["a.model"].contains("package A {\n}\n"));
        assert!(write_back.edited_files["b.model"].contains("Vehicle"));
        assert!(write_back.validation.ok);
    }

    const COMMENTED_SOURCE: &str = "// keep me\npackage Demo {\n    // vehicle def\n    part def Vehicle {\n        part engine;\n    }\n}\n";

    fn ast_span(
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> crate::authoring::frontend::ast::SourceSpan {
        crate::authoring::frontend::ast::SourceSpan {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Hand-built AST mirroring `COMMENTED_SOURCE`, spans included, so the
    /// localized write-back machinery can be exercised without a parser.
    fn commented_project() -> super::AuthoringProject {
        use crate::authoring::frontend::ast;

        let engine = ast::Declaration::GenericUsage(ast::GenericUsageDecl {
            keyword: "part".to_string(),
            name: "engine".to_string(),
            is_implicit_name: false,
            ty: None,
            reference_target: None,
            allocation_source: None,
            allocation_target: None,
            metadata_properties: BTreeMap::new(),
            multiplicity: None,
            expression: None,
            additional_types: Vec::new(),
            specializes: Vec::new(),
            subsets: Vec::new(),
            redefines: Vec::new(),
            body_members: Vec::new(),
            comments: Vec::new(),
            docs: Vec::new(),
            modifiers: Vec::new(),
            span: ast_span(5, 9, 5, 20),
        });
        let vehicle = ast::Declaration::GenericDefinition(ast::GenericDefinitionDecl {
            keyword: "part".to_string(),
            name: "Vehicle".to_string(),
            specializes: Vec::new(),
            members: vec![engine],
            comments: vec![crate::authoring::frontend::ast::CommentNote {
                text: " vehicle def".to_string(),
                kind: crate::authoring::frontend::ast::CommentKind::Line,
            }],
            docs: Vec::new(),
            modifiers: Vec::new(),
            span: ast_span(4, 5, 6, 5),
        });
        let package = ast::PackageDecl {
            name: ast::QualifiedName {
                segments: vec!["Demo".to_string()],
                span: ast_span(2, 9, 2, 12),
            },
            members: vec![vehicle],
            imports: Vec::new(),
            definitions: Vec::new(),
            comments: vec![crate::authoring::frontend::ast::CommentNote {
                text: " keep me".to_string(),
                kind: crate::authoring::frontend::ast::CommentKind::Line,
            }],
            docs: Vec::new(),
            modifiers: Vec::new(),
            span: ast_span(2, 1, 7, 1),
        };
        let module = crate::authoring::frontend::ast::ParsedModule {
            package: Some(package),
            members: Vec::new(),
            imports: Vec::new(),
            definitions: Vec::new(),
        };
        super::AuthoringProject::from_parsed_modules(
            BTreeMap::from([("demo.sysml".to_string(), module)]),
            BTreeMap::from([("demo.sysml".to_string(), COMMENTED_SOURCE.to_string())]),
        )
        .unwrap()
    }

    fn replace_node(anchor: &str) -> super::RewriteInstruction {
        super::RewriteInstruction::ReplaceNode {
            file: "demo.sysml".to_string(),
            anchor_qname: anchor.to_string(),
            render_qname: anchor.to_string(),
        }
    }

    fn manual_mutation(rewrite_plan: Vec<super::RewriteInstruction>) -> super::MutationResult {
        super::MutationResult {
            changed_files: ["demo.sysml".to_string()].into(),
            changed_declarations: Default::default(),
            affected_element_ids: Default::default(),
            rewrite_plan,
        }
    }

    #[test]
    fn mutation_result_merge_unions_fields_and_keeps_plan_order() {
        let mut merged = manual_mutation(vec![replace_node("Demo.Vehicle")]);
        merged
            .changed_declarations
            .insert("Demo.Vehicle".to_string());
        merged
            .affected_element_ids
            .insert("type.Demo.Vehicle".to_string());

        let mut other = manual_mutation(vec![replace_node("Demo.Vehicle.engine")]);
        other.changed_files.insert("other.sysml".to_string());
        other
            .changed_declarations
            .insert("Demo.Vehicle.engine".to_string());
        other
            .affected_element_ids
            .insert("feature.Demo.Vehicle.engine".to_string());

        merged.merge(other);

        assert!(merged.changed_files.contains("demo.sysml"));
        assert!(merged.changed_files.contains("other.sysml"));
        assert!(merged.changed_declarations.contains("Demo.Vehicle"));
        assert!(merged.changed_declarations.contains("Demo.Vehicle.engine"));
        assert!(merged.affected_element_ids.contains("type.Demo.Vehicle"));
        assert!(
            merged
                .affected_element_ids
                .contains("feature.Demo.Vehicle.engine")
        );
        assert_eq!(
            merged.rewrite_plan,
            vec![
                replace_node("Demo.Vehicle"),
                replace_node("Demo.Vehicle.engine"),
            ]
        );
    }

    #[test]
    fn localized_writeback_dedupes_and_subsumes_rewrite_instructions() {
        let mut project = commented_project();
        // One duplicate anchor and one anchor whose span sits inside it: the
        // normalized plan must collapse to a single patch.
        let mutation = manual_mutation(vec![
            replace_node("Demo.Vehicle"),
            replace_node("Demo.Vehicle"),
            replace_node("Demo.Vehicle.engine"),
        ]);

        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::LocalizedPatch);
        assert_eq!(write_back.changed_spans["demo.sysml"].len(), 1);
        let text = write_back.edited_files.get("demo.sysml").unwrap();
        assert!(text.contains("// keep me"));
        assert!(text.contains("part engine;"));
    }

    #[test]
    fn merged_plan_drops_unmapped_child_covered_by_ancestor_container() {
        let mut project = commented_project();
        let mut merged = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.Vehicle"),
                },
                keyword: "part".to_string(),
                name: "wheel".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        // The doc edit anchors at the just-created usage, which has no
        // parse-time span; the merged plan must fall back to the ancestor
        // container re-render instead of the canonical rewrite.
        let doc_edit = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.Vehicle.wheel"),
                attribute: "doc".to_string(),
                value: json!("Front axle."),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();
        merged.merge(doc_edit);

        let write_back = project.write_back_mutation(&merged).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::LocalizedPatch);
        let text = write_back.edited_files.get("demo.sysml").unwrap();
        assert!(text.contains("// keep me"));
        assert!(text.contains("part wheel"));
        assert!(text.contains("Front axle."));
    }

    #[test]
    fn unmapped_anchor_without_ancestor_falls_back_to_canonical() {
        let mut project = commented_project();
        let mutation = manual_mutation(vec![replace_node("Demo.Ghost")]);

        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::CanonicalRewrite);
    }

    #[test]
    fn write_back_invalidates_source_map_so_stale_spans_never_splice() {
        let mut project = commented_project();
        let first = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.Vehicle"),
                },
                keyword: "part".to_string(),
                name: "wheel".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        let first_write_back = project.write_back_mutation(&first).unwrap();
        assert_eq!(first_write_back.mode, super::WriteBackMode::LocalizedPatch);

        // The accepted write-back rewrote the file, so the parse-time spans
        // are stale; the next mutation must not splice with them.
        let second = project
            .apply_mutation(Mutation::AddUsage {
                container: ContainerSelector::Declaration {
                    qualified_name: qname("Demo.Vehicle"),
                },
                keyword: "part".to_string(),
                name: "axle".to_string(),
                ty: None,
                specializes: Vec::new(),
            })
            .unwrap();
        let second_write_back = project.write_back_mutation(&second).unwrap();
        assert_eq!(
            second_write_back.mode,
            super::WriteBackMode::CanonicalRewrite
        );
        assert!(second_write_back.edited_files["demo.sysml"].contains("part axle;"));
        // Canonical fallback re-renders the whole file from the model; the
        // preserved comments must ride along.
        assert!(second_write_back.edited_files["demo.sysml"].contains("// keep me"));
        assert!(second_write_back.edited_files["demo.sysml"].contains("// vehicle def"));
    }

    #[test]
    fn render_comments_reproduces_line_and_block_comments_verbatim() {
        use crate::authoring::frontend::ast::{CommentKind, CommentNote};

        let comments = vec![
            CommentNote {
                text: " single line".to_string(),
                kind: CommentKind::Line,
            },
            CommentNote {
                text: " one block ".to_string(),
                kind: CommentKind::Block,
            },
            CommentNote {
                text: "\n     * multi\n     * line\n     ".to_string(),
                kind: CommentKind::Block,
            },
        ];

        let rendered = super::render_comments(&comments, 4);

        assert_eq!(rendered[0], "    // single line");
        assert_eq!(rendered[1], "    /* one block */");
        assert_eq!(rendered[2], "    /*\n     * multi\n     * line\n     */");
    }

    #[test]
    fn canonical_render_emits_leading_comments_before_declarations() {
        let project = commented_project();

        let write_back = project
            .write_back_changed_files(&["demo.sysml".to_string()].into())
            .unwrap();

        let text = &write_back.edited_files["demo.sysml"];
        assert!(text.contains("// keep me"));
        assert!(text.contains("// vehicle def"));
        let comment_at = text.find("// vehicle def").unwrap();
        let declaration_at = text.find("part def Vehicle").unwrap();
        assert!(
            comment_at < declaration_at,
            "the leading comment must render above its declaration:\n{text}"
        );
        let package_comment_at = text.find("// keep me").unwrap();
        let package_at = text.find("package Demo").unwrap();
        assert!(package_comment_at < package_at, "{text}");
    }

    #[test]
    fn localized_replace_never_duplicates_the_declarations_leading_comment() {
        let mut project = commented_project();
        let mutation = manual_mutation(vec![replace_node("Demo.Vehicle")]);

        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::LocalizedPatch);
        let text = &write_back.edited_files["demo.sysml"];
        // The splice starts at the declaration keyword; the comment bytes
        // above it stay in the original text and must not be re-rendered
        // inside the patch.
        assert_eq!(text.matches("// vehicle def").count(), 1, "{text}");
        assert_eq!(text.matches("// keep me").count(), 1, "{text}");
    }

    const DOC_SOURCE: &str = "// keep me\npackage Demo {\n    // vehicle def\n    doc /* Drives the demo. */\n    part def Vehicle;\n}\n";

    /// Hand-built AST mirroring `DOC_SOURCE`: the declaration carries a doc
    /// that the source spells in the doc-before form the canonical printer
    /// emits, with the parse span starting at the declaration keyword.
    fn doc_commented_project() -> super::AuthoringProject {
        use crate::authoring::frontend::ast;

        let vehicle = ast::Declaration::GenericDefinition(ast::GenericDefinitionDecl {
            keyword: "part".to_string(),
            name: "Vehicle".to_string(),
            specializes: Vec::new(),
            members: Vec::new(),
            comments: vec![crate::authoring::frontend::ast::CommentNote {
                text: " vehicle def".to_string(),
                kind: crate::authoring::frontend::ast::CommentKind::Line,
            }],
            docs: vec!["Drives the demo.".to_string()],
            modifiers: Vec::new(),
            span: ast_span(5, 5, 5, 21),
        });
        let package = ast::PackageDecl {
            name: ast::QualifiedName {
                segments: vec!["Demo".to_string()],
                span: ast_span(2, 9, 2, 12),
            },
            members: vec![vehicle],
            imports: Vec::new(),
            definitions: Vec::new(),
            comments: vec![crate::authoring::frontend::ast::CommentNote {
                text: " keep me".to_string(),
                kind: crate::authoring::frontend::ast::CommentKind::Line,
            }],
            docs: Vec::new(),
            modifiers: Vec::new(),
            span: ast_span(2, 1, 6, 1),
        };
        let module = crate::authoring::frontend::ast::ParsedModule {
            package: Some(package),
            members: Vec::new(),
            imports: Vec::new(),
            definitions: Vec::new(),
        };
        super::AuthoringProject::from_parsed_modules(
            BTreeMap::from([("demo.sysml".to_string(), module)]),
            BTreeMap::from([("demo.sysml".to_string(), DOC_SOURCE.to_string())]),
        )
        .unwrap()
    }

    #[test]
    fn localized_replace_of_doc_bearing_declaration_covers_the_doc_line() {
        let mut project = doc_commented_project();
        let mutation = manual_mutation(vec![replace_node("Demo.Vehicle")]);

        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::LocalizedPatch);
        let text = &write_back.edited_files["demo.sysml"];
        // The re-render includes the doc line, so the recorded span must
        // cover the doc bytes already in the source; a keyword-anchored
        // splice would duplicate them.
        assert_eq!(
            text.matches("doc /* Drives the demo. */").count(),
            1,
            "{text}"
        );
        assert_eq!(
            text, DOC_SOURCE,
            "an identity re-render must be byte-stable"
        );
    }

    #[test]
    fn localized_doc_edit_replaces_the_existing_doc_line_in_place() {
        let mut project = doc_commented_project();
        let mutation = project
            .apply_semantic_edit(SemanticEdit::SetAttribute {
                element: qname("Demo.Vehicle"),
                attribute: "text".to_string(),
                value: json!("Updated."),
                policy: AttributeWritePolicy::UpsertDirect,
            })
            .unwrap();

        let write_back = project.write_back_mutation(&mutation).unwrap();

        assert_eq!(write_back.mode, super::WriteBackMode::LocalizedPatch);
        let text = &write_back.edited_files["demo.sysml"];
        assert_eq!(
            text,
            "// keep me\npackage Demo {\n    // vehicle def\n    doc /* Updated. */\n    part def Vehicle;\n}\n",
            "the doc edit must swap the doc line without touching anything else"
        );
    }
}
