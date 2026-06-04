//! Fast ONVIF health / conformance check for a camera.
//!
//! ```text
//! cargo run --example healthcheck --features health -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     [--write] [--json | --json-pretty] [--baseline <file.json>]
//! ```
//!
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

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device_url) = args.next() else {
        eprintln!(
            "usage: healthcheck <device_url> [user] [pass] \
             [--write] [--json | --json-pretty] [--baseline <file.json>]"
        );
        std::process::exit(2);
    };

    let rest: Vec<String> = args.collect();
    let write = rest.iter().any(|a| a == "--write");
    let json = rest.iter().any(|a| a == "--json");
    let json_pretty = rest.iter().any(|a| a == "--json-pretty");
    let baseline_path: Option<&String> = rest
        .iter()
        .position(|a| a == "--baseline")
        .and_then(|i| rest.get(i + 1));
    let positional: Vec<&String> = rest
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            if a.starts_with("--") {
                return None;
            }
            // Skip the argument *after* --baseline (it's the path, not a positional).
            if i > 0 && rest[i - 1] == "--baseline" {
                return None;
            }
            Some(a)
        })
        .collect();

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
    if let (Some(user), Some(pass)) = (positional.first(), positional.get(1)) {
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
