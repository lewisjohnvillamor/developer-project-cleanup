//! Operating-system Trash / Recycle Bin.

use std::path::Path;

pub fn send_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

/// Whether the platform Trash is likely to work at all (it is not on some
/// headless Linux sessions or network shares).
pub fn trash_available() -> bool {
    cfg!(any(windows, target_os = "macos"))
        || std::env::var_os("XDG_DATA_HOME").is_some()
        || dirs::home_dir().is_some()
}
