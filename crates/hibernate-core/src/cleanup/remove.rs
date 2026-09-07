//! Permanent removal that keeps going when individual files are locked or
//! unreadable, and reports exactly what it could not remove.

use crate::history::FailedPath;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default, Clone)]
pub struct RemovalReport {
    pub bytes_removed: u64,
    pub files_removed: u64,
    pub dirs_removed: u64,
    pub failed: Vec<FailedPath>,
    pub cancelled: bool,
}

const MAX_FAILURES_RECORDED: usize = 200;

/// Remove a directory tree without following symlinks. Symlinks inside the
/// tree are unlinked, never dereferenced.
pub fn remove_tree(root: &Path, cancel: &AtomicBool) -> RemovalReport {
    let mut report = RemovalReport::default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            return report;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(err) => {
                record(&mut report, &dir, err);
                continue;
            }
        };
        dirs.push(dir);
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            match remove_file_or_link(&path, ft.is_symlink()) {
                Ok(()) => {
                    report.bytes_removed += len;
                    report.files_removed += 1;
                }
                Err(err) => record(&mut report, &path, err),
            }
        }
    }

    for dir in dirs.iter().rev() {
        match fs::remove_dir(dir) {
            Ok(()) => report.dirs_removed += 1,
            // Directories left non-empty by an earlier failure are expected.
            Err(err) if err.kind() == io::ErrorKind::DirectoryNotEmpty => {}
            Err(err) if !report.failed.is_empty() && err.raw_os_error() == Some(39) => {}
            Err(err) => record(&mut report, dir, err),
        }
    }
    report
}

fn remove_file_or_link(path: &Path, is_symlink: bool) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(first) => {
            // Windows: directory symlinks/junctions are removed with remove_dir,
            // and read-only files must be made writable first.
            if is_symlink && fs::remove_dir(path).is_ok() {
                return Ok(());
            }
            if first.kind() == io::ErrorKind::PermissionDenied {
                if let Ok(md) = fs::symlink_metadata(path) {
                    let mut perms = md.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    if fs::set_permissions(path, perms).is_ok() {
                        return fs::remove_file(path);
                    }
                }
            }
            Err(first)
        }
    }
}

fn record(report: &mut RemovalReport, path: &Path, err: io::Error) {
    if report.failed.len() < MAX_FAILURES_RECORDED {
        report.failed.push(FailedPath {
            path: path.to_path_buf(),
            error: err.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn removes_a_tree_and_counts_bytes() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("node_modules");
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/f1"), vec![0u8; 100]).unwrap();
        fs::write(root.join("a/f2"), vec![0u8; 50]).unwrap();
        let report = remove_tree(&root, &AtomicBool::new(false));
        assert!(!root.exists());
        assert_eq!(report.bytes_removed, 150);
        assert_eq!(report.files_removed, 2);
        assert_eq!(report.dirs_removed, 3);
        assert!(report.failed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn unlinks_symlinks_without_following() {
        let tmp = tempdir().unwrap();
        let keep = tmp.path().join("keep");
        fs::create_dir_all(&keep).unwrap();
        fs::write(keep.join("important"), "x").unwrap();
        let root = tmp.path().join("target");
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&keep, root.join("link")).unwrap();
        let report = remove_tree(&root, &AtomicBool::new(false));
        assert!(!root.exists());
        assert!(keep.join("important").exists());
        assert!(report.failed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn reports_files_it_cannot_remove() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc_geteuid() } == 0 {
            return; // root ignores directory permissions
        }
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("dist");
        let locked = root.join("locked");
        fs::create_dir_all(&locked).unwrap();
        fs::write(locked.join("f"), "x").unwrap();
        fs::write(root.join("ok"), "y").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
        let report = remove_tree(&root, &AtomicBool::new(false));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(report.files_removed, 1);
        assert!(!report.failed.is_empty());
        assert!(root.exists());
    }

    #[cfg(unix)]
    unsafe fn libc_geteuid() -> u32 {
        extern "C" {
            fn geteuid() -> u32;
        }
        geteuid()
    }
}
