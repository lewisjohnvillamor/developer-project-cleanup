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
#[cfg(windows)]
pub fn is_link(meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_type().is_symlink() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub fn is_link(meta: &Metadata) -> bool {
    meta.file_type().is_symlink()
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

/// Bytes this file actually occupies on disk, which is what removing it
/// frees. This is not its length:
///
/// * `node_modules` is mostly tiny files, and a 200-byte file still consumes
///   a whole block, so the length *understates* the real cost;
/// * a sparse file reports a length it does not occupy, so the length
///   *overstates* it, sometimes by orders of magnitude;
/// * filesystem compression means the same again.
///
/// Unix reports allocated blocks directly. Windows has no equivalent in
/// `std` (`GetCompressedFileSize` would need a bindings crate), so the
/// length is used there and the number is approximate for compressed or
/// sparse files.
#[cfg(unix)]
pub fn allocated_size(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    // `blocks()` is always in 512-byte units, whatever the block size.
    meta.blocks() * 512
}

#[cfg(not(unix))]
pub fn allocated_size(meta: &Metadata) -> u64 {
    meta.len()
}

/// How many directory entries point at this file's contents. More than one
/// means the bytes are shared, so removing this copy may free nothing.
/// Returns `None` where the platform does not expose it through `std`.
#[cfg(unix)]
pub fn link_count(meta: &Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.nlink())
}

#[cfg(not(unix))]
pub fn link_count(_meta: &Metadata) -> Option<u64> {
    None
}

/// Identity of a file's contents, so the same bytes reached through several
/// hard links are recognised as one thing.
#[cfg(unix)]
pub fn content_id(meta: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
pub fn content_id(_meta: &Metadata) -> Option<(u64, u64)> {
    None
}

/// Accounts for hard-linked files while measuring one tree.
///
/// Package managers that share a store — pnpm most visibly — hard-link the
/// same bytes into many projects. Counting such a file once per link would
/// overstate a tree, and counting it at all overstates what deleting the
/// tree frees, because the bytes survive as long as a link outside the tree
/// remains.
///
/// So: a file with one link counts immediately. A file with several is held
/// back, and at the end its bytes count only if every one of its links was
/// found inside this tree. Only multiply-linked files are remembered, which
/// keeps the bookkeeping proportional to how much sharing there is rather
/// than to the size of the tree.
#[derive(Debug, Default)]
pub struct LinkAccounting {
    shared: std::collections::HashMap<(u64, u64), SharedFile>,
}

#[derive(Debug)]
struct SharedFile {
    bytes: u64,
    links: u64,
    seen: u64,
}

impl LinkAccounting {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file. Returns the bytes to count immediately; shared files
    /// return 0 here and are resolved by [`Self::shared_bytes_freed`].
    pub fn observe(&mut self, meta: &Metadata) -> u64 {
        let bytes = allocated_size(meta);
        let links = link_count(meta).unwrap_or(1);
        if links <= 1 {
            return bytes;
        }
        let Some(id) = content_id(meta) else {
            return bytes;
        };
        let entry = self.shared.entry(id).or_insert(SharedFile {
            bytes,
            links,
            seen: 0,
        });
        entry.seen += 1;
        0
    }

    /// Bytes from shared files that removing this tree would actually free:
    /// those whose every link lives inside it.
    pub fn shared_bytes_freed(&self) -> u64 {
        self.shared
            .values()
            .filter(|f| f.seen >= f.links)
            .map(|f| f.bytes)
            .sum()
    }

    /// Bytes that look like they belong to this tree but are shared with
    /// something outside it, so removing the tree does not free them.
    pub fn shared_bytes_elsewhere(&self) -> u64 {
        self.shared
            .values()
            .filter(|f| f.seen < f.links)
            .map(|f| f.bytes)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.shared.is_empty()
    }
}
