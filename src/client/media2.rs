// ── Media2 Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    AudioDecoderConfiguration, AudioEncoderConfiguration, AudioEncoderConfigurationOptions,
    AudioOutputConfiguration, AudioSourceConfiguration, MediaProfile2, MetadataConfiguration,
    MetadataConfigurationOptions, VideoEncoderConfiguration2, VideoEncoderConfigurationOptions2,
    VideoEncoderInstances, VideoSourceConfiguration, VideoSourceConfigurationOptions,
    VideoSourceMode, xml_escape,
};

impl OnvifClient {
    /// List all media profiles from the Media2 service.
    ///
    /// `media2_url` is obtained from `caps.media2.url` via
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn get_profiles_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<MediaProfile2>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";
        const BODY: &str = "<tr2:GetProfiles/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetProfilesResponse")?;
        MediaProfile2::vec_from_xml(resp)
    }

    /// Retrieve an RTSP stream URI via the Media2 service.
    ///
    /// Returns the URI string directly (no `MediaUri` wrapper, unlike Media1).
    pub async fn get_stream_uri_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetStreamUri";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tr2:GetStreamUri>\
               <tr2:Protocol>RTSP</tr2:Protocol>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
             </tr2:GetStreamUri>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStreamUriResponse")?;
        resp.child("Uri")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Uri").into())
    }

    /// Retrieve an HTTP snapshot URI via the Media2 service.
    ///
    /// Returns the URI string directly (no `MediaUri` wrapper, unlike Media1).
    pub async fn get_snapshot_uri_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tr2:GetSnapshotUri>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
             </tr2:GetSnapshotUri>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSnapshotUriResponse")?;
        resp.child("Uri")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Uri").into())
    }

    /// List all video source configurations via the Media2 service.
    pub async fn get_video_source_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<VideoSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations";
        const BODY: &str = "<tr2:GetVideoSourceConfigurations/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationsResponse")?;
        VideoSourceConfiguration::vec_from_xml(resp)
    }

    /// Apply a modified video source configuration via the Media2 service.
    ///
    /// Note: Media2 does not use `ForcePersistence`.
    pub async fn set_video_source_configuration_media2(
        &self,
        media2_url: &str,
        config: &VideoSourceConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration";
        let body = format!(
            "<tr2:SetVideoSourceConfiguration>{cfg}</tr2:SetVideoSourceConfiguration>",
            cfg = config.to_xml_body_media2()
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoSourceConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video source configuration via Media2.
    pub async fn get_video_source_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoSourceConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!(
                "<tr2:ConfigurationToken>{}</tr2:ConfigurationToken>",
                xml_escape(tok)
            ),
            None => String::new(),
        };
        let body = format!(
            "<tr2:GetVideoSourceConfigurationOptions>{inner}</tr2:GetVideoSourceConfigurationOptions>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceConfigurationOptionsResponse")?;
        VideoSourceConfigurationOptions::from_xml(resp)
    }

    /// List all video encoder configurations from the Media2 service.
    ///
    /// Returns [`VideoEncoderConfiguration2`] which uses a flat layout with
    /// native H.265 support.
    pub async fn get_video_encoder_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<VideoEncoderConfiguration2>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
        const BODY: &str = "<tr2:GetVideoEncoderConfigurations/>";

        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        VideoEncoderConfiguration2::vec_from_xml(resp)
    }

    /// Retrieve a single video encoder configuration by token from the Media2 service.
    pub async fn get_video_encoder_configuration_media2(
        &self,
        media2_url: &str,
        token: &str,
    ) -> Result<VideoEncoderConfiguration2, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
        let token = xml_escape(token);
        let body = format!(
            "<tr2:GetVideoEncoderConfigurations>\
               <tr2:ConfigurationToken>{token}</tr2:ConfigurationToken>\
             </tr2:GetVideoEncoderConfigurations>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationsResponse")?;
        let configs = VideoEncoderConfiguration2::vec_from_xml(resp)?;
        configs
            .into_iter()
            .next()
            .ok_or_else(|| crate::soap::SoapError::missing("Configurations").into())
    }

    /// Apply a modified video encoder configuration via the Media2 service.
    ///
    /// Note: Media2 does not use `ForcePersistence`.
    pub async fn set_video_encoder_configuration_media2(
        &self,
        media2_url: &str,
        config: &VideoEncoderConfiguration2,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration";
        let body = format!(
            "<tr2:SetVideoEncoderConfiguration>{cfg}</tr2:SetVideoEncoderConfiguration>",
            cfg = config.to_xml_body()
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetVideoEncoderConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for video encoder configuration via Media2.
    ///
    /// Media2 returns one options entry per supported encoding type.
    pub async fn get_video_encoder_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
    ) -> Result<VideoEncoderConfigurationOptions2, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions";
        let inner = match config_token {
            Some(tok) => format!(
                "<tr2:ConfigurationToken>{}</tr2:ConfigurationToken>",
                xml_escape(tok)
            ),
            None => String::new(),
        };
        let body = format!(
            "<tr2:GetVideoEncoderConfigurationOptions>{inner}</tr2:GetVideoEncoderConfigurationOptions>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderConfigurationOptionsResponse")?;
        VideoEncoderConfigurationOptions2::from_xml(resp)
    }

    /// Retrieve encoder instance capacity info for a video source configuration (Media2).
    pub async fn get_video_encoder_instances_media2(
        &self,
        media2_url: &str,
        config_token: &str,
    ) -> Result<VideoEncoderInstances, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances";
        let config_token = xml_escape(config_token);
        let body = format!(
            "<tr2:GetVideoEncoderInstances>\
               <tr2:ConfigurationToken>{config_token}</tr2:ConfigurationToken>\
             </tr2:GetVideoEncoderInstances>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoEncoderInstancesResponse")?;
        VideoEncoderInstances::from_xml(resp)
    }

    /// Create a new media profile via the Media2 service.
    ///
    /// Returns the token of the newly created profile.
    pub async fn create_profile_media2(
        &self,
        media2_url: &str,
        name: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/CreateProfile";
        let name = xml_escape(name);
        let body = format!(
            "<tr2:CreateProfile>\
               <tr2:Name>{name}</tr2:Name>\
             </tr2:CreateProfile>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateProfileResponse")?;
        resp.child("Token")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Token").into())
    }

    /// Delete a media profile via the Media2 service.
    pub async fn delete_profile_media2(
        &self,
        media2_url: &str,
        token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/DeleteProfile";
        let token = xml_escape(token);
        let body = format!(
            "<tr2:DeleteProfile>\
               <tr2:Token>{token}</tr2:Token>\
             </tr2:DeleteProfile>"
        );

        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteProfileResponse")?;
        Ok(())
    }

    // ── AddConfiguration / RemoveConfiguration ───────────────────────────────

    /// Bind one or more configurations to a media profile (Media2).
    ///
    /// ONVIF Media2 WSDL `AddConfiguration` — Profile T §7.10/§8.1/§8.5/§8.10/§8.13.
    /// `config_type` is one of: `"VideoSource"`, `"VideoEncoder"`,
    /// `"AudioSource"`, `"AudioEncoder"`, `"AudioOutput"`, `"AudioDecoder"`,
    /// `"Metadata"`, `"Analytics"`, `"PTZ"`.
    pub async fn add_configuration_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
        config_type: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/AddConfiguration";
        let profile_token = xml_escape(profile_token);
        let config_type = xml_escape(config_type);
        let config_token = xml_escape(config_token);
        let body = format!(
            "<tr2:AddConfiguration>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
               <tr2:Configuration>\
                 <tr2:Type>{config_type}</tr2:Type>\
                 <tr2:Token>{config_token}</tr2:Token>\
               </tr2:Configuration>\
             </tr2:AddConfiguration>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AddConfigurationResponse")?;
        Ok(())
    }

    /// Remove a configuration from a media profile (Media2).
    ///
    /// ONVIF Media2 WSDL `RemoveConfiguration` — Profile T §7.10/§8.1/§8.5.
    pub async fn remove_configuration_media2(
        &self,
        media2_url: &str,
        profile_token: &str,
        config_type: &str,
        config_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/RemoveConfiguration";
        let profile_token = xml_escape(profile_token);
        let config_type = xml_escape(config_type);
        let config_token = xml_escape(config_token);
        let body = format!(
            "<tr2:RemoveConfiguration>\
               <tr2:ProfileToken>{profile_token}</tr2:ProfileToken>\
               <tr2:Configuration>\
                 <tr2:Type>{config_type}</tr2:Type>\
                 <tr2:Token>{config_token}</tr2:Token>\
               </tr2:Configuration>\
             </tr2:RemoveConfiguration>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemoveConfigurationResponse")?;
        Ok(())
    }

    // ── Metadata configurations ──────────────────────────────────────────────

    /// List metadata configurations via Media2.
    ///
    /// ONVIF Media2 WSDL `GetMetadataConfigurations` — Profile T §7.14/§7.15.
    pub async fn get_metadata_configurations_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
        profile_token: Option<&str>,
    ) -> Result<Vec<MetadataConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations";
        let mut inner = String::new();
        if let Some(tok) = config_token {
            inner.push_str(&format!(
                "<tr2:ConfigurationToken>{}</tr2:ConfigurationToken>",
                xml_escape(tok)
            ));
        }
        if let Some(tok) = profile_token {
            inner.push_str(&format!(
                "<tr2:ProfileToken>{}</tr2:ProfileToken>",
                xml_escape(tok)
            ));
        }
        let body =
            format!("<tr2:GetMetadataConfigurations>{inner}</tr2:GetMetadataConfigurations>");
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetMetadataConfigurationsResponse")?;
        MetadataConfiguration::vec_from_xml(resp)
    }

    /// Apply a metadata configuration via Media2.
    ///
    /// ONVIF Media2 WSDL `SetMetadataConfiguration` — Profile T §7.15.
    pub async fn set_metadata_configuration_media2(
        &self,
        media2_url: &str,
        config: &MetadataConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetMetadataConfiguration";
        let body = format!(
            "<tr2:SetMetadataConfiguration>{}</tr2:SetMetadataConfiguration>",
            config.to_xml_body()
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetMetadataConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve metadata configuration options via Media2.
    ///
    /// ONVIF Media2 WSDL `GetMetadataConfigurationOptions` — Profile T §7.15.
    pub async fn get_metadata_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
        profile_token: Option<&str>,
    ) -> Result<MetadataConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurationOptions";
        let mut inner = String::new();
        if let Some(tok) = config_token {
            inner.push_str(&format!(
                "<tr2:ConfigurationToken>{}</tr2:ConfigurationToken>",
                xml_escape(tok)
            ));
        }
        if let Some(tok) = profile_token {
            inner.push_str(&format!(
                "<tr2:ProfileToken>{}</tr2:ProfileToken>",
                xml_escape(tok)
            ));
        }
        let body = format!(
            "<tr2:GetMetadataConfigurationOptions>{inner}</tr2:GetMetadataConfigurationOptions>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetMetadataConfigurationOptionsResponse")?;
        MetadataConfigurationOptions::from_xml(resp)
    }

    // ── Audio source configurations (Media2) ─────────────────────────────────

    /// List audio source configurations via Media2.
    ///
    /// ONVIF Media2 WSDL `GetAudioSourceConfigurations` — Profile T §8.10.
    pub async fn get_audio_source_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<AudioSourceConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetAudioSourceConfigurations";
        const BODY: &str = "<tr2:GetAudioSourceConfigurations/>";
        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioSourceConfigurationsResponse")?;
        AudioSourceConfiguration::vec_from_xml(resp)
    }

    // ── Audio encoder configurations (Media2) ────────────────────────────────

    /// List audio encoder configurations via Media2.
    ///
    /// ONVIF Media2 WSDL `GetAudioEncoderConfigurations` — Profile T §8.11.
    pub async fn get_audio_encoder_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<AudioEncoderConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurations";
        const BODY: &str = "<tr2:GetAudioEncoderConfigurations/>";
        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioEncoderConfigurationsResponse")?;
        AudioEncoderConfiguration::vec_from_xml(resp)
    }

    /// Retrieve audio encoder configuration options via Media2.
    ///
    /// ONVIF Media2 WSDL `GetAudioEncoderConfigurationOptions` — Profile T §8.11.
    pub async fn get_audio_encoder_configuration_options_media2(
        &self,
        media2_url: &str,
        config_token: Option<&str>,
    ) -> Result<AudioEncoderConfigurationOptions, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver20/media/wsdl/GetAudioEncoderConfigurationOptions";
        let inner = config_token
            .map(|tok| {
                format!(
                    "<tr2:ConfigurationToken>{}</tr2:ConfigurationToken>",
                    xml_escape(tok)
                )
            })
            .unwrap_or_default();
        let body = format!(
            "<tr2:GetAudioEncoderConfigurationOptions>{inner}</tr2:GetAudioEncoderConfigurationOptions>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioEncoderConfigurationOptionsResponse")?;
        AudioEncoderConfigurationOptions::from_xml(resp)
    }

    /// Apply an audio encoder configuration via Media2.
    ///
    /// ONVIF Media2 WSDL `SetAudioEncoderConfiguration` — Profile T §8.11.
    pub async fn set_audio_encoder_configuration_media2(
        &self,
        media2_url: &str,
        config: &AudioEncoderConfiguration,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetAudioEncoderConfiguration";
        let body = format!(
            "<tr2:SetAudioEncoderConfiguration>\
               {}\
             </tr2:SetAudioEncoderConfiguration>",
            config.to_xml_body()
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetAudioEncoderConfigurationResponse")?;
        Ok(())
    }

    // ── Audio output / decoder configurations (Media2) ───────────────────────

    /// List audio output configurations via Media2.
    ///
    /// ONVIF Media2 WSDL `GetAudioOutputConfigurations` — Profile T §8.13.
    pub async fn get_audio_output_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<AudioOutputConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetAudioOutputConfigurations";
        const BODY: &str = "<tr2:GetAudioOutputConfigurations/>";
        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioOutputConfigurationsResponse")?;
        AudioOutputConfiguration::vec_from_xml(resp)
    }

    /// List audio decoder configurations via Media2.
    ///
    /// ONVIF Media2 WSDL `GetAudioDecoderConfigurations` — Profile T §8.13.
    pub async fn get_audio_decoder_configurations_media2(
        &self,
        media2_url: &str,
    ) -> Result<Vec<AudioDecoderConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetAudioDecoderConfigurations";
        const BODY: &str = "<tr2:GetAudioDecoderConfigurations/>";
        let xml = self.call(media2_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetAudioDecoderConfigurationsResponse")?;
        AudioDecoderConfiguration::vec_from_xml(resp)
    }

    // ── Video source modes (Media2) ──────────────────────────────────────────

    /// List available video source modes via Media2.
    ///
    /// ONVIF Media2 WSDL `GetVideoSourceModes` — Profile T §8.7.
    pub async fn get_video_source_modes_media2(
        &self,
        media2_url: &str,
        video_source_token: &str,
    ) -> Result<Vec<VideoSourceMode>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceModes";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<tr2:GetVideoSourceModes>\
               <tr2:VideoSourceToken>{video_source_token}</tr2:VideoSourceToken>\
             </tr2:GetVideoSourceModes>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetVideoSourceModesResponse")?;
        VideoSourceMode::vec_from_xml(resp)
    }

    /// Switch the video source to a different mode.
    ///
    /// ONVIF Media2 WSDL `SetVideoSourceMode` — Profile T §8.7.
    /// Returns `true` if the device requires a reboot to apply the change.
    pub async fn set_video_source_mode_media2(
        &self,
        media2_url: &str,
        video_source_token: &str,
        video_source_mode_token: &str,
    ) -> Result<bool, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceMode";
        let video_source_token = xml_escape(video_source_token);
        let video_source_mode_token = xml_escape(video_source_mode_token);
        let body = format!(
            "<tr2:SetVideoSourceMode>\
               <tr2:VideoSourceToken>{video_source_token}</tr2:VideoSourceToken>\
               <tr2:VideoSourceModeToken>{video_source_mode_token}</tr2:VideoSourceModeToken>\
             </tr2:SetVideoSourceMode>"
        );
        let xml = self.call(media2_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SetVideoSourceModeResponse")?;
        Ok(resp
            .child("Reboot")
            .is_some_and(|n| n.text() == "true" || n.text() == "1"))
    }
}
