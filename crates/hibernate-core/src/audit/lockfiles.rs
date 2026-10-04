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
pub const SUPPORTED: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.lock",
];

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
    let pnpm = root.join("pnpm-lock.yaml");
    if pnpm.is_file() {
        packages.extend(parse_pnpm(&fs::read_to_string(&pnpm).unwrap_or_default()));
    }
    let yarn = root.join("yarn.lock");
    if yarn.is_file() {
        packages.extend(parse_yarn(&fs::read_to_string(&yarn).unwrap_or_default()));
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

/// Split `name@rest` at the first `@` that is not the scope marker, so
/// `@scope/pkg@1.0.0` names `@scope/pkg`. Taking the *first* such `@` rather
/// than the last matters for Yarn's patch protocol, whose specifier embeds a
/// second package reference after the first.
fn split_name(spec: &str) -> Option<(&str, &str)> {
    let at = spec.get(1..)?.find('@')? + 1;
    let (name, rest) = spec.split_at(at);
    Some((name, &rest[1..]))
}

/// Only report versions that look like a registry release. Git URLs,
/// tarballs and local paths have no version an advisory could name, and
/// reporting them would only produce noise.
fn looks_like_release(version: &str) -> bool {
    version.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// `pnpm-lock.yaml`. Read line by line rather than as YAML: only the keys of
/// the top-level `packages:` map matter, and each one already spells out
/// name and version. Three generations of key are handled —
///
/// - v9 `lodash@4.17.20` and v6 `/lodash@4.17.20`, both possibly followed
///   by a peer-dependency suffix in parentheses;
/// - v5 `/lodash/4.17.20`, whose peer suffix starts with `_`.
pub fn parse_pnpm(text: &str) -> Vec<Package> {
    let legacy = text
        .lines()
        .find_map(|l| l.strip_prefix("lockfileVersion:"))
        .map(|v| v.trim().trim_matches(|c| c == '\'' || c == '"'))
        .is_some_and(|v| v.starts_with('5') || v.starts_with('4') || v.starts_with('3'));

    let mut out = Vec::new();
    let mut in_packages = false;
    for line in text.lines() {
        // A new top-level key ends the section we are reading.
        if !line.starts_with(' ') && !line.is_empty() {
            in_packages = line.trim_end() == "packages:";
            continue;
        }
        if !in_packages {
            continue;
        }
        // Package keys sit at exactly two spaces of indent.
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        let Some(key) = rest.trim_end().strip_suffix(':') else {
            continue;
        };
        let key = key.trim_matches(|c| c == '\'' || c == '"');
        let key = key.strip_prefix('/').unwrap_or(key);

        let parsed = if legacy {
            key.rsplit_once('/').map(|(name, version)| {
                // Versions never contain `_`, so it can only start a peer
                // suffix. Names can, which is why this is split second.
                (name, version.split('_').next().unwrap_or(version))
            })
        } else {
            let key = key.split('(').next().unwrap_or(key);
            split_name(key)
        };
        let Some((name, version)) = parsed else {
            continue;
        };
        if name.is_empty() || !looks_like_release(version) {
            continue;
        }
        out.push(Package {
            ecosystem: Ecosystem::Npm,
            name: name.to_string(),
            version: version.to_string(),
        });
    }
    out
}

/// `yarn.lock`, both Yarn 1 (`version "1.2.3"`) and Berry (`version: 1.2.3`).
/// Each entry is a header listing the specifiers that resolved to it,
/// followed by indented fields; the name comes from the header and the
/// version from its field.
pub fn parse_yarn(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;

    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            // `"a@^1", "a@^1.2":` — every specifier names the same package,
            // so the first is enough.
            name = line
                .trim_end()
                .strip_suffix(':')
                .and_then(|h| h.split(',').next())
                .map(|h| h.trim().trim_matches('"'))
                .filter(|spec| {
                    // Workspace members and local paths are this project's
                    // own code, not something a registry advisory covers.
                    !spec.starts_with("__metadata")
                        && !["@workspace:", "@link:", "@portal:", "@file:"]
                            .iter()
                            .any(|p| spec.contains(p))
                })
                .and_then(split_name)
                .map(|(n, _)| n.to_string());
            continue;
        }
        let Some(current) = &name else {
            continue;
        };
        let field = line.trim();
        let version = field
            .strip_prefix("version:")
            .or_else(|| field.strip_prefix("version "))
            .map(|v| v.trim().trim_matches('"'));
        if let Some(version) = version {
            if looks_like_release(version) {
                out.push(Package {
                    ecosystem: Ecosystem::Npm,
                    name: current.clone(),
                    version: version.to_string(),
                });
            }
            // One version per entry; ignore any later `version` text.
            name = None;
        }
    }
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

    #[test]
    fn reads_a_pnpm_v9_lockfile() {
        let text = "lockfileVersion: '9.0'

importers:

  .:
    dependencies:
      lodash:
        specifier: ^4.17.20
        version: 4.17.20

packages:

  '@scope/pkg@2.0.0':
    resolution: {integrity: sha512-x}

  lodash@4.17.20:
    resolution: {integrity: sha512-y}

  react-dom@18.2.0(react@18.2.0):
    resolution: {integrity: sha512-z}

  from-git@https://codeload.github.com/a/b/tar.gz/abc:
    resolution: {tarball: https://example}

snapshots:

  lodash@4.17.20: {}
";
        let parsed = parse_pnpm(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("@scope/pkg", "2.0.0"),
                ("lodash", "4.17.20"),
                ("react-dom", "18.2.0"),
            ],
            "peer suffixes are stripped, git sources skipped, importers and snapshots ignored"
        );
    }

    #[test]
    fn reads_a_pnpm_v6_lockfile() {
        let text = "lockfileVersion: '6.0'

packages:

  /lodash@4.17.20:
    resolution: {integrity: sha512-y}

  /@babel/core@7.24.0(supports-color@8.1.1):
    resolution: {integrity: sha512-z}
";
        let parsed = parse_pnpm(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(got, vec![("@babel/core", "7.24.0"), ("lodash", "4.17.20")]);
    }

    #[test]
    fn reads_a_pnpm_v5_lockfile() {
        let text = "lockfileVersion: 5.4

packages:

  /lodash/4.17.20:
    resolution: {integrity: sha512-y}

  /@babel/core/7.24.0_supports-color@8.1.1:
    resolution: {integrity: sha512-z}

  /lodash_utils/1.0.0:
    resolution: {integrity: sha512-w}
";
        let parsed = parse_pnpm(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("@babel/core", "7.24.0"),
                ("lodash", "4.17.20"),
                ("lodash_utils", "1.0.0"),
            ],
            "the v5 peer suffix is cut at `_`, but an underscore in a name survives"
        );
    }

    #[test]
    fn reads_a_yarn_v1_lockfile() {
        let text = r#"# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1


"@babel/code-frame@^7.0.0", "@babel/code-frame@^7.10.4":
  version "7.10.4"
  resolved "https://registry.yarnpkg.com/@babel/code-frame/-/code-frame-7.10.4.tgz"
  dependencies:
    "@babel/highlight" "^7.10.4"

lodash@^4.17.20:
  version "4.17.20"
  resolved "https://registry.yarnpkg.com/lodash/-/lodash-4.17.20.tgz"
"#;
        let parsed = parse_yarn(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(
            got,
            vec![("@babel/code-frame", "7.10.4"), ("lodash", "4.17.20")],
            "a dependency's own range under `dependencies:` is not mistaken for its version"
        );
    }

    #[test]
    fn reads_a_yarn_berry_lockfile() {
        let text = r#"__metadata:
  version: 8
  cacheKey: 10c0

"@scope/pkg@npm:^2.0.0":
  version: 2.0.0
  resolution: "@scope/pkg@npm:2.0.0"

"lodash@npm:^4.17.20, lodash@npm:^4.17.21":
  version: 4.17.21
  resolution: "lodash@npm:4.17.21"

"resolve@patch:resolve@npm%3A^1.22.0#~builtin<compat/resolve>":
  version: 1.22.8
  resolution: "resolve@patch:resolve@npm%3A1.22.8"

"web@workspace:.":
  version: 0.0.0-use.local
  resolution: "web@workspace:."
"#;
        let parsed = parse_yarn(text);
        let mut got = names(&parsed);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("@scope/pkg", "2.0.0"),
                ("lodash", "4.17.21"),
                ("resolve", "1.22.8"),
            ],
            "metadata and the workspace's own entry are skipped; a patched package keeps its real name"
        );
    }

    /// Nonsense in must not produce confident nonsense out.
    #[test]
    fn unreadable_input_yields_nothing() {
        assert!(parse_npm("not json").is_empty());
        assert!(parse_npm("{}").is_empty());
        assert!(parse_cargo("").is_empty());
        assert!(parse_pnpm("").is_empty());
        assert!(parse_pnpm("packages:\n  not a key\n").is_empty());
        assert!(parse_yarn("").is_empty());
        assert!(
            parse_yarn("lodash@^1:\n  resolved \"x\"\n").is_empty(),
            "an entry with no version is not reported"
        );
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
