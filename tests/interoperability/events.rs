use super::*;
use oxvif::{
    OnvifError,
    events::{
        EndpointPolicy, EventItem, ManagedEvent, ManagedEventOptions, ManagedEventStop,
        QualifiedContent,
    },
    soap::SoapError,
};

const URL: &str = "http://camera.test/events";
const SUB: &str = "http://camera.test/subscription";
const CREATE: &str =
    "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest";
const PULL: &str =
    "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest";
const RENEW: &str = "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest";
const CLOSE: &str = "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest";

fn grant(url: &str, parameters: &str, times: &str) -> Reply {
    reply(&format!(
        "<e:CreatePullPointSubscriptionResponse><e:SubscriptionReference><a:Address>{url}</a:Address><a:ReferenceParameters>{parameters}</a:ReferenceParameters></e:SubscriptionReference>{times}</e:CreatePullPointSubscriptionResponse>"
    ))
}
fn lease(token: &str) -> Reply {
    grant(
        SUB,
        &format!("<r:Route xmlns:r=\"urn:routing\">{token}</r:Route>"),
        "<n:CurrentTime>2050-01-01T00:00:00Z</n:CurrentTime><n:TerminationTime>2050-01-01T00:00:12Z</n:TerminationTime>",
    )
}
const NOTICE: &str = r#"<n:NotificationMessage><n:Topic xmlns:q="urn:camera:topics" Dialect="urn:example:dialect">q:Motion</n:Topic><n:Message><t:Message UtcTime="2050-01-01T00:00:01Z" PropertyOperation="Changed"><t:Source><t:SimpleItem Name="channel" Value="left"/></t:Source><t:Key><t:SimpleItem Name="object" Value="key-42"/></t:Key><t:Data><t:SimpleItem Name="motion" Value="first"/><t:SimpleItem Name="motion" Value="second"/><t:ElementItem Name="details" xmlns:q="urn:camera:outer">pre<q:Part xmlns:q="urn:camera:inner">payload</q:Part>post</t:ElementItem></t:Data></t:Message></n:Message></n:NotificationMessage>"#;
fn page(notifications: &str) -> Reply {
    reply(&format!(
        "<e:PullMessagesResponse><e:CurrentTime>2050-01-01T00:00:01Z</e:CurrentTime><e:TerminationTime>2050-01-01T00:00:12Z</e:TerminationTime>{notifications}</e:PullMessagesResponse>"
    ))
}

fn close() -> Reply {
    reply("<n:UnsubscribeResponse/>")
}

#[tokio::test]
async fn qualified_references_route_same_address_with_distinct_headers() {
    let rich = r#"<r:Route xmlns:r="urn:routing" xmlns:q="urn:outer" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="q:Selector" a:IsReferenceParameter="false">first&amp;<r:Part xmlns:q="urn:inner" xsi:type="q:Entry"/>last</r:Route><r:Extra xmlns:r="urn:routing">second</r:Extra>"#;
    let script = Script::new(vec![
        (
            CREATE,
            grant(
                SUB,
                rich,
                "<n:CurrentTime>2050-01-01T00:00:00Z</n:CurrentTime><n:TerminationTime>2050-01-01T00:00:12Z</n:TerminationTime>",
            ),
        ),
        (CREATE, lease("distinct")),
        (PULL, page(NOTICE)),
        (
            RENEW,
            reply(
                "<n:RenewResponse><n:TerminationTime>2050-01-01T00:00:42Z</n:TerminationTime></n:RenewResponse>",
            ),
        ),
        (CLOSE, close()),
    ]);
    let client = script.client();
    let policy = EndpointPolicy::default();
    let timeout = Duration::from_secs(2);
    let first = client
        .create_referenced_pull_point(URL, "PT12S", &policy, timeout)
        .await
        .unwrap();
    let second = client
        .create_referenced_pull_point(URL, "PT12S", &policy, timeout)
        .await
        .unwrap();
    assert_eq!(first.reference.address(), second.reference.address());
    assert_eq!(first.to_legacy().termination_time, "2050-01-01T00:00:12Z");
    let params = first.reference.parameters();
    assert_eq!(
        params.iter().map(|p| p.local_name()).collect::<Vec<_>>(),
        ["Route", "Extra"]
    );
    assert_eq!(params[0].namespace_for_prefix("q"), Some("urn:outer"));
    assert!(
        matches!(params[0].content(), [QualifiedContent::Text(a), QualifiedContent::Element(b), QualifiedContent::Text(c)] if a == "first&" && c == "last" && b.namespace_for_prefix("q") == Some("urn:inner"))
    );
    let page = client
        .pull_referenced_messages(&first.reference, "PT1S", 1, &policy, timeout)
        .await
        .unwrap();
    let event = &page.notifications[0];
    assert_eq!(
        event.topic.as_ref().unwrap().namespace_for_prefix("q"),
        Some("urn:camera:topics")
    );
    assert_eq!(
        event.topic.as_ref().unwrap().attributes()[0].value(),
        "urn:example:dialect"
    );
    assert!(
        matches!(&event.key[..], [EventItem::Simple {name, value}] if name == "object" && value == "key-42")
    );
    assert!(
        matches!(&event.data[..], [EventItem::Simple {value: a, ..}, EventItem::Simple {value: b, ..}, EventItem::Element(e)] if a == "first" && b == "second" && matches!(&e.content()[1], QualifiedContent::Element(part) if part.namespace() == "urn:camera:inner"))
    );
    assert_eq!(event.to_legacy().data["motion"], "second");
    let renewed = client
        .renew_referenced_pull_point(&first.reference, "PT30S", &policy, timeout)
        .await
        .unwrap();
    assert_eq!(renewed.current_time, None);
    assert_eq!(
        renewed.termination_time.as_deref(),
        Some("2050-01-01T00:00:42Z")
    );
    client
        .unsubscribe_referenced_pull_point(&second.reference, &policy, timeout)
        .await
        .unwrap();
    let calls = script.calls();
    for call in &calls[2..] {
        assert_eq!(call.url, SUB);
        assert!(
            call.body
                .contains(&format!("<wsa:Action>{}</wsa:Action>", call.action))
        );
        assert!(call.body.contains("<wsa:MessageID>urn:uuid:"));
    }
    for call in &calls[2..4] {
        assert_eq!(
            reference_headers(&call.body),
            vec![
                ("Route".into(), "true".into()),
                ("Extra".into(), "true".into())
            ]
        );
        assert!(call.body.contains(
            "first&amp;<r:Part xmlns:q=\"urn:inner\" xsi:type=\"q:Entry\"></r:Part>last"
        ));
        assert!(call.body.contains("xmlns:q=\"urn:outer\""));
        assert!(!call.body.contains("IsReferenceParameter=\"false\""));
    }
    assert_eq!(
        reference_headers(&calls[4].body),
        vec![("Route".into(), "true".into())]
    );
    assert!(calls[4].body.contains(">distinct</r:Route>"));
    script.assert_done();
}

fn reference_headers(xml: &str) -> Vec<(String, String)> {
    use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};
    let mut reader = NsReader::from_str(xml);
    let mut headers = Vec::new();
    let mut depth = 0;
    let mut header = false;
    loop {
        match reader.read_event().unwrap() {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(event, Event::Empty(_));
                let start = match event {
                    Event::Start(start) | Event::Empty(start) => start,
                    _ => unreachable!(),
                };
                let (ns, local) = reader.resolver().resolve_element(start.name());
                if depth == 1 {
                    header = matches!(ns, ResolveResult::Bound(ns) if ns.as_ref() == SOAP)
                        && local.as_ref() == "Header";
                }
                if header
                    && depth == 2
                    && matches!(ns, ResolveResult::Bound(ns) if ns.as_ref() == "urn:routing")
                {
                    let attribute = start
                        .attributes()
                        .map(Result::unwrap)
                        .find(|attr| {
                            let (ns, name) = reader.resolver().resolve_attribute(attr.key);
                            name.as_ref() == "IsReferenceParameter"
                                && matches!(ns, ResolveResult::Bound(ns) if ns.as_ref() == WSA)
                        })
                        .unwrap();
                    headers.push((local.as_ref().to_owned(), attribute.value.into_owned()));
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 1 {
                    header = false;
                }
            }
            Event::Eof => return headers,
            _ => {}
        }
    }
}

#[tokio::test]
async fn endpoint_policy_and_qualified_structure_refuse_before_followup_io() {
    let cases = [
        (
            grant("http://other.test/sub", "", ""),
            "event endpoint origin",
        ),
        (
            grant("http://user:pass@camera.test/sub", "", ""),
            "invalid HTTP(S)",
        ),
        (
            grant(SUB, "<a:To>http://other.test</a:To>", ""),
            "reserved reference parameter namespace",
        ),
        (
            reply(
                "<e:CreatePullPointSubscriptionResponse><e:SubscriptionReference><a:Address>http://camera.test/subscription</a:Address><a:Address>http://camera.test/other</a:Address></e:SubscriptionReference></e:CreatePullPointSubscriptionResponse>",
            ),
            "duplicate qualified XML field",
        ),
        (
            reply(
                "<e:CreatePullPointSubscriptionResponse><e:SubscriptionReference><wrong:Address xmlns:wrong=\"urn:unrelated\">http://camera.test/sub</wrong:Address></e:SubscriptionReference></e:CreatePullPointSubscriptionResponse>",
            ),
            "Missing required field: Address",
        ),
        (
            grant(
                SUB,
                "<Route xmlns=\"urn:routing\" xmlns:x=\"urn:alias\" xmlns:y=\"urn:alias\" x:key=\"one\" y:key=\"two\"/>",
                "",
            ),
            "duplicate expanded XML attribute",
        ),
        (
            grant(
                SUB,
                &format!(
                    "<Route xmlns=\"urn:routing\">{}</Route>",
                    "x".repeat(256 * 1024)
                ),
                "",
            ),
            "reference parameter retention limit exceeded",
        ),
    ];
    for (response, expected) in cases {
        let script = Script::new(vec![(CREATE, response)]);
        let error = script
            .client()
            .create_referenced_pull_point(
                URL,
                "PT12S",
                &EndpointPolicy::default(),
                Duration::from_secs(2),
            )
            .await
            .err()
            .unwrap();
        let expected = expected.replace("event endpoint", "SOAP endpoint");
        assert!(error.to_string().contains(&expected), "{error}");
        assert_eq!(script.calls().len(), 1);
        script.assert_done();
    }
    let script = Script::new(vec![
        (CREATE, grant("http://other.test/sub", "", "")),
        (CLOSE, close()),
    ]);
    let policy = EndpointPolicy::default()
        .allow_origin("http://other.test")
        .unwrap();
    let client = script.client();
    let point = client
        .create_referenced_pull_point(URL, "PT12S", &policy, Duration::from_secs(2))
        .await
        .unwrap();
    client
        .unsubscribe_referenced_pull_point(&point.reference, &policy, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(script.calls()[1].url, "http://other.test/sub");
}

fn options() -> ManagedEventOptions {
    ManagedEventOptions {
        pull_wait: Duration::from_secs(1),
        request_timeout: Duration::from_secs(20),
        lease: Duration::from_secs(12),
        queue_messages: 1,
        ..Default::default()
    }
}

#[tokio::test(start_paused = true)]
async fn managed_renewal_proceeds_during_pending_pull_and_full_queue() {
    let script = Script::new(vec![
        (CREATE, lease("lease-one")),
        (PULL, page(NOTICE)),
        (PULL, Reply::Pending),
        (
            RENEW,
            reply(
                "<n:RenewResponse><n:TerminationTime>2050-01-01T00:00:21Z</n:TerminationTime></n:RenewResponse>",
            ),
        ),
        (CLOSE, close()),
    ]);
    let mut subscription = script
        .client()
        .managed_events(URL, options())
        .await
        .unwrap();
    script.until_calls(4).await;
    let calls = script.calls();
    assert!(calls[2].at.duration_since(calls[1].at) >= Duration::from_millis(100));
    assert!(calls[3].at.duration_since(calls[0].at) < Duration::from_secs(12));
    assert!(calls[3].body.contains(">lease-one</r:Route>"));
    assert_eq!(subscription.close().await.stop, ManagedEventStop::Closed);
    assert!(subscription.close().await.cleanup_error.is_none());
    assert!(
        matches!(subscription.next().await, Some(Ok(ManagedEvent::Notification(event))) if event.to_legacy().topic == "q:Motion")
    );
    assert!(subscription.next().await.is_none());
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_eq!(script.calls().len(), 5);
    script.assert_done();
}

#[tokio::test(start_paused = true)]
async fn managed_recovery_announces_gap_and_replaces_reference_once() {
    let script = Script::new(vec![
        (CREATE, lease("old")),
        (PULL, Reply::Status(503)),
        (CLOSE, close()),
        (CREATE, lease("replacement")),
        (PULL, page(NOTICE)),
        (CLOSE, close()),
    ]);
    let mut subscription = script
        .client()
        .managed_events(URL, options())
        .await
        .unwrap();
    assert!(matches!(
        subscription.next().await,
        Some(Ok(ManagedEvent::Gap))
    ));
    assert!(
        matches!(subscription.next().await, Some(Ok(ManagedEvent::Notification(event))) if event.utc_time == "2050-01-01T00:00:01Z")
    );
    let report = subscription.close().await;
    assert_eq!(report.recoveries, 1);
    assert_eq!(report.recovery_cleanup_failures, 0);
    assert_eq!(report.stop, ManagedEventStop::Closed);
    let calls = script.calls();
    assert!(calls[2].body.contains(">old</r:Route>"));
    assert!(calls[4].body.contains(">replacement</r:Route>"));
    assert!(calls[5].body.contains(">replacement</r:Route>"));
    script.assert_done();
}

#[tokio::test(start_paused = true)]
async fn managed_overflow_and_auth_failure_are_explicit_terminal_results() {
    let script = Script::new(vec![
        (CREATE, lease("bounded")),
        (PULL, page(&format!("{NOTICE}{NOTICE}"))),
        (CLOSE, close()),
    ]);
    let mut subscription = script
        .client()
        .managed_events(URL, options())
        .await
        .unwrap();
    script.until_calls(3).await;
    assert!(matches!(
        subscription.next().await,
        Some(Ok(ManagedEvent::Notification(_)))
    ));
    assert!(matches!(
        subscription.next().await,
        Some(Err(ManagedEventStop::QueueFull))
    ));
    assert!(subscription.next().await.is_none());
    assert_eq!(subscription.close().await.stop, ManagedEventStop::QueueFull);
    script.assert_done();

    let script = Script::new(vec![
        (CREATE, lease("denied")),
        (PULL, Reply::Status(403)),
        (CLOSE, Reply::Status(401)),
    ]);
    let mut subscription = script
        .client()
        .managed_events(URL, options())
        .await
        .unwrap();
    assert!(matches!(
        subscription.next().await,
        Some(Err(ManagedEventStop::RequestFailed))
    ));
    let report = subscription.close().await;
    assert_eq!(report.recoveries, 0);
    assert!(
        matches!(&report.cause, Some(OnvifError::Transport(TransportError::HttpStatus {status: 403, body})) if body == "fixture-status-body")
    );
    assert!(
        matches!(&report.cleanup_error, Some(OnvifError::Transport(TransportError::HttpStatus {status: 401, body})) if body == "fixture-status-body")
    );
    script.assert_done();
}

#[tokio::test(start_paused = true)]
async fn invalid_grant_cleans_up_known_reference_and_drop_stops_local_work() {
    let script = Script::new(vec![
        (
            CREATE,
            grant(
                SUB,
                "",
                "<n:CurrentTime>2050-02-30T00:00:00Z</n:CurrentTime><n:TerminationTime>2050-03-01T00:00:00Z</n:TerminationTime>",
            ),
        ),
        (CLOSE, close()),
    ]);
    let error = script
        .client()
        .managed_events(URL, options())
        .await
        .err()
        .unwrap();
    assert!(
        matches!(error.cause, OnvifError::Soap(SoapError::InvalidValue {field: "event lease timing", value}) if value == "invalid grant")
    );
    assert!(error.cleanup_error.is_none());
    script.assert_done();

    let script = Script::new(vec![(CREATE, lease("abandoned")), (PULL, Reply::Pending)]);
    let subscription = script
        .client()
        .managed_events(URL, options())
        .await
        .unwrap();
    script.until_calls(2).await;
    drop(subscription);
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_eq!(script.calls().len(), 2);
    script.assert_done();
}

#[tokio::test(start_paused = true)]
async fn managed_byte_cap_and_recovery_limit_stop_with_cleanup() {
    let script = Script::new(vec![
        (CREATE, lease("bytes")),
        (PULL, page(NOTICE)),
        (CLOSE, close()),
    ]);
    let mut sub = script
        .client()
        .managed_events(
            URL,
            ManagedEventOptions {
                queue_bytes: 8,
                ..options()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        sub.next().await,
        Some(Err(ManagedEventStop::QueueFull))
    ));
    assert_eq!(sub.close().await.stop, ManagedEventStop::QueueFull);
    script.assert_done();

    let script = Script::new(vec![
        (CREATE, lease("limit")),
        (PULL, Reply::Status(503)),
        (CLOSE, close()),
    ]);
    let mut sub = script
        .client()
        .managed_events(
            URL,
            ManagedEventOptions {
                max_recoveries: 0,
                ..options()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        sub.next().await,
        Some(Err(ManagedEventStop::RecoveryExhausted))
    ));
    assert_eq!(sub.close().await.recoveries, 0);
    assert!(script.calls()[2].body.contains(">limit</r:Route>"));
    script.assert_done();
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn managed_event_owner_reads_ep2_and_closes_the_remote_lease() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    let session = oxvif::OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();
    let mut owner = session.managed_events(options()).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(3), owner.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, ManagedEvent::Notification(event) if !event.to_legacy().topic.is_empty())
    );
    let report = owner.close().await;
    assert_eq!(report.stop, ManagedEventStop::Closed);
    assert!(report.cleanup_error.is_none());
    assert_eq!(report.recoveries, 0);
}
