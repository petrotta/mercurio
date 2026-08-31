pub mod library;
pub mod model_state;
pub mod mpack;
pub mod paths;
pub mod performance;
pub mod plugin_registry;
pub mod project_sources;
pub mod symbol_index;
pub mod workspace;
pub mod workspace_cache;

pub use library::{
    BaselineLibraryConfig, KparLocator, KparPackageBuild, KparPackageSource, LibraryCacheMetadata,
    LibraryProviderConfig, LocalPackageManifest, LocalPackageRecord, LocalPackageRepository,
    LocalPackageSource, MercurioLockFile, MercurioLockedPackage, MercurioPackageBuildProvenance,
    MercurioPackageDependency, MercurioPackageManifest, MercurioPackageSourceProvenance,
    PackageKirCache, PackageKirCacheManifest, PackageReference, ResolvedLibraryArtifact,
    load_baseline_library_document, package_bytes_digest, parse_package_reference,
    write_kpar_package,
};
pub use model_state::{
    InputSource, InputSourceKind, InputSourceSet, MODEL_SERVICE_API_VERSION, ModelArtifact,
    ModelBuildRecord, ModelRevision, ModelRevisionDescriptor, ModelRevisionEnvelope,
    ModelRevisionId, ModelRevisionProducer, ModelRevisionPushMode, ModelServicePullRequest,
    ModelServicePullResponse, ModelServicePushRevisionRequest, ModelServicePushRevisionResponse,
    ModelServicePushStatus, ModelState, ModelStateDescriptor, ModelStateError, ModelStateId,
    RemoteModelRef,
};
pub use mpack::{
    MpackLanguageProfile, MpackLibrary, MpackManifest, MpackPythonPackage,
    MpackPythonWrapperBinding, MpackRequirements, MpackRulepack, MpackService,
    MpackValidationError, validate_mpack_manifest,
};
pub use paths::{
    bundled_extension_repo_path, bundled_package_repo_path, bundled_stdlib_package_set_path,
    default_kernel_library_path, default_package_kir_cache_path, default_package_repo_path,
    default_stdlib_path, default_stdlib_rulepack_path, default_user_config_path,
    default_workspace_root, repo_path, repo_root,
};
pub use performance::{
    CachePerformanceConfig, CachePerformanceReport, CachePerformanceScenarioReport,
    CachePerformanceTimings, CoreScalabilityCreationStrategy, CoreScalabilityMetricConfig,
    CoreScalabilityReport, CoreScalabilityScenarioReport, CoreScalabilityTimings,
    EmfComparisonReport, KirPerformanceConfig, KirPerformanceMemory, KirPerformanceReport,
    KirPerformanceScenarioReport, KirPerformanceTimings, MemoryMetric, SemanticDiffSummary,
    TimingMetric, run_cache_performance, run_core_scalability_metric, run_kir_performance,
};
pub use plugin_registry::{
    InstalledMpack, MpackActivationIndex, MpackAssetRef,
    PluginInstallSource as RegistryPluginInstallSource, PluginRegistryError,
    default_plugin_registry_root, install_plugin_manifest, installed_mpack_manifests,
    installed_plugin_manifest_paths, mpack_activation_index, plugin_manifest_dir,
    plugin_package_digest, plugin_registry_root, publish_plugin_package,
    read_plugin_install_source, read_plugin_manifest as read_registry_plugin_manifest,
};
pub use project_sources::{
    PROJECT_DESCRIPTOR_FILE_NAME, ProjectSourceFile, ProjectSourceMode, ProjectSourceSet,
    resolve_project_sources,
};
pub use symbol_index::WorkspaceSymbolIndex;
pub use workspace::{
    ProjectDescriptor, ProjectExtensionDescriptor, ProjectModelConfig, ResolvedWorkspaceContext,
    ResolvedWorkspaceLibrary, WorkspaceConfig, WorkspaceConfigError, WorkspaceContextOptions,
    WorkspaceLibraryConfig, WorkspaceLibraryRole, WorkspacePluginConfig,
    discover_project_extension_descriptor_path, discover_workspace_config_path,
    resolve_project_descriptor_context, resolve_workspace_context,
    resolve_workspace_context_from_config_path, resolve_workspace_context_with_options,
};
pub use workspace_cache::{
    PersistentCacheStatus, PersistentCompileResult, PersistentWorkspaceCache,
    PersistentWorkspaceCacheOptions, RUNTIME_CACHE_FORMAT_VERSION, RuntimeCacheManifest,
    RuntimeCachePolicy, WorkspaceCompileArtifactKey, WorkspaceCompileCacheManifest,
    WorkspaceCompileCacheOutputs, WorkspaceSourceFileFingerprint, load_runtime_artifact_cache,
    load_runtime_artifact_cache_with_rejection_reason, runtime_artifact_from_binary_bytes,
    runtime_artifact_to_binary_bytes, source_file_fingerprints, workspace_compile_artifact_key,
    write_runtime_artifact_cache,
};
