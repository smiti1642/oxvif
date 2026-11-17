#![cfg(feature = "recording-replay")]

use std::{future, sync::Arc, time::Duration};

use oxvif::{
    OnvifSession,
    recording::RecordingTimestamp,
    recording::replay::{
        ReplayCleanup, ReplayEnd, ReplayErrorKind, ReplayOptions, ReplayRequest, ReplayStage,
    },
    transport::{Transport, TransportError},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::timeout,
};

const START: &str = "2026-11-17T08:02:03.25+08:00";
const SEEK: &str = "2026-11-17T00:17:00Z";
const SESSION: &str = "replay-42";

struct Soap {
    endpoint: String,
    media_uri: String,
    advertised: bool,
}

#[async_trait::async_trait]
impl Transport for Soap {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        assert_eq!(url, self.endpoint);
        let response = match action.rsplit('/').next().unwrap() {
            "GetCapabilities" => {
                let replay = if self.advertised {
                    format!(
                        "<tt:Replay><tt:XAddr>{}</tt:XAddr></tt:Replay>",
                        self.endpoint
                    )
                } else {
                    String::new()
                };
                format!(
                    "<tds:GetCapabilitiesResponse><tds:Capabilities><tt:Extension>{replay}</tt:Extension></tds:Capabilities></tds:GetCapabilitiesResponse>"
                )
            }
            "GetServices" => "<tds:GetServicesResponse/>".to_owned(),
            "GetReplayUri" => {
                assert!(body.contains("<trp:RecordingToken>a&lt;&amp;b</trp:RecordingToken>"));
                assert!(body.contains("<tt:Stream>RTP-Unicast</tt:Stream>"));
                assert!(body.contains("<tt:Protocol>RTSP</tt:Protocol>"));
                format!(
                    "<trp:GetReplayUriResponse><trp:Uri>{}</trp:Uri></trp:GetReplayUriResponse>",
                    self.media_uri
                )
            }
            _ => panic!("unexpected SOAP action {action}"),
        };
        Ok(format!(
            concat!(
                r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" "#,
                r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl" "#,
                r#"xmlns:trp="http://www.onvif.org/ver10/replay/wsdl" "#,
                r#"xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body>{}</s:Body></s:Envelope>"#,
            ),
            response
        ))
    }
}

async fn session(address: &str, media_uri: String, tls: bool, advertised: bool) -> OnvifSession {
    let scheme = if tls { "https" } else { "http" };
    let endpoint = format!("{scheme}://{address}/device");
    OnvifSession::builder(&endpoint)
        .with_transport(Arc::new(Soap {
            endpoint: endpoint.clone(),
            media_uri,
            advertised,
        }))
        .build()
        .await
        .unwrap()
}

async fn fixture<F: Future<Output = ()> + Send + 'static>(
    script: impl FnOnce(TcpStream) -> F + Send + 'static,
) -> (OnvifSession, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let session = session(
        &address,
        format!("rtsp://embedded:uri-canary@{address}/replay/recording"),
        false,
        true,
    )
    .await;
    let peer = tokio::spawn(async move {
        timeout(Duration::from_secs(8), async {
            script(listener.accept().await.unwrap().0).await;
        })
        .await
        .expect("loopback peer must finish");
    });
    (session, peer)
}

struct Request {
    wire: String,
    cseq: u32,
}

impl Request {
    fn has(&self, header: &str) -> bool {
        self.wire
            .lines()
            .any(|line| line.eq_ignore_ascii_case(header))
    }

    fn path(&self) -> &str {
        self.wire.split_ascii_whitespace().nth(1).unwrap()
    }
}

async fn request(stream: &mut (impl AsyncRead + Unpin), method: &str) -> Request {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        assert!(bytes.len() < 16 * 1024);
        bytes.push(stream.read_u8().await.unwrap());
    }
    let wire = String::from_utf8(bytes).unwrap();
    assert!(wire.starts_with(&format!("{method} ")), "{wire}");
    assert!(!wire.contains("uri-canary"));
    let cseq = wire
        .lines()
        .find_map(|line| {
            let (header, value) = line.split_once(':')?;
            header
                .eq_ignore_ascii_case("CSeq")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    Request { wire, cseq }
}

async fn reply(
    stream: &mut (impl AsyncWrite + Unpin),
    command: &Request,
    status: &str,
    headers: &str,
    body: &str,
) {
    let data = format!(
        "RTSP/1.0 {status}\r\nCSeq: {}\r\n{headers}Content-Length: {}\r\n\r\n{body}",
        command.cseq,
        body.len(),
    );
    stream.write_all(data.as_bytes()).await.unwrap();
}

async fn describe(stream: &mut (impl AsyncRead + AsyncWrite + Unpin), digest: bool) {
    let mut describe = request(stream, "DESCRIBE").await;
    if digest {
        reply(
            stream,
            &describe,
            "401 Unauthorized",
            "WWW-Authenticate: Digest realm=\"cam\", nonce=\"nonce\", qop=\"auth\"\r\n",
            "",
        )
        .await;
        describe = request(stream, "DESCRIBE").await;
        assert!(
            describe
                .wire
                .to_ascii_lowercase()
                .contains("authorization: digest")
        );
        assert!(!describe.wire.contains("pass-canary"));
    }
    let content_base = describe.path().rsplit_once('/').unwrap().0;
    let headers = format!("Content-Type: application/sdp\r\nContent-Base: {content_base}/\r\n");
    let sdp = concat!(
        "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=Replay\r\nt=0 0\r\n",
        "a=control:aggregate\r\n",
        "m=video 0 RTP/AVP 97 96\r\na=rtpmap:97 H265/90000\r\n",
        "a=rtpmap:96 H264/90000\r\na=fmtp:96 packetization-mode=1\r\na=control:track\r\n",
    );
    reply(stream, &describe, "200 OK", &headers, sdp).await;
}

async fn setup(stream: &mut (impl AsyncRead + AsyncWrite + Unpin), seconds: u32, digest: bool) {
    describe(stream, digest).await;
    let setup = request(stream, "SETUP").await;
    assert!(setup.path().ends_with("/replay/track"));
    assert!(setup.has("Require: onvif-replay"));
    assert!(setup.has("Transport: RTP/AVP/TCP;unicast;interleaved=0-1"));
    let headers = format!(
        "Session: {SESSION};timeout={seconds}\r\nTransport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n",
    );
    reply(stream, &setup, "200 OK", &headers, "").await;
}

async fn play(stream: &mut (impl AsyncRead + Unpin), seek: bool) -> Request {
    let play = request(stream, "PLAY").await;
    assert!(play.path().ends_with("/replay/aggregate"));
    assert!(play.has("Require: onvif-replay"));
    assert!(play.has(&format!("Session: {SESSION}")));
    let range = if seek {
        "clock=20261117T001700Z-"
    } else {
        "clock=20261117T000203.25Z-"
    };
    assert!(play.has(&format!("Range: {range}")));
    assert_eq!(play.has("Immediate: yes"), seek);
    play
}

fn absolute(value: &str) -> time::OffsetDateTime {
    value.parse::<RecordingTimestamp>().unwrap().instant()
}

fn replay_request(value: &str) -> ReplayRequest {
    ReplayRequest::from(value.parse::<RecordingTimestamp>().unwrap())
}

fn packet(play: &Request, number: u16, date: &str, flags: u8) -> [u8; 33] {
    let time = absolute(date);
    let ntp_seconds = (time.unix_timestamp() + 2_208_988_800) as u32;
    let fraction = ((u64::from(time.nanosecond()) << 32) / 1_000_000_000) as u32;
    let mut bytes = [0u8; 33];
    bytes[..6].copy_from_slice(&[b'$', 0, 0, 29, 0x90, 0xe0]);
    bytes[6..8].copy_from_slice(&number.to_be_bytes());
    bytes[8..12].copy_from_slice(&u32::from(number).wrapping_mul(90_000).to_be_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_be_bytes()); // SSRC
    bytes[16..20].copy_from_slice(&[0xab, 0xac, 0, 3]);
    bytes[20..24].copy_from_slice(&ntp_seconds.to_be_bytes());
    bytes[24..28].copy_from_slice(&fraction.to_be_bytes());
    bytes[28] = flags;
    bytes[29] = play.cseq as u8;
    bytes[32] = 0x65;
    bytes
}

async fn teardown(stream: &mut (impl AsyncRead + AsyncWrite + Unpin), status: &str) {
    let stop = request(stream, "TEARDOWN").await;
    assert!(stop.path().ends_with("/replay/aggregate"));
    assert!(stop.has(&format!("Session: {SESSION}")));
    reply(stream, &stop, status, "", "").await;
}

#[tokio::test]
async fn starts_and_seeks_on_the_same_session_and_reports_only_evidenced_recording_gaps() {
    let (camera, peer) = fixture(|mut stream| async move {
        setup(&mut stream, 60, true).await;
        let first = play(&mut stream, false).await;
        stream
            .write_all(&packet(&first, 10, "2026-11-17T00:02:00Z", 0x80))
            .await
            .unwrap();
        reply(
            &mut stream,
            &first,
            "200 OK",
            "Range: clock=20261117T000201Z-\r\n",
            "",
        )
        .await;
        stream
            .write_all(&packet(&first, 11, "2026-11-17T00:02:01Z", 0x80))
            .await
            .unwrap();

        let seek = play(&mut stream, true).await;
        stream
            .write_all(&packet(&first, 12, "2026-11-17T00:02:02Z", 0x80))
            .await
            .unwrap();
        stream
            .write_all(&packet(&seek, 40, "2026-11-17T00:17:10Z", 0xa0))
            .await
            .unwrap();
        reply(&mut stream, &seek, "200 OK", "", "").await;
        stream
            .write_all(&packet(&first, 13, "2026-11-17T00:02:03Z", 0x80))
            .await
            .unwrap();
        for (number, date, flags) in [
            (41, "2026-11-17T00:17:11Z", 0x40),
            (42, "2026-11-17T00:17:50Z", 0x80),
            (43, "2026-11-17T00:17:51Z", 0x40),
            (45, "2026-11-17T00:18:00Z", 0xa0),
        ] {
            stream
                .write_all(&packet(&seek, number, date, flags))
                .await
                .unwrap();
        }
        teardown(&mut stream, "200 OK").await;
    })
    .await;
    let (mut owner, first) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            Some(("operator", "pass-canary")),
            &ReplayOptions::default(),
            future::pending(),
        )
        .await
        .unwrap();
    assert_eq!(owner.track().payload_type, 96);
    assert_eq!(owner.track().fmtp.as_deref(), Some("packetization-mode=1"));
    assert_eq!(first.requested_at, absolute(START));
    assert_eq!(
        first.server_reported_at,
        Some(absolute("2026-11-17T00:02:01Z"))
    );
    assert_eq!(first.observed_at, absolute("2026-11-17T00:02:00Z"));
    assert!(first.clean_point);
    let packet = owner.next().await.unwrap().unwrap();
    assert_eq!(packet.access_unit.unwrap().recorded_at, first.observed_at);

    let moved = owner.seek(replay_request(SEEK)).await.unwrap();
    assert_eq!(moved.requested_at, absolute(SEEK));
    assert_eq!(moved.observed_at, absolute("2026-11-17T00:17:10Z"));
    let packet = owner.next().await.unwrap().unwrap();
    assert_eq!(packet.access_unit.unwrap().recorded_at, moved.observed_at);
    assert!(packet.gap_before.is_none());
    let last = owner.next().await.unwrap().unwrap();
    assert!(last.access_unit.unwrap().section_end);
    assert!(last.gap_before.is_none());
    let resumed = owner.next().await.unwrap().unwrap();
    let gap = resumed.gap_before.unwrap();
    assert_eq!(
        gap.previous_section_ended_at,
        absolute("2026-11-17T00:17:11Z")
    );
    assert_eq!(
        gap.next_section_started_at,
        absolute("2026-11-17T00:17:50Z")
    );
    assert!(owner.next().await.unwrap().unwrap().gap_before.is_none());
    let loss = owner.next().await.unwrap().unwrap();
    assert!(loss.transport_discontinuity);
    assert!(loss.gap_before.is_none());
    let stop = owner.close().await;
    assert_eq!(stop.end, ReplayEnd::Closed);
    assert_eq!(stop.start, Some(moved));
    assert_eq!(stop.cleanup, ReplayCleanup::Released);
    assert_eq!(owner.close().await, stop);
    peer.await.unwrap();
}

#[tokio::test]
async fn distinguishes_missing_service_wrong_destination_and_explicit_replay_refusal() {
    for (advertised, kind) in [
        (false, ReplayErrorKind::MissingReplayService),
        (true, ReplayErrorKind::DestinationRejected),
    ] {
        let camera = session(
            "127.0.0.1:1",
            "rtsp://localhost:1/replay".into(),
            false,
            advertised,
        )
        .await;
        let Err(error) = camera
            .start_recording_replay(
                "a<&b",
                replay_request(START),
                None,
                &ReplayOptions::default(),
                future::pending(),
            )
            .await
        else {
            panic!("unexpected playback");
        };
        assert_eq!(error.kind, kind);
        assert_eq!(error.cleanup, ReplayCleanup::NotNeeded);
    }

    let (camera, peer) = fixture(|mut stream| async move {
        describe(&mut stream, false).await;
        let setup = request(&mut stream, "SETUP").await;
        assert!(setup.has("Require: onvif-replay"));
        reply(&mut stream, &setup, "551 Option Not Supported", "", "").await;
    })
    .await;
    let Err(error) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &ReplayOptions::default(),
            future::pending(),
        )
        .await
    else {
        panic!("unexpected playback");
    };
    assert_eq!(error.stage, ReplayStage::Setup);
    assert_eq!(error.kind, ReplayErrorKind::UnsupportedReplay);
    assert_eq!(error.cleanup, ReplayCleanup::NotNeeded);
    peer.await.unwrap();
}

#[tokio::test]
async fn accepted_play_with_unrelated_media_leaves_position_unconfirmed() {
    let (camera, peer) = fixture(|mut stream| async move {
        setup(&mut stream, 60, false).await;
        let play = play(&mut stream, false).await;
        reply(
            &mut stream,
            &play,
            "200 OK",
            "Range: clock=20261117T000203.25Z-\r\n",
            "",
        )
        .await;
        let mut old = packet(&play, 10, "2026-11-17T00:02:03.25Z", 0x80);
        old[29] = old[29].wrapping_sub(1);
        stream.write_all(&old).await.unwrap();
        teardown(&mut stream, "500 Internal Server Error").await;
    })
    .await;
    let options = ReplayOptions {
        startup_timeout: Duration::from_millis(750),
        ..Default::default()
    };
    let Err(error) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &options,
            future::pending(),
        )
        .await
    else {
        panic!("unexpected playback");
    };
    assert_eq!(error.stage, ReplayStage::Media);
    assert_eq!(error.kind, ReplayErrorKind::PositionUnconfirmed);
    assert!(error.start.is_none());
    assert_eq!(
        error.cleanup,
        ReplayCleanup::Failed(ReplayErrorKind::Refused(500))
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn cancels_a_pending_seek_with_no_overlapping_control_request() {
    let (seeking, seen) = oneshot::channel();
    let (camera, peer) = fixture(|mut stream| async move {
        setup(&mut stream, 60, false).await;
        let initial = play(&mut stream, false).await;
        reply(&mut stream, &initial, "200 OK", "", "").await;
        stream
            .write_all(&packet(&initial, 10, "2026-11-17T00:02:00Z", 0x80))
            .await
            .unwrap();
        let _seek = play(&mut stream, true).await;
        seeking.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
    })
    .await;
    let (mut owner, _) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &ReplayOptions::default(),
            future::pending(),
        )
        .await
        .unwrap();
    let cancel = owner.cancellation();
    let (seek, _) = tokio::join!(owner.seek(replay_request(SEEK)), async {
        seen.await.unwrap();
        cancel.cancel();
    });
    let error = seek.unwrap_err();
    assert_eq!(error.kind, ReplayErrorKind::Cancelled);
    assert_eq!(error.cleanup, ReplayCleanup::Unconfirmed);
    assert_eq!(owner.close().await.end, ReplayEnd::Cancelled);
    peer.await.unwrap();
}

#[tokio::test]
async fn slow_consumer_during_keep_alive_gets_an_independent_teardown() {
    let (closed, received) = oneshot::channel();
    let (camera, peer) = fixture(|mut stream| async move {
        setup(&mut stream, 1, false).await;
        let initial = play(&mut stream, false).await;
        reply(&mut stream, &initial, "200 OK", "", "").await;
        stream
            .write_all(&packet(&initial, 10, "2026-11-17T00:02:00Z", 0x80))
            .await
            .unwrap();
        let keep_alive = request(&mut stream, "GET_PARAMETER").await;
        assert!(keep_alive.has(&format!("Session: {SESSION}")));
        let burst: Vec<_> = (11..=266)
            .flat_map(|number| packet(&initial, number, "2026-11-17T00:02:01Z", 0x80))
            .collect();
        stream.write_all(&burst).await.unwrap();
        reply(&mut stream, &keep_alive, "200 OK", "", "").await;
        teardown(&mut stream, "200 OK").await;
        closed.send(()).unwrap();
    })
    .await;
    let options = ReplayOptions {
        max_buffered_bytes: 29,
        ..Default::default()
    };
    let (mut owner, _) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &options,
            future::pending(),
        )
        .await
        .unwrap();
    received.await.unwrap();
    assert!(owner.next().await.unwrap().is_some());
    let error = owner.next().await.unwrap_err();
    assert_eq!(error.kind, ReplayErrorKind::SlowConsumer);
    assert_eq!(error.cleanup, ReplayCleanup::Released);
    peer.await.unwrap();
}

#[tokio::test]
async fn verified_rtsps_keeps_terminal_access_unit_distinct_from_a_recording_gap() {
    use tokio_rustls::rustls::{ServerConfig, pki_types::PrivateKeyDer};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let cert = rcgen::generate_simple_self_signed(["127.0.0.1".to_owned()]).unwrap();
    let pem = cert.cert.pem();
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.cert.der().clone()],
            PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into()),
        )
        .unwrap();
    let peer = tokio::spawn(async move {
        timeout(Duration::from_secs(8), async {
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
            assert!(
                acceptor
                    .accept(listener.accept().await.unwrap().0)
                    .await
                    .is_err()
            );
            let mut stream = acceptor
                .accept(listener.accept().await.unwrap().0)
                .await
                .unwrap();
            setup(&mut stream, 60, false).await;
            let initial = play(&mut stream, false).await;
            reply(&mut stream, &initial, "200 OK", "", "").await;
            let mut start = packet(&initial, 10, "2026-11-17T00:02:00Z", 0xd0);
            start[5] &= 0x7f;
            stream.write_all(&start).await.unwrap();
            // Final fragment shares the RTP timestamp and carries no Replay
            // extension. The section only ends after this marker.
            let mut end = [0u8; 17];
            end[..16].copy_from_slice(&start[..16]);
            end[3] = 13;
            end[4] = 0x80;
            end[5] |= 0x80;
            end[6..8].copy_from_slice(&11u16.to_be_bytes());
            end[16] = 0x65;
            stream.write_all(&end).await.unwrap();
            teardown(&mut stream, "200 OK").await;
        })
        .await
        .unwrap();
    });
    let camera = session(
        &address,
        format!("rtsps://{address}/replay/recording"),
        true,
        true,
    )
    .await;
    let Err(error) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &ReplayOptions::default(),
            future::pending(),
        )
        .await
    else {
        panic!("unexpected TLS trust");
    };
    assert_eq!(error.kind, ReplayErrorKind::Tls);
    assert_eq!(error.stage, ReplayStage::Connect);
    let options = ReplayOptions {
        ca_certificates: vec![pem.into_bytes()],
        idle_timeout: Duration::from_millis(350),
        ..Default::default()
    };
    let (mut owner, _) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &options,
            future::pending(),
        )
        .await
        .unwrap();
    let first = owner.next().await.unwrap().unwrap();
    assert!(first.access_unit.unwrap().terminal);
    assert!(first.gap_before.is_none());
    let last = owner.next().await.unwrap().unwrap();
    assert!(last.access_unit.is_none());
    assert!(last.gap_before.is_none());
    assert!(owner.next().await.unwrap().is_none());
    let stop = owner.close().await;
    assert_eq!(stop.end, ReplayEnd::EndOfFootage);
    assert_eq!(stop.cleanup, ReplayCleanup::Released);
    peer.await.unwrap();
}

#[tokio::test]
async fn dropping_the_seek_future_preserves_its_result_and_new_media() {
    let (seeking, seen) = oneshot::channel();
    let (continue_play, wait) = oneshot::channel();
    let (camera, peer) = fixture(|mut stream| async move {
        setup(&mut stream, 60, false).await;
        let initial = play(&mut stream, false).await;
        reply(&mut stream, &initial, "200 OK", "", "").await;
        stream
            .write_all(&packet(&initial, 10, "2026-11-17T00:02:00Z", 0x80))
            .await
            .unwrap();
        let seek = play(&mut stream, true).await;
        seeking.send(()).unwrap();
        wait.await.unwrap();
        reply(&mut stream, &seek, "200 OK", "", "").await;
        stream
            .write_all(&packet(&seek, 40, SEEK, 0xa0))
            .await
            .unwrap();
        teardown(&mut stream, "200 OK").await;
    })
    .await;
    let (mut owner, _) = camera
        .start_recording_replay(
            "a<&b",
            replay_request(START),
            None,
            &ReplayOptions::default(),
            future::pending(),
        )
        .await
        .unwrap();
    tokio::select! {
        result = owner.seek(replay_request(SEEK)) => panic!("seek completed early: {result:?}"),
        result = seen => result.unwrap(),
    }
    continue_play.send(()).unwrap();
    let packet = owner.next().await.unwrap().unwrap();
    assert_eq!(packet.access_unit.unwrap().recorded_at, absolute(SEEK));
    owner.cancellation().cancel();
    let stop = owner.close().await;
    assert_eq!(stop.end, ReplayEnd::Cancelled);
    assert_eq!(stop.cleanup, ReplayCleanup::Released);
    peer.await.unwrap();
}
