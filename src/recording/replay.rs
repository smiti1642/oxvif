//! Bounded ONVIF Replay over one RTSP 1.0 TCP interleaved video track.

mod media;
mod transport;

use std::{future::Future, time::Duration};

use time::OffsetDateTime;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{Instant, timeout_at},
};

use crate::{OnvifSession, scoped_rpc::token, stream_probe::StreamOutcome};

use super::RecordingTimestamp;
use transport::{Delivery, Driver, PublishedPacket, open, run};

/// A forward playback range on the recorder's UTC timeline.
#[derive(Clone, Debug)]
pub struct ReplayRequest {
    /// Requested start time.
    pub start: RecordingTimestamp,
    /// Optional end time, strictly later than the start.
    pub end: Option<RecordingTimestamp>,
}

impl From<RecordingTimestamp> for ReplayRequest {
    /// Request forward replay from an absolute time.
    fn from(start: RecordingTimestamp) -> Self {
        Self { start, end: None }
    }
}

/// Budgets and trust configuration for a Replay connection.
#[derive(Clone)]
pub struct ReplayOptions {
    /// Total budget for URI lookup, RTSP negotiation and first media evidence.
    pub startup_timeout: Duration,
    /// Budget for a seek or keep-alive exchange, including seek media evidence.
    pub request_timeout: Duration,
    /// End the observation after this long without accepted media.
    pub idle_timeout: Duration,
    /// Independent budget for TEARDOWN.
    pub teardown_timeout: Duration,
    /// Maximum number of buffered media packets.
    pub max_buffered_packets: usize,
    /// Maximum number of buffered media bytes.
    pub max_buffered_bytes: usize,
    /// Additional PEM roots for RTSPS. System roots are also used.
    pub ca_certificates: Vec<Vec<u8>>,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(15),
            teardown_timeout: Duration::from_secs(1),
            max_buffered_packets: 128,
            max_buffered_bytes: 4 * 1024 * 1024,
            ca_certificates: Vec::new(),
        }
    }
}

impl ReplayOptions {
    fn validate(&self) -> bool {
        [
            self.startup_timeout,
            self.request_timeout,
            self.idle_timeout,
            self.teardown_timeout,
        ]
        .into_iter()
        .all(|duration| !duration.is_zero() && duration <= Duration::from_secs(86_400))
            && (1..=4096).contains(&self.max_buffered_packets)
            && (1..=64 * 1024 * 1024).contains(&self.max_buffered_bytes)
    }
}

/// Negotiated single H.264 video track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayTrack {
    /// Video RTP payload type.
    pub payload_type: u8,
    /// SDP format parameters for this payload, without the payload number.
    pub fmtp: Option<String>,
}

/// Evidence for the first observed access unit of a playback position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayStart {
    /// Requested absolute starting time.
    pub requested_at: OffsetDateTime,
    /// Start reported in the PLAY response's clock Range, when parseable.
    pub server_reported_at: Option<OffsetDateTime>,
    /// UTC time from the first correlated RTP Replay extension.
    pub observed_at: OffsetDateTime,
    /// The recorder marked that access unit as a synchronization point.
    pub clean_point: bool,
}

/// A gap bounded by two observed sections of the same recording track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordingGap {
    /// Timestamp of the last access unit of the preceding section.
    pub previous_section_ended_at: OffsetDateTime,
    /// Timestamp of the synchronization point that starts the next section.
    pub next_section_started_at: OffsetDateTime,
}

/// Replay timing and continuity markers on the start of an access unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayAccessUnit {
    /// UTC timestamp of the original recording.
    pub recorded_at: OffsetDateTime,
    /// The device signalled a synchronization point.
    pub clean_point: bool,
    /// The device signalled the end of the current recording section.
    pub section_end: bool,
    /// The device signalled a discontinuity in transmission.
    pub discontinuity: bool,
    /// The device signalled a terminal access unit.
    pub terminal: bool,
}

/// One RTP packet on the negotiated track.
pub struct ReplayPacket {
    /// Complete RTP packet, including the header and any Replay extension.
    pub bytes: Vec<u8>,
    /// Timestamp and flags on the first packet of this access unit.
    pub access_unit: Option<ReplayAccessUnit>,
    /// The gap immediately before the new access unit, when independently evidenced.
    pub gap_before: Option<RecordingGap>,
    /// Sequence or SSRC discontinuity, including unusable preceding access-unit data.
    pub transport_discontinuity: bool,
}

impl std::fmt::Debug for ReplayPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplayPacket")
            .field("bytes", &self.bytes.len())
            .field("access_unit", &self.access_unit)
            .field("gap_before", &self.gap_before)
            .field("transport_discontinuity", &self.transport_discontinuity)
            .finish()
    }
}

/// The stage where a replay operation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayStage {
    /// Validate local replay inputs.
    Request,
    /// Resolve the SOAP Replay service and URI.
    ReplayUri,
    /// Validate the media destination and establish a connection.
    Connect,
    /// Describe and select a video track.
    Describe,
    /// Negotiate transport and a session.
    Setup,
    /// Send the first PLAY.
    Play,
    /// Wait for initial or continuing media.
    Media,
    /// Move an established session to a different time.
    Seek,
    /// Maintain the active session.
    KeepAlive,
}

/// Secret-free classification of a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayErrorKind {
    /// Invalid options, token, or time range.
    InvalidRequest,
    /// No Replay SOAP endpoint was resolved.
    MissingReplayService,
    /// The SOAP call to resolve the URI failed.
    ReplayUri,
    /// SOAP or RTSP destination failed the trust policy.
    DestinationRejected,
    /// RTSP authentication was required or rejected.
    AuthenticationRequired,
    /// The selected transport or codec is outside the supported scope.
    UnsupportedMedia,
    /// The device explicitly rejected the ONVIF Replay feature tag.
    UnsupportedReplay,
    /// The RTSP server refused the operation with this status.
    Refused(u16),
    /// Correlation, framing, SDP or replay extension was invalid.
    InvalidResponse,
    /// Network I/O failed or the connection closed unexpectedly.
    Io,
    /// TLS handshake or certificate validation failed.
    Tls,
    /// An RTSP protocol or buffering budget was exceeded.
    Limit,
    /// The requested position could not be verified from RTP.
    PositionUnconfirmed,
    /// The operation's time budget expired.
    TimedOut,
    /// The consumer could not keep up within the buffer budget.
    SlowConsumer,
    /// The caller cancelled the operation.
    Cancelled,
    /// The local session has already ended.
    Closed,
}

impl From<StreamOutcome> for ReplayErrorKind {
    fn from(outcome: StreamOutcome) -> Self {
        match outcome {
            StreamOutcome::AuthenticationRequired => Self::AuthenticationRequired,
            StreamOutcome::DestinationRejected => Self::DestinationRejected,
            StreamOutcome::Refused(status) => Self::Refused(status),
            StreamOutcome::Limit => Self::Limit,
            StreamOutcome::Io => Self::Io,
            StreamOutcome::Tls => Self::Tls,
            StreamOutcome::TimedOut => Self::TimedOut,
            StreamOutcome::Cancelled => Self::Cancelled,
            StreamOutcome::UnsupportedCodec | StreamOutcome::UnsupportedTransport => {
                Self::UnsupportedMedia
            }
            _ => Self::InvalidResponse,
        }
    }
}

/// Independent disposition of the RTSP session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayCleanup {
    /// No remote cleanup was needed for this operation.
    NotNeeded,
    /// The server acknowledged TEARDOWN.
    Released,
    /// Remote release could not be confirmed.
    Unconfirmed,
    /// TEARDOWN was attempted and failed.
    Failed(ReplayErrorKind),
}

/// A replay failed. The error carries no device strings or credentials.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("recording replay failed at {stage:?}: {kind:?}")]
pub struct ReplayFailure {
    /// Operation that failed.
    pub stage: ReplayStage,
    /// Classification of the primary failure.
    pub kind: ReplayErrorKind,
    /// First observed media evidence, if failure followed a confirmed position.
    pub start: Option<ReplayStart>,
    /// Independent cleanup result.
    pub cleanup: ReplayCleanup,
}

/// Why the session stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayEnd {
    /// Explicit close.
    Closed,
    /// Cooperative cancellation.
    Cancelled,
    /// Terminal media was observed before the session closed or went idle.
    EndOfFootage,
    /// The session could no longer deliver trustworthy media.
    Failed {
        /// Failing stage.
        stage: ReplayStage,
        /// Primary failure.
        kind: ReplayErrorKind,
    },
}

/// Terminal state retained independently of the media queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayStop {
    /// Why delivery stopped.
    pub end: ReplayEnd,
    /// Latest confirmed position.
    pub start: Option<ReplayStart>,
    /// RTSP teardown result.
    pub cleanup: ReplayCleanup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopRequest {
    Running,
    Close,
    Cancel,
    Abandon,
}

/// Cooperative cancellation for the established session, including a pending seek.
#[derive(Clone)]
pub struct ReplayCancellation(watch::Sender<StopRequest>);

impl ReplayCancellation {
    /// Request bounded remote cleanup.
    pub fn cancel(&self) {
        request_stop(&self.0, StopRequest::Cancel);
    }
}

fn request_stop(sender: &watch::Sender<StopRequest>, next: StopRequest) {
    sender.send_if_modified(|state| {
        if *state != StopRequest::Running {
            return false;
        }
        *state = next;
        true
    });
}

async fn stopped(receiver: &mut watch::Receiver<StopRequest>) -> StopRequest {
    match receiver
        .wait_for(|state| *state != StopRequest::Running)
        .await
    {
        Ok(state) => *state,
        Err(_) => StopRequest::Abandon,
    }
}

struct Seek {
    range: transport::Range,
    delivery: Delivery,
    result: oneshot::Sender<ReplayStart>,
}

/// Owner of a single Replay session. Use `close` to observe remote cleanup.
pub struct ReplaySession {
    track: ReplayTrack,
    media: mpsc::Receiver<PublishedPacket>,
    seek: mpsc::Sender<Seek>,
    stop: watch::Sender<StopRequest>,
    stopped: watch::Receiver<Option<ReplayStop>>,
    limits: transport::BufferLimits,
    pending: Option<oneshot::Receiver<ReplayStart>>,
}

impl ReplaySession {
    /// Description of the negotiated H.264 track.
    pub fn track(&self) -> &ReplayTrack {
        &self.track
    }

    /// Obtain a handle that can cancel while an operation is pending.
    pub fn cancellation(&self) -> ReplayCancellation {
        ReplayCancellation(self.stop.clone())
    }

    /// Jump within the same session and wait for correlated RTP evidence.
    ///
    /// If this future is dropped after submission, the next API call observes
    /// that seek's result before submitting a new command or returning media.
    pub async fn seek(&mut self, request: ReplayRequest) -> Result<ReplayStart, ReplayFailure> {
        let range = transport::Range::try_from(request)
            .map_err(|kind| local_failure(ReplayStage::Request, kind))?;
        self.finish_seek().await?;
        if self.seek.is_closed() {
            return Err(self.terminal_failure(ReplayStage::Seek).await);
        }
        let (delivery, media) = self.limits.channel();
        let (result, pending) = oneshot::channel();
        match self.seek.try_send(Seek {
            range,
            delivery,
            result,
        }) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                return Err(self.terminal_failure(ReplayStage::Seek).await);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                return Err(local_failure(ReplayStage::Seek, ReplayErrorKind::Limit));
            }
        }
        // Dropping the old receiver releases all queued media for that position.
        // The replacement and pending response are installed before any await.
        self.media = media;
        self.pending = Some(pending);
        self.finish_seek()
            .await?
            .ok_or_else(|| local_failure(ReplayStage::Seek, ReplayErrorKind::PositionUnconfirmed))
    }

    /// Read the next packet, including the packet observed as the first start.
    pub async fn next(&mut self) -> Result<Option<ReplayPacket>, ReplayFailure> {
        self.finish_seek().await?;
        if let Some(packet) = self.media.recv().await {
            return Ok(Some(packet.packet));
        }
        let stop = self.terminal().await;
        match stop.end {
            ReplayEnd::Failed { stage, kind } => Err(ReplayFailure {
                stage,
                kind,
                start: stop.start,
                cleanup: stop.cleanup,
            }),
            _ => Ok(None),
        }
    }

    /// Stop locally, attempt bounded TEARDOWN, and return the cached result.
    pub async fn close(&mut self) -> ReplayStop {
        request_stop(&self.stop, StopRequest::Close);
        let report = self.terminal().await;
        self.media.close();
        while self.media.try_recv().is_ok() {}
        report
    }

    async fn finish_seek(&mut self) -> Result<Option<ReplayStart>, ReplayFailure> {
        let Some(pending) = &mut self.pending else {
            return Ok(None);
        };
        let result = pending.await;
        self.pending = None;
        match result {
            Ok(start) => Ok(Some(start)),
            Err(_) => Err(self.terminal_failure(ReplayStage::Seek).await),
        }
    }

    async fn terminal(&mut self) -> ReplayStop {
        match self.stopped.wait_for(Option::is_some).await {
            Ok(state) => state.as_ref().expect("terminal report present").clone(),
            Err(_) => ReplayStop {
                end: ReplayEnd::Failed {
                    stage: ReplayStage::Media,
                    kind: ReplayErrorKind::Io,
                },
                start: None,
                cleanup: ReplayCleanup::Unconfirmed,
            },
        }
    }

    async fn terminal_failure(&mut self, fallback: ReplayStage) -> ReplayFailure {
        let stop = self.terminal().await;
        let (stage, kind) = match stop.end {
            ReplayEnd::Failed { stage, kind } => (stage, kind),
            ReplayEnd::Cancelled => (fallback, ReplayErrorKind::Cancelled),
            _ => (fallback, ReplayErrorKind::Closed),
        };
        ReplayFailure {
            stage,
            kind,
            start: stop.start,
            cleanup: stop.cleanup,
        }
    }
}

impl Drop for ReplaySession {
    fn drop(&mut self) {
        request_stop(&self.stop, StopRequest::Abandon);
    }
}

impl OnvifSession {
    /// Obtain the Replay URI and start one H.264 track at an absolute time.
    ///
    /// RTSP credentials and the startup cancellation future apply to this
    /// call. For cancellation after startup use the returned owner's handle.
    pub async fn start_recording_replay(
        &self,
        recording_token: &str,
        request: ReplayRequest,
        credentials: Option<(&str, &str)>,
        options: &ReplayOptions,
        cancel: impl Future<Output = ()>,
    ) -> Result<(ReplaySession, ReplayStart), ReplayFailure> {
        let range = transport::Range::try_from(request)
            .map_err(|kind| local_failure(ReplayStage::Request, kind))?;
        if !options.validate() || token(recording_token).is_err() {
            return Err(local_failure(
                ReplayStage::Request,
                ReplayErrorKind::InvalidRequest,
            ));
        }
        let device = self.client().device_url();
        let deadline = Instant::now() + options.startup_timeout;
        let mut stage = ReplayStage::ReplayUri;
        let mut driver = Driver::default();
        let startup = async {
            let endpoint = self
                .capabilities()
                .replay
                .url
                .as_deref()
                .ok_or(ReplayErrorKind::MissingReplayService)?;
            self.endpoint_policy
                .check(device, endpoint)
                .map_err(|_| ReplayErrorKind::DestinationRejected)?;
            let uri = self
                .get_replay_uri(recording_token, "RTP-Unicast", "RTSP")
                .await
                .map_err(|_| ReplayErrorKind::ReplayUri)?;
            open(&mut driver, &mut stage, &uri, device, credentials, options).await?;
            stage = ReplayStage::Play;
            driver.play(&range, false, options.into(), &mut stage).await
        };
        let result = tokio::select! {
            biased;
            () = cancel => Err(ReplayErrorKind::Cancelled),
            result = timeout_at(deadline, startup) => match result {
                Ok(result) => result,
                Err(_) => Err(driver.timeout_kind()),
            },
        };
        let playback = match result {
            Ok(value) => value,
            Err(kind) => {
                return Err(ReplayFailure {
                    stage,
                    kind,
                    start: None,
                    cleanup: driver.cleanup(options.teardown_timeout).await,
                });
            }
        };
        let start = playback.start.clone();
        let track = driver.track().clone();
        let limits = transport::BufferLimits::from(options);
        let (output, media) = limits.channel();
        let (seek, seek_rx) = mpsc::channel(1);
        let (stop, stop_rx) = watch::channel(StopRequest::Running);
        let (terminal, stopped) = watch::channel(None);
        tokio::spawn(run(
            driver,
            playback,
            output,
            seek_rx,
            stop_rx,
            terminal,
            options.into(),
        ));
        Ok((
            ReplaySession {
                track,
                media,
                seek,
                stop,
                stopped,
                limits,
                pending: None,
            },
            start,
        ))
    }
}

fn local_failure(stage: ReplayStage, kind: ReplayErrorKind) -> ReplayFailure {
    ReplayFailure {
        stage,
        kind,
        start: None,
        cleanup: ReplayCleanup::NotNeeded,
    }
}
