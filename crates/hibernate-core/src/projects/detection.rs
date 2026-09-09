//! Decide whether a directory is a project root and, if so, describe it.

use super::stacks::{
    DOTNET_SUFFIXES, HASKELL_SUFFIXES, MARKERS, NODE_FRAMEWORKS, NODE_LOCKFILES, PYTHON_LOCKFILES,
    TERRAFORM_SUFFIXES,
};
use crate::cleanup::rules::glob_match;
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
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_link = crate::fsx::is_link(&meta);
        out.push(DirEntryLite {
            name: entry.file_name().to_string_lossy().into_owned(),
            // A link that points at a directory is not a directory to walk.
            is_dir: meta.is_dir() && !is_link,
            is_file: meta.is_file() && !is_link,
            is_symlink: is_link,
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
    /// Member globs declared by the workspace, relative to its root
    /// (`apps/*`, `crates/**`, `packages/ui`). Empty means "unknown, absorb
    /// every same-ecosystem child".
    pub workspace_globs: Vec<String>,
}

impl Detection {
    /// Whether a nested detection at `relative` (forward-slash path relative
    /// to this project) should be folded into this one.
    pub fn absorbs(&self, nested: &Detection, relative: &str) -> bool {
        if !self.is_workspace || !nested.stacks.iter().all(|s| self.stacks.contains(s)) {
            return false;
        }
        if self.workspace_globs.is_empty() {
            return true;
        }
        self.workspace_globs
            .iter()
            .any(|g| path_glob_match(g, relative))
    }
}

/// Glob for relative paths: `*` matches within one segment, `**` matches any
/// number of segments. A pattern without wildcards must match exactly.
pub fn path_glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_matches('/').trim_start_matches("./");
    let path = path.trim_matches('/');
    fn go(p: &[&str], n: &[&str]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some(&"**") => (0..=n.len()).any(|i| go(&p[1..], &n[i..])),
            Some(seg) => match n.first() {
                Some(name) if glob_match(seg, name) => go(&p[1..], &n[1..]),
                _ => false,
            },
        }
    }
    let p: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let n: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    go(&p, &n)
}

/// Maximum marker file size we are willing to parse (package.json etc.).
const MAX_MARKER_BYTES: u64 = 2 * 1024 * 1024;

pub fn detect(dir: &Path, entries: &[DirEntryLite]) -> Option<Detection> {
    let mut det = Detection::default();
    let has = |name: &str| entries.iter().any(|e| e.name == name && !e.is_dir);
    let has_dir = |name: &str| entries.iter().any(|e| e.name == name && e.is_dir);
    let files_with = |suffixes: &[&str]| -> Vec<String> {
        entries
            .iter()
            .filter(|e| !e.is_dir && suffixes.iter().any(|s| e.name.ends_with(s)))
            .map(|e| e.name.clone())
            .collect()
    };

    for (marker, stack) in MARKERS {
        if has(marker) {
            det.markers.push((*marker).to_string());
            if !det.stacks.contains(stack) {
                det.stacks.push(*stack);
            }
        }
    }
    let dotnet_markers = files_with(DOTNET_SUFFIXES);
    if !dotnet_markers.is_empty() {
        det.stacks.push(Stack::DotNet);
        if let Some(sln) = dotnet_markers.iter().find(|m| m.ends_with(".sln")) {
            det.is_workspace = true;
            det.workspace_globs = parse_sln_members(&read_small(&dir.join(sln)));
        }
        det.markers.extend(dotnet_markers);
    }
    let haskell_markers = files_with(HASKELL_SUFFIXES);
    if !haskell_markers.is_empty() && !det.stacks.contains(&Stack::Haskell) {
        det.stacks.push(Stack::Haskell);
        det.markers.extend(haskell_markers);
    }
    let tf_markers = files_with(TERRAFORM_SUFFIXES);
    if !tf_markers.is_empty() {
        det.stacks.push(Stack::Terraform);
        det.markers.push(tf_markers[0].clone());
    }
    if has_dir("Assets") && has_dir("ProjectSettings") && has_dir("Packages") {
        det.stacks.push(Stack::Unity);
        det.markers.push("ProjectSettings/".into());
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
            det.workspace_globs = parse_toml_string_array(&text, "[workspace]", "members");
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
        det.workspace_globs = read_small(&dir.join("go.work"))
            .lines()
            .filter_map(|l| {
                l.trim().strip_prefix("use ").map(|u| {
                    u.trim()
                        .trim_matches(|c| c == '(' || c == ')')
                        .trim()
                        .to_string()
                })
            })
            .filter(|u| !u.is_empty())
            .collect();
    }
    if det.stacks.contains(&Stack::Maven) {
        let text = read_small(&dir.join("pom.xml"));
        if text.contains("<module>") {
            det.is_workspace = true;
            det.workspace_globs = text
                .split("<module>")
                .skip(1)
                .filter_map(|rest| rest.split("</module>").next())
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
        }
    }
    if det.stacks.contains(&Stack::Gradle) {
        for settings in ["settings.gradle", "settings.gradle.kts"] {
            if has(settings) {
                det.is_workspace = true;
                det.workspace_globs = parse_gradle_includes(&read_small(&dir.join(settings)));
                break;
            }
        }
    }
    if det.stacks.contains(&Stack::Dart) {
        let text = read_small(&dir.join("pubspec.yaml"));
        if text.contains("flutter") {
            det.frameworks.push("Flutter".into());
        }
    }
    if det.stacks.contains(&Stack::Elixir) {
        let text = read_small(&dir.join("mix.exs"));
        if text.contains(":phoenix") {
            det.frameworks.push("Phoenix".into());
        }
        if text.contains("apps_path:") {
            det.is_workspace = true;
            det.workspace_globs = vec!["apps/*".into()];
        }
    }
    if det.stacks.contains(&Stack::Haskell) {
        det.package_manager = Some(if has("stack.yaml") { "stack" } else { "cabal" }.into());
    }
    if det.stacks.contains(&Stack::Swift) && !det.stacks.contains(&Stack::CocoaPods) {
        det.package_manager = Some("swift".into());
    }

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
        if let Some(ws) = json.get("workspaces") {
            det.is_workspace = true;
            let list = ws
                .as_array()
                .cloned()
                .or_else(|| ws.get("packages").and_then(|p| p.as_array()).cloned())
                .unwrap_or_default();
            det.workspace_globs = list
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
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
    if has("pnpm-workspace.yaml") {
        det.is_workspace = true;
        let globs = parse_yaml_list(&read_small(&dir.join("pnpm-workspace.yaml")), "packages");
        if !globs.is_empty() {
            det.workspace_globs = globs;
        }
    }
    if has("lerna.json") || has("nx.json") {
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

/// `key = ["a", "b"]` inside `[section]`, possibly spanning lines.
pub fn parse_toml_string_array(text: &str, section: &str, key: &str) -> Vec<String> {
    let mut in_section = false;
    let mut collecting = false;
    let mut buf = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if collecting {
                break;
            }
            in_section = t == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if !collecting {
            if let Some(rest) = t.strip_prefix(key) {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    collecting = true;
                    buf.push_str(rest);
                    if rest.contains(']') {
                        break;
                    }
                }
            }
        } else {
            buf.push_str(t);
            if t.contains(']') {
                break;
            }
        }
    }
    let inner = buf.split_once('[').map(|(_, r)| r).unwrap_or("");
    let inner = inner.split(']').next().unwrap_or("");
    inner
        .split(',')
        .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect()
}

/// Minimal YAML: the sequence under a top-level `key:`.
pub fn parse_yaml_list(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_key = false;
    for line in text.lines() {
        let t = line.trim_end();
        if t.trim_start() != t && in_key {
            if let Some(item) = t.trim().strip_prefix("- ") {
                let v = item.trim().trim_matches(|c| c == '"' || c == '\'');
                if !v.is_empty() && !v.starts_with('#') {
                    out.push(v.to_string());
                }
            }
            continue;
        }
        if t.starts_with('-') && in_key {
            if let Some(item) = t.strip_prefix("- ") {
                out.push(
                    item.trim()
                        .trim_matches(|c| c == '"' || c == '\'')
                        .to_string(),
                );
            }
            continue;
        }
        in_key = t.trim_end_matches(':') == key && t.ends_with(':');
    }
    out
}

/// Gradle `include ':app', ':lib:core'` → `app`, `lib/core`.
pub fn parse_gradle_includes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some(rest) = t
            .strip_prefix("include")
            .or_else(|| t.strip_prefix("includeBuild"))
        else {
            continue;
        };
        let rest = rest.trim_start_matches('(').trim();
        for part in rest.split(',') {
            let name = part
                .trim()
                .trim_end_matches(')')
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');
            if name.is_empty() {
                continue;
            }
            out.push(name.trim_start_matches(':').replace(':', "/"));
        }
    }
    out
}

/// Directories of the projects listed in a Visual Studio solution.
pub fn parse_sln_members(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if !t.starts_with("Project(") {
            continue;
        }
        let parts: Vec<&str> = t.split('"').collect();
        // Project("{guid}") = "Name", "Path\To\Name.csproj", "{guid}"
        if let Some(path) = parts.get(5) {
            let p = path.replace('\\', "/");
            if let Some((dir, _)) = p.rsplit_once('/') {
                out.push(dir.to_string());
            }
        }
    }
    out
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
            "[workspace]\nmembers = [\n  \"crates/*\",\n  \"tools/cli\",\n]\n",
        )
        .unwrap();
        fs::write(tmp.path().join("pyproject.toml"), "[project]\nname='x'\n").unwrap();
        fs::write(tmp.path().join("poetry.lock"), "").unwrap();
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(det.stacks, vec![Stack::Rust, Stack::Python]);
        assert!(det.is_workspace);
        assert_eq!(det.workspace_globs, vec!["crates/*", "tools/cli"]);
        assert_eq!(det.package_manager.as_deref(), Some("poetry"));
    }

    #[test]
    fn detects_dotnet_solution_members() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("App.sln"),
            "Project(\"{FAE04EC0}\") = \"Api\", \"src\\Api\\Api.csproj\", \"{1}\"\nEndProject\n",
        )
        .unwrap();
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(det.stacks, vec![Stack::DotNet]);
        assert!(det.is_workspace);
        assert_eq!(det.workspace_globs, vec!["src/Api"]);
    }

    #[test]
    fn detects_new_ecosystems() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Package.swift"), "").unwrap();
        fs::write(tmp.path().join("mix.exs"), "deps: [{:phoenix, \"~> 1.7\"}]").unwrap();
        fs::write(tmp.path().join("app.cabal"), "").unwrap();
        fs::write(tmp.path().join("build.zig"), "").unwrap();
        fs::write(tmp.path().join("main.tf"), "").unwrap();
        for d in ["Assets", "ProjectSettings", "Packages"] {
            fs::create_dir(tmp.path().join(d)).unwrap();
        }
        let det = detect(tmp.path(), &entries(tmp.path())).unwrap();
        assert_eq!(
            det.stacks,
            vec![
                Stack::Swift,
                Stack::Elixir,
                Stack::Haskell,
                Stack::Zig,
                Stack::Unity,
                Stack::Terraform
            ]
        );
        assert!(det.frameworks.contains(&"Phoenix".to_string()));
    }

    #[test]
    fn plain_directory_is_not_a_project() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("notes.txt"), "hi").unwrap();
        assert!(detect(tmp.path(), &entries(tmp.path())).is_none());
    }

    #[test]
    fn workspace_absorbs_only_declared_members() {
        let root = Detection {
            stacks: vec![Stack::Node],
            is_workspace: true,
            workspace_globs: vec!["apps/*".into(), "packages/**".into()],
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
        assert!(root.absorbs(&member, "apps/web"));
        assert!(root.absorbs(&member, "packages/ui/icons"));
        assert!(!root.absorbs(&member, "examples/demo"));
        assert!(!root.absorbs(&other, "apps/web"));
        let unknown = Detection {
            stacks: vec![Stack::Node],
            is_workspace: true,
            ..Default::default()
        };
        assert!(unknown.absorbs(&member, "anything/at/all"));
    }

    #[test]
    fn parses_workspace_files() {
        assert_eq!(
            parse_yaml_list(
                "packages:\n  - 'apps/*'\n  - \"packages/*\"\n  # comment\nother: 1\n",
                "packages"
            ),
            vec!["apps/*", "packages/*"]
        );
        assert_eq!(
            parse_gradle_includes(
                "rootProject.name = 'x'\ninclude ':app', ':lib:core'\ninclude(\":tools\")\n"
            ),
            vec!["app", "lib/core", "tools"]
        );
        assert_eq!(
            parse_toml_string_array(
                "[package]\nname='a'\n[workspace]\nmembers = [\"a\", 'b/*']\n",
                "[workspace]",
                "members"
            ),
            vec!["a", "b/*"]
        );
        assert!(path_glob_match("crates/*", "crates/core"));
        assert!(!path_glob_match("crates/*", "crates/core/sub"));
        assert!(path_glob_match("crates/**", "crates/core/sub"));
        assert!(path_glob_match("./tools/cli", "tools/cli"));
    }
}
