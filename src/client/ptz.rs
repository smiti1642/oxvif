// ── PTZ Service ───────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    PtzConfiguration, PtzConfigurationOptions, PtzNode, PtzPreset, PtzStatus, xml_escape,
};

impl OnvifClient {
    /// Move the camera to an absolute position.
    ///
    /// Coordinates are in the normalised range `[-1.0, 1.0]` for pan/tilt
    /// and `[0.0, 1.0]` for zoom. `ptz_url` comes from
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn ptz_absolute_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:AbsoluteMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Position>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Position>\
             </tptz:AbsoluteMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "AbsoluteMoveResponse")?;
        Ok(())
    }

    /// Move the camera by a relative offset from the current position.
    ///
    /// Values are in the normalised range `[-1.0, 1.0]` for pan/tilt
    /// and `[-1.0, 1.0]` for zoom.
    pub async fn ptz_relative_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:RelativeMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Translation>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Translation>\
             </tptz:RelativeMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RelativeMoveResponse")?;
        Ok(())
    }

    /// Start continuous pan/tilt/zoom movement at the given velocities.
    ///
    /// Values are in the normalised range `[-1.0, 1.0]`. Call
    /// [`ptz_stop`](Self::ptz_stop) to halt movement.
    pub async fn ptz_continuous_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        pan: f32,
        tilt: f32,
        zoom: f32,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:ContinuousMove>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:Velocity>\
                 <tt:PanTilt x=\"{pan}\" y=\"{tilt}\"/>\
                 <tt:Zoom x=\"{zoom}\"/>\
               </tptz:Velocity>\
             </tptz:ContinuousMove>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "ContinuousMoveResponse")?;
        Ok(())
    }

    /// Stop all ongoing PTZ movement.
    pub async fn ptz_stop(&self, ptz_url: &str, profile_token: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/Stop";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:Stop>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PanTilt>true</tptz:PanTilt>\
               <tptz:Zoom>true</tptz:Zoom>\
             </tptz:Stop>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "StopResponse")?;
        Ok(())
    }

    /// List all saved PTZ presets for the given profile.
    pub async fn ptz_get_presets(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<Vec<PtzPreset>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetPresets";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:GetPresets>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:GetPresets>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetPresetsResponse")?;
        PtzPreset::vec_from_xml(resp)
    }

    /// Move the camera to a saved PTZ preset.
    ///
    /// `preset_token` comes from a [`PtzPreset`] returned by
    /// [`ptz_get_presets`](Self::ptz_get_presets).
    pub async fn ptz_goto_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GotoPreset";
        let profile_token = xml_escape(profile_token);
        let preset_token = xml_escape(preset_token);
        let body = format!(
            "<tptz:GotoPreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PresetToken>{preset_token}</tptz:PresetToken>\
             </tptz:GotoPreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "GotoPresetResponse")?;
        Ok(())
    }

    /// Save the current camera position as a named preset.
    ///
    /// Pass `preset_name` to label the preset and `preset_token` to overwrite
    /// an existing preset rather than create a new one. Returns the token of
    /// the saved (or updated) preset.
    pub async fn ptz_set_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_name: Option<&str>,
        preset_token: Option<&str>,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/SetPreset";
        let profile_token = xml_escape(profile_token);
        let name_el = preset_name
            .map(|n| format!("<tptz:PresetName>{}</tptz:PresetName>", xml_escape(n)))
            .unwrap_or_default();
        let token_el = preset_token
            .map(|t| format!("<tptz:PresetToken>{}</tptz:PresetToken>", xml_escape(t)))
            .unwrap_or_default();
        let body = format!(
            "<tptz:SetPreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               {name_el}{token_el}\
             </tptz:SetPreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SetPresetResponse")?;
        resp.child("PresetToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("PresetToken").into())
    }

    /// Delete a saved PTZ preset.
    ///
    /// `preset_token` comes from a [`PtzPreset`] returned by
    /// [`ptz_get_presets`](Self::ptz_get_presets).
    pub async fn ptz_remove_preset(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/RemovePreset";
        let profile_token = xml_escape(profile_token);
        let preset_token = xml_escape(preset_token);
        let body = format!(
            "<tptz:RemovePreset>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               <tptz:PresetToken>{preset_token}</tptz:PresetToken>\
             </tptz:RemovePreset>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemovePresetResponse")?;
        Ok(())
    }

    /// Query the current PTZ position and movement state.
    ///
    /// Returns a [`PtzStatus`] with the normalised pan, tilt, and zoom
    /// positions, and a movement state string (`"IDLE"` or `"MOVING"`).
    pub async fn ptz_get_status(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<PtzStatus, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetStatus";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:GetStatus>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:GetStatus>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStatusResponse")?;
        PtzStatus::from_xml(resp)
    }

    /// Move the camera to its configured home position.
    pub async fn ptz_goto_home_position(
        &self,
        ptz_url: &str,
        profile_token: &str,
        speed: Option<f32>,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GotoHomePosition";
        let profile_token = xml_escape(profile_token);
        let speed_el = speed
            .map(|s| format!("<tptz:Speed><tt:Zoom x=\"{s}\"/></tptz:Speed>"))
            .unwrap_or_default();
        let body = format!(
            "<tptz:GotoHomePosition>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
               {speed_el}\
             </tptz:GotoHomePosition>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "GotoHomePositionResponse")?;
        Ok(())
    }

    /// Set the current PTZ position as the home position.
    pub async fn ptz_set_home_position(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/SetHomePosition";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:SetHomePosition>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:SetHomePosition>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetHomePositionResponse")?;
        Ok(())
    }

    // ── PTZ Configuration ─────────────────────────────────────────────────────

    /// List all PTZ configurations on the device.
    ///
    /// `ptz_url` comes from `caps.ptz.url` returned by
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn ptz_get_configurations(
        &self,
        ptz_url: &str,
    ) -> Result<Vec<PtzConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurations";
        const BODY: &str = "<tptz:GetConfigurations/>";
        let xml = self.call(ptz_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetConfigurationsResponse")?;
        PtzConfiguration::vec_from_xml(resp)
    }

    /// Retrieve a single PTZ configuration by token.
    ///
    /// `ptz_url` comes from `caps.ptz.url`.
    pub async fn ptz_get_configuration(
        &self,
        ptz_url: &str,
        config_token: &str,
    ) -> Result<PtzConfiguration, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration";
        let body = format!(
            "<tptz:GetConfiguration>\
               <tptz:PTZConfigurationToken>{}</tptz:PTZConfigurationToken>\
             </tptz:GetConfiguration>",
            xml_escape(config_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetConfigurationResponse")?;
        let node = resp
            .child("PTZConfiguration")
            .ok_or_else(|| crate::soap::SoapError::missing("PTZConfiguration"))?;
        PtzConfiguration::from_xml(node)
    }

    /// Write a PTZ configuration back to the device.
    ///
    /// Obtain the current config via
    /// [`ptz_get_configuration`](Self::ptz_get_configuration),
    /// modify the fields, then call this method.
    pub async fn ptz_set_configuration(
        &self,
        ptz_url: &str,
        config: &PtzConfiguration,
        force_persist: bool,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/SetConfiguration";
        let persist = if force_persist { "true" } else { "false" };
        let body = format!(
            "<tptz:SetConfiguration>\
               {}\
               <tptz:ForcePersistence>{persist}</tptz:ForcePersistence>\
             </tptz:SetConfiguration>",
            config.to_xml_body()
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve valid parameter ranges for a PTZ configuration.
    ///
    /// `ptz_url` comes from `caps.ptz.url`.
    pub async fn ptz_get_configuration_options(
        &self,
        ptz_url: &str,
        config_token: &str,
    ) -> Result<PtzConfigurationOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions";
        let body = format!(
            "<tptz:GetConfigurationOptions>\
               <tptz:ConfigurationToken>{}</tptz:ConfigurationToken>\
             </tptz:GetConfigurationOptions>",
            xml_escape(config_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetConfigurationOptionsResponse")?;
        PtzConfigurationOptions::from_xml(resp)
    }

    /// List all PTZ nodes on the device.
    ///
    /// `ptz_url` comes from `caps.ptz.url`.
    pub async fn ptz_get_nodes(&self, ptz_url: &str) -> Result<Vec<PtzNode>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetNodes";
        const BODY: &str = "<tptz:GetNodes/>";
        let xml = self.call(ptz_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNodesResponse")?;
        PtzNode::vec_from_xml(resp)
    }

    /// Retrieve a single PTZ node by token.
    ///
    /// ONVIF PTZ WSDL `GetNode` — Profile T §8.2 (mandatory when PTZ
    /// configuration is supported; client shall support at least one of
    /// `GetNodes` or `GetNode`).
    pub async fn ptz_get_node(
        &self,
        ptz_url: &str,
        node_token: &str,
    ) -> Result<PtzNode, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetNode";
        let body = format!(
            "<tptz:GetNode>\
               <tptz:NodeToken>{}</tptz:NodeToken>\
             </tptz:GetNode>",
            xml_escape(node_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNodeResponse")?;
        let node = resp
            .child("PTZNode")
            .ok_or_else(|| crate::soap::SoapError::missing("PTZNode"))?;
        PtzNode::from_xml(node)
    }

    /// List PTZ configurations compatible with a given media profile.
    ///
    /// ONVIF PTZ WSDL `GetCompatibleConfigurations` — Profile T §8.1
    /// (mandatory when PTZ profile configuration is supported).
    pub async fn ptz_get_compatible_configurations(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<Vec<PtzConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetCompatibleConfigurations";
        let profile_token = xml_escape(profile_token);
        let body = format!(
            "<tptz:GetCompatibleConfigurations>\
               <tptz:ProfileToken>{profile_token}</tptz:ProfileToken>\
             </tptz:GetCompatibleConfigurations>"
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetCompatibleConfigurationsResponse")?;
        PtzConfiguration::vec_from_xml(resp)
    }
}
