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
        let timeout_el = self
            .default_ptz_timeout
            .as_deref()
            .map(|t| {
                format!(
                    "<tt:DefaultPTZTimeout>{}</tt:DefaultPTZTimeout>",
                    xml_escape(t)
                )
            })
            .unwrap_or_default();
        format!(
            "<tptz:PTZConfiguration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:NodeToken>{node_token}</tt:NodeToken>\
               {timeout_el}\
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
                })
            })
            .collect()
    }
}
