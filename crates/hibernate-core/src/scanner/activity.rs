//! "Last active" and status derivation.
//!
//! Generated directories never influence activity: a stale `.next/cache`
//! must not make an abandoned project look alive.

use crate::model::{GitState, HibernationRecord, Project, ProjectStatus, Safety};
use chrono::{DateTime, Duration, Utc};
use std::time::SystemTime;

/// Below this many reclaimable bytes a project that was hibernated by the
/// app still counts as hibernated.
pub const HIBERNATED_THRESHOLD_BYTES: u64 = 1_000_000;

pub fn to_utc(t: SystemTime) -> Option<DateTime<Utc>> {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| DateTime::<Utc>::from(SystemTime::UNIX_EPOCH + d))
}

pub fn status(
    last_activity: Option<DateTime<Utc>>,
    protected: bool,
    hibernation: Option<&HibernationRecord>,
    reclaimable_bytes: u64,
    dormant_after_days: u32,
    now: DateTime<Utc>,
) -> ProjectStatus {
    if protected {
        return ProjectStatus::Protected;
    }
    if hibernation.is_some() && reclaimable_bytes < HIBERNATED_THRESHOLD_BYTES {
        return ProjectStatus::Hibernated;
    }
    match last_activity {
        Some(at) if now - at < Duration::days(dormant_after_days as i64) => ProjectStatus::Active,
        _ => ProjectStatus::Dormant,
    }
}

/// Project-level safety and the reasons behind it.
pub fn safety(project: &Project) -> (Safety, Vec<String>) {
    let mut reasons = Vec::new();
    if let Some(git) = &project.git {
        match git.state {
            GitState::Modified => reasons.push(format!(
                "{} uncommitted change{}",
                git.modified_count,
                if git.modified_count == 1 { "" } else { "s" }
            )),
            GitState::Untracked => reasons.push(format!(
                "{} untracked file{}",
                git.untracked_count,
                if git.untracked_count == 1 { "" } else { "s" }
            )),
            GitState::RemoteMissing => reasons.push("No Git remote configured".into()),
            GitState::NoRepo => reasons.push("Not a Git repository".into()),
            _ => {}
        }
    }
    let review = project
        .artifacts
        .iter()
        .filter(|a| a.safety == Safety::Review)
        .count();
    if review > 0 {
        reasons.push(format!(
            "{review} folder{} need{} review before removal",
            if review == 1 { "" } else { "s" },
            if review == 1 { "s" } else { "" }
        ));
    }
    let blocking = reasons
        .iter()
        .any(|r| !r.starts_with("No Git remote") && !r.starts_with("Not a Git"));
    (
        if blocking {
            Safety::Review
        } else {
            Safety::Safe
        },
        reasons,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> HibernationRecord {
        HibernationRecord {
            hibernated_at: Utc::now(),
            previous_bytes: 10,
            saved_bytes: 9,
            history_entry_id: "x".into(),
        }
    }

    #[test]
    fn derives_status() {
        let now = Utc::now();
        assert_eq!(
            status(Some(now - Duration::days(4)), false, None, 0, 14, now),
            ProjectStatus::Active
        );
        assert_eq!(
            status(Some(now - Duration::days(14)), false, None, 0, 14, now),
            ProjectStatus::Dormant
        );
        assert_eq!(
            status(None, false, None, 0, 14, now),
            ProjectStatus::Dormant
        );
        assert_eq!(
            status(Some(now), true, None, 0, 14, now),
            ProjectStatus::Protected
        );
        assert_eq!(
            status(Some(now), false, Some(&record()), 0, 14, now),
            ProjectStatus::Hibernated
        );
        // Artifacts came back: not hibernated any more.
        assert_eq!(
            status(Some(now), false, Some(&record()), 50_000_000, 14, now),
            ProjectStatus::Active
        );
    }
}
