//! Phase 2: measure one project, classifying artifact directories.

use crate::cleanup::rules::{CleanupRule, RuleSet};
use crate::fsx;
use crate::model::{CleanupArtifact, Stack};
use crate::scanner::cache::{fingerprint, SharedTreeCache};
use crate::scanner::traversal::{is_default_ignored, is_user_ignored, MAX_DEPTH};
use chrono::Utc;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[derive(Debug, Default)]
pub struct Measurement {
    pub total_bytes: u64,
    pub file_count: u64,
    /// Newest modification time among non-generated files.
    pub source_modified_at: Option<SystemTime>,
    pub artifacts: Vec<CleanupArtifact>,
    pub protected_entries: Vec<String>,
    pub warnings: Vec<String>,
    /// Artifact / ignored trees whose size came from the cache.
    pub cache_hits: u32,
}

pub struct MeasureOptions<'a> {
    pub follow_symlinks: bool,
    pub rules: &'a RuleSet,
    /// Every discovered project path. Nested projects are skipped so that
    /// their bytes are only counted once.
    pub project_paths: &'a HashSet<PathBuf>,
    pub ignored_paths: &'a [PathBuf],
    /// Fingerprint cache for whole-tree measurements. `None` forces a full
    /// walk of every tree.
    pub cache: Option<&'a SharedTreeCache>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSize {
    /// Bytes on disk that removing this tree would actually free.
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    /// Bytes inside this tree that are hard-linked to something outside it,
    /// so they survive its removal. pnpm's shared store is the usual reason.
    pub shared_elsewhere: u64,
}

struct Frame {
    path: PathBuf,
    /// Inside `.git/` or similar: counted, but never an artifact and never
    /// a source of activity.
    generated: bool,
    depth: usize,
}

const CANCEL_CHECK_EVERY: u64 = 512;

pub fn measure(
    project: &Path,
    stacks: &[Stack],
    restore_hint: &dyn Fn(&CleanupRule) -> Option<String>,
    opts: &MeasureOptions,
    cancel: &AtomicBool,
) -> Measurement {
    let mut m = Measurement::default();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut stack = vec![Frame {
        path: project.to_path_buf(),
        generated: false,
        depth: 0,
    }];
    let mut ops: u64 = 0;

    while let Some(frame) = stack.pop() {
        ops += 1;
        if ops.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.load(Ordering::Relaxed) {
            break;
        }
        let entries = match fs::read_dir(&frame.path) {
            Ok(e) => e,
            Err(err) => {
                m.warnings.push(format!("{}: {err}", frame.path.display()));
                continue;
            }
        };
        let at_root = frame.path == project;

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            if at_root && opts.rules.is_protected(&name) {
                m.protected_entries.push(if meta.is_dir() {
                    format!("{name}/")
                } else {
                    name.clone()
                });
            }

            if fsx::is_link(&meta) {
                if opts.follow_symlinks
                    && !frame.generated
                    && fs::metadata(&child).map(|md| md.is_dir()).unwrap_or(false)
                {
                    if let Ok(c) = fs::canonicalize(&child) {
                        if c.starts_with(project) || !visited.insert(c) {
                            continue;
                        }
                    }
                    stack.push(Frame {
                        path: child,
                        generated: false,
                        depth: frame.depth + 1,
                    });
                    continue;
                }
                m.total_bytes += fsx::allocated_size(&meta);
                m.file_count += 1;
                continue;
            }

            if fsx::is_real_dir(&meta) {
                if opts.project_paths.contains(&child) {
                    continue; // a separate (nested) project
                }
                if is_user_ignored(&child, opts.ignored_paths) {
                    continue;
                }
                if frame.depth >= MAX_DEPTH {
                    continue;
                }
                if !frame.generated {
                    if is_default_ignored(&name) {
                        // `.git/` and friends: counted as a whole tree so the
                        // cache can skip large object stores.
                        let (tree, hit) = measure_tree_cached(&child, opts.cache, cancel);
                        m.total_bytes += tree.bytes;
                        m.file_count += tree.files;
                        m.cache_hits += hit as u32;
                        continue;
                    }
                    if let Some(rule) = opts.rules.match_for(&name, stacks) {
                        let (tree, hit) = measure_tree_cached(&child, opts.cache, cancel);
                        m.cache_hits += hit as u32;
                        m.total_bytes += tree.bytes;
                        m.file_count += tree.files;
                        m.artifacts.push(CleanupArtifact {
                            relative_path: relative(project, &child),
                            path: child,
                            kind: rule.id.clone(),
                            category: rule.category,
                            bytes: tree.bytes,
                            file_count: tree.files,
                            dir_count: tree.dirs,
                            safety: rule.safety,
                            regeneratable: rule.regeneratable,
                            explanation: rule.explanation.clone(),
                            restore_hint: restore_hint(rule),
                            tracked_by_git: false,
                            ignored_by_git: false,
                        });
                        continue;
                    }
                }
                stack.push(Frame {
                    path: child,
                    generated: frame.generated,
                    depth: frame.depth + 1,
                });
                continue;
            }

            // Regular file. Count what it occupies on disk, not its length.
            {
                let md = &meta;
                m.total_bytes += fsx::allocated_size(md);
                m.file_count += 1;
                if !frame.generated {
                    if let Ok(modified) = md.modified() {
                        if m.source_modified_at.map(|t| modified > t).unwrap_or(true) {
                            m.source_modified_at = Some(modified);
                        }
                    }
                }
            }
        }
    }

    m.protected_entries.sort();
    m.protected_entries.truncate(16);
    m.artifacts.sort_by_key(|a| std::cmp::Reverse(a.bytes));
    m
}

/// Measure a tree, reusing the cached size when its fingerprint matches.
/// Returns the size and whether it was a cache hit.
pub fn measure_tree_cached(
    root: &Path,
    cache: Option<&SharedTreeCache>,
    cancel: &AtomicBool,
) -> (TreeSize, bool) {
    let Some(cache) = cache else {
        return (measure_tree(root, cancel), false);
    };
    let now = Utc::now();
    let fp = fingerprint(root);
    if let Some(fp) = fp {
        if let Ok(mut c) = cache.lock() {
            if let Some(size) = c.get(root, fp, now) {
                return (size, true);
            }
        }
    }
    let size = measure_tree(root, cancel);
    if let (Some(fp), false) = (fp, cancel.load(Ordering::Relaxed)) {
        if let Ok(mut c) = cache.lock() {
            c.insert(root, fp, size, now);
        }
    }
    (size, false)
}

/// Size of a whole directory tree. Symlinks count as their own size and are
/// never followed: artifact trees like pnpm's `node_modules/` are full of
/// links that would otherwise be double counted.
pub fn measure_tree(root: &Path, cancel: &AtomicBool) -> TreeSize {
    let mut size = TreeSize {
        bytes: 0,
        files: 0,
        dirs: 1,
        shared_elsewhere: 0,
    };
    let mut stack = vec![root.to_path_buf()];
    let mut links = fsx::LinkAccounting::new();
    let mut ops: u64 = 0;
    while let Some(dir) = stack.pop() {
        ops += 1;
        if ops.is_multiple_of(CANCEL_CHECK_EVERY) && cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(md) = entry.metadata() else { continue };
            if fsx::is_real_dir(&md) {
                size.dirs += 1;
                stack.push(entry.path());
            } else {
                size.files += 1;
                // Shared files are held back until we know whether every
                // link to them lives inside this tree.
                size.bytes += links.observe(&md);
            }
        }
    }
    size.bytes += links.shared_bytes_freed();
    size.shared_elsewhere = links.shared_bytes_elsewhere();
    size
}

/// `apps/web/.next/` style relative path with forward slashes.
pub fn relative(project: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(project).unwrap_or(path);
    let mut s: String = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    s.push('/');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Sizes are now bytes *on disk*, so a file occupies whole blocks.
    /// Assert the content is accounted for, within one block per file.
    #[track_caller]
    fn assert_on_disk(actual: u64, content: u64, files: u64) {
        assert!(
            actual >= content,
            "on-disk size {actual} is below the {content} bytes of content"
        );
        let ceiling = content + (files + 1) * 4096;
        assert!(
            actual <= ceiling,
            "on-disk size {actual} exceeds {content} bytes of content plus block slack ({ceiling})"
        );
    }

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn measures_artifacts_and_totals() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("web");
        write(&p.join("package.json"), 100);
        write(&p.join("src/index.ts"), 200);
        write(&p.join("node_modules/a/index.js"), 5000);
        write(&p.join("node_modules/b/index.js"), 3000);
        write(&p.join(".next/cache/x"), 1000);
        write(&p.join(".git/objects/aa"), 700);
        write(&p.join(".env"), 10);
        // Nested Next output inside a sub folder still counts.
        write(&p.join("apps/docs/.next/y"), 400);
        // A directory named target is not an artifact for a Node project.
        write(&p.join("target/thing"), 50);

        let rules = RuleSet::builtin();
        let paths = HashSet::new();
        let opts = MeasureOptions {
            follow_symlinks: false,
            rules: &rules,
            project_paths: &paths,
            ignored_paths: &[],
            cache: None,
        };
        let m = measure(
            &p,
            &[Stack::Node],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_on_disk(
            m.total_bytes,
            100 + 200 + 5000 + 3000 + 1000 + 700 + 10 + 400 + 50,
            m.file_count,
        );
        assert_eq!(m.file_count, 9);
        let kinds: Vec<String> = m
            .artifacts
            .iter()
            .map(|a| a.relative_path.clone())
            .collect();
        assert_eq!(
            kinds,
            vec!["node_modules/", ".next/", "apps/docs/.next/"],
            "largest artifact first"
        );
        assert_on_disk(m.artifacts[0].bytes, 8000, 2);
        assert_on_disk(m.artifacts[1].bytes, 1000, 1);
        assert_on_disk(m.artifacts[2].bytes, 400, 1);
        assert_eq!(m.artifacts[0].file_count, 2);
        assert_eq!(m.artifacts[0].dir_count, 3);
        assert_eq!(
            m.protected_entries,
            vec![".env", ".git/", "package.json", "src/"]
        );
    }

    #[test]
    fn nested_projects_are_excluded_from_parent() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("ws");
        write(&p.join("Cargo.toml"), 10);
        write(&p.join("target/debug/bin"), 900);
        write(&p.join("frontend/package.json"), 10);
        write(&p.join("frontend/node_modules/x"), 500);
        let rules = RuleSet::builtin();
        let mut paths = HashSet::new();
        paths.insert(p.join("frontend"));
        let opts = MeasureOptions {
            follow_symlinks: false,
            rules: &rules,
            project_paths: &paths,
            ignored_paths: &[],
            cache: None,
        };
        let m = measure(
            &p,
            &[Stack::Rust],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_on_disk(m.total_bytes, 910, m.file_count);
        assert_eq!(m.artifacts.len(), 1);
        assert_eq!(m.artifacts[0].kind, "rust-target");
    }

    /// Sizes must describe what removal frees, not what the files claim.
    /// `node_modules` is thousands of tiny files that each occupy a whole
    /// block, so lengths understate it; sparse files and pnpm's hard-linked
    /// store make lengths overstate it.
    #[cfg(unix)]
    #[test]
    fn measures_what_removal_would_actually_free() {
        let tmp = tempdir().unwrap();
        let store = tmp.path().join("store");
        let nm = tmp.path().join("node_modules");
        fs::create_dir_all(&store).unwrap();
        fs::create_dir_all(nm.join("pkg")).unwrap();

        // Shared with a store outside the tree: removing node_modules frees
        // nothing for it, because the store still links to the same bytes.
        let shared = store.join("blob.bin");
        fs::write(&shared, vec![b'x'; 100_000]).unwrap();
        fs::hard_link(&shared, nm.join("pkg/blob.bin")).unwrap();

        // A tiny file still occupies a whole block.
        fs::write(nm.join("pkg/small.bin"), vec![b'x'; 10]).unwrap();

        let size = measure_tree(&nm, &AtomicBool::new(false));

        assert_eq!(size.files, 2);
        assert!(
            size.shared_elsewhere >= 100_000,
            "the hard-linked blob is shared outside the tree, got {}",
            size.shared_elsewhere
        );
        assert!(
            size.bytes < 100_000,
            "bytes must exclude content that survives removal, got {}",
            size.bytes
        );
        assert!(
            size.bytes > 10,
            "a 10-byte file still occupies a block, got {}",
            size.bytes
        );

        // Once the outside link goes, the bytes really would be freed.
        fs::remove_file(&shared).unwrap();
        let after = measure_tree(&nm, &AtomicBool::new(false));
        assert_eq!(after.shared_elsewhere, 0);
        assert!(
            after.bytes >= 100_000,
            "with no outside link the blob counts, got {}",
            after.bytes
        );
    }

    /// A sparse file reports a length it does not occupy.
    #[cfg(unix)]
    #[test]
    fn sparse_files_are_not_counted_as_their_length() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("target");
        fs::create_dir_all(&dir).unwrap();
        let f = fs::File::create(dir.join("sparse.bin")).unwrap();
        f.set_len(64 * 1024 * 1024).unwrap();
        drop(f);

        let size = measure_tree(&dir, &AtomicBool::new(false));
        assert!(
            size.bytes < 1024 * 1024,
            "a sparse file occupies almost nothing, got {}",
            size.bytes
        );
    }

    #[test]
    fn cached_trees_are_reused_until_they_change() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("web");
        write(&p.join("package.json"), 10);
        for i in 0..250 {
            write(
                &p.join(format!("node_modules/pkg{}/index.js", i % 25))
                    .with_file_name(format!("f{i}.js")),
                100,
            );
        }
        let rules = RuleSet::builtin();
        let paths = HashSet::new();
        let cache = std::sync::Mutex::new(crate::scanner::cache::TreeCache::default());
        let opts = MeasureOptions {
            follow_symlinks: false,
            rules: &rules,
            project_paths: &paths,
            ignored_paths: &[],
            cache: Some(&cache),
        };
        let first = measure(
            &p,
            &[Stack::Node],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_eq!(first.cache_hits, 0);
        let second = measure(
            &p,
            &[Stack::Node],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_eq!(second.cache_hits, 1);
        assert_eq!(second.artifacts[0].bytes, first.artifacts[0].bytes);
        // Change deep inside a package: the fingerprint moves, so it is re-measured.
        write(&p.join("node_modules/pkg3/new.js"), 5000);
        let third = measure(
            &p,
            &[Stack::Node],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_eq!(third.cache_hits, 0);
        assert!(
            third.artifacts[0].bytes > first.artifacts[0].bytes,
            "the added file grows the measured size"
        );
    }

    #[test]
    fn generated_files_do_not_affect_activity() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("proj");
        write(&p.join("package.json"), 10);
        write(&p.join("node_modules/fresh"), 10);
        let old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        let f = fs::File::options()
            .write(true)
            .open(p.join("package.json"))
            .unwrap();
        f.set_modified(old).unwrap();
        let rules = RuleSet::builtin();
        let paths = HashSet::new();
        let opts = MeasureOptions {
            follow_symlinks: false,
            rules: &rules,
            project_paths: &paths,
            ignored_paths: &[],
            cache: None,
        };
        let m = measure(
            &p,
            &[Stack::Node],
            &|_| None,
            &opts,
            &AtomicBool::new(false),
        );
        assert_eq!(m.source_modified_at, Some(old));
    }
}
