use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DEFAULT_STDLIB_RELATIVE_PATH: &str = "resources/foundation/empty.kir.json";
const DEFAULT_STDLIB_RULEPACK_RELATIVE_PATH: &str = "resources/foundation/core.rulepack.json";
const DEFAULT_LANGUAGE_PROFILE_ROOT_RELATIVE_PATH: &str = "resources/language-profiles";
const DEFAULT_BUNDLED_LIBRARY_REPO_RELATIVE_PATH: &str = "packages/libraries";
const DEFAULT_BUNDLED_EXTENSION_REPO_RELATIVE_PATH: &str = "packages/extensions";
const DEFAULT_BUNDLED_STDLIB_PACKAGE_SET_RELATIVE_PATH: &str = "packages/libraries";
const DEFAULT_KERNEL_LIBRARY_RELATIVE_PATH: &str = "resources/foundation/empty.kir.json";
const DEFAULT_MODEL_DELTA_LIBRARY_RELATIVE_PATH: &str = "resources/foundation/empty.kir.json";
const REPO_SENTINELS: [&str; 2] = ["Cargo.toml", "resources/foundation/empty.kir.json"];

pub fn default_stdlib_path() -> PathBuf {
    default_model_library_path()
}

pub fn default_model_library_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_MODEL_LIBRARY_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(path) = std::env::var("MERCURIO_STDLIB_PATH") {
        return PathBuf::from(path);
    }

    repo_path(DEFAULT_STDLIB_RELATIVE_PATH)
}

pub fn default_stdlib_rulepack_path() -> PathBuf {
    default_model_rulepack_path()
}

pub fn default_model_rulepack_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_MODEL_RULEPACK_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(path) = std::env::var("MERCURIO_STDLIB_RULEPACK_PATH") {
        return PathBuf::from(path);
    }

    repo_path(DEFAULT_STDLIB_RULEPACK_RELATIVE_PATH)
}

pub fn default_kernel_library_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_KERNEL_LIBRARY_PATH") {
        return PathBuf::from(path);
    }

    repo_path(DEFAULT_KERNEL_LIBRARY_RELATIVE_PATH)
}

pub fn default_model_delta_library_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_MODEL_DELTA_LIBRARY_PATH") {
        return PathBuf::from(path);
    }

    repo_path(DEFAULT_MODEL_DELTA_LIBRARY_RELATIVE_PATH)
}

pub fn default_language_profile_path(profile_id: &str) -> PathBuf {
    repo_path(DEFAULT_LANGUAGE_PROFILE_ROOT_RELATIVE_PATH)
        .join(profile_id)
        .join("profile.json")
}

pub fn default_package_repo_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_PACKAGE_REPO") {
        return PathBuf::from(path);
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(repo_root);

    home.join(".mercurio").join("packages")
}

pub fn default_user_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(repo_root);

    home.join(".mercurio").join("config.json")
}

pub fn default_package_kir_cache_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_PACKAGE_KIR_CACHE") {
        return PathBuf::from(path);
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(repo_root);

    home.join(".mercurio").join("cache").join("kir")
}

pub fn bundled_package_repo_path() -> PathBuf {
    repo_path(DEFAULT_BUNDLED_LIBRARY_REPO_RELATIVE_PATH)
}

pub fn bundled_extension_repo_path() -> PathBuf {
    repo_path(DEFAULT_BUNDLED_EXTENSION_REPO_RELATIVE_PATH)
}

pub fn bundled_stdlib_package_set_path() -> PathBuf {
    repo_path(DEFAULT_BUNDLED_STDLIB_PACKAGE_SET_RELATIVE_PATH)
}

pub fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(Path::new(relative))
}

pub fn repo_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(resolve_repo_root).clone()
}

pub fn default_workspace_root() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(repo_root);

    let documents = home.join("Documents");
    if documents.is_dir() {
        documents.join("Mercurio")
    } else {
        home.join("Mercurio")
    }
}

fn resolve_repo_root() -> PathBuf {
    if let Ok(path) = std::env::var("MERCURIO_REPO_ROOT") {
        let candidate = PathBuf::from(path);
        if looks_like_repo_root(&candidate) {
            return candidate;
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(candidate) = find_repo_root(&manifest_dir) {
        return candidate;
    }

    if let Ok(current_dir) = std::env::current_dir()
        && let Some(candidate) = find_repo_root(&current_dir)
    {
        return candidate;
    }

    manifest_dir
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if looks_like_repo_root(ancestor) {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

fn looks_like_repo_root(path: &Path) -> bool {
    REPO_SENTINELS
        .iter()
        .all(|relative| path.join(relative).exists())
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_stdlib_rulepack_path_points_to_resource() {
        let path = super::default_stdlib_rulepack_path();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("core.rulepack.json")
        );
        assert!(path.exists());
    }

    #[test]
    fn default_kernel_library_path_points_to_committed_kir() {
        let path = super::default_kernel_library_path();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("empty.kir.json")
        );
        assert!(path.exists());
    }

    #[test]
    fn default_model_delta_library_path_points_to_committed_kir() {
        let path = super::default_model_delta_library_path();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("empty.kir.json")
        );
        assert!(path.exists());
    }
}
