use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedName {
    pub segments: Vec<String>,
    pub span: SourceSpan,
}

/// The surface form of a preserved `//` or non-doc `/* */` comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentKind {
    Line,
    Block,
}

/// A non-doc comment preserved on the declaration it precedes. The text is
/// the raw interior (after `//`, or between `/*` and `*/`) so a re-render can
/// reproduce the original bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentNote {
    pub text: String,
    pub kind: CommentKind,
}

impl QualifiedName {
    pub fn as_colon_string(&self) -> String {
        self.segments.join("::")
    }

    pub fn as_dot_string(&self) -> String {
        self.segments.join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    Literal(LiteralExpr),
    Name(QualifiedName),
    SelfRef(SourceSpan),
    Tuple {
        items: Vec<Expr>,
        span: SourceSpan,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: SourceSpan,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: SourceSpan,
    },
    Path {
        root: Box<Expr>,
        segment: String,
        span: SourceSpan,
    },
    Call {
        function: String,
        args: Vec<Expr>,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralExpr {
    Integer(i64),
    Real(String),
    Boolean(bool),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

/// A namespace query: `import`, or its view-scoped twin `expose`.
///
/// The two are parallel branches of the same metamodel shape —
/// `SysML::MembershipImport`/`NamespaceImport` alongside
/// `SysML::MembershipExpose`/`NamespaceExpose` — and share one syntax, so they
/// share one node here (save-as-view SV-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDecl {
    pub path: QualifiedName,
    /// `true` for `expose`, `false` for `import`. Both carry the same path
    /// syntax; only the metaclass and the visibility semantics differ.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_expose: bool,
    /// Raw text of a filter condition, e.g. `@Safety` from
    /// `import vehicle::**[@Safety]`. Held verbatim rather than parsed into an
    /// expression: the parser's job is to stop losing it, and evaluating it
    /// against an element's metadata features belongs to the resolver
    /// (save-as-view SV-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiplicityRange {
    pub lower: String,
    pub upper: String,
    pub raw: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericDefinitionDecl {
    pub keyword: String,
    pub name: String,
    pub specializes: Vec<QualifiedName>,
    pub members: Vec<Declaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericUsageDecl {
    pub keyword: String,
    pub name: String,
    pub is_implicit_name: bool,
    pub ty: Option<QualifiedName>,
    pub reference_target: Option<QualifiedName>,
    pub allocation_source: Option<QualifiedName>,
    pub allocation_target: Option<QualifiedName>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata_properties: BTreeMap<String, String>,
    pub multiplicity: Option<MultiplicityRange>,
    pub expression: Option<Expr>,
    pub additional_types: Vec<QualifiedName>,
    pub specializes: Vec<QualifiedName>,
    pub subsets: Vec<QualifiedName>,
    pub redefines: Vec<QualifiedName>,
    pub body_members: Vec<Declaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasDecl {
    pub name: String,
    pub target: QualifiedName,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Declaration {
    Package(PackageDecl),
    Import(ImportDecl),
    GenericDefinition(GenericDefinitionDecl),
    GenericUsage(GenericUsageDecl),
    Alias(AliasDecl),
}

impl Declaration {
    pub fn as_definition_like(&self) -> Option<GenericDefinitionDecl> {
        match self {
            Self::GenericDefinition(definition) => Some(definition.clone()),
            _ => None,
        }
    }

    pub fn as_usage_like(&self) -> Option<GenericUsageDecl> {
        match self {
            Self::GenericUsage(usage) => Some(usage.clone()),
            _ => None,
        }
    }

    pub fn child_declarations(&self) -> &[Declaration] {
        match self {
            Self::Package(package) => &package.members,
            Self::GenericDefinition(definition) => &definition.members,
            Self::GenericUsage(usage) => &usage.body_members,
            Self::Import(_) | Self::Alias(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDecl {
    pub name: QualifiedName,
    pub members: Vec<Declaration>,
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<GenericDefinitionDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentNote>,
    pub docs: Vec<String>,
    pub modifiers: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ParsedModule {
    pub package: Option<PackageDecl>,
    pub members: Vec<Declaration>,
    pub imports: Vec<ImportDecl>,
    pub definitions: Vec<GenericDefinitionDecl>,
}

impl PackageDecl {
    pub fn definition_like_declarations(&self) -> Vec<GenericDefinitionDecl> {
        let mut definitions = self
            .members
            .iter()
            .filter_map(Declaration::as_definition_like)
            .collect::<Vec<_>>();
        if definitions.is_empty() {
            definitions.extend(self.definitions.iter().cloned());
        }
        definitions
    }
}

impl ParsedModule {
    pub fn definition_like_declarations(&self) -> Vec<GenericDefinitionDecl> {
        let mut definitions = self
            .members
            .iter()
            .filter_map(Declaration::as_definition_like)
            .collect::<Vec<_>>();
        if definitions.is_empty() {
            definitions.extend(self.definitions.iter().cloned());
        }
        definitions
    }
}
