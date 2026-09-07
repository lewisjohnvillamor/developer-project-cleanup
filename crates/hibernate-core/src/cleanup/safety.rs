//! The gate in front of every removal. Nothing is deleted unless it passes
//! every check here, in this order:
//!
//! 1. non-empty path with enough components to never be a drive or `/home`
//! 2. the owning project is not protected
//! 3. the artifact was produced by the scan (no arbitrary paths)
//! 4. it exists, is a real directory, and is not a symlink
//! 5. its canonical path is strictly inside the canonical project path
//! 6. the project is inside one of the configured scan roots
//! 7. its name matches a cleanup rule and no protected pattern

use crate::cleanup::rules::RuleSet;
use crate::model::Project;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    #[error("empty path")]
    EmptyPath,
    #[error("refusing to touch a filesystem or drive root: {0}")]
    FilesystemRoot(PathBuf),
    #[error("path is too close to the filesystem root to be an artifact: {0}")]
    TooShallow(PathBuf),
    #[error("project is protected: {0}")]
    ProjectProtected(String),
    #[error("not part of the last scan for this project: {0}")]
    NotInScanResults(PathBuf),
    #[error("no longer exists: {0}")]
    NotFound(PathBuf),
    #[error("is a symbolic link, which is never removed: {0}")]
    IsSymlink(PathBuf),
    #[error("is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("cannot resolve path: {0}")]
    Unresolvable(PathBuf),
    #[error("{artifact} is outside project {project}")]
    OutsideProject { artifact: PathBuf, project: PathBuf },
    #[error("project is outside every configured scan folder: {0}")]
    OutsideScanRoots(PathBuf),
    #[error("name is protected and never removed: {0}")]
    ProtectedName(String),
    #[error("no cleanup rule matches: {0}")]
    NoMatchingRule(String),
}

fn normal_depth(path: &Path) -> usize {
    path.components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count()
}

/// Check a single artifact. Returns its canonical path on success.
pub fn validate_artifact(
    scan_roots: &[PathBuf],
    project: &Project,
    artifact: &Path,
    rules: &RuleSet,
) -> Result<PathBuf, SafetyError> {
    if artifact.as_os_str().is_empty() || artifact.components().count() == 0 {
        return Err(SafetyError::EmptyPath);
    }
    if artifact.parent().is_none() {
        return Err(SafetyError::FilesystemRoot(artifact.to_path_buf()));
    }
    if normal_depth(artifact) < 2 || normal_depth(&project.path) < 1 {
        return Err(SafetyError::TooShallow(artifact.to_path_buf()));
    }
    if project.protected {
        return Err(SafetyError::ProjectProtected(project.name.clone()));
    }
    if project.artifact(artifact).is_none() {
        return Err(SafetyError::NotInScanResults(artifact.to_path_buf()));
    }

    let meta = std::fs::symlink_metadata(artifact)
        .map_err(|_| SafetyError::NotFound(artifact.to_path_buf()))?;
    if meta.file_type().is_symlink() {
        return Err(SafetyError::IsSymlink(artifact.to_path_buf()));
    }
    if !meta.is_dir() {
        return Err(SafetyError::NotADirectory(artifact.to_path_buf()));
    }

    let canon = std::fs::canonicalize(artifact)
        .map_err(|_| SafetyError::Unresolvable(artifact.to_path_buf()))?;
    let project_canon = std::fs::canonicalize(&project.path)
        .map_err(|_| SafetyError::Unresolvable(project.path.clone()))?;
    if canon == project_canon || !canon.starts_with(&project_canon) {
        return Err(SafetyError::OutsideProject {
            artifact: canon,
            project: project_canon,
        });
    }
    if canon.parent().is_none() || normal_depth(&canon) < 2 {
        return Err(SafetyError::FilesystemRoot(canon));
    }

    let inside_root = scan_roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|r| project_canon.starts_with(&r))
            .unwrap_or(false)
    });
    if !inside_root {
        return Err(SafetyError::OutsideScanRoots(project_canon));
    }

    let name = canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if rules.is_protected(&name) {
        return Err(SafetyError::ProtectedName(name));
    }
    if !rules.is_barrier(&name) {
        return Err(SafetyError::NoMatchingRule(name));
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;

    fn project_at(path: &Path, artifacts: &[&Path]) -> Project {
        Project {
            id: project_id(path),
            name: "p".into(),
            path: path.to_path_buf(),
            scan_root: path.parent().unwrap().to_path_buf(),
            parent_path: None,
            stacks: vec![Stack::Node],
            frameworks: vec![],
            workspace_members: vec![],
            scan_duration_ms: 0,
            cache_hits: 0,
            package_manager: None,
            total_bytes: 0,
            reclaimable_bytes: 0,
            review_bytes: 0,
            file_count: 0,
            last_activity_at: None,
            activity: ActivitySources::default(),
            git: None,
            status: ProjectStatus::Dormant,
            safety: Safety::Safe,
            safety_reasons: vec![],
            artifacts: artifacts
                .iter()
                .map(|a| CleanupArtifact {
                    path: a.to_path_buf(),
                    relative_path: String::new(),
                    kind: "node_modules".into(),
                    category: ArtifactCategory::NodeDependencies,
                    bytes: 0,
                    file_count: 0,
                    dir_count: 0,
                    safety: Safety::Safe,
                    regeneratable: true,
                    explanation: String::new(),
                    restore_hint: None,
                })
                .collect(),
            protected_entries: vec![],
            protected: false,
            ignored_until: None,
            hibernation: None,
            scanned_at: Utc::now(),
            warnings: vec![],
        }
    }

    #[test]
    fn accepts_a_scanned_artifact_inside_the_project() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let proj = root.join("web");
        let nm = proj.join("node_modules");
        fs::create_dir_all(&nm).unwrap();
        let p = project_at(&proj, &[&nm]);
        let rules = RuleSet::builtin();
        assert!(validate_artifact(&[root], &p, &nm, &rules).is_ok());
    }

    #[test]
    fn rejects_everything_dangerous() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let proj = root.join("web");
        let nm = proj.join("node_modules");
        let src = proj.join("src");
        fs::create_dir_all(&nm).unwrap();
        fs::create_dir_all(&src).unwrap();
        let rules = RuleSet::builtin();

        // Roots and shallow paths.
        let p = project_at(&proj, &[&nm]);
        assert_eq!(
            validate_artifact(std::slice::from_ref(&root), &p, Path::new(""), &rules),
            Err(SafetyError::EmptyPath)
        );
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &p, Path::new("/"), &rules),
            Err(SafetyError::FilesystemRoot(_))
        ));
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &p, Path::new("/home"), &rules),
            Err(SafetyError::TooShallow(_))
        ));

        // Not produced by the scan.
        let unknown = proj.join("dist");
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &p, &unknown, &rules),
            Err(SafetyError::NotInScanResults(_))
        ));

        // Protected name, even when listed.
        let p_src = project_at(&proj, &[&src]);
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &p_src, &src, &rules),
            Err(SafetyError::ProtectedName(_))
        ));

        // Protected project.
        let mut protected = project_at(&proj, &[&nm]);
        protected.protected = true;
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &protected, &nm, &rules),
            Err(SafetyError::ProjectProtected(_))
        ));

        // Outside scan roots.
        let other = tempdir().unwrap();
        assert!(matches!(
            validate_artifact(&[other.path().to_path_buf()], &p, &nm, &rules),
            Err(SafetyError::OutsideScanRoots(_))
        ));

        // Missing.
        let gone = proj.join("coverage");
        let p_gone = project_at(&proj, &[&gone]);
        assert!(matches!(
            validate_artifact(std::slice::from_ref(&root), &p_gone, &gone, &rules),
            Err(SafetyError::NotFound(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_escapes() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let proj = root.join("web");
        fs::create_dir_all(&proj).unwrap();
        let outside = tempdir().unwrap();
        let link = proj.join("node_modules");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let p = project_at(&proj, &[&link]);
        let rules = RuleSet::builtin();
        assert!(matches!(
            validate_artifact(&[root], &p, &link, &rules),
            Err(SafetyError::IsSymlink(_))
        ));
    }
}
