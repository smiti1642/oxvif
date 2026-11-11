#![cfg(feature = "stream-probe")]

use std::{future, sync::Arc, time::Duration};

use oxvif::stream_probe::{
    StreamOutcome as Outcome, StreamProbeOptions, VideoCodec, probe, probe_until,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

async fn request(stream: &mut (impl AsyncRead + Unpin), method: &str) -> (String, u32) {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(stream.read_u8().await.unwrap());
    }
    let request = String::from_utf8(bytes).unwrap();
    assert!(request.starts_with(&format!("{method} ")), "{request}");
    assert!(!request.contains("uri-secret"), "{request}");
    let sequence = request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("CSeq")
                .then(|| value.trim().parse::<u32>().unwrap())
        })
        .unwrap();
    (request, sequence)
}

async fn response(
    stream: &mut (impl AsyncWrite + Unpin),
    sequence: u32,
    status: &str,
    headers: &str,
    body: &str,
) {
    let response = format!(
        "RTSP/1.0 {status}\r\nCSeq: {sequence}\r\n{headers}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    for part in response.as_bytes().chunks(17) {
        stream.write_all(part).await.unwrap();
        tokio::task::yield_now().await;
    }
}

fn sdp(codec: &str, control: &str) -> String {
    format!(
        "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=Camera\r\nt=0 0\r\nm=video 0 RTP/AVP 96\r\na=rtpmap:96 {codec}/90000\r\na=control:{control}\r\n"
    )
}

async fn playback_server(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    send_packet: bool,
    playing: Option<oneshot::Sender<()>>,
) {
    let (options, seq) = request(stream, "OPTIONS").await;
    assert!(!options.to_ascii_lowercase().contains("authorization:"));
    response(
        stream,
        seq,
        "401 Unauthorized",
        "WWW-Authenticate: Digest realm=\"cam\", nonce=\"challenge\", qop=\"auth\"\r\n",
        "",
    )
    .await;
    let (describe, seq) = request(stream, "DESCRIBE").await;
    assert!(
        describe
            .to_ascii_lowercase()
            .contains("authorization: digest")
    );
    assert!(describe.contains("query-secret"));
    response(
        stream,
        seq,
        "200 OK",
        "Content-Type: application/sdp\r\n",
        &sdp("H264", "track"),
    )
    .await;
    let (setup, seq) = request(stream, "SETUP").await;
    assert!(setup.lines().next().unwrap().contains("/track "));
    assert!(setup.contains("interleaved=0-1"));
    response(
        stream,
        seq,
        "200 OK",
        "Session: session123;timeout=20\r\nTransport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n",
        "",
    )
    .await;
    let (play, seq) = request(stream, "PLAY").await;
    assert!(play.to_ascii_lowercase().contains("session: session123"));
    response(stream, seq, "200 OK", "", "").await;
    if send_packet {
        let mut packet = [0; 17];
        packet[..6].copy_from_slice(&[b'$', 0, 0, 13, 0x80, 96]);
        stream.write_all(&packet).await.unwrap();
    }
    if let Some(playing) = playing {
        playing.send(()).unwrap();
    }
    let (_, seq) = request(stream, "TEARDOWN").await;
    response(stream, seq, "200 OK", "", "").await;
}

#[tokio::test]
async fn digest_fragmented_response_and_first_packet() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        playback_server(&mut listener.accept().await.unwrap().0, true, None).await;
    });
    let uri = format!("rtsp://uri-secret:uri-secret@{addr}/stream?query-secret");
    let report = probe(
        &uri,
        &format!("http://{addr}/device"),
        Some(("tester", "account-secret")),
        &StreamProbeOptions {
            observe_packet: true,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(report.connection, Outcome::Passed);
    assert_eq!(report.options, Outcome::AuthenticationRequired);
    assert_eq!(report.describe, Outcome::Passed);
    assert_eq!(report.codec, Some(VideoCodec::H264));
    assert_eq!(report.setup, Outcome::Passed);
    assert_eq!(report.play, Outcome::Passed);
    assert_eq!(report.packet, Outcome::Passed);
    assert_eq!(report.teardown, Outcome::Passed);
    assert!(!format!("{report:?}").contains("secret"));
    server.await.unwrap();
}

#[tokio::test]
async fn packet_timeout_and_cancellation_both_teardown() {
    for cancel in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (playing, ready) = oneshot::channel();
        let server = tokio::spawn(async move {
            playback_server(
                &mut listener.accept().await.unwrap().0,
                false,
                Some(playing),
            )
            .await;
        });
        let report = probe_until(
            &format!("rtsp://{addr}/stream?query-secret"),
            &format!("http://{addr}"),
            Some(("tester", "account-secret")),
            &StreamProbeOptions {
                observe_packet: true,
                packet_timeout: Duration::from_millis(200),
                ..Default::default()
            },
            async move {
                ready.await.unwrap();
                if cancel {
                    // Wait until the client has consumed the reply on the local socket.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                } else {
                    future::pending::<()>().await;
                }
            },
        )
        .await;
        assert_eq!(report.play, Outcome::Passed);
        assert_eq!(
            report.packet,
            if cancel {
                Outcome::Cancelled
            } else {
                Outcome::TimedOut
            }
        );
        assert_eq!(report.teardown, Outcome::Passed);
        server.await.unwrap();
    }
}

#[tokio::test]
async fn refusal_malformed_correlation_auth_and_codec_are_distinct() {
    for (case, options, describe) in [
        ("redirect", Outcome::Refused(302), Outcome::Skipped),
        ("cseq", Outcome::InvalidResponse, Outcome::Skipped),
        ("limit", Outcome::Limit, Outcome::Skipped),
        ("basic", Outcome::Passed, Outcome::AuthenticationRequired),
        ("digest", Outcome::Passed, Outcome::AuthenticationRequired),
        ("codec", Outcome::Passed, Outcome::UnsupportedCodec),
        ("control", Outcome::Passed, Outcome::Passed),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut stream = listener.accept().await.unwrap().0;
            let (_, seq) = request(&mut stream, "OPTIONS").await;
            match case {
                "redirect" => {
                    return response(
                        &mut stream,
                        seq,
                        "302 Moved",
                        "Location: rtsp://other.invalid\r\n",
                        "",
                    )
                    .await;
                }
                "cseq" => return response(&mut stream, seq + 1, "200 OK", "", "").await,
                "limit" => {
                    stream
                        .write_all(b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 65537\r\n\r\n")
                        .await
                        .unwrap();
                    return;
                }
                _ => response(&mut stream, seq, "200 OK", "", "").await,
            }
            let (first, seq) = request(&mut stream, "DESCRIBE").await;
            assert!(!first.to_ascii_lowercase().contains("authorization:"));
            match case {
                "basic" => {
                    response(
                        &mut stream,
                        seq,
                        "401 Unauthorized",
                        "WWW-Authenticate: Basic realm=\"cam\"\r\n",
                        "",
                    )
                    .await
                }
                "digest" => {
                    response(
                        &mut stream,
                        seq,
                        "401 Unauthorized",
                        "WWW-Authenticate: Digest realm=\"cam\", nonce=\"challenge\"\r\n",
                        "",
                    )
                    .await;
                    let (second, seq) = request(&mut stream, "DESCRIBE").await;
                    assert!(
                        second
                            .to_ascii_lowercase()
                            .contains("authorization: digest")
                    );
                    response(&mut stream, seq, "401 Unauthorized", "", "").await;
                }
                "codec" => {
                    response(
                        &mut stream,
                        seq,
                        "200 OK",
                        "Content-Type: application/sdp\r\n",
                        &sdp("unknown", "track"),
                    )
                    .await
                }
                "control" => {
                    response(
                        &mut stream,
                        seq,
                        "200 OK",
                        "Content-Type: application/sdp\r\n",
                        &sdp("H265", "rtsp://other.invalid/track"),
                    )
                    .await
                }
                _ => unreachable!(),
            }
            let mut buf = [0; 1];
            assert_eq!(stream.read(&mut buf).await.unwrap(), 0);
        });
        let report = probe(
            &format!("rtsp://{addr}/stream"),
            "http://127.0.0.1/device",
            Some(("test", "test")),
            &StreamProbeOptions {
                observe_packet: true,
                ..Default::default()
            },
        )
        .await;
        assert_eq!(report.options, options, "{case}");
        assert_eq!(report.describe, describe, "{case}");
        if case == "control" {
            assert_eq!(report.setup, Outcome::DestinationRejected);
        }
        server.await.unwrap();
    }
}

#[tokio::test]
async fn ipv6_and_verified_rtsps() {
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cert = rcgen::generate_simple_self_signed(["::1".to_string()]).unwrap();
    let pem = cert.cert.pem();
    let config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.cert.der().clone()],
            tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(
                cert.signing_key.serialize_der().into(),
            ),
        )
        .unwrap();
    let server = tokio::spawn(async move {
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let (socket, _) = listener.accept().await.unwrap();
        assert!(acceptor.accept(socket).await.is_err());
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        let (_, seq) = request(&mut stream, "OPTIONS").await;
        response(
            &mut stream,
            seq,
            "401 Unauthorized",
            "WWW-Authenticate: Basic realm=\"cam\"\r\n",
            "",
        )
        .await;
        let (describe, seq) = request(&mut stream, "DESCRIBE").await;
        assert!(
            describe
                .to_ascii_lowercase()
                .contains("authorization: basic")
        );
        response(
            &mut stream,
            seq,
            "200 OK",
            "Content-Type: application/sdp\r\n",
            &sdp("H265", "track"),
        )
        .await;
    });
    let uri = format!("rtsps://{addr}/stream");
    let device = "https://[::1]/device";
    assert_eq!(
        probe(&uri, device, None, &StreamProbeOptions::default())
            .await
            .connection,
        Outcome::Tls
    );
    let report = probe(
        &uri,
        device,
        Some(("test", "test")),
        &StreamProbeOptions {
            ca_certificates: vec![pem.into_bytes()],
            ..Default::default()
        },
    )
    .await;
    assert_eq!(report.connection, Outcome::Passed);
    assert_eq!(report.options, Outcome::AuthenticationRequired);
    assert_eq!(report.codec, Some(VideoCodec::H265));
    server.await.unwrap();
    for uri in [
        "rtsp://127.0.0.1/stream",
        "rtsp://[::1]/stream",
        "http://[::1]/stream",
        "rtsp://[::1]/\r\nx",
    ] {
        assert_eq!(
            probe(uri, device, None, &StreamProbeOptions::default())
                .await
                .connection,
            Outcome::DestinationRejected
        );
    }
}

#[cfg(feature = "talkback")]
#[tokio::test]
async fn talkback_negotiates_send_track_paces_pcmu_and_tears_down() {
    use oxvif::stream_probe::talkback::{InputError, Talkback, TalkbackEnd, TalkbackOptions};

    let (full, _unused) = Talkback::channel();
    for _ in 0..16 {
        full.try_send([0; 160]).unwrap();
    }
    assert_eq!(full.try_send([0; 160]), Err(InputError::Full));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let (describe, seq) = request(&mut stream, "DESCRIBE").await;
        assert!(describe.contains("www.onvif.org/ver20/backchannel"));
        let sdp = concat!(
            "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=Speaker\r\nt=0 0\r\n",
            "m=audio 0 RTP/AVP 0\r\na=recvonly\r\na=control:listen\r\n",
            "m=audio 0 RTP/AVP 97 98\r\na=sendonly\r\na=control:talk\r\na=rtpmap:97 PCMU/8000\r\na=rtpmap:98 G726-24/8000\r\n"
        );
        response(
            &mut stream,
            seq,
            "200 OK",
            "Content-Type: application/sdp\r\n",
            sdp,
        )
        .await;
        let (setup, seq) = request(&mut stream, "SETUP").await;
        assert!(setup.lines().next().unwrap().contains("/talk "));
        assert!(setup.contains("www.onvif.org/ver20/backchannel"));
        response(
            &mut stream,
            seq,
            "200 OK",
            "Session: talk-session\r\nTransport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n",
            "",
        )
        .await;
        let (play, seq) = request(&mut stream, "PLAY").await;
        assert!(play.to_ascii_lowercase().contains("session: talk-session"));
        response(&mut stream, seq, "200 OK", "", "").await;
        let mut first = [0; 176];
        let mut second = [0; 176];
        stream.read_exact(&mut first).await.unwrap();
        stream.read_exact(&mut second).await.unwrap();
        assert_eq!(&first[..6], &[b'$', 0, 0, 172, 0x80, 0x80 | 97]);
        assert_eq!(second[5] & 0x7f, 97);
        assert_eq!(&first[12..16], &second[12..16]);
        let sequence = |packet: &[u8]| u16::from_be_bytes(packet[6..8].try_into().unwrap());
        let timestamp = |packet: &[u8]| u32::from_be_bytes(packet[8..12].try_into().unwrap());
        assert_eq!(sequence(&second), sequence(&first).wrapping_add(1));
        assert!(timestamp(&second).wrapping_sub(timestamp(&first)) >= 160);
        assert_eq!(&first[16..21], &[0xff, 0x80, 0x00, 0xce, 0x4e]);
        let (stop, seq) = request(&mut stream, "TEARDOWN").await;
        assert!(stop.contains("www.onvif.org/ver20/backchannel"));
        response(&mut stream, seq, "200 OK", "", "").await;
    });
    let (input, owner) = Talkback::channel();
    let mut frame = [0; 160];
    frame[..5].copy_from_slice(&[0, 32124, -32124, 1000, -1000]);
    input.send(frame).await.unwrap();
    input.send(frame).await.unwrap();
    drop(input);
    let report = owner
        .run_until(
            &format!("rtsp://{addr}/stream"),
            "http://127.0.0.1/device",
            None,
            &TalkbackOptions::default(),
            future::pending(),
        )
        .await;
    assert!(report.negotiated);
    assert_eq!(report.sent_frames, 2);
    assert_eq!(report.end, TalkbackEnd::InputClosed);
    assert_eq!(report.teardown, Outcome::Passed);
    server.await.unwrap();
}

#[cfg(feature = "talkback")]
#[tokio::test]
async fn talkback_rejects_receive_only_track_without_setup() {
    use oxvif::stream_probe::talkback::{Talkback, TalkbackEnd, TalkbackOptions};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let (_, seq) = request(&mut stream, "DESCRIBE").await;
        response(&mut stream, seq, "200 OK", "Content-Type: application/sdp\r\n",
            "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=Camera\r\nt=0 0\r\nm=audio 0 RTP/AVP 0\r\na=control:listen\r\na=recvonly\r\n").await;
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
    });
    let (_input, owner) = Talkback::channel();
    let report = owner
        .run_until(
            &format!("rtsp://{addr}/stream"),
            "http://127.0.0.1/device",
            None,
            &TalkbackOptions::default(),
            future::pending(),
        )
        .await;
    assert!(!report.negotiated);
    assert_eq!(report.end, TalkbackEnd::Stopped(Outcome::UnsupportedCodec));
    assert_eq!(report.sent_frames, 0);
    server.await.unwrap();
}
