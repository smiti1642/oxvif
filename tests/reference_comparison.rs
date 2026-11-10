#![cfg(feature = "metamorph")]
use oxvif::metamorph::*;
use std::sync::{Arc, Mutex};

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const TT: &str = "http://www.onvif.org/ver10/schema";
const HOST: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
const INFO: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
const PROFILES1: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
const PROFILES2: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";
const ENCODERS1: &str = "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations";
const ENCODERS2: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";

fn envelope(service: &str, content: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='{SOAP}' xmlns:m='{service}' xmlns:t='{TT}'><s:Body>{content}</s:Body></s:Envelope>"
    )
}
fn request(action: &str, parameters: &str) -> String {
    let (service, operation) = action.rsplit_once('/').unwrap();
    envelope(
        service,
        &format!("<m:{operation}>{parameters}</m:{operation}>"),
    )
}
fn response_hostname(name: &str, dhcp: bool) -> String {
    envelope(
        HOST.rsplit_once('/').unwrap().0,
        &format!(
            "<m:GetHostnameResponse><m:HostnameInformation><t:FromDHCP>{dhcp}</t:FromDHCP><t:Name>{name}</t:Name></m:HostnameInformation></m:GetHostnameResponse>"
        ),
    )
}
fn response_info(firmware: &str) -> String {
    envelope(
        INFO.rsplit_once('/').unwrap().0,
        &format!(
            "<m:GetDeviceInformationResponse><m:Manufacturer>synthetic maker</m:Manufacturer><m:Model>reference model</m:Model><m:FirmwareVersion>{firmware}</m:FirmwareVersion><m:SerialNumber>synthetic serial</m:SerialNumber><m:HardwareId>synthetic hardware</m:HardwareId></m:GetDeviceInformationResponse>"
        ),
    )
}
fn profiles(action: &str, entries: &[(&str, &str)]) -> String {
    let service = action.rsplit_once('/').unwrap().0;
    let prefix = if action == PROFILES2 { "m" } else { "t" };
    let rows = entries
        .iter()
        .map(|(token, name)| {
            format!(
                "<m:Profiles token='{token}'><{prefix}:Name>{name}</{prefix}:Name></m:Profiles>"
            )
        })
        .collect::<String>();
    envelope(
        service,
        &format!("<m:GetProfilesResponse>{rows}</m:GetProfilesResponse>"),
    )
}
fn encoders(action: &str, bitrate: u32) -> String {
    envelope(
        action.rsplit_once('/').unwrap().0,
        &format!(
            "<m:GetVideoEncoderConfigurationsResponse><m:Configurations token='encoder-a'><t:Name>selected encoder</t:Name><t:Encoding>H264</t:Encoding><t:Resolution><t:Width>1920</t:Width><t:Height>1080</t:Height></t:Resolution><t:RateControl><t:FrameRateLimit>25</t:FrameRateLimit><t:BitrateLimit>{bitrate}</t:BitrateLimit></t:RateControl></m:Configurations></m:GetVideoEncoderConfigurationsResponse>"
        ),
    )
}
fn exchange(action: &str, response: String) -> Fixture {
    Fixture {
        action: action.into(),
        key_canon: "deliberate legacy collision".into(),
        request_raw: request(action, ""),
        response_raw: response,
    }
}
fn corpus(fixtures: Vec<Fixture>) -> FixtureCorpus {
    FixtureCorpus {
        device: "synthetic reference".into(),
        fixtures,
    }
}
async fn one(left: Fixture, right: Fixture, options: ComparisonOptions) -> ComparisonRow {
    let report = compare_corpora(&corpus(vec![left]), &corpus(vec![right]), &options)
        .await
        .unwrap();
    assert_eq!(report.rows.len(), 1, "{report:?}");
    report.rows.into_iter().next().unwrap()
}

#[tokio::test]
async fn selected_values_detect_hostname_firmware_and_bitrate_without_wire_value_leaks() {
    let row = one(
        exchange(HOST, response_hostname("private-host-alpha", false)),
        exchange(HOST, response_hostname("private-host-beta", false)),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Different);
    assert_eq!(row.changed_fields, ["/name"]);
    assert_eq!(row.projection, Some(ComparisonProjection::Hostname));
    let json = serde_json::to_string(&row).unwrap();
    assert!(!json.contains("private-host") && !json.contains("Envelope"));
    let row = one(
        exchange(INFO, response_info("release-a")),
        exchange(INFO, response_info("release-b")),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Different);
    assert_eq!(row.changed_fields, ["/firmware_version"]);
    for action in [ENCODERS1, ENCODERS2] {
        let row = one(
            exchange(action, encoders(action, 1200)),
            exchange(action, encoders(action, 2400)),
            ComparisonOptions::default(),
        )
        .await;
        assert_eq!(row.outcome, ComparisonOutcome::Different);
        assert_eq!(row.changed_fields, ["/configurations/0/bitrate_limit"]);
        assert!(
            row.unexamined_left
                .iter()
                .any(|field| field.ends_with("FrameRateLimit"))
        );
        let destination = |address: &str| {
            encoders(action, 1200).replace("</m:Configurations>", &format!("<t:Multicast><t:Address><t:Type>IPv4</t:Type><t:IPv4Address>{address}</t:IPv4Address></t:Address></t:Multicast></m:Configurations>"))
        };
        let row = one(
            exchange(action, destination("239.10.0.1")),
            exchange(action, destination("239.10.0.2")),
            ComparisonOptions::default(),
        )
        .await;
        assert_eq!(row.outcome, ComparisonOutcome::Different);
        assert_eq!(
            row.changed_fields,
            ["/configurations/0/multicast_address/value"]
        );
        let options = ComparisonOptions {
            masks: vec![ComparisonMask {
                action: action.into(),
                field: "/configurations/*/multicast_address".into(),
            }],
            ..ComparisonOptions::default()
        };
        let row = one(
            exchange(action, destination("239.10.0.1")),
            exchange(action, destination("239.10.0.2")),
            options,
        )
        .await;
        assert_eq!(row.outcome, ComparisonOutcome::Equal);
        assert_eq!(row.masks, ["/configurations/*/multicast_address"]);
    }
}

#[tokio::test]
async fn repeated_values_align_only_by_unique_tokens_and_default_masks_are_empty() {
    for action in [PROFILES1, PROFILES2] {
        let first = exchange(
            action,
            profiles(action, &[("channel-a", "alpha"), ("channel-b", "beta")]),
        );
        let reordered = exchange(
            action,
            profiles(action, &[("channel-b", "beta"), ("channel-a", "alpha")]),
        );
        let row = one(first.clone(), reordered, ComparisonOptions::default()).await;
        assert_eq!(row.outcome, ComparisonOutcome::Equal);
        assert!(row.masks.is_empty());
        let changed = exchange(
            action,
            profiles(action, &[("channel-z", "alpha"), ("channel-b", "beta")]),
        );
        let row = one(first.clone(), changed, ComparisonOptions::default()).await;
        assert_eq!(row.outcome, ComparisonOutcome::Different);
        assert!(
            row.changed_fields
                .iter()
                .any(|field| field.ends_with("/token"))
        );
        let duplicate = exchange(
            action,
            profiles(action, &[("channel-a", "alpha"), ("channel-a", "beta")]),
        );
        let row = one(first.clone(), duplicate, ComparisonOptions::default()).await;
        assert_eq!(row.outcome, ComparisonOutcome::Ambiguous);
        assert_eq!(row.reason, ComparisonReason::DuplicateTokens);
        let missing = exchange(
            action,
            profiles(action, &[("channel-a", "alpha")]).replace(" token='channel-a'", ""),
        );
        assert_eq!(
            one(first, missing, ComparisonOptions::default())
                .await
                .outcome,
            ComparisonOutcome::Unparseable
        );
    }
}

#[tokio::test]
async fn namespace_aliases_are_equal_but_selected_namespace_shadowing_is_unparseable() {
    let response = response_hostname("stable", false);
    let aliased = response.replace("xmlns:t=", "xmlns:x=").replace("t:", "x:");
    let row = one(
        exchange(HOST, response.clone()),
        exchange(HOST, aliased),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Equal);
    let entity_alias = response.replace(TT, "http://www.onvif.org/ver10/&#115;chema");
    assert_eq!(
        one(
            exchange(HOST, response.clone()),
            exchange(HOST, entity_alias),
            ComparisonOptions::default()
        )
        .await
        .outcome,
        ComparisonOutcome::Equal
    );
    let scalar_alias = response.replace(
        "<t:FromDHCP>false</t:FromDHCP>",
        "<t:FromDHCP> 0 </t:FromDHCP>",
    );
    assert_eq!(
        one(
            exchange(HOST, response.clone()),
            exchange(HOST, scalar_alias),
            ComparisonOptions::default()
        )
        .await
        .outcome,
        ComparisonOutcome::Equal
    );
    let mixed = response.replace(
        "<m:HostnameInformation>",
        "<m:HostnameInformation>unexamined-mixed-content",
    );
    assert_eq!(
        one(
            exchange(HOST, response.clone()),
            exchange(HOST, mixed),
            ComparisonOptions::default()
        )
        .await
        .outcome,
        ComparisonOutcome::Unparseable
    );
    let changed = response.replace("<t:Name>", "<t:Name xmlns:t='urn:wrong-selected-field'>");
    assert_eq!(
        one(
            exchange(HOST, response.clone()),
            exchange(HOST, changed),
            ComparisonOptions::default()
        )
        .await
        .outcome,
        ComparisonOutcome::Unparseable
    );
    let wrong_response = response.replace(
        "<m:GetHostnameResponse>",
        "<m:GetHostnameResponse xmlns:m='urn:wrong-response'>",
    );
    assert_eq!(
        one(
            exchange(HOST, response.clone()),
            exchange(HOST, wrong_response),
            ComparisonOptions::default()
        )
        .await
        .outcome,
        ComparisonOutcome::Unparseable
    );
    let qname = response.replace("<m:HostnameInformation>", "<m:HostnameInformation xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xsi:type='unresolved:Host'>");
    let row = one(
        exchange(HOST, response.clone()),
        exchange(HOST, qname),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Unsupported);
    assert_eq!(row.reason, ComparisonReason::UnresolvedQName);
    let unknown = response.replace("</m:HostnameInformation>", "<v:Extension xmlns:v='urn:synthetic:vendor'><v:Reading>private extension value</v:Reading></v:Extension></m:HostnameInformation>");
    let row = one(
        exchange(HOST, response),
        exchange(HOST, unknown),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Equal);
    assert!(
        row.unexamined_right
            .iter()
            .any(|path| path.ends_with("{urn:synthetic:vendor}Extension"))
    );
    assert!(row.unexamined_left.is_empty());
    assert!(
        !serde_json::to_string(&row)
            .unwrap()
            .contains("private extension value")
    );
}

#[tokio::test]
async fn full_action_and_proven_request_identity_survive_key_collisions_and_row_reordering() {
    let mut a = exchange(PROFILES2, profiles(PROFILES2, &[("a", "alpha")]));
    a.request_raw = request(PROFILES2, "<m:Token>a</m:Token>");
    let mut b = exchange(PROFILES2, profiles(PROFILES2, &[("b", "beta")]));
    b.request_raw = request(PROFILES2, "<m:Token>b</m:Token>");
    let left = corpus(vec![a.clone(), b.clone()]);
    let mut alias_a = a.clone();
    alias_a.key_canon = "a different bucket hint".into();
    alias_a.request_raw = alias_a
        .request_raw
        .replace("xmlns:m=", "xmlns:q=")
        .replace("m:", "q:");
    let right = corpus(vec![b.clone(), alias_a]);
    let report = compare_corpora(&left, &right, &ComparisonOptions::default())
        .await
        .unwrap();
    assert!(report.all_equal(), "{report:?}");
    assert_eq!(
        (&report.rows[0].left, &report.rows[0].right),
        (&vec![0], &vec![1])
    );
    assert_eq!(
        (&report.rows[1].left, &report.rows[1].right),
        (&vec![1], &vec![0])
    );
    let repeated = compare_corpora(&left, &right, &ComparisonOptions::default())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&report).unwrap(),
        serde_json::to_vec(&repeated).unwrap()
    );
    let mut endpoint_a = exchange(HOST, response_hostname("stable", false));
    let mut endpoint_b = endpoint_a.clone();
    let header = |target: &str, id: &str| {
        format!(
            "<s:Header xmlns:w='http://www.w3.org/2005/08/addressing'><w:To>{target}</w:To><w:MessageID>{id}</w:MessageID></s:Header><s:Body>"
        )
    };
    endpoint_a.request_raw = endpoint_a
        .request_raw
        .replace("<s:Body>", &header("http://private-a", "urn:uuid:first"));
    endpoint_b.request_raw = endpoint_b
        .request_raw
        .replace("<s:Body>", &header("http://private-b", "urn:uuid:second"));
    assert_eq!(
        one(endpoint_a, endpoint_b, ComparisonOptions::default())
            .await
            .outcome,
        ComparisonOutcome::Equal
    );
    let duplicate = compare_corpora(
        &corpus(vec![a.clone(), a.clone()]),
        &corpus(vec![a.clone()]),
        &ComparisonOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(duplicate.rows[0].outcome, ComparisonOutcome::Ambiguous);
    assert_eq!(duplicate.rows[0].left, [0, 1]);
    assert_eq!(duplicate.rows[0].right, [0]);
    let duplicate_only = compare_corpora(
        &corpus(vec![a.clone(), a]),
        &corpus(Vec::new()),
        &ComparisonOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(duplicate_only.rows[0].outcome, ComparisonOutcome::Ambiguous);
    let mixed = compare_corpora(
        &corpus(vec![exchange(PROFILES1, profiles(PROFILES1, &[]))]),
        &corpus(vec![exchange(PROFILES2, profiles(PROFILES2, &[]))]),
        &ComparisonOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(mixed.rows.len(), 2);
    let outcomes = mixed
        .rows
        .iter()
        .map(|row| (row.action.as_str(), row.outcome))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(outcomes[PROFILES1], ComparisonOutcome::MissingRight);
    assert_eq!(outcomes[PROFILES2], ComparisonOutcome::MissingLeft);
}

#[tokio::test]
async fn invalid_replay_escape_hatches_faults_and_unmapped_actions_never_produce_equal() {
    let malformed = "<Envelope>unfinished";
    let mut store = FixtureStore::new("raw replay compatibility");
    store.record(HOST, malformed, &response_hostname("stable", false));
    assert!(store.lookup_request(HOST, malformed).is_some());
    let mut raw = exchange(HOST, response_hostname("stable", false));
    raw.request_raw = malformed.into();
    let report = compare_corpora(
        &corpus(vec![raw.clone()]),
        &corpus(vec![raw]),
        &ComparisonOptions::default(),
    )
    .await
    .unwrap();
    assert!(
        report
            .rows
            .iter()
            .all(|row| row.outcome == ComparisonOutcome::Unsupported
                && row.reason == ComparisonReason::RequestIdentity)
    );
    assert!(!report.all_equal());
    let row = one(
        exchange(HOST, response_hostname("stable", false)),
        exchange(HOST, "<Envelope>unfinished".into()),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Unparseable);
    let fault = envelope(
        HOST.rsplit_once('/').unwrap().0,
        "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><s:Reason><s:Text>private Fault password=secret</s:Text></s:Reason></s:Fault>",
    );
    let row = one(
        exchange(HOST, response_hostname("stable", false)),
        exchange(HOST, fault),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::DeviceFault);
    assert!(
        !serde_json::to_string(&row)
            .unwrap()
            .contains("password=secret")
    );
    let future = "http://example.invalid/future/GetHostname";
    let row = one(
        exchange(future, response_hostname("stable", false)),
        exchange(future, response_hostname("stable", false)),
        ComparisonOptions::default(),
    )
    .await;
    assert_eq!(row.outcome, ComparisonOutcome::Unsupported);
    assert_eq!(row.reason, ComparisonReason::UnmappedAction);
    let mut qname = exchange(HOST, response_hostname("stable", false));
    qname.request_raw = qname.request_raw.replace("<m:GetHostname>", "<m:GetHostname xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xsi:type='unknown:Type'>");
    let report = compare_corpora(
        &corpus(vec![qname.clone()]),
        &corpus(vec![qname]),
        &ComparisonOptions::default(),
    )
    .await
    .unwrap();
    assert!(
        report
            .rows
            .iter()
            .all(|row| row.outcome == ComparisonOutcome::Unsupported)
    );
}

#[tokio::test]
async fn masks_are_explicit_field_scoped_and_never_remove_semantic_list_identity() {
    let left = exchange(HOST, response_hostname("alpha", false));
    let right = exchange(HOST, response_hostname("beta", false));
    let options = ComparisonOptions {
        masks: vec![ComparisonMask {
            action: HOST.into(),
            field: "/name".into(),
        }],
        ..ComparisonOptions::default()
    };
    let row = one(left, right, options).await;
    assert_eq!(row.outcome, ComparisonOutcome::Equal);
    assert_eq!(row.masks, ["/name"]);
    let profile = exchange(PROFILES1, profiles(PROFILES1, &[("a", "alpha")]));
    for (action, field) in [
        (PROFILES1, "/profiles/*/token"),
        (HOST, "/unknown"),
        ("urn:unknown", "/name"),
    ] {
        let options = ComparisonOptions {
            masks: vec![ComparisonMask {
                action: action.into(),
                field: field.into(),
            }],
            ..ComparisonOptions::default()
        };
        assert!(matches!(
            compare_corpora(
                &corpus(vec![profile.clone()]),
                &corpus(vec![profile.clone()]),
                &options
            )
            .await,
            Err(ComparisonError::InvalidMask)
        ));
    }
}

#[tokio::test]
async fn work_output_and_projection_limits_refuse_instead_of_truncating_equal() {
    let first = exchange(HOST, response_hostname("stable", false));
    let input = corpus(vec![first.clone()]);
    let report = compare_corpora(&input, &input, &ComparisonOptions::default())
        .await
        .unwrap();
    let size = serde_json::to_vec(&report).unwrap().len();
    let mut options = ComparisonOptions::default();
    options.limits.report_bytes = size;
    assert!(
        compare_corpora(&input, &input, &options)
            .await
            .unwrap()
            .all_equal()
    );
    options.limits.report_bytes = size - 1;
    assert!(matches!(
        compare_corpora(&input, &input, &options).await,
        Err(ComparisonError::Limit {
            resource: "report bytes",
            ..
        })
    ));
    let left = corpus(vec![first.clone(), exchange(INFO, response_info("one"))]);
    let mut options = ComparisonOptions::default();
    options.limits.pairs = 1;
    assert!(matches!(
        compare_corpora(&left, &left, &options).await,
        Err(ComparisonError::Limit {
            resource: "request groups",
            ..
        })
    ));
    let mut a = exchange(PROFILES2, profiles(PROFILES2, &[]));
    a.request_raw = request(PROFILES2, "<m:Token>a</m:Token>");
    let mut b = a.clone();
    b.request_raw = request(PROFILES2, "<m:Token>b</m:Token>");
    let input = corpus(vec![a, b]);
    let mut options = ComparisonOptions::default();
    options.limits.identity_checks = 1;
    assert!(matches!(
        compare_corpora(&input, &input, &options).await,
        Err(ComparisonError::Limit {
            resource: "request identity checks",
            ..
        })
    ));
    let mut options = ComparisonOptions::default();
    options.limits.field_paths = 1;
    assert!(matches!(
        compare_corpora(
            &corpus(vec![first.clone()]),
            &corpus(vec![exchange(HOST, response_hostname("different", true))]),
            &options
        )
        .await,
        Err(ComparisonError::Limit {
            resource: "field paths",
            ..
        })
    ));
    let unknown_namespace = "v".repeat(1025);
    let large = response_hostname("stable", false).replace(
        "</m:HostnameInformation>",
        &format!("<v:Extension xmlns:v='{unknown_namespace}'/></m:HostnameInformation>"),
    );
    assert!(matches!(
        compare_corpora(
            &corpus(vec![first]),
            &corpus(vec![exchange(HOST, large)]),
            &ComparisonOptions::default()
        )
        .await,
        Err(ComparisonError::Limit {
            resource: "namespace bytes",
            ..
        })
    ));
}

#[tokio::test]
async fn progress_counts_request_groups_after_bounded_pairing() {
    let input = corpus(vec![
        exchange(HOST, response_hostname("stable", false)),
        exchange(INFO, response_info("one")),
    ]);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let report = compare_corpora_with_progress(
        &input,
        &input,
        &ComparisonOptions::default(),
        move |event| seen.lock().unwrap().push(event),
    )
    .await
    .unwrap();
    assert!(report.all_equal());
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .map(|event| (event.completed, event.total, event.outcome))
            .collect::<Vec<_>>(),
        [
            (1, 2, ComparisonOutcome::Equal),
            (2, 2, ComparisonOutcome::Equal)
        ]
    );
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn all_six_projections_compare_selected_synthetic_recordings_with_unexamined_fields() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    let mut selection = SurfaceSelection::none();
    for operation in [
        SurfaceOp::GetDeviceInformation,
        SurfaceOp::GetHostname,
        SurfaceOp::GetProfiles,
        SurfaceOp::GetProfilesMedia2,
        SurfaceOp::GetVideoEncoderConfigurations,
        SurfaceOp::GetVideoEncoderConfigurationsMedia2,
    ] {
        selection = selection.with(operation);
    }
    let (store, sweep) = record_surface_with_progress(
        server.device_url(),
        None,
        "synthetic reference",
        &selection,
        |_| {},
    )
    .await
    .unwrap();
    assert!(sweep.is_complete());
    let fixtures = store
        .fixtures()
        .iter()
        .filter(|fixture| {
            [HOST, INFO, PROFILES1, PROFILES2, ENCODERS1, ENCODERS2]
                .contains(&fixture.action.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(fixtures.len(), 6);
    let corpus = corpus(fixtures);
    let report = compare_corpora(&corpus, &corpus, &ComparisonOptions::default())
        .await
        .unwrap();
    assert!(report.all_equal(), "{report:?}");
    assert_eq!(report.rows.len(), 6);
    assert!(
        report
            .rows
            .iter()
            .filter(|row| matches!(
                row.projection,
                Some(ComparisonProjection::Profiles | ComparisonProjection::EncoderCore)
            ))
            .all(|row| !row.unexamined_left.is_empty())
    );
}
