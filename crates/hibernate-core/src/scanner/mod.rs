//! The scan engine. Two phases:
//!
//! 1. **Discovery** walks the scan roots looking for project markers. It is
//!    cheap (directory listings only) and skips artifact directories, so the
//!    first project names appear almost immediately.
//! 2. **Measurement** sizes each discovered project on a bounded thread pool,
//!    classifying artifact directories and computing activity and Git state.
//!    Each finished project is emitted as soon as it is ready.

pub mod activity;
pub mod cache;
pub mod size;
pub mod traversal;

use crate::cleanup::rules::RuleSet;
use crate::config::AppState;
use crate::git;
use crate::model::*;
use crate::wake;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use cache::{SharedTreeCache, TreeCache};
pub use traversal::DiscoveredProject;

/// Measuring a single project longer than this produces a warning.
pub const SLOW_PROJECT_SECS: u64 = 20;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub follow_symlinks: bool,
    pub scan_hidden: bool,
    /// Measurement threads. 0 = automatic.
    pub max_concurrency: usize,
    pub ignored_paths: Vec<PathBuf>,
    pub dormant_after_days: u32,
    pub inspect_git: bool,
    pub rules: RuleSet,
    /// Fingerprint cache from the previous scan. `None` = full rescan.
    pub tree_cache: Option<Arc<Mutex<TreeCache>>>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            follow_symlinks: false,
            scan_hidden: true,
            max_concurrency: 0,
            ignored_paths: Vec::new(),
            dormant_after_days: 14,
            inspect_git: true,
            rules: RuleSet::builtin(),
            tree_cache: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category: ArtifactCategory,
    pub bytes: u64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub roots: Vec<PathBuf>,
    pub project_count: usize,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub review_bytes: u64,
    pub by_category: Vec<CategoryTotal>,
    pub duration_ms: u128,
    pub warning_count: usize,
    pub finished_at: DateTime<Utc>,
    /// Artifact trees whose size was reused from the previous scan.
    #[serde(default)]
    pub cache_hits: usize,
}

impl ScanSummary {
    pub fn from_projects(
        roots: &[PathBuf],
        projects: &[Project],
        duration_ms: u128,
        warning_count: usize,
    ) -> Self {
        let mut totals: Vec<CategoryTotal> = ArtifactCategory::ALL
            .iter()
            .map(|c| CategoryTotal {
                category: *c,
                bytes: 0,
                count: 0,
            })
            .collect();
        for p in projects {
            for a in &p.artifacts {
                if a.safety != Safety::Safe {
                    continue;
                }
                let idx = ArtifactCategory::ALL
                    .iter()
                    .position(|c| *c == a.category)
                    .unwrap_or(ArtifactCategory::ALL.len() - 1);
                totals[idx].bytes += a.bytes;
                totals[idx].count += 1;
            }
        }
        totals.retain(|t| t.count > 0);
        totals.sort_by_key(|t| std::cmp::Reverse(t.bytes));
        ScanSummary {
            roots: roots.to_vec(),
            project_count: projects.len(),
            total_bytes: projects.iter().map(|p| p.total_bytes).sum(),
            reclaimable_bytes: projects.iter().map(|p| p.reclaimable_bytes).sum(),
            review_bytes: projects.iter().map(|p| p.review_bytes).sum(),
            by_category: totals,
            duration_ms,
            warning_count,
            finished_at: Utc::now(),
            cache_hits: projects.iter().map(|p| p.cache_hits as usize).sum(),
        }
    }
}

/// Progress events. Tagged so the frontend can switch on `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScanEvent {
    Started {
        roots: Vec<PathBuf>,
    },
    Discovered {
        path: PathBuf,
        name: String,
        stacks: Vec<Stack>,
        discovered: usize,
    },
    Scanned {
        project: Box<Project>,
        scanned: usize,
        discovered: usize,
    },
    Warning {
        path: Option<PathBuf>,
        message: String,
    },
    Finished {
        summary: ScanSummary,
    },
    Cancelled {
        scanned: usize,
        discovered: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub projects: Vec<Project>,
    pub summary: ScanSummary,
    pub cancelled: bool,
}

/// Run a full scan. `emit` is called from worker threads, so it must be
/// `Sync`; the desktop shell forwards events to the UI, the CLI prints them.
pub fn scan(
    roots: &[PathBuf],
    opts: &ScanOptions,
    state: &AppState,
    cancel: &AtomicBool,
    emit: &(dyn Fn(ScanEvent) + Sync),
) -> ScanResult {
    let started = Instant::now();
    let now = Utc::now();
    let warnings = AtomicUsize::new(0);
    let warn = |path: Option<PathBuf>, message: String| {
        warnings.fetch_add(1, Ordering::Relaxed);
        emit(ScanEvent::Warning { path, message });
    };

    // ---- normalise roots --------------------------------------------------
    let mut clean_roots: Vec<PathBuf> = Vec::new();
    for root in roots {
        if root.as_os_str().is_empty() {
            continue;
        }
        match std::fs::canonicalize(root) {
            Ok(c) if c.is_dir() => {
                if !clean_roots.contains(&c) {
                    clean_roots.push(c);
                }
            }
            Ok(_) => warn(Some(root.clone()), "Not a directory".into()),
            Err(e) => warn(Some(root.clone()), format!("Cannot open folder: {e}")),
        }
    }
    // Drop roots nested inside other roots to avoid double counting.
    let nested: Vec<PathBuf> = clean_roots
        .iter()
        .filter(|r| clean_roots.iter().any(|o| o != *r && r.starts_with(o)))
        .cloned()
        .collect();
    for r in &nested {
        warn(
            Some(r.clone()),
            "Folder is inside another scan folder and was skipped".into(),
        );
    }
    clean_roots.retain(|r| !nested.contains(r));

    emit(ScanEvent::Started {
        roots: clean_roots.clone(),
    });

    for root in &clean_roots {
        if root.parent().is_none() {
            warn(
                Some(root.clone()),
                "This is an entire drive. Scanning it can take a long time.".into(),
            );
        }
        if root.to_string_lossy().starts_with("\\\\") {
            warn(
                Some(root.clone()),
                "This looks like a network location. Deep scans of network drives can be very slow.".into(),
            );
        }
    }

    // ---- phase 1: discovery ------------------------------------------------
    let mut discovered: Vec<DiscoveredProject> = Vec::new();
    let disc_opts = traversal::DiscoveryOptions {
        follow_symlinks: opts.follow_symlinks,
        scan_hidden: opts.scan_hidden,
        ignored_paths: &opts.ignored_paths,
        rules: &opts.rules,
    };
    for root in &clean_roots {
        let mut count = discovered.len();
        let found = traversal::discover(
            root,
            &disc_opts,
            cancel,
            &mut |p| {
                count += 1;
                emit(ScanEvent::Discovered {
                    path: p.path.clone(),
                    name: file_name(&p.path),
                    stacks: p.detection.stacks.clone(),
                    discovered: count,
                });
            },
            &mut |path, message| warn(Some(path.to_path_buf()), message),
        );
        discovered.extend(found);
        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        emit(ScanEvent::Cancelled {
            scanned: 0,
            discovered: discovered.len(),
        });
        return ScanResult {
            projects: Vec::new(),
            summary: ScanSummary::from_projects(
                &clean_roots,
                &[],
                started.elapsed().as_millis(),
                warnings.load(Ordering::Relaxed),
            ),
            cancelled: true,
        };
    }

    // ---- phase 2: measurement -----------------------------------------------
    let project_paths: HashSet<PathBuf> = discovered.iter().map(|d| d.path.clone()).collect();
    let git_ok = opts.inspect_git && git::git_available();
    let threads = if opts.max_concurrency == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 8)
    } else {
        opts.max_concurrency.clamp(1, 64)
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("hibernate-scan-{i}"))
        .build();

    let scanned = AtomicUsize::new(0);
    let total = discovered.len();
    let measure_one = |d: &DiscoveredProject| -> Option<Project> {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let project = build_project(d, opts, state, &project_paths, git_ok, now, cancel);
        let n = scanned.fetch_add(1, Ordering::Relaxed) + 1;
        emit(ScanEvent::Scanned {
            project: Box::new(project.clone()),
            scanned: n,
            discovered: total,
        });
        Some(project)
    };

    let results: Vec<Option<Project>> = match pool {
        Ok(pool) => pool.install(|| discovered.par_iter().map(measure_one).collect()),
        Err(_) => discovered.iter().map(measure_one).collect(),
    };

    let mut projects: Vec<Project> = results.into_iter().flatten().collect();
    if !cancel.load(Ordering::Relaxed) {
        if let Some(cache) = &opts.tree_cache {
            if let Ok(mut c) = cache.lock() {
                c.prune_untouched();
            }
        }
    }
    projects.sort_by(|a, b| {
        b.reclaimable_bytes
            .cmp(&a.reclaimable_bytes)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let cancelled = cancel.load(Ordering::Relaxed);
    let summary = ScanSummary::from_projects(
        &clean_roots,
        &projects,
        started.elapsed().as_millis(),
        warnings.load(Ordering::Relaxed),
    );
    if cancelled {
        emit(ScanEvent::Cancelled {
            scanned: projects.len(),
            discovered: total,
        });
    } else {
        emit(ScanEvent::Finished {
            summary: summary.clone(),
        });
    }
    ScanResult {
        projects,
        summary,
        cancelled,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[allow(clippy::too_many_arguments)]
fn build_project(
    d: &DiscoveredProject,
    opts: &ScanOptions,
    state: &AppState,
    project_paths: &HashSet<PathBuf>,
    git_ok: bool,
    now: DateTime<Utc>,
    cancel: &AtomicBool,
) -> Project {
    let det = &d.detection;
    let pm = det.package_manager.clone();
    let restore_hint = |rule: &crate::cleanup::rules::CleanupRule| -> Option<String> {
        if rule.id == "node_modules" {
            return Some(wake::node_install_step(pm.as_deref().unwrap_or("npm")).display);
        }
        rule.restore_hint.clone()
    };
    let started = Instant::now();
    let m = size::measure(
        &d.path,
        &det.stacks,
        &restore_hint,
        &size::MeasureOptions {
            follow_symlinks: opts.follow_symlinks,
            rules: &opts.rules,
            project_paths,
            ignored_paths: &opts.ignored_paths,
            cache: opts.tree_cache.as_deref(),
        },
        cancel,
    );
    let scan_duration_ms = started.elapsed().as_millis() as u64;
    let mut m = m;

    let git_info = git::inspect(&d.path, git_ok);

    // A rule matches on a directory's name, which says nothing about whether
    // the project actually treats it as generated. Ask Git: anything it
    // tracks holds committed or staged work and is never offered for removal,
    // however well the name matches.
    if git_ok && git_info.is_repo {
        let dirs: Vec<String> = m
            .artifacts
            .iter()
            .map(|a| a.relative_path.clone())
            .collect();
        if let Some(facts) = git::artifact_facts(&d.path, &dirs) {
            for artifact in &mut m.artifacts {
                let key = artifact.relative_path.trim_end_matches('/');
                artifact.ignored_by_git = facts.ignored.contains(key);
                if facts.tracked.contains(key) {
                    artifact.tracked_by_git = true;
                    artifact.safety = Safety::Protected;
                    artifact.regeneratable = false;
                    artifact.explanation = format!(
                        "Git tracks files inside this folder, so it holds committed or staged work. {} is not applied here.",
                        artifact.kind
                    );
                    artifact.restore_hint = None;
                }
            }
        }
    }
    let m = m;

    let mut warnings = m.warnings.clone();
    if scan_duration_ms > SLOW_PROJECT_SECS * 1000 {
        warnings.push(format!(
            "Measuring took {:.0}s. Consider excluding this folder or its largest sub-folders if it is not a project you care about.",
            scan_duration_ms as f64 / 1000.0
        ));
    }

    let activity = ActivitySources {
        source_modified_at: m
            .source_modified_at
            .and_then(activity::to_utc)
            .map(|t| t.min(now)),
        git_commit_at: git_info.last_commit_at.map(|t| t.min(now)),
        app_activity_at: state.app_activity.get(&d.path).copied(),
    };
    let last_activity_at = activity.latest();

    let reclaimable_bytes: u64 = m
        .artifacts
        .iter()
        .filter(|a| a.safety == Safety::Safe)
        .map(|a| a.bytes)
        .sum();
    let review_bytes: u64 = m
        .artifacts
        .iter()
        .filter(|a| a.safety == Safety::Review)
        .map(|a| a.bytes)
        .sum();

    let protected = state.protected_paths.contains(&d.path);
    let ignored_until = state
        .ignored
        .get(&d.path)
        .copied()
        .filter(|until| *until > now);
    let hibernation = state.hibernations.get(&d.path).cloned();
    let status = activity::status(
        last_activity_at,
        protected,
        hibernation.as_ref(),
        reclaimable_bytes,
        opts.dormant_after_days,
        now,
    );

    let mut project = Project {
        id: project_id(&d.path),
        name: file_name(&d.path),
        path: d.path.clone(),
        scan_root: d.scan_root.clone(),
        parent_path: d.parent.clone(),
        stacks: det.stacks.clone(),
        frameworks: det.frameworks.clone(),
        workspace_members: d.members.clone(),
        scan_duration_ms,
        cache_hits: m.cache_hits,
        package_manager: pm,
        total_bytes: m.total_bytes,
        reclaimable_bytes,
        review_bytes,
        file_count: m.file_count,
        last_activity_at,
        activity,
        git: Some(git_info),
        status,
        safety: Safety::Safe,
        safety_reasons: Vec::new(),
        artifacts: m.artifacts,
        protected_entries: m.protected_entries,
        protected,
        ignored_until,
        hibernation,
        scanned_at: now,
        warnings,
    };
    let (safety, reasons) = activity::safety(&project);
    project.safety = safety;
    project.safety_reasons = reasons;
    project
}
