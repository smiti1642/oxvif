// ── Media Service ─────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    AudioEncoderConfiguration, AudioEncoderConfigurationOptions, AudioSource,
    AudioSourceConfiguration, MediaProfile, OsdConfiguration, OsdOptions, SnapshotUri, StreamUri,
    VideoEncoderConfiguration, VideoEncoderConfigurationOptions, VideoSource,
    VideoSourceConfiguration, VideoSourceConfigurationOptions, xml_escape,
};

impl OnvifClient {
    /// List all media profiles available on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    /// Each returned [`MediaProfile`] contains a `token` that can be passed to
    /// [`get_stream_uri`](Self::get_stream_uri).
    pub async fn get_profiles(&self, media_url: &str) -> Result<Vec<MediaProfile>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
        const BODY: &str = "<trt:GetProfiles/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetProfilesResponse")?;
        MediaProfile::vec_from_xml(resp)
    }

    /// Retrieve an RTSP stream URI for the given media profile.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities);
    /// `profile_token` comes from a [`MediaProfile`] returned by
    /// [`get_profiles`](Self::get_profiles).
    pub async fn get_stream_uri(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<StreamUri, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetStreamUri";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:GetStreamUri>\
               <trt:StreamSetup>\
                 <tt:Stream>RTP-Unicast</tt:Stream>\
                 <tt:Transport><tt:Protocol>RTSP</tt:Protocol></tt:Transport>\
               </trt:StreamSetup>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetStreamUri>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStreamUriResponse")?;
        StreamUri::from_xml(resp)
    }

    /// Retrieve an HTTP snapshot URI for the given media profile.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities);
    /// `profile_token` comes from a [`MediaProfile`] returned by
    /// [`get_profiles`](Self::get_profiles).
    pub async fn get_snapshot_uri(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<SnapshotUri, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:GetSnapshotUri>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetSnapshotUri>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSnapshotUriResponse")?;
        SnapshotUri::from_xml(resp)
    }

    /// Create a new, initially empty media profile.
    ///
    /// `token` is optional; if omitted the device assigns one. Returns the
    /// newly created [`MediaProfile`] including the assigned token.
    pub async fn create_profile(
        &self,
        media_url: &str,
        name: &str,
        token: Option<&str>,
    ) -> Result<MediaProfile, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/CreateProfile";
        let token_el = token
            .map(|t| format!("<trt:Token>{}</trt:Token>", xml_escape(t)))
            .unwrap_or_default();
        let body = format!(
            "<trt:CreateProfile>\
               <trt:Name>{}</trt:Name>\
               {token_el}\
             </trt:CreateProfile>",
            xml_escape(name)
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateProfileResponse")?;
        let p = resp
            .child("Profile")
            .ok_or_else(|| crate::soap::SoapError::missing("Profile"))?;
        MediaProfile::from_xml(p)
    }

    /// Delete a non-fixed media profile.
    ///
    /// Fixed profiles (where `profile.fixed == true`) cannot be deleted.
    pub async fn delete_profile(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/DeleteProfile";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:DeleteProfile>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:DeleteProfile>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteProfileResponse")?;
        Ok(())
    }

    /// Retrieve a single media profile by token.
    pub async fn get_profile(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<MediaProfile, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfile";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:GetProfile>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:GetProfile>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetProfileResponse")?;
        let p = resp
            .child("Profile")
            .ok_or_else(|| crate::soap::SoapError::missing("Profile"))?;
        MediaProfile::from_xml(p)
    }

    /// Bind a video encoder configuration to a media profile.
    ///
    /// `config_token` comes from a [`VideoEncoderConfiguration`] returned by
    /// [`get_video_encoder_configurations`](Self::get_video_encoder_configurations).
    pub async fn add_video_encoder_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration";
        let profile_token = xml_escape(profile_token);
        let config_token = xml_escape(config_token);
        let body = format!(
            "<trt:AddVideoEncoderConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
               <trt:ConfigurationToken>{config_token}</trt:ConfigurationToken>\
             </trt:AddVideoEncoderConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AddVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Remove the video encoder configuration from a media profile.
    pub async fn remove_video_encoder_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:RemoveVideoEncoderConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:RemoveVideoEncoderConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemoveVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Bind a video source configuration to a media profile.
    ///
    /// `config_token` comes from a [`VideoSourceConfiguration`] returned by
    /// [`get_video_source_configurations`](Self::get_video_source_configurations).
    pub async fn add_video_source_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration";
        let profile_token = xml_escape(profile_token);
        let config_token = xml_escape(config_token);
        let body = format!(
            "<trt:AddVideoSourceConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
               <trt:ConfigurationToken>{config_token}</trt:ConfigurationToken>\
             </trt:AddVideoSourceConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AddVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Remove the video source configuration from a media profile.
    pub async fn remove_video_source_configuration(
        &self,
        media_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<trt:RemoveVideoSourceConfiguration>\
               <trt:ProfileToken>{profile_token}</trt:ProfileToken>\
             </trt:RemoveVideoSourceConfiguration>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemoveVideoSourceConfigurationResponse")?;
        Ok(())
    }

    // ── Video Source Service ──────────────────────────────────────────────────

    /// List all physical video sources available on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_sources(&self, media_url: &str) -> Result<Vec<VideoSource>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSources";
        const BODY: &str = "<trt:GetVideoSources/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourcesResponse")?;
        VideoSource::vec_from_xml(resp)
    }

    /// List all video source configurations on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_source_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations";
        const BODY: &str = "<trt:GetVideoSourceConfigurations/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationsResponse")?;
        VideoSourceConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single video source configuration by token.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_source_configuration(
        &self,
        media_url: &str,
        token: &str,
    ) -> Result<VideoSourceConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration";
        let token = xml_escape(token);
        let body = format!(
            "<trt:GetVideoSourceConfiguration>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoSourceConfiguration>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationResponse")?;
        let node = resp
            .child("Configuration")
            .ok_or_else(|| crate::soap::SoapError::missing("Configuration"))?;
        VideoSourceConfiguration::from_xml(node)
    }

    /// Apply a modified video source configuration to the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn set_video_source_configuration(
        &self,
        media_url: &str,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration";
        let body = format!(
            "<trt:SetVideoSourceConfiguration>\
               {cfg}\
               <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetVideoSourceConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video source configuration.
    ///
    /// Pass `config_token` to narrow the options to a specific configuration,
    /// or `None` to retrieve options valid for all configurations.
    pub async fn get_video_source_configuration_options(
        &self,
        media_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!(
                "<trt:ConfigurationToken>{}</trt:ConfigurationToken>",
                xml_escape(tok)
            ),
            None => String::new(),
        };
        let body = format!(
            "<trt:GetVideoSourceConfigurationOptions>{inner}</trt:GetVideoSourceConfigurationOptions>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationOptionsResponse")?;
        VideoSourceConfigurationOptions::from_xml(resp)
    }

    // ── Video Encoder Service ─────────────────────────────────────────────────

    /// List all video encoder configurations on the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_encoder_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<VideoEncoderConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations";
        const BODY: &str = "<trt:GetVideoEncoderConfigurations/>";

        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        VideoEncoderConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single video encoder configuration by token.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_video_encoder_configuration(
        &self,
        media_url: &str,
        token: &str,
    ) -> Result<VideoEncoderConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration";
        let token = xml_escape(token);
        let body = format!(
            "<trt:GetVideoEncoderConfiguration>\
               <trt:ConfigurationToken>{token}</trt:ConfigurationToken>\
             </trt:GetVideoEncoderConfiguration>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationResponse")?;
        let node = resp
            .child("Configuration")
            .ok_or_else(|| crate::soap::SoapError::missing("Configuration"))?;
        VideoEncoderConfiguration::from_xml(node)
    }

    /// Apply a modified video encoder configuration to the device.
    ///
    /// `media_url` is obtained from [`get_capabilities`](Self::get_capabilities).
    pub async fn set_video_encoder_configuration(
        &self,
        media_url: &str,
        config: &VideoEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration";
        let body = format!(
            "<trt:SetVideoEncoderConfiguration>\
               {cfg}\
               <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetVideoEncoderConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video encoder configuration.
    ///
    /// Pass `config_token` to narrow the options to a specific configuration,
    /// or `None` to retrieve options valid for all configurations.
    pub async fn get_video_encoder_configuration_options(
        &self,
        media_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!(
                "<trt:ConfigurationToken>{}</trt:ConfigurationToken>",
                xml_escape(tok)
            ),
            None => String::new(),
        };
        let body = format!(
            "<trt:GetVideoEncoderConfigurationOptions>{inner}</trt:GetVideoEncoderConfigurationOptions>"
        );

        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationOptionsResponse")?;
        VideoEncoderConfigurationOptions::from_xml(resp)
    }

    // ── OSD Service ───────────────────────────────────────────────────────────────

    /// List all OSD elements attached to a video source configuration.
    ///
    /// Pass `None` for `config_token` to list all OSDs on the device.
    /// ONVIF Media WSDL §5.14: GetOSDs accepts an optional
    /// `ConfigurationToken` (video source configuration reference) to filter
    /// results.  This is distinct from `GetOSD` which takes an `OSDToken`.
    pub async fn get_osds(
        &self,
        media_url: &str,
        config_token: Option<&str>,
    ) -> Result<Vec<OsdConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetOSDs";
        let inner = config_token
            .map(|t| {
                format!(
                    "<trt:ConfigurationToken>{}</trt:ConfigurationToken>",
                    xml_escape(t)
                )
            })
            .unwrap_or_default();
        let body = format!("<trt:GetOSDs>{inner}</trt:GetOSDs>");
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetOSDsResponse")?;
        OsdConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single OSD element by token.
    pub async fn get_osd(
        &self,
        media_url: &str,
        osd_token: &str,
    ) -> Result<OsdConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetOSD";
        let osd_token = xml_escape(osd_token);
        let body = format!("<trt:GetOSD><trt:OSDToken>{osd_token}</trt:OSDToken></trt:GetOSD>");
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetOSDResponse")?;
        resp.child("OSDConfiguration")
            .ok_or_else(|| crate::soap::SoapError::missing("OSDConfiguration").into())
            .and_then(OsdConfiguration::from_xml)
    }

    /// Update an existing OSD element.
    pub async fn set_osd(&self, media_url: &str, osd: &OsdConfiguration) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetOSD";
        let body = format!("<trt:SetOSD>{}</trt:SetOSD>", osd.to_xml_body());
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetOSDResponse")?;
        Ok(())
    }

    /// Create a new OSD element and return the assigned token.
    ///
    /// Set `osd.token` to an empty string; the device assigns the token.
    pub async fn create_osd(
        &self,
        media_url: &str,
        osd: &OsdConfiguration,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/CreateOSD";
        let body = format!("<trt:CreateOSD>{}</trt:CreateOSD>", osd.to_xml_body());
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateOSDResponse")?;
        resp.child("OSDToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("OSDToken").into())
    }

    /// Delete an OSD element.
    pub async fn delete_osd(&self, media_url: &str, osd_token: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/DeleteOSD";
        let osd_token = xml_escape(osd_token);
        let body =
            format!("<trt:DeleteOSD><trt:OSDToken>{osd_token}</trt:OSDToken></trt:DeleteOSD>");
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteOSDResponse")?;
        Ok(())
    }

    /// Retrieve the valid OSD configuration options for a video source configuration.
    pub async fn get_osd_options(
        &self,
        media_url: &str,
        config_token: &str,
    ) -> Result<OsdOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetOSDOptions";
        let config_token = xml_escape(config_token);
        let body = format!(
            "<trt:GetOSDOptions>\
               <trt:ConfigurationToken>{config_token}</trt:ConfigurationToken>\
             </trt:GetOSDOptions>"
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetOSDOptionsResponse")?;
        OsdOptions::from_xml(resp)
    }

    // ── Audio Service ─────────────────────────────────────────────────────────

    /// List all physical audio inputs on the device.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_audio_sources(&self, media_url: &str) -> Result<Vec<AudioSource>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetAudioSources";
        const BODY: &str = "<trt:GetAudioSources/>";
        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioSourcesResponse")?;
        AudioSource::vec_from_xml(resp)
    }

    /// List all audio source configurations.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_audio_source_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<AudioSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetAudioSourceConfigurations";
        const BODY: &str = "<trt:GetAudioSourceConfigurations/>";
        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioSourceConfigurationsResponse")?;
        AudioSourceConfiguration::vec_from_xml(resp)
    }

    /// List all audio encoder configurations.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_audio_encoder_configurations(
        &self,
        media_url: &str,
    ) -> Result<Vec<AudioEncoderConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations";
        const BODY: &str = "<trt:GetAudioEncoderConfigurations/>";
        let xml = self.call(media_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioEncoderConfigurationsResponse")?;
        AudioEncoderConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single audio encoder configuration by token.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_audio_encoder_configuration(
        &self,
        media_url: &str,
        config_token: &str,
    ) -> Result<AudioEncoderConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration";
        let body = format!(
            "<trt:GetAudioEncoderConfiguration>\
               <trt:ConfigurationToken>{}</trt:ConfigurationToken>\
             </trt:GetAudioEncoderConfiguration>",
            xml_escape(config_token)
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioEncoderConfigurationResponse")?;
        let node = resp
            .child("Configuration")
            .ok_or_else(|| crate::soap::SoapError::missing("Configuration"))?;
        AudioEncoderConfiguration::from_xml(node)
    }

    /// Write an audio encoder configuration back to the device.
    ///
    /// Obtain the current config via
    /// [`get_audio_encoder_configuration`](Self::get_audio_encoder_configuration),
    /// modify the fields you want to change, then call this method.
    pub async fn set_audio_encoder_configuration(
        &self,
        media_url: &str,
        config: &AudioEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/media/wsdl/SetAudioEncoderConfiguration";
        let body = format!(
            "<trt:SetAudioEncoderConfiguration>\
               {}\
               <trt:ForcePersistence>true</trt:ForcePersistence>\
             </trt:SetAudioEncoderConfiguration>",
            config.to_xml_body()
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetAudioEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve valid parameter ranges for an audio encoder configuration.
    ///
    /// `media_url` comes from [`get_capabilities`](Self::get_capabilities).
    pub async fn get_audio_encoder_configuration_options(
        &self,
        media_url: &str,
        config_token: &str,
    ) -> Result<AudioEncoderConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurationOptions";
        let body = format!(
            "<trt:GetAudioEncoderConfigurationOptions>\
               <trt:ConfigurationToken>{}</trt:ConfigurationToken>\
             </trt:GetAudioEncoderConfigurationOptions>",
            xml_escape(config_token)
        );
        let xml = self.call(media_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioEncoderConfigurationOptionsResponse")?;
        AudioEncoderConfigurationOptions::from_xml(resp)
    }
}
