//! Fingerprint cache for expensive directory trees (`node_modules/`,
//! `target/`, `.git/`). A tree's fingerprint is built from the modification
//! times of the directory, its children and its grandchildren, which is a
//! handful of `readdir` calls instead of walking every file. When the
//! fingerprint matches the previous scan, the cached size is reused.

use crate::config::{load_json, save_json};
use crate::scanner::size::TreeSize;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedTree {
    pub fingerprint: u64,
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    pub measured_at: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TreeCache {
    pub entries: HashMap<PathBuf, CachedTree>,
    #[serde(skip)]
    touched: HashSet<PathBuf>,
    #[serde(skip)]
    pub hits: usize,
    #[serde(skip)]
    pub misses: usize,
}

/// Entries older than this are re-measured regardless of fingerprint.
pub const MAX_AGE_DAYS: i64 = 30;

/// Trees with fewer entries than this are not worth caching.
const MIN_FILES_TO_CACHE: u64 = 200;

/// Stop fingerprinting after this many grandchild listings to bound the cost
/// on pathological trees; such trees simply never hit the cache.
const MAX_CHILD_DIRS: usize = 4000;

impl TreeCache {
    pub fn load(path: &Path) -> Self {
        load_json(path)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        save_json(path, self)
    }

    /// Look up a tree. Marks the path as touched so it survives pruning.
    pub fn get(&mut self, path: &Path, fingerprint: u64, now: DateTime<Utc>) -> Option<TreeSize> {
        self.touched.insert(path.to_path_buf());
        let entry = self.entries.get(path)?;
        if entry.fingerprint != fingerprint || (now - entry.measured_at).num_days() > MAX_AGE_DAYS {
            self.misses += 1;
            return None;
        }
        self.hits += 1;
        Some(TreeSize {
            bytes: entry.bytes,
            files: entry.files,
            dirs: entry.dirs,
        })
    }

    pub fn insert(&mut self, path: &Path, fingerprint: u64, size: TreeSize, now: DateTime<Utc>) {
        if size.files < MIN_FILES_TO_CACHE {
            return;
        }
        self.touched.insert(path.to_path_buf());
        self.entries.insert(
            path.to_path_buf(),
            CachedTree {
                fingerprint,
                bytes: size.bytes,
                files: size.files,
                dirs: size.dirs,
                measured_at: now,
            },
        );
    }

    /// Drop entries for trees that the last scan did not see at all
    /// (removed artifacts, folders no longer scanned). Call after a
    /// complete, uncancelled scan.
    pub fn prune_untouched(&mut self) {
        let touched = std::mem::take(&mut self.touched);
        self.entries.retain(|p, _| touched.contains(p));
    }

    pub fn forget(&mut self, path: &Path) {
        self.entries.remove(path);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.touched.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Shared handle used by the scanner's worker threads.
pub type SharedTreeCache = Mutex<TreeCache>;

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *hash ^= *b as u64;
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn hash_time(hash: &mut u64, t: SystemTime) {
    let nanos = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    hash_bytes(hash, &nanos.to_le_bytes());
}

/// Fingerprint a directory from two levels of metadata. `None` when the
/// directory cannot be read.
pub fn fingerprint(dir: &Path) -> Option<u64> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let meta = fs::metadata(dir).ok()?;
    hash_time(&mut hash, meta.modified().ok()?);

    let mut child_dirs: Vec<PathBuf> = Vec::new();
    let mut children: Vec<(String, u64, Option<SystemTime>, bool)> = Vec::new();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let Ok(md) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = md.is_dir() && !md.file_type().is_symlink();
        children.push((name, md.len(), md.modified().ok(), is_dir));
        if is_dir {
            child_dirs.push(entry.path());
        }
    }
    // Sort so the hash does not depend on directory iteration order.
    children.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, len, modified, is_dir) in &children {
        hash_bytes(&mut hash, name.as_bytes());
        hash_bytes(&mut hash, &len.to_le_bytes());
        hash_bytes(&mut hash, &[*is_dir as u8]);
        if let Some(m) = modified {
            hash_time(&mut hash, *m);
        }
    }

    if child_dirs.len() > MAX_CHILD_DIRS {
        return None;
    }
    child_dirs.sort();
    for child in child_dirs {
        let Ok(entries) = fs::read_dir(&child) else {
            continue;
        };
        let mut grand: Vec<(String, u64, Option<SystemTime>)> = Vec::new();
        for entry in entries.flatten() {
            let Ok(md) = entry.metadata() else { continue };
            grand.push((
                entry.file_name().to_string_lossy().into_owned(),
                md.len(),
                md.modified().ok(),
            ));
        }
        grand.sort_by(|a, b| a.0.cmp(&b.0));
        hash_bytes(&mut hash, &(grand.len() as u64).to_le_bytes());
        for (name, len, modified) in grand {
            hash_bytes(&mut hash, name.as_bytes());
            hash_bytes(&mut hash, &len.to_le_bytes());
            if let Some(m) = modified {
                hash_time(&mut hash, m);
            }
        }
    }
    Some(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn fingerprint_changes_when_nested_content_changes() {
        let tmp = tempdir().unwrap();
        let nm = tmp.path().join("node_modules");
        write(&nm.join("a/index.js"), 10);
        write(&nm.join("b/index.js"), 10);
        let f1 = fingerprint(&nm).unwrap();
        assert_eq!(f1, fingerprint(&nm).unwrap(), "stable across calls");
        // Grandchild-level change: a new file inside a package.
        write(&nm.join("a/extra.js"), 10);
        let f2 = fingerprint(&nm).unwrap();
        assert_ne!(f1, f2);
        // Size change of a grandchild file.
        write(&nm.join("a/extra.js"), 20);
        assert_ne!(f2, fingerprint(&nm).unwrap());
    }

    #[test]
    fn cache_hits_only_on_matching_fingerprint_and_prunes() {
        let mut cache = TreeCache::default();
        let now = Utc::now();
        let p = Path::new("/x/node_modules");
        let size = TreeSize {
            bytes: 100,
            files: 500,
            dirs: 10,
        };
        assert!(cache.get(p, 1, now).is_none());
        cache.insert(p, 1, size, now);
        assert_eq!(cache.get(p, 1, now).unwrap().bytes, 100);
        assert!(cache.get(p, 2, now).is_none());
        assert!(cache
            .get(p, 1, now + chrono::Duration::days(MAX_AGE_DAYS + 1))
            .is_none());
        // Small trees are not cached.
        cache.insert(
            Path::new("/x/small"),
            1,
            TreeSize {
                bytes: 1,
                files: 3,
                dirs: 1,
            },
            now,
        );
        assert_eq!(cache.len(), 1);
        // Pruning keeps only touched paths.
        let mut fresh = TreeCache::default();
        fresh.insert(p, 1, size, now);
        fresh.insert(Path::new("/y/target"), 1, size, now);
        fresh.touched.clear();
        fresh.get(p, 1, now);
        fresh.prune_untouched();
        assert_eq!(fresh.len(), 1);
    }

    #[test]
    fn round_trips_through_json() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("cache.json");
        let mut cache = TreeCache::default();
        cache.insert(
            Path::new("/x/target"),
            7,
            TreeSize {
                bytes: 9,
                files: 900,
                dirs: 3,
            },
            Utc::now(),
        );
        cache.save(&file).unwrap();
        let loaded = TreeCache::load(&file);
        assert_eq!(
            loaded
                .entries
                .get(Path::new("/x/target"))
                .unwrap()
                .fingerprint,
            7
        );
    }
}
