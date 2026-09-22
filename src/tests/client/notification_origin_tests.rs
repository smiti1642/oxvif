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

#[test]
fn notify_http_framing_rejects_ambiguous_or_unsupported_lengths() {
    for request in [
        "POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\n",
        "POST / HTTP/1.0\r\ncOnTeNt-LeNgTh: 003\r\nX-Extra: ok\r\n\r\n",
    ] {
        assert_eq!(notify_content_length(request.as_bytes()).unwrap(), 3);
    }
    for headers in [
        "Content-Length: 1\r\nContent-Length: 1",
        "Content-Length: +1",
        "Content-Length: -1",
        "Content-Length: 1, 1",
        "Content-Length: ",
        "Content-Length: 184467440737095516160",
        "Content-Length: 1048577",
        "Transfer-Encoding: chunked",
        "Content-Length: 1\r\nTransfer-Encoding: identity",
        "Content-Length : 1",
        " Content-Length: 1",
        "broken",
        "Host: localhost",
    ] {
        let request = format!("POST / HTTP/1.1\r\n{headers}\r\n\r\n");
        assert!(
            notify_content_length(request.as_bytes()).is_err(),
            "{headers}"
        );
    }
    assert!(notify_content_length(b"GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n").is_err());
    assert!(
        notify_content_length(b"POST / HTTP/1.1\r\nX: \xff\r\nContent-Length: 0\r\n\r\n").is_err()
    );
    let boundary = format!("POST / HTTP/1.1\r\nContent-Length: {MAX_NOTIFY_BODY}\r\n\r\n");
    assert_eq!(
        notify_content_length(boundary.as_bytes()).unwrap(),
        MAX_NOTIFY_BODY
    );
}

async fn wait_for_permits(permits: &tokio::sync::Semaphore, available: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while permits.available_permits() != available {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("connection permit transition");
}

#[tokio::test]
async fn notify_deadline_closes_partial_headers_and_bodies_and_releases_capacity() {
    use tokio::io::AsyncReadExt;
    for partial in [
        &b"POST / HTTP/1.1\r\n"[..],
        &b"POST / HTTP/1.1\r\nContent-Length: 50\r\n\r\n<"[..],
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let task = tokio::spawn(serve_notifications_bounded(
            listener,
            tx,
            permits.clone(),
            Duration::from_millis(100),
        ));
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(partial).await.unwrap();
        wait_for_permits(&permits, 0).await;
        let mut byte = [0];
        let closed = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(closed, Ok(0) | Err(_)));
        wait_for_permits(&permits, 1).await;
        assert!(rx.try_recv().is_err());
        send(address, &envelope(&notification("after-timeout"))).await;
        let message = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_payload(&message.message, "after-timeout");
        drop(rx);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn notify_overload_closes_extra_socket_then_recovers_without_losing_origin() {
    use tokio::io::AsyncReadExt;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(serve_notifications_bounded(
        listener,
        tx,
        permits.clone(),
        Duration::from_secs(5),
    ));
    let held = tokio::net::TcpStream::connect(address).await.unwrap();
    wait_for_permits(&permits, 0).await;
    let mut refused = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_secs(2), refused.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(closed, Ok(0) | Err(_)));
    assert_eq!(permits.available_permits(), 0);
    assert!(rx.try_recv().is_err());
    drop(held);
    wait_for_permits(&permits, 1).await;
    let peer = send(address, &envelope(&notification("after-overload"))).await;
    let message = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(message.peer, peer);
    assert_payload(&message.message, "after-overload");
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn notify_invalid_framing_and_utf8_return_400_without_events() {
    use tokio::io::AsyncReadExt;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(serve_notifications(listener, tx));
    for request in [
        &b"POST / HTTP/1.1\r\nContent-Length: 1\r\n\r\n\xff"[..],
        &b"POST / HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 1\r\n\r\n"[..],
        &b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"[..],
    ] {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let _ = socket.write_all(request).await;
        let mut response = Vec::new();
        // Some platforms reset a socket closed with unread bytes; bytes already
        // read still establish the exact rejection response.
        let _ = tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response))
            .await
            .unwrap();
        assert!(
            response.starts_with(b"HTTP/1.1 400 Bad Request"),
            "{response:?}"
        );
        assert!(rx.try_recv().is_err());
    }
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn notify_header_limit_accepts_boundary_and_rejects_one_byte_over() {
    for size in [MAX_NOTIFY_HEADERS, MAX_NOTIFY_HEADERS + 1] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let prefix = "POST / HTTP/1.1\r\nContent-Length: 0\r\nX: ";
        let request = format!("{prefix}{}\r\n\r\n", "x".repeat(size - prefix.len() - 4));
        let reader = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_http_body(&mut socket).await
        });
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let _ = socket.write_all(request.as_bytes()).await;
        let result = tokio::time::timeout(Duration::from_secs(2), reader)
            .await
            .unwrap()
            .unwrap();
        if size == MAX_NOTIFY_HEADERS {
            assert_eq!(result.unwrap(), "");
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(error.to_string(), "headers too large");
        }
    }
}
