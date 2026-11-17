//! Bounded RTSP 1.0 diagnostics for a stream URI returned by a camera.
//!
//! A packet observation verifies one negotiated video RTP payload type on a
//! TCP interleaved channel. No decoder or background task is created.

use std::{future::Future, time::Duration};

use reqwest::Url;
use rtsp_types::{Message, Method, headers};
use tokio::time::timeout;

use crate::rtsp::{
    Connection, PACKET_LIMIT, ProbeResult, checked_join, description_base, destination,
    interleaved_transport, is_rtp, session_id, stream_url, successful, track,
};

#[cfg(feature = "talkback")]
pub mod talkback;

/// Bounded diagnostics configuration. Credentials are provided separately.
#[derive(Clone)]
pub struct StreamProbeOptions {
    /// Total connection, request and observation deadline.
    pub timeout: Duration,
    /// Attempt single-track TCP interleaved playback after DESCRIBE.
    pub observe_packet: bool,
    /// Maximum wait for a video packet within the total deadline.
    pub packet_timeout: Duration,
    /// Separate budget for best-effort TEARDOWN on the same connection.
    pub teardown_timeout: Duration,
    /// Additional PEM-encoded CA certificates for RTSPS.
    pub ca_certificates: Vec<Vec<u8>>,
}

impl Default for StreamProbeOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            observe_packet: false,
            packet_timeout: Duration::from_secs(2),
            teardown_timeout: Duration::from_secs(1),
            ca_certificates: Vec::new(),
        }
    }
}

/// Controlled, secret-free outcome of one diagnostic stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum StreamOutcome {
    /// Stage was skipped by policy, options or a failed prerequisite.
    #[default]
    Skipped,
    /// Stage completed as expected.
    Passed,
    /// RTSP answered with a 401 challenge or rejected authentication.
    AuthenticationRequired,
    /// The URI is invalid, changes host or would downgrade TLS.
    DestinationRejected,
    /// The peer refused an operation with this status. Redirects are refused.
    Refused(u16),
    /// The reply or its correlation/framing was invalid.
    InvalidResponse,
    /// A response or packet budget was exhausted.
    Limit,
    /// Connect, read or write failed.
    Io,
    /// RTSPS certificate validation or handshake failed.
    Tls,
    /// The SDP was malformed or missing.
    InvalidSdp,
    /// The SDP contained no supported video RTP payload.
    UnsupportedCodec,
    /// The device did not negotiate the requested TCP interleaving.
    UnsupportedTransport,
    /// Deadline expired.
    TimedOut,
    /// The caller signalled cooperative cancellation.
    Cancelled,
}

/// Independently reported stream observations. Contains no raw URI or reply.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StreamProbeReport {
    /// Parsed URI, TCP connection and, for RTSPS, certificate/handshake.
    pub connection: StreamOutcome,
    /// Anonymous RTSP OPTIONS. A 401 establishes reachability.
    pub options: StreamOutcome,
    /// DESCRIBE and SDP media selection, with challenge-based authentication.
    pub describe: StreamOutcome,
    /// Selected codec from the supported diagnostic allowlist.
    pub codec: Option<VideoCodec>,
    /// Optional interleaved transport negotiation.
    pub setup: StreamOutcome,
    /// Optional aggregate PLAY.
    pub play: StreamOutcome,
    /// Optional first matching video packet.
    pub packet: StreamOutcome,
    /// Cleanup attempted for a known session identifier.
    pub teardown: StreamOutcome,
}

/// Codec identified by SDP. No payload decoding is performed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VideoCodec {
    /// H.264/AVC.
    H264,
    /// H.265/HEVC.
    H265,
    /// RTP/JPEG, including static payload type 26.
    Jpeg,
    /// MPEG-4 Part 2 video.
    Mpeg4,
}

/// Probe a URI on the configured device host. Userinfo is removed, redirects
/// are refused, and Basic authentication is permitted only over verified TLS.
pub async fn probe(
    uri: &str,
    device_url: &str,
    credentials: Option<(&str, &str)>,
    options: &StreamProbeOptions,
) -> StreamProbeReport {
    probe_until(
        uri,
        device_url,
        credentials,
        options,
        std::future::pending(),
    )
    .await
}

/// Probe with cooperative cancellation. Resolving `cancel` ends observation and
/// attempts bounded TEARDOWN when a session is known. Dropping this future
/// closes the local connection, leaving remote session expiry to the device.
pub async fn probe_until(
    uri: &str,
    device_url: &str,
    credentials: Option<(&str, &str)>,
    options: &StreamProbeOptions,
    cancel: impl Future<Output = ()>,
) -> StreamProbeReport {
    let mut report = StreamProbeReport::default();
    let Some(uri) = destination(uri, device_url) else {
        report.connection = StreamOutcome::DestinationRejected;
        return report;
    };
    let mut connection = None;
    let mut stage = Stage::Connection;
    let run = async {
        connection = Some(Connection::open(&uri, credentials, &options.ca_certificates).await?);
        report.connection = StreamOutcome::Passed;
        observe(
            connection.as_mut().expect("connection established"),
            &uri,
            options,
            &mut stage,
            &mut report,
        )
        .await
    };
    let result = tokio::select! {
        biased;
        () = cancel => Err(StreamOutcome::Cancelled),
        result = timeout(options.timeout, run) => result.unwrap_or(Err(StreamOutcome::TimedOut)),
    };
    if let Err(error) = result {
        *stage.outcome(&mut report) = error;
    }
    if let Some(mut connection) = connection
        && connection.session.is_some()
    {
        report.teardown = timeout(options.teardown_timeout, async {
            let response = connection
                .command(Method::Teardown, &uri, &[], true)
                .await?;
            successful(&response)
        })
        .await
        .unwrap_or(Err(StreamOutcome::TimedOut))
        .map_or_else(|error| error, |()| StreamOutcome::Passed);
    }
    report
}

#[derive(Clone, Copy)]
enum Stage {
    Connection,
    Options,
    Describe,
    Setup,
    Play,
    Packet,
}

impl Stage {
    fn outcome(self, report: &mut StreamProbeReport) -> &mut StreamOutcome {
        match self {
            Self::Connection => &mut report.connection,
            Self::Options => &mut report.options,
            Self::Describe => &mut report.describe,
            Self::Setup => &mut report.setup,
            Self::Play => &mut report.play,
            Self::Packet => &mut report.packet,
        }
    }
}

/// Check the URI scheme and host without exposing the URI's contents.
pub fn valid_stream_uri(uri: &str) -> bool {
    stream_url(uri).is_some()
}

async fn observe(
    connection: &mut Connection,
    uri: &Url,
    options: &StreamProbeOptions,
    stage: &mut Stage,
    report: &mut StreamProbeReport,
) -> ProbeResult<()> {
    *stage = Stage::Options;
    let response = connection.command(Method::Options, uri, &[], false).await?;
    report.options = successful(&response).map_or_else(|error| error, |()| StreamOutcome::Passed);
    if matches!(report.options, StreamOutcome::Refused(300..=399)) {
        return Err(report.options);
    }
    *stage = Stage::Describe;
    let response = connection
        .command(
            Method::Describe,
            uri,
            &[(headers::ACCEPT, "application/sdp")],
            true,
        )
        .await?;
    successful(&response)?;
    let track = track(&response)?;
    report.describe = StreamOutcome::Passed;
    report.codec = Some(track.codec);
    if !options.observe_packet {
        return Ok(());
    }

    *stage = Stage::Setup;
    let base = description_base(&response, uri)?;
    let setup_uri = checked_join(
        &base,
        track.control.as_deref().ok_or(StreamOutcome::InvalidSdp)?,
    )?;
    let response = connection
        .command(
            Method::Setup,
            &setup_uri,
            &[(headers::TRANSPORT, "RTP/AVP/TCP;unicast;interleaved=0-1")],
            true,
        )
        .await?;
    successful(&response)?;
    connection.session = Some(session_id(&response)?);
    if !interleaved_transport(&response) {
        return Err(StreamOutcome::UnsupportedTransport);
    }
    report.setup = StreamOutcome::Passed;

    *stage = Stage::Play;
    let response = connection.command(Method::Play, uri, &[], true).await?;
    successful(&response)?;
    report.play = StreamOutcome::Passed;
    *stage = Stage::Packet;
    timeout(
        options.packet_timeout,
        first_packet(connection, track.payload),
    )
    .await
    .map_err(|_| StreamOutcome::TimedOut)??;
    report.packet = StreamOutcome::Passed;
    Ok(())
}

async fn first_packet(connection: &mut Connection, payload: u8) -> ProbeResult<()> {
    for _ in 0..PACKET_LIMIT {
        let Message::Data(data) = connection.read_message().await? else {
            return Err(StreamOutcome::InvalidResponse);
        };
        if data.channel_id() == 0 && is_rtp(data.as_slice(), payload) {
            return Ok(());
        }
    }
    Err(StreamOutcome::Limit)
}
