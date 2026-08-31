use std::fmt;

use crate::kir::DiagnosticKind;
use serde::{Deserialize, Serialize};

use crate::language_contracts::ast::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    #[serde(default = "default_diagnostic_kind")]
    pub kind: DiagnosticKind,
    pub span: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            kind: DiagnosticKind::Syntax,
            span,
            subjects: Vec::new(),
        }
    }

    pub fn semantic(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self::new(message, span).with_kind(DiagnosticKind::Validation)
    }

    pub fn with_kind(mut self, kind: DiagnosticKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subjects.push(subject.into());
        self
    }
}

fn default_diagnostic_kind() -> DiagnosticKind {
    DiagnosticKind::Syntax
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.span {
            Some(span) => write!(
                f,
                "{} at {}:{}-{}:{}",
                self.message, span.start_line, span.start_col, span.end_line, span.end_col
            ),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for Diagnostic {}
