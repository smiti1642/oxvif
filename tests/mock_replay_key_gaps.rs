//! K27 executable known gaps, not correct-behavior or conformance acceptance.
//! Replace these expectations with distinct-key/response assertions when fixed;
//! never restore a collision to keep this source baseline green.
#![cfg(feature = "metamorph")]

use oxvif::{
    metamorph::{FixtureStore, MetamorphTransport},
    transport::Transport,
};

const ACTION: &str = "urn:project-key-controls/GetResource";
const FIRST: &str = "<first-recording-927/>";
const SECOND: &str = "<second-recording-928/>";

#[tokio::test]
async fn known_gap_distinct_requests_collide_within_one_action() {
    for (case, first, second) in [
        (
            "leading space",
            "<GetResource><ProfileToken> P</ProfileToken></GetResource>",
            "<GetResource><ProfileToken>P</ProfileToken></GetResource>",
        ),
        (
            "repeated space",
            "<GetResource><ProfileToken>A  B</ProfileToken></GetResource>",
            "<GetResource><ProfileToken>A B</ProfileToken></GetResource>",
        ),
        (
            "field namespace",
            "<GetResource><x:ProfileToken xmlns:x='urn:first'>P</x:ProfileToken></GetResource>",
            "<GetResource><x:ProfileToken xmlns:x='urn:second'>P</x:ProfileToken></GetResource>",
        ),
        (
            "text versus structure",
            "<GetResource><A>&lt;B&gt;&lt;/B&gt;</A></GetResource>",
            "<GetResource><A><B/></A></GetResource>",
        ),
        (
            "attribute boundary",
            "<GetResource a='x&#34; b=&#34;y'/>",
            "<GetResource a='x' b='y'/>",
        ),
        (
            "ignored trailing root",
            "<GetResource/>",
            "<GetResource/><Different/>",
        ),
    ] {
        assert_ne!(first, second, "the wire inputs must differ: {case}");
        let mut store = FixtureStore::new("synthetic-K27");
        store.record(ACTION, first, FIRST);
        let first_key = store.fixtures()[0].key_canon.clone();
        store.record(ACTION, second, SECOND);
        assert_eq!(
            store.len(),
            1,
            "K27 baseline moved: {case}; replace with corrected invariant"
        );
        assert_eq!(
            store.fixtures()[0].key_canon,
            first_key,
            "collision premise: {case}"
        );
        assert_eq!(store.fixtures()[0].request_raw, second);
        let replay = MetamorphTransport::new(store);
        for request in [first, second] {
            assert_eq!(
                replay
                    .soap_post("http://mock", ACTION, request.into())
                    .await
                    .unwrap(),
                SECOND,
                "K27 baseline moved: {case}; each request must eventually retain its own recording"
            );
        }
    }
}

#[tokio::test]
async fn ordinary_token_and_complete_action_controls_remain_distinct() {
    let first = "<GetResource><ProfileToken>P</ProfileToken></GetResource>";
    let second = "<GetResource><ProfileToken>Q</ProfileToken></GetResource>";
    let other_action = "urn:other-key-controls/GetResource";
    let mut store = FixtureStore::new("synthetic-K27-controls");
    store.record(ACTION, first, FIRST);
    store.record(ACTION, second, SECOND);
    store.record(other_action, first, "<other-service-929/>");
    assert_eq!(store.len(), 3);
    let replay = MetamorphTransport::new(store);
    for (action, request, expected) in [
        (ACTION, first, FIRST),
        (ACTION, second, SECOND),
        (other_action, first, "<other-service-929/>"),
    ] {
        assert_eq!(
            replay
                .soap_post("http://mock", action, request.into())
                .await
                .unwrap(),
            expected
        );
    }
}
