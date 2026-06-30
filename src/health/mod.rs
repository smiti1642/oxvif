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
    Category, CheckResult, CheckStatus, HealthReport, ProfileAssessment, ProfileVerdict,
    ReportDiff, SlowedCheck,
};

use std::time::Instant;

use tokio::task::JoinSet;

use crate::OnvifSession;

/// Builder + runner for a single device health check.
pub struct HealthCheck {
    device_url: String,
    credentials: Option<(String, String)>,
    write_checks: bool,
    clock_sync: bool,
}

impl HealthCheck {
    /// Target a device by its device-service URL.
    pub fn new(device_url: impl Into<String>) -> Self {
        Self {
            device_url: device_url.into(),
            credentials: None,
            write_checks: false,
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
        spawn_check!(checks::recording_services);
        spawn_check!(checks::media);
        spawn_check!(checks::imaging);
        spawn_check!(checks::ptz);
        spawn_check!(checks::events);
        spawn_check!(checks::network);
        spawn_check!(checks::users);
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
        HealthReport {
            target: self.device_url,
            total_elapsed: started.elapsed(),
            checks,
            profiles,
        }
    }
}

fn check_passed(checks: &[CheckResult], id: &str) -> bool {
    checks
        .iter()
        .any(|c| c.id == id && matches!(c.status, CheckStatus::Pass | CheckStatus::Warn(_)))
}

fn verdict(checks: &[CheckResult], required: &[&'static str]) -> (ProfileVerdict, Vec<String>) {
    let missing: Vec<String> = required
        .iter()
        .copied()
        .filter(|id| !check_passed(checks, id))
        .map(String::from)
        .collect();
    let v = if missing.is_empty() {
        ProfileVerdict::Conformant
    } else if missing.len() < required.len() {
        ProfileVerdict::Partial
    } else {
        ProfileVerdict::Unsupported
    };
    (v, missing)
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
                "get_imaging_settings",
                "get_event_properties",
            ],
        ),
        // Profile G presence (recording/search/replay) — advertised, not
        // exercised; see `checks::recording_services`.
        profile_g: verdict(checks, &["recording", "search", "replay"]),
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
        assert_ne!(report.profiles.profile_s.0, ProfileVerdict::Unsupported);
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
