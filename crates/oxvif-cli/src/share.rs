//! Closed projection for reports intended to be shared outside the local machine.

use oxvif::{
    OnvifError,
    health::snapshot::{SnapshotError, SnapshotErrorKind},
    media_workflow::{MediaAttempt, MediaCatalogue, MediaService},
    session::{
        ServiceResolution, SessionBuildReport, SessionErrorClass, SessionStage, SessionStageOutcome,
    },
    stream_probe::StreamOutcome,
    transport::TransportError,
};
use serde::Serialize;

use crate::{AppError, ErrorCode};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Diagnose,
    ConfigValidate,
    Unknown,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Input,
    Configuration,
    Execution,
    Registry,
    IndexedSnapshots,
    OrphanedSnapshots,
    Target,
    Credentials,
    CredentialStore,
    TransportSetup,
    OnvifSession,
    ClockRead,
    Capabilities,
    ServicesFallback,
    DeviceInfo,
    MediaProfiles,
    ProfileSelection,
    StreamUri,
    SnapshotUri,
    SnapshotDestination,
    SnapshotFetch,
    SnapshotSignature,
    RtspConnection,
    RtspOptions,
    RtspDescribe,
    RtspTransport,
    RtspPlay,
    RtspFirstPacket,
    RtspTeardown,
    VideoDecode,
}

impl Stage {
    pub(crate) fn workflow(name: &str) -> Option<Self> {
        Some(match name {
            "onvif_session" => Self::OnvifSession,
            "device_info" => Self::DeviceInfo,
            "media_profiles" => Self::MediaProfiles,
            "profile_selection" => Self::ProfileSelection,
            "stream_uri" => Self::StreamUri,
            "snapshot_uri" => Self::SnapshotUri,
            "snapshot_destination" => Self::SnapshotDestination,
            "snapshot_fetch" => Self::SnapshotFetch,
            "snapshot_signature" => Self::SnapshotSignature,
            "rtsp_connection" => Self::RtspConnection,
            "rtsp_options" => Self::RtspOptions,
            "rtsp_describe" => Self::RtspDescribe,
            "rtsp_transport" => Self::RtspTransport,
            "rtsp_play" => Self::RtspPlay,
            "rtsp_first_packet" => Self::RtspFirstPacket,
            "rtsp_teardown" => Self::RtspTeardown,
            "video_decode" => Self::VideoDecode,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Failed,
    Warning,
    Unsupported,
    Blocked,
    NotEnabled,
    NotImplemented,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    InvalidInput,
    MissingTarget,
    TargetNotFound,
    ConfigurationUnavailable,
    RegistryIo,
    RegistryCorrupt,
    RegistryVersion,
    SnapshotMissing,
    ScanLimit,
    UnindexedSnapshots,
    CredentialStoreUnavailable,
    CredentialMissing,
    CredentialIncomplete,
    InvalidCa,
    TransportSetupFailed,
    Timeout,
    TransportFailure,
    RemoteAuthorizationRejected,
    ResponseLimit,
    InvalidResponse,
    SoapFault,
    Unsupported,
    NoProfiles,
    ProfileNotFound,
    ProfileRequired,
    DestinationRejected,
    InvalidImage,
    SnapshotHttpRejected,
    AuthenticationChallenge,
    RtspTls,
    RtspRefused,
    FirstPacketMissing,
    CleanupFailed,
    Cancelled,
    Unknown,
}

impl Reason {
    pub(crate) fn from_error(error: &AppError) -> Self {
        match error.code {
            ErrorCode::InvalidArgument => Self::InvalidInput,
            ErrorCode::MissingTarget => Self::MissingTarget,
            ErrorCode::DeviceNotFound | ErrorCode::ResourceNotFound => Self::TargetNotFound,
            ErrorCode::ConfigUnavailable => Self::ConfigurationUnavailable,
            ErrorCode::RegistryIo => Self::RegistryIo,
            ErrorCode::RegistryCorrupt => Self::RegistryCorrupt,
            ErrorCode::RegistryVersionUnsupported => Self::RegistryVersion,
            _ => Self::Unknown,
        }
    }

    fn action(self, stage: Stage) -> Action {
        match self {
            Self::InvalidInput => Action::CheckUsage,
            Self::MissingTarget | Self::TargetNotFound => Action::SelectTarget,
            Self::ConfigurationUnavailable | Self::RegistryIo => Action::CheckConfigAccess,
            Self::RegistryCorrupt => Action::CheckRegistryBackup,
            Self::RegistryVersion => Action::CheckRegistryVersion,
            Self::SnapshotMissing => Action::RestoreSnapshot,
            Self::ScanLimit => Action::ReviewConfigSize,
            Self::UnindexedSnapshots => Action::ReviewOrphanedSnapshots,
            Self::CredentialStoreUnavailable => Action::CheckCredentialStore,
            Self::CredentialMissing => Action::SetCredential,
            Self::CredentialIncomplete => Action::CheckCredentialPair,
            Self::InvalidCa | Self::RtspTls => Action::CheckTlsTrust,
            Self::TransportSetupFailed => Action::CheckTransportOptions,
            Self::Timeout | Self::TransportFailure => Action::CheckReachability,
            Self::RemoteAuthorizationRejected => Action::CheckRemoteAuth,
            Self::SoapFault | Self::InvalidResponse if matches!(stage, Stage::ClockRead) => {
                Action::CheckDeviceClock
            }
            Self::ResponseLimit => Action::CheckResponseLimits,
            Self::InvalidResponse | Self::SoapFault => Action::CheckFirmware,
            Self::Unsupported => Action::CheckServiceSupport,
            Self::NoProfiles | Self::ProfileNotFound | Self::ProfileRequired => {
                Action::SelectProfile
            }
            Self::DestinationRejected => Action::CheckDestination,
            Self::InvalidImage => Action::CheckSnapshotImage,
            Self::SnapshotHttpRejected => Action::CheckSnapshotHttp,
            Self::AuthenticationChallenge => Action::CheckAuthChallenge,
            Self::RtspRefused => Action::CheckRtspService,
            Self::FirstPacketMissing => Action::CheckRtspVideo,
            Self::CleanupFailed => Action::CheckRtspCleanup,
            Self::Cancelled => Action::Retry,
            Self::Unknown => Action::InspectLocally,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    CheckUsage,
    SelectTarget,
    CheckConfigAccess,
    CheckRegistryBackup,
    CheckRegistryVersion,
    RestoreSnapshot,
    ReviewConfigSize,
    ReviewOrphanedSnapshots,
    CheckCredentialStore,
    SetCredential,
    CheckCredentialPair,
    CheckTlsTrust,
    CheckTransportOptions,
    CheckReachability,
    CheckRemoteAuth,
    CheckDeviceClock,
    CheckResponseLimits,
    CheckFirmware,
    CheckServiceSupport,
    SelectProfile,
    CheckDestination,
    CheckSnapshotImage,
    CheckSnapshotHttp,
    CheckAuthChallenge,
    CheckRtspService,
    CheckRtspVideo,
    CheckRtspCleanup,
    Retry,
    InspectLocally,
}

impl Action {
    fn next_step(&self) -> &'static str {
        match self {
            Self::CheckUsage => "Check the command syntax and share-mode options with --help.",
            Self::SelectTarget => {
                "Select a saved camera from oxvif devices or specify an explicit --target."
            }
            Self::CheckConfigAccess => {
                "Use oxvif config path locally and check the configuration directory and permissions."
            }
            Self::CheckRegistryBackup => {
                "Check the local registry or indexed snapshot against a known-good backup."
            }
            Self::CheckRegistryVersion => {
                "Check which CLI version created the registry before migrating it."
            }
            Self::RestoreSnapshot => {
                "Check the indexed snapshot locally and restore its file from a known-good backup."
            }
            Self::ReviewConfigSize => {
                "Review the size and number of registry and snapshot files locally."
            }
            Self::ReviewOrphanedSnapshots => {
                "Use oxvif config validate locally to review unindexed files."
            }
            Self::CheckCredentialStore => {
                #[cfg(windows)]
                return "Check that Windows Credential Manager is available to this process.";
                #[cfg(target_os = "macos")]
                return "Unlock the login Keychain and check this process's access.";
                #[cfg(target_os = "linux")]
                return "Check that this D-Bus session has an unlocked Secret Service provider.";
                #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
                return "Check whether a supported native credential store is available.";
            }
            Self::SetCredential => "Set the camera credential locally with oxvif auth.",
            Self::CheckCredentialPair => {
                "Check locally that both a username and password were supplied."
            }
            Self::CheckTlsTrust => {
                "Check the server certificate, trust chain, hostname and supplied CA bundle locally."
            }
            Self::CheckTransportOptions => "Check the timeout and TLS settings locally.",
            Self::CheckReachability => {
                "Check camera reachability, service port and TLS trust, then retry the affected check."
            }
            Self::CheckRemoteAuth => {
                "Check the camera account permissions and configured authentication mode locally."
            }
            Self::CheckDeviceClock => {
                "Check whether GetSystemDateAndTime works and whether the camera returns valid UTC time."
            }
            Self::CheckResponseLimits => {
                "Check whether the camera response exceeds the documented limit."
            }
            Self::CheckFirmware => {
                "Check the affected service and camera firmware against known-good behavior."
            }
            Self::CheckServiceSupport => {
                "Check the advertised service and supported media profiles."
            }
            Self::SelectProfile => {
                "Run oxvif profiles locally and pass an available token with --profile."
            }
            Self::CheckDestination => {
                "Check whether the advertised endpoint violates the host or TLS destination policy."
            }
            Self::CheckSnapshotImage => {
                "Check whether the snapshot endpoint returns a supported image."
            }
            Self::CheckSnapshotHttp => "Check the snapshot endpoint and its HTTP status locally.",
            Self::CheckAuthChallenge => {
                "Check the authentication methods offered by the camera service."
            }
            Self::CheckRtspService => {
                "Check the stream profile and RTSP service configuration locally."
            }
            Self::CheckRtspVideo => "Check whether the selected stream sends video over RTSP TCP.",
            Self::CheckRtspCleanup => {
                "Check the camera's RTSP session handling before repeating the probe."
            }
            Self::Retry => "Run the check again when it can finish.",
            Self::InspectLocally => {
                "Review the failed stage locally with the ordinary diagnostic command."
            }
        }
    }
}

pub(crate) fn onvif_reason(error: &OnvifError) -> Reason {
    match error {
        OnvifError::Transport(TransportError::Deadline) => Reason::Timeout,
        OnvifError::Transport(TransportError::Http(error)) if error.is_timeout() => Reason::Timeout,
        OnvifError::Transport(TransportError::HttpStatus {
            status: 401 | 403, ..
        }) => Reason::RemoteAuthorizationRejected,
        OnvifError::Transport(TransportError::ResponseTooLarge { .. })
        | OnvifError::Soap(oxvif::soap::SoapError::ResourceLimit { .. }) => Reason::ResponseLimit,
        OnvifError::Transport(_) => Reason::TransportFailure,
        OnvifError::Soap(error) if error.not_authorized() => Reason::RemoteAuthorizationRejected,
        OnvifError::Soap(oxvif::soap::SoapError::Fault { .. }) => Reason::SoapFault,
        OnvifError::Soap(_) => Reason::InvalidResponse,
        _ => Reason::Unknown,
    }
}

pub(crate) fn snapshot_reason(error: &SnapshotError) -> Reason {
    match error.kind() {
        SnapshotErrorKind::Destination => Reason::DestinationRejected,
        SnapshotErrorKind::InvalidConfiguration => Reason::TransportSetupFailed,
        SnapshotErrorKind::Transport => Reason::TransportFailure,
        SnapshotErrorKind::Timeout => Reason::Timeout,
        SnapshotErrorKind::HttpStatus(401 | 403) => Reason::RemoteAuthorizationRejected,
        SnapshotErrorKind::HttpStatus(_) => Reason::SnapshotHttpRejected,
        SnapshotErrorKind::AuthenticationChallenge => Reason::AuthenticationChallenge,
        SnapshotErrorKind::Limit => Reason::ResponseLimit,
        SnapshotErrorKind::InvalidImage => Reason::InvalidImage,
        _ => Reason::Unknown,
    }
}

pub(crate) fn rtsp_reason(stage: &str, outcome: StreamOutcome) -> (Option<Reason>, Option<u16>) {
    let reason = match outcome {
        StreamOutcome::Passed | StreamOutcome::Skipped => return (None, None),
        StreamOutcome::AuthenticationRequired if stage == "rtsp_options" => {
            return (None, Some(401));
        }
        StreamOutcome::Refused(status) => {
            return (
                Some(if matches!(status, 401 | 403) {
                    Reason::RemoteAuthorizationRejected
                } else if stage == "rtsp_teardown" {
                    Reason::CleanupFailed
                } else {
                    Reason::RtspRefused
                }),
                Some(status),
            );
        }
        StreamOutcome::AuthenticationRequired => {
            return (Some(Reason::RemoteAuthorizationRejected), Some(401));
        }
        _ if stage == "rtsp_teardown" => Reason::CleanupFailed,
        StreamOutcome::Tls => Reason::RtspTls,
        StreamOutcome::Io => Reason::TransportFailure,
        StreamOutcome::TimedOut if stage == "rtsp_first_packet" => Reason::FirstPacketMissing,
        StreamOutcome::TimedOut => Reason::Timeout,
        StreamOutcome::DestinationRejected => Reason::DestinationRejected,
        StreamOutcome::InvalidResponse | StreamOutcome::InvalidSdp => Reason::InvalidResponse,
        StreamOutcome::Limit => Reason::ResponseLimit,
        StreamOutcome::UnsupportedCodec | StreamOutcome::UnsupportedTransport => {
            Reason::Unsupported
        }
        StreamOutcome::Cancelled => Reason::Cancelled,
    };
    (Some(reason), None)
}

#[derive(Serialize)]
struct Check {
    stage: Stage,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    SavedDevice,
    Direct,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Passed,
    Failed,
    CompletedWithWarnings,
}

pub struct Report {
    command: Command,
    selection: Option<Selection>,
    authenticated_request: Option<bool>,
    checks: Vec<Check>,
    clock_offset_secs: Option<i64>,
    services: Vec<ServiceResolution>,
    media_service: Option<MediaService>,
    media_attempts: Vec<MediaAttempt>,
    session_attempts: Option<u32>,
    options: Option<Options>,
    cancellation: Option<tokio::sync::watch::Receiver<bool>>,
}

#[derive(Serialize)]
struct Options {
    timeout_ms: u64,
    retries: u32,
    clock_sync: &'static str,
    soap_auth: &'static str,
    media_service: &'static str,
    rtsp_probe: &'static str,
    custom_ca: bool,
}

#[derive(Serialize)]
struct Document<'a> {
    kind: &'static str,
    report_version: u8,
    cli_version: &'static str,
    os: &'static str,
    architecture: &'static str,
    command: Command,
    selection: Option<&'a Selection>,
    credentials_configured: Option<bool>,
    outcome: Outcome,
    complete: bool,
    exit_code: u8,
    primary_issue: Option<&'a Check>,
    checks: &'a [Check],
    clock_offset_secs: Option<i64>,
    services: &'a [ServiceResolution],
    media_service: Option<MediaService>,
    media_attempts: &'a [MediaAttempt],
    session_attempts: Option<u32>,
    options: Option<&'a Options>,
}

impl Report {
    pub fn new(command: Command) -> Self {
        Self {
            command,
            selection: None,
            authenticated_request: None,
            checks: Vec::new(),
            clock_offset_secs: None,
            services: Vec::new(),
            media_service: None,
            media_attempts: Vec::new(),
            session_attempts: None,
            options: None,
            cancellation: None,
        }
    }

    pub fn check(&mut self, stage: Stage, status: Status, reason: Option<Reason>) {
        self.observation(stage, status, reason, None, None, None);
    }

    pub(crate) fn observation(
        &mut self,
        stage: Stage,
        status: Status,
        reason: Option<Reason>,
        elapsed_ms: Option<u64>,
        http_status: Option<u16>,
        count: Option<usize>,
    ) {
        let action = reason.map(|reason| reason.action(stage));
        let next_step = action.as_ref().map(Action::next_step);
        self.checks.push(Check {
            stage,
            status,
            reason,
            action,
            next_step,
            elapsed_ms,
            http_status,
            count,
        });
    }

    pub fn fail(&mut self, stage: Stage, reason: Reason) {
        self.check(stage, Status::Failed, Some(reason));
    }

    pub(crate) fn error(&mut self, stage: Stage, error: &AppError) {
        self.fail(stage, Reason::from_error(error));
    }

    pub(crate) fn target(&mut self, direct: bool) {
        self.selection = Some(if direct {
            Selection::Direct
        } else {
            Selection::SavedDevice
        });
    }

    pub(crate) fn credentials(&mut self, configured: bool) {
        self.authenticated_request = Some(configured);
    }

    pub(crate) fn block_camera_checks(&mut self) {
        for stage in [
            Stage::OnvifSession,
            Stage::DeviceInfo,
            Stage::MediaProfiles,
            Stage::ProfileSelection,
            Stage::StreamUri,
            Stage::SnapshotUri,
            Stage::SnapshotDestination,
            Stage::SnapshotFetch,
            Stage::SnapshotSignature,
        ] {
            self.check(stage, Status::Blocked, None);
        }
    }

    pub(crate) fn session(&mut self, report: &SessionBuildReport, connected: bool) {
        self.clock_offset_secs = report.clock_offset_secs;
        self.services.clone_from(&report.services);
        for stage in &report.stages {
            let name = match stage.stage {
                SessionStage::ClockRead => Stage::ClockRead,
                SessionStage::Capabilities => Stage::Capabilities,
                SessionStage::ServicesFallback => Stage::ServicesFallback,
            };
            let status = match stage.outcome {
                SessionStageOutcome::Succeeded => Status::Passed,
                SessionStageOutcome::Skipped => Status::NotEnabled,
                SessionStageOutcome::NotAttempted => Status::Blocked,
                SessionStageOutcome::Failed if connected => Status::Warning,
                SessionStageOutcome::Failed => Status::Failed,
            };
            let reason = stage.error_class.map(|class| match class {
                SessionErrorClass::Authentication => Reason::RemoteAuthorizationRejected,
                SessionErrorClass::Timeout => Reason::Timeout,
                SessionErrorClass::Transport => Reason::TransportFailure,
                SessionErrorClass::ResponseLimit | SessionErrorClass::XmlLimit => {
                    Reason::ResponseLimit
                }
                SessionErrorClass::XmlSyntax | SessionErrorClass::ResponseData => {
                    Reason::InvalidResponse
                }
                SessionErrorClass::DeviceFault => Reason::SoapFault,
                _ => Reason::Unknown,
            });
            self.observation(
                name,
                status,
                reason,
                Some(stage.elapsed_ms),
                stage.http_status,
                None,
            );
        }
    }

    pub(crate) fn session_attempts(&mut self, count: u32) {
        self.session_attempts = Some(count);
    }

    pub(crate) fn media(&mut self, catalogue: &MediaCatalogue) {
        self.media_service = Some(catalogue.service);
        self.media_attempts.clone_from(&catalogue.attempts);
    }

    pub fn options(&mut self, options: &crate::ExecutionOptions) {
        use crate::{ClockSyncPolicy, RtspProbeMode};
        use oxvif::{client::AuthenticationMode, media_workflow::MediaSelection};
        self.options = Some(Options {
            timeout_ms: options.timeout.as_millis().try_into().unwrap_or(u64::MAX),
            retries: options.retries,
            clock_sync: match options.clock_sync {
                ClockSyncPolicy::Auto => "auto",
                ClockSyncPolicy::Always => "always",
                ClockSyncPolicy::Never => "never",
            },
            soap_auth: match options.soap_auth {
                AuthenticationMode::Combined => "combined",
                AuthenticationMode::Digest => "digest",
                AuthenticationMode::WsSecurity => "ws_security",
            },
            media_service: match options.media_service {
                None | Some(MediaSelection::Media1) => "media1",
                Some(MediaSelection::Media2) => "media2",
                Some(MediaSelection::Auto) => "auto",
            },
            rtsp_probe: match options.rtsp_probe {
                None => "disabled",
                Some(RtspProbeMode::Describe) => "describe",
                Some(RtspProbeMode::Packet) => "packet",
            },
            custom_ca: !options.ca_certificates.is_empty(),
        });
    }

    pub fn watch_cancellation(&mut self, receiver: tokio::sync::watch::Receiver<bool>) {
        self.cancellation = Some(receiver);
    }

    pub(crate) fn cancellation(&self) -> Option<tokio::sync::watch::Receiver<bool>> {
        self.cancellation.clone()
    }

    pub fn cancel(&mut self) {
        self.check(Stage::Execution, Status::Cancelled, Some(Reason::Cancelled));
    }

    pub(crate) fn media_attempts(&mut self, attempts: &[MediaAttempt]) {
        self.media_attempts.clear();
        self.media_attempts.extend_from_slice(attempts);
    }

    pub fn write(&self, exit_code: u8, mut output: impl std::io::Write) -> std::io::Result<()> {
        let primary_issue = self
            .checks
            .iter()
            .find(|check| check.status == Status::Failed)
            .or_else(|| {
                self.checks
                    .iter()
                    .find(|check| check.status == Status::Cancelled)
            });
        let outcome = if primary_issue.is_some() || exit_code != 0 {
            Outcome::Failed
        } else if self
            .checks
            .iter()
            .any(|check| check.status == Status::Warning)
        {
            Outcome::CompletedWithWarnings
        } else {
            Outcome::Passed
        };
        let document = Document {
            kind: "shareable_diagnostic",
            report_version: 1,
            cli_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            command: self.command,
            selection: self.selection.as_ref(),
            credentials_configured: self.authenticated_request,
            outcome,
            complete: !self.checks.iter().any(|check| {
                matches!(check.status, Status::Blocked | Status::Cancelled)
                    || (check.status == Status::Failed
                        && matches!(
                            check.stage,
                            Stage::Input | Stage::Configuration | Stage::Execution
                        ))
            }),
            exit_code,
            primary_issue,
            checks: &self.checks,
            clock_offset_secs: self.clock_offset_secs,
            services: &self.services,
            media_service: self.media_service,
            media_attempts: &self.media_attempts,
            session_attempts: self.session_attempts,
            options: self.options.as_ref(),
        };
        let mut bytes = serde_json::to_vec_pretty(&document)?;
        bytes.push(b'\n');
        if bytes.len() > 64 * 1024 {
            return Err(std::io::Error::other(
                "Diagnostic report exceeds its size limit.",
            ));
        }
        output.write_all(&bytes)?;
        output.flush()
    }
}
