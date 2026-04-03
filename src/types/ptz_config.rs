use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── PtzSpaceRange ─────────────────────────────────────────────────────────────

/// A single PTZ space definition with a URI and coordinate ranges.
#[derive(Debug, Clone, Default)]
pub struct PtzSpaceRange {
    /// ONVIF space URI (e.g.
    /// `http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace`).
    pub uri: String,
    /// Min/max range on the X axis (pan or zoom).
    pub x_range: (f32, f32),
    /// Min/max range on the Y axis (tilt). `None` for zoom-only spaces.
    pub y_range: Option<(f32, f32)>,
}

fn parse_space_range(node: &XmlNode) -> PtzSpaceRange {
    let uri = xml_str(node, "URI").unwrap_or_default();
    let x_range = node
        .child("XRange")
        .map(|r| {
            let min = r
                .child("Min")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(-1.0);
            let max = r
                .child("Max")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(1.0);
            (min, max)
        })
        .unwrap_or((-1.0, 1.0));
    let y_range = node.child("YRange").map(|r| {
        let min = r
            .child("Min")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(-1.0);
        let max = r
            .child("Max")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(1.0);
        (min, max)
    });
    PtzSpaceRange {
        uri,
        x_range,
        y_range,
    }
}

// ── PtzSpeed ──────────────────────────────────────────────────────────────────

/// Default PTZ movement speed stored in a `PtzConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct PtzSpeed {
    /// Default pan (x) and tilt (y) speed, normalised range `[0, 1]`.
    pub pan_tilt: Option<(f32, f32)>,
    /// Default zoom speed, normalised range `[0, 1]`.
    pub zoom: Option<f32>,
}

// ── PtzConfiguration ──────────────────────────────────────────────────────────

/// PTZ configuration returned by `GetConfigurations` / `GetConfiguration`.
///
/// Pass a modified copy to `ptz_set_configuration`.
#[derive(Debug, Clone)]
pub struct PtzConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    /// Token of the PTZ node this configuration targets.
    pub node_token: String,
    /// Default PTZ operation timeout as ISO 8601 duration (e.g. `"PT5S"`).
    pub default_ptz_timeout: Option<String>,
    /// Default coordinate space URI for absolute pan/tilt moves.
    pub default_abs_pan_tilt_space: Option<String>,
    /// Default coordinate space URI for absolute zoom moves.
    pub default_abs_zoom_space: Option<String>,
    /// Default coordinate space URI for relative pan/tilt moves.
    pub default_rel_pan_tilt_space: Option<String>,
    /// Default coordinate space URI for relative zoom moves.
    pub default_rel_zoom_space: Option<String>,
    /// Default coordinate space URI for continuous pan/tilt velocity.
    pub default_cont_pan_tilt_space: Option<String>,
    /// Default coordinate space URI for continuous zoom velocity.
    pub default_cont_zoom_space: Option<String>,
    /// Default movement speed used when no speed is specified in a move command.
    pub default_ptz_speed: Option<PtzSpeed>,
    /// Pan/tilt position limits, if set.
    pub pan_tilt_limits: Option<PtzSpaceRange>,
    /// Zoom position limits, if set.
    pub zoom_limits: Option<PtzSpaceRange>,
}

impl PtzConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("PTZConfiguration/@token"))?
            .to_string();
        Ok(Self {
            token,
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            node_token: xml_str(node, "NodeToken").unwrap_or_default(),
            default_ptz_timeout: xml_str(node, "DefaultPTZTimeout"),
            default_abs_pan_tilt_space: xml_str(node, "DefaultAbsolutePanTiltPositionSpace"),
            default_abs_zoom_space: xml_str(node, "DefaultAbsoluteZoomPositionSpace"),
            default_rel_pan_tilt_space: xml_str(node, "DefaultRelativePanTiltTranslationSpace"),
            default_rel_zoom_space: xml_str(node, "DefaultRelativeZoomTranslationSpace"),
            default_cont_pan_tilt_space: xml_str(node, "DefaultContinuousPanTiltVelocitySpace"),
            default_cont_zoom_space: xml_str(node, "DefaultContinuousZoomVelocitySpace"),
            default_ptz_speed: node.child("DefaultPTZSpeed").map(|s| PtzSpeed {
                pan_tilt: s.child("PanTilt").and_then(|n| {
                    let x = n.attr("x")?.parse().ok()?;
                    let y = n.attr("y")?.parse().ok()?;
                    Some((x, y))
                }),
                zoom: s.child("Zoom").and_then(|n| n.attr("x")?.parse().ok()),
            }),
            pan_tilt_limits: node
                .path(&["PanTiltLimits", "Range"])
                .map(parse_space_range),
            zoom_limits: node.path(&["ZoomLimits", "Range"]).map(parse_space_range),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("PTZConfiguration")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<tptz:PTZConfiguration>` XML fragment for
    /// `SetConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        let opt_str = |v: &Option<String>, tag: &str| -> String {
            v.as_deref()
                .map(|s| format!("<tt:{tag}>{}<tt:/{tag}>", xml_escape(s)))
                .unwrap_or_default()
        };
        let timeout_el = opt_str(&self.default_ptz_timeout, "DefaultPTZTimeout");
        let abs_pt = opt_str(
            &self.default_abs_pan_tilt_space,
            "DefaultAbsolutePanTiltPositionSpace",
        );
        let abs_z = opt_str(
            &self.default_abs_zoom_space,
            "DefaultAbsoluteZoomPositionSpace",
        );
        let rel_pt = opt_str(
            &self.default_rel_pan_tilt_space,
            "DefaultRelativePanTiltTranslationSpace",
        );
        let rel_z = opt_str(
            &self.default_rel_zoom_space,
            "DefaultRelativeZoomTranslationSpace",
        );
        let cont_pt = opt_str(
            &self.default_cont_pan_tilt_space,
            "DefaultContinuousPanTiltVelocitySpace",
        );
        let cont_z = opt_str(
            &self.default_cont_zoom_space,
            "DefaultContinuousZoomVelocitySpace",
        );
        let speed_el = match &self.default_ptz_speed {
            Some(s) => {
                let pt = s
                    .pan_tilt
                    .map(|(x, y)| format!("<tt:PanTilt x=\"{x}\" y=\"{y}\"/>"))
                    .unwrap_or_default();
                let z = s
                    .zoom
                    .map(|x| format!("<tt:Zoom x=\"{x}\"/>"))
                    .unwrap_or_default();
                format!("<tt:DefaultPTZSpeed>{pt}{z}</tt:DefaultPTZSpeed>")
            }
            None => String::new(),
        };
        format!(
            "<tptz:PTZConfiguration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:NodeToken>{node_token}</tt:NodeToken>\
               {abs_pt}{abs_z}{rel_pt}{rel_z}{cont_pt}{cont_z}\
               {speed_el}{timeout_el}\
             </tptz:PTZConfiguration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            node_token = xml_escape(&self.node_token),
        )
    }
}

// ── PtzConfigurationOptions ───────────────────────────────────────────────────

/// Valid parameter ranges for `SetConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct PtzConfigurationOptions {
    /// Minimum PTZ operation timeout as ISO 8601 duration.
    pub ptz_timeout_min: Option<String>,
    /// Maximum PTZ operation timeout as ISO 8601 duration.
    pub ptz_timeout_max: Option<String>,
}

impl PtzConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("PTZConfigurationOptions")
            .ok_or_else(|| SoapError::missing("PTZConfigurationOptions"))?;
        Ok(Self {
            ptz_timeout_min: opts
                .path(&["PTZTimeout", "Min"])
                .map(|n| n.text().to_string()),
            ptz_timeout_max: opts
                .path(&["PTZTimeout", "Max"])
                .map(|n| n.text().to_string()),
        })
    }
}

// ── PtzNode ───────────────────────────────────────────────────────────────────

/// A PTZ node returned by `GetNodes`.
///
/// Describes the physical PTZ capabilities of the device (supported spaces,
/// preset count, home position support, etc.).
#[derive(Debug, Clone)]
pub struct PtzNode {
    /// Opaque token; referenced by `PtzConfiguration.node_token`.
    pub token: String,
    pub name: String,
    /// `true` if the home position is fixed and cannot be changed.
    pub fixed_home_position: bool,
    /// `true` if `GoHome` / `SetHome` are supported.
    pub home_supported: bool,
    /// Maximum number of presets this node supports.
    pub max_presets: u32,
    /// Auxiliary command strings supported by this node.
    pub aux_commands: Vec<String>,
    /// Supported pan/tilt position and speed spaces.
    pub pan_tilt_spaces: Vec<PtzSpaceRange>,
    /// Supported zoom position and speed spaces.
    pub zoom_spaces: Vec<PtzSpaceRange>,
}

impl PtzNode {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("PTZNode")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("PTZNode/@token"))?
                    .to_string();
                let spaces = n.child("SupportedPTZSpaces");
                let pan_tilt_spaces = spaces
                    .map(|s| {
                        s.children_named("AbsolutePanTiltPositionSpace")
                            .chain(s.children_named("RelativePanTiltTranslationSpace"))
                            .chain(s.children_named("ContinuousPanTiltVelocitySpace"))
                            .chain(s.children_named("PanTiltSpeedSpace"))
                            .map(parse_space_range)
                            .collect()
                    })
                    .unwrap_or_default();
                let zoom_spaces = spaces
                    .map(|s| {
                        s.children_named("AbsoluteZoomPositionSpace")
                            .chain(s.children_named("RelativeZoomTranslationSpace"))
                            .chain(s.children_named("ContinuousZoomVelocitySpace"))
                            .chain(s.children_named("ZoomSpeedSpace"))
                            .map(parse_space_range)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Self {
                    token,
                    name: xml_str(n, "Name").unwrap_or_default(),
                    fixed_home_position: n.attr("FixedHomePosition") == Some("true"),
                    home_supported: n
                        .child("HomeSupported")
                        .is_some_and(|h| h.text() == "true" || h.text() == "1"),
                    max_presets: xml_u32(n, "MaximumNumberOfPresets").unwrap_or(0),
                    aux_commands: n
                        .children_named("AuxiliaryCommands")
                        .map(|c| c.text().to_string())
                        .collect(),
                    pan_tilt_spaces,
                    zoom_spaces,
                })
            })
            .collect()
    }
}
