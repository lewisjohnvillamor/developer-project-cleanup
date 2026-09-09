//! Where `log::` calls land.
//!
//! The app reports failed writes to the window, but a window is a poor bug
//! report: the user needs a file they can attach. This is a deliberately
//! small sink — one file next to the app's other data, truncated when it
//! grows past a megabyte — rather than a logging framework, because that is
//! the whole requirement.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Past this size the log is started again. A developer tool that runs for
/// months should not quietly grow a log nobody reads.
const MAX_BYTES: u64 = 1_000_000;

struct FileLogger {
    level: log::LevelFilter,
    file: Mutex<Option<File>>,
}

impl log::Log for FileLogger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        meta.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {} {}\n",
            chrono::Utc::now().to_rfc3339(),
            record.level(),
            record.target(),
            record.args()
        );
        // Always to stderr, so `cargo run` and a terminal launch show it too.
        eprint!("{line}");
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

static LOGGER: OnceLock<FileLogger> = OnceLock::new();

/// Open the log for appending, starting it again when it has grown too big.
/// `None` when the file cannot be opened, which leaves stderr as the sink.
fn open_log(path: &Path) -> Option<File> {
    if fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = fs::remove_file(path);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Start logging to `path`. Failing to open the file is not fatal: logging to
/// stderr alone is better than refusing to start the app.
pub fn init(path: &Path, level: log::LevelFilter) {
    let file = open_log(path);
    let logger = LOGGER.get_or_init(|| FileLogger {
        level,
        file: Mutex::new(file),
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_oversized_log_is_started_again_and_a_small_one_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hibernate.log");

        fs::write(&path, b"earlier run\n").unwrap();
        let mut file = open_log(&path).unwrap();
        file.write_all(b"later run\n").unwrap();
        drop(file);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "earlier run\nlater run\n"
        );

        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        drop(open_log(&path).unwrap());
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            0,
            "oversized log starts again"
        );
    }

    /// A log that cannot be opened must not stop the app from starting.
    #[test]
    fn an_unopenable_log_is_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let blocked = tmp.path().join("not-a-dir");
        fs::write(&blocked, b"x").unwrap();
        assert!(open_log(&blocked.join("hibernate.log")).is_none());
    }
}
