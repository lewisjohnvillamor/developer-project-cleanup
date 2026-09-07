//! Process-wide state shared by every command.

use chrono::{DateTime, Utc};
use hibernate_core::cleanup::quarantine::Quarantine;
use hibernate_core::config::{load_json, save_json, AppPaths, AppState, Settings};
use hibernate_core::history::{HistoryStore, ScanTrend};
use hibernate_core::model::Project;
use hibernate_core::scanner::activity;
use hibernate_core::scanner::{ScanSummary, TreeCache};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

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
        }
    }

    pub fn save_tree_cache(&self) {
        let cache = Self::lock(&self.tree_cache);
        if let Err(err) = cache.save(&self.paths.tree_cache_file) {
            log::warn!("cannot save tree cache: {err}");
        }
    }

    pub fn save_trend(&self) {
        if let Err(err) = Self::lock(&self.trend).save(&self.paths) {
            log::warn!("cannot save scan trend: {err}");
        }
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
        Self::lock(&self.settings)
            .save(&self.paths)
            .map_err(|e| e.to_string())
    }

    pub fn save_state(&self) -> Result<(), String> {
        Self::lock(&self.state)
            .save(&self.paths)
            .map_err(|e| e.to_string())
    }

    pub fn save_history(&self) -> Result<(), String> {
        Self::lock(&self.history)
            .save(&self.paths)
            .map_err(|e| e.to_string())
    }

    pub fn save_snapshot(&self) {
        let snap = Self::lock(&self.snapshot).clone();
        if let Err(err) = save_json(&self.paths.last_scan_file, &snap) {
            log::warn!("cannot save last scan: {err}");
        }
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
