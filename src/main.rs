//! oxvif — usage examples
//!
//! Copy `.env.example` to `.env`, fill in your camera details, then run:
//!
//! ```sh
//! cargo run -- full-workflow
//! cargo run -- device-info
//! cargo run -- stream-uris
//! cargo run -- error-handling
//! ```

use oxvif::{Capabilities, DeviceInfo, MediaProfile, OnvifClient, OnvifError, StreamUri};
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
    println!("  full-workflow    Discover capabilities, list profiles, fetch RTSP URIs");
    println!("  device-info      Print manufacturer, model, and firmware version");
    println!("  stream-uris      Print RTSP URIs for every media profile");
    println!("  error-handling   Demonstrate typed error variants");
}

// ── Example 1: full workflow ──────────────────────────────────────────────────

/// Typical end-to-end flow:
///   1. Fetch device capabilities to discover service URLs
///   2. Use the media URL to list media profiles
///   3. Fetch the RTSP stream URI for each profile
async fn full_workflow(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Full workflow ===");
    println!("Connecting to {}", cfg.camera_url);

    // Build client with credentials.  utc_offset can be obtained from
    // GetSystemDateAndTime (not yet implemented); pass 0 if clocks are in sync.
    let client = OnvifClient::new(&cfg.camera_url)
        .with_credentials(&cfg.username, &cfg.password)
        .with_utc_offset(0);

    // Step 1 — capabilities
    let caps: Capabilities = client.get_capabilities().await?;
    print_capabilities(&caps);

    let media_url = match &caps.media_url {
        Some(u) => u.clone(),
        None => {
            println!("Device does not advertise a Media service — stopping.");
            return Ok(());
        }
    };

    // Step 2 — media profiles
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

    // Step 3 — RTSP URIs
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

fn print_capabilities(caps: &Capabilities) {
    println!("\nCapabilities:");
    print_optional("  Device   ", &caps.device_url);
    print_optional("  Media    ", &caps.media_url);
    print_optional("  PTZ      ", &caps.ptz_url);
    print_optional("  Events   ", &caps.events_url);
    print_optional("  Imaging  ", &caps.imaging_url);
    print_optional("  Analytics", &caps.analytics_url);
}

fn print_optional(label: &str, value: &Option<String>) {
    match value {
        Some(v) => println!("{label}: {v}"),
        None => println!("{label}: (not supported)"),
    }
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
/// Useful for quickly discovering all streams a camera exposes.
async fn stream_uris(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Stream URIs ===");

    let client = OnvifClient::new(&cfg.camera_url).with_credentials(&cfg.username, &cfg.password);

    let caps = client.get_capabilities().await?;
    let media_url = caps
        .media_url
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

// ── Example 4: error handling ─────────────────────────────────────────────────

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
