//! Result types for a [`HealthCheck`](super::HealthCheck) run.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::OnvifError;
use crate::soap::SoapError;

/// Which area of the device a check exercises. Also drives report grouping order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
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
    Coverage,
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
            Category::Coverage => "Parse coverage",
            Category::Write => "Write round-trip",
        }
    }
}

/// Outcome of a single check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason")]
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

/// Machine-readable classification of a failing check's underlying error.
/// Lets consumers group faults across brands (by `subcode`) and separate genuine
/// device faults from client-side preconditions or transport problems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// Device returned a `<s:Fault>`.
    SoapFault,
    /// Client-side precondition unmet (e.g. a required service URL wasn't
    /// advertised, so the call was never sent).
    Precondition,
    /// Response received but could not be parsed / didn't match the schema.
    Parse,
    /// Network / TLS / non-200 HTTP failure before a SOAP response.
    Http,
    /// The caller asked for something the ONVIF schema disallows; no request sent.
    InvalidArgument,
}

/// Structured facts about a failing check's error, extracted at the source so
/// the flat `CheckStatus::Fail(reason)` string doesn't have to be re-parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckError {
    pub class: ErrorClass,
    /// SOAP fault `Code/Value` (e.g. `SOAP-ENV:Sender`), when a fault.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fault_code: Option<String>,
    /// SOAP fault `Code/Subcode/Value` (e.g. `ter:NotAuthorized`) — the stable
    /// cross-brand grouping key, when the device supplies it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subcode: Option<String>,
    /// Human-readable reason (fault `Reason/Text`, or the error's Display).
    pub reason: String,
    /// Verbatim `<Detail>` text, when present.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<String>,
}

impl From<&OnvifError> for CheckError {
    fn from(e: &OnvifError) -> Self {
        match e {
            OnvifError::Soap(SoapError::Fault {
                code,
                reason,
                subcode,
                detail,
            }) => CheckError {
                class: ErrorClass::SoapFault,
                fault_code: (!code.is_empty()).then(|| code.clone()),
                subcode: subcode.clone(),
                reason: if reason.is_empty() {
                    e.to_string()
                } else {
                    reason.clone()
                },
                detail: detail.clone(),
            },
            OnvifError::Soap(SoapError::MissingField(_)) => CheckError {
                class: ErrorClass::Precondition,
                fault_code: None,
                subcode: None,
                reason: e.to_string(),
                detail: None,
            },
            OnvifError::Soap(
                SoapError::XmlParse(_)
                | SoapError::MissingBody
                | SoapError::UnexpectedResponse(_)
                | SoapError::InvalidValue { .. },
            ) => CheckError {
                class: ErrorClass::Parse,
                fault_code: None,
                subcode: None,
                reason: e.to_string(),
                detail: None,
            },
            OnvifError::Transport(_) => CheckError {
                class: ErrorClass::Http,
                fault_code: None,
                subcode: None,
                reason: e.to_string(),
                detail: None,
            },
            OnvifError::InvalidArgument(_) => CheckError {
                class: ErrorClass::InvalidArgument,
                fault_code: None,
                subcode: None,
                reason: e.to_string(),
                detail: None,
            },
        }
    }
}

/// Result of one named check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    /// Stable identifier (e.g. `"get_profiles"`).
    pub id: String,
    /// Grouping category.
    pub category: Category,
    /// Pass / Warn / Fail / Skip.
    pub status: CheckStatus,
    /// Extra context (parsed values on success, error text otherwise).
    pub detail: String,
    /// Structured error facts when this check failed (absent on pass/warn/skip).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<CheckError>,
    /// Wall-clock time this check took.
    #[serde(with = "duration_ms", rename = "elapsed_ms")]
    pub elapsed: Duration,
}

mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u128(d.as_millis())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}

impl CheckResult {
    pub(crate) fn pass(
        id: impl Into<String>,
        category: Category,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category,
            status: CheckStatus::Pass,
            detail: detail.into(),
            error: None,
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn warn(
        id: impl Into<String>,
        category: Category,
        reason: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category,
            status: CheckStatus::Warn(reason.into()),
            detail: detail.into(),
            error: None,
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn fail(
        id: impl Into<String>,
        category: Category,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category,
            status: CheckStatus::Fail(reason.into()),
            detail: String::new(),
            error: None,
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn skip(
        id: impl Into<String>,
        category: Category,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category,
            status: CheckStatus::Skip(reason.into()),
            detail: String::new(),
            error: None,
            elapsed: Duration::ZERO,
        }
    }

    /// Fail carrying both the human reason (the error's Display) and structured
    /// error facts extracted from the same `OnvifError`.
    pub(crate) fn fail_from(id: impl Into<String>, category: Category, err: &OnvifError) -> Self {
        Self {
            id: id.into(),
            category,
            status: CheckStatus::Fail(err.to_string()),
            detail: String::new(),
            error: Some(CheckError::from(err)),
            elapsed: Duration::ZERO,
        }
    }

    /// Attach structured error facts to a check built with a custom reason string.
    pub(crate) fn with_error(mut self, err: &OnvifError) -> Self {
        self.error = Some(CheckError::from(err));
        self
    }

    pub(crate) fn with_elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = elapsed;
        self
    }
}

/// Per-profile conformance verdict derived from the check results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileAssessment {
    /// Profile S (video streaming): verdict + non-passing required check ids.
    pub profile_s: (ProfileVerdict, Vec<String>),
    /// Profile T (advanced streaming / imaging / events).
    pub profile_t: (ProfileVerdict, Vec<String>),
    /// Profile G (recording / search / replay) — not exercised; informational.
    pub profile_g: (ProfileVerdict, Vec<String>),
}

/// The full result of a [`HealthCheck`](super::HealthCheck) run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    /// Device URL the check ran against.
    pub target: String,
    /// Total wall-clock time for the whole run.
    #[serde(with = "duration_ms", rename = "total_elapsed_ms")]
    pub total_elapsed: Duration,
    /// Individual check results, ordered by category.
    pub checks: Vec<CheckResult>,
    /// Per-profile conformance verdicts.
    pub profiles: ProfileAssessment,
    /// Device clock skew vs local in seconds (`device_utc - local_utc`), when the
    /// `system_date_time` check obtained it. Large values are the usual cause of
    /// spurious WS-Security auth failures.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub clock_skew_s: Option<i64>,
    /// ONVIF profiles the device *self-declares* via its scopes (canonical
    /// letters, e.g. `["S", "T", "G"]`). This is the vendor's claim, independent
    /// of [`profiles`](Self::profiles) — which is what oxvif actually *assessed*.
    /// Comparing the two surfaces "declares Profile G but replay/search fail".
    /// Empty when scopes were unavailable.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub declared_profiles: Vec<String>,
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

    /// Serialise the report as a compact single-line JSON string. Durations
    /// are emitted as integer milliseconds (fields suffixed `_ms`).
    pub fn to_json(&self) -> String {
        // serde_json on the fully-serializable HealthReport — infallible.
        serde_json::to_string(self).expect("HealthReport is fully serializable")
    }

    /// Serialise the report as pretty-printed JSON (indented, line-separated).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("HealthReport is fully serializable")
    }

    /// Compare this report against an earlier baseline and report the
    /// differences relevant for regression tracking (e.g. between firmware
    /// versions).
    pub fn diff(&self, prev: &HealthReport) -> ReportDiff {
        ReportDiff::compute(prev, self)
    }
}

// ── ReportDiff ────────────────────────────────────────────────────────────────

/// Differences between two [`HealthReport`]s, surfacing the things that
/// matter for regression tracking.
///
/// Computed via [`HealthReport::diff`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportDiff {
    /// Checks that were not failing in `prev` but are failing now.
    pub flipped_to_fail: Vec<String>,
    /// Checks that were failing in `prev` but are not failing now.
    pub flipped_to_pass: Vec<String>,
    /// Check ids present in the new report but not in the baseline.
    pub new_checks: Vec<String>,
    /// Check ids present in the baseline but missing from the new report.
    pub removed_checks: Vec<String>,
    /// Checks whose runtime grew significantly: now > 2 × prev AND
    /// the delta exceeds 100 ms (filters millisecond-scale noise).
    /// Tuples are `(id, prev_ms, now_ms)`.
    pub slowed: Vec<SlowedCheck>,
}

/// One entry in [`ReportDiff::slowed`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlowedCheck {
    pub id: String,
    pub prev_ms: u128,
    pub now_ms: u128,
}

impl ReportDiff {
    /// `true` if nothing flipped, nothing changed set, and nothing slowed.
    pub fn is_empty(&self) -> bool {
        self.flipped_to_fail.is_empty()
            && self.flipped_to_pass.is_empty()
            && self.new_checks.is_empty()
            && self.removed_checks.is_empty()
            && self.slowed.is_empty()
    }

    fn compute(prev: &HealthReport, now: &HealthReport) -> Self {
        use std::collections::HashMap;
        let prev_by_id: HashMap<&str, &CheckResult> =
            prev.checks.iter().map(|c| (c.id.as_str(), c)).collect();
        let now_by_id: HashMap<&str, &CheckResult> =
            now.checks.iter().map(|c| (c.id.as_str(), c)).collect();

        let mut flipped_to_fail = Vec::new();
        let mut flipped_to_pass = Vec::new();
        let mut new_checks = Vec::new();
        let mut slowed = Vec::new();

        for c in &now.checks {
            match prev_by_id.get(c.id.as_str()) {
                None => new_checks.push(c.id.clone()),
                Some(p) => {
                    let was_fail = matches!(p.status, CheckStatus::Fail(_));
                    let is_fail = matches!(c.status, CheckStatus::Fail(_));
                    match (was_fail, is_fail) {
                        (false, true) => flipped_to_fail.push(c.id.clone()),
                        (true, false) => flipped_to_pass.push(c.id.clone()),
                        _ => {}
                    }
                    let prev_ms = p.elapsed.as_millis();
                    let now_ms = c.elapsed.as_millis();
                    if now_ms > prev_ms.saturating_mul(2) && now_ms.saturating_sub(prev_ms) > 100 {
                        slowed.push(SlowedCheck {
                            id: c.id.clone(),
                            prev_ms,
                            now_ms,
                        });
                    }
                }
            }
        }

        let removed_checks: Vec<String> = prev
            .checks
            .iter()
            .filter(|c| !now_by_id.contains_key(c.id.as_str()))
            .map(|c| c.id.clone())
            .collect();

        Self {
            flipped_to_fail,
            flipped_to_pass,
            new_checks,
            removed_checks,
            slowed,
        }
    }
}

impl fmt::Display for ReportDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Diff vs baseline:")?;
        if self.is_empty() {
            writeln!(f, "  no changes")?;
            return Ok(());
        }
        if !self.flipped_to_fail.is_empty() {
            writeln!(f, "  flipped → FAIL: {}", self.flipped_to_fail.join(", "))?;
        }
        if !self.flipped_to_pass.is_empty() {
            writeln!(f, "  recovered    : {}", self.flipped_to_pass.join(", "))?;
        }
        if !self.new_checks.is_empty() {
            writeln!(f, "  new checks   : {}", self.new_checks.join(", "))?;
        }
        if !self.removed_checks.is_empty() {
            writeln!(f, "  removed      : {}", self.removed_checks.join(", "))?;
        }
        if !self.slowed.is_empty() {
            writeln!(f, "  slowed:")?;
            for s in &self.slowed {
                writeln!(f, "    {:<28} {:>5}ms → {:>5}ms", s.id, s.prev_ms, s.now_ms)?;
            }
        }
        Ok(())
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, status: CheckStatus, elapsed_ms: u64) -> CheckResult {
        let mut c = CheckResult::pass(id, Category::Media, "");
        c.status = status;
        c.elapsed = Duration::from_millis(elapsed_ms);
        c
    }

    #[test]
    fn check_error_classifies_variants() {
        // SOAP fault → SoapFault with the subcode preserved as the grouping key.
        let e = OnvifError::Soap(SoapError::Fault {
            code: "SOAP-ENV:Sender".into(),
            reason: "Sender not Authorized".into(),
            subcode: Some("ter:NotAuthorized".into()),
            detail: None,
        });
        let ce = CheckError::from(&e);
        assert_eq!(ce.class, ErrorClass::SoapFault);
        assert_eq!(ce.fault_code.as_deref(), Some("SOAP-ENV:Sender"));
        assert_eq!(ce.subcode.as_deref(), Some("ter:NotAuthorized"));
        assert_eq!(ce.reason, "Sender not Authorized");

        // Client-side precondition (unadvertised service) → Precondition, not a fault.
        let e = OnvifError::Soap(SoapError::missing("Search service URL"));
        assert_eq!(CheckError::from(&e).class, ErrorClass::Precondition);
    }

    fn assess() -> ProfileAssessment {
        ProfileAssessment {
            profile_s: (ProfileVerdict::Conformant, vec![]),
            profile_t: (ProfileVerdict::Conformant, vec![]),
            profile_g: (ProfileVerdict::Unsupported, vec!["recording".into()]),
        }
    }

    fn report(checks: Vec<CheckResult>) -> HealthReport {
        HealthReport {
            target: "http://test/onvif/device_service".into(),
            total_elapsed: Duration::from_millis(123),
            checks,
            profiles: assess(),
            clock_skew_s: None,
            declared_profiles: vec![],
        }
    }

    #[test]
    fn diff_flags_pass_to_fail() {
        let prev = report(vec![check("a", CheckStatus::Pass, 10)]);
        let now = report(vec![check("a", CheckStatus::Fail("boom".into()), 10)]);
        let d = now.diff(&prev);
        assert_eq!(d.flipped_to_fail, vec!["a".to_string()]);
        assert!(d.flipped_to_pass.is_empty());
    }

    #[test]
    fn diff_flags_fail_to_pass() {
        let prev = report(vec![check("a", CheckStatus::Fail("x".into()), 10)]);
        let now = report(vec![check("a", CheckStatus::Pass, 10)]);
        let d = now.diff(&prev);
        assert_eq!(d.flipped_to_pass, vec!["a".to_string()]);
        assert!(d.flipped_to_fail.is_empty());
    }

    #[test]
    fn diff_flags_warn_is_not_flipped() {
        // Warn is not a fail, so warn → pass and pass → warn don't flip.
        let prev = report(vec![check("a", CheckStatus::Warn("slow".into()), 10)]);
        let now = report(vec![check("a", CheckStatus::Pass, 10)]);
        let d = now.diff(&prev);
        assert!(d.flipped_to_pass.is_empty());
        assert!(d.flipped_to_fail.is_empty());
    }

    #[test]
    fn diff_lists_added_and_removed_checks() {
        let prev = report(vec![
            check("a", CheckStatus::Pass, 1),
            check("b", CheckStatus::Pass, 1),
        ]);
        let now = report(vec![
            check("a", CheckStatus::Pass, 1),
            check("c", CheckStatus::Pass, 1),
        ]);
        let d = now.diff(&prev);
        assert_eq!(d.new_checks, vec!["c".to_string()]);
        assert_eq!(d.removed_checks, vec!["b".to_string()]);
    }

    #[test]
    fn diff_slowed_requires_doubled_and_delta_over_100ms() {
        // 10 → 30 ms (delta < 100 ms) → not slowed.
        let prev = report(vec![check("a", CheckStatus::Pass, 10)]);
        let now = report(vec![check("a", CheckStatus::Pass, 30)]);
        assert!(now.diff(&prev).slowed.is_empty());

        // 200 → 350 ms (less than 2×) → not slowed.
        let prev = report(vec![check("b", CheckStatus::Pass, 200)]);
        let now = report(vec![check("b", CheckStatus::Pass, 350)]);
        assert!(now.diff(&prev).slowed.is_empty());

        // 100 → 250 ms (2.5× AND delta 150 > 100) → slowed.
        let prev = report(vec![check("c", CheckStatus::Pass, 100)]);
        let now = report(vec![check("c", CheckStatus::Pass, 250)]);
        let d = now.diff(&prev);
        assert_eq!(d.slowed.len(), 1);
        assert_eq!(d.slowed[0].id, "c");
        assert_eq!(d.slowed[0].prev_ms, 100);
        assert_eq!(d.slowed[0].now_ms, 250);
    }

    #[test]
    fn diff_is_empty_when_nothing_changed() {
        let r = report(vec![check("a", CheckStatus::Pass, 5)]);
        assert!(r.diff(&r).is_empty());
    }

    #[test]
    fn report_round_trips_through_json() {
        let r = report(vec![
            check("a", CheckStatus::Pass, 12),
            check("b", CheckStatus::Fail("nope".into()), 7),
        ]);
        let json = r.to_json();
        let re: HealthReport = serde_json::from_str(&json).expect("json deserialises");
        assert_eq!(re.target, r.target);
        assert_eq!(re.checks.len(), r.checks.len());
        assert_eq!(re.checks[1].id, "b");
        assert!(matches!(re.checks[1].status, CheckStatus::Fail(_)));
        assert_eq!(re.checks[0].elapsed, Duration::from_millis(12));
        assert_eq!(re.total_elapsed, Duration::from_millis(123));
    }
}
