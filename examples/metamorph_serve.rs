//! Serve a recorded camera clone from a bound-port `MockServer` (the
//! "container") and print a structural quirk diff of the clone vs oxvif's
//! synthetic (spec-ideal) mock.
//!
//! ```text
//! cargo run --example metamorph_serve --features metamorph-server -- \
//!     tests/fixtures/hikvision-ds-2cd2085
//! ```
//!
//! Then point any ONVIF client (oxdm, ODM, Frigate) — or oxvif's own
//! `HealthCheck` — at the printed device URL to drive the cloned camera.

use oxvif::metamorph::FixtureStore;
use oxvif::mock::MockServer;

#[tokio::main]
async fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("usage: metamorph_serve <fixtures_dir>");
        std::process::exit(2);
    };

    let store = match FixtureStore::load(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to load {dir}/fixtures.json: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "loaded clone '{}' ({} exchanges)",
        store.device(),
        store.len()
    );

    // Structural quirk diff vs the synthetic baseline (structure only — see
    // `FixtureStore::diff_against_synthetic`).
    let report = store.diff_against_synthetic();
    if report.is_empty() {
        println!("quirk diff: no structural drift vs the synthetic baseline");
    } else {
        println!(
            "quirk diff: {} operation(s) deviate structurally:",
            report.quirks.len()
        );
        for q in &report.quirks {
            let op = q.action.rsplit('/').next().unwrap_or(&q.action);
            if !q.only_in_clone.is_empty() {
                println!("  {op}  + clone-only: {:?}", q.only_in_clone);
            }
            if !q.only_in_synthetic.is_empty() {
                println!("  {op}  - missing vs baseline: {:?}", q.only_in_synthetic);
            }
        }
    }

    // Serve the clone. Any HTTP ONVIF client can now drive it.
    let server = match MockServer::builder().replay(store).start().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to start server: {e}");
            std::process::exit(1);
        }
    };
    println!("\nserving clone at {}", server.device_url());
    println!("point a client / HealthCheck here; Ctrl-C to stop.");

    // Park until the process is killed; the server shuts down on drop.
    std::future::pending::<()>().await;
}
