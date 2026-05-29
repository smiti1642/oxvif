//! Result types for a [`HealthCheck`](super::HealthCheck) run.

use std::fmt;
use std::time::Duration;

/// Which area of the device a check exercises. Also drives report grouping order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Connectivity,
    Time,
    Services,
    Media,
    Imaging,
    Ptz,
    Events,
    Network,
    Users,
    Write,
}

impl Category {
    /// Human-readable group label.
    pub fn label(self) -> &'static str {
        match self {
            Category::Connectivity => "Connectivity",
            Category::Time => "Time",
            Category::Services => "Services",
            Category::Media => "Media",
            Category::Imaging => "Imaging",
            Category::Ptz => "PTZ",
            Category::Events => "Events",
            Category::Network => "Network",
            Category::Users => "Users",
            Category::Write => "Write round-trip",
        }
    }
}

/// Outcome of a single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    /// The check succeeded.
    Pass,
    /// The check succeeded but something is off (reason attached).
    Warn(String),
    /// The check failed (reason attached).
    Fail(String),
    /// The check did not run (reason attached, e.g. service not advertised).
    Skip(String),
}

impl CheckStatus {
    /// Short uppercase tag for table output.
    pub fn tag(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "PASS",
            CheckStatus::Warn(_) => "WARN",
            CheckStatus::Fail(_) => "FAIL",
            CheckStatus::Skip(_) => "SKIP",
        }
    }
}

/// Result of one named check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Stable identifier (e.g. `"get_profiles"`).
    pub id: &'static str,
    /// Grouping category.
    pub category: Category,
    /// Pass / Warn / Fail / Skip.
    pub status: CheckStatus,
    /// Extra context (parsed values on success, error text otherwise).
    pub detail: String,
    /// Wall-clock time this check took.
    pub elapsed: Duration,
}

impl CheckResult {
    pub(crate) fn pass(id: &'static str, category: Category, detail: impl Into<String>) -> Self {
        Self {
            id,
            category,
            status: CheckStatus::Pass,
            detail: detail.into(),
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn warn(
        id: &'static str,
        category: Category,
        reason: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            category,
            status: CheckStatus::Warn(reason.into()),
            detail: detail.into(),
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn fail(id: &'static str, category: Category, reason: impl Into<String>) -> Self {
        Self {
            id,
            category,
            status: CheckStatus::Fail(reason.into()),
            detail: String::new(),
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn skip(id: &'static str, category: Category, reason: impl Into<String>) -> Self {
        Self {
            id,
            category,
            status: CheckStatus::Skip(reason.into()),
            detail: String::new(),
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn with_elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = elapsed;
        self
    }
}

/// Per-profile conformance verdict derived from the check results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileVerdict {
    /// All required checks passed.
    Conformant,
    /// Some required checks passed, others failed or were skipped.
    Partial,
    /// No required checks passed (profile likely unsupported).
    Unsupported,
}

impl ProfileVerdict {
    fn label(self) -> &'static str {
        match self {
            ProfileVerdict::Conformant => "conformant",
            ProfileVerdict::Partial => "partial",
            ProfileVerdict::Unsupported => "unsupported",
        }
    }
}

/// Conformance assessment for the ONVIF profiles, derived from check results.
#[derive(Debug, Clone)]
pub struct ProfileAssessment {
    /// Profile S (video streaming): verdict + non-passing required check ids.
    pub profile_s: (ProfileVerdict, Vec<&'static str>),
    /// Profile T (advanced streaming / imaging / events).
    pub profile_t: (ProfileVerdict, Vec<&'static str>),
    /// Profile G (recording / search / replay) — not exercised; informational.
    pub profile_g: (ProfileVerdict, Vec<&'static str>),
}

/// The full result of a [`HealthCheck`](super::HealthCheck) run.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Device URL the check ran against.
    pub target: String,
    /// Total wall-clock time for the whole run.
    pub total_elapsed: Duration,
    /// Individual check results, ordered by category.
    pub checks: Vec<CheckResult>,
    /// Per-profile conformance verdicts.
    pub profiles: ProfileAssessment,
}

impl HealthReport {
    /// Number of checks with a given status discriminant.
    pub fn count(&self, want: fn(&CheckStatus) -> bool) -> usize {
        self.checks.iter().filter(|c| want(&c.status)).count()
    }

    /// `true` when no check failed (warnings and skips are tolerated).
    pub fn ok(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Fail(_)))
    }
}

impl fmt::Display for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ONVIF health check — {}", self.target)?;
        writeln!(
            f,
            "  {} pass · {} warn · {} fail · {} skip   ({} ms total)",
            self.count(|s| matches!(s, CheckStatus::Pass)),
            self.count(|s| matches!(s, CheckStatus::Warn(_))),
            self.count(|s| matches!(s, CheckStatus::Fail(_))),
            self.count(|s| matches!(s, CheckStatus::Skip(_))),
            self.total_elapsed.as_millis(),
        )?;
        let mut last: Option<Category> = None;
        for c in &self.checks {
            if last != Some(c.category) {
                writeln!(f, "\n  [{}]", c.category.label())?;
                last = Some(c.category);
            }
            let note = match &c.status {
                CheckStatus::Pass => c.detail.clone(),
                CheckStatus::Warn(r) | CheckStatus::Fail(r) | CheckStatus::Skip(r) => r.clone(),
            };
            writeln!(
                f,
                "    {:<4} {:<28} {:>5}ms  {}",
                c.status.tag(),
                c.id,
                c.elapsed.as_millis(),
                note,
            )?;
        }
        writeln!(f, "\n  Profiles:")?;
        for (name, (verdict, missing)) in [
            ("S", &self.profiles.profile_s),
            ("T", &self.profiles.profile_t),
            ("G", &self.profiles.profile_g),
        ] {
            let miss = if missing.is_empty() {
                String::new()
            } else {
                format!("  (missing: {})", missing.join(", "))
            };
            writeln!(f, "    Profile {name}: {}{miss}", verdict.label())?;
        }
        Ok(())
    }
}
