use crate::helpers::soap;

pub fn resp_profiles() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetProfilesResponse>
          <trt:Profiles token="Profile_1" fixed="true">
            <tt:Name>mainStream</tt:Name>
            <tt:VideoSourceConfiguration token="VSC_1">
              <tt:Name>VSConfig1</tt:Name>
              <tt:UseCount>2</tt:UseCount>
              <tt:SourceToken>VS_1</tt:SourceToken>
              <tt:Bounds x="0" y="0" width="1920" height="1080"/>
            </tt:VideoSourceConfiguration>
            <tt:VideoEncoderConfiguration token="VEC_1">
              <tt:Name>H264</tt:Name>
              <tt:UseCount>1</tt:UseCount>
              <tt:Encoding>H264</tt:Encoding>
              <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
              <tt:RateControl><tt:FrameRateLimit>30</tt:FrameRateLimit><tt:BitrateLimit>4096</tt:BitrateLimit></tt:RateControl>
            </tt:VideoEncoderConfiguration>
          </trt:Profiles>
          <trt:Profiles token="Profile_2" fixed="false">
            <tt:Name>subStream</tt:Name>
            <tt:VideoSourceConfiguration token="VSC_1">
              <tt:Name>VSConfig1</tt:Name>
              <tt:UseCount>2</tt:UseCount>
              <tt:SourceToken>VS_1</tt:SourceToken>
              <tt:Bounds x="0" y="0" width="1920" height="1080"/>
            </tt:VideoSourceConfiguration>
            <tt:VideoEncoderConfiguration token="VEC_2">
              <tt:Name>H264_sub</tt:Name>
              <tt:UseCount>1</tt:UseCount>
              <tt:Encoding>H264</tt:Encoding>
              <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
              <tt:RateControl><tt:FrameRateLimit>15</tt:FrameRateLimit><tt:BitrateLimit>1024</tt:BitrateLimit></tt:RateControl>
            </tt:VideoEncoderConfiguration>
          </trt:Profiles>
        </trt:GetProfilesResponse>"#,
    )
}

pub fn resp_profile() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetProfileResponse>
          <trt:Profile token="Profile_1" fixed="true">
            <tt:Name>mainStream</tt:Name>
            <tt:VideoSourceConfiguration token="VSC_1">
              <tt:Name>VSConfig1</tt:Name>
              <tt:UseCount>2</tt:UseCount>
              <tt:SourceToken>VS_1</tt:SourceToken>
              <tt:Bounds x="0" y="0" width="1920" height="1080"/>
            </tt:VideoSourceConfiguration>
          </trt:Profile>
        </trt:GetProfileResponse>"#,
    )
}

pub fn resp_stream_uri() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetStreamUriResponse>
          <trt:MediaUri>
            <tt:Uri>rtsp://127.0.0.1:554/mock/stream</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>"#,
    )
}

pub fn resp_snapshot_uri(base: &str) -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetSnapshotUriResponse>
          <trt:MediaUri>
            <tt:Uri>{base}/mock/snapshot.jpg</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetSnapshotUriResponse>"#
        ),
    )
}

pub fn resp_create_profile() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:CreateProfileResponse>
          <trt:Profile token="Profile_New" fixed="false">
            <tt:Name>oxvif-test-profile</tt:Name>
          </trt:Profile>
        </trt:CreateProfileResponse>"#,
    )
}

pub fn resp_video_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourcesResponse>
          <trt:VideoSources token="VS_1">
            <tt:Framerate>25</tt:Framerate>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
          </trt:VideoSources>
        </trt:GetVideoSourcesResponse>"#,
    )
}

pub fn resp_video_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationsResponse>
          <trt:Configurations token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </trt:Configurations>
        </trt:GetVideoSourceConfigurationsResponse>"#,
    )
}

pub fn resp_video_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationsResponse>
          <trt:Configurations token="VEC_1">
            <tt:Name>MainStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
            <tt:Quality>5</tt:Quality>
            <tt:RateControl>
              <tt:FrameRateLimit>25</tt:FrameRateLimit>
              <tt:EncodingInterval>1</tt:EncodingInterval>
              <tt:BitrateLimit>4096</tt:BitrateLimit>
            </tt:RateControl>
            <tt:H264>
              <tt:GovLength>25</tt:GovLength>
              <tt:H264Profile>Main</tt:H264Profile>
            </tt:H264>
          </trt:Configurations>
          <trt:Configurations token="VEC_2">
            <tt:Name>SubStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>JPEG</tt:Encoding>
            <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
            <tt:Quality>3</tt:Quality>
          </trt:Configurations>
        </trt:GetVideoEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_audio_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourcesResponse>
          <trt:AudioSources token="AudioSource_1">
            <tt:Channels>1</tt:Channels>
          </trt:AudioSources>
        </trt:GetAudioSourcesResponse>"#,
    )
}

pub fn resp_audio_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationsResponse>
          <trt:Configurations token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configurations>
        </trt:GetAudioEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_osds() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetOSDsResponse>
          <trt:OSDs token="OSD_1">
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>DateAndTime</tt:Type>
            </tt:TextString>
          </trt:OSDs>
        </trt:GetOSDsResponse>"#,
    )
}

pub fn resp_video_source_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationResponse>
          <trt:Configuration token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </trt:Configuration>
        </trt:GetVideoSourceConfigurationResponse>"#,
    )
}

pub fn resp_video_source_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationOptionsResponse>
          <trt:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>1920</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>1080</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>VS_1</tt:VideoSourceTokensAvailable>
          </trt:Options>
        </trt:GetVideoSourceConfigurationOptionsResponse>"#,
    )
}

pub fn resp_video_encoder_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationResponse>
          <trt:Configuration token="VEC_1">
            <tt:Name>MainStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
            <tt:Quality>5</tt:Quality>
            <tt:RateControl>
              <tt:FrameRateLimit>25</tt:FrameRateLimit>
              <tt:EncodingInterval>1</tt:EncodingInterval>
              <tt:BitrateLimit>4096</tt:BitrateLimit>
            </tt:RateControl>
            <tt:H264>
              <tt:GovLength>25</tt:GovLength>
              <tt:H264Profile>Main</tt:H264Profile>
            </tt:H264>
          </trt:Configuration>
        </trt:GetVideoEncoderConfigurationResponse>"#,
    )
}

pub fn resp_video_encoder_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            <tt:H264>
              <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
              <tt:ResolutionsAvailable><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:ResolutionsAvailable>
              <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
              <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
              <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:EncodingIntervalRange>
              <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
              <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
            </tt:H264>
          </trt:Options>
        </trt:GetVideoEncoderConfigurationOptionsResponse>"#,
    )
}

pub fn resp_osd() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetOSDResponse>
          <trt:OSDConfiguration token="OSD_1">
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>DateAndTime</tt:Type>
            </tt:TextString>
          </trt:OSDConfiguration>
        </trt:GetOSDResponse>"#,
    )
}

pub fn resp_create_osd() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:CreateOSDResponse>
          <trt:OSDToken>OSD_2</trt:OSDToken>
        </trt:CreateOSDResponse>"#,
    )
}

pub fn resp_osd_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetOSDOptionsResponse>
          <trt:OSDOptions>
            <tt:MaximumNumberOfOSDs>8</tt:MaximumNumberOfOSDs>
            <tt:Type>Text</tt:Type>
            <tt:Type>Image</tt:Type>
            <tt:PositionOption>
              <tt:Type>UpperLeft</tt:Type>
              <tt:Type>LowerRight</tt:Type>
              <tt:Type>Custom</tt:Type>
            </tt:PositionOption>
            <tt:TextOption>
              <tt:Type>Plain</tt:Type>
              <tt:Type>Date</tt:Type>
              <tt:Type>DateAndTime</tt:Type>
            </tt:TextOption>
          </trt:OSDOptions>
        </trt:GetOSDOptionsResponse>"#,
    )
}

pub fn resp_audio_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourceConfigurationsResponse>
          <trt:Configurations token="ASC_1">
            <tt:Name>AudioSourceConfig1</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:SourceToken>AudioSource_1</tt:SourceToken>
          </trt:Configurations>
        </trt:GetAudioSourceConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationResponse>
          <trt:Configuration token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configuration>
        </trt:GetAudioEncoderConfigurationResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:Encoding>G711</tt:Encoding>
            <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
          </trt:Options>
          <trt:Options>
            <tt:Encoding>AAC</tt:Encoding>
            <tt:BitrateList><tt:Items>64 128 256</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>16 32 44</tt:Items></tt:SampleRateList>
          </trt:Options>
        </trt:GetAudioEncoderConfigurationOptionsResponse>"#,
    )
}
