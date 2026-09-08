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

    /// True when at least one artifact can be brought back: it sits in
    /// quarantine, or in the OS Trash on platforms that let us restore it.
    pub fn restorable(&self) -> bool {
        let trash_ok = crate::cleanup::trash::trash_restore_supported();
        self.restored_at.is_none()
            && self.projects.iter().any(|p| {
                p.artifacts.iter().any(|a| match a.outcome {
                    ArtifactOutcome::Quarantined { .. } => true,
                    ArtifactOutcome::Trashed => trash_ok,
                    _ => false,
                })
            })
    }

    pub fn largest_savings(&self, n: usize) -> Vec<(&str, u64)> {
        let mut v: Vec<(&str, u64)> = self
            .projects
            .iter()
            .map(|p| (p.name.as_str(), p.bytes_recovered))
            .collect();
        v.sort_by_key(|entry| std::cmp::Reverse(entry.1));
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

/// One row per finished scan, for the "reclaimable over time" trend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRecord {
    pub at: DateTime<Utc>,
    pub project_count: usize,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub review_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanTrend {
    /// Oldest first.
    pub records: Vec<ScanRecord>,
}

impl ScanTrend {
    pub const MAX_RECORDS: usize = 365;

    pub fn load(paths: &AppPaths) -> Self {
        load_json(&paths.scan_trend_file)
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        save_json(&paths.scan_trend_file, self)
    }

    /// Append a record, replacing an earlier one from the same day so the
    /// trend has at most one point per day.
    pub fn push(&mut self, record: ScanRecord) {
        let day = record.at.date_naive();
        self.records.retain(|r| r.at.date_naive() != day);
        self.records.push(record);
        if self.records.len() > Self::MAX_RECORDS {
            let excess = self.records.len() - Self::MAX_RECORDS;
            self.records.drain(0..excess);
        }
    }
}

#[cfg(test)]
mod trend_tests {
    use super::*;

    #[test]
    fn one_point_per_day() {
        let mut t = ScanTrend::default();
        let now = Utc::now();
        let rec = |at: DateTime<Utc>, r: u64| ScanRecord {
            at,
            project_count: 1,
            total_bytes: 10,
            reclaimable_bytes: r,
            review_bytes: 0,
        };
        t.push(rec(now - chrono::Duration::days(1), 5));
        t.push(rec(now, 1));
        t.push(rec(now, 2));
        assert_eq!(t.records.len(), 2);
        assert_eq!(t.records.last().unwrap().reclaimable_bytes, 2);
    }
}
