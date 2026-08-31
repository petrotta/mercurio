use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackManifest {
    pub id: String,
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub requires: Option<MpackRequirements>,
    #[serde(default)]
    pub libraries: Vec<MpackLibrary>,
    #[serde(default, alias = "languageProfiles")]
    pub language_profiles: Vec<MpackLanguageProfile>,
    #[serde(default)]
    pub rulepacks: Vec<MpackRulepack>,
    #[serde(default, alias = "pythonPackages")]
    pub python_packages: Vec<MpackPythonPackage>,
    #[serde(default)]
    pub services: Vec<MpackService>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackRequirements {
    #[serde(default)]
    pub mercurio: Option<String>,
    #[serde(default)]
    pub kir: Option<String>,
    #[serde(default, alias = "pluginAbi")]
    pub plugin_abi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackLibrary {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub locator: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackLanguageProfile {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub stdlib: Option<String>,
    #[serde(default, alias = "pythonWrappers")]
    pub python_wrappers: Option<MpackPythonWrapperBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackPythonWrapperBinding {
    pub module: String,
    pub path: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackRulepack {
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackPythonPackage {
    pub module: String,
    pub path: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpackService {
    pub id: String,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub function: Option<String>,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpackValidationError {
    pub field: String,
    pub message: String,
}

impl fmt::Display for MpackValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for MpackValidationError {}

pub fn validate_mpack_manifest(manifest: &MpackManifest) -> Result<(), Vec<MpackValidationError>> {
    let mut errors = Vec::new();
    require_non_empty(&mut errors, "id", &manifest.id);
    require_non_empty(&mut errors, "version", &manifest.version);
    require_non_empty(&mut errors, "name", &manifest.name);

    for (index, library) in manifest.libraries.iter().enumerate() {
        if library.path.as_deref().is_none_or(str::is_empty)
            && library.locator.as_deref().is_none_or(str::is_empty)
        {
            errors.push(MpackValidationError {
                field: format!("libraries[{index}]"),
                message: "library must declare path or locator".to_string(),
            });
        }
    }

    for (index, profile) in manifest.language_profiles.iter().enumerate() {
        require_non_empty(
            &mut errors,
            &format!("languageProfiles[{index}].id"),
            &profile.id,
        );
        require_non_empty(
            &mut errors,
            &format!("languageProfiles[{index}].path"),
            &profile.path,
        );
        if let Some(binding) = &profile.python_wrappers {
            require_non_empty(
                &mut errors,
                &format!("languageProfiles[{index}].pythonWrappers.module"),
                &binding.module,
            );
            require_non_empty(
                &mut errors,
                &format!("languageProfiles[{index}].pythonWrappers.path"),
                &binding.path,
            );
        }
    }

    for (index, package) in manifest.python_packages.iter().enumerate() {
        require_non_empty(
            &mut errors,
            &format!("pythonPackages[{index}].module"),
            &package.module,
        );
        require_non_empty(
            &mut errors,
            &format!("pythonPackages[{index}].path"),
            &package.path,
        );
    }

    for (index, rulepack) in manifest.rulepacks.iter().enumerate() {
        require_non_empty(
            &mut errors,
            &format!("rulepacks[{index}].path"),
            &rulepack.path,
        );
    }

    for (index, service) in manifest.services.iter().enumerate() {
        require_non_empty(&mut errors, &format!("services[{index}].id"), &service.id);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn require_non_empty(errors: &mut Vec<MpackValidationError>, field: &str, value: &str) {
    if value.trim().is_empty() {
        errors.push(MpackValidationError {
            field: field.to_string(),
            message: "must not be empty".to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_stdlib_support_manifest_shape() {
        let manifest = MpackManifest {
            id: "org.mercurio.model-stdlib-support".to_string(),
            version: "2.0.0".to_string(),
            name: "Model Stdlib Support".to_string(),
            kind: Some("stdlib_support".to_string()),
            description: None,
            requires: None,
            libraries: vec![MpackLibrary {
                id: Some("org.omg/model-stdlib".to_string()),
                path: Some("libraries/model-stdlib.kpar".to_string()),
                locator: None,
                sha256: None,
                role: Some("baseline".to_string()),
            }],
            language_profiles: vec![MpackLanguageProfile {
                id: "model-2.0-pilot-0.57.0".to_string(),
                path: "profiles/model-2.0-pilot-0.57.0/profile.json".to_string(),
                stdlib: Some("libraries/model-stdlib.kpar".to_string()),
                python_wrappers: Some(MpackPythonWrapperBinding {
                    module: "mercurio_model_2_0".to_string(),
                    path: "python".to_string(),
                    entrypoint: Some("mercurio_model_2_0:register".to_string()),
                }),
            }],
            rulepacks: vec![MpackRulepack {
                path: "rules/stdlib.rulepack.json".to_string(),
                id: None,
            }],
            python_packages: Vec::new(),
            services: Vec::new(),
            metadata: BTreeMap::new(),
        };

        assert!(validate_mpack_manifest(&manifest).is_ok());
    }
}
