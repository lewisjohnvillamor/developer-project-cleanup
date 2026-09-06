//! Settings, persisted per-project state, and where they live on disk.

use crate::cleanup::rules::{CleanupRule, RuleSet};
use crate::model::HibernationRecord;
use crate::scanner::ScanOptions;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "ProjectHibernate";
pub const DATA_DIR_ENV: &str = "PROJECT_HIBERNATE_DATA_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// What happens to removed artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    /// Move to the operating system's Trash / Recycle Bin.
    #[default]
    Trash,
    /// Move into the app's quarantine folder, restorable from History.
    Quarantine,
    /// Delete immediately.
    Permanent,
}

impl Disposition {
    pub fn label(self) -> &'static str {
        match self {
            Disposition::Trash => "Recycle Bin / Trash",
            Disposition::Quarantine => "Quarantine",
            Disposition::Permanent => "Permanent delete",
        }
    }

    pub fn parse(s: &str) -> Option<Disposition> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "trash" | "recycle" | "bin" => Disposition::Trash,
            "quarantine" => Disposition::Quarantine,
            "permanent" | "delete" => Disposition::Permanent,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub remember_folders: bool,
    pub scan_roots: Vec<PathBuf>,
    /// 0 = automatic.
    pub max_concurrency: usize,
    pub follow_symlinks: bool,
    pub scan_hidden: bool,
    pub inspect_git: bool,
    pub disposition: Disposition,
    pub quarantine_retention_days: u32,
    /// Bulk-select amber (review) artifacts too.
    pub include_review_items: bool,
    pub dormant_after_days: u32,
    pub ignored_paths: Vec<PathBuf>,
    pub custom_rules: Vec<CleanupRule>,
    pub protected_patterns: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            remember_folders: true,
            scan_roots: Vec::new(),
            max_concurrency: 0,
            follow_symlinks: false,
            scan_hidden: true,
            inspect_git: true,
            disposition: Disposition::Trash,
            quarantine_retention_days: 7,
            include_review_items: false,
            dormant_after_days: 14,
            ignored_paths: Vec::new(),
            custom_rules: Vec::new(),
            protected_patterns: Vec::new(),
        }
    }
}

impl Settings {
    pub fn rule_set(&self) -> RuleSet {
        RuleSet::with_custom(&self.custom_rules, &self.protected_patterns)
    }

    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            follow_symlinks: self.follow_symlinks,
            scan_hidden: self.scan_hidden,
            max_concurrency: self.max_concurrency,
            ignored_paths: self.ignored_paths.clone(),
            dormant_after_days: self.dormant_after_days.max(1),
            inspect_git: self.inspect_git,
            rules: self.rule_set(),
        }
    }

    /// Clamp values into sane ranges after loading user-edited files.
    pub fn sanitised(mut self) -> Self {
        self.quarantine_retention_days = self.quarantine_retention_days.clamp(1, 365);
        self.dormant_after_days = self.dormant_after_days.clamp(1, 3650);
        self.max_concurrency = self.max_concurrency.min(64);
        self.scan_roots.retain(|p| !p.as_os_str().is_empty());
        self.scan_roots.dedup();
        self
    }

    pub fn load(paths: &AppPaths) -> Self {
        load_json::<Settings>(&paths.settings_file).sanitised()
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        save_json(&paths.settings_file, self)
    }
}

/// Per-project state that survives rescans: protection, ignore windows,
/// hibernation records and in-app activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppState {
    pub protected_paths: BTreeSet<PathBuf>,
    pub ignored: BTreeMap<PathBuf, DateTime<Utc>>,
    pub hibernations: BTreeMap<PathBuf, HibernationRecord>,
    pub app_activity: BTreeMap<PathBuf, DateTime<Utc>>,
}

impl AppState {
    pub fn load(paths: &AppPaths) -> Self {
        load_json(&paths.state_file)
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        save_json(&paths.state_file, self)
    }

    pub fn set_protected(&mut self, path: &Path, protected: bool) {
        if protected {
            self.protected_paths.insert(path.to_path_buf());
        } else {
            self.protected_paths.remove(path);
        }
    }

    /// `None` clears the ignore.
    pub fn set_ignored_until(&mut self, path: &Path, until: Option<DateTime<Utc>>) {
        match until {
            Some(u) => {
                self.ignored.insert(path.to_path_buf(), u);
            }
            None => {
                self.ignored.remove(path);
            }
        }
    }

    pub fn touch_activity(&mut self, path: &Path, at: DateTime<Utc>) {
        self.app_activity.insert(path.to_path_buf(), at);
    }

    /// Drop ignore windows that have already expired.
    pub fn prune(&mut self, now: DateTime<Utc>) {
        self.ignored.retain(|_, until| *until > now);
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub settings_file: PathBuf,
    pub state_file: PathBuf,
    pub history_file: PathBuf,
    pub quarantine_dir: PathBuf,
    pub last_scan_file: PathBuf,
}

impl AppPaths {
    /// `$PROJECT_HIBERNATE_DATA_DIR`, else the platform data directory
    /// (`%APPDATA%\ProjectHibernate`, `~/Library/Application Support/ProjectHibernate`,
    /// `~/.local/share/ProjectHibernate`).
    pub fn default_locations() -> Self {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()) {
            return AppPaths::in_dir(Path::new(&dir));
        }
        let base = dirs::data_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        AppPaths::in_dir(&base.join(APP_DIR_NAME))
    }

    pub fn in_dir(dir: &Path) -> Self {
        AppPaths {
            data_dir: dir.to_path_buf(),
            settings_file: dir.join("settings.json"),
            state_file: dir.join("state.json"),
            history_file: dir.join("history.json"),
            quarantine_dir: dir.join("quarantine"),
            last_scan_file: dir.join("last-scan.json"),
        }
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.quarantine_dir)
    }
}

/// Read a JSON file, falling back to the default when it is missing or
/// unreadable. A corrupt file is renamed aside so nothing is silently lost.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(err) => {
                log::warn!(
                    "{}: could not parse ({err}); starting fresh",
                    path.display()
                );
                let backup = path.with_extension("json.corrupt");
                let _ = fs::rename(path, backup);
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

/// Atomic JSON write: temp file in the same directory, then rename.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn settings_round_trip_and_defaults() {
        let tmp = tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        let loaded = Settings::load(&paths);
        assert_eq!(loaded.disposition, Disposition::Trash);
        assert_eq!(loaded.dormant_after_days, 14);
        let mut s = loaded.clone();
        s.scan_roots.push(PathBuf::from("/tmp/projects"));
        s.disposition = Disposition::Quarantine;
        s.save(&paths).unwrap();
        let again = Settings::load(&paths);
        assert_eq!(again.scan_roots, vec![PathBuf::from("/tmp/projects")]);
        assert_eq!(again.disposition, Disposition::Quarantine);
    }

    #[test]
    fn corrupt_files_are_set_aside() {
        let tmp = tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        fs::write(&paths.settings_file, b"{not json").unwrap();
        let s = Settings::load(&paths);
        assert_eq!(s.dormant_after_days, 14);
        assert!(tmp.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn state_round_trip_with_path_keys() {
        let tmp = tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        let mut st = AppState::default();
        st.set_protected(Path::new("/p/a"), true);
        st.set_ignored_until(
            Path::new("/p/b"),
            Some(Utc::now() + chrono::Duration::days(7)),
        );
        st.save(&paths).unwrap();
        let again = AppState::load(&paths);
        assert!(again.protected_paths.contains(Path::new("/p/a")));
        assert!(again.ignored.contains_key(Path::new("/p/b")));
    }
}
