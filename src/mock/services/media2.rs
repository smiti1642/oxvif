use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{SharedState, VideoEncoderState};
use crate::mock::xml_parse::{extract_attr, extract_tag};

const NS: &str = r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#;

/// The `ConfigurationToken` of a per-channel Media2 request, or a SOAP Fault.
/// Same reasoning as `media::require_config_token` — see that doc comment.
fn require_config_token(body: &str, missing_reason: &str) -> Result<String, String> {
    extract_tag(body, "ConfigurationToken")
        .filter(|t| !t.is_empty())
        .ok_or_else(|| resp_soap_fault("env:Sender", missing_reason))
}

pub fn resp_profiles_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetProfilesResponse>
          <tr2:Profiles token="Profile_A" fixed="true">
            <tt:Name>mainStream</tt:Name>
            <tr2:Configurations>
              <tr2:VideoSource token="VSC_1"/>
              <tr2:VideoEncoder token="VEC_1"/>
            </tr2:Configurations>
          </tr2:Profiles>
          <tr2:Profiles token="Profile_B" fixed="false">
            <tt:Name>subStream</tt:Name>
          </tr2:Profiles>
          <tr2:Profiles token="Profile_C" fixed="true">
            <tt:Name>mainStream2</tt:Name>
            <tr2:Configurations>
              <tr2:VideoSource token="VSC_2"/>
              <tr2:VideoEncoder token="VEC_3"/>
            </tr2:Configurations>
          </tr2:Profiles>
          <tr2:Profiles token="Profile_D" fixed="false">
            <tt:Name>subStream2</tt:Name>
            <tr2:Configurations>
              <tr2:VideoSource token="VSC_2"/>
              <tr2:VideoEncoder token="VEC_4"/>
            </tr2:Configurations>
          </tr2:Profiles>
        </tr2:GetProfilesResponse>"#,
    )
}

pub fn resp_stream_uri_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetStreamUriResponse>
          <tr2:Uri>rtsp://127.0.0.1:554/mock/h265</tr2:Uri>
        </tr2:GetStreamUriResponse>"#,
    )
}

pub fn resp_snapshot_uri_media2(base: &str) -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        &format!(
            r#"<tr2:GetSnapshotUriResponse>
          <tr2:Uri>{base}/mock/snapshot.jpg</tr2:Uri>
        </tr2:GetSnapshotUriResponse>"#
        ),
    )
}

pub fn resp_video_source_configurations_media2(state: &SharedState) -> String {
    let vscs = state.read().video_source_configs.clone();
    let items: String = vscs
        .iter()
        .map(|c| {
            format!(
                r#"<tr2:Configurations token="{token}">
            <tt:Name>{name}</tt:Name>
            <tt:UseCount>{use_count}</tt:UseCount>
            <tt:SourceToken>{source}</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="{width}" height="{height}"/>
          </tr2:Configurations>"#,
                token = c.token,
                name = c.name,
                use_count = c.use_count,
                source = c.source_token,
                width = c.width,
                height = c.height,
            )
        })
        .collect();
    soap(
        NS,
        &format!(
            "<tr2:GetVideoSourceConfigurationsResponse>{items}</tr2:GetVideoSourceConfigurationsResponse>"
        ),
    )
}

/// Per-channel — the bounds ceiling is the addressed sensor's own resolution.
pub fn resp_video_source_configuration_options_media2(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VSCOPT2-5511") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vscs = state.read().video_source_configs.clone();
    let Some(c) = vscs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VSCOPT2-5512: {want}"));
    };
    soap(
        NS,
        &format!(
            r#"<tr2:GetVideoSourceConfigurationOptionsResponse>
          <tr2:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>{width}</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>{height}</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>{source}</tt:VideoSourceTokensAvailable>
          </tr2:Options>
        </tr2:GetVideoSourceConfigurationOptionsResponse>"#,
            width = c.width,
            height = c.height,
            source = c.source_token,
        ),
    )
}

/// Per-channel. Media2 returns one `Options` block **per encoding**, so the
/// H.265 block is offered only where the sensor can actually do it — sensor 1.
/// Sensor 2 gets H.264 alone, at its own smaller resolution list.
pub fn resp_video_encoder_configuration_options_media2(state: &SharedState, body: &str) -> String {
    let want = match require_config_token(body, "NoConfigToken-VECOPT2-5513") {
        Ok(t) => t,
        Err(fault) => return fault,
    };
    let vecs = state.read().video_encoders.clone();
    let Some(c) = vecs.iter().find(|c| c.token == want) else {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-VECOPT2-5514: {want}"));
    };

    let resolutions: String = c
        .resolutions
        .iter()
        .map(|(w, h)| {
            format!(
                "<tt:ResolutionsAvailable><tt:Width>{w}</tt:Width>\
                 <tt:Height>{h}</tt:Height></tt:ResolutionsAvailable>"
            )
        })
        .collect();

    // Only the 5MP sensor advertises H.265. Nothing else in the mock lets a
    // test tell "this device supports H265" from "this *channel* supports it".
    let h265 = if c.source_token == "VS_1" {
        format!(
            r#"<tr2:Options>
            <tt:Encoding>H265</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            {resolutions}
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>32768</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>60</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>600</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
          </tr2:Options>"#
        )
    } else {
        String::new()
    };

    soap(
        NS,
        &format!(
            r#"<tr2:GetVideoEncoderConfigurationOptionsResponse>
          <tr2:Options>
            <tt:Encoding>H264</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            {resolutions}
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Baseline</tt:ProfilesSupported>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
            <tt:ProfilesSupported>High</tt:ProfilesSupported>
          </tr2:Options>
          {h265}
        </tr2:GetVideoEncoderConfigurationOptionsResponse>"#
        ),
    )
}

/// `GetVideoEncoderConfigurations` (Media2) — renders the encoder config from
/// state. If the request carries a `ConfigurationToken`, only the matching
/// config is returned (empty list otherwise), mirroring ONVIF token filtering.
/// Pairs with [`handle_set_video_encoder_configuration`] for Set → Get roundtrips.
pub fn resp_video_encoder_configurations(state: &SharedState, body: &str) -> String {
    let vecs = state.read().video_encoders.clone();
    let want = extract_tag(body, "ConfigurationToken").filter(|t| !t.is_empty());
    let items: String = vecs
        .iter()
        .filter(|c| want.as_deref().is_none_or(|t| t == c.token))
        .map(render_video_encoder)
        .collect();
    soap(
        NS,
        &format!(
            "<tr2:GetVideoEncoderConfigurationsResponse>{items}</tr2:GetVideoEncoderConfigurationsResponse>"
        ),
    )
}

/// `SetVideoEncoderConfiguration` (Media2) — persists the posted fields into
/// state so a following `GetVideoEncoderConfigurations` reflects them. Only the
/// fields present in the request body are updated.
pub fn handle_set_video_encoder_configuration(state: &SharedState, body: &str) -> String {
    // The token now *selects* which of the four channels to write, rather than
    // renaming the single global config it used to be. An absent or unknown
    // token is a fault: with more than one encoder, writing to a guessed
    // channel is the same silent-wrong-answer failure the getters avoid.
    let Some(want) = extract_attr(body, "Configuration", "token").filter(|t| !t.is_empty()) else {
        return resp_soap_fault("env:Sender", "NoConfigToken-SETVEC2-5515");
    };
    if !state.read().video_encoders.iter().any(|c| c.token == want) {
        return resp_soap_fault("env:Sender", &format!("NoSuchConfig-SETVEC2-5516: {want}"));
    }
    state.modify(|s| {
        let Some(ve) = s.video_encoders.iter_mut().find(|c| c.token == want) else {
            return;
        };
        if let Some(v) = extract_tag(body, "Name") {
            ve.name = v;
        }
        if let Some(v) = extract_tag(body, "Encoding") {
            ve.encoding = v;
        }
        if let Some(v) = extract_tag(body, "Width").and_then(|x| x.parse().ok()) {
            ve.width = v;
        }
        if let Some(v) = extract_tag(body, "Height").and_then(|x| x.parse().ok()) {
            ve.height = v;
        }
        if let Some(v) = extract_tag(body, "Quality").and_then(|x| x.parse().ok()) {
            ve.quality = v;
        }
        if let Some(v) = extract_tag(body, "FrameRateLimit").and_then(|x| x.parse().ok()) {
            ve.frame_rate_limit = v;
        }
        if let Some(v) = extract_tag(body, "BitrateLimit").and_then(|x| x.parse().ok()) {
            ve.bitrate_limit = v;
        }
        if let Some(v) = extract_tag(body, "GovLength").and_then(|x| x.parse().ok()) {
            ve.gov_length = v;
        }
        if let Some(v) = extract_tag(body, "Profile") {
            ve.profile = v;
        }
    });
    resp_empty("tr2", "SetVideoEncoderConfigurationResponse")
}

/// Render one `<tr2:Configurations>` element from encoder state, in the flat
/// Media2 shape `VideoEncoderConfiguration2::from_xml` expects.
fn render_video_encoder(ve: &VideoEncoderState) -> String {
    format!(
        r#"<tr2:Configurations token="{token}">
            <tt:Name>{name}</tt:Name>
            <tt:UseCount>{use_count}</tt:UseCount>
            <tt:Encoding>{encoding}</tt:Encoding>
            <tt:Resolution><tt:Width>{width}</tt:Width><tt:Height>{height}</tt:Height></tt:Resolution>
            <tt:RateControl>
              <tt:FrameRateLimit>{fr}</tt:FrameRateLimit>
              <tt:BitrateLimit>{br}</tt:BitrateLimit>
            </tt:RateControl>
            <tt:GovLength>{gov}</tt:GovLength>
            <tt:Profile>{profile}</tt:Profile>
            <tt:Quality>{quality}</tt:Quality>
          </tr2:Configurations>"#,
        token = ve.token,
        name = ve.name,
        use_count = ve.use_count,
        encoding = ve.encoding,
        width = ve.width,
        height = ve.height,
        fr = ve.frame_rate_limit,
        br = ve.bitrate_limit,
        gov = ve.gov_length,
        profile = ve.profile,
        quality = ve.quality,
    )
}

pub fn resp_video_encoder_instances() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoEncoderInstancesResponse>
          <tr2:Info>
            <tt:Total>4</tt:Total>
            <tt:Encoding>
              <tt:Encoding>H264</tt:Encoding>
              <tt:Number>2</tt:Number>
            </tt:Encoding>
            <tt:Encoding>
              <tt:Encoding>H265</tt:Encoding>
              <tt:Number>2</tt:Number>
            </tt:Encoding>
          </tr2:Info>
        </tr2:GetVideoEncoderInstancesResponse>"#,
    )
}

pub fn resp_create_profile_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:CreateProfileResponse>
          <tr2:Token>Profile_New_M2</tr2:Token>
        </tr2:CreateProfileResponse>"#,
    )
}

pub fn resp_metadata_configurations() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetMetadataConfigurationsResponse>
          <tr2:Configurations token="MetaConf_1">
            <tt:Name>MetadataConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Analytics>true</tt:Analytics>
            <tt:PTZStatus>
              <tt:Status>false</tt:Status>
              <tt:Position>true</tt:Position>
            </tt:PTZStatus>
          </tr2:Configurations>
        </tr2:GetMetadataConfigurationsResponse>"#,
    )
}

pub fn resp_metadata_configuration_options() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetMetadataConfigurationOptionsResponse>
          <tr2:Options>
            <tt:PTZStatusFilterOptions/>
          </tr2:Options>
        </tr2:GetMetadataConfigurationOptionsResponse>"#,
    )
}

pub fn resp_audio_source_configurations_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioSourceConfigurationsResponse>
          <tr2:Configurations token="ASC_1">
            <tt:Name>AudioSourceConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:SourceToken>AudioSrc_1</tt:SourceToken>
          </tr2:Configurations>
        </tr2:GetAudioSourceConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configurations_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioEncoderConfigurationsResponse>
          <tr2:Configurations token="AEC_1">
            <tt:Name>AudioEncoderConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </tr2:Configurations>
        </tr2:GetAudioEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration_options_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioEncoderConfigurationOptionsResponse>
          <tr2:Options>
            <tt:Options>
              <tt:Encoding>G711</tt:Encoding>
              <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
              <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
            </tt:Options>
          </tr2:Options>
        </tr2:GetAudioEncoderConfigurationOptionsResponse>"#,
    )
}

pub fn resp_audio_output_configurations() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioOutputConfigurationsResponse>
          <tr2:Configurations token="AOC_1">
            <tt:Name>AudioOutput</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:OutputToken>AudioOut_1</tt:OutputToken>
            <tt:OutputLevel>50</tt:OutputLevel>
          </tr2:Configurations>
        </tr2:GetAudioOutputConfigurationsResponse>"#,
    )
}

pub fn resp_audio_decoder_configurations() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetAudioDecoderConfigurationsResponse>
          <tr2:Configurations token="ADC_1">
            <tt:Name>AudioDecoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
          </tr2:Configurations>
        </tr2:GetAudioDecoderConfigurationsResponse>"#,
    )
}

pub fn resp_video_source_modes() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoSourceModesResponse>
          <tr2:VideoSourceModes token="Mode_1">
            <tt:MaxFramerate>30</tt:MaxFramerate>
            <tt:MaxResolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:MaxResolution>
            <tt:Encodings>H264 H265</tt:Encodings>
            <tt:Reboot>false</tt:Reboot>
          </tr2:VideoSourceModes>
        </tr2:GetVideoSourceModesResponse>"#,
    )
}

pub fn resp_set_video_source_mode() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:SetVideoSourceModeResponse>
          <tr2:Reboot>false</tr2:Reboot>
        </tr2:SetVideoSourceModeResponse>"#,
    )
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `tr2:Capabilities2` — note the `2` in the type name; ver20 media.wsdl
/// defines no type called `Capabilities`.
///
/// Claims track what this mock dispatches: `GetSnapshotUri`,
/// `GetVideoSourceModes`/`SetVideoSourceMode`, and the `AddConfiguration`
/// kinds. Privacy masks and WebRTC are not implemented, so `Mask` /
/// `SourceMask` are `false` and `WebRTC` is `0` — an **`xs:int` session
/// count**, not a boolean.
pub fn resp_service_capabilities_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetServiceCapabilitiesResponse>
          <tr2:Capabilities SnapshotUri="true"
                            Rotation="false"
                            VideoSourceMode="true"
                            OSD="false"
                            Mask="false"
                            SourceMask="false"
                            WebRTC="0">
            <tr2:ProfileCapabilities MaximumNumberOfProfiles="8"
                                     ConfigurationsSupported="VideoSource VideoEncoder AudioSource AudioEncoder Metadata"/>
            <tr2:StreamingCapabilities RTSPStreaming="true"
                                       RTPMulticast="false"
                                       RTP_RTSP_TCP="true"
                                       NonAggregateControl="false"
                                       AutoStartMulticast="false"/>
          </tr2:Capabilities>
        </tr2:GetServiceCapabilitiesResponse>"#,
    )
}
