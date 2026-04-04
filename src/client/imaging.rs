// ── Imaging Service ───────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    FocusMove, ImagingMoveOptions, ImagingOptions, ImagingSettings, ImagingStatus, xml_escape,
};

impl OnvifClient {
    /// Retrieve the current image quality settings for a video source.
    ///
    /// `imaging_url` is obtained from
    /// [`get_capabilities`](Self::get_capabilities) via `caps.imaging.url`.
    /// `video_source_token` comes from a [`VideoSource`] returned by
    /// [`get_video_sources`](Self::get_video_sources).
    pub async fn get_imaging_settings(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingSettings, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:GetImagingSettings>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetImagingSettings>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetImagingSettingsResponse")?;
        ImagingSettings::from_xml(resp)
    }

    /// Apply modified image quality settings to a video source.
    ///
    /// Obtain the current settings with
    /// [`get_imaging_settings`](Self::get_imaging_settings), modify the
    /// fields you want to change, then pass the result here.
    pub async fn set_imaging_settings(
        &self,
        imaging_url: &str,
        video_source_token: &str,
        settings: &ImagingSettings,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/SetImagingSettings";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:SetImagingSettings>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
               {settings_xml}\
               <timg:ForcePersistence>true</timg:ForcePersistence>\
             </timg:SetImagingSettings>",
            settings_xml = settings.to_xml_body()
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetImagingSettingsResponse")?;
        Ok(())
    }

    /// Retrieve the valid parameter ranges for `set_imaging_settings`.
    ///
    /// Use the returned [`ImagingOptions`] to validate or clamp values before
    /// calling [`set_imaging_settings`](Self::set_imaging_settings).
    pub async fn get_imaging_options(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetOptions";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:GetOptions>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetOptions>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetOptionsResponse")?;
        ImagingOptions::from_xml(resp)
    }

    /// Move the focus to an absolute position, by a relative distance, or start
    /// continuous movement.
    ///
    /// Build the command with [`FocusMove`]. Call
    /// [`imaging_stop`](Self::imaging_stop) to halt continuous movement.
    pub async fn imaging_move(
        &self,
        imaging_url: &str,
        video_source_token: &str,
        focus: &FocusMove,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/Move";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:Move>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
               <timg:Focus>{}</timg:Focus>\
             </timg:Move>",
            focus.to_xml_body()
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "MoveResponse")?;
        Ok(())
    }

    /// Stop any ongoing focus movement.
    pub async fn imaging_stop(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/Stop";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:Stop>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:Stop>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "StopResponse")?;
        Ok(())
    }

    /// Retrieve the valid focus movement ranges for a video source.
    pub async fn imaging_get_move_options(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingMoveOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetMoveOptions";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:GetMoveOptions>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetMoveOptions>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetMoveOptionsResponse")?;
        ImagingMoveOptions::from_xml(resp)
    }

    /// Retrieve the current focus position and movement state.
    pub async fn imaging_get_status(
        &self,
        imaging_url: &str,
        video_source_token: &str,
    ) -> Result<ImagingStatus, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/imaging/wsdl/GetStatus";
        let video_source_token = xml_escape(video_source_token);
        let body = format!(
            "<timg:GetStatus>\
               <timg:VideoSourceToken>{video_source_token}</timg:VideoSourceToken>\
             </timg:GetStatus>"
        );
        let xml = self.call(imaging_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStatusResponse")?;
        ImagingStatus::from_xml(resp)
    }
}
