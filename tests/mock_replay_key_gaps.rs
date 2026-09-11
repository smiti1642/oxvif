//! K27 collision containment and remaining storage gaps, not conformance acceptance.
//! Storage still collides; replay must fall through instead of substituting a
//! different request's response. Never restore substitution to keep tests green.
#![cfg(feature = "metamorph")]

use oxvif::{
    metamorph::{FixtureStore, MetamorphTransport},
    transport::Transport,
};

const ACTION: &str = "urn:project-key-controls/GetResource";
const FIRST: &str = "<first-recording-927/>";
const SECOND: &str = "<second-recording-928/>";

#[tokio::test]
async fn namespace_aliases_entities_and_real_header_ephemera_still_replay() {
    use oxvif::soap::{SoapEnvelope, WsSecurityToken};
    let recorded = SoapEnvelope::new("<a:GetResource xmlns:a='urn:resource'><a:ProfileToken>A&amp;B</a:ProfileToken></a:GetResource>".into())
        .with_wsa_to("http://recorded.invalid/onvif")
        .with_security(WsSecurityToken::from_parts("probe", "digest-a", "nonce-a", "2000-01-01T00:00:00Z"))
        .build();
    let incoming = SoapEnvelope::new("<b:GetResource xmlns:b='urn:resource'>\n <b:ProfileToken><![CDATA[A&B]]></b:ProfileToken>\n</b:GetResource>".into())
        .with_wsa_to("http://replayed.invalid/onvif")
        .with_security(WsSecurityToken::from_parts("probe", "digest-b", "nonce-b", "2001-01-01T00:00:00Z"))
        .build();
    let mut store = FixtureStore::new("synthetic-identity-equivalence");
    store.record(ACTION, &recorded, FIRST);
    assert_ne!(store.fixtures()[0].request_raw, incoming);
    let replay = MetamorphTransport::new(store.clone());
    assert_eq!(
        replay
            .soap_post("http://mock", ACTION, incoming.clone())
            .await
            .unwrap(),
        FIRST
    );
    #[cfg(feature = "metamorph-server")]
    {
        let server = oxvif::mock::MockServer::builder()
            .port(0)
            .replay(store)
            .start()
            .await
            .unwrap();
        let http = oxvif::transport::HttpTransport::new();
        assert_eq!(
            http.soap_post(server.device_url(), ACTION, incoming)
                .await
                .unwrap(),
            FIRST
        );
    }
}

#[tokio::test]
async fn legacy_key_collision_does_not_replay_a_different_request() {
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
        (
            "body field is not header ephemera",
            "<GetResource><Created>first</Created></GetResource>",
            "<GetResource><Created>second</Created></GetResource>",
        ),
        (
            "mixed content ordering",
            "<GetResource><A>x<B/>y</A></GetResource>",
            "<GetResource><A>xy<B/></A></GetResource>",
        ),
        (
            "QName-valued type binding",
            "<GetResource xmlns:x='urn:first' xmlns:i='http://www.w3.org/2001/XMLSchema-instance'><A i:type='x:Shape'/></GetResource>",
            "<GetResource xmlns:x='urn:second' xmlns:i='http://www.w3.org/2001/XMLSchema-instance'><A i:type='x:Shape'/></GetResource>",
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
        let fallback = oxvif::mock::MockTransport::new()
            .soap_post("http://mock", ACTION, first.into())
            .await
            .unwrap();
        assert_ne!(fallback, SECOND);
        let replay = MetamorphTransport::new(store.clone());
        assert_eq!(
            replay
                .soap_post("http://mock", ACTION, first.into())
                .await
                .unwrap(),
            fallback,
            "a colliding request must fall through, not replay another identity: {case}"
        );
        assert_eq!(
            replay
                .soap_post("http://mock", ACTION, second.into())
                .await
                .unwrap(),
            SECOND,
            "an exact raw recording remains replayable: {case}"
        );
        #[cfg(feature = "metamorph-server")]
        {
            let server = oxvif::mock::MockServer::builder()
                .port(0)
                .replay(store)
                .start()
                .await
                .unwrap();
            let http = oxvif::transport::HttpTransport::new();
            assert_eq!(
                http.soap_post(server.device_url(), ACTION, first.into())
                    .await
                    .unwrap(),
                fallback,
                "HTTP containment: {case}"
            );
            assert_eq!(
                http.soap_post(server.device_url(), ACTION, second.into())
                    .await
                    .unwrap(),
                SECOND,
                "HTTP exact replay: {case}"
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
