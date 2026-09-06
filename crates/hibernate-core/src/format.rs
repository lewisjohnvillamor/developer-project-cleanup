//! Human-readable formatting shared by the CLI and (as reference behaviour)
//! the frontend. Sizes use decimal units (1 GB = 1,000,000,000 bytes), which
//! matches what most developers see in Finder and `du -h --si`.

use chrono::{DateTime, Utc};

/// Format a byte count as `1.42 GB`, `890 MB`, `31 MB`, `512 KB`, `12 B`.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

/// Format a timestamp relative to now: `today`, `4 days ago`, `2 months ago`.
pub fn relative(at: DateTime<Utc>) -> String {
    relative_to(at, Utc::now())
}

pub fn relative_to(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - at).num_seconds();
    if secs < 0 {
        return "in the future".to_string();
    }
    let days = secs / 86_400;
    match days {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        d if d < 30 => format!("{d} days ago"),
        d if d < 365 => {
            let months = d / 30;
            if months == 1 {
                "1 month ago".to_string()
            } else {
                format!("{months} months ago")
            }
        }
        d => {
            let years = d / 365;
            if years == 1 {
                "1 year ago".to_string()
            } else {
                format!("{years} years ago")
            }
        }
    }
}

/// Compact day count used in dense tables: `0d`, `14d`, `67d`.
pub fn days_ago(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = (now - at).num_days().max(0);
    format!("{days}d")
}

/// Thousands separators for counts: `36,412`.
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn formats_bytes_with_decimal_units() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_000), "1.00 KB");
        assert_eq!(bytes(31_000_000), "31.0 MB");
        assert_eq!(bytes(890_000_000), "890 MB");
        assert_eq!(bytes(1_420_000_000), "1.42 GB");
        assert_eq!(bytes(94_600_000_000), "94.6 GB");
    }

    #[test]
    fn formats_relative_time() {
        let now = Utc::now();
        assert_eq!(relative_to(now, now), "today");
        assert_eq!(relative_to(now - Duration::days(1), now), "yesterday");
        assert_eq!(relative_to(now - Duration::days(14), now), "14 days ago");
        assert_eq!(relative_to(now - Duration::days(67), now), "2 months ago");
        assert_eq!(relative_to(now - Duration::days(400), now), "1 year ago");
    }

    #[test]
    fn formats_counts() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1_267), "1,267");
        assert_eq!(count(36_412), "36,412");
        assert_eq!(count(1_000_000), "1,000,000");
    }
}
