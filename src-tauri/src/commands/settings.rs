use crate::state::AppCtx;
use hibernate_core::config::Settings;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_settings(ctx: State<'_, Arc<AppCtx>>) -> Settings {
    AppCtx::lock(&ctx.settings).clone()
}

#[tauri::command]
pub fn update_settings(
    ctx: State<'_, Arc<AppCtx>>,
    settings: Settings,
) -> Result<Settings, String> {
    let clean = settings.sanitised();
    {
        let mut current = AppCtx::lock(&ctx.settings);
        *current = clean.clone();
    }
    ctx.save_settings()?;
    Ok(clean)
}
