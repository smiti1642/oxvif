//! Events service handlers.
//!
//! Each live pull point owns its filter, bounded queue, counter and expiry in
//! private per-instance runtime. PullMessages returns immediately (no pacing
//! sleep), delivering queued IO changes or one synthetic motion/rule event.

use crate::mock::helpers::soap;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) mod lifecycle;

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
/// Four independent pull points are modeled, including renew and unsubscribe.
/// `Subscribe` requires a receipt-only opt-in and does not create push delivery.
/// Subscription policies, a pausable manager and MQTT
/// broker is modeled; `EventBrokerProtocols` remains omitted. Full capability
/// reconciliation is tracked separately under W17.
pub fn resp_event_service_capabilities() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl""#,
        r#"<tev:GetServiceCapabilitiesResponse>
          <tev:Capabilities WSSubscriptionPolicySupport="false"
                            WSPausableSubscriptionManagerInterfaceSupport="false"
                            MaxNotificationProducers="0"
                            MaxPullPoints="4"
                            PersistentNotificationStorage="false"
                            MaxEventBrokers="0"
                            MetadataOverMQTT="false"/>
        </tev:GetServiceCapabilitiesResponse>"#,
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::mock::{request::Request, state::MockState};
    const E: &str = "http://www.onvif.org/ver10/events/wsdl";
    pub fn create(state: &MockState, filter: Option<&str>) -> String {
        let filter = filter.map(|f| format!("<e:Filter><n:TopicExpression xmlns:n='http://docs.oasis-open.org/wsn/b-2' xmlns:t='http://www.onvif.org/ver10/topics' Dialect='http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet'>{f}</n:TopicExpression></e:Filter>")).unwrap_or_default();
        let xml = format!(
            "<e:CreatePullPointSubscription xmlns:e='{E}'>{filter}</e:CreatePullPointSubscription>"
        );
        let request = Request::parse(&xml).unwrap();
        super::lifecycle::create(
            "http://mock",
            state,
            request.operation(E, "CreatePullPointSubscription").unwrap(),
        )
    }
    pub fn pull(state: &MockState) -> String {
        let xml = format!(
            "<e:PullMessages xmlns:e='{E}'><e:Timeout>PT0S</e:Timeout><e:MessageLimit>1</e:MessageLimit></e:PullMessages>"
        );
        let request = Request::parse(&xml).unwrap();
        super::lifecycle::pull(
            "http://mock",
            state,
            request.operation(E, "PullMessages").unwrap(),
            Some("http://mock/onvif/events/subscription_1"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        epoch_to_civil,
        test_support::{create, pull},
    };
    use crate::mock::state::MockState;

    #[test]
    fn epoch_to_civil_known_dates() {
        assert_eq!(epoch_to_civil(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(epoch_to_civil(1_704_067_200), (2024, 1, 1, 0, 0, 0));
        assert_eq!(epoch_to_civil(1_709_210_096), (2024, 2, 29, 12, 34, 56));
    }
    #[test]
    fn pull_messages_returns_distinct_events() {
        let s = MockState::new();
        create(&s, None);
        let a = pull(&s);
        let b = pull(&s);
        assert_ne!(a, b);
        assert!(a.contains("MotionAlarm"));
        assert!(b.contains("ObjectsInside"));
    }
    #[test]
    fn create_pull_point_parses_filter_union() {
        let s = MockState::new();
        assert!(
            create(
                &s,
                Some("t:VideoSource/MotionAlarm|t:RuleEngine/FieldDetector/ObjectsInside")
            )
            .contains("CreatePullPointSubscriptionResponse")
        );
        assert!(pull(&s).contains("MotionAlarm"));
        assert!(pull(&s).contains("ObjectsInside"));
    }
    #[test]
    fn pull_messages_respects_filter() {
        let s = MockState::new();
        create(&s, Some("t:VideoSource/MotionAlarm"));
        assert!(pull(&s).contains("NotificationMessage"));
        let filtered = pull(&s);
        assert!(!filtered.contains("NotificationMessage"));
        assert!(filtered.contains("PullMessagesResponse"));
    }
    #[test]
    fn event_state_is_per_instance() {
        let a = MockState::new();
        let b = MockState::new();
        create(&a, None);
        create(&b, None);
        pull(&a);
        pull(&a);
        pull(&b);
        assert_eq!(a.read().event_seq, 2);
        assert_eq!(b.read().event_seq, 1);
    }
}
