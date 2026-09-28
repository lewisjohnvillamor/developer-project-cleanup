//! Reading the exact versions a project pins.
//!
//! A lockfile is the whole reason this works on a hibernated project: it is
//! protected, it survives the cleanup, and it names every version the project
//! would reinstall. Nothing here needs `node_modules/` or `target/` to be on
//! disk.
//!
//! Parsing is deliberately narrow. Each reader extracts name and version and
//! nothing else, and anything it cannot read it skips rather than guesses at:
//! a wrong package name is a false warning, and a security feature that cries
//! wolf gets switched off.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;

/// The registry a package came from. Matches OSV's ecosystem names, which is
/// what the advisory data is keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ecosystem {
    // Serialised as the registry writes it, so exported JSON reads the same
    // as the advisory data it came from rather than as Rust identifiers.
    #[serde(rename = "npm")]
    Npm,
    #[serde(rename = "crates.io")]
    CratesIo,
}

impl Ecosystem {
    /// The string OSV uses in `affected[].package.ecosystem`.
    pub fn osv_name(self) -> &'static str {
        match self {
            Ecosystem::Npm => "npm",
            Ecosystem::CratesIo => "crates.io",
        }
    }

    pub fn from_osv(name: &str) -> Option<Ecosystem> {
        // OSV qualifies some ecosystems with a distribution, e.g.
        // "Alpine:v3.16"; the part before the colon is the ecosystem.
        match name.split(':').next().unwrap_or(name).trim() {
            "npm" => Some(Ecosystem::Npm),
            "crates.io" => Some(Ecosystem::CratesIo),
            _ => None,
        }
    }
}

impl fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.osv_name())
    }
}

/// One pinned dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub ecosystem: Ecosystem,
    pub name: String,
    pub version: String,
}

/// The lockfiles this can read, relative to a project root.
pub const SUPPORTED: &[&str] = &["package-lock.json", "Cargo.lock"];

/// Read every lockfile at `root` that we understand. Missing or unreadable
/// files simply contribute nothing; a project with no lockfile we can parse
/// yields an empty list, which the caller reports as "not audited" rather
/// than as "clean".
pub fn read(root: &Path) -> Vec<Package> {
    let mut packages = Vec::new();
    let npm = root.join("package-lock.json");
    if npm.is_file() {
        packages.extend(parse_npm(&fs::read_to_string(&npm).unwrap_or_default()));
    }
    let cargo = root.join("Cargo.lock");
    if cargo.is_file() {
        packages.extend(parse_cargo(&fs::read_to_string(&cargo).unwrap_or_default()));
    }
    packages.sort_by(|a, b| (a.name.as_str(), a.version.as_str()).cmp(&(&b.name, &b.version)));
    packages.dedup();
    packages
}

/// `package-lock.json`, both the modern `packages` map (npm 7+) and the older
/// nested `dependencies` tree.
pub fn parse_npm(text: &str) -> Vec<Package> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    // npm 7+ : a flat map keyed by install path.
    if let Some(map) = root.get("packages").and_then(|v| v.as_object()) {
        for (path, entry) in map {
            // The empty key is the project itself, not a dependency. Entries
            // marked `link` point at a workspace member already scanned in
            // its own right.
            if path.is_empty() || entry.get("link").and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }
            // "node_modules/a/node_modules/b" names package b.
            let Some(name) = path
                .rsplit("node_modules/")
                .next()
                .filter(|n| !n.is_empty())
            else {
                continue;
            };
            let Some(version) = entry.get("version").and_then(|v| v.as_str()) else {
                continue;
            };
            out.push(Package {
                ecosystem: Ecosystem::Npm,
                name: name.to_string(),
                version: version.to_string(),
            });
        }
    }

    // npm 6 and earlier: a recursive tree.
    if out.is_empty() {
        if let Some(deps) = root.get("dependencies").and_then(|v| v.as_object()) {
            collect_npm_v1(deps, &mut out);
        }
    }
    out
}

fn collect_npm_v1(deps: &serde_json::Map<String, serde_json::Value>, out: &mut Vec<Package>) {
    for (name, entry) in deps {
        if let Some(version) = entry.get("version").and_then(|v| v.as_str()) {
            out.push(Package {
                ecosystem: Ecosystem::Npm,
                name: name.clone(),
                version: version.to_string(),
            });
        }
        if let Some(nested) = entry.get("dependencies").and_then(|v| v.as_object()) {
            collect_npm_v1(nested, out);
        }
    }
}

/// `Cargo.lock`: a sequence of `[[package]]` tables. Parsed by hand because
/// the shape is fixed and a TOML dependency would be a lot of crate for two
/// fields.
pub fn parse_cargo(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut in_package = false;
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;

    let mut flush = |name: &mut Option<String>, version: &mut Option<String>| {
        if let (Some(n), Some(v)) = (name.take(), version.take()) {
            out.push(Package {
                ecosystem: Ecosystem::CratesIo,
                name: n,
                version: v,
            });
        }
    };

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut name, &mut version);
            in_package = line == "[[package]]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(value) = field(line, "name") {
            name = Some(value);
        } else if let Some(value) = field(line, "version") {
            version = Some(value);
        }
    }
    flush(&mut name, &mut version);
    out
}

/// `key = "value"` with the quotes removed, or `None` for any other line.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let value = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(packages: &[Package]) -> Vec<(&str, &str)> {
        packages
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect()
    }

    #[test]
    fn reads_a_modern_npm_lockfile() {
        let text = r#"{
          "lockfileVersion": 3,
          "packages": {
            "": { "name": "web", "version": "1.0.0" },
            "node_modules/left-pad": { "version": "1.3.0" },
            "node_modules/lodash": { "version": "4.17.20" },
            "node_modules/a/node_modules/lodash": { "version": "3.10.1" },
            "packages/ui": { "link": true, "resolved": "packages/ui" }
          }
        }"#;
        let got = parse_npm(text);
        let mut got = names(&got);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("left-pad", "1.3.0"),
                ("lodash", "3.10.1"),
                ("lodash", "4.17.20"),
            ],
            "the project itself and workspace links are not dependencies"
        );
    }

    #[test]
    fn reads_a_scoped_package_name() {
        let text = r#"{"packages":{"node_modules/@scope/pkg":{"version":"2.0.0"}}}"#;
        assert_eq!(names(&parse_npm(text)), vec![("@scope/pkg", "2.0.0")]);
    }

    #[test]
    fn falls_back_to_the_old_npm_tree() {
        let text = r#"{
          "lockfileVersion": 1,
          "dependencies": {
            "left-pad": { "version": "1.3.0" },
            "outer": { "version": "2.0.0", "dependencies": { "inner": { "version": "0.1.0" } } }
          }
        }"#;
        let parsed = parse_npm(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("inner", "0.1.0"),
                ("left-pad", "1.3.0"),
                ("outer", "2.0.0")
            ]
        );
    }

    #[test]
    fn reads_a_cargo_lockfile() {
        let text = r#"
version = 4

[[package]]
name = "anyhow"
version = "1.0.86"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "my-app"
version = "0.1.0"
dependencies = [
 "anyhow",
]
"#;
        assert_eq!(
            names(&parse_cargo(text)),
            vec![("anyhow", "1.0.86"), ("my-app", "0.1.0")]
        );
    }

    /// Nonsense in must not produce confident nonsense out.
    #[test]
    fn unreadable_input_yields_nothing() {
        assert!(parse_npm("not json").is_empty());
        assert!(parse_npm("{}").is_empty());
        assert!(parse_cargo("").is_empty());
        assert!(
            parse_cargo("[[package]]\nname = \"x\"\n").is_empty(),
            "a package with no version is not reported"
        );
    }

    #[test]
    fn reads_both_lockfiles_in_one_project_and_deduplicates() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package-lock.json"),
            r#"{"packages":{"node_modules/x":{"version":"1.0.0"}}}"#,
        )
        .unwrap();
        fs::write(
            tmp.path().join("Cargo.lock"),
            "[[package]]\nname = \"y\"\nversion = \"2.0.0\"\n",
        )
        .unwrap();
        let got = read(tmp.path());
        assert_eq!(got.len(), 2);
        assert!(got
            .iter()
            .any(|p| p.ecosystem == Ecosystem::Npm && p.name == "x"));
        assert!(got
            .iter()
            .any(|p| p.ecosystem == Ecosystem::CratesIo && p.name == "y"));
    }

    /// Exported findings are read by people and by other tools, so the
    /// ecosystem has to read as the registry names it.
    #[test]
    fn an_ecosystem_serialises_as_the_registry_names_it() {
        let p = Package {
            ecosystem: Ecosystem::CratesIo,
            name: "time".into(),
            version: "0.1.44".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""ecosystem":"crates.io""#), "got {json}");
        assert_eq!(serde_json::from_str::<Package>(&json).unwrap(), p);
        assert_eq!(Ecosystem::CratesIo.to_string(), "crates.io");
    }

    #[test]
    fn a_project_with_no_lockfile_reads_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read(tmp.path()).is_empty());
    }
}
