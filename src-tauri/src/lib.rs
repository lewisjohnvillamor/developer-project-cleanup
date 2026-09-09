//! Tauri shell: a thin command layer over `hibernate-core`. Long operations
//! run on their own threads and report progress through events:
//!
//! - `scan-event`        — [`hibernate_core::ScanEvent`]
//! - `hibernate-event`   — [`hibernate_core::HibernateEvent`]
//! - `wake-event`        — [`commands::wake::WakeEvent`]
//! - `projects-updated`  — `Vec<Project>` whose cached state changed
//! - `app-error`         — [`state::AppError`], a write to disk that failed

mod commands;
mod logging;
mod scheduler;
mod state;
#[cfg(test)]
mod testing;

use state::AppCtx;
use std::sync::Arc;
use tauri::Emitter;

pub fn run() {
    let ctx = Arc::new(AppCtx::load());
    logging::init(&ctx.paths.log_file, log::LevelFilter::Info);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(ctx.clone())
        .setup(move |app| {
            // Let background threads report failed writes to the window.
            let window = app.handle().clone();
            ctx.on_error(move |err| {
                // Nothing useful is left to do if the event itself cannot be
                // delivered; `report` has already written the log line.
                let _ = window.emit("app-error", err);
            });
            // Expire old quarantine batches in the background.
            let purge_ctx = ctx.clone();
            std::thread::spawn(move || purge_ctx.purge_quarantine());
            // Scheduled background scans.
            scheduler::spawn(app.handle().clone(), ctx.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::app::validate_folder,
            commands::app::open_project_folder,
            commands::app::copy_text,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::scan::get_last_scan,
            commands::scan::start_scan,
            commands::scan::cancel_scan,
            commands::scan::get_scan_trend,
            commands::scan::get_cache_info,
            commands::scan::clear_tree_cache,
            commands::tools::get_global_caches,
            commands::tools::preview_rule,
            commands::tools::export_projects,
            commands::tools::list_quarantine,
            commands::tools::purge_quarantine_batch,
            commands::tools::get_diagnostics,
            commands::scan::set_protected,
            commands::scan::ignore_project,
            commands::scan::unignore_project,
            commands::hibernate::plan_hibernate,
            commands::hibernate::start_hibernate,
            commands::hibernate::cancel_hibernate,
            commands::hibernate::get_history,
            commands::hibernate::restore_entry,
            commands::wake::get_wake_plan,
            commands::wake::start_wake,
            commands::wake::cancel_wake,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Project Hibernate");
}
