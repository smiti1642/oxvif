//! Clone a real camera into a param-aware metamorph fixture set.
//!
//! Drives the standard read surface against a live device through
//! [`RecordingTransport`], then writes a single `fixtures.json` keyed by the
//! canonical (ephemera-masked) request — so it survives per-request nonce /
//! timestamp jitter and keeps distinct `token=` params apart. Replay the result
//! in-process with [`MetamorphTransport`] (no camera needed).
//!
//! ```text
//! cargo run --example metamorph_record --features metamorph -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     tests/fixtures/hikvision-ds-2cd2085
//! ```

use std::sync::{Arc, Mutex};

use oxvif::OnvifSession;
use oxvif::metamorph::{FixtureStore, RecordingTransport};
use oxvif::transport::{HttpTransport, Transport};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device_url) = args.next() else {
        eprintln!(
            "usage: metamorph_record <device_url> [user] [pass] <out_dir>\n\
             example: metamorph_record http://192.168.1.100/onvif/device_service \
             admin secret tests/fixtures/hikvision-ds-2cd2085"
        );
        std::process::exit(2);
    };

    let rest: Vec<String> = args.collect();
    let Some(out_dir) = rest.last().cloned() else {
        eprintln!("missing <out_dir>");
        std::process::exit(2);
    };
    // Positional args between `device_url` and the trailing `out_dir` are the
    // optional `[user] [pass]`.
    let creds: Vec<&String> = rest[..rest.len().saturating_sub(1)].iter().collect();

    // Label the store by the output directory's final segment (`<vendor>-<model>`).
    let device_label = std::path::Path::new(&out_dir)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("device")
        .to_string();

    let mut http = HttpTransport::new();
    if let (Some(u), Some(p)) = (creds.first(), creds.get(1)) {
        http = http.with_credentials((*u).clone(), (*p).clone());
    }
    let inner: Arc<dyn Transport> = Arc::new(http);
    let store = Arc::new(Mutex::new(FixtureStore::new(device_label)));
    let tap: Arc<dyn Transport> = Arc::new(RecordingTransport::new(inner, store.clone()));

    let mut builder = OnvifSession::builder(&device_url).with_transport(tap);
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

    // Exercise the standard read surface; each call is recorded.
    let _ = session.get_device_info().await;
    let _ = session.get_system_date_and_time().await;
    let _ = session.get_services().await;
    let _ = session.get_hostname().await;
    let profiles = session.get_profiles().await;
    if let Ok(ps) = &profiles {
        // Per-profile reads exercise the param-aware key (token=A vs token=B).
        for p in ps {
            let _ = session.get_stream_uri(&p.token).await;
            let _ = session.get_snapshot_uri(&p.token).await;
        }
    }
    let _ = session.get_video_encoder_configurations().await;
    if let Ok(sources) = session.get_video_sources().await
        && let Some(s) = sources.first()
    {
        let _ = session.get_imaging_settings(&s.token).await;
    }
    let _ = session.ptz_get_nodes().await;
    let _ = session.get_network_interfaces().await;

    let store = store.lock().unwrap();
    if let Err(e) = store.save(&out_dir) {
        eprintln!("failed to write fixtures to {out_dir}: {e}");
        std::process::exit(1);
    }
    println!(
        "recorded {} exchanges to {out_dir}/fixtures.json",
        store.len()
    );
}
