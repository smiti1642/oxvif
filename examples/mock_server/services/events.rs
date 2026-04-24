use crate::helpers::soap;
use crate::xml_parse::extract_tag;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Per-process counter so each PullMessages returns a distinct event the
/// UI can visibly differentiate. Real cameras emit when motion is actually
/// detected; for the mock we just synthesize a fresh event on every pull
/// so the UI's scrolling log demonstrably grows.
static EVENT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Deadline for the *next* event to fire. Pull requests sleep until this
/// Instant (capped by PULL_MAX_WAIT), then advance it by EVENT_INTERVAL
/// atomically so the stream paces itself regardless of client polling
/// speed. Stored as a tokio::time::Instant so `start_paused` tests can
/// drive it with `tokio::time::advance`.
fn next_event_deadline() -> &'static Mutex<tokio::time::Instant> {
    static CELL: OnceLock<Mutex<tokio::time::Instant>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(tokio::time::Instant::now()))
}

/// Accepted topics for the active subscription, parsed from the client's
/// `<wsnt:TopicExpression>` on CreatePullPointSubscription. `None` means
/// no filter = emit every generated event. A `Some(vec)` means only emit
/// events whose topic exactly matches one of the entries.
///
/// Single-client assumption: we don't track per-subscription filters;
/// whichever client most recently subscribed wins. Fine for oxdm dev.
fn filter_topics() -> &'static Mutex<Option<Vec<String>>> {
    static CELL: OnceLock<Mutex<Option<Vec<String>>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

/// Spacing between synthesized events. 3 s feels alive but not spammy.
const EVENT_INTERVAL: Duration = Duration::from_secs(3);
/// Cap on how long a single pull will block waiting for the next event.
/// Picked just under typical client long-poll timeouts (oxdm uses PT5S)
/// so empty responses come back before the client gives up the request.
const PULL_MAX_WAIT: Duration = Duration::from_millis(4_500);

pub fn resp_event_properties() -> String {
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wstop="http://docs.oasis-open.org/wsn/t-1""#,
        r#"<tev:GetEventPropertiesResponse>
          <tev:TopicNamespaceLocation>http://www.onvif.org/onvif/ver10/topics/topicns.xml</tev:TopicNamespaceLocation>
          <wstop:FixedTopicSet>true</wstop:FixedTopicSet>
          <wstop:TopicSet>
            <tns1:VideoSource wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:MotionAlarm wstop:topic="true"/>
            </tns1:VideoSource>
            <tns1:RuleEngine wstop:topic="false" xmlns:tns1="http://www.onvif.org/ver10/topics">
              <tns1:FieldDetector wstop:topic="false">
                <tns1:ObjectsInside wstop:topic="true"/>
              </tns1:FieldDetector>
            </tns1:RuleEngine>
          </wstop:TopicSet>
          <tev:TopicExpressionDialect>http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet</tev:TopicExpressionDialect>
        </tev:GetEventPropertiesResponse>"#,
    )
}

/// Format SystemTime as RFC3339-ish UTC ("2026-04-23T12:34:56Z").
fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Civil-date conversion via the time crate would pull in a heavy dep;
    // for a mock the proleptic Gregorian breakdown below is plenty.
    let (y, m, d, hh, mm, ss) = epoch_to_civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert seconds-since-Unix-epoch to (year, month, day, hour, min, sec).
/// Uses Howard Hinnant's days_from_civil algorithm in reverse — exact for
/// any date in the proleptic Gregorian calendar.
fn epoch_to_civil(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let time_of_day = (secs % 86_400) as u32;
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;

    // days_from_civil inverse — assumes days are since 1970-01-01.
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

pub fn resp_create_pull_point_subscription(base: &str, body: &str) -> String {
    // Parse the optional <wsnt:TopicExpression>topic1|topic2|...</...>
    // and store it so subsequent PullMessages can filter. Empty or absent
    // means "no filter" — clients subscribing without a filter expect
    // every topic.
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
    *filter_topics().lock().unwrap() = new_filter;

    let now = now_rfc3339();
    soap(
        r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
        &format!(
            r#"<tev:CreatePullPointSubscriptionResponse>
          <tev:SubscriptionReference>
            <wsa:Address>{base}/onvif/events/subscription_1</wsa:Address>
          </tev:SubscriptionReference>
          <tev:CurrentTime>{now}</tev:CurrentTime>
          <tev:TerminationTime>{now}</tev:TerminationTime>
        </tev:CreatePullPointSubscriptionResponse>"#
        ),
    )
}

pub async fn resp_pull_messages() -> String {
    let target = *next_event_deadline().lock().unwrap();
    let now = tokio::time::Instant::now();
    let wait = target.saturating_duration_since(now).min(PULL_MAX_WAIT);
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }

    // After waking, check whether we actually reached the slot. If the
    // wait was capped by PULL_MAX_WAIT (no new event ready yet), return
    // an empty PullMessagesResponse — a valid ONVIF reply that tells the
    // client "no events this round, long-poll again."
    let woke_at = tokio::time::Instant::now();
    if woke_at < target {
        let now_str = now_rfc3339();
        return soap(
            r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl""#,
            &format!(
                r#"<tev:PullMessagesResponse>
              <tev:CurrentTime>{now_str}</tev:CurrentTime>
              <tev:TerminationTime>{now_str}</tev:TerminationTime>
            </tev:PullMessagesResponse>"#
            ),
        );
    }
    // Claim the slot and reserve the next one EVENT_INTERVAL out.
    *next_event_deadline().lock().unwrap() = target + EVENT_INTERVAL;

    let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let now = now_rfc3339();
    // Alternate between a motion alarm and a "rule engine" event so the
    // log shows variety. Pure rotation — mocks don't need RNG.
    let (topic, source_name, source_value, data_name, data_value) = if seq % 2 == 1 {
        (
            "tns1:VideoSource/MotionAlarm",
            "VideoSourceToken",
            "VideoSource_1",
            "IsMotion",
            if (seq / 2) % 2 == 0 { "true" } else { "false" },
        )
    } else {
        (
            "tns1:RuleEngine/FieldDetector/ObjectsInside",
            "Rule",
            "MyFieldDetectorRule",
            "ObjectId",
            // Rotate object IDs so every event row reads differently.
            match seq % 6 {
                0 => "42",
                2 => "7",
                _ => "13",
            },
        )
    };

    // Apply the subscription's topic filter, if any. When the candidate
    // topic is filtered out, we still consumed the slot (simulating the
    // underlying event firing on the camera) but return a zero-message
    // response — which is exactly what a real camera does with a
    // ConcreteSet filter that doesn't match.
    if let Some(ref allowed) = *filter_topics().lock().unwrap() {
        if !allowed.iter().any(|a| a == topic) {
            return soap(
                r#"xmlns:tev="http://www.onvif.org/ver10/events/wsdl""#,
                &format!(
                    r#"<tev:PullMessagesResponse>
                  <tev:CurrentTime>{now}</tev:CurrentTime>
                  <tev:TerminationTime>{now}</tev:TerminationTime>
                </tev:PullMessagesResponse>"#
                ),
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_civil_known_dates() {
        // 1970-01-01T00:00:00Z
        assert_eq!(epoch_to_civil(0), (1970, 1, 1, 0, 0, 0));
        // 2024-01-01T00:00:00Z — round-trippable check against a known
        // post-Y2K leap-year boundary.
        assert_eq!(epoch_to_civil(1_704_067_200), (2024, 1, 1, 0, 0, 0));
        // 2024-02-29T12:34:56Z — leap day, non-trivial case.
        assert_eq!(epoch_to_civil(1_709_210_096), (2024, 2, 29, 12, 34, 56));
    }

    /// Serializes event tests — EVENT_SEQ / filter / deadline are all
    /// module-level globals, so parallel tests stomp on each other.
    fn test_lock() -> &'static Mutex<()> {
        static CELL: OnceLock<Mutex<()>> = OnceLock::new();
        CELL.get_or_init(|| Mutex::new(()))
    }

    /// Lock + clear the shared state. Caller MUST bind the returned
    /// guard (`let _lock = reset_state();`) — dropping it early releases
    /// the lock and lets another test clobber state mid-run.
    #[must_use]
    fn reset_state() -> std::sync::MutexGuard<'static, ()> {
        let guard = test_lock().lock().unwrap_or_else(|e| e.into_inner());
        *next_event_deadline().lock().unwrap() = tokio::time::Instant::now();
        *filter_topics().lock().unwrap() = None;
        EVENT_SEQ.store(0, Ordering::Relaxed);
        guard
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pull_messages_returns_distinct_events() {
        let _lock = reset_state();
        // Paused tokio time + advance() lets the EVENT_INTERVAL sleep
        // complete instantly while still proving the deadline logic works.
        let a = resp_pull_messages().await;
        tokio::time::advance(EVENT_INTERVAL + Duration::from_millis(100)).await;
        let b = resp_pull_messages().await;
        assert_ne!(a, b);
        assert!(a.contains("NotificationMessage"));
        assert!(b.contains("NotificationMessage"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pull_messages_paces_itself() {
        // Real time (not paused) — back-to-back pulls should take at
        // least EVENT_INTERVAL because the second one blocks waiting
        // for its slot. Use a tighter assertion than the full interval
        // so timer slack on slow CI doesn't cause flakes.
        let _lock = reset_state();
        let _ = resp_pull_messages().await;
        let start = tokio::time::Instant::now();
        let _ = resp_pull_messages().await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= EVENT_INTERVAL - Duration::from_millis(200),
            "second pull returned in {elapsed:?}, expected ~{EVENT_INTERVAL:?}"
        );
    }

    #[test]
    fn create_pull_point_parses_filter() {
        let _lock = reset_state();
        let body = r#"<tev:CreatePullPointSubscription>
            <tev:Filter>
              <wsnt:TopicExpression Dialect="...ConcreteSet">
                tns1:VideoSource/MotionAlarm|tns1:RuleEngine/FieldDetector/ObjectsInside
              </wsnt:TopicExpression>
            </tev:Filter>
          </tev:CreatePullPointSubscription>"#;
        let _ = resp_create_pull_point_subscription("http://mock", body);
        let stored = filter_topics().lock().unwrap().clone();
        assert_eq!(
            stored,
            Some(vec![
                "tns1:VideoSource/MotionAlarm".to_string(),
                "tns1:RuleEngine/FieldDetector/ObjectsInside".to_string(),
            ])
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pull_messages_respects_filter() {
        let _lock = reset_state();
        // Filter to motion only; rule-engine events (even seq) must come
        // back as empty responses, motion events (odd seq) as full.
        *filter_topics().lock().unwrap() = Some(vec!["tns1:VideoSource/MotionAlarm".to_string()]);
        let seq1 = resp_pull_messages().await; // seq=1 → motion → pass
        tokio::time::advance(EVENT_INTERVAL + Duration::from_millis(100)).await;
        let seq2 = resp_pull_messages().await; // seq=2 → rule → filtered
        assert!(seq1.contains("NotificationMessage"), "motion must pass");
        assert!(
            !seq2.contains("NotificationMessage"),
            "rule-engine must be filtered"
        );
        assert!(
            seq2.contains("PullMessagesResponse"),
            "empty response still valid"
        );
    }
}
