use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::language_contracts::select_project_source_paths;
use crate::workspace::{ProjectDescriptor, WorkspaceConfigError};

pub const PROJECT_DESCRIPTOR_FILE_NAME: &str = ".project.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSourceMode {
    Descriptor,
    Inferred,
    Loose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSourceFile {
    pub relative_path: String,
    pub path: PathBuf,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSourceSet {
    pub workspace_root: PathBuf,
    pub descriptor_path: Option<PathBuf>,
    pub descriptor: Option<ProjectDescriptor>,
    pub mode: ProjectSourceMode,
    pub files: Vec<ProjectSourceFile>,
}

pub fn resolve_project_sources(
    open_path: &Path,
    supported_extensions: &[&str],
) -> Result<ProjectSourceSet, WorkspaceConfigError> {
    if !open_path.exists() {
        return Err(WorkspaceConfigError::Invalid(format!(
            "project path does not exist: {}",
            open_path.display()
        )));
    }

    let descriptor_path = if open_path.is_file()
        && open_path.file_name().and_then(|value| value.to_str())
            == Some(PROJECT_DESCRIPTOR_FILE_NAME)
    {
        Some(open_path.to_path_buf())
    } else {
        find_project_descriptor(open_path)
    };

    if let Some(descriptor_path) = descriptor_path {
        return resolve_descriptor_sources(&descriptor_path, supported_extensions);
    }

    if open_path.is_file() {
        let workspace_root = open_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let canonical_root = workspace_root.canonicalize()?;
        let mut files = BTreeMap::new();
        collect_file(&canonical_root, open_path, supported_extensions, &mut files)?;
        return Ok(ProjectSourceSet {
            workspace_root: canonical_root,
            descriptor_path: None,
            descriptor: None,
            mode: ProjectSourceMode::Loose,
            files: files.into_values().collect(),
        });
    }

    let workspace_root = open_path.canonicalize()?;
    let mut files = BTreeMap::new();
    collect_path(
        &workspace_root,
        &workspace_root,
        supported_extensions,
        true,
        &mut files,
    )?;
    Ok(ProjectSourceSet {
        workspace_root,
        descriptor_path: None,
        descriptor: None,
        mode: ProjectSourceMode::Inferred,
        files: files.into_values().collect(),
    })
}

fn resolve_descriptor_sources(
    descriptor_path: &Path,
    supported_extensions: &[&str],
) -> Result<ProjectSourceSet, WorkspaceConfigError> {
    let descriptor_path = descriptor_path.canonicalize()?;
    let workspace_root = descriptor_path
        .parent()
        .ok_or_else(|| {
            WorkspaceConfigError::Invalid("project descriptor has no parent directory".to_string())
        })?
        .to_path_buf();
    let descriptor = ProjectDescriptor::from_path(&descriptor_path)?;
    let selections = if !descriptor.model.entrypoints.is_empty() {
        descriptor.model.entrypoints.as_slice()
    } else if !descriptor.model.source_roots.is_empty() {
        descriptor.model.source_roots.as_slice()
    } else {
        &[]
    };
    for selection in selections {
        let selected_path = workspace_root.join(selection);
        if !selected_path.exists() {
            return Err(WorkspaceConfigError::Invalid(format!(
                "project source path does not exist: {selection}"
            )));
        }
        let canonical_selection = selected_path.canonicalize()?;
        if !canonical_selection.starts_with(&workspace_root) {
            return Err(WorkspaceConfigError::Invalid(format!(
                "project source path escapes the project root: {selection}"
            )));
        }
    }
    let mut files = BTreeMap::new();
    collect_path(
        &workspace_root,
        &workspace_root,
        supported_extensions,
        true,
        &mut files,
    )?;
    let candidates = files.keys().cloned().collect::<Vec<_>>();
    let selected = select_project_source_paths(
        &descriptor.model.entrypoints,
        &descriptor.model.source_roots,
        &candidates,
    )
    .map_err(|error| WorkspaceConfigError::Invalid(error.to_string()))?
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    files.retain(|path, _| selected.contains(path));
    Ok(ProjectSourceSet {
        workspace_root,
        descriptor_path: Some(descriptor_path),
        descriptor: Some(descriptor),
        mode: ProjectSourceMode::Descriptor,
        files: files.into_values().collect(),
    })
}

fn find_project_descriptor(open_path: &Path) -> Option<PathBuf> {
    let start = if open_path.is_dir() {
        open_path
    } else {
        open_path.parent()?
    };
    start
        .ancestors()
        .map(|ancestor| ancestor.join(PROJECT_DESCRIPTOR_FILE_NAME))
        .find(|candidate| candidate.is_file())
}

fn collect_path(
    workspace_root: &Path,
    path: &Path,
    supported_extensions: &[&str],
    is_project_root: bool,
    files: &mut BTreeMap<String, ProjectSourceFile>,
) -> Result<(), WorkspaceConfigError> {
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(workspace_root) {
        return Err(WorkspaceConfigError::Invalid(format!(
            "project source path escapes the project root: {}",
            path.display()
        )));
    }
    if canonical_path.is_file() {
        return collect_file(workspace_root, &canonical_path, supported_extensions, files);
    }
    if !canonical_path.is_dir() {
        return Ok(());
    }
    if !is_project_root
        && canonical_path != workspace_root
        && canonical_path.join(PROJECT_DESCRIPTOR_FILE_NAME).is_file()
    {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(&canonical_path)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for entry in entries {
        if entry.is_dir() && ignored_directory(&entry) {
            continue;
        }
        collect_path(workspace_root, &entry, supported_extensions, false, files)?;
    }
    Ok(())
}

fn collect_file(
    workspace_root: &Path,
    path: &Path,
    supported_extensions: &[&str],
    files: &mut BTreeMap<String, ProjectSourceFile>,
) -> Result<(), WorkspaceConfigError> {
    if !supports_path(path, supported_extensions) {
        return Ok(());
    }
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(workspace_root) {
        return Err(WorkspaceConfigError::Invalid(format!(
            "project source path escapes the project root: {}",
            path.display()
        )));
    }
    let relative = canonical_path.strip_prefix(workspace_root).map_err(|_| {
        WorkspaceConfigError::Invalid(format!(
            "failed to make project source relative: {}",
            canonical_path.display()
        ))
    })?;
    let relative_path = relative.to_string_lossy().replace('\\', "/");
    let text = std::fs::read_to_string(&canonical_path)?;
    files.insert(
        relative_path.clone(),
        ProjectSourceFile {
            relative_path,
            path: canonical_path,
            text,
        },
    );
    Ok(())
}

fn supports_path(path: &Path, supported_extensions: &[&str]) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    supported_extensions
        .iter()
        .any(|candidate| extension.eq_ignore_ascii_case(candidate.trim_start_matches('.')))
}

fn ignored_directory(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".git" | ".worktrees" | "node_modules" | "target")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mercurio_{name}_{nonce}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, text).expect("write fixture");
    }

    #[test]
    fn entrypoints_override_source_roots() {
        let root = temp_dir("entrypoints_override");
        write(&root.join("model/main.sysml"), "package Main {}");
        write(&root.join("model/extra.sysml"), "package Extra {}");
        write(
            &root.join(PROJECT_DESCRIPTOR_FILE_NAME),
            r#"{"version":2,"model":{"sourceRoots":["model"],"entrypoints":["model/main.sysml"]}}"#,
        );
        let sources = resolve_project_sources(&root, &["sysml", "kerml"]).expect("resolve");
        assert_eq!(sources.mode, ProjectSourceMode::Descriptor);
        assert_eq!(
            sources
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["model/main.sysml"]
        );
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn descriptorless_folder_is_inferred_and_nested_projects_are_not_claimed() {
        let root = temp_dir("inferred_nested");
        write(&root.join("main.sysml"), "package Main {}");
        write(&root.join("nested/model.sysml"), "package Nested {}");
        write(
            &root.join("nested/.project.json"),
            r#"{"version":2,"model":{"entrypoints":["model.sysml"]}}"#,
        );
        let sources = resolve_project_sources(&root, &["sysml"]).expect("resolve");
        assert_eq!(sources.mode, ProjectSourceMode::Inferred);
        assert_eq!(sources.files.len(), 1);
        assert_eq!(sources.files[0].relative_path, "main.sysml");
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn descriptor_source_cannot_escape_root() {
        let parent = temp_dir("escape");
        let root = parent.join("project");
        std::fs::create_dir_all(&root).expect("create project");
        write(&parent.join("outside.sysml"), "package Outside {}");
        write(
            &root.join(PROJECT_DESCRIPTOR_FILE_NAME),
            r#"{"version":2,"model":{"entrypoints":["../outside.sysml"]}}"#,
        );
        let error = resolve_project_sources(&root, &["sysml"]).expect_err("reject escape");
        assert!(error.to_string().contains("escapes the project root"));
        std::fs::remove_dir_all(parent).expect("remove temp dir");
    }
}
