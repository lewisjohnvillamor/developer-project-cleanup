//! Plan and execute a hibernate run.

use crate::cleanup::quarantine::Quarantine;
use crate::cleanup::remove::remove_tree;
use crate::cleanup::rules::RuleSet;
use crate::cleanup::safety::validate_artifact;
use crate::cleanup::trash::send_to_trash;
use crate::config::Disposition;
use crate::history::{ArtifactOutcome, HistoryArtifact, HistoryEntry, HistoryProject};
use crate::model::{CleanupArtifact, GitState, Project, Safety};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// One project the user chose, optionally narrowed to specific artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedProject {
    pub project_id: String,
    /// When `None`, every eligible artifact is included.
    #[serde(default)]
    pub artifact_paths: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HibernateRequest {
    pub selection: Vec<SelectedProject>,
    /// Include amber (review) artifacts. Off by default.
    #[serde(default)]
    pub include_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedProject {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub total_bytes: u64,
    pub artifacts: Vec<CleanupArtifact>,
    pub bytes: u64,
    /// Review artifacts left out because the user did not opt in.
    pub skipped_review: Vec<CleanupArtifact>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HibernatePlan {
    pub projects: Vec<PlannedProject>,
    pub total_bytes: u64,
    pub folder_count: u64,
    pub file_count: u64,
    pub review_count: usize,
    pub skipped_protected: Vec<String>,
    pub skipped_unknown: Vec<String>,
    pub disposition: Disposition,
}

impl HibernatePlan {
    pub fn is_empty(&self) -> bool {
        self.projects.iter().all(|p| p.artifacts.is_empty())
    }
}

/// Build the review shown before anything happens.
pub fn plan(
    projects: &[Project],
    request: &HibernateRequest,
    disposition: Disposition,
) -> HibernatePlan {
    let by_id: HashMap<&str, &Project> = projects.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut plan = HibernatePlan {
        projects: Vec::new(),
        total_bytes: 0,
        folder_count: 0,
        file_count: 0,
        review_count: 0,
        skipped_protected: Vec::new(),
        skipped_unknown: Vec::new(),
        disposition,
    };

    for sel in &request.selection {
        let Some(project) = by_id.get(sel.project_id.as_str()) else {
            plan.skipped_unknown.push(sel.project_id.clone());
            continue;
        };
        if project.protected {
            plan.skipped_protected.push(project.name.clone());
            continue;
        }
        let mut chosen = Vec::new();
        let mut skipped_review = Vec::new();
        for artifact in &project.artifacts {
            if let Some(paths) = &sel.artifact_paths {
                if !paths.iter().any(|p| p == &artifact.path) {
                    continue;
                }
            }
            match artifact.safety {
                Safety::Safe => chosen.push(artifact.clone()),
                Safety::Review if request.include_review || sel.artifact_paths.is_some() => {
                    plan.review_count += 1;
                    chosen.push(artifact.clone());
                }
                Safety::Review => skipped_review.push(artifact.clone()),
                Safety::Protected => {}
            }
        }
        let mut warnings = Vec::new();
        if let Some(git) = &project.git {
            match git.state {
                GitState::Modified | GitState::Untracked => {
                    warnings.push("This project contains uncommitted changes. Only generated folders are removed.".into())
                }
                _ => {}
            }
        }
        let bytes: u64 = chosen.iter().map(|a| a.bytes).sum();
        plan.total_bytes += bytes;
        plan.folder_count += chosen.iter().map(|a| a.dir_count).sum::<u64>();
        plan.file_count += chosen.iter().map(|a| a.file_count).sum::<u64>();
        plan.projects.push(PlannedProject {
            id: project.id.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            total_bytes: project.total_bytes,
            artifacts: chosen,
            bytes,
            skipped_review,
            warnings,
        });
    }
    plan.projects.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    plan
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HibernateEvent {
    Started {
        entry_id: String,
        project_count: usize,
        total_bytes: u64,
    },
    ProjectStarted {
        project_id: String,
        name: String,
        index: usize,
    },
    ArtifactStarted {
        project_id: String,
        relative_path: String,
        bytes: u64,
    },
    ArtifactFinished {
        project_id: String,
        relative_path: String,
        bytes_recovered: u64,
        outcome: ArtifactOutcome,
    },
    ProjectFinished {
        project_id: String,
        name: String,
        bytes_recovered: u64,
        error_count: usize,
        completed: usize,
        total: usize,
        total_recovered: u64,
    },
    Finished {
        entry: Box<HistoryEntry>,
    },
}

pub struct ExecutionContext<'a> {
    pub scan_roots: &'a [PathBuf],
    pub rules: &'a RuleSet,
    pub projects: &'a [Project],
    pub quarantine: &'a Quarantine,
}

/// Execute a plan. Every artifact is re-validated against the scan results
/// immediately before it is touched.
pub fn execute(
    plan: &HibernatePlan,
    ctx: &ExecutionContext,
    cancel: &AtomicBool,
    emit: &dyn Fn(HibernateEvent),
) -> HistoryEntry {
    let started_at = Utc::now();
    let entry_id = HistoryEntry::new_id(started_at);
    let by_id: HashMap<&str, &Project> = ctx.projects.iter().map(|p| (p.id.as_str(), p)).collect();
    let batch_dir = ctx.quarantine.batch_dir(&entry_id, started_at);

    emit(HibernateEvent::Started {
        entry_id: entry_id.clone(),
        project_count: plan.projects.len(),
        total_bytes: plan.total_bytes,
    });

    let mut entry = HistoryEntry {
        id: entry_id.clone(),
        started_at,
        finished_at: started_at,
        disposition: plan.disposition,
        projects: Vec::new(),
        total_recovered: 0,
        project_count: 0,
        error_count: 0,
        cancelled: false,
        restored_at: None,
    };

    let total = plan.projects.len();
    'projects: for (index, planned) in plan.projects.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            entry.cancelled = true;
            break;
        }
        emit(HibernateEvent::ProjectStarted {
            project_id: planned.id.clone(),
            name: planned.name.clone(),
            index,
        });
        let mut hp = HistoryProject {
            project_id: planned.id.clone(),
            name: planned.name.clone(),
            path: planned.path.clone(),
            previous_bytes: planned.total_bytes,
            bytes_recovered: 0,
            artifacts: Vec::new(),
            error_count: 0,
        };

        for artifact in &planned.artifacts {
            if cancel.load(Ordering::Relaxed) {
                entry.cancelled = true;
                if !hp.artifacts.is_empty() {
                    finish_project(&mut entry, hp, index, total, emit);
                }
                break 'projects;
            }
            emit(HibernateEvent::ArtifactStarted {
                project_id: planned.id.clone(),
                relative_path: artifact.relative_path.clone(),
                bytes: artifact.bytes,
            });

            let (recovered, outcome) = match by_id.get(planned.id.as_str()) {
                None => (
                    0,
                    ArtifactOutcome::Skipped {
                        reason: "project is not part of the last scan".into(),
                    },
                ),
                Some(project) => {
                    match validate_artifact(ctx.scan_roots, project, &artifact.path, ctx.rules) {
                        Err(err) => (
                            0,
                            ArtifactOutcome::Skipped {
                                reason: err.to_string(),
                            },
                        ),
                        Ok(canon) => dispose(
                            plan.disposition,
                            &canon,
                            artifact,
                            planned,
                            &batch_dir,
                            ctx.quarantine,
                            cancel,
                        ),
                    }
                }
            };

            if outcome.is_error() {
                hp.error_count += 1;
            }
            hp.bytes_recovered += recovered;
            emit(HibernateEvent::ArtifactFinished {
                project_id: planned.id.clone(),
                relative_path: artifact.relative_path.clone(),
                bytes_recovered: recovered,
                outcome: outcome.clone(),
            });
            hp.artifacts.push(HistoryArtifact {
                path: artifact.path.clone(),
                relative_path: artifact.relative_path.clone(),
                kind: artifact.kind.clone(),
                bytes: artifact.bytes,
                outcome,
            });
        }
        finish_project(&mut entry, hp, index, total, emit);
    }

    entry.finished_at = Utc::now();
    entry.project_count = entry.projects.len();
    emit(HibernateEvent::Finished {
        entry: Box::new(entry.clone()),
    });
    entry
}

fn finish_project(
    entry: &mut HistoryEntry,
    hp: HistoryProject,
    index: usize,
    total: usize,
    emit: &dyn Fn(HibernateEvent),
) {
    entry.total_recovered += hp.bytes_recovered;
    entry.error_count += hp.error_count;
    emit(HibernateEvent::ProjectFinished {
        project_id: hp.project_id.clone(),
        name: hp.name.clone(),
        bytes_recovered: hp.bytes_recovered,
        error_count: hp.error_count,
        completed: index + 1,
        total,
        total_recovered: entry.total_recovered,
    });
    entry.projects.push(hp);
}

fn dispose(
    disposition: Disposition,
    canon: &std::path::Path,
    artifact: &CleanupArtifact,
    planned: &PlannedProject,
    batch_dir: &std::path::Path,
    quarantine: &Quarantine,
    cancel: &AtomicBool,
) -> (u64, ArtifactOutcome) {
    match disposition {
        Disposition::Trash => match send_to_trash(canon) {
            Ok(()) => (artifact.bytes, ArtifactOutcome::Trashed),
            Err(error) => (0, ArtifactOutcome::Failed { error }),
        },
        Disposition::Quarantine => {
            match quarantine.quarantine(
                batch_dir,
                &planned.name,
                &artifact.relative_path,
                canon,
                cancel,
            ) {
                Ok(quarantine_path) => (
                    artifact.bytes,
                    ArtifactOutcome::Quarantined { quarantine_path },
                ),
                Err(err) => (
                    0,
                    ArtifactOutcome::Failed {
                        error: err.to_string(),
                    },
                ),
            }
        }
        Disposition::Permanent => {
            let report = remove_tree(canon, cancel);
            if report.cancelled {
                (
                    report.bytes_removed,
                    ArtifactOutcome::PartiallyDeleted {
                        failed: report.failed,
                    },
                )
            } else if report.failed.is_empty() && !canon.exists() {
                (report.bytes_removed, ArtifactOutcome::Deleted)
            } else {
                (
                    report.bytes_removed,
                    ArtifactOutcome::PartiallyDeleted {
                        failed: report.failed,
                    },
                )
            }
        }
    }
}

/// Restore every quarantined artifact of a history entry. Returns the
/// number restored and the errors encountered.
pub fn restore_entry(entry: &mut HistoryEntry, quarantine: &Quarantine) -> (usize, Vec<String>) {
    let mut restored = 0;
    let mut errors = Vec::new();
    for project in &mut entry.projects {
        for artifact in &mut project.artifacts {
            let ArtifactOutcome::Quarantined { quarantine_path } = &artifact.outcome else {
                continue;
            };
            match quarantine.restore(quarantine_path, &artifact.path) {
                Ok(()) => {
                    restored += 1;
                    artifact.outcome = ArtifactOutcome::Restored;
                }
                Err(err) => errors.push(format!("{}: {err}", artifact.path.display())),
            }
        }
    }
    if restored > 0 && !entry.restorable() {
        entry.restored_at = Some(Utc::now());
    }
    (restored, errors)
}

/// Record hibernation results in the persisted state so the next scan can
/// show these projects as hibernated.
pub fn record_hibernations(entry: &HistoryEntry, state: &mut crate::config::AppState) {
    for p in &entry.projects {
        if p.bytes_recovered == 0 {
            continue;
        }
        state.hibernations.insert(
            p.path.clone(),
            crate::model::HibernationRecord {
                hibernated_at: entry.finished_at,
                previous_bytes: p.previous_bytes,
                saved_bytes: p.bytes_recovered,
                history_entry_id: entry.id.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppState;
    use crate::scanner::{scan, ScanOptions};
    use std::fs;
    use tempfile::tempdir;

    fn write(path: &std::path::Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, Vec<Project>, Vec<PathBuf>) {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("Projects");
        write(&root.join("web/package.json"), 10);
        write(&root.join("web/src/index.ts"), 10);
        write(&root.join("web/node_modules/a/b.js"), 5000);
        write(&root.join("web/.next/x"), 1000);
        write(&root.join("api/pyproject.toml"), 10);
        write(&root.join("api/.venv/lib/x"), 3000);
        write(&root.join("api/__pycache__/m.pyc"), 100);
        let roots = vec![root];
        let result = scan(
            &roots,
            &ScanOptions {
                inspect_git: false,
                ..Default::default()
            },
            &AppState::default(),
            &AtomicBool::new(false),
            &|_| {},
        );
        (tmp, result.projects, roots)
    }

    #[test]
    fn plan_excludes_review_and_protected_by_default() {
        let (_tmp, projects, _roots) = fixture();
        let sel: Vec<SelectedProject> = projects
            .iter()
            .map(|p| SelectedProject {
                project_id: p.id.clone(),
                artifact_paths: None,
            })
            .collect();
        let req = HibernateRequest {
            selection: sel.clone(),
            include_review: false,
        };
        let p = plan(&projects, &req, Disposition::Permanent);
        assert_eq!(p.projects.len(), 2);
        assert_eq!(p.total_bytes, 5000 + 1000 + 100);
        assert_eq!(p.review_count, 0);
        let api = p.projects.iter().find(|p| p.name == "api").unwrap();
        assert_eq!(api.skipped_review.len(), 1);

        let req = HibernateRequest {
            selection: sel,
            include_review: true,
        };
        let p = plan(&projects, &req, Disposition::Permanent);
        assert_eq!(p.total_bytes, 5000 + 1000 + 100 + 3000);
        assert_eq!(p.review_count, 1);

        let mut protected = projects.clone();
        protected[0].protected = true;
        let req = HibernateRequest {
            selection: vec![SelectedProject {
                project_id: protected[0].id.clone(),
                artifact_paths: None,
            }],
            include_review: true,
        };
        let p = plan(&protected, &req, Disposition::Permanent);
        assert!(p.projects.is_empty());
        assert_eq!(p.skipped_protected, vec![protected[0].name.clone()]);
    }

    #[test]
    fn executes_permanent_and_quarantine_runs() {
        let (tmp, projects, roots) = fixture();
        let quarantine = Quarantine::new(&tmp.path().join("q"));
        let rules = RuleSet::builtin();
        let web = projects.iter().find(|p| p.name == "web").unwrap();
        let api = projects.iter().find(|p| p.name == "api").unwrap();

        // Permanent delete of web.
        let req = HibernateRequest {
            selection: vec![SelectedProject {
                project_id: web.id.clone(),
                artifact_paths: None,
            }],
            include_review: false,
        };
        let p = plan(&projects, &req, Disposition::Permanent);
        let ctx = ExecutionContext {
            scan_roots: &roots,
            rules: &rules,
            projects: &projects,
            quarantine: &quarantine,
        };
        let events = std::sync::Mutex::new(Vec::new());
        let entry = execute(&p, &ctx, &AtomicBool::new(false), &|e| {
            events.lock().unwrap().push(e)
        });
        let events = events.into_inner().unwrap();
        assert_eq!(entry.total_recovered, 6000);
        assert_eq!(entry.error_count, 0);
        assert!(!web.path.join("node_modules").exists());
        assert!(!web.path.join(".next").exists());
        assert!(web.path.join("src/index.ts").exists());
        assert!(web.path.join("package.json").exists());
        assert!(matches!(
            events.last(),
            Some(HibernateEvent::Finished { .. })
        ));

        // Quarantine api including review, then restore.
        let req = HibernateRequest {
            selection: vec![SelectedProject {
                project_id: api.id.clone(),
                artifact_paths: None,
            }],
            include_review: true,
        };
        let p = plan(&projects, &req, Disposition::Quarantine);
        let mut entry = execute(&p, &ctx, &AtomicBool::new(false), &|_| {});
        assert_eq!(entry.total_recovered, 3100);
        assert!(!api.path.join(".venv").exists());
        assert!(entry.restorable());
        let (restored, errors) = restore_entry(&mut entry, &quarantine);
        assert_eq!(restored, 2);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(api.path.join(".venv/lib/x").exists());
        assert!(!entry.restorable());
    }

    #[test]
    fn stale_scan_results_are_skipped_not_deleted() {
        let (_tmp, mut projects, roots) = fixture();
        let quarantine = Quarantine::new(&_tmp.path().join("q"));
        let rules = RuleSet::builtin();
        // Tamper: point an artifact at the source folder.
        let web = projects.iter_mut().find(|p| p.name == "web").unwrap();
        web.artifacts[0].path = web.path.join("src");
        let req = HibernateRequest {
            selection: vec![SelectedProject {
                project_id: web.id.clone(),
                artifact_paths: None,
            }],
            include_review: false,
        };
        let p = plan(&projects, &req, Disposition::Permanent);
        let ctx = ExecutionContext {
            scan_roots: &roots,
            rules: &rules,
            projects: &projects,
            quarantine: &quarantine,
        };
        let entry = execute(&p, &ctx, &AtomicBool::new(false), &|_| {});
        let web = projects.iter().find(|p| p.name == "web").unwrap();
        assert!(web.path.join("src/index.ts").exists());
        assert!(entry.error_count >= 1);
        assert!(entry.projects[0]
            .artifacts
            .iter()
            .any(|a| matches!(a.outcome, ArtifactOutcome::Skipped { .. })));
    }
}
