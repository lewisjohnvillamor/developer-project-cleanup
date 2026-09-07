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

/// Move items back out of the Trash by their original paths. Supported on
/// Windows and freedesktop Linux; macOS has no public API for it.
pub fn restore_from_trash(originals: &[std::path::PathBuf]) -> (usize, Vec<String>) {
    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    {
        use trash::os_limited::{list, restore_all};
        let items = match list() {
            Ok(items) => items,
            Err(e) => return (0, vec![format!("cannot list the Trash: {e}")]),
        };
        let mut errors = Vec::new();
        let mut restored = 0;
        for original in originals {
            let matching: Vec<trash::TrashItem> = items
                .iter()
                .filter(|i| i.original_path() == *original)
                .cloned()
                .collect();
            let Some(newest) = matching.into_iter().max_by_key(|i| i.time_deleted) else {
                errors.push(format!("{}: not found in the Trash", original.display()));
                continue;
            };
            if original.exists() {
                errors.push(format!(
                    "{}: a folder already exists at the original location",
                    original.display()
                ));
                continue;
            }
            match restore_all(vec![newest]) {
                Ok(()) => restored += 1,
                Err(e) => errors.push(format!("{}: {e}", original.display())),
            }
        }
        (restored, errors)
    }
    #[cfg(not(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    )))]
    {
        (
            0,
            originals
                .iter()
                .map(|p| format!("{}: restoring from the Trash is not supported on this platform; use Finder", p.display()))
                .collect(),
        )
    }
}

/// True when [`restore_from_trash`] can work on this platform.
pub fn trash_restore_supported() -> bool {
    cfg!(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real user Trash, so it only runs when asked for.
    #[test]
    #[ignore]
    fn trash_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("node_modules");
        std::fs::create_dir_all(dir.join("pkg")).unwrap();
        std::fs::write(dir.join("pkg/index.js"), "x").unwrap();
        send_to_trash(&dir).unwrap();
        assert!(!dir.exists());
        let (restored, errors) = restore_from_trash(std::slice::from_ref(&dir));
        assert_eq!(restored, 1, "{errors:?}");
        assert!(dir.join("pkg/index.js").exists());
    }
}
