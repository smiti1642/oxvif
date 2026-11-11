#![cfg(feature = "mock")]

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use oxvif::{
    OnvifClient,
    mock::{AckOnlyOperation, MockState, MockTransport},
    soap::{SoapEnvelope, find_response, parse_soap_body},
    transport::Transport,
};
#[cfg(feature = "mock-server")]
use oxvif::{RecordingSearchCleanup, RecordingSearchOptions};

const MOCK: &str = "http://mock";
const NS: &str = "http://www.onvif.org/ver10/search/wsdl";

fn seed(state: &MockState) {
    state.modify(|device| {
        let record = device.recording.recordings[0].clone();
        device.recording.recordings = ["A<&", "B", "C"]
            .into_iter()
            .map(|token| {
                let mut record = record.clone();
                record.token = token.into();
                record
            })
            .collect();
    });
}

async fn post(transport: &dyn Transport, op: &str, fields: &str) -> String {
    let action = format!("{NS}/{op}");
    let xml = SoapEnvelope::new(format!("<q:{op} xmlns:q=\"{NS}\">{fields}</q:{op}>")).build();
    let response = transport
        .soap_post(MOCK, &action, xml.clone())
        .await
        .unwrap();
    if !response.contains("<s:Fault")
        && let Some(directory) = std::env::var_os("OXVIF_SEARCH_WIRE")
    {
        static ORDINAL: AtomicUsize = AtomicUsize::new(0);
        let ordinal = ORDINAL.fetch_add(1, Ordering::Relaxed);
        let directory = std::path::Path::new(&directory);
        std::fs::write(
            directory.join(format!("{ordinal:03}-{op}-request.xml")),
            xml,
        )
        .unwrap();
        std::fs::write(
            directory.join(format!("{ordinal:03}-{op}-response.xml")),
            &response,
        )
        .unwrap();
    }
    response
}

fn fault(xml: &str) {
    let body = parse_soap_body(xml).unwrap();
    assert!(matches!(
        find_response(&body, "missing"),
        Err(oxvif::soap::SoapError::Fault { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn cursors_are_isolated_and_expire_and_refusal_preserves_state() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    seed(&state);
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = MockTransport::with_state(state);
    let client = OnvifClient::new(MOCK).with_transport(Arc::new(transport.clone()));
    let first = client.find_recordings(MOCK, None, "PT2S").await.unwrap();
    let second = client
        .find_recordings(MOCK, Some(2), "PT10S")
        .await
        .unwrap();
    assert_ne!(first, second);
    let isolated = MockTransport::new();
    let other = OnvifClient::new(MOCK).with_transport(Arc::new(isolated));
    let other_token = other.find_recordings(MOCK, None, "PT10S").await.unwrap();
    assert_ne!(first, other_token);
    assert!(
        other
            .get_recording_search_results(MOCK, &first, 1, "PT0S")
            .await
            .is_err()
    );

    let before = serde_json::to_value(&*transport.device().read()).unwrap();
    fault(
        &post(
            &transport,
            "GetRecordingSearchResults",
            &format!("<q:SearchToken>{first}</q:SearchToken><q:MaxResults>0</q:MaxResults>"),
        )
        .await,
    );
    assert_eq!(
        serde_json::to_value(&*transport.device().read()).unwrap(),
        before
    );
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
    let page = client
        .get_recording_search_results(MOCK, &first, 1, "PT0S")
        .await
        .unwrap();
    assert_eq!(page.recording_information[0].recording_token, "A<&");
    assert_eq!(page.search_state, "Searching");

    transport
        .device()
        .modify(|device| device.recording.recordings.clear());
    let page = client
        .get_recording_search_results(MOCK, &second, 1, "PT0S")
        .await
        .unwrap();
    assert_eq!(page.recording_information[0].recording_token, "A<&");
    let page = client
        .get_recording_search_results(MOCK, &second, 1, "PT0S")
        .await
        .unwrap();
    assert_eq!(page.recording_information[0].recording_token, "B");
    assert_eq!(page.search_state, "Completed");
    let last = client
        .get_recording_search_results(MOCK, &second, 1, "PT0S")
        .await
        .unwrap();
    assert!(last.recording_information.is_empty());

    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(
        client
            .get_recording_search_results(MOCK, &first, 1, "PT0S")
            .await
            .is_err()
    );
    client.end_search(MOCK, &second).await.unwrap();
    assert!(client.end_search(MOCK, &second).await.is_err());
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn find_scope_and_admission_and_opt_in_receipt_are_bounded() {
    let transport = MockTransport::new();
    seed(transport.device());
    let client = OnvifClient::new(MOCK).with_transport(Arc::new(transport.clone()));
    let reply = post(&transport, "FindRecordings",
        "<q:Scope><tt:IncludedRecordings>B</tt:IncludedRecordings></q:Scope><q:KeepAliveTime>PT60S</q:KeepAliveTime>").await;
    let body = parse_soap_body(&reply).unwrap();
    let token = find_response(&body, "FindRecordingsResponse")
        .unwrap()
        .child("SearchToken")
        .unwrap()
        .text();
    let receipt = transport
        .clone()
        .with_acknowledgment_only(AckOnlyOperation::SearchEnd);
    let reply = post(
        &receipt,
        "EndSearch",
        &format!("<q:SearchToken>{token}</q:SearchToken>"),
    )
    .await;
    assert!(reply.contains("EndSearchResponse"));
    let state = post(
        &transport,
        "GetSearchState",
        &format!("<q:SearchToken>{token}</q:SearchToken>"),
    )
    .await;
    assert!(state.contains("<State>Searching</State>"));
    let page = client
        .get_recording_search_results(MOCK, token, 10, "PT0S")
        .await
        .unwrap();
    assert_eq!(page.recording_information.len(), 1);
    assert_eq!(page.recording_information[0].recording_token, "B");
    let raw_token = client
        .find_recordings(MOCK, Some(1), "PT60S")
        .await
        .unwrap();
    let raw_poll = post(
        &transport,
        "GetRecordingSearchResults",
        &format!("<q:SearchToken>{raw_token}</q:SearchToken><q:MaxResults>1</q:MaxResults>"),
    )
    .await;
    assert!(raw_poll.contains("<tt:RecordingToken>A&lt;&amp;</tt:RecordingToken>"));
    assert!(
        post(
            &transport,
            "EndSearch",
            &format!("<q:SearchToken>{raw_token}</q:SearchToken>")
        )
        .await
        .contains("EndSearchResponse")
    );
    for _ in 0..31 {
        client.find_recordings(MOCK, None, "PT60S").await.unwrap();
    }
    assert!(client.find_recordings(MOCK, None, "PT60S").await.is_err());
    client.end_search(MOCK, token).await.unwrap();
    let next = client.find_recordings(MOCK, None, "PT60S").await.unwrap();
    assert_ne!(next, token);
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn recording_search_owner_runs_over_mock_http() {
    let server = oxvif::mock::MockServer::builder().start().await.unwrap();
    seed(server.device());
    let session = oxvif::OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();
    let report = session
        .search_recordings_with_report(
            RecordingSearchOptions::default()
                .with_page_size(1)
                .with_min_poll_interval(Duration::from_millis(1)),
        )
        .await
        .unwrap();
    assert_eq!(report.pages(), 3);
    assert_eq!(
        report
            .recordings()
            .iter()
            .map(|item| item.recording_token.as_str())
            .collect::<Vec<_>>(),
        ["A<&", "B", "C"]
    );
    assert!(matches!(
        report.cleanup(),
        RecordingSearchCleanup::Released { .. }
    ));
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn replay_uses_live_search_state_instead_of_static_tokens() {
    let mut store = oxvif::metamorph::FixtureStore::new("stale");
    let find = format!("{NS}/FindRecordings");
    let request = SoapEnvelope::new(format!("<x:FindRecordings xmlns:x=\"{NS}\"><x:Scope/><x:KeepAliveTime>PT60S</x:KeepAliveTime></x:FindRecordings>")).build();
    let response = SoapEnvelope::new(format!("<x:FindRecordingsResponse xmlns:x=\"{NS}\"><x:SearchToken>recorded-token</x:SearchToken></x:FindRecordingsResponse>")).build();
    store.record(&find, &request, &response);
    let transport = oxvif::metamorph::MetamorphTransport::new(store);
    let client = OnvifClient::new(MOCK).with_transport(Arc::new(transport));
    let token = client.find_recordings(MOCK, None, "PT60S").await.unwrap();
    assert_ne!(token, "recorded-token");
    assert!(client.end_search(MOCK, "recorded-token").await.is_err());
    client.end_search(MOCK, &token).await.unwrap();
}
