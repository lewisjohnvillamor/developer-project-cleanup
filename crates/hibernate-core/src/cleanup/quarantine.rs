//! Application quarantine: removed artifacts are moved into the app's data
//! directory and can be restored from History until they expire.
//!
//! Layout: `quarantine/<YYYY-MM-DD>/<entry id>/<project>/<artifact path>`.

use crate::cleanup::remove::remove_tree;
use chrono::{DateTime, NaiveDate, Utc};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineBatch {
    pub entry_id: String,
    pub date: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub file_count: u64,
    pub projects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Quarantine {
    root: PathBuf,
}

impl Quarantine {
    pub fn new(root: &Path) -> Self {
        Quarantine {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn batch_dir(&self, entry_id: &str, now: DateTime<Utc>) -> PathBuf {
        self.root
            .join(now.format("%Y-%m-%d").to_string())
            .join(entry_id)
    }

    /// Move `source` into the batch. Uses a rename when possible and falls
    /// back to copy + delete across filesystems.
    pub fn quarantine(
        &self,
        batch_dir: &Path,
        project_name: &str,
        relative_path: &str,
        source: &Path,
        cancel: &AtomicBool,
    ) -> io::Result<PathBuf> {
        let rel = relative_path.trim_matches('/');
        let mut dest = batch_dir.join(sanitise(project_name)).join(rel);
        if dest.exists() {
            let stamp = Utc::now().format("%H%M%S%3f").to_string();
            dest = dest.with_file_name(format!(
                "{}-{stamp}",
                dest.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::rename(source, &dest) {
            Ok(()) => Ok(dest),
            Err(rename_err) => {
                // Cross-device: copy then remove.
                copy_tree(source, &dest).map_err(|e| {
                    io::Error::new(e.kind(), format!("{e} (after rename failed: {rename_err})"))
                })?;
                let report = remove_tree(source, cancel);
                if !report.failed.is_empty() {
                    return Err(io::Error::other(format!(
                        "copied to quarantine but {} item(s) could not be removed from the project",
                        report.failed.len()
                    )));
                }
                Ok(dest)
            }
        }
    }

    /// Move a quarantined artifact back to its original location.
    pub fn restore(&self, quarantine_path: &Path, original: &Path) -> io::Result<()> {
        if !quarantine_path.starts_with(&self.root) {
            return Err(io::Error::other("path is not inside the quarantine folder"));
        }
        if !quarantine_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "quarantined copy no longer exists",
            ));
        }
        if original.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a folder already exists at the original location",
            ));
        }
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::rename(quarantine_path, original) {
            Ok(()) => Ok(()),
            Err(_) => {
                copy_tree(quarantine_path, original)?;
                let report = remove_tree(quarantine_path, &AtomicBool::new(false));
                if !report.failed.is_empty() {
                    log::warn!(
                        "quarantine copy left behind at {}",
                        quarantine_path.display()
                    );
                }
                Ok(())
            }
        }
    }

    /// Delete batches older than `retention_days`. Returns the removed dirs.
    pub fn purge_expired(&self, retention_days: u32, now: DateTime<Utc>) -> Vec<PathBuf> {
        let mut removed = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return removed;
        };
        let cutoff = now.date_naive() - chrono::Duration::days(retention_days as i64);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(date) = NaiveDate::parse_from_str(&name, "%Y-%m-%d") else {
                continue;
            };
            if date < cutoff {
                let path = entry.path();
                let report = remove_tree(&path, &AtomicBool::new(false));
                if report.failed.is_empty() {
                    removed.push(path);
                }
            }
        }
        removed
    }

    /// Every batch on disk, newest first.
    pub fn list_batches(&self) -> Vec<QuarantineBatch> {
        let mut out = Vec::new();
        let Ok(days) = fs::read_dir(&self.root) else {
            return out;
        };
        for day in days.flatten() {
            let day_name = day.file_name().to_string_lossy().into_owned();
            if NaiveDate::parse_from_str(&day_name, "%Y-%m-%d").is_err() {
                continue;
            }
            let Ok(batches) = fs::read_dir(day.path()) else {
                continue;
            };
            for batch in batches.flatten() {
                if !batch.path().is_dir() {
                    continue;
                }
                let projects: Vec<String> = fs::read_dir(batch.path())
                    .map(|it| {
                        it.flatten()
                            .filter(|e| e.path().is_dir())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let size =
                    crate::scanner::size::measure_tree(&batch.path(), &AtomicBool::new(false));
                out.push(QuarantineBatch {
                    entry_id: batch.file_name().to_string_lossy().into_owned(),
                    date: day_name.clone(),
                    path: batch.path(),
                    bytes: size.bytes,
                    file_count: size.files,
                    projects,
                });
            }
        }
        out.sort_by(|a, b| b.entry_id.cmp(&a.entry_id));
        out
    }

    /// Permanently delete one batch. The path must be inside the quarantine.
    pub fn purge_batch(&self, entry_id: &str) -> io::Result<u64> {
        let batch = self
            .list_batches()
            .into_iter()
            .find(|b| b.entry_id == entry_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such quarantine batch"))?;
        if !batch.path.starts_with(&self.root) || batch.path == self.root {
            return Err(io::Error::other(
                "refusing to purge outside the quarantine folder",
            ));
        }
        let report = remove_tree(&batch.path, &AtomicBool::new(false));
        if !report.failed.is_empty() {
            return Err(io::Error::other(format!(
                "{} item(s) could not be removed",
                report.failed.len()
            )));
        }
        // Remove the day folder when it is empty.
        if let Some(day) = batch.path.parent() {
            let _ = fs::remove_dir(day);
        }
        Ok(report.bytes_removed)
    }

    /// Bytes currently held in quarantine.
    pub fn size(&self) -> u64 {
        crate::scanner::size::measure_tree(&self.root, &AtomicBool::new(false)).bytes
    }
}

fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "project".into()
    } else {
        cleaned
    }
}

/// Recursive copy that recreates symlinks instead of following them.
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let mut stack = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        fs::create_dir_all(&to)?;
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let target = to.join(entry.file_name());
            if crate::fsx::is_link(&meta) {
                copy_symlink(&entry.path(), &target)?;
            } else if crate::fsx::is_real_dir(&meta) {
                stack.push((entry.path(), target));
            } else {
                fs::copy(entry.path(), target)?;
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target = fs::read_link(from)?;
    std::os::unix::fs::symlink(target, to)
}

#[cfg(windows)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target = fs::read_link(from)?;
    let resolved = if target.is_absolute() {
        target.clone()
    } else {
        from.parent()
            .map(|p| p.join(&target))
            .unwrap_or(target.clone())
    };
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

#[cfg(not(any(unix, windows)))]
fn copy_symlink(_from: &Path, _to: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn quarantine_and_restore_round_trip() {
        let tmp = tempdir().unwrap();
        let q = Quarantine::new(&tmp.path().join("quarantine"));
        let project = tmp.path().join("web");
        let nm = project.join("node_modules");
        fs::create_dir_all(nm.join("pkg")).unwrap();
        fs::write(nm.join("pkg/index.js"), "hi").unwrap();

        let now = Utc::now();
        let batch = q.batch_dir("entry-1", now);
        let dest = q
            .quarantine(&batch, "web", "node_modules/", &nm, &AtomicBool::new(false))
            .unwrap();
        assert!(!nm.exists());
        assert!(dest.join("pkg/index.js").exists());
        assert!(dest.starts_with(q.root()));

        q.restore(&dest, &nm).unwrap();
        assert!(nm.join("pkg/index.js").exists());
        assert!(!dest.exists());
        // Restoring twice fails cleanly.
        assert!(q.restore(&dest, &nm).is_err());
    }

    #[test]
    fn lists_and_purges_batches() {
        let tmp = tempdir().unwrap();
        let q = Quarantine::new(&tmp.path().join("quarantine"));
        let src = tmp.path().join("web/node_modules");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.js"), "xx").unwrap();
        let batch = q.batch_dir("e1", Utc::now());
        q.quarantine(
            &batch,
            "web",
            "node_modules/",
            &src,
            &AtomicBool::new(false),
        )
        .unwrap();
        let batches = q.list_batches();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].entry_id, "e1");
        assert_eq!(batches[0].projects, vec!["web"]);
        assert_eq!(batches[0].bytes, 2);
        assert!(q.purge_batch("nope").is_err());
        assert_eq!(q.purge_batch("e1").unwrap(), 2);
        assert!(q.list_batches().is_empty());
    }

    #[test]
    fn purges_old_batches_only() {
        let tmp = tempdir().unwrap();
        let q = Quarantine::new(tmp.path());
        fs::create_dir_all(tmp.path().join("2020-01-01/old")).unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        fs::create_dir_all(tmp.path().join(&today).join("new")).unwrap();
        fs::create_dir_all(tmp.path().join("not-a-date")).unwrap();
        let removed = q.purge_expired(7, Utc::now());
        assert_eq!(removed.len(), 1);
        assert!(!tmp.path().join("2020-01-01").exists());
        assert!(tmp.path().join(&today).exists());
        assert!(tmp.path().join("not-a-date").exists());
    }
}
