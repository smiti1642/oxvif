//! Fast ONVIF health / conformance check for a camera.
//!
//! ```text
//! cargo run --example healthcheck --features health -- \
//!     http://192.168.1.100/onvif/device_service admin password [--write]
//! ```
//!
//! `--write` enables the opt-in, non-destructive write round-trip check.
//! Exits non-zero if any check failed.

use oxvif::health::HealthCheck;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device_url) = args.next() else {
        eprintln!("usage: healthcheck <device_url> [user] [pass] [--write]");
        std::process::exit(2);
    };

    let rest: Vec<String> = args.collect();
    let write = rest.iter().any(|a| a == "--write");
    let positional: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();

    let mut hc = HealthCheck::new(device_url).with_write_checks(write);
    if let (Some(user), Some(pass)) = (positional.first(), positional.get(1)) {
        hc = hc.with_credentials((*user).clone(), (*pass).clone());
    }

    let report = hc.run().await;
    print!("{report}");
    if !report.ok() {
        std::process::exit(1);
    }
}
