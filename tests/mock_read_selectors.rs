//! Project-authored selector controls; no schema-derived fixture catalogue.
#![cfg(feature = "mock")]

use oxvif::{
    mock::{MockState, MockTransport},
    soap::{SoapError, find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "http://www.onvif.org/ver10/media/wsdl",
        "GetOSD",
        "OSDToken",
        "OSD",
    ),
    (
        "http://www.onvif.org/ver20/ptz/wsdl",
        "GetNode",
        "NodeToken",
        "PTZNode",
    ),
    (
        "http://www.onvif.org/ver20/ptz/wsdl",
        "GetConfiguration",
        "PTZConfigurationToken",
        "PTZConfiguration",
    ),
    (
        "http://www.onvif.org/ver10/recording/wsdl",
        "GetRecordingJobState",
        "JobToken",
        "State",
    ),
];
const TOKENS: &[(&str, &str)] = &[
    (
        "selector &amp; <one> \"'",
        "selector &amp;amp; &lt;one&gt; &quot;&apos;",
    ),
    ("selector two", "selector two"),
];

fn seed(state: &MockState) {
    state.modify(|s| {
        let first = s.osd.osds[0].clone();
        s.osd.osds = vec![first.clone(), first];
        for (i, (token, _)) in TOKENS.iter().enumerate() {
            s.osd.osds[i].token = (*token).into();
            s.osd.osds[i].video_source_config_token = format!("source-{i} & value");
            s.ptz_nodes[i].token = (*token).into();
            s.ptz_nodes[i].name = format!("node-{i} & value");
            s.ptz_configs[i].token = (*token).into();
            s.ptz_configs[i].name = format!("config-{i} & value");
            s.recording.jobs[i].token = (*token).into();
            s.recording.jobs[i].recording_token = format!("recording-{i} & value");
        }
    });
}

async fn request(
    transport: &dyn Transport,
    url: &str,
    ns: &str,
    op: &str,
    field: &str,
    header: &str,
) -> String {
    transport.soap_post(url, &format!("{ns}/{op}"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:p='{ns}' xmlns:x='urn:decoy'><s:Header>{header}</s:Header><s:Body><p:{op}>{field}</p:{op}></s:Body></s:Envelope>"
    )).await.unwrap()
}

async fn exercise(transport: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    seed(state);
    let before = serde_json::to_value(&*state.read()).unwrap();
    let changes = hooks.load(Ordering::SeqCst);
    for &(ns, op, field, result) in CASES {
        for (i, (token, encoded)) in TOKENS.iter().enumerate() {
            // Earlier Header, foreign and extension selectors cannot shadow the direct field.
            let decoy = format!("<p:{field}>not-stored</p:{field}>");
            let input = format!(
                "<x:{field}>not-stored</x:{field}><x:Extension>{decoy}</x:Extension><p:{field}>{encoded}</p:{field}>"
            );
            let response = request(transport, url, ns, op, &input, &decoy).await;
            let body = parse_soap_body(&response).unwrap();
            let child = find_response(&body, &format!("{op}Response"))
                .unwrap()
                .child(result)
                .unwrap();
            if op != "GetRecordingJobState" {
                assert_eq!(child.attr("token"), Some(*token), "{op}: {response}");
            }
            let (path, expected) = match op {
                "GetOSD" => (
                    "VideoSourceConfigurationToken",
                    format!("source-{i} & value"),
                ),
                "GetNode" => ("Name", format!("node-{i} & value")),
                "GetConfiguration" => ("Name", format!("config-{i} & value")),
                _ => ("RecordingToken", format!("recording-{i} & value")),
            };
            assert_eq!(
                child.child(path).unwrap().text.as_deref(),
                Some(expected.as_str())
            );
            // NsReader independently resolves the response wrapper, unlike XmlNode above.
            let mut reader = quick_xml::NsReader::from_str(&response);
            let mut found = false;
            loop {
                let (namespace, event) = reader.read_resolved_event().unwrap();
                match event {
                    quick_xml::events::Event::Start(e)
                        if e.local_name().as_ref() == format!("{op}Response") =>
                    {
                        assert_eq!(
                            namespace,
                            quick_xml::name::ResolveResult::Bound(quick_xml::name::Namespace(ns))
                        );
                        found = true;
                    }
                    quick_xml::events::Event::Eof => break,
                    _ => {}
                }
            }
            assert!(found);
        }
        for input in [
            format!("<p:{field}>selector two</p:{field}><p:{field}>selector two</p:{field}>"),
            format!("<p:{field}><p:Nested>selector two</p:Nested></p:{field}>"),
        ] {
            let response = request(transport, url, ns, op, &input, "").await;
            assert_eq!(
                find_response(
                    &parse_soap_body(&response).unwrap(),
                    &format!("{op}Response")
                )
                .unwrap_err(),
                SoapError::Fault {
                    code: "s:Sender".into(),
                    subcode: Some("ter:InvalidArgs".into()),
                    reason: "Invalid Args".into(),
                    detail: None,
                }
            );
        }
        for input in [
            String::new(),
            format!("<p:{field}/>"),
            format!("<x:{field}>selector two</x:{field}>"),
            format!("<x:Extension><p:{field}>selector two</p:{field}></x:Extension>"),
            format!("<p:{field}>wrong-family-token</p:{field}>"),
        ] {
            let response = request(
                transport,
                url,
                ns,
                op,
                &input,
                &format!("<p:{field}>selector two</p:{field}>"),
            )
            .await;
            assert!(
                matches!(
                    find_response(
                        &parse_soap_body(&response).unwrap(),
                        &format!("{op}Response")
                    ),
                    Err(SoapError::Fault { .. })
                ),
                "{op}: {response}"
            );
        }
        let response = transport
            .soap_post(
                url,
                &format!("{ns}/{op}"),
                format!(
                    "<{op} xmlns='{ns}'><{field}><![CDATA[{}]]></{field}></{op}>",
                    TOKENS[0].0
                ),
            )
            .await
            .unwrap();
        find_response(
            &parse_soap_body(&response).unwrap(),
            &format!("{op}Response"),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            before,
            "{op} changed state"
        );
        assert_eq!(hooks.load(Ordering::SeqCst), changes, "{op} fired hook");
    }
    // The unusual identity is absent from another instance.
    let other = MockTransport::new();
    let response = request(
        &other,
        "http://mock",
        CASES[0].0,
        "GetOSD",
        &format!("<p:OSDToken>{}</p:OSDToken>", TOKENS[0].1),
        "",
    )
    .await;
    assert!(find_response(&parse_soap_body(&response).unwrap(), "GetOSDResponse").is_err());
}

#[tokio::test]
async fn scoped_read_selectors_in_process() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    }));
    let mock = MockTransport::with_state(state);
    exercise(&mock, "http://mock", mock.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn scoped_read_selectors_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    exercise(
        &oxvif::transport::HttpTransport::new(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}
