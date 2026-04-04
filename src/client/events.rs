// ── Events Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{EventProperties, NotificationMessage, PullPointSubscription};
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
        const ACTION: &str =
            "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/RenewRequest";

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
            "http://www.onvif.org/ver10/events/wsdl/SubscriptionManager/UnsubscribeRequest";
        const BODY: &str = "<wsnt:Unsubscribe/>";

        let xml = self.call(subscription_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "UnsubscribeResponse")?;
        Ok(())
    }

    /// Wrap `pull_messages` polling into an infinite async stream of notification
    /// messages.
    ///
    /// Each `pull_messages` call fetches up to `max_messages` events and waits
    /// up to `wait_time` (ISO 8601 duration, e.g. `"PT5S"`) for at least one to
    /// arrive before returning. The stream yields individual messages one at a
    /// time; errors stop the stream.
    ///
    /// The stream is infinite — use [`futures::StreamExt::take`] or a `select!`
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
