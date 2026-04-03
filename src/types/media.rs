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
