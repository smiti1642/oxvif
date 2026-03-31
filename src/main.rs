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
//! cargo run -- video-config
//! cargo run -- error-handling
//! ```

use oxvif::{Capabilities, DeviceInfo, MediaProfile, OnvifClient, OnvifError, SystemDateTime};
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
        "video-config" => video_config(&cfg).await,
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
    println!("  video-config     Video sources, encoder configs and options");
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

/// End-to-end flow exercising every implemented operation:
///   1. Sync device clock → correct WS-Security timestamps
///   2. GetCapabilities   → discover service URLs + feature flags
///   3. GetDeviceInformation
///   4. GetProfiles       → list media profiles
///   5. GetStreamUri      → RTSP URI per profile
///   6. GetSnapshotUri    → HTTP snapshot URI per profile
///   7. GetPresets        → PTZ presets (skipped if no PTZ service)
async fn full_workflow(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Full workflow ===");
    println!("Connecting to {}", cfg.camera_url);

    // ── 1. Clock sync ─────────────────────────────────────────────────────────
    println!("\n-- GetSystemDateAndTime --");
    let base = OnvifClient::new(&cfg.camera_url);
    let utc_offset = match base.get_system_date_and_time().await {
        Ok(dt) => {
            let tz = if dt.timezone.is_empty() {
                "(none)".into()
            } else {
                dt.timezone.clone()
            };
            println!(
                "  Timezone: {tz}  DST: {}  UTC unix: {:?}",
                dt.daylight_savings, dt.utc_unix
            );
            let off = dt.utc_offset_secs();
            if off.abs() > 5 {
                println!("  Clock skew {off:+}s — applying offset");
            } else {
                println!("  Clocks in sync");
            }
            off
        }
        Err(e) => {
            println!("  (skipped — {e})");
            0
        }
    };

    let client = OnvifClient::new(&cfg.camera_url)
        .with_credentials(&cfg.username, &cfg.password)
        .with_utc_offset(utc_offset);

    // ── 2. Capabilities ───────────────────────────────────────────────────────
    println!("\n-- GetCapabilities --");
    let caps = client.get_capabilities().await?;
    print_capabilities(&caps);

    // ── 3. Device info ────────────────────────────────────────────────────────
    println!("\n-- GetDeviceInformation --");
    match client.get_device_info().await {
        Ok(info) => {
            println!(
                "  {}/{} fw:{} sn:{}",
                info.manufacturer, info.model, info.firmware_version, info.serial_number
            );
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 4–6. Media ────────────────────────────────────────────────────────────
    let media_url = match &caps.media.url {
        Some(u) => u.clone(),
        None => {
            println!("\nDevice does not advertise a Media service — stopping.");
            return Ok(());
        }
    };

    println!("\n-- GetProfiles --");
    let profiles: Vec<MediaProfile> = client.get_profiles(&media_url).await?;
    println!("  Found {} profile(s)", profiles.len());
    for p in &profiles {
        println!(
            "  [{token}] {name}  fixed={fixed}",
            token = p.token,
            name = p.name,
            fixed = p.fixed
        );
    }

    println!("\n-- GetStreamUri --");
    for profile in &profiles {
        match client.get_stream_uri(&media_url, &profile.token).await {
            Ok(uri) => {
                println!("  '{}' → {}", profile.name, uri.uri);
                if uri.invalid_after_connect {
                    println!("    (one-time URI)");
                }
                if !uri.timeout.is_empty() && uri.timeout != "PT0S" {
                    println!("    (timeout: {})", uri.timeout);
                }
            }
            Err(e) => println!("  '{}' ERROR: {e}", profile.name),
        }
    }

    println!("\n-- GetSnapshotUri --");
    for profile in &profiles {
        match client.get_snapshot_uri(&media_url, &profile.token).await {
            Ok(snap) => println!("  '{}' → {}", profile.name, snap.uri),
            Err(e) => println!("  '{}' ERROR: {e}", profile.name),
        }
    }

    // ── 7. PTZ ────────────────────────────────────────────────────────────────
    println!("\n-- PTZ GetPresets --");
    match &caps.ptz_url {
        None => println!("  (no PTZ service)"),
        Some(ptz_url) => {
            for profile in &profiles {
                match client.ptz_get_presets(ptz_url, &profile.token).await {
                    Ok(presets) if presets.is_empty() => {
                        println!("  '{}': (no presets)", profile.name);
                    }
                    Ok(presets) => {
                        println!("  '{}': {} preset(s)", profile.name, presets.len());
                        for p in &presets {
                            let pt = p
                                .pan_tilt
                                .map(|(x, y)| format!("{x:+.3}/{y:+.3}"))
                                .unwrap_or_else(|| "—".into());
                            let z = p
                                .zoom
                                .map(|z| format!("{z:.3}"))
                                .unwrap_or_else(|| "—".into());
                            println!("    [{}] {}  pan/tilt={pt} zoom={z}", p.token, p.name);
                        }
                    }
                    Err(e) => println!("  '{}' ERROR: {e}", profile.name),
                }
            }
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

// ── Example 7: video configuration ───────────────────────────────────────────

/// Prints all video sources, video source configurations, and video encoder
/// configurations together with available encoding options.
async fn video_config(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Video configuration ===");

    let (client, caps) = connect(cfg).await?;
    let media_url = match &caps.media.url {
        Some(u) => u.clone(),
        None => {
            println!("No Media service advertised.");
            return Ok(());
        }
    };

    // ── Physical video inputs ─────────────────────────────────────────────────
    println!("\n-- GetVideoSources --");
    match client.get_video_sources(&media_url).await {
        Ok(sources) => {
            println!("  Found {} source(s)", sources.len());
            for s in &sources {
                println!(
                    "  [{}]  {}  @ {:.0} fps",
                    s.token, s.resolution, s.framerate
                );
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // ── Video source configurations ───────────────────────────────────────────
    println!("\n-- GetVideoSourceConfigurations --");
    match client.get_video_source_configurations(&media_url).await {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                println!(
                    "  [{}] '{}' → source:{} bounds:{}x{}+{}+{}",
                    c.token,
                    c.name,
                    c.source_token,
                    c.bounds.width,
                    c.bounds.height,
                    c.bounds.x,
                    c.bounds.y
                );

                // Options for first config only (to avoid flooding output)
                if c.token == cfgs[0].token {
                    match client
                        .get_video_source_configuration_options(&media_url, Some(&c.token))
                        .await
                    {
                        Ok(opts) => {
                            if let Some(br) = &opts.bounds_range {
                                println!(
                                    "    bounds range: w=[{}-{}] h=[{}-{}]",
                                    br.width_range.min,
                                    br.width_range.max,
                                    br.height_range.min,
                                    br.height_range.max,
                                );
                            }
                        }
                        Err(e) => println!("    options ERROR: {e}"),
                    }
                }
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // ── Video encoder configurations ──────────────────────────────────────────
    println!("\n-- GetVideoEncoderConfigurations --");
    match client.get_video_encoder_configurations(&media_url).await {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                let rc = c.rate_control.as_ref();
                println!(
                    "  [{}] '{}' → {} {}  fps:{} bitrate:{}kbps",
                    c.token,
                    c.name,
                    c.encoding,
                    c.resolution,
                    rc.map(|r| r.frame_rate_limit).unwrap_or(0),
                    rc.map(|r| r.bitrate_limit).unwrap_or(0),
                );
                if let Some(h) = &c.h264 {
                    println!("    H.264: profile={} gop={}", h.profile, h.gov_length);
                }
                if let Some(h) = &c.h265 {
                    println!("    H.265: profile={} gop={}", h.profile, h.gov_length);
                }
            }

            // Options for first encoder config
            if let Some(first) = cfgs.first() {
                println!(
                    "\n-- GetVideoEncoderConfigurationOptions [{}] --",
                    first.token
                );
                match client
                    .get_video_encoder_configuration_options(&media_url, Some(&first.token))
                    .await
                {
                    Ok(opts) => {
                        if let Some(qr) = opts.quality_range {
                            println!("  Quality range: {:.0}–{:.0}", qr.min, qr.max);
                        }
                        if let Some(h264) = &opts.h264 {
                            println!(
                                "  H.264 resolutions: {}",
                                h264.resolutions
                                    .iter()
                                    .map(|r| r.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                            println!("  H.264 profiles: {}", h264.profiles.join(", "));
                            if let Some(br) = h264.frame_rate_range {
                                println!("  H.264 fps range: {}-{}", br.min, br.max);
                            }
                            if let Some(br) = h264.bitrate_range {
                                println!("  H.264 bitrate range: {}-{} kbps", br.min, br.max);
                            }
                        }
                        if let Some(h265) = &opts.h265 {
                            println!(
                                "  H.265 resolutions: {}",
                                h265.resolutions
                                    .iter()
                                    .map(|r| r.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        if let Some(jpeg) = &opts.jpeg {
                            println!(
                                "  JPEG resolutions: {}",
                                jpeg.resolutions
                                    .iter()
                                    .map(|r| r.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                    }
                    Err(e) => println!("  ERROR: {e}"),
                }
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    Ok(())
}

// ── Example 8: error handling ─────────────────────────────────────────────────

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
