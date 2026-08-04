use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── Shared primitives ─────────────────────────────────────────────────────────

/// Width × height in pixels.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Integer min/max range (inclusive).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IntRange {
    /// Lowest accepted value, inclusive.
    pub min: i32,
    /// Highest accepted value, inclusive.
    pub max: i32,
}

/// Floating-point min/max range (inclusive).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FloatRange {
    /// Lowest accepted value, inclusive.
    pub min: f32,
    /// Highest accepted value, inclusive.
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

/// Splits a whitespace-separated **attribute** value into parsed items.
///
/// Several members of `tt:VideoEncoder2ConfigurationOptions` are `xs:list`-typed
/// attributes — `tt:IntList`, `tt:FloatList` and `tt:StringAttrList` are each
/// `<xs:list itemType="…"/>` — so a single attribute carries the whole
/// collection. Reading them as repeated child elements, which this crate did
/// until 0.15, returns an empty `Vec` from every conformant device.
fn attr_list<T: std::str::FromStr>(node: &XmlNode, name: &str) -> Vec<T> {
    node.attr(name)
        .map(|v| {
            v.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a `tt:IntList` attribute the schema constrains to *exactly two* values,
/// the lower and upper bound.
///
/// Any other count is not a range the caller could act on, so it reads as
/// absent rather than as a half-guessed one.
fn attr_int_range(node: &XmlNode, name: &str) -> Option<IntRange> {
    match attr_list::<i32>(node, name)[..] {
        [min, max] => Some(IntRange { min, max }),
        _ => None,
    }
}

// ── VideoSource ───────────────────────────────────────────────────────────────

/// A physical video input channel returned by `GetVideoSources`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        resp.children_named("VideoSources")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("VideoSources/@token"))?
                    .to_string();
                Ok(Self {
                    token,
                    framerate: n
                        .child("Framerate")
                        .and_then(|f| f.text().parse().ok())
                        .unwrap_or(0.0),
                    resolution: n
                        .child("Resolution")
                        .and_then(parse_resolution)
                        .unwrap_or_default(),
                })
            })
            .collect()
    }
}

// ── VideoSourceConfiguration ──────────────────────────────────────────────────

/// Rectangular crop/position window applied to a physical video source.
///
/// Returned by `GetVideoSourceConfiguration(s)`.
/// Pass a modified copy to `SetVideoSourceConfiguration`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct VideoSourceConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    /// Token of the physical `VideoSource` this config reads from.
    pub source_token: String,
    /// Crop window within the physical sensor.
    pub bounds: SourceBounds,
}

/// Rectangular region within a video source, in pixels.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SourceBounds {
    /// Left edge of the window, in pixels from the sensor's origin.
    pub x: i32,
    /// Top edge of the window, in pixels from the sensor's origin.
    pub y: i32,
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
}

impl VideoSourceConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("Configuration/@token"))?
            .to_string();
        Ok(Self {
            token,
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
///
/// # Changed in 0.15
///
/// `max_limit` is read from the **XML attribute** `MaximumNumberOfProfiles` on
/// the `Options` element. `tt:VideoSourceConfigurationOptions` declares it as
/// `xs:attribute`; its only child *elements* are `BoundsRange`,
/// `VideoSourceTokensAvailable` and `Extension`. Earlier releases looked for a
/// child element of that name, so the field was `None` from every conformant
/// device.
///
/// The name looks like an element because it *is* one elsewhere. ONVIF declares
/// it four times, and only here and on the two `GetServiceCapabilities` profile
/// types is it an attribute:
///
/// | declared on | form | oxvif reads it as |
/// |---|---|---|
/// | `tt:VideoSourceConfigurationOptions` | attribute | `max_limit`, **here** |
/// | `tt:ProfileCapabilities` (`GetCapabilities`) | element | [`MediaCapabilities::max_profiles`] |
/// | `trt:ProfileCapabilities` (`GetServiceCapabilities`) | attribute | [`MediaProfileCapabilities::maximum_number_of_profiles`] |
/// | `tr2:ProfileCapabilities` (`GetServiceCapabilities`) | attribute | [`Media2ProfileCapabilities::maximum_number_of_profiles`] |
///
/// [`MediaCapabilities::max_profiles`]: crate::MediaCapabilities::max_profiles
/// [`MediaProfileCapabilities::maximum_number_of_profiles`]: crate::MediaProfileCapabilities::maximum_number_of_profiles
/// [`Media2ProfileCapabilities::maximum_number_of_profiles`]: crate::Media2ProfileCapabilities::maximum_number_of_profiles
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct BoundsRange {
    /// Accepted values for [`SourceBounds::x`].
    pub x_range: IntRange,
    /// Accepted values for [`SourceBounds::y`].
    pub y_range: IntRange,
    /// Accepted values for [`SourceBounds::width`].
    pub width_range: IntRange,
    /// Accepted values for [`SourceBounds::height`].
    pub height_range: IntRange,
}

impl VideoSourceConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        Ok(Self {
            // An `xs:attribute` of `tt:VideoSourceConfigurationOptions`, not a
            // child element — see the type's doc comment.
            max_limit: opts
                .attr("MaximumNumberOfProfiles")
                .and_then(|v| v.trim().parse().ok()),
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VideoEncoding {
    /// Motion JPEG — every frame independently compressed.
    Jpeg,
    /// H.264 / AVC. The default here because it is what almost every ONVIF
    /// camera streams.
    #[default]
    H264,
    /// H.265 / HEVC. Settable only through Media2 — see
    /// [`VideoEncoderConfiguration`].
    H265,
    /// An encoding string this crate does not model, kept verbatim as the
    /// device reported it (`"MPEG4"`, `"JPEG2000"`, …).
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

/// Multicast streaming configuration embedded in a video encoder configuration.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct MulticastConfiguration {
    /// Multicast IPv4 (or IPv6) group address.
    pub address: String,
    /// Destination UDP port.
    pub port: u32,
    /// IP time-to-live for multicast packets.
    pub ttl: u32,
    /// `true` if the device starts streaming automatically on boot.
    pub auto_start: bool,
}

impl MulticastConfiguration {
    /// Parse a `<tt:Multicast>` element.
    ///
    /// `tt:MulticastConfiguration` is one type, shared by the video encoder,
    /// the audio encoder and both of their Media2 counterparts, so this is
    /// written once rather than once per host type.
    pub(crate) fn from_xml(m: &XmlNode) -> Self {
        Self {
            address: m
                .path(&["Address", "IPv4Address"])
                .map(|n| n.text().to_string())
                .or_else(|| {
                    m.path(&["Address", "IPv6Address"])
                        .map(|n| n.text().to_string())
                })
                .unwrap_or_default(),
            port: xml_u32(m, "Port").unwrap_or(0),
            ttl: xml_u32(m, "TTL").unwrap_or(0),
            auto_start: m
                .child("AutoStart")
                .is_some_and(|n| n.text() == "true" || n.text() == "1"),
        }
    }

    /// Render a `<tt:Multicast>` element.
    pub(crate) fn to_xml_body(&self) -> String {
        format!(
            "<tt:Multicast>\
               <tt:Address><tt:Type>IPv4</tt:Type><tt:IPv4Address>{}</tt:IPv4Address></tt:Address>\
               <tt:Port>{}</tt:Port>\
               <tt:TTL>{}</tt:TTL>\
               <tt:AutoStart>{}</tt:AutoStart>\
             </tt:Multicast>",
            xml_escape(&self.address),
            self.port,
            self.ttl,
            self.auto_start,
        )
    }
}

/// Video codec settings for one stream, returned by `GetVideoEncoderConfiguration(s)`.
///
/// Pass a modified copy to `SetVideoEncoderConfiguration` to change resolution,
/// frame rate, bitrate, or codec profile.
///
/// # H265
///
/// The Media1 schema covers JPEG / MPEG4 / H264 only. To set an H265 encoder,
/// use the Media2 path: [`VideoEncoderConfiguration2`](crate::types::VideoEncoderConfiguration2)
/// with [`set_video_encoder_configuration_media2`](crate::OnvifClient::set_video_encoder_configuration_media2).
///
/// The Media1 [`set_video_encoder_configuration`](crate::OnvifClient::set_video_encoder_configuration)
/// rejects H265 with [`OnvifError::InvalidArgument`](crate::OnvifError::InvalidArgument).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct VideoEncoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    /// Compression format the encoder produces.
    pub encoding: VideoEncoding,
    /// Output frame size.
    pub resolution: Resolution,
    /// Encoder quality level. Valid range is device-specific; see `GetVideoEncoderConfigurationOptions`.
    pub quality: f32,
    /// Frame rate and bitrate limits. `None` when the device left the element
    /// out, which means it is not accepting rate control on this encoder.
    pub rate_control: Option<VideoRateControl>,
    /// H.264 specific settings; `None` when `encoding != H264`.
    pub h264: Option<H264Configuration>,
    /// H.265 specific settings; `None` when `encoding != H265`.
    pub h265: Option<H265Configuration>,
    /// Multicast streaming settings, if configured.
    pub multicast: Option<MulticastConfiguration>,
    /// RTSP session keep-alive timeout (ISO 8601 duration, e.g. `"PT60S"`).
    /// Required by the ONVIF schema on `SetVideoEncoderConfiguration`; preserved
    /// across a read-modify-write so strict devices don't reject the update.
    pub session_timeout: Option<String>,
    /// When `true`, the device guarantees the configured frame rate even under load.
    /// This is an XSD attribute on the configuration element, not a child.
    pub guaranteed_frame_rate: Option<bool>,
}

/// Frame rate, encoding interval, and bitrate limits.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct VideoRateControl {
    /// Maximum frames per second the encoder produces.
    pub frame_rate_limit: u32,
    /// Encode every Nth frame (1 = all frames).
    pub encoding_interval: u32,
    /// Maximum bitrate in kbps.
    pub bitrate_limit: u32,
}

/// H.264-specific codec settings.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct H264Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.264 profile: `"Baseline"`, `"Main"`, `"High"`, or `"Extended"`.
    pub profile: String,
}

/// H.265-specific codec settings.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct H265Configuration {
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: u32,
    /// H.265 profile: `"Main"`, `"Main10"`, etc.
    pub profile: String,
}

impl VideoEncoderConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("Configuration/@token"))?
            .to_string();
        Ok(Self {
            token,
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
            multicast: node
                .child("Multicast")
                .map(MulticastConfiguration::from_xml),
            session_timeout: xml_str(node, "SessionTimeout"),
            // GuaranteedFrameRate is an XSD attribute; tolerate the
            // child-element form some devices emit as a fallback.
            guaranteed_frame_rate: node
                .attr("GuaranteedFrameRate")
                .map(|s| s.to_string())
                .or_else(|| {
                    node.child("GuaranteedFrameRate")
                        .map(|n| n.text().to_string())
                })
                .map(|v| v == "true" || v == "1"),
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
        let multicast = self
            .multicast
            .as_ref()
            .map(MulticastConfiguration::to_xml_body)
            .unwrap_or_default();
        let session_timeout = match &self.session_timeout {
            Some(v) => format!("<tt:SessionTimeout>{}</tt:SessionTimeout>", xml_escape(v)),
            None => String::new(),
        };
        // GuaranteedFrameRate is an XSD attribute on Configuration, not a child.
        let gfr_attr = match self.guaranteed_frame_rate {
            Some(v) => format!(" GuaranteedFrameRate=\"{v}\""),
            None => String::new(),
        };
        // Element order follows the onvif.xsd VideoEncoderConfiguration sequence
        // (Encoding, Resolution, Quality, RateControl, H264, Multicast,
        // SessionTimeout); strict devices reject out-of-order children. Quality
        // in particular must precede RateControl.
        format!(
            "<trt:Configuration token=\"{token}\"{gfr_attr}>\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}\
               <tt:Quality>{quality}</tt:Quality>\
               {rate}{h264}{h265}{multicast}{session_timeout}\
             </trt:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            // `VideoEncoding::Other` carries the device's raw string; going
            // through `Display` would put it on the wire unescaped.
            encoding = xml_escape(self.encoding.as_str()),
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions ─────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration`.
///
/// # Where the codec blocks live
///
/// Media1 grew two extension levels, so the same codec can arrive at more than
/// one depth and the deeper copy carries *more* data:
///
/// ```text
/// Options/JPEG                      tt:JpegOptions    — no BitrateRange
/// Options/H264                      tt:H264Options    — no BitrateRange
/// Options/Extension/JPEG            tt:JpegOptions2   — adds BitrateRange
/// Options/Extension/H264            tt:H264Options2   — adds BitrateRange
/// Options/Extension/Extension/H265  tt:H265Options    — the ONLY place H265 exists
/// ```
///
/// Devices commonly send **both** copies of JPEG/H264, with the bitrate range
/// present only in the extension. The parser therefore prefers the deeper node
/// and falls back to the shallower one, so `bitrate_range` is populated
/// whichever form the device chose.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VideoEncoderConfigurationOptions {
    /// Accepted values for `VideoEncoderConfiguration::quality`.
    pub quality_range: Option<FloatRange>,
    /// JPEG options; `None` if the device does not offer JPEG.
    pub jpeg: Option<JpegOptions>,
    /// H.264 options; `None` if the device does not offer H.264.
    pub h264: Option<H264Options>,
    /// H.265 options; `None` if the device does not offer H.265.
    pub h265: Option<H265Options>,
}

/// Valid options for JPEG encoding.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JpegOptions {
    /// Frame sizes the device accepts for JPEG.
    pub resolutions: Vec<Resolution>,
    /// Accepted frame rates, in frames per second.
    pub frame_rate_range: Option<IntRange>,
    /// Accepted encoding intervals — encode every *n*-th frame. `1` encodes
    /// every frame.
    pub encoding_interval_range: Option<IntRange>,
}

/// Valid options for H.264 encoding.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct H264Options {
    /// Frame sizes the device accepts for H.264.
    pub resolutions: Vec<Resolution>,
    /// Accepted GOV lengths — frames between keyframes. Larger means smaller
    /// bitrate but slower seek and recovery.
    pub gov_length_range: Option<IntRange>,
    /// Accepted frame rates, in frames per second.
    pub frame_rate_range: Option<IntRange>,
    /// Accepted encoding intervals — encode every *n*-th frame. `1` encodes
    /// every frame.
    pub encoding_interval_range: Option<IntRange>,
    /// Accepted bitrates, in kbps.
    pub bitrate_range: Option<IntRange>,
    /// Supported H.264 profiles (e.g. `"Baseline"`, `"Main"`, `"High"`).
    pub profiles: Vec<String>,
}

/// Valid options for H.265 encoding.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct H265Options {
    /// Frame sizes the device accepts for H.265.
    pub resolutions: Vec<Resolution>,
    /// Accepted GOV lengths — frames between keyframes.
    pub gov_length_range: Option<IntRange>,
    /// Accepted frame rates, in frames per second.
    pub frame_rate_range: Option<IntRange>,
    /// Accepted encoding intervals — encode every *n*-th frame. `1` encodes
    /// every frame.
    pub encoding_interval_range: Option<IntRange>,
    /// Accepted bitrates, in kbps.
    pub bitrate_range: Option<IntRange>,
    /// Supported H.265 profiles.
    pub profiles: Vec<String>,
}

impl VideoEncoderConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;

        // Codec blocks may sit at the top level, one extension down, or (H265
        // only) two down — see the type docs. Prefer the deepest form: it is
        // the same type plus `BitrateRange`, so a device sending both copies
        // carries the bitrate only in the extension, and reading just the
        // top-level one silently drops it. The H265 fallback to a top-level
        // node is not schema-legal but costs nothing and keeps any device
        // relying on it working.
        let ext = opts.child("Extension");
        let ext2 = ext.and_then(|e| e.child("Extension"));
        let jpeg_node = ext
            .and_then(|e| e.child("JPEG"))
            .or_else(|| opts.child("JPEG"));
        let h264_node = ext
            .and_then(|e| e.child("H264"))
            .or_else(|| opts.child("H264"));
        let h265_node = ext2
            .and_then(|e| e.child("H265"))
            .or_else(|| opts.child("H265"));

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
            jpeg: jpeg_node.map(|jpeg| JpegOptions {
                resolutions: jpeg
                    .children_named("ResolutionsAvailable")
                    .filter_map(parse_resolution)
                    .collect(),
                frame_rate_range: jpeg.child("FrameRateRange").map(parse_int_range_node),
                encoding_interval_range: jpeg
                    .child("EncodingIntervalRange")
                    .map(parse_int_range_node),
            }),
            h264: h264_node.map(|h| H264Options {
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
            h265: h265_node.map(|h| H265Options {
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
///
/// # Changed in 0.15
///
/// `gov_length` and `profile` are read from, and written as, **XML attributes**
/// on the configuration element — `tt:VideoEncoder2Configuration` declares
/// `GovLength` and `Profile` as `xs:attribute`, alongside `AnchorFrameDistance`,
/// `GuaranteedFrameRate`, `Signed` and the inherited required `token`. Its only
/// child *elements* are `Name`, `UseCount`, `Encoding`, `Resolution`,
/// `RateControl`, `Multicast` and `Quality`. Earlier releases parsed and emitted
/// both as child elements, so a conformant device's values were silently
/// dropped on read and silently ignored on write.
///
/// The two names look like elements because they *are* elements elsewhere:
/// Media1 nests `GovLength` inside `tt:H264Configuration` / `tt:Mpeg4Configuration`
/// and calls the profile `H264Profile`. Media2 flattened them into attributes of
/// one type.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct VideoEncoderConfiguration2 {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles currently referencing this configuration.
    pub use_count: u32,
    /// Compression format the encoder produces. Unlike Media1, H.265 is
    /// settable here.
    pub encoding: VideoEncoding,
    /// Output frame size.
    pub resolution: Resolution,
    /// Encoder quality level. Valid range is device-specific.
    pub quality: f32,
    /// Codec-specific rate control. `None` if the device omits it.
    pub rate_control: Option<VideoRateControl2>,
    /// Group-of-pictures length (keyframe interval in frames).
    pub gov_length: Option<u32>,
    /// Codec profile (e.g. `"High"` for H.264, `"Main"` for H.265).
    pub profile: Option<String>,
}

/// Simplified rate control for Media2 (no `EncodingInterval`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct VideoRateControl2 {
    /// Maximum output frame rate, in frames per second.
    pub frame_rate_limit: u32,
    /// Maximum output bitrate, in kbps.
    pub bitrate_limit: u32,
}

impl VideoEncoderConfiguration2 {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("Configuration/@token"))?
            .to_string();
        Ok(Self {
            token,
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
            // Attributes, not child elements. `tt:VideoEncoder2Configuration`
            // declares `GovLength` and `Profile` as `xs:attribute`; the same
            // two names are child elements on Media1's `tt:H264Configuration`,
            // which is how the element form got written here in the first
            // place. Reading them with `xml_u32`/`xml_str` returned `None`
            // against every conformant device.
            gov_length: node.attr("GovLength").and_then(|v| v.trim().parse().ok()),
            profile: node
                .attr("Profile")
                .filter(|p| !p.is_empty())
                .map(str::to_string),
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
        // Attributes on the configuration element — see the type's docs. An
        // absent `Option` omits the attribute rather than writing an empty one:
        // both are `use="optional"` and a device is entitled to reject `""`.
        let gov = self
            .gov_length
            .map(|g| format!(" GovLength=\"{g}\""))
            .unwrap_or_default();
        let profile = self
            .profile
            .as_deref()
            .map(|p| format!(" Profile=\"{}\"", xml_escape(p)))
            .unwrap_or_default();
        format!(
            "<tr2:Configuration token=\"{token}\"{gov}{profile}>\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {res}{rate}\
               <tt:Quality>{quality}</tt:Quality>\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            // `VideoEncoding::Other` carries the device's raw string; going
            // through `Display` would put it on the wire unescaped.
            encoding = xml_escape(self.encoding.as_str()),
            quality = self.quality,
        )
    }
}

// ── VideoEncoderConfigurationOptions2 ────────────────────────────────────────

/// Valid parameter ranges for `SetVideoEncoderConfiguration` (Media2).
///
/// Media2 returns one [`VideoEncoderOptions2`] entry per supported encoding.
/// Match on `opts.options[i].encoding` to find the set relevant to you.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderConfigurationOptions2 {
    /// One entry per encoding type the device supports (H264, H265, JPEG, …).
    pub options: Vec<VideoEncoderOptions2>,
}

/// Per-encoding options entry within [`VideoEncoderConfigurationOptions2`].
///
/// # Changed in 0.15
///
/// `gov_length_range`, `frame_rates` and `profiles` are read from **XML
/// attributes** on the `Options` element, not from child elements.
/// `tt:VideoEncoder2ConfigurationOptions` declares exactly four child elements —
/// `Encoding`, `QualityRange`, `ResolutionsAvailable` and `BitrateRange` — and
/// everything else as `xs:attribute`. Earlier releases read all three as
/// elements, so all three came back empty or `None` from every conformant
/// device.
///
/// Two of them are `xs:list`-typed, which makes this a change of *cardinality*
/// and not only of location: `ProfilesSupported` is a `tt:StringAttrList` and
/// `FrameRatesSupported` a `tt:FloatList`, so **one** attribute carries the
/// whole space-separated collection where the parser had expected N repeated
/// elements. `GovLengthRange` is a `tt:IntList` the schema constrains to exactly
/// two values, the lower and upper bound — the same information the Media1
/// `<tt:GovLengthRange><tt:Min/><tt:Max/></tt:GovLengthRange>` element carries,
/// spelled as an attribute.
///
/// The names look like elements because on Media1 they are: `tt:H264Options`
/// declares `GovLengthRange`, `FrameRateRange` and `EncodingIntervalRange` as
/// `tt:IntRange` *elements*, and its profile list as repeated
/// `H264ProfilesSupported` elements. Media2 flattened the same facts into
/// attributes of one type. [`H264Options`] is unchanged and correct.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderOptions2 {
    /// The encoding these options apply to.
    pub encoding: VideoEncoding,
    /// Accepted values for `VideoEncoderConfiguration2::quality`.
    pub quality_range: Option<FloatRange>,
    /// Frame sizes the device accepts for this encoding.
    pub resolutions: Vec<Resolution>,
    /// Accepted bitrates, in kbps.
    pub bitrate_range: Option<IntRange>,
    /// Discrete supported target frame rates, in fps, highest first.
    ///
    /// Fractional rates such as `12.5` are ordinary here, which is why this is
    /// `f32` and not an integer type — `tt:FloatList` is a list of `xs:float`.
    pub frame_rates: Vec<f32>,
    /// Accepted GOV lengths — frames between keyframes.
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
                    // The three below are `xs:attribute`s, two of them
                    // `xs:list`-typed — see the type's doc comment. There is no
                    // `FrameRateRange` on this type at all; the discrete
                    // `FrameRatesSupported` list is what Media2 offers instead.
                    frame_rates: attr_list(opt, "FrameRatesSupported"),
                    gov_length_range: attr_int_range(opt, "GovLengthRange"),
                    profiles: attr_list(opt, "ProfilesSupported"),
                })
                .collect(),
        })
    }
}

// ── VideoEncoderInstances ─────────────────────────────────────────────────────

/// Encoder capacity info returned by `GetVideoEncoderInstances` (Media2).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct VideoEncoderInstances {
    /// Total number of encoder instances available on the source.
    pub total: u32,
    /// Per-encoding breakdown of available instances.
    pub encodings: Vec<EncoderInstanceInfo>,
}

/// Available instance count for one encoding type.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct EncoderInstanceInfo {
    /// The encoding this count applies to.
    pub encoding: VideoEncoding,
    /// How many encoder instances of that encoding are still available.
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
