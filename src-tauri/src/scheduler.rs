//! Background scans on a schedule. The decision logic is pure so it can be
//! tested; the loop just applies it every minute.

use crate::commands::scan::run_scan_blocking;
use crate::state::AppCtx;
use chrono::{DateTime, Duration, Utc};
use hibernate_core::format;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Why a scheduled scan did not start this tick.
#[derive(Debug, PartialEq, Eq)]
pub enum Skip {
    Disabled,
    NoRoots,
    Busy,
    NotDueYet,
}

/// Decide whether a scheduled scan should run now.
pub fn decide(
    scheduled_scan_hours: u32,
    has_roots: bool,
    busy: bool,
    last_scan: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<(), Skip> {
    if scheduled_scan_hours == 0 {
        return Err(Skip::Disabled);
    }
    if !has_roots {
        return Err(Skip::NoRoots);
    }
    if busy {
        return Err(Skip::Busy);
    }
    match last_scan {
        Some(last) if now - last < Duration::hours(scheduled_scan_hours as i64) => {
            Err(Skip::NotDueYet)
        }
        _ => Ok(()),
    }
}

/// The notification body after a scheduled scan, or `None` when the
/// reclaimable total is below the user's threshold.
pub fn notification_body(
    reclaimable_bytes: u64,
    project_count: usize,
    threshold_bytes: u64,
) -> Option<String> {
    if reclaimable_bytes < threshold_bytes.max(1) {
        return None;
    }
    Some(format!(
        "{} safely reclaimable across {} project{}. Open Project Hibernate to review.",
        format::bytes(reclaimable_bytes),
        project_count,
        if project_count == 1 { "" } else { "s" }
    ))
}

pub fn spawn(app: AppHandle, ctx: Arc<AppCtx>) {
    std::thread::Builder::new()
        .name("hibernate-scheduler".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let (hours, roots, threshold) = {
                let s = AppCtx::lock(&ctx.settings);
                (
                    s.scheduled_scan_hours,
                    s.scan_roots.clone(),
                    s.notify_threshold_bytes,
                )
            };
            let last = AppCtx::lock(&ctx.snapshot).scanned_at;
            let busy = ctx.scan.is_running() || ctx.hibernate.is_running() || ctx.wake.is_running();
            if decide(hours, !roots.is_empty(), busy, last, Utc::now()).is_err() {
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
            if let Some(body) = notification_body(
                result.summary.reclaimable_bytes,
                result.summary.project_count,
                threshold,
            ) {
                let _ = app
                    .notification()
                    .builder()
                    .title("Project Hibernate")
                    .body(body)
                    .show();
            }
        })
        .expect("spawn scheduler thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_decisions() {
        let now = Utc::now();
        assert_eq!(decide(0, true, false, None, now), Err(Skip::Disabled));
        assert_eq!(decide(24, false, false, None, now), Err(Skip::NoRoots));
        assert_eq!(decide(24, true, true, None, now), Err(Skip::Busy));
        assert_eq!(decide(24, true, false, None, now), Ok(()));
        assert_eq!(
            decide(24, true, false, Some(now - Duration::hours(2)), now),
            Err(Skip::NotDueYet)
        );
        assert_eq!(
            decide(24, true, false, Some(now - Duration::hours(25)), now),
            Ok(())
        );
    }

    #[test]
    fn notifies_only_above_threshold() {
        assert!(notification_body(500_000_000, 3, 1_000_000_000).is_none());
        let body = notification_body(12_300_000_000, 18, 1_000_000_000).unwrap();
        assert!(body.starts_with("12.3 GB"));
        assert!(body.contains("18 projects"));
        assert!(notification_body(1, 1, 0).unwrap().contains("1 project."));
    }
}
