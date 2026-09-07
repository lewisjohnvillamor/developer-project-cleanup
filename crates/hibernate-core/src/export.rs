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
        "name,path,stacks,frameworks,status,safety,last_activity,total_bytes,reclaimable_bytes,review_bytes,file_count,git_state,git_branch,protected,artifacts\n",
    );
    for p in projects {
        let artifacts = p
            .artifacts
            .iter()
            .map(|a| {
                format!(
                    "{}={}{}",
                    a.relative_path,
                    a.bytes,
                    if a.safety == Safety::Review {
                        " (review)"
                    } else {
                        ""
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

    #[test]
    fn escapes_csv() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("has,comma"), "\"has,comma\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
