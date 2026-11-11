//! Service-qualified read workflows over the native media codecs.

use crate::{
    Media2StreamingCapabilities, MediaProfile, MediaProfile2, MediaStreamingCapabilities,
    OnvifError, OnvifSession, SnapshotUri, StreamUri, VideoEncoderConfiguration,
    VideoEncoderConfiguration2,
    session::{ServiceUrlSource, SessionErrorClass},
    soap::SoapError,
};

/// Native service that owns a profile token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum MediaService {
    /// Media1.
    Media1,
    /// Media2.
    Media2,
}

/// Explicit service selection for the read workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSelection {
    /// Only query Media1.
    Media1,
    /// Only query Media2.
    Media2,
    /// Prefer advertised Media2 and try Media1 only when absent or unsupported.
    Auto,
}

/// A profile identity in a particular service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileHandle {
    service: MediaService,
    token: String,
}

impl ProfileHandle {
    /// Service that owns the token.
    pub fn service(&self) -> MediaService {
        self.service
    }
    /// Opaque token as returned by the device.
    pub fn token(&self) -> &str {
        &self.token
    }
}

/// Profile payload in its native service type.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NativeMediaProfile {
    /// Media1 payload.
    Media1(MediaProfile),
    /// Media2 payload.
    Media2(MediaProfile2),
}

impl NativeMediaProfile {
    /// Opaque token in the profile's native service.
    pub fn token(&self) -> &str {
        match self {
            Self::Media1(p) => &p.token,
            Self::Media2(p) => &p.token,
        }
    }
    /// Service and token for subsequent reads.
    pub fn handle(&self) -> ProfileHandle {
        match self {
            Self::Media1(p) => ProfileHandle {
                service: MediaService::Media1,
                token: p.token.clone(),
            },
            Self::Media2(p) => ProfileHandle {
                service: MediaService::Media2,
                token: p.token.clone(),
            },
        }
    }
    /// Reported profile name.
    pub fn name(&self) -> &str {
        match self {
            Self::Media1(p) => &p.name,
            Self::Media2(p) => &p.name,
        }
    }
    /// Bound video encoder configuration identity.
    pub fn encoder_token(&self) -> Option<&str> {
        match self {
            Self::Media1(p) => p.video_encoder_token.as_deref(),
            Self::Media2(p) => p.video_encoder_token.as_deref(),
        }
    }
    /// Underlying video source identity when the profile reports it.
    pub fn source_token(&self) -> Option<&str> {
        match self {
            Self::Media1(p) => p.video_source_token.as_deref(),
            Self::Media2(p) => p.video_source_token.as_deref(),
        }
    }
}

/// Controlled disposition of one candidate service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum MediaAttemptOutcome {
    /// No endpoint was resolved.
    NotAdvertised,
    /// An explicit ActionNotSupported fault was returned.
    Unsupported,
    /// Profile query succeeded.
    Succeeded,
    /// An error stopped selection.
    Failed(SessionErrorClass),
}

/// Service selection evidence without URLs or raw device errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MediaAttempt {
    /// Candidate service.
    pub service: MediaService,
    /// How the Session resolved its endpoint.
    pub endpoint_source: ServiceUrlSource,
    /// Query disposition.
    pub outcome: MediaAttemptOutcome,
}

/// A successful profile catalogue and its routing evidence.
#[derive(Debug, Clone)]
pub struct MediaCatalogue {
    /// Selected service.
    pub service: MediaService,
    /// Service resolution provenance.
    pub endpoint_source: ServiceUrlSource,
    /// Profiles in device order.
    pub profiles: Vec<NativeMediaProfile>,
    /// Attempts in evaluation order.
    pub attempts: Vec<MediaAttempt>,
}

impl MediaCatalogue {
    /// Select an opaque token, or the single profile if a token was omitted.
    pub fn select(&self, token: Option<&str>) -> Result<ProfileHandle, OnvifError> {
        if token.is_none() && self.profiles.len() == 1 {
            return Ok(self.profiles[0].handle());
        }
        let mut matches = self
            .profiles
            .iter()
            .filter(|profile| token == Some(profile.token()));
        if let Some(first) = matches.next()
            && matches.next().is_none()
        {
            return Ok(first.handle());
        }
        Err(OnvifError::InvalidArgument(
            "a unique media profile token is required".into(),
        ))
    }
    /// Filter by underlying source token without conflating source/configuration identities.
    pub fn profiles_for_source<'a>(
        &'a self,
        token: &'a str,
    ) -> impl Iterator<Item = &'a NativeMediaProfile> {
        self.profiles
            .iter()
            .filter(move |p| p.source_token() == Some(token))
    }
}

/// A failed profile catalogue query, with attempts and original error.
#[derive(Debug, thiserror::Error)]
#[error("media catalogue query failed")]
pub struct MediaCatalogueError {
    /// Original typed error.
    #[source]
    pub cause: OnvifError,
    /// Service selection evidence.
    pub attempts: Vec<MediaAttempt>,
}

/// Transport requested in a stream URI query.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StreamRequest {
    /// RTP interleaved with RTSP over TCP.
    #[default]
    Rtsp,
    /// RTP over UDP unicast.
    UdpUnicast,
    /// RTP over UDP multicast.
    UdpMulticast,
    /// RTSP tunnel over HTTP.
    HttpTunnel,
}

/// Reported streaming capabilities of the selected service.
#[derive(Debug, Clone, Copy)]
pub enum StreamCapabilities<'a> {
    /// Media1 capability fields.
    Media1(&'a MediaStreamingCapabilities),
    /// Media2 capability fields.
    Media2(&'a Media2StreamingCapabilities),
}

impl StreamRequest {
    fn validate(
        self,
        service: MediaService,
        capabilities: StreamCapabilities<'_>,
    ) -> Result<(), OnvifError> {
        let explicitly_unavailable = match (service, capabilities) {
            (MediaService::Media1, StreamCapabilities::Media1(caps)) => {
                caps.no_rtsp_streaming == Some(true)
                    || (self == Self::Rtsp && caps.rtp_rtsp_tcp == Some(false))
                    || (self == Self::UdpMulticast && caps.rtp_multicast == Some(false))
            }
            (MediaService::Media2, StreamCapabilities::Media2(caps)) => {
                caps.rtsp_streaming == Some(false)
                    || (self == Self::Rtsp && caps.rtp_rtsp_tcp == Some(false))
                    || (self == Self::UdpMulticast && caps.rtp_multicast == Some(false))
            }
            _ => {
                return Err(OnvifError::InvalidArgument(
                    "stream capabilities belong to a different media service".into(),
                ));
            }
        };
        if explicitly_unavailable {
            return Err(OnvifError::InvalidArgument(
                "stream transport was reported unavailable".into(),
            ));
        }
        Ok(())
    }
}

/// URI metadata retained in its native service representation.
#[derive(Debug, Clone)]
pub enum MediaUri<T> {
    /// Media1 URI and lifetime flags.
    Media1(T),
    /// Media2 URI.
    Media2(String),
}

impl<T> MediaUri<T> {
    /// Service that returned this URI.
    pub fn service(&self) -> MediaService {
        match self {
            Self::Media1(_) => MediaService::Media1,
            Self::Media2(_) => MediaService::Media2,
        }
    }
}

impl MediaUri<StreamUri> {
    /// Stream URI as supplied by the selected service.
    pub fn uri(&self) -> &str {
        match self {
            Self::Media1(v) => &v.uri,
            Self::Media2(v) => v,
        }
    }
}

impl MediaUri<SnapshotUri> {
    /// Snapshot URI as supplied by the selected service.
    pub fn uri(&self) -> &str {
        match self {
            Self::Media1(v) => &v.uri,
            Self::Media2(v) => v,
        }
    }
}

/// Native video encoder configuration list from the selected service.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum MediaEncoders {
    /// Media1 configurations.
    Media1(Vec<VideoEncoderConfiguration>),
    /// Media2 configurations.
    Media2(Vec<VideoEncoderConfiguration2>),
}

fn unsupported(error: &OnvifError) -> bool {
    matches!(error, OnvifError::Soap(error) if error.action_not_supported())
}

impl OnvifSession {
    fn media_route(&self, service: MediaService) -> (Option<&str>, ServiceUrlSource) {
        match service {
            MediaService::Media1 => (
                self.capabilities().media.url.as_deref(),
                self.media_sources[0],
            ),
            MediaService::Media2 => (
                self.capabilities().media2.url.as_deref(),
                self.media_sources[1],
            ),
        }
    }

    fn trusted_media(&self, service: MediaService) -> Result<reqwest::Url, OnvifError> {
        let url = self
            .media_route(service)
            .0
            .ok_or_else(|| SoapError::missing("selected media service URL"))?;
        self.endpoint_policy.check(self.client().device_url(), url)
    }

    /// Read profiles using explicit service selection and same-origin routing.
    pub async fn media_catalogue(
        &self,
        selection: MediaSelection,
    ) -> Result<MediaCatalogue, Box<MediaCatalogueError>> {
        use MediaService::{Media1, Media2};
        let services: &[MediaService] = match selection {
            MediaSelection::Media1 => &[Media1],
            MediaSelection::Media2 => &[Media2],
            MediaSelection::Auto => &[Media2, Media1],
        };
        let client = self.client().direct();
        let mut attempts = Vec::new();
        let mut cause = SoapError::missing("selected media service URL").into();
        for &service in services {
            let (url, endpoint_source) = self.media_route(service);
            let outcome = if url.is_none() {
                MediaAttemptOutcome::NotAdvertised
            } else {
                let result = async {
                    let url = self.trusted_media(service)?;
                    let profiles: Vec<NativeMediaProfile> = match service {
                        Media1 => client
                            .get_profiles(url.as_str())
                            .await?
                            .into_iter()
                            .map(NativeMediaProfile::Media1)
                            .collect(),
                        Media2 => client
                            .get_profiles_media2(url.as_str())
                            .await?
                            .into_iter()
                            .map(NativeMediaProfile::Media2)
                            .collect(),
                    };
                    let mut tokens = std::collections::HashSet::new();
                    if profiles.iter().any(|p| !tokens.insert(p.token())) {
                        return Err(SoapError::InvalidValue {
                            field: "media profile token",
                            value: "duplicate".into(),
                        }
                        .into());
                    }
                    Ok(profiles)
                }
                .await;
                match result {
                    Ok(profiles) => {
                        attempts.push(MediaAttempt {
                            service,
                            endpoint_source,
                            outcome: MediaAttemptOutcome::Succeeded,
                        });
                        return Ok(MediaCatalogue {
                            service,
                            endpoint_source,
                            profiles,
                            attempts,
                        });
                    }
                    Err(error) => {
                        let outcome = if unsupported(&error) {
                            MediaAttemptOutcome::Unsupported
                        } else {
                            MediaAttemptOutcome::Failed(SessionErrorClass::from_error(&error))
                        };
                        cause = error;
                        outcome
                    }
                }
            };
            attempts.push(MediaAttempt {
                service,
                endpoint_source,
                outcome,
            });
            if matches!(outcome, MediaAttemptOutcome::Failed(_)) {
                break;
            }
        }
        Err(Box::new(MediaCatalogueError { cause, attempts }))
    }

    /// Request a stream URI using the selected native profile and wire protocol.
    pub async fn media_stream_uri(
        &self,
        profile: &ProfileHandle,
        request: StreamRequest,
    ) -> Result<MediaUri<StreamUri>, OnvifError> {
        let url = self.trusted_media(profile.service)?;
        let client = self.client().direct();
        match profile.service {
            MediaService::Media1 => {
                let (stream, protocol) = match request {
                    StreamRequest::Rtsp => ("RTP-Unicast", "RTSP"),
                    StreamRequest::UdpUnicast => ("RTP-Unicast", "UDP"),
                    StreamRequest::UdpMulticast => ("RTP-Multicast", "UDP"),
                    StreamRequest::HttpTunnel => ("RTP-Unicast", "HTTP"),
                };
                client
                    .media1_stream_uri(url.as_str(), profile.token(), stream, protocol)
                    .await
                    .map(MediaUri::Media1)
            }
            MediaService::Media2 => {
                let protocol = match request {
                    StreamRequest::Rtsp => "RTSP",
                    StreamRequest::UdpUnicast => "RtspUnicast",
                    StreamRequest::UdpMulticast => "RtspMulticast",
                    StreamRequest::HttpTunnel => "RtspOverHttp",
                };
                client
                    .media2_stream_uri(url.as_str(), profile.token(), protocol)
                    .await
                    .map(MediaUri::Media2)
            }
        }
    }

    /// Request the snapshot URI from the service that owns the profile.
    pub async fn media_snapshot_uri(
        &self,
        profile: &ProfileHandle,
    ) -> Result<MediaUri<SnapshotUri>, OnvifError> {
        let url = self.trusted_media(profile.service)?;
        let client = self.client().direct();
        match profile.service {
            MediaService::Media1 => client
                .get_snapshot_uri(url.as_str(), profile.token())
                .await
                .map(MediaUri::Media1),
            MediaService::Media2 => client
                .get_snapshot_uri_media2(url.as_str(), profile.token())
                .await
                .map(MediaUri::Media2),
        }
    }

    /// Validate explicit streaming capabilities before requesting a URI.
    /// Unreported capability fields defer to the device.
    pub async fn media_stream_uri_with_capabilities(
        &self,
        profile: &ProfileHandle,
        request: StreamRequest,
        capabilities: StreamCapabilities<'_>,
    ) -> Result<MediaUri<StreamUri>, OnvifError> {
        request.validate(profile.service, capabilities)?;
        self.media_stream_uri(profile, request).await
    }

    /// Read video encoder configurations compatible with this profile.
    pub async fn media_compatible_video_encoders(
        &self,
        profile: &ProfileHandle,
    ) -> Result<MediaEncoders, OnvifError> {
        use crate::{soap::qualified::response, types::xml_escape};
        let url = self.trusted_media(profile.service)?;
        let (ns, prefix, operation, action, response_name) = match profile.service {
            MediaService::Media1 => (
                "http://www.onvif.org/ver10/media/wsdl",
                "trt",
                "GetCompatibleVideoEncoderConfigurations",
                "http://www.onvif.org/ver10/media/wsdl/GetCompatibleVideoEncoderConfigurations",
                "GetCompatibleVideoEncoderConfigurationsResponse",
            ),
            MediaService::Media2 => (
                "http://www.onvif.org/ver20/media/wsdl",
                "tr2",
                "GetVideoEncoderConfigurations",
                "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations",
                "GetVideoEncoderConfigurationsResponse",
            ),
        };
        let body = format!(
            "<{prefix}:{operation}><{prefix}:ProfileToken>{}</{prefix}:ProfileToken></{prefix}:{operation}>",
            xml_escape(profile.token())
        );
        let xml = self
            .client()
            .direct()
            .call(url.as_str(), action, &body)
            .await?;
        let node = response(&xml, ns, response_name)?
            .legacy_projection(&[ns, "http://www.onvif.org/ver10/schema"]);
        match profile.service {
            MediaService::Media1 => {
                VideoEncoderConfiguration::vec_from_xml(&node).map(MediaEncoders::Media1)
            }
            MediaService::Media2 => {
                VideoEncoderConfiguration2::vec_from_xml(&node).map(MediaEncoders::Media2)
            }
        }
    }

    /// List encoder configurations in the selected service, for token-based joins.
    pub async fn media_video_encoders(
        &self,
        service: MediaService,
    ) -> Result<MediaEncoders, OnvifError> {
        let url = self.trusted_media(service)?;
        let client = self.client().direct();
        match service {
            MediaService::Media1 => client
                .get_video_encoder_configurations(url.as_str())
                .await
                .map(MediaEncoders::Media1),
            MediaService::Media2 => client
                .get_video_encoder_configurations_media2(url.as_str())
                .await
                .map(MediaEncoders::Media2),
        }
    }
}
