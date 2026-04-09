use super::{xml_bool, xml_str};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── MediaProfile ──────────────────────────────────────────────────────────────

/// A single media profile returned by `GetProfiles`.
///
/// Pass `token` to [`get_stream_uri`](crate::client::OnvifClient::get_stream_uri)
/// to retrieve the RTSP URI for this profile.
#[derive(Debug, Clone)]
pub struct MediaProfile {
    /// Opaque identifier used in subsequent media service calls.
    pub token: String,
    /// Human-readable profile name (e.g. `"mainStream"`, `"subStream"`).
    pub name: String,
    /// `true` if the profile is fixed and cannot be deleted.
    pub fixed: bool,
    /// Token of the bound `VideoSourceConfiguration`, if any.
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
        Ok(Self {
            token,
            fixed: p.attr("fixed") == Some("true"),
            name: xml_str(p, "Name").unwrap_or_default(),
            video_source_token: p
                .child("VideoSourceConfiguration")
                .and_then(|n| n.attr("token"))
                .map(str::to_string),
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
#[derive(Debug, Clone)]
pub struct MediaProfile2 {
    pub token: String,
    pub name: String,
    pub fixed: bool,
    /// Token of the bound `VideoSourceConfiguration`, if any.
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
                Ok(Self {
                    token,
                    name: xml_str(p, "Name").unwrap_or_default(),
                    fixed: p.attr("fixed") == Some("true"),
                    video_source_token: p
                        .path(&["Configurations", "VideoSource"])
                        .and_then(|n| n.attr("token"))
                        .map(str::to_string),
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
#[derive(Debug, Clone)]
pub struct MetadataConfiguration {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    /// Whether analytics events are embedded in the metadata stream.
    pub analytics: bool,
    /// PTZ status delivery via metadata stream.
    pub ptz_status_position: bool,
    pub ptz_status_move_status: bool,
    /// Multicast settings, if any.
    pub multicast_address: Option<String>,
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
            ptz_status_position: ptz.is_some_and(|p| xml_bool(p, "Position")),
            ptz_status_move_status: ptz.is_some_and(|p| xml_bool(p, "MoveStatus")),
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
                 <tt:Position>{pos}</tt:Position>\
                 <tt:MoveStatus>{ms}</tt:MoveStatus>\
               </tt:PTZStatus>\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            analytics = self.analytics,
            pos = self.ptz_status_position,
            ms = self.ptz_status_move_status,
        )
    }
}

// ── MetadataConfigurationOptions ──────────────────────────────────────────────

/// Valid ranges for metadata configuration returned by
/// `GetMetadataConfigurationOptions` (Media2).
#[derive(Debug, Clone)]
pub struct MetadataConfigurationOptions {
    pub ptz_status_filter_supported: bool,
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
#[derive(Debug, Clone)]
pub struct AudioDecoderConfiguration {
    pub token: String,
    pub name: String,
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
#[derive(Debug, Clone)]
pub struct AudioOutputConfiguration {
    pub token: String,
    pub name: String,
    pub use_count: u32,
    pub output_token: String,
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
#[derive(Debug, Clone)]
pub struct VideoSourceMode {
    pub token: String,
    pub max_framerate: f32,
    pub max_resolution_width: u32,
    pub max_resolution_height: u32,
    pub encodings: Vec<String>,
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
