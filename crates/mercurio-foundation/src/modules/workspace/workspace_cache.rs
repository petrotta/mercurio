use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::language_contracts::LanguageRegistry;

use crate::authoring::source_set::{
    SourceDocument, compile_source_documents, compile_source_documents_with_registry,
};
use crate::kir::{KIR_SCHEMA_VERSION, KirDocument, KirError};
use crate::model::{Edge, Element, ElementProperties, GraphArtifact};
use crate::runtime::{DerivedIndexes, Explanation, Fact};
use crate::runtime::{Runtime, RuntimeArtifact};
use crate::semantic_services::semantic_validation::{
    SemanticValidationMode, SemanticValidationPolicy, SemanticValidationReport,
    validate_kir_semantics_with_context_and_policy,
};

const CACHE_SCHEMA_VERSION: u32 = 8;
const ARTIFACT_FAMILY_COMPILE: &str = "compile";
const DOCUMENT_FILE_NAME: &str = "document.kir.json";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const RUNTIME_CACHE_FILE_NAME: &str = "runtime.mruntime";
const RUNTIME_CACHE_MANIFEST_FILE_NAME: &str = "runtime.mruntime.manifest.json";
pub const RUNTIME_CACHE_FORMAT_VERSION: u16 = 3;

#[derive(Debug, Clone)]
pub struct PersistentWorkspaceCache {
    root: PathBuf,
    options: PersistentWorkspaceCacheOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistentWorkspaceCacheOptions {
    pub runtime_cache: RuntimeCachePolicy,
    pub validation_policy: SemanticValidationPolicy,
}

impl Default for PersistentWorkspaceCacheOptions {
    fn default() -> Self {
        Self {
            runtime_cache: RuntimeCachePolicy::ReadWrite,
            validation_policy: SemanticValidationPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCachePolicy {
    ReadWrite,
    ReadOnly,
    Disabled,
}

impl RuntimeCachePolicy {
    fn can_read(self) -> bool {
        matches!(self, Self::ReadWrite | Self::ReadOnly)
    }

    fn can_write(self) -> bool {
        matches!(self, Self::ReadWrite)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistentCacheStatus {
    FreshCompile,
    PersistentHit,
    PersistentMiss,
    PersistentRejected { reason: String },
}

#[derive(Debug, Clone)]
pub struct PersistentCompileResult {
    pub document: KirDocument,
    pub runtime_artifact: RuntimeArtifact,
    pub validation_report: SemanticValidationReport,
    pub cache_status: PersistentCacheStatus,
    pub artifact_key: String,
    pub cache_write_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSourceFileFingerprint {
    pub path: String,
    pub size_bytes: usize,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCompileArtifactKey {
    pub source_authority: String,
    pub source_tree_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_config_digest: Option<String>,
    pub compiler_digest: String,
    pub kir_schema_version: String,
    pub validation_policy_digest: String,
    pub library_context_digest: String,
    pub mapping_rules_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCompileCacheManifest {
    pub cache_schema_version: u32,
    pub artifact_family: String,
    pub artifact_key: String,
    pub key: WorkspaceCompileArtifactKey,
    pub files: Vec<WorkspaceSourceFileFingerprint>,
    pub outputs: WorkspaceCompileCacheOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCompileCacheOutputs {
    pub kir: String,
    pub runtime_cache: String,
    pub runtime_cache_manifest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_artifact: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCacheManifest {
    pub runtime_format_version: u16,
    pub kir_schema_version: String,
    pub source_digest: String,
    pub runtime_digest: String,
    pub generator: String,
    pub element_count: usize,
    pub edge_count: usize,
    pub subtype_count: usize,
    pub ownership_count: usize,
    pub inherited_feature_count: usize,
    pub requirement_count: usize,
}

struct RuntimeCacheRead {
    bytes: Vec<u8>,
    manifest: RuntimeCacheManifest,
}

enum CacheLookup {
    Hit {
        document: KirDocument,
        runtime_artifact: RuntimeArtifact,
    },
    Miss,
    Rejected(String),
}

impl PersistentWorkspaceCache {
    pub fn for_workspace_root(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            root: workspace_root
                .into()
                .join(".workspace-cache")
                .join("compile"),
            options: PersistentWorkspaceCacheOptions::default(),
        }
    }

    pub fn from_cache_root(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            options: PersistentWorkspaceCacheOptions::default(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn options(&self) -> PersistentWorkspaceCacheOptions {
        self.options
    }

    pub fn with_options(mut self, options: PersistentWorkspaceCacheOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_semantic_validation_policy(mut self, policy: SemanticValidationPolicy) -> Self {
        self.options.validation_policy = policy;
        self
    }

    pub fn without_runtime_cache_writes(mut self) -> Self {
        self.options.runtime_cache = RuntimeCachePolicy::ReadOnly;
        self
    }

    pub fn without_runtime_cache(mut self) -> Self {
        self.options.runtime_cache = RuntimeCachePolicy::Disabled;
        self
    }

    pub fn compile_source_documents(
        &self,
        source_documents: Vec<SourceDocument>,
        library_context: &KirDocument,
        workspace_config_path: Option<&Path>,
    ) -> Result<PersistentCompileResult, KirError> {
        self.compile_source_documents_with(
            source_documents,
            library_context,
            workspace_config_path,
            |source_documents, library_context| {
                compile_source_documents(source_documents, library_context)
            },
        )
    }

    pub fn compile_source_documents_with_registry(
        &self,
        source_documents: Vec<SourceDocument>,
        library_context: &KirDocument,
        workspace_config_path: Option<&Path>,
        registry: &LanguageRegistry,
    ) -> Result<PersistentCompileResult, KirError> {
        self.compile_source_documents_with(
            source_documents,
            library_context,
            workspace_config_path,
            |source_documents, library_context| {
                compile_source_documents_with_registry(source_documents, library_context, registry)
            },
        )
    }

    fn compile_source_documents_with(
        &self,
        source_documents: Vec<SourceDocument>,
        library_context: &KirDocument,
        workspace_config_path: Option<&Path>,
        compile: impl Fn(Vec<SourceDocument>, &KirDocument) -> Result<KirDocument, KirError>,
    ) -> Result<PersistentCompileResult, KirError> {
        let (key, files) = workspace_compile_artifact_key(
            &source_documents,
            library_context,
            workspace_config_path,
            self.options.validation_policy,
        )?;
        let artifact_key = artifact_key_digest(&key)?;

        if !self.options.runtime_cache.can_read() {
            let document = compile(source_documents, library_context)?;
            let runtime_artifact = runtime_artifact_for_document(&document, library_context)?;
            let validation_report = self.validate_kir_semantics(&document, library_context)?;
            return Ok(PersistentCompileResult {
                document,
                runtime_artifact,
                validation_report,
                cache_status: PersistentCacheStatus::FreshCompile,
                artifact_key,
                cache_write_error: None,
            });
        }

        match self.load_compile_artifact(&artifact_key, &key, &files)? {
            CacheLookup::Hit {
                document,
                runtime_artifact,
            } => {
                let validation_report = self.validate_kir_semantics(&document, library_context)?;
                return Ok(PersistentCompileResult {
                    document,
                    runtime_artifact,
                    validation_report,
                    cache_status: PersistentCacheStatus::PersistentHit,
                    artifact_key,
                    cache_write_error: None,
                });
            }
            CacheLookup::Miss => {
                let document = compile(source_documents, library_context)?;
                let runtime_artifact = runtime_artifact_for_document(&document, library_context)?;
                let validation_report = self.validate_kir_semantics(&document, library_context)?;
                let cache_write_error = self.write_compile_artifact_if_enabled(
                    &artifact_key,
                    &key,
                    &files,
                    &document,
                    &runtime_artifact,
                );
                return Ok(PersistentCompileResult {
                    document,
                    runtime_artifact,
                    validation_report,
                    cache_status: PersistentCacheStatus::PersistentMiss,
                    artifact_key,
                    cache_write_error,
                });
            }
            CacheLookup::Rejected(reason) => {
                let document = compile(source_documents, library_context)?;
                let runtime_artifact = runtime_artifact_for_document(&document, library_context)?;
                let validation_report = self.validate_kir_semantics(&document, library_context)?;
                let cache_write_error = self.write_compile_artifact_if_enabled(
                    &artifact_key,
                    &key,
                    &files,
                    &document,
                    &runtime_artifact,
                );
                return Ok(PersistentCompileResult {
                    document,
                    runtime_artifact,
                    validation_report,
                    cache_status: PersistentCacheStatus::PersistentRejected { reason },
                    artifact_key,
                    cache_write_error,
                });
            }
        }
    }

    fn validate_kir_semantics(
        &self,
        document: &KirDocument,
        library_context: &KirDocument,
    ) -> Result<SemanticValidationReport, KirError> {
        let policy = self.options.validation_policy;
        let report = validate_kir_semantics_with_context_and_policy(
            document,
            Some(library_context),
            policy,
        )?;
        if policy.mode == SemanticValidationMode::Error && report.has_errors() {
            return Err(KirError::Model(format!(
                "semantic validation failed with {} error diagnostics",
                report.error_count()
            )));
        }
        Ok(report)
    }

    fn write_compile_artifact_if_enabled(
        &self,
        artifact_key: &str,
        key: &WorkspaceCompileArtifactKey,
        files: &[WorkspaceSourceFileFingerprint],
        document: &KirDocument,
        runtime_artifact: &RuntimeArtifact,
    ) -> Option<String> {
        self.options.runtime_cache.can_write().then(|| {
            self.write_compile_artifact(artifact_key, key, files, document, runtime_artifact)
                .err()
                .map(|err| err.to_string())
        })?
    }

    fn artifact_dir(&self, artifact_key: &str) -> PathBuf {
        self.root
            .join("artifacts")
            .join(safe_cache_segment(artifact_key))
    }

    fn load_compile_artifact(
        &self,
        artifact_key: &str,
        key: &WorkspaceCompileArtifactKey,
        files: &[WorkspaceSourceFileFingerprint],
    ) -> Result<CacheLookup, KirError> {
        let artifact_dir = self.artifact_dir(artifact_key);
        let manifest_path = artifact_dir.join(MANIFEST_FILE_NAME);
        let document_path = artifact_dir.join(DOCUMENT_FILE_NAME);
        let runtime_cache_path = artifact_dir.join(RUNTIME_CACHE_FILE_NAME);
        let runtime_cache_manifest_path = artifact_dir.join(RUNTIME_CACHE_MANIFEST_FILE_NAME);

        if !manifest_path.is_file() {
            return Ok(CacheLookup::Miss);
        }

        let manifest = match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|input| serde_json::from_str::<WorkspaceCompileCacheManifest>(&input).ok())
        {
            Some(manifest) => manifest,
            None => return Ok(CacheLookup::Rejected("manifest is unreadable".to_string())),
        };

        if let Some(reason) = manifest_rejection_reason(&manifest, artifact_key, key, files) {
            return Ok(CacheLookup::Rejected(reason));
        }

        let cache_source = cache_source_bytes(key, files)?;
        if !document_path.is_file() {
            return Ok(CacheLookup::Rejected(
                "cached KIR text is missing".to_string(),
            ));
        }
        let document = match KirDocument::from_path(&document_path) {
            Ok(document) => document,
            Err(err) => {
                return Ok(CacheLookup::Rejected(format!(
                    "cached KIR is invalid: {err}"
                )));
            }
        };
        let runtime_artifact = match load_runtime_artifact_cache(
            &runtime_cache_path,
            &runtime_cache_manifest_path,
            &cache_source,
        )? {
            Some(artifact) => artifact,
            None => {
                return Ok(CacheLookup::Rejected(
                    "cached runtime is missing or invalid".to_string(),
                ));
            }
        };
        Ok(CacheLookup::Hit {
            document,
            runtime_artifact,
        })
    }

    fn write_compile_artifact(
        &self,
        artifact_key: &str,
        key: &WorkspaceCompileArtifactKey,
        files: &[WorkspaceSourceFileFingerprint],
        document: &KirDocument,
        runtime_artifact: &RuntimeArtifact,
    ) -> Result<(), KirError> {
        let final_dir = self.artifact_dir(artifact_key);
        if final_dir.is_dir() {
            return self.write_compile_artifact_files(
                &final_dir,
                artifact_key,
                key,
                files,
                document,
                runtime_artifact,
            );
        }

        let tmp_dir = self.root.join("tmp").join(format!(
            "{}-{}",
            process_id_segment(),
            unique_cache_nonce()
        ));
        std::fs::create_dir_all(&tmp_dir)?;

        self.write_compile_artifact_files(
            &tmp_dir,
            artifact_key,
            key,
            files,
            document,
            runtime_artifact,
        )?;

        if let Some(parent) = final_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::rename(&tmp_dir, &final_dir) {
            Ok(()) => Ok(()),
            Err(err) if final_dir.is_dir() => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Ok(())
                }
            }
            Err(err) => Err(KirError::Io(err)),
        }
    }

    fn write_compile_artifact_files(
        &self,
        dir: &Path,
        artifact_key: &str,
        key: &WorkspaceCompileArtifactKey,
        files: &[WorkspaceSourceFileFingerprint],
        document: &KirDocument,
        runtime_artifact: &RuntimeArtifact,
    ) -> Result<(), KirError> {
        std::fs::create_dir_all(dir)?;
        let manifest = WorkspaceCompileCacheManifest {
            cache_schema_version: CACHE_SCHEMA_VERSION,
            artifact_family: ARTIFACT_FAMILY_COMPILE.to_string(),
            artifact_key: artifact_key.to_string(),
            key: key.clone(),
            files: files.to_vec(),
            outputs: WorkspaceCompileCacheOutputs {
                kir: DOCUMENT_FILE_NAME.to_string(),
                runtime_cache: RUNTIME_CACHE_FILE_NAME.to_string(),
                runtime_cache_manifest: RUNTIME_CACHE_MANIFEST_FILE_NAME.to_string(),
                runtime_artifact: None,
            },
        };
        document.write_pretty_to_path(&dir.join(DOCUMENT_FILE_NAME))?;
        let cache_source = cache_source_bytes(key, files)?;
        write_runtime_artifact_cache(
            &dir.join(RUNTIME_CACHE_FILE_NAME),
            &dir.join(RUNTIME_CACHE_MANIFEST_FILE_NAME),
            runtime_artifact,
            &cache_source,
        )?;
        std::fs::write(
            dir.join(MANIFEST_FILE_NAME),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        let roundtrip_manifest: WorkspaceCompileCacheManifest =
            serde_json::from_str(&std::fs::read_to_string(dir.join(MANIFEST_FILE_NAME))?)?;
        if roundtrip_manifest != manifest {
            return Err(KirError::Model(
                "persistent cache manifest failed roundtrip validation".to_string(),
            ));
        }
        KirDocument::from_path(&dir.join(DOCUMENT_FILE_NAME))?;
        if load_runtime_artifact_cache(
            &dir.join(RUNTIME_CACHE_FILE_NAME),
            &dir.join(RUNTIME_CACHE_MANIFEST_FILE_NAME),
            &cache_source,
        )?
        .is_none()
        {
            return Err(KirError::Model(
                "persistent cache runtime artifact failed manifest validation".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn workspace_compile_artifact_key(
    source_documents: &[SourceDocument],
    library_context: &KirDocument,
    workspace_config_path: Option<&Path>,
    validation_policy: SemanticValidationPolicy,
) -> Result<
    (
        WorkspaceCompileArtifactKey,
        Vec<WorkspaceSourceFileFingerprint>,
    ),
    KirError,
> {
    let files = source_file_fingerprints(source_documents);
    let source_tree_digest = digest_source_file_fingerprints(&files);
    let workspace_config_digest = workspace_config_path
        .filter(|path| path.is_file())
        .map(digest_file)
        .transpose()?;
    let library_context_digest = digest_json(library_context)?;
    let mapping_rules_digest = mapping_rules_digest()?;

    Ok((
        WorkspaceCompileArtifactKey {
            source_authority: "local_files".to_string(),
            source_tree_digest,
            workspace_config_digest,
            compiler_digest: compiler_digest(),
            kir_schema_version: KIR_SCHEMA_VERSION.to_string(),
            validation_policy_digest: semantic_validation_policy_digest(validation_policy),
            library_context_digest,
            mapping_rules_digest,
        },
        files,
    ))
}

pub fn source_file_fingerprints(
    source_documents: &[SourceDocument],
) -> Vec<WorkspaceSourceFileFingerprint> {
    let mut files = source_documents
        .iter()
        .map(|source| WorkspaceSourceFileFingerprint {
            path: normalized_source_path(&source.path),
            size_bytes: source.content.len(),
            content_digest: digest_labeled_chunks([(
                "content".as_bytes(),
                source.content.as_bytes(),
            )]),
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

fn manifest_rejection_reason(
    manifest: &WorkspaceCompileCacheManifest,
    artifact_key: &str,
    key: &WorkspaceCompileArtifactKey,
    files: &[WorkspaceSourceFileFingerprint],
) -> Option<String> {
    if manifest.cache_schema_version != CACHE_SCHEMA_VERSION {
        return Some("cache schema version changed".to_string());
    }
    if manifest.artifact_family != ARTIFACT_FAMILY_COMPILE {
        return Some("artifact family does not match compile cache".to_string());
    }
    if manifest.artifact_key != artifact_key {
        return Some("artifact key does not match manifest location".to_string());
    }
    if &manifest.key != key {
        return Some("artifact key inputs changed".to_string());
    }
    if manifest.files != files {
        return Some("source file fingerprints changed".to_string());
    }
    if manifest.outputs.kir != DOCUMENT_FILE_NAME {
        return Some("manifest output path is not recognized".to_string());
    }
    if manifest.outputs.runtime_cache != RUNTIME_CACHE_FILE_NAME {
        return Some("manifest runtime cache path is not recognized".to_string());
    }
    if manifest.outputs.runtime_cache_manifest != RUNTIME_CACHE_MANIFEST_FILE_NAME {
        return Some("manifest runtime cache manifest path is not recognized".to_string());
    }
    None
}

pub fn write_runtime_artifact_cache(
    runtime_path: &Path,
    manifest_path: &Path,
    runtime_artifact: &RuntimeArtifact,
    source_bytes: &[u8],
) -> Result<(), KirError> {
    if let Some(parent) = runtime_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let runtime_bytes = runtime_artifact_to_binary_bytes(runtime_artifact)?;
    let manifest = RuntimeCacheManifest {
        runtime_format_version: RUNTIME_CACHE_FORMAT_VERSION,
        kir_schema_version: KIR_SCHEMA_VERSION.to_string(),
        source_digest: digest_labeled_chunks([("source".as_bytes(), source_bytes)]),
        runtime_digest: digest_labeled_chunks([("runtime".as_bytes(), runtime_bytes.as_slice())]),
        generator: format!("mercurio-foundation/{}", env!("CARGO_PKG_VERSION")),
        element_count: runtime_artifact.graph.elements.len(),
        edge_count: runtime_artifact.graph.edges.len(),
        subtype_count: runtime_artifact.derived.subtypes.len(),
        ownership_count: runtime_artifact.derived.ownership.len(),
        inherited_feature_count: runtime_artifact.derived.inherited_features.len(),
        requirement_count: runtime_artifact.derived.requirements.len(),
    };
    std::fs::write(runtime_path, runtime_bytes)?;
    std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

pub fn load_runtime_artifact_cache(
    runtime_path: &Path,
    manifest_path: &Path,
    source_bytes: &[u8],
) -> Result<Option<RuntimeArtifact>, KirError> {
    load_runtime_artifact_cache_with_rejection_reason(runtime_path, manifest_path, source_bytes)
        .map(|result| result.ok())
}

pub fn load_runtime_artifact_cache_with_rejection_reason(
    runtime_path: &Path,
    manifest_path: &Path,
    source_bytes: &[u8],
) -> Result<Result<RuntimeArtifact, String>, KirError> {
    let Some(read) = read_runtime_cache_files(runtime_path, manifest_path)? else {
        return Ok(Err("runtime artifact or manifest is missing".to_string()));
    };
    if !runtime_cache_manifest_matches(&read.manifest, &read.bytes, source_bytes) {
        return Ok(Err(runtime_cache_manifest_rejection_reason(
            &read.manifest,
            &read.bytes,
            source_bytes,
        )));
    }
    let runtime_artifact = match runtime_artifact_from_binary_bytes(&read.bytes) {
        Ok(artifact) => artifact,
        Err(err) => return Ok(Err(format!("runtime artifact decode failed: {err}"))),
    };
    if !runtime_cache_counts_match(&read.manifest, &runtime_artifact) {
        return Ok(Err(
            "runtime artifact counts do not match manifest".to_string()
        ));
    }
    Ok(Ok(runtime_artifact))
}

fn read_runtime_cache_files(
    runtime_path: &Path,
    manifest_path: &Path,
) -> Result<Option<RuntimeCacheRead>, KirError> {
    let runtime_bytes = match std::fs::read(runtime_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(KirError::Io(err)),
    };
    let manifest_bytes = match std::fs::read(manifest_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(KirError::Io(err)),
    };
    let manifest: RuntimeCacheManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };
    Ok(Some(RuntimeCacheRead {
        bytes: runtime_bytes,
        manifest,
    }))
}

fn runtime_cache_manifest_matches(
    manifest: &RuntimeCacheManifest,
    runtime_bytes: &[u8],
    source_bytes: &[u8],
) -> bool {
    manifest.runtime_format_version == RUNTIME_CACHE_FORMAT_VERSION
        && manifest.kir_schema_version == KIR_SCHEMA_VERSION
        && manifest.source_digest == digest_labeled_chunks([("source".as_bytes(), source_bytes)])
        && manifest.runtime_digest == digest_labeled_chunks([("runtime".as_bytes(), runtime_bytes)])
}

fn runtime_cache_manifest_rejection_reason(
    manifest: &RuntimeCacheManifest,
    runtime_bytes: &[u8],
    source_bytes: &[u8],
) -> String {
    if manifest.runtime_format_version != RUNTIME_CACHE_FORMAT_VERSION {
        return format!(
            "runtime format version mismatch: manifest={} expected={}",
            manifest.runtime_format_version, RUNTIME_CACHE_FORMAT_VERSION
        );
    }
    if manifest.kir_schema_version != KIR_SCHEMA_VERSION {
        return format!(
            "KIR schema version mismatch: manifest={} expected={}",
            manifest.kir_schema_version, KIR_SCHEMA_VERSION
        );
    }
    let expected_source_digest = digest_labeled_chunks([("source".as_bytes(), source_bytes)]);
    if manifest.source_digest != expected_source_digest {
        return format!(
            "source digest mismatch: manifest={} expected={}",
            manifest.source_digest, expected_source_digest
        );
    }
    let expected_runtime_digest = digest_labeled_chunks([("runtime".as_bytes(), runtime_bytes)]);
    if manifest.runtime_digest != expected_runtime_digest {
        return format!(
            "runtime digest mismatch: manifest={} expected={}",
            manifest.runtime_digest, expected_runtime_digest
        );
    }
    "runtime artifact manifest is not valid".to_string()
}

fn runtime_cache_counts_match(
    manifest: &RuntimeCacheManifest,
    runtime_artifact: &RuntimeArtifact,
) -> bool {
    runtime_artifact.graph.elements.len() == manifest.element_count
        && runtime_artifact.graph.edges.len() == manifest.edge_count
        && runtime_artifact.derived.subtypes.len() == manifest.subtype_count
        && runtime_artifact.derived.ownership.len() == manifest.ownership_count
        && runtime_artifact.derived.inherited_features.len() == manifest.inherited_feature_count
        && runtime_artifact.derived.requirements.len() == manifest.requirement_count
}

pub fn runtime_artifact_to_binary_bytes(
    runtime_artifact: &RuntimeArtifact,
) -> Result<Vec<u8>, KirError> {
    let payload = compact_runtime_payload_to_bytes(runtime_artifact)?;
    let mut bytes = Vec::with_capacity(10 + payload.len());
    bytes.extend_from_slice(b"MRUN");
    bytes.extend_from_slice(&RUNTIME_CACHE_FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(payload.len())
            .map_err(|_| KirError::Model("runtime artifact payload exceeds u32".to_string()))?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub fn runtime_artifact_from_binary_bytes(bytes: &[u8]) -> Result<RuntimeArtifact, KirError> {
    if bytes.len() < 10 || &bytes[0..4] != b"MRUN" {
        return Err(KirError::Model("invalid runtime cache header".to_string()));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != RUNTIME_CACHE_FORMAT_VERSION {
        return Err(KirError::Model(format!(
            "unsupported runtime cache version {version}"
        )));
    }
    let payload_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
    let payload = bytes
        .get(10..10 + payload_len)
        .ok_or_else(|| KirError::Model("truncated runtime cache payload".to_string()))?;
    if bytes.len() != 10 + payload_len {
        return Err(KirError::Model("trailing runtime cache bytes".to_string()));
    }
    compact_runtime_payload_from_bytes(payload)
}

fn compact_runtime_payload_to_bytes(runtime: &RuntimeArtifact) -> Result<Vec<u8>, KirError> {
    let mut strings = StringTableBuilder::default();
    for element in &runtime.graph.elements {
        strings.intern(&element.element_id)?;
        strings.intern(element.kind.as_ref())?;
        for key in element.properties.keys() {
            strings.intern(key.as_ref())?;
        }
    }
    for edge in &runtime.graph.edges {
        strings.intern(edge.relation.as_ref())?;
    }
    for (left, right) in &runtime.derived.subtypes {
        strings.intern(left)?;
        strings.intern(right)?;
    }
    for (left, right) in &runtime.derived.ownership {
        strings.intern(left)?;
        strings.intern(right)?;
    }
    for (left, right) in &runtime.derived.inherited_features {
        strings.intern(left)?;
        strings.intern(right)?;
    }
    for value in &runtime.derived.requirements {
        strings.intern(value)?;
    }
    for (key, values) in &runtime.derived.satisfied_by {
        strings.intern(key)?;
        for value in values {
            strings.intern(value)?;
        }
    }
    for (key, values) in &runtime.derived.verified_by {
        strings.intern(key)?;
        for value in values {
            strings.intern(value)?;
        }
    }

    let mut writer = BinaryWriter::new();
    writer.write_string_table(strings.values())?;
    write_graph_records(&mut writer, &strings, &runtime.graph)?;
    write_string_pair_set(&mut writer, &strings, &runtime.derived.subtypes)?;
    write_string_pair_set(&mut writer, &strings, &runtime.derived.ownership)?;
    write_string_pair_set(&mut writer, &strings, &runtime.derived.inherited_features)?;
    write_string_set(&mut writer, &strings, &runtime.derived.requirements)?;
    write_string_set_map(&mut writer, &strings, &runtime.derived.satisfied_by)?;
    write_string_set_map(&mut writer, &strings, &runtime.derived.verified_by)?;
    let diagnostic_section = diagnostic_payload_to_bytes(runtime.derived.explanations())?;
    writer.write_bytes(&diagnostic_section, "runtime diagnostic section length")?;
    Ok(writer.into_bytes())
}

fn compact_runtime_payload_from_bytes(bytes: &[u8]) -> Result<RuntimeArtifact, KirError> {
    let mut reader = BinaryReader::new(bytes);
    let strings = reader.read_string_table()?;
    let graph = read_graph_records(&mut reader, &strings)?;
    let mut derived = DerivedIndexes::default();
    derived.subtypes = read_string_pair_set(&mut reader, &strings)?;
    derived.ownership = read_string_pair_set(&mut reader, &strings)?;
    derived.inherited_features = read_string_pair_set(&mut reader, &strings)?;
    derived.requirements = read_string_set(&mut reader, &strings)?;
    derived.satisfied_by = read_string_set_map(&mut reader, &strings)?;
    derived.verified_by = read_string_set_map(&mut reader, &strings)?;
    let _diagnostic_section = reader.read_bytes("runtime diagnostic section")?;
    reader.finish()?;
    Ok(RuntimeArtifact { graph, derived })
}

fn diagnostic_payload_to_bytes(
    explanations: &BTreeMap<Fact, Explanation>,
) -> Result<Vec<u8>, KirError> {
    let mut strings = StringTableBuilder::default();
    for (fact, explanation) in explanations {
        intern_fact_strings(&mut strings, fact)?;
        strings.intern(&explanation.rule_id)?;
        for source_fact in &explanation.source_facts {
            intern_fact_strings(&mut strings, source_fact)?;
        }
    }

    let mut writer = BinaryWriter::new();
    writer.write_string_table(strings.values())?;
    write_explanations(&mut writer, &strings, explanations)?;
    Ok(writer.into_bytes())
}

#[cfg(test)]
fn diagnostic_payload_from_bytes(bytes: &[u8]) -> Result<BTreeMap<Fact, Explanation>, KirError> {
    let mut reader = BinaryReader::new(bytes);
    let strings = reader.read_string_table()?;
    let explanations = read_explanations(&mut reader, &strings)?;
    reader.finish()?;
    Ok(explanations)
}

fn intern_fact_strings(strings: &mut StringTableBuilder, fact: &Fact) -> Result<(), KirError> {
    strings.intern(&fact.predicate)?;
    for term in &fact.terms {
        strings.intern(term)?;
    }
    Ok(())
}

fn write_graph_records(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    graph: &GraphArtifact,
) -> Result<(), KirError> {
    writer.write_len(graph.elements.len(), "graph element count")?;
    for element in &graph.elements {
        writer.write_u32(element.id);
        writer.write_u32(strings.index_of(&element.element_id)?);
        writer.write_u32(strings.index_of(element.kind.as_ref())?);
        writer.write_u8(element.layer);
        writer.write_len(element.properties.len(), "element property count")?;
        for (key, value) in element.properties.iter() {
            writer.write_u32(strings.index_of(key.as_ref())?);
            writer.write_json_value(value)?;
        }
    }
    writer.write_len(graph.edges.len(), "graph edge count")?;
    for edge in &graph.edges {
        writer.write_u32(edge.source);
        writer.write_u32(edge.target);
        writer.write_u32(strings.index_of(edge.relation.as_ref())?);
    }
    Ok(())
}

fn read_graph_records(
    reader: &mut BinaryReader<'_>,
    strings: &[Arc<str>],
) -> Result<GraphArtifact, KirError> {
    let element_count = reader.read_len("graph element count")?;
    let mut elements = Vec::with_capacity(element_count);
    for _ in 0..element_count {
        let id = reader.read_u32()?;
        let element_id = reader.read_string_ref(strings)?.to_string();
        let kind = reader.read_string_arc(strings)?;
        let layer = reader.read_u8()?;
        let property_count = reader.read_len("element property count")?;
        let mut properties = BTreeMap::new();
        for _ in 0..property_count {
            let key = reader.read_string_arc(strings)?;
            let value = reader.read_json_value()?;
            properties.insert(key, value);
        }
        elements.push(Element {
            id,
            element_id: element_id.clone(),
            kind,
            layer,
            properties: ElementProperties::from_declared_arc_for_artifact(element_id, properties),
        });
    }
    let edge_count = reader.read_len("graph edge count")?;
    let mut edges = Vec::with_capacity(edge_count);
    for _ in 0..edge_count {
        edges.push(Edge {
            source: reader.read_u32()?,
            target: reader.read_u32()?,
            relation: reader.read_string_arc(strings)?,
        });
    }
    Ok(GraphArtifact { elements, edges })
}

fn write_string_pair_set(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    values: &BTreeSet<(String, String)>,
) -> Result<(), KirError> {
    writer.write_len(values.len(), "string pair set count")?;
    for (left, right) in values {
        writer.write_u32(strings.index_of(left)?);
        writer.write_u32(strings.index_of(right)?);
    }
    Ok(())
}

fn read_string_pair_set(
    reader: &mut BinaryReader<'_>,
    strings: &[Arc<str>],
) -> Result<BTreeSet<(String, String)>, KirError> {
    let count = reader.read_len("string pair set count")?;
    let mut values = BTreeSet::new();
    for _ in 0..count {
        values.insert((
            reader.read_string_ref(strings)?.to_string(),
            reader.read_string_ref(strings)?.to_string(),
        ));
    }
    Ok(values)
}

fn write_string_set(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    values: &BTreeSet<String>,
) -> Result<(), KirError> {
    writer.write_len(values.len(), "string set count")?;
    for value in values {
        writer.write_u32(strings.index_of(value)?);
    }
    Ok(())
}

fn read_string_set(
    reader: &mut BinaryReader<'_>,
    strings: &[Arc<str>],
) -> Result<BTreeSet<String>, KirError> {
    let count = reader.read_len("string set count")?;
    let mut values = BTreeSet::new();
    for _ in 0..count {
        values.insert(reader.read_string_ref(strings)?.to_string());
    }
    Ok(values)
}

fn write_string_set_map(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    values: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), KirError> {
    writer.write_len(values.len(), "string set map count")?;
    for (key, set) in values {
        writer.write_u32(strings.index_of(key)?);
        write_string_set(writer, strings, set)?;
    }
    Ok(())
}

fn read_string_set_map(
    reader: &mut BinaryReader<'_>,
    strings: &[Arc<str>],
) -> Result<BTreeMap<String, BTreeSet<String>>, KirError> {
    let count = reader.read_len("string set map count")?;
    let mut values = BTreeMap::new();
    for _ in 0..count {
        let key = reader.read_string_ref(strings)?.to_string();
        let set = read_string_set(reader, strings)?;
        values.insert(key, set);
    }
    Ok(values)
}

fn write_explanations(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    explanations: &BTreeMap<Fact, Explanation>,
) -> Result<(), KirError> {
    writer.write_len(explanations.len(), "explanation count")?;
    for (fact, explanation) in explanations {
        write_fact(writer, strings, fact)?;
        writer.write_u32(strings.index_of(&explanation.rule_id)?);
        writer.write_len(explanation.source_facts.len(), "source fact count")?;
        for source_fact in &explanation.source_facts {
            write_fact(writer, strings, source_fact)?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn read_explanations(
    reader: &mut BinaryReader<'_>,
    strings: &[Arc<str>],
) -> Result<BTreeMap<Fact, Explanation>, KirError> {
    let count = reader.read_len("explanation count")?;
    let mut explanations = BTreeMap::new();
    for _ in 0..count {
        let fact = read_fact(reader, strings)?;
        let rule_id = reader.read_string_ref(strings)?.to_string();
        let source_fact_count = reader.read_len("source fact count")?;
        let mut source_facts = Vec::with_capacity(source_fact_count);
        for _ in 0..source_fact_count {
            source_facts.push(read_fact(reader, strings)?);
        }
        explanations.insert(
            fact,
            Explanation {
                rule_id,
                source_facts,
            },
        );
    }
    Ok(explanations)
}

fn write_fact(
    writer: &mut BinaryWriter,
    strings: &StringTableBuilder,
    fact: &Fact,
) -> Result<(), KirError> {
    writer.write_u32(strings.index_of(&fact.predicate)?);
    writer.write_len(fact.terms.len(), "fact term count")?;
    for term in &fact.terms {
        writer.write_u32(strings.index_of(term)?);
    }
    Ok(())
}

#[cfg(test)]
fn read_fact(reader: &mut BinaryReader<'_>, strings: &[Arc<str>]) -> Result<Fact, KirError> {
    let predicate = reader.read_string_ref(strings)?.to_string();
    let term_count = reader.read_len("fact term count")?;
    let mut terms = Vec::with_capacity(term_count);
    for _ in 0..term_count {
        terms.push(reader.read_string_ref(strings)?.to_string());
    }
    Ok(Fact { predicate, terms })
}

#[derive(Default)]
struct StringTableBuilder {
    by_value: HashMap<String, u32>,
    values: Vec<String>,
}

impl StringTableBuilder {
    fn intern(&mut self, value: &str) -> Result<u32, KirError> {
        if let Some(index) = self.by_value.get(value) {
            return Ok(*index);
        }
        let index = u32::try_from(self.values.len())
            .map_err(|_| KirError::Model("string table exceeds u32".to_string()))?;
        self.values.push(value.to_string());
        self.by_value.insert(value.to_string(), index);
        Ok(index)
    }

    fn index_of(&self, value: &str) -> Result<u32, KirError> {
        self.by_value
            .get(value)
            .copied()
            .ok_or_else(|| KirError::Model(format!("missing string table value `{value}`")))
    }

    fn values(&self) -> &[String] {
        &self.values
    }
}

struct BinaryWriter {
    bytes: Vec<u8>,
}

impl BinaryWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_len(&mut self, value: usize, label: &str) -> Result<(), KirError> {
        let value =
            u32::try_from(value).map_err(|_| KirError::Model(format!("{label} exceeds u32")))?;
        self.write_u32(value);
        Ok(())
    }

    fn write_bytes(&mut self, value: &[u8], label: &str) -> Result<(), KirError> {
        self.write_len(value.len(), label)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn write_string_table(&mut self, values: &[String]) -> Result<(), KirError> {
        self.write_len(values.len(), "string table count")?;
        for value in values {
            self.write_bytes(value.as_bytes(), "string table value length")?;
        }
        Ok(())
    }

    fn write_json_value(&mut self, value: &serde_json::Value) -> Result<(), KirError> {
        let bytes = serde_json::to_vec(value)?;
        self.write_bytes(&bytes, "JSON value length")
    }
}

struct BinaryReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> Result<(), KirError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(KirError::Model("trailing compact cache bytes".to_string()))
        }
    }

    fn read_u8(&mut self) -> Result<u8, KirError> {
        let value = *self
            .bytes
            .get(self.offset)
            .ok_or_else(|| KirError::Model("truncated compact cache u8".to_string()))?;
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, KirError> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or_else(|| KirError::Model("u32 length overflows".to_string()))?;
        if end > self.bytes.len() {
            return Err(KirError::Model("truncated compact cache u32".to_string()));
        }
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_len(&mut self, label: &str) -> Result<usize, KirError> {
        let value = self.read_u32()?;
        usize::try_from(value).map_err(|_| KirError::Model(format!("{label} exceeds usize")))
    }

    fn read_exact(&mut self, len: usize, label: &str) -> Result<&'a [u8], KirError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| KirError::Model(format!("{label} length overflows")))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| KirError::Model(format!("truncated compact cache {label}")))?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_bytes(&mut self, label: &str) -> Result<&'a [u8], KirError> {
        let len = self.read_len(label)?;
        self.read_exact(len, label)
    }

    fn read_string_table(&mut self) -> Result<Vec<Arc<str>>, KirError> {
        let count = self.read_len("string table count")?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes = self.read_bytes("string table value")?;
            let value = std::str::from_utf8(bytes)
                .map_err(|err| KirError::Model(format!("invalid string table utf8: {err}")))?;
            values.push(Arc::<str>::from(value));
        }
        Ok(values)
    }

    fn read_string_ref<'b>(&mut self, strings: &'b [Arc<str>]) -> Result<&'b str, KirError> {
        let index = self.read_u32()? as usize;
        strings
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| KirError::Model(format!("invalid string table index {index}")))
    }

    fn read_string_arc(&mut self, strings: &[Arc<str>]) -> Result<Arc<str>, KirError> {
        let index = self.read_u32()? as usize;
        strings
            .get(index)
            .cloned()
            .ok_or_else(|| KirError::Model(format!("invalid string table index {index}")))
    }

    fn read_json_value(&mut self) -> Result<serde_json::Value, KirError> {
        let bytes = self.read_bytes("JSON value")?;
        Ok(serde_json::from_slice(bytes)?)
    }
}

fn cache_source_bytes(
    key: &WorkspaceCompileArtifactKey,
    files: &[WorkspaceSourceFileFingerprint],
) -> Result<Vec<u8>, KirError> {
    Ok(serde_json::to_vec(&(key, files))?)
}

fn runtime_artifact_for_document(
    document: &KirDocument,
    library_context: &KirDocument,
) -> Result<RuntimeArtifact, KirError> {
    let merged_document = KirDocument::merge([library_context.clone(), document.clone()])?;
    Runtime::from_document(merged_document)
        .map(Runtime::into_artifact)
        .map_err(|err| KirError::Model(format!("failed to build runtime artifact: {err}")))
}

fn artifact_key_digest(key: &WorkspaceCompileArtifactKey) -> Result<String, KirError> {
    digest_json(key)
}

fn semantic_validation_policy_digest(policy: SemanticValidationPolicy) -> String {
    let cache_key = policy.cache_key();
    digest_labeled_chunks([(
        "semantic_validation_policy".as_bytes(),
        cache_key.as_bytes(),
    )])
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, KirError> {
    Ok(digest_labeled_chunks([(
        "json".as_bytes(),
        serde_json::to_vec(value)?.as_slice(),
    )]))
}

fn digest_file(path: &Path) -> Result<String, KirError> {
    let bytes = std::fs::read(path)?;
    Ok(digest_labeled_chunks([(
        "file".as_bytes(),
        bytes.as_slice(),
    )]))
}

fn digest_source_file_fingerprints(files: &[WorkspaceSourceFileFingerprint]) -> String {
    digest_labeled_chunks(files.iter().flat_map(|file| {
        [
            ("path".as_bytes(), file.path.as_bytes()),
            ("content_digest".as_bytes(), file.content_digest.as_bytes()),
        ]
    }))
}

fn mapping_rules_digest() -> Result<String, KirError> {
    Ok(digest_labeled_chunks([(
        "mapping".as_bytes(),
        "language-service-provided".as_bytes(),
    )]))
}

fn compiler_digest() -> String {
    digest_labeled_chunks([
        ("crate".as_bytes(), "mercurio-foundation".as_bytes()),
        ("version".as_bytes(), env!("CARGO_PKG_VERSION").as_bytes()),
        ("kir_schema".as_bytes(), KIR_SCHEMA_VERSION.as_bytes()),
    ])
}

fn digest_labeled_chunks<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = (&'a [u8], &'a [u8])>,
{
    let mut hash = FNV_OFFSET;
    for (label, bytes) in chunks {
        hash = digest_bytes(hash, &(label.len() as u64).to_le_bytes());
        hash = digest_bytes(hash, label);
        hash = digest_bytes(hash, &(bytes.len() as u64).to_le_bytes());
        hash = digest_bytes(hash, bytes);
    }
    format!("fnv1a64:{hash:016x}")
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn digest_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn normalized_source_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn safe_cache_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn process_id_segment() -> String {
    std::process::id().to_string()
}

#[cfg(target_arch = "wasm32")]
fn process_id_segment() -> String {
    "wasm".to_string()
}

fn unique_cache_nonce() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ counter.rotate_left(17)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::language_contracts::LanguageRegistry;
    use serde_json::Value;

    use super::{
        PersistentCacheStatus, PersistentWorkspaceCache, RUNTIME_CACHE_FILE_NAME,
        RUNTIME_CACHE_MANIFEST_FILE_NAME, RuntimeCachePolicy, SemanticValidationMode,
        SemanticValidationPolicy, WorkspaceCompileCacheManifest, source_file_fingerprints,
    };
    use crate::authoring::source_set::SourceDocument;
    use crate::kir::{KIR_SCHEMA_VERSION, KirDocument, KirElement, KirError};
    use crate::runtime::Runtime;

    #[test]
    fn persistent_compile_cache_reuses_unchanged_artifact() {
        let root = temp_dir("persistent_hit");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(first.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(second.cache_status, PersistentCacheStatus::PersistentHit);
        assert_eq!(first.document, second.document);
        assert!(second.cache_write_error.is_none());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_writes_runtime_cache_outputs() {
        let root = temp_dir("persistent_runtime_cache_outputs");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let dir = artifact_dir(&cache, &first.artifact_key);

        assert!(dir.join("document.kir.json").is_file());
        assert!(dir.join(RUNTIME_CACHE_FILE_NAME).is_file());
        assert!(dir.join(RUNTIME_CACHE_MANIFEST_FILE_NAME).is_file());
        assert!(!dir.join("runtime-artifact.json").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_read_only_does_not_write_runtime_cache() {
        let root = temp_dir("persistent_read_only");
        let cache =
            PersistentWorkspaceCache::for_workspace_root(&root).without_runtime_cache_writes();
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(cache.options().runtime_cache, RuntimeCachePolicy::ReadOnly);
        assert_eq!(first.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(second.cache_status, PersistentCacheStatus::PersistentMiss);
        assert!(first.cache_write_error.is_none());
        assert!(!artifact_dir(&cache, &first.artifact_key).is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_disabled_skips_runtime_cache() {
        let root = temp_dir("persistent_disabled");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root).without_runtime_cache();
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let result = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(cache.options().runtime_cache, RuntimeCachePolicy::Disabled);
        assert_eq!(result.cache_status, PersistentCacheStatus::FreshCompile);
        assert!(result.cache_write_error.is_none());
        assert!(!artifact_dir(&cache, &result.artifact_key).is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_hit_preserves_runtime_end_state() {
        let root = temp_dir("persistent_runtime_equivalence");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![
            SourceDocument::new("domain.model", "package Domain { part def Camera; }"),
            SourceDocument::new(
                "usage.model",
                "package Usage {
                  part camera : Domain.Camera;
                }",
            ),
        ];

        let miss = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let hit = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(miss.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(hit.cache_status, PersistentCacheStatus::PersistentHit);
        assert_eq!(miss.document, hit.document);

        let miss_runtime = Runtime::from_artifact(miss.runtime_artifact).unwrap();
        let hit_runtime = Runtime::from_artifact(hit.runtime_artifact).unwrap();

        assert_eq!(
            miss_runtime.graph().elements(),
            hit_runtime.graph().elements()
        );
        assert_eq!(miss_runtime.graph().edges(), hit_runtime.graph().edges());
        assert_eq!(
            miss_runtime.derived().subtypes,
            hit_runtime.derived().subtypes
        );
        assert_eq!(
            miss_runtime.derived().ownership,
            hit_runtime.derived().ownership
        );
        assert_eq!(
            miss_runtime.derived().inherited_features,
            hit_runtime.derived().inherited_features
        );
        assert_eq!(
            miss_runtime.derived().requirements,
            hit_runtime.derived().requirements
        );
        assert_eq!(
            miss_runtime.derived().satisfied_by,
            hit_runtime.derived().satisfied_by
        );
        assert_eq!(
            miss_runtime.derived().verified_by,
            hit_runtime.derived().verified_by
        );
        assert!(!miss_runtime.derived().explanations().is_empty());
        assert!(hit_runtime.derived().explanations().is_empty());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_reports_semantic_validation_on_miss_and_hit() {
        let root = temp_dir("persistent_validation_report");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root)
            .with_semantic_validation_policy(SemanticValidationPolicy {
                mode: SemanticValidationMode::Warn,
                ..SemanticValidationPolicy::default()
            });
        let library_context = validation_library_context();
        let sources = vec![SourceDocument::new("demo.model", "transition start")];

        let miss = cache
            .compile_source_documents_with(
                sources.clone(),
                &library_context,
                None,
                compile_transition_warning_document,
            )
            .unwrap();
        let hit = cache
            .compile_source_documents_with(
                sources,
                &library_context,
                None,
                compile_transition_warning_document,
            )
            .unwrap();

        assert_eq!(miss.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(hit.cache_status, PersistentCacheStatus::PersistentHit);
        assert_eq!(miss.validation_report, hit.validation_report);
        assert_eq!(hit.validation_report.warning_count(), 1);
        assert_eq!(
            hit.validation_report.diagnostics[0].code,
            "kir.metamodel.endpoints.incomplete"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_semantic_validation_errors_by_default() {
        let root = temp_dir("persistent_validation_error");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = validation_library_context();
        let sources = vec![SourceDocument::new("demo.model", "transition start")];

        let error = cache
            .compile_source_documents_with(
                sources,
                &library_context,
                None,
                compile_transition_warning_document,
            )
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("semantic validation failed with 1 error diagnostics")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_cache_diagnostic_explanations_round_trip_separately() {
        let root = temp_dir("persistent_runtime_diagnostics");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![
            SourceDocument::new("domain.model", "package Domain { part def Camera; }"),
            SourceDocument::new(
                "usage.model",
                "package Usage {
                  part camera : Domain.Camera;
                }",
            ),
        ];

        let result = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let explanations = result.runtime_artifact.derived.explanations();
        let bytes = super::diagnostic_payload_to_bytes(explanations).unwrap();
        let decoded = super::diagnostic_payload_from_bytes(&bytes).unwrap();

        assert!(!explanations.is_empty());
        assert_eq!(decoded, *explanations);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_cache_v3_fixture_decodes_hot_runtime_section() {
        let bytes =
            decode_hex_fixture(include_str!("../../../tests/fixtures/runtime_cache_v3.hex"));

        assert_eq!(&bytes[0..4], b"MRUN");
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 3);

        let artifact = super::runtime_artifact_from_binary_bytes(&bytes).unwrap();

        assert_eq!(artifact.graph.elements.len(), 1);
        assert_eq!(artifact.graph.edges.len(), 0);
        let element = &artifact.graph.elements[0];
        assert_eq!(element.id, 0);
        assert_eq!(element.element_id, "pkg.Fixture");
        assert_eq!(element.kind.as_ref(), "model.Package");
        assert_eq!(element.layer, 2);
        assert_eq!(
            element.properties.get("qualified_name"),
            Some(&Value::String("Fixture".to_string()))
        );
        assert_eq!(
            element.properties.get("declared_name"),
            Some(&Value::String("Fixture".to_string()))
        );
        assert!(artifact.derived.subtypes.is_empty());
        assert!(artifact.derived.explanations().is_empty());
    }

    #[test]
    fn persistent_compile_cache_invalidates_changed_source() {
        let root = temp_dir("persistent_changed_source");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();

        let first_sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];
        let second_sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def OtherThing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                first_sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let second = cache
            .compile_source_documents_with_registry(
                second_sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(first.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(second.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_ne!(first.artifact_key, second.artifact_key);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_invalidates_changed_config() {
        let root = temp_dir("persistent_changed_config");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let config_path = root.join("workspace.json");
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        std::fs::write(&config_path, r#"{"version":1,"name":"A"}"#).unwrap();
        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                Some(&config_path),
                &test_language_registry(),
            )
            .unwrap();
        std::fs::write(&config_path, r#"{"version":1,"name":"B"}"#).unwrap();
        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                Some(&config_path),
                &test_language_registry(),
            )
            .unwrap();

        assert_eq!(first.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_eq!(second.cache_status, PersistentCacheStatus::PersistentMiss);
        assert_ne!(first.artifact_key, second.artifact_key);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_corrupt_manifest_and_falls_back() {
        let root = temp_dir("persistent_corrupt_manifest");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let manifest_path = cache
            .root()
            .join("artifacts")
            .join(first.artifact_key.replace(':', "_"))
            .join("manifest.json");
        std::fs::write(&manifest_path, "{ not-json").unwrap();

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_corrupt_kir_and_falls_back() {
        let root = temp_dir("persistent_corrupt_kir");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let document_path = cache
            .root()
            .join("artifacts")
            .join(first.artifact_key.replace(':', "_"))
            .join("document.kir.json");
        std::fs::write(&document_path, "{ not-json").unwrap();

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_invalid_runtime_cache() {
        let root = temp_dir("persistent_runtime_invalid");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let dir = artifact_dir(&cache, &first.artifact_key);
        std::fs::write(dir.join(RUNTIME_CACHE_FILE_NAME), b"NOPE").unwrap();

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_missing_runtime_cache() {
        let root = temp_dir("persistent_runtime_missing");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let dir = artifact_dir(&cache, &first.artifact_key);
        std::fs::remove_file(dir.join(RUNTIME_CACHE_FILE_NAME)).unwrap();

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_stale_runtime_cache() {
        let root = temp_dir("persistent_runtime_stale");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let dir = artifact_dir(&cache, &first.artifact_key);
        mutate_runtime_manifest(&dir, |manifest| {
            manifest.runtime_digest = "fnv1a64:stale".to_string();
        });

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_compile_cache_rejects_runtime_source_digest_mismatch() {
        let root = temp_dir("persistent_runtime_source_digest_mismatch");
        let cache = PersistentWorkspaceCache::for_workspace_root(&root);
        let library_context = test_library_context();
        let sources = vec![SourceDocument::new(
            "demo.model",
            "package Demo { part def Thing; }",
        )];

        let first = cache
            .compile_source_documents_with_registry(
                sources.clone(),
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();
        let dir = artifact_dir(&cache, &first.artifact_key);
        mutate_runtime_manifest(&dir, |manifest| {
            manifest.source_digest = "fnv1a64:stale-source".to_string();
        });

        let second = cache
            .compile_source_documents_with_registry(
                sources,
                &library_context,
                None,
                &test_language_registry(),
            )
            .unwrap();

        assert!(matches!(
            second.cache_status,
            PersistentCacheStatus::PersistentRejected { .. }
        ));
        assert_eq!(first.document, second.document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_file_fingerprints_are_path_order_independent() {
        let left = vec![
            SourceDocument::new("b.model", "package B {}"),
            SourceDocument::new("a.model", "package A {}"),
        ];
        let right = vec![
            SourceDocument::new("a.model", "package A {}"),
            SourceDocument::new("b.model", "package B {}"),
        ];

        assert_eq!(
            source_file_fingerprints(&left),
            source_file_fingerprints(&right)
        );
    }

    #[test]
    fn manifest_roundtrip_keeps_required_key_fields() {
        let manifest = WorkspaceCompileCacheManifest {
            cache_schema_version: super::CACHE_SCHEMA_VERSION,
            artifact_family: "compile".to_string(),
            artifact_key: "fnv1a64:test".to_string(),
            key: super::WorkspaceCompileArtifactKey {
                source_authority: "local_files".to_string(),
                source_tree_digest: "fnv1a64:source".to_string(),
                workspace_config_digest: Some("fnv1a64:config".to_string()),
                compiler_digest: "fnv1a64:compiler".to_string(),
                kir_schema_version: KIR_SCHEMA_VERSION.to_string(),
                validation_policy_digest: super::semantic_validation_policy_digest(
                    SemanticValidationPolicy::default(),
                ),
                library_context_digest: "fnv1a64:library".to_string(),
                mapping_rules_digest: "fnv1a64:mapping".to_string(),
            },
            files: Vec::new(),
            outputs: super::WorkspaceCompileCacheOutputs {
                kir: "document.kir.json".to_string(),
                runtime_cache: "runtime.mruntime".to_string(),
                runtime_cache_manifest: "runtime.mruntime.manifest.json".to_string(),
                runtime_artifact: None,
            },
        };

        let encoded = serde_json::to_string(&manifest).unwrap();
        let decoded: WorkspaceCompileCacheManifest = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, manifest);
    }

    fn artifact_dir(cache: &PersistentWorkspaceCache, artifact_key: &str) -> std::path::PathBuf {
        cache
            .root()
            .join("artifacts")
            .join(artifact_key.replace(':', "_"))
    }

    fn mutate_runtime_manifest(
        artifact_dir: &std::path::Path,
        mutate: impl FnOnce(&mut super::RuntimeCacheManifest),
    ) {
        let path = artifact_dir.join(RUNTIME_CACHE_MANIFEST_FILE_NAME);
        let mut manifest: super::RuntimeCacheManifest =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        mutate(&mut manifest);
        std::fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn test_library_context() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![
                KirElement {
                    id: "Parts::Part".to_string(),
                    kind: "Model::PartDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "qualified_name".to_string(),
                        Value::String("Parts.Part".to_string()),
                    )]),
                },
                KirElement {
                    id: "Items::Item::subparts".to_string(),
                    kind: "Model::PartUsage".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "qualified_name".to_string(),
                        Value::String("Items.Item.subparts".to_string()),
                    )]),
                },
            ],
        }
    }

    fn test_language_registry() -> LanguageRegistry {
        crate::authoring::test_support::toy_language::registry()
    }

    fn compile_transition_warning_document(
        _sources: Vec<SourceDocument>,
        _library_context: &KirDocument,
    ) -> Result<KirDocument, KirError> {
        Ok(transition_warning_document())
    }

    fn transition_warning_document() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::from([(
                "kir_schema_version".to_string(),
                Value::String(KIR_SCHEMA_VERSION.to_string()),
            )]),
            elements: vec![KirElement {
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
            }],
        }
    }

    fn validation_library_context() -> KirDocument {
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

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "mercurio_workspace_cache_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn decode_hex_fixture(input: &str) -> Vec<u8> {
        let hex = input
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(hex.len() % 2 == 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }
}
