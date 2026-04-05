// ── Events Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{EventProperties, NotificationMessage, PullPointSubscription, PushSubscription};
use futures_core::Stream;

impl OnvifClient {
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

        let filter_el = filter
            .map(|f| {
                format!(
                    "<tev:Filter>\
                       <wsnt:TopicExpression \
                         Dialect=\"http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet\"\
                       >{f}</wsnt:TopicExpression>\
                     </tev:Filter>"
                )
            })
            .unwrap_or_default();

        let termination_el = initial_termination_time
            .map(|t| format!("<tev:InitialTerminationTime>{t}</tev:InitialTerminationTime>"))
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

        let filter_el = filter
            .map(|f| {
                format!(
                    "<wsnt:Filter>\
                       <wsnt:TopicExpression \
                         Dialect=\"http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet\"\
                       >{f}</wsnt:TopicExpression>\
                     </wsnt:Filter>"
                )
            })
            .unwrap_or_default();

        let termination_el = termination_time
            .map(|t| format!("<wsnt:InitialTerminationTime>{t}</wsnt:InitialTerminationTime>"))
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
    Box::pin(async_stream::stream! {
        let Ok(listener) = tokio::net::TcpListener::bind(bind_addr).await else { return; };
        loop {
            let Ok((mut conn, _)) = listener.accept().await else { break; };
            for msg in handle_notify_connection(&mut conn).await {
                yield msg;
            }
        }
    })
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
            break pos + 4;
        }
        if buf.len() > 131_072 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
    };

    let headers = std::str::from_utf8(&buf[..header_end]).unwrap_or("");
    let content_length: usize = headers
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split_once(':').map(|x| x.1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    let already_read = buf.len() - header_end;
    if content_length > already_read {
        buf.resize(header_end + content_length, 0);
        conn.read_exact(&mut buf[header_end + already_read..])
            .await?;
    }

    Ok(String::from_utf8_lossy(&buf[header_end..header_end + content_length]).into_owned())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
