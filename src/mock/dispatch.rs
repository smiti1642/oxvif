use crate::mock::helpers::{resp_empty, resp_soap_fault};
use crate::mock::services::{device, events, imaging, media, media2, ptz, recording};
use crate::mock::state::SharedState;

pub fn dispatch(action: &str, base: &str, state: &SharedState, body: &str) -> String {
    let op = action.rsplit('/').next().unwrap_or("");

    // Events share one sub-dispatcher across the ONVIF and OASIS WSN namespaces.
    let response =
        if action.contains("/events/wsdl/") || action.contains("docs.oasis-open.org/wsn/") {
            dispatch_events(op, base, state, body)
        } else if let Some(tail) = action.strip_prefix("http://www.onvif.org/") {
            if tail.starts_with("ver10/device/wsdl/") {
                dispatch_device(op, base, state, body)
            } else if tail.starts_with("ver20/media/wsdl/") {
                dispatch_media2(op, base, state, body)
            } else if tail.starts_with("ver10/media/wsdl/") {
                dispatch_media(op, base, state, body)
            } else if tail.starts_with("ver20/ptz/wsdl/") {
                dispatch_ptz(op, state, body)
            } else if tail.starts_with("ver20/imaging/wsdl/") {
                dispatch_imaging(op, state, body)
            } else if tail.starts_with("ver10/recording/wsdl/") {
                dispatch_recording(op)
            } else if tail.starts_with("ver10/search/wsdl/") {
                dispatch_search(op)
            } else if tail.starts_with("ver10/replay/wsdl/") {
                dispatch_replay(op)
            } else {
                None
            }
        } else {
            None
        };

    response.unwrap_or_else(|| {
        eprintln!("  [WARN] unhandled action: {action}");
        resp_soap_fault("s:Receiver", &format!("Not implemented: {action}"))
    })
}

fn dispatch_device(op: &str, base: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => device::resp_service_capabilities(),
        "GetSystemDateAndTime" => device::resp_system_date_and_time(state),
        "SetSystemDateAndTime" => device::handle_set_system_date_and_time(state, body),
        "GetCapabilities" => device::resp_capabilities(base),
        "GetServices" => device::resp_services(base),
        "GetDeviceInformation" => device::resp_device_info(state),
        "GetHostname" => device::resp_hostname(state),
        "SetHostname" => device::handle_set_hostname(state, body),
        "GetNTP" => device::resp_ntp(state),
        "SetNTP" => device::handle_set_ntp(state, body),
        "GetDNS" => device::resp_dns(state),
        "SetDNS" => device::handle_set_dns(state, body),
        "GetScopes" => device::resp_scopes(state),
        "SetScopes" => device::handle_set_scopes(state, body),
        "GetUsers" => device::resp_users(state),
        "CreateUsers" => device::handle_create_users(state, body),
        "DeleteUsers" => device::handle_delete_users(state, body),
        "SetUser" => device::handle_set_user(state, body),
        "GetNetworkInterfaces" => device::resp_network_interfaces(state),
        "SetNetworkInterfaces" => device::handle_set_network_interfaces(state, body),
        "GetNetworkProtocols" => device::resp_network_protocols(state),
        "SetNetworkProtocols" => device::handle_set_network_protocols(state, body),
        "GetNetworkDefaultGateway" => device::resp_network_default_gateway(state),
        "SetNetworkDefaultGateway" => device::handle_set_network_default_gateway(state, body),
        "SendAuxiliaryCommand" => device::resp_send_auxiliary_command(),
        "GetSystemLog" => device::resp_system_log(),
        "GetRelayOutputs" => device::resp_relay_outputs(state),
        "SetRelayOutputState" => device::handle_set_relay_output_state(state, body),
        "SetRelayOutputSettings" => device::handle_set_relay_output_settings(state, body),
        "GetDigitalInputs" => device::resp_digital_inputs(state),
        "SetSystemFactoryDefault" => resp_empty("tds", "SetSystemFactoryDefaultResponse"),
        "GetStorageConfigurations" => device::resp_storage_configurations(),
        "SetStorageConfiguration" => resp_empty("tds", "SetStorageConfigurationResponse"),
        "GetSystemUris" => device::resp_system_uris(base),
        "StartFirmwareUpgrade" => device::resp_start_firmware_upgrade(base),
        "StartSystemRestore" => device::resp_start_system_restore(base),
        "GetDiscoveryMode" => device::resp_discovery_mode(state),
        "SetDiscoveryMode" => resp_empty("tds", "SetDiscoveryModeResponse"),
        "SystemReboot" => device::resp_system_reboot(),
        _ => return None,
    })
}

fn dispatch_media(op: &str, base: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => media::resp_service_capabilities(),
        "GetProfiles" => media::resp_profiles(state),
        "GetProfile" => media::resp_profile(state, body),
        "CreateProfile" => media::handle_create_profile(state, body),
        "DeleteProfile" => media::handle_delete_profile(state, body),
        "GetStreamUri" => media::resp_stream_uri(),
        "GetSnapshotUri" => media::resp_snapshot_uri(base),
        "GetVideoSources" => media::resp_video_sources(state),
        "GetVideoSourceConfigurations" => media::resp_video_source_configurations(state),
        "GetVideoSourceConfiguration" => media::resp_video_source_configuration(state, body),
        "SetVideoSourceConfiguration" => resp_empty("trt", "SetVideoSourceConfigurationResponse"),
        "GetVideoSourceConfigurationOptions" => {
            media::resp_video_source_configuration_options(state, body)
        }
        "GetVideoEncoderConfigurations" => media::resp_video_encoder_configurations(state, body),
        "GetVideoEncoderConfiguration" => media::resp_video_encoder_configuration(state, body),
        "SetVideoEncoderConfiguration" => resp_empty("trt", "SetVideoEncoderConfigurationResponse"),
        "GetVideoEncoderConfigurationOptions" => {
            media::resp_video_encoder_configuration_options(state, body)
        }
        "AddVideoEncoderConfiguration" => resp_empty("trt", "AddVideoEncoderConfigurationResponse"),
        "RemoveVideoEncoderConfiguration" => {
            resp_empty("trt", "RemoveVideoEncoderConfigurationResponse")
        }
        "AddVideoSourceConfiguration" => resp_empty("trt", "AddVideoSourceConfigurationResponse"),
        "RemoveVideoSourceConfiguration" => {
            resp_empty("trt", "RemoveVideoSourceConfigurationResponse")
        }
        "GetAudioSources" => media::resp_audio_sources(),
        "GetAudioSourceConfigurations" => media::resp_audio_source_configurations(),
        "GetAudioEncoderConfiguration" => media::resp_audio_encoder_configuration(),
        "GetAudioEncoderConfigurations" => media::resp_audio_encoder_configurations(),
        "SetAudioEncoderConfiguration" => resp_empty("trt", "SetAudioEncoderConfigurationResponse"),
        "GetAudioEncoderConfigurationOptions" => media::resp_audio_encoder_configuration_options(),
        "GetOSD" => media::resp_osd(state, body),
        "GetOSDs" => media::resp_osds(state, body),
        "SetOSD" => media::handle_set_osd(state, body),
        "CreateOSD" => media::handle_create_osd(state, body),
        "DeleteOSD" => media::handle_delete_osd(state, body),
        "GetOSDOptions" => media::resp_osd_options(),
        _ => return None,
    })
}

fn dispatch_media2(op: &str, base: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => media2::resp_service_capabilities_media2(),
        "GetProfiles" => media2::resp_profiles_media2(),
        "CreateProfile" => media2::resp_create_profile_media2(),
        "DeleteProfile" => resp_empty("tr2", "DeleteProfileResponse"),
        "AddConfiguration" => resp_empty("tr2", "AddConfigurationResponse"),
        "RemoveConfiguration" => resp_empty("tr2", "RemoveConfigurationResponse"),
        "GetStreamUri" => media2::resp_stream_uri_media2(),
        "GetSnapshotUri" => media2::resp_snapshot_uri_media2(base),
        "GetVideoSourceConfigurations" => media2::resp_video_source_configurations_media2(state),
        "SetVideoSourceConfiguration" => resp_empty("tr2", "SetVideoSourceConfigurationResponse"),
        "GetVideoSourceConfigurationOptions" => {
            media2::resp_video_source_configuration_options_media2(state, body)
        }
        "GetVideoEncoderConfigurations" => media2::resp_video_encoder_configurations(state, body),
        "SetVideoEncoderConfiguration" => {
            media2::handle_set_video_encoder_configuration(state, body)
        }
        "GetVideoEncoderConfigurationOptions" => {
            media2::resp_video_encoder_configuration_options_media2(state, body)
        }
        "GetVideoEncoderInstances" => media2::resp_video_encoder_instances(),
        "GetMetadataConfigurations" => media2::resp_metadata_configurations(),
        "SetMetadataConfiguration" => resp_empty("tr2", "SetMetadataConfigurationResponse"),
        "GetMetadataConfigurationOptions" => media2::resp_metadata_configuration_options(),
        "GetAudioSourceConfigurations" => media2::resp_audio_source_configurations_media2(),
        "GetAudioEncoderConfigurations" => media2::resp_audio_encoder_configurations_media2(),
        "GetAudioEncoderConfigurationOptions" => {
            media2::resp_audio_encoder_configuration_options_media2()
        }
        "SetAudioEncoderConfiguration" => resp_empty("tr2", "SetAudioEncoderConfigurationResponse"),
        "GetAudioOutputConfigurations" => media2::resp_audio_output_configurations(),
        "GetAudioDecoderConfigurations" => media2::resp_audio_decoder_configurations(),
        "GetVideoSourceModes" => media2::resp_video_source_modes(),
        "SetVideoSourceMode" => media2::resp_set_video_source_mode(),
        _ => return None,
    })
}

fn dispatch_ptz(op: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => ptz::resp_ptz_service_capabilities(),
        "GetStatus" => ptz::resp_ptz_status(state),
        "GetPresets" => ptz::resp_ptz_presets(state),
        "SetPreset" => ptz::handle_ptz_set_preset(state, body),
        "RemovePreset" => ptz::handle_ptz_remove_preset(state, body),
        "GotoPreset" => ptz::handle_ptz_goto_preset(state, body),
        "AbsoluteMove" => ptz::handle_ptz_absolute_move(state, body),
        "RelativeMove" => ptz::handle_ptz_relative_move(state, body),
        "ContinuousMove" => ptz::handle_ptz_continuous_move(state, body),
        "Stop" => ptz::handle_ptz_stop(),
        "GotoHomePosition" => ptz::handle_ptz_goto_home_position(state),
        "SetHomePosition" => ptz::handle_ptz_set_home_position(state),
        "GetNodes" => ptz::resp_ptz_nodes(),
        "GetNode" => ptz::resp_ptz_node(),
        "GetConfigurations" => ptz::resp_ptz_configurations(),
        "GetCompatibleConfigurations" => ptz::resp_ptz_compatible_configurations(),
        "GetConfiguration" => ptz::resp_ptz_configuration(),
        "SetConfiguration" => resp_empty("tptz", "SetConfigurationResponse"),
        "GetConfigurationOptions" => ptz::resp_ptz_configuration_options(),
        "GetPresetTours" => ptz::resp_ptz_preset_tours(state),
        "GetPresetTour" => ptz::resp_ptz_preset_tour(state, body),
        "GetPresetTourOptions" => ptz::resp_ptz_preset_tour_options(state),
        "CreatePresetTour" => ptz::handle_ptz_create_preset_tour(state),
        "ModifyPresetTour" => ptz::handle_ptz_modify_preset_tour(state, body),
        "OperatePresetTour" => ptz::handle_ptz_operate_preset_tour(state, body),
        "RemovePresetTour" => ptz::handle_ptz_remove_preset_tour(state, body),
        "SendAuxiliaryCommand" => ptz::handle_ptz_send_auxiliary_command(body),
        _ => return None,
    })
}

fn dispatch_imaging(op: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => imaging::resp_imaging_service_capabilities(),
        "GetImagingSettings" => imaging::resp_imaging_settings(state),
        "SetImagingSettings" => imaging::handle_set_imaging_settings(state, body),
        "GetOptions" => imaging::resp_imaging_options(),
        "GetStatus" => imaging::resp_imaging_status(),
        "GetMoveOptions" => imaging::resp_imaging_move_options(),
        "Move" => resp_empty("timg", "MoveResponse"),
        "Stop" => resp_empty("timg", "StopResponse"),
        _ => return None,
    })
}

fn dispatch_events(op: &str, base: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilitiesRequest" => events::resp_event_service_capabilities(),
        "GetEventPropertiesRequest" => events::resp_event_properties(),
        "CreatePullPointSubscriptionRequest" => {
            events::resp_create_pull_point_subscription(base, state, body)
        }
        "PullMessagesRequest" => events::resp_pull_messages(state),
        "SubscribeRequest" => events::resp_subscribe(base),
        "RenewRequest" => events::resp_renew(),
        "UnsubscribeRequest" => resp_empty("wsnt", "UnsubscribeResponse"),
        "SetSynchronizationPointRequest" => resp_empty("tev", "SetSynchronizationPointResponse"),
        _ => return None,
    })
}

// Recording, Search and Replay are three separate ONVIF services that happen to
// share `src/mock/services/recording.rs`. They must NOT share a dispatcher:
// `op` is the last path segment of the action URI, so all three define a
// distinct `GetServiceCapabilities` that arrives here as the same string. Until
// 0.15 they were one match block, which is exactly why that operation could not
// be added for any of them.
fn dispatch_recording(op: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => recording::resp_recording_service_capabilities(),
        "GetRecordings" => recording::resp_recordings(),
        "CreateRecording" => recording::resp_create_recording(),
        "DeleteRecording" => resp_empty("trc", "DeleteRecordingResponse"),
        "CreateTrack" => recording::resp_create_track(),
        "DeleteTrack" => resp_empty("trc", "DeleteTrackResponse"),
        "GetRecordingJobs" => recording::resp_recording_jobs(),
        "CreateRecordingJob" => recording::resp_create_recording_job(),
        "SetRecordingJobMode" => resp_empty("trc", "SetRecordingJobModeResponse"),
        "DeleteRecordingJob" => resp_empty("trc", "DeleteRecordingJobResponse"),
        "GetRecordingJobState" => recording::resp_recording_job_state(),
        _ => return None,
    })
}

fn dispatch_search(op: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => recording::resp_search_service_capabilities(),
        "FindRecordings" => recording::resp_find_recordings(),
        "GetRecordingSearchResults" => recording::resp_recording_search_results(),
        "EndSearch" => resp_empty("tse", "EndSearchResponse"),
        _ => return None,
    })
}

fn dispatch_replay(op: &str) -> Option<String> {
    Some(match op {
        "GetServiceCapabilities" => recording::resp_replay_service_capabilities(),
        "GetReplayUri" => recording::resp_replay_uri(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::state::MockState;

    /// Every service's `GetServiceCapabilities` action URI. The `op` segment is
    /// identical across all nine — that is the whole reason routing has to key
    /// on the namespace prefix and not on `op`.
    const CAPS: &[(&str, &str)] = &[
        (
            "device",
            "http://www.onvif.org/ver10/device/wsdl/GetServiceCapabilities",
        ),
        (
            "media",
            "http://www.onvif.org/ver10/media/wsdl/GetServiceCapabilities",
        ),
        (
            "media2",
            "http://www.onvif.org/ver20/media/wsdl/GetServiceCapabilities",
        ),
        (
            "ptz",
            "http://www.onvif.org/ver20/ptz/wsdl/GetServiceCapabilities",
        ),
        (
            "imaging",
            "http://www.onvif.org/ver20/imaging/wsdl/GetServiceCapabilities",
        ),
        // Events is the odd one out twice over: its action URI carries a
        // portType segment *and* a `Request` suffix, so `op` here is
        // `GetServiceCapabilitiesRequest`, not `GetServiceCapabilities`.
        (
            "events",
            "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetServiceCapabilitiesRequest",
        ),
        (
            "recording",
            "http://www.onvif.org/ver10/recording/wsdl/GetServiceCapabilities",
        ),
        (
            "search",
            "http://www.onvif.org/ver10/search/wsdl/GetServiceCapabilities",
        ),
        (
            "replay",
            "http://www.onvif.org/ver10/replay/wsdl/GetServiceCapabilities",
        ),
    ];

    fn call(action: &str) -> String {
        let state = MockState::new();
        dispatch(action, "http://mock", &state, "")
    }

    /// All nine answer, and none of them falls through to the
    /// "Not implemented" fault. This is the test that would have failed before
    /// recording/search/replay were split into three dispatchers.
    #[test]
    fn every_service_answers_get_service_capabilities() {
        for (name, action) in CAPS {
            let out = call(action);
            assert!(
                !out.contains("Not implemented"),
                "{name}: unhandled action, got {out}"
            );
            assert!(
                out.contains("GetServiceCapabilitiesResponse"),
                "{name}: no response element, got {out}"
            );
        }
    }

    /// Recording, Search and Replay share `services/recording.rs` and used to
    /// share a dispatcher. Assert each returns *its own* capability type, not
    /// whichever one the shared match arm happened to list first.
    #[test]
    fn recording_search_replay_are_not_confused_with_each_other() {
        let recording = call(CAPS[6].1);
        let search = call(CAPS[7].1);
        let replay = call(CAPS[8].1);

        assert!(recording.contains("<trc:Capabilities"), "got {recording}");
        assert!(search.contains("<tse:Capabilities"), "got {search}");
        assert!(replay.contains("<trp:Capabilities"), "got {replay}");

        // ...and specifically not each other's.
        assert!(!recording.contains("tse:") && !recording.contains("trp:"));
        assert!(!search.contains("trc:") && !search.contains("trp:"));
        assert!(!replay.contains("trc:") && !replay.contains("tse:"));
    }

    /// Attribute names that are one plausible letter away from wrong. Each was
    /// verified against the published schema during the 0.15 Stage 0 pass; a
    /// typo here parses as "attribute absent" forever without failing anything
    /// else, which is why it is asserted as a literal string.
    #[test]
    fn capability_attribute_spelling_matches_the_schema() {
        let imaging = call(CAPS[4].1);
        assert!(
            imaging.contains("AdaptablePreset="),
            "timg:Capabilities uses AdaptablePreset (singular, 'Adaptable'), got {imaging}"
        );
        assert!(
            !imaging.contains("AdaptivePreset"),
            "AdaptivePresets is the wrong spelling, got {imaging}"
        );

        let media2 = call(CAPS[2].1);
        assert!(
            media2.contains("<tr2:Capabilities "),
            "the response element is Capabilities even though the type is Capabilities2, got {media2}"
        );
        assert!(
            media2.contains(r#"WebRTC="0""#),
            "tr2 WebRTC is an xs:int session count, not a bool, got {media2}"
        );

        let device = call(CAPS[0].1);
        for dotted in ["TLS1.2=", "X.509Token="] {
            assert!(
                device.contains(dotted),
                "tds:SecurityCapabilities attribute {dotted} carries a literal dot, got {device}"
            );
        }

        let ptz = call(CAPS[3].1);
        assert!(
            ptz.contains(r#"MoveAndTrack="PresetToken PTZVector""#),
            "tptz MoveAndTrack is a whitespace-separated tt:StringList, got {ptz}"
        );
    }

    /// The mock deliberately omits some optional attributes rather than sending
    /// them as `false`, so that a parser conflating "absent" with "said no" has
    /// something to fail against. Pin the omissions, or a later well-meaning
    /// edit fills them in and silently removes the only coverage of that path.
    #[test]
    fn optional_attributes_are_deliberately_omitted() {
        let recording = call(CAPS[6].1);
        assert!(
            !recording.contains("OnboardStorage"),
            "OnboardStorage must stay absent: it is the only capability attribute \
             with a schema default, and that default is true. got {recording}"
        );

        let events = call(CAPS[5].1);
        assert!(
            !events.contains("EventBrokerProtocols"),
            "a device with MaxEventBrokers=0 should not advertise a protocol list, got {events}"
        );
        assert!(
            !events.contains("WSPullPointSupport"),
            "WSPullPointSupport belongs to the device-level tt:EventCapabilities, \
             not to tev:Capabilities. got {events}"
        );

        let ptz = call(CAPS[3].1);
        assert!(
            !ptz.contains("EFlip") && !ptz.contains("Reverse"),
            "EFlip/Reverse are the omitted-attribute case for PTZ, got {ptz}"
        );
    }

    /// The device-level `GetCapabilities` and the Device service's
    /// `GetServiceCapabilities` are different operations returning different
    /// shapes. Assert the mock did not start answering one with the other.
    #[test]
    fn device_service_capabilities_is_not_device_capabilities() {
        let service = call(CAPS[0].1);
        assert!(service.contains("<tds:Misc "), "got {service}");
        assert!(
            !service.contains("XAddr"),
            "GetServiceCapabilities carries no service URLs; XAddr belongs to \
             GetCapabilities. got {service}"
        );

        let device_caps = dispatch(
            "http://www.onvif.org/ver10/device/wsdl/GetCapabilities",
            "http://mock",
            &MockState::new(),
            "",
        );
        assert!(device_caps.contains("XAddr"), "got {device_caps}");
        assert!(!device_caps.contains("<tds:Misc "), "got {device_caps}");
    }
}
