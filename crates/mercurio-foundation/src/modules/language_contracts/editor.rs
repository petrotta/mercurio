use std::fmt;

use serde::{Deserialize, Serialize};

use crate::language_contracts::ast::ParsedModule;
use crate::language_contracts::diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRange {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl TextRange {
    pub fn new(start_byte: usize, end_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }

    pub fn is_empty(self) -> bool {
        self.start_byte == self.end_byte
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: TextRange,
    pub replacement: String,
}

impl TextEdit {
    pub fn new(range: TextRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseSessionStatus {
    Ok,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseSnapshot {
    pub source_name: String,
    pub revision: u64,
    pub status: ParseSessionStatus,
    pub module: ParsedModule,
    pub diagnostics: Vec<Diagnostic>,
    pub changed_ranges: Vec<TextRange>,
    pub changed_declaration_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParseSessionError {
    RevisionMismatch {
        expected: u64,
        actual: u64,
    },
    InvalidEditRange {
        range: TextRange,
        text_len: usize,
    },
    NonBoundaryEdit {
        offset: usize,
    },
    OverlappingEditRanges {
        previous: TextRange,
        next: TextRange,
    },
}

impl fmt::Display for ParseSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RevisionMismatch { expected, actual } => write!(
                f,
                "parse session revision mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidEditRange { range, text_len } => write!(
                f,
                "invalid edit range {}..{} for text length {text_len}",
                range.start_byte, range.end_byte
            ),
            Self::NonBoundaryEdit { offset } => {
                write!(f, "edit offset {offset} is not a UTF-8 character boundary")
            }
            Self::OverlappingEditRanges { previous, next } => write!(
                f,
                "overlapping edit ranges {}..{} and {}..{}",
                previous.start_byte, previous.end_byte, next.start_byte, next.end_byte
            ),
        }
    }
}

impl std::error::Error for ParseSessionError {}
