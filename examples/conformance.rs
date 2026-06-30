//! Parser conformance check against a fleet of **real** ONVIF devices.
//!
//! The mirror image of the `mock_server` example: where `mock_server` *simulates*
//! a device so you can test oxvif without hardware, `conformance` points oxvif at
//! *real* cameras and reports where its parsers silently drop data — the bug
//! class behind the Profile G recording and ImagingOptions fixes (a parser that
//! looks for the wrong element name returns empty/`None` with no error).
//!
//! ```text
//! cargo run --example conformance --features mock -- [device-list-file]
//! ```
//!
//! The device-list file (default `.cameras`, gitignored — it holds credentials)
//! has one pipe-delimited device per line:
//!
//! ```text
//! # name | device_service_url | user | pass
//! frontdoor | http://192.0.2.10/onvif/device_service | admin | secret
//! ```
//!
//! For each device it runs the read surface through a [`CapturingTransport`],
//! writing every raw SOAP response to `conformance_out/<name>/<Action>.resp.xml`,
//! and prints a parsed summary. Two silent-parse signals stand out:
//!
//! * **list-emptying** — a `… 0 items` count where the raw response clearly
//!   carries items (diff the dump). `HealthCheck` also catches this subclass
//!   automatically via its parse-coverage checks.
//! * **field-defaulting** — an optional field that came back empty/`false` where
//!   the device reported a value, e.g. `imaging_options exp_time=false` when the
//!   `GetOptions` capture contains `Min/MaxExposureTime`. Diff the raw dump
//!   against the parsed struct to confirm.
//!
//! Captures + the device list are gitignored; nothing here is committed.

use std::sync::Arc;
use std::time::Duration;

use oxvif::transport::{HttpTransport, Transport};
use oxvif::{CapturingTransport, OnvifError, OnvifSession};

struct Device {
    name: String,
    url: String,
    user: String,
    pass: String,
}

fn load_devices(path: &str) -> Vec<Device> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read device list {path}: {e}"));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let p: Vec<&str> = l.split('|').map(str::trim).collect();
            (p.len() == 4).then(|| Device {
                name: p[0].into(),
                url: p[1].into(),
                user: p[2].into(),
                pass: p[3].into(),
            })
        })
        .collect()
}

fn report<T: std::fmt::Display>(op: &str, r: Result<T, OnvifError>) {
    match r {
        Ok(v) => println!("  {op:<22} {v}"),
        Err(e) => println!("  {op:<22} ERR: {e}"),
    }
}

#[tokio::main]
async fn main() {
    let list_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".cameras".to_string());
    let devices = load_devices(&list_path);
    println!(
        "# oxvif conformance — {} device(s) from {list_path}\n",
        devices.len()
    );

    for dev in devices {
        let dir = format!("conformance_out/{}", dev.name);
        let http = HttpTransport::new().with_credentials(dev.user.clone(), dev.pass.clone());
        let inner: Arc<dyn Transport> = Arc::new(http);
        let cap: Arc<dyn Transport> = Arc::new(CapturingTransport::new(inner, &dir));

        let built = tokio::time::timeout(
            Duration::from_secs(12),
            OnvifSession::builder(&dev.url)
                .with_transport(cap)
                .with_credentials(dev.user.clone(), dev.pass.clone())
                .build(),
        )
        .await;
        let s = match built {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                println!("## {} — BUILD ERR: {e}\n", dev.name);
                continue;
            }
            Err(_) => {
                println!("## {} — BUILD TIMEOUT\n", dev.name);
                continue;
            }
        };

        println!("## {} ({})", dev.name, dev.url);
        report(
            "device_info",
            s.get_device_info()
                .await
                .map(|i| format!("{} {}", i.manufacturer, i.model)),
        );
        report(
            "profiles",
            s.get_profiles()
                .await
                .map(|v| format!("{} profile(s)", v.len())),
        );
        report(
            "video_encoders",
            s.get_video_encoder_configurations()
                .await
                .map(|v| format!("{} config(s)", v.len())),
        );
        if let Ok(srcs) = s.get_video_sources().await {
            if let Some(src) = srcs.first() {
                report(
                    "imaging_options",
                    s.get_imaging_options(&src.token).await.map(|o| {
                        format!(
                            "exp_time={} gain={} iris={}  (false where the device reports a range = field-defaulting)",
                            o.exposure_time_range.is_some(),
                            o.gain_range.is_some(),
                            o.iris_range.is_some(),
                        )
                    }),
                );
            }
        }
        report(
            "ptz_nodes",
            s.ptz_get_nodes()
                .await
                .map(|v| format!("{} node(s)", v.len())),
        );
        report(
            "recordings",
            s.get_recordings()
                .await
                .map(|v| format!("{} recording(s)", v.len())),
        );
        report(
            "search_recordings",
            s.search_recordings(None)
                .await
                .map(|v| format!("{} found", v.len())),
        );
        report(
            "network_interfaces",
            s.get_network_interfaces()
                .await
                .map(|v| format!("{} interface(s)", v.len())),
        );
        report(
            "users",
            s.get_users().await.map(|v| format!("{} user(s)", v.len())),
        );
        println!();
    }
    println!(
        "raw captures in conformance_out/<name>/  — diff *.resp.xml against the parsed counts above"
    );
}
