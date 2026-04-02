use super::{FloatRange, xml_str};
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
            out.push_str(&format!("<tt:IrCutFilter>{v}</tt:IrCutFilter>"));
        }
        if let Some(ref m) = self.white_balance_mode {
            out.push_str(&format!(
                "<tt:WhiteBalance><tt:Mode>{m}</tt:Mode></tt:WhiteBalance>"
            ));
        }
        if let Some(ref m) = self.exposure_mode {
            out.push_str(&format!(
                "<tt:Exposure><tt:Mode>{m}</tt:Mode></tt:Exposure>"
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
