#![cfg(feature = "metamorph")]
use oxvif::{
    metamorph::*,
    transport::{Transport, TransportError},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const HOST: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxvif-corpus-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for name in ["fixtures.json", "manifest.json"] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn identity() -> CorpusIdentity {
    CorpusIdentity {
        pseudonym: "synthetic-two-source".into(),
        vendor: Some("independently-authored mock".into()),
        model: None,
        firmware: None,
        recording_commit: "08ea4b4+local-p5".into(),
        provenance: "synthetic; not device or schema evidence".into(),
        sharing_scope: "reviewed synthetic test only".into(),
    }
}

fn hostname(name: &str) -> String {
    format!(
        "<Envelope><Body><GetHostnameResponse><HostnameInformation><FromDHCP>false</FromDHCP><Name>{name}</Name></HostnameInformation></GetHostnameResponse></Body></Envelope>"
    )
}

fn fixture(action: &str, response: &str) -> Fixture {
    Fixture {
        action: action.into(),
        key_canon: "legacy bucket".into(),
        request_raw: "<GetHostname/>".into(),
        response_raw: response.into(),
    }
}

fn manifest(corpus: &FixtureCorpus) -> CorpusManifest {
    CorpusManifest {
        version: 1,
        identity: identity(),
        selected_actions: corpus
            .fixtures
            .iter()
            .map(|row| row.action.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        sweep: vec![SweepObservation {
            operation: SurfaceOp::GetHostname,
            outcome: OpOutcome::Recorded,
        }],
        transport: RecordingEvidence::default(),
        exchanges: corpus
            .fixtures
            .iter()
            .enumerate()
            .map(|(ordinal, row)| ExpectedExchange {
                ordinal,
                action: row.action.clone(),
                request_raw: row.request_raw.clone(),
                fields: BTreeMap::from([("/name".into(), json!("expected-host"))]),
            })
            .collect(),
    }
}

#[test]
fn bounded_loader_preserves_duplicates_and_enforces_exact_file_row_and_xml_limits() {
    let directory = Directory::new();
    let corpus = FixtureCorpus {
        device: identity().pseudonym,
        fixtures: vec![
            fixture(HOST, &hostname("first")),
            fixture(HOST, &hostname("second")),
        ],
    };
    let bytes = serde_json::to_vec(&corpus).unwrap();
    let path = directory.0.join("fixtures.json");
    std::fs::write(&path, &bytes).unwrap();
    let limits = CorpusLimits {
        file_bytes: bytes.len(),
        fixtures: 2,
        xml_bytes: corpus.fixtures[1].response_raw.len(),
    };
    let loaded = FixtureCorpus::load(&directory.0, limits).unwrap();
    assert_eq!(loaded.fixtures.len(), 2);
    assert_eq!(loaded.fixtures[0].response_raw, hostname("first"));
    assert_eq!(loaded.fixtures[1].response_raw, hostname("second"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(matches!(
        FixtureCorpus::load(
            &directory.0,
            CorpusLimits {
                file_bytes: bytes.len() - 1,
                ..limits
            }
        ),
        Err(CorpusError::Limit {
            resource: "file bytes",
            ..
        })
    ));
    assert!(matches!(
        FixtureCorpus::load(
            &directory.0,
            CorpusLimits {
                fixtures: 1,
                ..limits
            }
        ),
        Err(CorpusError::Limit {
            resource: "fixtures",
            ..
        })
    ));
    assert!(matches!(
        FixtureCorpus::load(
            &directory.0,
            CorpusLimits {
                xml_bytes: limits.xml_bytes - 1,
                ..limits
            }
        ),
        Err(CorpusError::Limit {
            resource: "XML bytes",
            ..
        })
    ));
    assert!(matches!(
        FixtureCorpus::load(
            &directory.0,
            CorpusLimits {
                file_bytes: 0,
                ..limits
            }
        ),
        Err(CorpusError::InvalidLimit)
    ));
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        FixtureCorpus::load(&directory.0, limits),
        Err(CorpusError::Io(_))
    ));
    assert!(matches!(
        CorpusManifest::load(directory.0.join("manifest.json"), 1024),
        Err(CorpusError::Io(_))
    ));
}

#[tokio::test]
async fn regression_distinguishes_refusal_unverified_parse_failure_and_missing_evidence() {
    let fault = "<Envelope><Body><Fault><Code><Value>s:Sender</Value></Code><Reason><Text>private fault password=secret</Text></Reason></Fault></Body></Envelope>";
    let corpus = FixtureCorpus {
        device: identity().pseudonym,
        fixtures: vec![
            fixture(HOST, &hostname("expected-host")),
            fixture(HOST, fault),
            fixture(
                "urn:unknown/GetHostname",
                "<Envelope><Body><Unknown/></Body></Envelope>",
            ),
            fixture(HOST, "<Envelope>"),
        ],
    };
    let mut expected = manifest(&corpus);
    let report = verify_corpus(&corpus, &expected).await.unwrap();
    assert_eq!(
        report
            .rows
            .iter()
            .map(|row| row.outcome)
            .collect::<Vec<_>>(),
        [
            RegressionOutcome::Passed,
            RegressionOutcome::Faulted,
            RegressionOutcome::Unverified,
            RegressionOutcome::Failed
        ]
    );
    assert!(!report.passed());
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("private fault")
    );
    expected.exchanges[0].fields.clear();
    assert_eq!(
        verify_corpus(&corpus, &expected).await.unwrap().rows[0].outcome,
        RegressionOutcome::NoAssertions
    );
    expected.exchanges[0].request_raw.push(' ');
    assert_eq!(
        verify_corpus(&corpus, &expected).await.unwrap().rows[0].outcome,
        RegressionOutcome::IdentityMismatch
    );
    expected.exchanges.pop();
    assert!(
        verify_corpus(&corpus, &expected)
            .await
            .unwrap()
            .coverage_errors
            .contains(&"fixture_coverage".into())
    );

    let single = FixtureCorpus {
        device: identity().pseudonym,
        fixtures: vec![fixture(HOST, &hostname("expected-host"))],
    };
    let mut expected = manifest(&single);
    assert!(verify_corpus(&single, &expected).await.unwrap().passed());
    expected.transport.unavailable = true;
    assert!(!verify_corpus(&single, &expected).await.unwrap().passed());
    expected.transport.unavailable = false;
    expected.transport.omitted = 1;
    assert!(!verify_corpus(&single, &expected).await.unwrap().passed());
    expected.transport.omitted = 0;
    expected.sweep[0].outcome = OpOutcome::Failed;
    assert!(!verify_corpus(&single, &expected).await.unwrap().passed());
    expected.sweep[0].outcome = OpOutcome::Recorded;
    expected.exchanges[0]
        .fields
        .insert("/name".into(), json!("wrong-host"));
    let report = verify_corpus(&single, &expected).await.unwrap();
    assert_eq!(report.rows[0].outcome, RegressionOutcome::FieldMismatch);
    assert_eq!(report.rows[0].differing_fields, ["/name"]);
}

struct Refused;
#[async_trait::async_trait]
impl Transport for Refused {
    async fn soap_post(&self, _: &str, _: &str, _: String) -> Result<String, TransportError> {
        Err(TransportError::HttpStatus {
            status: 403,
            body: "private-http-body-password=secret".into(),
        })
    }
}

#[tokio::test]
async fn recording_transport_failures_have_separate_bounded_identity_and_controlled_causes() {
    let store = Arc::new(Mutex::new(FixtureStore::new("synthetic-two-source")));
    let disabled = RecordingTransport::new(Arc::new(Refused), store.clone());
    assert!(disabled.take_evidence().unavailable);
    let tap = RecordingTransport::new(Arc::new(Refused), store.clone()).with_evidence();
    let body = "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:wsse='http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd'><s:Header><wsse:Security><wsse:Password>private-password</wsse:Password><wsse:Nonce>private-nonce</wsse:Nonce></wsse:Security></s:Header><s:Body><Read><VideoSourceToken>VS_2</VideoSourceToken></Read></s:Body></s:Envelope>";
    assert!(
        tap.soap_post("http://private-target", HOST, body.into())
            .await
            .is_err()
    );
    let evidence = tap.take_evidence();
    assert_eq!(evidence.failures.len(), 1);
    assert_eq!(evidence.failures[0].http_status, Some(403));
    assert_eq!(
        evidence.failures[0].error_class,
        oxvif::session::SessionErrorClass::Authentication
    );
    assert!(evidence.failures[0].request_raw.contains("VS_2"));
    let rendered = serde_json::to_string(&evidence).unwrap();
    for secret in [
        "private-password",
        "private-nonce",
        "private-target",
        "private-http-body",
    ] {
        assert!(!rendered.contains(secret));
    }
    let corpus = FixtureCorpus {
        device: identity().pseudonym,
        fixtures: vec![fixture(HOST, &hostname("expected-host"))],
    };
    let mut expected = manifest(&corpus);
    expected.transport = evidence.clone();
    let report = verify_corpus(&corpus, &expected).await.unwrap();
    assert_eq!(report.transport_failed, 1);
    assert!(!report.passed());
    assert_eq!(store.lock().unwrap().len(), 0);
    assert!(tap.take_evidence().failures.is_empty());
    let exact = "x".repeat(4 * 1024 * 1024 - HOST.len());
    assert!(
        tap.soap_post("http://private-target", HOST, exact)
            .await
            .is_err()
    );
    assert!(
        tap.soap_post("http://private-target", "x", String::new())
            .await
            .is_err()
    );
    let evidence = tap.take_evidence();
    assert_eq!(evidence.failures.len(), 1);
    assert_eq!(evidence.omitted, 1);
    for _ in 0..513 {
        assert!(
            tap.soap_post("http://private-target", HOST, "x".into())
                .await
                .is_err()
        );
    }
    let evidence = tap.take_evidence();
    assert_eq!(evidence.failures.len(), 512);
    assert_eq!(evidence.omitted, 1);
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn selected_recording_and_manifest_preserve_all_profiles_and_different_source_values() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    let selection = SurfaceSelection::none()
        .with(SurfaceOp::GetProfile)
        .with(SurfaceOp::GetImagingSettings);
    let progress = Arc::new(Mutex::new(Vec::new()));
    let seen = progress.clone();
    let (store, sweep, evidence) = record_surface_with_evidence(
        server.device_url(),
        None,
        identity().pseudonym,
        &selection,
        move |event| seen.lock().unwrap().push(event),
    )
    .await
    .unwrap();
    assert!(sweep.is_complete());
    assert!(evidence.failures.is_empty() && !evidence.unavailable && evidence.omitted == 0);
    assert_eq!(
        progress
            .lock()
            .unwrap()
            .last()
            .map(|event| (event.done, event.total)),
        Some((4, 4))
    );
    let corpus = FixtureCorpus {
        device: identity().pseudonym,
        fixtures: store.fixtures().to_vec(),
    };
    let mut expected = manifest(&corpus);
    expected.sweep = sweep
        .entries()
        .into_iter()
        .map(|(operation, outcome)| SweepObservation { operation, outcome })
        .collect();
    expected.transport = evidence;
    let mut profile_tokens = Vec::new();
    let mut sources = BTreeMap::new();
    for row in &mut expected.exchanges {
        let body = oxvif::soap::parse_soap_body(&row.request_raw).unwrap();
        let operation = body.children.first().unwrap();
        row.fields = match row.action.rsplit('/').next().unwrap() {
            "GetCapabilities" => BTreeMap::from([(
                "/media/url".into(),
                json!(format!("{}/onvif/media", server.base_url())),
            )]),
            "GetServices" => BTreeMap::from([(
                "/0/namespace".into(),
                json!("http://www.onvif.org/ver10/device/wsdl"),
            )]),
            "GetProfiles" => BTreeMap::from([
                ("/0/token".into(), json!("Profile_1")),
                ("/2/video_source_token".into(), json!("VS_2")),
            ]),
            "GetProfile" => {
                let token = operation.child("ProfileToken").unwrap().text();
                profile_tokens.push(token.to_owned());
                let name = match token {
                    "Profile_1" => "mainStream",
                    "Profile_2" => "subStream",
                    "Profile_3" => "mainStream2",
                    "Profile_4" => "subStream2",
                    _ => panic!("unexpected profile"),
                };
                BTreeMap::from([
                    ("/token".into(), json!(token)),
                    ("/name".into(), json!(name)),
                ])
            }
            "GetVideoSources" => BTreeMap::from([
                ("/0/token".into(), json!("VS_1")),
                ("/1/token".into(), json!("VS_2")),
            ]),
            "GetImagingSettings" => {
                let token = operation.child("VideoSourceToken").unwrap().text();
                let brightness = match token {
                    "VS_1" => 60.0,
                    "VS_2" => 45.0,
                    _ => panic!("unexpected source"),
                };
                sources.insert(token.to_owned(), brightness);
                BTreeMap::from([("/brightness".into(), json!(brightness))])
            }
            _ => panic!("unexpected recorded Action"),
        };
    }
    assert_eq!(
        profile_tokens,
        ["Profile_1", "Profile_2", "Profile_3", "Profile_4"]
    );
    assert_eq!(
        sources,
        BTreeMap::from([("VS_1".to_owned(), 60.0), ("VS_2".to_owned(), 45.0)])
    );
    assert_eq!(corpus.fixtures.len(), 10);
    let directory = Directory::new();
    store.save(&directory.0).unwrap();
    std::fs::write(
        directory.0.join("manifest.json"),
        serde_json::to_vec_pretty(&expected).unwrap(),
    )
    .unwrap();
    let loaded = FixtureCorpus::load(&directory.0, CorpusLimits::default()).unwrap();
    let expected = CorpusManifest::load(directory.0.join("manifest.json"), 1024 * 1024).unwrap();
    let report = verify_corpus(&loaded, &expected).await.unwrap();
    assert!(report.passed(), "{report:?}");
    let mut wrong = expected.clone();
    let second = wrong
        .exchanges
        .iter_mut()
        .find(|row| row.request_raw.contains("VS_2") && row.action.ends_with("/GetImagingSettings"))
        .unwrap();
    second.fields.insert("/brightness".into(), json!(60.0));
    let report = verify_corpus(&loaded, &wrong).await.unwrap();
    assert!(!report.passed());
    assert_eq!(
        report
            .rows
            .iter()
            .filter(|row| row.outcome == RegressionOutcome::FieldMismatch)
            .count(),
        1
    );
}
