//! NET 1: full mock action snapshot.
//!
//! Black-box: drives the public `oxvif` API against `oxvif::mock::MockTransport`
//! only, so it lives in `tests/` rather than inside the library crate.
//!
//! Run with: `cargo test --features mock --test mock_action_snapshot`
//!
//! See the doc comment on `mock_action_snapshot_matches_expected_list`.
#![cfg(feature = "mock")]

use oxvif::OnvifClient;
use oxvif::error::OnvifError;
use oxvif::mock::MockTransport as MockDevice;
use oxvif::soap::SoapError;
use std::sync::Arc;

// Only the SOAP *action* drives `MockTransport`'s dispatcher, so these URLs
// are cosmetic — they exist to keep the call sites readable.
const DEVICE: &str = "http://mock/onvif/device_service";
const DEVICEIO: &str = "http://mock/onvif/deviceio_service";
const MEDIA: &str = "http://mock/onvif/media_service";
const MEDIA2: &str = "http://mock/onvif/media2_service";
const PTZ: &str = "http://mock/onvif/ptz_service";
const IMAGING: &str = "http://mock/onvif/imaging_service";
const EVENTS: &str = "http://mock/onvif/events_service";
const RECORDING: &str = "http://mock/onvif/recording_service";
const SEARCH: &str = "http://mock/onvif/search_service";
const REPLAY: &str = "http://mock/onvif/replay_service";
const SUBSCRIPTION: &str = "http://mock/onvif/subscription/1";

// Tokens the default `MockState` actually knows — see `src/mock/state.rs`
// (`default_profiles`, `default_video_encoder`, `default_osd`, …) and the
// static responses in `src/mock/services/*.rs`.
const PROFILE: &str = "Profile_1";
const PROFILE2: &str = "Profile_2";
const VSC: &str = "VSC_1";
const VEC: &str = "VEC_1";
// A real sensor in the default state — Imaging is per-VideoSourceToken and
// the mock now refuses a token it does not know.
const VIDEO_SOURCE: &str = "VS_1";
const AEC: &str = "AEC_1";
const OSD: &str = "OSD_1";
const PTZ_NODE: &str = "PTZNode_1";
const PTZ_CONFIG: &str = "PTZConfig_1";
const PRESET: &str = "Preset_1";
const RELAY: &str = "RelayOutput_1";
const IFACE: &str = "eth0";
const RECORDING_TOKEN: &str = "Rec_001";
const TRACK: &str = "VIDEO001";
const JOB: &str = "Job_001";
const SEARCH_TOKEN: &str = "search_mock_001";
const VS_MODE: &str = "Mode_1";

/// A client wired to a *fresh* mock device. One per probe, so a write in
/// one operation cannot perturb the outcome of the next.
fn fresh() -> OnvifClient {
    OnvifClient::new(DEVICE).with_transport(Arc::new(MockDevice::new()))
}

/// Collapse a client result into a short, stable outcome string.
///
/// The error arms are deliberately lossless about *which* error it was —
/// pinning "it failed" alone would let a stage swap one failure mode for
/// another unnoticed.
fn outcome<T>(r: Result<T, OnvifError>) -> String {
    match r {
        Ok(_) => "ok".to_string(),
        Err(OnvifError::Soap(SoapError::UnexpectedResponse(tag))) => {
            format!("unexpected-response:{tag}")
        }
        Err(OnvifError::Soap(SoapError::MissingField(f))) => format!("missing-field:{f}"),
        Err(OnvifError::Soap(SoapError::MissingBody)) => "missing-body".to_string(),
        Err(OnvifError::Soap(SoapError::XmlParse(e))) => format!("xml-parse:{e}"),
        Err(OnvifError::Soap(SoapError::InvalidValue { field, value })) => {
            format!("invalid-value:{field}={value}")
        }
        Err(OnvifError::Soap(SoapError::Fault { code, reason, .. })) => {
            format!("soap-fault:{code}:{reason}")
        }
        Err(OnvifError::InvalidArgument(m)) => format!("invalid-argument:{m}"),
        Err(OnvifError::Transport(e)) => format!("transport:{e}"),
    }
}

/// `probe!(out, "name", method(args…))` — run `method` on a fresh mock and
/// push its outcome under `name`.
macro_rules! probe {
    ($out:ident, $name:literal, $method:ident ( $($arg:expr),* $(,)? )) => {{
        let c = fresh();
        $out.push(($name, outcome(c.$method($($arg),*).await)));
    }};
}

/// EXPECTED — the hand-maintained snapshot of what every ONVIF operation
/// `OnvifClient` exposes currently does against `MockTransport`.
///
/// **This list is written by hand on purpose.** It is not derived from the
/// run, so when a stage fixes (or breaks) an operation this test fails and
/// forces a deliberate one-line edit here. Do not "make it pass" by
/// regenerating it.
const EXPECTED: &[(&str, &str)] = &[
    // ── Device ────────────────────────────────────────────────────────────
    ("get_capabilities", "ok"),
    ("get_services", "ok"),
    ("get_system_date_and_time", "ok"),
    ("set_system_date_and_time", "ok"),
    ("get_device_info", "ok"),
    ("get_hostname", "ok"),
    ("set_hostname", "ok"),
    ("get_ntp", "ok"),
    ("set_ntp", "ok"),
    ("system_reboot", "ok"),
    ("get_scopes", "ok"),
    ("set_scopes", "ok"),
    ("get_users", "ok"),
    ("create_users", "ok"),
    ("delete_users", "ok"),
    ("set_user", "ok"),
    ("get_network_interfaces", "ok"),
    ("set_network_interfaces", "ok"),
    ("get_network_protocols", "ok"),
    ("set_network_protocols", "ok"),
    ("get_dns", "ok"),
    ("set_dns", "ok"),
    ("get_network_default_gateway", "ok"),
    ("set_network_default_gateway", "ok"),
    ("send_auxiliary_command", "ok"),
    ("get_system_log", "ok"),
    ("get_relay_outputs", "ok"),
    ("set_relay_output_settings", "ok"),
    ("set_relay_output_state", "ok"),
    ("get_digital_inputs", "ok"),
    ("set_system_factory_default", "ok"),
    ("get_storage_configurations", "ok"),
    ("set_storage_configuration", "ok"),
    ("get_system_uris", "ok"),
    ("start_firmware_upgrade", "ok"),
    ("start_system_restore", "ok"),
    ("get_discovery_mode", "ok"),
    ("set_discovery_mode", "ok"),
    // ── Events ────────────────────────────────────────────────────────────
    ("get_event_properties", "ok"),
    ("create_pull_point_subscription", "ok"),
    ("pull_messages", "ok"),
    ("renew_subscription", "ok"),
    ("set_synchronization_point", "ok"),
    ("unsubscribe", "ok"),
    ("subscribe", "ok"),
    // ── Imaging ───────────────────────────────────────────────────────────
    ("get_imaging_settings", "ok"),
    ("set_imaging_settings", "ok"),
    ("get_imaging_options", "ok"),
    ("imaging_move", "ok"),
    ("imaging_stop", "ok"),
    ("imaging_get_move_options", "ok"),
    ("imaging_get_status", "ok"),
    // ── Media1 ────────────────────────────────────────────────────────────
    ("get_profiles", "ok"),
    ("get_stream_uri", "ok"),
    ("get_snapshot_uri", "ok"),
    ("create_profile", "ok"),
    ("delete_profile", "ok"),
    ("get_profile", "ok"),
    ("add_video_encoder_configuration", "ok"),
    ("remove_video_encoder_configuration", "ok"),
    ("add_video_source_configuration", "ok"),
    ("remove_video_source_configuration", "ok"),
    ("get_video_sources", "ok"),
    ("get_video_source_configurations", "ok"),
    ("get_video_source_configuration", "ok"),
    ("set_video_source_configuration", "ok"),
    ("get_video_source_configuration_options", "ok"),
    ("get_video_encoder_configurations", "ok"),
    ("get_video_encoder_configuration", "ok"),
    ("set_video_encoder_configuration", "ok"),
    ("get_video_encoder_configuration_options", "ok"),
    ("get_osds", "ok"),
    ("get_osd", "ok"),
    ("set_osd", "ok"),
    ("create_osd", "ok"),
    ("delete_osd", "ok"),
    ("get_osd_options", "ok"),
    ("get_audio_sources", "ok"),
    ("get_audio_source_configurations", "ok"),
    ("get_audio_encoder_configurations", "ok"),
    ("get_audio_encoder_configuration", "ok"),
    ("set_audio_encoder_configuration", "ok"),
    ("get_audio_encoder_configuration_options", "ok"),
    // ── Media2 ────────────────────────────────────────────────────────────
    ("get_profiles_media2", "ok"),
    ("get_stream_uri_media2", "ok"),
    ("get_snapshot_uri_media2", "ok"),
    ("get_video_source_configurations_media2", "ok"),
    ("set_video_source_configuration_media2", "ok"),
    ("get_video_source_configuration_options_media2", "ok"),
    ("get_video_encoder_configurations_media2", "ok"),
    ("get_video_encoder_configuration_media2", "ok"),
    ("set_video_encoder_configuration_media2", "ok"),
    ("get_video_encoder_configuration_options_media2", "ok"),
    ("get_video_encoder_instances_media2", "ok"),
    ("create_profile_media2", "ok"),
    ("delete_profile_media2", "ok"),
    ("add_configuration_media2", "ok"),
    ("remove_configuration_media2", "ok"),
    ("get_metadata_configurations_media2", "ok"),
    ("set_metadata_configuration_media2", "ok"),
    ("get_metadata_configuration_options_media2", "ok"),
    ("get_audio_source_configurations_media2", "ok"),
    ("get_audio_encoder_configurations_media2", "ok"),
    ("get_audio_encoder_configuration_options_media2", "ok"),
    ("set_audio_encoder_configuration_media2", "ok"),
    ("get_audio_output_configurations_media2", "ok"),
    ("get_audio_decoder_configurations_media2", "ok"),
    ("get_video_source_modes_media2", "ok"),
    // Deliberately a fault, not "ok". The mock models no sensor-mode catalogue
    // and oxvif's `VideoSourceMode` has no active-mode field, so a success here
    // would be a claim no getter in this crate could ever contradict —
    // `CLAUDE.md` step 5c. See `media2::resp_set_video_source_mode`.
    (
        "set_video_source_mode_media2",
        "soap-fault:ter:ActionNotSupported:NotModelled-VSMODE-5813: the mock does not switch \
         video source modes; nothing was stored, and no getter could show it if it had been",
    ),
    // ── PTZ ───────────────────────────────────────────────────────────────
    ("ptz_absolute_move", "ok"),
    ("ptz_relative_move", "ok"),
    ("ptz_continuous_move", "ok"),
    ("ptz_stop", "ok"),
    ("ptz_get_presets", "ok"),
    ("ptz_goto_preset", "ok"),
    ("ptz_set_preset", "ok"),
    ("ptz_remove_preset", "ok"),
    ("ptz_get_status", "ok"),
    ("ptz_goto_home_position", "ok"),
    ("ptz_set_home_position", "ok"),
    ("ptz_get_configurations", "ok"),
    ("ptz_get_configuration", "ok"),
    ("ptz_set_configuration", "ok"),
    ("ptz_get_configuration_options", "ok"),
    ("ptz_get_nodes", "ok"),
    ("ptz_get_node", "ok"),
    ("ptz_get_compatible_configurations", "ok"),
    // ── Recording / Search / Replay ───────────────────────────────────────
    ("get_recordings", "ok"),
    ("create_recording", "ok"),
    ("delete_recording", "ok"),
    ("create_track", "ok"),
    ("delete_track", "ok"),
    ("get_recording_jobs", "ok"),
    ("create_recording_job", "ok"),
    ("set_recording_job_mode", "ok"),
    ("delete_recording_job", "ok"),
    ("get_recording_job_state", "ok"),
    ("find_recordings", "ok"),
    ("get_recording_search_results", "ok"),
    ("end_search", "ok"),
    ("get_replay_uri", "ok"),
];

/// Drive every operation once against a fresh mock and collect outcomes.
async fn observed() -> Vec<(&'static str, String)> {
    use oxvif::types::{
        FocusMove, IpStackConfig, NetworkInterfaceConfig, OsdConfiguration, OsdPosition,
        OsdTextString, RecordingConfiguration, RecordingJobConfiguration, SetDateTimeRequest,
    };

    let mut out: Vec<(&'static str, String)> = Vec::new();

    // ── Device ────────────────────────────────────────────────────────────
    probe!(out, "get_capabilities", get_capabilities());
    probe!(out, "get_services", get_services());
    probe!(out, "get_system_date_and_time", get_system_date_and_time());
    {
        let c = fresh();
        let req = SetDateTimeRequest {
            datetime_type: "NTP".into(),
            daylight_savings: false,
            timezone: "UTC0".into(),
            utc_datetime: None,
        };
        out.push((
            "set_system_date_and_time",
            outcome(c.set_system_date_and_time(&req).await),
        ));
    }
    probe!(out, "get_device_info", get_device_info());
    probe!(out, "get_hostname", get_hostname());
    probe!(out, "set_hostname", set_hostname("snapshot-cam"));
    probe!(out, "get_ntp", get_ntp());
    probe!(out, "set_ntp", set_ntp(false, &["pool.ntp.org"]));
    probe!(out, "system_reboot", system_reboot());
    probe!(out, "get_scopes", get_scopes());
    probe!(
        out,
        "set_scopes",
        set_scopes(&["onvif://www.onvif.org/name/snapshot"])
    );
    probe!(out, "get_users", get_users());
    probe!(
        out,
        "create_users",
        create_users(&[("snapshot", "pw123456", "User")])
    );
    probe!(out, "delete_users", delete_users(&["operator"]));
    probe!(
        out,
        "set_user",
        set_user("operator", Some("pw123456"), "User")
    );
    probe!(out, "get_network_interfaces", get_network_interfaces());
    {
        let c = fresh();
        let cfg = NetworkInterfaceConfig {
            enabled: true,
            mtu: Some(1500),
            ipv4: Some(IpStackConfig {
                enabled: true,
                from_dhcp: true,
                manual: Vec::new(),
            }),
            ipv6: None,
        };
        out.push((
            "set_network_interfaces",
            outcome(c.set_network_interfaces(IFACE, &cfg).await),
        ));
    }
    probe!(out, "get_network_protocols", get_network_protocols());
    probe!(
        out,
        "set_network_protocols",
        set_network_protocols(&[("HTTP", true, &[80][..])])
    );
    probe!(out, "get_dns", get_dns());
    probe!(out, "set_dns", set_dns(false, &["8.8.8.8"]));
    probe!(
        out,
        "get_network_default_gateway",
        get_network_default_gateway()
    );
    probe!(
        out,
        "set_network_default_gateway",
        set_network_default_gateway(&["192.168.1.1"])
    );
    probe!(
        out,
        "send_auxiliary_command",
        send_auxiliary_command("tt:Wiper|On")
    );
    probe!(out, "get_system_log", get_system_log("System"));
    probe!(out, "get_relay_outputs", get_relay_outputs());
    probe!(
        out,
        "set_relay_output_settings",
        set_relay_output_settings(RELAY, "Monostable", "PT2S", "open")
    );
    probe!(
        out,
        "set_relay_output_state",
        set_relay_output_state(RELAY, "active")
    );
    probe!(out, "get_digital_inputs", get_digital_inputs(DEVICEIO));
    probe!(
        out,
        "set_system_factory_default",
        set_system_factory_default("Soft")
    );
    probe!(
        out,
        "get_storage_configurations",
        get_storage_configurations()
    );
    probe!(
        out,
        "set_storage_configuration",
        set_storage_configuration("SD_01", "NFS", "/mnt/rec", "nfs://10.0.0.9/rec", "admin")
    );
    probe!(out, "get_system_uris", get_system_uris());
    probe!(out, "start_firmware_upgrade", start_firmware_upgrade());
    probe!(out, "start_system_restore", start_system_restore());
    probe!(out, "get_discovery_mode", get_discovery_mode());
    probe!(
        out,
        "set_discovery_mode",
        set_discovery_mode("Discoverable")
    );

    // ── Events ────────────────────────────────────────────────────────────
    probe!(out, "get_event_properties", get_event_properties(EVENTS));
    probe!(
        out,
        "create_pull_point_subscription",
        create_pull_point_subscription(EVENTS, None, Some("PT60S"))
    );
    probe!(out, "pull_messages", pull_messages(SUBSCRIPTION, "PT1S", 5));
    probe!(
        out,
        "renew_subscription",
        renew_subscription(SUBSCRIPTION, "PT60S")
    );
    probe!(
        out,
        "set_synchronization_point",
        set_synchronization_point(SUBSCRIPTION)
    );
    probe!(out, "unsubscribe", unsubscribe(SUBSCRIPTION));
    probe!(
        out,
        "subscribe",
        subscribe(EVENTS, "http://consumer/notify", None, Some("PT60S"))
    );

    // ── Imaging ───────────────────────────────────────────────────────────
    probe!(
        out,
        "get_imaging_settings",
        get_imaging_settings(IMAGING, VIDEO_SOURCE)
    );
    {
        let c = fresh();
        let settings = c
            .get_imaging_settings(IMAGING, VIDEO_SOURCE)
            .await
            .expect("prerequisite: get_imaging_settings");
        out.push((
            "set_imaging_settings",
            outcome(
                c.set_imaging_settings(IMAGING, VIDEO_SOURCE, &settings)
                    .await,
            ),
        ));
    }
    probe!(
        out,
        "get_imaging_options",
        get_imaging_options(IMAGING, VIDEO_SOURCE)
    );
    probe!(
        out,
        "imaging_move",
        imaging_move(IMAGING, VIDEO_SOURCE, &FocusMove::Continuous { speed: 0.5 })
    );
    probe!(out, "imaging_stop", imaging_stop(IMAGING, VIDEO_SOURCE));
    probe!(
        out,
        "imaging_get_move_options",
        imaging_get_move_options(IMAGING, VIDEO_SOURCE)
    );
    probe!(
        out,
        "imaging_get_status",
        imaging_get_status(IMAGING, VIDEO_SOURCE)
    );

    // ── Media1 ────────────────────────────────────────────────────────────
    probe!(out, "get_profiles", get_profiles(MEDIA));
    probe!(out, "get_stream_uri", get_stream_uri(MEDIA, PROFILE));
    probe!(out, "get_snapshot_uri", get_snapshot_uri(MEDIA, PROFILE));
    probe!(
        out,
        "create_profile",
        create_profile(MEDIA, "snapshot", None)
    );
    probe!(out, "delete_profile", delete_profile(MEDIA, PROFILE2));
    probe!(out, "get_profile", get_profile(MEDIA, PROFILE));
    probe!(
        out,
        "add_video_encoder_configuration",
        add_video_encoder_configuration(MEDIA, PROFILE, VEC)
    );
    probe!(
        out,
        "remove_video_encoder_configuration",
        remove_video_encoder_configuration(MEDIA, PROFILE)
    );
    probe!(
        out,
        "add_video_source_configuration",
        add_video_source_configuration(MEDIA, PROFILE, VSC)
    );
    probe!(
        out,
        "remove_video_source_configuration",
        remove_video_source_configuration(MEDIA, PROFILE)
    );
    probe!(out, "get_video_sources", get_video_sources(MEDIA));
    probe!(
        out,
        "get_video_source_configurations",
        get_video_source_configurations(MEDIA)
    );
    probe!(
        out,
        "get_video_source_configuration",
        get_video_source_configuration(MEDIA, VSC)
    );
    {
        let c = fresh();
        let cfg = c
            .get_video_source_configuration(MEDIA, VSC)
            .await
            .expect("prerequisite: get_video_source_configuration");
        out.push((
            "set_video_source_configuration",
            outcome(c.set_video_source_configuration(MEDIA, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_video_source_configuration_options",
        get_video_source_configuration_options(MEDIA, VSC)
    );
    probe!(
        out,
        "get_video_encoder_configurations",
        get_video_encoder_configurations(MEDIA)
    );
    probe!(
        out,
        "get_video_encoder_configuration",
        get_video_encoder_configuration(MEDIA, VEC)
    );
    {
        let c = fresh();
        let cfg = c
            .get_video_encoder_configuration(MEDIA, VEC)
            .await
            .expect("prerequisite: get_video_encoder_configuration");
        out.push((
            "set_video_encoder_configuration",
            outcome(c.set_video_encoder_configuration(MEDIA, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_video_encoder_configuration_options",
        get_video_encoder_configuration_options(MEDIA, VEC)
    );
    probe!(out, "get_osds", get_osds(MEDIA, Some(VSC)));
    probe!(out, "get_osd", get_osd(MEDIA, OSD));
    {
        let c = fresh();
        let osd = c.get_osd(MEDIA, OSD).await.expect("prerequisite: get_osd");
        out.push(("set_osd", outcome(c.set_osd(MEDIA, &osd).await)));
    }
    {
        // A *new* Plain-text overlay: the mock enforces per-text-type OSD
        // quotas, and the seeded overlay already uses up the single
        // DateAndTime slot, so re-creating that one would fault.
        let c = fresh();
        let osd = OsdConfiguration {
            token: String::new(),
            video_source_config_token: VSC.into(),
            type_: "Text".into(),
            position: OsdPosition {
                type_: "LowerRight".into(),
                ..Default::default()
            },
            text_string: Some(OsdTextString {
                type_: "Plain".into(),
                plain_text: Some("net-1".into()),
                font_size: Some(16),
                ..Default::default()
            }),
            image_path: None,
        };
        out.push(("create_osd", outcome(c.create_osd(MEDIA, &osd).await)));
    }
    probe!(out, "delete_osd", delete_osd(MEDIA, OSD));
    probe!(out, "get_osd_options", get_osd_options(MEDIA, VSC));
    probe!(out, "get_audio_sources", get_audio_sources(MEDIA));
    probe!(
        out,
        "get_audio_source_configurations",
        get_audio_source_configurations(MEDIA)
    );
    probe!(
        out,
        "get_audio_encoder_configurations",
        get_audio_encoder_configurations(MEDIA)
    );
    probe!(
        out,
        "get_audio_encoder_configuration",
        get_audio_encoder_configuration(MEDIA, AEC)
    );
    {
        let c = fresh();
        let cfg = c
            .get_audio_encoder_configuration(MEDIA, AEC)
            .await
            .expect("prerequisite: get_audio_encoder_configuration");
        out.push((
            "set_audio_encoder_configuration",
            outcome(c.set_audio_encoder_configuration(MEDIA, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_audio_encoder_configuration_options",
        get_audio_encoder_configuration_options(MEDIA, AEC)
    );

    // ── Media2 ────────────────────────────────────────────────────────────
    probe!(out, "get_profiles_media2", get_profiles_media2(MEDIA2));
    probe!(
        out,
        "get_stream_uri_media2",
        get_stream_uri_media2(MEDIA2, PROFILE)
    );
    probe!(
        out,
        "get_snapshot_uri_media2",
        get_snapshot_uri_media2(MEDIA2, PROFILE)
    );
    probe!(
        out,
        "get_video_source_configurations_media2",
        get_video_source_configurations_media2(MEDIA2)
    );
    {
        let c = fresh();
        let cfg = c
            .get_video_source_configurations_media2(MEDIA2)
            .await
            .expect("prerequisite: get_video_source_configurations_media2")
            .remove(0);
        out.push((
            "set_video_source_configuration_media2",
            outcome(c.set_video_source_configuration_media2(MEDIA2, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_video_source_configuration_options_media2",
        get_video_source_configuration_options_media2(MEDIA2, VSC)
    );
    probe!(
        out,
        "get_video_encoder_configurations_media2",
        get_video_encoder_configurations_media2(MEDIA2)
    );
    probe!(
        out,
        "get_video_encoder_configuration_media2",
        get_video_encoder_configuration_media2(MEDIA2, VEC)
    );
    {
        let c = fresh();
        let cfg = c
            .get_video_encoder_configuration_media2(MEDIA2, VEC)
            .await
            .expect("prerequisite: get_video_encoder_configuration_media2");
        out.push((
            "set_video_encoder_configuration_media2",
            outcome(c.set_video_encoder_configuration_media2(MEDIA2, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_video_encoder_configuration_options_media2",
        get_video_encoder_configuration_options_media2(MEDIA2, VEC)
    );
    probe!(
        out,
        "get_video_encoder_instances_media2",
        get_video_encoder_instances_media2(MEDIA2, VEC)
    );
    probe!(
        out,
        "create_profile_media2",
        create_profile_media2(MEDIA2, "snapshot2")
    );
    probe!(
        out,
        "delete_profile_media2",
        delete_profile_media2(MEDIA2, PROFILE2)
    );
    probe!(
        out,
        "add_configuration_media2",
        add_configuration_media2(MEDIA2, PROFILE, "VideoEncoder", VEC)
    );
    probe!(
        out,
        "remove_configuration_media2",
        remove_configuration_media2(MEDIA2, PROFILE, "VideoEncoder", VEC)
    );
    probe!(
        out,
        "get_metadata_configurations_media2",
        get_metadata_configurations_media2(MEDIA2, None, None)
    );
    {
        let c = fresh();
        let cfg = c
            .get_metadata_configurations_media2(MEDIA2, None, None)
            .await
            .expect("prerequisite: get_metadata_configurations_media2")
            .remove(0);
        out.push((
            "set_metadata_configuration_media2",
            outcome(c.set_metadata_configuration_media2(MEDIA2, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_metadata_configuration_options_media2",
        get_metadata_configuration_options_media2(MEDIA2, None, None)
    );
    probe!(
        out,
        "get_audio_source_configurations_media2",
        get_audio_source_configurations_media2(MEDIA2)
    );
    probe!(
        out,
        "get_audio_encoder_configurations_media2",
        get_audio_encoder_configurations_media2(MEDIA2)
    );
    probe!(
        out,
        "get_audio_encoder_configuration_options_media2",
        get_audio_encoder_configuration_options_media2(MEDIA2, AEC)
    );
    {
        let c = fresh();
        let cfg = c
            .get_audio_encoder_configurations_media2(MEDIA2)
            .await
            .expect("prerequisite: get_audio_encoder_configurations_media2")
            .remove(0);
        out.push((
            "set_audio_encoder_configuration_media2",
            outcome(c.set_audio_encoder_configuration_media2(MEDIA2, &cfg).await),
        ));
    }
    probe!(
        out,
        "get_audio_output_configurations_media2",
        get_audio_output_configurations_media2(MEDIA2)
    );
    probe!(
        out,
        "get_audio_decoder_configurations_media2",
        get_audio_decoder_configurations_media2(MEDIA2)
    );
    probe!(
        out,
        "get_video_source_modes_media2",
        get_video_source_modes_media2(MEDIA2, VIDEO_SOURCE)
    );
    probe!(
        out,
        "set_video_source_mode_media2",
        set_video_source_mode_media2(MEDIA2, VIDEO_SOURCE, VS_MODE)
    );

    // ── PTZ ───────────────────────────────────────────────────────────────
    probe!(
        out,
        "ptz_absolute_move",
        ptz_absolute_move(PTZ, PROFILE, 0.5, -0.3, 0.7)
    );
    probe!(
        out,
        "ptz_relative_move",
        ptz_relative_move(PTZ, PROFILE, 0.1, 0.1, 0.0)
    );
    probe!(
        out,
        "ptz_continuous_move",
        ptz_continuous_move(PTZ, PROFILE, 0.2, 0.0, 0.0)
    );
    probe!(out, "ptz_stop", ptz_stop(PTZ, PROFILE));
    probe!(out, "ptz_get_presets", ptz_get_presets(PTZ, PROFILE));
    probe!(
        out,
        "ptz_goto_preset",
        ptz_goto_preset(PTZ, PROFILE, PRESET)
    );
    probe!(
        out,
        "ptz_set_preset",
        ptz_set_preset(PTZ, PROFILE, Some("Snapshot"), None)
    );
    probe!(
        out,
        "ptz_remove_preset",
        ptz_remove_preset(PTZ, PROFILE, PRESET)
    );
    probe!(out, "ptz_get_status", ptz_get_status(PTZ, PROFILE));
    probe!(
        out,
        "ptz_goto_home_position",
        ptz_goto_home_position(PTZ, PROFILE, Some(0.5))
    );
    probe!(
        out,
        "ptz_set_home_position",
        ptz_set_home_position(PTZ, PROFILE)
    );
    probe!(out, "ptz_get_configurations", ptz_get_configurations(PTZ));
    probe!(
        out,
        "ptz_get_configuration",
        ptz_get_configuration(PTZ, PTZ_CONFIG)
    );
    {
        let c = fresh();
        let cfg = c
            .ptz_get_configuration(PTZ, PTZ_CONFIG)
            .await
            .expect("prerequisite: ptz_get_configuration");
        out.push((
            "ptz_set_configuration",
            outcome(c.ptz_set_configuration(PTZ, &cfg, true).await),
        ));
    }
    probe!(
        out,
        "ptz_get_configuration_options",
        ptz_get_configuration_options(PTZ, PTZ_CONFIG)
    );
    probe!(out, "ptz_get_nodes", ptz_get_nodes(PTZ));
    probe!(out, "ptz_get_node", ptz_get_node(PTZ, PTZ_NODE));
    probe!(
        out,
        "ptz_get_compatible_configurations",
        ptz_get_compatible_configurations(PTZ, PROFILE)
    );

    // ── Recording / Search / Replay ───────────────────────────────────────
    probe!(out, "get_recordings", get_recordings(RECORDING));
    {
        let c = fresh();
        let cfg = RecordingConfiguration {
            source_name: "Snapshot Cam".into(),
            source_id: "urn:uuid:snapshot".into(),
            location: "Lab".into(),
            description: "net-1 snapshot".into(),
            content: "Motion".into(),
            maximum_retention_time: "PT0S".into(),
        };
        out.push((
            "create_recording",
            outcome(c.create_recording(RECORDING, &cfg).await),
        ));
    }
    probe!(
        out,
        "delete_recording",
        delete_recording(RECORDING, RECORDING_TOKEN)
    );
    probe!(
        out,
        "create_track",
        create_track(RECORDING, RECORDING_TOKEN, "Video", "main track")
    );
    probe!(
        out,
        "delete_track",
        delete_track(RECORDING, RECORDING_TOKEN, TRACK)
    );
    probe!(out, "get_recording_jobs", get_recording_jobs(RECORDING));
    {
        let c = fresh();
        let cfg = RecordingJobConfiguration {
            recording_token: RECORDING_TOKEN.into(),
            mode: "Active".into(),
            priority: 1,
            source_token: PROFILE.into(),
        };
        out.push((
            "create_recording_job",
            outcome(c.create_recording_job(RECORDING, &cfg).await),
        ));
    }
    probe!(
        out,
        "set_recording_job_mode",
        set_recording_job_mode(RECORDING, JOB, "Idle")
    );
    probe!(
        out,
        "delete_recording_job",
        delete_recording_job(RECORDING, JOB)
    );
    probe!(
        out,
        "get_recording_job_state",
        get_recording_job_state(RECORDING, JOB)
    );
    probe!(
        out,
        "find_recordings",
        find_recordings(SEARCH, Some(10), "PT60S")
    );
    probe!(
        out,
        "get_recording_search_results",
        get_recording_search_results(SEARCH, SEARCH_TOKEN, 10, "PT5S")
    );
    probe!(out, "end_search", end_search(SEARCH, SEARCH_TOKEN));
    probe!(
        out,
        "get_replay_uri",
        get_replay_uri(REPLAY, RECORDING_TOKEN, "RTP-Unicast", "RTSP")
    );

    out
}

/// NET 1 — the full mock action snapshot.
///
/// Covers **141 ONVIF operations**: every `pub async fn` on `OnvifClient`
/// that maps to exactly one SOAP action. Enumerated mechanically from
/// `grep 'pub async fn' src/client/*.rs`; the only two exclusions are
/// `search_recordings` (a convenience wrapper over FindRecordings +
/// GetRecordingSearchResults + EndSearch, all three of which are covered
/// individually) and `notification_listener` (a free function returning a
/// stream, not a single request/response).
///
/// Each operation runs against its own fresh `MockTransport`, so no write
/// leaks into the next probe and the snapshot is order-independent.
///
/// The pass/fail set lives in `EXPECTED` above, written out by hand. When a
/// later stage changes what the mock answers, this test fails and the
/// EXPECTED line for that operation must be edited deliberately.
#[tokio::test]
async fn mock_action_snapshot_matches_expected_list() {
    let observed = observed().await;

    let observed_names: Vec<&str> = observed.iter().map(|(n, _)| *n).collect();
    let expected_names: Vec<&str> = EXPECTED.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        observed_names, expected_names,
        "the probed operation list and EXPECTED must stay in lock-step \
         (same operations, same order)"
    );

    let mut drift: Vec<String> = Vec::new();
    for ((name, got), (_, want)) in observed.iter().zip(EXPECTED.iter()) {
        if got != want {
            drift.push(format!("  {name}: expected {want:?}, got {got:?}"));
        }
    }
    assert!(
        drift.is_empty(),
        "mock action snapshot drifted from EXPECTED — update the listed \
         lines deliberately, one per operation:\n{}",
        drift.join("\n")
    );
}
