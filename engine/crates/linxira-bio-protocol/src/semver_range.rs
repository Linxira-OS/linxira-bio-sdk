//! Minimal semver range matching for workflow pack `core_compatibility`.
//!
//! The supported grammar is a comma-separated list of comparators, e.g.
//! `">=0.1.0,<1.0.0"` or `"^0.2.3"`. Each comparator is one of:
//! `>=`, `<=`, `>`, `<`, `=`, `~` (tilde, patch-bound), `^` (caret), or a
//! bare version (exact match). `*` or an empty range matches any version.
//!
//! Versions are compared as up-to-three numeric components (`major.minor.patch`);
//! missing trailing components count as zero (`1.2` == `1.2.0`).

/// A parsed three-component numeric version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        let mut components = input.split('.');
        let major = components.next().unwrap_or_default();
        let minor = components.next().unwrap_or("0");
        let patch = components.next().unwrap_or("0");
        if components.next().is_some() {
            return None;
        }
        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            patch: patch.parse().ok()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparatorKind {
    GreaterOrEqual,
    LessOrEqual,
    Greater,
    Less,
    Equal,
    Tilde,
    Caret,
}

struct Comparator {
    kind: ComparatorKind,
    version: Version,
}

impl Comparator {
    fn matches(&self, version: Version) -> bool {
        match self.kind {
            ComparatorKind::GreaterOrEqual => version >= self.version,
            ComparatorKind::LessOrEqual => version <= self.version,
            ComparatorKind::Greater => version > self.version,
            ComparatorKind::Less => version < self.version,
            ComparatorKind::Equal => version == self.version,
            ComparatorKind::Tilde => {
                // `~1.2.3` -> `>=1.2.3,<1.3.0`; `~1.2` -> `>=1.2.0,<1.3.0`.
                let upper = Version {
                    major: self.version.major,
                    minor: self.version.minor + 1,
                    patch: 0,
                };
                version >= self.version && version < upper
            }
            ComparatorKind::Caret => {
                // `^1.2.3` -> `>=1.2.3,<2.0.0`; `^0.2.3` -> `>=0.2.3,<0.3.0`;
                // `^0.0.3` -> `>=0.0.3,<0.0.4`.
                let upper = if self.version.major > 0 {
                    Version {
                        major: self.version.major + 1,
                        minor: 0,
                        patch: 0,
                    }
                } else if self.version.minor > 0 {
                    Version {
                        major: 0,
                        minor: self.version.minor + 1,
                        patch: 0,
                    }
                } else {
                    Version {
                        major: 0,
                        minor: 0,
                        patch: self.version.patch + 1,
                    }
                };
                version >= self.version && version < upper
            }
        }
    }
}

fn parse_comparator(input: &str) -> Option<Comparator> {
    let input = input.trim();
    if input.is_empty() || input == "*" {
        return None;
    }
    for (prefix, kind) in [
        (">=", ComparatorKind::GreaterOrEqual),
        ("<=", ComparatorKind::LessOrEqual),
        (">", ComparatorKind::Greater),
        ("<", ComparatorKind::Less),
        ("=", ComparatorKind::Equal),
        ("~", ComparatorKind::Tilde),
        ("^", ComparatorKind::Caret),
    ] {
        if let Some(rest) = input.strip_prefix(prefix) {
            let version = Version::parse(rest)?;
            return Some(Comparator { kind, version });
        }
    }
    let version = Version::parse(input)?;
    Some(Comparator {
        kind: ComparatorKind::Equal,
        version,
    })
}

/// Returns `true` when `version` satisfies the semver `range`.
///
/// A missing, empty, or `*` range matches every version; a range with any
/// invalid comparator is a mismatch (never a panic).
pub fn core_compatibility_matches(range: &str, version: &str) -> bool {
    let Some(version) = Version::parse(version) else {
        return false;
    };
    let Some(parsed) = parse_range(range) else {
        return false;
    };
    parsed.iter().all(|comparator| comparator.matches(version))
}

fn parse_range(range: &str) -> Option<Vec<Comparator>> {
    let range = range.trim();
    if range.is_empty() || range == "*" {
        return Some(Vec::new());
    }
    let comparators = range
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_comparator)
        .collect::<Option<Vec<_>>>()?;
    if comparators.is_empty() {
        return None;
    }
    Some(comparators)
}

#[cfg(test)]
mod tests {
    use super::core_compatibility_matches;

    #[test]
    fn matches_the_canonical_milestone_range() {
        assert!(core_compatibility_matches(">=0.1.0,<1.0.0", "0.1.1"));
        assert!(core_compatibility_matches(">=0.1.0,<1.0.0", "0.2.0"));
        assert!(!core_compatibility_matches(">=0.1.0,<1.0.0", "1.0.0"));
        assert!(!core_compatibility_matches(">=0.1.0,<1.0.0", "0.0.9"));
    }

    #[test]
    fn matches_basic_comparators() {
        assert!(core_compatibility_matches(">=0.1.0", "0.1.0"));
        assert!(core_compatibility_matches(">0.1.0", "0.1.1"));
        assert!(!core_compatibility_matches(">0.1.0", "0.1.0"));
        assert!(core_compatibility_matches("<=1.0.0", "0.9.9"));
        assert!(!core_compatibility_matches("<1.0.0", "1.0.0"));
        assert!(core_compatibility_matches("=0.1.1", "0.1.1"));
        assert!(!core_compatibility_matches("=0.1.1", "0.1.2"));
    }

    #[test]
    fn matches_bare_versions_with_implied_patch() {
        assert!(core_compatibility_matches("0.1", "0.1.0"));
        assert!(!core_compatibility_matches("0.1", "0.1.1"));
    }

    #[test]
    fn matches_tilde_and_caret_ranges() {
        assert!(core_compatibility_matches("~1.2.3", "1.2.9"));
        assert!(!core_compatibility_matches("~1.2.3", "1.3.0"));
        assert!(core_compatibility_matches("~1.2", "1.2.5"));
        assert!(!core_compatibility_matches("~1.2", "1.3.0"));
        assert!(core_compatibility_matches("^1.2.3", "1.9.0"));
        assert!(!core_compatibility_matches("^1.2.3", "2.0.0"));
        assert!(core_compatibility_matches("^0.2.3", "0.2.9"));
        assert!(!core_compatibility_matches("^0.2.3", "0.3.0"));
        assert!(core_compatibility_matches("^0.0.3", "0.0.3"));
        assert!(!core_compatibility_matches("^0.0.3", "0.0.4"));
    }

    #[test]
    fn handles_wildcard_and_invalid_ranges() {
        assert!(core_compatibility_matches("", "0.1.0"));
        assert!(core_compatibility_matches("*", "9.9.9"));
        assert!(!core_compatibility_matches("not-a-range", "0.1.0"));
        assert!(!core_compatibility_matches(
            ">=0.1.0,<1.0.0",
            "not-a-version"
        ));
        assert!(!core_compatibility_matches(">=1.0.0,<2.0.0", "0.5.0"));
    }
}
