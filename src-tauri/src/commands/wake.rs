use crate::state::AppCtx;
use chrono::Utc;
use hibernate_core::wake::{self, WakePlan};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Clone)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WakeEvent {
    StepStarted {
        project_id: String,
        step_index: usize,
        display: String,
    },
    Line {
        project_id: String,
        text: String,
    },
    StepFinished {
        project_id: String,
        step_index: usize,
        exit_code: i32,
        duration_ms: u128,
    },
    Finished {
        project_id: String,
        success: bool,
        duration_ms: u128,
        message: String,
    },
}

#[tauri::command]
pub fn get_wake_plan(
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
) -> Result<Option<WakePlan>, String> {
    ctx.with_project(&project_id, |p| wake::plan_for(p))
}

#[tauri::command]
pub fn start_wake(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
) -> Result<WakePlan, String> {
    let plan = ctx
        .with_project(&project_id, |p| wake::plan_for(p))?
        .ok_or_else(|| "No wake command is known for this project".to_string())?;
    if !ctx.wake.begin() {
        return Err("Another project is already waking up".into());
    }
    let ctx = ctx.inner().clone();
    let cancel = ctx.wake.cancel.clone();
    let returned = plan.clone();
    std::thread::Builder::new()
        .name("hibernate-wake".into())
        .spawn(move || {
            let started = Instant::now();
            let mut success = true;
            let mut message = String::new();
            for (i, step) in plan.steps.iter().enumerate() {
                let _ = app.emit(
                    "wake-event",
                    WakeEvent::StepStarted {
                        project_id: project_id.clone(),
                        step_index: i,
                        display: step.display.clone(),
                    },
                );
                let step_started = Instant::now();
                let pid = project_id.clone();
                let app2 = app.clone();
                let result = wake::run_step(step, &plan.cwd, &cancel, &mut |text| {
                    let _ = app2.emit(
                        "wake-event",
                        WakeEvent::Line {
                            project_id: pid.clone(),
                            text,
                        },
                    );
                });
                let exit_code = match result {
                    Ok(code) => code,
                    Err(err) => {
                        message = format!("Could not start `{}`: {err}", step.program);
                        -1
                    }
                };
                let _ = app.emit(
                    "wake-event",
                    WakeEvent::StepFinished {
                        project_id: project_id.clone(),
                        step_index: i,
                        exit_code,
                        duration_ms: step_started.elapsed().as_millis(),
                    },
                );
                if exit_code != 0 {
                    success = false;
                    if message.is_empty() {
                        message = if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            "Cancelled".into()
                        } else {
                            format!("`{}` exited with status {exit_code}", step.display)
                        };
                    }
                    break;
                }
            }
            if success {
                message = format!(
                    "Completed in {:.1} seconds.",
                    started.elapsed().as_secs_f64()
                );
                let path = ctx.with_project(&project_id, |p| p.path.clone()).ok();
                if let Some(path) = path {
                    {
                        let mut state = AppCtx::lock(&ctx.state);
                        state.hibernations.remove(&path);
                        state.touch_activity(&path, Utc::now());
                    }
                    ctx.save_state().ok();
                    let updated = ctx.with_project(&project_id, |p| {
                        p.hibernation = None;
                        p.activity.app_activity_at = Some(Utc::now());
                        p.last_activity_at = p.activity.latest();
                        p.clone()
                    });
                    if let Ok(mut p) = updated {
                        ctx.refresh_status(&mut p);
                        let _ = ctx.with_project(&project_id, |c| c.status = p.status);
                        ctx.save_snapshot();
                        let _ = app.emit("projects-updated", vec![p]);
                    }
                }
            }
            let _ = app.emit(
                "wake-event",
                WakeEvent::Finished {
                    project_id: project_id.clone(),
                    success,
                    duration_ms: started.elapsed().as_millis(),
                    message,
                },
            );
            ctx.wake.end();
        })
        .map_err(|e| e.to_string())?;
    Ok(returned)
}

#[tauri::command]
pub fn cancel_wake(ctx: State<'_, Arc<AppCtx>>) {
    ctx.wake.request_cancel();
}
