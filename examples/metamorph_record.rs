//! Clone a real camera into a param-aware metamorph fixture set.
//!
//! Drives the standard read surface against a live device through
//! `record_surface_with_evidence`, then writes `fixtures.json` keyed by the
//! canonical (ephemera-masked) request — so it survives per-request nonce /
//! timestamp jitter and keeps distinct `token=` params apart. Replay the result
//! in-process with `MetamorphTransport`, or serve it from a bound-port
//! `MockServer` (see `metamorph_serve`).
//!
//! ```text
//! cargo run --example metamorph_record --features metamorph -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     /private/oxvif-captures/camera-a
//! ```

use oxvif::metamorph::{SurfaceSelection, record_surface_with_evidence};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device_url) = args.next() else {
        eprintln!(
            "usage: metamorph_record <device_url> [user] [pass] <out_dir>\n\
             example: metamorph_record http://192.168.1.100/onvif/device_service \
             admin secret /private/oxvif-captures/camera-a"
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
    let cred_pair = match (creds.first(), creds.get(1)) {
        (Some(u), Some(p)) => Some((u.as_str(), p.as_str())),
        _ => None,
    };

    // Label the store by the output directory's final segment (`<vendor>-<model>`).
    let device_label = std::path::Path::new(&out_dir)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("device")
        .to_string();

    let (store, sweep, evidence) = match record_surface_with_evidence(
        &device_url,
        cred_pair,
        device_label,
        &SurfaceSelection::recommended(),
        |progress| {
            eprintln!(
                "resolved operation {:?}: {}/{}",
                progress.op, progress.done, progress.total
            )
        },
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("recording failed: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = store.save(&out_dir) {
        eprintln!("failed to write fixtures to {out_dir}: {e}");
        std::process::exit(1);
    }
    // Review provenance/sharing decisions and independent typed expectations
    // before creating a CorpusManifest; recording outcomes alone cannot pass it.
    let metadata = serde_json::json!({
        "sweep": sweep.entries().into_iter().map(|(operation, outcome)| oxvif::metamorph::SweepObservation { operation, outcome }).collect::<Vec<_>>(),
        "transport": evidence,
        "selected_actions": store.summary().actions.into_iter().map(|entry| entry.action).collect::<Vec<_>>(),
        "manifest_status": "requires reviewed identity, revision, provenance, sharing scope and typed expectations",
    });
    let metadata = match serde_json::to_vec_pretty(&metadata) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("cannot serialize recording outcomes: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = std::fs::write(
        std::path::Path::new(&out_dir).join("recording-outcomes.json"),
        metadata,
    ) {
        eprintln!("cannot save recording outcomes: {error}");
        std::process::exit(1);
    }
    let summary = store.summary();
    println!(
        "recorded {} exchanges across {} Actions to {out_dir}/fixtures.json ({} Faults, {} unreadable, {} collision buckets)",
        summary.fixtures,
        summary.actions.len(),
        summary.faults,
        summary.unreadable,
        summary.collision_buckets,
    );
}
