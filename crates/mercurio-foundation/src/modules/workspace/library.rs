use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::kir::{Diagnostic, KIR_SCHEMA_VERSION, KirDocument, KirError};
use crate::semantic_services::semantic_validation::{
    SemanticValidationReport, validate_kir_semantics, validate_kir_semantics_with_context,
};
use crate::workspace::paths::{
    bundled_package_repo_path, bundled_stdlib_package_set_path, default_model_library_path,
    default_package_kir_cache_path, default_package_repo_path, default_user_config_path,
};

pub const DEFAULT_STDLIB_LOCATOR: &str = "kpar:org.omg/model-stdlib:2.0.0";
pub const DEFAULT_MODEL_LIBRARY_LOCATOR: &str = DEFAULT_STDLIB_LOCATOR;
const DEFAULT_STDLIB_PACKAGE_SET_ENTRY: &str =
    "https://www.omg.org/spec/Model/20250201/Systems-Library.kpar";
const KPAR_PRECOMPILED_KIR_ENTRY: &str = "document.kir.json";
const MERCURIO_PACKAGE_MANIFEST_ENTRY: &str = "mercurio.package.json";
use crate::authoring::source_set::{SourceDocument, compile_source_documents};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineLibraryConfig {
    #[serde(default = "default_baseline_library_id")]
    pub id: String,
    #[serde(default)]
    pub provider: LibraryProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryProviderConfig {
    #[default]
    BundledStdlib,
    #[serde(alias = "local_kir_file")]
    PrecompiledKirArtifact {
        path: String,
    },
    ModelDirectory {
        path: String,
    },
    KparFile {
        path: String,
    },
    KparLocator {
        locator: String,
    },
    PackageSetDirectory {
        path: String,
        entry: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryCacheMetadata {
    pub source_kind: String,
    pub source_identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    pub importer_version: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedLibraryArtifact {
    pub library_id: String,
    pub source_kind: String,
    pub source_path: Option<PathBuf>,
    pub cache_metadata: Option<LibraryCacheMetadata>,
    pub validation_report: SemanticValidationReport,
    pub document: KirDocument,
}

#[derive(Debug, Clone)]
pub struct LibrarySourceFingerprint {
    pub library_id: String,
    pub source_kind: String,
    pub source_path: Option<PathBuf>,
    pub cache_metadata: LibraryCacheMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KparPackageBuild {
    pub name: String,
    pub version: Option<String>,
    pub sources: Vec<KparPackageSource>,
    pub precompiled_kir: Option<KirDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KparPackageSource {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPackageManifest {
    pub schema: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub file: String,
    pub digest: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<LocalPackageSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPackageRecord {
    pub repository_path: String,
    pub manifest_path: String,
    pub package_path: String,
    pub manifest: LocalPackageManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<PackageVerification>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioPackageManifest {
    pub schema: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<MercurioPackageDependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kir_schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MercurioPackageSourceProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<MercurioPackageBuildProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioPackageDependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub locator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioPackageSourceProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioPackageBuildProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioLockFile {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<MercurioLockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MercurioLockedPackage {
    pub name: String,
    pub version: String,
    pub locator: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageVerification {
    pub name: String,
    pub version: String,
    pub file: String,
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manifest: Option<MercurioPackageManifest>,
    pub project_name: Option<String>,
    pub project_version: Option<String>,
    pub source_count: usize,
    pub has_precompiled_kir: bool,
    pub precompiled_kir_element_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPackageSource {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPackageRepository {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UserPackageConfig {
    #[serde(default)]
    pub package_repositories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageKirCache {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageKirCacheManifest {
    pub schema: String,
    pub package: String,
    pub version: String,
    pub locator: String,
    pub source_digest: String,
    pub importer_version: String,
    pub context_digest: String,
    pub document: String,
    pub element_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KparLocator {
    raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReference {
    pub scheme: String,
    pub name: String,
    pub version: Option<String>,
    pub digest: Option<String>,
    pub raw: String,
}

impl Default for BaselineLibraryConfig {
    fn default() -> Self {
        Self::bundled_model_library()
    }
}

impl BaselineLibraryConfig {
    pub fn bundled_stdlib() -> Self {
        Self::bundled_model_library()
    }

    pub fn bundled_model_library() -> Self {
        Self {
            id: "stdlib".to_string(),
            provider: LibraryProviderConfig::BundledStdlib,
        }
    }

    pub fn stdlib_locator() -> Self {
        Self::model_library_locator()
    }

    pub fn model_library_locator() -> Self {
        Self {
            id: "stdlib".to_string(),
            provider: LibraryProviderConfig::KparLocator {
                locator: DEFAULT_MODEL_LIBRARY_LOCATOR.to_string(),
            },
        }
    }

    pub fn resolve(&self) -> Result<ResolvedLibraryArtifact, KirError> {
        self.provider.resolve(&self.id)
    }

    pub fn resolve_from(&self, base_dir: &Path) -> Result<ResolvedLibraryArtifact, KirError> {
        self.provider.resolve_from(&self.id, Some(base_dir))
    }
}

fn resolved_library_artifact(
    library_id: &str,
    source_kind: String,
    source_path: Option<PathBuf>,
    cache_metadata: Option<LibraryCacheMetadata>,
    document: KirDocument,
    library_context: Option<&KirDocument>,
) -> Result<ResolvedLibraryArtifact, KirError> {
    let validation_report = validate_kir_semantics_with_context(&document, library_context)?;
    Ok(ResolvedLibraryArtifact {
        library_id: library_id.to_string(),
        source_kind,
        source_path,
        cache_metadata,
        validation_report,
        document,
    })
}

impl LibraryProviderConfig {
    pub fn resolve(&self, library_id: &str) -> Result<ResolvedLibraryArtifact, KirError> {
        self.resolve_with_context(library_id, None, None)
    }

    pub fn resolve_from(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
    ) -> Result<ResolvedLibraryArtifact, KirError> {
        self.resolve_with_context(library_id, base_dir, None)
    }

    pub fn resolve_with_context(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
        library_context: Option<&KirDocument>,
    ) -> Result<ResolvedLibraryArtifact, KirError> {
        match self {
            Self::BundledStdlib => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(default_model_library_path);
                let document = KirDocument::from_path(&source_path)?;

                resolved_library_artifact(
                    library_id,
                    fingerprint.source_kind,
                    Some(source_path),
                    Some(fingerprint.cache_metadata),
                    document,
                    library_context,
                )
            }
            Self::PrecompiledKirArtifact { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let document = KirDocument::from_path(&source_path)?;

                resolved_library_artifact(
                    library_id,
                    fingerprint.source_kind,
                    Some(source_path),
                    Some(fingerprint.cache_metadata),
                    document,
                    library_context,
                )
            }
            Self::ModelDirectory { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_model_library_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let document = compile_model_directory(&source_path, context_document)?;

                resolved_library_artifact(
                    library_id,
                    fingerprint.source_kind,
                    Some(source_path.clone()),
                    Some(fingerprint.cache_metadata),
                    document,
                    Some(context_document),
                )
            }
            Self::KparFile { path } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_model_library_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let (document, package_metadata) =
                    compile_kpar_file(&source_path, context_document)?;

                resolved_library_artifact(
                    library_id,
                    fingerprint.source_kind,
                    Some(source_path.clone()),
                    Some(LibraryCacheMetadata {
                        source_version: package_metadata
                            .and_then(|metadata| metadata.version)
                            .or(fingerprint.cache_metadata.source_version.clone()),
                        ..fingerprint.cache_metadata
                    }),
                    document,
                    Some(context_document),
                )
            }
            Self::KparLocator { locator } => {
                let locator = KparLocator::parse(locator.clone());
                if let Some(path) = locator.as_str().strip_prefix("file:") {
                    return Self::KparFile {
                        path: path.to_string(),
                    }
                    .resolve_with_context(
                        library_id,
                        base_dir,
                        library_context,
                    );
                }

                let Some(package_ref) = locator.package_reference() else {
                    return Err(KirError::Model(format!(
                        "unsupported KPAR locator '{}'",
                        locator.as_str()
                    )));
                };
                let Some(version) = package_ref.version.as_deref() else {
                    return Err(KirError::Model(format!(
                        "KPAR locator '{}' must include a version for local repository resolution",
                        locator.as_str()
                    )));
                };
                let name = package_ref.name.as_str();

                for repo in LocalPackageRepository::resolution_repositories() {
                    if let Some(source_path) = repo.find_package(name, version)? {
                        let fallback_context =
                            KirDocument::from_path(&default_model_library_path())?;
                        let context_document = library_context.unwrap_or(&fallback_context);
                        let source_digest = digest_file(&source_path)?;
                        if let Some(expected_digest) = package_ref.digest.as_deref()
                            && source_digest != expected_digest
                        {
                            return Err(KirError::Model(format!(
                                "KPAR package digest mismatch for {}:{}: expected {}, got {}",
                                name, version, expected_digest, source_digest
                            )));
                        }
                        let (document, package_metadata) = PackageKirCache::default_user()
                            .load_or_compile(
                                name,
                                version,
                                locator.as_str(),
                                &source_path,
                                &source_digest,
                                context_document,
                            )?;
                        return resolved_library_artifact(
                            library_id,
                            "kpar_locator".to_string(),
                            Some(source_path.clone()),
                            Some(LibraryCacheMetadata {
                                source_kind: "kpar_locator".to_string(),
                                source_identity: locator.as_str().to_string(),
                                source_version: package_metadata
                                    .and_then(|metadata| metadata.version)
                                    .or_else(|| Some(version.to_string())),
                                source_digest: Some(source_digest),
                                importer_version: env!("CARGO_PKG_VERSION").to_string(),
                            }),
                            document,
                            Some(context_document),
                        );
                    }
                }

                if locator.as_str() == DEFAULT_STDLIB_LOCATOR {
                    if let Ok(artifact) =
                        resolve_bundled_stdlib_package_set(library_id, base_dir, library_context)
                    {
                        return Ok(artifact);
                    }
                    return Self::BundledStdlib.resolve_with_context(
                        library_id,
                        base_dir,
                        library_context,
                    );
                }

                Err(kpar_package_not_found_error(name, version))
            }
            Self::PackageSetDirectory { path, entry } => {
                let fingerprint = self.source_fingerprint(library_id, base_dir)?;
                let source_path = fingerprint
                    .source_path
                    .clone()
                    .unwrap_or_else(|| resolve_provider_path(path, base_dir));
                let fallback_context = KirDocument::from_path(&default_model_library_path())?;
                let context_document = library_context.unwrap_or(&fallback_context);
                let (document, package_metadata) =
                    compile_kpar_package_set(&source_path, entry, context_document)?;

                resolved_library_artifact(
                    library_id,
                    fingerprint.source_kind,
                    Some(source_path.clone()),
                    Some(LibraryCacheMetadata {
                        source_version: package_metadata.and_then(|metadata| metadata.version),
                        ..fingerprint.cache_metadata
                    }),
                    document,
                    Some(context_document),
                )
            }
        }
    }

    pub fn source_fingerprint(
        &self,
        library_id: &str,
        base_dir: Option<&Path>,
    ) -> Result<LibrarySourceFingerprint, KirError> {
        let importer_version = env!("CARGO_PKG_VERSION").to_string();
        match self {
            Self::BundledStdlib => {
                let source_path = default_model_library_path();
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "bundled_stdlib".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "bundled_stdlib".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::PrecompiledKirArtifact { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "precompiled_kir_artifact".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "precompiled_kir_artifact".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::ModelDirectory { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "model_directory".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "model_directory".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: None,
                        source_digest: Some(digest_model_directory(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::KparFile { path } => {
                let source_path = resolve_provider_path(path, base_dir);
                let (_, package_metadata) = collect_kpar_source_files(&source_path)?;
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "kpar_file".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "kpar_file".to_string(),
                        source_identity: source_path.display().to_string(),
                        source_version: package_metadata.and_then(|metadata| metadata.version),
                        source_digest: Some(digest_file(&source_path)?),
                        importer_version,
                    },
                })
            }
            Self::KparLocator { locator } => {
                let locator = KparLocator::parse(locator.clone());
                if let Some(path) = locator.as_str().strip_prefix("file:") {
                    return Self::KparFile {
                        path: path.to_string(),
                    }
                    .source_fingerprint(library_id, base_dir);
                }

                let Some(package_ref) = locator.package_reference() else {
                    return Err(KirError::Model(format!(
                        "unsupported KPAR locator '{}'",
                        locator.as_str()
                    )));
                };
                let Some(version) = package_ref.version.as_deref() else {
                    return Err(KirError::Model(format!(
                        "KPAR locator '{}' must include a version for local repository resolution",
                        locator.as_str()
                    )));
                };
                let name = package_ref.name.as_str();

                for repo in LocalPackageRepository::resolution_repositories() {
                    if let Some(source_path) = repo.find_package(name, version)? {
                        let source_digest = digest_file(&source_path)?;
                        if let Some(expected_digest) = package_ref.digest.as_deref()
                            && source_digest != expected_digest
                        {
                            return Err(KirError::Model(format!(
                                "KPAR package digest mismatch for {}:{}: expected {}, got {}",
                                name, version, expected_digest, source_digest
                            )));
                        }
                        return Ok(LibrarySourceFingerprint {
                            library_id: library_id.to_string(),
                            source_kind: "kpar_locator".to_string(),
                            source_path: Some(source_path.clone()),
                            cache_metadata: LibraryCacheMetadata {
                                source_kind: "kpar_locator".to_string(),
                                source_identity: locator.as_str().to_string(),
                                source_version: Some(version.to_string()),
                                source_digest: Some(source_digest),
                                importer_version,
                            },
                        });
                    }
                }

                if locator.as_str() == DEFAULT_STDLIB_LOCATOR {
                    if let Ok(fingerprint) =
                        bundled_stdlib_package_set_fingerprint(library_id, base_dir)
                    {
                        return Ok(fingerprint);
                    }
                    return Self::BundledStdlib.source_fingerprint(library_id, base_dir);
                }

                Err(kpar_package_not_found_error(name, version))
            }
            Self::PackageSetDirectory { path, entry } => {
                let source_path = resolve_provider_path(path, base_dir);
                let package_index = build_kpar_package_index(&source_path)?;
                let entry_key = package_index.resolve(entry, None).ok_or_else(|| {
                    KirError::Model(format!(
                        "package-set entry '{entry}' not found in {}",
                        source_path.display()
                    ))
                })?;
                let source_version = package_index
                    .packages
                    .get(&entry_key)
                    .and_then(|package| package.metadata.as_ref())
                    .and_then(|metadata| metadata.version.clone());
                Ok(LibrarySourceFingerprint {
                    library_id: library_id.to_string(),
                    source_kind: "package_set_directory".to_string(),
                    source_path: Some(source_path.clone()),
                    cache_metadata: LibraryCacheMetadata {
                        source_kind: "package_set_directory".to_string(),
                        source_identity: format!("{}#{}", source_path.display(), entry),
                        source_version,
                        source_digest: Some(digest_kpar_directory(&source_path)?),
                        importer_version,
                    },
                })
            }
        }
    }
}

fn resolve_provider_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_relative() {
        base_dir
            .map(|base_dir| base_dir.join(&candidate))
            .unwrap_or(candidate)
    } else {
        candidate
    }
}

fn resolve_bundled_stdlib_package_set(
    library_id: &str,
    base_dir: Option<&Path>,
    library_context: Option<&KirDocument>,
) -> Result<ResolvedLibraryArtifact, KirError> {
    LibraryProviderConfig::PackageSetDirectory {
        path: bundled_stdlib_package_set_path().display().to_string(),
        entry: DEFAULT_STDLIB_PACKAGE_SET_ENTRY.to_string(),
    }
    .resolve_with_context(library_id, base_dir, library_context)
}

fn bundled_stdlib_package_set_fingerprint(
    library_id: &str,
    base_dir: Option<&Path>,
) -> Result<LibrarySourceFingerprint, KirError> {
    LibraryProviderConfig::PackageSetDirectory {
        path: bundled_stdlib_package_set_path().display().to_string(),
        entry: DEFAULT_STDLIB_PACKAGE_SET_ENTRY.to_string(),
    }
    .source_fingerprint(library_id, base_dir)
}

fn configured_package_repository_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(value) = std::env::var("MERCURIO_PACKAGE_REPOSITORIES") {
        paths.extend(split_package_repository_list(&value));
    }

    let config_path = default_user_config_path();
    if config_path.is_file()
        && let Ok(input) = std::fs::read_to_string(&config_path)
        && let Ok(config) = serde_json::from_str::<UserPackageConfig>(&input)
    {
        paths.extend(
            config
                .package_repositories
                .into_iter()
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from),
        );
    }

    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.display().to_string()))
        .collect()
}

fn split_package_repository_list(value: &str) -> Vec<PathBuf> {
    value
        .split(';')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn kpar_package_not_found_error(name: &str, version: &str) -> KirError {
    let searched = LocalPackageRepository::resolution_repositories()
        .into_iter()
        .map(|repo| format!("- {}", repo.root().display()))
        .collect::<Vec<_>>()
        .join("\n");
    KirError::Model(format!(
        "KPAR package '{name}' version '{version}' was not found in configured package repositories\nsearched:\n{searched}"
    ))
}

pub fn load_baseline_library_document() -> Result<KirDocument, KirError> {
    Ok(BaselineLibraryConfig::bundled_stdlib().resolve()?.document)
}

impl LocalPackageRepository {
    pub fn default_user() -> Self {
        Self::new(default_package_repo_path())
    }

    pub fn bundled() -> Self {
        Self::new(bundled_package_repo_path())
    }

    pub fn configured() -> Vec<Self> {
        configured_package_repository_paths()
            .into_iter()
            .map(Self::new)
            .collect()
    }

    pub fn resolution_repositories() -> Vec<Self> {
        let mut repositories = Vec::new();
        repositories.push(Self::default_user());
        repositories.extend(Self::configured());
        repositories.push(Self::bundled());
        repositories
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn package_dir(&self, name: &str, version: &str) -> PathBuf {
        let mut path = self.root.clone();
        for segment in name.split('/') {
            path.push(safe_package_path_segment(segment));
        }
        path.push(safe_package_path_segment(version));
        path
    }

    pub fn package_file_name(name: &str, version: &str) -> String {
        let leaf_name = name.rsplit('/').next().unwrap_or(name);
        format!(
            "{}-{}.kpar",
            safe_package_path_segment(leaf_name),
            safe_package_path_segment(version)
        )
    }

    pub fn package_path(&self, name: &str, version: &str) -> PathBuf {
        self.package_dir(name, version)
            .join(Self::package_file_name(name, version))
    }

    pub fn manifest_path(&self, name: &str, version: &str) -> PathBuf {
        self.package_dir(name, version).join("manifest.json")
    }

    pub fn find_package(&self, name: &str, version: &str) -> Result<Option<PathBuf>, KirError> {
        let manifest_path = self.manifest_path(name, version);
        let package_path = self.package_path(name, version);
        if !manifest_path.is_file() && !package_path.is_file() {
            return Ok(None);
        }
        if manifest_path.is_file() {
            let manifest = self.read_manifest(name, version)?;
            let resolved_path = self.package_dir(name, version).join(&manifest.file);
            if !resolved_path.is_file() {
                return Ok(None);
            }
            let digest = digest_file(&resolved_path)?;
            if digest != manifest.digest {
                return Err(KirError::Model(format!(
                    "local package digest mismatch for {name}:{version}: expected {}, got {}",
                    manifest.digest, digest
                )));
            }
            return Ok(Some(resolved_path));
        }
        Ok(package_path.is_file().then_some(package_path))
    }

    pub fn read_manifest(
        &self,
        name: &str,
        version: &str,
    ) -> Result<LocalPackageManifest, KirError> {
        let manifest_path = self.manifest_path(name, version);
        let input = std::fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&input).map_err(KirError::Json)
    }

    pub fn list_packages(&self) -> Result<Vec<LocalPackageRecord>, KirError> {
        let mut records = Vec::new();
        if !self.root.is_dir() {
            return Ok(records);
        }

        for manifest_path in collect_local_manifest_paths(&self.root)? {
            let input = std::fs::read_to_string(&manifest_path)?;
            let manifest: LocalPackageManifest = serde_json::from_str(&input)?;
            let package_path = manifest_path
                .parent()
                .unwrap_or(self.root.as_path())
                .join(&manifest.file);
            let verification = self.verify_package(&manifest.name, &manifest.version).ok();
            records.push(LocalPackageRecord {
                repository_path: self.root.to_string_lossy().to_string(),
                manifest_path: manifest_path.to_string_lossy().to_string(),
                package_path: package_path.to_string_lossy().to_string(),
                manifest,
                verification,
            });
        }

        records.sort_by(|left, right| {
            left.manifest
                .name
                .cmp(&right.manifest.name)
                .then_with(|| left.manifest.version.cmp(&right.manifest.version))
                .then_with(|| left.repository_path.cmp(&right.repository_path))
        });
        Ok(records)
    }

    pub fn verify_package(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PackageVerification, KirError> {
        let manifest = self.read_manifest(name, version)?;
        if manifest.name != name || manifest.version != version {
            return Err(KirError::Model(format!(
                "package manifest identity mismatch: expected {name}:{version}, got {}:{}",
                manifest.name, manifest.version
            )));
        }
        if manifest.kind != "kpar" {
            return Err(KirError::Model(format!(
                "unsupported package kind '{}'",
                manifest.kind
            )));
        }
        let Some(package_path) = self.find_package(name, version)? else {
            return Err(KirError::Model(format!(
                "package {name} version {version} was not found in {}",
                self.root.display()
            )));
        };
        let archive = verify_kpar_archive(&package_path)?;
        if let Some(project_name) = &archive.project_name
            && project_name != name
        {
            return Err(KirError::Model(format!(
                "package project name mismatch: manifest has {name}, archive has {project_name}"
            )));
        }
        if let Some(project_version) = &archive.project_version
            && project_version != version
        {
            return Err(KirError::Model(format!(
                "package project version mismatch: manifest has {version}, archive has {project_version}"
            )));
        }
        Ok(PackageVerification {
            name: manifest.name,
            version: manifest.version,
            file: manifest.file,
            digest: manifest.digest,
            package_manifest: archive.package_manifest,
            project_name: archive.project_name,
            project_version: archive.project_version,
            source_count: archive.source_count,
            has_precompiled_kir: archive.has_precompiled_kir,
            precompiled_kir_element_count: archive.precompiled_kir_element_count,
            validation_diagnostics: archive.validation_diagnostics,
        })
    }

    pub fn stage_kpar(
        &self,
        source_path: &Path,
        name: &str,
        version: &str,
        source: Option<LocalPackageSource>,
    ) -> Result<LocalPackageManifest, KirError> {
        let package_dir = self.package_dir(name, version);
        std::fs::create_dir_all(&package_dir)?;
        let file = Self::package_file_name(name, version);
        let package_path = package_dir.join(&file);
        std::fs::copy(source_path, &package_path)?;
        let manifest = LocalPackageManifest {
            schema: "dev.mercurio.local-package.v1".to_string(),
            name: name.to_string(),
            version: version.to_string(),
            kind: "kpar".to_string(),
            file,
            digest: digest_file(&package_path)?,
            created_at: package_created_at(),
            source,
        };
        std::fs::write(
            package_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;
        Ok(manifest)
    }

    pub fn publish_to_repository(
        &self,
        target: &LocalPackageRepository,
        name: &str,
        version: &str,
        force: bool,
    ) -> Result<LocalPackageManifest, KirError> {
        self.copy_package_to(target, name, version, force)
    }

    pub fn pull_from_repository(
        &self,
        source: &LocalPackageRepository,
        name: &str,
        version: &str,
        force: bool,
    ) -> Result<LocalPackageManifest, KirError> {
        source.copy_package_to(self, name, version, force)
    }

    fn copy_package_to(
        &self,
        target: &LocalPackageRepository,
        name: &str,
        version: &str,
        force: bool,
    ) -> Result<LocalPackageManifest, KirError> {
        let Some(source_package_path) = self.find_package(name, version)? else {
            return Err(KirError::Model(format!(
                "package {name} version {version} was not found in {}",
                self.root.display()
            )));
        };
        let manifest = self.read_manifest(name, version)?;
        let target_dir = target.package_dir(name, version);
        let target_manifest_path = target.manifest_path(name, version);
        let target_package_path = target_dir.join(&manifest.file);
        if !force && (target_manifest_path.exists() || target_package_path.exists()) {
            return Err(KirError::Model(format!(
                "package {name} version {version} already exists in {}; use --force to overwrite",
                target.root.display()
            )));
        }
        std::fs::create_dir_all(&target_dir)?;
        std::fs::copy(&source_package_path, &target_package_path)?;
        std::fs::write(
            target_manifest_path,
            serde_json::to_string_pretty(&manifest)?,
        )?;
        Ok(manifest)
    }
}

fn collect_local_manifest_paths(root: &Path) -> Result<Vec<PathBuf>, KirError> {
    let mut manifests = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(entry_path);
            } else if entry.file_name().to_string_lossy() == "manifest.json" {
                manifests.push(entry_path);
            }
        }
    }

    manifests.sort();
    Ok(manifests)
}

impl PackageKirCache {
    pub fn default_user() -> Self {
        Self::new(default_package_kir_cache_path())
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn cache_dir(
        &self,
        package: &str,
        version: &str,
        source_digest: &str,
        context_digest: &str,
    ) -> PathBuf {
        let mut path = self.root.clone();
        for segment in package.split('/') {
            path.push(safe_package_path_segment(segment));
        }
        path.push(safe_package_path_segment(version));
        path.push(safe_package_path_segment(source_digest));
        path.push(safe_package_path_segment(context_digest));
        path
    }

    pub fn document_path(
        &self,
        package: &str,
        version: &str,
        source_digest: &str,
        context_digest: &str,
    ) -> PathBuf {
        self.cache_dir(package, version, source_digest, context_digest)
            .join("document.kir.json")
    }

    pub fn manifest_path(
        &self,
        package: &str,
        version: &str,
        source_digest: &str,
        context_digest: &str,
    ) -> PathBuf {
        self.cache_dir(package, version, source_digest, context_digest)
            .join("manifest.json")
    }

    fn load_or_compile(
        &self,
        package: &str,
        version: &str,
        locator: &str,
        source_path: &Path,
        source_digest: &str,
        library_context: &KirDocument,
    ) -> Result<(KirDocument, Option<KparProjectMetadata>), KirError> {
        let importer_version = env!("CARGO_PKG_VERSION").to_string();
        let context_digest = digest_kir_document(library_context)?;
        if let Some(document) = self.load_cached_document(
            package,
            version,
            locator,
            source_digest,
            &importer_version,
            &context_digest,
        )? {
            let (_, package_metadata) = collect_kpar_source_files(source_path)?;
            return Ok((document, package_metadata));
        }

        let (document, package_metadata) = compile_kpar_file(source_path, library_context)?;
        self.store_document(
            package,
            version,
            locator,
            source_digest,
            &importer_version,
            &context_digest,
            &document,
        )?;
        Ok((document, package_metadata))
    }

    fn load_cached_document(
        &self,
        package: &str,
        version: &str,
        locator: &str,
        source_digest: &str,
        importer_version: &str,
        context_digest: &str,
    ) -> Result<Option<KirDocument>, KirError> {
        let document_path = self.document_path(package, version, source_digest, context_digest);
        let manifest_path = self.manifest_path(package, version, source_digest, context_digest);
        if !document_path.is_file() || !manifest_path.is_file() {
            return Ok(None);
        }

        let manifest = match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|input| serde_json::from_str::<PackageKirCacheManifest>(&input).ok())
        {
            Some(manifest) => manifest,
            None => return Ok(None),
        };
        if manifest.package != package
            || manifest.version != version
            || manifest.locator != locator
            || manifest.source_digest != source_digest
            || manifest.importer_version != importer_version
            || manifest.context_digest != context_digest
            || manifest.document != "document.kir.json"
        {
            return Ok(None);
        }

        Ok(KirDocument::from_path(&document_path).ok())
    }

    fn store_document(
        &self,
        package: &str,
        version: &str,
        locator: &str,
        source_digest: &str,
        importer_version: &str,
        context_digest: &str,
        document: &KirDocument,
    ) -> Result<(), KirError> {
        let document_path = self.document_path(package, version, source_digest, context_digest);
        let manifest_path = self.manifest_path(package, version, source_digest, context_digest);
        document.write_pretty_to_path(&document_path)?;
        let manifest = PackageKirCacheManifest {
            schema: "dev.mercurio.package-kir-cache.v1".to_string(),
            package: package.to_string(),
            version: version.to_string(),
            locator: locator.to_string(),
            source_digest: source_digest.to_string(),
            importer_version: importer_version.to_string(),
            context_digest: context_digest.to_string(),
            document: "document.kir.json".to_string(),
            element_count: document.elements.len(),
        };
        std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        Ok(())
    }
}

impl KparLocator {
    pub fn parse(locator: impl Into<String>) -> Self {
        Self {
            raw: locator.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn package_reference(&self) -> Option<PackageReference> {
        parse_package_reference(&self.raw)
    }
}

pub fn parse_package_reference(locator: &str) -> Option<PackageReference> {
    let (scheme, remainder) = if let Some(value) = locator.strip_prefix("kpar:") {
        ("kpar", value)
    } else if let Some(value) = locator.strip_prefix("oci://") {
        ("oci", value)
    } else {
        return None;
    };

    let (coordinate, digest) = match remainder.split_once('@') {
        Some((coordinate, digest)) => {
            if !is_sha256_digest(digest) {
                return None;
            }
            (coordinate, Some(digest.to_string()))
        }
        None => (remainder, None),
    };
    let (name, version) = coordinate
        .rsplit_once(':')
        .map(|(name, version)| (name, Some(version.to_string())))
        .unwrap_or((coordinate, None));
    if name.trim().is_empty()
        || version
            .as_deref()
            .is_some_and(|version| version.trim().is_empty())
    {
        return None;
    }

    Some(PackageReference {
        scheme: scheme.to_string(),
        name: name.to_string(),
        version,
        digest,
        raw: locator.to_string(),
    })
}

pub fn write_kpar_package(path: &Path, package: &KparPackageBuild) -> Result<(), KirError> {
    if package.name.trim().is_empty() {
        return Err(KirError::Model(
            "package name must not be empty".to_string(),
        ));
    }
    if package.sources.is_empty() && package.precompiled_kir.is_none() {
        return Err(KirError::Model(
            "package must contain at least one source file or precompiled KIR document".to_string(),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut seen_paths = BTreeSet::new();
    let mut sources = package.sources.clone();
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    for source in &sources {
        validate_kpar_source_path(&source.path)?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(KirError::Model(format!(
                "duplicate package source path: {}",
                source.path
            )));
        }
    }

    let file = std::fs::File::create(path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();

    writer
        .start_file(".project.json", options)
        .map_err(zip_error_to_kir_error)?;
    let mut project = serde_json::Map::new();
    project.insert(
        "name".to_string(),
        serde_json::Value::String(package.name.clone()),
    );
    if let Some(version) = &package.version {
        project.insert(
            "version".to_string(),
            serde_json::Value::String(version.clone()),
        );
    }
    project.insert("usage".to_string(), serde_json::Value::Array(Vec::new()));
    writer.write_all(serde_json::Value::Object(project).to_string().as_bytes())?;

    writer
        .start_file(".meta.json", options)
        .map_err(zip_error_to_kir_error)?;
    writer.write_all(br#"{"files":[]}"#)?;

    writer
        .start_file(MERCURIO_PACKAGE_MANIFEST_ENTRY, options)
        .map_err(zip_error_to_kir_error)?;
    let manifest = MercurioPackageManifest {
        schema: "dev.mercurio.package.v1".to_string(),
        name: package.name.clone(),
        version: package
            .version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string()),
        kind: "kpar".to_string(),
        dependencies: Vec::new(),
        kir_schema_version: Some(KIR_SCHEMA_VERSION.to_string()),
        source: None,
        build: None,
    };
    writer.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    if let Some(document) = &package.precompiled_kir {
        writer
            .start_file(KPAR_PRECOMPILED_KIR_ENTRY, options)
            .map_err(zip_error_to_kir_error)?;
        writer.write_all(serde_json::to_string_pretty(document)?.as_bytes())?;
    }

    for source in &sources {
        writer
            .start_file(&source.path, options)
            .map_err(zip_error_to_kir_error)?;
        writer.write_all(source.content.as_bytes())?;
    }

    writer.finish().map_err(zip_error_to_kir_error)?;
    Ok(())
}

fn compile_model_directory(
    path: &Path,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    let source_files = collect_model_directory_source_files(path)?;
    compile_library_source_files(source_files, library_context)
}

fn compile_kpar_file(
    path: &Path,
    library_context: &KirDocument,
) -> Result<(KirDocument, Option<KparProjectMetadata>), KirError> {
    let KparArchiveContent {
        source_files,
        package_metadata,
        package_manifest: _,
        precompiled_kir,
    } = collect_kpar_archive_content(path)?;
    if let Some(document) = precompiled_kir {
        return Ok((document, package_metadata));
    }
    let document = compile_library_source_files(source_files, library_context)?;
    Ok((document, package_metadata))
}

fn compile_kpar_package_set(
    path: &Path,
    entry: &str,
    library_context: &KirDocument,
) -> Result<(KirDocument, Option<KparProjectMetadata>), KirError> {
    let package_index = build_kpar_package_index(path)?;
    let entry_key = package_index.resolve(entry, None).ok_or_else(|| {
        KirError::Model(format!(
            "package-set entry '{entry}' not found in {}",
            path.display()
        ))
    })?;
    let mut visit_stack = Vec::new();
    let mut ordered_keys = Vec::new();
    let mut visited = BTreeSet::new();
    collect_package_order(
        &package_index,
        &entry_key,
        &mut visit_stack,
        &mut visited,
        &mut ordered_keys,
    )?;

    let mut merged_context = library_context.clone();
    let mut package_documents = Vec::new();
    let mut root_metadata = None;

    for package_key in ordered_keys {
        let package = package_index.packages.get(&package_key).ok_or_else(|| {
            KirError::Model(format!(
                "indexed package '{package_key}' missing from package set"
            ))
        })?;
        let (document, metadata) = compile_kpar_file(&package.path, &merged_context)?;
        if package_key == entry_key {
            root_metadata = metadata.clone();
        }
        merged_context = KirDocument::merge([merged_context, document.clone()])?;
        package_documents.push(document);
    }

    Ok((KirDocument::merge(package_documents)?, root_metadata))
}

fn compile_library_source_files(
    source_files: Vec<SourceDocument>,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    compile_source_documents(source_files, library_context)
}

fn collect_model_directory_source_files(path: &Path) -> Result<Vec<SourceDocument>, KirError> {
    let mut files = Vec::new();
    collect_model_directory_source_files_recursive(path, path, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_model_directory_source_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<SourceDocument>,
) -> Result<(), KirError> {
    let mut entries = std::fs::read_dir(current)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_model_directory_source_files_recursive(root, &path, files)?;
            continue;
        }

        if !is_library_source_file(&path) {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let source_name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push(SourceDocument::new(source_name, content));
    }

    Ok(())
}

fn digest_file(path: &Path) -> Result<String, KirError> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(package_bytes_digest(&bytes))
}

pub fn package_bytes_digest(bytes: &[u8]) -> String {
    format_sha256_digest(bytes)
}

fn digest_kir_document(document: &KirDocument) -> Result<String, KirError> {
    let bytes = serde_json::to_vec(document)?;
    Ok(format_stable_digest([(
        "kir-document".as_bytes(),
        bytes.as_slice(),
    )]))
}

fn safe_package_path_segment(value: &str) -> String {
    let mut segment = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if segment.is_empty() || segment == "." || segment == ".." {
        segment = "package".to_string();
    }
    segment
}

fn package_created_at() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

fn digest_model_directory(path: &Path) -> Result<String, KirError> {
    let source_files = collect_model_directory_source_files(path)?;
    Ok(format_stable_digest(source_files.iter().flat_map(|file| {
        [
            ("path".as_bytes(), file.path.as_bytes()),
            ("content".as_bytes(), file.content.as_bytes()),
        ]
    })))
}

fn digest_kpar_directory(path: &Path) -> Result<String, KirError> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut items = Vec::new();

    for entry in entries {
        let package_path = entry.path();
        if package_path.extension().and_then(|value| value.to_str()) != Some("kpar") {
            continue;
        }

        let package_name = package_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let mut file = std::fs::File::open(&package_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        items.push((package_name, bytes));
    }

    Ok(format_stable_digest(items.iter().flat_map(
        |(name, bytes)| {
            [
                ("path".as_bytes(), name.as_bytes()),
                ("content".as_bytes(), bytes.as_slice()),
            ]
        },
    )))
}

fn format_stable_digest<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = (&'a [u8], &'a [u8])>,
{
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for (label, bytes) in chunks {
        for byte in label
            .iter()
            .chain(&(bytes.len() as u64).to_le_bytes())
            .chain(bytes)
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    format!("fnv1a64:{hash:016x}")
}

fn format_sha256_digest(bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    let mut output = String::from("sha256:");
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() + 8) % 64 != 0 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut hash = H0;
    for chunk in message.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, bytes) in chunk.chunks_exact(4).enumerate().take(16) {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut a = hash[0];
        let mut b = hash[1];
        let mut c = hash[2];
        let mut d = hash[3];
        let mut e = hash[4];
        let mut f = hash[5];
        let mut g = hash[6];
        let mut h = hash[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        hash[0] = hash[0].wrapping_add(a);
        hash[1] = hash[1].wrapping_add(b);
        hash[2] = hash[2].wrapping_add(c);
        hash[3] = hash[3].wrapping_add(d);
        hash[4] = hash[4].wrapping_add(e);
        hash[5] = hash[5].wrapping_add(f);
        hash[6] = hash[6].wrapping_add(g);
        hash[7] = hash[7].wrapping_add(h);
    }

    let mut output = [0u8; 32];
    for (index, word) in hash.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

struct KparArchiveContent {
    source_files: Vec<SourceDocument>,
    package_metadata: Option<KparProjectMetadata>,
    package_manifest: Option<MercurioPackageManifest>,
    precompiled_kir: Option<KirDocument>,
}

fn collect_kpar_archive_content(path: &Path) -> Result<KparArchiveContent, KirError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_kir_error)?;
    let mut files = Vec::new();
    let mut package_metadata = None;
    let mut package_manifest = None;
    let mut precompiled_kir = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error_to_kir_error)?;
        if !entry.is_file() {
            continue;
        }

        let entry_name = entry.name().replace('\\', "/");
        if entry_name == ".project.json" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            package_metadata = serde_json::from_str(&content).ok();
            continue;
        }

        if entry_name == MERCURIO_PACKAGE_MANIFEST_ENTRY {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            package_manifest = Some(serde_json::from_str(&content)?);
            continue;
        }

        if entry_name == KPAR_PRECOMPILED_KIR_ENTRY {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            precompiled_kir = Some(KirDocument::from_str(&content)?);
            continue;
        }

        if !is_library_archive_source_entry(&entry_name) {
            continue;
        }

        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        files.push(SourceDocument::new(entry_name, content));
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(KparArchiveContent {
        source_files: files,
        package_metadata,
        package_manifest,
        precompiled_kir,
    })
}

fn collect_kpar_source_files(
    path: &Path,
) -> Result<(Vec<SourceDocument>, Option<KparProjectMetadata>), KirError> {
    let content = collect_kpar_archive_content(path)?;
    let _package_manifest = content.package_manifest;
    Ok((content.source_files, content.package_metadata))
}

struct KparArchiveVerification {
    project_name: Option<String>,
    project_version: Option<String>,
    package_manifest: Option<MercurioPackageManifest>,
    source_count: usize,
    has_precompiled_kir: bool,
    precompiled_kir_element_count: Option<usize>,
    validation_diagnostics: Vec<Diagnostic>,
}

fn verify_kpar_archive(path: &Path) -> Result<KparArchiveVerification, KirError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_kir_error)?;
    let mut package_metadata = None;
    let mut package_manifest = None;
    let mut source_count = 0usize;
    let mut precompiled_kir_element_count = None;
    let mut validation_diagnostics = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error_to_kir_error)?;
        if !entry.is_file() {
            continue;
        }

        let entry_name = entry.name().replace('\\', "/");
        if entry_name == ".project.json" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            package_metadata = Some(serde_json::from_str::<KparProjectMetadata>(&content)?);
            continue;
        }

        if entry_name == MERCURIO_PACKAGE_MANIFEST_ENTRY {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            package_manifest = Some(serde_json::from_str::<MercurioPackageManifest>(&content)?);
            continue;
        }

        if entry_name == KPAR_PRECOMPILED_KIR_ENTRY {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            let document = KirDocument::from_str(&content)?;
            precompiled_kir_element_count = Some(document.elements.len());
            validation_diagnostics.extend(validate_kir_semantics(&document)?.diagnostics);
            continue;
        }

        if is_library_archive_source_entry(&entry_name) {
            source_count += 1;
        }
    }

    let Some(package_metadata) = package_metadata else {
        return Err(KirError::Model(format!(
            "KPAR package {} is missing .project.json",
            path.display()
        )));
    };
    if source_count == 0 && precompiled_kir_element_count.is_none() {
        return Err(KirError::Model(format!(
            "KPAR package {} contains no source files or precompiled KIR document",
            path.display()
        )));
    }

    Ok(KparArchiveVerification {
        project_name: package_metadata.name,
        project_version: package_metadata.version,
        package_manifest,
        source_count,
        has_precompiled_kir: precompiled_kir_element_count.is_some(),
        precompiled_kir_element_count,
        validation_diagnostics,
    })
}

fn build_kpar_package_index(path: &Path) -> Result<KparPackageIndex, KirError> {
    let mut packages = BTreeMap::new();
    let mut aliases = BTreeMap::<String, Vec<String>>::new();
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let package_path = entry.path();
        if package_path.extension().and_then(|value| value.to_str()) != Some("kpar") {
            continue;
        }

        let (_, metadata) = collect_kpar_source_files(&package_path)?;
        let package_key = package_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                KirError::Model(format!(
                    "failed to derive package file name from {}",
                    package_path.display()
                ))
            })?
            .to_string();
        let package = IndexedKparPackage {
            key: package_key.clone(),
            path: package_path,
            metadata,
        };

        for alias in package.aliases() {
            aliases.entry(alias).or_default().push(package_key.clone());
        }

        packages.insert(package_key, package);
    }

    Ok(KparPackageIndex { packages, aliases })
}

fn collect_package_order(
    index: &KparPackageIndex,
    package_key: &str,
    visit_stack: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
    ordered_keys: &mut Vec<String>,
) -> Result<(), KirError> {
    if visited.contains(package_key) {
        return Ok(());
    }
    if visit_stack.iter().any(|entry| entry == package_key) {
        let cycle = visit_stack
            .iter()
            .cloned()
            .chain(std::iter::once(package_key.to_string()))
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(KirError::Model(format!(
            "cyclic package dependency detected: {cycle}"
        )));
    }

    let package = index.packages.get(package_key).ok_or_else(|| {
        KirError::Model(format!(
            "package '{package_key}' missing from package index"
        ))
    })?;
    visit_stack.push(package_key.to_string());

    if let Some(metadata) = &package.metadata {
        for dependency in &metadata.usage {
            let dependency_key = index
                .resolve(
                    &dependency.resource,
                    dependency.version_constraint.as_deref(),
                )
                .ok_or_else(|| {
                    KirError::Model(format!(
                        "failed to resolve package dependency '{}'{} in package '{}'",
                        dependency.resource,
                        dependency
                            .version_constraint
                            .as_deref()
                            .map(|version| format!(" @ {version}"))
                            .unwrap_or_default(),
                        package_key
                    ))
                })?;
            collect_package_order(index, &dependency_key, visit_stack, visited, ordered_keys)?;
        }
    }

    visit_stack.pop();
    visited.insert(package_key.to_string());
    ordered_keys.push(package_key.to_string());
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct KparProjectMetadata {
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    usage: Vec<KparDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct KparDependency {
    resource: String,
    #[serde(default, rename = "versionConstraint")]
    version_constraint: Option<String>,
}

#[derive(Debug, Clone)]
struct IndexedKparPackage {
    key: String,
    path: PathBuf,
    metadata: Option<KparProjectMetadata>,
}

impl IndexedKparPackage {
    fn aliases(&self) -> Vec<String> {
        let mut aliases = BTreeSet::new();
        aliases.insert(normalize_package_alias(&self.key));

        if let Some(stem) = Path::new(&self.key)
            .file_stem()
            .and_then(|value| value.to_str())
        {
            aliases.insert(normalize_package_alias(stem));
            aliases.insert(normalize_package_alias(&strip_version_suffix(stem)));
        }

        if let Some(metadata) = &self.metadata {
            if let Some(name) = metadata.name.as_deref() {
                aliases.insert(normalize_package_alias(name));
                aliases.insert(normalize_package_alias(&strip_metadata_prefix(name)));
                aliases.insert(format!(
                    "{}.kpar",
                    normalize_package_alias(&strip_metadata_prefix(name))
                ));
            }
        }

        aliases
            .into_iter()
            .filter(|alias| !alias.is_empty())
            .collect()
    }
}

#[derive(Debug, Clone)]
struct KparPackageIndex {
    packages: BTreeMap<String, IndexedKparPackage>,
    aliases: BTreeMap<String, Vec<String>>,
}

impl KparPackageIndex {
    fn resolve(&self, reference: &str, version: Option<&str>) -> Option<String> {
        let reference_aliases = package_reference_aliases(reference);
        let mut matches = reference_aliases
            .into_iter()
            .filter_map(|alias| self.aliases.get(&alias))
            .flat_map(|entries| entries.iter().cloned())
            .collect::<BTreeSet<_>>();

        if let Some(version) = version {
            matches.retain(|package_key| {
                self.packages
                    .get(package_key)
                    .and_then(|package| package.metadata.as_ref())
                    .and_then(|metadata| metadata.version.as_deref())
                    == Some(version)
            });
        }

        if matches.len() == 1 {
            matches.into_iter().next()
        } else {
            None
        }
    }
}

fn default_baseline_library_id() -> String {
    "stdlib".to_string()
}

fn strip_version_suffix(value: &str) -> String {
    match value.rsplit_once('-') {
        Some((prefix, suffix))
            if suffix
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.') =>
        {
            prefix.to_string()
        }
        _ => value.to_string(),
    }
}

fn strip_metadata_prefix(value: &str) -> String {
    value
        .trim()
        .strip_prefix("Kernel ")
        .or_else(|| value.trim().strip_prefix("Model "))
        .unwrap_or(value.trim())
        .to_string()
}

fn normalize_package_alias(value: &str) -> String {
    value
        .trim()
        .replace('_', "-")
        .replace(' ', "-")
        .to_ascii_lowercase()
}

fn package_reference_aliases(reference: &str) -> Vec<String> {
    let mut aliases = BTreeSet::new();
    let trimmed = reference.trim();
    aliases.insert(normalize_package_alias(trimmed));

    if let Some(file_name) = trimmed.rsplit('/').next() {
        aliases.insert(normalize_package_alias(file_name));
        if let Some(stem) = Path::new(file_name)
            .file_stem()
            .and_then(|value| value.to_str())
        {
            aliases.insert(normalize_package_alias(stem));
            aliases.insert(normalize_package_alias(&strip_version_suffix(stem)));
        }
    }

    aliases
        .into_iter()
        .filter(|alias| !alias.is_empty())
        .collect()
}

fn validate_kpar_source_path(path: &str) -> Result<(), KirError> {
    let normalized = path.replace('\\', "/");
    if normalized.trim().is_empty() {
        return Err(KirError::Model(
            "package source path must not be empty".to_string(),
        ));
    }
    if normalized.starts_with('/') || normalized.contains("/../") || normalized.starts_with("../") {
        return Err(KirError::Model(format!(
            "package source path must be relative and stay inside the package: {path}"
        )));
    }
    if !is_library_archive_source_entry(&normalized) {
        return Err(KirError::Model(format!(
            "package source path must end in .model, .core, .sysml, or .kerml: {path}"
        )));
    }
    Ok(())
}

fn zip_error_to_kir_error(error: zip::result::ZipError) -> KirError {
    KirError::Io(std::io::Error::other(error))
}

fn is_library_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("model" | "core" | "sysml" | "kerml")
    )
}

fn is_library_archive_source_entry(entry_name: &str) -> bool {
    entry_name.ends_with(".model")
        || entry_name.ends_with(".core")
        || entry_name.ends_with(".sysml")
        || entry_name.ends_with(".kerml")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::{
        BaselineLibraryConfig, KparPackageBuild, KparPackageSource, LibraryProviderConfig,
        write_kpar_package,
    };
    use crate::kir::{KIR_SCHEMA_VERSION, KirDocument, KirElement};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn bundled_baseline_config_defaults_to_stdlib_provider() {
        let config = BaselineLibraryConfig::default();

        assert_eq!(config.id, "stdlib");
        assert_eq!(config.provider, LibraryProviderConfig::BundledStdlib);
    }

    #[test]
    fn stdlib_locator_resolves_bundled_package_before_fallbacks() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-stdlib-locator-empty-repo-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &temp_root);
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
            std::env::set_var("MERCURIO_CONFIG_PATH", temp_root.join("config.json"));
        }

        let artifact = BaselineLibraryConfig::stdlib_locator().resolve().unwrap();

        assert_eq!(artifact.library_id, "stdlib");
        assert_eq!(artifact.source_kind, "bundled_stdlib");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            None
        );
        assert!(
            artifact
                .source_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                == Some("empty.kir.json")
        );
        assert!(artifact.document.elements.is_empty());

        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_CONFIG_PATH");
        }
        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn stdlib_locator_fingerprint_uses_bundled_package_before_fallbacks() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-stdlib-fingerprint-empty-repo-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &temp_root);
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
            std::env::set_var("MERCURIO_CONFIG_PATH", temp_root.join("config.json"));
        }

        let fingerprint = BaselineLibraryConfig::stdlib_locator()
            .provider
            .source_fingerprint("stdlib", None)
            .unwrap();

        assert_eq!(fingerprint.source_kind, "bundled_stdlib");
        assert_eq!(fingerprint.cache_metadata.source_version.as_deref(), None);
        assert!(
            fingerprint
                .cache_metadata
                .source_identity
                .contains("empty.kir.json")
        );

        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_CONFIG_PATH");
        }
        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_resolves_bundled_ai_profile_package() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-ai-profile-bundled-repo-{}",
            std::process::id()
        ));
        let user_repo = temp_root.join("user-packages");
        let cache_root = temp_root.join("kir-cache");
        std::fs::create_dir_all(&user_repo).unwrap();
        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &user_repo);
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
            std::env::set_var("MERCURIO_CONFIG_PATH", temp_root.join("config.json"));
            std::env::set_var("MERCURIO_PACKAGE_KIR_CACHE", &cache_root);
        }

        let artifact = LibraryProviderConfig::KparLocator {
            locator: "kpar:dev.mercurio/ai-profile:1.0.0".to_string(),
        }
        .resolve("ai-profile")
        .unwrap();

        assert_eq!(artifact.source_kind, "kpar_locator");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("1.0.0")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.AiProfile.AiGuidance")
        );

        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_CONFIG_PATH");
            std::env::remove_var("MERCURIO_PACKAGE_KIR_CACHE");
        }
        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn precompiled_kir_artifact_provider_resolves_document_from_file() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-local-library-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kir_path = temp_root.join("sample.kir.json");
        let sample = KirDocument {
            metadata: BTreeMap::from([("source".to_string(), Value::String("test".to_string()))]),
            elements: vec![KirElement {
                id: "Demo::Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        };
        sample.write_pretty_to_path(&kir_path).unwrap();

        let artifact = LibraryProviderConfig::PrecompiledKirArtifact {
            path: kir_path.display().to_string(),
        }
        .resolve("demo")
        .unwrap();

        assert_eq!(artifact.library_id, "demo");
        assert_eq!(artifact.source_kind, "precompiled_kir_artifact");
        assert_eq!(artifact.document.elements.len(), 1);
        assert_eq!(artifact.document.elements[0].id, "Demo::Thing");

        std::fs::remove_file(kir_path).unwrap();
        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn precompiled_kir_artifact_provider_resolves_relative_to_base_dir() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-library-relative-{}",
            std::process::id()
        ));
        let base_dir = temp_root.join("project");
        std::fs::create_dir_all(&base_dir).unwrap();
        let kir_path = base_dir.join("baseline").join("sample.kir.json");
        let sample = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::RelativeThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        sample.write_pretty_to_path(&kir_path).unwrap();

        let artifact = LibraryProviderConfig::PrecompiledKirArtifact {
            path: "baseline/sample.kir.json".to_string(),
        }
        .resolve_from("demo", Some(&base_dir))
        .unwrap();

        assert_eq!(artifact.document.elements[0].id, "Demo::RelativeThing");

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn deserializes_legacy_local_kir_file_alias() {
        let config: LibraryProviderConfig = serde_json::from_value(serde_json::json!({
            "kind": "local_kir_file",
            "path": "baseline/sample.kir.json"
        }))
        .unwrap();

        assert_eq!(
            config,
            LibraryProviderConfig::PrecompiledKirArtifact {
                path: "baseline/sample.kir.json".to_string()
            }
        );
    }

    #[test]
    fn model_directory_provider_compiles_source_backed_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-model-library-{}", std::process::id()));
        let source_dir = temp_root.join("library");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("domain.model"),
            "package Demo {\n  part def Thing;\n}\n",
        )
        .unwrap();

        let artifact = LibraryProviderConfig::ModelDirectory {
            path: source_dir.display().to_string(),
        }
        .resolve("demo")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Demo.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn model_directory_provider_compiles_core_sources() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-foundation-directory-library-{}",
            std::process::id()
        ));
        let source_dir = temp_root.join("library");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("kernel.model"),
            "package Kernel {\n  feature def SemanticThing;\n}\n",
        )
        .unwrap();

        let artifact = LibraryProviderConfig::ModelDirectory {
            path: source_dir.display().to_string(),
        }
        .resolve("kernel-lib")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_file_provider_compiles_source_backed_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-library-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");
        write_test_kpar(
            &kpar_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("domain-lib")
        .unwrap();

        assert_eq!(artifact.source_kind, "kpar_file");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("1.2.3")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn write_kpar_package_writes_source_backed_library() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-write-kpar-library-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");

        write_kpar_package(
            &kpar_path,
            &KparPackageBuild {
                name: "Domain Library".to_string(),
                version: Some("1.2.3".to_string()),
                precompiled_kir: None,
                sources: vec![KparPackageSource {
                    path: "domain.model".to_string(),
                    content: "package Domain {\n  part def Thing;\n}\n".to_string(),
                }],
            },
        )
        .unwrap();

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("domain-lib")
        .unwrap();

        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("1.2.3")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_stages_and_finds_kpar() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-package-repo-{}",
            std::process::id()
        ));
        let repo = super::LocalPackageRepository::new(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "domain-lib",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );

        let manifest = repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();
        let staged = repo.find_package("domain-lib", "1.2.3").unwrap().unwrap();

        assert_eq!(manifest.name, "domain-lib");
        assert_eq!(manifest.version, "1.2.3");
        assert!(staged.is_file());
        assert_eq!(
            staged.file_name().and_then(|value| value.to_str()),
            Some("domain-lib-1.2.3.kpar")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_file_provider_prefers_precompiled_kir_payload() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-precompiled-kir-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![KirElement {
                id: "type.Precompiled.Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "qualified_name".to_string(),
                    Value::String("Precompiled.Thing".to_string()),
                )]),
            }],
        };

        write_kpar_package(
            &kpar_path,
            &KparPackageBuild {
                name: "Domain Library".to_string(),
                version: Some("1.2.3".to_string()),
                precompiled_kir: Some(document),
                sources: vec![KparPackageSource {
                    path: "invalid.model".to_string(),
                    content: "this is not model".to_string(),
                }],
            },
        )
        .unwrap();

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("domain-lib")
        .unwrap();

        assert_eq!(artifact.document.elements.len(), 1);
        assert_eq!(artifact.document.elements[0].id, "type.Precompiled.Thing");

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn write_kpar_package_preserves_sysml_source_entries() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-sysml-source-{}", std::process::id()));
        let repo = super::LocalPackageRepository::new(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("ai-profile.kpar");
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![KirElement {
                id: "metadata.AiProfile.AiGuidance".to_string(),
                kind: "MetadataDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "qualified_name".to_string(),
                    Value::String("AiProfile.AiGuidance".to_string()),
                )]),
            }],
        };

        write_kpar_package(
            &kpar_path,
            &KparPackageBuild {
                name: "dev.mercurio/ai-profile".to_string(),
                version: Some("1.0.0".to_string()),
                precompiled_kir: Some(document),
                sources: vec![KparPackageSource {
                    path: "src/ai-profile.sysml".to_string(),
                    content: "package AiProfile {\n  metadata def AiGuidance;\n}\n".to_string(),
                }],
            },
        )
        .unwrap();
        repo.stage_kpar(&kpar_path, "dev.mercurio/ai-profile", "1.0.0", None)
            .unwrap();

        let verification = repo
            .verify_package("dev.mercurio/ai-profile", "1.0.0")
            .unwrap();
        assert_eq!(verification.source_count, 1);
        assert!(verification.has_precompiled_kir);

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn write_kpar_package_allows_kir_only_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-kir-only-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("stdlib.kpar");
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![KirElement {
                id: "type.Stdlib.Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "qualified_name".to_string(),
                    Value::String("Stdlib.Thing".to_string()),
                )]),
            }],
        };

        write_kpar_package(
            &kpar_path,
            &KparPackageBuild {
                name: "Stdlib".to_string(),
                version: Some("2.0.0".to_string()),
                precompiled_kir: Some(document),
                sources: Vec::new(),
            },
        )
        .unwrap();

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("stdlib")
        .unwrap();

        assert_eq!(artifact.document.elements[0].id, "type.Stdlib.Thing");

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_verifies_kir_only_package() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-package-verify-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("stdlib.kpar");
        let repo = super::LocalPackageRepository::new(temp_root.join("repo"));
        let document = KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![KirElement {
                id: "type.Stdlib.Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "qualified_name".to_string(),
                    Value::String("Stdlib.Thing".to_string()),
                )]),
            }],
        };

        write_kpar_package(
            &source_path,
            &KparPackageBuild {
                name: "org.omg/model-stdlib".to_string(),
                version: Some("2.0.0".to_string()),
                precompiled_kir: Some(document),
                sources: Vec::new(),
            },
        )
        .unwrap();
        repo.stage_kpar(&source_path, "org.omg/model-stdlib", "2.0.0", None)
            .unwrap();

        let verification = repo
            .verify_package("org.omg/model-stdlib", "2.0.0")
            .unwrap();

        assert_eq!(verification.name, "org.omg/model-stdlib");
        assert_eq!(verification.version, "2.0.0");
        assert_eq!(verification.source_count, 0);
        assert!(verification.has_precompiled_kir);
        assert_eq!(verification.precompiled_kir_element_count, Some(1));

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_verification_reports_precompiled_kir_semantic_warnings() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-package-validation-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("behavior.kpar");
        let repo = super::LocalPackageRepository::new(temp_root.join("repo"));

        write_kpar_package(
            &source_path,
            &KparPackageBuild {
                name: "behavior-lib".to_string(),
                version: Some("1.0.0".to_string()),
                precompiled_kir: Some(package_validation_warning_document()),
                sources: Vec::new(),
            },
        )
        .unwrap();
        repo.stage_kpar(&source_path, "behavior-lib", "1.0.0", None)
            .unwrap();

        let verification = repo.verify_package("behavior-lib", "1.0.0").unwrap();

        assert_eq!(verification.validation_diagnostics.len(), 1);
        assert_eq!(
            verification.validation_diagnostics[0].code,
            "kir.metamodel.endpoints.incomplete"
        );
        assert_eq!(
            verification.validation_diagnostics[0].subjects,
            vec!["transition.Demo.start".to_string()]
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_file_provider_compiles_source_backed_library() {
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-locator-file-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("domain-lib.kpar");
        write_test_kpar(
            &kpar_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );

        let artifact = LibraryProviderConfig::KparLocator {
            locator: format!("file:{}", kpar_path.display()),
        }
        .resolve("domain-lib")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_coordinate_resolves_from_local_package_repo() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-locator-repo-{}", std::process::id()));
        let repo = super::LocalPackageRepository::new(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "domain-lib",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        repo.stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &temp_root);
        }
        let artifact = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3".to_string(),
        }
        .resolve("domain-lib")
        .unwrap();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
        }

        assert_eq!(artifact.source_kind, "kpar_locator");
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn package_reference_parses_kpar_and_oci_digest_locators() {
        let kpar = super::parse_package_reference(
            "kpar:org.example/domain-lib:1.2.3@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();
        assert_eq!(kpar.scheme, "kpar");
        assert_eq!(kpar.name, "org.example/domain-lib");
        assert_eq!(kpar.version.as_deref(), Some("1.2.3"));
        assert_eq!(
            kpar.digest.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );

        let oci = super::parse_package_reference(
            "oci://ghcr.io/org.example/domain-lib@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap();
        assert_eq!(oci.scheme, "oci");
        assert_eq!(oci.name, "ghcr.io/org.example/domain-lib");
        assert_eq!(oci.version, None);
        assert_eq!(
            oci.digest.as_deref(),
            Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }

    #[test]
    fn package_file_digest_uses_sha256() {
        assert_eq!(
            super::format_sha256_digest(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn staged_package_manifest_uses_sha256_digest() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-package-sha256-{}",
            std::process::id()
        ));
        let repo = super::LocalPackageRepository::new(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "domain-lib",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );

        let manifest = repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        assert!(manifest.digest.starts_with("sha256:"));
        assert_eq!(manifest.digest.len(), "sha256:".len() + 64);
        let verification = repo.verify_package("domain-lib", "1.2.3").unwrap();
        let package_manifest = verification.package_manifest.unwrap();
        assert_eq!(package_manifest.schema, "dev.mercurio.package.v1");
        assert_eq!(package_manifest.name, "domain-lib");
        assert_eq!(package_manifest.version, "1.2.3");
        assert_eq!(package_manifest.kind, "kpar");
        assert_eq!(
            package_manifest.kir_schema_version.as_deref(),
            Some(KIR_SCHEMA_VERSION)
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_digest_pin_must_match_package_bytes() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-locator-digest-pin-{}",
            std::process::id()
        ));
        let repo = super::LocalPackageRepository::new(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        let manifest = repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &temp_root);
        }
        let artifact = LibraryProviderConfig::KparLocator {
            locator: format!("kpar:domain-lib:1.2.3@{}", manifest.digest),
        }
        .resolve("domain-lib")
        .unwrap();
        let err = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3@sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        }
        .resolve("domain-lib")
        .unwrap_err();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
        }

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );
        assert!(err.to_string().contains("digest mismatch"));

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_reuses_cached_kir_document() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root =
            std::env::temp_dir().join(format!("mercurio-kpar-kir-cache-{}", std::process::id()));
        let repo_root = temp_root.join("packages");
        let cache_root = temp_root.join("kir-cache");
        let repo = super::LocalPackageRepository::new(&repo_root);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        repo.stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &repo_root);
            std::env::set_var("MERCURIO_PACKAGE_KIR_CACHE", &cache_root);
        }
        let first = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3".to_string(),
        }
        .resolve("domain-lib")
        .unwrap();
        assert!(
            first
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        let cache_document = find_first_file_named(&cache_root, "document.kir.json").unwrap();
        let cached = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Cached.Thing".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "qualified_name".to_string(),
                    Value::String("Cached.Thing".to_string()),
                )]),
            }],
        };
        cached.write_pretty_to_path(&cache_document).unwrap();

        let second = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3".to_string(),
        }
        .resolve("domain-lib")
        .unwrap();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_PACKAGE_KIR_CACHE");
        }

        assert!(
            second
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Cached.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_resolves_from_configured_repository_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-configured-repo-env-{}",
            std::process::id()
        ));
        let published_repo = temp_root.join("published");
        let repo = super::LocalPackageRepository::new(&published_repo);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        repo.stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::set_var("MERCURIO_PACKAGE_REPOSITORIES", &published_repo);
        }
        let artifact = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3".to_string(),
        }
        .resolve("domain-lib")
        .unwrap();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
        }

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_resolves_from_user_config_repository() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-configured-repo-file-{}",
            std::process::id()
        ));
        let published_repo = temp_root.join("published");
        let config_path = temp_root.join("config.json");
        let repo = super::LocalPackageRepository::new(&published_repo);
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        repo.stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();
        std::fs::write(
            &config_path,
            serde_json::json!({
                "package_repositories": [published_repo.display().to_string()]
            })
            .to_string(),
        )
        .unwrap();

        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
            std::env::set_var("MERCURIO_CONFIG_PATH", &config_path);
        }
        let artifact = LibraryProviderConfig::KparLocator {
            locator: "kpar:domain-lib:1.2.3".to_string(),
        }
        .resolve("domain-lib")
        .unwrap();
        unsafe {
            std::env::remove_var("MERCURIO_CONFIG_PATH");
        }

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_not_found_reports_searched_repositories() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-locator-missing-{}",
            std::process::id()
        ));
        let user_repo = temp_root.join("user");
        let configured_repo = temp_root.join("configured");
        let config_path = temp_root.join("config.json");
        std::fs::create_dir_all(&temp_root).unwrap();
        std::fs::write(
            &config_path,
            serde_json::json!({
                "package_repositories": [configured_repo.display().to_string()]
            })
            .to_string(),
        )
        .unwrap();

        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &user_repo);
            std::env::set_var("MERCURIO_CONFIG_PATH", &config_path);
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
        }
        let err = LibraryProviderConfig::KparLocator {
            locator: "kpar:missing-lib:9.9.9".to_string(),
        }
        .resolve("missing-lib")
        .unwrap_err();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
            std::env::remove_var("MERCURIO_CONFIG_PATH");
        }

        let message = err.to_string();
        assert!(message.contains("missing-lib"));
        assert!(message.contains("9.9.9"));
        assert!(message.contains("searched:"));
        assert!(message.contains(&user_repo.display().to_string()));
        assert!(message.contains(&configured_repo.display().to_string()));
        assert!(message.contains(&super::bundled_package_repo_path().display().to_string()));

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_locator_fingerprint_not_found_reports_searched_repositories() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-kpar-locator-fingerprint-missing-{}",
            std::process::id()
        ));
        let user_repo = temp_root.join("user");
        std::fs::create_dir_all(&temp_root).unwrap();

        unsafe {
            std::env::set_var("MERCURIO_PACKAGE_REPO", &user_repo);
            std::env::remove_var("MERCURIO_CONFIG_PATH");
            std::env::remove_var("MERCURIO_PACKAGE_REPOSITORIES");
        }
        let err = LibraryProviderConfig::KparLocator {
            locator: "kpar:missing-lib:9.9.9".to_string(),
        }
        .source_fingerprint("missing-lib", None)
        .unwrap_err();
        unsafe {
            std::env::remove_var("MERCURIO_PACKAGE_REPO");
        }

        let message = err.to_string();
        assert!(message.contains("missing-lib"));
        assert!(message.contains("searched:"));
        assert!(message.contains(&user_repo.display().to_string()));
        assert!(message.contains(&super::bundled_package_repo_path().display().to_string()));

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_publishes_to_target_repository() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-package-publish-{}",
            std::process::id()
        ));
        let source_repo = super::LocalPackageRepository::new(temp_root.join("source"));
        let target_repo = super::LocalPackageRepository::new(temp_root.join("target"));
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        source_repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        let manifest = source_repo
            .publish_to_repository(&target_repo, "domain-lib", "1.2.3", false)
            .unwrap();
        let published = target_repo
            .find_package("domain-lib", "1.2.3")
            .unwrap()
            .unwrap();

        assert_eq!(manifest.name, "domain-lib");
        assert!(published.is_file());
        assert_eq!(
            source_repo.read_manifest("domain-lib", "1.2.3").unwrap(),
            target_repo.read_manifest("domain-lib", "1.2.3").unwrap()
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_publish_rejects_existing_without_force() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-package-publish-existing-{}",
            std::process::id()
        ));
        let source_repo = super::LocalPackageRepository::new(temp_root.join("source"));
        let target_repo = super::LocalPackageRepository::new(temp_root.join("target"));
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        source_repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();
        source_repo
            .publish_to_repository(&target_repo, "domain-lib", "1.2.3", false)
            .unwrap();

        let err = source_repo
            .publish_to_repository(&target_repo, "domain-lib", "1.2.3", false)
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));

        source_repo
            .publish_to_repository(&target_repo, "domain-lib", "1.2.3", true)
            .unwrap();

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn local_package_repository_pulls_from_source_repository() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-local-package-pull-{}",
            std::process::id()
        ));
        let source_repo = super::LocalPackageRepository::new(temp_root.join("source"));
        let target_repo = super::LocalPackageRepository::new(temp_root.join("target"));
        std::fs::create_dir_all(&temp_root).unwrap();
        let source_path = temp_root.join("source.kpar");
        write_test_kpar(
            &source_path,
            "Domain Library",
            "1.2.3",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );
        source_repo
            .stage_kpar(&source_path, "domain-lib", "1.2.3", None)
            .unwrap();

        let manifest = target_repo
            .pull_from_repository(&source_repo, "domain-lib", "1.2.3", false)
            .unwrap();
        let pulled = target_repo
            .find_package("domain-lib", "1.2.3")
            .unwrap()
            .unwrap();

        assert_eq!(manifest.name, "domain-lib");
        assert!(pulled.is_file());
        assert_eq!(
            source_repo.read_manifest("domain-lib", "1.2.3").unwrap(),
            target_repo.read_manifest("domain-lib", "1.2.3").unwrap()
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn kpar_file_provider_compiles_core_sources() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-foundation-kpar-library-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();
        let kpar_path = temp_root.join("kernel-lib.kpar");
        write_test_kpar(
            &kpar_path,
            "Kernel Library",
            "1.2.3",
            &[(
                "kernel.model",
                "package Kernel {\n  feature def SemanticThing;\n}\n",
            )],
        );

        let artifact = LibraryProviderConfig::KparFile {
            path: kpar_path.display().to_string(),
        }
        .resolve("kernel-lib")
        .unwrap();

        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    #[test]
    fn package_set_directory_provider_resolves_dependency_closure() {
        let temp_root = std::env::temp_dir().join(format!(
            "mercurio-package-set-library-{}",
            std::process::id()
        ));
        let package_dir = temp_root.join("package-set");
        std::fs::create_dir_all(&package_dir).unwrap();

        write_test_kpar_with_usage(
            &package_dir.join("Kernel_Semantic_Library-1.0.0.kpar"),
            "Kernel Semantic Library",
            "1.0.0",
            &[],
            &[(
                "semantic.model",
                "package Kernel {\n  feature def SemanticThing;\n}\n",
            )],
        );
        write_test_kpar_with_usage(
            &package_dir.join("Model_Systems_Library-2.0.0.kpar"),
            "Model Systems Library",
            "2.0.0",
            &[(
                "https://www.omg.org/spec/Core/20250201/Semantic-Library.kpar",
                "1.0.0",
            )],
            &[(
                "systems.model",
                "package Systems {\n  part def SystemThing;\n}\n",
            )],
        );

        let artifact = LibraryProviderConfig::PackageSetDirectory {
            path: package_dir.display().to_string(),
            entry: "https://www.omg.org/spec/Model/20250201/Systems-Library.kpar".to_string(),
        }
        .resolve("systems")
        .unwrap();
        let fingerprint = LibraryProviderConfig::PackageSetDirectory {
            path: package_dir.display().to_string(),
            entry: "https://www.omg.org/spec/Model/20250201/Systems-Library.kpar".to_string(),
        }
        .source_fingerprint("systems", None)
        .unwrap();

        assert_eq!(artifact.source_kind, "package_set_directory");
        assert_eq!(
            artifact
                .cache_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_version.as_deref()),
            Some("2.0.0")
        );
        assert_eq!(fingerprint.source_kind, "package_set_directory");
        assert_eq!(
            fingerprint.cache_metadata.source_version.as_deref(),
            Some("2.0.0")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Systems.SystemThing")
        );

        std::fs::remove_dir_all(temp_root).unwrap();
    }

    fn write_test_kpar(
        path: &std::path::Path,
        name: &str,
        version: &str,
        entries: &[(&str, &str)],
    ) {
        write_test_kpar_with_usage(path, name, version, &[], entries);
    }

    fn write_test_kpar_with_usage(
        path: &std::path::Path,
        name: &str,
        version: &str,
        usage: &[(&str, &str)],
        entries: &[(&str, &str)],
    ) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();

        writer.start_file(".project.json", options).unwrap();
        writer
            .write_all(
                serde_json::json!({
                    "name": name,
                    "version": version,
                    "usage": usage
                        .iter()
                        .map(|(resource, version_constraint)| serde_json::json!({
                            "resource": resource,
                            "versionConstraint": version_constraint
                        }))
                        .collect::<Vec<_>>()
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();

        writer.start_file(".meta.json", options).unwrap();
        writer.write_all(br#"{"files":[]}"#).unwrap();

        writer
            .start_file(super::MERCURIO_PACKAGE_MANIFEST_ENTRY, options)
            .unwrap();
        writer
            .write_all(
                serde_json::to_string_pretty(&super::MercurioPackageManifest {
                    schema: "dev.mercurio.package.v1".to_string(),
                    name: name.to_string(),
                    version: version.to_string(),
                    kind: "kpar".to_string(),
                    dependencies: Vec::new(),
                    kir_schema_version: Some(KIR_SCHEMA_VERSION.to_string()),
                    source: None,
                    build: None,
                })
                .unwrap()
                .as_bytes(),
            )
            .unwrap();

        for (entry_name, content) in entries {
            writer.start_file(*entry_name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }

        writer.finish().unwrap();
    }

    fn package_validation_warning_document() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![
                KirElement {
                    id: "SysML::Systems::TransitionUsage".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "qualified_name".to_string(),
                        Value::String("SysML::Systems::TransitionUsage".to_string()),
                    )]),
                },
                metamodel_feature(
                    "metafeature.TransitionUsage.source",
                    "SysML::Systems::TransitionUsage",
                    "source",
                ),
                metamodel_feature(
                    "metafeature.TransitionUsage.target",
                    "SysML::Systems::TransitionUsage",
                    "target",
                ),
                KirElement {
                    id: "transition.Demo.start".to_string(),
                    kind: "SysML::Systems::TransitionUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.start".to_string()),
                        ),
                        (
                            "source".to_string(),
                            Value::String("state.Demo.initial".to_string()),
                        ),
                    ]),
                },
            ],
        }
    }

    fn metamodel_feature(id: &str, owner: &str, kir_property: &str) -> KirElement {
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties: BTreeMap::from([
                ("qualified_name".to_string(), Value::String(id.to_string())),
                ("owner".to_string(), Value::String(owner.to_string())),
                (
                    "kir_property".to_string(),
                    Value::String(kir_property.to_string()),
                ),
                (
                    "feature_kind".to_string(),
                    Value::String("reference".to_string()),
                ),
            ]),
        }
    }

    fn find_first_file_named(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
        if !root.is_dir() {
            return None;
        }
        let mut entries = std::fs::read_dir(root)
            .ok()?
            .collect::<Result<Vec<_>, std::io::Error>>()
            .ok()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_first_file_named(&path, name) {
                    return Some(found);
                }
            } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
                return Some(path);
            }
        }
        None
    }
}
