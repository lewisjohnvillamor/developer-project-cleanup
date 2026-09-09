//! Lightweight Git inspection. Branch, remote and last-commit time are read
//! straight from `.git/` without spawning anything; working-tree status uses
//! the `git` binary when it is available.

use crate::model::{GitInfo, GitState};
use chrono::{DateTime, Utc};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// True when a `git` binary is on PATH. Cached for the process lifetime.
pub fn git_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let mut cmd = Command::new("git");
        cmd.arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_window(&mut cmd);
        cmd.status().map(|s| s.success()).unwrap_or(false)
    })
}

/// Resolve the directory holding HEAD/config/logs for a project. `.git` may
/// be a file pointing elsewhere (worktrees, submodules).
fn git_dir(project: &Path) -> Option<PathBuf> {
    let dot_git = project.join(".git");
    let meta = fs::symlink_metadata(&dot_git).ok()?;
    if meta.is_dir() {
        return Some(dot_git);
    }
    if meta.is_file() {
        let text = fs::read_to_string(&dot_git).ok()?;
        let target = text.trim().strip_prefix("gitdir:")?.trim();
        let target = Path::new(target);
        let resolved = if target.is_absolute() {
            target.to_path_buf()
        } else {
            project.join(target)
        };
        return resolved.is_dir().then_some(resolved);
    }
    None
}

pub fn inspect(project: &Path, run_status: bool) -> GitInfo {
    let Some(dir) = git_dir(project) else {
        return GitInfo::no_repo();
    };

    let branch = fs::read_to_string(dir.join("HEAD")).ok().and_then(|head| {
        let head = head.trim();
        if let Some(r) = head.strip_prefix("ref:") {
            Some(r.trim().trim_start_matches("refs/heads/").to_string())
        } else if head.len() >= 7 {
            Some(format!("detached @ {}", &head[..7]))
        } else {
            None
        }
    });

    // Worktrees keep config in the common dir.
    let common = fs::read_to_string(dir.join("commondir"))
        .ok()
        .map(|c| dir.join(c.trim()))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| dir.clone());
    let remote_configured = fs::read_to_string(common.join("config"))
        .ok()
        .map(|c| c.contains("[remote \""));

    let last_commit_at = last_commit_time(&dir);

    let mut info = GitInfo {
        is_repo: true,
        state: GitState::Unknown,
        is_clean: None,
        modified_count: 0,
        untracked_count: 0,
        remote_configured,
        branch,
        last_commit_at,
    };

    if run_status {
        if let Some((modified, untracked)) = status_counts(project) {
            info.modified_count = modified;
            info.untracked_count = untracked;
            info.is_clean = Some(modified == 0 && untracked == 0);
        }
    }

    info.state = match (info.is_clean, info.modified_count, info.untracked_count) {
        (None, _, _) => GitState::Unknown,
        (Some(_), m, _) if m > 0 => GitState::Modified,
        (Some(_), _, u) if u > 0 => GitState::Untracked,
        (Some(_), _, _) if info.remote_configured == Some(false) => GitState::RemoteMissing,
        _ => GitState::Clean,
    };
    info
}

/// Timestamp of the most recent reflog entry for HEAD, which is the last
/// commit / checkout / pull. Falls back to the mtime of the HEAD ref file.
fn last_commit_time(git_dir: &Path) -> Option<DateTime<Utc>> {
    if let Ok(log) = fs::read_to_string(git_dir.join("logs").join("HEAD")) {
        if let Some(line) = log.lines().rev().find(|l| !l.trim().is_empty()) {
            if let Some(ts) = parse_reflog_timestamp(line) {
                return Some(ts);
            }
        }
    }
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let ref_file = head
        .trim()
        .strip_prefix("ref:")
        .map(|r| git_dir.join(r.trim()))
        .unwrap_or_else(|| git_dir.join("HEAD"));
    let modified = fs::metadata(ref_file).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified))
}

/// Reflog lines look like:
/// `<old> <new> Name <email> 1694000000 +0200\tcommit: message`
pub fn parse_reflog_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let head = line.split('\t').next()?;
    let after_email = head.rsplit_once('>')?.1;
    let ts: i64 = after_email.split_whitespace().next()?.parse().ok()?;
    DateTime::<Utc>::from_timestamp(ts, 0)
}

fn status_counts(project: &Path) -> Option<(u32, u32)> {
    if !git_available() {
        return None;
    }
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(project)
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    hide_window(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut modified = 0;
    let mut untracked = 0;
    for line in text.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else if !line.trim().is_empty() {
            modified += 1;
        }
    }
    Some((modified, untracked))
}

#[allow(unused_variables)]
fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_reflog_lines() {
        let line =
            "0000000 abc1234 Lewis <lewis@example.com> 1694000000 +0200\tcommit (initial): hi";
        let ts = parse_reflog_timestamp(line).unwrap();
        assert_eq!(ts.timestamp(), 1694000000);
        assert!(parse_reflog_timestamp("garbage").is_none());
    }

    #[test]
    fn reads_repo_metadata_without_git_binary() {
        let tmp = tempdir().unwrap();
        let git = tmp.path().join(".git");
        fs::create_dir_all(git.join("logs")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            git.join("config"),
            "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = x\n",
        )
        .unwrap();
        fs::write(
            git.join("logs").join("HEAD"),
            "0 1 A <a@b> 1694000000 +0000\tcommit: one\n0 1 A <a@b> 1694100000 +0000\tcommit: two\n",
        )
        .unwrap();
        let info = inspect(tmp.path(), false);
        assert!(info.is_repo);
        assert_eq!(info.branch.as_deref(), Some("main"));
        assert_eq!(info.remote_configured, Some(true));
        assert_eq!(info.last_commit_at.unwrap().timestamp(), 1694100000);
        assert_eq!(info.state, GitState::Unknown);
    }

    #[test]
    fn non_repo() {
        let tmp = tempdir().unwrap();
        let info = inspect(tmp.path(), true);
        assert!(!info.is_repo);
        assert_eq!(info.state, GitState::NoRepo);
    }
}

/// What Git says about a set of candidate artifact directories.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitArtifactFacts {
    /// Relative paths that contain at least one tracked file. Removing these
    /// would delete work that is committed or staged, so they are never
    /// treated as regeneratable however well their name matches a rule.
    pub tracked: BTreeSet<String>,
    /// Relative paths the repository's ignore rules exclude. This is positive
    /// confirmation that the project itself considers them generated.
    pub ignored: BTreeSet<String>,
}

/// Ask Git about candidate artifact directories, in two calls per project
/// rather than one per directory.
///
/// `relative_dirs` are paths relative to the project root, with or without a
/// trailing slash. Returns `None` when Git is unavailable or the project is
/// not a repository, in which case callers keep their rule-based verdict.
pub fn artifact_facts(project: &Path, relative_dirs: &[String]) -> Option<GitArtifactFacts> {
    if relative_dirs.is_empty() || !git_available() || git_dir(project).is_none() {
        return None;
    }
    let specs: Vec<String> = relative_dirs
        .iter()
        .map(|d| d.trim_end_matches('/').to_string())
        .filter(|d| !d.is_empty())
        .collect();
    if specs.is_empty() {
        return None;
    }

    let mut facts = GitArtifactFacts::default();

    // Tracked: any file Git knows about beneath a candidate directory. The
    // output is the file path, so map it back to the directory that contains it.
    if let Some(out) = run_git(project, &["ls-files", "-z", "--"], &specs) {
        for file in out.split('\0').filter(|s| !s.is_empty()) {
            if let Some(dir) = specs.iter().find(|d| {
                file == d.as_str()
                    || file
                        .strip_prefix(d.as_str())
                        .is_some_and(|r| r.starts_with('/'))
            }) {
                facts.tracked.insert(dir.clone());
            }
        }
    }

    // Ignored: `check-ignore` only accepts -z together with --stdin, and
    // feeding the paths in NUL-separated is also the only form that survives
    // newlines in a path. It exits 1 when nothing matches, which is not an
    // error, so a missing result simply means "none of them".
    if let Some(out) = run_git_stdin(project, &["check-ignore", "-z", "--stdin"], &specs) {
        for dir in out.split('\0').filter(|s| !s.is_empty()) {
            facts.ignored.insert(dir.trim_end_matches('/').to_string());
        }
    }

    Some(facts)
}

/// Run a git subcommand that reads NUL-separated paths on stdin.
fn run_git_stdin(project: &Path, args: &[&str], specs: &[String]) -> Option<String> {
    use std::io::Write;
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(project)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_window(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    {
        let mut stdin = child.stdin.take()?;
        for spec in specs {
            if stdin.write_all(spec.as_bytes()).is_err() || stdin.write_all(b"\0").is_err() {
                break;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    (!text.is_empty()).then_some(text)
}

/// Run a git subcommand with pathspecs, returning stdout when it produced any.
fn run_git(project: &Path, args: &[&str], specs: &[String]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(project)
        .args(args)
        .args(specs)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    hide_window(&mut cmd);
    let output = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod artifact_facts_tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A folder whose name matches a cleanup rule but that Git tracks is
    /// real work, not build output. A folder the repository ignores is
    /// confirmed generated.
    #[test]
    fn separates_tracked_folders_from_ignored_ones() {
        if !git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        git(repo, &["init", "-q"]);

        // `dist/` is committed here: some projects ship generated output.
        write(&repo.join("dist/bundle.js"), "console.log(1)");
        // `node_modules/` and `build/` are ignored, as usual.
        write(&repo.join(".gitignore"), "node_modules/\nbuild/\n");
        write(&repo.join("node_modules/left-pad/index.js"), "x");
        write(&repo.join("build/out.o"), "x");
        write(&repo.join("README.md"), "hi");
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-qm", "init"]);

        let dirs = vec![
            "dist/".to_string(),
            "node_modules/".to_string(),
            "build/".to_string(),
        ];
        let facts = artifact_facts(repo, &dirs).expect("a repository with candidates");

        assert!(
            facts.tracked.contains("dist"),
            "committed dist/ must be seen as tracked"
        );
        assert!(!facts.tracked.contains("node_modules"));
        assert!(!facts.tracked.contains("build"));

        assert!(facts.ignored.contains("node_modules"));
        assert!(facts.ignored.contains("build"));
        assert!(!facts.ignored.contains("dist"));
    }

    #[test]
    fn returns_nothing_outside_a_repository() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(artifact_facts(tmp.path(), &["dist/".to_string()]).is_none());
        // No candidates means no work to do.
        assert!(artifact_facts(tmp.path(), &[]).is_none());
    }
}
