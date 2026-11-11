//! Bounded RTSP 1.0 diagnostics for a stream URI returned by a camera.
//!
//! A packet observation verifies one negotiated video RTP payload type on a
//! TCP interleaved channel. No decoder or background task is created.

use std::{future::Future, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Url;
use rtsp_types::{Message, Method, Request, Response, Version, headers};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_rustls::{TlsConnector, rustls};

#[cfg(feature = "talkback")]
pub mod talkback;

const HEADER_BYTES: usize = 16 * 1024;
const BODY_BYTES: usize = 64 * 1024;
const PACKET_LIMIT: usize = 64;
const URI_BYTES: usize = 8 * 1024;

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
        connection
            .as_mut()
            .expect("connection established")
            .observe(&uri, options, &mut stage, &mut report)
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

fn stream_url(uri: &str) -> Option<Url> {
    if uri.len() > URI_BYTES || uri.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return None;
    }
    Url::parse(uri).ok().filter(|uri| {
        matches!(uri.scheme(), "rtsp" | "rtsps") && uri.has_host() && uri.fragment().is_none()
    })
}

fn destination(uri: &str, device: &str) -> Option<Url> {
    let device = Url::parse(device).ok()?;
    let mut uri = stream_url(uri)?;
    if !matches!(device.scheme(), "http" | "https")
        || !uri.host_str()?.eq_ignore_ascii_case(device.host_str()?)
        || (device.scheme() == "https" && uri.scheme() != "rtsps")
    {
        return None;
    }
    uri.set_password(None).ok()?;
    uri.set_username("").ok()?;
    Some(uri)
}

fn port(uri: &Url) -> u16 {
    uri.port()
        .unwrap_or(if uri.scheme() == "rtsps" { 322 } else { 554 })
}

fn same_authority(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str()
            .zip(b.host_str())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        && port(a) == port(b)
        && b.username().is_empty()
        && b.password().is_none()
}

trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}

type ProbeResult<T> = Result<T, StreamOutcome>;

struct Connection<'a> {
    socket: Box<dyn Socket>,
    buffer: Vec<u8>,
    sequence: u32,
    credentials: Option<(&'a str, &'a str)>,
    challenge: Option<Challenge>,
    session: Option<String>,
    tls: bool,
}

enum Challenge {
    Digest(Box<digest_auth::WwwAuthenticateHeader>),
    Basic,
}

impl<'a> Connection<'a> {
    async fn open(
        uri: &Url,
        credentials: Option<(&'a str, &'a str)>,
        ca_certificates: &[Vec<u8>],
    ) -> ProbeResult<Self> {
        let host = uri
            .host_str()
            .expect("validated host")
            .trim_matches(['[', ']']);
        let stream = TcpStream::connect((host, port(uri)))
            .await
            .map_err(|_| StreamOutcome::Io)?;
        let tls = uri.scheme() == "rtsps";
        let socket: Box<dyn Socket> = if tls {
            let name = rustls::pki_types::ServerName::try_from(host.to_owned())
                .map_err(|_| StreamOutcome::Tls)?;
            let config = tls_config(ca_certificates)?;
            Box::new(
                TlsConnector::from(Arc::new(config))
                    .connect(name, stream)
                    .await
                    .map_err(|_| StreamOutcome::Tls)?,
            )
        } else {
            Box::new(stream)
        };
        Ok(Self {
            socket,
            buffer: Vec::new(),
            sequence: 0,
            credentials,
            challenge: None,
            session: None,
            tls,
        })
    }

    async fn observe(
        &mut self,
        uri: &Url,
        options: &StreamProbeOptions,
        stage: &mut Stage,
        report: &mut StreamProbeReport,
    ) -> ProbeResult<()> {
        *stage = Stage::Options;
        let response = self.command(Method::Options, uri, &[], false).await?;
        report.options =
            successful(&response).map_or_else(|error| error, |()| StreamOutcome::Passed);
        if matches!(report.options, StreamOutcome::Refused(300..=399)) {
            return Err(report.options);
        }
        *stage = Stage::Describe;
        let response = self
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
        let response = self
            .command(
                Method::Setup,
                &setup_uri,
                &[(headers::TRANSPORT, "RTP/AVP/TCP;unicast;interleaved=0-1")],
                true,
            )
            .await?;
        successful(&response)?;
        self.session = Some(session_id(&response)?);
        if !interleaved_transport(&response) {
            return Err(StreamOutcome::UnsupportedTransport);
        }
        report.setup = StreamOutcome::Passed;

        *stage = Stage::Play;
        let response = self.command(Method::Play, uri, &[], true).await?;
        successful(&response)?;
        report.play = StreamOutcome::Passed;
        *stage = Stage::Packet;
        timeout(options.packet_timeout, self.first_packet(track.payload))
            .await
            .map_err(|_| StreamOutcome::TimedOut)??;
        report.packet = StreamOutcome::Passed;
        Ok(())
    }

    async fn command(
        &mut self,
        method: Method,
        uri: &Url,
        extra: &[(headers::HeaderName, &str)],
        authenticate: bool,
    ) -> ProbeResult<Response<Vec<u8>>> {
        let response = self.exchange(&method, uri, extra, authenticate).await?;
        if u16::from(response.status()) != 401 {
            return Ok(response);
        }
        self.challenge = response
            .header(&headers::WWW_AUTHENTICATE)
            .and_then(|value| challenge(value.as_str(), self.tls));
        if authenticate && self.challenge.is_some() && self.credentials.is_some() {
            self.exchange(&method, uri, extra, true).await
        } else {
            Ok(response)
        }
    }

    async fn exchange(
        &mut self,
        method: &Method,
        uri: &Url,
        extra: &[(headers::HeaderName, &str)],
        authenticate: bool,
    ) -> ProbeResult<Response<Vec<u8>>> {
        self.sequence += 1;
        let mut request = Request::builder(method.clone(), Version::V1_0)
            .request_uri(uri.clone())
            .header(headers::CSEQ, self.sequence.to_string())
            .header(headers::USER_AGENT, "oxvif");
        for (name, value) in extra {
            request = request.header(name.clone(), *value);
        }
        if let Some(session) = &self.session {
            request = request.header(headers::SESSION, session.as_str());
        }
        if authenticate && let Some(value) = self.authorization(method, uri)? {
            request = request.header(headers::AUTHORIZATION, value);
        }
        let mut bytes = Vec::new();
        request
            .empty()
            .write(&mut bytes)
            .map_err(|_| StreamOutcome::InvalidResponse)?;
        if bytes.len() > HEADER_BYTES {
            return Err(StreamOutcome::Limit);
        }
        self.socket
            .write_all(&bytes)
            .await
            .map_err(|_| StreamOutcome::Io)?;
        for _ in 0..PACKET_LIMIT {
            match self.read_message().await? {
                Message::Response(response)
                    if response.version() == Version::V1_0
                        && response
                            .header(&headers::CSEQ)
                            .and_then(|value| value.as_str().parse::<u32>().ok())
                            == Some(self.sequence) =>
                {
                    return Ok(response);
                }
                Message::Data(_) if self.session.is_some() => {}
                _ => return Err(StreamOutcome::InvalidResponse),
            }
        }
        Err(StreamOutcome::Limit)
    }

    fn authorization(&mut self, method: &Method, uri: &Url) -> ProbeResult<Option<String>> {
        let Some((user, password)) = self.credentials else {
            return Ok(None);
        };
        if user.len().saturating_add(password.len()) > 4096
            || user.chars().chain(password.chars()).any(char::is_control)
        {
            return Err(StreamOutcome::AuthenticationRequired);
        }
        let value = match &mut self.challenge {
            Some(Challenge::Digest(challenge)) => {
                let context = digest_auth::AuthContext::new_with_method(
                    user,
                    password,
                    uri.as_str(),
                    None::<&[u8]>,
                    digest_auth::HttpMethod(<&str>::from(method).into()),
                );
                challenge
                    .respond(&context)
                    .map_err(|_| StreamOutcome::AuthenticationRequired)?
                    .to_string()
            }
            Some(Challenge::Basic) if !user.contains(':') => {
                format!("Basic {}", STANDARD.encode(format!("{user}:{password}")))
            }
            _ => return Ok(None),
        };
        if value.chars().any(char::is_control) {
            return Err(StreamOutcome::AuthenticationRequired);
        }
        Ok(Some(value))
    }

    async fn read_more(&mut self) -> ProbeResult<()> {
        let mut chunk = [0; 4096];
        let len = self
            .socket
            .read(&mut chunk)
            .await
            .map_err(|_| StreamOutcome::Io)?;
        if len == 0 {
            return Err(StreamOutcome::Io);
        }
        self.buffer.extend_from_slice(&chunk[..len]);
        Ok(())
    }

    async fn read_message(&mut self) -> ProbeResult<Message<Vec<u8>>> {
        if self.buffer.is_empty() {
            self.read_more().await?;
        }
        let frame_bytes = if self.buffer[0] == b'$' {
            while self.buffer.len() < 4 {
                self.read_more().await?;
            }
            4 + usize::from(u16::from_be_bytes([self.buffer[2], self.buffer[3]]))
        } else {
            let mut search_from = 0;
            let head_end = loop {
                if let Some(end) = self.buffer[search_from..]
                    .windows(4)
                    .position(|b| b == b"\r\n\r\n")
                {
                    break search_from + end + 4;
                }
                if self.buffer.len() >= HEADER_BYTES {
                    return Err(StreamOutcome::Limit);
                }
                search_from = self.buffer.len().saturating_sub(3);
                self.read_more().await?;
            };
            if head_end > HEADER_BYTES {
                return Err(StreamOutcome::Limit);
            }
            let body_bytes = body_length(&self.buffer[..head_end])?;
            head_end + body_bytes
        };
        while self.buffer.len() < frame_bytes {
            self.read_more().await?;
        }
        let (message, consumed) = Message::parse(&self.buffer[..frame_bytes])
            .map_err(|_| StreamOutcome::InvalidResponse)?;
        if consumed != frame_bytes {
            return Err(StreamOutcome::InvalidResponse);
        }
        self.buffer.drain(..consumed);
        Ok(message)
    }

    async fn first_packet(&mut self, payload: u8) -> ProbeResult<()> {
        for _ in 0..PACKET_LIMIT {
            let Message::Data(data) = self.read_message().await? else {
                return Err(StreamOutcome::InvalidResponse);
            };
            if data.channel_id() == 0 && is_rtp(data.as_slice(), payload) {
                return Ok(());
            }
        }
        Err(StreamOutcome::Limit)
    }
}

fn tls_config(certificates: &[Vec<u8>]) -> ProbeResult<rustls::ClientConfig> {
    use rustls::pki_types::{CertificateDer, pem::PemObject};
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    for pem in certificates {
        let mut found = false;
        for cert in CertificateDer::pem_slice_iter(pem) {
            roots
                .add(cert.map_err(|_| StreamOutcome::Tls)?)
                .map_err(|_| StreamOutcome::Tls)?;
            found = true;
        }
        if !found {
            return Err(StreamOutcome::Tls);
        }
    }
    Ok(rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| StreamOutcome::Tls)?
    .with_root_certificates(roots)
    .with_no_client_auth())
}

fn challenge(value: &str, tls: bool) -> Option<Challenge> {
    let (scheme, parameters) = value.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("digest") {
        digest_auth::parse(&format!("Digest {parameters}"))
            .ok()
            .map(Box::new)
            .map(Challenge::Digest)
    } else if tls && scheme.eq_ignore_ascii_case("basic") {
        Some(Challenge::Basic)
    } else {
        None
    }
}

fn body_length(header: &[u8]) -> ProbeResult<usize> {
    let header = std::str::from_utf8(header).map_err(|_| StreamOutcome::InvalidResponse)?;
    let mut length = None;
    for (index, line) in header
        .split("\r\n")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .enumerate()
    {
        if index >= 64 {
            return Err(StreamOutcome::Limit);
        }
        let (name, value) = line.split_once(':').ok_or(StreamOutcome::InvalidResponse)?;
        if name.eq_ignore_ascii_case("Content-Length") {
            let value = value
                .trim()
                .parse::<usize>()
                .map_err(|_| StreamOutcome::InvalidResponse)?;
            if value > BODY_BYTES {
                return Err(StreamOutcome::Limit);
            }
            if length.replace(value).is_some() {
                return Err(StreamOutcome::InvalidResponse);
            }
        }
    }
    Ok(length.unwrap_or(0))
}

fn successful(response: &Response<Vec<u8>>) -> ProbeResult<()> {
    match u16::from(response.status()) {
        200 => Ok(()),
        401 => Err(StreamOutcome::AuthenticationRequired),
        status => Err(StreamOutcome::Refused(status)),
    }
}

struct VideoTrack {
    codec: VideoCodec,
    payload: u8,
    control: Option<String>,
}

fn description_base(response: &Response<Vec<u8>>, uri: &Url) -> ProbeResult<Url> {
    response
        .header(&headers::CONTENT_BASE)
        .or_else(|| response.header(&headers::CONTENT_LOCATION))
        .map_or(Ok(uri.clone()), |value| checked_join(uri, value.as_str()))
}

fn parse_sdp(response: &Response<Vec<u8>>) -> ProbeResult<sdp_types::Session> {
    if !response
        .header(&headers::CONTENT_TYPE)
        .is_some_and(|value| {
            value
                .as_str()
                .split(';')
                .next()
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/sdp"))
        })
    {
        return Err(StreamOutcome::InvalidSdp);
    }
    sdp_types::Session::parse(response.body()).map_err(|_| StreamOutcome::InvalidSdp)
}

fn track(response: &Response<Vec<u8>>) -> ProbeResult<VideoTrack> {
    let sdp = parse_sdp(response)?;
    for media in sdp.medias.iter().filter(|media| {
        media.media == "video" && matches!(media.proto.as_str(), "RTP/AVP" | "RTP/AVP/TCP")
    }) {
        for payload in media
            .fmt
            .split_ascii_whitespace()
            .filter_map(|value| value.parse::<u8>().ok())
            .filter(|&n| n < 128)
        {
            if let Some(codec) = video_codec(media, payload) {
                return Ok(VideoTrack {
                    codec,
                    payload,
                    control: media
                        .get_first_attribute_value("control")
                        .flatten()
                        .map(str::to_owned),
                });
            }
        }
    }
    Err(StreamOutcome::UnsupportedCodec)
}

fn video_codec(media: &sdp_types::Media, payload: u8) -> Option<VideoCodec> {
    let mapping = media
        .attributes
        .iter()
        .filter(|attr| attr.attribute == "rtpmap")
        .filter_map(|attr| attr.value.as_deref()?.split_once(' '))
        .find(|(number, _)| number.parse::<u8>() == Ok(payload));
    let Some((_, encoding)) = mapping else {
        return (payload == 26).then_some(VideoCodec::Jpeg);
    };
    let (codec, clock) = encoding.split_once('/')?;
    if clock != "90000" {
        return None;
    }
    match codec.to_ascii_uppercase().as_str() {
        "H264" => Some(VideoCodec::H264),
        "H265" => Some(VideoCodec::H265),
        "JPEG" => Some(VideoCodec::Jpeg),
        "MP4V-ES" => Some(VideoCodec::Mpeg4),
        _ => None,
    }
}

fn checked_join(base: &Url, value: &str) -> ProbeResult<Url> {
    if value.is_empty()
        || value.len() > URI_BYTES
        || value == "*"
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(StreamOutcome::DestinationRejected);
    }
    base.join(value)
        .ok()
        .filter(|joined| {
            same_authority(base, joined)
                && joined.fragment().is_none()
                && joined.as_str().len() <= URI_BYTES
        })
        .ok_or(StreamOutcome::DestinationRejected)
}

fn session_id(response: &Response<Vec<u8>>) -> ProbeResult<String> {
    let id = response
        .header(&headers::SESSION)
        .and_then(|value| value.as_str().split(';').next())
        .map(str::trim)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 256
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-._+".contains(&byte))
        })
        .ok_or(StreamOutcome::InvalidResponse)?;
    Ok(id.to_owned())
}

fn interleaved_transport(response: &Response<Vec<u8>>) -> bool {
    let Some(value) = response.header(&headers::TRANSPORT) else {
        return false;
    };
    let mut parts = value.as_str().split(';').map(str::trim);
    parts
        .next()
        .is_some_and(|proto| proto.eq_ignore_ascii_case("RTP/AVP/TCP"))
        && parts
            .clone()
            .any(|part| part.eq_ignore_ascii_case("interleaved=0-1"))
        && !value.as_str().contains(',')
        && !parts.any(|part| part.eq_ignore_ascii_case("multicast"))
}

fn is_rtp(data: &[u8], payload: u8) -> bool {
    if data.len() <= 12 || data[0] >> 6 != 2 || data[1] & 0x7f != payload {
        return false;
    }
    let mut header = 12 + usize::from(data[0] & 0xf) * 4;
    if data[0] & 0x10 != 0 {
        let Some(extension) = data.get(header..header + 4) else {
            return false;
        };
        header += 4 + usize::from(u16::from_be_bytes([extension[2], extension[3]])) * 4;
    }
    let padding = if data[0] & 0x20 != 0 {
        let padding = usize::from(data[data.len() - 1]);
        if padding == 0 {
            return false;
        }
        padding
    } else {
        0
    };
    data.len() > header + padding
}
