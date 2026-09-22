// ── Events Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    EventProperties, EventsServiceCapabilities, NotificationMessage, PullPointSubscription,
    PushSubscription, ReceivedNotification, xml_escape,
};
use futures_core::Stream;

impl OnvifClient {
    /// Ask the events service what it can do.
    ///
    /// `max_pull_points` is the number of concurrent
    /// [`create_pull_point_subscription`](Self::create_pull_point_subscription)
    /// endpoints the device will hold — the practical limit on how many
    /// independent event consumers one camera can serve.
    ///
    /// This type has **no `WSPullPointSupport`**; that attribute belongs to the
    /// device-level [`get_capabilities`](Self::get_capabilities) answer.
    pub async fn events_get_service_capabilities(
        &self,
        events_url: &str,
    ) -> Result<EventsServiceCapabilities, OnvifError> {
        // The events action URI carries a portType segment *and* a `Request`
        // suffix, unlike the other eight services.
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetServiceCapabilitiesRequest";
        const BODY: &str = "<tev:GetServiceCapabilities/>";

        let xml = self.call(events_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetServiceCapabilitiesResponse")?;
        EventsServiceCapabilities::from_xml(resp)
    }

    /// Retrieve all event topics advertised by the device.
    ///
    /// `events_url` is obtained from [`get_capabilities`](Self::get_capabilities)
    /// via `caps.events.url`.
    pub async fn get_event_properties(
        &self,
        events_url: &str,
    ) -> Result<EventProperties, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/EventPortType/GetEventPropertiesRequest";
        const BODY: &str = "<tev:GetEventProperties/>";

        let xml = self.call(events_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetEventPropertiesResponse")?;
        EventProperties::from_xml(resp)
    }

    /// Subscribe to device events using a pull-point endpoint.
    ///
    /// - `filter` — optional topic filter expression (e.g.
    ///   `"tns1:VideoSource/MotionAlarm"`); pass `None` to subscribe to all topics.
    /// - `initial_termination_time` — ISO 8601 duration or absolute time
    ///   (e.g. `"PT60S"`); pass `None` to use the device default.
    ///
    /// Returns a [`PullPointSubscription`] whose `reference_url` must be passed
    /// to [`pull_messages`](Self::pull_messages),
    /// [`renew_subscription`](Self::renew_subscription), and
    /// [`unsubscribe`](Self::unsubscribe).
    pub async fn create_pull_point_subscription(
        &self,
        events_url: &str,
        filter: Option<&str>,
        initial_termination_time: Option<&str>,
    ) -> Result<PullPointSubscription, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest";

        // Topic filter expressions are XML-escaped per XML spec requirements;
        // the device's XML parser will unescape them transparently.
        let filter_el = filter
            .map(|f| {
                format!(
                    "<tev:Filter>\
                       <wsnt:TopicExpression \
                         Dialect=\"http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet\"\
                       >{}</wsnt:TopicExpression>\
                     </tev:Filter>",
                    xml_escape(f)
                )
            })
            .unwrap_or_default();

        let termination_el = initial_termination_time
            .map(|t| {
                format!(
                    "<tev:InitialTerminationTime>{}</tev:InitialTerminationTime>",
                    xml_escape(t)
                )
            })
            .unwrap_or_default();

        let body = format!(
            "<tev:CreatePullPointSubscription>\
               {filter_el}{termination_el}\
             </tev:CreatePullPointSubscription>"
        );

        let xml = self.call(events_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreatePullPointSubscriptionResponse")?;
        PullPointSubscription::from_xml(resp)
    }

    /// Pull pending event messages from a subscription.
    ///
    /// - `subscription_url` — the `reference_url` from [`PullPointSubscription`].
    /// - `timeout` — ISO 8601 duration to long-poll for events (e.g. `"PT5S"`).
    /// - `max_messages` — maximum number of messages to return per call.
    pub async fn pull_messages(
        &self,
        subscription_url: &str,
        timeout: &str,
        max_messages: u32,
    ) -> Result<Vec<NotificationMessage>, OnvifError> {
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest";

        let timeout = xml_escape(timeout);
        let body = format!(
            "<tev:PullMessages>\
               <tev:Timeout>{timeout}</tev:Timeout>\
               <tev:MessageLimit>{max_messages}</tev:MessageLimit>\
             </tev:PullMessages>"
        );

        let xml = self.call(subscription_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "PullMessagesResponse")?;
        Ok(NotificationMessage::vec_from_xml(resp))
    }

    /// Extend the lifetime of an active pull-point subscription.
    ///
    /// `subscription_url` is the `reference_url` from [`PullPointSubscription`].
    /// `termination_time` is an ISO 8601 duration or absolute timestamp
    /// (e.g. `"PT60S"`).
    ///
    /// Returns the new termination timestamp set by the device.
    pub async fn renew_subscription(
        &self,
        subscription_url: &str,
        termination_time: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest";

        let termination_time = xml_escape(termination_time);
        let body = format!(
            "<wsnt:Renew>\
               <wsnt:TerminationTime>{termination_time}</wsnt:TerminationTime>\
             </wsnt:Renew>"
        );

        let xml = self.call(subscription_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "RenewResponse")?;
        Ok(resp
            .child("TerminationTime")
            .map(|n| n.text().to_string())
            .unwrap_or_default())
    }

    /// Request the device to generate a synchronisation point for all
    /// subscribed event sources.
    ///
    /// ONVIF Events WSDL `SetSynchronizationPoint` — Profile T §7.7 (mandatory).
    /// After calling this, the next `PullMessages` (or push notification)
    /// will contain the current state of all event sources, allowing the
    /// client to synchronise its view of the world.
    ///
    /// `subscription_url` is the `reference_url` from [`PullPointSubscription`].
    pub async fn set_synchronization_point(
        &self,
        subscription_url: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest";
        const BODY: &str = "<tev:SetSynchronizationPoint/>";

        let xml = self.call(subscription_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetSynchronizationPointResponse")?;
        Ok(())
    }

    /// Cancel an active pull-point subscription.
    ///
    /// `subscription_url` is the `reference_url` from [`PullPointSubscription`].
    pub async fn unsubscribe(&self, subscription_url: &str) -> Result<(), OnvifError> {
        const ACTION: &str =
            "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest";
        const BODY: &str = "<wsnt:Unsubscribe/>";

        let xml = self.call(subscription_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "UnsubscribeResponse")?;
        Ok(())
    }

    /// Subscribe to device events using the WS-BaseNotification push model.
    ///
    /// The device will HTTP-POST `Notify` messages directly to `consumer_url`
    /// whenever events occur — no polling required.  Use
    /// [`notification_listener`] to start a TCP server that receives those
    /// messages as an async stream.
    ///
    /// - `consumer_url` — the URL the device will POST notifications to
    ///   (e.g. `"http://192.168.1.50:8080/notify"`).
    /// - `filter` — optional topic filter (e.g. `"tns1:VideoSource/MotionAlarm"`);
    ///   `None` subscribes to all topics.
    /// - `termination_time` — ISO 8601 duration or absolute timestamp
    ///   (e.g. `"PT60S"`); `None` uses the device default.
    ///
    /// Returns a [`PushSubscription`] whose `subscription_reference` can be
    /// passed to [`renew_subscription`](Self::renew_subscription) and
    /// [`unsubscribe`](Self::unsubscribe).
    pub async fn subscribe(
        &self,
        events_url: &str,
        consumer_url: &str,
        filter: Option<&str>,
        termination_time: Option<&str>,
    ) -> Result<PushSubscription, OnvifError> {
        const ACTION: &str =
            "http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/SubscribeRequest";

        let consumer_url = xml_escape(consumer_url);
        let filter_el = filter
            .map(|f| {
                format!(
                    "<wsnt:Filter>\
                       <wsnt:TopicExpression \
                         Dialect=\"http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet\"\
                       >{}</wsnt:TopicExpression>\
                     </wsnt:Filter>",
                    xml_escape(f)
                )
            })
            .unwrap_or_default();

        let termination_el = termination_time
            .map(|t| {
                format!(
                    "<wsnt:InitialTerminationTime>{}</wsnt:InitialTerminationTime>",
                    xml_escape(t)
                )
            })
            .unwrap_or_default();

        let body = format!(
            "<wsnt:Subscribe>\
               <wsnt:ConsumerReference>\
                 <wsa:Address>{consumer_url}</wsa:Address>\
               </wsnt:ConsumerReference>\
               {filter_el}{termination_el}\
             </wsnt:Subscribe>"
        );

        let xml = self.call(events_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SubscribeResponse")?;
        PushSubscription::from_xml(resp)
    }

    /// Wrap `pull_messages` polling into an infinite async stream of notification
    /// messages.
    ///
    /// Each `pull_messages` call fetches up to `max_messages` events and waits
    /// up to `wait_time` (ISO 8601 duration, e.g. `"PT5S"`) for at least one to
    /// arrive before returning. The stream yields individual messages one at a
    /// time; errors stop the stream.
    ///
    /// The stream is infinite — use `StreamExt::take` or a `select!`
    /// block to bound it, and call [`unsubscribe`](Self::unsubscribe) when done.
    ///
    /// # Example (requires `futures` in caller's `[dependencies]`)
    ///
    /// ```no_run
    /// use futures::StreamExt as _;
    ///
    /// # async fn example(session: oxvif::OnvifSession) -> Result<(), oxvif::OnvifError> {
    /// let sub = session.create_pull_point_subscription(None, Some("PT60S")).await?;
    /// let mut stream = session.event_stream(&sub.reference_url, "PT5S", 10);
    /// while let Some(Ok(msg)) = stream.next().await {
    ///     println!("Event: {} {:?}", msg.topic, msg.data);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn event_stream<'a>(
        &'a self,
        subscription_url: &'a str,
        timeout: &'a str,
        max_messages: u32,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Result<NotificationMessage, OnvifError>> + 'a>> {
        Box::pin(async_stream::try_stream! {
            loop {
                let messages = self
                    .pull_messages(subscription_url, timeout, max_messages)
                    .await?;
                for msg in messages {
                    yield msg;
                }
            }
        })
    }
}

// ── notification_listener ─────────────────────────────────────────────────────

/// Start a minimal HTTP server that receives ONVIF push-event `Notify` POSTs
/// and yields the parsed [`NotificationMessage`] items as an infinite async stream.
///
/// `bind_addr` is the local address to listen on (e.g. `"0.0.0.0:8080".parse()?`).
/// Pass the corresponding public URL as `consumer_url` to
/// [`subscribe`](super::OnvifClient::subscribe).
///
/// The stream is infinite — use `StreamExt::take` or `tokio::select!` to stop
/// it, then call [`unsubscribe`](super::OnvifClient::unsubscribe) to cancel the
/// device subscription.
///
/// Bind failures close this legacy stream without a diagnostic. Prefer
/// [`notification_listener_with_peer`] for bind errors, readiness and TCP origin.
/// Dropping either stream stops accepting and cancels its owned connections;
/// device-side subscriptions must still be cancelled separately.
/// Both listeners allow at most 32 active connections, with a 10-second deadline
/// for reading a request and writing its acknowledgment. Headers are limited to
/// 128 KiB and the UTF-8 body to 1 MiB. Only HTTP/1.0 or HTTP/1.1 POST with one
/// decimal `Content-Length` is supported; `Transfer-Encoding` is rejected.
/// Invalid framing gets a best-effort HTTP 400 and yields no events; overload or
/// deadline expiry closes the socket. A full event queue holds connection slots
/// until consumed; queue delivery is outside the request deadline.
/// This minimal HTTP listener is not an authenticated Internet-facing server.
///
/// # Example
///
/// ```no_run
/// use futures::StreamExt as _;
/// use std::net::SocketAddr;
///
/// # async fn example(client: oxvif::OnvifClient, events_url: &str) -> Result<(), oxvif::OnvifError> {
/// let bind: SocketAddr = "0.0.0.0:8080".parse().unwrap();
/// let consumer_url = "http://192.168.1.50:8080/notify";
///
/// let mut stream = oxvif::notification_listener(bind);
/// let sub = client.subscribe(events_url, consumer_url, None, Some("PT60S")).await?;
///
/// while let Some(msg) = stream.next().await {
///     println!("Push event: {} {:?}", msg.topic, msg.data);
/// }
/// client.unsubscribe(&sub.subscription_reference).await?;
/// # Ok(())
/// # }
/// ```
pub fn notification_listener(
    bind_addr: std::net::SocketAddr,
) -> std::pin::Pin<Box<dyn futures_core::Stream<Item = NotificationMessage> + Send>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<ReceivedNotification>(256);

    // Spawn a background task that accepts connections concurrently so that
    // rapid-fire notifications from one or more devices are not serialised.
    tokio::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::bind(bind_addr).await else {
            return;
        };
        serve_notifications(listener, tx).await;
    });

    legacy_notifications(rx)
}

fn legacy_notifications(
    mut rx: tokio::sync::mpsc::Receiver<ReceivedNotification>,
) -> std::pin::Pin<Box<dyn Stream<Item = NotificationMessage> + Send>> {
    Box::pin(async_stream::stream! {
        while let Some(msg) = rx.recv().await {
            yield msg.message;
        }
    })
}

/// Bind a minimal push-event listener and report each notification's TCP origin.
///
/// Binding completes before this function returns: the listener is ready even
/// before the stream is first polled. Each [`ReceivedNotification`] contains the
/// actual socket peer, not an XML or proxy-header identity. Behind NAT or a proxy,
/// that address may not identify a camera uniquely and is not authentication.
/// Dropping the stream stops accepting and cancels owned connection tasks.
/// Cancel the device subscription separately with [`OnvifClient::unsubscribe`].
/// It shares the connection, deadline and framing limits documented on
/// [`notification_listener`]; it does not add TLS or auth.
///
/// # Errors
/// Returns the operating system's bind error (for example, address already in use).
/// Subsequent accept failure ends the stream; per-request parse failures yield no
/// notifications. It is not a diagnostic stream for connection errors.
///
/// # Example
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use futures::StreamExt as _;
/// let mut events = oxvif::notification_listener_with_peer("127.0.0.1:8080".parse()?).await?;
/// while let Some(received) = events.next().await {
///     // Exporting the address is an explicit application decision.
///     println!("{}: {}", received.peer, received.message.topic);
/// }
/// # Ok(()) }
/// ```
pub async fn notification_listener_with_peer(
    bind_addr: std::net::SocketAddr,
) -> std::io::Result<std::pin::Pin<Box<dyn Stream<Item = ReceivedNotification> + Send>>> {
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    Ok(notification_stream(listener))
}

fn notification_stream(
    listener: tokio::net::TcpListener,
) -> std::pin::Pin<Box<dyn Stream<Item = ReceivedNotification> + Send>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(serve_notifications(listener, tx));
    Box::pin(async_stream::stream! {
        while let Some(received) = rx.recv().await {
            yield received;
        }
    })
}

async fn serve_notifications(
    listener: tokio::net::TcpListener,
    tx: tokio::sync::mpsc::Sender<ReceivedNotification>,
) {
    serve_notifications_bounded(
        listener,
        tx,
        std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_NOTIFY_CONNECTIONS)),
        NOTIFY_REQUEST_TIMEOUT,
    )
    .await;
}

// Limits are private policy parameters so the same path can be exercised with
// short deadlines and a one-connection budget without slow timing-based tests.
async fn serve_notifications_bounded(
    listener: tokio::net::TcpListener,
    tx: tokio::sync::mpsc::Sender<ReceivedNotification>,
    permits: std::sync::Arc<tokio::sync::Semaphore>,
    request_timeout: std::time::Duration,
) {
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = tx.closed() => break,
            _ = connections.join_next(), if !connections.is_empty() => {},
            accepted = listener.accept() => {
                let Ok((mut conn, peer)) = accepted else { break };
                let Ok(permit) = permits.clone().try_acquire_owned() else {
                    // Overload closes the newly accepted socket without spawning
                    // work or consuming another body buffer.
                    continue;
                };
                let tx = tx.clone();
                connections.spawn(async move {
                    let _permit = permit;
                    let Ok(messages) = tokio::time::timeout(
                        request_timeout, handle_notify_connection(&mut conn),
                    ).await else { return };
                    for message in messages {
                        if tx.send(ReceivedNotification { message, peer }).await.is_err() {
                            break;
                        }
                    }
                });
            }
        }
    }
    connections.shutdown().await;
}

async fn handle_notify_connection(conn: &mut tokio::net::TcpStream) -> Vec<NotificationMessage> {
    use tokio::io::AsyncWriteExt;

    let body = match read_http_body(conn).await {
        Ok(b) => b,
        Err(_) => {
            let _ = conn
                .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                .await;
            return vec![];
        }
    };
    let _ = conn
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await;

    let Ok(root) = crate::soap::XmlNode::parse(&body) else {
        return vec![];
    };
    let body_el = root.child("Body").unwrap_or(&root);
    let notify = body_el.child("Notify").unwrap_or(body_el);
    NotificationMessage::vec_from_xml(notify)
}

/// Maximum accepted notification body size (1 MiB).  ONVIF Notify messages
/// are small XML documents; anything larger is almost certainly not a
/// legitimate notification.
const MAX_NOTIFY_BODY: usize = 1_048_576;
const MAX_NOTIFY_HEADERS: usize = 131_072;
const MAX_NOTIFY_CONNECTIONS: usize = 32;
const NOTIFY_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn invalid_notify_request(reason: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, reason)
}

fn notify_content_length(headers: &[u8]) -> std::io::Result<usize> {
    let text = std::str::from_utf8(headers)
        .map_err(|_| invalid_notify_request("invalid header encoding"))?;
    let mut lines = text.split("\r\n");
    let parts = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    if !matches!(parts.as_slice(), ["POST", _, "HTTP/1.0" | "HTTP/1.1"]) {
        return Err(invalid_notify_request("expected HTTP POST"));
    }
    let mut length = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid_notify_request("malformed header"))?;
        if name.is_empty() || name.bytes().any(|b| b.is_ascii_whitespace()) {
            return Err(invalid_notify_request("malformed header name"));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid_notify_request("transfer encoding is unsupported"));
        }
        if name.eq_ignore_ascii_case("content-length") {
            let value = value.trim();
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid_notify_request(
                    "invalid or duplicate content length",
                ));
            }
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| invalid_notify_request("invalid content length"))?,
            );
        }
    }
    let length = length.ok_or_else(|| invalid_notify_request("missing content length"))?;
    if length > MAX_NOTIFY_BODY {
        return Err(invalid_notify_request("notification body too large"));
    }
    Ok(length)
}

async fn read_http_body(conn: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    use tokio::io::AsyncReadExt;

    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

    // Read until we find the HTTP header terminator.
    let header_end = loop {
        let n = conn.read(&mut tmp).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before headers",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            if pos + 4 > MAX_NOTIFY_HEADERS {
                return Err(invalid_notify_request("headers too large"));
            }
            break pos + 4;
        }
        if buf.len() >= MAX_NOTIFY_HEADERS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
    };

    let content_length = notify_content_length(&buf[..header_end])?;

    let already_read = buf.len() - header_end;
    if content_length > already_read {
        buf.resize(header_end + content_length, 0);
        conn.read_exact(&mut buf[header_end + already_read..])
            .await?;
    }

    String::from_utf8(buf[header_end..header_end + content_length].to_vec())
        .map_err(|_| invalid_notify_request("invalid notification UTF-8"))
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
#[path = "../tests/client/events_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/client/notification_origin_tests.rs"]
mod origin_tests;
