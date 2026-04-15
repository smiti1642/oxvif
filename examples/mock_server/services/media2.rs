use crate::helpers::soap;

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

pub fn resp_video_source_configurations_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoSourceConfigurationsResponse>
          <tr2:Configurations token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </tr2:Configurations>
        </tr2:GetVideoSourceConfigurationsResponse>"#,
    )
}

pub fn resp_video_source_configuration_options_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoSourceConfigurationOptionsResponse>
          <tr2:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>1920</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>1080</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>VS_1</tt:VideoSourceTokensAvailable>
          </tr2:Options>
        </tr2:GetVideoSourceConfigurationOptionsResponse>"#,
    )
}

pub fn resp_video_encoder_configuration_options_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetVideoEncoderConfigurationOptionsResponse>
          <tr2:Options>
            <tt:Encoding>H264</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
            <tt:ResolutionsAvailable><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:ResolutionsAvailable>
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Baseline</tt:ProfilesSupported>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
            <tt:ProfilesSupported>High</tt:ProfilesSupported>
          </tr2:Options>
          <tr2:Options>
            <tt:Encoding>H265</tt:Encoding>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            <tt:ResolutionsAvailable><tt:Width>3840</tt:Width><tt:Height>2160</tt:Height></tt:ResolutionsAvailable>
            <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
            <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>32768</tt:Max></tt:BitrateRange>
            <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>60</tt:Max></tt:FrameRateRange>
            <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>600</tt:Max></tt:GovLengthRange>
            <tt:ProfilesSupported>Main</tt:ProfilesSupported>
          </tr2:Options>
        </tr2:GetVideoEncoderConfigurationOptionsResponse>"#,
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
