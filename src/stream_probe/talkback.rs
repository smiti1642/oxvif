//! Bounded ONVIF RTSP backchannel with one PCMU send track.
//!
//! This push-to-talk owner sends mono 8 kHz audio on TCP interleaving.
//! Full duplex and audio capture are separate concerns.

use std::{future::Future, time::Duration};

use reqwest::Url;
use rtsp_types::{Message, Method, headers};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc,
    time::{Instant, timeout},
};

use super::{
    Connection, ProbeResult, StreamOutcome, checked_join, description_base, destination,
    interleaved_transport, parse_sdp, session_id, successful,
};

const BACKCHANNEL: &str = "www.onvif.org/ver20/backchannel";
const FRAME_TIME: Duration = Duration::from_millis(20);
const QUEUE_FRAMES: usize = 16;

/// One 20 ms, signed PCM, mono frame sampled at 8 kHz.
pub type PcmFrame = [i16; 160];

/// Bounded PCMU backchannel options.
#[derive(Clone)]
pub struct TalkbackOptions {
    /// Whole lifetime, including negotiation and sending.
    pub timeout: Duration,
    /// Independent TEARDOWN budget.
    pub teardown_timeout: Duration,
    /// Maximum frames to send.
    pub max_frames: u32,
    /// Additional PEM CAs for a verified RTSPS connection.
    pub ca_certificates: Vec<Vec<u8>>,
}

impl Default for TalkbackOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            teardown_timeout: Duration::from_secs(1),
            max_frames: 1500,
            ca_certificates: Vec::new(),
        }
    }
}

/// Bounded input producer. Drop all producers to drain and stop the session.
#[derive(Clone)]
pub struct TalkbackInput(mpsc::Sender<PcmFrame>);

/// Why an input frame could not be queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    /// The bounded input queue is full. This frame was not queued.
    #[error("talkback input queue is full")]
    Full,
    /// The owner stopped. This frame was not queued.
    #[error("talkback input queue is closed")]
    Closed,
}

impl TalkbackInput {
    /// Queue one frame, waiting for capacity.
    pub async fn send(&self, frame: PcmFrame) -> Result<(), InputError> {
        self.0.send(frame).await.map_err(|_| InputError::Closed)
    }

    /// Try to queue one frame without blocking the input producer.
    pub fn try_send(&self, frame: PcmFrame) -> Result<(), InputError> {
        self.0.try_send(frame).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => InputError::Full,
            mpsc::error::TrySendError::Closed(_) => InputError::Closed,
        })
    }
}

/// Single owner of a 16-frame audio queue and one negotiated RTSP session.
pub struct Talkback(mpsc::Receiver<PcmFrame>);

/// Why the talkback owner finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TalkbackEnd {
    /// Input closed and all queued frames were sent.
    InputClosed,
    /// The configured frame limit was reached.
    FrameLimit,
    /// Connection or negotiation failed, or streaming was interrupted.
    Stopped(StreamOutcome),
}

/// Bounded, secret-free local evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkbackReport {
    /// Whether PLAY accepted the selected backchannel.
    pub negotiated: bool,
    /// Fully written RTP packets. The peer may have received a partial last packet.
    pub sent_frames: u32,
    /// Why sending stopped.
    pub end: TalkbackEnd,
    /// Independent TEARDOWN outcome for a known session.
    pub teardown: StreamOutcome,
}

impl Talkback {
    /// Construct a bounded input and a single owner. No network I/O occurs.
    pub fn channel() -> (TalkbackInput, Self) {
        let (sender, receiver) = mpsc::channel(QUEUE_FRAMES);
        (TalkbackInput(sender), Self(receiver))
    }

    /// Negotiate, pace and send one audio track until input closes, a limit is
    /// reached or cancellation resolves. Cleanup uses the same connection.
    /// Dropping this future closes the local connection.
    pub async fn run_until(
        mut self,
        uri: &str,
        device_url: &str,
        credentials: Option<(&str, &str)>,
        options: &TalkbackOptions,
        cancel: impl Future<Output = ()>,
    ) -> TalkbackReport {
        let mut report = TalkbackReport {
            negotiated: false,
            sent_frames: 0,
            end: TalkbackEnd::Stopped(StreamOutcome::DestinationRejected),
            teardown: StreamOutcome::Skipped,
        };
        let Some(uri) = destination(uri, device_url) else {
            return report;
        };
        if options.max_frames == 0 || options.timeout.is_zero() {
            report.end = TalkbackEnd::Stopped(StreamOutcome::Limit);
            return report;
        }
        let mut connection = None;
        let run = async {
            let conn = connection
                .insert(Connection::open(&uri, credentials, &options.ca_certificates).await?);
            let payload = negotiate(conn, &uri).await?;
            report.negotiated = true;
            send_frames(
                conn,
                &mut self.0,
                payload,
                &mut report.sent_frames,
                options.max_frames,
            )
            .await
        };
        let result = tokio::select! {
            biased;
            () = cancel => Err(StreamOutcome::Cancelled),
            result = timeout(options.timeout, run) => result.unwrap_or(Err(StreamOutcome::TimedOut)),
        };
        report.end = result.unwrap_or_else(TalkbackEnd::Stopped);
        self.0.close();
        if let Some(mut connection) = connection
            && connection.session.is_some()
        {
            report.teardown = timeout(options.teardown_timeout, async {
                let response = connection
                    .command(
                        Method::Teardown,
                        &uri,
                        &[(headers::REQUIRE, BACKCHANNEL)],
                        true,
                    )
                    .await?;
                successful(&response)
            })
            .await
            .unwrap_or(Err(StreamOutcome::TimedOut))
            .map_or_else(|error| error, |()| StreamOutcome::Passed);
        }
        report
    }
}

async fn negotiate(connection: &mut Connection<'_>, uri: &Url) -> ProbeResult<u8> {
    let response = connection
        .command(
            Method::Describe,
            uri,
            &[
                (headers::ACCEPT, "application/sdp"),
                (headers::REQUIRE, BACKCHANNEL),
            ],
            true,
        )
        .await?;
    successful(&response)?;
    let sdp = parse_sdp(&response)?;
    let (payload, control) = backchannel_track(&sdp).ok_or(StreamOutcome::UnsupportedCodec)?;
    let base = description_base(&response, uri)?;
    let setup_uri = checked_join(&base, control)?;
    let response = connection
        .command(
            Method::Setup,
            &setup_uri,
            &[
                (headers::TRANSPORT, "RTP/AVP/TCP;unicast;interleaved=0-1"),
                (headers::REQUIRE, BACKCHANNEL),
            ],
            true,
        )
        .await?;
    successful(&response)?;
    connection.session = Some(session_id(&response)?);
    if !interleaved_transport(&response) {
        return Err(StreamOutcome::UnsupportedTransport);
    }
    let response = connection
        .command(Method::Play, uri, &[(headers::REQUIRE, BACKCHANNEL)], true)
        .await?;
    successful(&response)?;
    Ok(payload)
}

fn backchannel_track(sdp: &sdp_types::Session) -> Option<(u8, &str)> {
    for media in &sdp.medias {
        if media.media != "audio"
            || !matches!(media.proto.as_str(), "RTP/AVP" | "RTP/AVP/TCP")
            || media.get_first_attribute_value("sendonly") != Some(None)
            || media.get_first_attribute_value("recvonly").is_some()
        {
            continue;
        }
        let Some(control) = media.get_first_attribute_value("control").flatten() else {
            continue;
        };
        for pt in media
            .fmt
            .split_ascii_whitespace()
            .filter_map(|value| value.parse::<u8>().ok())
        {
            let mut mappings = media
                .attributes
                .iter()
                .filter(|attr| attr.attribute == "rtpmap")
                .filter_map(|attr| attr.value.as_deref()?.split_once(' '))
                .filter(|(number, _)| number.parse::<u8>() == Ok(pt));
            let allowed = match mappings.next() {
                Some((_, codec)) => {
                    codec.eq_ignore_ascii_case("PCMU/8000")
                        || codec.eq_ignore_ascii_case("PCMU/8000/1")
                }
                None => pt == 0,
            };
            if (pt == 0 || (96..=127).contains(&pt)) && allowed && mappings.next().is_none() {
                return Some((pt, control));
            }
        }
    }
    None
}

async fn send_frames(
    connection: &mut Connection<'_>,
    input: &mut mpsc::Receiver<PcmFrame>,
    payload: u8,
    sent: &mut u32,
    limit: u32,
) -> ProbeResult<TalkbackEnd> {
    let start = Instant::now();
    let mut next = start;
    let mut sequence = rand::random::<u16>();
    let clock_base = rand::random::<u32>();
    let mut sample_clock = 0u64;
    let ssrc = rand::random::<u32>();
    let mut remaining_control = 256usize;
    while *sent < limit {
        let frame = tokio::select! {
            frame = input.recv() => match frame {
                Some(frame) => frame,
                None => return Ok(TalkbackEnd::InputClosed),
            },
            message = connection.read_message() => match message? {
                Message::Data(data) if data.channel_id() == 1 && remaining_control > 0 => {
                    remaining_control -= 1;
                    continue;
                }
                Message::Data(_) => return Err(StreamOutcome::Limit),
                _ => return Err(StreamOutcome::InvalidResponse),
            }
        };
        tokio::time::sleep_until(next).await;
        let now = Instant::now();
        let marker = *sent == 0 || now.saturating_duration_since(next) > FRAME_TIME;
        if marker {
            let elapsed_samples = ((now - start).as_micros() * 8 / 1000) as u64;
            sample_clock = sample_clock.max(elapsed_samples);
        }
        let timestamp = clock_base.wrapping_add(sample_clock as u32);
        let mut packet = [0; 176];
        packet[..6].copy_from_slice(&[
            b'$',
            0,
            0,
            172,
            0x80,
            payload | if marker { 0x80 } else { 0 },
        ]);
        packet[6..8].copy_from_slice(&sequence.to_be_bytes());
        packet[8..12].copy_from_slice(&timestamp.to_be_bytes());
        packet[12..16].copy_from_slice(&ssrc.to_be_bytes());
        for (byte, sample) in packet[16..].iter_mut().zip(frame) {
            *byte = pcmu(sample);
        }
        connection
            .socket
            .write_all(&packet)
            .await
            .map_err(|_| StreamOutcome::Io)?;
        *sent += 1;
        sample_clock += 160;
        sequence = sequence.wrapping_add(1);
        next = now + FRAME_TIME;
    }
    Ok(TalkbackEnd::FrameLimit)
}

fn pcmu(sample: i16) -> u8 {
    let magnitude = u32::from(sample.unsigned_abs()).min(32635) + 132;
    let exponent = (24 - magnitude.leading_zeros()) as u8;
    let mantissa = ((magnitude >> (exponent + 3)) & 15) as u8;
    !(if sample < 0 { 0x80 } else { 0 } | (exponent << 4) | mantissa)
}
