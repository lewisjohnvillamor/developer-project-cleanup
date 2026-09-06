//! Decide whether a directory is a project root and, if so, describe it.

use super::stacks::{DOTNET_SUFFIXES, MARKERS, NODE_FRAMEWORKS, NODE_LOCKFILES, PYTHON_LOCKFILES};
use crate::model::Stack;
use std::fs;
use std::io;
use std::path::Path;

/// Cheap directory listing that avoids `stat` where the platform allows it.
#[derive(Debug, Clone)]
pub struct DirEntryLite {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}

pub fn read_dir_lite(dir: &Path) -> io::Result<Vec<DirEntryLite>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        out.push(DirEntryLite {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            is_symlink: ft.is_symlink(),
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Default)]
pub struct Detection {
    pub stacks: Vec<Stack>,
    pub frameworks: Vec<String>,
    pub package_manager: Option<String>,
    /// Marker files that triggered detection, for explanations.
    pub markers: Vec<String>,
    /// True when the project declares sub-packages (Cargo workspace, npm /
    /// pnpm workspaces, Gradle multi-project, Maven modules, .sln, go.work).
    /// Same-ecosystem projects nested inside a workspace are treated as
    /// members of it rather than separate projects.
    pub is_workspace: bool,
}

impl Detection {
    /// Whether a nested detection should be folded into this one.
    pub fn absorbs(&self, nested: &Detection) -> bool {
        self.is_workspace && nested.stacks.iter().all(|s| self.stacks.contains(s))
    }
}

/// Maximum marker file size we are willing to parse (package.json etc.).
const MAX_MARKER_BYTES: u64 = 2 * 1024 * 1024;

pub fn detect(dir: &Path, entries: &[DirEntryLite]) -> Option<Detection> {
    let mut det = Detection::default();
    let has = |name: &str| entries.iter().any(|e| e.name == name && !e.is_dir);
    let has_dir = |name: &str| entries.iter().any(|e| e.name == name && e.is_dir);

    for (marker, stack) in MARKERS {
        if has(marker) {
            det.markers.push((*marker).to_string());
            if !det.stacks.contains(stack) {
                det.stacks.push(*stack);
            }
        }
    }
    let dotnet_markers: Vec<&DirEntryLite> = entries
        .iter()
        .filter(|e| !e.is_dir && DOTNET_SUFFIXES.iter().any(|s| e.name.ends_with(s)))
        .collect();
    if !dotnet_markers.is_empty() {
        det.stacks.push(Stack::DotNet);
        for m in &dotnet_markers {
            det.markers.push(m.name.clone());
        }
        if dotnet_markers.iter().any(|m| m.name.ends_with(".sln")) {
            det.is_workspace = true;
        }
    }

    if det.stacks.is_empty() {
        return None;
    }

    // Keep stacks in a stable, meaningful order.
    det.stacks.sort();
    det.stacks.dedup();

    if det.stacks.contains(&Stack::Node) {
        detect_node(dir, entries, &mut det);
    }
    if det.stacks.contains(&Stack::Rust) {
        let text = read_small(&dir.join("Cargo.toml"));
        if text.lines().any(|l| l.trim() == "[workspace]") {
            det.is_workspace = true;
        }
        if has_dir("src-tauri") && !det.frameworks.iter().any(|f| f == "Tauri") {
            det.frameworks.push("Tauri".into());
        }
    }
    if det.stacks.contains(&Stack::Python) {
        for (lock, pm) in PYTHON_LOCKFILES {
            if has(lock) {
                det.package_manager.get_or_insert_with(|| (*pm).to_string());
                break;
            }
        }
        if det.package_manager.is_none() {
            det.package_manager = Some("pip".into());
        }
    }
    if det.stacks.contains(&Stack::Go) && has("go.work") {
        det.is_workspace = true;
    }
    if det.stacks.contains(&Stack::Maven) {
        let text = read_small(&dir.join("pom.xml"));
        if text.contains("<module>") {
            det.is_workspace = true;
        }
    }
    if det.stacks.contains(&Stack::Gradle) && (has("settings.gradle") || has("settings.gradle.kts"))
    {
        det.is_workspace = true;
    }
    if det.stacks.contains(&Stack::Dart) {
        let text = read_small(&dir.join("pubspec.yaml"));
        if text.contains("flutter") {
            det.frameworks.push("Flutter".into());
        }
    }

    // A single package manager label for the UI: prefer Node's.
    Some(det)
}

fn detect_node(dir: &Path, entries: &[DirEntryLite], det: &mut Detection) {
    let has = |name: &str| entries.iter().any(|e| e.name == name && !e.is_dir);

    let mut pm: Option<String> = None;
    let mut frameworks: Vec<String> = Vec::new();

    let text = read_small(&dir.join("package.json"));
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(spec) = json.get("packageManager").and_then(|v| v.as_str()) {
            let name = spec.split('@').next().unwrap_or("").trim();
            if !name.is_empty() {
                pm = Some(name.to_string());
            }
        }
        if json.get("workspaces").is_some() {
            det.is_workspace = true;
        }
        let mut deps: Vec<String> = Vec::new();
        for key in ["dependencies", "devDependencies", "peerDependencies"] {
            if let Some(map) = json.get(key).and_then(|v| v.as_object()) {
                deps.extend(map.keys().cloned());
            }
        }
        for (dep, label) in NODE_FRAMEWORKS {
            if deps.iter().any(|d| d == dep) && !frameworks.iter().any(|f| f == label) {
                frameworks.push((*label).to_string());
            }
        }
        if deps.iter().any(|d| d == "typescript") || has("tsconfig.json") {
            frameworks.push("TypeScript".into());
        }
    } else if has("tsconfig.json") {
        frameworks.push("TypeScript".into());
    }

    if pm.is_none() {
        for (lock, name) in NODE_LOCKFILES {
            if has(lock) {
                pm = Some((*name).to_string());
                break;
            }
        }
    }
    if has("pnpm-workspace.yaml") || has("lerna.json") || has("nx.json") {
        det.is_workspace = true;
    }
    if has("manifest.json") {
        let manifest = read_small(&dir.join("manifest.json"));
        if manifest.contains("\"manifest_version\"") {
            frameworks.push("Chrome Extension".into());
        }
    }

    // Keep the list short: a primary framework, then at most two flavours.
    frameworks.truncate(3);
    for f in frameworks {
        if !det.frameworks.contains(&f) {
            det.frameworks.push(f);
        }
    }
    det.package_manager = pm.or_else(|| Some("npm".into()));
}

fn read_small(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(meta) if meta.len() <= MAX_MARKER_BYTES => fs::read_to_string(path).unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn entries(dir: &Path) -> Vec<DirEntryLite> {
        read_dir_lite(dir).unwrap()
    }

    #[test]
    fn detects_next_pnpm_project() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"web","dependencies":{"next":"14","react":"18"},"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(det.stacks, vec![Stack::Node]);
        assert_eq!(det.frameworks, vec!["Next.js", "React", "TypeScript"]);
        assert_eq!(det.package_manager.as_deref(), Some("pnpm"));
        assert!(!det.is_workspace);
    }

    #[test]
    fn detects_multi_stack_and_workspaces() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n",
        )
        .unwrap();
        fs::write(tmp.path().join("pyproject.toml"), "[project]\nname='x'\n").unwrap();
        fs::write(tmp.path().join("poetry.lock"), "").unwrap();
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(det.stacks, vec![Stack::Rust, Stack::Python]);
        assert!(det.is_workspace);
        assert_eq!(det.package_manager.as_deref(), Some("poetry"));
    }

    #[test]
    fn detects_dotnet_solution() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("App.sln"), "").unwrap();
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(det.stacks, vec![Stack::DotNet]);
        assert!(det.is_workspace);
    }

    #[test]
    fn plain_directory_is_not_a_project() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("notes.txt"), "hi").unwrap();
        assert!(detect(tmp.path(), &entries(tmp.path())).is_none());
    }

    #[test]
    fn workspace_absorbs_same_ecosystem_members() {
        let root = Detection {
            stacks: vec![Stack::Node],
            is_workspace: true,
            ..Default::default()
        };
        let member = Detection {
            stacks: vec![Stack::Node],
            ..Default::default()
        };
        let other = Detection {
            stacks: vec![Stack::Rust],
            ..Default::default()
        };
        assert!(root.absorbs(&member));
        assert!(!root.absorbs(&other));
    }
}
