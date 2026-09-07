//! Read-only report of global toolchain caches (`~/.cargo/registry`, the npm
//! cache, the pnpm store, …). Hibernate never removes these; it shows their
//! size and the official command that cleans them.

use crate::scanner::size::measure_tree;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalCache {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    pub exists: bool,
    pub bytes: u64,
    pub file_count: u64,
    /// The tool's own cleanup command, when it has one.
    pub clean_command: Option<String>,
    pub note: String,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn local_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| home().join(".local/share"))
}

fn user_cache() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| home().join(".cache"))
}

/// Candidate cache locations for this machine. Paths that do not exist are
/// still returned (with `exists = false`) so the UI can explain them.
pub fn known_caches() -> Vec<GlobalCache> {
    let mk = |id: &str, label: &str, path: PathBuf, clean: Option<&str>, note: &str| GlobalCache {
        id: id.into(),
        label: label.into(),
        path,
        exists: false,
        bytes: 0,
        file_count: 0,
        clean_command: clean.map(str::to_string),
        note: note.into(),
    };
    let cargo_home = env_path("CARGO_HOME").unwrap_or_else(|| home().join(".cargo"));
    let npm_cache = env_path("npm_config_cache").unwrap_or_else(|| {
        if cfg!(windows) {
            local_data().join("npm-cache")
        } else {
            home().join(".npm/_cacache")
        }
    });
    let pnpm_store = if cfg!(windows) {
        local_data().join("pnpm/store")
    } else if cfg!(target_os = "macos") {
        home().join("Library/pnpm/store")
    } else {
        local_data().join("pnpm/store")
    };
    let yarn_cache = if cfg!(windows) {
        local_data().join("Yarn/Cache")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Caches/Yarn")
    } else {
        user_cache().join("yarn")
    };
    let pip_cache = if cfg!(windows) {
        local_data().join("pip/Cache")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Caches/pip")
    } else {
        user_cache().join("pip")
    };
    let uv_cache = env_path("UV_CACHE_DIR").unwrap_or_else(|| {
        if cfg!(target_os = "macos") {
            home().join("Library/Caches/uv")
        } else {
            user_cache().join("uv")
        }
    });
    let go_mod = env_path("GOMODCACHE").unwrap_or_else(|| {
        env_path("GOPATH")
            .unwrap_or_else(|| home().join("go"))
            .join("pkg/mod")
    });
    let gradle = env_path("GRADLE_USER_HOME")
        .unwrap_or_else(|| home().join(".gradle"))
        .join("caches");
    let composer = if cfg!(windows) {
        local_data().join("Composer")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Caches/composer")
    } else {
        user_cache().join("composer")
    };
    vec![
        mk(
            "cargo-registry",
            "Cargo registry",
            cargo_home.join("registry"),
            Some("cargo cache --autoclean  (cargo install cargo-cache)"),
            "Downloaded crate sources. Re-fetched on demand.",
        ),
        mk(
            "cargo-git",
            "Cargo git checkouts",
            cargo_home.join("git"),
            Some("cargo cache --autoclean"),
            "Git dependencies checked out by Cargo.",
        ),
        mk(
            "npm-cache",
            "npm cache",
            npm_cache,
            Some("npm cache clean --force"),
            "Content-addressed tarball cache shared by every Node project.",
        ),
        mk(
            "pnpm-store",
            "pnpm store",
            pnpm_store,
            Some("pnpm store prune"),
            "Hard-linked package store. Pruning removes packages no project references.",
        ),
        mk(
            "yarn-cache",
            "Yarn cache",
            yarn_cache,
            Some("yarn cache clean"),
            "Downloaded packages for Yarn.",
        ),
        mk(
            "bun-cache",
            "Bun install cache",
            home().join(".bun/install/cache"),
            Some("bun pm cache rm"),
            "Downloaded packages for Bun.",
        ),
        mk(
            "pip-cache",
            "pip cache",
            pip_cache,
            Some("pip cache purge"),
            "Wheels and HTTP responses cached by pip.",
        ),
        mk(
            "uv-cache",
            "uv cache",
            uv_cache,
            Some("uv cache clean"),
            "Wheels and builds cached by uv.",
        ),
        mk(
            "go-mod",
            "Go module cache",
            go_mod,
            Some("go clean -modcache"),
            "Downloaded Go modules.",
        ),
        mk(
            "gradle-caches",
            "Gradle caches",
            gradle,
            Some("rm -rf ~/.gradle/caches  (safe when no build is running)"),
            "Dependencies and build cache shared by Gradle projects.",
        ),
        mk(
            "maven-repo",
            "Maven local repository",
            home().join(".m2/repository"),
            None,
            "Dependencies shared by Maven projects. Re-downloaded on demand.",
        ),
        mk(
            "composer-cache",
            "Composer cache",
            composer,
            Some("composer clear-cache"),
            "Downloaded PHP packages.",
        ),
        mk(
            "cocoapods-cache",
            "CocoaPods cache",
            home().join("Library/Caches/CocoaPods"),
            Some("pod cache clean --all"),
            "Pod specs and downloaded pods.",
        ),
    ]
}

/// Measure every known cache that exists.
pub fn measure_caches(cancel: &AtomicBool) -> Vec<GlobalCache> {
    let mut out = known_caches();
    for c in &mut out {
        if c.path.is_dir() {
            c.exists = true;
            let size = measure_tree(&c.path, cancel);
            c.bytes = size.bytes;
            c.file_count = size.files;
        }
    }
    out.sort_by(|a, b| b.exists.cmp(&a.exists).then(b.bytes.cmp(&a.bytes)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_caches_with_commands() {
        let caches = known_caches();
        assert!(caches.iter().any(|c| c.id == "cargo-registry"));
        assert!(caches.iter().all(|c| !c.label.is_empty()));
    }
}
