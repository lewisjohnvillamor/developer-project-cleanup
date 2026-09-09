//! Filesystem checks whose correct answer differs by platform.
//!
//! The engine's safety rests on never following a link out of the project it
//! is cleaning. On Unix that means symbolic links. On Windows it means any
//! *reparse point*: directory symbolic links, junctions (`mklink /J`, which
//! ordinary users can create without elevation), volume mount points, and
//! newer kinds such as OneDrive placeholders and AppExecLinks.
//!
//! `std`'s `is_symlink()` is not a sufficient test on Windows, because it
//! answers for the symlink and mount-point reparse tags only. A directory
//! carrying any other reparse tag reports `is_dir() == true` and
//! `is_symlink() == false`, so a recursive walk would descend through it and
//! a recursive delete would empty whatever sits on the other side.
//!
//! Everything here therefore asks a blunter question: *is this entry a door
//! to somewhere else?* Anything that might be gets treated as a leaf, never
//! descended into and never followed.

use std::fs::Metadata;
use std::io;
use std::path::Path;

/// True when the entry is a link to somewhere else and must not be followed:
/// a symbolic link on any platform, or any reparse point on Windows.
pub fn is_link(meta: &Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        return meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

/// True when the entry is a directory we may safely descend into: a real
/// directory that is not a link to somewhere else.
pub fn is_real_dir(meta: &Metadata) -> bool {
    meta.is_dir() && !is_link(meta)
}

/// Read link-aware metadata for a path without following it.
pub fn symlink_metadata(path: &Path) -> io::Result<Metadata> {
    std::fs::symlink_metadata(path)
}

/// Remove a link itself, never its target. Directory-shaped links
/// (Windows junctions and directory symlinks, and symlinks to directories on
/// Unix) need `remove_dir`; file-shaped ones need `remove_file`. Try both,
/// because the shape is not always knowable up front.
pub fn remove_link(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(first) => std::fs::remove_dir(path).map_err(|_| first),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plain_entries_are_not_links() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("f");
        std::fs::write(&file, "x").unwrap();
        assert!(!is_link(&symlink_metadata(&file).unwrap()));
        assert!(!is_link(&symlink_metadata(tmp.path()).unwrap()));
        assert!(is_real_dir(&symlink_metadata(tmp.path()).unwrap()));
        assert!(!is_real_dir(&symlink_metadata(&file).unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_links_and_never_real_dirs() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let meta = symlink_metadata(&link).unwrap();
        assert!(is_link(&meta));
        assert!(
            !is_real_dir(&meta),
            "a link to a directory is not a directory to descend into"
        );

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(
            target.is_dir(),
            "removing the link must leave the target alone"
        );
    }

    /// A junction reports as a directory and, depending on the Windows
    /// version, may not report as a symlink. It must still be treated as a
    /// link so that walks stop at it.
    #[cfg(windows)]
    #[test]
    fn junctions_are_links_and_never_real_dirs() {
        use std::process::Command;
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("important.txt"), "keep me").unwrap();
        let link = tmp.path().join("junction");

        let made = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !made {
            return; // junction creation unavailable; nothing to assert
        }

        let meta = symlink_metadata(&link).unwrap();
        assert!(
            is_link(&meta),
            "a junction is a reparse point and must count as a link"
        );
        assert!(
            !is_real_dir(&meta),
            "a walk must not descend through a junction"
        );

        remove_link(&link).unwrap();
        assert!(
            target.join("important.txt").exists(),
            "removing the junction must leave the target's contents alone"
        );
    }
}
