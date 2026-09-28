//! Which dormant projects carry known-vulnerable dependencies.
//!
//! The blind spot this fills: Dependabot and Renovate only watch what you
//! push to a forge. A local experiment, an archived clone, a client project
//! from last year — nobody is watching those, and they are exactly the ones
//! you have stopped thinking about. This app already holds an inventory of
//! them, which is the expensive half of the problem.
//!
//! It works on hibernated projects because lockfiles are protected and never
//! removed: the audit needs the lockfile, not `node_modules/`.
//!
//! **Nothing here touches the network.** The advisory data is read from a
//! local path. How it arrives — a download, a checked-out copy of OSV's
//! export, a corporate mirror — is deliberately somebody else's decision,
//! and keeping it out of the engine is what lets the safety claims in the
//! README stay checkable by reading the dependency list.

pub mod lockfiles;
pub mod osv;
pub mod version;

pub use lockfiles::{Ecosystem, Package};
pub use osv::{AdvisoryDb, Finding, Severity};

use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What an audit found in one project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAudit {
    pub project_id: String,
    pub name: String,
    pub path: PathBuf,
    /// How many pinned dependencies were read. Zero means no lockfile we
    /// understand, which is reported as *not audited* rather than as clean —
    /// the difference matters when the answer is a security claim.
    pub packages: usize,
    pub findings: Vec<Finding>,
}

impl ProjectAudit {
    /// True when there was nothing to check, so the absence of findings says
    /// nothing either way.
    pub fn not_audited(&self) -> bool {
        self.packages == 0
    }

    pub fn worst(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }
}

/// An audit across a set of projects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub projects: Vec<ProjectAudit>,
    /// Advisories loaded, so a report can say what it was checked against.
    pub advisories: usize,
}

impl AuditReport {
    pub fn total_findings(&self) -> usize {
        self.projects.iter().map(|p| p.findings.len()).sum()
    }

    /// Projects with at least one finding, worst first, so a caller can show
    /// the ones that matter without re-sorting.
    pub fn affected(&self) -> Vec<&ProjectAudit> {
        let mut out: Vec<&ProjectAudit> = self
            .projects
            .iter()
            .filter(|p| !p.findings.is_empty())
            .collect();
        out.sort_by(|a, b| {
            b.worst()
                .cmp(&a.worst())
                .then_with(|| b.findings.len().cmp(&a.findings.len()))
                .then_with(|| a.name.cmp(&b.name))
        });
        out
    }
}

/// Audit one project directory against the database.
pub fn audit_path(path: &Path, db: &AdvisoryDb) -> (usize, Vec<Finding>) {
    let packages = lockfiles::read(path);
    let mut findings = Vec::new();
    for package in &packages {
        findings.extend(db.check(package));
    }
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.package.name.cmp(&b.package.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    (packages.len(), findings)
}

/// Audit every project the last scan found.
pub fn audit(projects: &[Project], db: &AdvisoryDb) -> AuditReport {
    let mut report = AuditReport {
        advisories: db.len(),
        ..AuditReport::default()
    };
    for project in projects {
        let (packages, findings) = audit_path(&project.path, db);
        report.projects.push(ProjectAudit {
            project_id: project.id.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            packages,
            findings,
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ADVISORIES: &str = r#"[
      {
        "id": "GHSA-lodash",
        "summary": "Prototype pollution in lodash",
        "database_specific": { "severity": "HIGH" },
        "affected": [{
          "package": { "ecosystem": "npm", "name": "lodash" },
          "ranges": [{ "type": "SEMVER", "events": [
            { "introduced": "0" }, { "fixed": "4.17.21" }
          ]}]
        }]
      },
      {
        "id": "RUSTSEC-time",
        "summary": "Segfault in time",
        "database_specific": { "severity": "CRITICAL" },
        "affected": [{
          "package": { "ecosystem": "crates.io", "name": "time" },
          "ranges": [{ "type": "SEMVER", "events": [
            { "introduced": "0.1.0" }, { "fixed": "0.2.23" }
          ]}]
        }]
      }
    ]"#;

    fn db() -> AdvisoryDb {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("advisories.json");
        fs::write(&file, ADVISORIES).unwrap();
        AdvisoryDb::load(&file).unwrap()
    }

    /// The point of the whole feature: a project whose generated folders are
    /// long gone is still fully auditable, because the lockfile is protected
    /// and stays behind.
    #[test]
    fn a_hibernated_project_can_still_be_audited() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("web");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("package.json"), r#"{"name":"web"}"#).unwrap();
        fs::write(
            project.join("package-lock.json"),
            r#"{"packages":{"node_modules/lodash":{"version":"4.17.20"}}}"#,
        )
        .unwrap();
        // No node_modules/ at all: this project has been hibernated.
        assert!(!project.join("node_modules").exists());

        let (packages, findings) = audit_path(&project, &db());
        assert_eq!(packages, 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "GHSA-lodash");
        assert_eq!(findings[0].fixed.as_deref(), Some("4.17.21"));
    }

    #[test]
    fn no_lockfile_reads_as_not_audited_rather_than_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let (packages, findings) = audit_path(tmp.path(), &db());
        assert_eq!(packages, 0);
        assert!(findings.is_empty());

        let audit = ProjectAudit {
            project_id: "x".into(),
            name: "x".into(),
            path: tmp.path().into(),
            packages,
            findings,
        };
        assert!(
            audit.not_audited(),
            "an empty result with nothing read must not be presented as a clean bill of health"
        );
    }

    #[test]
    fn a_clean_project_is_distinguishable_from_an_unaudited_one() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package-lock.json"),
            r#"{"packages":{"node_modules/lodash":{"version":"4.17.21"}}}"#,
        )
        .unwrap();
        let (packages, findings) = audit_path(tmp.path(), &db());
        assert_eq!(packages, 1);
        assert!(findings.is_empty());

        let audit = ProjectAudit {
            project_id: "x".into(),
            name: "x".into(),
            path: tmp.path().into(),
            packages,
            findings,
        };
        assert!(
            !audit.not_audited(),
            "a lockfile was read, so the all-clear is real"
        );
        assert_eq!(audit.worst(), None);
    }

    #[test]
    fn covers_more_than_one_ecosystem_in_one_project() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package-lock.json"),
            r#"{"packages":{"node_modules/lodash":{"version":"4.17.20"}}}"#,
        )
        .unwrap();
        fs::write(
            tmp.path().join("Cargo.lock"),
            "[[package]]\nname = \"time\"\nversion = \"0.1.44\"\n",
        )
        .unwrap();
        let (packages, findings) = audit_path(tmp.path(), &db());
        assert_eq!(packages, 2);
        assert_eq!(findings.len(), 2);
        // Worst first, so the thing to act on is at the top.
        assert_eq!(findings[0].severity, Severity::Critical);
        assert_eq!(findings[0].id, "RUSTSEC-time");
    }

    #[test]
    fn a_report_ranks_affected_projects_worst_first() {
        let low = ProjectAudit {
            project_id: "a".into(),
            name: "a".into(),
            path: PathBuf::from("/a"),
            packages: 5,
            findings: vec![Finding {
                package: Package {
                    ecosystem: Ecosystem::Npm,
                    name: "x".into(),
                    version: "1".into(),
                },
                id: "L".into(),
                summary: String::new(),
                severity: Severity::Low,
                fixed: None,
            }],
        };
        let mut high = low.clone();
        high.project_id = "b".into();
        high.name = "b".into();
        high.findings[0].severity = Severity::Critical;
        let clean = ProjectAudit {
            findings: Vec::new(),
            ..low.clone()
        };

        let report = AuditReport {
            projects: vec![low, clean, high],
            advisories: 2,
        };
        let affected = report.affected();
        assert_eq!(
            affected.len(),
            2,
            "the clean project is not listed as affected"
        );
        assert_eq!(affected[0].name, "b");
        assert_eq!(report.total_findings(), 2);
    }
}
