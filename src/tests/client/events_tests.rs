//! Unit tests for the Events methods on `OnvifClient`
//! (`src/client/events.rs`).

use super::*;
use crate::tests::common::*;
use futures::StreamExt as _;

// ── Events service fixtures ───────────────────────────────────────────────

fn event_properties_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
          <s:Body>
            <tev:GetEventPropertiesResponse>
              <tev:TopicSet>
                <tns1:VideoSource xmlns:tns1="http://www.onvif.org/ver10/topics">
                  <tns1:MotionAlarm/>
                  <tns1:ImageTooBlurry/>
                </tns1:VideoSource>
                <tns1:RuleEngine xmlns:tns1="http://www.onvif.org/ver10/topics">
                  <tns1:Cell>
                    <tns1:Motion/>
                  </tns1:Cell>
                </tns1:RuleEngine>
              </tev:TopicSet>
            </tev:GetEventPropertiesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn create_pull_point_subscription_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl"
                      xmlns:wsa="http://www.w3.org/2005/08/addressing">
          <s:Body>
            <tev:CreatePullPointSubscriptionResponse>
              <tev:SubscriptionReference>
                <wsa:Address>http://192.168.1.1/onvif/events/subscription_1</wsa:Address>
              </tev:SubscriptionReference>
              <tev:CurrentTime>2024-01-01T00:00:00Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
            </tev:CreatePullPointSubscriptionResponse>
          </s:Body>
        </s:Envelope>"#
}

fn pull_messages_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tev:PullMessagesResponse>
              <tev:CurrentTime>2024-01-01T00:00:10Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
              <wsnt:NotificationMessage>
                <wsnt:Topic>tns1:VideoSource/MotionAlarm</wsnt:Topic>
                <wsnt:Message>
                  <tt:Message UtcTime="2024-01-01T00:00:09Z">
                    <tt:Source>
                      <tt:SimpleItem Name="VideoSourceToken" Value="VideoSource_1"/>
                    </tt:Source>
                    <tt:Data>
                      <tt:SimpleItem Name="IsMotion" Value="true"/>
                    </tt:Data>
                  </tt:Message>
                </wsnt:Message>
              </wsnt:NotificationMessage>
            </tev:PullMessagesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn pull_messages_empty_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
          <s:Body>
            <tev:PullMessagesResponse>
              <tev:CurrentTime>2024-01-01T00:00:10Z</tev:CurrentTime>
              <tev:TerminationTime>2024-01-01T00:01:00Z</tev:TerminationTime>
            </tev:PullMessagesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn renew_subscription_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2">
          <s:Body>
            <wsnt:RenewResponse>
              <wsnt:TerminationTime>2024-01-01T00:02:00Z</wsnt:TerminationTime>
            </wsnt:RenewResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_event_properties ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_event_properties_flattens_topics() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(event_properties_xml()));

    let props = client
        .get_event_properties("http://192.168.1.1/onvif/events_service")
        .await
        .unwrap();

    assert!(
        props.topics.iter().any(|t| t.contains("MotionAlarm")),
        "topics should contain MotionAlarm"
    );
    assert!(
        props.topics.iter().any(|t| t.contains("Motion")),
        "topics should contain nested Motion topic"
    );
}

// ── create_pull_point_subscription ────────────────────────────────────────

#[tokio::test]
async fn test_create_pull_point_subscription_returns_reference_url() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(create_pull_point_subscription_xml()));

    let sub = client
        .create_pull_point_subscription(
            "http://192.168.1.1/onvif/events_service",
            None,
            Some("PT60S"),
        )
        .await
        .unwrap();

    assert_eq!(
        sub.reference_url,
        "http://192.168.1.1/onvif/events/subscription_1"
    );
    assert_eq!(sub.termination_time, "2024-01-01T00:01:00Z");
}

#[tokio::test]
async fn test_create_pull_point_subscription_with_filter() {
    let (transport, captured) = RecordingTransport::new(create_pull_point_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_pull_point_subscription(
            "http://192.168.1.1/onvif/events_service",
            Some("tns1:VideoSource/MotionAlarm"),
            None,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("tns1:VideoSource/MotionAlarm"));
    assert!(body.contains("TopicExpression"));
}

#[tokio::test]
async fn test_create_pull_point_subscription_without_filter_omits_filter_el() {
    let (transport, captured) = RecordingTransport::new(create_pull_point_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_pull_point_subscription("http://192.168.1.1/onvif/events_service", None, None)
        .await
        .unwrap();

    assert!(!captured.lock().unwrap().body.contains("Filter"));
}

// ── pull_messages ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_pull_messages_parses_notification() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_xml()));

    let msgs = client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT5S",
            100,
        )
        .await
        .unwrap();

    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].topic.contains("MotionAlarm"));
    assert_eq!(msgs[0].utc_time, "2024-01-01T00:00:09Z");
    assert_eq!(
        msgs[0].source.get("VideoSourceToken").map(String::as_str),
        Some("VideoSource_1")
    );
    assert_eq!(
        msgs[0].data.get("IsMotion").map(String::as_str),
        Some("true")
    );
}

#[tokio::test]
async fn test_pull_messages_empty_returns_empty_vec() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_empty_xml()));

    let msgs = client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT5S",
            100,
        )
        .await
        .unwrap();

    assert!(msgs.is_empty());
}

#[tokio::test]
async fn test_pull_messages_sends_timeout_and_limit() {
    let (transport, captured) = RecordingTransport::new(pull_messages_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .pull_messages(
            "http://192.168.1.1/onvif/events/subscription_1",
            "PT10S",
            50,
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("PT10S"));
    assert!(body.contains("50"));
}

// ── renew_subscription ────────────────────────────────────────────────────

#[tokio::test]
async fn test_renew_subscription_returns_new_termination_time() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(renew_subscription_xml()));

    let new_time = client
        .renew_subscription("http://192.168.1.1/onvif/events/subscription_1", "PT60S")
        .await
        .unwrap();

    assert_eq!(new_time, "2024-01-01T00:02:00Z");
}

#[tokio::test]
async fn test_renew_subscription_sends_termination_time() {
    let (transport, captured) = RecordingTransport::new(renew_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .renew_subscription("http://192.168.1.1/onvif/events/subscription_1", "PT120S")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("PT120S"));
}

// ── unsubscribe ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_renew_subscription_uses_oasis_action_uri() {
    let (transport, captured) = RecordingTransport::new(renew_subscription_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .renew_subscription("http://192.168.1.1/onvif/events/subscription_1", "PT60S")
        .await
        .unwrap();
    assert_eq!(
        captured.lock().unwrap().action,
        "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest"
    );
}

#[tokio::test]
async fn test_unsubscribe_uses_oasis_action_uri() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
         <s:Body><tev:UnsubscribeResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .unsubscribe("http://192.168.1.1/onvif/events/subscription_1")
        .await
        .unwrap();
    assert_eq!(
        captured.lock().unwrap().action,
        "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest"
    );
}

// ── Direction-3: event_stream ─────────────────────────────────────────────────

#[tokio::test]
async fn test_event_stream_yields_notification_messages() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(pull_messages_xml()));
    let mut stream = client.event_stream("http://192.168.1.1/onvif/subscription_1", "PT5S", 10);
    let msg = stream.next().await.expect("stream should yield").unwrap();
    assert!(msg.topic.contains("MotionAlarm"));
}

#[tokio::test]
async fn test_event_stream_error_on_bad_response() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
         <s:Body>
           <s:Fault>
             <s:Code><s:Value>s:Receiver</s:Value></s:Code>
             <s:Reason><s:Text>SubscriptionExpired-7712</s:Text></s:Reason>
           </s:Fault>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let mut stream = client.event_stream("http://192.168.1.1/onvif/subscription_1", "PT5S", 10);
    let result = stream.next().await.expect("stream should yield an error");
    // The stream must surface the device's fault verbatim, not merely stop:
    // a caller distinguishes "subscription gone, resubscribe" from any other
    // failure by the code and reason.
    assert_fault(
        result.unwrap_err(),
        "s:Receiver",
        "SubscriptionExpired-7712",
    );
}

// ── Subscribe (WS-BaseNotification push) ─────────────────────────────────────

fn subscribe_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
                    xmlns:wsa="http://www.w3.org/2005/08/addressing">
          <s:Body>
            <wsnt:SubscribeResponse>
              <wsnt:SubscriptionReference>
                <wsa:Address>http://192.168.1.1/onvif/events/push_sub_1</wsa:Address>
              </wsnt:SubscriptionReference>
              <wsnt:CurrentTime>2026-04-05T00:00:00Z</wsnt:CurrentTime>
              <wsnt:TerminationTime>2026-04-05T00:01:00Z</wsnt:TerminationTime>
            </wsnt:SubscribeResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_subscribe_parses_push_subscription() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(subscribe_response_xml()));
    let sub = client
        .subscribe(
            "http://192.168.1.1/onvif/events",
            "http://192.168.1.50:8080/notify",
            None,
            Some("PT60S"),
        )
        .await
        .unwrap();
    assert_eq!(
        sub.subscription_reference,
        "http://192.168.1.1/onvif/events/push_sub_1"
    );
    assert_eq!(sub.current_time, "2026-04-05T00:00:00Z");
    assert_eq!(sub.termination_time, "2026-04-05T00:01:00Z");
}

#[tokio::test]
async fn test_subscribe_uses_oasis_action_uri() {
    let (transport, captured) = RecordingTransport::new(subscribe_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let _ = client
        .subscribe(
            "http://192.168.1.1/onvif/events",
            "http://192.168.1.50:8080/notify",
            None,
            Some("PT60S"),
        )
        .await;
    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/SubscribeRequest"
    );
    assert!(c.body.contains("ConsumerReference"));
    assert!(c.body.contains("192.168.1.50:8080/notify"));
}

#[tokio::test]
async fn test_subscribe_with_filter_includes_topic_expression() {
    let (transport, captured) = RecordingTransport::new(subscribe_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let _ = client
        .subscribe(
            "http://192.168.1.1/onvif/events",
            "http://192.168.1.50:8080/notify",
            Some("tns1:VideoSource/MotionAlarm"),
            Some("PT60S"),
        )
        .await;
    let c = captured.lock().unwrap();
    assert!(c.body.contains("TopicExpression"));
    assert!(c.body.contains("MotionAlarm"));
}

#[tokio::test]
async fn test_subscribe_soap_fault_returns_error() {
    let xml = make_soap_fault_xml("env:Sender", "InvalidConsumerReference");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let result = client
        .subscribe(
            "http://192.168.1.1/onvif/events",
            "http://192.168.1.50:8080/notify",
            None,
            None,
        )
        .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("InvalidConsumerReference")
    );
}

#[tokio::test]
async fn test_subscribe_escapes_consumer_url() {
    let sub_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
                      xmlns:wsa="http://www.w3.org/2005/08/addressing">
          <s:Body>
            <wsnt:SubscribeResponse>
              <wsnt:SubscriptionReference>
                <wsa:Address>http://192.168.1.1/onvif/events/sub1</wsa:Address>
              </wsnt:SubscriptionReference>
              <wsnt:CurrentTime>2024-01-01T00:00:00Z</wsnt:CurrentTime>
              <wsnt:TerminationTime>2024-01-01T01:00:00Z</wsnt:TerminationTime>
            </wsnt:SubscribeResponse>
          </s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(sub_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .subscribe(
            "http://192.168.1.1/onvif/events",
            "http://evil.com/notify?a=1&b=2",
            Some("tns1:Topic&Subtopic"),
            Some("PT60S"),
        )
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("a=1&amp;b=2"),
        "consumer_url ampersand must be escaped: {body}"
    );
    assert!(
        body.contains("Topic&amp;Subtopic"),
        "filter ampersand must be escaped: {body}"
    );
}

// ── SetSynchronizationPoint ───────────────────────────────────────────────

#[tokio::test]
async fn test_set_synchronization_point_ok() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tev="http://www.onvif.org/ver10/events/wsdl">
          <s:Body><tev:SetSynchronizationPointResponse/></s:Body>
        </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_synchronization_point("http://192.168.1.1/onvif/events/sub1")
        .await
        .unwrap();

    let action = captured.lock().unwrap().action.clone();
    assert!(action.contains("SetSynchronizationPoint"));
}
