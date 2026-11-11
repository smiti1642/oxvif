//! Independent scripted-device controls for the owned recording workflow.
use async_trait::async_trait;
use oxvif::{
    OnvifClient, OnvifError, RecordingSearchCleanup as Cleanup, RecordingSearchOptions as Options,
    RecordingSearchStopReason as Stop,
    soap::SoapError,
    transport::{Transport, TransportError},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

const URL: &str = "http://camera.test/search";
const FIND: &str = "http://www.onvif.org/ver10/search/wsdl/FindRecordings";
const POLL: &str = "http://www.onvif.org/ver10/search/wsdl/GetRecordingSearchResults";
const END: &str = "http://www.onvif.org/ver10/search/wsdl/EndSearch";

enum Reply {
    Xml(String),
    Status(u16),
    Pending,
}
#[derive(Clone, Debug)]
struct Call {
    url: String,
    action: String,
    body: String,
    at: tokio::time::Instant,
}
struct Device {
    steps: Mutex<VecDeque<(&'static str, Reply)>>,
    calls: Mutex<Vec<Call>>,
    changed: tokio::sync::Notify,
}
impl Device {
    fn new(steps: Vec<(&'static str, Reply)>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(steps.into()),
            calls: Mutex::new(Vec::new()),
            changed: tokio::sync::Notify::new(),
        })
    }
    fn client(self: &Arc<Self>) -> OnvifClient {
        OnvifClient::new("http://camera.test/device").with_transport(self.clone())
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn assert_actions(&self, actions: &[&str]) {
        let calls = self.calls();
        assert_eq!(
            calls.iter().map(|c| c.action.as_str()).collect::<Vec<_>>(),
            actions
        );
        assert!(calls.iter().all(|c| c.url == URL));
        assert!(
            self.steps.lock().unwrap().is_empty(),
            "unused scripted replies"
        );
    }
    async fn until_calls(&self, count: usize) {
        loop {
            let changed = self.changed.notified();
            if self.calls.lock().unwrap().len() >= count {
                return;
            }
            changed.await;
        }
    }
}
#[async_trait]
impl Transport for Device {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        self.calls.lock().unwrap().push(Call {
            url: url.into(),
            action: action.into(),
            body,
            at: tokio::time::Instant::now(),
        });
        self.changed.notify_one();
        let (expected, reply) = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected RPC");
        assert_eq!(action, expected);
        match reply {
            Reply::Xml(xml) => Ok(xml),
            Reply::Status(status) => Err(TransportError::HttpStatus {
                status,
                body: "private-body".into(),
            }),
            Reply::Pending => std::future::pending().await,
        }
    }
}
fn envelope(body: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tse="http://www.onvif.org/ver10/search/wsdl" xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body>{body}</s:Body></s:Envelope>"#
    )
}
fn found(token: &str) -> Reply {
    Reply::Xml(envelope(&format!(
        "<tse:FindRecordingsResponse><tse:SearchToken>{token}</tse:SearchToken></tse:FindRecordingsResponse>"
    )))
}
fn page(state: &str, records: &str) -> Reply {
    Reply::Xml(envelope(&format!(
        "<tse:GetRecordingSearchResultsResponse><tse:ResultList><tt:SearchState>{state}</tt:SearchState>{records}</tse:ResultList></tse:GetRecordingSearchResultsResponse>"
    )))
}
fn released() -> Reply {
    Reply::Xml(envelope(
        "<tse:EndSearchResponse><tse:Endpoint>2026-11-11T10:20:30Z</tse:Endpoint></tse:EndSearchResponse>",
    ))
}
// Hand-authored synthetic data; no device capture or external schema is embedded.
const FIRST: &str = "<tt:RecordingInformation><tt:RecordingToken>same</tt:RecordingToken><tt:Source><tt:Name>門口</tt:Name></tt:Source><tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording><tt:LatestRecording>2026-01-02T00:00:00Z</tt:LatestRecording><tt:Content>motion</tt:Content><tt:RecordingStatus>Recording</tt:RecordingStatus></tt:RecordingInformation>";
const SECOND: &str = "<tt:RecordingInformation><tt:RecordingToken>same</tt:RecordingToken><tt:Source><tt:Name>rear</tt:Name></tt:Source><tt:Content>continuous</tt:Content><tt:RecordingStatus>Stopped</tt:RecordingStatus></tt:RecordingInformation>";
fn assert_released(cleanup: &Cleanup) {
    assert!(
        matches!(cleanup, Cleanup::Released { endpoint: Some(value) } if value == "2026-11-11T10:20:30Z")
    );
}
fn assert_status(error: &OnvifError, expected: u16) {
    assert!(
        matches!(error, OnvifError::Transport(TransportError::HttpStatus { status, body }) if *status == expected && body == "private-body")
    );
}

#[tokio::test(start_paused = true)]
async fn queued_then_completed_preserves_records_order_fields_and_wire_options() {
    let device = Device::new(vec![
        (FIND, found("opaque&amp;&lt;")),
        (POLL, page("Queued", "")),
        (POLL, page("Searching", FIRST)),
        (POLL, page("Completed", SECOND)),
        (END, released()),
    ]);
    let options = Options::default()
        .with_page_size(17)
        .with_max_matches(Some(3))
        .with_wait_time(Duration::from_millis(125))
        .with_keep_alive(Duration::from_millis(60500));
    let report = device
        .client()
        .search_recordings_with_report(URL, options)
        .await
        .unwrap();
    assert_eq!(report.stop_reason(), Stop::Completed);
    assert_eq!(report.pages(), 3);
    assert_released(report.cleanup());
    let records = report.recordings();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].recording_token, "same");
    assert_eq!(
        records[1].recording_token, "same",
        "opaque tokens must not deduplicate pages"
    );
    assert_eq!(records[0].source_name, "門口");
    assert_eq!(
        records[0].earliest_recording.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
    assert_eq!(
        records[0].latest_recording.as_deref(),
        Some("2026-01-02T00:00:00Z")
    );
    assert_eq!(records[0].content, "motion");
    assert_eq!(records[0].recording_status, "Recording");
    assert_eq!(records[1].source_name, "rear");
    assert_eq!(records[1].earliest_recording, None);
    assert_eq!(records[1].latest_recording, None);
    assert_eq!(records[1].content, "continuous");
    assert_eq!(records[1].recording_status, "Stopped");
    device.assert_actions(&[FIND, POLL, POLL, POLL, END]);
    let calls = device.calls();
    assert!(calls[0].body.contains("<tse:MaxMatches>3</tse:MaxMatches>"));
    assert!(
        calls[0]
            .body
            .contains("<tse:KeepAliveTime>PT60.5S</tse:KeepAliveTime>")
    );
    for call in &calls[1..] {
        assert!(
            call.body
                .contains("<tse:SearchToken>opaque&amp;&lt;</tse:SearchToken>")
        );
    }
    for call in &calls[1..4] {
        assert!(call.body.contains("<tse:MaxResults>17</tse:MaxResults>"));
        assert!(call.body.contains("<tse:WaitTime>PT0.125S</tse:WaitTime>"));
    }
    assert!(calls[2].at.duration_since(calls[1].at) >= Duration::from_millis(100));
    assert!(calls[3].at.duration_since(calls[2].at) >= Duration::from_millis(100));
}

#[tokio::test]
async fn failed_find_or_empty_token_never_sends_cleanup() {
    for reply in [Reply::Status(403), found("")] {
        let device = Device::new(vec![(FIND, reply)]);
        let failure = device
            .client()
            .search_recordings_with_report(URL, Options::default())
            .await
            .unwrap_err();
        assert_eq!(failure.report().stop_reason(), Stop::FindFailed);
        assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
        match failure.cause().unwrap() {
            OnvifError::Transport(_) => assert_status(failure.cause().unwrap(), 403),
            error => assert!(matches!(
                error,
                OnvifError::Soap(SoapError::MissingField("SearchToken"))
            )),
        }
        device.assert_actions(&[FIND]);
    }
}

#[tokio::test(start_paused = true)]
async fn poll_failure_retains_partial_page_and_primary_error_when_cleanup_also_fails() {
    let device = Device::new(vec![
        (FIND, found("token-A")),
        (POLL, page("Searching", FIRST)),
        (POLL, Reply::Status(503)),
        (END, Reply::Status(401)),
    ]);
    let failure = device
        .client()
        .search_recordings_with_report(URL, Options::default())
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::PollFailed);
    assert_eq!(failure.report().recordings()[0].source_name, "門口");
    assert_eq!(failure.report().pages(), 1);
    assert_status(failure.cause().unwrap(), 503);
    match failure.report().cleanup() {
        Cleanup::Failed(error) => assert_status(error, 401),
        other => panic!("{other:?}"),
    }
    assert!(!failure.to_string().contains("private-body"));
    assert!(std::error::Error::source(&*failure).is_some());
    assert!(
        device.calls()[3]
            .body
            .contains("<tse:SearchToken>token-A</tse:SearchToken>")
    );
    device.assert_actions(&[FIND, POLL, POLL, END]);
}

#[tokio::test]
async fn malformed_poll_still_releases_the_exact_token() {
    let device = Device::new(vec![
        (FIND, found("token-B")),
        (POLL, Reply::Xml("<broken>".into())),
        (END, released()),
    ]);
    let failure = device
        .client()
        .search_recordings_with_report(URL, Options::default())
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::PollFailed);
    assert!(
        matches!(failure.cause(), Some(OnvifError::Soap(SoapError::XmlParse(message))) if message == "incomplete XML document")
    );
    assert_released(failure.report().cleanup());
    assert!(
        device.calls()[2]
            .body
            .contains("<tse:SearchToken>token-B</tse:SearchToken>")
    );
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test(start_paused = true)]
async fn completion_on_last_page_succeeds_but_nonterminal_boundary_is_failure() {
    for (state, expected) in [
        ("Completed", Stop::Completed),
        ("Searching", Stop::PageLimit),
        ("private-unknown-state", Stop::UnknownState),
    ] {
        let device = Device::new(vec![
            (FIND, found("boundary")),
            (POLL, page(state, FIRST)),
            (END, released()),
        ]);
        let result = device
            .client()
            .search_recordings_with_report(URL, Options::default().with_limits(1, 1, 4096))
            .await;
        let report = match result {
            Ok(report) => {
                assert_eq!(expected, Stop::Completed);
                report
            }
            Err(failure) => {
                assert_ne!(expected, Stop::Completed);
                assert!(!failure.to_string().contains("private-unknown-state"));
                assert!(failure.cause().is_none());
                failure.into_parts().0
            }
        };
        assert_eq!(report.stop_reason(), expected);
        assert_eq!(report.pages(), 1);
        assert_eq!(
            report.recordings().len(),
            usize::from(expected != Stop::UnknownState)
        );
        assert_released(report.cleanup());
        device.assert_actions(&[FIND, POLL, END]);
    }
}

#[tokio::test(start_paused = true)]
async fn item_and_max_matches_limits_reject_whole_page_and_keep_earlier_results() {
    for options in [
        Options::default().with_limits(5, 1, 4096),
        Options::default().with_max_matches(Some(1)),
    ] {
        let device = Device::new(vec![
            (FIND, found("items")),
            (POLL, page("Searching", FIRST)),
            (POLL, page("Completed", SECOND)),
            (END, released()),
        ]);
        let failure = device
            .client()
            .search_recordings_with_report(URL, options)
            .await
            .unwrap_err();
        assert_eq!(failure.report().stop_reason(), Stop::ItemLimit);
        assert_eq!(failure.report().pages(), 2);
        assert_eq!(failure.report().recordings().len(), 1);
        assert_eq!(failure.report().recordings()[0].source_name, "門口");
        assert_released(failure.report().cleanup());
        device.assert_actions(&[FIND, POLL, POLL, END]);
    }
    let device = Device::new(vec![
        (FIND, found("whole")),
        (POLL, page("Completed", &format!("{FIRST}{SECOND}"))),
        (END, released()),
    ]);
    let failure = device
        .client()
        .search_recordings_with_report(URL, Options::default().with_limits(1, 1, 4096))
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::ItemLimit);
    assert!(failure.report().recordings().is_empty());
    assert_eq!(failure.report().retained_bytes(), 0);
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test(start_paused = true)]
async fn retained_byte_limit_accepts_exact_boundary_and_rejects_one_byte_over() {
    // Explicit expected decoded payload: 4 + 6 + 20 + 20 + 6 + 9 UTF-8 bytes.
    let exact = std::mem::size_of::<oxvif::RecordingInformation>() + 65;
    for limit in [exact, exact - 1] {
        let device = Device::new(vec![
            (FIND, found("bytes")),
            (POLL, page("Completed", FIRST)),
            (END, released()),
        ]);
        let result = device
            .client()
            .search_recordings_with_report(URL, Options::default().with_limits(1, 1, limit))
            .await;
        if limit == exact {
            let report = result.unwrap();
            assert_eq!(report.retained_bytes(), exact);
            assert_eq!(report.recordings().len(), 1);
        } else {
            let failure = result.unwrap_err();
            assert_eq!(failure.report().stop_reason(), Stop::ByteLimit);
            assert!(failure.report().recordings().is_empty());
            assert_eq!(failure.report().retained_bytes(), 0);
        }
        device.assert_actions(&[FIND, POLL, END]);
    }
    // Each page fits separately; their sum is one byte over. This must not
    // devolve into a per-response limit and accept the second page.
    let second_bytes = std::mem::size_of::<oxvif::RecordingInformation>() + 25;
    let device = Device::new(vec![
        (FIND, found("accumulated-bytes")),
        (POLL, page("Searching", FIRST)),
        (POLL, page("Completed", SECOND)),
        (END, released()),
    ]);
    let failure = device
        .client()
        .search_recordings_with_report(
            URL,
            Options::default().with_limits(2, 2, exact + second_bytes - 1),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::ByteLimit);
    assert_eq!(failure.report().recordings().len(), 1);
    assert_eq!(failure.report().recordings()[0].source_name, "門口");
    assert_eq!(failure.report().retained_bytes(), exact);
    assert_eq!(failure.report().pages(), 2);
    assert_released(failure.report().cleanup());
    device.assert_actions(&[FIND, POLL, POLL, END]);
}

#[tokio::test(start_paused = true)]
async fn find_and_poll_deadlines_are_distinct_from_cleanup_timeout() {
    let device = Device::new(vec![(FIND, Reply::Pending)]);
    let start = tokio::time::Instant::now();
    let failure = device
        .client()
        .search_recordings_with_report(
            URL,
            Options::default().with_total_timeout(Duration::from_secs(3)),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::Deadline);
    assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
    assert_eq!(start.elapsed(), Duration::from_secs(3));
    device.assert_actions(&[FIND]);
    let device = Device::new(vec![
        (FIND, found("deadline")),
        (POLL, Reply::Pending),
        (END, Reply::Pending),
    ]);
    let start = tokio::time::Instant::now();
    let failure = device
        .client()
        .search_recordings_with_report(
            URL,
            Options::default()
                .with_total_timeout(Duration::from_secs(3))
                .with_cleanup_timeout(Duration::from_secs(2)),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::Deadline);
    assert!(matches!(failure.report().cleanup(), Cleanup::TimedOut));
    assert_eq!(start.elapsed(), Duration::from_secs(5));
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test]
async fn cooperative_cancel_before_find_and_while_find_has_no_known_token() {
    let device = Device::new(vec![]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    owner.cancellation().cancel();
    let failure = owner.run().await.unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::Cancelled);
    assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
    device.assert_actions(&[]);
    let device = Device::new(vec![(FIND, Reply::Pending)]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    let cancellation = owner.cancellation();
    let (result, ()) = tokio::join!(owner.run(), async {
        device.until_calls(1).await;
        cancellation.cancel();
    });
    let failure = result.unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::Cancelled);
    assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
    device.assert_actions(&[FIND]);
}

#[tokio::test(start_paused = true)]
async fn cooperative_cancel_after_token_runs_cleanup_once_and_owner_cannot_restart() {
    let device = Device::new(vec![
        (FIND, found("cancel")),
        (POLL, page("Searching", FIRST)),
        (POLL, Reply::Pending),
        (END, released()),
    ]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    let cancellation = owner.cancellation();
    let (result, ()) = tokio::join!(owner.run(), async {
        device.until_calls(3).await;
        cancellation.cancel();
    });
    let failure = result.unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::Cancelled);
    assert_eq!(failure.report().recordings().len(), 1);
    assert_eq!(failure.report().recordings()[0].content, "motion");
    assert_eq!(failure.report().pages(), 1);
    assert_released(failure.report().cleanup());
    assert_released(&*owner.cancel_and_close().await);
    assert_released(&*owner.close().await);
    let failure = owner.run().await.unwrap_err();
    assert_eq!(failure.report().stop_reason(), Stop::AlreadyUsed);
    assert_eq!(failure.report().pages(), 0);
    assert_eq!(failure.report().retained_bytes(), 0);
    device.assert_actions(&[FIND, POLL, POLL, END]);
}

#[tokio::test]
async fn externally_dropped_run_keeps_owner_available_for_explicit_close() {
    let device = Device::new(vec![
        (FIND, found("suspend")),
        (POLL, Reply::Pending),
        (END, released()),
    ]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    {
        let run = owner.run();
        tokio::pin!(run);
        tokio::select! { biased; result = &mut run => panic!("unexpected completion: {result:?}"), () = device.until_calls(2) => {} }
    }
    assert_released(&*owner.cancel_and_close().await);
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test]
async fn externally_dropped_cleanup_caches_interrupted_without_retry() {
    let device = Device::new(vec![
        (FIND, found("interrupt")),
        (POLL, page("Completed", "")),
        (END, Reply::Pending),
    ]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    {
        let run = owner.run();
        tokio::pin!(run);
        tokio::select! { biased; result = &mut run => panic!("unexpected completion: {result:?}"), () = device.until_calls(3) => {} }
    }
    let first = owner.close().await;
    let second = owner.close().await;
    assert!(matches!(&*first, Cleanup::Interrupted));
    assert!(Arc::ptr_eq(&first, &second));
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test]
async fn owner_drop_cancels_locally_without_spawning_remote_cleanup() {
    let device = Device::new(vec![(FIND, found("drop")), (POLL, Reply::Pending)]);
    let mut owner = device
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    let cancellation = owner.cancellation();
    {
        let run = owner.run();
        tokio::pin!(run);
        tokio::select! { biased; result = &mut run => panic!("unexpected completion: {result:?}"), () = device.until_calls(2) => {} }
    }
    assert!(!cancellation.is_cancelled());
    drop(owner);
    tokio::task::yield_now().await;
    assert!(cancellation.is_cancelled());
    device.assert_actions(&[FIND, POLL]);
}

#[tokio::test]
async fn completed_results_keep_cleanup_failure_independent() {
    for reply in [
        Reply::Status(500),
        Reply::Xml(envelope("<tse:EndSearchResponse/>")),
    ] {
        let device = Device::new(vec![
            (FIND, found("complete")),
            (POLL, page("Completed", FIRST)),
            (END, reply),
        ]);
        let report = device
            .client()
            .search_recordings_with_report(URL, Options::default())
            .await
            .unwrap();
        assert_eq!(report.stop_reason(), Stop::Completed);
        assert_eq!(report.recordings().len(), 1);
        match report.cleanup() {
            Cleanup::Failed(error) => assert_status(error, 500),
            Cleanup::Released { endpoint } => assert_eq!(endpoint, &None),
            other => panic!("{other:?}"),
        }
        device.assert_actions(&[FIND, POLL, END]);
    }
}

#[tokio::test]
async fn invalid_options_fail_before_io_with_explicit_local_cause() {
    let device = Device::new(vec![]);
    for options in [
        Options::default().with_limits(0, 1, 1),
        Options::default().with_limits(1, 0, 1),
        Options::default().with_limits(1, 1, 0),
        Options::default().with_total_timeout(Duration::ZERO),
        Options::default().with_total_timeout(Duration::MAX),
        Options::default().with_cleanup_timeout(Duration::ZERO),
        Options::default().with_cleanup_timeout(Duration::MAX),
        Options::default().with_min_poll_interval(Duration::ZERO),
        Options::default().with_min_poll_interval(Duration::MAX),
        Options::default().with_keep_alive(Duration::ZERO),
        Options::default().with_page_size(0),
        Options::default().with_page_size(u32::MAX),
        Options::default().with_max_matches(Some(0)),
        Options::default().with_max_matches(Some(u32::MAX)),
    ] {
        let failure = device
            .client()
            .search_recordings_with_report(URL, options)
            .await
            .unwrap_err();
        assert_eq!(failure.report().stop_reason(), Stop::InvalidOptions);
        assert!(
            matches!(failure.cause(), Some(OnvifError::InvalidArgument(message)) if message == "Invalid recording search budgets or xs:int result limits")
        );
        assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
    }
    device.assert_actions(&[]);
}

#[tokio::test(start_paused = true)]
async fn legacy_helper_preserves_primary_error_and_rejects_incomplete_results() {
    let device = Device::new(vec![
        (FIND, found("legacy")),
        (POLL, Reply::Status(503)),
        (END, Reply::Status(401)),
    ]);
    let error = device
        .client()
        .search_recordings(URL, None)
        .await
        .unwrap_err();
    assert_status(&error, 503);
    device.assert_actions(&[FIND, POLL, END]);
    let device = Device::new(vec![
        (FIND, found("legacy")),
        (POLL, page("Unknown", FIRST)),
        (END, released()),
    ]);
    assert!(matches!(
        device.client().search_recordings(URL, None).await,
        Err(OnvifError::RecordingSearchIncomplete {
            reason: Stop::UnknownState
        })
    ));
    device.assert_actions(&[FIND, POLL, END]);
    let mut steps = vec![(FIND, found("legacy-pages"))];
    steps.extend((0..20).map(|_| (POLL, page("Searching", ""))));
    steps.push((END, released()));
    let device = Device::new(steps);
    assert!(matches!(
        device.client().search_recordings(URL, None).await,
        Err(OnvifError::RecordingSearchIncomplete {
            reason: Stop::PageLimit
        })
    ));
    assert_eq!(device.calls().len(), 22);
    assert_eq!(device.calls().last().unwrap().action, END);
    assert!(device.steps.lock().unwrap().is_empty());
}

#[tokio::test]
async fn two_owners_do_not_exchange_tokens_or_cleanup_outcomes() {
    let first = Device::new(vec![
        (FIND, found("independent-A")),
        (POLL, Reply::Pending),
        (END, released()),
    ]);
    let second = Device::new(vec![
        (FIND, found("independent-B")),
        (POLL, page("Completed", SECOND)),
        (END, Reply::Status(403)),
    ]);
    let mut owner = first
        .client()
        .recording_search(URL, Options::default())
        .unwrap();
    let cancellation = owner.cancellation();
    let (a, b, ()) = tokio::join!(
        owner.run(),
        async {
            second
                .client()
                .search_recordings_with_report(URL, Options::default())
                .await
        },
        async {
            first.until_calls(2).await;
            second.until_calls(3).await;
            cancellation.cancel();
        }
    );
    let a = a.unwrap_err();
    assert_eq!(a.report().stop_reason(), Stop::Cancelled);
    assert_released(a.report().cleanup());
    let b = b.unwrap();
    assert_eq!(b.recordings()[0].source_name, "rear");
    match b.cleanup() {
        Cleanup::Failed(error) => assert_status(error, 403),
        other => panic!("{other:?}"),
    }
    for (device, own, other) in [
        (&first, "independent-A", "independent-B"),
        (&second, "independent-B", "independent-A"),
    ] {
        for call in &device.calls()[1..] {
            assert!(call.body.contains(own));
            assert!(!call.body.contains(other));
        }
        device.assert_actions(&[FIND, POLL, END]);
    }
}

#[tokio::test]
async fn actual_http_action_specific_replies_preserve_soap_fault_and_cleanup() {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    let actions = Arc::new(Mutex::new(Vec::new()));
    for status in [StatusCode::BAD_REQUEST, StatusCode::INTERNAL_SERVER_ERROR] {
        actions.lock().unwrap().clear();
        let router = Router::new().route("/search", post(move |State(actions): State<Arc<Mutex<Vec<String>>>>, headers: HeaderMap, body: String| async move {
            let content_type = headers["content-type"].to_str().unwrap();
            let action = [FIND, POLL, END].into_iter().find(|action| content_type.contains(action)).unwrap();
            actions.lock().unwrap().push(action.to_owned());
            let (code, response) = match action {
                FIND => (StatusCode::OK, envelope("<tse:FindRecordingsResponse><tse:SearchToken>wire-token</tse:SearchToken></tse:FindRecordingsResponse>")),
                POLL => (status, envelope("<s:Fault><s:Code><s:Value>s:Receiver</s:Value><s:Subcode><s:Value>ter:TestFailure</s:Value></s:Subcode></s:Code><s:Reason><s:Text xml:lang=\"en\">synthetic primary fault</s:Text></s:Reason></s:Fault>")),
                END => { assert!(body.contains("<tse:SearchToken>wire-token</tse:SearchToken>")); (StatusCode::OK, envelope("<tse:EndSearchResponse><tse:Endpoint>2026-11-11T10:20:30Z</tse:Endpoint></tse:EndSearchResponse>")) },
                _ => unreachable!(),
            };
            (code, [("content-type", "application/soap+xml")], response)
        })).with_state(actions.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/search", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let failure = OnvifClient::new(&url)
            .search_recordings_with_report(&url, Options::default())
            .await
            .unwrap_err();
        assert_eq!(failure.report().stop_reason(), Stop::PollFailed);
        assert!(
            matches!(failure.cause(), Some(OnvifError::Soap(SoapError::Fault { reason, subcode: Some(subcode), .. })) if reason == "synthetic primary fault" && subcode == "ter:TestFailure")
        );
        assert_released(failure.report().cleanup());
        assert_eq!(*actions.lock().unwrap(), [FIND, POLL, END]);
        server.abort();
        let _ = server.await;
    }
}

#[tokio::test]
async fn cleanup_fault_keeps_original_fields_without_overriding_completion() {
    let fault = envelope(
        "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><s:Reason><s:Text xml:lang=\"en\">cleanup-refused-883</s:Text></s:Reason><s:Detail>cleanup-evidence-991</s:Detail></s:Fault>",
    );
    let device = Device::new(vec![
        (FIND, found("cleanup-fault")),
        (POLL, page("Completed", SECOND)),
        (END, Reply::Xml(fault)),
    ]);
    let report = device
        .client()
        .search_recordings_with_report(URL, Options::default())
        .await
        .unwrap();
    assert_eq!(report.stop_reason(), Stop::Completed);
    assert_eq!(report.recordings()[0].content, "continuous");
    assert!(
        matches!(report.cleanup(), Cleanup::Failed(OnvifError::Soap(SoapError::Fault { code, reason, detail: Some(detail), subcode: None })) if code == "s:Sender" && reason == "cleanup-refused-883" && detail == "cleanup-evidence-991")
    );
    device.assert_actions(&[FIND, POLL, END]);
}

#[tokio::test]
async fn session_owners_use_resolved_search_url_and_missing_service_preserves_cause() {
    const CAPS: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
    const SERVICES: &str = "http://www.onvif.org/ver10/device/wsdl/GetServices";
    for present in [true, false] {
        let search = if present {
            format!("<tt:Search><tt:XAddr>{URL}</tt:XAddr></tt:Search>")
        } else {
            String::new()
        };
        let caps = envelope(&format!(
            "<d:GetCapabilitiesResponse xmlns:d=\"http://www.onvif.org/ver10/device/wsdl\"><d:Capabilities><tt:Extension><tt:Recording><tt:XAddr>http://camera.test/recording</tt:XAddr></tt:Recording>{search}<tt:Replay><tt:XAddr>http://camera.test/replay</tt:XAddr></tt:Replay><tt:Media2><tt:XAddr>http://camera.test/media2</tt:XAddr></tt:Media2><tt:DeviceIO><tt:XAddr>http://camera.test/io</tt:XAddr></tt:DeviceIO></tt:Extension></d:Capabilities></d:GetCapabilitiesResponse>"
        ));
        let mut steps = vec![(CAPS, Reply::Xml(caps))];
        if present {
            for token in ["session-owner", "session-report"] {
                steps.extend([
                    (FIND, found(token)),
                    (POLL, page("Completed", SECOND)),
                    (END, released()),
                ]);
            }
        } else {
            steps.push((
                SERVICES,
                Reply::Xml(envelope(
                    "<d:GetServicesResponse xmlns:d=\"http://www.onvif.org/ver10/device/wsdl\"/>",
                )),
            ));
        }
        let device = Device::new(steps);
        let session = oxvif::OnvifSession::builder("http://camera.test/device")
            .with_transport(device.clone())
            .build()
            .await
            .unwrap();
        let before = device.calls().len();
        if present {
            let mut owner = session.recording_search(Options::default()).unwrap();
            assert_eq!(
                device.calls().len(),
                before,
                "constructing an owner must not do I/O"
            );
            let report = owner.run().await.unwrap();
            assert_eq!(report.recordings()[0].source_name, "rear");
            assert_released(report.cleanup());
            let report = session
                .search_recordings_with_report(Options::default())
                .await
                .unwrap();
            assert_eq!(report.recordings()[0].content, "continuous");
            assert_released(report.cleanup());
            assert_eq!(
                device.calls()[before..]
                    .iter()
                    .map(|call| call.action.as_str())
                    .collect::<Vec<_>>(),
                [FIND, POLL, END, FIND, POLL, END]
            );
            assert!(device.calls()[before..].iter().all(|call| call.url == URL));
        } else {
            assert!(matches!(
                session.recording_search(Options::default()),
                Err(OnvifError::Soap(SoapError::MissingField(
                    "Search service URL"
                )))
            ));
            let failure = session
                .search_recordings_with_report(Options::default())
                .await
                .unwrap_err();
            assert_eq!(failure.report().stop_reason(), Stop::FindFailed);
            assert!(matches!(
                failure.cause(),
                Some(OnvifError::Soap(SoapError::MissingField(
                    "Search service URL"
                )))
            ));
            assert!(matches!(failure.report().cleanup(), Cleanup::NoKnownToken));
            assert_eq!(
                device.calls().len(),
                before,
                "missing service must not issue RPCs"
            );
        }
        assert!(device.steps.lock().unwrap().is_empty());
    }
}

#[cfg(feature = "health")]
#[test]
fn incomplete_workflow_health_classification_is_not_a_fabricated_fault() {
    let error = OnvifError::RecordingSearchIncomplete {
        reason: Stop::PageLimit,
    };
    let report = oxvif::health::CheckError::from(&error);
    assert_eq!(report.class, oxvif::health::ErrorClass::Precondition);
    assert_eq!(report.fault_code, None);
    assert_eq!(report.subcode, None);
    assert_eq!(report.detail, None);
    assert_eq!(report.reason, "Recording search incomplete: page limit");
}
