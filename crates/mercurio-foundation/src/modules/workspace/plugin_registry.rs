use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::workspace::mpack::{
    MpackLanguageProfile, MpackLibrary, MpackManifest, MpackPythonPackage, MpackRulepack,
    validate_mpack_manifest,
};
use crate::workspace::paths::default_user_config_path;

#[derive(Debug)]
pub enum PluginRegistryError {
    Io(String),
    Invalid(String),
}

impl fmt::Display for PluginRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) | Self::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PluginRegistryError {}

#[derive(Debug, Clone)]
pub struct PluginInstallSource {
    pub manifest: Value,
    pub package_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMpack {
    pub manifest_path: PathBuf,
    pub package_path: Option<PathBuf>,
    pub manifest: MpackManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpackAssetRef<T> {
    pub manifest_id: String,
    pub manifest_version: String,
    pub manifest_path: PathBuf,
    pub package_path: Option<PathBuf>,
    pub asset_path: Option<PathBuf>,
    pub entry: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MpackActivationIndex {
    pub installed: Vec<InstalledMpack>,
    pub libraries: Vec<MpackAssetRef<MpackLibrary>>,
    pub language_profiles: Vec<MpackAssetRef<MpackLanguageProfile>>,
    pub rulepacks: Vec<MpackAssetRef<MpackRulepack>>,
    pub python_packages: Vec<MpackAssetRef<MpackPythonPackage>>,
}

pub fn plugin_registry_root(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(default_plugin_registry_root)
}

pub fn default_plugin_registry_root() -> PathBuf {
    default_user_config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".mercurio"))
        .join("plugins")
}

pub fn plugin_manifest_dir(root: &Path, id: &str, version: &str) -> PathBuf {
    root.join("installed")
        .join(safe_plugin_path_segment(id))
        .join(safe_plugin_path_segment(version))
}

pub fn read_plugin_install_source(path: &Path) -> Result<PluginInstallSource, PluginRegistryError> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mpack"))
    {
        return Ok(PluginInstallSource {
            manifest: read_plugin_manifest_from_mpack(path)?,
            package_path: Some(path.to_path_buf()),
        });
    }
    Ok(PluginInstallSource {
        manifest: read_plugin_manifest(path)?,
        package_path: None,
    })
}

pub fn read_plugin_manifest(path: &Path) -> Result<Value, PluginRegistryError> {
    let input = std::fs::read_to_string(path).map_err(|err| {
        PluginRegistryError::Io(format!("failed to read {}: {err}", path.display()))
    })?;
    serde_json::from_str(&input).map_err(|err| {
        PluginRegistryError::Invalid(format!("invalid plugin manifest {}: {err}", path.display()))
    })
}

pub fn read_plugin_manifest_from_mpack(path: &Path) -> Result<Value, PluginRegistryError> {
    let file = std::fs::File::open(path).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to read plugin package {}: {err}",
            path.display()
        ))
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| {
        PluginRegistryError::Invalid(format!(
            "invalid plugin package archive {}: {err}",
            path.display()
        ))
    })?;
    let mut manifest_entry = archive.by_name("extension.json").map_err(|err| {
        PluginRegistryError::Invalid(format!(
            "plugin package {} is missing extension.json: {err}",
            path.display()
        ))
    })?;
    let mut input = String::new();
    manifest_entry.read_to_string(&mut input).map_err(|err| {
        PluginRegistryError::Invalid(format!(
            "failed to read extension.json from {}: {err}",
            path.display()
        ))
    })?;
    serde_json::from_str(&input).map_err(|err| {
        PluginRegistryError::Invalid(format!(
            "invalid plugin manifest in package {}: {err}",
            path.display()
        ))
    })
}

pub fn install_plugin_manifest(
    root: &Path,
    id: &str,
    version: &str,
    manifest: &Value,
    package_path: Option<&Path>,
    force: bool,
) -> Result<PathBuf, PluginRegistryError> {
    let target_dir = plugin_manifest_dir(root, id, version);
    let target_path = target_dir.join("extension.json");
    if !force && target_path.exists() {
        return Err(PluginRegistryError::Invalid(format!(
            "plugin {id} version {version} already exists in {}; use --force to overwrite",
            root.display()
        )));
    }
    std::fs::create_dir_all(&target_dir).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to create plugin directory {}: {err}",
            target_dir.display()
        ))
    })?;
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(|err| {
        PluginRegistryError::Invalid(format!("failed to encode plugin manifest: {err}"))
    })?;
    std::fs::write(&target_path, manifest_json).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to install plugin manifest {}: {err}",
            target_path.display()
        ))
    })?;
    if let Some(package_path) = package_path {
        let target_package = target_dir.join("plugin.mpack");
        std::fs::copy(package_path, &target_package).map_err(|err| {
            PluginRegistryError::Io(format!(
                "failed to install plugin package {}: {err}",
                target_package.display()
            ))
        })?;
    }
    Ok(target_path)
}

pub fn publish_plugin_package(
    package_path: &Path,
    root: &Path,
    id: &str,
    version: &str,
    force: bool,
) -> Result<PathBuf, PluginRegistryError> {
    let target_dir = root
        .join(safe_plugin_path_segment(id))
        .join(safe_plugin_path_segment(version));
    let target_path = target_dir.join("plugin.mpack");
    if !force && target_path.exists() {
        return Err(PluginRegistryError::Invalid(format!(
            "plugin package {id}:{version} already exists in {}; use --force to overwrite",
            root.display()
        )));
    }
    std::fs::create_dir_all(&target_dir).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to create plugin repository directory {}: {err}",
            target_dir.display()
        ))
    })?;
    std::fs::copy(package_path, &target_path).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to publish plugin package {}: {err}",
            target_path.display()
        ))
    })?;
    Ok(target_path)
}

pub fn installed_plugin_manifest_paths(root: &Path) -> Result<Vec<PathBuf>, PluginRegistryError> {
    let mut paths = Vec::new();
    let installed = root.join("installed");
    if installed.exists() {
        collect_installed_plugin_manifest_paths(&installed, &mut paths)?;
    }
    if root.exists() {
        collect_installed_plugin_manifest_paths(root, &mut paths)?;
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn installed_mpack_manifests(root: &Path) -> Result<Vec<InstalledMpack>, PluginRegistryError> {
    installed_plugin_manifest_paths(root)?
        .into_iter()
        .map(read_installed_mpack_manifest)
        .collect()
}

pub fn mpack_activation_index(root: &Path) -> Result<MpackActivationIndex, PluginRegistryError> {
    let installed = installed_mpack_manifests(root)?;
    let mut index = MpackActivationIndex {
        installed: installed.clone(),
        ..MpackActivationIndex::default()
    };

    for installed_mpack in installed {
        let base_dir = installed_mpack
            .manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let manifest_id = installed_mpack.manifest.id.clone();
        let manifest_version = installed_mpack.manifest.version.clone();
        let manifest_path = installed_mpack.manifest_path.clone();
        let package_path = installed_mpack.package_path.clone();

        for entry in installed_mpack.manifest.libraries {
            index.libraries.push(MpackAssetRef {
                manifest_id: manifest_id.clone(),
                manifest_version: manifest_version.clone(),
                manifest_path: manifest_path.clone(),
                package_path: package_path.clone(),
                asset_path: entry.path.as_deref().map(|path| base_dir.join(path)),
                entry,
            });
        }
        for entry in installed_mpack.manifest.language_profiles {
            index.language_profiles.push(MpackAssetRef {
                manifest_id: manifest_id.clone(),
                manifest_version: manifest_version.clone(),
                manifest_path: manifest_path.clone(),
                package_path: package_path.clone(),
                asset_path: Some(base_dir.join(&entry.path)),
                entry,
            });
        }
        for entry in installed_mpack.manifest.rulepacks {
            index.rulepacks.push(MpackAssetRef {
                manifest_id: manifest_id.clone(),
                manifest_version: manifest_version.clone(),
                manifest_path: manifest_path.clone(),
                package_path: package_path.clone(),
                asset_path: Some(base_dir.join(&entry.path)),
                entry,
            });
        }
        for entry in installed_mpack.manifest.python_packages {
            index.python_packages.push(MpackAssetRef {
                manifest_id: manifest_id.clone(),
                manifest_version: manifest_version.clone(),
                manifest_path: manifest_path.clone(),
                package_path: package_path.clone(),
                asset_path: Some(base_dir.join(&entry.path)),
                entry,
            });
        }
    }

    Ok(index)
}

fn read_installed_mpack_manifest(path: PathBuf) -> Result<InstalledMpack, PluginRegistryError> {
    let raw = read_plugin_manifest(&path)?;
    let manifest: MpackManifest = serde_json::from_value(raw).map_err(|err| {
        PluginRegistryError::Invalid(format!("invalid MPack manifest {}: {err}", path.display()))
    })?;
    validate_mpack_manifest(&manifest).map_err(|errors| {
        let errors = errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        PluginRegistryError::Invalid(format!(
            "invalid MPack manifest {}: {errors}",
            path.display()
        ))
    })?;
    let package_path = path.with_file_name("plugin.mpack");
    Ok(InstalledMpack {
        manifest_path: path,
        package_path: package_path.is_file().then_some(package_path),
        manifest,
    })
}

fn collect_installed_plugin_manifest_paths(
    current: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), PluginRegistryError> {
    for entry in std::fs::read_dir(current).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to read plugin directory {}: {err}",
            current.display()
        ))
    })? {
        let entry = entry.map_err(|err| {
            PluginRegistryError::Io(format!("failed to read plugin directory entry: {err}"))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_installed_plugin_manifest_paths(&path, paths)?;
        } else if path.file_name().and_then(|value| value.to_str()) == Some("extension.json") {
            paths.push(path);
        }
    }
    Ok(())
}

pub fn plugin_package_digest(path: &Path) -> Result<String, PluginRegistryError> {
    let bytes = std::fs::read(path).map_err(|err| {
        PluginRegistryError::Io(format!(
            "failed to read plugin package {}: {err}",
            path.display()
        ))
    })?;
    Ok(format_stable_digest([(
        "file".as_bytes(),
        bytes.as_slice(),
    )]))
}

fn format_stable_digest<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = (&'a [u8], &'a [u8])>,
{
    let mut hash = 0xcbf29ce484222325u64;
    for (label, bytes) in chunks {
        hash = digest_bytes(hash, &(label.len() as u64).to_le_bytes());
        hash = digest_bytes(hash, label);
        hash = digest_bytes(hash, &(bytes.len() as u64).to_le_bytes());
        hash = digest_bytes(hash, bytes);
    }
    format!("fnv1a64:{hash:016x}")
}

fn digest_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn safe_plugin_path_segment(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn install_plugin_manifest_preserves_package_archive() {
        let root = temp_dir("mercurio-plugin-registry-core");
        let package_path = root.join("sample.mpack");
        std::fs::create_dir_all(&root).unwrap();
        let file = std::fs::File::create(&package_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("extension.json", zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"org.example","version":"1.0.0","name":"Example"}"#)
            .unwrap();
        zip.finish().unwrap();

        let source = read_plugin_install_source(&package_path).unwrap();
        let path = install_plugin_manifest(
            &root.join("plugins"),
            "org.example",
            "1.0.0",
            &source.manifest,
            source.package_path.as_deref(),
            false,
        )
        .unwrap();

        assert!(path.is_file());
        assert!(path.with_file_name("plugin.mpack").is_file());
        let manifests = installed_plugin_manifest_paths(&root.join("plugins")).unwrap();
        assert_eq!(manifests, vec![path]);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publish_plugin_package_writes_repository_layout() {
        let root = temp_dir("mercurio-plugin-publish-core");
        let package_path = root.join("sample.mpack");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&package_path, b"package").unwrap();

        let published = publish_plugin_package(
            &package_path,
            &root.join("repo"),
            "org.example/plugin",
            "1.0.0",
            false,
        )
        .unwrap();

        assert_eq!(
            published,
            root.join("repo")
                .join("org.example_plugin")
                .join("1.0.0")
                .join("plugin.mpack")
        );
        assert_eq!(
            plugin_package_digest(&published).unwrap(),
            "fnv1a64:ba03c15973f83f90"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mpack_activation_index_resolves_installed_assets() {
        let root = temp_dir("mercurio-mpack-activation-core");
        let manifest = serde_json::json!({
            "id": "org.mercurio.model-stdlib-support",
            "version": "2.0.0",
            "name": "Model Stdlib Support",
            "libraries": [
                {"id": "org.omg/model-stdlib", "path": "libraries/model.kpar"}
            ],
            "languageProfiles": [
                {
                    "id": "model-2.0",
                    "path": "profiles/model/profile.json",
                    "pythonWrappers": {
                        "module": "mercurio_model",
                        "path": "python"
                    }
                }
            ],
            "rulepacks": [
                {"id": "stdlib", "path": "rules/stdlib.json"}
            ],
            "pythonPackages": [
                {"module": "mercurio_model", "path": "python", "profile": "model-2.0"}
            ]
        });
        let manifest_path = install_plugin_manifest(
            &root.join("plugins"),
            "org.mercurio.model-stdlib-support",
            "2.0.0",
            &manifest,
            None,
            false,
        )
        .unwrap();

        let index = mpack_activation_index(&root.join("plugins")).unwrap();

        assert_eq!(index.installed.len(), 1);
        assert_eq!(index.language_profiles[0].entry.id, "model-2.0");
        assert_eq!(
            index.language_profiles[0].asset_path.as_ref().unwrap(),
            &manifest_path
                .parent()
                .unwrap()
                .join("profiles")
                .join("model")
                .join("profile.json")
        );
        assert_eq!(index.python_packages[0].entry.module, "mercurio_model");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mpack_activation_index_resolves_extension_repository_layout() {
        let root = temp_dir("mercurio-mpack-extension-repo-core");
        let extension_dir = root.join("org.mercurio.model-stdlib-support").join("2.0.0");
        std::fs::create_dir_all(&extension_dir).unwrap();
        std::fs::write(extension_dir.join("plugin.mpack"), b"package").unwrap();
        std::fs::write(
            extension_dir.join("extension.json"),
            r#"{
  "id": "org.mercurio.model-stdlib-support",
  "version": "2.0.0",
  "name": "Model Stdlib Support",
  "libraries": [
    {"id": "org.omg/model-stdlib", "path": "libraries/model.kpar"}
  ],
  "languageProfiles": [
    {
      "id": "model-2.0",
      "path": "profiles/model/profile.json",
      "pythonWrappers": {
        "module": "mercurio_model",
        "path": "python"
      }
    }
  ],
  "pythonPackages": [
    {"module": "mercurio_model", "path": "python", "profile": "model-2.0"}
  ]
}"#,
        )
        .unwrap();

        let index = mpack_activation_index(&root).unwrap();

        assert_eq!(index.installed.len(), 1);
        assert_eq!(
            index.installed[0].package_path.as_ref().unwrap(),
            &extension_dir.join("plugin.mpack")
        );
        assert_eq!(
            index.python_packages[0].asset_path.as_ref().unwrap(),
            &extension_dir.join("python")
        );
        assert_eq!(
            index.libraries[0].asset_path.as_ref().unwrap(),
            &extension_dir.join("libraries").join("model.kpar")
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
