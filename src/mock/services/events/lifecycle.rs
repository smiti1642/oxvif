//! Private, volatile pull-point runtime. No public request/state fields are added.

use crate::mock::{
    fault::{
        Code, Fault, INVALID_ARGS, MOCK_REQUEST_LIMIT, MOCK_REQUEST_POLICY, MOCK_UNMODELED_EFFECT,
    },
    helpers::soap,
    policy::{AckOnlyOperation, AckOnlyPolicy},
    request::{Node, RequestError},
    state::{DeviceState, PendingIoEvent, SharedState},
};
use crate::types::xml_escape;
use std::{
    collections::{BTreeMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

const E: &str = "http://www.onvif.org/ver10/events/wsdl";
const N: &str = "http://docs.oasis-open.org/wsn/b-2";
const TOPICS: &str = "http://www.onvif.org/ver10/topics";
const DIALECT: &str = "http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet";
const CREATE: &str =
    "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest";
const PULL: &str =
    "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest";
const RENEW: &str = "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest";
const UNSUBSCRIBE: &str =
    "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest";
const CAPACITY: usize = 4;
const QUEUE_LIMIT: usize = 128;

#[derive(Clone, Debug, Default)]
pub(crate) struct Runtime {
    next_id: u64,
    subscriptions: BTreeMap<String, Subscription>,
    #[cfg(test)]
    now_override: Option<u64>,
}

#[derive(Clone, Debug)]
struct Subscription {
    expires: u64,
    filter: Option<Vec<String>>,
    queue: VecDeque<PendingIoEvent>,
    overflowed: bool,
    seq: u64,
}

impl Runtime {
    fn now(&self) -> u64 {
        #[cfg(test)]
        if let Some(now) = self.now_override {
            return now;
        }
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

fn timestamp(time: u64) -> String {
    let (y, m, d, h, min, sec) = super::epoch_to_civil(time);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{sec:02}Z")
}
fn policy(reason: &str) -> String {
    Fault::new(Code::Sender, &[MOCK_REQUEST_POLICY], reason).to_xml()
}
fn unsupported() -> String {
    Fault::new(
        Code::Sender,
        &[MOCK_UNMODELED_EFFECT],
        "Unmodeled pull-point setting",
    )
    .to_xml()
}
fn shape(node: &Node, ns: &str, names: &[&str], attrs: &[&str]) -> Result<(), String> {
    if node
        .element_children()
        .any(|(uri, name, _)| uri != ns || !names.contains(&name))
        || node
            .expanded_attributes()
            .any(|(uri, name, _)| !uri.is_empty() || !attrs.contains(&name))
    {
        return Err(unsupported());
    }
    node.check_child_sequence(ns, names)
        .map_err(RequestError::to_fault)
}
fn scalar<'a>(node: &'a Node, ns: &str, name: &str) -> Result<Option<&'a str>, String> {
    node.child(ns, name)
        .map_err(RequestError::to_fault)?
        .map(|n| {
            if n.expanded_attributes().next().is_some() {
                return Err(unsupported());
            }
            n.scalar_text().map_err(RequestError::to_fault)
        })
        .transpose()
}
fn required<'a>(node: &'a Node, ns: &str, name: &str) -> Result<&'a str, String> {
    scalar(node, ns, name)?.ok_or_else(|| RequestError::MissingField.to_fault())
}

// Deliberately bounded duration subset: integer PT hours/minutes/seconds,
// including combinations. Calendar and fractional durations are not modeled.
fn duration(value: &str) -> Option<u64> {
    let mut rest = value.strip_prefix("PT")?;
    let mut total = 0u64;
    let mut previous = 0;
    let mut any = false;
    while !rest.is_empty() {
        let split = rest.find(|c: char| !c.is_ascii_digit())?;
        let number: u64 = rest.get(..split)?.parse().ok()?;
        let unit = *rest.as_bytes().get(split)?;
        let (rank, factor) = match unit {
            b'H' => (1, 3600),
            b'M' => (2, 60),
            b'S' => (3, 1),
            _ => return None,
        };
        if rank <= previous {
            return None;
        }
        total = total.checked_add(number.checked_mul(factor)?)?;
        previous = rank;
        any = true;
        rest = rest.get(split + 1..)?;
    }
    any.then_some(total)
}

fn absolute(value: &str) -> Option<u64> {
    if value.len() != 20 || !value.is_ascii() {
        return None;
    }
    if &value[4..5] != "-"
        || &value[7..8] != "-"
        || &value[10..11] != "T"
        || &value[13..14] != ":"
        || &value[16..17] != ":"
        || &value[19..] != "Z"
    {
        return None;
    }
    let y: i64 = value[..4].parse().ok()?;
    let mo: i64 = value[5..7].parse().ok()?;
    let d: i64 = value[8..10].parse().ok()?;
    let h: i64 = value[11..13].parse().ok()?;
    let min: i64 = value[14..16].parse().ok()?;
    let sec: i64 = value[17..19].parse().ok()?;
    if !(1970..=9999).contains(&y)
        || !(1..=12).contains(&mo)
        || !(1..=31).contains(&d)
        || !(0..24).contains(&h)
        || !(0..60).contains(&min)
        || !(0..60).contains(&sec)
    {
        return None;
    }
    let adjusted = y - i64::from(mo <= 2);
    let era = adjusted.div_euclid(400);
    let year = adjusted - era * 400;
    let month = mo + if mo > 2 { -3 } else { 9 };
    let days =
        era * 146097 + year * 365 + year / 4 - year / 100 + (153 * month + 2) / 5 + d - 1 - 719468;
    let time = u64::try_from(days * 86400 + h * 3600 + min * 60 + sec).ok()?;
    (timestamp(time) == value).then_some(time)
}
fn expiry(value: Option<&str>, now: u64) -> Result<u64, String> {
    let value = value.unwrap_or("PT60S").trim();
    let end = duration(value)
        .and_then(|d| now.checked_add(d))
        .or_else(|| absolute(value));
    end.filter(|end| *end > now && *end - now <= 3600)
        .ok_or_else(|| policy("Pull-point lifetime must be within 1..3600 seconds"))
}

fn filter(operation: &Node) -> Result<Option<Vec<String>>, String> {
    let Some(filter) = operation
        .child(E, "Filter")
        .map_err(RequestError::to_fault)?
    else {
        return Ok(None);
    };
    shape(filter, N, &["TopicExpression"], &[])?;
    let Some(expression) = filter
        .child(N, "TopicExpression")
        .map_err(RequestError::to_fault)?
    else {
        return Ok(None);
    };
    // This scalar owns Dialect; do not run the container text-sequence check.
    if expression
        .expanded_attributes()
        .any(|(ns, name, _)| !ns.is_empty() || name != "Dialect")
        || expression.attribute("", "Dialect") != Some(DIALECT)
    {
        return Err(unsupported());
    }
    let text = expression.scalar_text().map_err(RequestError::to_fault)?;
    let mut selected = Vec::new();
    for part in text.split('|') {
        let part = part.trim();
        let (first, tail) = part.split_once('/').ok_or_else(unsupported)?;
        let (prefix, root) = first.split_once(':').unwrap_or(("", first));
        if expression.namespace_for_prefix(prefix) != Some(TOPICS) {
            return Err(unsupported());
        }
        let path = format!("{root}/{tail}");
        if ![
            "VideoSource/MotionAlarm",
            "RuleEngine/FieldDetector/ObjectsInside",
            "Device/Trigger/DigitalInput",
            "Device/Trigger/Relay",
        ]
        .contains(&path.as_str())
        {
            return Err(unsupported());
        }
        if !selected.contains(&path) {
            selected.push(path);
        }
    }
    Ok(Some(selected))
}

fn allowed(sub: &Subscription, topic: &str) -> bool {
    sub.filter
        .as_ref()
        .is_none_or(|f| f.iter().any(|t| t == topic))
}
fn io_topic(event: &PendingIoEvent) -> &str {
    match event.kind {
        "DigitalInput" => "Device/Trigger/DigitalInput",
        "RelayOutput" => "Device/Trigger/Relay",
        _ => "Device/Trigger/Unknown",
    }
}
fn fanout(runtime: &mut Runtime, state: &DeviceState, now: u64) {
    runtime.subscriptions.retain(|_, sub| sub.expires > now);
    for sub in runtime.subscriptions.values_mut() {
        for event in &state.pending_io_events {
            if allowed(sub, io_topic(event)) {
                if sub.queue.len() == QUEUE_LIMIT {
                    sub.overflowed = true;
                } else {
                    sub.queue.push_back(event.clone());
                }
            }
        }
    }
}

fn notification(
    topic: &str,
    source: (&str, &str),
    data: (&str, &str),
    seq: Option<u64>,
    now: u64,
) -> String {
    let seq = seq
        .map(|n| format!("<tt:SimpleItem Name=\"Seq\" Value=\"{n}\"/>"))
        .unwrap_or_default();
    format!(
        "<wsnt:NotificationMessage><wsnt:Topic Dialect=\"{DIALECT}\">tns1:{topic}</wsnt:Topic><wsnt:Message><tt:Message UtcTime=\"{}\" PropertyOperation=\"Changed\"><tt:Source><tt:SimpleItem Name=\"{}\" Value=\"{}\"/></tt:Source><tt:Data>{seq}<tt:SimpleItem Name=\"{}\" Value=\"{}\"/></tt:Data></tt:Message></wsnt:Message></wsnt:NotificationMessage>",
        timestamp(now),
        xml_escape(source.0),
        xml_escape(source.1),
        xml_escape(data.0),
        xml_escape(data.1)
    )
}
fn response(op: &str, body: &str) -> String {
    soap(
        &format!(
            "xmlns:tev=\"{E}\" xmlns:wsnt=\"{N}\" xmlns:wsa=\"http://www.w3.org/2005/08/addressing\" xmlns:tns1=\"{TOPICS}\""
        ),
        &format!("{op}{body}"),
    )
}
fn unknown() -> String {
    policy("Unknown or expired pull-point endpoint")
}

#[cfg(feature = "metamorph")]
pub(crate) fn is_lifecycle(action: &str) -> bool {
    matches!(action, CREATE | PULL | RENEW | UNSUBSCRIBE)
}

pub(crate) fn create(base: &str, state: &SharedState, operation: &Node) -> String {
    execute(CREATE, base, state, operation, None).unwrap_or_else(|fault| fault)
}
pub(crate) fn pull(
    base: &str,
    state: &SharedState,
    operation: &Node,
    endpoint: Option<&str>,
) -> String {
    execute(PULL, base, state, operation, endpoint).unwrap_or_else(|fault| fault)
}
pub(crate) fn renew(
    base: &str,
    state: &SharedState,
    operation: &Node,
    endpoint: Option<&str>,
    policy: &AckOnlyPolicy,
) -> String {
    if policy.enabled(AckOnlyOperation::EventsRenew) {
        return super::resp_renew();
    }
    execute(RENEW, base, state, operation, endpoint).unwrap_or_else(|fault| fault)
}
pub(crate) fn unsubscribe(
    base: &str,
    state: &SharedState,
    operation: &Node,
    endpoint: Option<&str>,
    policy: &AckOnlyPolicy,
) -> String {
    if policy.enabled(AckOnlyOperation::EventsUnsubscribe) {
        return crate::mock::helpers::resp_empty("wsnt", "UnsubscribeResponse");
    }
    execute(UNSUBSCRIBE, base, state, operation, endpoint).unwrap_or_else(|fault| fault)
}

fn execute(
    action: &str,
    base: &str,
    state: &SharedState,
    operation: &Node,
    endpoint: Option<&str>,
) -> Result<String, String> {
    let (selected, time, limit) = match action {
        CREATE => {
            shape(operation, E, &["Filter", "InitialTerminationTime"], &[])?;
            (
                filter(operation)?,
                scalar(operation, E, "InitialTerminationTime")?,
                0,
            )
        }
        PULL => {
            shape(operation, E, &["Timeout", "MessageLimit"], &[])?;
            let timeout = required(operation, E, "Timeout")?;
            if duration(timeout.trim()).is_none_or(|t| t > 60) {
                return Err(policy("Pull-point timeout must be within 0..60 seconds"));
            }
            let n: i32 = required(operation, E, "MessageLimit")?
                .trim()
                .parse()
                .map_err(|_| RequestError::EmptyField.to_fault())?;
            if n <= 0 {
                return Err(Fault::new(
                    Code::Sender,
                    &[INVALID_ARGS],
                    "MessageLimit must be positive",
                )
                .to_xml());
            }
            (None, Some(timeout), (n as usize).min(QUEUE_LIMIT))
        }
        RENEW => {
            shape(operation, N, &["TerminationTime"], &[])?;
            (None, Some(required(operation, N, "TerminationTime")?), 0)
        }
        UNSUBSCRIBE => {
            shape(operation, N, &[], &[])?;
            (None, None, 0)
        }
        _ => return Err(unsupported()),
    };
    // Always state -> runtime. Both locks are released before on_change callbacks.
    state.modify_returning_if(|device| {
        let mut runtime = state.subscriptions.lock().unwrap_or_else(|e| e.into_inner());
        let now = runtime.now();
        let endpoint = endpoint.unwrap_or_default();
        if action != CREATE && !runtime.subscriptions.get(endpoint).is_some_and(|s| s.expires > now) { return Err(unknown()); }
        let expires = if action == CREATE || action == RENEW { expiry(time, now)? } else { 0 };
        let mut next = runtime.clone();
        let mut synthesized = false;
        let answer = match action {
            CREATE => {
                if next.subscriptions.values().filter(|s| s.expires > now).count() >= CAPACITY {
                    return Err(Fault::new(Code::Receiver, &[MOCK_REQUEST_LIMIT], "Pull-point capacity exceeded").to_xml());
                }
                let id = next.next_id.checked_add(1).ok_or_else(|| policy("Pull-point identity space exhausted"))?;
                let url = format!("{base}/onvif/events/subscription_{id}");
                fanout(&mut next, device, now);
                next.next_id = id;
                next.subscriptions.insert(url.clone(), Subscription { expires, filter: selected.clone(), queue: VecDeque::new(), overflowed: false, seq: 0 });
                response("<tev:CreatePullPointSubscriptionResponse>", &format!("<tev:SubscriptionReference><wsa:Address>{}</wsa:Address></tev:SubscriptionReference><wsnt:CurrentTime>{}</wsnt:CurrentTime><wsnt:TerminationTime>{}</wsnt:TerminationTime></tev:CreatePullPointSubscriptionResponse>", xml_escape(&url), timestamp(now), timestamp(expires)))
            }
            RENEW => {
                let sub = next.subscriptions.get_mut(endpoint).ok_or_else(unknown)?;
                sub.expires = expires;
                fanout(&mut next, device, now);
                response("<wsnt:RenewResponse>", &format!("<wsnt:TerminationTime>{}</wsnt:TerminationTime><wsnt:CurrentTime>{}</wsnt:CurrentTime></wsnt:RenewResponse>", timestamp(expires), timestamp(now)))
            }
            UNSUBSCRIBE => {
                next.subscriptions.remove(endpoint);
                fanout(&mut next, device, now);
                response("", "<wsnt:UnsubscribeResponse/>")
            }
            PULL => {
                fanout(&mut next, device, now);
                let sub = next.subscriptions.get_mut(endpoint).ok_or_else(unknown)?;
                if sub.overflowed { return Err(policy("Pull-point queue overflow; unsubscribe and create a new subscription")); }
                let mut body = String::new();
                for _ in 0..limit {
                    let Some(event) = sub.queue.pop_front() else { break; };
                    let name = if event.kind == "DigitalInput" { "InputToken" } else { "RelayToken" };
                    body.push_str(&notification(io_topic(&event), (name, &event.token), ("LogicalState", if event.logical_state == "active" { "true" } else { "false" }), None, now));
                }
                if body.is_empty() {
                    synthesized = true;
                    sub.seq = sub.seq.wrapping_add(1);
                    let (topic, source, data) = if sub.seq % 2 == 1 { ("VideoSource/MotionAlarm", ("VideoSourceToken", "VS_1"), ("IsMotion", if (sub.seq / 2) & 1 == 0 { "true" } else { "false" })) }
                        else { ("RuleEngine/FieldDetector/ObjectsInside", ("Rule", "MyFieldDetectorRule"), ("ObjectId", "7")) };
                    if allowed(sub, topic) { body = notification(topic, source, data, Some(sub.seq), now); }
                }
                let keep_alive = duration(time.unwrap_or("PT0S").trim()).unwrap_or(0);
                sub.expires = sub.expires.max(now.saturating_add(keep_alive));
                response("<tev:PullMessagesResponse>", &format!("<tev:CurrentTime>{}</tev:CurrentTime><tev:TerminationTime>{}</tev:TerminationTime>{body}</tev:PullMessagesResponse>", timestamp(now), timestamp(sub.expires)))
            }
            _ => return Err(unsupported()),
        };
        // Commit only after the whole candidate succeeds, including fan-out and
        // slow-consumer overflow checks. Refusals preserve ingress and runtime.
        *runtime = next;
        device.pending_io_events.clear();
        if synthesized { device.event_seq = device.event_seq.wrapping_add(1); }
        Ok(answer)
    }, Result::is_ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{request::Request, state::MockState};

    #[test]
    fn reentrant_hook_runs_after_runtime_commit_and_counter_wrap() {
        use std::sync::{
            Arc, OnceLock, Weak,
            atomic::{AtomicBool, Ordering},
        };
        let mut state = MockState::new();
        call(&state, CREATE, &create_xml("PT60S"), None).unwrap();
        let url = "http://mock/onvif/events/subscription_1";
        state.modify(|s| s.event_seq = u64::MAX);
        state
            .subscriptions
            .lock()
            .unwrap()
            .subscriptions
            .get_mut(url)
            .unwrap()
            .seq = u64::MAX;
        let target = Arc::new(OnceLock::<Weak<MockState>>::new());
        let callback_target = target.clone();
        let fired = Arc::new(AtomicBool::new(false));
        let callback_fired = fired.clone();
        state.set_on_change(Arc::new(move |_| {
            if !callback_fired.swap(true, Ordering::SeqCst) {
                let state = callback_target.get().unwrap().upgrade().unwrap();
                assert_eq!(state.read().event_seq, 0);
                let unsubscribe = format!("<n:Unsubscribe xmlns:n='{N}'/>");
                call(&state, UNSUBSCRIBE, &unsubscribe, Some(url)).unwrap();
            }
        }));
        let state = Arc::new(state);
        target.set(Arc::downgrade(&state)).unwrap();
        let response = call(&state, PULL, &pull_xml(), Some(url)).unwrap();
        assert!(response.contains("ObjectsInside"));
        assert!(response.contains("Name=\"Seq\" Value=\"0\""));
        assert!(fired.load(Ordering::SeqCst));
        assert_eq!(call(&state, PULL, &pull_xml(), Some(url)), Err(unknown()));
    }

    fn call(
        state: &MockState,
        action: &str,
        xml: &str,
        endpoint: Option<&str>,
    ) -> Result<String, String> {
        let request = Request::parse(xml).unwrap();
        let (ns, name) = match action {
            CREATE => (E, "CreatePullPointSubscription"),
            PULL => (E, "PullMessages"),
            RENEW => (N, "Renew"),
            _ => (N, "Unsubscribe"),
        };
        execute(
            action,
            "http://mock",
            state,
            request.operation(ns, name).unwrap(),
            endpoint,
        )
    }
    fn create_xml(time: &str) -> String {
        format!(
            "<e:CreatePullPointSubscription xmlns:e='{E}'><e:InitialTerminationTime>{time}</e:InitialTerminationTime></e:CreatePullPointSubscription>"
        )
    }
    fn pull_xml() -> String {
        format!(
            "<e:PullMessages xmlns:e='{E}'><e:Timeout>PT0S</e:Timeout><e:MessageLimit>1</e:MessageLimit></e:PullMessages>"
        )
    }
    fn snapshot(state: &MockState) -> (serde_json::Value, String) {
        (
            serde_json::to_value(&*state.read()).unwrap(),
            format!(
                "{:?}|{:?}|{}|{:?}",
                *state.subscriptions.lock().unwrap(),
                state.read().pending_io_events,
                state.read().event_seq,
                state.read().event_filter
            ),
        )
    }

    #[test]
    fn controlled_expiry_renewal_and_counter_exhaustion_preserve_rejections() {
        let state = MockState::new();
        state.subscriptions.lock().unwrap().now_override = Some(1704067200);
        let first = call(&state, CREATE, &create_xml("PT1M"), None).unwrap();
        assert!(first.contains("2024-01-01T00:01:00Z"));
        let url = "http://mock/onvif/events/subscription_1";
        let renew = format!(
            "<n:Renew xmlns:n='{N}'><n:TerminationTime>2024-01-01T00:02:00Z</n:TerminationTime></n:Renew>"
        );
        assert!(
            call(&state, RENEW, &renew, Some(url))
                .unwrap()
                .contains("2024-01-01T00:02:00Z")
        );
        state.subscriptions.lock().unwrap().now_override = Some(1704067320);
        let before = snapshot(&state);
        assert_eq!(call(&state, PULL, &pull_xml(), Some(url)), Err(unknown()));
        assert_eq!(call(&state, RENEW, &renew, Some(url)), Err(unknown()));
        assert_eq!(snapshot(&state), before);
        let next = call(&state, CREATE, &create_xml("PT60S"), None).unwrap();
        assert!(next.contains("subscription_2"));
        assert_eq!(state.subscriptions.lock().unwrap().subscriptions.len(), 1);
        state.subscriptions.lock().unwrap().next_id = u64::MAX;
        let before = snapshot(&state);
        assert_eq!(
            call(&state, CREATE, &create_xml("PT60S"), None),
            Err(policy("Pull-point identity space exhausted"))
        );
        assert_eq!(snapshot(&state), before);
    }

    #[test]
    fn time_forms_validate_calendar_and_checked_arithmetic() {
        assert_eq!(duration("PT1H2M3S"), Some(3723));
        assert_eq!(duration("PT0S"), Some(0));
        for text in [
            "PT",
            "PT1S1M",
            "PT1M1M",
            "PT18446744073709551615H",
            "PT+1S",
            "PT-1S",
            "PTNaNS",
            "PT1.5S",
        ] {
            assert_eq!(duration(text), None, "{text}");
        }
        assert_eq!(absolute("2024-02-29T00:00:00Z"), Some(1709164800));
        for text in [
            "2023-02-29T00:00:00Z",
            "2024-04-31T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:60:00Z",
            "2024-01-01T00:00:60Z",
            "2024-01-01T00:00:00+00:00",
            "北024-01-01T00:00:00Z",
        ] {
            assert_eq!(absolute(text), None, "{text}");
        }
    }

    #[test]
    fn topic_prefixes_belong_to_the_expression_scope() {
        let state = MockState::new();
        let template = format!(
            "<e:CreatePullPointSubscription xmlns:e='{E}' xmlns:n='{N}' xmlns:t='urn:decoy'><e:Filter><n:TopicExpression xmlns:t='{TOPICS}' Dialect='{DIALECT}'>t:Device/Trigger/DigitalInput</n:TopicExpression></e:Filter></e:CreatePullPointSubscription>"
        );
        call(&state, CREATE, &template, None).unwrap();
        assert_eq!(
            state
                .subscriptions
                .lock()
                .unwrap()
                .subscriptions
                .values()
                .next()
                .unwrap()
                .filter,
            Some(vec!["Device/Trigger/DigitalInput".into()])
        );
        let before = snapshot(&state);
        let wrong = template.replace(&format!("xmlns:t='{TOPICS}'"), "xmlns:t='urn:wrong'");
        assert_eq!(call(&state, CREATE, &wrong, None), Err(unsupported()));
        assert_eq!(snapshot(&state), before);
        let default = template
            .replace(&format!("xmlns:t='{TOPICS}'"), &format!("xmlns='{TOPICS}'"))
            .replace("t:Device/", "Device/");
        call(&state, CREATE, &default, None).unwrap();
        assert_eq!(state.subscriptions.lock().unwrap().subscriptions.len(), 2);
    }

    #[test]
    fn slow_consumer_overflow_is_explicit_and_does_not_stop_a_fast_consumer() {
        let state = MockState::new();
        call(&state, CREATE, &create_xml("PT60S"), None).unwrap();
        call(&state, CREATE, &create_xml("PT60S"), None).unwrap();
        let fast = "http://mock/onvif/events/subscription_1";
        let slow = "http://mock/onvif/events/subscription_2";
        for i in 0..=QUEUE_LIMIT {
            state.modify(|s| {
                s.pending_io_events.push(PendingIoEvent {
                    kind: "DigitalInput",
                    token: i.to_string(),
                    logical_state: "active".into(),
                })
            });
            let xml = call(&state, PULL, &pull_xml(), Some(fast)).unwrap();
            assert!(xml.contains(&format!("Name=\"InputToken\" Value=\"{i}\"")));
        }
        let before = snapshot(&state);
        assert_eq!(
            call(&state, PULL, &pull_xml(), Some(slow)),
            Err(policy(
                "Pull-point queue overflow; unsubscribe and create a new subscription"
            ))
        );
        assert_eq!(snapshot(&state), before);
        assert_eq!(
            state.subscriptions.lock().unwrap().subscriptions[slow]
                .queue
                .len(),
            QUEUE_LIMIT
        );
        call(&state, PULL, &pull_xml(), Some(fast)).unwrap();
        let unsubscribe = format!("<n:Unsubscribe xmlns:n='{N}'/>");
        call(&state, UNSUBSCRIBE, &unsubscribe, Some(slow)).unwrap();
        assert!(
            !state
                .subscriptions
                .lock()
                .unwrap()
                .subscriptions
                .contains_key(slow)
        );
    }
}
