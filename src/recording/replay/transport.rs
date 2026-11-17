use std::{sync::Arc, time::Duration};

use reqwest::Url;
use rtsp_types::{Data, Message, Method, Response, headers};
use time::{OffsetDateTime, UtcOffset};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    time::{Instant, sleep_until, timeout},
};

use crate::{
    rtsp::{
        Connection, checked_join, description_base, destination, interleaved_transport, is_rtp,
        parse_sdp, session_id, successful, video_codec,
    },
    stream_probe::{StreamOutcome, VideoCodec},
};

use super::{
    ReplayCleanup, ReplayEnd, ReplayErrorKind as Error, ReplayOptions, ReplayPacket, ReplayRequest,
    ReplayStage, ReplayStart, ReplayStop, ReplayTrack, Seek, StopRequest,
    media::{Position, starts_position},
    stopped,
};
use crate::recording::RecordingTimestamp;

const REPLAY: &str = "onvif-replay";
const DEFAULT_SESSION_TIMEOUT: Duration = Duration::from_secs(60);
// Rapid replay may leave RTP ahead of the RTSP response on the same
// socket. Bound that scan separately from the media delivery queue.
const RESPONSE_MESSAGE_LIMIT: usize = 1024;

pub(super) struct Range {
    pub(super) start: OffsetDateTime,
    header: String,
}

impl TryFrom<ReplayRequest> for Range {
    type Error = Error;

    fn try_from(request: ReplayRequest) -> Result<Self, Error> {
        let start = utc(request.start.instant())?;
        let end = request
            .end
            .as_ref()
            .map(|end| utc(end.instant()))
            .transpose()?;
        if end.is_some_and(|end| end <= start) {
            return Err(Error::InvalidRequest);
        }
        let start_wire = utc_wire(start);
        let end_wire = end.map(utc_wire).unwrap_or_default();
        Ok(Self {
            start,
            header: format!("clock={start_wire}-{end_wire}"),
        })
    }
}

fn utc(time: OffsetDateTime) -> Result<OffsetDateTime, Error> {
    time.checked_to_offset(UtcOffset::UTC)
        .filter(|time| (1..=9999).contains(&time.year()))
        .ok_or(Error::InvalidRequest)
}

fn utc_wire(time: OffsetDateTime) -> String {
    let fraction = format!(".{:09}", time.nanosecond());
    let fraction = if time.nanosecond() == 0 {
        ""
    } else {
        fraction.trim_end_matches('0')
    };
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}{fraction}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second(),
    )
}

fn response_start(response: &Response<Vec<u8>>) -> Option<OffsetDateTime> {
    let (start, _) = response
        .header(&headers::RANGE)?
        .as_str()
        .trim()
        .strip_prefix("clock=")?
        .split_once('-')?;
    if !(16..=26).contains(&start.len())
        || !start.is_ascii()
        || start.as_bytes()[8] != b'T'
        || !start.ends_with('Z')
    {
        return None;
    }
    let timestamp = format!(
        "{}-{}-{}T{}:{}:{}",
        &start[..4],
        &start[4..6],
        &start[6..8],
        &start[9..11],
        &start[11..13],
        &start[13..],
    );
    Some(timestamp.parse::<RecordingTimestamp>().ok()?.instant())
}

#[derive(Clone, Copy)]
pub(super) struct BufferLimits {
    packets: usize,
    bytes: usize,
}

impl From<&ReplayOptions> for BufferLimits {
    fn from(options: &ReplayOptions) -> Self {
        Self {
            packets: options.max_buffered_packets,
            bytes: options.max_buffered_bytes,
        }
    }
}

impl BufferLimits {
    pub(super) fn channel(self) -> (Delivery, mpsc::Receiver<PublishedPacket>) {
        let (tx, rx) = mpsc::channel(self.packets);
        (
            Delivery {
                tx,
                budget: Arc::new(Semaphore::new(self.bytes)),
            },
            rx,
        )
    }
}

pub(super) struct RunOptions {
    limits: BufferLimits,
    operation: Duration,
    idle: Duration,
    cleanup: Duration,
}

impl From<&ReplayOptions> for RunOptions {
    fn from(options: &ReplayOptions) -> Self {
        Self {
            limits: options.into(),
            operation: options.request_timeout,
            idle: options.idle_timeout,
            cleanup: options.teardown_timeout,
        }
    }
}

pub(super) struct PublishedPacket {
    pub(super) packet: ReplayPacket,
    _budget: OwnedSemaphorePermit,
}

pub(super) struct Delivery {
    tx: mpsc::Sender<PublishedPacket>,
    budget: Arc<Semaphore>,
}

impl Delivery {
    fn send(&self, packet: ReplayPacket) -> Result<(), Error> {
        // The owner replaces the receiver when submitting a new position.
        if self.tx.is_closed() {
            return Ok(());
        }
        let Ok(budget) = self
            .budget
            .clone()
            .try_acquire_many_owned(packet.bytes.len() as u32)
        else {
            return if self.tx.is_closed() {
                Ok(())
            } else {
                Err(Error::SlowConsumer)
            };
        };
        match self.tx.try_send(PublishedPacket {
            packet,
            _budget: budget,
        }) {
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) if self.tx.is_closed() => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(Error::SlowConsumer),
        }
    }
}

/// Media interleaved ahead of a PLAY reply must survive reply correlation.
#[derive(Default)]
struct EarlyMedia {
    cseq: Option<u32>,
    bytes: usize,
    saw_video: bool,
    overflow: bool,
    data: Vec<Data<Vec<u8>>>,
}

impl EarlyMedia {
    fn push(
        &mut self,
        cseq: u32,
        data: Data<Vec<u8>>,
        payload: u8,
        seek: bool,
        limits: BufferLimits,
    ) {
        if self.cseq != Some(cseq) {
            self.data.clear();
            self.bytes = 0;
            self.cseq = Some(cseq);
            self.saw_video = false;
            self.overflow = false;
        }
        if data.channel_id() != 0 || !is_rtp(data.as_slice(), payload) {
            return;
        }
        self.saw_video = true;
        if self.overflow || (self.data.is_empty() && !starts_position(&data, payload, cseq, seek)) {
            return;
        }
        if self.data.len() == limits.packets || self.bytes + data.as_slice().len() > limits.bytes {
            self.overflow = true;
            return;
        }
        self.bytes += data.as_slice().len();
        self.data.push(data);
    }
}

#[derive(Default)]
pub(super) struct Driver {
    connection: Option<Connection>,
    target: Option<Url>,
    track: Option<ReplayTrack>,
    setup_uncertain: bool,
    play_sequences: [u64; 4],
    saw_video: bool,
    keep_alive: Duration,
}

pub(super) struct Playback {
    pub(super) start: ReplayStart,
    position: Position,
    buffered: Vec<ReplayPacket>,
}

pub(super) async fn open(
    driver: &mut Driver,
    stage: &mut ReplayStage,
    uri: &str,
    device: &str,
    credentials: Option<(&str, &str)>,
    options: &ReplayOptions,
) -> Result<(), Error> {
    *stage = ReplayStage::Connect;
    let uri = destination(uri, device).ok_or(Error::DestinationRejected)?;
    let connection = driver
        .connection
        .insert(Connection::open(&uri, credentials, &options.ca_certificates).await?);
    connection.set_response_message_limit(RESPONSE_MESSAGE_LIMIT);
    *stage = ReplayStage::Describe;
    let response = connection
        .command(
            Method::Describe,
            &uri,
            &[(headers::ACCEPT, "application/sdp")],
            true,
        )
        .await?;
    successful(&response)?;
    let base = description_base(&response, &uri)?;
    let description = select_track(&response)?;
    let aggregate_uri = description
        .aggregate
        .as_deref()
        .map(|control| control_uri(&base, control))
        .transpose()?;
    let setup_uri = match description.control.as_deref() {
        Some(control) => control_uri(&base, control)?,
        None => aggregate_uri.clone().unwrap_or(base),
    };
    driver.target = Some(aggregate_uri.unwrap_or_else(|| setup_uri.clone()));
    driver.track = Some(description.track);
    *stage = ReplayStage::Setup;
    driver.setup_uncertain = true;
    let response = connection
        .command(
            Method::Setup,
            &setup_uri,
            &[
                (headers::TRANSPORT, "RTP/AVP/TCP;unicast;interleaved=0-1"),
                (headers::REQUIRE, REPLAY),
            ],
            true,
        )
        .await?;
    if let Err(error) = replay_success(&response) {
        driver.setup_uncertain = false;
        return Err(error);
    }
    connection.session = Some(session_id(&response)?);
    driver.setup_uncertain = false;
    if !interleaved_transport(&response) {
        return Err(Error::UnsupportedMedia);
    }
    driver.keep_alive = session_timeout(&response).min(DEFAULT_SESSION_TIMEOUT) / 2;
    Ok(())
}

fn control_uri(base: &Url, control: &str) -> Result<Url, Error> {
    if control == "*" {
        Ok(base.clone())
    } else {
        Ok(checked_join(base, control)?)
    }
}

struct TrackDescription {
    track: ReplayTrack,
    control: Option<String>,
    aggregate: Option<String>,
}

fn select_track(response: &Response<Vec<u8>>) -> Result<TrackDescription, Error> {
    let sdp = parse_sdp(response)?;
    for media in sdp.medias.iter().filter(|media| {
        media.media == "video" && matches!(media.proto.as_str(), "RTP/AVP" | "RTP/AVP/TCP")
    }) {
        for payload_type in media
            .fmt
            .split_ascii_whitespace()
            .filter_map(|value| value.parse::<u8>().ok())
        {
            if payload_type > 127 || video_codec(media, payload_type) != Some(VideoCodec::H264) {
                continue;
            }
            let fmtp = media
                .attributes
                .iter()
                .filter(|attr| attr.attribute == "fmtp")
                .filter_map(|attr| attr.value.as_deref()?.split_once(' '))
                .find(|(payload, _)| payload.parse::<u8>() == Ok(payload_type))
                .map(|(_, value)| value.trim());
            if fmtp.is_some_and(|value| {
                value
                    .split(';')
                    .filter_map(|part| part.split_once('='))
                    .any(|(name, value)| {
                        name.trim().eq_ignore_ascii_case("packetization-mode")
                            && !matches!(value.trim(), "0" | "1")
                    })
            }) {
                continue;
            }
            let control = media
                .get_first_attribute_value("control")
                .flatten()
                .map(str::to_owned);
            if control.is_none() && sdp.medias.len() != 1 {
                return Err(Error::InvalidResponse);
            }
            let aggregate = sdp
                .get_first_attribute_value("control")
                .flatten()
                .map(str::to_owned);
            return Ok(TrackDescription {
                track: ReplayTrack {
                    payload_type,
                    fmtp: fmtp.map(str::to_owned),
                },
                control,
                aggregate,
            });
        }
    }
    Err(Error::UnsupportedMedia)
}

fn session_timeout(response: &Response<Vec<u8>>) -> Duration {
    response
        .header(&headers::SESSION)
        .and_then(|value| {
            value.as_str().split(';').skip(1).find_map(|parameter| {
                let (name, value) = parameter.trim().split_once('=')?;
                name.eq_ignore_ascii_case("timeout")
                    .then(|| value.trim().parse::<u64>().ok())
                    .flatten()
            })
        })
        .filter(|&seconds| seconds != 0)
        .map_or(DEFAULT_SESSION_TIMEOUT, Duration::from_secs)
}

fn check_session(response: &Response<Vec<u8>>, expected: Option<&str>) -> Result<(), Error> {
    if response.header(&headers::SESSION).is_some()
        && Some(session_id(response)?.as_str()) != expected
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}

fn replay_success(response: &Response<Vec<u8>>) -> Result<(), Error> {
    successful(response).map_err(|reason| match reason {
        StreamOutcome::Refused(551) => Error::UnsupportedReplay,
        other => other.into(),
    })
}

impl Driver {
    pub(super) fn track(&self) -> &ReplayTrack {
        self.track.as_ref().expect("negotiated track")
    }

    pub(super) fn timeout_kind(&self) -> Error {
        if self.saw_video {
            Error::PositionUnconfirmed
        } else {
            Error::TimedOut
        }
    }

    async fn read(&mut self) -> Result<Message<Vec<u8>>, Error> {
        Ok(self
            .connection
            .as_mut()
            .expect("active connection")
            .read_message()
            .await?)
    }

    pub(super) async fn play(
        &mut self,
        range: &Range,
        seek: bool,
        limits: BufferLimits,
        stage: &mut ReplayStage,
    ) -> Result<Playback, Error> {
        self.saw_video = false;
        let payload = self.track().payload_type;
        let connection = self.connection.as_mut().expect("active connection");
        let target = self.target.as_ref().expect("validated control URI");
        let mut headers = vec![
            (headers::REQUIRE, REPLAY),
            (headers::RANGE, range.header.as_str()),
        ];
        if seek {
            headers.push((
                headers::HeaderName::from_static_str("Immediate").expect("valid ASCII header"),
                "yes",
            ));
        }
        let mut early = EarlyMedia::default();
        let response = connection
            .command_with_data(Method::Play, target, &headers, true, |cseq, data| {
                early.push(cseq, data, payload, seek, limits);
                Ok::<_, Error>(())
            })
            .await?;
        replay_success(&response)?;
        check_session(&response, connection.session.as_deref())?;
        let cseq: u32 = response
            .header(&headers::CSEQ)
            .ok_or(Error::InvalidResponse)?
            .as_str()
            .parse()
            .map_err(|_| Error::InvalidResponse)?;
        mark_play(&mut self.play_sequences, cseq)?;
        if !seek {
            *stage = ReplayStage::Media;
        }
        let mut position = Position::new(cseq, seek, range.start);
        let mut buffered = Vec::new();
        if early.cseq == Some(cseq) {
            if early.overflow {
                return Err(Error::Limit);
            }
            self.saw_video = early.saw_video;
            for data in early.data {
                buffered.extend(position.accept(data, payload)?);
            }
        }
        let first = loop {
            if let Some(first) = position.first() {
                break first;
            }
            let message = match connection.read_message().await {
                Err(StreamOutcome::Io) if self.saw_video => {
                    return Err(Error::PositionUnconfirmed);
                }
                result => result?,
            };
            let Message::Data(data) = message else {
                return Err(Error::InvalidResponse);
            };
            self.saw_video |= data.channel_id() == 0 && is_rtp(data.as_slice(), payload);
            if let Some(packet) = position.accept(data, payload)? {
                if packet.bytes.len() > limits.bytes {
                    return Err(Error::Limit);
                }
                buffered.push(packet);
            }
        };
        let start = ReplayStart {
            requested_at: range.start,
            server_reported_at: response_start(&response),
            observed_at: first.recorded_at,
            clean_point: first.clean_point,
        };
        Ok(Playback {
            start,
            position,
            buffered,
        })
    }

    async fn keep_alive(
        &mut self,
        output: &Delivery,
        position: &mut Position,
        last_packet: &mut Instant,
    ) -> Result<(), Error> {
        let payload = self.track().payload_type;
        let connection = self.connection.as_mut().expect("active connection");
        let target = self.target.as_ref().expect("validated control URI");
        let mut media_error = None;
        let mut on_data = |_, data| {
            if media_error.is_none() {
                let result = position.accept(data, payload).and_then(|packet| {
                    if let Some(packet) = packet {
                        output.send(packet)?;
                        *last_packet = Instant::now();
                    }
                    Ok(())
                });
                media_error = result.err();
            }
            Ok::<_, Error>(())
        };
        let mut response = connection
            .command_with_data(Method::GetParameter, target, &[], true, &mut on_data)
            .await?;
        if matches!(u16::from(response.status()), 405 | 501) {
            response = connection
                .command_with_data(Method::Options, target, &[], true, on_data)
                .await?;
        }
        successful(&response)?;
        check_session(&response, connection.session.as_deref())?;
        media_error.map_or(Ok(()), Err)
    }

    pub(super) async fn cleanup(&mut self, deadline: Duration) -> ReplayCleanup {
        let Some(connection) = &mut self.connection else {
            return ReplayCleanup::NotNeeded;
        };
        if connection.session.is_none() {
            return if self.setup_uncertain {
                ReplayCleanup::Unconfirmed
            } else {
                ReplayCleanup::NotNeeded
            };
        }
        if connection.has_pending_response() {
            return ReplayCleanup::Unconfirmed;
        }
        let target = self
            .target
            .as_ref()
            .expect("active session has a control URI");
        match timeout(deadline, async {
            let response = connection
                .command(Method::Teardown, target, &[], true)
                .await?;
            successful(&response)?;
            check_session(&response, connection.session.as_deref())
        })
        .await
        {
            Ok(Ok(())) => ReplayCleanup::Released,
            Ok(Err(kind)) => ReplayCleanup::Failed(kind),
            Err(_) => ReplayCleanup::Failed(Error::TimedOut),
        }
    }
}

fn mark_play(sequences: &mut [u64; 4], cseq: u32) -> Result<(), Error> {
    let index = usize::from(cseq as u8);
    let word = &mut sequences[index / 64];
    let mask = 1 << (index % 64);
    if *word & mask != 0 {
        return Err(Error::PositionUnconfirmed);
    }
    *word |= mask;
    Ok(())
}

pub(super) async fn run(
    mut driver: Driver,
    mut playback: Playback,
    mut output: Delivery,
    mut seek: mpsc::Receiver<Seek>,
    mut stop: watch::Receiver<StopRequest>,
    report: watch::Sender<Option<ReplayStop>>,
    options: RunOptions,
) {
    let (end, abandon) = tokio::select! {
        biased;
        request = stopped(&mut stop) => match request {
            StopRequest::Close => (ReplayEnd::Closed, false),
            StopRequest::Cancel => (ReplayEnd::Cancelled, false),
            _ => (ReplayEnd::Closed, true),
        },
        end = serve(&mut driver, &mut playback, &mut output, &mut seek, &options) => (end, false),
    };
    let cleanup = if abandon {
        ReplayCleanup::Unconfirmed
    } else {
        driver.cleanup(options.cleanup).await
    };
    drop(driver);
    report.send_replace(Some(ReplayStop {
        end,
        start: Some(playback.start),
        cleanup,
    }));
}

async fn serve(
    driver: &mut Driver,
    playback: &mut Playback,
    output: &mut Delivery,
    seek: &mut mpsc::Receiver<Seek>,
    options: &RunOptions,
) -> ReplayEnd {
    if let Err(kind) = publish(playback, output) {
        return fail(ReplayStage::Media, kind);
    }
    let payload = driver.track().payload_type;
    let mut last_packet = Instant::now();
    let mut keep_alive = last_packet + driver.keep_alive;
    loop {
        tokio::select! {
            biased;
            command = seek.recv() => {
                let Some(command) = command else { return ReplayEnd::Closed; };
                let mut stage = ReplayStage::Seek;
                match timeout(options.operation, driver.play(&command.range, true, options.limits, &mut stage)).await {
                    Ok(Ok(next)) => {
                        *playback = next;
                        *output = command.delivery;
                        if let Err(kind) = publish(playback, output) {
                            return fail(ReplayStage::Seek, kind);
                        }
                        let _ = command.result.send(playback.start.clone());
                        last_packet = Instant::now();
                        keep_alive = last_packet + driver.keep_alive;
                    }
                    Ok(Err(kind)) => return fail(stage, kind),
                    Err(_) => return fail(stage, driver.timeout_kind()),
                }
            }
            _ = sleep_until(last_packet + options.idle) => {
                return if playback.position.terminal() {
                    ReplayEnd::EndOfFootage
                } else {
                    fail(ReplayStage::Media, Error::TimedOut)
                };
            }
            _ = sleep_until(keep_alive) => {
                let result = timeout(options.operation, driver.keep_alive(output, &mut playback.position, &mut last_packet))
                    .await.unwrap_or(Err(Error::TimedOut));
                if let Err(kind) = result {
                    return if playback.position.terminal() && kind == Error::Io {
                        ReplayEnd::EndOfFootage
                    } else {
                        fail(ReplayStage::KeepAlive, kind)
                    };
                }
                keep_alive = Instant::now() + driver.keep_alive;
            }
            message = driver.read() => {
                let packet = match message {
                    Ok(Message::Data(data)) => playback.position.accept(data, payload),
                    Ok(_) => Err(Error::InvalidResponse),
                    Err(Error::Io) if playback.position.terminal() => return ReplayEnd::EndOfFootage,
                    Err(kind) => Err(kind),
                };
                match packet {
                    Ok(Some(packet)) => {
                        if let Err(kind) = output.send(packet) {
                            return fail(ReplayStage::Media, kind);
                        }
                        last_packet = Instant::now();
                    }
                    Ok(None) => {}
                    Err(kind) => return fail(ReplayStage::Media, kind),
                }
            }
        }
    }
}

fn publish(playback: &mut Playback, delivery: &Delivery) -> Result<(), Error> {
    for packet in playback.buffered.drain(..) {
        delivery.send(packet)?;
    }
    Ok(())
}

fn fail(stage: ReplayStage, kind: Error) -> ReplayEnd {
    ReplayEnd::Failed { stage, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_absolute_utc_and_rejects_empty_forward_range() {
        let start: RecordingTimestamp = "2026-11-17T08:02:03.040+08:00".parse().unwrap();
        let end: RecordingTimestamp = "2026-11-17T00:03:00Z".parse().unwrap();
        let range = Range::try_from(ReplayRequest {
            start: start.clone(),
            end: Some(end),
        })
        .unwrap();
        assert_eq!(range.header, "clock=20261117T000203.04Z-20261117T000300Z");
        let reversed = ReplayRequest {
            start: start.clone(),
            end: Some(start),
        };
        assert!(matches!(
            Range::try_from(reversed),
            Err(Error::InvalidRequest)
        ));
        let out_of_range = "9999-12-31T23:59:59-01:00"
            .parse::<RecordingTimestamp>()
            .unwrap();
        assert!(matches!(
            Range::try_from(ReplayRequest::from(out_of_range)),
            Err(Error::InvalidRequest)
        ));
    }

    #[test]
    fn rejects_a_play_cseq_reused_after_intervening_positions() {
        let mut sequences = Default::default();
        assert!(mark_play(&mut sequences, 4).is_ok());
        assert!(mark_play(&mut sequences, 8).is_ok());
        assert_eq!(
            mark_play(&mut sequences, 260),
            Err(Error::PositionUnconfirmed)
        );
    }
}
