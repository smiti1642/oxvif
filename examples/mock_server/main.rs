//! ONVIF mock server — a thin wrapper over [`oxvif::mock::MockServer`] that adds
//! TOML file persistence (state survives restarts, for the OxDM dev workflow).
//!
//! The mock engine itself now lives in the library (`oxvif::mock`); this binary
//! just wires it to a port and a state file. Requires the `mock-server` feature.
//!
//! ```sh
//! # Default: state saved to ~/.oxvif/mock_device.toml, port 18080
//! cargo run --example mock_server --features mock-server
//!
//! # Custom port + config file
//! cargo run --example mock_server --features mock-server -- 19090 --config /path/to/state.toml
//!
//! # Credentials: admin / admin
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs2::FileExt;
use oxvif::mock::{DeviceState, MockServer};

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(18080);
    let path = resolve_state_path();
    let initial = load_state(&path).unwrap_or_default();

    let flush_path = path.clone();
    let server = MockServer::builder()
        .port(port)
        // Auth not enforced — connect credential-free (OxDM workflow). Default
        // users admin/admin & operator/operator still exist if a client sends
        // WS-Security; they're just not required.
        .initial_state(initial)
        .on_change(Arc::new(move |s: &DeviceState| flush_state(&flush_path, s)))
        .start()
        .await
        .expect("bind failed");

    // Write the file once at startup so it exists even before the first Set.
    flush_state(&path, &server.device().read());

    println!("ONVIF mock server listening on {}", server.base_url());
    println!("  ONVIF_URL={}", server.device_url());
    println!("  Auth: not enforced (credentials optional)");
    println!("  State file: {}", path.display());
    println!();
    println!("Press Ctrl-C to stop.");

    // Keep the process (and the background server task) alive until killed.
    std::future::pending::<()>().await;
}

/// Parse `--config <path>` from the CLI, else default to `~/.oxvif/mock_device.toml`.
fn resolve_state_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--config" {
            if let Some(p) = args.get(i + 1) {
                return PathBuf::from(p);
            }
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".oxvif")
        .join("mock_device.toml")
}

/// Load device state from a TOML file, or `None` if absent/unparseable.
fn load_state(path: &Path) -> Option<DeviceState> {
    let content = std::fs::read_to_string(path).ok()?;
    match toml::from_str::<DeviceState>(&content) {
        Ok(s) => {
            eprintln!("  Loaded state from {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!(
                "  [WARN] Failed to parse {}: {e}, using defaults",
                path.display()
            );
            None
        }
    }
}

/// Atomically persist device state to TOML: write a sibling `.tmp` under an
/// exclusive lock, then rename it over the target — a crash never leaves a
/// half-written file.
fn flush_state(path: &Path, state: &DeviceState) {
    let content = match toml::to_string_pretty(state) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  [ERROR] serialize state: {e}");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let tmp_path = {
        let mut name = path
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_else(|| std::ffi::OsString::from("state"));
        name.push(".tmp");
        path.with_file_name(name)
    };

    match std::fs::File::create(&tmp_path) {
        Ok(mut file) => {
            let _ = file.lock_exclusive();
            let write_ok = file.write_all(content.as_bytes()).is_ok() && file.sync_all().is_ok();
            let _ = FileExt::unlock(&file);
            drop(file);
            if write_ok {
                if std::fs::rename(&tmp_path, path).is_err() {
                    let _ = std::fs::remove_file(&tmp_path);
                }
            } else {
                let _ = std::fs::remove_file(&tmp_path);
            }
        }
        Err(e) => eprintln!("  [ERROR] create tempfile {}: {e}", tmp_path.display()),
    }
}
