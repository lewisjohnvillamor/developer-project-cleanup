//! Export the project table for spreadsheets and scripts.

use crate::model::{Project, Safety};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn iso(t: Option<DateTime<Utc>>) -> String {
    t.map(|t| t.to_rfc3339()).unwrap_or_default()
}

pub fn to_csv(projects: &[Project]) -> String {
    let mut out = String::from(
        "name,path,stacks,frameworks,status,safety,last_activity,total_bytes,reclaimable_bytes,review_bytes,shared_elsewhere_bytes,file_count,git_state,git_branch,protected,artifacts\n",
    );
    for p in projects {
        let artifacts = p
            .artifacts
            .iter()
            .map(|a| {
                format!(
                    "{}={}{}{}",
                    a.relative_path,
                    a.bytes,
                    if a.safety == Safety::Review {
                        " (review)"
                    } else {
                        ""
                    },
                    if a.shared_elsewhere > 0 {
                        format!(" (+{} shared)", a.shared_elsewhere)
                    } else {
                        String::new()
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let row = [
            csv_escape(&p.name),
            csv_escape(&p.path.to_string_lossy()),
            csv_escape(
                &p.stacks
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
                    .join("+"),
            ),
            csv_escape(&p.frameworks.join("+")),
            format!("{:?}", p.status).to_lowercase(),
            format!("{:?}", p.safety).to_lowercase(),
            iso(p.last_activity_at),
            p.total_bytes.to_string(),
            p.reclaimable_bytes.to_string(),
            p.review_bytes.to_string(),
            // Hard-linked into a shared store elsewhere, so not freed by
            // removal and already left out of reclaimable_bytes.
            p.artifacts
                .iter()
                .map(|a| a.shared_elsewhere)
                .sum::<u64>()
                .to_string(),
            p.file_count.to_string(),
            p.git
                .as_ref()
                .map(|g| g.state.label().to_string())
                .unwrap_or_default(),
            csv_escape(
                &p.git
                    .as_ref()
                    .and_then(|g| g.branch.clone())
                    .unwrap_or_default(),
            ),
            p.protected.to_string(),
            csv_escape(&artifacts),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

pub fn to_json(projects: &[Project]) -> String {
    serde_json::to_string_pretty(projects).unwrap_or_else(|_| "[]".into())
}

pub fn render(projects: &[Project], format: ExportFormat) -> String {
    match format {
        ExportFormat::Csv => to_csv(projects),
        ExportFormat::Json => to_json(projects),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spreadsheet of reclaimable space is exactly where someone adds the
    /// column up and compares it to what they see on disk. Without the
    /// shared figure, a pnpm project's honest number looks like an error.
    #[cfg(unix)]
    #[test]
    fn csv_carries_what_is_shared_with_a_store() {
        use crate::config::AppState;
        use crate::scanner::{scan, ScanOptions};
        use std::fs;
        use std::sync::atomic::AtomicBool;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Projects");
        let web = root.join("web");
        fs::create_dir_all(web.join("node_modules/pkg")).unwrap();
        fs::write(web.join("package.json"), r#"{"name":"web"}"#).unwrap();
        let store = tmp.path().join("store");
        fs::create_dir_all(&store).unwrap();
        fs::write(store.join("blob"), vec![b'x'; 150_000]).unwrap();
        fs::hard_link(store.join("blob"), web.join("node_modules/pkg/blob")).unwrap();

        let result = scan(
            &[root],
            &ScanOptions {
                inspect_git: false,
                ..Default::default()
            },
            &AppState::default(),
            &AtomicBool::new(false),
            &|_| {},
        );
        let csv = to_csv(&result.projects);
        let mut lines = csv.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = header
            .iter()
            .position(|h| *h == "shared_elsewhere_bytes")
            .expect("the shared column is in the header");
        let row = lines.next().expect("one project row");
        // No field before the shared column contains a comma, so a plain
        // split is enough to find it.
        let shared: u64 = row.split(',').nth(col).unwrap().parse().unwrap();
        assert!(shared >= 150_000, "got {shared} in: {row}");
        assert!(
            row.contains("shared)"),
            "the artifact detail says so too: {row}"
        );
    }

    #[test]
    fn escapes_csv() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("has,comma"), "\"has,comma\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
