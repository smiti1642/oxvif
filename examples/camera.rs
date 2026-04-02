//! oxvif — live IPCam integration tests / usage examples
//!
//! Copy `.env.example` to `.env`, fill in your camera details, then run:
//!
//! ```sh
//! cargo run --example camera -- full-workflow
//! cargo run --example camera -- device-info
//! cargo run --example camera -- device-management
//! cargo run --example camera -- stream-uris
//! cargo run --example camera -- snapshot-uris
//! cargo run --example camera -- system-datetime
//! cargo run --example camera -- ptz-presets
//! cargo run --example camera -- ptz-status
//! cargo run --example camera -- video-config
//! cargo run --example camera -- video-config-media2
//! cargo run --example camera -- imaging
//! cargo run --example camera -- events
//! cargo run --example camera -- discovery
//! cargo run --example camera -- error-handling
//! ```

use std::time::Duration;

use oxvif::{
    Capabilities, DeviceInfo, ImagingSettings, MediaProfile, OnvifClient, OnvifError,
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
        "device-management" => device_management(&cfg).await,
        "stream-uris" => stream_uris(&cfg).await,
        "snapshot-uris" => snapshot_uris(&cfg).await,
        "system-datetime" => system_datetime(&cfg).await,
        "ptz-presets" => ptz_presets(&cfg).await,
        "ptz-status" => ptz_status(&cfg).await,
        "video-config" => video_config(&cfg).await,
        "video-config-media2" => video_config_media2(&cfg).await,
        "imaging" => imaging(&cfg).await,
        "events" => events(&cfg).await,
        "discovery" => discovery_example().await,
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
    println!("oxvif IPCam integration examples");
    println!();
    println!("USAGE:");
    println!("  cargo run -- <example>");
    println!();
    println!("EXAMPLES:");
    println!("  full-workflow        All implemented operations end-to-end");
    println!("  device-info          Manufacturer, model, firmware");
    println!("  device-management    Hostname, NTP, GetServices");
    println!("  stream-uris          Tabular RTSP URI listing");
    println!("  snapshot-uris        Tabular HTTP snapshot URI listing");
    println!("  system-datetime      Device clock and UTC offset");
    println!("  ptz-presets          List all PTZ presets");
    println!("  ptz-status           Current PTZ pan/tilt/zoom position");
    println!("  video-config         Video sources, encoder configs, options (Media1)");
    println!("  video-config-media2  Media2 profiles, H.265 encoder configs");
    println!("  imaging              Imaging settings and parameter ranges");
    println!("  events               Subscribe, pull, and unsubscribe ONVIF events");
    println!("  discovery            WS-Discovery UDP multicast probe");
    println!("  error-handling       Typed error variant matching demo");
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Create a clock-synced, authenticated client and return it with capabilities.
async fn connect(cfg: &Config) -> Result<(OnvifClient, Capabilities), OnvifError> {
    let base = OnvifClient::new(&cfg.camera_url);
    let utc_offset = base
        .get_system_date_and_time()
        .await
        .map(|dt| dt.utc_offset_secs())
        .unwrap_or(0);

    let client = OnvifClient::new(&cfg.camera_url)
        .with_credentials(&cfg.username, &cfg.password)
        .with_utc_offset(utc_offset);

    let caps = client.get_capabilities().await?;
    Ok((client, caps))
}

fn print_capabilities(caps: &Capabilities) {
    println!("\nCapabilities:");
    print_opt("  Device   ", &caps.device.url);
    print_opt("  Media    ", &caps.media.url);
    print_opt("  PTZ      ", &caps.ptz_url);
    print_opt("  Imaging  ", &caps.imaging_url);
    print_opt("  Events   ", &caps.events.url);
    print_opt("  Analytics", &caps.analytics.url);
    print_opt("  Media2   ", &caps.media2_url);
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

fn print_opt(label: &str, value: &Option<String>) {
    match value {
        Some(v) => println!("{label}: {v}"),
        None => println!("{label}: (not supported)"),
    }
}

fn section(title: &str) {
    println!("\n-- {title} --");
}

// ── Example 1: full workflow ──────────────────────────────────────────────────

/// End-to-end exercise of every implemented operation.
///
/// All write operations that alter device state are either skipped or paired
/// with a matching cleanup (e.g. CreateProfile → DeleteProfile) so the camera
/// is left in the same state it was found in.
async fn full_workflow(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Full workflow ===");
    println!("Connecting to {}", cfg.camera_url);

    // ── 1. Clock sync ─────────────────────────────────────────────────────────
    section("GetSystemDateAndTime");
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
    section("GetCapabilities");
    let caps = client.get_capabilities().await?;
    print_capabilities(&caps);

    // ── 3. GetServices ────────────────────────────────────────────────────────
    // Store results so Media2 URL can be derived as a fallback later.
    section("GetServices");
    let services = match client.get_services().await {
        Ok(services) => {
            println!("  Found {} service(s)", services.len());
            for svc in &services {
                println!(
                    "  v{}.{}  {}",
                    svc.version_major, svc.version_minor, svc.url
                );
            }
            services
        }
        Err(e) => {
            println!("  (skipped — {e})");
            vec![]
        }
    };

    // ── 4. Device information ─────────────────────────────────────────────────
    section("GetDeviceInformation");
    match client.get_device_info().await {
        Ok(info) => println!(
            "  {}/{} fw:{} sn:{}",
            info.manufacturer, info.model, info.firmware_version, info.serial_number
        ),
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 5. Hostname ───────────────────────────────────────────────────────────
    section("GetHostname");
    match client.get_hostname().await {
        Ok(h) => {
            let src = if h.from_dhcp { "DHCP" } else { "static" };
            let name = h.name.as_deref().unwrap_or("(none)");
            println!("  Hostname: {name}  (source: {src})");
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 6. NTP ────────────────────────────────────────────────────────────────
    section("GetNTP");
    match client.get_ntp().await {
        Ok(ntp) => {
            let src = if ntp.from_dhcp { "DHCP" } else { "manual" };
            if ntp.servers.is_empty() {
                println!("  Source: {src}  Servers: (none configured)");
            } else {
                println!("  Source: {src}  Servers: {}", ntp.servers.join(", "));
            }
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 7–9. Media ────────────────────────────────────────────────────────────
    let media_url = match &caps.media.url {
        Some(u) => u.clone(),
        None => {
            println!("\nDevice does not advertise a Media service — stopping.");
            return Ok(());
        }
    };

    section("GetProfiles");
    let profiles: Vec<MediaProfile> = client.get_profiles(&media_url).await?;
    println!("  Found {} profile(s)", profiles.len());
    for p in &profiles {
        println!(
            "  [{token}] '{name}'  fixed={fixed}",
            token = p.token,
            name = p.name,
            fixed = p.fixed
        );
    }

    // Single-profile lookup for the first profile
    if let Some(first) = profiles.first() {
        section(&format!("GetProfile [{}]", first.token));
        match client.get_profile(&media_url, &first.token).await {
            Ok(p) => println!(
                "  [{token}] '{name}'  fixed={fixed}",
                token = p.token,
                name = p.name,
                fixed = p.fixed
            ),
            Err(e) => println!("  (skipped — {e})"),
        }
    }

    section("GetStreamUri");
    for profile in &profiles {
        match client.get_stream_uri(&media_url, &profile.token).await {
            Ok(uri) => {
                print!("  '{}' → {}", profile.name, uri.uri);
                if uri.invalid_after_connect {
                    print!("  [one-time]");
                }
                if !uri.timeout.is_empty() && uri.timeout != "PT0S" {
                    print!("  [timeout:{}]", uri.timeout);
                }
                println!();
            }
            Err(e) => println!("  '{}' ERROR: {e}", profile.name),
        }
    }

    section("GetSnapshotUri");
    for profile in &profiles {
        match client.get_snapshot_uri(&media_url, &profile.token).await {
            Ok(snap) => println!("  '{}' → {}", profile.name, snap.uri),
            Err(e) => println!("  '{}' ERROR: {e}", profile.name),
        }
    }

    // ── 10. Video sources ─────────────────────────────────────────────────────
    section("GetVideoSources");
    let video_sources = match client.get_video_sources(&media_url).await {
        Ok(sources) => {
            println!("  Found {} source(s)", sources.len());
            for s in &sources {
                println!(
                    "  [{}]  {}  @ {:.0} fps",
                    s.token, s.resolution, s.framerate
                );
            }
            sources
        }
        Err(e) => {
            println!("  (skipped — {e})");
            vec![]
        }
    };

    // ── 11. Video source configurations ──────────────────────────────────────
    section("GetVideoSourceConfigurations");
    match client.get_video_source_configurations(&media_url).await {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                println!(
                    "  [{}] '{}' → source:{} bounds:{}x{}+{}+{}  use_count:{}",
                    c.token,
                    c.name,
                    c.source_token,
                    c.bounds.width,
                    c.bounds.height,
                    c.bounds.x,
                    c.bounds.y,
                    c.use_count,
                );
            }
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 12. Video encoder configurations ─────────────────────────────────────
    section("GetVideoEncoderConfigurations");
    match client.get_video_encoder_configurations(&media_url).await {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                let rc = c.rate_control.as_ref();
                println!(
                    "  [{}] '{}' → {} {}  fps:{}  bitrate:{}kbps",
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
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 13. Imaging settings (first video source) ─────────────────────────────
    if let Some(imaging_url) = &caps.imaging_url {
        if let Some(vs) = video_sources.first() {
            section(&format!("GetImagingSettings [{}]", vs.token));
            match client.get_imaging_settings(imaging_url, &vs.token).await {
                Ok(s) => {
                    print_imaging_settings(&s);
                }
                Err(e) => println!("  (skipped — {e})"),
            }

            section(&format!("GetImagingOptions [{}]", vs.token));
            match client.get_imaging_options(imaging_url, &vs.token).await {
                Ok(opts) => {
                    if let Some(r) = opts.brightness {
                        println!("  Brightness: {:.0}–{:.0}", r.min, r.max);
                    }
                    if let Some(r) = opts.contrast {
                        println!("  Contrast  : {:.0}–{:.0}", r.min, r.max);
                    }
                    if !opts.ir_cut_filter_modes.is_empty() {
                        println!(
                            "  IR cut filter modes: {}",
                            opts.ir_cut_filter_modes.join(", ")
                        );
                    }
                    if !opts.white_balance_modes.is_empty() {
                        println!(
                            "  White balance modes: {}",
                            opts.white_balance_modes.join(", ")
                        );
                    }
                    if !opts.exposure_modes.is_empty() {
                        println!("  Exposure modes: {}", opts.exposure_modes.join(", "));
                    }
                }
                Err(e) => println!("  (skipped — {e})"),
            }
        }
    } else {
        println!("\n-- Imaging --");
        println!("  (no Imaging service URL in capabilities)");
    }

    // ── 14. PTZ ───────────────────────────────────────────────────────────────
    if let Some(ptz_url) = &caps.ptz_url {
        for profile in &profiles {
            section(&format!("PTZ GetStatus [{}]", profile.token));
            match client.ptz_get_status(ptz_url, &profile.token).await {
                Ok(status) => {
                    let pan = status.pan.map(|v| format!("{v:+.4}")).unwrap_or("—".into());
                    let tilt = status
                        .tilt
                        .map(|v| format!("{v:+.4}"))
                        .unwrap_or("—".into());
                    let zoom = status.zoom.map(|v| format!("{v:.4}")).unwrap_or("—".into());
                    println!(
                        "  pan:{pan}  tilt:{tilt}  zoom:{zoom}  \
                         move:{}/{}",
                        status.pan_tilt_status, status.zoom_status
                    );
                }
                Err(e) => println!("  (skipped — {e})"),
            }

            section(&format!("PTZ GetPresets [{}]", profile.token));
            match client.ptz_get_presets(ptz_url, &profile.token).await {
                Ok(presets) if presets.is_empty() => println!("  (no presets saved)"),
                Ok(presets) => {
                    println!("  {} preset(s)", presets.len());
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
                Err(e) => println!("  (skipped — {e})"),
            }
        }
    } else {
        println!("\n-- PTZ --");
        println!("  (no PTZ service)");
    }

    // ── 15. Profile lifecycle test ────────────────────────────────────────────
    // Create a temporary profile then immediately delete it to verify the
    // complete profile management lifecycle without leaving state on the device.
    section("CreateProfile + DeleteProfile (lifecycle test)");
    match client
        .create_profile(&media_url, "oxvif-test-profile", None)
        .await
    {
        Ok(p) => {
            println!(
                "  Created  [{token}] '{name}'  fixed={fixed}",
                token = p.token,
                name = p.name,
                fixed = p.fixed
            );
            match client.delete_profile(&media_url, &p.token).await {
                Ok(()) => println!("  Deleted  [{}] — device state restored", p.token),
                Err(e) => println!("  Delete failed (manual cleanup needed): {e}"),
            }
        }
        Err(e) => println!("  (skipped — {e})"),
    }

    // ── 16. Media2 ────────────────────────────────────────────────────────────
    // GetCapabilities often omits Media2; fall back to the URL found via
    // GetServices (step 3) when that happens.
    let media2_url = caps.media2_url.clone().or_else(|| {
        services
            .iter()
            .find(|s| s.is_media2())
            .map(|s| s.url.clone())
    });

    if let Some(m2_url) = media2_url {
        section("GetProfiles (Media2)");
        match client.get_profiles_media2(&m2_url).await {
            Ok(profiles2) => {
                println!("  Found {} Media2 profile(s)", profiles2.len());
                for p in &profiles2 {
                    println!(
                        "  [{token}] '{name}'  fixed={fixed}  vsc={vsc:?}  vec={vec:?}",
                        token = p.token,
                        name = p.name,
                        fixed = p.fixed,
                        vsc = p.video_source_token,
                        vec = p.video_encoder_token,
                    );
                }
                // Stream + snapshot via Media2
                if let Some(p) = profiles2.first() {
                    match client.get_stream_uri_media2(&m2_url, &p.token).await {
                        Ok(uri) => println!("  Stream URI (Media2, first): {uri}"),
                        Err(e) => println!("  Stream URI (Media2): {e}"),
                    }
                }
            }
            Err(e) => println!("  (skipped — {e})"),
        }
    }

    println!("\n=== Full workflow complete ===");
    Ok(())
}

// ── Example 2: device info ────────────────────────────────────────────────────

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

// ── Example 3: device management ─────────────────────────────────────────────

/// Shows hostname, NTP configuration, and all service endpoints.
/// Read-only — does not modify the device.
async fn device_management(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Device management ===");

    let (client, caps) = connect(cfg).await?;

    // Services
    section("GetServices");
    match client.get_services().await {
        Ok(services) => {
            println!("  {:<8}  {:<40}  URL", "Version", "Namespace");
            println!("  {}", "-".repeat(80));
            for svc in &services {
                println!(
                    "  v{}.{:<5}  {:<40}  {}",
                    svc.version_major,
                    svc.version_minor,
                    // Truncate long namespaces
                    svc.namespace
                        .trim_start_matches("http://www.onvif.org/")
                        .trim_start_matches("http://"),
                    svc.url
                );
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // Hostname
    section("GetHostname");
    match client.get_hostname().await {
        Ok(h) => {
            let src = if h.from_dhcp { "DHCP" } else { "static" };
            println!("  Hostname : {}", h.name.as_deref().unwrap_or("(not set)"));
            println!("  Source   : {src}");
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // NTP
    section("GetNTP");
    match client.get_ntp().await {
        Ok(ntp) => {
            let src = if ntp.from_dhcp { "DHCP" } else { "manual" };
            println!("  Source   : {src}");
            if ntp.servers.is_empty() {
                println!("  Servers  : (none configured)");
            } else {
                for (i, s) in ntp.servers.iter().enumerate() {
                    println!("  Server {i} : {s}");
                }
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // Device security flags (from capabilities)
    section("Security capabilities");
    let sec = &caps.device.security;
    println!("  UsernameToken      : {}", sec.username_token);
    println!("  TLS 1.2            : {}", sec.tls_1_2);
    println!("  X.509 token        : {}", sec.x509_token);
    println!("  Onboard key gen    : {}", sec.onboard_key_generation);
    println!("  Access policy cfg  : {}", sec.access_policy_config);

    // Device system flags
    section("System capabilities");
    let sys = &caps.device.system;
    println!("  DiscoveryResolve   : {}", sys.discovery_resolve);
    println!("  RemoteDiscovery    : {}", sys.remote_discovery);
    println!("  FirmwareUpgrade    : {}", sys.firmware_upgrade);
    println!("  SystemLogging      : {}", sys.system_logging);
    println!("  SystemBackup       : {}", sys.system_backup);

    // ── Events ────────────────────────────────────────────────────────────────
    if let Some(events_url) = &caps.events.url {
        section("Events — GetEventProperties");
        match client.get_event_properties(events_url).await {
            Ok(props) => {
                println!("  {} topic(s) available", props.topics.len());
                for t in props.topics.iter().take(8) {
                    println!("  - {t}");
                }
                if props.topics.len() > 8 {
                    println!("  … ({} more)", props.topics.len() - 8);
                }
            }
            Err(e) => println!("  (skipped — {e})"),
        }

        if caps.events.ws_pull_point {
            section("Events — CreatePullPointSubscription / PullMessages / Unsubscribe");
            match client
                .create_pull_point_subscription(events_url, None, Some("PT30S"))
                .await
            {
                Ok(sub) => {
                    println!("  Subscription URL : {}", sub.reference_url);
                    println!("  Termination time : {}", sub.termination_time);
                    match client.pull_messages(&sub.reference_url, "PT2S", 10).await {
                        Ok(msgs) => {
                            if msgs.is_empty() {
                                println!("  No pending events");
                            } else {
                                println!("  {} event(s) received:", msgs.len());
                                for m in &msgs {
                                    println!(
                                        "  [{}] {}  src={:?}  data={:?}",
                                        m.utc_time, m.topic, m.source, m.data
                                    );
                                }
                            }
                        }
                        Err(e) => println!("  PullMessages skipped — {e}"),
                    }
                    if let Err(e) = client.unsubscribe(&sub.reference_url).await {
                        println!("  Unsubscribe skipped — {e}");
                    } else {
                        println!("  Unsubscribed successfully");
                    }
                }
                Err(e) => println!("  (skipped — {e})"),
            }
        }
    }

    Ok(())
}

// ── Example 4: stream URIs ────────────────────────────────────────────────────

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

    // Also try Media2 if available
    if let Some(m2_url) = &caps.media2_url {
        println!("\n{:<20} RTSP URI (Media2)", "Profile");
        println!("{}", "-".repeat(80));
        match client.get_profiles_media2(m2_url).await {
            Ok(profiles2) => {
                for p in &profiles2 {
                    match client.get_stream_uri_media2(m2_url, &p.token).await {
                        Ok(uri) => println!("{:<20} {}", p.name, uri),
                        Err(e) => println!("{:<20} ERROR: {e}", p.name),
                    }
                }
            }
            Err(e) => println!("Media2 profiles: ERROR: {e}"),
        }
    }

    Ok(())
}

// ── Example 5: snapshot URIs ──────────────────────────────────────────────────

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

// ── Example 6: system date and time ──────────────────────────────────────────

async fn system_datetime(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== System date and time ===");

    let client = OnvifClient::new(&cfg.camera_url);
    let dt: SystemDateTime = match client.get_system_date_and_time().await {
        Ok(dt) => dt,
        Err(_) => {
            println!("(unauthenticated failed — retrying with credentials)");
            OnvifClient::new(&cfg.camera_url)
                .with_credentials(&cfg.username, &cfg.password)
                .get_system_date_and_time()
                .await?
        }
    };

    match dt.utc_unix {
        Some(unix) => {
            let secs = unix % 60;
            let mins = (unix / 60) % 60;
            let hours = (unix / 3600) % 24;
            let days = unix / 86_400;
            println!("Device UTC   : Unix {unix}  ({days}d {hours:02}:{mins:02}:{secs:02} UTC)");
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
            "  ! Clock skew detected. Use .with_utc_offset({offset}) to keep WS-Security valid."
        );
    } else {
        println!("  Clocks are in sync — no offset needed.");
    }

    Ok(())
}

// ── Example 7: PTZ presets ────────────────────────────────────────────────────

async fn ptz_presets(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== PTZ presets ===");

    let (client, caps) = connect(cfg).await?;

    let ptz_url = match caps.ptz_url.clone() {
        Some(u) => {
            println!("PTZ service: {u}");
            u
        }
        None => {
            println!("Device does not advertise a PTZ service.");
            return Ok(());
        }
    };

    let media_url = match caps.media.url.clone() {
        Some(u) => u,
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
                    "  {:<10} {:<24} {:>14}  {:>8}",
                    "Token", "Name", "Pan/Tilt", "Zoom"
                );
                println!("  {}", "-".repeat(60));
                for p in &presets {
                    let pt = match p.pan_tilt {
                        Some((x, y)) => format!("{x:+.4}/{y:+.4}"),
                        None => "—".to_string(),
                    };
                    let z = match p.zoom {
                        Some(z) => format!("{z:.4}"),
                        None => "—".to_string(),
                    };
                    println!("  {:<10} {:<24} {:>14}  {:>8}", p.token, p.name, pt, z);
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    }

    Ok(())
}

// ── Example 8: PTZ status ─────────────────────────────────────────────────────

/// Shows the current pan/tilt/zoom position and movement state for every
/// media profile. Reports "IDLE" / "MOVING" / "UNKNOWN" per axis.
async fn ptz_status(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== PTZ status ===");

    let (client, caps) = connect(cfg).await?;

    let ptz_url = match caps.ptz_url.clone() {
        Some(u) => {
            println!("PTZ service: {u}");
            u
        }
        None => {
            println!("Device does not advertise a PTZ service.");
            return Ok(());
        }
    };

    let media_url = match caps.media.url.clone() {
        Some(u) => u,
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

    println!(
        "\n{:<20} {:>10}  {:>10}  {:>8}  {:>14}  {:>8}",
        "Profile", "Pan", "Tilt", "Zoom", "Pan/Tilt move", "Zoom move"
    );
    println!("{}", "-".repeat(80));

    for profile in &profiles {
        match client.ptz_get_status(&ptz_url, &profile.token).await {
            Ok(status) => {
                let pan = status.pan.map(|v| format!("{v:+.4}")).unwrap_or("—".into());
                let tilt = status
                    .tilt
                    .map(|v| format!("{v:+.4}"))
                    .unwrap_or("—".into());
                let zoom = status.zoom.map(|v| format!("{v:.4}")).unwrap_or("—".into());
                println!(
                    "{:<20} {:>10}  {:>10}  {:>8}  {:>14}  {:>8}",
                    profile.name, pan, tilt, zoom, status.pan_tilt_status, status.zoom_status,
                );
            }
            Err(e) => println!("{:<20} ERROR: {e}", profile.name),
        }
    }

    Ok(())
}

// ── Example 9: video configuration ───────────────────────────────────────────

async fn video_config(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Video configuration (Media1) ===");

    let (client, caps) = connect(cfg).await?;
    let media_url = match caps.media.url.clone() {
        Some(u) => u,
        None => {
            println!("No Media service advertised.");
            return Ok(());
        }
    };

    section("GetVideoSources");
    let video_sources = match client.get_video_sources(&media_url).await {
        Ok(sources) => {
            println!("  Found {} source(s)", sources.len());
            for s in &sources {
                println!(
                    "  [{}]  {}  @ {:.0} fps",
                    s.token, s.resolution, s.framerate
                );
            }
            sources
        }
        Err(e) => {
            println!("  ERROR: {e}");
            vec![]
        }
    };

    section("GetVideoSourceConfigurations");
    let vsc_list = match client.get_video_source_configurations(&media_url).await {
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
                    c.bounds.y,
                );
            }
            cfgs
        }
        Err(e) => {
            println!("  ERROR: {e}");
            vec![]
        }
    };

    // Options for first config
    if let Some(first) = vsc_list.first() {
        section(&format!(
            "GetVideoSourceConfigurationOptions [{}]",
            first.token
        ));
        match client
            .get_video_source_configuration_options(&media_url, Some(&first.token))
            .await
        {
            Ok(opts) => {
                if let Some(br) = &opts.bounds_range {
                    println!(
                        "  Bounds range: w=[{}-{}] h=[{}-{}]",
                        br.width_range.min,
                        br.width_range.max,
                        br.height_range.min,
                        br.height_range.max,
                    );
                }
                if !opts.source_tokens.is_empty() {
                    println!("  Available sources: {}", opts.source_tokens.join(", "));
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    }

    section("GetVideoEncoderConfigurations");
    let enc_list = match client.get_video_encoder_configurations(&media_url).await {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                let rc = c.rate_control.as_ref();
                println!(
                    "  [{}] '{}' → {} {}  fps:{}  bitrate:{}kbps",
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
            cfgs
        }
        Err(e) => {
            println!("  ERROR: {e}");
            vec![]
        }
    };

    if let Some(first) = enc_list.first() {
        section(&format!(
            "GetVideoEncoderConfigurationOptions [{}]",
            first.token
        ));
        match client
            .get_video_encoder_configuration_options(&media_url, Some(&first.token))
            .await
        {
            Ok(opts) => {
                if let Some(qr) = opts.quality_range {
                    println!("  Quality range: {:.0}–{:.0}", qr.min, qr.max);
                }
                for (label, codec_opts) in [
                    (
                        "H.264",
                        opts.h264.as_ref().map(|h| {
                            (
                                &h.resolutions[..],
                                &h.profiles[..],
                                h.frame_rate_range,
                                h.bitrate_range,
                            )
                        }),
                    ),
                    (
                        "H.265",
                        opts.h265.as_ref().map(|h| {
                            (
                                &h.resolutions[..],
                                &h.profiles[..],
                                h.frame_rate_range,
                                h.bitrate_range,
                            )
                        }),
                    ),
                    (
                        "JPEG",
                        opts.jpeg
                            .as_ref()
                            .map(|j| (&j.resolutions[..], &[][..], j.frame_rate_range, None)),
                    ),
                ] {
                    if let Some((res, profiles, fps_range, bps_range)) = codec_opts {
                        println!(
                            "  {label} resolutions: {}",
                            res.iter()
                                .map(|r| r.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        if !profiles.is_empty() {
                            println!("  {label} profiles: {}", profiles.join(", "));
                        }
                        if let Some(r) = fps_range {
                            println!("  {label} fps range: {}-{}", r.min, r.max);
                        }
                        if let Some(r) = bps_range {
                            println!("  {label} bitrate range: {}-{} kbps", r.min, r.max);
                        }
                    }
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    }

    // Profile management demo: create a new profile, bind configs, then clean up
    section("Profile management lifecycle");
    let profiles = client.get_profiles(&media_url).await?;
    match client
        .create_profile(&media_url, "oxvif-test-profile", None)
        .await
    {
        Ok(test_profile) => {
            println!(
                "  Created  [{}] '{}'",
                test_profile.token, test_profile.name
            );

            // Bind first video source config if available
            if let Some(vsc) = vsc_list.first() {
                match client
                    .add_video_source_configuration(&media_url, &test_profile.token, &vsc.token)
                    .await
                {
                    Ok(()) => println!(
                        "  Bound    VideoSourceConfig [{}] → profile [{}]",
                        vsc.token, test_profile.token
                    ),
                    Err(e) => println!("  AddVideoSourceConfiguration: {e}"),
                }
            }

            // Bind first video encoder config if available
            if let Some(venc) = enc_list.first() {
                match client
                    .add_video_encoder_configuration(&media_url, &test_profile.token, &venc.token)
                    .await
                {
                    Ok(()) => println!(
                        "  Bound    VideoEncoderConfig [{}] → profile [{}]",
                        venc.token, test_profile.token
                    ),
                    Err(e) => println!("  AddVideoEncoderConfiguration: {e}"),
                }
            }

            // Verify via GetProfile
            match client.get_profile(&media_url, &test_profile.token).await {
                Ok(p) => println!("  Verified [{}] '{}'  fixed={}", p.token, p.name, p.fixed),
                Err(e) => println!("  GetProfile: {e}"),
            }

            // Clean up
            match client.delete_profile(&media_url, &test_profile.token).await {
                Ok(()) => println!(
                    "  Deleted  [{}] — device state restored",
                    test_profile.token
                ),
                Err(e) => println!("  Delete failed (manual cleanup needed): {e}"),
            }
        }
        Err(e) => println!("  CreateProfile: {e}"),
    }

    let _ = (video_sources, profiles);
    Ok(())
}

// ── Example 10: Media2 video configuration ────────────────────────────────────

async fn video_config_media2(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Media2 video configuration ===");

    let (client, caps) = connect(cfg).await?;

    let media2_url = match caps.media2_url.clone() {
        Some(u) => {
            println!("Media2 URL (GetCapabilities): {u}");
            u
        }
        None => {
            println!("Media2 not in GetCapabilities — trying GetServices...");
            match client.get_services().await {
                Ok(services) => match services.into_iter().find(|s| s.is_media2()) {
                    Some(svc) => {
                        println!("Media2 URL (GetServices): {}", svc.url);
                        svc.url
                    }
                    None => {
                        println!("Device does not support Media2.");
                        return Ok(());
                    }
                },
                Err(e) => {
                    println!("GetServices failed: {e} — device does not support Media2.");
                    return Ok(());
                }
            }
        }
    };

    section("GetProfiles (Media2)");
    let profiles2 = match client.get_profiles_media2(&media2_url).await {
        Ok(p) => {
            println!("  Found {} profile(s)", p.len());
            for pr in &p {
                println!(
                    "  [{token}] '{name}'  fixed={fixed}  vsc={vsc:?}  vec={vec:?}",
                    token = pr.token,
                    name = pr.name,
                    fixed = pr.fixed,
                    vsc = pr.video_source_token,
                    vec = pr.video_encoder_token,
                );
            }
            p
        }
        Err(e) => {
            println!("  ERROR: {e}");
            vec![]
        }
    };

    // Stream + snapshot URIs
    if let Some(p) = profiles2.first() {
        section(&format!(
            "GetStreamUri / GetSnapshotUri (Media2) [{}]",
            p.token
        ));
        match client.get_stream_uri_media2(&media2_url, &p.token).await {
            Ok(uri) => println!("  Stream  : {uri}"),
            Err(e) => println!("  Stream  ERROR: {e}"),
        }
        match client.get_snapshot_uri_media2(&media2_url, &p.token).await {
            Ok(uri) => println!("  Snapshot: {uri}"),
            Err(e) => println!("  Snapshot ERROR: {e}"),
        }
    }

    section("GetVideoEncoderConfigurations (Media2)");
    let enc_cfgs2 = match client
        .get_video_encoder_configurations_media2(&media2_url)
        .await
    {
        Ok(cfgs) => {
            println!("  Found {} config(s)", cfgs.len());
            for c in &cfgs {
                let rc = c.rate_control.as_ref();
                println!(
                    "  [{}] '{}' → {} {}  fps:{}  bitrate:{}kbps  gop:{:?}  profile:{:?}",
                    c.token,
                    c.name,
                    c.encoding,
                    c.resolution,
                    rc.map(|r| r.frame_rate_limit).unwrap_or(0),
                    rc.map(|r| r.bitrate_limit).unwrap_or(0),
                    c.gov_length,
                    c.profile,
                );
            }
            cfgs
        }
        Err(e) => {
            println!("  ERROR: {e}");
            vec![]
        }
    };

    section("GetVideoEncoderConfigurationOptions (Media2)");
    match client
        .get_video_encoder_configuration_options_media2(&media2_url, None)
        .await
    {
        Ok(opts) => {
            println!("  Found {} option set(s)", opts.options.len());
            for opt in &opts.options {
                println!("  {}:", opt.encoding);
                if !opt.resolutions.is_empty() {
                    println!(
                        "    Resolutions: {}",
                        opt.resolutions
                            .iter()
                            .map(|r| r.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if !opt.profiles.is_empty() {
                    println!("    Profiles: {}", opt.profiles.join(", "));
                }
                if let Some(br) = opt.bitrate_range {
                    println!("    Bitrate: {}-{} kbps", br.min, br.max);
                }
                if let Some(gr) = opt.gov_length_range {
                    println!("    GoP: {}-{}", gr.min, gr.max);
                }
                if let Some(fr) = opt.frame_rate_range {
                    println!("    FPS: {}-{}", fr.min, fr.max);
                }
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    section("GetVideoEncoderInstances (Media2)");
    let first_vsc = client
        .get_video_source_configurations_media2(&media2_url)
        .await
        .ok()
        .and_then(|cfgs| cfgs.into_iter().next().map(|c| c.token));

    if let Some(vsc_token) = first_vsc {
        match client
            .get_video_encoder_instances_media2(&media2_url, &vsc_token)
            .await
        {
            Ok(inst) => {
                println!("  Total instances: {}  (vsc={})", inst.total, vsc_token);
                for enc in &inst.encodings {
                    println!("    {}: {} instance(s)", enc.encoding, enc.number);
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    } else {
        println!("  (no video source configurations available)");
    }

    let _ = (profiles2, enc_cfgs2);
    Ok(())
}

// ── Example 11: imaging settings ─────────────────────────────────────────────

/// Reads and displays the imaging settings and valid ranges for every video
/// source. Read-only — does not modify the device.
async fn imaging(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Imaging settings ===");

    let (client, caps) = connect(cfg).await?;

    let imaging_url = match caps.imaging_url.clone() {
        Some(u) => {
            println!("Imaging service: {u}");
            u
        }
        None => {
            println!("Device does not advertise an Imaging service.");
            return Ok(());
        }
    };

    let media_url = match caps.media.url.clone() {
        Some(u) => u,
        None => {
            println!("No media service — cannot list video sources.");
            return Ok(());
        }
    };

    let sources = match client.get_video_sources(&media_url).await {
        Ok(s) if !s.is_empty() => s,
        Ok(_) => {
            println!("No video sources found.");
            return Ok(());
        }
        Err(e) => {
            println!("GetVideoSources ERROR: {e}");
            return Ok(());
        }
    };

    for source in &sources {
        println!(
            "\nVideo source: [{}]  {}  @ {:.0} fps",
            source.token, source.resolution, source.framerate
        );

        section(&format!("GetImagingSettings [{}]", source.token));
        match client
            .get_imaging_settings(&imaging_url, &source.token)
            .await
        {
            Ok(s) => print_imaging_settings(&s),
            Err(e) => println!("  ERROR: {e}"),
        }

        section(&format!("GetImagingOptions [{}]", source.token));
        match client
            .get_imaging_options(&imaging_url, &source.token)
            .await
        {
            Ok(opts) => {
                let ranges = [
                    ("Brightness    ", opts.brightness),
                    ("ColorSaturation", opts.color_saturation),
                    ("Contrast      ", opts.contrast),
                    ("Sharpness     ", opts.sharpness),
                ];
                for (label, range) in ranges {
                    if let Some(r) = range {
                        println!("  {label}: {:.0} – {:.0}", r.min, r.max);
                    }
                }
                if !opts.ir_cut_filter_modes.is_empty() {
                    println!("  IR cut filter  : {}", opts.ir_cut_filter_modes.join(", "));
                }
                if !opts.white_balance_modes.is_empty() {
                    println!("  White balance  : {}", opts.white_balance_modes.join(", "));
                }
                if !opts.exposure_modes.is_empty() {
                    println!("  Exposure       : {}", opts.exposure_modes.join(", "));
                }
            }
            Err(e) => println!("  ERROR: {e}"),
        }
    }

    Ok(())
}

fn print_imaging_settings(s: &ImagingSettings) {
    let fmt = |v: Option<f32>| v.map(|f| format!("{f:.1}")).unwrap_or_else(|| "—".into());
    println!("  Brightness      : {}", fmt(s.brightness));
    println!("  ColorSaturation : {}", fmt(s.color_saturation));
    println!("  Contrast        : {}", fmt(s.contrast));
    println!("  Sharpness       : {}", fmt(s.sharpness));
    println!(
        "  IR cut filter   : {}",
        s.ir_cut_filter.as_deref().unwrap_or("—")
    );
    println!(
        "  White balance   : {}",
        s.white_balance_mode.as_deref().unwrap_or("—")
    );
    println!(
        "  Exposure        : {}",
        s.exposure_mode.as_deref().unwrap_or("—")
    );
}

// ── Example 12: events ────────────────────────────────────────────────────────

async fn events(cfg: &Config) -> Result<(), OnvifError> {
    println!("=== Events ===");

    let (client, caps) = connect(cfg).await?;
    let events_url = caps
        .events
        .url
        .ok_or_else(|| oxvif::soap::SoapError::missing("Events service not found"))?;

    // GetEventProperties
    section("GetEventProperties");
    match client.get_event_properties(&events_url).await {
        Ok(props) => {
            println!("  {} topic(s) available", props.topics.len());
            for t in &props.topics {
                println!("  - {t}");
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    if !caps.events.ws_pull_point {
        println!("\nDevice does not support WS-PullPoint — skipping subscription.");
        return Ok(());
    }

    // CreatePullPointSubscription
    section("CreatePullPointSubscription");
    let sub = client
        .create_pull_point_subscription(&events_url, None, Some("PT60S"))
        .await?;
    println!("  Subscription URL : {}", sub.reference_url);
    println!("  Termination time : {}", sub.termination_time);

    // PullMessages
    section("PullMessages (PT5S timeout)");
    match client.pull_messages(&sub.reference_url, "PT5S", 50).await {
        Ok(msgs) => {
            if msgs.is_empty() {
                println!("  No pending events in 5 seconds");
            } else {
                println!("  {} event(s) received:", msgs.len());
                for m in &msgs {
                    println!("  Topic   : {}", m.topic);
                    println!("  UtcTime : {}", m.utc_time);
                    for (k, v) in &m.source {
                        println!("  Source  : {k} = {v}");
                    }
                    for (k, v) in &m.data {
                        println!("  Data    : {k} = {v}");
                    }
                    println!();
                }
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }

    // Renew
    section("Renew");
    match client.renew_subscription(&sub.reference_url, "PT60S").await {
        Ok(new_time) => println!("  New termination time: {new_time}"),
        Err(e) => println!("  (skipped — {e})"),
    }

    // Unsubscribe
    section("Unsubscribe");
    match client.unsubscribe(&sub.reference_url).await {
        Ok(()) => println!("  Unsubscribed successfully"),
        Err(e) => println!("  (skipped — {e})"),
    }

    Ok(())
}

// ── Example 13: WS-Discovery ──────────────────────────────────────────────────

async fn discovery_example() -> Result<(), OnvifError> {
    println!("=== WS-Discovery (3 second probe) ===");
    println!("Sending multicast Probe to 239.255.255.250:3702 ...");

    let devices = oxvif::discovery::probe(Duration::from_secs(3)).await;

    if devices.is_empty() {
        println!("No ONVIF devices found on local network.");
        println!("Tip: ensure the camera is on the same L2 segment and responds to WS-Discovery.");
    } else {
        println!("Found {} device(s):", devices.len());
        for (i, d) in devices.iter().enumerate() {
            println!("\n  [{i}] {}", d.endpoint);
            for addr in &d.xaddrs {
                println!("      XAddr : {addr}");
            }
            for scope in &d.scopes {
                if scope.contains("onvif.org") {
                    println!("      Scope : {scope}");
                }
            }
        }
    }

    Ok(())
}

// ── Example 14: error handling ────────────────────────────────────────────────

async fn error_handling_example(cfg: &Config) -> Result<(), OnvifError> {
    use oxvif::error::OnvifError as Err_;
    use oxvif::soap::SoapError;
    use oxvif::transport::TransportError;

    println!("=== Error handling ===");
    println!("Connecting to {} ...", cfg.camera_url);

    let client = OnvifClient::new(&cfg.camera_url).with_credentials(&cfg.username, &cfg.password);

    match client.get_capabilities().await {
        Ok(caps) => {
            println!("Connected successfully.");
            print_capabilities(&caps);
        }
        Err(Err_::Transport(TransportError::Http(e))) => {
            eprintln!("Network error: {e}");
            eprintln!("Check that the camera is reachable at {}", cfg.camera_url);
        }
        Err(Err_::Transport(TransportError::HttpStatus { status, body })) => {
            eprintln!("HTTP {status} from device");
            if !body.is_empty() {
                eprintln!("Body: {body}");
            }
        }
        Err(Err_::Soap(SoapError::Fault { code, reason })) => {
            eprintln!("SOAP Fault [{code}]: {reason}");
            eprintln!("Tip: verify username / password.");
        }
        Err(e) => {
            eprintln!("Unexpected error: {e}");
        }
    }

    Ok(())
}
