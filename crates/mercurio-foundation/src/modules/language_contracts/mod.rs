//! Contracts implemented by Mercurio source-language frontends.
//!
//! This crate exposes the shared AST, diagnostic, expression, report, and
//! service traits used by language plugins. Prefer the root-level re-exports
//! for common integration code.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};

pub mod ast;
pub mod diagnostics;
pub mod editor;
pub mod expression;
pub mod lexer;
pub mod project;
pub mod reports;
pub mod service;
pub mod workbench;

pub use ast::*;
pub use diagnostics::Diagnostic;
pub use editor::{ParseSessionError, ParseSessionStatus, ParseSnapshot, TextEdit, TextRange};
pub use expression::{
    BinaryExpressionOp, ExpressionEvaluationContext, ExpressionEvaluationError, ExpressionIr,
    ExpressionIrError, ExpressionPathRoot, ExpressionPathSegment, ExpressionValidationError,
    UnaryExpressionOp,
};
pub use lexer::{Token, TokenKind, lex};
pub use project::{ProjectSourceSelectionError, select_project_source_paths};
pub use reports::{ParseReport, SemanticCompileReport, SemanticCompileStatus};
pub use service::{CompileContext, LanguageRegistry, LanguageService};
pub use workbench::{
    DocumentSymbols, ElementAtPosition, LanguageAnalysis, ReferenceDescriptor, ScopeProvider,
    SourceDocument, SymbolDescriptor, analysis_from_compile_report, document_symbols,
    line_col_to_byte, source_span_to_text_range,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageId(String);

impl LanguageId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for LanguageId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for LanguageId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Concept(Cow<'static, str>);

impl Concept {
    pub const ATTRIBUTE_USAGE: Self = Self(Cow::Borrowed("attribute_usage"));
    pub const CONSTRAINT_USAGE: Self = Self(Cow::Borrowed("constraint_usage"));
    pub const FEATURE: Self = Self(Cow::Borrowed("feature"));
    pub const ITEM_DEFINITION: Self = Self(Cow::Borrowed("item_definition"));
    pub const ITEM_USAGE: Self = Self(Cow::Borrowed("item_usage"));
    pub const PACKAGE: Self = Self(Cow::Borrowed("package"));
    pub const PART_DEFINITION: Self = Self(Cow::Borrowed("part_definition"));
    pub const PART_USAGE: Self = Self(Cow::Borrowed("part_usage"));
    pub const REQUIREMENT_USAGE: Self = Self(Cow::Borrowed("requirement_usage"));
    pub const TYPE: Self = Self(Cow::Borrowed("type"));
    pub const VERIFICATION_CASE_USAGE: Self = Self(Cow::Borrowed("verification_case_usage"));
    pub const VIEW: Self = Self(Cow::Borrowed("view"));
    pub const VIEWPOINT: Self = Self(Cow::Borrowed("viewpoint"));

    pub fn new(value: impl Into<String>) -> Self {
        Self(Cow::Owned(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'de> Deserialize<'de> for Concept {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::new)
    }
}

impl From<&'static str> for Concept {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for Concept {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for Concept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[deprecated(note = "use Concept; concept keys are open strings")]
pub type SemanticConcept = Concept;

#[cfg(test)]
mod tests {
    use super::{Concept, LanguageId};

    #[test]
    fn concept_serializes_as_existing_snake_case_key() {
        let encoded = serde_json::to_string(&Concept::PART_DEFINITION).unwrap();
        assert_eq!(encoded, r#""part_definition""#);
        let decoded: Concept = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, Concept::PART_DEFINITION);
    }

    #[test]
    fn language_id_is_a_transparent_string() {
        let encoded = serde_json::to_string(&LanguageId::from("model")).unwrap();
        assert_eq!(encoded, r#""model""#);
    }
}
