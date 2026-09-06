use crate::state::AppCtx;
use hibernate_core::cleanup::hibernate::{
    self as hib, ExecutionContext, HibernateEvent, HibernatePlan, HibernateRequest,
};
use hibernate_core::history::{ArtifactOutcome, HistoryEntry, HistoryStore};
use hibernate_core::model::{Project, Safety};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn plan_hibernate(ctx: State<'_, Arc<AppCtx>>, request: HibernateRequest) -> HibernatePlan {
    let disposition = AppCtx::lock(&ctx.settings).disposition;
    let snap = AppCtx::lock(&ctx.snapshot);
    hib::plan(&snap.projects, &request, disposition)
}

#[tauri::command]
pub fn start_hibernate(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    request: HibernateRequest,
) -> Result<(), String> {
    if ctx.scan.is_running() {
        return Err("Wait for the scan to finish before hibernating".into());
    }
    if !ctx.hibernate.begin() {
        return Err("A cleanup is already running".into());
    }
    let (disposition, rules) = {
        let s = AppCtx::lock(&ctx.settings);
        (s.disposition, s.rule_set())
    };
    let (projects, roots) = {
        let snap = AppCtx::lock(&ctx.snapshot);
        let roots = snap
            .summary
            .as_ref()
            .map(|s| s.roots.clone())
            .unwrap_or_default();
        (snap.projects.clone(), roots)
    };
    let plan = hib::plan(&projects, &request, disposition);
    if plan.is_empty() {
        ctx.hibernate.end();
        return Err("Nothing to hibernate in the selected projects".into());
    }

    let ctx = ctx.inner().clone();
    let cancel = ctx.hibernate.cancel.clone();
    std::thread::Builder::new()
        .name("hibernate-run".into())
        .spawn(move || {
            let quarantine = ctx.quarantine();
            let exec = ExecutionContext {
                scan_roots: &roots,
                rules: &rules,
                projects: &projects,
                quarantine: &quarantine,
            };
            let emit = |event: HibernateEvent| {
                let _ = app.emit("hibernate-event", &event);
            };
            let entry = hib::execute(&plan, &exec, &cancel, &emit);
            finish_run(&ctx, &app, &entry);
            ctx.hibernate.end();
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Persist the outcome and update the cached projects so the UI reflects the
/// new sizes without a rescan.
fn finish_run(ctx: &AppCtx, app: &AppHandle, entry: &HistoryEntry) {
    {
        let mut state = AppCtx::lock(&ctx.state);
        hib::record_hibernations(entry, &mut state);
    }
    {
        let mut history = AppCtx::lock(&ctx.history);
        history.push(entry.clone());
    }
    let _ = ctx.save_state();
    let _ = ctx.save_history();

    let mut changed: Vec<Project> = Vec::new();
    {
        let state = AppCtx::lock(&ctx.state);
        let mut snap = AppCtx::lock(&ctx.snapshot);
        for hp in &entry.projects {
            let Some(project) = snap.projects.iter_mut().find(|p| p.id == hp.project_id) else {
                continue;
            };
            for ha in &hp.artifacts {
                let removed = matches!(
                    ha.outcome,
                    ArtifactOutcome::Trashed
                        | ArtifactOutcome::Quarantined { .. }
                        | ArtifactOutcome::Deleted
                );
                if removed {
                    project.artifacts.retain(|a| a.path != ha.path);
                    project.total_bytes = project.total_bytes.saturating_sub(ha.bytes);
                    project.file_count = project.file_count.saturating_sub(1);
                }
            }
            project.reclaimable_bytes = project
                .artifacts
                .iter()
                .filter(|a| a.safety == Safety::Safe)
                .map(|a| a.bytes)
                .sum();
            project.review_bytes = project
                .artifacts
                .iter()
                .filter(|a| a.safety == Safety::Review)
                .map(|a| a.bytes)
                .sum();
            project.hibernation = state.hibernations.get(&project.path).cloned();
            let (safety, reasons) = hibernate_core::scanner::activity::safety(project);
            project.safety = safety;
            project.safety_reasons = reasons;
            changed.push(project.clone());
        }
        let total_bytes: u64 = snap.projects.iter().map(|p| p.total_bytes).sum();
        let reclaimable_bytes: u64 = snap.projects.iter().map(|p| p.reclaimable_bytes).sum();
        let review_bytes: u64 = snap.projects.iter().map(|p| p.review_bytes).sum();
        if let Some(summary) = snap.summary.as_mut() {
            summary.total_bytes = total_bytes;
            summary.reclaimable_bytes = reclaimable_bytes;
            summary.review_bytes = review_bytes;
        }
    }
    for p in &mut changed {
        ctx.refresh_status(p);
        let _ = ctx.with_project(&p.id.clone(), |cached| cached.status = p.status);
    }
    ctx.save_snapshot();
    let _ = app.emit("projects-updated", &changed);
}

#[tauri::command]
pub fn cancel_hibernate(ctx: State<'_, Arc<AppCtx>>) {
    ctx.hibernate.request_cancel();
}

#[tauri::command]
pub fn get_history(ctx: State<'_, Arc<AppCtx>>) -> HistoryStore {
    AppCtx::lock(&ctx.history).clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored: usize,
    pub errors: Vec<String>,
    pub entry: HistoryEntry,
}

#[tauri::command]
pub async fn restore_entry(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    entry_id: String,
) -> Result<RestoreResult, String> {
    let ctx = ctx.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let quarantine = ctx.quarantine();
        let (restored, errors, entry) = {
            let mut history = AppCtx::lock(&ctx.history);
            let entry = history
                .get_mut(&entry_id)
                .ok_or_else(|| "History entry not found".to_string())?;
            let (restored, errors) = hib::restore_entry(entry, &quarantine);
            (restored, errors, entry.clone())
        };
        let _ = ctx.save_history();
        let mut changed = Vec::new();
        {
            let mut state = AppCtx::lock(&ctx.state);
            let mut snap = AppCtx::lock(&ctx.snapshot);
            for p in &entry.projects {
                state.hibernations.remove(&p.path);
                if let Some(project) = snap.projects.iter_mut().find(|c| c.id == p.project_id) {
                    project.hibernation = None;
                    changed.push(project.clone());
                }
            }
        }
        let _ = ctx.save_state();
        for p in &mut changed {
            ctx.refresh_status(p);
            let _ = ctx.with_project(&p.id.clone(), |cached| cached.status = p.status);
        }
        ctx.save_snapshot();
        let _ = app.emit("projects-updated", &changed);
        Ok(RestoreResult {
            restored,
            errors,
            entry,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
