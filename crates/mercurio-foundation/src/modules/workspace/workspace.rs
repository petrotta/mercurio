use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::kir::{KirDocument, KirError};
use crate::semantic_services::semantic_validation::{
    SemanticValidationReport, validate_kir_semantics_with_context,
};
use crate::workspace::library::{
    BaselineLibraryConfig, LibraryCacheMetadata, LibraryProviderConfig, LibrarySourceFingerprint,
    ResolvedLibraryArtifact,
};

pub const PROJECT_EXTENSION_DESCRIPTOR_FILE_NAME: &str = "mercurio.extensions.json";

fn is_model_source_file(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()).is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(default = "default_workspace_config_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub libraries: Vec<WorkspaceLibraryConfig>,
    #[serde(default)]
    pub plugins: Vec<WorkspacePluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ProjectDescriptor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default = "default_project_descriptor_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "ProjectModelConfig::is_empty")]
    pub model: ProjectModelConfig,
    #[serde(default, alias = "libraries")]
    pub dependencies: Vec<WorkspaceLibraryConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metamodel: Option<String>,
}

impl ProjectModelConfig {
    fn is_empty(&self) -> bool {
        self.source_roots.is_empty() && self.entrypoints.is_empty() && self.metamodel.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExtensionDescriptor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default = "default_project_extension_descriptor_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_plugins: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<serde_json::Value>,
}

impl ProjectExtensionDescriptor {
    pub fn from_path(path: &Path) -> Result<Self, WorkspaceConfigError> {
        let input = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&input)?)
    }
}

#[derive(Debug)]
pub enum WorkspaceConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Kir(KirError),
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct ResolvedWorkspaceContext {
    pub workspace_root: PathBuf,
    pub config_path: Option<PathBuf>,
    pub config: Option<WorkspaceConfig>,
    pub extension_path: Option<PathBuf>,
    pub extension: Option<ProjectExtensionDescriptor>,
    pub resolved_libraries: Vec<ResolvedWorkspaceLibrary>,
    pub validation_report: SemanticValidationReport,
    pub library_context_document: KirDocument,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextOptions {
    pub config_path: Option<PathBuf>,
    pub cache_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLibraryRole {
    Baseline,
    Dependency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLibraryConfig {
    #[serde(default = "default_workspace_library_id")]
    pub id: String,
    #[serde(default = "default_workspace_library_role")]
    pub role: WorkspaceLibraryRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<LibraryProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePluginConfig {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedWorkspaceLibrary {
    pub id: String,
    pub role: WorkspaceLibraryRole,
    pub source_kind: String,
    pub source_path: Option<PathBuf>,
    pub cache_metadata: Option<LibraryCacheMetadata>,
    pub cache_path: Option<PathBuf>,
    pub cached_element_count: Option<usize>,
    pub validation_report: SemanticValidationReport,
    pub document: KirDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceLibraryCacheManifest {
    pub library_id: String,
    pub role: WorkspaceLibraryRole,
    pub source_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importer_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_digest: Option<String>,
    pub element_count: usize,
}

impl std::fmt::Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read workspace config: {err}"),
            Self::Json(err) => write!(f, "failed to parse workspace config: {err}"),
            Self::Kir(err) => write!(f, "failed to resolve workspace libraries: {err}"),
            Self::Invalid(message) => write!(f, "invalid workspace config: {message}"),
        }
    }
}

impl std::error::Error for WorkspaceConfigError {}

impl From<std::io::Error> for WorkspaceConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for WorkspaceConfigError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<KirError> for WorkspaceConfigError {
    fn from(value: KirError) -> Self {
        Self::Kir(value)
    }
}

impl WorkspaceConfig {
    pub fn from_path(path: &Path) -> Result<Self, WorkspaceConfigError> {
        let input = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&input)?)
    }
}

impl ProjectDescriptor {
    pub fn from_path(path: &Path) -> Result<Self, WorkspaceConfigError> {
        read_project_descriptor(path).map(|(descriptor, _)| descriptor)
    }

    pub fn to_workspace_config(&self) -> WorkspaceConfig {
        WorkspaceConfig {
            version: self.version,
            name: self.name.clone(),
            libraries: self.dependencies.clone(),
            plugins: Vec::new(),
        }
    }
}

fn read_project_descriptor(
    path: &Path,
) -> Result<(ProjectDescriptor, Option<ProjectExtensionDescriptor>), WorkspaceConfigError> {
    let input = std::fs::read_to_string(path)?;
    let mut value = serde_json::from_str::<serde_json::Value>(&input)?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    let legacy_extension = if version <= 1 {
        value
            .as_object_mut()
            .map(|object| {
                let project_plugins = object
                    .remove("projectPlugins")
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                let capabilities = object
                    .remove("capabilities")
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                let views = object
                    .remove("views")
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                Ok::<_, serde_json::Error>(ProjectExtensionDescriptor {
                    schema: None,
                    version: default_project_extension_descriptor_version(),
                    project_plugins,
                    capabilities,
                    views,
                })
            })
            .transpose()?
    } else {
        None
    };
    let descriptor = serde_json::from_value(value)?;
    let legacy_extension = legacy_extension.filter(|extension| {
        !extension.project_plugins.is_empty()
            || !extension.capabilities.is_empty()
            || !extension.views.is_empty()
    });
    Ok((descriptor, legacy_extension))
}
pub fn resolve_workspace_context(
    open_path: &Path,
) -> Result<ResolvedWorkspaceContext, WorkspaceConfigError> {
    resolve_workspace_context_with_options(open_path, WorkspaceContextOptions::default())
}

pub fn resolve_workspace_context_from_config_path(
    open_path: &Path,
    config_path: impl Into<PathBuf>,
) -> Result<ResolvedWorkspaceContext, WorkspaceConfigError> {
    resolve_workspace_context_with_options(
        open_path,
        WorkspaceContextOptions {
            config_path: Some(config_path.into()),
            cache_root: None,
        },
    )
}

pub fn resolve_project_descriptor_context(
    descriptor_path: impl AsRef<Path>,
) -> Result<ResolvedWorkspaceContext, WorkspaceConfigError> {
    let descriptor_path = descriptor_path.as_ref();
    let (descriptor, legacy_extension) = read_project_descriptor(descriptor_path)?;
    let config = descriptor.to_workspace_config();
    let workspace_root = descriptor_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let (library_context_document, resolved_libraries) =
        resolve_library_context_document(Some(&config), Some(&workspace_root), None)?;
    let extension_path = discover_project_extension_descriptor_path(&workspace_root);
    let extension = extension_path
        .as_deref()
        .map(ProjectExtensionDescriptor::from_path)
        .transpose()?
        .or(legacy_extension);

    let validation_report = aggregate_workspace_validation_report(&resolved_libraries);

    Ok(ResolvedWorkspaceContext {
        workspace_root,
        config_path: Some(descriptor_path.to_path_buf()),
        config: Some(config),
        extension_path,
        extension,
        resolved_libraries,
        validation_report,
        library_context_document,
    })
}

pub fn resolve_workspace_context_with_options(
    open_path: &Path,
    options: WorkspaceContextOptions,
) -> Result<ResolvedWorkspaceContext, WorkspaceConfigError> {
    let config_path = options.config_path;
    let config_root = config_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    let config = config_path
        .as_deref()
        .map(WorkspaceConfig::from_path)
        .transpose()?;
    let workspace_root = config_root
        .clone()
        .unwrap_or_else(|| default_workspace_root_for_open_path(open_path));
    let cache_root = options.cache_root;
    let (library_context_document, resolved_libraries) = resolve_library_context_document(
        config.as_ref(),
        config_root.as_deref(),
        cache_root.as_deref(),
    )?;

    let validation_report = aggregate_workspace_validation_report(&resolved_libraries);

    Ok(ResolvedWorkspaceContext {
        workspace_root,
        config_path,
        config,
        extension_path: None,
        extension: None,
        resolved_libraries,
        validation_report,
        library_context_document,
    })
}

fn aggregate_workspace_validation_report(
    resolved_libraries: &[ResolvedWorkspaceLibrary],
) -> SemanticValidationReport {
    SemanticValidationReport {
        diagnostics: resolved_libraries
            .iter()
            .flat_map(|library| library.validation_report.diagnostics.iter().cloned())
            .collect(),
    }
}

pub fn discover_project_extension_descriptor_path(workspace_root: &Path) -> Option<PathBuf> {
    let candidate = workspace_root.join(PROJECT_EXTENSION_DESCRIPTOR_FILE_NAME);
    candidate.is_file().then_some(candidate)
}

pub fn discover_workspace_config_path(open_path: &Path, config_file_name: &str) -> Option<PathBuf> {
    let start = if open_path.is_dir() {
        open_path
    } else {
        open_path.parent()?
    };

    for ancestor in start.ancestors() {
        let candidate = ancestor.join(config_file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn resolve_library_context_document(
    config: Option<&WorkspaceConfig>,
    config_root: Option<&Path>,
    cache_root: Option<&Path>,
) -> Result<(KirDocument, Vec<ResolvedWorkspaceLibrary>), WorkspaceConfigError> {
    let workspace_libraries = config
        .map(|config| config.libraries.as_slice())
        .unwrap_or(&[]);

    let mut resolved_libraries = Vec::new();
    let baseline_configs = workspace_libraries
        .iter()
        .filter(|library| library.role == WorkspaceLibraryRole::Baseline)
        .map(WorkspaceLibraryConfig::to_baseline_library_config)
        .collect::<Result<Vec<_>, _>>()?;
    let dependency_configs = workspace_libraries
        .iter()
        .filter(|library| library.role == WorkspaceLibraryRole::Dependency)
        .map(WorkspaceLibraryConfig::to_baseline_library_config)
        .collect::<Result<Vec<_>, _>>()?;
    let baseline_configs = baseline_configs;
    for library in &baseline_configs {
        resolved_libraries.push(resolve_or_load_workspace_library(
            library,
            WorkspaceLibraryRole::Baseline,
            config_root,
            cache_root,
            None,
        )?);
    }
    let baseline_documents = resolved_libraries
        .iter()
        .map(|library| library.document.clone())
        .collect::<Vec<_>>();

    let mut library_context = KirDocument::merge(baseline_documents)?;

    for library in &dependency_configs {
        let resolved_library = resolve_or_load_workspace_library(
            library,
            WorkspaceLibraryRole::Dependency,
            config_root,
            cache_root,
            Some(&library_context),
        )?;
        library_context = KirDocument::merge([library_context, resolved_library.document.clone()])?;
        resolved_libraries.push(resolved_library);
    }

    Ok((library_context, resolved_libraries))
}

fn default_workspace_root_for_open_path(open_path: &Path) -> PathBuf {
    if is_model_source_file(open_path) {
        open_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else if open_path.is_dir() {
        open_path.to_path_buf()
    } else {
        open_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

impl ResolvedWorkspaceLibrary {
    fn from_artifact(
        role: WorkspaceLibraryRole,
        artifact: &ResolvedLibraryArtifact,
        cache_path: Option<PathBuf>,
    ) -> Self {
        Self {
            id: artifact.library_id.clone(),
            role,
            source_kind: artifact.source_kind.clone(),
            source_path: artifact.source_path.clone(),
            cache_metadata: artifact.cache_metadata.clone(),
            cache_path,
            cached_element_count: Some(artifact.document.elements.len()),
            validation_report: artifact.validation_report.clone(),
            document: artifact.document.clone(),
        }
    }
}

impl WorkspaceLibraryConfig {
    fn to_baseline_library_config(&self) -> Result<BaselineLibraryConfig, WorkspaceConfigError> {
        let provider = match (&self.locator, &self.provider) {
            (Some(locator), None) => LibraryProviderConfig::KparLocator {
                locator: locator.clone(),
            },
            (None, Some(LibraryProviderConfig::KparLocator { .. })) => {
                return Err(WorkspaceConfigError::Invalid(format!(
                    "library '{}' must use the top-level locator field instead of provider kind kpar_locator",
                    self.id
                )));
            }
            (None, Some(provider)) => provider.clone(),
            (Some(_), Some(_)) => {
                return Err(WorkspaceConfigError::Invalid(format!(
                    "library '{}' must use either locator or provider, not both",
                    self.id
                )));
            }
            (None, None) => {
                return Err(WorkspaceConfigError::Invalid(format!(
                    "library '{}' must declare locator or provider",
                    self.id
                )));
            }
        };

        Ok(BaselineLibraryConfig {
            id: self.id.clone(),
            provider,
        })
    }
}

fn resolve_or_load_workspace_library(
    library: &BaselineLibraryConfig,
    role: WorkspaceLibraryRole,
    config_root: Option<&Path>,
    cache_root: Option<&Path>,
    library_context: Option<&KirDocument>,
) -> Result<ResolvedWorkspaceLibrary, WorkspaceConfigError> {
    let context_digest = library_context.map(kir_document_digest).transpose()?;
    let fingerprint = cache_root
        .map(|_| {
            library
                .provider
                .source_fingerprint(&library.id, config_root)
        })
        .transpose()?;

    if let (Some(cache_root), Some(fingerprint)) = (cache_root, fingerprint.as_ref()) {
        if let Some((artifact, cache_path)) = load_cached_library(
            cache_root,
            role,
            fingerprint,
            context_digest.as_deref(),
            library_context,
        )? {
            return Ok(ResolvedWorkspaceLibrary::from_artifact(
                role,
                &artifact,
                Some(cache_path),
            ));
        }
    }

    let artifact =
        library
            .provider
            .resolve_with_context(&library.id, config_root, library_context)?;
    let cache_path =
        cache_resolved_library(cache_root, role, &artifact, context_digest.as_deref())?;
    Ok(ResolvedWorkspaceLibrary::from_artifact(
        role, &artifact, cache_path,
    ))
}

fn cache_resolved_library(
    cache_root: Option<&Path>,
    role: WorkspaceLibraryRole,
    artifact: &ResolvedLibraryArtifact,
    context_digest: Option<&str>,
) -> Result<Option<PathBuf>, WorkspaceConfigError> {
    let Some(cache_root) = cache_root else {
        return Ok(None);
    };

    let library_cache_dir = cache_root.join(safe_cache_segment(&artifact.library_id));
    let document_path = library_cache_dir.join("document.kir.json");
    artifact.document.write_pretty_to_path(&document_path)?;

    let manifest = WorkspaceLibraryCacheManifest {
        library_id: artifact.library_id.clone(),
        role,
        source_kind: artifact.source_kind.clone(),
        source_path: artifact
            .source_path
            .as_ref()
            .map(|path| path.display().to_string()),
        source_identity: artifact
            .cache_metadata
            .as_ref()
            .map(|metadata| metadata.source_identity.clone()),
        source_version: artifact
            .cache_metadata
            .as_ref()
            .and_then(|metadata| metadata.source_version.clone()),
        source_digest: artifact
            .cache_metadata
            .as_ref()
            .and_then(|metadata| metadata.source_digest.clone()),
        importer_version: artifact
            .cache_metadata
            .as_ref()
            .map(|metadata| metadata.importer_version.clone()),
        context_digest: context_digest.map(str::to_string),
        element_count: artifact.document.elements.len(),
    };
    std::fs::write(
        library_cache_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    Ok(Some(document_path))
}

fn load_cached_library(
    cache_root: &Path,
    role: WorkspaceLibraryRole,
    fingerprint: &LibrarySourceFingerprint,
    context_digest: Option<&str>,
    library_context: Option<&KirDocument>,
) -> Result<Option<(ResolvedLibraryArtifact, PathBuf)>, WorkspaceConfigError> {
    let library_cache_dir = cache_root.join(safe_cache_segment(&fingerprint.library_id));
    let document_path = library_cache_dir.join("document.kir.json");
    let manifest_path = library_cache_dir.join("manifest.json");

    if !document_path.is_file() || !manifest_path.is_file() {
        return Ok(None);
    }

    let manifest = match std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|input| serde_json::from_str::<WorkspaceLibraryCacheManifest>(&input).ok())
    {
        Some(manifest) => manifest,
        None => return Ok(None),
    };

    if !cache_manifest_matches(&manifest, role, fingerprint, context_digest) {
        return Ok(None);
    }

    let document = match KirDocument::from_path(&document_path) {
        Ok(document) => document,
        Err(_) => return Ok(None),
    };
    let validation_report = validate_kir_semantics_with_context(&document, library_context)?;

    Ok(Some((
        ResolvedLibraryArtifact {
            library_id: fingerprint.library_id.clone(),
            source_kind: fingerprint.source_kind.clone(),
            source_path: fingerprint.source_path.clone(),
            cache_metadata: Some(fingerprint.cache_metadata.clone()),
            validation_report,
            document,
        },
        document_path,
    )))
}

fn cache_manifest_matches(
    manifest: &WorkspaceLibraryCacheManifest,
    role: WorkspaceLibraryRole,
    fingerprint: &LibrarySourceFingerprint,
    context_digest: Option<&str>,
) -> bool {
    manifest.library_id == fingerprint.library_id
        && manifest.role == role
        && manifest.source_kind == fingerprint.source_kind
        && manifest.source_identity == Some(fingerprint.cache_metadata.source_identity.clone())
        && manifest.source_version == fingerprint.cache_metadata.source_version
        && manifest.source_digest == fingerprint.cache_metadata.source_digest
        && manifest.importer_version == Some(fingerprint.cache_metadata.importer_version.clone())
        && manifest.context_digest.as_deref() == context_digest
}

fn kir_document_digest(document: &KirDocument) -> Result<String, WorkspaceConfigError> {
    let bytes = serde_json::to_vec(document)?;
    Ok(format!("fnv1a64:{:016x}", stable_digest_bytes(&bytes)))
}

fn stable_digest_bytes(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn safe_cache_segment(value: &str) -> String {
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
        segment = "library".to_string();
    }
    segment
}

fn default_workspace_config_version() -> u32 {
    1
}

fn default_project_descriptor_version() -> u32 {
    1
}

fn default_project_extension_descriptor_version() -> u32 {
    1
}

fn default_workspace_library_id() -> String {
    "stdlib".to_string()
}

fn default_workspace_library_role() -> WorkspaceLibraryRole {
    WorkspaceLibraryRole::Dependency
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::Path;

    use serde_json::Value;

    use super::{
        PROJECT_EXTENSION_DESCRIPTOR_FILE_NAME, ProjectDescriptor, ProjectExtensionDescriptor,
        ResolvedWorkspaceContext, WorkspaceConfig, WorkspaceConfigError, WorkspaceContextOptions,
        WorkspaceLibraryRole, discover_project_extension_descriptor_path,
        discover_workspace_config_path, resolve_project_descriptor_context,
        resolve_workspace_context, resolve_workspace_context_with_options,
    };
    use crate::kir::{KirDocument, KirElement};

    const TEST_WORKSPACE_CONFIG_FILE_NAME: &str = "workspace.json";

    #[test]
    fn discovers_config_from_ancestor_directory_with_caller_supplied_name() {
        let root = temp_dir("discover_config");
        let nested = root.join("models").join("subsystem");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            root.join(TEST_WORKSPACE_CONFIG_FILE_NAME),
            "{\"version\":1}",
        )
        .unwrap();

        let found =
            discover_workspace_config_path(&nested, TEST_WORKSPACE_CONFIG_FILE_NAME).unwrap();

        assert_eq!(found, root.join(TEST_WORKSPACE_CONFIG_FILE_NAME));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_configless_core_context_from_kernel_baseline() {
        let root = temp_dir("workspace_context_core_default");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {}\n").unwrap();

        let resolved = resolve_workspace_context(&nested_file).unwrap();

        assert!(resolved.config_path.is_none());
        assert!(resolved.resolved_libraries.is_empty());
        assert!(resolved.library_context_document.elements.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_with_local_baseline_library_override() {
        let root = temp_dir("workspace_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let library_path = root.join("baseline.kir.json");
        let library = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::LibraryThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::from([(
                    "declared_name".to_string(),
                    Value::String("LibraryThing".to_string()),
                )]),
            }],
        };
        library.write_pretty_to_path(&library_path).unwrap();

        let config = serde_json::json!({
            "version": 1,
            "name": "Demo Project",
            "libraries": [
                {
                    "id": "custom",
                    "role": "baseline",
                    "provider": {
                        "kind": "precompiled_kir_artifact",
                        "path": library_path.display().to_string()
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert_eq!(resolved.workspace_root, root);
        assert_eq!(
            resolved.config.unwrap().name.as_deref(),
            Some("Demo Project")
        );
        assert_eq!(resolved.resolved_libraries.len(), 1);
        assert_eq!(resolved.resolved_libraries[0].id, "custom");
        assert_eq!(
            resolved.resolved_libraries[0].role,
            WorkspaceLibraryRole::Baseline
        );
        assert_eq!(
            resolved.resolved_libraries[0].source_kind,
            "precompiled_kir_artifact"
        );
        assert_eq!(resolved.library_context_document.elements.len(), 1);
        assert_eq!(
            resolved.library_context_document.elements[0].id,
            "Demo::LibraryThing"
        );
        let cache_path = resolved.resolved_libraries[0].cache_path.as_ref().unwrap();
        assert_eq!(
            cache_path,
            &root
                .join(".workspace-cache")
                .join("libraries")
                .join("custom")
                .join("document.kir.json")
        );
        assert!(cache_path.is_file());
        assert!(cache_path.with_file_name("manifest.json").is_file());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_config_defaults_version_to_one() {
        let config: WorkspaceConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config.version, 1);
        assert!(config.libraries.is_empty());
        assert!(config.plugins.is_empty());
    }

    #[test]
    fn project_descriptor_accepts_v2_dependencies_shape() {
        let descriptor: ProjectDescriptor = serde_json::from_str(
            r#"{
  "schema": "dev.mercurio.project.v2",
  "version": 2,
  "id": "org.example.sensor-system",
  "name": "Sensor System",
  "model": {
    "sourceRoots": ["model"],
    "entrypoints": ["model/sensor_system.sysml"],
    "metamodel": "sysml-2.0-metamodel-0.57.0"
  },
  "dependencies": [
    {
      "id": "stdlib",
      "role": "baseline",
      "provider": {
        "kind": "bundled_stdlib"
      }
    }
  ]
}"#,
        )
        .unwrap();

        assert_eq!(descriptor.version, 2);
        assert_eq!(descriptor.id.as_deref(), Some("org.example.sensor-system"));
        assert_eq!(descriptor.model.source_roots, vec!["model".to_string()]);
        assert_eq!(
            descriptor.model.entrypoints,
            vec!["model/sensor_system.sysml".to_string()]
        );
        assert_eq!(descriptor.dependencies.len(), 1);
        assert_eq!(descriptor.dependencies[0].id, "stdlib");
        assert_eq!(
            descriptor.dependencies[0].role,
            WorkspaceLibraryRole::Baseline
        );
    }

    #[test]
    fn project_descriptor_rejects_inline_extension_fields() {
        let err = serde_json::from_str::<ProjectDescriptor>(
            r#"{
  "id": "org.example.structural-connectivity",
  "name": "Structural Connectivity Example",
  "version": 1,
  "capabilities": [
    "plugins/structural-connectivity/mercurio.plugin.json"
  ],
  "projectPlugins": [
    "plugins/pacti-contract-analysis"
  ],
  "views": [
    {
      "label": "Simulation",
      "entry": "sim/launch.py"
    }
  ]
}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn project_descriptor_rejects_project_plugin_directories() {
        let err = serde_json::from_str::<ProjectDescriptor>(
            r#"{
  "name": "Project Plugin Pacti Analysis",
  "version": 1,
  "projectPlugins": [
    "plugins/pacti-contract-analysis"
  ]
}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn project_descriptor_path_migrates_v1_inline_extension_fields() {
        let root = temp_dir("legacy_inline_project_extensions");
        let descriptor_path = root.join(".project.json");
        std::fs::write(
            &descriptor_path,
            r#"{
  "version": 1,
  "name": "Legacy Project",
  "projectPlugins": ["plugins/domain"],
  "capabilities": ["plugins/domain/mercurio.plugin.json"],
  "views": [{"label": "Domain View", "kind": "table"}]
}"#,
        )
        .unwrap();

        let descriptor = ProjectDescriptor::from_path(&descriptor_path).unwrap();
        assert_eq!(descriptor.name.as_deref(), Some("Legacy Project"));
        let serialized = serde_json::to_value(&descriptor).unwrap();
        assert!(serialized.get("projectPlugins").is_none());
        assert!(serialized.get("capabilities").is_none());
        assert!(serialized.get("views").is_none());

        let resolved = resolve_project_descriptor_context(&descriptor_path).unwrap();
        assert!(resolved.extension_path.is_none());
        let extension = resolved.extension.unwrap();
        assert_eq!(extension.project_plugins, vec!["plugins/domain"]);
        assert_eq!(extension.capabilities.len(), 1);
        assert_eq!(extension.views.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_descriptor_path_keeps_v2_inline_extension_fields_invalid() {
        let root = temp_dir("invalid_v2_inline_project_extensions");
        let descriptor_path = root.join(".project.json");
        std::fs::write(
            &descriptor_path,
            r#"{"version":2,"name":"Modern Project","views":[]}"#,
        )
        .unwrap();

        let error = ProjectDescriptor::from_path(&descriptor_path).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn project_descriptor_accepts_libraries_dependency_alias() {
        let descriptor = serde_json::from_str::<ProjectDescriptor>(
            r#"{
  "name": "Project Plugin Pacti Analysis",
  "libraries": [
    {
      "id": "stdlib",
      "role": "baseline",
      "provider": {
        "kind": "bundled_stdlib"
      }
    }
  ]
}"#,
        )
        .unwrap();

        assert_eq!(descriptor.dependencies.len(), 1);
        assert_eq!(descriptor.dependencies[0].id, "stdlib");
        assert_eq!(
            descriptor.dependencies[0].role,
            WorkspaceLibraryRole::Baseline
        );
    }

    #[test]
    fn project_descriptor_rejects_string_version() {
        let err = serde_json::from_str::<ProjectDescriptor>(
            r#"{
  "name": "String Version",
  "version": "1"
}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid type"));
    }

    #[test]
    fn resolves_project_descriptor_dependencies_as_library_context() {
        let root = temp_dir("project_descriptor_context");
        let descriptor_path = root.join(".project.json");
        let dependency_path = root.join("dependency.kir.json");
        let dependency = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Dependency::Thing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        dependency.write_pretty_to_path(&dependency_path).unwrap();
        std::fs::write(
            &descriptor_path,
            r#"{
  "schema": "dev.mercurio.project.v2",
  "version": 2,
  "dependencies": [
    {
      "id": "dependency",
      "role": "baseline",
      "provider": {
        "kind": "precompiled_kir_artifact",
        "path": "dependency.kir.json"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let resolved = resolve_project_descriptor_context(&descriptor_path).unwrap();

        assert_eq!(resolved.workspace_root, root);
        assert_eq!(resolved.resolved_libraries.len(), 1);
        assert_eq!(resolved.resolved_libraries[0].id, "dependency");
        assert!(!resolved.library_context_document.elements.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_project_descriptor_with_adjacent_extension_descriptor() {
        let root = temp_dir("project_descriptor_extensions");
        let descriptor_path = root.join(".project.json");
        std::fs::write(
            &descriptor_path,
            r#"{
  "schema": "dev.mercurio.project.v2",
  "version": 2,
  "name": "Split Descriptor"
}"#,
        )
        .unwrap();
        let extension_path = root.join(PROJECT_EXTENSION_DESCRIPTOR_FILE_NAME);
        std::fs::write(
            &extension_path,
            r#"{
  "schema": "dev.mercurio.extensions.v1",
  "version": 1,
  "projectPlugins": ["plugins/domain"],
  "capabilities": ["plugins/domain/mercurio.plugin.json"],
  "views": [{"label": "Domain View", "kind": "table"}]
}"#,
        )
        .unwrap();

        let resolved = resolve_project_descriptor_context(&descriptor_path).unwrap();

        assert_eq!(
            discover_project_extension_descriptor_path(&root).as_deref(),
            Some(extension_path.as_path())
        );
        assert_eq!(
            resolved.extension_path.as_deref(),
            Some(extension_path.as_path())
        );
        let extension = resolved.extension.unwrap();
        assert_eq!(
            extension.project_plugins,
            vec!["plugins/domain".to_string()]
        );
        assert_eq!(
            extension.capabilities,
            vec!["plugins/domain/mercurio.plugin.json".to_string()]
        );
        assert_eq!(extension.views.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_extension_descriptor_parses_views_and_capabilities() {
        let descriptor: ProjectExtensionDescriptor = serde_json::from_str(
            r#"{
  "schema": "dev.mercurio.extensions.v1",
  "version": 1,
  "projectPlugins": ["plugins/pacti-contract-analysis"],
  "capabilities": [
    "plugins/structural-connectivity/mercurio.plugin.json"
  ],
  "views": [
    {
      "label": "Voron Print Sequence",
      "kind": "simulation",
      "entry": "sim/launch.py"
    }
  ]
}"#,
        )
        .unwrap();

        assert_eq!(descriptor.version, 1);
        assert_eq!(
            descriptor.project_plugins,
            vec!["plugins/pacti-contract-analysis".to_string()]
        );
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(descriptor.views.len(), 1);
    }

    #[test]
    fn workspace_config_accepts_plugin_pins() {
        let config: WorkspaceConfig = serde_json::from_str(
            r#"{
  "version": 1,
  "plugins": [
    {
      "id": "org.mercurio.samples.wasm-echo",
      "version": "0.1.0",
      "locator": "mpack:org.mercurio.samples.wasm-echo:0.1.0",
      "digest": "fnv1a64:sample"
    }
  ]
}"#,
        )
        .unwrap();

        assert_eq!(config.plugins.len(), 1);
        assert_eq!(config.plugins[0].id, "org.mercurio.samples.wasm-echo");
        assert_eq!(
            config.plugins[0].locator.as_deref(),
            Some("mpack:org.mercurio.samples.wasm-echo:0.1.0")
        );
    }

    #[test]
    fn workspace_config_rejects_legacy_baseline_libraries_field() {
        let err = serde_json::from_str::<WorkspaceConfig>(
            r#"{"version":1,"baseline_libraries":[],"libraries":[]}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("baseline_libraries"));
    }

    #[test]
    fn resolves_workspace_context_with_additional_library_dependencies() {
        let root = temp_dir("workspace_dependency_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let dependency_path = root.join("deps").join("library.kir.json");
        let dependency = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::DependencyThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        };
        dependency.write_pretty_to_path(&dependency_path).unwrap();

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "dep",
                    "provider": {
                        "kind": "precompiled_kir_artifact",
                        "path": "deps/library.kir.json"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "Demo::DependencyThing")
        );
        assert!(!resolved.library_context_document.elements.is_empty());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_reuses_valid_cached_library_document() {
        let root = temp_dir("workspace_reuses_cached_library");
        let model_path = root.join("demo.model");
        std::fs::write(&model_path, "package Demo {\n}\n").unwrap();

        let library_path = root.join("baseline.kir.json");
        let original_library = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::OriginalLibraryThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        original_library
            .write_pretty_to_path(&library_path)
            .unwrap();

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "custom",
                    "role": "baseline",
                    "provider": {
                        "kind": "precompiled_kir_artifact",
                        "path": "baseline.kir.json"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let first = resolve_test_workspace_context(&model_path, &config_path).unwrap();
        let cache_path = first.resolved_libraries[0].cache_path.as_ref().unwrap();
        let cached_library = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "Demo::CachedLibraryThing".to_string(),
                kind: "PartDefinition".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        cached_library.write_pretty_to_path(cache_path).unwrap();

        let second = resolve_test_workspace_context(&model_path, &config_path).unwrap();

        assert!(
            second
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "Demo::CachedLibraryThing")
        );
        assert!(
            !second
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "Demo::OriginalLibraryThing")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_with_source_backed_library_dependency() {
        let root = temp_dir("workspace_source_dependency_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let library_dir = root.join("libraries").join("domain-lib");
        std::fs::create_dir_all(&library_dir).unwrap();
        std::fs::write(
            library_dir.join("domain.model"),
            "package Domain {\n  part def Thing;\n}\n",
        )
        .unwrap();

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "domain-lib",
                    "provider": {
                        "kind": "model_directory",
                        "path": "libraries/domain-lib"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_with_core_library_dependency() {
        let root = temp_dir("workspace_core_dependency_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let library_dir = root.join("libraries").join("kernel-lib");
        std::fs::create_dir_all(&library_dir).unwrap();
        std::fs::write(
            library_dir.join("kernel.model"),
            "package Kernel {\n  feature def SemanticThing;\n}\n",
        )
        .unwrap();

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "kernel-lib",
                    "provider": {
                        "kind": "model_directory",
                        "path": "libraries/kernel-lib"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );
        assert!(resolved.resolved_libraries[0].cache_path.is_some());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_with_kpar_library_dependency() {
        let root = temp_dir("workspace_kpar_dependency_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let library_path = root.join("libraries").join("domain-lib.kpar");
        std::fs::create_dir_all(library_path.parent().unwrap()).unwrap();
        write_test_kpar(
            &library_path,
            "Domain Library",
            "1.0.0",
            &[("domain.model", "package Domain {\n  part def Thing;\n}\n")],
        );

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "domain-lib",
                    "provider": {
                        "kind": "kpar_file",
                        "path": "libraries/domain-lib.kpar"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Thing")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_workspace_context_with_package_set_library_dependency() {
        let root = temp_dir("workspace_package_set_dependency_context");
        let nested_file = root.join("models").join("demo.model");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, "package Demo {\n}\n").unwrap();

        let package_set_dir = root.join("libraries").join("model.library.kpar");
        std::fs::create_dir_all(&package_set_dir).unwrap();
        write_test_kpar_with_usage(
            &package_set_dir.join("Kernel_Semantic_Library-1.0.0.kpar"),
            "Kernel Semantic Library",
            "1.0.0",
            &[],
            &[(
                "semantic.model",
                "package Kernel {\n  part def SemanticThing;\n}\n",
            )],
        );
        write_test_kpar_with_usage(
            &package_set_dir.join("Model_Systems_Library-2.0.0.kpar"),
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

        let config = serde_json::json!({
            "version": 1,
            "libraries": [
                {
                    "id": "systems-lib",
                    "provider": {
                        "kind": "package_set_directory",
                        "path": "libraries/model.library.kpar",
                        "entry": "https://www.omg.org/spec/Model/20250201/Systems-Library.kpar"
                    }
                }
            ]
        });
        let config_path = write_test_workspace_config(&root, &config);

        let resolved = resolve_test_workspace_context(&nested_file, &config_path).unwrap();

        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "type.Kernel.SemanticThing")
        );
        assert!(
            resolved
                .library_context_document
                .elements
                .iter()
                .any(|element| element.id == "type.Systems.SystemThing")
        );
        assert!(resolved.resolved_libraries.iter().any(|library| {
            library.role == WorkspaceLibraryRole::Dependency && library.id == "systems-lib"
        }));
        assert!(
            resolved
                .resolved_libraries
                .iter()
                .any(|library| library.role == WorkspaceLibraryRole::Dependency
                    && library.id == "systems-lib"
                    && library.source_kind == "package_set_directory")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "mercurio_workspace_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_test_workspace_config(root: &Path, value: &serde_json::Value) -> std::path::PathBuf {
        let config_path = root.join(TEST_WORKSPACE_CONFIG_FILE_NAME);
        std::fs::write(&config_path, serde_json::to_string_pretty(value).unwrap()).unwrap();
        config_path
    }

    fn resolve_test_workspace_context(
        open_path: &Path,
        config_path: &Path,
    ) -> Result<ResolvedWorkspaceContext, WorkspaceConfigError> {
        resolve_workspace_context_with_options(
            open_path,
            WorkspaceContextOptions {
                config_path: Some(config_path.to_path_buf()),
                cache_root: Some(
                    config_path
                        .parent()
                        .unwrap()
                        .join(".workspace-cache")
                        .join("libraries"),
                ),
            },
        )
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

        for (entry_name, content) in entries {
            writer.start_file(*entry_name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }

        writer.finish().unwrap();
    }
}
