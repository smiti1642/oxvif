//! K27 collision-safe storage and request-aware replay, not conformance acceptance.
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
    store.record(ACTION, &incoming, FIRST);
    assert_eq!(
        store.len(),
        1,
        "qualified ephemera updates the same recording"
    );
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
async fn legacy_key_collisions_preserve_both_recordings_and_replay_identity() {
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
            "unqualified header is not SOAP ephemera",
            "<Envelope><Header><MessageID>first</MessageID></Header><Body><GetResource/></Body></Envelope>",
            "<Envelope><Header><MessageID>second</MessageID></Header><Body><GetResource/></Body></Envelope>",
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
        assert_eq!(store.len(), 2, "distinct identities must coexist: {case}");
        assert_eq!(
            store.fixtures()[1].key_canon,
            first_key,
            "legacy collision: {case}"
        );
        assert_eq!(store.fixtures()[0].request_raw, first);
        assert_eq!(store.fixtures()[1].request_raw, second);
        let details = store.diff_details();
        assert_eq!(details.len(), 2, "report retains every fixture: {case}");
        assert!(details[0].clone_xml.contains("first-recording-927"));
        assert!(details[1].clone_xml.contains("second-recording-928"));
        let progress = std::sync::Mutex::new(Vec::new());
        let report = store.diff_against_synthetic_with_progress(|p| {
            progress.lock().unwrap().push(p);
        });
        assert_eq!(report.compared, 2);
        assert_eq!(report.quirks.len(), 2);
        assert!(
            report.quirks[0]
                .only_in_clone
                .iter()
                .any(|p| p.contains("first-recording-927"))
        );
        assert!(
            report.quirks[1]
                .only_in_clone
                .iter()
                .any(|p| p.contains("second-recording-928"))
        );
        let progress = progress.into_inner().unwrap();
        assert_eq!(
            progress
                .iter()
                .map(|p| (p.done, p.total))
                .collect::<Vec<_>>(),
            vec![(1, 2), (2, 2)]
        );
        assert!(
            store.lookup(ACTION, &first_key).is_none(),
            "ambiguous key-only lookup: {case}"
        );
        assert_eq!(
            store.lookup_request(ACTION, first).unwrap().response_raw,
            FIRST
        );
        assert_eq!(
            store.lookup_request(ACTION, second).unwrap().response_raw,
            SECOND
        );
        assert!(
            store
                .lookup_request("urn:wrong/GetResource", first)
                .is_none()
        );
        assert!(
            store
                .lookup_request(ACTION, "<GetResource missing='true'/>")
                .is_none()
        );

        let dir = std::env::temp_dir().join(format!(
            "oxvif-k27-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&dir).unwrap();
        store.save(&dir).unwrap();
        let path = dir.join("fixtures.json");
        let bytes = std::fs::read(&path).unwrap();
        let mut loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "load never rewrites");
        assert_eq!(loaded.len(), 2, "load retains collision: {case}");
        assert!(loaded.lookup(ACTION, &first_key).is_none());
        loaded.record(ACTION, first, "<updated-first-930/>");
        assert_eq!(loaded.len(), 2, "same request replaces only itself: {case}");
        assert_eq!(
            loaded.lookup_request(ACTION, first).unwrap().response_raw,
            "<updated-first-930/>"
        );
        assert_eq!(
            loaded.lookup_request(ACTION, second).unwrap().response_raw,
            SECOND
        );
        loaded.save(&dir).unwrap();
        let loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.lookup_request(ACTION, first).unwrap().response_raw,
            "<updated-first-930/>"
        );
        assert_eq!(
            loaded.lookup_request(ACTION, second).unwrap().response_raw,
            SECOND
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
        let replay = MetamorphTransport::new(store.clone());
        assert_eq!(
            replay
                .soap_post("http://mock", ACTION, first.into())
                .await
                .unwrap(),
            FIRST,
            "the first colliding recording is retained: {case}"
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
                FIRST,
                "HTTP first identity: {case}"
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
