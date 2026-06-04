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
    /// Manual white-balance red-channel gain (device-native range).
    /// Only meaningful when `white_balance_mode == "MANUAL"`.
    pub wb_cr_gain: Option<f32>,
    /// Manual white-balance blue-channel gain (device-native range).
    /// Only meaningful when `white_balance_mode == "MANUAL"`.
    pub wb_cb_gain: Option<f32>,
    /// Exposure mode: `"AUTO"` or `"MANUAL"`.
    pub exposure_mode: Option<String>,
    /// Auto-exposure priority: `"FrameRate"` or `"LowNoise"`.
    pub exposure_priority: Option<String>,
    /// Manual exposure time in seconds. Only meaningful when
    /// `exposure_mode == "MANUAL"`.
    pub exposure_time: Option<f32>,
    /// Manual sensor gain in dB. Only meaningful when
    /// `exposure_mode == "MANUAL"`.
    pub exposure_gain: Option<f32>,
    /// Manual iris value (F-number). Only meaningful when
    /// `exposure_mode == "MANUAL"`.
    pub exposure_iris: Option<f32>,
    /// Backlight compensation mode: `"OFF"` or `"ON"`.
    pub backlight_compensation: Option<String>,
    /// Autofocus mode: `"AUTO"`, `"MANUAL"`, or `"OnePush"`.
    pub focus_mode: Option<String>,
    /// Default focus speed for autofocus operations (device-native range).
    pub focus_default_speed: Option<f32>,
    /// Near-end focus limit (device-native units). Constrains autofocus search.
    pub focus_near_limit: Option<f32>,
    /// Far-end focus limit (device-native units). Constrains autofocus search.
    pub focus_far_limit: Option<f32>,
    /// Wide dynamic range mode: `"OFF"` or `"ON"`.
    pub wide_dynamic_range_mode: Option<String>,
    /// Wide dynamic range intensity level (device-native range, typically 0–100).
    pub wide_dynamic_range_level: Option<f32>,
    /// Image stabilization mode: `"OFF"`, `"ON"`, or `"Extended"`.
    pub image_stabilization_mode: Option<String>,
    /// Tone compensation mode: `"OFF"`, `"ON"`, or `"Auto"`.
    pub tone_compensation_mode: Option<String>,
}

impl ImagingSettings {
    /// Parse from a `GetImagingSettingsResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let s = resp
            .child("ImagingSettings")
            .ok_or_else(|| SoapError::missing("ImagingSettings"))?;

        let parse_f32 = |child: &str| s.child(child).and_then(|n| n.text().parse::<f32>().ok());

        let parse_nested_f32 =
            |path: &[&str]| s.path(path).and_then(|n| n.text().parse::<f32>().ok());
        let parse_nested_str = |path: &[&str]| {
            s.path(path)
                .map(|n| n.text().to_string())
                .filter(|v| !v.is_empty())
        };

        Ok(Self {
            brightness: parse_f32("Brightness"),
            color_saturation: parse_f32("ColorSaturation"),
            contrast: parse_f32("Contrast"),
            sharpness: parse_f32("Sharpness"),
            ir_cut_filter: xml_str(s, "IrCutFilter").filter(|v| !v.is_empty()),
            white_balance_mode: parse_nested_str(&["WhiteBalance", "Mode"]),
            wb_cr_gain: parse_nested_f32(&["WhiteBalance", "CrGain"]),
            wb_cb_gain: parse_nested_f32(&["WhiteBalance", "CbGain"]),
            exposure_mode: parse_nested_str(&["Exposure", "Mode"]),
            exposure_priority: parse_nested_str(&["Exposure", "Priority"]),
            exposure_time: parse_nested_f32(&["Exposure", "ExposureTime"]),
            exposure_gain: parse_nested_f32(&["Exposure", "Gain"]),
            exposure_iris: parse_nested_f32(&["Exposure", "Iris"]),
            backlight_compensation: parse_nested_str(&["BacklightCompensation", "Mode"]),
            focus_mode: parse_nested_str(&["Focus", "AutoFocusMode"]),
            focus_default_speed: parse_nested_f32(&["Focus", "DefaultSpeed"]),
            focus_near_limit: parse_nested_f32(&["Focus", "NearLimit"]),
            focus_far_limit: parse_nested_f32(&["Focus", "FarLimit"]),
            wide_dynamic_range_mode: parse_nested_str(&["WideDynamicRange", "Mode"]),
            wide_dynamic_range_level: parse_nested_f32(&["WideDynamicRange", "Level"]),
            image_stabilization_mode: parse_nested_str(&["ImageStabilization", "Mode"]),
            tone_compensation_mode: parse_nested_str(&["ToneCompensation", "Mode"]),
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
        // WhiteBalance20 sequence: Mode, CrGain, CbGain
        if self.white_balance_mode.is_some()
            || self.wb_cr_gain.is_some()
            || self.wb_cb_gain.is_some()
        {
            out.push_str("<tt:WhiteBalance>");
            if let Some(ref m) = self.white_balance_mode {
                out.push_str(&format!("<tt:Mode>{}</tt:Mode>", xml_escape(m)));
            }
            if let Some(v) = self.wb_cr_gain {
                out.push_str(&format!("<tt:CrGain>{v}</tt:CrGain>"));
            }
            if let Some(v) = self.wb_cb_gain {
                out.push_str(&format!("<tt:CbGain>{v}</tt:CbGain>"));
            }
            out.push_str("</tt:WhiteBalance>");
        }
        // Exposure20 sequence: Mode, Priority, Window, Min/MaxExposureTime,
        // Min/MaxGain, Min/MaxIris, ExposureTime, Gain, Iris. We model only
        // the fields a caller can set: Mode, Priority, ExposureTime, Gain, Iris.
        if self.exposure_mode.is_some()
            || self.exposure_priority.is_some()
            || self.exposure_time.is_some()
            || self.exposure_gain.is_some()
            || self.exposure_iris.is_some()
        {
            out.push_str("<tt:Exposure>");
            if let Some(ref m) = self.exposure_mode {
                out.push_str(&format!("<tt:Mode>{}</tt:Mode>", xml_escape(m)));
            }
            if let Some(ref p) = self.exposure_priority {
                out.push_str(&format!("<tt:Priority>{}</tt:Priority>", xml_escape(p)));
            }
            if let Some(v) = self.exposure_time {
                out.push_str(&format!("<tt:ExposureTime>{v}</tt:ExposureTime>"));
            }
            if let Some(v) = self.exposure_gain {
                out.push_str(&format!("<tt:Gain>{v}</tt:Gain>"));
            }
            if let Some(v) = self.exposure_iris {
                out.push_str(&format!("<tt:Iris>{v}</tt:Iris>"));
            }
            out.push_str("</tt:Exposure>");
        }
        if let Some(ref m) = self.backlight_compensation {
            out.push_str(&format!(
                "<tt:BacklightCompensation><tt:Mode>{}</tt:Mode></tt:BacklightCompensation>",
                xml_escape(m)
            ));
        }
        // FocusConfiguration20 sequence: AutoFocusMode, DefaultSpeed, NearLimit, FarLimit
        if self.focus_mode.is_some()
            || self.focus_default_speed.is_some()
            || self.focus_near_limit.is_some()
            || self.focus_far_limit.is_some()
        {
            out.push_str("<tt:Focus>");
            if let Some(ref m) = self.focus_mode {
                out.push_str(&format!(
                    "<tt:AutoFocusMode>{}</tt:AutoFocusMode>",
                    xml_escape(m)
                ));
            }
            if let Some(v) = self.focus_default_speed {
                out.push_str(&format!("<tt:DefaultSpeed>{v}</tt:DefaultSpeed>"));
            }
            if let Some(v) = self.focus_near_limit {
                out.push_str(&format!("<tt:NearLimit>{v}</tt:NearLimit>"));
            }
            if let Some(v) = self.focus_far_limit {
                out.push_str(&format!("<tt:FarLimit>{v}</tt:FarLimit>"));
            }
            out.push_str("</tt:Focus>");
        }
        if self.wide_dynamic_range_mode.is_some() || self.wide_dynamic_range_level.is_some() {
            out.push_str("<tt:WideDynamicRange>");
            if let Some(ref m) = self.wide_dynamic_range_mode {
                out.push_str(&format!("<tt:Mode>{}</tt:Mode>", xml_escape(m)));
            }
            if let Some(v) = self.wide_dynamic_range_level {
                out.push_str(&format!("<tt:Level>{v}</tt:Level>"));
            }
            out.push_str("</tt:WideDynamicRange>");
        }
        if let Some(ref m) = self.image_stabilization_mode {
            out.push_str(&format!(
                "<tt:ImageStabilization><tt:Mode>{}</tt:Mode></tt:ImageStabilization>",
                xml_escape(m)
            ));
        }
        if let Some(ref m) = self.tone_compensation_mode {
            out.push_str(&format!(
                "<tt:ToneCompensation><tt:Mode>{}</tt:Mode></tt:ToneCompensation>",
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
    /// Valid exposure time range in seconds (`Exposure/ExposureTime` Min/Max).
    pub exposure_time_range: Option<FloatRange>,
    /// Valid gain range in dB (`Exposure/Gain` Min/Max).
    pub gain_range: Option<FloatRange>,
    /// Valid iris range in F-number (`Exposure/Iris` Min/Max).
    pub iris_range: Option<FloatRange>,
    /// Supported auto-focus modes (e.g. `["AUTO", "MANUAL"]`) from `Focus/AFModes`.
    pub focus_af_modes: Vec<String>,
    /// Valid auto-focus speed range (`Focus/AutoFocusSpeed` Min/Max).
    pub focus_speed_range: Option<FloatRange>,
    /// Valid wide dynamic range level range (`WideDynamicRange/Level` Min/Max).
    pub wdr_level_range: Option<FloatRange>,
    /// Supported wide dynamic range modes (e.g. `["ON", "OFF"]`) from `WideDynamicRange/Mode`.
    pub wdr_modes: Vec<String>,
    /// Supported backlight compensation modes from `BacklightCompensation/Mode`.
    pub backlight_compensation_modes: Vec<String>,
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

        let parse_nested_range = |parent: Option<&XmlNode>, child: &str| {
            parent.and_then(|p| p.child(child)).map(|n| FloatRange {
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

        let exposure = opts.child("Exposure");
        let focus = opts.child("Focus");
        let wdr = opts.child("WideDynamicRange");
        let blc = opts.child("BacklightCompensation");

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
            exposure_modes: exposure
                .map(|e| {
                    e.children_named("Mode")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            exposure_time_range: parse_nested_range(exposure, "ExposureTime"),
            gain_range: parse_nested_range(exposure, "Gain"),
            iris_range: parse_nested_range(exposure, "Iris"),
            focus_af_modes: focus
                .map(|f| {
                    f.children_named("AutoFocusModes")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            focus_speed_range: parse_nested_range(focus, "DefaultSpeed"),
            wdr_level_range: parse_nested_range(wdr, "Level"),
            wdr_modes: wdr
                .map(|w| {
                    w.children_named("Mode")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            backlight_compensation_modes: blc
                .map(|b| {
                    b.children_named("Mode")
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
