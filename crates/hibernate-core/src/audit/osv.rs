//! Matching pinned versions against an OSV advisory database.
//!
//! [OSV](https://ossf.github.io/osv-schema/) is one schema across npm,
//! crates.io, PyPI, Go and the rest, which is why it is worth reading
//! directly rather than shelling out to a different auditor per ecosystem.
//!
//! The database is supplied as a local path — a directory of advisory JSON
//! files, a single file holding an array, or one advisory per file. Nothing
//! here reaches the network: how the data arrives on disk is a separate
//! decision, and keeping it separate is what lets the engine stay offline.

use super::lockfiles::{Ecosystem, Package};
use super::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// How bad, as the advisory itself describes it. Deliberately coarse: the
/// number a CVSS vector encodes is less useful here than "should I care".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Unknown,
    Low,
    Moderate,
    High,
    Critical,
}

impl Severity {
    fn parse(text: &str) -> Severity {
        match text.trim().to_ascii_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "HIGH" => Severity::High,
            "MODERATE" | "MEDIUM" => Severity::Moderate,
            "LOW" => Severity::Low,
            _ => Severity::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Moderate => "moderate",
            Severity::Low => "low",
            Severity::Unknown => "unknown",
        }
    }
}

/// One vulnerable dependency in one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub package: Package,
    /// Advisory id, e.g. `GHSA-...` or `RUSTSEC-...`.
    pub id: String,
    pub summary: String,
    pub severity: Severity,
    /// The first version that is not affected, when the advisory names one.
    pub fixed: Option<String>,
}

/// An advisory as OSV publishes it. Only the fields needed to match and to
/// explain a match are read; the rest of the schema is ignored on purpose so
/// a new optional field cannot break parsing.
#[derive(Debug, Deserialize)]
struct Advisory {
    #[serde(default)]
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    details: String,
    #[serde(default)]
    withdrawn: Option<String>,
    #[serde(default)]
    database_specific: Option<serde_json::Value>,
    #[serde(default)]
    affected: Vec<Affected>,
}

#[derive(Debug, Deserialize)]
struct Affected {
    #[serde(default)]
    package: AffectedPackage,
    #[serde(default)]
    ranges: Vec<Range>,
    /// Explicitly enumerated affected versions.
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    database_specific: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct AffectedPackage {
    #[serde(default)]
    ecosystem: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Range {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    events: Vec<HashMap<String, String>>,
}

/// Advisories indexed by the package they affect, so a lookup is a hash hit
/// rather than a scan of the whole database.
#[derive(Debug, Default)]
pub struct AdvisoryDb {
    by_package: HashMap<(Ecosystem, String), Vec<Entry>>,
    advisories: usize,
}

#[derive(Debug)]
struct Entry {
    id: String,
    summary: String,
    severity: Severity,
    ranges: Vec<Range>,
    versions: Vec<String>,
}

impl AdvisoryDb {
    /// Number of advisories that were loaded and understood.
    pub fn len(&self) -> usize {
        self.advisories
    }

    pub fn is_empty(&self) -> bool {
        self.advisories == 0
    }

    /// Load from a file or a directory of files. A directory is walked
    /// recursively, which is the shape OSV's own exports unpack to. Files
    /// that are not advisories are skipped quietly; a database half full of
    /// unrelated JSON is still a usable database.
    pub fn load(path: &Path) -> io::Result<AdvisoryDb> {
        let mut db = AdvisoryDb::default();
        if path.is_dir() {
            let mut stack = vec![path.to_path_buf()];
            while let Some(dir) = stack.pop() {
                for entry in fs::read_dir(&dir)?.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|e| e == "json") {
                        db.add_text(&fs::read_to_string(&p).unwrap_or_default());
                    }
                }
            }
        } else {
            db.add_text(&fs::read_to_string(path)?);
        }
        Ok(db)
    }

    /// Accept either a single advisory object or an array of them.
    fn add_text(&mut self, text: &str) {
        if let Ok(list) = serde_json::from_str::<Vec<Advisory>>(text) {
            for a in list {
                self.add(a);
            }
        } else if let Ok(one) = serde_json::from_str::<Advisory>(text) {
            self.add(one);
        }
    }

    fn add(&mut self, advisory: Advisory) {
        // A withdrawn advisory was retracted by its publisher. Reporting one
        // is exactly the false alarm that makes people stop reading.
        if advisory.withdrawn.is_some() || advisory.id.is_empty() {
            return;
        }
        let summary = if advisory.summary.is_empty() {
            advisory.details.lines().next().unwrap_or("").to_string()
        } else {
            advisory.summary.clone()
        };
        let top_severity = severity_of(advisory.database_specific.as_ref());

        let mut indexed = false;
        for affected in advisory.affected {
            let Some(ecosystem) = Ecosystem::from_osv(&affected.package.ecosystem) else {
                continue;
            };
            if affected.package.name.is_empty() {
                continue;
            }
            let severity = match severity_of(affected.database_specific.as_ref()) {
                Severity::Unknown => top_severity,
                s => s,
            };
            self.by_package
                .entry((ecosystem, affected.package.name.clone()))
                .or_default()
                .push(Entry {
                    id: advisory.id.clone(),
                    summary: summary.clone(),
                    severity,
                    ranges: affected.ranges,
                    versions: affected.versions,
                });
            indexed = true;
        }
        if indexed {
            self.advisories += 1;
        }
    }

    /// Every advisory that covers this exact version.
    pub fn check(&self, package: &Package) -> Vec<Finding> {
        let Some(entries) = self
            .by_package
            .get(&(package.ecosystem, package.name.clone()))
        else {
            return Vec::new();
        };
        let parsed = Version::parse(&package.version);
        let mut out = Vec::new();
        for entry in entries {
            // An explicit list is the most precise statement an advisory can
            // make, so it is checked first and without needing to parse.
            let listed = entry.versions.iter().any(|v| v == &package.version);
            let in_range = parsed
                .as_ref()
                .is_some_and(|v| entry.ranges.iter().any(|r| range_covers(r, v)));
            if !listed && !in_range {
                continue;
            }
            if out.iter().any(|f: &Finding| f.id == entry.id) {
                continue;
            }
            out.push(Finding {
                package: package.clone(),
                id: entry.id.clone(),
                summary: entry.summary.clone(),
                severity: entry.severity,
                fixed: entry
                    .ranges
                    .iter()
                    .filter_map(|r| first_fix(r, parsed.as_ref()))
                    .min()
                    .map(|v| v.0),
            });
        }
        out.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
        out
    }
}

/// `database_specific.severity`, which is where GitHub and RustSec put the
/// word most people actually act on.
fn severity_of(value: Option<&serde_json::Value>) -> Severity {
    value
        .and_then(|v| v.get("severity"))
        .and_then(|v| v.as_str())
        .map(Severity::parse)
        .unwrap_or(Severity::Unknown)
}

/// Whether a range's events place `version` inside it.
///
/// Events are ordered: an `introduced` opens a window and the next `fixed`
/// or `last_affected` closes it. A version is affected when it sits in an
/// open window. Ranges we do not understand — `GIT`, say — never match,
/// because a guess here is a false alarm.
fn range_covers(range: &Range, version: &Version) -> bool {
    if range.kind != "SEMVER" && range.kind != "ECOSYSTEM" {
        return false;
    }
    let mut open: Option<Option<Version>> = None;
    for event in &range.events {
        if let Some(introduced) = event.get("introduced") {
            // "0" is OSV's way of saying "from the beginning".
            open = Some(if introduced == "0" {
                None
            } else {
                match Version::parse(introduced) {
                    Some(v) => Some(v),
                    // An unreadable lower bound would make the window
                    // unbounded, so close it instead of over-reporting.
                    None => {
                        open = None;
                        continue;
                    }
                }
            });
        } else if let Some(fixed) = event.get("fixed") {
            if let Some(lower) = &open {
                let above = lower.as_ref().is_none_or(|l| version >= l);
                if above && Version::parse(fixed).is_some_and(|f| *version < f) {
                    return true;
                }
            }
            open = None;
        } else if let Some(last) = event.get("last_affected") {
            if let Some(lower) = &open {
                let above = lower.as_ref().is_none_or(|l| version >= l);
                if above && Version::parse(last).is_some_and(|l| *version <= l) {
                    return true;
                }
            }
            open = None;
        }
    }
    // A window left open runs to the latest release.
    match open {
        Some(lower) => lower.as_ref().is_none_or(|l| version >= l),
        None => false,
    }
}

/// The lowest `fixed` version in a range that is above the affected one, so
/// the report can say what to upgrade to.
fn first_fix(range: &Range, version: Option<&Version>) -> Option<(String, Version)> {
    let mut best: Option<(String, Version)> = None;
    for event in &range.events {
        let Some(fixed) = event.get("fixed") else {
            continue;
        };
        let Some(parsed) = Version::parse(fixed) else {
            continue;
        };
        if version.is_some_and(|v| parsed <= *v) {
            continue;
        }
        if best.as_ref().is_none_or(|(_, b)| parsed < *b) {
            best = Some((fixed.clone(), parsed));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npm(name: &str, version: &str) -> Package {
        Package {
            ecosystem: Ecosystem::Npm,
            name: name.into(),
            version: version.into(),
        }
    }

    fn db_from(json: &str) -> AdvisoryDb {
        let mut db = AdvisoryDb::default();
        db.add_text(json);
        db
    }

    const LODASH: &str = r#"{
      "id": "GHSA-lodash",
      "summary": "Prototype pollution in lodash",
      "database_specific": { "severity": "HIGH" },
      "affected": [{
        "package": { "ecosystem": "npm", "name": "lodash" },
        "ranges": [{ "type": "SEMVER", "events": [
          { "introduced": "0" }, { "fixed": "4.17.21" }
        ]}]
      }]
    }"#;

    #[test]
    fn flags_a_version_below_the_fix_and_clears_the_fix_itself() {
        let db = db_from(LODASH);
        let hits = db.check(&npm("lodash", "4.17.20"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "GHSA-lodash");
        assert_eq!(hits[0].severity, Severity::High);
        assert_eq!(hits[0].fixed.as_deref(), Some("4.17.21"));

        assert!(
            db.check(&npm("lodash", "4.17.21")).is_empty(),
            "the fix is not affected"
        );
        assert!(db.check(&npm("lodash", "5.0.0")).is_empty());
        assert!(
            db.check(&npm("left-pad", "1.0.0")).is_empty(),
            "a different package"
        );
    }

    #[test]
    fn respects_the_lower_bound_of_a_window() {
        let db = db_from(
            r#"{
          "id": "GHSA-window",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "ranges": [{ "type": "SEMVER", "events": [
              { "introduced": "2.0.0" }, { "fixed": "2.5.0" }
            ]}]
          }]
        }"#,
        );
        assert!(
            db.check(&npm("pkg", "1.9.0")).is_empty(),
            "below the window"
        );
        assert_eq!(
            db.check(&npm("pkg", "2.0.0")).len(),
            1,
            "the lower bound is affected"
        );
        assert_eq!(db.check(&npm("pkg", "2.4.9")).len(), 1);
        assert!(db.check(&npm("pkg", "2.5.0")).is_empty(), "the fix is not");
    }

    #[test]
    fn an_unclosed_window_runs_to_the_latest_release() {
        let db = db_from(
            r#"{
          "id": "GHSA-open",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "ranges": [{ "type": "SEMVER", "events": [{ "introduced": "3.0.0" }]}]
          }]
        }"#,
        );
        assert!(db.check(&npm("pkg", "2.9.9")).is_empty());
        assert_eq!(db.check(&npm("pkg", "99.0.0")).len(), 1);
        assert_eq!(
            db.check(&npm("pkg", "99.0.0"))[0].fixed,
            None,
            "nothing to upgrade to yet"
        );
    }

    #[test]
    fn honours_last_affected_as_an_inclusive_bound() {
        let db = db_from(
            r#"{
          "id": "GHSA-last",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "ranges": [{ "type": "ECOSYSTEM", "events": [
              { "introduced": "0" }, { "last_affected": "1.2.3" }
            ]}]
          }]
        }"#,
        );
        assert_eq!(
            db.check(&npm("pkg", "1.2.3")).len(),
            1,
            "last_affected is inclusive"
        );
        assert!(db.check(&npm("pkg", "1.2.4")).is_empty());
    }

    #[test]
    fn matches_an_explicitly_listed_version_without_parsing_it() {
        let db = db_from(
            r#"{
          "id": "GHSA-listed",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "versions": ["not-semver-at-all"]
          }]
        }"#,
        );
        assert_eq!(db.check(&npm("pkg", "not-semver-at-all")).len(), 1);
        assert!(db.check(&npm("pkg", "1.0.0")).is_empty());
    }

    /// A retracted advisory is exactly the false alarm that trains people to
    /// ignore the feature.
    #[test]
    fn a_withdrawn_advisory_is_never_reported() {
        let db = db_from(
            r#"{
          "id": "GHSA-gone",
          "withdrawn": "2024-01-01T00:00:00Z",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "ranges": [{ "type": "SEMVER", "events": [{ "introduced": "0" }]}]
          }]
        }"#,
        );
        assert!(db.is_empty());
        assert!(db.check(&npm("pkg", "1.0.0")).is_empty());
    }

    #[test]
    fn a_range_type_we_do_not_understand_never_matches() {
        let db = db_from(
            r#"{
          "id": "GHSA-git",
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "ranges": [{ "type": "GIT", "events": [
              { "introduced": "abc123" }, { "fixed": "def456" }
            ]}]
          }]
        }"#,
        );
        assert!(db.check(&npm("pkg", "1.0.0")).is_empty());
    }

    #[test]
    fn ecosystems_we_do_not_read_are_ignored() {
        let db = db_from(
            r#"{
          "id": "PYSEC-1",
          "affected": [{
            "package": { "ecosystem": "PyPI", "name": "requests" },
            "ranges": [{ "type": "ECOSYSTEM", "events": [{ "introduced": "0" }]}]
          }]
        }"#,
        );
        assert!(
            db.is_empty(),
            "an advisory with nothing we can match is not counted"
        );
    }

    #[test]
    fn severity_falls_back_from_the_affected_entry_to_the_advisory() {
        let db = db_from(
            r#"{
          "id": "GHSA-sev",
          "database_specific": { "severity": "LOW" },
          "affected": [{
            "package": { "ecosystem": "npm", "name": "pkg" },
            "database_specific": { "severity": "CRITICAL" },
            "ranges": [{ "type": "SEMVER", "events": [{ "introduced": "0" }]}]
          }]
        }"#,
        );
        assert_eq!(
            db.check(&npm("pkg", "1.0.0"))[0].severity,
            Severity::Critical
        );
    }

    #[test]
    fn findings_come_back_worst_first() {
        let db = db_from(&format!(
            "[{},{}]",
            LODASH,
            r#"{
              "id": "GHSA-critical",
              "database_specific": { "severity": "CRITICAL" },
              "affected": [{
                "package": { "ecosystem": "npm", "name": "lodash" },
                "ranges": [{ "type": "SEMVER", "events": [{ "introduced": "0" }]}]
              }]
            }"#
        ));
        assert_eq!(db.len(), 2);
        let hits = db.check(&npm("lodash", "4.17.20"));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].severity, Severity::Critical);
        assert_eq!(hits[1].severity, Severity::High);
    }

    #[test]
    fn loads_a_directory_of_advisories() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("npm/lodash")).unwrap();
        fs::write(tmp.path().join("npm/lodash/GHSA-lodash.json"), LODASH).unwrap();
        fs::write(tmp.path().join("npm/README.md"), "not an advisory").unwrap();
        let db = AdvisoryDb::load(tmp.path()).unwrap();
        assert_eq!(db.len(), 1);
        assert_eq!(db.check(&npm("lodash", "4.17.20")).len(), 1);
    }
}
