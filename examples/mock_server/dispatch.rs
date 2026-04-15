use crate::helpers::{resp_empty, resp_soap_fault};
use crate::services::{device, events, imaging, media, media2, ptz, recording};
use crate::state::SharedState;

pub fn dispatch(action: &str, base: &str, state: &SharedState, body: &str) -> String {
    match action {
        // ── Device ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime" => {
            device::resp_system_date_and_time(state)
        }
        "http://www.onvif.org/ver10/device/wsdl/GetCapabilities" => device::resp_capabilities(base),
        "http://www.onvif.org/ver10/device/wsdl/GetServices" => device::resp_services(base),
        "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation" => {
            device::resp_device_info(state)
        }
        "http://www.onvif.org/ver10/device/wsdl/GetHostname" => device::resp_hostname(state),
        "http://www.onvif.org/ver10/device/wsdl/SetHostname" => {
            device::handle_set_hostname(state, body)
        }
        "http://www.onvif.org/ver10/device/wsdl/GetNTP" => device::resp_ntp(state),
        "http://www.onvif.org/ver10/device/wsdl/SetNTP" => device::handle_set_ntp(state, body),
        "http://www.onvif.org/ver10/device/wsdl/GetScopes" => device::resp_scopes(state),
        "http://www.onvif.org/ver10/device/wsdl/SetScopes" => {
            device::handle_set_scopes(state, body)
        }
        "http://www.onvif.org/ver10/device/wsdl/GetUsers" => device::resp_users(state),
        "http://www.onvif.org/ver10/device/wsdl/CreateUsers" => {
            device::handle_create_users(state, body)
        }
        "http://www.onvif.org/ver10/device/wsdl/DeleteUsers" => {
            device::handle_delete_users(state, body)
        }
        "http://www.onvif.org/ver10/device/wsdl/SetUser" => device::handle_set_user(state, body),
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkInterfaces" => {
            device::resp_network_interfaces()
        }
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces" => {
            device::resp_set_network_interfaces()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkProtocols" => {
            device::resp_network_protocols()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetDNS" => device::resp_dns(state),
        "http://www.onvif.org/ver10/device/wsdl/SetDNS" => device::handle_set_dns(state, body),
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkDefaultGateway" => {
            device::resp_network_default_gateway(state)
        }
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkDefaultGateway" => {
            resp_empty("tds", "SetNetworkDefaultGatewayResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SendAuxiliaryCommand" => {
            device::resp_send_auxiliary_command()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetSystemLog" => device::resp_system_log(),
        "http://www.onvif.org/ver10/device/wsdl/GetRelayOutputs" => device::resp_relay_outputs(),
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState" => {
            resp_empty("tds", "SetRelayOutputStateResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings" => {
            resp_empty("tds", "SetRelayOutputSettingsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols" => {
            resp_empty("tds", "SetNetworkProtocolsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault" => {
            resp_empty("tds", "SetSystemFactoryDefaultResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/GetStorageConfigurations" => {
            device::resp_storage_configurations()
        }
        "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration" => {
            resp_empty("tds", "SetStorageConfigurationResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/GetSystemUris" => device::resp_system_uris(base),
        "http://www.onvif.org/ver10/device/wsdl/GetDiscoveryMode" => {
            device::resp_discovery_mode(state)
        }
        "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode" => {
            resp_empty("tds", "SetDiscoveryModeResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetSystemDateAndTime" => {
            device::handle_set_system_date_and_time(state, body)
        }
        "http://www.onvif.org/ver10/device/wsdl/SystemReboot" => device::resp_system_reboot(),

        // ── Media1 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/media/wsdl/GetProfiles" => media::resp_profiles(),
        "http://www.onvif.org/ver10/media/wsdl/GetProfile" => media::resp_profile(),
        "http://www.onvif.org/ver10/media/wsdl/GetStreamUri" => media::resp_stream_uri(),
        "http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri" => media::resp_snapshot_uri(base),
        "http://www.onvif.org/ver10/media/wsdl/CreateProfile" => media::resp_create_profile(),
        "http://www.onvif.org/ver10/media/wsdl/DeleteProfile" => {
            resp_empty("trt", "DeleteProfileResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSources" => media::resp_video_sources(),
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations" => {
            media::resp_video_source_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations" => {
            media::resp_video_encoder_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetAudioSources" => media::resp_audio_sources(),
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations" => {
            media::resp_audio_encoder_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetOSDs" => media::resp_osds(),
        "http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration" => {
            resp_empty("trt", "ConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration" => {
            media::resp_video_source_configuration()
        }
        "http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration" => {
            resp_empty("trt", "SetVideoSourceConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions" => {
            media::resp_video_source_configuration_options()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration" => {
            media::resp_video_encoder_configuration()
        }
        "http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration" => {
            resp_empty("trt", "SetVideoEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions" => {
            media::resp_video_encoder_configuration_options()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetOSD" => media::resp_osd(),
        "http://www.onvif.org/ver10/media/wsdl/SetOSD" => resp_empty("trt", "SetOSDResponse"),
        "http://www.onvif.org/ver10/media/wsdl/CreateOSD" => media::resp_create_osd(),
        "http://www.onvif.org/ver10/media/wsdl/DeleteOSD" => resp_empty("trt", "DeleteOSDResponse"),
        "http://www.onvif.org/ver10/media/wsdl/GetOSDOptions" => media::resp_osd_options(),
        "http://www.onvif.org/ver10/media/wsdl/GetAudioSourceConfigurations" => {
            media::resp_audio_source_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration" => {
            media::resp_audio_encoder_configuration()
        }
        "http://www.onvif.org/ver10/media/wsdl/SetAudioEncoderConfiguration" => {
            resp_empty("trt", "SetAudioEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurationOptions" => {
            media::resp_audio_encoder_configuration_options()
        }

        // ── Media2 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/media/wsdl/GetProfiles" => media2::resp_profiles_media2(),
        "http://www.onvif.org/ver20/media/wsdl/GetStreamUri" => media2::resp_stream_uri_media2(),
        "http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri" => {
            media2::resp_snapshot_uri_media2(base)
        }
        "http://www.onvif.org/ver20/media/wsdl/DeleteProfile" => {
            resp_empty("tr2", "DeleteProfileResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations" => {
            media2::resp_video_source_configurations_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration" => {
            resp_empty("tr2", "SetVideoSourceConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions" => {
            media2::resp_video_source_configuration_options_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration" => {
            resp_empty("tr2", "SetVideoEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions" => {
            media2::resp_video_encoder_configuration_options_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances" => {
            media2::resp_video_encoder_instances()
        }
        "http://www.onvif.org/ver20/media/wsdl/CreateProfile" => {
            media2::resp_create_profile_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/AddConfiguration" => {
            resp_empty("tr2", "AddConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/RemoveConfiguration" => {
            resp_empty("tr2", "RemoveConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations" => {
            media2::resp_metadata_configurations()
        }
        "http://www.onvif.org/ver20/media/wsdl/SetMetadataConfiguration" => {
            resp_empty("tr2", "SetMetadataConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurationOptions" => {
            media2::resp_metadata_configuration_options()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetAudioSourceConfigurations" => {
            media2::resp_audio_source_configurations_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurations" => {
            media2::resp_audio_encoder_configurations_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurationOptions" => {
            media2::resp_audio_encoder_configuration_options_media2()
        }
        "http://www.onvif.org/ver20/media/wsdl/SetAudioEncoderConfiguration" => {
            resp_empty("tr2", "SetAudioEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetAudioOutputConfigurations" => {
            media2::resp_audio_output_configurations()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetAudioDecoderConfigurations" => {
            media2::resp_audio_decoder_configurations()
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceModes" => {
            media2::resp_video_source_modes()
        }
        "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceMode" => {
            media2::resp_set_video_source_mode()
        }

        // ── PTZ ───────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/ptz/wsdl/GetStatus" => ptz::resp_ptz_status(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetPresets" => ptz::resp_ptz_presets(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetNodes" => ptz::resp_ptz_nodes(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetNode" => ptz::resp_ptz_node(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurations" => ptz::resp_ptz_configurations(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetCompatibleConfigurations" => {
            ptz::resp_ptz_configurations()
        }
        "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/Stop"
        | "http://www.onvif.org/ver20/ptz/wsdl/GotoPreset"
        | "http://www.onvif.org/ver20/ptz/wsdl/GotoHomePosition"
        | "http://www.onvif.org/ver20/ptz/wsdl/SetHomePosition"
        | "http://www.onvif.org/ver20/ptz/wsdl/RemovePreset" => resp_empty("tptz", "PTZResponse"),
        "http://www.onvif.org/ver20/ptz/wsdl/SetPreset" => ptz::resp_ptz_set_preset(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration" => ptz::resp_ptz_configuration(),
        "http://www.onvif.org/ver20/ptz/wsdl/SetConfiguration" => {
            resp_empty("tptz", "SetConfigurationResponse")
        }
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions" => {
            ptz::resp_ptz_configuration_options()
        }

        // ── Imaging ───────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings" => {
            imaging::resp_imaging_settings(state)
        }
        "http://www.onvif.org/ver20/imaging/wsdl/SetImagingSettings" => {
            imaging::handle_set_imaging_settings(state, body)
        }
        "http://www.onvif.org/ver20/imaging/wsdl/GetOptions" => imaging::resp_imaging_options(),
        "http://www.onvif.org/ver20/imaging/wsdl/GetStatus" => imaging::resp_imaging_status(),
        "http://www.onvif.org/ver20/imaging/wsdl/GetMoveOptions" => {
            imaging::resp_imaging_move_options()
        }
        "http://www.onvif.org/ver20/imaging/wsdl/Move"
        | "http://www.onvif.org/ver20/imaging/wsdl/Stop" => resp_empty("timg", "ImagingResponse"),

        // ── Events ───────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetEventPropertiesRequest" => {
            events::resp_event_properties()
        }
        "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest" => {
            events::resp_create_pull_point_subscription()
        }
        "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest" => {
            events::resp_pull_messages()
        }
        "http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/SubscribeRequest" => {
            events::resp_subscribe()
        }
        "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest" => {
            events::resp_renew()
        }
        "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest" => {
            resp_empty("wsnt", "UnsubscribeResponse")
        }
        "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest" => {
            resp_empty("tev", "SetSynchronizationPointResponse")
        }

        // ── Recording ─────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/recording/wsdl/GetRecordings" => recording::resp_recordings(),
        "http://www.onvif.org/ver10/recording/wsdl/CreateRecording" => {
            recording::resp_create_recording()
        }
        "http://www.onvif.org/ver10/recording/wsdl/DeleteRecording" => {
            resp_empty("trc", "DeleteRecordingResponse")
        }
        "http://www.onvif.org/ver10/recording/wsdl/CreateTrack" => recording::resp_create_track(),
        "http://www.onvif.org/ver10/recording/wsdl/DeleteTrack" => {
            resp_empty("trc", "DeleteTrackResponse")
        }
        "http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobs" => {
            recording::resp_recording_jobs()
        }
        "http://www.onvif.org/ver10/recording/wsdl/CreateRecordingJob" => {
            recording::resp_create_recording_job()
        }
        "http://www.onvif.org/ver10/recording/wsdl/SetRecordingJobMode" => {
            resp_empty("trc", "SetRecordingJobModeResponse")
        }
        "http://www.onvif.org/ver10/recording/wsdl/DeleteRecordingJob" => {
            resp_empty("trc", "DeleteRecordingJobResponse")
        }
        "http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobState" => {
            recording::resp_recording_job_state()
        }
        "http://www.onvif.org/ver10/search/wsdl/FindRecordings" => {
            recording::resp_find_recordings()
        }
        "http://www.onvif.org/ver10/search/wsdl/GetRecordingSearchResults" => {
            recording::resp_recording_search_results()
        }
        "http://www.onvif.org/ver10/search/wsdl/EndSearch" => {
            resp_empty("tse", "EndSearchResponse")
        }
        "http://www.onvif.org/ver10/replay/wsdl/GetReplayUri" => recording::resp_replay_uri(),

        // ── Unknown ───────────────────────────────────────────────────────────
        other => {
            eprintln!("  [WARN] unhandled action: {other}");
            resp_soap_fault("s:Receiver", &format!("Not implemented: {other}"))
        }
    }
}
