//! Lightweight cleanup history, one entry per hibernate run.

use crate::config::{load_json, save_json, AppPaths, Disposition};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailedPath {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ArtifactOutcome {
    Trashed,
    Quarantined {
        quarantine_path: PathBuf,
    },
    Deleted,
    /// Some files could not be removed (locked, permission denied).
    PartiallyDeleted {
        failed: Vec<FailedPath>,
    },
    Failed {
        error: String,
    },
    /// Refused by a safety check before anything was touched.
    Skipped {
        reason: String,
    },
    /// Moved back out of quarantine.
    Restored,
}

impl ArtifactOutcome {
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ArtifactOutcome::PartiallyDeleted { .. }
                | ArtifactOutcome::Failed { .. }
                | ArtifactOutcome::Skipped { .. }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryArtifact {
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: String,
    pub bytes: u64,
    pub outcome: ArtifactOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryProject {
    pub project_id: String,
    pub name: String,
    pub path: PathBuf,
    pub previous_bytes: u64,
    pub bytes_recovered: u64,
    pub artifacts: Vec<HistoryArtifact>,
    pub error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub disposition: Disposition,
    pub projects: Vec<HistoryProject>,
    pub total_recovered: u64,
    pub project_count: usize,
    pub error_count: usize,
    pub cancelled: bool,
    pub restored_at: Option<DateTime<Utc>>,
}

impl HistoryEntry {
    pub fn new_id(now: DateTime<Utc>) -> String {
        let nanos = now.timestamp_subsec_nanos() & 0xffff;
        format!("{}-{nanos:04x}", now.format("%Y%m%d-%H%M%S"))
    }

    /// True when at least one artifact is still sitting in quarantine.
    pub fn restorable(&self) -> bool {
        self.restored_at.is_none()
            && self.projects.iter().any(|p| {
                p.artifacts
                    .iter()
                    .any(|a| matches!(a.outcome, ArtifactOutcome::Quarantined { .. }))
            })
    }

    pub fn largest_savings(&self, n: usize) -> Vec<(&str, u64)> {
        let mut v: Vec<(&str, u64)> = self
            .projects
            .iter()
            .map(|p| (p.name.as_str(), p.bytes_recovered))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryStore {
    /// Newest first.
    pub entries: Vec<HistoryEntry>,
}

impl HistoryStore {
    pub const MAX_ENTRIES: usize = 200;

    pub fn load(paths: &AppPaths) -> Self {
        load_json(&paths.history_file)
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        save_json(&paths.history_file, self)
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        self.entries.truncate(Self::MAX_ENTRIES);
    }

    pub fn get(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut HistoryEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn total_recovered(&self) -> u64 {
        self.entries.iter().map(|e| e.total_recovered).sum()
    }
}
