use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── OsdPosition ───────────────────────────────────────────────────────────────

/// Screen position of an OSD element.
#[derive(Debug, Clone, Default)]
pub struct OsdPosition {
    /// Position type: `"UpperLeft"`, `"UpperRight"`, `"LowerLeft"`,
    /// `"LowerRight"`, or `"Custom"`.
    pub type_: String,
    /// Normalised X coordinate `[-1.0, 1.0]`, used for `"Custom"` type.
    pub x: Option<f32>,
    /// Normalised Y coordinate `[-1.0, 1.0]`, used for `"Custom"` type.
    pub y: Option<f32>,
}

impl OsdPosition {
    fn from_xml(node: &XmlNode) -> Self {
        let pos = node.child("Position");
        Self {
            type_: node
                .child("Type")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            x: pos.and_then(|p| p.attr("x")).and_then(|v| v.parse().ok()),
            y: pos.and_then(|p| p.attr("y")).and_then(|v| v.parse().ok()),
        }
    }

    pub(crate) fn to_xml_body(&self) -> String {
        let pos_el = match (self.x, self.y) {
            (Some(x), Some(y)) => format!("<tt:Pos x=\"{x}\" y=\"{y}\"/>"),
            _ => String::new(),
        };
        format!(
            "<tt:Position>\
               <tt:Type>{}</tt:Type>\
               {pos_el}\
             </tt:Position>",
            xml_escape(&self.type_)
        )
    }
}

// ── OsdColor ──────────────────────────────────────────────────────────────────

/// A color value used in OSD font and background settings.
///
/// Channels (`x`, `y`, `z`) map to YCbCr or RGB depending on the device's
/// colorspace URI. Most devices use YCbCr: X = luma, Y = Cb, Z = Cr.
#[derive(Debug, Clone, Default)]
pub struct OsdColor {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Colorspace URI (e.g. `"http://www.onvif.org/ver10/colorspace/YCbCr"`).
    pub colorspace: Option<String>,
    /// Transparency level: `0.0` = fully opaque, `1.0` = fully transparent.
    pub transparent: Option<f32>,
}

impl OsdColor {
    fn from_xml(node: &XmlNode) -> Self {
        let color = node.child("Color");
        Self {
            x: color
                .and_then(|c| c.attr("X"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            y: color
                .and_then(|c| c.attr("Y"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            z: color
                .and_then(|c| c.attr("Z"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            colorspace: color.and_then(|c| c.attr("Colorspace")).map(str::to_string),
            transparent: node
                .child("Transparent")
                .and_then(|n| n.text().parse().ok()),
        }
    }

    pub(crate) fn to_xml_body(&self) -> String {
        let cs = self
            .colorspace
            .as_deref()
            .map(|s| format!(" Colorspace=\"{}\"", xml_escape(s)))
            .unwrap_or_default();
        let transparent_el = self
            .transparent
            .map(|t| format!("<tt:Transparent>{t}</tt:Transparent>"))
            .unwrap_or_default();
        format!(
            "<tt:Color X=\"{}\" Y=\"{}\" Z=\"{}\"{cs}/>{transparent_el}",
            self.x, self.y, self.z
        )
    }
}

// ── OsdTextString ─────────────────────────────────────────────────────────────

/// Text content settings for an OSD element of type `"Text"`.
#[derive(Debug, Clone, Default)]
pub struct OsdTextString {
    /// Text type: `"Plain"`, `"Date"`, `"Time"`, or `"DateAndTime"`.
    pub type_: String,
    /// Displayed text for `"Plain"` type.
    pub plain_text: Option<String>,
    /// Date format string (e.g. `"MM/DD/YYYY"`).
    pub date_format: Option<String>,
    /// Time format string (e.g. `"HH:mm:ss"`).
    pub time_format: Option<String>,
    /// Font size in points.
    pub font_size: Option<u32>,
    /// Font color.
    pub font_color: Option<OsdColor>,
    /// Background color.
    pub background_color: Option<OsdColor>,
    /// If `true`, the text persists across device reboots.
    pub is_persistent_text: Option<bool>,
}

impl OsdTextString {
    fn from_xml(node: &XmlNode) -> Self {
        Self {
            type_: node
                .child("Type")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            plain_text: xml_str(node, "PlainText"),
            date_format: xml_str(node, "DateFormat"),
            time_format: xml_str(node, "TimeFormat"),
            font_size: xml_u32(node, "FontSize"),
            font_color: node.child("FontColor").map(OsdColor::from_xml),
            background_color: node.child("BackgroundColor").map(OsdColor::from_xml),
            is_persistent_text: node
                .child("IsPersistentText")
                .map(|n| n.text() == "true" || n.text() == "1"),
        }
    }

    pub(crate) fn to_xml_body(&self) -> String {
        let plain_el = self
            .plain_text
            .as_deref()
            .map(|t| format!("<tt:PlainText>{}</tt:PlainText>", xml_escape(t)))
            .unwrap_or_default();
        let date_el = self
            .date_format
            .as_deref()
            .map(|t| format!("<tt:DateFormat>{}</tt:DateFormat>", xml_escape(t)))
            .unwrap_or_default();
        let time_el = self
            .time_format
            .as_deref()
            .map(|t| format!("<tt:TimeFormat>{}</tt:TimeFormat>", xml_escape(t)))
            .unwrap_or_default();
        let font_el = self
            .font_size
            .map(|s| format!("<tt:FontSize>{s}</tt:FontSize>"))
            .unwrap_or_default();
        let font_color_el = self
            .font_color
            .as_ref()
            .map(|c| format!("<tt:FontColor>{}</tt:FontColor>", c.to_xml_body()))
            .unwrap_or_default();
        let bg_color_el = self
            .background_color
            .as_ref()
            .map(|c| {
                format!(
                    "<tt:BackgroundColor>{}</tt:BackgroundColor>",
                    c.to_xml_body()
                )
            })
            .unwrap_or_default();
        let persistent_el = self
            .is_persistent_text
            .map(|v| format!("<tt:IsPersistentText>{v}</tt:IsPersistentText>"))
            .unwrap_or_default();
        format!(
            "<tt:TextString>\
               <tt:Type>{}</tt:Type>\
               {plain_el}{date_el}{time_el}{font_el}\
               {font_color_el}{bg_color_el}{persistent_el}\
             </tt:TextString>",
            xml_escape(&self.type_)
        )
    }
}

// ── OsdConfiguration ──────────────────────────────────────────────────────────

/// A single on-screen display (OSD) element returned by `GetOSDs` / `GetOSD`.
///
/// Pass a modified copy to `set_osd`, or a new instance (with an empty token)
/// to `create_osd`.
#[derive(Debug, Clone)]
pub struct OsdConfiguration {
    /// Opaque token. Empty when creating a new OSD via `create_osd`.
    pub token: String,
    /// Token of the video source configuration this OSD is attached to.
    pub video_source_config_token: String,
    /// OSD type: `"Text"` or `"Image"`.
    pub type_: String,
    /// Position on screen.
    pub position: OsdPosition,
    /// Text content settings; present when `type_` is `"Text"`.
    pub text_string: Option<OsdTextString>,
    /// Image path; present when `type_` is `"Image"`.
    pub image_path: Option<String>,
}

impl OsdConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("OSDConfiguration/@token"))?
            .to_string();
        Ok(Self {
            token,
            video_source_config_token: xml_str(node, "VideoSourceConfigurationToken")
                .unwrap_or_default(),
            type_: xml_str(node, "Type").unwrap_or_default(),
            position: node
                .child("Position")
                .map(OsdPosition::from_xml)
                .unwrap_or_default(),
            text_string: node.child("TextString").map(OsdTextString::from_xml),
            image_path: node
                .path(&["Image", "ImgPath"])
                .map(|n| n.text().to_string()),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("OSDConfiguration")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<tt:OSDConfiguration>` XML fragment for
    /// `SetOSD` / `CreateOSD`.
    pub(crate) fn to_xml_body(&self) -> String {
        let token_attr = if self.token.is_empty() {
            String::new()
        } else {
            format!(" token=\"{}\"", xml_escape(&self.token))
        };
        let text_el = self
            .text_string
            .as_ref()
            .map(|t| t.to_xml_body())
            .unwrap_or_default();
        let img_el = self
            .image_path
            .as_deref()
            .map(|p| {
                format!(
                    "<tt:Image><tt:ImgPath>{}</tt:ImgPath></tt:Image>",
                    xml_escape(p)
                )
            })
            .unwrap_or_default();
        format!(
            "<tt:OSDConfiguration{token_attr}>\
               <tt:VideoSourceConfigurationToken>{vsc}</tt:VideoSourceConfigurationToken>\
               <tt:Type>{type_}</tt:Type>\
               {pos}\
               {text_el}{img_el}\
             </tt:OSDConfiguration>",
            vsc = xml_escape(&self.video_source_config_token),
            type_ = xml_escape(&self.type_),
            pos = self.position.to_xml_body(),
        )
    }
}

// ── OsdOptions ────────────────────────────────────────────────────────────────

/// Valid OSD configuration options returned by `get_osd_options`.
#[derive(Debug, Clone, Default)]
pub struct OsdOptions {
    /// Maximum number of OSDs supported by this video source configuration.
    pub max_osd: u32,
    /// Supported OSD types (e.g. `["Text", "Image"]`).
    pub types: Vec<String>,
    /// Supported position types (e.g. `["UpperLeft", "Custom"]`).
    pub position_types: Vec<String>,
    /// Supported text types (e.g. `["Plain", "Date", "DateAndTime"]`).
    pub text_types: Vec<String>,
}

impl OsdOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("OSDOptions")
            .ok_or_else(|| SoapError::missing("OSDOptions"))?;
        Ok(Self {
            max_osd: xml_u32(opts, "MaximumNumberOfOSDs").unwrap_or(0),
            types: opts
                .children_named("Type")
                .map(|n| n.text().to_string())
                .collect(),
            position_types: opts
                .child("PositionOption")
                .map(|p| {
                    p.children_named("Type")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            text_types: opts
                .child("TextOption")
                .map(|t| {
                    t.children_named("Type")
                        .map(|n| n.text().to_string())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}
