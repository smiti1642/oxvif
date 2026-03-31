use super::*;
use crate::soap::XmlNode;

fn parse(xml: &str) -> XmlNode {
    XmlNode::parse(xml).unwrap()
}

mod capabilities {
    use super::*;

    /// Full response exercising every field defined in the ONVIF spec.
    const FULL: &str = r#"<GetCapabilitiesResponse>
          <Capabilities>
            <Device>
              <XAddr>http://192.168.1.1/onvif/device_service</XAddr>
              <Network>
                <IPFilter>false</IPFilter>
                <ZeroConfiguration>true</ZeroConfiguration>
                <IPVersion6>false</IPVersion6>
                <DynDNS>false</DynDNS>
              </Network>
              <System>
                <DiscoveryResolve>true</DiscoveryResolve>
                <DiscoveryBye>true</DiscoveryBye>
                <RemoteDiscovery>false</RemoteDiscovery>
                <SystemBackup>true</SystemBackup>
                <SystemLogging>true</SystemLogging>
                <FirmwareUpgrade>true</FirmwareUpgrade>
              </System>
              <IO>
                <InputConnectors>1</InputConnectors>
                <RelayOutputs>2</RelayOutputs>
              </IO>
              <Security>
                <TLS1.2>true</TLS1.2>
                <OnboardKeyGeneration>false</OnboardKeyGeneration>
                <AccessPolicyConfig>false</AccessPolicyConfig>
                <X.509Token>false</X.509Token>
                <UsernameToken>true</UsernameToken>
              </Security>
            </Device>
            <Media>
              <XAddr>http://192.168.1.1/onvif/media_service</XAddr>
              <StreamingCapabilities>
                <RTPMulticast>false</RTPMulticast>
                <RTP_TCP>true</RTP_TCP>
                <RTP_RTSP_TCP>true</RTP_RTSP_TCP>
              </StreamingCapabilities>
              <MaximumNumberOfProfiles>5</MaximumNumberOfProfiles>
            </Media>
            <PTZ>
              <XAddr>http://192.168.1.1/onvif/ptz_service</XAddr>
            </PTZ>
            <Events>
              <XAddr>http://192.168.1.1/onvif/events_service</XAddr>
              <WSSubscriptionPolicySupport>true</WSSubscriptionPolicySupport>
              <WSPullPointSupport>true</WSPullPointSupport>
            </Events>
            <Imaging>
              <XAddr>http://192.168.1.1/onvif/imaging_service</XAddr>
            </Imaging>
            <Analytics>
              <XAddr>http://192.168.1.1/onvif/analytics_service</XAddr>
              <RuleSupport>true</RuleSupport>
              <AnalyticsModuleSupport>true</AnalyticsModuleSupport>
            </Analytics>
            <Extension>
              <DeviceIO>  <XAddr>http://192.168.1.1/onvif/deviceio_service</XAddr>  </DeviceIO>
              <Recording> <XAddr>http://192.168.1.1/onvif/recording_service</XAddr> </Recording>
              <Search>    <XAddr>http://192.168.1.1/onvif/search_service</XAddr>    </Search>
              <Replay>    <XAddr>http://192.168.1.1/onvif/replay_service</XAddr>    </Replay>
            </Extension>
          </Capabilities>
        </GetCapabilitiesResponse>"#;

    // ── Service URLs ──────────────────────────────────────────────────────

    #[test]
    fn test_all_service_urls_parsed() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert_eq!(
            caps.device.url.as_deref(),
            Some("http://192.168.1.1/onvif/device_service")
        );
        assert_eq!(
            caps.media.url.as_deref(),
            Some("http://192.168.1.1/onvif/media_service")
        );
        assert_eq!(
            caps.ptz_url.as_deref(),
            Some("http://192.168.1.1/onvif/ptz_service")
        );
        assert_eq!(
            caps.events.url.as_deref(),
            Some("http://192.168.1.1/onvif/events_service")
        );
        assert_eq!(
            caps.imaging_url.as_deref(),
            Some("http://192.168.1.1/onvif/imaging_service")
        );
        assert_eq!(
            caps.analytics.url.as_deref(),
            Some("http://192.168.1.1/onvif/analytics_service")
        );
    }

    #[test]
    fn test_extension_service_urls_parsed() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert_eq!(
            caps.device_io_url.as_deref(),
            Some("http://192.168.1.1/onvif/deviceio_service")
        );
        assert_eq!(
            caps.recording_url.as_deref(),
            Some("http://192.168.1.1/onvif/recording_service")
        );
        assert_eq!(
            caps.search_url.as_deref(),
            Some("http://192.168.1.1/onvif/search_service")
        );
        assert_eq!(
            caps.replay_url.as_deref(),
            Some("http://192.168.1.1/onvif/replay_service")
        );
    }

    // ── Device sub-capabilities ───────────────────────────────────────────

    #[test]
    fn test_device_network_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(!caps.device.network.ip_filter);
        assert!(caps.device.network.zero_configuration);
        assert!(!caps.device.network.ip_version6);
        assert!(!caps.device.network.dyn_dns);
    }

    #[test]
    fn test_device_system_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(caps.device.system.discovery_resolve);
        assert!(caps.device.system.discovery_bye);
        assert!(!caps.device.system.remote_discovery);
        assert!(caps.device.system.system_backup);
        assert!(caps.device.system.system_logging);
        assert!(caps.device.system.firmware_upgrade);
    }

    #[test]
    fn test_device_io_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert_eq!(caps.device.io.input_connectors, Some(1));
        assert_eq!(caps.device.io.relay_outputs, Some(2));
    }

    #[test]
    fn test_device_security_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(caps.device.security.tls_1_2);
        assert!(!caps.device.security.onboard_key_generation);
        assert!(!caps.device.security.x509_token);
        assert!(caps.device.security.username_token);
    }

    // ── Media sub-capabilities ────────────────────────────────────────────

    #[test]
    fn test_media_streaming_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(!caps.media.streaming.rtp_multicast);
        assert!(caps.media.streaming.rtp_tcp);
        assert!(caps.media.streaming.rtp_rtsp_tcp);
    }

    #[test]
    fn test_media_max_profiles() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert_eq!(caps.media.max_profiles, Some(5));
    }

    // ── Events sub-capabilities ───────────────────────────────────────────

    #[test]
    fn test_events_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(caps.events.ws_subscription_policy);
        assert!(caps.events.ws_pull_point);
    }

    // ── Analytics sub-capabilities ────────────────────────────────────────

    #[test]
    fn test_analytics_capabilities() {
        let caps = Capabilities::from_xml(&parse(FULL)).unwrap();
        assert!(caps.analytics.rule_support);
        assert!(caps.analytics.analytics_module_support);
    }

    // ── Absence / error cases ─────────────────────────────────────────────

    #[test]
    fn test_optional_services_absent_are_none() {
        let xml = r#"<GetCapabilitiesResponse>
              <Capabilities>
                <Device><XAddr>http://192.168.1.1/onvif/device_service</XAddr></Device>
                <Media> <XAddr>http://192.168.1.1/onvif/media_service</XAddr> </Media>
              </Capabilities>
            </GetCapabilitiesResponse>"#;
        let caps = Capabilities::from_xml(&parse(xml)).unwrap();
        assert!(caps.ptz_url.is_none());
        assert!(caps.events.url.is_none());
        assert!(caps.imaging_url.is_none());
        assert!(caps.analytics.url.is_none());
        assert!(caps.recording_url.is_none());
    }

    #[test]
    fn test_absent_boolean_fields_default_to_false() {
        let xml = r#"<GetCapabilitiesResponse>
              <Capabilities>
                <Device><XAddr>http://192.168.1.1/onvif/device_service</XAddr></Device>
              </Capabilities>
            </GetCapabilitiesResponse>"#;
        let caps = Capabilities::from_xml(&parse(xml)).unwrap();
        assert!(!caps.device.network.ip_filter);
        assert!(!caps.device.system.firmware_upgrade);
        assert!(!caps.device.security.username_token);
        assert!(!caps.media.streaming.rtp_tcp);
        assert!(!caps.events.ws_pull_point);
    }

    #[test]
    fn test_missing_capabilities_node_is_error() {
        let err = Capabilities::from_xml(&parse("<GetCapabilitiesResponse/>")).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("Capabilities"))
        ));
    }
}

mod device_info {
    use super::*;

    const FULL: &str = r#"<GetDeviceInformationResponse>
          <Manufacturer>Hikvision</Manufacturer>
          <Model>DS-2CD2085G1-I</Model>
          <FirmwareVersion>V5.6.1 build 190813</FirmwareVersion>
          <SerialNumber>DS-2CD2085G1-I20190619AACH123456789</SerialNumber>
          <HardwareId>0x00</HardwareId>
        </GetDeviceInformationResponse>"#;

    #[test]
    fn test_all_fields_parsed() {
        let info = DeviceInfo::from_xml(&parse(FULL)).unwrap();
        assert_eq!(info.manufacturer, "Hikvision");
        assert_eq!(info.model, "DS-2CD2085G1-I");
        assert_eq!(info.firmware_version, "V5.6.1 build 190813");
        assert_eq!(info.serial_number, "DS-2CD2085G1-I20190619AACH123456789");
        assert_eq!(info.hardware_id, "0x00");
    }

    #[test]
    fn test_absent_fields_default_to_empty_string() {
        let info = DeviceInfo::from_xml(&parse("<GetDeviceInformationResponse/>")).unwrap();
        assert_eq!(info.manufacturer, "");
        assert_eq!(info.model, "");
        assert_eq!(info.firmware_version, "");
    }

    #[test]
    fn test_partial_response_fills_present_fields() {
        let xml = r#"<GetDeviceInformationResponse>
              <Manufacturer>Axis</Manufacturer>
              <Model>P3245-V</Model>
            </GetDeviceInformationResponse>"#;
        let info = DeviceInfo::from_xml(&parse(xml)).unwrap();
        assert_eq!(info.manufacturer, "Axis");
        assert_eq!(info.model, "P3245-V");
        assert_eq!(info.firmware_version, "");
    }
}

mod media_profile {
    use super::*;

    const TWO_PROFILES: &str = r#"<GetProfilesResponse>
          <Profiles token="Profile_1" fixed="true">
            <Name>mainStream</Name>
          </Profiles>
          <Profiles token="Profile_2" fixed="false">
            <Name>subStream</Name>
          </Profiles>
        </GetProfilesResponse>"#;

    #[test]
    fn test_two_profiles_returned() {
        let profiles = MediaProfile::vec_from_xml(&parse(TWO_PROFILES)).unwrap();
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn test_profile_fields() {
        let profiles = MediaProfile::vec_from_xml(&parse(TWO_PROFILES)).unwrap();
        assert_eq!(profiles[0].token, "Profile_1");
        assert_eq!(profiles[0].name, "mainStream");
        assert!(profiles[0].fixed);
        assert_eq!(profiles[1].token, "Profile_2");
        assert_eq!(profiles[1].name, "subStream");
        assert!(!profiles[1].fixed);
    }

    #[test]
    fn test_empty_response_returns_empty_vec() {
        let profiles = MediaProfile::vec_from_xml(&parse("<GetProfilesResponse/>")).unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_fixed_absent_defaults_to_false() {
        let xml = r#"<GetProfilesResponse>
              <Profiles token="tok"><Name>noFixed</Name></Profiles>
            </GetProfilesResponse>"#;
        let profiles = MediaProfile::vec_from_xml(&parse(xml)).unwrap();
        assert!(!profiles[0].fixed);
    }
}

mod stream_uri {
    use super::*;

    const FULL: &str = r#"<GetStreamUriResponse>
          <MediaUri>
            <Uri>rtsp://192.168.1.1:554/Streaming/Channels/101</Uri>
            <InvalidAfterConnect>false</InvalidAfterConnect>
            <InvalidAfterReboot>false</InvalidAfterReboot>
            <Timeout>PT0S</Timeout>
          </MediaUri>
        </GetStreamUriResponse>"#;

    #[test]
    fn test_all_fields_parsed() {
        let uri = StreamUri::from_xml(&parse(FULL)).unwrap();
        assert_eq!(uri.uri, "rtsp://192.168.1.1:554/Streaming/Channels/101");
        assert!(!uri.invalid_after_connect);
        assert!(!uri.invalid_after_reboot);
        assert_eq!(uri.timeout, "PT0S");
    }

    #[test]
    fn test_invalid_after_connect_and_reboot_true() {
        let xml = r#"<GetStreamUriResponse>
              <MediaUri>
                <Uri>rtsp://192.168.1.1/stream</Uri>
                <InvalidAfterConnect>true</InvalidAfterConnect>
                <InvalidAfterReboot>true</InvalidAfterReboot>
              </MediaUri>
            </GetStreamUriResponse>"#;
        let uri = StreamUri::from_xml(&parse(xml)).unwrap();
        assert!(uri.invalid_after_connect);
        assert!(uri.invalid_after_reboot);
    }

    #[test]
    fn test_missing_media_uri_is_error() {
        let err = StreamUri::from_xml(&parse("<GetStreamUriResponse/>")).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("MediaUri"))
        ));
    }

    #[test]
    fn test_missing_uri_element_is_error() {
        let xml = "<GetStreamUriResponse><MediaUri/></GetStreamUriResponse>";
        let err = StreamUri::from_xml(&parse(xml)).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("Uri"))
        ));
    }

    #[test]
    fn test_empty_uri_text_is_error() {
        let xml = "<GetStreamUriResponse><MediaUri><Uri></Uri></MediaUri></GetStreamUriResponse>";
        let err = StreamUri::from_xml(&parse(xml)).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("Uri"))
        ));
    }

    #[test]
    fn test_timeout_absent_defaults_to_empty() {
        let xml = r#"<GetStreamUriResponse>
              <MediaUri><Uri>rtsp://x/s</Uri></MediaUri>
            </GetStreamUriResponse>"#;
        let uri = StreamUri::from_xml(&parse(xml)).unwrap();
        assert_eq!(uri.timeout, "");
    }
}

mod system_date_time {
    use super::*;

    const FULL: &str = r#"<GetSystemDateAndTimeResponse>
          <SystemDateAndTime>
            <DateTimeType>NTP</DateTimeType>
            <DaylightSavings>true</DaylightSavings>
            <TimeZone><TZ>CST-8</TZ></TimeZone>
            <UTCDateTime>
              <Time><Hour>10</Hour><Minute>30</Minute><Second>45</Second></Time>
              <Date><Year>2024</Year><Month>6</Month><Day>15</Day></Date>
            </UTCDateTime>
          </SystemDateAndTime>
        </GetSystemDateAndTimeResponse>"#;

    #[test]
    fn test_utc_unix_correct() {
        let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
        // 2024-06-15T10:30:45Z = 1_718_447_445
        assert_eq!(dt.utc_unix, Some(1_718_447_445));
    }

    #[test]
    fn test_daylight_savings_parsed() {
        let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
        assert!(dt.daylight_savings);
    }

    #[test]
    fn test_timezone_parsed() {
        let dt = SystemDateTime::from_xml(&parse(FULL)).unwrap();
        assert_eq!(dt.timezone, "CST-8");
    }

    #[test]
    fn test_missing_utc_datetime_gives_none() {
        let xml = r#"<GetSystemDateAndTimeResponse>
              <SystemDateAndTime>
                <DaylightSavings>false</DaylightSavings>
              </SystemDateAndTime>
            </GetSystemDateAndTimeResponse>"#;
        let dt = SystemDateTime::from_xml(&parse(xml)).unwrap();
        assert!(dt.utc_unix.is_none());
    }

    #[test]
    fn test_missing_system_date_and_time_is_error() {
        let err = SystemDateTime::from_xml(&parse("<GetSystemDateAndTimeResponse/>")).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("SystemDateAndTime"))
        ));
    }

    #[test]
    fn test_civil_to_unix_epoch() {
        assert_eq!(civil_to_unix(1970, 1, 1, 0, 0, 0), 0);
    }

    #[test]
    fn test_civil_to_unix_known_date() {
        // 2024-01-01T00:00:00Z = 1_704_067_200
        assert_eq!(civil_to_unix(2024, 1, 1, 0, 0, 0), 1_704_067_200);
    }
}

mod snapshot_uri {
    use super::*;

    const FULL: &str = r#"<GetSnapshotUriResponse>
          <MediaUri>
            <Uri>http://192.168.1.1/onvif/snapshot?channel=1</Uri>
            <InvalidAfterConnect>false</InvalidAfterConnect>
            <InvalidAfterReboot>true</InvalidAfterReboot>
            <Timeout>PT60S</Timeout>
          </MediaUri>
        </GetSnapshotUriResponse>"#;

    #[test]
    fn test_all_fields_parsed() {
        let uri = SnapshotUri::from_xml(&parse(FULL)).unwrap();
        assert_eq!(uri.uri, "http://192.168.1.1/onvif/snapshot?channel=1");
        assert!(!uri.invalid_after_connect);
        assert!(uri.invalid_after_reboot);
        assert_eq!(uri.timeout, "PT60S");
    }

    #[test]
    fn test_missing_media_uri_is_error() {
        let err = SnapshotUri::from_xml(&parse("<GetSnapshotUriResponse/>")).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("MediaUri"))
        ));
    }

    #[test]
    fn test_empty_uri_is_error() {
        let xml = "<GetSnapshotUriResponse><MediaUri><Uri/></MediaUri></GetSnapshotUriResponse>";
        let err = SnapshotUri::from_xml(&parse(xml)).unwrap_err();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("Uri"))
        ));
    }
}

mod ptz_preset {
    use super::*;

    const TWO_PRESETS: &str = r#"<GetPresetsResponse>
          <Preset token="1">
            <Name>Front Gate</Name>
            <PTZPosition>
              <PanTilt x="0.1" y="-0.2"/>
              <Zoom x="0.5"/>
            </PTZPosition>
          </Preset>
          <Preset token="2">
            <Name>Parking Lot</Name>
          </Preset>
        </GetPresetsResponse>"#;

    #[test]
    fn test_two_presets_returned() {
        let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
        assert_eq!(presets.len(), 2);
    }

    #[test]
    fn test_preset_fields() {
        let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
        assert_eq!(presets[0].token, "1");
        assert_eq!(presets[0].name, "Front Gate");
    }

    #[test]
    fn test_preset_position_parsed() {
        let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
        let (pan, tilt) = presets[0].pan_tilt.unwrap();
        assert!((pan - 0.1).abs() < 1e-5);
        assert!((tilt - (-0.2)).abs() < 1e-5);
        assert!((presets[0].zoom.unwrap() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_preset_without_position_is_none() {
        let presets = PtzPreset::vec_from_xml(&parse(TWO_PRESETS)).unwrap();
        assert!(presets[1].pan_tilt.is_none());
        assert!(presets[1].zoom.is_none());
    }

    #[test]
    fn test_empty_response_returns_empty_vec() {
        let presets = PtzPreset::vec_from_xml(&parse("<GetPresetsResponse/>")).unwrap();
        assert!(presets.is_empty());
    }
}

mod video {
    use super::*;

    // ── VideoSource ───────────────────────────────────────────────────────

    const TWO_SOURCES: &str = r#"<GetVideoSourcesResponse>
          <VideoSources token="VideoSource_1">
            <Framerate>25</Framerate>
            <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
          </VideoSources>
          <VideoSources token="VideoSource_2">
            <Framerate>15</Framerate>
            <Resolution><Width>1280</Width><Height>720</Height></Resolution>
          </VideoSources>
        </GetVideoSourcesResponse>"#;

    #[test]
    fn test_video_sources_count() {
        let sources = VideoSource::vec_from_xml(&parse(TWO_SOURCES)).unwrap();
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn test_video_sources_fields() {
        let sources = VideoSource::vec_from_xml(&parse(TWO_SOURCES)).unwrap();
        assert_eq!(sources[0].token, "VideoSource_1");
        assert!((sources[0].framerate - 25.0).abs() < 1e-5);
        assert_eq!(
            sources[0].resolution,
            Resolution {
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(sources[1].token, "VideoSource_2");
        assert_eq!(
            sources[1].resolution,
            Resolution {
                width: 1280,
                height: 720
            }
        );
    }

    // ── VideoSourceConfiguration ──────────────────────────────────────────

    const VSC_XML: &str = r#"<Configuration token="VSC_1">
          <Name>VideoSourceConfig</Name>
          <UseCount>2</UseCount>
          <SourceToken>VideoSource_1</SourceToken>
          <Bounds x="0" y="0" width="1920" height="1080"/>
        </Configuration>"#;

    #[test]
    fn test_video_source_configuration_from_xml() {
        let cfg = VideoSourceConfiguration::from_xml(&parse(VSC_XML)).unwrap();
        assert_eq!(cfg.token, "VSC_1");
        assert_eq!(cfg.name, "VideoSourceConfig");
        assert_eq!(cfg.use_count, 2);
        assert_eq!(cfg.source_token, "VideoSource_1");
        assert_eq!(cfg.bounds.x, 0);
        assert_eq!(cfg.bounds.y, 0);
        assert_eq!(cfg.bounds.width, 1920);
        assert_eq!(cfg.bounds.height, 1080);
    }

    #[test]
    fn test_video_source_configuration_to_xml_body_round_trip() {
        let cfg = VideoSourceConfiguration {
            token: "tok1".into(),
            name: "MyCfg".into(),
            use_count: 1,
            source_token: "src1".into(),
            bounds: SourceBounds {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
        };
        let xml = cfg.to_xml_body();
        assert!(xml.contains("token=\"tok1\""));
        assert!(xml.contains("<tt:Name>MyCfg</tt:Name>"));
        assert!(xml.contains("<tt:SourceToken>src1</tt:SourceToken>"));
        assert!(xml.contains("x=\"10\""));
        assert!(xml.contains("y=\"20\""));
        assert!(xml.contains("width=\"640\""));
        assert!(xml.contains("height=\"480\""));
    }

    // ── VideoSourceConfigurationOptions ───────────────────────────────────

    const VSCO_XML: &str = r#"<GetVideoSourceConfigurationOptionsResponse>
          <Options>
            <MaximumNumberOfProfiles>5</MaximumNumberOfProfiles>
            <BoundsRange>
              <XRange><Min>0</Min><Max>0</Max></XRange>
              <YRange><Min>0</Min><Max>0</Max></YRange>
              <WidthRange><Min>320</Min><Max>1920</Max></WidthRange>
              <HeightRange><Min>240</Min><Max>1080</Max></HeightRange>
            </BoundsRange>
            <VideoSourceTokensAvailable>VideoSource_1</VideoSourceTokensAvailable>
            <VideoSourceTokensAvailable>VideoSource_2</VideoSourceTokensAvailable>
          </Options>
        </GetVideoSourceConfigurationOptionsResponse>"#;

    #[test]
    fn test_video_source_configuration_options_from_xml() {
        let opts = VideoSourceConfigurationOptions::from_xml(&parse(VSCO_XML)).unwrap();
        assert_eq!(opts.max_limit, Some(5));
        assert_eq!(opts.source_tokens.len(), 2);
        assert_eq!(opts.source_tokens[0], "VideoSource_1");
        let br = opts.bounds_range.unwrap();
        assert_eq!(br.width_range.min, 320);
        assert_eq!(br.width_range.max, 1920);
        assert_eq!(br.height_range.min, 240);
        assert_eq!(br.height_range.max, 1080);
    }

    // ── VideoEncoding ─────────────────────────────────────────────────────

    #[test]
    fn test_video_encoding_from_str() {
        assert_eq!(VideoEncoding::from_str("JPEG"), VideoEncoding::Jpeg);
        assert_eq!(VideoEncoding::from_str("H264"), VideoEncoding::H264);
        assert_eq!(VideoEncoding::from_str("H265"), VideoEncoding::H265);
        assert_eq!(VideoEncoding::from_str("H.265"), VideoEncoding::H265);
        assert_eq!(
            VideoEncoding::from_str("MPEG4"),
            VideoEncoding::Other("MPEG4".into())
        );
    }

    // ── VideoEncoderConfiguration ─────────────────────────────────────────

    const VEC_XML: &str = r#"<Configuration token="VideoEncoder_1">
          <Name>MainStream</Name>
          <UseCount>1</UseCount>
          <Encoding>H264</Encoding>
          <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
          <Quality>5</Quality>
          <RateControl>
            <FrameRateLimit>25</FrameRateLimit>
            <EncodingInterval>1</EncodingInterval>
            <BitrateLimit>4096</BitrateLimit>
          </RateControl>
          <H264>
            <GovLength>30</GovLength>
            <H264Profile>Main</H264Profile>
          </H264>
        </Configuration>"#;

    #[test]
    fn test_video_encoder_configuration_from_xml() {
        let cfg = VideoEncoderConfiguration::from_xml(&parse(VEC_XML)).unwrap();
        assert_eq!(cfg.token, "VideoEncoder_1");
        assert_eq!(cfg.name, "MainStream");
        assert_eq!(cfg.use_count, 1);
        assert_eq!(cfg.encoding, VideoEncoding::H264);
        assert_eq!(
            cfg.resolution,
            Resolution {
                width: 1920,
                height: 1080
            }
        );
        assert!((cfg.quality - 5.0).abs() < 1e-5);
        let rc = cfg.rate_control.unwrap();
        assert_eq!(rc.frame_rate_limit, 25);
        assert_eq!(rc.encoding_interval, 1);
        assert_eq!(rc.bitrate_limit, 4096);
        let h264 = cfg.h264.unwrap();
        assert_eq!(h264.gov_length, 30);
        assert_eq!(h264.profile, "Main");
        assert!(cfg.h265.is_none());
    }

    const TWO_VEC_XML: &str = r#"<GetVideoEncoderConfigurationsResponse>
          <Configurations token="VideoEncoder_1">
            <Name>MainStream</Name>
            <UseCount>1</UseCount>
            <Encoding>H264</Encoding>
            <Resolution><Width>1920</Width><Height>1080</Height></Resolution>
            <Quality>5</Quality>
          </Configurations>
          <Configurations token="VideoEncoder_2">
            <Name>SubStream</Name>
            <UseCount>1</UseCount>
            <Encoding>JPEG</Encoding>
            <Resolution><Width>640</Width><Height>480</Height></Resolution>
            <Quality>3</Quality>
          </Configurations>
        </GetVideoEncoderConfigurationsResponse>"#;

    #[test]
    fn test_video_encoder_configuration_vec_from_xml() {
        let cfgs = VideoEncoderConfiguration::vec_from_xml(&parse(TWO_VEC_XML)).unwrap();
        assert_eq!(cfgs.len(), 2);
        assert_eq!(cfgs[0].token, "VideoEncoder_1");
        assert_eq!(cfgs[1].encoding, VideoEncoding::Jpeg);
    }

    #[test]
    fn test_video_encoder_configuration_to_xml_body_round_trip() {
        let cfg = VideoEncoderConfiguration {
            token: "enc1".into(),
            name: "Main".into(),
            use_count: 1,
            encoding: VideoEncoding::H264,
            resolution: Resolution {
                width: 1280,
                height: 720,
            },
            quality: 4.0,
            rate_control: Some(VideoRateControl {
                frame_rate_limit: 30,
                encoding_interval: 1,
                bitrate_limit: 2048,
            }),
            h264: Some(H264Configuration {
                gov_length: 25,
                profile: "Baseline".into(),
            }),
            h265: None,
        };
        let xml = cfg.to_xml_body();
        assert!(xml.contains("token=\"enc1\""));
        assert!(xml.contains("<tt:Encoding>H264</tt:Encoding>"));
        assert!(xml.contains("<tt:Width>1280</tt:Width>"));
        assert!(xml.contains("<tt:FrameRateLimit>30</tt:FrameRateLimit>"));
        assert!(xml.contains("<tt:GovLength>25</tt:GovLength>"));
        assert!(xml.contains("<tt:H264Profile>Baseline</tt:H264Profile>"));
    }

    // ── VideoEncoderConfigurationOptions ──────────────────────────────────

    const VECO_XML: &str = r#"<GetVideoEncoderConfigurationOptionsResponse>
          <Options>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <JPEG>
              <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
              <ResolutionsAvailable><Width>1280</Width><Height>720</Height></ResolutionsAvailable>
              <FrameRateRange><Min>1</Min><Max>30</Max></FrameRateRange>
              <EncodingIntervalRange><Min>1</Min><Max>1</Max></EncodingIntervalRange>
            </JPEG>
            <H264>
              <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
              <GovLengthRange><Min>1</Min><Max>150</Max></GovLengthRange>
              <FrameRateRange><Min>1</Min><Max>30</Max></FrameRateRange>
              <EncodingIntervalRange><Min>1</Min><Max>1</Max></EncodingIntervalRange>
              <BitrateRange><Min>32</Min><Max>16384</Max></BitrateRange>
              <H264ProfilesSupported>Baseline</H264ProfilesSupported>
              <H264ProfilesSupported>Main</H264ProfilesSupported>
              <H264ProfilesSupported>High</H264ProfilesSupported>
            </H264>
          </Options>
        </GetVideoEncoderConfigurationOptionsResponse>"#;

    #[test]
    fn test_video_encoder_configuration_options_from_xml() {
        let opts = VideoEncoderConfigurationOptions::from_xml(&parse(VECO_XML)).unwrap();
        let qr = opts.quality_range.unwrap();
        assert!((qr.min - 1.0).abs() < 1e-5);
        assert!((qr.max - 10.0).abs() < 1e-5);
        let jpeg = opts.jpeg.unwrap();
        assert_eq!(jpeg.resolutions.len(), 2);
        assert_eq!(
            jpeg.resolutions[0],
            Resolution {
                width: 1920,
                height: 1080
            }
        );
        let fr = jpeg.frame_rate_range.unwrap();
        assert_eq!(fr.min, 1);
        assert_eq!(fr.max, 30);
        let h264 = opts.h264.unwrap();
        assert_eq!(h264.profiles.len(), 3);
        assert_eq!(h264.profiles[0], "Baseline");
        let br = h264.bitrate_range.unwrap();
        assert_eq!(br.min, 32);
        assert_eq!(br.max, 16384);
        let glr = h264.gov_length_range.unwrap();
        assert_eq!(glr.max, 150);
        assert!(opts.h265.is_none());
    }
}

mod media2 {
    use super::*;

    // ── MediaProfile2 ─────────────────────────────────────────────────────

    const TWO_PROFILES2: &str = r#"<GetProfilesResponse>
          <Profiles token="Profile_A" fixed="true">
            <Name>mainStream</Name>
            <Configurations>
              <VideoSource token="VSC_1"/>
              <VideoEncoder token="VEC_1"/>
            </Configurations>
          </Profiles>
          <Profiles token="Profile_B" fixed="false">
            <Name>subStream</Name>
            <Configurations>
              <VideoSource token="VSC_1"/>
            </Configurations>
          </Profiles>
        </GetProfilesResponse>"#;

    #[test]
    fn test_media_profile2_vec_from_xml() {
        let profiles = MediaProfile2::vec_from_xml(&parse(TWO_PROFILES2)).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].token, "Profile_A");
        assert_eq!(profiles[0].name, "mainStream");
        assert!(profiles[0].fixed);
        assert_eq!(profiles[0].video_source_token.as_deref(), Some("VSC_1"));
        assert_eq!(profiles[0].video_encoder_token.as_deref(), Some("VEC_1"));
        assert_eq!(profiles[1].token, "Profile_B");
        assert_eq!(profiles[1].name, "subStream");
        assert!(!profiles[1].fixed);
        assert_eq!(profiles[1].video_source_token.as_deref(), Some("VSC_1"));
        assert!(profiles[1].video_encoder_token.is_none());
    }

    // ── VideoEncoderConfiguration2 ────────────────────────────────────────

    const H265_CONFIG: &str = r#"<Configurations token="VEC_H265">
          <Name>H265Stream</Name>
          <UseCount>1</UseCount>
          <Encoding>H265</Encoding>
          <Resolution><Width>3840</Width><Height>2160</Height></Resolution>
          <Quality>7</Quality>
          <RateControl>
            <FrameRateLimit>30</FrameRateLimit>
            <BitrateLimit>8192</BitrateLimit>
          </RateControl>
          <GovLength>60</GovLength>
          <Profile>Main</Profile>
        </Configurations>"#;

    #[test]
    fn test_video_encoder_configuration2_from_xml_h265() {
        let cfg = VideoEncoderConfiguration2::from_xml(&parse(H265_CONFIG)).unwrap();
        assert_eq!(cfg.token, "VEC_H265");
        assert_eq!(cfg.name, "H265Stream");
        assert_eq!(cfg.encoding, VideoEncoding::H265);
        assert_eq!(
            cfg.resolution,
            Resolution {
                width: 3840,
                height: 2160
            }
        );
        assert!((cfg.quality - 7.0).abs() < 1e-5);
        let rc = cfg.rate_control.unwrap();
        assert_eq!(rc.frame_rate_limit, 30);
        assert_eq!(rc.bitrate_limit, 8192);
        assert_eq!(cfg.gov_length, Some(60));
        assert_eq!(cfg.profile.as_deref(), Some("Main"));
    }

    #[test]
    fn test_video_encoder_configuration2_to_xml_body() {
        let cfg = VideoEncoderConfiguration2 {
            token: "enc2".into(),
            name: "H265Main".into(),
            use_count: 1,
            encoding: VideoEncoding::H265,
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            quality: 6.0,
            rate_control: Some(VideoRateControl2 {
                frame_rate_limit: 25,
                bitrate_limit: 4096,
            }),
            gov_length: Some(50),
            profile: Some("Main".into()),
        };
        let xml = cfg.to_xml_body();
        assert!(xml.contains("token=\"enc2\""));
        assert!(xml.contains("<tt:Encoding>H265</tt:Encoding>"));
        assert!(xml.contains("<tt:Width>1920</tt:Width>"));
        assert!(xml.contains("<tt:FrameRateLimit>25</tt:FrameRateLimit>"));
        assert!(xml.contains("<tt:BitrateLimit>4096</tt:BitrateLimit>"));
        assert!(xml.contains("<tt:GovLength>50</tt:GovLength>"));
        assert!(xml.contains("<tt:Profile>Main</tt:Profile>"));
        // No EncodingInterval (Media2 only has FrameRateLimit + BitrateLimit)
        assert!(!xml.contains("EncodingInterval"));
    }

    // ── VideoEncoderConfigurationOptions2 ────────────────────────────────

    const OPTIONS2_XML: &str = r#"<GetVideoEncoderConfigurationOptionsResponse>
          <Options>
            <Encoding>H264</Encoding>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <ResolutionsAvailable><Width>1920</Width><Height>1080</Height></ResolutionsAvailable>
            <ResolutionsAvailable><Width>1280</Width><Height>720</Height></ResolutionsAvailable>
            <BitrateRange><Min>32</Min><Max>16384</Max></BitrateRange>
            <GovLengthRange><Min>1</Min><Max>150</Max></GovLengthRange>
            <ProfilesSupported>Baseline</ProfilesSupported>
            <ProfilesSupported>Main</ProfilesSupported>
            <ProfilesSupported>High</ProfilesSupported>
          </Options>
          <Options>
            <Encoding>H265</Encoding>
            <QualityRange><Min>1</Min><Max>10</Max></QualityRange>
            <ResolutionsAvailable><Width>3840</Width><Height>2160</Height></ResolutionsAvailable>
            <BitrateRange><Min>64</Min><Max>32768</Max></BitrateRange>
            <GovLengthRange><Min>1</Min><Max>200</Max></GovLengthRange>
            <ProfilesSupported>Main</ProfilesSupported>
            <ProfilesSupported>Main10</ProfilesSupported>
          </Options>
        </GetVideoEncoderConfigurationOptionsResponse>"#;

    #[test]
    fn test_video_encoder_configuration_options2_from_xml() {
        let opts = VideoEncoderConfigurationOptions2::from_xml(&parse(OPTIONS2_XML)).unwrap();
        assert_eq!(opts.options.len(), 2);

        let h264 = &opts.options[0];
        assert_eq!(h264.encoding, VideoEncoding::H264);
        let qr = h264.quality_range.unwrap();
        assert!((qr.min - 1.0).abs() < 1e-5);
        assert!((qr.max - 10.0).abs() < 1e-5);
        assert_eq!(h264.resolutions.len(), 2);
        assert_eq!(h264.profiles.len(), 3);
        assert_eq!(h264.profiles[1], "Main");
        let br = h264.bitrate_range.unwrap();
        assert_eq!(br.max, 16384);

        let h265 = &opts.options[1];
        assert_eq!(h265.encoding, VideoEncoding::H265);
        assert_eq!(h265.resolutions.len(), 1);
        assert_eq!(
            h265.resolutions[0],
            Resolution {
                width: 3840,
                height: 2160
            }
        );
        assert_eq!(h265.profiles.len(), 2);
        assert_eq!(h265.profiles[0], "Main");
        let glr = h265.gov_length_range.unwrap();
        assert_eq!(glr.max, 200);
    }

    // ── VideoEncoderInstances ─────────────────────────────────────────────

    const INSTANCES_XML: &str = r#"<GetVideoEncoderInstancesResponse>
          <Info>
            <Total>4</Total>
            <Encoding>
              <Encoding>H264</Encoding>
              <Number>2</Number>
            </Encoding>
            <Encoding>
              <Encoding>H265</Encoding>
              <Number>2</Number>
            </Encoding>
          </Info>
        </GetVideoEncoderInstancesResponse>"#;

    #[test]
    fn test_video_encoder_instances_from_xml() {
        let inst = VideoEncoderInstances::from_xml(&parse(INSTANCES_XML)).unwrap();
        assert_eq!(inst.total, 4);
        assert_eq!(inst.encodings.len(), 2);
        assert_eq!(inst.encodings[0].encoding, VideoEncoding::H264);
        assert_eq!(inst.encodings[0].number, 2);
        assert_eq!(inst.encodings[1].encoding, VideoEncoding::H265);
        assert_eq!(inst.encodings[1].number, 2);
    }
}
