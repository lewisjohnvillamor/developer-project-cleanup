//! Process-wide state shared by every command.

use chrono::{DateTime, Utc};
use hibernate_core::cleanup::quarantine::Quarantine;
use hibernate_core::config::{load_json, save_json, AppPaths, AppState, Settings};
use hibernate_core::history::{HistoryStore, ScanTrend};
use hibernate_core::model::Project;
use hibernate_core::scanner::activity;
use hibernate_core::scanner::{ScanSummary, TreeCache};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tauri::{AppHandle, Emitter};

/// The last scan, kept in memory and on disk so the app opens with data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanSnapshot {
    pub projects: Vec<Project>,
    pub summary: Option<ScanSummary>,
    pub scanned_at: Option<DateTime<Utc>>,
}

pub struct Job {
    pub running: AtomicBool,
    pub cancel: Arc<AtomicBool>,
}

impl Job {
    fn new() -> Self {
        Job {
            running: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Try to claim the job. Returns `false` when it is already running.
    pub fn begin(&self) -> bool {
        if self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancel.store(false, Ordering::SeqCst);
        true
    }

    pub fn end(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// A write to disk that did not happen. The app keeps running on its
/// in-memory copy, but something it promised to remember is not on disk: the
/// user has to know, because the next launch will disagree with the screen in
/// front of them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// What the app was doing, phrased for a person.
    pub operation: String,
    /// The file it was writing.
    pub path: PathBuf,
    pub message: String,
}

pub struct AppCtx {
    pub paths: AppPaths,
    pub settings: Mutex<Settings>,
    pub state: Mutex<AppState>,
    pub history: Mutex<HistoryStore>,
    pub snapshot: Mutex<ScanSnapshot>,
    pub tree_cache: Arc<Mutex<TreeCache>>,
    pub trend: Mutex<ScanTrend>,
    pub scan: Job,
    pub hibernate: Job,
    pub wake: Job,
    pub caches: Job,
    /// Set once during Tauri setup so background threads can reach the window.
    /// Unset in tests and before setup, where reporting falls back to the log.
    app: OnceLock<AppHandle>,
}

impl AppCtx {
    pub fn load() -> Self {
        let paths = AppPaths::default_locations();
        if let Err(err) = paths.ensure() {
            log::warn!("cannot create data dir {}: {err}", paths.data_dir.display());
        }
        let settings = Settings::load(&paths);
        let mut state = AppState::load(&paths);
        state.prune(Utc::now());
        let history = HistoryStore::load(&paths);
        let snapshot: ScanSnapshot = load_json(&paths.last_scan_file);
        let tree_cache = TreeCache::load(&paths.tree_cache_file);
        let trend = ScanTrend::load(&paths);
        AppCtx {
            paths,
            settings: Mutex::new(settings),
            state: Mutex::new(state),
            history: Mutex::new(history),
            snapshot: Mutex::new(snapshot),
            tree_cache: Arc::new(Mutex::new(tree_cache)),
            trend: Mutex::new(trend),
            scan: Job::new(),
            hibernate: Job::new(),
            wake: Job::new(),
            caches: Job::new(),
            app: OnceLock::new(),
        }
    }

    /// Give the context a handle to the window so failures can be shown.
    pub fn attach(&self, app: AppHandle) {
        let _ = self.app.set(app);
    }

    /// Report a failure the user would otherwise never see, and return its
    /// message. Every write to disk goes through here instead of being
    /// discarded at the call site: a cleanup the app forgets to record is a
    /// cleanup the user cannot undo, and silence would make that look like
    /// success.
    fn report(&self, operation: &str, path: &Path, err: &io::Error) -> String {
        let message = err.to_string();
        log::error!("{operation} failed ({}): {message}", path.display());
        if let Some(app) = self.app.get() {
            // Nothing useful is left to do if the event itself cannot be
            // delivered; the log line above is the fallback record.
            let _ = app.emit(
                "app-error",
                AppError {
                    operation: operation.to_string(),
                    path: path.to_path_buf(),
                    message: message.clone(),
                },
            );
        }
        message
    }

    fn checked(&self, operation: &str, path: &Path, result: io::Result<()>) -> Result<(), String> {
        result.map_err(|err| self.report(operation, path, &err))
    }

    /// The size cache is an optimisation, so a failure costs a slower next
    /// scan and nothing else. It is still reported rather than dropped.
    pub fn save_tree_cache(&self) {
        let result = Self::lock(&self.tree_cache).save(&self.paths.tree_cache_file);
        let _ = self.checked(
            "Saving cached folder sizes",
            &self.paths.tree_cache_file,
            result,
        );
    }

    pub fn save_trend(&self) {
        let result = Self::lock(&self.trend).save(&self.paths);
        let _ = self.checked("Saving scan history", &self.paths.scan_trend_file, result);
    }

    pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn quarantine(&self) -> Quarantine {
        Quarantine::new(&self.paths.quarantine_dir)
    }

    pub fn purge_quarantine(&self) {
        let days = Self::lock(&self.settings).quarantine_retention_days;
        let removed = self.quarantine().purge_expired(days, Utc::now());
        if !removed.is_empty() {
            log::info!("expired {} quarantine batch(es)", removed.len());
        }
    }

    pub fn save_settings(&self) -> Result<(), String> {
        let result = Self::lock(&self.settings).save(&self.paths);
        self.checked("Saving settings", &self.paths.settings_file, result)
    }

    /// Protected flags, hidden projects and hibernation records. Losing this
    /// is the worst of the three: the app would forget that a project was
    /// hibernated, and with it how to wake the project up.
    pub fn save_state(&self) -> Result<(), String> {
        let result = Self::lock(&self.state).save(&self.paths);
        self.checked("Saving project state", &self.paths.state_file, result)
    }

    pub fn save_history(&self) -> Result<(), String> {
        let result = Self::lock(&self.history).save(&self.paths);
        self.checked("Saving cleanup history", &self.paths.history_file, result)
    }

    pub fn save_snapshot(&self) {
        let snap = Self::lock(&self.snapshot).clone();
        let result = save_json(&self.paths.last_scan_file, &snap);
        let _ = self.checked("Saving the last scan", &self.paths.last_scan_file, result);
    }

    /// Re-derive a cached project's status after its persisted state changed.
    pub fn refresh_status(&self, project: &mut Project) {
        let settings = Self::lock(&self.settings);
        project.status = activity::status(
            project.last_activity_at,
            project.protected,
            project.hibernation.as_ref(),
            project.reclaimable_bytes,
            settings.dormant_after_days,
            Utc::now(),
        );
    }

    /// Run `f` on the cached project with this id and return the result.
    pub fn with_project<R>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Project) -> R,
    ) -> Result<R, String> {
        let mut snap = Self::lock(&self.snapshot);
        let project = snap
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "Project is not part of the last scan. Scan again.".to_string())?;
        Ok(f(project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn ctx_with_paths(paths: AppPaths) -> AppCtx {
        AppCtx {
            paths,
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(AppState::default()),
            history: Mutex::new(HistoryStore::default()),
            snapshot: Mutex::new(ScanSnapshot::default()),
            tree_cache: Arc::new(Mutex::new(TreeCache::default())),
            trend: Mutex::new(ScanTrend::default()),
            scan: Job::new(),
            hibernate: Job::new(),
            wake: Job::new(),
            caches: Job::new(),
            app: OnceLock::new(),
        }
    }

    /// A write that cannot happen must come back as an error. The app keeps
    /// working from memory either way, so a silent failure looks exactly like
    /// success until the next launch contradicts it — and by then the record
    /// of what was removed, and how to restore it, is gone.
    #[test]
    fn a_write_that_fails_is_reported_rather_than_swallowed() {
        let tmp = TempDir::new("blocked");
        // A regular file where the data directory should be, so every write
        // beneath it fails the way a full or read-only disk would.
        let blocked = tmp.path().join("data");
        std::fs::write(&blocked, b"not a directory").unwrap();
        let ctx = ctx_with_paths(AppPaths::in_dir(&blocked));

        for (what, result) in [
            ("settings", ctx.save_settings()),
            ("project state", ctx.save_state()),
            ("cleanup history", ctx.save_history()),
        ] {
            let err = result.expect_err("saving into a file must fail");
            assert!(!err.is_empty(), "{what} failed without saying why");
        }
    }

    /// The same write succeeds when the directory is real, so the test above
    /// is failing for the reason it claims.
    #[test]
    fn a_write_that_can_happen_succeeds() {
        let tmp = TempDir::new("writable");
        let paths = AppPaths::in_dir(tmp.path());
        paths.ensure().unwrap();
        let ctx = ctx_with_paths(paths);
        ctx.save_settings().unwrap();
        ctx.save_state().unwrap();
        ctx.save_history().unwrap();
        assert!(ctx.paths.history_file.exists());
    }
}
