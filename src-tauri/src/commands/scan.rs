use crate::state::{AppCtx, ScanSnapshot};
use chrono::{Duration, Utc};
use hibernate_core::model::{ignore_forever, Project};
use hibernate_core::scanner::{scan, ScanEvent};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_last_scan(ctx: State<'_, Arc<AppCtx>>) -> ScanSnapshot {
    AppCtx::lock(&ctx.snapshot).clone()
}

#[tauri::command]
pub fn start_scan(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    roots: Vec<PathBuf>,
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

    let (opts, state) = {
        let mut settings = AppCtx::lock(&ctx.settings);
        settings.scan_roots = roots.clone();
        let opts = settings.scan_options();
        drop(settings);
        let state = AppCtx::lock(&ctx.state).clone();
        (opts, state)
    };
    let _ = ctx.save_settings();

    let ctx = ctx.inner().clone();
    let cancel = ctx.scan.cancel.clone();
    std::thread::Builder::new()
        .name("hibernate-scan".into())
        .spawn(move || {
            let emit = |event: ScanEvent| {
                let _ = app.emit("scan-event", &event);
            };
            let result = scan(&roots, &opts, &state, &cancel, &emit);
            {
                let mut snap = AppCtx::lock(&ctx.snapshot);
                if !result.cancelled || !result.projects.is_empty() {
                    snap.projects = result.projects;
                    snap.summary = Some(result.summary);
                    snap.scanned_at = Some(Utc::now());
                }
            }
            ctx.save_snapshot();
            ctx.scan.end();
        })
        .map_err(|e| e.to_string())?;
    Ok(())
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
