use crate::helpers::{resp_empty, resp_soap_fault};
use crate::services::{device, events, imaging, media, media2, ptz, recording};
use crate::state::SharedState;

pub fn dispatch(action: &str, base: &str, state: &SharedState, body: &str) -> String {
    let op = action.rsplit('/').next().unwrap_or("");

    // Events share one sub-dispatcher across the ONVIF and OASIS WSN namespaces.
    let response =
        if action.contains("/events/wsdl/") || action.contains("docs.oasis-open.org/wsn/") {
            dispatch_events(op)
        } else if let Some(tail) = action.strip_prefix("http://www.onvif.org/") {
            if tail.starts_with("ver10/device/wsdl/") {
                dispatch_device(op, base, state, body)
            } else if tail.starts_with("ver20/media/wsdl/") {
                dispatch_media2(op, base)
            } else if tail.starts_with("ver10/media/wsdl/") {
                dispatch_media(op, base)
            } else if tail.starts_with("ver20/ptz/wsdl/") {
                dispatch_ptz(op)
            } else if tail.starts_with("ver20/imaging/wsdl/") {
                dispatch_imaging(op, state, body)
            } else if tail.starts_with("ver10/recording/wsdl/")
                || tail.starts_with("ver10/search/wsdl/")
                || tail.starts_with("ver10/replay/wsdl/")
            {
                dispatch_recording(op)
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
        "GetRelayOutputs" => device::resp_relay_outputs(),
        "SetRelayOutputState" => resp_empty("tds", "SetRelayOutputStateResponse"),
        "SetRelayOutputSettings" => resp_empty("tds", "SetRelayOutputSettingsResponse"),
        "SetSystemFactoryDefault" => resp_empty("tds", "SetSystemFactoryDefaultResponse"),
        "GetStorageConfigurations" => device::resp_storage_configurations(),
        "SetStorageConfiguration" => resp_empty("tds", "SetStorageConfigurationResponse"),
        "GetSystemUris" => device::resp_system_uris(base),
        "GetDiscoveryMode" => device::resp_discovery_mode(state),
        "SetDiscoveryMode" => resp_empty("tds", "SetDiscoveryModeResponse"),
        "SystemReboot" => device::resp_system_reboot(),
        _ => return None,
    })
}

fn dispatch_media(op: &str, base: &str) -> Option<String> {
    Some(match op {
        "GetProfiles" => media::resp_profiles(),
        "GetProfile" => media::resp_profile(),
        "CreateProfile" => media::resp_create_profile(),
        "DeleteProfile" => resp_empty("trt", "DeleteProfileResponse"),
        "GetStreamUri" => media::resp_stream_uri(),
        "GetSnapshotUri" => media::resp_snapshot_uri(base),
        "GetVideoSources" => media::resp_video_sources(),
        "GetVideoSourceConfigurations" => media::resp_video_source_configurations(),
        "GetVideoSourceConfiguration" => media::resp_video_source_configuration(),
        "SetVideoSourceConfiguration" => resp_empty("trt", "SetVideoSourceConfigurationResponse"),
        "GetVideoSourceConfigurationOptions" => media::resp_video_source_configuration_options(),
        "GetVideoEncoderConfigurations" => media::resp_video_encoder_configurations(),
        "GetVideoEncoderConfiguration" => media::resp_video_encoder_configuration(),
        "SetVideoEncoderConfiguration" => resp_empty("trt", "SetVideoEncoderConfigurationResponse"),
        "GetVideoEncoderConfigurationOptions" => media::resp_video_encoder_configuration_options(),
        "AddVideoEncoderConfiguration"
        | "RemoveVideoEncoderConfiguration"
        | "AddVideoSourceConfiguration"
        | "RemoveVideoSourceConfiguration" => resp_empty("trt", "ConfigurationResponse"),
        "GetAudioSources" => media::resp_audio_sources(),
        "GetAudioSourceConfigurations" => media::resp_audio_source_configurations(),
        "GetAudioEncoderConfiguration" => media::resp_audio_encoder_configuration(),
        "GetAudioEncoderConfigurations" => media::resp_audio_encoder_configurations(),
        "SetAudioEncoderConfiguration" => resp_empty("trt", "SetAudioEncoderConfigurationResponse"),
        "GetAudioEncoderConfigurationOptions" => media::resp_audio_encoder_configuration_options(),
        "GetOSD" => media::resp_osd(),
        "GetOSDs" => media::resp_osds(),
        "SetOSD" => resp_empty("trt", "SetOSDResponse"),
        "CreateOSD" => media::resp_create_osd(),
        "DeleteOSD" => resp_empty("trt", "DeleteOSDResponse"),
        "GetOSDOptions" => media::resp_osd_options(),
        _ => return None,
    })
}

fn dispatch_media2(op: &str, base: &str) -> Option<String> {
    Some(match op {
        "GetProfiles" => media2::resp_profiles_media2(),
        "CreateProfile" => media2::resp_create_profile_media2(),
        "DeleteProfile" => resp_empty("tr2", "DeleteProfileResponse"),
        "AddConfiguration" => resp_empty("tr2", "AddConfigurationResponse"),
        "RemoveConfiguration" => resp_empty("tr2", "RemoveConfigurationResponse"),
        "GetStreamUri" => media2::resp_stream_uri_media2(),
        "GetSnapshotUri" => media2::resp_snapshot_uri_media2(base),
        "GetVideoSourceConfigurations" => media2::resp_video_source_configurations_media2(),
        "SetVideoSourceConfiguration" => resp_empty("tr2", "SetVideoSourceConfigurationResponse"),
        "GetVideoSourceConfigurationOptions" => {
            media2::resp_video_source_configuration_options_media2()
        }
        "SetVideoEncoderConfiguration" => resp_empty("tr2", "SetVideoEncoderConfigurationResponse"),
        "GetVideoEncoderConfigurationOptions" => {
            media2::resp_video_encoder_configuration_options_media2()
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

fn dispatch_ptz(op: &str) -> Option<String> {
    Some(match op {
        "GetStatus" => ptz::resp_ptz_status(),
        "GetPresets" => ptz::resp_ptz_presets(),
        "SetPreset" => ptz::resp_ptz_set_preset(),
        "GetNodes" => ptz::resp_ptz_nodes(),
        "GetNode" => ptz::resp_ptz_node(),
        "GetConfigurations" | "GetCompatibleConfigurations" => ptz::resp_ptz_configurations(),
        "GetConfiguration" => ptz::resp_ptz_configuration(),
        "SetConfiguration" => resp_empty("tptz", "SetConfigurationResponse"),
        "GetConfigurationOptions" => ptz::resp_ptz_configuration_options(),
        "AbsoluteMove" | "RelativeMove" | "ContinuousMove" | "Stop" | "GotoPreset"
        | "GotoHomePosition" | "SetHomePosition" | "RemovePreset" => {
            resp_empty("tptz", "PTZResponse")
        }
        _ => return None,
    })
}

fn dispatch_imaging(op: &str, state: &SharedState, body: &str) -> Option<String> {
    Some(match op {
        "GetImagingSettings" => imaging::resp_imaging_settings(state),
        "SetImagingSettings" => imaging::handle_set_imaging_settings(state, body),
        "GetOptions" => imaging::resp_imaging_options(),
        "GetStatus" => imaging::resp_imaging_status(),
        "GetMoveOptions" => imaging::resp_imaging_move_options(),
        "Move" | "Stop" => resp_empty("timg", "ImagingResponse"),
        _ => return None,
    })
}

fn dispatch_events(op: &str) -> Option<String> {
    Some(match op {
        "GetEventPropertiesRequest" => events::resp_event_properties(),
        "CreatePullPointSubscriptionRequest" => events::resp_create_pull_point_subscription(),
        "PullMessagesRequest" => events::resp_pull_messages(),
        "SubscribeRequest" => events::resp_subscribe(),
        "RenewRequest" => events::resp_renew(),
        "UnsubscribeRequest" => resp_empty("wsnt", "UnsubscribeResponse"),
        "SetSynchronizationPointRequest" => resp_empty("tev", "SetSynchronizationPointResponse"),
        _ => return None,
    })
}

fn dispatch_recording(op: &str) -> Option<String> {
    Some(match op {
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
        "FindRecordings" => recording::resp_find_recordings(),
        "GetRecordingSearchResults" => recording::resp_recording_search_results(),
        "EndSearch" => resp_empty("tse", "EndSearchResponse"),
        "GetReplayUri" => recording::resp_replay_uri(),
        _ => return None,
    })
}
