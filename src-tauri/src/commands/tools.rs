use crate::state::AppCtx;
use hibernate_core::caches::{measure_caches, GlobalCache};
use hibernate_core::cleanup::quarantine::QuarantineBatch;
use hibernate_core::cleanup::rules::{preview_matches, RuleMatch};
use hibernate_core::export::{render, ExportFormat};
use hibernate_core::format;
use hibernate_core::model::Stack;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// Sizes of global toolchain caches. Read-only; can take a while on big
/// machines, so it runs off the main thread.
#[tauri::command]
pub async fn get_global_caches(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<GlobalCache>, String> {
    if !ctx.caches.begin() {
        return Err("Already measuring caches".into());
    }
    let ctx = ctx.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let r = measure_caches(&ctx.caches.cancel);
        ctx.caches.end();
        r
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(result)
}

/// Which folders in the last scan a candidate rule would match.
#[tauri::command]
pub async fn preview_rule(
    ctx: State<'_, Arc<AppCtx>>,
    pattern: String,
    ecosystems: Vec<Stack>,
) -> Result<Vec<RuleMatch>, String> {
    let projects = AppCtx::lock(&ctx.snapshot).projects.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        preview_matches(&projects, &pattern, &ecosystems, 40, &cancel)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Write the current project table to `path` as CSV or JSON.
#[tauri::command]
pub fn export_projects(
    ctx: State<'_, Arc<AppCtx>>,
    path: PathBuf,
    format: ExportFormat,
) -> Result<usize, String> {
    if path.as_os_str().is_empty() {
        return Err("No file chosen".into());
    }
    let projects = AppCtx::lock(&ctx.snapshot).projects.clone();
    let text = render(&projects, format);
    std::fs::write(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(projects.len())
}

#[tauri::command]
pub async fn list_quarantine(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<QuarantineBatch>, String> {
    let q = ctx.quarantine();
    tauri::async_runtime::spawn_blocking(move || q.list_batches())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn purge_quarantine_batch(
    ctx: State<'_, Arc<AppCtx>>,
    entry_id: String,
) -> Result<u64, String> {
    let q = ctx.quarantine();
    let ctx2 = ctx.inner().clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        q.purge_batch(&entry_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    // A purged batch can no longer be restored.
    {
        let mut history = AppCtx::lock(&ctx2.history);
        for entry in &mut history.entries {
            for p in &mut entry.projects {
                for a in &mut p.artifacts {
                    if let hibernate_core::history::ArtifactOutcome::Quarantined {
                        quarantine_path,
                    } = &a.outcome
                    {
                        if !quarantine_path.exists() {
                            a.outcome = hibernate_core::history::ArtifactOutcome::Deleted;
                        }
                    }
                }
            }
        }
    }
    ctx2.save_history().ok();
    Ok(bytes)
}

/// Plain-text bug-report material: version, platform, settings and the last
/// scan's warnings. No file contents, no project names beyond counts.
#[tauri::command]
pub fn get_diagnostics(ctx: State<'_, Arc<AppCtx>>) -> String {
    let settings = AppCtx::lock(&ctx.settings).clone();
    let snap = AppCtx::lock(&ctx.snapshot).clone();
    let history = AppCtx::lock(&ctx.history).clone();
    let mut out = String::new();
    out.push_str(&format!(
        "Project Hibernate {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!(
        "OS: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    out.push_str(&format!(
        "git available: {}\n",
        hibernate_core::git::git_available()
    ));
    out.push_str(&format!("data dir: {}\n", ctx.paths.data_dir.display()));
    // Where the log file the user should attach actually lives.
    out.push_str(&format!("log file: {}\n", ctx.paths.log_file.display()));
    out.push_str(&format!(
        "tree cache entries: {}\n",
        AppCtx::lock(&ctx.tree_cache).len()
    ));
    out.push_str("\n[settings]\n");
    out.push_str(
        &serde_json::to_string_pretty(&serde_json::json!({
            "scanRoots": settings.scan_roots.len(),
            "maxConcurrency": settings.max_concurrency,
            "followSymlinks": settings.follow_symlinks,
            "scanHidden": settings.scan_hidden,
            "inspectGit": settings.inspect_git,
            "disposition": settings.disposition,
            "quarantineRetentionDays": settings.quarantine_retention_days,
            "includeReviewItems": settings.include_review_items,
            "dormantAfterDays": settings.dormant_after_days,
            "incrementalScans": settings.incremental_scans,
            "scheduledScanHours": settings.scheduled_scan_hours,
            "customRules": settings.custom_rules.len(),
            "protectedPatterns": settings.protected_patterns.len(),
            "ignoredPaths": settings.ignored_paths.len(),
        }))
        .unwrap_or_default(),
    );
    out.push_str("\n\n[last scan]\n");
    match &snap.summary {
        Some(s) => {
            out.push_str(&format!(
                "projects: {}  total: {}  reclaimable: {}  review: {}  duration: {} ms  cache hits: {}  warnings: {}\n",
                s.project_count,
                format::bytes(s.total_bytes),
                format::bytes(s.reclaimable_bytes),
                format::bytes(s.review_bytes),
                s.duration_ms,
                s.cache_hits,
                s.warning_count
            ));
            let slow: Vec<&hibernate_core::model::Project> = snap
                .projects
                .iter()
                .filter(|p| p.scan_duration_ms > 5000)
                .collect();
            if !slow.is_empty() {
                out.push_str(&format!("slow projects (>5s): {}\n", slow.len()));
            }
            for p in snap
                .projects
                .iter()
                .filter(|p| !p.warnings.is_empty())
                .take(20)
            {
                for w in &p.warnings {
                    out.push_str(&format!("  warning: {w}\n"));
                }
            }
        }
        None => out.push_str("none\n"),
    }
    out.push_str(&format!(
        "\n[history]\nentries: {}  errors: {}\n",
        history.entries.len(),
        history.entries.iter().map(|e| e.error_count).sum::<usize>()
    ));
    out
}
