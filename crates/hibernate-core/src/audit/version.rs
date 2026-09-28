//! Just enough semantic versioning to say whether a version falls inside an
//! advisory's range.
//!
//! npm and crates.io both publish semver, so a small comparator covers the
//! real cases without another dependency. What it deliberately does *not* do
//! is guess: a version it cannot parse compares as `None`, and the caller
//! treats that as "no match" rather than inventing an ordering. A security
//! warning nobody trusts is worse than no warning at all, so this errs
//! towards silence.

use std::cmp::Ordering;

/// A parsed `major.minor.patch[-prerelease]`. Build metadata (`+sha`) is
/// ignored, as semver requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    parts: [u64; 3],
    /// Dot-separated prerelease identifiers. Empty means a release, which
    /// always sorts above any prerelease of the same numbers.
    pre: Vec<String>,
}

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        // Build metadata never affects ordering.
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, pre),
            None => (text, ""),
        };

        let mut parts = [0u64; 3];
        let mut seen = 0;
        for (i, segment) in core.split('.').enumerate() {
            if i >= 3 {
                return None;
            }
            parts[i] = segment.parse().ok()?;
            seen += 1;
        }
        if seen == 0 {
            return None;
        }

        let pre = if pre.is_empty() {
            Vec::new()
        } else {
            pre.split('.').map(str::to_string).collect()
        };
        Some(Version { parts, pre })
    }

    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

/// Compare one prerelease identifier against another. Numeric identifiers
/// sort below alphanumeric ones and compare numerically; everything else
/// compares as text, which is what semver specifies.
fn compare_identifier(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let numbers = self.parts.cmp(&other.parts);
        if numbers != Ordering::Equal {
            return numbers;
        }
        // 1.0.0-rc.1 comes before 1.0.0.
        match (self.is_prerelease(), other.is_prerelease()) {
            (false, false) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => {
                for (a, b) in self.pre.iter().zip(other.pre.iter()) {
                    let ord = compare_identifier(a, b);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} should parse"))
    }

    #[test]
    fn orders_releases_by_number() {
        assert!(v("1.2.3") < v("1.2.4"));
        assert!(v("1.2.3") < v("1.3.0"));
        assert!(v("1.9.9") < v("2.0.0"));
        assert_eq!(v("1.2.3"), v("1.2.3"));
        // Missing components default to zero, so 1.2 is 1.2.0.
        assert_eq!(v("1.2"), v("1.2.0"));
        assert!(v("2") > v("1.99.99"));
        // Build metadata is not part of the order.
        assert_eq!(v("1.2.3+build.5"), v("1.2.3"));
        assert_eq!(v("v1.2.3"), v("1.2.3"));
    }

    #[test]
    fn a_prerelease_comes_before_its_release() {
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
        assert!(v("1.0.0-alpha") < v("1.0.0-beta"));
        assert!(v("1.0.0-alpha.1") < v("1.0.0-alpha.2"));
        // A numeric identifier sorts below an alphanumeric one.
        assert!(v("1.0.0-1") < v("1.0.0-alpha"));
        // Numeric identifiers compare as numbers, not as text.
        assert!(v("1.0.0-2") < v("1.0.0-10"));
        // More identifiers means later, all else equal.
        assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
    }

    /// Anything this cannot read is reported as unparseable so the caller can
    /// stay quiet, rather than being coerced into a wrong ordering.
    #[test]
    fn refuses_to_guess_at_what_it_cannot_read() {
        for bad in ["", "latest", "1.2.3.4", "1.x", "not-a-version", "~1.0.0"] {
            assert!(Version::parse(bad).is_none(), "{bad} should not parse");
        }
    }
}
