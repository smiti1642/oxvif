//! Safe Session initialization metadata. Wire data remains in explicit captures.
use crate::{OnvifError, soap::SoapError, transport::TransportError, types::Capabilities};

/// Controlled failure classification for routine diagnostics; contains no wire text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum SessionErrorClass {
    /// HTTP authorization refused.
    Authentication,
    /// HTTP request deadline expired.
    Timeout,
    /// Other network, TLS or HTTP failure.
    Transport,
    /// HTTP body byte limit refused the response.
    ResponseLimit,
    /// XML byte, depth or element-count limit refused the response.
    XmlLimit,
    /// XML syntax/policy failure.
    XmlSyntax,
    /// Device returned a SOAP Fault.
    DeviceFault,
    /// Required response data was missing, invalid or incomplete.
    ResponseData,
    /// A local argument was invalid.
    InvalidArgument,
}

impl SessionErrorClass {
    pub(crate) fn from_error(error: &OnvifError) -> Self {
        match error {
            OnvifError::Transport(TransportError::Deadline) => Self::Timeout,
            OnvifError::Transport(TransportError::HttpStatus {
                status: 401 | 403, ..
            }) => Self::Authentication,
            OnvifError::Transport(TransportError::Http(error)) if error.is_timeout() => {
                Self::Timeout
            }
            OnvifError::Transport(TransportError::ResponseTooLarge { .. }) => Self::ResponseLimit,
            OnvifError::Transport(_) => Self::Transport,
            OnvifError::Soap(SoapError::ResourceLimit { .. }) => Self::XmlLimit,
            OnvifError::Soap(SoapError::XmlParse(_)) => Self::XmlSyntax,
            OnvifError::Soap(error) if error.not_authorized() => Self::Authentication,
            OnvifError::Soap(SoapError::Fault { .. }) => Self::DeviceFault,
            OnvifError::Soap(_) => Self::ResponseData,
            OnvifError::InvalidArgument(_) => Self::InvalidArgument,
            OnvifError::RecordingSearchIncomplete { .. } => Self::ResponseData,
        }
    }
}

/// Initialization stage. Counts SOAP operations, not Digest HTTP exchanges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SessionStage {
    /// Optional UTC clock read.
    ClockRead,
    /// Mandatory capability read.
    Capabilities,
    /// Optional missing-URL fallback.
    ServicesFallback,
}

/// Result of an initialization stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SessionStageOutcome {
    /// An earlier mandatory stage stopped initialization.
    NotAttempted,
    /// Disabled or unnecessary.
    Skipped,
    /// Read completed successfully.
    Succeeded,
    /// Read failed; only fallback failure is best-effort.
    Failed,
}

/// Safe result of one Session initialization stage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionStageReport {
    /// Controlled stage identity.
    pub stage: SessionStage,
    /// Stage disposition.
    pub outcome: SessionStageOutcome,
    /// Stage duration; zero for stages not performed.
    pub elapsed_ms: u64,
    /// Controlled error class; no Fault text or reqwest error string.
    pub error_class: Option<SessionErrorClass>,
    /// Known failing HTTP status only. Successful String transport results
    /// cannot distinguish HTTP 200/400/500, so their status remains unknown.
    pub http_status: Option<u16>,
}

/// Service labels used in Session URL resolution reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SessionService {
    /// Configured device management endpoint.
    Device,
    /// Media1.
    Media,
    /// Media2.
    Media2,
    /// PTZ.
    Ptz,
    /// Imaging.
    Imaging,
    /// Events.
    Events,
    /// Recording.
    Recording,
    /// Search.
    Search,
    /// Replay.
    Replay,
    /// DeviceIO.
    DeviceIo,
}

/// Source of a resolved URL, without retaining the URL itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ServiceUrlSource {
    /// Original builder device endpoint.
    Configured,
    /// GetCapabilities.
    Capabilities,
    /// GetServices fallback.
    ServicesFallback,
    /// Still unavailable.
    Missing,
}

/// Safe URL provenance; never includes userinfo, hosts or query strings.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ServiceResolution {
    /// Controlled service label.
    pub service: SessionService,
    /// Where the Session obtained this URL.
    pub source: ServiceUrlSource,
}

/// Safe projection of Session initialization, including best-effort fallback
/// failure and missing services. Serialize this report for routine diagnostics,
/// rather than the original failure cause or a raw capture.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionBuildReport {
    /// Clock, capabilities and fallback stages, always in that order.
    pub stages: Vec<SessionStageReport>,
    /// Applied UTC offset when the device supplied UTCDateTime; otherwise unknown.
    pub clock_offset_secs: Option<i64>,
    /// All Session routing services in deterministic order, with no URL text.
    pub services: Vec<ServiceResolution>,
}

impl SessionBuildReport {
    pub(crate) fn new() -> Self {
        let mut report = Self {
            stages: [
                SessionStage::ClockRead,
                SessionStage::Capabilities,
                SessionStage::ServicesFallback,
            ]
            .into_iter()
            .map(|stage| SessionStageReport {
                stage,
                outcome: SessionStageOutcome::NotAttempted,
                elapsed_ms: 0,
                error_class: None,
                http_status: None,
            })
            .collect(),
            clock_offset_secs: None,
            services: vec![ServiceResolution {
                service: SessionService::Device,
                source: ServiceUrlSource::Configured,
            }],
        };
        report.update_services(&Capabilities::default(), ServiceUrlSource::Capabilities);
        report
    }

    pub(crate) fn stage(
        &mut self,
        index: usize,
        outcome: SessionStageOutcome,
        started: std::time::Instant,
        error: Option<&OnvifError>,
    ) {
        let entry = &mut self.stages[index];
        entry.outcome = outcome;
        entry.elapsed_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        entry.error_class = error.map(SessionErrorClass::from_error);
        entry.http_status = match error {
            Some(OnvifError::Transport(TransportError::HttpStatus { status, .. })) => Some(*status),
            _ => None,
        };
    }

    pub(crate) fn skip(&mut self, index: usize) {
        self.stages[index].outcome = SessionStageOutcome::Skipped;
    }

    pub(crate) fn update_services(&mut self, caps: &Capabilities, source: ServiceUrlSource) {
        for (service, url) in [
            (SessionService::Media, &caps.media.url),
            (SessionService::Media2, &caps.media2.url),
            (SessionService::Ptz, &caps.ptz.url),
            (SessionService::Imaging, &caps.imaging.url),
            (SessionService::Events, &caps.events.url),
            (SessionService::Recording, &caps.recording.url),
            (SessionService::Search, &caps.search.url),
            (SessionService::Replay, &caps.replay.url),
            (SessionService::DeviceIo, &caps.device_io.url),
        ] {
            let resolved = if url.is_some() {
                source
            } else {
                ServiceUrlSource::Missing
            };
            if let Some(entry) = self
                .services
                .iter_mut()
                .find(|entry| entry.service == service)
            {
                if entry.source == ServiceUrlSource::Missing {
                    entry.source = resolved;
                }
            } else {
                self.services.push(ServiceResolution {
                    service,
                    source: resolved,
                });
            }
        }
    }
}

/// Mandatory initialization failure with the original typed cause and a safe report.
/// Display is controlled; inspect `error` explicitly for wire-level investigation.
#[derive(Debug, thiserror::Error)]
#[error("Session initialization failed")]
pub struct SessionBuildFailure {
    /// Original error, which can contain sensitive device/URL/Fault information.
    #[source]
    pub error: OnvifError,
    /// Routine diagnostic projection; contains no wire text.
    pub report: SessionBuildReport,
}
