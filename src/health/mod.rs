//! Fast, scriptable ONVIF health / conformance check for a target device.
//!
//! A lightweight, readable alternative to the official ONVIF Device Test Tool.
//! Point it at a camera and it runs a curated set of read-only checks
//! concurrently, then reports per-check Pass/Warn/Fail/Skip with timings plus a
//! Profile S/T/G assessment.
//!
//! This module is **additive and feature-gated** (`health`): it builds on
//! [`OnvifSession`] internally but adds no methods to it.
//!
//! ```no_run
//! # async fn run() {
//! use oxvif::health::HealthCheck;
//! let report = HealthCheck::new("http://192.168.1.100/onvif/device_service")
//!     .with_credentials("admin", "password")
//!     .run()
//!     .await;
//! println!("{report}");
//! # }
//! ```

mod checks;
mod coverage;
mod report;

pub use report::{
    Category, CheckError, CheckResult, CheckStatus, ErrorClass, HealthReport, ProfileAssessment,
    ProfileState, ProfileVerdict, ReportDiff, SlowedCheck,
};

use std::time::Instant;

use tokio::task::JoinSet;

use crate::OnvifSession;

/// Builder + runner for a single device health check.
pub struct HealthCheck {
    device_url: String,
    credentials: Option<(String, String)>,
    write_checks: bool,
    liveness_probes: bool,
    clock_sync: bool,
}

impl HealthCheck {
    /// Target a device by its device-service URL.
    pub fn new(device_url: impl Into<String>) -> Self {
        Self {
            device_url: device_url.into(),
            credentials: None,
            write_checks: false,
            liveness_probes: false,
            clock_sync: false,
        }
    }

    /// Supply credentials for WS-Security / HTTP Digest.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Enable the opt-in, non-destructive write round-trip check (re-applies an
    /// unchanged video encoder configuration to exercise the `Set` path).
    pub fn with_write_checks(mut self, enabled: bool) -> Self {
        self.write_checks = enabled;
        self
    }

    /// Enable opt-in active liveness probes that go beyond the SOAP responses:
    /// an RTSP `OPTIONS` reachability probe on the stream URI, a snapshot byte
    /// fetch (validated as a real image, not a 0-byte body or an HTML error
    /// page), and a real Profile G exercise (recording search + replay URI)
    /// instead of advertised-only presence. Off by default because these open
    /// extra network connections (RTSP/TCP and HTTP GET) the read-only SOAP
    /// checks never touch.
    pub fn with_liveness_probes(mut self, enabled: bool) -> Self {
        self.liveness_probes = enabled;
        self
    }

    /// Sync the WS-Security timestamp to the device clock before checks
    /// (mirrors [`OnvifSessionBuilder::with_clock_sync`](crate::OnvifSessionBuilder::with_clock_sync)).
    pub fn with_clock_sync(mut self, enabled: bool) -> Self {
        self.clock_sync = enabled;
        self
    }

    /// Run the checks and produce a [`HealthReport`].
    pub async fn run(self) -> HealthReport {
        let started = Instant::now();

        // 1. Connectivity — build the session (one GetCapabilities round-trip).
        let conn_start = Instant::now();
        // Wrap the HTTP transport in a coverage tap so the parse-coverage check
        // can compare parsed item counts against the raw responses. Credentials
        // go on the transport (HTTP Digest) AND the builder (WS-Security).
        let mut http = crate::transport::HttpTransport::new();
        if let Some((u, p)) = &self.credentials {
            http = http.with_credentials(u.clone(), p.clone());
        }
        let tap = std::sync::Arc::new(coverage::CoverageTransport::new(std::sync::Arc::new(http)));
        let mut builder = OnvifSession::builder(&self.device_url).with_transport(tap.clone());
        if let Some((u, p)) = &self.credentials {
            builder = builder.with_credentials(u.clone(), p.clone());
        }
        if self.clock_sync {
            builder = builder.with_clock_sync();
        }
        let session = match builder.build().await {
            Ok(s) => s,
            Err(e) => {
                // Can't reach / auth the device — report just the failure.
                let conn = CheckResult::fail("connect", Category::Connectivity, e.to_string())
                    .with_elapsed(conn_start.elapsed());
                return HealthReport {
                    target: self.device_url,
                    total_elapsed: started.elapsed(),
                    profiles: assess(std::slice::from_ref(&conn)),
                    checks: vec![conn],
                    clock_skew_s: None,
                    declared_profiles: vec![],
                };
            }
        };
        let mut checks = vec![
            CheckResult::pass("connect", Category::Connectivity, "GetCapabilities ok")
                .with_elapsed(conn_start.elapsed()),
        ];

        // 2. Independent checks, concurrently (session is cheap to clone).
        let mut set: JoinSet<Vec<CheckResult>> = JoinSet::new();
        macro_rules! spawn_check {
            ($f:path) => {{
                let s = session.clone();
                set.spawn(async move { $f(&s).await });
            }};
        }
        spawn_check!(checks::device_info);
        spawn_check!(checks::time);
        spawn_check!(checks::services);
        spawn_check!(checks::imaging);
        spawn_check!(checks::ptz);
        spawn_check!(checks::events);
        spawn_check!(checks::network);
        spawn_check!(checks::users);
        // Media (stream/snapshot) and Profile G take the liveness flag; media
        // also needs the credentials for an authenticated snapshot GET.
        {
            let s = session.clone();
            let liveness = self.liveness_probes;
            let creds = self.credentials.clone();
            set.spawn(async move { checks::media(&s, liveness, creds.as_ref()).await });
        }
        {
            let s = session.clone();
            let liveness = self.liveness_probes;
            set.spawn(async move { checks::recording_services(&s, liveness).await });
        }
        // Negative auth-enforcement probe — opens its own credential-free
        // session, so it takes the device URL rather than the authed session.
        {
            let url = self.device_url.clone();
            let had_creds = self.credentials.is_some();
            set.spawn(async move { checks::auth_enforcement(&url, had_creds).await });
        }
        if self.write_checks {
            spawn_check!(checks::write_roundtrip);
        }
        // Parse-coverage runs over the same tap (re-calls list ops; each
        // comparison uses its own call's raw, so it is race-free).
        {
            let s = session.clone();
            let tap = tap.clone();
            set.spawn(async move { coverage::coverage(&s, &tap).await });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(mut v) => checks.append(&mut v),
                Err(e) => checks.push(CheckResult::fail(
                    "internal",
                    Category::Connectivity,
                    format!("check task panicked: {e}"),
                )),
            }
        }

        // 3. Stable ordering for the report.
        checks.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.id.cmp(&b.id)));

        let profiles = assess(&checks);
        let clock_skew_s = checks
            .iter()
            .find(|c| c.id == "system_date_time")
            .and_then(|c| checks::parse_skew(&c.detail));
        // Vendor-declared profiles from scopes (best-effort; the health check is
        // infallible, so a scopes failure just leaves this empty).
        let declared_profiles = session
            .get_scopes()
            .await
            .ok()
            .map(|sc| declared_profiles_from_scopes(&sc))
            .unwrap_or_default();
        HealthReport {
            target: self.device_url,
            total_elapsed: started.elapsed(),
            checks,
            profiles,
            clock_skew_s,
            declared_profiles,
        }
    }
}

/// Extract the ONVIF profiles a device self-declares from its scope URIs.
///
/// Scope form: `onvif://www.onvif.org/Profile/<X>`, where `<X>` is `Streaming`
/// (Profile S) or a single letter (`G`, `T`, `M`, `A`, `C`, `D`, `K`). Returns
/// canonical letters, deduped and ordered `[S, T, G, M, A, C, D, K]`; unknown
/// tokens are dropped.
fn declared_profiles_from_scopes(scopes: &[String]) -> Vec<String> {
    const ORDER: [&str; 8] = ["S", "T", "G", "M", "A", "C", "D", "K"];
    let mut found = std::collections::HashSet::new();
    for scope in scopes {
        let Some((_, rest)) = scope.split_once("/Profile/") else {
            continue;
        };
        let tok = rest.split('/').next().unwrap_or("");
        let letter: Option<&'static str> = match tok.to_ascii_uppercase().as_str() {
            "STREAMING" | "S" => Some("S"),
            "T" => Some("T"),
            "G" => Some("G"),
            "M" => Some("M"),
            "A" => Some("A"),
            "C" => Some("C"),
            "D" => Some("D"),
            "K" => Some("K"),
            _ => None,
        };
        if let Some(l) = letter {
            found.insert(l);
        }
    }
    ORDER
        .iter()
        .filter(|l| found.contains(*l))
        .map(|l| l.to_string())
        .collect()
}

#[cfg(all(test, feature = "mock-server"))]
fn check_passed(checks: &[CheckResult], id: &str) -> bool {
    checks
        .iter()
        .any(|c| c.id == id && matches!(c.status, CheckStatus::Pass | CheckStatus::Warn(_)))
}

/// Why a required check did not contribute a pass to a profile.
enum ReqOutcome {
    Passed,
    /// Verified to genuinely fail (a real device fault).
    FailedVerified,
    /// Could not be tested — skipped, or failed only because auth was blocked.
    Unverifiable,
}

fn classify_required(checks: &[CheckResult], id: &str) -> ReqOutcome {
    match checks.iter().find(|c| c.id == id) {
        Some(c) => match &c.status {
            CheckStatus::Pass | CheckStatus::Warn(_) => ReqOutcome::Passed,
            CheckStatus::Skip(_) => ReqOutcome::Unverifiable,
            CheckStatus::Fail(_) => {
                if c.error.as_ref().is_some_and(CheckError::is_auth) {
                    ReqOutcome::Unverifiable
                } else {
                    ReqOutcome::FailedVerified
                }
            }
        },
        // Expected but absent from the report → treat as a genuine gap.
        None => ReqOutcome::FailedVerified,
    }
}

fn verdict(checks: &[CheckResult], required: &[&'static str]) -> ProfileState {
    let mut missing = Vec::new();
    let mut unverified = Vec::new();
    let mut passed = 0usize;
    for id in required {
        match classify_required(checks, id) {
            ReqOutcome::Passed => passed += 1,
            ReqOutcome::FailedVerified => missing.push((*id).to_string()),
            ReqOutcome::Unverifiable => unverified.push((*id).to_string()),
        }
    }
    let verdict = if missing.is_empty() && unverified.is_empty() {
        ProfileVerdict::Conformant
    } else if !missing.is_empty() && passed > 0 {
        ProfileVerdict::Partial
    } else if missing.is_empty() {
        // Nothing genuinely failed, but some required checks couldn't be tested.
        ProfileVerdict::Inconclusive
    } else {
        // Genuine failures and nothing passed.
        ProfileVerdict::Unsupported
    };
    ProfileState {
        verdict,
        missing,
        unverified,
    }
}

fn assess(checks: &[CheckResult]) -> ProfileAssessment {
    ProfileAssessment {
        profile_s: verdict(
            checks,
            &[
                "connect",
                "get_services",
                "get_profiles",
                "get_stream_uri",
                "get_snapshot_uri",
                "get_video_encoder_configurations",
            ],
        ),
        profile_t: verdict(
            checks,
            &[
                "connect",
                "get_profiles",
                "get_stream_uri",
                "media2",
                "get_imaging_settings",
                "get_event_properties",
                "event_motion_topic",
            ],
        ),
        // Profile G (recording/search/replay) — advertised-only by default, or
        // genuinely exercised when liveness probing is on; see
        // `checks::recording_services`.
        profile_g: verdict(checks, &["recording", "search", "replay"]),
    }
}

#[cfg(test)]
mod declared_tests {
    use super::declared_profiles_from_scopes;

    #[test]
    fn parses_and_canonicalizes_profile_scopes() {
        let scopes = vec![
            "onvif://www.onvif.org/name/Camera1".to_string(),
            "onvif://www.onvif.org/Profile/G".to_string(),
            "onvif://www.onvif.org/Profile/Streaming".to_string(), // Profile S
            "onvif://www.onvif.org/Profile/M".to_string(),
            "onvif://www.onvif.org/Profile/G".to_string(), // dupe
            "onvif://www.onvif.org/Profile/Zzz".to_string(), // unknown → dropped
        ];
        // Canonical order [S, T, G, M, A, C, D, K]; deduped; unknown dropped.
        assert_eq!(declared_profiles_from_scopes(&scopes), ["S", "G", "M"]);
    }

    #[test]
    fn empty_when_no_profile_scopes() {
        let scopes = vec!["onvif://www.onvif.org/location/Lobby".to_string()];
        assert!(declared_profiles_from_scopes(&scopes).is_empty());
        assert!(declared_profiles_from_scopes(&[]).is_empty());
    }
}

#[cfg(test)]
mod verdict_tests {
    use super::*;

    fn chk(id: &str, status: CheckStatus, error: Option<CheckError>) -> CheckResult {
        CheckResult {
            id: id.into(),
            category: Category::Media,
            status,
            detail: String::new(),
            error,
            elapsed: None,
        }
    }
    fn soap(class: ErrorClass, subcode: &str, reason: &str) -> CheckError {
        CheckError {
            class,
            fault_code: None,
            subcode: (!subcode.is_empty()).then(|| subcode.to_string()),
            reason: reason.into(),
            detail: None,
        }
    }

    #[test]
    fn verdict_splits_auth_from_genuine_failure() {
        use ProfileVerdict::*;
        let auth = || {
            Some(soap(
                ErrorClass::SoapFault,
                "ter:NotAuthorized",
                "sender not authorized",
            ))
        };
        let parse = || Some(soap(ErrorClass::Parse, "", "bad xml"));

        // Something passed, the rest was auth-blocked → Inconclusive (unknown),
        // NOT Partial; the blocked id lands in `unverified`, not `missing`.
        let st = verdict(
            &[
                chk("a", CheckStatus::Pass, None),
                chk("b", CheckStatus::Fail("x".into()), auth()),
            ],
            &["a", "b"],
        );
        assert_eq!(st.verdict, Inconclusive);
        assert_eq!(st.unverified, ["b"]);
        assert!(st.missing.is_empty());

        // A genuine (non-auth) failure alongside a pass → Partial, id in `missing`.
        let st = verdict(
            &[
                chk("a", CheckStatus::Pass, None),
                chk("b", CheckStatus::Fail("x".into()), parse()),
            ],
            &["a", "b"],
        );
        assert_eq!(st.verdict, Partial);
        assert_eq!(st.missing, ["b"]);
        assert!(st.unverified.is_empty());

        // All required pass → Conformant.
        assert_eq!(
            verdict(&[chk("a", CheckStatus::Pass, None)], &["a"]).verdict,
            Conformant
        );
        // Genuine failures, nothing passed → Unsupported.
        assert_eq!(
            verdict(&[chk("a", CheckStatus::Fail("x".into()), parse())], &["a"]).verdict,
            Unsupported
        );
    }
}

#[cfg(all(test, feature = "mock-server"))]
mod tests {
    use super::*;
    use crate::mock::MockServer;

    #[tokio::test]
    async fn healthcheck_against_mock_passes_core() {
        let server = MockServer::start().await.unwrap();
        let report = HealthCheck::new(server.device_url()).run().await;

        assert!(report.ok(), "mock health check had failures:\n{report}");
        assert!(check_passed(&report.checks, "connect"));
        assert!(check_passed(&report.checks, "get_profiles"));
        assert!(check_passed(&report.checks, "get_users"));
        // The mock advertises Media/Imaging/PTZ/Events, so Profile S/T should
        // not come back Unsupported.
        assert_ne!(
            report.profiles.profile_s.verdict,
            ProfileVerdict::Unsupported
        );
        // Parse-coverage must not false-positive on the compliant mock: the
        // parser's item counts match the raw response item counts.
        assert!(
            !report
                .checks
                .iter()
                .any(|c| c.category == Category::Coverage
                    && matches!(c.status, CheckStatus::Warn(_))),
            "unexpected parse-coverage warning on the mock:\n{report}"
        );
    }

    #[tokio::test]
    async fn liveness_probes_exercise_profile_g_and_fetch_snapshot() {
        let server = MockServer::start().await.unwrap();
        let report = HealthCheck::new(server.device_url())
            .with_liveness_probes(true)
            .run()
            .await;

        let detail = |id: &str| {
            report
                .checks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.detail.clone())
                .unwrap_or_default()
        };

        // Snapshot was actually fetched and validated as a real image (the mock
        // serves a test-pattern JPEG at /mock/snapshot.jpg).
        assert!(
            check_passed(&report.checks, "get_snapshot_uri"),
            "snapshot check should pass:\n{report}"
        );
        assert!(
            detail("get_snapshot_uri").contains("image"),
            "snapshot detail should note a validated image, got: {:?}",
            detail("get_snapshot_uri")
        );

        // Profile G is now genuinely exercised, not advertised-only: the mock
        // answers FindRecordings / GetReplayUri / GetRecordings, so all three
        // pass and the detail no longer carries the "(not exercised)" marker.
        for id in ["recording", "search", "replay"] {
            assert!(
                check_passed(&report.checks, id),
                "{id} should pass when exercised:\n{report}"
            );
            assert!(
                !detail(id).contains("not exercised"),
                "{id} should be exercised, not presence-only"
            );
        }
        assert_eq!(
            report.profiles.profile_g.verdict,
            ProfileVerdict::Conformant,
            "exercised Profile G should be Conformant against the mock:\n{report}"
        );
    }

    #[tokio::test]
    async fn profile_t_gates_on_media2_and_motion_topic() {
        let server = MockServer::start().await.unwrap();
        let report = HealthCheck::new(server.device_url()).run().await;

        // The mock advertises Media2 and a motion-alarm topic, so both
        // Profile-T-defining checks pass and Profile T is Conformant.
        assert!(
            check_passed(&report.checks, "media2"),
            "mock advertises Media2:\n{report}"
        );
        assert!(
            check_passed(&report.checks, "event_motion_topic"),
            "mock exposes a motion-alarm topic:\n{report}"
        );
        assert_eq!(
            report.profiles.profile_t.verdict,
            ProfileVerdict::Conformant,
            "exercised Profile T should be Conformant against the mock:\n{report}"
        );
    }

    #[tokio::test]
    async fn negative_auth_probe_flags_and_confirms_enforcement() {
        let status_of = |report: &HealthReport| {
            report
                .checks
                .iter()
                .find(|c| c.id == "auth_enforcement")
                .expect("auth_enforcement check present")
                .status
                .clone()
        };

        // Auth OFF + credentials supplied → the device serves GetDeviceInformation
        // anonymously → a security Warn.
        let open = MockServer::start().await.unwrap();
        let report = HealthCheck::new(open.device_url())
            .with_credentials("admin", "admin")
            .run()
            .await;
        assert!(
            matches!(status_of(&report), CheckStatus::Warn(_)),
            "auth-off device should warn:\n{report}"
        );

        // No credentials → nothing to compare against → Skip.
        let report = HealthCheck::new(open.device_url()).run().await;
        assert!(
            matches!(status_of(&report), CheckStatus::Skip(_)),
            "no credentials → skip:\n{report}"
        );

        // Auth ENFORCED + valid credentials → the anonymous read is rejected → Pass.
        let locked = MockServer::builder()
            .enforce_auth(true)
            .start()
            .await
            .unwrap();
        let report = HealthCheck::new(locked.device_url())
            .with_credentials("admin", "admin")
            .run()
            .await;
        assert!(
            matches!(status_of(&report), CheckStatus::Pass),
            "auth-enforced device should pass:\n{report}"
        );
    }

    #[tokio::test]
    async fn healthcheck_write_roundtrip_against_mock() {
        let server = MockServer::start().await.unwrap();
        let report = HealthCheck::new(server.device_url())
            .with_write_checks(true)
            .run()
            .await;
        assert!(
            check_passed(&report.checks, "set_video_encoder_roundtrip"),
            "write round-trip should pass against the mock:\n{report}"
        );
    }

    #[tokio::test]
    async fn healthcheck_report_to_json_is_valid_and_round_trips() {
        let server = MockServer::start().await.unwrap();
        let report = HealthCheck::new(server.device_url()).run().await;

        // Compact and pretty are both valid JSON and equivalent values.
        let compact: serde_json::Value =
            serde_json::from_str(&report.to_json()).expect("compact JSON parses");
        let pretty: serde_json::Value =
            serde_json::from_str(&report.to_json_pretty()).expect("pretty JSON parses");
        assert_eq!(compact, pretty);

        // Core fields are present and durations are integer-millisecond.
        assert!(compact.get("target").and_then(|v| v.as_str()).is_some());
        assert!(
            compact
                .get("total_elapsed_ms")
                .and_then(|v| v.as_u64())
                .is_some()
        );
        let checks = compact
            .get("checks")
            .and_then(|v| v.as_array())
            .expect("checks array");
        let first = &checks[0];
        assert!(first.get("id").and_then(|v| v.as_str()).is_some());
        assert!(first.get("elapsed_ms").and_then(|v| v.as_u64()).is_some());
        // CheckStatus is tagged: { "kind": "Pass" } or { "kind": "Fail", "reason": "..." }
        let status = first.get("status").expect("status field");
        assert!(status.get("kind").and_then(|v| v.as_str()).is_some());
    }
}
