// ── PTZ Service ───────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    PtzConfiguration, PtzConfigurationOptions, PtzNode, PtzPreset, PtzPresetTour,
    PtzPresetTourOperation, PtzPresetTourOptions, PtzServiceCapabilities, PtzStatus, xml_escape,
};

impl OnvifClient {
    /// Ask the PTZ service what it can do.
    ///
    /// Worth calling before [`ptz_get_status`](Self::ptz_get_status): a device
    /// with `move_status` or `status_position` unset is not obliged to fill in
    /// those parts of the status response, so an empty field there means
    /// "never reported" rather than "not moving".
    pub async fn ptz_get_service_capabilities(
        &self,
        ptz_url: &str,
    ) -> Result<PtzServiceCapabilities, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetServiceCapabilities";
        const BODY: &str = "<tptz:GetServiceCapabilities/>";

        let xml = self.call(ptz_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetServiceCapabilitiesResponse")?;
        PtzServiceCapabilities::from_xml(resp)
    }

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
    ///
    /// [`pan_tilt_limits`](PtzConfiguration::pan_tilt_limits) and
    /// [`zoom_limits`](PtzConfiguration::zoom_limits) are sent; they were read
    /// and silently dropped until 0.15, so a round-trip cleared whatever limits
    /// the device had. The absolute pan/tilt space is written under ONVIF's own
    /// spelling — see
    /// [`default_abs_pan_tilt_space`](PtzConfiguration::default_abs_pan_tilt_space).
    ///
    /// # Errors
    ///
    /// Returns [`OnvifError::InvalidArgument`] **before any request is sent**
    /// when `pan_tilt_limits` is `Some` with `y_range: None`. Its `Range` is a
    /// `tt:Space2DDescription`, whose `YRange` is required, so there is no
    /// conformant document to send. `zoom_limits` is one-dimensional and has no
    /// such constraint.
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
            config.to_xml_body()?
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

    // ── Preset tours ──────────────────────────────────────────────────────────
    //
    // A preset tour is a stored guard tour: a named sequence of stops the
    // camera walks unattended. All seven operations are per-profile, so every
    // one of them takes a `profile_token` — a device given no profile answers
    // for whichever one it considers default, which on a multi-sensor camera is
    // the wrong lens.

    /// List every preset tour stored against a media profile.
    ///
    /// `ptz_url` comes from `caps.ptz.url` returned by
    /// [`get_capabilities`](Self::get_capabilities).
    pub async fn ptz_get_preset_tours(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<Vec<PtzPresetTour>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetPresetTours";
        let body = format!(
            "<tptz:GetPresetTours>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
             </tptz:GetPresetTours>",
            xml_escape(profile_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetPresetToursResponse")?;
        PtzPresetTour::vec_from_xml(resp)
    }

    /// Retrieve one preset tour by token.
    pub async fn ptz_get_preset_tour(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_tour_token: &str,
    ) -> Result<PtzPresetTour, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetPresetTour";
        let body = format!(
            "<tptz:GetPresetTour>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               <tptz:PresetTourToken>{}</tptz:PresetTourToken>\
             </tptz:GetPresetTour>",
            xml_escape(profile_token),
            xml_escape(preset_tour_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetPresetTourResponse")?;
        PtzPresetTour::from_xml(resp)
    }

    /// Retrieve what preset tours this device will accept.
    ///
    /// `preset_tour_token` is optional: pass `Some(token)` to ask what may be
    /// changed about an existing tour, or `None` to ask what a new tour may
    /// contain. Read this before building a tour for
    /// [`ptz_modify_preset_tour`](Self::ptz_modify_preset_tour) — a stop
    /// outside the returned bounds comes back as a fault rather than being
    /// clamped.
    pub async fn ptz_get_preset_tour_options(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_tour_token: Option<&str>,
    ) -> Result<PtzPresetTourOptions, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetPresetTourOptions";
        let tour = preset_tour_token
            .map(|t| {
                format!(
                    "<tptz:PresetTourToken>{}</tptz:PresetTourToken>",
                    xml_escape(t)
                )
            })
            .unwrap_or_default();
        let body = format!(
            "<tptz:GetPresetTourOptions>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               {tour}\
             </tptz:GetPresetTourOptions>",
            xml_escape(profile_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetPresetTourOptionsResponse")?;
        PtzPresetTourOptions::from_xml(resp)
    }

    /// Create an empty preset tour and return its new token.
    ///
    /// The tour has no stops yet. Fill it in with
    /// [`ptz_modify_preset_tour`](Self::ptz_modify_preset_tour), then start it
    /// with [`ptz_operate_preset_tour`](Self::ptz_operate_preset_tour).
    pub async fn ptz_create_preset_tour(
        &self,
        ptz_url: &str,
        profile_token: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/CreatePresetTour";
        let body = format!(
            "<tptz:CreatePresetTour>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
             </tptz:CreatePresetTour>",
            xml_escape(profile_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreatePresetTourResponse")?;
        resp.child("PresetTourToken")
            .map(|n| n.text().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| crate::soap::SoapError::missing("PresetTourToken").into())
    }

    /// Write a preset tour back to the device.
    ///
    /// This is the only Tier 1 operation that sends structured user data, so
    /// it is the one where escaping matters: `tour.name` and every preset
    /// token inside `tour.tour_spots` are user- or device-supplied and go
    /// through `xml_escape` on the way out.
    ///
    /// Obtain the tour with
    /// [`ptz_get_preset_tour`](Self::ptz_get_preset_tour), modify it, and pass
    /// it here. `tour.token` must name an existing tour — the one from
    /// [`ptz_create_preset_tour`](Self::ptz_create_preset_tour) if the tour is
    /// new.
    pub async fn ptz_modify_preset_tour(
        &self,
        ptz_url: &str,
        profile_token: &str,
        tour: &PtzPresetTour,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/ModifyPresetTour";
        let body = format!(
            "<tptz:ModifyPresetTour>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               {tour_xml}\
             </tptz:ModifyPresetTour>",
            xml_escape(profile_token),
            tour_xml = tour.to_xml_body()
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "ModifyPresetTourResponse")?;
        Ok(())
    }

    /// Start, stop or pause a preset tour.
    pub async fn ptz_operate_preset_tour(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_tour_token: &str,
        operation: PtzPresetTourOperation,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/OperatePresetTour";
        let body = format!(
            "<tptz:OperatePresetTour>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               <tptz:PresetTourToken>{}</tptz:PresetTourToken>\
               <tptz:Operation>{}</tptz:Operation>\
             </tptz:OperatePresetTour>",
            xml_escape(profile_token),
            xml_escape(preset_tour_token),
            operation.as_str()
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "OperatePresetTourResponse")?;
        Ok(())
    }

    /// Delete a preset tour.
    pub async fn ptz_remove_preset_tour(
        &self,
        ptz_url: &str,
        profile_token: &str,
        preset_tour_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/RemovePresetTour";
        let body = format!(
            "<tptz:RemovePresetTour>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               <tptz:PresetTourToken>{}</tptz:PresetTourToken>\
             </tptz:RemovePresetTour>",
            xml_escape(profile_token),
            xml_escape(preset_tour_token)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "RemovePresetTourResponse")?;
        Ok(())
    }

    // ── Auxiliary commands ────────────────────────────────────────────────────

    /// Send an auxiliary command to a media profile — wiper, washer, IR lamp.
    ///
    /// **This is not [`send_auxiliary_command`](Self::send_auxiliary_command).**
    /// That one is the *Device* service operation, takes no profile, and
    /// returns nothing useful. This is the PTZ service operation, is scoped to
    /// a profile, and **returns the device's answer**. Cameras that implement a
    /// wiper generally implement it here — try this one first, and fall back to
    /// the Device operation on firmware that faults.
    ///
    /// The values a given camera accepts are **discoverable**, not guessable:
    /// they are vendor-namespaced, so the schema does not enumerate them and
    /// this crate deliberately does not model them as an enum. Read
    /// [`DeviceServiceCapabilities::misc`](crate::DeviceServiceCapabilities::misc)
    /// → `auxiliary_commands` for the list this device advertises. Common
    /// values are `"tt:Wiper|On"`, `"tt:Wiper|Off"`, `"tt:Washer|On"`,
    /// `"tt:IRLamp|On"`, `"tt:IRLamp|Auto"`.
    ///
    /// `tt:AuxiliaryData` has a schema `maxLength` of 128. That is **not**
    /// enforced here: the device rejects an over-long value with a fault, and a
    /// client-side length check would be a second source of truth that drifts
    /// from the firmware.
    pub async fn ptz_send_auxiliary_command(
        &self,
        ptz_url: &str,
        profile_token: &str,
        auxiliary_data: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver20/ptz/wsdl/SendAuxiliaryCommand";
        let body = format!(
            "<tptz:SendAuxiliaryCommand>\
               <tptz:ProfileToken>{}</tptz:ProfileToken>\
               <tptz:AuxiliaryData>{}</tptz:AuxiliaryData>\
             </tptz:SendAuxiliaryCommand>",
            xml_escape(profile_token),
            xml_escape(auxiliary_data)
        );
        let xml = self.call(ptz_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SendAuxiliaryCommandResponse")?;
        resp.child("AuxiliaryResponse")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("AuxiliaryResponse").into())
    }
}

#[cfg(test)]
#[path = "../tests/client/ptz_tests.rs"]
mod tests;
