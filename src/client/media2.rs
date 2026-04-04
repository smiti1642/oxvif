// ── Media2 Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    MediaProfile2, VideoEncoderConfiguration2, VideoEncoderConfigurationOptions2,
    VideoEncoderInstances, VideoSourceConfiguration, VideoSourceConfigurationOptions, xml_escape,
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
}
