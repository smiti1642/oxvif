//! ODM Compatibility Check
//!
//! Measures how complete oxvif's coverage is against every ONVIF API used by
//! ONVIF Device Manager (ODM v2.2.250). See `ODM.md` in the oxdm repo for the
//! full reference table.
//!
//! ```sh
//! cargo run --example odm_compat
//! ```
//!
//! Required:  `ONVIF_URL=http://<ip>/onvif/device_service`
//! Optional:  `ONVIF_USERNAME` (default: admin)  `ONVIF_PASSWORD` (default: empty)
//!
//! Output legend:
//!   `PASS`     — oxvif has the API and the device responded successfully
//!   `FAIL`     — oxvif has the API but the device returned an error
//!   `SKIP`     — oxvif has the API (write op, destructive, or needs complex prereq)
//!   `NOT_IMPL` — oxvif does not implement this ONVIF method yet

use std::{env, fmt, time::Duration};

use oxvif::{OnvifClient, OnvifSession, discovery};

// ── Config ────────────────────────────────────────────────────────────────────

struct Config {
    camera_url: String,
    username: String,
    password: String,
}

impl Config {
    fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        Self {
            camera_url: env::var("ONVIF_URL").expect(
                "ONVIF_URL not set — e.g. ONVIF_URL=http://192.168.1.10/onvif/device_service",
            ),
            username: env::var("ONVIF_USERNAME").unwrap_or_else(|_| "admin".into()),
            password: env::var("ONVIF_PASSWORD").unwrap_or_default(),
        }
    }
}

// ── Report ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Status {
    Pass(String),
    Fail(String),
    Skip(String),
    NotImpl,
}

struct Check {
    method: &'static str,
    status: Status,
}

struct Report {
    sections: Vec<(&'static str, Vec<Check>)>,
}

impl Report {
    fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    fn section(&mut self, name: &'static str) {
        self.sections.push((name, Vec::new()));
    }

    fn checks(&mut self) -> &mut Vec<Check> {
        &mut self.sections.last_mut().expect("call section() first").1
    }

    fn pass(&mut self, method: &'static str, info: impl fmt::Display) {
        self.checks().push(Check {
            method,
            status: Status::Pass(info.to_string()),
        });
    }

    fn fail(&mut self, method: &'static str, err: impl fmt::Display) {
        self.checks().push(Check {
            method,
            status: Status::Fail(err.to_string()),
        });
    }

    fn skip(&mut self, method: &'static str, reason: &'static str) {
        self.checks().push(Check {
            method,
            status: Status::Skip(reason.into()),
        });
    }

    fn not_impl(&mut self, method: &'static str) {
        self.checks().push(Check {
            method,
            status: Status::NotImpl,
        });
    }

    /// Record a result; also returns whether it passed (for prerequisite chaining).
    fn record<T, E: fmt::Display>(
        &mut self,
        method: &'static str,
        result: Result<T, E>,
        info: impl FnOnce(&T) -> String,
    ) -> Option<T> {
        match result {
            Ok(v) => {
                let s = info(&v);
                self.checks().push(Check {
                    method,
                    status: Status::Pass(s),
                });
                Some(v)
            }
            Err(e) => {
                self.checks().push(Check {
                    method,
                    status: Status::Fail(e.to_string()),
                });
                None
            }
        }
    }

    fn print(&self) {
        const W: usize = 72;
        for (section, checks) in &self.sections {
            println!("\n{section}");
            println!("{}", "─".repeat(W));
            for c in checks {
                let (tag, detail) = match &c.status {
                    Status::Pass(s) => ("PASS    ", s.as_str()),
                    Status::Fail(s) => ("FAIL    ", s.as_str()),
                    Status::Skip(s) => ("SKIP    ", s.as_str()),
                    Status::NotImpl => ("NOT_IMPL", ""),
                };
                let detail = if detail.len() > 54 {
                    &detail[..51]
                } else {
                    detail
                };
                println!("  [{tag}]  {:<44}  {detail}", c.method);
            }
        }

        let all: Vec<&Check> = self.sections.iter().flat_map(|(_, v)| v).collect();
        let pass = all
            .iter()
            .filter(|c| matches!(c.status, Status::Pass(_)))
            .count();
        let fail = all
            .iter()
            .filter(|c| matches!(c.status, Status::Fail(_)))
            .count();
        let skip = all
            .iter()
            .filter(|c| matches!(c.status, Status::Skip(_)))
            .count();
        let not_impl = all
            .iter()
            .filter(|c| matches!(c.status, Status::NotImpl))
            .count();
        let total = all.len();
        let oxvif = pass + fail + skip;

        println!("\n{}", "═".repeat(W));
        println!("  ODM methods checked          : {total}");
        println!(
            "  oxvif implements             : {oxvif} / {total}  ({:.0}%)",
            oxvif as f64 / total as f64 * 100.0
        );
        println!("  PASS  (read — OK)            : {pass}");
        println!("  FAIL  (read — device error)  : {fail}");
        println!("  SKIP  (write / no prereq)    : {skip}");
        println!("  NOT_IMPL                     : {not_impl}");
        println!("{}", "═".repeat(W));
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    let mut r = Report::new();

    // ── §1 WS-Discovery ──────────────────────────────────────────────────────
    r.section("§1  WS-Discovery");
    {
        let found = discovery::probe(Duration::from_secs(3)).await;
        r.pass("probe()", format!("found {} device(s)", found.len()));
        r.skip(
            "listen()",
            "passive 30 s monitor — omitted to keep check fast",
        );
    }

    // ── Connect ───────────────────────────────────────────────────────────────
    eprintln!("Connecting to {} …", cfg.camera_url);
    let session = match OnvifSession::builder(&cfg.camera_url)
        .with_credentials(&cfg.username, &cfg.password)
        .with_clock_sync()
        .build()
        .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Connection failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("Connected.\n");

    // Raw client — used for methods not exposed on the session.
    let client = OnvifClient::new(&cfg.camera_url).with_credentials(&cfg.username, &cfg.password);

    // ── §2 Device Service — information ──────────────────────────────────────
    r.section("§2  Device Service — information");
    r.record(
        "GetDeviceInformation",
        session.get_device_info().await,
        |d| format!("{} {} fw={}", d.manufacturer, d.model, d.firmware_version),
    );
    r.record("GetCapabilities", client.get_capabilities().await, |c| {
        let mut s = Vec::new();
        if c.media.url.is_some() {
            s.push("media");
        }
        if c.media2.url.is_some() {
            s.push("media2");
        }
        if c.ptz.url.is_some() {
            s.push("ptz");
        }
        if c.imaging.url.is_some() {
            s.push("imaging");
        }
        if c.events.url.is_some() {
            s.push("events");
        }
        if c.recording.url.is_some() {
            s.push("recording");
        }
        s.join(" ")
    });
    r.record("GetServices", session.get_services().await, |s| {
        format!("{} service(s)", s.len())
    });
    r.record("GetScopes", session.get_scopes().await, |s| {
        s.iter()
            .find(|sc| sc.contains("/name/"))
            .cloned()
            .unwrap_or_default()
    });
    r.not_impl("GetWsdlUrl");

    // ── §2 Device Service — network ───────────────────────────────────────────
    r.section("§2  Device Service — network");
    r.record(
        "GetNetworkInterfaces",
        session.get_network_interfaces().await,
        |ni| {
            ni.iter()
                .map(|i| i.token.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    r.record("GetHostname", session.get_hostname().await, |h| {
        format!(
            "{} (dhcp={})",
            h.name.as_deref().unwrap_or("—"),
            h.from_dhcp
        )
    });
    r.skip("SetHostname", "write operation");
    r.not_impl("SetHostnameFromDHCP");
    r.record("GetDNS", session.get_dns().await, |d| {
        format!("dhcp={} servers=[{}]", d.from_dhcp, d.servers.join(", "))
    });
    r.skip("SetDNS", "write operation");
    r.record("GetNTP", session.get_ntp().await, |n| {
        format!("dhcp={}", n.from_dhcp)
    });
    r.skip("SetNTP", "write operation");
    r.record(
        "GetNetworkDefaultGateway",
        session.get_network_default_gateway().await,
        |g| g.ipv4_addresses.first().cloned().unwrap_or_default(),
    );
    r.skip("SetNetworkDefaultGateway", "write operation");
    r.record(
        "GetNetworkProtocols",
        session.get_network_protocols().await,
        |ps| {
            ps.iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    r.skip("SetNetworkProtocols", "write operation");
    r.not_impl("GetZeroConfiguration");
    r.not_impl("SetZeroConfiguration");
    r.not_impl("GetDynamicDNS");
    r.not_impl("SetDynamicDNS");
    r.not_impl("GetIPAddressFilter");
    r.not_impl("SetIPAddressFilter");
    r.record(
        "GetDiscoveryMode",
        session.get_discovery_mode().await,
        |m| m.clone(),
    );
    r.skip("SetDiscoveryMode", "write operation");
    r.not_impl("GetRemoteDiscoveryMode");
    r.not_impl("SetRemoteDiscoveryMode");
    r.not_impl("GetDPAddresses");
    r.not_impl("SetDPAddresses");

    // ── §2 Device Service — time ──────────────────────────────────────────────
    r.section("§2  Device Service — time");
    r.record(
        "GetSystemDateAndTime",
        session.get_system_date_and_time().await,
        |dt| format!("utc_offset={}s", dt.utc_offset_secs()),
    );
    r.skip("SetSystemDateAndTime", "write operation");

    // ── §2 Device Service — users ─────────────────────────────────────────────
    r.section("§2  Device Service — users");
    r.record("GetUsers", session.get_users().await, |us| {
        us.iter()
            .map(|u| u.username.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });
    r.skip("CreateUsers", "write operation");
    r.skip("SetUser", "write operation");
    r.skip("DeleteUsers", "write operation");

    // ── §2 Device Service — scopes ────────────────────────────────────────────
    r.section("§2  Device Service — scopes");
    r.skip("SetScopes", "write operation");
    r.not_impl("AddScopes");
    r.not_impl("RemoveScopes");

    // ── §2 Device Service — I/O ───────────────────────────────────────────────
    r.section("§2  Device Service — I/O");
    r.record("GetRelayOutputs", session.get_relay_outputs().await, |rs| {
        format!("{} relay(s)", rs.len())
    });
    r.skip("SetRelayOutputSettings", "write operation");
    r.skip("SetRelayOutputState", "write operation");

    // ── §2 Device Service — certificates ─────────────────────────────────────
    r.section("§2  Device Service — certificates");
    for m in [
        "GetCertificates",
        "GetCertificatesStatus",
        "CreateCertificate",
        "DeleteCertificates",
        "SetCertificatesStatus",
        "LoadCertificates",
        "GetPkcs10Request",
        "GetAccessPolicy",
        "SetAccessPolicy",
        "SetClientCertificateMode",
    ] {
        r.not_impl(m);
    }

    // ── §2 Device Service — maintenance ──────────────────────────────────────
    r.section("§2  Device Service — maintenance");
    r.skip("SystemReboot", "destructive — not called");
    r.not_impl("GetSystemBackup");
    r.not_impl("RestoreSystem");
    r.skip("SetSystemFactoryDefault", "destructive — not called");
    r.not_impl("UpgradeSystemFirmware");
    r.not_impl("StartFirmwareUpgrade");
    r.record(
        "GetSystemLog",
        session.get_system_log("System").await,
        |l| format!("{} char(s)", l.string.as_deref().unwrap_or("").len()),
    );
    r.not_impl("GetSystemSupportInformation");

    // ── §3 Media Service — profiles ───────────────────────────────────────────
    r.section("§3  Media Service — profiles");
    let profiles_result = session.get_profiles().await;
    let first_profile_token: Option<String> = profiles_result
        .as_ref()
        .ok()
        .and_then(|ps| ps.first())
        .map(|p| p.token.clone());
    r.record("GetProfiles", profiles_result, |ps| {
        ps.iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });
    if let Some(tok) = &first_profile_token {
        r.record("GetProfile", session.get_profile(tok).await, |p| {
            p.name.clone()
        });
    } else {
        r.skip("GetProfile", "no profiles found");
    }
    r.skip("CreateProfile", "write operation");
    r.skip("DeleteProfile", "write operation");
    r.record("GetVideoSources", session.get_video_sources().await, |vs| {
        format!("{} source(s)", vs.len())
    });
    r.record("GetAudioSources", session.get_audio_sources().await, |a| {
        format!("{} source(s)", a.len())
    });
    r.skip("AddVideoSourceConfiguration", "write operation");
    r.skip("RemoveVideoSourceConfiguration", "write operation");
    r.skip("AddVideoEncoderConfiguration", "write operation");
    r.skip("RemoveVideoEncoderConfiguration", "write operation");
    r.skip("AddAudioSourceConfiguration", "write operation");
    r.skip("RemoveAudioSourceConfiguration", "write operation");
    r.not_impl("AddPTZConfiguration");
    r.not_impl("RemovePTZConfiguration");
    r.not_impl("AddMetadataConfiguration");
    r.not_impl("RemoveMetadataConfiguration");
    r.skip("AddVideoAnalyticsConfiguration", "write operation");
    r.skip("RemoveVideoAnalyticsConfiguration", "write operation");

    // ── §3 Media Service — video encoder ──────────────────────────────────────
    r.section("§3  Media Service — video encoder");
    let vec_result = session.get_video_encoder_configurations().await;
    let first_vec_token: Option<String> = vec_result
        .as_ref()
        .ok()
        .and_then(|cs| cs.first())
        .map(|c| c.token.clone());
    r.record("GetVideoEncoderConfigurations", vec_result, |cs| {
        format!("{} config(s)", cs.len())
    });
    if let Some(tok) = &first_vec_token {
        r.record(
            "GetVideoEncoderConfiguration",
            session.get_video_encoder_configuration(tok).await,
            |c| {
                format!(
                    "{:?} {}x{}",
                    c.encoding, c.resolution.width, c.resolution.height
                )
            },
        );
        r.record(
            "GetVideoEncoderConfigurationOptions",
            session
                .get_video_encoder_configuration_options(Some(tok.as_str()))
                .await,
            |_| "ok".into(),
        );
    } else {
        r.skip("GetVideoEncoderConfiguration", "no video encoder configs");
        r.skip(
            "GetVideoEncoderConfigurationOptions",
            "no video encoder configs",
        );
    }
    r.skip("SetVideoEncoderConfiguration", "write operation");
    r.not_impl("GetCompatibleVideoEncoderConfigurations");
    r.not_impl("GetGuaranteedNumberOfVideoEncoderInstances");

    // ── §3 Media Service — video source ───────────────────────────────────────
    r.section("§3  Media Service — video source");
    let vsc_result = session.get_video_source_configurations().await;
    let first_vsc_token: Option<String> = vsc_result
        .as_ref()
        .ok()
        .and_then(|cs| cs.first())
        .map(|c| c.token.clone());
    let first_vsrc_token: Option<String> = vsc_result
        .as_ref()
        .ok()
        .and_then(|cs| cs.first())
        .map(|c| c.source_token.clone());
    r.record("GetVideoSourceConfigurations", vsc_result, |cs| {
        format!("{} config(s)", cs.len())
    });
    if let Some(tok) = &first_vsc_token {
        r.record(
            "GetVideoSourceConfiguration",
            session.get_video_source_configuration(tok).await,
            |c| format!("src={}", c.source_token),
        );
        r.record(
            "GetVideoSourceConfigurationOptions",
            session
                .get_video_source_configuration_options(Some(tok.as_str()))
                .await,
            |_| "ok".into(),
        );
    } else {
        r.skip("GetVideoSourceConfiguration", "no video source configs");
        r.skip(
            "GetVideoSourceConfigurationOptions",
            "no video source configs",
        );
    }
    r.skip("SetVideoSourceConfiguration", "write operation");
    r.not_impl("GetCompatibleVideoSourceConfigurations");

    // ── §3 Media Service — audio ──────────────────────────────────────────────
    r.section("§3  Media Service — audio");
    let aec_result = session.get_audio_encoder_configurations().await;
    let first_aec_token: Option<String> = aec_result
        .as_ref()
        .ok()
        .and_then(|cs| cs.first())
        .map(|c| c.token.clone());
    r.record("GetAudioEncoderConfigurations", aec_result, |cs| {
        format!("{} config(s)", cs.len())
    });
    if let Some(tok) = &first_aec_token {
        r.record(
            "GetAudioEncoderConfiguration",
            session.get_audio_encoder_configuration(tok).await,
            |c| format!("{:?}", c.encoding),
        );
        r.record(
            "GetAudioEncoderConfigurationOptions",
            session.get_audio_encoder_configuration_options(tok).await,
            |_| "ok".into(),
        );
    } else {
        r.skip("GetAudioEncoderConfiguration", "no audio encoder configs");
        r.skip(
            "GetAudioEncoderConfigurationOptions",
            "no audio encoder configs",
        );
    }
    r.skip("SetAudioEncoderConfiguration", "write operation");
    r.record(
        "GetAudioSourceConfigurations",
        session.get_audio_source_configurations().await,
        |cs| format!("{} config(s)", cs.len()),
    );
    r.not_impl("GetAudioSourceConfiguration");
    r.not_impl("SetAudioSourceConfiguration");
    r.not_impl("GetAudioSourceConfigurationOptions");
    r.not_impl("GetCompatibleAudioEncoderConfigurations");
    r.not_impl("GetCompatibleAudioSourceConfigurations");

    // ── §3 Media Service — metadata ────────────────────────────────────────────
    r.section("§3  Media Service — metadata");
    r.not_impl("GetMetadataConfigurations");
    r.not_impl("GetMetadataConfiguration");
    r.not_impl("SetMetadataConfiguration");
    r.not_impl("GetMetadataConfigurationOptions");
    r.not_impl("GetCompatibleMetadataConfigurations");

    // ── §3 Media Service — stream & snapshot ──────────────────────────────────
    r.section("§3  Media Service — stream & snapshot");
    if let Some(tok) = &first_profile_token {
        r.record("GetStreamUri", session.get_stream_uri(tok).await, |u| {
            u.uri.clone()
        });
        r.record("GetSnapshotUri", session.get_snapshot_uri(tok).await, |u| {
            u.uri.clone()
        });
    } else {
        r.skip("GetStreamUri", "no profiles found");
        r.skip("GetSnapshotUri", "no profiles found");
    }
    r.not_impl("SetSynchronizationPoint");
    r.not_impl("StartMulticastStreaming");
    r.not_impl("StopMulticastStreaming");

    // ── §3 Media Service — video analytics ────────────────────────────────────
    r.section("§3  Media Service — video analytics");
    r.not_impl("GetVideoAnalyticsConfigurations");
    r.not_impl("GetVideoAnalyticsConfiguration");
    r.not_impl("SetVideoAnalyticsConfiguration");
    r.not_impl("GetCompatibleVideoAnalyticsConfigurations");

    // ── §4 PTZ — config ────────────────────────────────────────────────────────
    r.section("§4  PTZ — config");
    let ptz_nodes = session.ptz_get_nodes().await;
    r.record("GetNodes", ptz_nodes, |ns| format!("{} node(s)", ns.len()));
    let ptz_cfgs_result = session.ptz_get_configurations().await;
    let first_ptz_cfg_token: Option<String> = ptz_cfgs_result
        .as_ref()
        .ok()
        .and_then(|cs| cs.first())
        .map(|c| c.token.clone());
    r.record("GetConfigurations", ptz_cfgs_result, |cs| {
        format!("{} config(s)", cs.len())
    });
    if let Some(tok) = &first_ptz_cfg_token {
        r.record(
            "GetConfiguration",
            session.ptz_get_configuration(tok).await,
            |c| c.name.clone(),
        );
        r.record(
            "GetConfigurationOptions",
            session.ptz_get_configuration_options(tok).await,
            |_| "ok".into(),
        );
    } else {
        r.skip(
            "GetConfiguration",
            "no PTZ configs (device may not have PTZ)",
        );
        r.skip("GetConfigurationOptions", "no PTZ configs");
    }
    r.skip("SetConfiguration", "write operation");

    // ── §4 PTZ — presets ───────────────────────────────────────────────────────
    r.section("§4  PTZ — presets");
    if let Some(tok) = &first_profile_token {
        r.record("GetPresets", session.ptz_get_presets(tok).await, |ps| {
            format!("{} preset(s)", ps.len())
        });
    } else {
        r.skip("GetPresets", "no profiles found");
    }
    r.skip("SetPreset", "write operation");
    r.skip("RemovePreset", "write operation");
    r.skip("GotoPreset", "moves camera — not called");

    // ── §4 PTZ — move ──────────────────────────────────────────────────────────
    r.section("§4  PTZ — move");
    r.skip("AbsoluteMove", "moves camera — not called");
    r.skip("RelativeMove", "moves camera — not called");
    r.skip("ContinuousMove", "moves camera — not called");
    r.skip("Stop", "write operation");
    r.not_impl("SendAuxiliaryCommand");

    // ── §4 PTZ — home position ─────────────────────────────────────────────────
    r.section("§4  PTZ — home position");
    r.skip("GotoHomePosition", "moves camera — not called");
    r.skip("SetHomePosition", "write operation");

    // ── §4 PTZ — status ────────────────────────────────────────────────────────
    r.section("§4  PTZ — status");
    if let Some(tok) = &first_profile_token {
        r.record("GetStatus", session.ptz_get_status(tok).await, |s| {
            format!("pan={:?} tilt={:?} zoom={:?}", s.pan, s.tilt, s.zoom)
        });
    } else {
        r.skip("GetStatus", "no profiles found");
    }

    // ── §5 Imaging Service ─────────────────────────────────────────────────────
    r.section("§5  Imaging Service");
    if let Some(vsrc) = &first_vsrc_token {
        r.record(
            "GetImagingSettings",
            session.get_imaging_settings(vsrc).await,
            |s| format!("brightness={:?}", s.brightness),
        );
        r.skip("SetImagingSettings", "write operation");
        r.record(
            "GetOptions",
            session.get_imaging_options(vsrc).await,
            |_| "ok".into(),
        );
        r.record("GetStatus", session.imaging_get_status(vsrc).await, |_| {
            "ok".into()
        });
        r.record(
            "GetMoveOptions",
            session.imaging_get_move_options(vsrc).await,
            |_| "ok".into(),
        );
        r.skip("Move", "moves focus motor — not called");
        r.skip("Stop", "write operation");
    } else {
        for m in [
            "GetImagingSettings",
            "SetImagingSettings",
            "GetOptions",
            "GetStatus",
            "GetMoveOptions",
            "Move",
            "Stop",
        ] {
            r.skip(m, "no video source found");
        }
    }

    // ── §6 Events / Notification ───────────────────────────────────────────────
    r.section("§6  Events / Notification");
    r.record(
        "GetEventProperties",
        session.get_event_properties().await,
        |ep| format!("{} topic(s)", ep.topics.len()),
    );
    let sub_result = session
        .create_pull_point_subscription(None, Some("PT60S"))
        .await;
    match sub_result {
        Ok(ref sub) => r.pass("CreatePullPointSubscription", &sub.reference_url),
        Err(ref e) => r.fail("CreatePullPointSubscription", e),
    }
    if let Ok(sub) = sub_result {
        r.skip("PullMessages", "requires polling loop — not called");
        r.skip("RenewSubscription", "write operation");
        match session.unsubscribe(&sub.reference_url).await {
            Ok(_) => r.pass("Unsubscribe", "cleaned up"),
            Err(e) => r.fail("Unsubscribe", e),
        }
    } else {
        r.skip("PullMessages", "CreatePullPointSubscription failed");
        r.skip("RenewSubscription", "CreatePullPointSubscription failed");
        r.skip("Unsubscribe", "CreatePullPointSubscription failed");
    }
    r.skip(
        "Subscribe (push)",
        "requires a local HTTP listener endpoint",
    );
    r.not_impl("SetSynchronizationPoint");
    r.not_impl("GetCurrentMessage");

    // ── §7 Analytics Service ───────────────────────────────────────────────────
    r.section("§7  Analytics Service");
    for m in [
        "GetSupportedAnalyticsModules",
        "GetAnalyticsModules",
        "CreateAnalyticsModules",
        "ModifyAnalyticsModules",
        "DeleteAnalyticsModules",
        "GetSupportedRules",
        "GetRules",
        "CreateRules",
        "ModifyRules",
        "DeleteRules",
    ] {
        r.not_impl(m);
    }

    // ── §8 Recording Service — recordings ─────────────────────────────────────
    r.section("§8  Recording Service — recordings");
    let recordings_result = session.get_recordings().await;
    let first_rec_token: Option<String> = recordings_result
        .as_ref()
        .ok()
        .and_then(|rs| rs.first())
        .map(|r| r.token.clone());
    r.record("GetRecordings", recordings_result, |rs| {
        format!("{} recording(s)", rs.len())
    });
    r.skip("CreateRecording", "write operation");
    r.not_impl("GetRecordingConfiguration");
    r.skip("SetRecordingConfiguration", "write operation");
    r.skip("DeleteRecording", "destructive — not called");

    // ── §8 Recording Service — tracks ─────────────────────────────────────────
    r.section("§8  Recording Service — tracks");
    r.skip("CreateTrack", "write operation");
    r.not_impl("GetTrackConfiguration");
    r.not_impl("SetTrackConfiguration");
    r.skip("DeleteTrack", "destructive — not called");

    // ── §8 Recording Service — jobs ────────────────────────────────────────────
    r.section("§8  Recording Service — jobs");
    let jobs_result = session.get_recording_jobs().await;
    let first_job_token: Option<String> = jobs_result
        .as_ref()
        .ok()
        .and_then(|js| js.first())
        .map(|j| j.token.clone());
    r.record("GetRecordingJobs", jobs_result, |js| {
        format!("{} job(s)", js.len())
    });
    r.skip("CreateRecordingJob", "write operation");
    r.not_impl("GetRecordingJobConfiguration");
    r.skip("SetRecordingJobMode", "write operation");
    if let Some(tok) = &first_job_token {
        r.record(
            "GetRecordingJobState",
            session.get_recording_job_state(tok).await,
            |s| s.active_state.clone(),
        );
    } else {
        r.skip("GetRecordingJobState", "no recording jobs found");
    }
    r.skip("DeleteRecordingJob", "destructive — not called");

    // ── §9 Search Service ──────────────────────────────────────────────────────
    r.section("§9  Search Service");
    r.not_impl("GetRecordingSummary");
    r.not_impl("GetRecordingInformation");
    r.not_impl("GetMediaAttributes");
    // find_recordings / get_recording_search_results exist but need recording service URL
    if first_rec_token.is_some() {
        r.skip(
            "FindRecordings",
            "exists in oxvif — needs active search session",
        );
        r.skip(
            "GetRecordingSearchResults",
            "exists in oxvif — needs active search session",
        );
        r.skip("EndSearch", "exists in oxvif — needs active search session");
    } else {
        r.skip("FindRecordings", "no recordings on device");
        r.skip("GetRecordingSearchResults", "no recordings on device");
        r.skip("EndSearch", "no recordings on device");
    }
    r.not_impl("FindEvents");
    r.not_impl("GetEventSearchResults");
    r.not_impl("FindMetadata");
    r.not_impl("GetMetadataSearchResults");
    r.not_impl("FindPTZPosition");
    r.not_impl("GetPTZPositionSearchResults");
    r.not_impl("GetSearchState");

    // ── §10 Replay Service ─────────────────────────────────────────────────────
    r.section("§10 Replay Service");
    if let Some(tok) = &first_rec_token {
        r.record(
            "GetReplayUri",
            session.get_replay_uri(tok, "RTP-Unicast", "RTSP").await,
            |u| u.clone(),
        );
    } else {
        r.skip("GetReplayUri", "no recordings on device");
    }
    r.not_impl("GetReplayConfiguration");
    r.not_impl("SetReplayConfiguration");

    // ── §11 Display Service ────────────────────────────────────────────────────
    r.section("§11 Display Service");
    for m in [
        "GetLayout",
        "SetLayout",
        "GetDisplayOptions",
        "GetPaneConfigurations",
        "GetPaneConfiguration",
        "SetPaneConfigurations",
        "SetPaneConfiguration",
        "CreatePaneConfiguration",
        "DeletePaneConfiguration",
    ] {
        r.not_impl(m);
    }

    // ── §12 Receiver Service ───────────────────────────────────────────────────
    r.section("§12 Receiver Service");
    for m in [
        "GetReceivers",
        "GetReceiver",
        "CreateReceiver",
        "ConfigureReceiver",
        "SetReceiverMode",
        "GetReceiverState",
        "DeleteReceiver",
    ] {
        r.not_impl(m);
    }

    // ── §13 Action Engine Service ──────────────────────────────────────────────
    r.section("§13 Action Engine Service");
    for m in [
        "GetSupportedActions",
        "GetActions",
        "CreateActions",
        "ModifyActions",
        "DeleteActions",
        "GetActionTriggers",
        "CreateActionTriggers",
        "ModifyActionTriggers",
        "DeleteActionTriggers",
    ] {
        r.not_impl(m);
    }

    // ── Print ─────────────────────────────────────────────────────────────────
    r.print();
}
