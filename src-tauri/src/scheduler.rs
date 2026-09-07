//! Optional background scans. Every minute the scheduler checks whether a
//! scan is due (per `Settings::scheduled_scan_hours`), runs one if the app
//! is idle, and posts a system notification when enough space is
//! reclaimable.

use crate::commands::scan::run_scan_blocking;
use crate::state::AppCtx;
use chrono::{Duration, Utc};
use hibernate_core::format;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn start(app: AppHandle, ctx: Arc<AppCtx>) {
    std::thread::Builder::new()
        .name("hibernate-scheduler".into())
        .spawn(move || loop {
            std::thread::sleep(StdDuration::from_secs(60));
            let (hours, roots, threshold) = {
                let s = AppCtx::lock(&ctx.settings);
                (
                    s.scheduled_scan_hours,
                    s.scan_roots.clone(),
                    s.notify_threshold_bytes,
                )
            };
            if hours == 0 || roots.is_empty() {
                continue;
            }
            let last = AppCtx::lock(&ctx.snapshot).scanned_at;
            let due = last
                .map(|t| Utc::now() - t >= Duration::hours(hours as i64))
                .unwrap_or(true);
            if !due || ctx.hibernate.is_running() || ctx.wake.is_running() {
                continue;
            }
            if !ctx.scan.begin() {
                continue;
            }
            let result = run_scan_blocking(&app, &ctx, roots, false);
            ctx.scan.end();
            if result.cancelled {
                continue;
            }
            let reclaimable = result.summary.reclaimable_bytes;
            if reclaimable >= threshold {
                let _ = app
                    .notification()
                    .builder()
                    .title("Project Hibernate")
                    .body(format!(
                        "{} reclaimable across {} projects. Open the app to review.",
                        format::bytes(reclaimable),
                        result.summary.project_count
                    ))
                    .show();
            }
        })
        .expect("spawn scheduler");
}
