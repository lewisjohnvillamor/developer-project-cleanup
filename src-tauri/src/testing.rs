//! A scratch directory for tests.
//!
//! `tempfile` would do this in one line, but as a dev-dependency of *this*
//! crate it drags `windows-sys 0.61` and `windows-link`'s raw-dylib imports
//! into a test binary that already links Tauri's own (older) Windows stack.
//! The mixture produced an import the loader could not resolve, and the test
//! binary died with STATUS_ENTRYPOINT_NOT_FOUND before running a single test.
//! Sixteen lines here keep the crate's dependency graph identical between
//! `cargo build` and `cargo test`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let unique = format!(
            "project-hibernate-{tag}-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create scratch directory");
        TempDir(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
