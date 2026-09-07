use crate::state::AppCtx;
use chrono::Utc;
use hibernate_core::cleanup::trash::{trash_available, trash_restore_supported};
use hibernate_core::git::git_available;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub data_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub quarantine_bytes: u64,
    pub git_available: bool,
    pub trash_available: bool,
    /// Whether items sent to the OS Trash can be restored from History.
    pub trash_restore_supported: bool,
    pub home_dir: Option<PathBuf>,
}

#[tauri::command]
pub fn get_app_info(ctx: State<'_, Arc<AppCtx>>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        data_dir: ctx.paths.data_dir.clone(),
        quarantine_dir: ctx.paths.quarantine_dir.clone(),
        quarantine_bytes: ctx.quarantine().size(),
        git_available: git_available(),
        trash_available: trash_available(),
        trash_restore_supported: trash_restore_supported(),
        home_dir: dirs::home_dir(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub path: PathBuf,
    pub exists: bool,
    pub warning: Option<String>,
}

/// Normalise a folder the user typed or picked and flag risky choices.
#[tauri::command]
pub fn validate_folder(path: String) -> Result<FolderInfo, String> {
    let raw = PathBuf::from(path.trim());
    if raw.as_os_str().is_empty() {
        return Err("Enter a folder path".into());
    }
    let canonical = std::fs::canonicalize(&raw).map_err(|e| format!("{}: {e}", raw.display()))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a folder", canonical.display()));
    }
    let mut warning = None;
    if canonical.parent().is_none() {
        warning = Some("This is an entire drive. Scanning it can take a long time.".into());
    } else if dirs::home_dir().as_deref() == Some(canonical.as_path()) {
        warning = Some("This is your whole home folder. Scanning it works but takes longer than a projects folder.".into());
    } else if canonical.to_string_lossy().starts_with("\\\\") {
        warning = Some("Network locations can be very slow to scan.".into());
    }
    Ok(FolderInfo {
        path: canonical,
        exists: true,
        warning,
    })
}

#[tauri::command]
pub fn open_project_folder(
    app: AppHandle,
    ctx: State<'_, Arc<AppCtx>>,
    project_id: String,
) -> Result<(), String> {
    let path = ctx.with_project(&project_id, |p| p.path.clone())?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())?;
    {
        let mut state = AppCtx::lock(&ctx.state);
        state.touch_activity(&path, Utc::now());
    }
    let _ = ctx.save_state();
    Ok(())
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}
