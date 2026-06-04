//! Record a fixture set by exercising the read paths against a real camera.
//!
//! ```text
//! cargo run --example record_fixtures --features "mock health" -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     tests/fixtures/<vendor>-<model>
//! ```
//!
//! Every SOAP request/response pair is written to the output directory as
//! `<action>.req.xml` / `<action>.resp.xml`. The captured set covers the
//! services HealthCheck exercises (Device, Media, Imaging, PTZ, Events,
//! Network, Users) — enough to drive parser tests without the camera.

use std::sync::Arc;

use oxvif::transport::{HttpTransport, Transport};
use oxvif::{CapturingTransport, OnvifSession};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device_url) = args.next() else {
        eprintln!(
            "usage: record_fixtures <device_url> [user] [pass] <out_dir>\n\
             example: record_fixtures http://192.168.1.100/onvif/device_service \
             admin secret tests/fixtures/hikvision-ds-2cd2085"
        );
        std::process::exit(2);
    };

    let rest: Vec<String> = args.collect();
    let Some(out_dir) = rest.last().cloned() else {
        eprintln!("missing <out_dir>");
        std::process::exit(2);
    };
    // Positional args between `device_url` and the trailing `out_dir` are
    // optional `[user] [pass]`.
    let creds: Vec<&String> = rest[..rest.len().saturating_sub(1)].iter().collect();

    // Wrap a real HTTP transport in CapturingTransport so every SOAP exchange
    // lands on disk in addition to flowing through to the device.
    let mut http = HttpTransport::new();
    if let (Some(u), Some(p)) = (creds.first(), creds.get(1)) {
        http = http.with_credentials((*u).clone(), (*p).clone());
    }
    let inner: Arc<dyn Transport> = Arc::new(http);
    let capturing: Arc<dyn Transport> = Arc::new(CapturingTransport::new(inner, &out_dir));

    let mut builder = OnvifSession::builder(&device_url).with_transport(capturing);
    if let (Some(u), Some(p)) = (creds.first(), creds.get(1)) {
        builder = builder.with_credentials((*u).clone(), (*p).clone());
    }
    let session = match builder.build().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("session build failed: {e}");
            std::process::exit(1);
        }
    };

    // Exercise the same read surface HealthCheck does, capturing each call.
    let _ = session.get_device_info().await;
    let _ = session.get_system_date_and_time().await;
    let _ = session.get_services().await;
    let profiles = session.get_profiles().await;
    if let Ok(ps) = &profiles
        && let Some(p) = ps.first()
    {
        let _ = session.get_stream_uri(&p.token).await;
        let _ = session.get_snapshot_uri(&p.token).await;
    }
    let _ = session.get_video_encoder_configurations().await;
    if let Ok(sources) = session.get_video_sources().await
        && let Some(s) = sources.first()
    {
        let _ = session.get_imaging_settings(&s.token).await;
        let _ = session.get_imaging_options(&s.token).await;
    }
    let _ = session.ptz_get_nodes().await;
    let _ = session.get_event_properties().await;
    let _ = session.get_network_interfaces().await;
    let _ = session.get_ntp().await;
    let _ = session.get_dns().await;
    let _ = session.get_users().await;

    println!("fixtures written to {out_dir}");
}
