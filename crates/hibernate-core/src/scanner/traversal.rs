//! Phase 1: find project roots without measuring anything.

use crate::cleanup::rules::RuleSet;
use crate::projects::{detect, read_dir_lite, Detection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Directories that never contain projects or artifacts. They still count
/// toward a project's total size.
pub const DEFAULT_IGNORED_DIRS: &[&str] = &[".git", ".hg", ".svn", ".idea", ".vscode"];

/// Hard limit on directory nesting, as a guard against pathological trees.
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub path: PathBuf,
    pub scan_root: PathBuf,
    /// The enclosing project, when nested.
    pub parent: Option<PathBuf>,
    pub detection: Detection,
    /// Workspace members folded into this project, relative to its root.
    pub members: Vec<String>,
}

pub struct DiscoveryOptions<'a> {
    pub follow_symlinks: bool,
    pub scan_hidden: bool,
    pub ignored_paths: &'a [PathBuf],
    pub rules: &'a RuleSet,
}

struct Frame {
    path: PathBuf,
    depth: usize,
    /// Index into the found list of the enclosing project.
    project: Option<usize>,
}

/// `apps/web` style relative path with forward slashes.
pub fn relative_slash(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn is_default_ignored(name: &str) -> bool {
    DEFAULT_IGNORED_DIRS.contains(&name)
}

pub fn is_user_ignored(path: &Path, ignored: &[PathBuf]) -> bool {
    ignored
        .iter()
        .any(|p| !p.as_os_str().is_empty() && path.starts_with(p))
}

/// Walk `root` and return every project found, calling `on_found` as each
/// one is discovered. Symlinks are not followed unless enabled, and even then
/// every directory is visited at most once.
pub fn discover(
    root: &Path,
    opts: &DiscoveryOptions,
    cancel: &AtomicBool,
    on_found: &mut dyn FnMut(&DiscoveredProject),
    on_warning: &mut dyn FnMut(&Path, String),
) -> Vec<DiscoveredProject> {
    let mut found: Vec<DiscoveredProject> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    if opts.follow_symlinks {
        if let Ok(c) = std::fs::canonicalize(root) {
            visited.insert(c);
        }
    }
    let mut stack = vec![Frame {
        path: root.to_path_buf(),
        depth: 0,
        project: None,
    }];

    while let Some(frame) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut entries = match read_dir_lite(&frame.path) {
            Ok(e) => e,
            Err(err) => {
                on_warning(&frame.path, format!("Cannot read folder: {err}"));
                continue;
            }
        };

        let mut current = frame.project;
        if let Some(det) = detect(&frame.path, &entries) {
            let absorbed = current
                .map(|i| {
                    let relative = relative_slash(&found[i].path, &frame.path);
                    found[i].detection.absorbs(&det, &relative)
                })
                .unwrap_or(false);
            if absorbed {
                if let Some(i) = current {
                    let relative = relative_slash(&found[i].path, &frame.path);
                    found[i].members.push(relative);
                }
            } else {
                let dp = DiscoveredProject {
                    path: frame.path.clone(),
                    scan_root: root.to_path_buf(),
                    parent: current.map(|i| found[i].path.clone()),
                    detection: det,
                    members: Vec::new(),
                };
                on_found(&dp);
                found.push(dp);
                current = Some(found.len() - 1);
            }
        }

        if frame.depth >= MAX_DEPTH {
            on_warning(
                &frame.path,
                format!("Folder nesting deeper than {MAX_DEPTH} levels was not searched"),
            );
            continue;
        }

        // Deterministic order: reverse-sort so the stack pops alphabetically.
        entries.sort_by(|a, b| b.name.cmp(&a.name));
        for e in entries {
            let child = frame.path.join(&e.name);
            let is_dir = if e.is_dir {
                true
            } else if e.is_symlink && opts.follow_symlinks {
                std::fs::metadata(&child)
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            } else {
                false
            };
            if !is_dir {
                continue;
            }
            if !opts.scan_hidden && e.name.starts_with('.') {
                continue;
            }
            if is_default_ignored(&e.name) || opts.rules.is_barrier(&e.name) {
                continue;
            }
            if is_user_ignored(&child, opts.ignored_paths) {
                continue;
            }
            if opts.follow_symlinks {
                match std::fs::canonicalize(&child) {
                    Ok(c) => {
                        if !visited.insert(c) {
                            continue;
                        }
                    }
                    Err(_) => continue,
                }
            }
            stack.push(Frame {
                path: child,
                depth: frame.depth + 1,
                project: current,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn run(root: &Path, follow: bool) -> Vec<DiscoveredProject> {
        let rules = RuleSet::builtin();
        let opts = DiscoveryOptions {
            follow_symlinks: follow,
            scan_hidden: true,
            ignored_paths: &[],
            rules: &rules,
        };
        discover(
            root,
            &opts,
            &AtomicBool::new(false),
            &mut |_| {},
            &mut |_, _| {},
        )
    }

    #[test]
    fn finds_nested_projects_but_not_inside_artifacts() {
        let tmp = tempdir().unwrap();
        let r = tmp.path();
        touch(&r.join("workspace/frontend/package.json"), "{}");
        touch(
            &r.join("workspace/frontend/node_modules/left-pad/package.json"),
            "{}",
        );
        touch(
            &r.join("workspace/backend/Cargo.toml"),
            "[package]\nname='b'",
        );
        touch(&r.join("workspace/backend/target/debug/x"), "");
        touch(&r.join("notes/todo.txt"), "");
        let found = run(r, false);
        let mut paths: Vec<String> = found
            .iter()
            .map(|p| {
                p.path
                    .strip_prefix(r)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["workspace/backend", "workspace/frontend"]);
    }

    #[test]
    fn workspace_members_are_folded_into_the_root() {
        let tmp = tempdir().unwrap();
        let r = tmp.path();
        touch(&r.join("mono/package.json"), r#"{"workspaces":["apps/*"]}"#);
        touch(&r.join("mono/apps/web/package.json"), "{}");
        touch(&r.join("mono/apps/api/package.json"), "{}");
        // A different ecosystem inside the workspace is still its own project.
        touch(&r.join("mono/tools/cli/Cargo.toml"), "[package]");
        let found = run(r, false);
        let mut paths: Vec<String> = found
            .iter()
            .map(|p| {
                p.path
                    .strip_prefix(r)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["mono", "mono/tools/cli"]);
        let cli = found.iter().find(|p| p.path.ends_with("cli")).unwrap();
        assert_eq!(cli.parent.as_deref(), Some(r.join("mono").as_path()));
        let mono = found.iter().find(|p| p.path.ends_with("mono")).unwrap();
        assert_eq!(mono.members, vec!["apps/api", "apps/web"]);
    }

    #[test]
    fn workspace_globs_limit_what_is_absorbed() {
        let tmp = tempdir().unwrap();
        let r = tmp.path();
        touch(
            &r.join("mono/package.json"),
            r#"{"workspaces":["packages/*"]}"#,
        );
        touch(&r.join("mono/packages/ui/package.json"), "{}");
        // Not a declared member: a separate project.
        touch(&r.join("mono/examples/demo/package.json"), "{}");
        let found = run(r, false);
        let mut paths: Vec<String> = found
            .iter()
            .map(|p| {
                p.path
                    .strip_prefix(r)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["mono", "mono/examples/demo"]);
    }

    #[test]
    fn same_ecosystem_nested_outside_workspace_is_separate() {
        let tmp = tempdir().unwrap();
        let r = tmp.path();
        touch(&r.join("site/package.json"), "{}");
        touch(&r.join("site/examples/demo/package.json"), "{}");
        let found = run(r, false);
        assert_eq!(found.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_by_default_and_never_loop() {
        let tmp = tempdir().unwrap();
        let r = tmp.path();
        touch(&r.join("a/package.json"), "{}");
        std::os::unix::fs::symlink(r.join("a"), r.join("a/link-to-self")).unwrap();
        std::os::unix::fs::symlink(r.join("a"), r.join("alias")).unwrap();
        assert_eq!(run(r, false).len(), 1);
        // Following: the alias resolves to an already visited directory.
        assert_eq!(run(r, true).len(), 1);
    }
}
