//! The receipt for the core promise.
//!
//! Every other test checks one rule in isolation. This one runs a real
//! cleanup against a deliberately hostile project tree, then compares a
//! manifest of *the entire filesystem under the fixture* before and after.
//!
//! The assertion is not "the right things were deleted". It is the stronger
//! claim the product actually makes: **nothing changed except the folders
//! the user was shown and approved.** Anything else — a followed symlink, a
//! protected file, a nested project, a committed folder, anything at all
//! outside the plan — fails the test.

use hibernate_core::cleanup::hibernate::{
    self as hib, ExecutionContext, HibernateRequest, SelectedProject,
};
use hibernate_core::cleanup::quarantine::Quarantine;
use hibernate_core::cleanup::rules::RuleSet;
use hibernate_core::config::{AppState, Disposition};
use hibernate_core::scanner::{scan, ScanOptions};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// What one filesystem entry is, in enough detail that any change shows up.
#[derive(Debug, PartialEq, Eq, Clone)]
enum Entry {
    Dir,
    File { len: u64, content: u64 },
    Symlink { target: PathBuf },
}

/// Record every entry under `root`, following nothing.
fn manifest(root: &Path) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let key = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).unwrap_or_default();
                out.insert(key, Entry::Symlink { target });
            } else if meta.is_dir() {
                out.insert(key, Entry::Dir);
                stack.push(path);
            } else {
                let bytes = fs::read(&path).unwrap_or_default();
                let mut h = DefaultHasher::new();
                bytes.hash(&mut h);
                out.insert(
                    key,
                    Entry::File {
                        len: meta.len(),
                        content: h.finish(),
                    },
                );
            }
        }
    }
    out
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[test]
fn a_real_cleanup_touches_nothing_it_was_not_shown() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path();
    // The scanner canonicalises its roots, so the paths it hands back are
    // spelled differently from the ones this test builds: on macOS a temp dir
    // lives under `/var`, a symlink to `/private/var`, and on Windows
    // canonicalisation yields a `\\?\` verbatim path. Keep both spellings so
    // engine paths can be related to fixture paths.
    let canonical_fixture = fs::canonicalize(fixture).unwrap();

    // ---- somewhere precious, outside the scan root entirely ---------------
    let vault = fixture.join("vault");
    write(&vault.join("thesis.txt"), "years of work");
    write(&vault.join("photos/wedding.jpg"), "binary-ish");

    // ---- the folder the user actually asks us to scan ---------------------
    let root = fixture.join("Projects");

    // A Node project with the usual removable folders, plus traps.
    let web = root.join("web");
    write(&web.join("package.json"), r#"{"name":"web"}"#);
    write(&web.join("package-lock.json"), "{}");
    write(&web.join("src/index.ts"), "export const x = 1;");
    write(&web.join("public/logo.svg"), "<svg/>");
    write(&web.join(".env"), "SECRET=hunter2");
    write(&web.join("app.db"), "sqlite");
    write(&web.join("migrations/001.sql"), "create table t;");
    write(&web.join("README.md"), "# web");
    // Genuinely removable.
    write(
        &web.join("node_modules/left-pad/index.js"),
        "module.exports=1",
    );
    write(&web.join(".next/cache/blob"), "cache");
    write(&web.join("coverage/lcov.info"), "coverage");

    // A Rust project whose target/ is removable.
    let api = root.join("api");
    write(&api.join("Cargo.toml"), "[package]\nname='api'");
    write(&api.join("src/main.rs"), "fn main() {}");
    write(&api.join("target/debug/api"), "binary");

    // A project that commits its build output: Git must veto the rule.
    let site = root.join("site");
    write(&site.join("package.json"), r#"{"name":"site"}"#);
    write(&site.join("dist/bundle.js"), "committed output");
    write(&site.join(".gitignore"), "node_modules/\n");
    write(&site.join("node_modules/dep/index.js"), "dep");
    let have_git = hibernate_core::git::git_available();
    if have_git {
        git(&site, &["init", "-q"]);
        git(&site, &["add", "-A"]);
        git(&site, &["commit", "-qm", "init"]);
    }

    // A nested project inside a removable-looking folder name must survive
    // as its own project rather than be swallowed.
    let nested = root.join("workspace/tools/cli");
    write(&nested.join("Cargo.toml"), "[package]\nname='cli'");
    write(&nested.join("src/main.rs"), "fn main() {}");

    // The trap: a symlink from inside a removable folder to the vault.
    // Following it would destroy the thesis.
    #[cfg(unix)]
    std::os::unix::fs::symlink(&vault, web.join("node_modules/.escape")).unwrap();

    let before = manifest(fixture);

    // ---- scan, plan and actually delete ----------------------------------
    let roots = vec![root.clone()];
    let opts = ScanOptions::default();
    let result = scan(
        &roots,
        &opts,
        &AppState::default(),
        &AtomicBool::new(false),
        &|_| {},
    );
    assert!(
        result.projects.len() >= 4,
        "expected the four projects, found {}",
        result.projects.len()
    );

    let request = HibernateRequest {
        // Select everything, including review items: the widest blast radius
        // a user could ask for.
        selection: result
            .projects
            .iter()
            .map(|p| SelectedProject {
                project_id: p.id.clone(),
                artifact_paths: None,
            })
            .collect(),
        include_review: true,
    };
    let plan = hib::plan(&result.projects, &request, Disposition::Permanent);
    assert!(!plan.is_empty(), "the plan should contain real work");

    let quarantine = Quarantine::new(&fixture.join("quarantine"));
    let rules = RuleSet::builtin();
    let ctx = ExecutionContext {
        scan_roots: &roots,
        rules: &rules,
        projects: &result.projects,
        quarantine: &quarantine,
    };
    let entry = hib::execute(&plan, &ctx, &AtomicBool::new(false), &|_| {});
    assert_eq!(entry.error_count, 0, "the cleanup itself should succeed");
    assert!(
        entry.total_recovered > 0,
        "something should have been removed"
    );

    // ---- the receipt ------------------------------------------------------
    let after = manifest(fixture);

    // Every path the plan was allowed to remove, as manifest keys.
    let approved: Vec<String> = plan
        .projects
        .iter()
        .flat_map(|p| p.artifacts.iter())
        .map(|a| {
            a.path
                .strip_prefix(&canonical_fixture)
                .or_else(|_| a.path.strip_prefix(fixture))
                .unwrap_or_else(|_| {
                    panic!("planned artifact outside the fixture: {}", a.path.display())
                })
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let was_approved = |key: &str| {
        approved
            .iter()
            .any(|a| key == a || key.starts_with(&format!("{a}/")))
    };

    let mut unapproved_removals = Vec::new();
    let mut modifications = Vec::new();
    for (key, before_entry) in &before {
        // The quarantine folder is ours and did not exist before.
        match after.get(key) {
            None => {
                if !was_approved(key) {
                    unapproved_removals.push(key.clone());
                }
            }
            Some(after_entry) if after_entry != before_entry => {
                modifications.push(key.clone());
            }
            Some(_) => {}
        }
    }
    let additions: Vec<String> = after
        .keys()
        .filter(|k| !before.contains_key(*k) && !k.starts_with("quarantine"))
        .cloned()
        .collect();

    assert!(
        unapproved_removals.is_empty(),
        "removed without being shown to the user: {unapproved_removals:#?}"
    );
    assert!(
        modifications.is_empty(),
        "modified files that should not have been touched: {modifications:#?}"
    );
    assert!(
        additions.is_empty(),
        "created files outside the quarantine: {additions:#?}"
    );

    // And the specific things we promise, stated plainly.
    assert_eq!(
        manifest(&vault),
        {
            let mut expected = BTreeMap::new();
            for (k, v) in &before {
                if let Some(rest) = k.strip_prefix("vault/") {
                    expected.insert(rest.to_string(), v.clone());
                }
            }
            expected
        },
        "the vault outside the scan root must be byte-identical"
    );
    for kept in [
        "Projects/web/.env",
        "Projects/web/src/index.ts",
        "Projects/web/app.db",
        "Projects/web/migrations/001.sql",
        "Projects/web/package-lock.json",
        "Projects/api/src/main.rs",
        "Projects/workspace/tools/cli/src/main.rs",
    ] {
        assert!(after.contains_key(kept), "{kept} must survive");
    }
    // The removable folders really did go.
    for gone in [
        "Projects/web/node_modules",
        "Projects/web/.next",
        "Projects/api/target",
    ] {
        assert!(!after.contains_key(gone), "{gone} should have been removed");
    }
    // Git's veto held: committed output survives even though `dist` matches a rule.
    if have_git {
        assert!(
            after.contains_key("Projects/site/dist/bundle.js"),
            "committed dist/ must survive because Git tracks it"
        );
    }
}
