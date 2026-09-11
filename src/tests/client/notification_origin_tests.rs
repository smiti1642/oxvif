//! Local transport-origin controls; no camera or subscription is required.
use super::*;
use futures::StreamExt;
use std::{net::SocketAddr, time::Duration};
use tokio::{io::AsyncWriteExt, net::TcpListener};

fn notification(value: &str) -> String {
    format!(
        r#"<wsnt:NotificationMessage><wsnt:Topic>tns1:Device/Trigger</wsnt:Topic>
      <wsnt:Message><tt:Message UtcTime="2026-01-01T00:00:00Z" PropertyOperation="Changed">
      <tt:Source><tt:SimpleItem Name="Token" Value="Source_1"/></tt:Source>
      <tt:Data><tt:SimpleItem Name="State" Value="{value}"/></tt:Data>
      </tt:Message></wsnt:Message></wsnt:NotificationMessage>"#
    )
}

fn envelope(messages: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
        xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
        xmlns:tt="http://www.onvif.org/ver10/schema"
        xmlns:tns1="http://www.onvif.org/ver10/topics">
        <s:Body><wsnt:Notify>{messages}</wsnt:Notify></s:Body></s:Envelope>"#
    )
}

async fn send(address: SocketAddr, body: &str) -> SocketAddr {
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let peer = socket.local_addr().unwrap();
    let request = format!(
        "POST /notify HTTP/1.1\r\nHost: localhost\r\nX-Forwarded-For: 203.0.113.9\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(request.as_bytes()).await.unwrap();
    peer
}

async fn next(
    stream: &mut std::pin::Pin<Box<dyn Stream<Item = ReceivedNotification> + Send>>,
) -> ReceivedNotification {
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .expect("notification")
}

fn assert_payload(message: &NotificationMessage, state: &str) {
    assert_eq!(message.topic, "tns1:Device/Trigger");
    assert_eq!(message.utc_time, "2026-01-01T00:00:00Z");
    assert_eq!(message.property_operation, "Changed");
    assert_eq!(message.source, [("Token".into(), "Source_1".into())].into());
    assert_eq!(message.data, [("State".into(), state.into())].into());
}

#[tokio::test]
async fn origin_preserves_payload_and_ignores_proxy_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut stream = notification_stream(listener);
    // Send before polling the stream: binding/readiness is not lazy.
    let peer = send(address, &envelope(&notification("true"))).await;
    let received = next(&mut stream).await;
    assert_eq!(received.peer, peer);
    assert_payload(&received.message, "true");
}

#[tokio::test]
async fn origin_distinguishes_identical_events_from_concurrent_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut stream = notification_stream(listener);
    let body = envelope(&notification("same"));
    let (a, b) = tokio::join!(send(address, &body), send(address, &body));
    assert_ne!(a, b);
    let mut actual = Vec::new();
    for _ in 0..2 {
        let received = next(&mut stream).await;
        assert_payload(&received.message, "same");
        actual.push(received.peer);
    }
    actual.sort();
    let mut expected = vec![a, b];
    expected.sort();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn origin_batch_keeps_each_payload_and_one_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut stream = notification_stream(listener);
    let body = envelope(&(notification("first") + &notification("second")));
    let peer = send(address, &body).await;
    for state in ["first", "second"] {
        let received = next(&mut stream).await;
        assert_eq!(received.peer, peer);
        assert_payload(&received.message, state);
    }
}

#[tokio::test]
async fn origin_bind_error_is_reported_without_port_reacquisition() {
    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result = notification_listener_with_peer(occupied.local_addr().unwrap()).await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("bind unexpectedly succeeded"),
    };
    assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    // Public constructor also succeeds on a genuinely available OS-assigned port.
    drop(
        notification_listener_with_peer("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn origin_drop_joins_idle_and_incomplete_connections() {
    for incomplete in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(256);
        let task = tokio::spawn(serve_notifications(listener, tx));
        let _connection = if incomplete {
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            socket
                .write_all(b"POST / HTTP/1.1\r\nContent-Length: 999\r\n\r\n<")
                .await
                .unwrap();
            // A complete request confirms that the server is processing connections.
            send(address, &envelope(&notification("barrier"))).await;
            let received = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .unwrap()
                .unwrap();
            assert_payload(&received.message, "barrier");
            Some(socket)
        } else {
            None
        };
        drop(rx);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        // Joining the supervisor includes JoinSet shutdown, not just accept abort.
        let rebound = TcpListener::bind(address).await.unwrap();
        assert_eq!(rebound.local_addr().unwrap(), address);
    }
}

#[tokio::test]
async fn origin_ipv6_loopback() {
    let listener = match TcpListener::bind("[::1]:0").await {
        Ok(listener) => listener,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
            ) =>
        {
            eprintln!("IPv6 loopback unavailable: {error}");
            return;
        }
        Err(error) => panic!("unexpected IPv6 bind failure: {error}"),
    };
    let address = listener.local_addr().unwrap();
    let mut stream = notification_stream(listener);
    let peer = send(address, &envelope(&notification("ipv6"))).await;
    let received = next(&mut stream).await;
    assert!(received.peer.is_ipv6());
    assert_eq!(received.peer, peer);
    assert_payload(&received.message, "ipv6");
}

#[tokio::test]
async fn origin_malformed_body_does_not_invent_events() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut stream = notification_stream(listener);
    send(address, "<invalid").await;
    let peer = send(address, &envelope(&notification("valid"))).await;
    let received = next(&mut stream).await;
    assert_eq!(received.peer, peer);
    assert_payload(&received.message, "valid");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.next())
            .await
            .is_err()
    );
}

#[test]
fn origin_fixture_declares_all_element_prefixes() {
    use quick_xml::{NsReader, events::Event, name::ResolveResult};
    let body = envelope(&notification("true"));
    for (xml, expected_unbound) in [
        (body.clone(), 0),
        (
            body.replace("xmlns:tt=\"http://www.onvif.org/ver10/schema\"", ""),
            8,
        ),
    ] {
        let mut reader = NsReader::from_str(&xml);
        let mut unbound = 0;
        loop {
            match reader.read_resolved_event().unwrap() {
                (ResolveResult::Unknown(_), Event::Start(_) | Event::Empty(_) | Event::End(_)) => {
                    unbound += 1
                }
                (_, Event::Eof) => break,
                _ => {}
            }
        }
        assert_eq!(unbound, expected_unbound);
    }
}

#[tokio::test]
async fn origin_legacy_mapping_keeps_payload_without_transport_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    let task = tokio::spawn(serve_notifications(listener, tx));
    let mut stream = legacy_notifications(rx);
    send(address, &envelope(&notification("legacy"))).await;
    let message = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_payload(&message, "legacy");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
}
