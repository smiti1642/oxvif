use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── Shared primitives ─────────────────────────────────────────────────────────

/// Width × height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Integer min/max range (inclusive).
#[derive(Debug, Clone, Copy, Default)]
pub struct IntRange {
    pub min: i32,
    pub max: i32,
}

/// Floating-point min/max range (inclusive).
#[derive(Debug, Clone, Copy, Default)]
pub struct FloatRange {
    pub min: f32,
    pub max: f32,
}

pub(super) fn parse_resolution(node: &XmlNode) -> Option<Resolution> {
    Some(Resolution {
        width: xml_u32(node, "Width")?,
        height: xml_u32(node, "Height")?,
    })
}

pub(super) fn parse_int_range_node(node: &XmlNode) -> IntRange {
    IntRange {
        min: node
            .child("Min")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(0),
        max: node
            .child("Max")
            .and_then(|n| n.text().parse().ok())
            .unwrap_or(0),
    }
}

// ── VideoSource ───────────────────────────────────────────────────────────────

/// A physical video input channel returned by `GetVideoSources`.
#[derive(Debug, Clone)]
pub struct VideoSource {
    /// Opaque token identifying this physical input.
    pub token: String,
    /// Maximum frame rate this input can deliver.
    pub framerate: f32,
    /// Native resolution of this input.
    pub resolution: Resolution,
}

impl VideoSource {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("VideoSources")
            .map(|n| Self {
                token: n.attr("token").unwrap_or("").to_string(),
                framerate: n
                    .child("Framerate")
                    .and_then(|f| f.text().parse().ok())
                    .unwrap_or(0.0),
                resolution: n
                    .child("Resolution")
                    .and_then(parse_resolution)
                    .unwrap_or_default(),
            })
            .collect())
    }
}

// ── VideoSourceConfiguration ──────────────────────────────────────────────────

/// Rectangular crop/position window applied to a physical video source.
///
/// Returned by `GetVideoSourceConfiguration(s)`.
/// Pass a modified copy to `SetVideoSourceConfiguration`.
#[derive(Debug, Clone)]
pub struct VideoSourceConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    /// Token of the physical `VideoSource` this config reads from.
    pub source_token: String,
    /// Crop window within the physical sensor.
    pub bounds: SourceBounds,
}

/// Rectangular region within a video source, in pixels.
#[derive(Debug, Clone, Default)]
pub struct SourceBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl VideoSourceConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            source_token: xml_str(node, "SourceToken").unwrap_or_default(),
            bounds: node
                .child("Bounds")
                .map(|b| SourceBounds {
                    x: b.attr("x").and_then(|v| v.parse().ok()).unwrap_or(0),
                    y: b.attr("y").and_then(|v| v.parse().ok()).unwrap_or(0),
                    width: b.attr("width").and_then(|v| v.parse().ok()).unwrap_or(0),
                    height: b.attr("height").and_then(|v| v.parse().ok()).unwrap_or(0),
                })
                .unwrap_or_default(),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for `SetVideoSourceConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:SourceToken>{source_token}</tt:SourceToken>\
               <tt:Bounds x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>\
             </trt:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            source_token = xml_escape(&self.source_token),
            x = self.bounds.x,
            y = self.bounds.y,
            w = self.bounds.width,
            h = self.bounds.height,
        )
    }

    /// Serialise to a `<tr2:Configuration>` XML fragment for `SetVideoSourceConfiguration` (Media2).
    pub(crate) fn to_xml_body_media2(&self) -> String {
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:SourceToken>{source_token}</tt:SourceToken>\
               <tt:Bounds x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            source_token = xml_escape(&self.source_token),
            x = self.bounds.x,
            y = self.bounds.y,
            w = self.bounds.width,
            h = self.bounds.height,
        )
    }
}

// ── VideoSourceConfigurationOptions ──────────────────────────────────────────

/// Valid parameter ranges for `SetVideoSourceConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct VideoSourceConfigurationOptions {
    /// Available video source tokens that can be referenced.
    pub source_tokens: Vec<String>,
    /// Maximum profiles that may reference a single video source configuration.
    pub max_limit: Option<u32>,
    /// Valid ranges for the `bounds` crop window.
    pub bounds_range: Option<BoundsRange>,
}

/// Valid coordinate ranges for `SourceBounds`.
#[derive(Debug, Clone, Default)]
pub struct BoundsRange {
    pub x_range: IntRange,
    pub y_range: IntRange,
    pub width_range: IntRange,
    pub height_range: IntRange,
}

impl VideoSourceConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        Ok(Self {
            max_limit: xml_u32(opts, "MaximumNumberOfProfiles"),
            bounds_range: opts.child("BoundsRange").map(|br| BoundsRange {
                x_range: br
                    .child("XRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                y_range: br
                    .child("YRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                width_range: br
                    .child("WidthRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
                height_range: br
                    .child("HeightRange")
                    .map(parse_int_range_node)
                    .unwrap_or_default(),
            }),
            source_tokens: opts
                .children_named("VideoSourceTokensAvailable")
                .map(|n| n.text().to_string())
                .collect(),
        })
    }
}

// ── VideoEncoding ─────────────────────────────────────────────────────────────

/// Video compression format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VideoEncoding {
    Jpeg,
    #[default]
    H264,
    H265,
    Other(String),
}

impl VideoEncoding {
    pub(crate) fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "JPEG" => Self::Jpeg,
            "H264" => Self::H264,
            "H265" | "H.265" => Self::H265,
            _ => Self::Other(s.to_string()),
        }
    }

    /// Returns the ONVIF wire string for this encoding (e.g. `"H264"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Jpeg => "JPEG",
            Self::H264 => "H264",
            Self::H265 => "H265",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for VideoEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── VideoEncoderConfiguration ─────────────────────────────────────────────────

/// Video codec settings for one stream, returned by `GetVideoEncoderConfiguration(s)`.
///
/// Pass a modified copy to `SetVideoEncoderConfiguration` to change resolution,
/// frame rate, bitrate, or codec profile.
#[derive(Debug, Clone)]
pub struct VideoEncoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    pub encoding: VideoEncoding,
    pub resolution: Resolution,
    /// Encoder quality level. Valid range is device-specific; see `GetVideoEncoderConfigurationOptions`.
    pub quality: f32,
    pub rate_control: Option<VideoRateControl>,
    /// H.264 specific settings; `None` when `encoding != H264`.
    pub h264: Option<H264Configuration>,
    /// H.265 specific settings; `None` when `encoding != H265`.
    pub h265: Option<H265Configuration>,
}

/// Frame rate, encoding interval, and bitrate limits.
#[derive(Debug, Clone)]
pub struct VideoRateControl {
    /// Maximum frames per second the encoder produces.
    pub frame_rate_limit: u32,
    /// Encode every Nth frame (1 = all frames).
    pub encoding_interval: u32,
    /// Maximum bitrate in kbps.
    pub bitrate_limit: u32,
}

/// H.264-specific codec settings.
#[derive(Debug, Clone)]
pub struct H264Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.264 profile: `"Baseline"`, `"Main"`, `"High"`, or `"Extended"`.
    pub profile: String,
}

/// H.265-specific codec settings.
#[derive(Debug, Clone)]
pub struct H265Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.265 profile: `"Main"`, `"Main10"`, etc.
    pub profile: String,
}

impl VideoEncoderConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            encoding: xml_str(node, "Encoding")
                .map(|s| VideoEncoding::from_str(&s))
                .unwrap_or_default(),
            resolution: node
                .child("Resolution")
                .and_then(parse_resolution)
                .unwrap_or_default(),
            quality: node
                .child("Quality")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(0.0),
            rate_control: node.child("RateControl").map(|rc| VideoRateControl {
                frame_rate_limit: xml_u32(rc, "FrameRateLimit").unwrap_or(0),
                encoding_interval: xml_u32(rc, "EncodingInterval").unwrap_or(1),
                bitrate_limit: xml_u32(rc, "BitrateLimit").unwrap_or(0),
            }),
            h264: node.child("H264").map(|n| H264Configuration {
                gov_length: xml_u32(n, "GovLength").unwrap_or(0),
                profile: xml_str(n, "H264Profile").unwrap_or_default(),
            }),
            h265: node.child("H265").map(|n| H265Configuration {
                gov_length: xml_u32(n, "GovLength").unwrap_or(0),
                profile: xml_str(n, "H265Profile").unwrap_or_default(),
            }),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for `SetVideoEncoderConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        let res = format!(
            "<tt:Resolution><tt:Width>{}</tt:Width><tt:Height>{}</tt:Height></tt:Resolution>",
            self.resolution.width, self.resolution.height
        );
        let rate = match &self.rate_control {
            Some(rc) => format!(
                "<tt:RateControl>\
                   <tt:FrameRateLimit>{}</tt:FrameRateLimit>\
                   <tt:EncodingInterval>{}</tt:EncodingInterval>\
                   <tt:BitrateLimit>{}</tt:BitrateLimit>\
                 </tt:RateControl>",
                rc.frame_rate_limit, rc.encoding_interval, rc.bitrate_limit
            ),
            None => String::new(),
        };
        let h264 = match &self.h264 {
            Some(h) => format!(
                "<tt:H264>\
                   <tt:GovLength>{}</tt:GovLength>\
                   <tt:H264Profile>{}</tt:H264Profile>\
                 </tt:H264>",
                h.gov_length,
                xml_escape(&h.profile)
            ),
            None => String::new(),
        };
        let h265 = match &self.h265 {
            Some(h) => format!(
                "<tt:H265>\
                   <tt:GovLength>{}</tt:GovLength>\
                   <tt:H265Profile>{}</tt:H265Profile>\
                 </tt:H265>",
                h.gov_length,
                xml_escape(&h.profile)
            ),
            None => String::new(),
        };
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}{rate}{h264}{h265}\
               <tt:Quality>{quality}</tt:Quality>\
             </trt:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            encoding = self.encoding,
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions ─────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration`.
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderConfigurationOptions {
    pub quality_range: Option<FloatRange>,
    pub jpeg: Option<JpegOptions>,
    pub h264: Option<H264Options>,
    pub h265: Option<H265Options>,
}

/// Valid options for JPEG encoding.
#[derive(Debug, Clone, Default)]
pub struct JpegOptions {
    pub resolutions: Vec<Resolution>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
}

/// Valid options for H.264 encoding.
#[derive(Debug, Clone, Default)]
pub struct H264Options {
    pub resolutions: Vec<Resolution>,
    pub gov_length_range: Option<IntRange>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
    pub bitrate_range: Option<IntRange>,
    /// Supported H.264 profiles (e.g. `"Baseline"`, `"Main"`, `"High"`).
    pub profiles: Vec<String>,
}

/// Valid options for H.265 encoding.
#[derive(Debug, Clone, Default)]
pub struct H265Options {
    pub resolutions: Vec<Resolution>,
    pub gov_length_range: Option<IntRange>,
    pub frame_rate_range: Option<IntRange>,
    pub encoding_interval_range: Option<IntRange>,
    pub bitrate_range: Option<IntRange>,
    /// Supported H.265 profiles.
    pub profiles: Vec<String>,
}

impl VideoEncoderConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        Ok(Self {
            quality_range: opts.child("QualityRange").map(|qr| FloatRange {
                min: qr
                    .child("Min")
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0.0),
                max: qr
                    .child("Max")
                    .and_then(|n| n.text().parse().ok())
                    .unwrap_or(0.0),
            }),
            jpeg: opts.child("JPEG").map(|jpeg| JpegOptions {
                resolutions: jpeg
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                frame_rate_range: jpeg.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: jpeg
                    .child("EncodingIntervalRange")
                    .map(parse_int_range_node),
            }),
            h264: opts.child("H264").map(|h| H264Options {
                resolutions: h
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                gov_length_range: h.child("GovLengthRange").map(parse_int_range_node),
                frame_rate_range: h.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: h.child("EncodingIntervalRange").map(parse_int_range_node),
                bitrate_range: h.child("BitrateRange").map(parse_int_range_node),
                profiles: h
                    .children_named("H264ProfilesSupported")
                    .map(|n| n.text().to_string())
                    .collect(),
            }),
            h265: opts.child("H265").map(|h| H265Options {
                resolutions: h
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                gov_length_range: h.child("GovLengthRange").map(parse_int_range_node),
                frame_rate_range: h.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: h.child("EncodingIntervalRange").map(parse_int_range_node),
                bitrate_range: h.child("BitrateRange").map(parse_int_range_node),
                profiles: h
                    .children_named("H265ProfilesSupported")
                    .map(|n| n.text().to_string())
                    .collect(),
            }),
        })
    }
}

// ── VideoEncoderConfiguration2 ────────────────────────────────────────────────

/// Video encoder configuration for Media2 — flat structure with native H.265.
///
/// Unlike [`VideoEncoderConfiguration`] (Media1), this uses a **flat** layout:
/// `gov_length` and `profile` are top-level fields, not nested under a codec
/// sub-struct. Use with `get_video_encoder_configurations_media2` and
/// `set_video_encoder_configuration_media2`.
#[derive(Debug, Clone)]
pub struct VideoEncoderConfiguration2 {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    pub encoding: VideoEncoding,
    pub resolution: Resolution,
    pub quality: f32,
    /// Codec-specific rate control. `None` if the device omits it.
    pub rate_control: Option<VideoRateControl2>,
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: Option<u32>,
    /// Codec profile (e.g. `"High"` for H.264, `"Main"` for H.265).
    pub profile: Option<String>,
}

/// Simplified rate control for Media2 (no `EncodingInterval`).
#[derive(Debug, Clone)]
pub struct VideoRateControl2 {
    pub frame_rate_limit: u32,
    pub bitrate_limit: u32,
}

impl VideoEncoderConfiguration2 {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            token: node.attr("token").unwrap_or("").to_string(),
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            encoding: xml_str(node, "Encoding")
                .map(|s| VideoEncoding::from_str(&s))
                .unwrap_or_default(),
            resolution: node
                .child("Resolution")
                .and_then(parse_resolution)
                .unwrap_or_default(),
            quality: node
                .child("Quality")
                .and_then(|n| n.text().parse().ok())
                .unwrap_or(0.0),
            rate_control: node.child("RateControl").map(|rc| VideoRateControl2 {
                frame_rate_limit: xml_u32(rc, "FrameRateLimit").unwrap_or(0),
                bitrate_limit: xml_u32(rc, "BitrateLimit").unwrap_or(0),
            }),
            gov_length: xml_u32(node, "GovLength"),
            profile: xml_str(node, "Profile"),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<tr2:Configuration>` XML fragment for `SetVideoEncoderConfiguration` (Media2).
    pub(crate) fn to_xml_body(&self) -> String {
        let res = format!(
            "<tt:Resolution><tt:Width>{}</tt:Width><tt:Height>{}</tt:Height></tt:Resolution>",
            self.resolution.width, self.resolution.height
        );
        let rate = match &self.rate_control {
            Some(rc) => format!(
                "<tt:RateControl>\
                   <tt:FrameRateLimit>{}</tt:FrameRateLimit>\
                   <tt:BitrateLimit>{}</tt:BitrateLimit>\
                 </tt:RateControl>",
                rc.frame_rate_limit, rc.bitrate_limit
            ),
            None => String::new(),
        };
        let gov = self
            .gov_length
            .map(|g| format!("<tt:GovLength>{g}</tt:GovLength>"))
            .unwrap_or_default();
        let profile = self
            .profile
            .as_deref()
            .map(|p| format!("<tt:Profile>{}</tt:Profile>", xml_escape(p)))
            .unwrap_or_default();
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}{rate}{gov}{profile}\
               <tt:Quality>{quality}</tt:Quality>\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            encoding = self.encoding,
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions2 ────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration` (Media2).
///
/// Media2 returns one [`VideoEncoderOptions2`] entry per supported encoding.
/// Match on `opts.options[i].encoding` to find the set relevant to you.
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderConfigurationOptions2 {
    /// One entry per encoding type the device supports (H264, H265, JPEG, …).
    pub options: Vec<VideoEncoderOptions2>,
}

/// Per-encoding options entry within [`VideoEncoderConfigurationOptions2`].
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderOptions2 {
    pub encoding: VideoEncoding,
    pub quality_range: Option<FloatRange>,
    pub resolutions: Vec<Resolution>,
    pub bitrate_range: Option<IntRange>,
    /// Discrete supported frame rates (may be empty if range is used instead).
    pub frame_rates: Vec<u32>,
    pub frame_rate_range: Option<IntRange>,
    pub gov_length_range: Option<IntRange>,
    /// Supported codec profiles (e.g. `"Main"`, `"High"`).
    pub profiles: Vec<String>,
}

impl VideoEncoderConfigurationOptions2 {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            options: resp
                .children_named("Options")
                .map(|opt| VideoEncoderOptions2 {
                    encoding: xml_str(opt, "Encoding")
                        .map(|s| VideoEncoding::from_str(&s))
                        .unwrap_or_default(),
                    quality_range: opt.child("QualityRange").map(|qr| FloatRange {
                        min: qr
                            .child("Min")
                            .and_then(|n| n.text().parse().ok())
                            .unwrap_or(0.0),
                        max: qr
                            .child("Max")
                            .and_then(|n| n.text().parse().ok())
                            .unwrap_or(0.0),
                    }),
                    resolutions: opt
                        .children_named("ResolutionsAvailable")
                        .filter_map(parse_resolution)
                        .collect(),
                    bitrate_range: opt.child("BitrateRange").map(parse_int_range_node),
                    frame_rates: opt
                        .children_named("FrameRatesSupported")
                        .filter_map(|n| n.text().parse().ok())
                        .collect(),
                    frame_rate_range: opt.child("FrameRateRange").map(parse_int_range_node),
                    gov_length_range: opt.child("GovLengthRange").map(parse_int_range_node),
                    profiles: opt
                        .children_named("ProfilesSupported")
                        .map(|n| n.text().to_string())
                        .collect(),
                })
                .collect(),
        })
    }
}

// ── VideoEncoderInstances ─────────────────────────────────────────────────────

/// Encoder capacity info returned by `GetVideoEncoderInstances` (Media2).
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderInstances {
    /// Total number of encoder instances available on the source.
    pub total: u32,
    /// Per-encoding breakdown of available instances.
    pub encodings: Vec<EncoderInstanceInfo>,
}

/// Available instance count for one encoding type.
#[derive(Debug, Clone)]
pub struct EncoderInstanceInfo {
    pub encoding: VideoEncoding,
    pub number: u32,
}

impl VideoEncoderInstances {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let info = resp
            .child("Info")
            .ok_or_else(|| SoapError::missing("Info"))?;
        Ok(Self {
            total: xml_u32(info, "Total").unwrap_or(0),
            encodings: info
                .children_named("Encoding")
                .map(|e| EncoderInstanceInfo {
                    encoding: xml_str(e, "Encoding")
                        .map(|s| VideoEncoding::from_str(&s))
                        .unwrap_or_default(),
                    number: xml_u32(e, "Number").unwrap_or(0),
                })
                .collect(),
        })
    }
}
