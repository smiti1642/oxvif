//! Clone a real camera into a param-aware metamorph fixture set.
//!
//! Drives the standard read surface against a live device through
//! [`record_standard_surface`], then writes a single `fixtures.json` keyed by the
//! canonical (ephemera-masked) request — so it survives per-request nonce /
//! timestamp jitter and keeps distinct `token=` params apart. Replay the result
//! in-process with `MetamorphTransport`, or serve it from a bound-port
//! `MockServer` (see `metamorph_serve`).
//!
//! ```text
//! cargo run --example metamorph_record --features metamorph -- \
//!     http://192.168.1.100/onvif/device_service admin password \
//!     tests/fixtures/hikvision-ds-2cd2085
//! ```

use oxvif::metamorph::record_standard_surface;

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

    let store = match record_standard_surface(&device_url, cred_pair, device_label).await {
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
    println!(
        "recorded {} exchanges to {out_dir}/fixtures.json",
        store.len()
    );
}
