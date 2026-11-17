//! Internal bounded RTSP 1.0 transport shared by diagnostics and media workflows.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Url;
use rtsp_types::{Data, Message, Method, Request, Response, Version, headers};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{TlsConnector, rustls};

use crate::stream_probe::{StreamOutcome, VideoCodec};

const HEADER_BYTES: usize = 16 * 1024;
const BODY_BYTES: usize = 64 * 1024;
pub(crate) const PACKET_LIMIT: usize = 64;
const URI_BYTES: usize = 8 * 1024;

pub(crate) fn stream_url(uri: &str) -> Option<Url> {
    if uri.len() > URI_BYTES || uri.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return None;
    }
    Url::parse(uri).ok().filter(|uri| {
        matches!(uri.scheme(), "rtsp" | "rtsps") && uri.has_host() && uri.fragment().is_none()
    })
}

pub(crate) fn destination(uri: &str, device: &str) -> Option<Url> {
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

pub(crate) trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}

pub(crate) type ProbeResult<T> = Result<T, StreamOutcome>;

pub(crate) struct Connection {
    pub(crate) socket: Box<dyn Socket>,
    buffer: Vec<u8>,
    sequence: u32,
    pending_sequence: Option<u32>,
    credentials: Option<(String, String)>,
    challenge: Option<Challenge>,
    pub(crate) session: Option<String>,
    tls: bool,
}

enum Challenge {
    Digest(Box<digest_auth::WwwAuthenticateHeader>),
    Basic,
}

impl Connection {
    pub(crate) async fn open(
        uri: &Url,
        credentials: Option<(&str, &str)>,
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
            pending_sequence: None,
            credentials: credentials.map(|(user, password)| (user.into(), password.into())),
            challenge: None,
            session: None,
            tls,
        })
    }

    pub(crate) async fn command(
        &mut self,
        method: Method,
        uri: &Url,
        extra: &[(headers::HeaderName, &str)],
        authenticate: bool,
    ) -> ProbeResult<Response<Vec<u8>>> {
        self.command_with_data(method, uri, extra, authenticate, |_, _| Ok(()))
            .await
    }

    pub(crate) async fn command_with_data<E: From<StreamOutcome>>(
        &mut self,
        method: Method,
        uri: &Url,
        extra: &[(headers::HeaderName, &str)],
        authenticate: bool,
        mut on_data: impl FnMut(u32, Data<Vec<u8>>) -> Result<(), E>,
    ) -> Result<Response<Vec<u8>>, E> {
        let response = self
            .exchange(&method, uri, extra, authenticate, &mut on_data)
            .await?;
        if u16::from(response.status()) != 401 {
            return Ok(response);
        }
        self.challenge = response
            .header(&headers::WWW_AUTHENTICATE)
            .and_then(|value| challenge(value.as_str(), self.tls));
        if authenticate && self.challenge.is_some() && self.credentials.is_some() {
            self.exchange(&method, uri, extra, true, &mut on_data).await
        } else {
            Ok(response)
        }
    }

    async fn exchange<E: From<StreamOutcome>>(
        &mut self,
        method: &Method,
        uri: &Url,
        extra: &[(headers::HeaderName, &str)],
        authenticate: bool,
        on_data: &mut impl FnMut(u32, Data<Vec<u8>>) -> Result<(), E>,
    ) -> Result<Response<Vec<u8>>, E> {
        self.sequence = self.sequence.checked_add(1).ok_or(StreamOutcome::Limit)?;
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
            return Err(StreamOutcome::Limit.into());
        }
        self.pending_sequence = Some(self.sequence);
        self.socket
            .write_all(&bytes)
            .await
            .map_err(|_| StreamOutcome::Io)?;
        for _ in 0..PACKET_LIMIT {
            match self.read_message().await? {
                Message::Response(response)
                    if response.version() == Version::V1_0
                        && response_sequence(&response) == self.pending_sequence =>
                {
                    self.pending_sequence = None;
                    return Ok(response);
                }
                Message::Data(data) if self.session.is_some() => on_data(self.sequence, data)?,
                _ => return Err(StreamOutcome::InvalidResponse.into()),
            }
        }
        Err(StreamOutcome::Limit.into())
    }

    fn authorization(&mut self, method: &Method, uri: &Url) -> ProbeResult<Option<String>> {
        let Some((user, password)) = self.credentials.as_ref() else {
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

    pub(crate) async fn read_message(&mut self) -> ProbeResult<Message<Vec<u8>>> {
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

pub(crate) fn successful(response: &Response<Vec<u8>>) -> ProbeResult<()> {
    match u16::from(response.status()) {
        200 => Ok(()),
        401 => Err(StreamOutcome::AuthenticationRequired),
        status => Err(StreamOutcome::Refused(status)),
    }
}

pub(crate) struct VideoTrack {
    pub(crate) codec: VideoCodec,
    pub(crate) payload: u8,
    pub(crate) control: Option<String>,
}

pub(crate) fn description_base(response: &Response<Vec<u8>>, uri: &Url) -> ProbeResult<Url> {
    response
        .header(&headers::CONTENT_BASE)
        .or_else(|| response.header(&headers::CONTENT_LOCATION))
        .map_or(Ok(uri.clone()), |value| checked_join(uri, value.as_str()))
}

pub(crate) fn parse_sdp(response: &Response<Vec<u8>>) -> ProbeResult<sdp_types::Session> {
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

pub(crate) fn track(response: &Response<Vec<u8>>) -> ProbeResult<VideoTrack> {
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

pub(crate) fn video_codec(media: &sdp_types::Media, payload: u8) -> Option<VideoCodec> {
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

pub(crate) fn checked_join(base: &Url, value: &str) -> ProbeResult<Url> {
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

pub(crate) fn session_id(response: &Response<Vec<u8>>) -> ProbeResult<String> {
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

pub(crate) fn interleaved_transport(response: &Response<Vec<u8>>) -> bool {
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

fn response_sequence(response: &Response<Vec<u8>>) -> Option<u32> {
    response.header(&headers::CSEQ)?.as_str().parse().ok()
}

pub(crate) fn is_rtp(data: &[u8], payload: u8) -> bool {
    rtp_extension(data, payload).is_some()
}

pub(crate) fn rtp_extension(data: &[u8], payload: u8) -> Option<Option<(u16, &[u8])>> {
    if data.len() <= 12 || data[0] >> 6 != 2 || data[1] & 0x7f != payload {
        return None;
    }
    let mut header = 12 + usize::from(data[0] & 0xf) * 4;
    let extension = if data[0] & 0x10 != 0 {
        let fields = data.get(header..header + 4)?;
        let id = u16::from_be_bytes([fields[0], fields[1]]);
        let size = usize::from(u16::from_be_bytes([fields[2], fields[3]])) * 4;
        let start = header + 4;
        header = start + size;
        Some((id, data.get(start..header)?))
    } else {
        None
    };
    let padding = if data[0] & 0x20 != 0 {
        let padding = usize::from(data[data.len() - 1]);
        if padding == 0 {
            return None;
        }
        padding
    } else {
        0
    };
    (data.len() > header + padding).then_some(extension)
}
