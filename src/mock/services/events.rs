//! Events service handlers.
//!
//! State (event counter + topic filter) lives **per mock instance** on
//! `DeviceState` (`event_seq` / `event_filter`) — not in process-global
//! statics — so two mock instances in two tests never share event state.
//! PullMessages is deterministic and returns immediately (no pacing sleep);
//! each call selects one queued or synthesized event using a filter snapshot.

use crate::mock::helpers::soap;
use crate::mock::state::SharedState;
use crate::mock::xml_parse::extract_tag;
use std::time::{SystemTime, UNIX_EPOCH};

/// `GetEventPropertiesResponse` mixes three namespaces, and which one each
/// member takes is decided by `event.wsdl` per element, not per response:
///
/// - `TopicNamespaceLocation` — declared locally, so `tev:`
/// - `FixedTopicSet`, `TopicExpressionDialect` — `ref="wsnt:…"`, so **`wsnt:`**
/// - `TopicSet` — `ref="wstop:…"`, so `wstop:`
///
/// Until 0.15.0 the mock put `FixedTopicSet` under `wstop:` (next to
/// `TopicSet`, which is `wstop:`) and `TopicExpressionDialect` under `tev:`
/// (next to `TopicNamespaceLocation`, which is `tev:`). Both were wrong by
/// exactly the same reasoning: **the neighbouring element's namespace is not
/// evidence.**
///
/// Two more members are `minOccurs=1` and were absent entirely:
/// `MessageContentFilterDialect` and `MessageContentSchemaLocation`, both
/// declared locally and so both `tev:`. They sit either side of the optional
/// `ProducerPropertiesFilterDialect` in the sequence, which is why the schema
/// order below is not the order the two names suggest.
pub fn resp_event_properties() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:wstop="http://docs.oasis-open.org/wsn/t-1""#,
        r#"<tev:GetEventPropertiesResponse>
          <tev:TopicNamespaceLocation>http://www.onvif.org/onvif/ver10/topics/topicns.xml</tev:TopicNamespaceLocation>
          <wsnt:FixedTopicSet>true</wsnt:FixedTopicSet>
          <wstop:TopicSet>
            <tns1:VideoSource wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:MotionAlarm wstop:topic="true"/>
            </tns1:VideoSource>
            <tns1:RuleEngine wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:FieldDetector wstop:topic="false">
                <tns1:ObjectsInside wstop:topic="true"/>
              </tns1:FieldDetector>
            </tns1:RuleEngine>
            <tns1:Device wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:Trigger wstop:topic="false">
                <tns1:DigitalInput wstop:topic="true"/>
                <tns1:Relay wstop:topic="true"/>
              </tns1:Trigger>
            </tns1:Device>
          </wstop:TopicSet>
          <wsnt:TopicExpressionDialect>http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet</wsnt:TopicExpressionDialect>
          <tev:MessageContentFilterDialect>http://www.onvif.org/ver10/tev/messageContentFilter/ItemFilter</tev:MessageContentFilterDialect>
          <tev:MessageContentSchemaLocation>http://www.onvif.org/onvif/ver10/schema/onvif.xsd</tev:MessageContentSchemaLocation>
        </tev:GetEventPropertiesResponse>"#,
    )
}

/// Format SystemTime as RFC3339-ish UTC ("2026-04-23T12:34:56Z").
fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = epoch_to_civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert seconds-since-Unix-epoch to (year, month, day, hour, min, sec).
/// Howard Hinnant's days_from_civil algorithm in reverse — exact for any
/// date in the proleptic Gregorian calendar.
fn epoch_to_civil(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let time_of_day = (secs % 86_400) as u32;
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y_ = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y_ + 1 } else { y_ } as i32;
    (y, m, d, hh, mm, ss)
}

pub fn resp_create_pull_point_subscription(base: &str, state: &SharedState, body: &str) -> String {
    // Parse the optional <wsnt:TopicExpression>topic1|topic2|...</...> and
    // store it so subsequent PullMessages can filter. Empty/absent = no filter.
    let new_filter = extract_tag(body, "TopicExpression").and_then(|expr| {
        let entries: Vec<String> = expr
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if entries.is_empty() {
            None
        } else {
            Some(entries)
        }
    });
    state.modify(|s| s.event_filter = new_filter);

    let now = now_rfc3339();
    // `CurrentTime` / `TerminationTime` are **`wsnt:`** here — `event.wsdl`
    // declares them on this response as `ref="wsnt:…"`.
    //
    // `PullMessagesResponse` below declares two elements of the *same two
    // names* locally, so there they are `tev:` and must stay `tev:`. Same file,
    // same service, same names, two namespaces. This is why the sweep was done
    // per declaration and not with a search-and-replace on the name.
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
        &format!(
            r#"<tev:CreatePullPointSubscriptionResponse>
          <tev:SubscriptionReference>
            <wsa:Address>{base}/onvif/events/subscription_1</wsa:Address>
          </tev:SubscriptionReference>
          <wsnt:CurrentTime>{now}</wsnt:CurrentTime>
          <wsnt:TerminationTime>{now}</wsnt:TerminationTime>
        </tev:CreatePullPointSubscriptionResponse>"#
        ),
    )
}

/// Select the next event immediately. A queued IO event consumes one queue slot;
/// otherwise the per-instance counter advances to synthesize an event. The
/// captured topic filter applies to both paths and can yield an empty response.
///
/// Out-of-band IO events queued by the `/mock/digital-input/...` simulator
/// endpoints win first — they drain the queue before the synthetic motion /
/// rule cycle resumes. This lets a test drive the input flip, poll once,
/// and assert the exact event content without racing against the
/// synthetic stream.
pub fn resp_pull_messages(state: &SharedState) -> String {
    // Select and snapshot under one lock; rendering and callbacks run outside it.
    let (pending, seq, filter) = state.modify_returning(|s| {
        let pending = if s.pending_io_events.is_empty() {
            s.event_seq = s.event_seq.wrapping_add(1);
            None
        } else {
            Some(s.pending_io_events.remove(0))
        };
        (pending, s.event_seq, s.event_filter.clone())
    });
    if let Some(ev) = pending {
        let topic = format!("tns1:Device/Trigger/{}", ev.kind);
        if is_filtered(&filter, &topic) {
            return empty_pull_response();
        }
        return io_event_response(&ev);
    }

    let now = now_rfc3339();

    // Alternate motion / rule-engine events so the log shows variety.
    let (topic, source_name, source_value, data_name, data_value) = if seq % 2 == 1 {
        (
            "tns1:VideoSource/MotionAlarm",
            "VideoSourceToken",
            "VS_1",
            "IsMotion",
            if (seq / 2) & 1 == 0 { "true" } else { "false" },
        )
    } else {
        (
            "tns1:RuleEngine/FieldDetector/ObjectsInside",
            "Rule",
            "MyFieldDetectorRule",
            "ObjectId",
            match seq % 6 {
                0 => "42",
                2 => "7",
                _ => "13",
            },
        )
    };

    // A filtered event still consumes one slot in this immediate mock model.
    if is_filtered(&filter, topic) {
        return empty_pull_response();
    }

    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:tns1="http://www.onvif.org/ver10/topics""#,
        &format!(
            r#"<tev:PullMessagesResponse>
          <tev:CurrentTime>{now}</tev:CurrentTime>
          <tev:TerminationTime>{now}</tev:TerminationTime>
          <wsnt:NotificationMessage>
            <wsnt:Topic Dialect="http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet">{topic}</wsnt:Topic>
            <wsnt:Message>
              <tt:Message UtcTime="{now}" PropertyOperation="Changed">
                <tt:Source>
                  <tt:SimpleItem Name="{source_name}" Value="{source_value}"/>
                </tt:Source>
                <tt:Data>
                  <tt:SimpleItem Name="Seq" Value="{seq}"/>
                  <tt:SimpleItem Name="{data_name}" Value="{data_value}"/>
                </tt:Data>
              </tt:Message>
            </wsnt:Message>
          </wsnt:NotificationMessage>
        </tev:PullMessagesResponse>"#
        ),
    )
}

fn is_filtered(filter: &Option<Vec<String>>, topic: &str) -> bool {
    filter
        .as_ref()
        .is_some_and(|allowed| !allowed.iter().any(|a| a == topic))
}

fn empty_pull_response() -> String {
    let now = now_rfc3339();
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl""#,
        &format!(
            "<tev:PullMessagesResponse><tev:CurrentTime>{now}</tev:CurrentTime><tev:TerminationTime>{now}</tev:TerminationTime></tev:PullMessagesResponse>"
        ),
    )
}

/// Build a `PullMessagesResponse` carrying one IO-trigger notification.
/// LogicalState is reported as the boolean ONVIF expects (`true`=active).
fn io_event_response(ev: &crate::mock::state::PendingIoEvent) -> String {
    let now = now_rfc3339();
    let topic = format!("tns1:Device/Trigger/{}", ev.kind);
    let source_name = match ev.kind {
        "DigitalInput" => "InputToken",
        "RelayOutput" => "RelayToken",
        _ => "Token",
    };
    let logical_bool = if ev.logical_state == "active" {
        "true"
    } else {
        "false"
    };
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:tns1="http://www.onvif.org/ver10/topics""#,
        &format!(
            r#"<tev:PullMessagesResponse>
          <tev:CurrentTime>{now}</tev:CurrentTime>
          <tev:TerminationTime>{now}</tev:TerminationTime>
          <wsnt:NotificationMessage>
            <wsnt:Topic Dialect="http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet">{topic}</wsnt:Topic>
            <wsnt:Message>
              <tt:Message UtcTime="{now}" PropertyOperation="Changed">
                <tt:Source>
                  <tt:SimpleItem Name="{source_name}" Value="{token}"/>
                </tt:Source>
                <tt:Data>
                  <tt:SimpleItem Name="LogicalState" Value="{logical_bool}"/>
                </tt:Data>
              </tt:Message>
            </wsnt:Message>
          </wsnt:NotificationMessage>
        </tev:PullMessagesResponse>"#,
            token = crate::types::xml_escape(&ev.token),
            topic = crate::types::xml_escape(&topic),
        ),
    )
}

pub fn resp_subscribe(base: &str) -> String {
    let now = now_rfc3339();
    soap(
        r#"xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
        &format!(
            r#"<wsnt:SubscribeResponse>
          <wsnt:SubscriptionReference>
            <wsa:Address>{base}/onvif/events/push_sub_1</wsa:Address>
          </wsnt:SubscriptionReference>
          <wsnt:CurrentTime>{now}</wsnt:CurrentTime>
          <wsnt:TerminationTime>{now}</wsnt:TerminationTime>
        </wsnt:SubscribeResponse>"#
        ),
    )
}

pub fn resp_renew() -> String {
    let now = now_rfc3339();
    soap(
        r#"xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2""#,
        &format!(
            r#"<wsnt:RenewResponse>
          <wsnt:TerminationTime>{now}</wsnt:TerminationTime>
          <wsnt:CurrentTime>{now}</wsnt:CurrentTime>
        </wsnt:RenewResponse>"#
        ),
    )
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `tev:Capabilities`.
///
/// **There is no `WSPullPointSupport` attribute on this type.** That name
/// belongs to `tt:EventCapabilities`, the device-level `GetCapabilities`
/// sub-tree, and `resp_capabilities` in `device.rs` is where the mock emits
/// it. The nearest thing here is `MaxPullPoints`.
///
/// These are static capability fixtures, not proof of complete subscription
/// support. `Subscribe`/`Renew` require acknowledgment-only opt-ins and do not
/// create push subscriptions or extend lifetimes. No pausable manager or MQTT
/// broker is modeled; `EventBrokerProtocols` remains omitted. Full capability
/// reconciliation is tracked separately under W17.
pub fn resp_event_service_capabilities() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl""#,
        r#"<tev:GetServiceCapabilitiesResponse>
          <tev:Capabilities WSSubscriptionPolicySupport="true"
                            WSPausableSubscriptionManagerInterfaceSupport="false"
                            MaxNotificationProducers="4"
                            MaxPullPoints="4"
                            PersistentNotificationStorage="false"
                            MaxEventBrokers="0"
                            MetadataOverMQTT="false"/>
        </tev:GetServiceCapabilitiesResponse>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::state::MockState;

    #[test]
    fn event_pull_keeps_filter_snapshot_across_reentrant_hook_and_counter_wrap() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let mut state = MockState::new();
        let changed = Arc::new(AtomicBool::new(false));
        let observed = changed.clone();
        let target = Arc::new(std::sync::OnceLock::<std::sync::Weak<MockState>>::new());
        let target_in_hook = target.clone();
        state.set_on_change(Arc::new(move |_| {
            if !observed.swap(true, Ordering::SeqCst) {
                target_in_hook
                    .get()
                    .unwrap()
                    .upgrade()
                    .unwrap()
                    .modify(|s| s.event_filter = Some(vec![]));
            }
        }));
        let state = Arc::new(state);
        target.set(Arc::downgrade(&state)).unwrap();
        let first = resp_pull_messages(&state);
        assert!(changed.load(Ordering::SeqCst));
        assert!(
            first.contains("NotificationMessage"),
            "filter belongs to the selected event snapshot"
        );
        assert!(!resp_pull_messages(&state).contains("NotificationMessage"));
        state.modify(|s| {
            s.event_seq = u64::MAX;
            s.event_filter = None;
        });
        assert!(resp_pull_messages(&state).contains("NotificationMessage"));
        assert_eq!(state.read().event_seq, 0);
    }

    #[test]
    fn epoch_to_civil_known_dates() {
        assert_eq!(epoch_to_civil(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(epoch_to_civil(1_704_067_200), (2024, 1, 1, 0, 0, 0));
        assert_eq!(epoch_to_civil(1_709_210_096), (2024, 2, 29, 12, 34, 56));
    }

    #[test]
    fn pull_messages_returns_distinct_events() {
        let s = MockState::new();
        let a = resp_pull_messages(&s);
        let b = resp_pull_messages(&s);
        assert_ne!(a, b);
        assert!(a.contains("NotificationMessage"));
        assert!(b.contains("NotificationMessage"));
    }

    #[test]
    fn create_pull_point_parses_filter() {
        let s = MockState::new();
        let body = r#"<tev:CreatePullPointSubscription>
            <tev:Filter>
              <wsnt:TopicExpression Dialect="...ConcreteSet">
                tns1:VideoSource/MotionAlarm|tns1:RuleEngine/FieldDetector/ObjectsInside
              </wsnt:TopicExpression>
            </tev:Filter>
          </tev:CreatePullPointSubscription>"#;
        let _ = resp_create_pull_point_subscription("http://mock", &s, body);
        assert_eq!(
            s.read().event_filter.clone(),
            Some(vec![
                "tns1:VideoSource/MotionAlarm".to_string(),
                "tns1:RuleEngine/FieldDetector/ObjectsInside".to_string(),
            ])
        );
    }

    #[test]
    fn pull_messages_respects_filter() {
        let s = MockState::new();
        s.modify(|st| st.event_filter = Some(vec!["tns1:VideoSource/MotionAlarm".to_string()]));
        let seq1 = resp_pull_messages(&s); // seq=1 → motion → pass
        let seq2 = resp_pull_messages(&s); // seq=2 → rule → filtered
        assert!(seq1.contains("NotificationMessage"), "motion must pass");
        assert!(
            !seq2.contains("NotificationMessage"),
            "rule-engine must be filtered"
        );
        assert!(
            seq2.contains("PullMessagesResponse"),
            "empty response valid"
        );
    }

    #[test]
    fn event_state_is_per_instance() {
        // Two mocks must not share the event counter (regression for the old
        // process-global EVENT_SEQ).
        let a = MockState::new();
        let b = MockState::new();
        let _ = resp_pull_messages(&a);
        let _ = resp_pull_messages(&a);
        let _ = resp_pull_messages(&b);
        assert_eq!(a.read().event_seq, 2);
        assert_eq!(b.read().event_seq, 1);
    }
}
