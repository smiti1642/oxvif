//! oxvif — usage examples
//!
//! Copy `.env.example` to `.env`, fill in your camera details, then run:
//!
//! ```sh
//! cargo run -- full-workflow
//! cargo run -- device-info
//! cargo run -- stream-uris
//! cargo run -- snapshot-uris
//! cargo run -- system-datetime
//! cargo run -- ptz-presets
//! cargo run -- error-handling
//! ```

use oxvif::{
    Capabilities, DeviceInfo, MediaProfile, OnvifClient, OnvifError, StreamUri,
    SystemDateTime,
};
use std::env;

// ── Configuration ─────────────────────────────────────────────────────────────

struct Config {
    camera_url: String,
    username: String,
    password: String,
}

impl Config {
    fn from_env() -> Self {
        // Load .env file if present; ignore error if it doesn't exist.
        let _ = dotenvy::dotenv();

        let camera_url = env::var("ONVIF_URL").expect(
            "ONVIF_URL is not set. Copy .env.example to .env and fill in your camera details.",
        );
        let username = env::var("ONVIF_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let password = env::var("ONVIF_PASSWORD").unwrap_or_else(|_| String::new());

        Self {
            camera_url,
            username,
            password,
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let example = env::args().nth(1).unwrap_or_else(|| "help".to_string());

    let cfg = Config::from_env();

    let result = match example.as_str() {
        "full-workflow" => full_workflow(&cfg).await,
        "device-info" => device_info_example(&cfg).await,
        "stream-uris" => stream_uris(&cfg).await,
        "snapshot-uris" => snapshot_uris(&cfg).await,
        "system-datetime" => system_datetime(&cfg).await,
        "ptz-presets" => ptz_presets(&cfg).await,
        "error-handling" => error_handling_example(&cfg).await,
        _ => {
            print_help();
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!("oxvif examples");
    println!();
    println!("USAGE:");
    println!("  cargo run -- <example>");
    println!();
    println!("EXAMPLES:");
    println!("  full-workflow    Capabilities → profiles → RTSP URIs");
    println!("  device-info      Manufacturer, model, firmware version");
    println!("  stream-uris      Tabular RTSP URI listing");
    println!("  snapshot-uris    Tabular HTTP snapshot URI listing");
    println!("  system-datetime  Device clock and UTC offset");
    println!("  ptz-presets      List all PTZ presets (requires PTZ camera)");
    println!("  error-handling   Typed error variant matching");
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn print_capabilities(caps: &Capabilities) {
    println!("\nCapabilities:");
    print_optional("  Device   ", &caps.device.url);
    print_optional("  Media    ", &caps.media.url);
    print_optional("  PTZ      ", &caps.ptz_url);
    print_optional("  Events   ", &caps.events.url);
    print_optional("  Imaging  ", &caps.imaging_url);
    print_optional("  Analytics", &caps.analytics.url);
    if caps.media.streaming.rtp_rtsp_tcp {
        println!("  Streaming : RTSP/TCP");
    }
    if caps.media.streaming.rtp_multicast {
        println!("  Streaming : RTP Multicast");
    }
    if let Some(n) = caps.media.max_profiles {
        println!("  Max profiles: {n}");
    }
    if caps.events.ws_pull_point {
        println!("  Events    : WS-PullPoint");
    }
    if caps.device.security.username_token {
        println!("  Auth      : UsernameToken");
    }
    if caps.device.system.firmware_upgrade {
        println!("  System    : firmware upgrade supported");
    }
}

fn print_optional(label: &str, value: &Option<String>) {
    match value {
        Some(v) => println!("{label}: {v}"),
        None => println!("{label}: (not supported)"),
    }
}

// Retrieve capabilities and apply the device clock offset.
// Returns (client_with_offset, capabilities).
async fn connect(cfg: &Config) -> Result<(OnvifClient, Capabilities), OnvifError> {
    // Step 1 — get clock without credentials (usually allowed unauthenticated)
    let base = OnvifClient::new(&cfg.camera_url);
    let utc_offset = match base.get_system_date_and_time().await {
        Ok(dt) => dt.utc_offset_secs(),
        Err(_) => 0,
    };

    // Step 2 — build authenticated client with corrected timestamp
    let client = OnvifClient::new(&cfg.camera_url)
        .with_credentials(&cfg.username, &cfg.password)
        .with_utc_offset(utc_offset);

    let caps = client.get_capabilities().await?;
    Ok((client, caps))
}

// ── Example 1: full workflow ──────────────────────────────────────────────────

/// End-to-end flow:
///   1. Sync device clock (unauthenticated GetSystemDateAndTime)
///   2. Fetch capabilities to discover service URLs
///   3. List media profiles
///   4. Fetch RTSP stream URI for each profile
async fn full_workflow(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Full workflow ===");
    println!("Connecting to {}", cfg.camera_url);

    let (client, caps) = connect(cfg).await?;
    print_capabilities(&caps);

    let media_url = match &caps.media.url {
        Some(u) => u.clone(),
        None => {
            println!("Device does not advertise a Media service — stopping.");
            return Ok(());
        }
    };

    let profiles: Vec<MediaProfile> = client.get_profiles(&media_url).await?;
    println!("\nFound {} profile(s):", profiles.len());
    for p in &profiles {
        println!(
            "  [{token}] {name}  (fixed={fixed})",
            token = p.token,
            name = p.name,
            fixed = p.fixed,
        );
    }

    println!();
    for profile in &profiles {
        let uri: StreamUri = client.get_stream_uri(&media_url, &profile.token).await?;
        println!("Profile '{}' → {}", profile.name, uri.uri);
        if uri.invalid_after_connect {
            println!("  (URI expires after first RTSP connection)");
        }
        if !uri.timeout.is_empty() && uri.timeout != "PT0S" {
            println!("  (URI timeout: {})", uri.timeout);
        }
    }

    Ok(())
}

// ── Example 2: device info ────────────────────────────────────────────────────

/// Many cameras expose device information without authentication.
/// This example tries unauthenticated first and falls back to credentials.
async fn device_info_example(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Device information ===");

    let info: DeviceInfo = match OnvifClient::new(&cfg.camera_url).get_device_info().await {
        Ok(info) => info,
        Err(_) => {
            println!("(unauthenticated request failed — retrying with credentials)");
            OnvifClient::new(&cfg.camera_url)
                .with_credentials(&cfg.username, &cfg.password)
                .get_device_info()
                .await?
        }
    };

    println!("Manufacturer : {}", info.manufacturer);
    println!("Model        : {}", info.model);
    println!("Firmware     : {}", info.firmware_version);
    println!("Serial       : {}", info.serial_number);
    println!("Hardware ID  : {}", info.hardware_id);

    Ok(())
}

// ── Example 3: stream URIs ────────────────────────────────────────────────────

/// Lists every media profile together with its RTSP URI.
async fn stream_uris(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Stream URIs ===");

    let (client, caps) = connect(cfg).await?;
    let media_url = caps
        .media
        .url
        .ok_or_else(|| oxvif::soap::SoapError::missing("Media service not found"))?;

    let profiles = client.get_profiles(&media_url).await?;

    if profiles.is_empty() {
        println!("No media profiles found.");
        return Ok(());
    }

    println!("{:<20} RTSP URI", "Profile");
    println!("{}", "-".repeat(80));

    for profile in &profiles {
        match client.get_stream_uri(&media_url, &profile.token).await {
            Ok(uri) => println!("{:<20} {}", profile.name, uri.uri),
            Err(e) => println!("{:<20} ERROR: {e}", profile.name),
        }
    }

    Ok(())
}

// ── Example 4: snapshot URIs ──────────────────────────────────────────────────

/// Lists HTTP snapshot URIs for every media profile.
/// Fetch any of these with curl or a browser to get a JPEG still image.
async fn snapshot_uris(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Snapshot URIs ===");

    let (client, caps) = connect(cfg).await?;
    let media_url = caps
        .media
        .url
        .ok_or_else(|| oxvif::soap::SoapError::missing("Media service not found"))?;

    let profiles = client.get_profiles(&media_url).await?;

    if profiles.is_empty() {
        println!("No media profiles found.");
        return Ok(());
    }

    println!("{:<20} Snapshot URI", "Profile");
    println!("{}", "-".repeat(80));

    for profile in &profiles {
        match client.get_snapshot_uri(&media_url, &profile.token).await {
            Ok(snap) => {
                let flags = match (snap.invalid_after_connect, snap.invalid_after_reboot) {
                    (true, _) => " [one-time]",
                    (_, true) => " [reboot-reset]",
                    _ => "",
                };
                println!("{:<20} {}{}", profile.name, snap.uri, flags);
            }
            Err(e) => println!("{:<20} ERROR: {e}", profile.name),
        }
    }

    Ok(())
}

// ── Example 5: system date and time ──────────────────────────────────────────

/// Prints the device clock, timezone, and the UTC offset used for WS-Security.
async fn system_datetime(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== System date and time ===");

    // GetSystemDateAndTime is typically accessible without credentials.
    let client = OnvifClient::new(&cfg.camera_url);
    let dt: SystemDateTime = match client.get_system_date_and_time().await {
        Ok(dt) => dt,
        Err(_) => {
            println!("(unauthenticated request failed — retrying with credentials)");
            OnvifClient::new(&cfg.camera_url)
                .with_credentials(&cfg.username, &cfg.password)
                .get_system_date_and_time()
                .await?
        }
    };

    match dt.utc_unix {
        Some(unix) => {
            println!("Device UTC   : Unix timestamp {unix}");
            // Render as ISO 8601 using chrono if available; here we format manually.
            let secs = unix % 60;
            let mins = (unix / 60) % 60;
            let hours = (unix / 3600) % 24;
            let days = unix / 86_400;
            println!("             : ~{days} days since epoch, {hours:02}:{mins:02}:{secs:02} UTC");
        }
        None => println!("Device UTC   : (not returned by device)"),
    }

    let tz = if dt.timezone.is_empty() {
        "(none)".to_string()
    } else {
        dt.timezone.clone()
    };
    println!("Timezone     : {tz}");
    println!("DST active   : {}", dt.daylight_savings);

    let offset = dt.utc_offset_secs();
    println!("UTC offset   : {offset:+} seconds (device − local)");
    if offset.abs() > 5 {
        println!(
            "  ⚠  Clock skew detected. Use .with_utc_offset({offset}) to keep WS-Security valid."
        );
    } else {
        println!("  Clocks are in sync — no offset needed.");
    }

    Ok(())
}

// ── Example 6: PTZ presets ────────────────────────────────────────────────────

/// Lists all PTZ presets for every media profile that supports PTZ.
/// Requires a camera with PTZ capability.
async fn ptz_presets(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== PTZ presets ===");

    let (client, caps) = connect(cfg).await?;

    let ptz_url = match &caps.ptz_url {
        Some(u) => u.clone(),
        None => {
            println!("Device does not advertise a PTZ service.");
            return Ok(());
        }
    };
    println!("PTZ service: {ptz_url}");

    let media_url = match &caps.media.url {
        Some(u) => u.clone(),
        None => {
            println!("No media service — cannot list profiles.");
            return Ok(());
        }
    };

    let profiles = client.get_profiles(&media_url).await?;
    if profiles.is_empty() {
        println!("No media profiles found.");
        return Ok(());
    }

    for profile in &profiles {
        println!("\nProfile '{}' (token: {}):", profile.name, profile.token);

        match client.ptz_get_presets(&ptz_url, &profile.token).await {
            Ok(presets) if presets.is_empty() => {
                println!("  (no presets saved)");
            }
            Ok(presets) => {
                println!(
                    "  {:<8} {:<20} {:>12}  {:>8}",
                    "Token", "Name", "Pan/Tilt", "Zoom"
                );
                println!("  {}", "-".repeat(52));
                for p in &presets {
                    let pt = match p.pan_tilt {
                        Some((x, y)) => format!("{x:+.3}/{y:+.3}"),
                        None => "—".to_string(),
                    };
                    let z = match p.zoom {
                        Some(z) => format!("{z:.3}"),
                        None => "—".to_string(),
                    };
                    println!("  {:<8} {:<20} {:>12}  {:>8}", p.token, p.name, pt, z);
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    }

    Ok(())
}

// ── Example 7: error handling ─────────────────────────────────────────────────

/// Demonstrates how to match on typed OnvifError variants.
async fn error_handling_example(cfg: &Config) -> Result<(), OnvifError> {
    use oxvif::error::OnvifError as Err_;
    use oxvif::soap::SoapError;
    use oxvif::transport::TransportError;

    println!("=== Error handling ===");
    println!("Attempting connection to {} ...", cfg.camera_url);

    let client = OnvifClient::new(&cfg.camera_url).with_credentials(&cfg.username, &cfg.password);

    match client.get_capabilities().await {
        Ok(caps) => {
            println!("Connected successfully.");
            print_capabilities(&caps);
        }

        // Network / TLS / timeout — underlying reqwest error
        Err(Err_::Transport(TransportError::Http(e))) => {
            eprintln!("Network error: {e}");
            eprintln!("Check that the camera is reachable at {}", cfg.camera_url);
        }

        // Unexpected HTTP status (e.g. 401 Unauthorized, 403 Forbidden)
        Err(Err_::Transport(TransportError::HttpStatus { status, body })) => {
            eprintln!("HTTP {status} from device");
            if !body.is_empty() {
                eprintln!("Body: {body}");
            }
        }

        // SOAP Fault returned by the device (e.g. NotAuthorized)
        Err(Err_::Soap(SoapError::Fault { code, reason })) => {
            eprintln!("SOAP Fault [{code}]: {reason}");
            eprintln!("Tip: verify username / password.");
        }

        // Any other SOAP or parse error
        Err(e) => {
            eprintln!("Unexpected error: {e}");
        }
    }

    Ok(())
}
