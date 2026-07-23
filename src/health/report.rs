//! Result types for a [`HealthCheck`](super::HealthCheck) run.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::OnvifError;
use crate::soap::SoapError;

/// Escape a string for XML: drop characters that are illegal in XML 1.0 (control
/// chars other than tab/newline/CR, which a device fault string could contain),
/// then entity-escape `& < > " '`. Safe for both attribute values and text.
fn x(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || c >= ' ')
        .collect();
    crate::types::xml_escape(&cleaned).into_owned()
}

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
    Security,
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
            Category::Security => "Security",
            Category::Coverage => "Parse coverage",
            Category::Write => "Write round-trip",
        }
    }
}

/// Outcome of a single check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
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

impl CheckError {
    /// `true` when this error is an authentication / authorization failure
    /// (credentials rejected, or a WS-Security token the device wouldn't accept)
    /// rather than a genuine device fault. Used to tell "we couldn't verify this"
    /// apart from "the device is genuinely non-conformant" — the same predicate
    /// the assessment logic and downstream consumers must agree on, so it lives
    /// here once.
    pub fn is_auth(&self) -> bool {
        let hay = format!("{} {}", self.subcode.as_deref().unwrap_or(""), self.reason)
            .to_ascii_lowercase();
        match self.class {
            ErrorClass::Http => hay.contains("401") || hay.contains("unauthorized"),
            ErrorClass::SoapFault => {
                hay.contains("notauthorized")
                    || hay.contains("not authorized")
                    || hay.contains("unauthorized")
                    || hay.contains("security token")
                    || hay.contains("authenticat")
            }
            _ => false,
        }
    }
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
    /// Wall-clock time this check took, or `None` when the check did not run
    /// (e.g. `Skip`) and so was never timed — serialised as `null`, distinct
    /// from a genuine `0` ms.
    #[serde(with = "opt_duration_ms", rename = "elapsed_ms")]
    pub elapsed: Option<Duration>,
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

/// Like [`duration_ms`] but for an optional duration: `None` serialises as JSON
/// `null` (a check that did not run / was not timed), `Some(d)` as integer ms.
mod opt_duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => s.serialize_some(&(d.as_millis() as u64)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        let ms: Option<u64> = Option::deserialize(d)?;
        Ok(ms.map(Duration::from_millis))
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
            elapsed: None,
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
            elapsed: None,
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
            elapsed: None,
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
            elapsed: None,
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
            elapsed: None,
        }
    }

    /// Attach structured error facts to a check built with a custom reason string.
    pub(crate) fn with_error(mut self, err: &OnvifError) -> Self {
        self.error = Some(CheckError::from(err));
        self
    }

    pub(crate) fn with_elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }
}

/// Per-profile conformance verdict derived from the check results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileVerdict {
    /// All required checks passed.
    Conformant,
    /// Some required checks passed, others were verified to genuinely fail.
    Partial,
    /// No required checks passed (profile likely unsupported).
    Unsupported,
    /// Nothing was verified to fail, but some required checks could not be
    /// tested (auth blocked / skipped) — conformance is *unknown*, not failed.
    /// Kept distinct from `Partial` so a reader never mistakes "couldn't test"
    /// for "tested and non-conformant".
    Inconclusive,
}

impl ProfileVerdict {
    fn label(self) -> &'static str {
        match self {
            ProfileVerdict::Conformant => "conformant",
            ProfileVerdict::Partial => "partial",
            ProfileVerdict::Unsupported => "unsupported",
            ProfileVerdict::Inconclusive => "inconclusive",
        }
    }
}

/// One profile's assessment: the verdict plus the required check ids behind it,
/// split by *why* they didn't pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileState {
    pub verdict: ProfileVerdict,
    /// Required checks that were verified to genuinely fail (device fault).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub missing: Vec<String>,
    /// Required checks that could not be verified (auth-blocked or skipped) —
    /// separated from `missing` so an auth failure is never read as a
    /// conformance failure.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unverified: Vec<String>,
}

/// Conformance assessment for the ONVIF profiles, derived from check results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileAssessment {
    /// Profile S (video streaming).
    pub profile_s: ProfileState,
    /// Profile T (advanced streaming / imaging / events).
    pub profile_t: ProfileState,
    /// Profile G (recording / search / replay) — not exercised; informational.
    pub profile_g: ProfileState,
}

/// One captured request/response for a check that failed — the raw evidence a
/// maintainer needs to see *why* a brand rejected a call. Populated only when
/// capture was enabled via [`HealthCheck::with_capture`](super::HealthCheck::with_capture).
///
/// The request has its WS-Security `Password` and `Nonce` blanked, so it never
/// carries credential-derivation material. IP addresses / serials in either
/// field are *not* scrubbed here — apply transport-agnostic redaction downstream
/// if the capture will be shared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedExchange {
    /// SOAP action (last URL segment, e.g. `GetStreamUri`).
    pub action: String,
    /// The request SOAP envelope, with WS-Security `Password`/`Nonce` redacted.
    pub request: String,
    /// The response body — a SOAP Fault envelope, or the transport error text.
    pub response: String,
    /// HTTP status when the failure was a non-SOAP transport error (e.g. `401`).
    /// `None` for a SOAP Fault (the device returned 400/500 with a fault body,
    /// which the transport collapses to a normal response) — read the fault code
    /// from `response` in that case.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub http_status: Option<u16>,
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
    /// Raw request/response for the checks that failed — populated only when
    /// capture was enabled via [`HealthCheck::with_capture`](super::HealthCheck::with_capture).
    /// Empty otherwise. See [`CapturedExchange`].
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub captured: Vec<CapturedExchange>,
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

    /// Render this report as a JUnit `<testsuite>` element (no `<?xml?>`
    /// declaration or `<testsuites>` wrapper), so it can be composed into a
    /// multi-device document. `name` names the suite (e.g. the device address).
    ///
    /// Each check becomes a `<testcase>`: `Fail` → `<failure>` (carrying the
    /// ONVIF subcode as `type` and any `<Detail>` as the body), `Skip` →
    /// `<skipped>`, `Warn` → a passing case with a `<system-out>` note (JUnit
    /// has no "warn", and a warning should not fail a CI gate). Each profile
    /// verdict is a testcase too: `Conformant` passes, `Partial`/`Unsupported`
    /// fail, `Inconclusive` is skipped.
    pub fn to_junit_testsuite(&self, name: &str) -> String {
        use std::fmt::Write as _;

        let mut cases = String::new();
        let mut tests = 0usize;
        let mut failures = 0usize;
        let mut skipped = 0usize;

        for c in &self.checks {
            tests += 1;
            let time = c.elapsed.map(|d| d.as_secs_f64()).unwrap_or(0.0);
            let _ = write!(
                cases,
                "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\"",
                x(&c.id),
                x(c.category.label()),
                time,
            );
            match &c.status {
                CheckStatus::Pass => cases.push_str("/>\n"),
                CheckStatus::Warn(reason) => {
                    let _ = write!(
                        cases,
                        ">\n      <system-out>WARN: {}</system-out>\n    </testcase>\n",
                        x(reason),
                    );
                }
                CheckStatus::Fail(reason) => {
                    failures += 1;
                    let typ = c
                        .error
                        .as_ref()
                        .map(|e| {
                            e.subcode
                                .clone()
                                .or_else(|| e.fault_code.clone())
                                .unwrap_or_else(|| format!("{:?}", e.class))
                        })
                        .unwrap_or_else(|| "fail".into());
                    let detail = c.error.as_ref().and_then(|e| e.detail.as_deref());
                    let _ = write!(
                        cases,
                        ">\n      <failure message=\"{}\" type=\"{}\">{}</failure>\n    </testcase>\n",
                        x(reason),
                        x(&typ),
                        x(detail.unwrap_or("")),
                    );
                }
                CheckStatus::Skip(reason) => {
                    skipped += 1;
                    let _ = write!(
                        cases,
                        ">\n      <skipped message=\"{}\"/>\n    </testcase>\n",
                        x(reason),
                    );
                }
            }
        }

        for (letter, st) in [
            ("S", &self.profiles.profile_s),
            ("T", &self.profiles.profile_t),
            ("G", &self.profiles.profile_g),
        ] {
            tests += 1;
            let _ = write!(
                cases,
                "    <testcase name=\"Profile {letter}\" classname=\"Profiles\"",
            );
            match st.verdict {
                ProfileVerdict::Conformant => cases.push_str("/>\n"),
                ProfileVerdict::Inconclusive => {
                    skipped += 1;
                    let _ = write!(
                        cases,
                        ">\n      <skipped message=\"inconclusive (unverified: {})\"/>\n    </testcase>\n",
                        x(&st.unverified.join(", ")),
                    );
                }
                ProfileVerdict::Partial | ProfileVerdict::Unsupported => {
                    failures += 1;
                    let _ = write!(
                        cases,
                        ">\n      <failure message=\"{} (missing: {})\" type=\"profile\"/>\n    </testcase>\n",
                        st.verdict.label(),
                        x(&st.missing.join(", ")),
                    );
                }
            }
        }

        format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{:.3}\">\n{}  </testsuite>\n",
            x(name),
            tests,
            failures,
            skipped,
            self.total_elapsed.as_secs_f64(),
            cases,
        )
    }

    /// Render this report as a standalone JUnit XML document (`<?xml?>` +
    /// `<testsuites>` wrapping a single `<testsuite>`) — consumable by any CI or
    /// test-report viewer that ingests JUnit XML.
    pub fn to_junit_xml(&self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n{}</testsuites>\n",
            self.to_junit_testsuite(&self.target),
        )
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
                    let prev_ms = p.elapsed.map(|d| d.as_millis()).unwrap_or(0);
                    let now_ms = c.elapsed.map(|d| d.as_millis()).unwrap_or(0);
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
                c.elapsed.map(|d| d.as_millis()).unwrap_or(0),
                note,
            )?;
        }
        writeln!(f, "\n  Profiles:")?;
        for (name, st) in [
            ("S", &self.profiles.profile_s),
            ("T", &self.profiles.profile_t),
            ("G", &self.profiles.profile_g),
        ] {
            let mut extra = String::new();
            if !st.missing.is_empty() {
                extra.push_str(&format!("  (missing: {})", st.missing.join(", ")));
            }
            if !st.unverified.is_empty() {
                extra.push_str(&format!("  (untested: {})", st.unverified.join(", ")));
            }
            writeln!(f, "    Profile {name}: {}{extra}", st.verdict.label())?;
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
        c.elapsed = Some(Duration::from_millis(elapsed_ms));
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
        let state = |verdict, missing: &[&str]| ProfileState {
            verdict,
            missing: missing.iter().map(|s| s.to_string()).collect(),
            unverified: vec![],
        };
        ProfileAssessment {
            profile_s: state(ProfileVerdict::Conformant, &[]),
            profile_t: state(ProfileVerdict::Conformant, &[]),
            profile_g: state(ProfileVerdict::Unsupported, &["recording"]),
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
            captured: vec![],
        }
    }

    #[test]
    fn junit_xml_maps_statuses_and_escapes() {
        let mut fail = check(
            "get_stream_uri",
            CheckStatus::Fail("boom & <bad>".into()),
            12,
        );
        fail.error = Some(CheckError {
            class: ErrorClass::SoapFault,
            fault_code: Some("SOAP-ENV:Sender".into()),
            subcode: Some("ter:NotAuthorized".into()),
            reason: "boom & <bad>".into(),
            detail: Some("detail\u{0007}text".into()), // includes an illegal XML control char
        });
        let checks = vec![
            check("connect", CheckStatus::Pass, 5),
            fail,
            check("recording", CheckStatus::Skip("not advertised".into()), 0),
            check(
                "get_snapshot_uri",
                CheckStatus::Warn("snapshot fetch: x".into()),
                8,
            ),
        ];
        let xml = report(checks).to_junit_xml();

        // Well-formed skeleton + suite named for the device.
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<testsuites>") && xml.contains("</testsuites>"));
        assert!(xml.contains("name=\"http://test/onvif/device_service\""));
        // 4 checks + 3 profiles = 7; check fail + Profile G unsupported = 2 failures; 1 skip.
        assert!(xml.contains("tests=\"7\""), "{xml}");
        assert!(xml.contains("failures=\"2\""), "{xml}");
        assert!(xml.contains("skipped=\"1\""), "{xml}");
        // Fail carries the ONVIF subcode as `type`; entities escaped; control char dropped.
        assert!(xml.contains("type=\"ter:NotAuthorized\""));
        assert!(xml.contains("boom &amp; &lt;bad&gt;"));
        assert!(!xml.contains('\u{0007}'));
        // Warn is a note, not a failure; skip and profile failure present.
        assert!(xml.contains("<system-out>WARN: snapshot fetch: x</system-out>"));
        assert!(xml.contains("<skipped message=\"not advertised\"/>"));
        assert!(xml.contains("unsupported (missing: recording)"));
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
        assert_eq!(re.checks[0].elapsed, Some(Duration::from_millis(12)));
        assert_eq!(re.total_elapsed, Duration::from_millis(123));
    }

    #[test]
    fn check_status_kind_is_lowercase() {
        let json = serde_json::to_string(&CheckStatus::Fail("x".into())).unwrap();
        assert_eq!(json, r#"{"kind":"fail","reason":"x"}"#);
        let json = serde_json::to_string(&CheckStatus::Pass).unwrap();
        assert_eq!(json, r#"{"kind":"pass"}"#);
    }

    #[test]
    fn elapsed_ms_is_null_for_unrun_check() {
        // Skip-constructed check → no elapsed → serialises as null.
        let skipped = CheckResult::skip("s", Category::Media, "not advertised");
        assert!(skipped.elapsed.is_none());
        let json = serde_json::to_string(&skipped).unwrap();
        assert!(json.contains(r#""elapsed_ms":null"#), "{json}");
        // A timed check serialises an integer, and both round-trip.
        let timed = check("t", CheckStatus::Pass, 5);
        let re: CheckResult =
            serde_json::from_str(&serde_json::to_string(&timed).unwrap()).unwrap();
        assert_eq!(re.elapsed, Some(Duration::from_millis(5)));
        let re: CheckResult = serde_json::from_str(&json).unwrap();
        assert!(re.elapsed.is_none());
    }

    #[test]
    fn profile_state_serialises_as_lowercase_object() {
        // Object shape (not a tuple/array), lowercase verdict, empty lists omitted.
        let st = ProfileState {
            verdict: ProfileVerdict::Conformant,
            missing: vec![],
            unverified: vec![],
        };
        assert_eq!(
            serde_json::to_string(&st).unwrap(),
            r#"{"verdict":"conformant"}"#
        );
        // Inconclusive keeps its unverified list.
        let st = ProfileState {
            verdict: ProfileVerdict::Inconclusive,
            missing: vec![],
            unverified: vec!["get_stream_uri".into()],
        };
        assert_eq!(
            serde_json::to_string(&st).unwrap(),
            r#"{"verdict":"inconclusive","unverified":["get_stream_uri"]}"#
        );
    }
}
