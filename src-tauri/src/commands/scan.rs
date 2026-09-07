use crate::state::{AppCtx, ScanSnapshot};
use chrono::{Duration, Utc};
use hibernate_core::history::ScanRecord;
use hibernate_core::model::{ignore_forever, Project};
use hibernate_core::scanner::{scan, ScanEvent, ScanResult};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_last_scan(ctx: State<'_, Arc<AppCtx>>) -> ScanSnapshot {
    AppCtx::lock(&ctx.snapshot).clone()
}

/// Run a scan on the calling thread, emitting `scan-event`s, and persist the
/// outcome. The caller must have claimed `ctx.scan` first.
pub fn run_scan_blocking(
    app: &AppHandle,
    ctx: &Arc<AppCtx>,
    roots: Vec<PathBuf>,
    full: bool,
) -> ScanResult {
    let (mut opts, state, incremental) = {
        let mut settings = AppCtx::lock(&ctx.settings);
        settings.scan_roots = roots.clone();
        let opts = settings.scan_options();
        let incremental = settings.incremental_scans;
        drop(settings);
        let state = AppCtx::lock(&ctx.state).clone();
        (opts, state, incremental)
    };
    let _ = ctx.save_settings();
    if full {
        AppCtx::lock(&ctx.tree_cache).clear();
    }
    if incremental {
        opts.tree_cache = Some(ctx.tree_cache.clone());
    }

    let cancel = ctx.scan.cancel.clone();
    let emit = |event: ScanEvent| {
        let _ = app.emit("scan-event", &event);
    };
    let result = scan(&roots, &opts, &state, &cancel, &emit);
    {
        let mut snap = AppCtx::lock(&ctx.snapshot);
        if !result.cancelled || !result.projects.is_empty() {
            snap.projects = result.projects.clone();
            snap.summary = Some(result.summary.clone());
            snap.scanned_at = Some(Utc::now());
        }
    }
    if !result.cancelled {
        let s = &result.summary;
        AppCtx::lock(&ctx.trend).push(ScanRecord {
            at: s.finished_at,
            project_count: s.project_count,
            total_bytes: s.total_bytes,
            reclaimable_bytes: s.reclaimable_bytes,
            review_bytes: s.review_bytes,
        });
        ctx.save_trend();
    }
    ctx.save_snapshot();
    ctx.save_tree_cache();
    result
}

/// `full = true` discards cached tree sizes and measures everything again.
#[tauri::command]
pub fn start_scan(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    roots: Vec<PathBuf>,
    full: Option<bool>,
) -> Result<(), String> {
    let roots: Vec<PathBuf> = roots
        .into_iter()
        .filter(|r| !r.as_os_str().is_empty())
        .collect();
    if roots.is_empty() {
        return Err("Choose at least one folder to scan".into());
    }
    if ctx.hibernate.is_running() {
        return Err("Wait for the current cleanup to finish before scanning".into());
    }
    if !ctx.scan.begin() {
        return Err("A scan is already running".into());
    }
    let ctx = ctx.inner().clone();
    let full = full.unwrap_or(false);
    std::thread::Builder::new()
        .name("hibernate-scan".into())
        .spawn(move || {
            run_scan_blocking(&app, &ctx, roots, full);
            ctx.scan.end();
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_scan_trend(ctx: State<'_, Arc<AppCtx>>) -> hibernate_core::history::ScanTrend {
    AppCtx::lock(&ctx.trend).clone()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub entries: usize,
}

#[tauri::command]
pub fn get_cache_info(ctx: State<'_, Arc<AppCtx>>) -> CacheInfo {
    CacheInfo {
        entries: AppCtx::lock(&ctx.tree_cache).len(),
    }
}

#[tauri::command]
pub fn clear_tree_cache(ctx: State<'_, Arc<AppCtx>>) {
    AppCtx::lock(&ctx.tree_cache).clear();
    ctx.save_tree_cache();
}

#[tauri::command]
pub fn cancel_scan(ctx: State<'_, Arc<AppCtx>>) {
    ctx.scan.request_cancel();
}

#[tauri::command]
pub fn set_protected(
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
    protected: bool,
) -> Result<Project, String> {
    let project = ctx.with_project(&project_id, |p| {
        p.protected = protected;
        p.clone()
    })?;
    {
        let mut state = AppCtx::lock(&ctx.state);
        state.set_protected(&project.path, protected);
    }
    ctx.save_state()?;
    let updated = ctx.with_project(&project_id, |p| {
        ctx.refresh_status(p);
        p.clone()
    })?;
    ctx.save_snapshot();
    Ok(updated)
}

/// `days = None` hides the project until it is manually restored.
#[tauri::command]
pub fn ignore_project(
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
    days: Option<u32>,
) -> Result<Project, String> {
    let until = match days {
        Some(d) => Utc::now() + Duration::days(d.clamp(1, 3650) as i64),
        None => ignore_forever(),
    };
    let project = ctx.with_project(&project_id, |p| {
        p.ignored_until = Some(until);
        p.clone()
    })?;
    {
        let mut state = AppCtx::lock(&ctx.state);
        state.set_ignored_until(&project.path, Some(until));
    }
    ctx.save_state()?;
    ctx.save_snapshot();
    Ok(project)
}

#[tauri::command]
pub fn unignore_project(
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
) -> Result<Project, String> {
    let project = ctx.with_project(&project_id, |p| {
        p.ignored_until = None;
        p.clone()
    })?;
    {
        let mut state = AppCtx::lock(&ctx.state);
        state.set_ignored_until(&project.path, None);
    }
    ctx.save_state()?;
    ctx.save_snapshot();
    Ok(project)
}
