use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::codegen::language::concepts::{Concept, LanguageId};
use crate::kir::KIR_SCHEMA_VERSION;
use crate::workspace::paths::default_language_profile_path;

pub const CURRENT_DEFAULT_PROFILE_ID: &str = "foundation-core";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageProfile {
    pub id: String,
    pub language: LanguageId,
    pub language_version: String,
    pub metamodel_version: String,
    pub stdlib_version: String,
    pub stdlib_path: String,
    pub kir_schema_version: String,
    #[serde(default)]
    pub canonical_kinds: BTreeMap<Concept, String>,
    #[serde(default)]
    pub semantic_anchors: BTreeMap<String, String>,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum LanguageProfileError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidProfile(String),
}

impl fmt::Display for LanguageProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read language profile: {err}"),
            Self::Json(err) => write!(f, "failed to parse language profile: {err}"),
            Self::InvalidProfile(message) => write!(f, "invalid language profile: {message}"),
        }
    }
}

impl std::error::Error for LanguageProfileError {}

impl From<std::io::Error> for LanguageProfileError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for LanguageProfileError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl LanguageProfile {
    pub fn from_str(input: &str) -> Result<Self, LanguageProfileError> {
        let profile: Self = serde_json::from_str(input)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn from_path(path: &Path) -> Result<Self, LanguageProfileError> {
        Self::from_str(&std::fs::read_to_string(path)?)
    }

    pub fn validate(&self) -> Result<(), LanguageProfileError> {
        if self.id.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "profile id must not be empty".to_string(),
            ));
        }
        if self.language_version.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "language_version must not be empty".to_string(),
            ));
        }
        if self.metamodel_version.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "metamodel_version must not be empty".to_string(),
            ));
        }
        if self.stdlib_version.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "stdlib_version must not be empty".to_string(),
            ));
        }
        if self.stdlib_path.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "stdlib_path must not be empty".to_string(),
            ));
        }
        if self.kir_schema_version.trim().is_empty() {
            return Err(LanguageProfileError::InvalidProfile(
                "kir_schema_version must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn load_language_profile(profile_id: &str) -> Result<LanguageProfile, LanguageProfileError> {
    LanguageProfile::from_path(&default_language_profile_path(profile_id))
}

pub fn default_language_profile() -> Result<LanguageProfile, LanguageProfileError> {
    if let Ok(path) = std::env::var("MERCURIO_LANGUAGE_PROFILE_PATH") {
        return LanguageProfile::from_path(Path::new(&path));
    }

    Ok(core_language_profile())
}

fn core_language_profile() -> LanguageProfile {
    LanguageProfile {
        id: CURRENT_DEFAULT_PROFILE_ID.to_string(),
        language: LanguageId::from("model"),
        language_version: "core".to_string(),
        metamodel_version: "foundation".to_string(),
        stdlib_version: "none".to_string(),
        stdlib_path: "resources/foundation/empty.kir.json".to_string(),
        kir_schema_version: KIR_SCHEMA_VERSION.to_string(),
        canonical_kinds: BTreeMap::from([
            (Concept::PACKAGE, "model.Package".to_string()),
            (Concept::TYPE, "model.Type".to_string()),
        ]),
        semantic_anchors: BTreeMap::new(),
        aliases: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_loads_and_names_current_stdlib() {
        let profile = default_language_profile().unwrap();

        assert_eq!(profile.id, CURRENT_DEFAULT_PROFILE_ID);
        assert_eq!(profile.language.as_str(), "model");
        assert_eq!(profile.stdlib_version, "none");
        assert_eq!(profile.canonical_kinds[&Concept::PACKAGE], "model.Package");
        assert!(!profile.semantic_anchors.contains_key("requirement_usage"));
    }
}
