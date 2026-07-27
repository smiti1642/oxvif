use super::{xml_bool, xml_str};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── MediaProfile ──────────────────────────────────────────────────────────────

/// A single media profile returned by `GetProfiles`.
///
/// Pass `token` to [`get_stream_uri`](crate::client::OnvifClient::get_stream_uri)
/// to retrieve the RTSP URI for this profile.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct MediaProfile {
    /// Opaque identifier used in subsequent media service calls.
    pub token: String,
    /// Human-readable profile name (e.g. `"mainStream"`, `"subStream"`).
    pub name: String,
    /// `true` if the profile is fixed and cannot be deleted.
    pub fixed: bool,
    /// Token of the bound `VideoSourceConfiguration`, if any.
    pub video_source_config_token: Option<String>,
    /// Token of the underlying `VideoSource` (the `<SourceToken>` inside
    /// `VideoSourceConfiguration`). Pass this to the Imaging service.
    pub video_source_token: Option<String>,
    /// Token of the bound `VideoEncoderConfiguration`, if any.
    pub video_encoder_token: Option<String>,
    /// Token of the bound `AudioSourceConfiguration`, if any.
    pub audio_source_token: Option<String>,
    /// Token of the bound `AudioEncoderConfiguration`, if any.
    pub audio_encoder_token: Option<String>,
    /// Token of the bound `PTZConfiguration`, if any.
    pub ptz_config_token: Option<String>,
}

impl MediaProfile {
    /// Parse a single `<Profile>` node (e.g. from `CreateProfileResponse` or
    /// `GetProfileResponse`).
    pub(crate) fn from_xml(p: &XmlNode) -> Result<Self, OnvifError> {
        let token = p
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("Profile/@token"))?
            .to_string();
        let vsc = p.child("VideoSourceConfiguration");
        Ok(Self {
            token,
            fixed: p.attr("fixed") == Some("true"),
            name: xml_str(p, "Name").unwrap_or_default(),
            video_source_config_token: vsc.and_then(|n| n.attr("token")).map(str::to_string),
            video_source_token: vsc.and_then(|n| xml_str(n, "SourceToken")),
            video_encoder_token: p
                .child("VideoEncoderConfiguration")
                .and_then(|n| n.attr("token"))
                .map(str::to_string),
            audio_source_token: p
                .child("AudioSourceConfiguration")
                .and_then(|n| n.attr("token"))
                .map(str::to_string),
            audio_encoder_token: p
                .child("AudioEncoderConfiguration")
                .and_then(|n| n.attr("token"))
                .map(str::to_string),
            ptz_config_token: p
                .child("PTZConfiguration")
                .and_then(|n| n.attr("token"))
                .map(str::to_string),
        })
    }

    /// Parse all `<trt:Profiles>` children from a `GetProfilesResponse` node.
    /// Returns an empty `Vec` if the response contains no profiles.
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Profiles")
            .map(Self::from_xml)
            .collect()
    }
}

// ── StreamUri ─────────────────────────────────────────────────────────────────

/// RTSP stream URI returned by `GetStreamUri`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct StreamUri {
    /// The RTSP URI to open with a media player (e.g. `rtsp://…/stream`).
    pub uri: String,
    /// If `true`, the URI becomes invalid after the first RTSP connection.
    pub invalid_after_connect: bool,
    /// If `true`, the URI becomes invalid after the device reboots.
    pub invalid_after_reboot: bool,
    /// ISO 8601 duration until the URI expires (e.g. `"PT0S"` = no expiry).
    pub timeout: String,
}

impl StreamUri {
    /// Parse from a `GetStreamUriResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let media_uri = resp
            .child("MediaUri")
            .ok_or_else(|| SoapError::missing("MediaUri"))?;

        let uri = media_uri
            .child("Uri")
            .map(|n| n.text().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SoapError::missing("Uri"))?;

        Ok(Self {
            uri,
            invalid_after_connect: xml_bool(media_uri, "InvalidAfterConnect"),
            invalid_after_reboot: xml_bool(media_uri, "InvalidAfterReboot"),
            timeout: xml_str(media_uri, "Timeout").unwrap_or_default(),
        })
    }
}

// ── SnapshotUri ───────────────────────────────────────────────────────────────

/// HTTP snapshot URI returned by `GetSnapshotUri`.
///
/// Fetch the URI with any HTTP client to retrieve a JPEG image.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct SnapshotUri {
    /// HTTP URL of the JPEG snapshot endpoint.
    pub uri: String,
    /// If `true`, the URI becomes invalid after the first HTTP request.
    pub invalid_after_connect: bool,
    /// If `true`, the URI becomes invalid after the device reboots.
    pub invalid_after_reboot: bool,
    /// ISO 8601 duration until the URI expires (e.g. `"PT0S"` = no expiry).
    pub timeout: String,
}

impl SnapshotUri {
    /// Parse from a `GetSnapshotUriResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let media_uri = resp
            .child("MediaUri")
            .ok_or_else(|| SoapError::missing("MediaUri"))?;

        let uri = media_uri
            .child("Uri")
            .map(|n| n.text().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SoapError::missing("Uri"))?;

        Ok(Self {
            uri,
            invalid_after_connect: xml_bool(media_uri, "InvalidAfterConnect"),
            invalid_after_reboot: xml_bool(media_uri, "InvalidAfterReboot"),
            timeout: xml_str(media_uri, "Timeout").unwrap_or_default(),
        })
    }
}

// ── MediaProfile2 ─────────────────────────────────────────────────────────────

/// A Media2 profile returned by `GetProfiles` (Media2).
///
/// Compared with [`MediaProfile`], this carries optional references to the
/// configurations currently bound to the profile.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct MediaProfile2 {
    /// Opaque profile token; the handle passed to every other Media2 call.
    pub token: String,
    /// Human-readable profile name (e.g. `"MainStream"`).
    pub name: String,
    /// `true` for a factory profile the device will not let you delete or
    /// reconfigure.
    pub fixed: bool,
    /// Token of the bound `VideoSourceConfiguration`, if any.
    pub video_source_config_token: Option<String>,
    /// Token of the underlying `VideoSource` (the `<SourceToken>` inside
    /// `VideoSourceConfiguration`). Pass this to the Imaging service.
    pub video_source_token: Option<String>,
    /// Token of the bound `VideoEncoderConfiguration2`, if any.
    pub video_encoder_token: Option<String>,
    /// Token of the bound `AudioSourceConfiguration`, if any.
    pub audio_source_token: Option<String>,
    /// Token of the bound `AudioEncoderConfiguration`, if any.
    pub audio_encoder_token: Option<String>,
    /// Token of the bound `PTZConfiguration`, if any.
    pub ptz_config_token: Option<String>,
}

impl MediaProfile2 {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Profiles")
            .map(|p| {
                let token = p
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("Profile/@token"))?
                    .to_string();
                let vsc = p.path(&["Configurations", "VideoSource"]);
                Ok(Self {
                    token,
                    name: xml_str(p, "Name").unwrap_or_default(),
                    fixed: p.attr("fixed") == Some("true"),
                    video_source_config_token: vsc
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
                    video_source_token: vsc.and_then(|n| xml_str(n, "SourceToken")),
                    video_encoder_token: p
                        .path(&["Configurations", "VideoEncoder"])
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
                    audio_source_token: p
                        .path(&["Configurations", "AudioSource"])
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
                    audio_encoder_token: p
                        .path(&["Configurations", "Audio"])
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
                    ptz_config_token: p
                        .path(&["Configurations", "PTZ"])
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
                })
            })
            .collect()
    }
}

// ── MetadataConfiguration ─────────────────────────────────────────────────────

/// Metadata stream configuration returned by `GetMetadataConfigurations` (Media2).
///
/// ONVIF Media2 WSDL — Profile T §7.14/§7.15 (conditional).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct MetadataConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    /// Whether analytics events are embedded in the metadata stream.
    pub analytics: bool,
    /// ONVIF `tt:PTZFilter/Status` — include PTZ move-status in metadata.
    pub ptz_status: bool,
    /// ONVIF `tt:PTZFilter/Position` — include PTZ position in metadata.
    pub ptz_position: bool,
    /// Multicast settings, if any.
    pub multicast_address: Option<String>,
    /// Multicast destination port, paired with `multicast_address`.
    pub multicast_port: Option<u32>,
}

impl MetadataConfiguration {
    pub(crate) fn from_xml(n: &XmlNode) -> Result<Self, OnvifError> {
        let token = n
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("MetadataConfiguration/@token"))?
            .to_string();
        let ptz = n.child("PTZStatus");
        Ok(Self {
            token,
            name: xml_str(n, "Name").unwrap_or_default(),
            use_count: n
                .child("UseCount")
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0),
            analytics: xml_bool(n, "Analytics"),
            // tt:PTZFilter — ONVIF schema defines Status + Position
            ptz_status: ptz.is_some_and(|p| xml_bool(p, "Status")),
            ptz_position: ptz.is_some_and(|p| xml_bool(p, "Position")),
            multicast_address: n
                .path(&["Multicast", "Address", "IPv4Address"])
                .map(|a| a.text().to_string()),
            multicast_port: n
                .path(&["Multicast", "Port"])
                .and_then(|p| p.text().parse().ok()),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    pub(crate) fn to_xml_body(&self) -> String {
        use super::xml_escape;
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Analytics>{analytics}</tt:Analytics>\
               <tt:PTZStatus>\
                 <tt:Status>{status}</tt:Status>\
                 <tt:Position>{pos}</tt:Position>\
               </tt:PTZStatus>\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            analytics = self.analytics,
            status = self.ptz_status,
            pos = self.ptz_position,
        )
    }
}

// ── MetadataConfigurationOptions ──────────────────────────────────────────────

/// Valid ranges for metadata configuration returned by
/// `GetMetadataConfigurationOptions` (Media2).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct MetadataConfigurationOptions {
    /// The device accepts a PTZ status filter — i.e. `ptz_status` /
    /// `ptz_position` on [`MetadataConfiguration`] are settable.
    pub ptz_status_filter_supported: bool,
    /// The device can embed analytics events in the metadata stream.
    pub analytics_supported: bool,
}

impl MetadataConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp.child("Options").unwrap_or(resp);
        Ok(Self {
            ptz_status_filter_supported: opts.child("PTZStatusFilterOptions").is_some(),
            analytics_supported: opts
                .path(&["Extension", "AnalyticsSupported"])
                .is_some_and(|n| n.text() == "true" || n.text() == "1"),
        })
    }
}

// ── AudioDecoderConfiguration ─────────────────────────────────────────────────

/// Audio decoder configuration for backchannel (audio output) returned by
/// `GetAudioDecoderConfigurations` (Media2).
///
/// ONVIF Media2 WSDL — Profile T §8.13 (conditional).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct AudioDecoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
}

impl AudioDecoderConfiguration {
    pub(crate) fn from_xml(n: &XmlNode) -> Result<Self, OnvifError> {
        let token = n
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("AudioDecoderConfiguration/@token"))?
            .to_string();
        Ok(Self {
            token,
            name: xml_str(n, "Name").unwrap_or_default(),
            use_count: n
                .child("UseCount")
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }
}

// ── AudioOutputConfiguration ──────────────────────────────────────────────────

/// Audio output configuration returned by `GetAudioOutputConfigurations` (Media2).
///
/// ONVIF Media2 WSDL — Profile T §8.13 (conditional).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct AudioOutputConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    /// Token of the physical `AudioOutput` (speaker) this config drives.
    pub output_token: String,
    /// Output volume, `0`–`100`. `None` when the device does not report one.
    pub output_level: Option<u32>,
}

impl AudioOutputConfiguration {
    pub(crate) fn from_xml(n: &XmlNode) -> Result<Self, OnvifError> {
        let token = n
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("AudioOutputConfiguration/@token"))?
            .to_string();
        Ok(Self {
            token,
            name: xml_str(n, "Name").unwrap_or_default(),
            use_count: n
                .child("UseCount")
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0),
            output_token: xml_str(n, "OutputToken").unwrap_or_default(),
            output_level: n.child("OutputLevel").and_then(|c| c.text().parse().ok()),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }
}

// ── VideoSourceMode ───────────────────────────────────────────────────────────

/// A video source mode returned by `GetVideoSourceModes` (Media2).
///
/// ONVIF Media2 WSDL — Profile T §8.7 (conditional).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct VideoSourceMode {
    /// Opaque token identifying this mode; pass it to `SetVideoSourceMode`.
    pub token: String,
    /// Highest frame rate, in frames per second, available in this mode.
    pub max_framerate: f32,
    /// Width in pixels of the largest resolution this mode offers.
    pub max_resolution_width: u32,
    /// Height in pixels of the largest resolution this mode offers.
    pub max_resolution_height: u32,
    /// Video encodings usable in this mode, as the device spelled them
    /// (`"H264"`, `"JPEG"`, …).
    pub encodings: Vec<String>,
    /// `true` if switching to this mode reboots the device — the stream drops
    /// and the device is unreachable until it comes back.
    pub reboot: bool,
}

impl VideoSourceMode {
    pub(crate) fn from_xml(n: &XmlNode) -> Result<Self, OnvifError> {
        let token = n
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("VideoSourceMode/@token"))?
            .to_string();
        Ok(Self {
            token,
            max_framerate: n
                .child("MaxFramerate")
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0.0),
            max_resolution_width: n
                .path(&["MaxResolution", "Width"])
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0),
            max_resolution_height: n
                .path(&["MaxResolution", "Height"])
                .and_then(|c| c.text().parse().ok())
                .unwrap_or(0),
            encodings: n
                .child("Encodings")
                .map(|e| e.text().split_whitespace().map(str::to_string).collect())
                .unwrap_or_default(),
            reboot: xml_bool(n, "Reboot"),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("VideoSourceModes")
            .map(Self::from_xml)
            .collect()
    }
}
