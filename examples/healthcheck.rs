//! Fast ONVIF health / conformance check for a camera.
//!
//! **No camera? Start here** — this runs a full check against an in-process
//! mock device and prints a real report:
//!
//! ```text
//! cargo run --example healthcheck --features health,mock-server -- --mock
//! ```
//!
//! Against a real camera:
//!
//! ```text
//! cargo run --example healthcheck --features health -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     [--write] [--json | --json-pretty] [--baseline <file.json>]
//! ```
//!
//! `--mock` starts a throwaway `MockServer` on an ephemeral port and checks
//! that instead of a device (needs the `mock-server` feature). Useful to see
//! the report format, and to try `--json` / `--baseline` without hardware.
//! `--write` enables the opt-in, non-destructive write round-trip check.
//! `--json` / `--json-pretty` emit machine-readable output instead of the
//! human-readable table.
//! `--baseline <file.json>` loads a previous JSON report and prints the
//! diff (checks that flipped to FAIL/PASS, added/removed, or slowed).
//!
//! Exits non-zero if any check failed in this run, or if anything flipped
//! to FAIL relative to the baseline.

use std::fs;

use oxvif::health::{HealthCheck, HealthReport};

const USAGE: &str = "usage: healthcheck <device_url> [user] [pass] \
     [--write] [--json | --json-pretty] [--baseline <file.json>]\n\
     \n\
     no camera:  cargo run --example healthcheck --features health,mock-server -- --mock";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let use_mock = args.iter().any(|a| a == "--mock");
    let write = args.iter().any(|a| a == "--write");
    let json = args.iter().any(|a| a == "--json");
    let json_pretty = args.iter().any(|a| a == "--json-pretty");
    let baseline_path: Option<&String> = args
        .iter()
        .position(|a| a == "--baseline")
        .and_then(|i| args.get(i + 1));
    let positional: Vec<&String> = args
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            if a.starts_with("--") {
                return None;
            }
            // Skip the argument *after* --baseline (it's the path, not a positional).
            if i > 0 && args[i - 1] == "--baseline" {
                return None;
            }
            Some(a)
        })
        .collect();

    // `--mock` supplies the target itself. The server must stay bound for the
    // whole run — it shuts down when dropped — so it lives until `main` ends.
    #[cfg(feature = "mock-server")]
    let _mock = if use_mock {
        Some(
            oxvif::mock::MockServer::start()
                .await
                .expect("failed to start the mock server"),
        )
    } else {
        None
    };
    #[cfg(feature = "mock-server")]
    let mock_url: Option<String> = _mock.as_ref().map(|s| s.device_url().to_string());
    #[cfg(not(feature = "mock-server"))]
    let mock_url: Option<String> = None;

    if use_mock && mock_url.is_none() {
        eprintln!(
            "--mock needs the `mock-server` feature:\n  \
             cargo run --example healthcheck --features health,mock-server -- --mock"
        );
        std::process::exit(2);
    }

    // With --mock the target is implicit, so credentials start at positional 0;
    // otherwise positional 0 is the device URL and credentials follow it.
    let creds_at = if mock_url.is_some() { 0 } else { 1 };
    let device_url = match (&mock_url, positional.first()) {
        (Some(u), _) => {
            println!("mock device at {u}\n");
            u.clone()
        }
        (None, Some(u)) => (*u).clone(),
        (None, None) => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };

    let baseline: Option<HealthReport> = match baseline_path {
        Some(p) => match fs::read_to_string(p) {
            Ok(s) => match serde_json::from_str::<HealthReport>(&s) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("failed to parse baseline {p}: {e}");
                    std::process::exit(2);
                }
            },
            Err(e) => {
                eprintln!("failed to read baseline {p}: {e}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    let mut hc = HealthCheck::new(device_url).with_write_checks(write);
    if let (Some(user), Some(pass)) = (positional.get(creds_at), positional.get(creds_at + 1)) {
        hc = hc.with_credentials((*user).clone(), (*pass).clone());
    }

    let report = hc.run().await;

    if json_pretty {
        println!("{}", report.to_json_pretty());
    } else if json {
        println!("{}", report.to_json());
    } else {
        print!("{report}");
        if let Some(prev) = &baseline {
            println!();
            print!("{}", report.diff(prev));
        }
    }

    let flipped_to_fail = baseline
        .as_ref()
        .map(|p| !report.diff(p).flipped_to_fail.is_empty())
        .unwrap_or(false);
    if !report.ok() || flipped_to_fail {
        std::process::exit(1);
    }
}
