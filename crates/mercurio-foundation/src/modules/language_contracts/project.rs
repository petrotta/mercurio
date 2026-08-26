//! Pure project-source selection shared by native and browser hosts.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectSourceSelectionError {
    AbsolutePath(String),
    ParentTraversal(String),
}

impl fmt::Display for ProjectSourceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbsolutePath(path) => {
                write!(formatter, "project source path must be relative: {path}")
            }
            Self::ParentTraversal(path) => {
                write!(
                    formatter,
                    "project source path escapes the project root: {path}"
                )
            }
        }
    }
}

impl std::error::Error for ProjectSourceSelectionError {}

/// Selects normalized candidate source paths using the descriptor policy.
///
/// Entrypoints are authoritative when present. Otherwise source roots are used;
/// an empty configuration selects every candidate. The function performs no I/O,
/// so WASM/browser hosts can apply the same policy to a virtual file inventory.
pub fn select_project_source_paths(
    entrypoints: &[String],
    source_roots: &[String],
    candidates: &[String],
) -> Result<Vec<String>, ProjectSourceSelectionError> {
    let selections = if !entrypoints.is_empty() {
        entrypoints
    } else {
        source_roots
    };
    let candidates = candidates
        .iter()
        .map(|candidate| normalize_relative_path(candidate))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if selections.is_empty() {
        return Ok(candidates.into_iter().collect());
    }
    let selections = selections
        .iter()
        .map(|selection| normalize_relative_path(selection))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(candidates
        .into_iter()
        .filter(|candidate| {
            selections.iter().any(|selection| {
                candidate == selection || candidate.starts_with(&format!("{selection}/"))
            })
        })
        .collect())
}

fn normalize_relative_path(path: &str) -> Result<String, ProjectSourceSelectionError> {
    let path = path.replace('\\', "/");
    if path.starts_with('/')
        || path
            .split('/')
            .next()
            .is_some_and(|component| component.ends_with(':'))
    {
        return Err(ProjectSourceSelectionError::AbsolutePath(path));
    }
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(ProjectSourceSelectionError::ParentTraversal(path)),
            value => components.push(value),
        }
    }
    Ok(components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrypoints_override_source_roots() {
        let selected = select_project_source_paths(
            &["model/main.sysml".to_string()],
            &["model".to_string()],
            &[
                "model/extra.sysml".to_string(),
                "model/main.sysml".to_string(),
            ],
        )
        .expect("selection");
        assert_eq!(selected, vec!["model/main.sysml"]);
    }

    #[test]
    fn source_roots_select_descendants_deterministically() {
        let selected = select_project_source_paths(
            &[],
            &["model".to_string()],
            &["other.sysml".to_string(), "model\\main.sysml".to_string()],
        )
        .expect("selection");
        assert_eq!(selected, vec!["model/main.sysml"]);
    }

    #[test]
    fn parent_traversal_is_rejected() {
        let error = select_project_source_paths(&["../outside.sysml".to_string()], &[], &[])
            .expect_err("reject escape");
        assert!(matches!(
            error,
            ProjectSourceSelectionError::ParentTraversal(_)
        ));
    }
}
