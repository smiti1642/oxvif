use super::{FloatRange, xml_escape, xml_str};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── ImagingSettings ───────────────────────────────────────────────────────────

/// Image quality settings returned by `GetImagingSettings`.
///
/// All numeric fields use the device's native range (commonly 0–100).
/// `None` means the device did not report that field.
///
/// Pass a modified copy to
/// [`set_imaging_settings`](crate::client::OnvifClient::set_imaging_settings).
#[derive(Debug, Clone, Default)]
pub struct ImagingSettings {
    /// Image brightness level.
    pub brightness: Option<f32>,
    /// Color saturation level.
    pub color_saturation: Option<f32>,
    /// Image contrast level.
    pub contrast: Option<f32>,
    /// Image sharpness level.
    pub sharpness: Option<f32>,
    /// IR cut filter mode: `"ON"`, `"OFF"`, or `"AUTO"`.
    pub ir_cut_filter: Option<String>,
    /// White balance mode: `"AUTO"` or `"MANUAL"`.
    pub white_balance_mode: Option<String>,
    /// Exposure mode: `"AUTO"` or `"MANUAL"`.
    pub exposure_mode: Option<String>,
    /// Backlight compensation mode: `"OFF"` or `"ON"`.
    pub backlight_compensation: Option<String>,
}

impl ImagingSettings {
    /// Parse from a `GetImagingSettingsResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let s = resp
            .child("ImagingSettings")
            .ok_or_else(|| SoapError::missing("ImagingSettings"))?;

        let parse_f32 = |child: &str| s.child(child).and_then(|n| n.text().parse::<f32>().ok());

        Ok(Self {
            brightness: parse_f32("Brightness"),
            color_saturation: parse_f32("ColorSaturation"),
            contrast: parse_f32("Contrast"),
            sharpness: parse_f32("Sharpness"),
            ir_cut_filter: xml_str(s, "IrCutFilter").filter(|v| !v.is_empty()),
            white_balance_mode: s
                .path(&["WhiteBalance", "Mode"])
                .map(|n| n.text().to_string())
                .filter(|v| !v.is_empty()),
            exposure_mode: s
                .path(&["Exposure", "Mode"])
                .map(|n| n.text().to_string())
                .filter(|v| !v.is_empty()),
            backlight_compensation: s
                .path(&["BacklightCompensation", "Mode"])
                .map(|n| n.text().to_string())
                .filter(|v| !v.is_empty()),
        })
    }

    /// Serialise to a `<timg:ImagingSettings>` XML fragment for
    /// `SetImagingSettings`.
    pub(crate) fn to_xml_body(&self) -> String {
        let mut out = String::from("<timg:ImagingSettings>");
        if let Some(v) = self.brightness {
            out.push_str(&format!("<tt:Brightness>{v}</tt:Brightness>"));
        }
        if let Some(v) = self.color_saturation {
            out.push_str(&format!("<tt:ColorSaturation>{v}</tt:ColorSaturation>"));
        }
        if let Some(v) = self.contrast {
            out.push_str(&format!("<tt:Contrast>{v}</tt:Contrast>"));
        }
        if let Some(v) = self.sharpness {
            out.push_str(&format!("<tt:Sharpness>{v}</tt:Sharpness>"));
        }
        if let Some(ref v) = self.ir_cut_filter {
            out.push_str(&format!(
                "<tt:IrCutFilter>{}</tt:IrCutFilter>",
                xml_escape(v)
            ));
        }
        if let Some(ref m) = self.white_balance_mode {
            out.push_str(&format!(
                "<tt:WhiteBalance><tt:Mode>{}</tt:Mode></tt:WhiteBalance>",
                xml_escape(m)
            ));
        }
        if let Some(ref m) = self.exposure_mode {
            out.push_str(&format!(
                "<tt:Exposure><tt:Mode>{}</tt:Mode></tt:Exposure>",
                xml_escape(m)
            ));
        }
        if let Some(ref m) = self.backlight_compensation {
            out.push_str(&format!(
                "<tt:BacklightCompensation><tt:Mode>{}</tt:Mode></tt:BacklightCompensation>",
                xml_escape(m)
            ));
        }
        out.push_str("</timg:ImagingSettings>");
        out
    }
}

// ── ImagingOptions ────────────────────────────────────────────────────────────

/// Valid parameter ranges for `SetImagingSettings`, returned by `GetOptions`.
#[derive(Debug, Clone, Default)]
pub struct ImagingOptions {
    /// Valid brightness range.
    pub brightness: Option<FloatRange>,
    /// Valid color saturation range.
    pub color_saturation: Option<FloatRange>,
    /// Valid contrast range.
    pub contrast: Option<FloatRange>,
    /// Valid sharpness range.
    pub sharpness: Option<FloatRange>,
    /// Supported IR cut filter modes (e.g. `["ON", "OFF", "AUTO"]`).
    pub ir_cut_filter_modes: Vec<String>,
    /// Supported white balance modes (e.g. `["AUTO", "MANUAL"]`).
    pub white_balance_modes: Vec<String>,
    /// Supported exposure modes (e.g. `["AUTO", "MANUAL"]`).
    pub exposure_modes: Vec<String>,
}

impl ImagingOptions {
    /// Parse from a `GetOptionsResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("ImagingOptions")
            .ok_or_else(|| SoapError::missing("ImagingOptions"))?;

        let parse_range = |child: &str| {
            opts.child(child).map(|n| FloatRange {
                min: n
                    .child("Min")
                    .and_then(|m| m.text().parse().ok())
                    .unwrap_or(0.0),
                max: n
                    .child("Max")
                    .and_then(|m| m.text().parse().ok())
                    .unwrap_or(0.0),
            })
        };

        Ok(Self {
            brightness: parse_range("Brightness"),
            color_saturation: parse_range("ColorSaturation"),
            contrast: parse_range("Contrast"),
            sharpness: parse_range("Sharpness"),
            ir_cut_filter_modes: opts
                .children_named("IrCutFilterModes")
                .map(|n| n.text().to_string())
                .collect(),
            white_balance_modes: opts
                .child("WhiteBalance")
                .map(|wb| {
                    wb.children_named("Mode")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            exposure_modes: opts
                .child("Exposure")
                .map(|e| {
                    e.children_named("Mode")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

// ── ImagingStatus ─────────────────────────────────────────────────────────────

/// Current imaging / focus status returned by `imaging_get_status`.
#[derive(Debug, Clone, Default)]
pub struct ImagingStatus {
    /// Current focus position in the device's native range.
    pub focus_position: Option<f32>,
    /// Focus move state: `"IDLE"`, `"MOVING"`, or `"UNKNOWN"`.
    pub focus_move_status: String,
}

impl ImagingStatus {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let status = resp
            .child("Status")
            .ok_or_else(|| SoapError::missing("Status"))?;
        Ok(Self {
            focus_position: status
                .path(&["FocusStatus20", "Position"])
                .and_then(|n| n.text().parse().ok()),
            focus_move_status: status
                .path(&["FocusStatus20", "MoveStatus"])
                .map(|n| n.text().to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string()),
        })
    }
}

// ── ImagingMoveOptions ────────────────────────────────────────────────────────

/// Valid focus movement ranges returned by `imaging_get_move_options`.
#[derive(Debug, Clone, Default)]
pub struct ImagingMoveOptions {
    /// Valid absolute focus position range.
    pub absolute_position_range: Option<FloatRange>,
    /// Valid absolute focus speed range.
    pub absolute_speed_range: Option<FloatRange>,
    /// Valid relative focus distance range.
    pub relative_distance_range: Option<FloatRange>,
    /// Valid relative focus speed range.
    pub relative_speed_range: Option<FloatRange>,
    /// Valid continuous focus speed range.
    pub continuous_speed_range: Option<FloatRange>,
}

impl ImagingMoveOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("MoveOptions")
            .ok_or_else(|| SoapError::missing("MoveOptions"))?;

        let range = |parent: &str, child: &str| {
            opts.child(parent)
                .and_then(|p| p.child(child))
                .map(|n| FloatRange {
                    min: n
                        .child("Min")
                        .and_then(|m| m.text().parse().ok())
                        .unwrap_or(0.0),
                    max: n
                        .child("Max")
                        .and_then(|m| m.text().parse().ok())
                        .unwrap_or(0.0),
                })
        };

        Ok(Self {
            absolute_position_range: range("Absolute", "PositionSpace"),
            absolute_speed_range: range("Absolute", "SpeedSpace"),
            relative_distance_range: range("Relative", "DistanceSpace"),
            relative_speed_range: range("Relative", "SpeedSpace"),
            continuous_speed_range: range("Continuous", "SpeedSpace"),
        })
    }
}

// ── FocusMove ─────────────────────────────────────────────────────────────────

/// Focus movement command passed to `imaging_move`.
#[derive(Debug, Clone)]
pub enum FocusMove {
    /// Move focus to an absolute position.
    Absolute {
        /// Target focus position in the device's native range.
        position: f32,
        /// Movement speed. `None` uses the device default.
        speed: Option<f32>,
    },
    /// Move focus by a relative distance.
    Relative {
        /// Distance to move (positive = far, negative = near).
        distance: f32,
        /// Movement speed. `None` uses the device default.
        speed: Option<f32>,
    },
    /// Start continuous focus movement at a given speed.
    ///
    /// Call `imaging_stop` to halt.
    Continuous {
        /// Movement speed: positive = far, negative = near.
        speed: f32,
    },
}

impl FocusMove {
    pub(crate) fn to_xml_body(&self) -> String {
        match self {
            Self::Absolute { position, speed } => {
                let speed_el = speed
                    .map(|s| format!("<timg:Speed>{s}</timg:Speed>"))
                    .unwrap_or_default();
                format!(
                    "<timg:Absolute>\
                       <timg:Position>{position}</timg:Position>\
                       {speed_el}\
                     </timg:Absolute>"
                )
            }
            Self::Relative { distance, speed } => {
                let speed_el = speed
                    .map(|s| format!("<timg:Speed>{s}</timg:Speed>"))
                    .unwrap_or_default();
                format!(
                    "<timg:Relative>\
                       <timg:Distance>{distance}</timg:Distance>\
                       {speed_el}\
                     </timg:Relative>"
                )
            }
            Self::Continuous { speed } => {
                format!("<timg:Continuous><timg:Speed>{speed}</timg:Speed></timg:Continuous>")
            }
        }
    }
}
