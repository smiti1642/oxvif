//! Synthetic boundary probes; project-authored inputs, not a schema corpus.
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

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const DEVICE: &str = "http://www.onvif.org/ver10/device/wsdl";

fn envelope(content: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='{SOAP}' xmlns:d='{DEVICE}'><s:Body>{content}</s:Body></s:Envelope>"
    )
}

fn expanded_fault_values(xml: &str) -> Vec<(String, String)> {
    use quick_xml::{
        NsReader,
        events::Event,
        name::{QName, ResolveResult},
    };
    let mut reader = NsReader::from_str(xml);
    let mut in_value = false;
    let mut values = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) if element.local_name().as_ref() == "Value" => in_value = true,
            Event::End(element) if element.local_name().as_ref() == "Value" => in_value = false,
            Event::Text(text) if in_value => {
                let (ns, local) = reader.resolver().resolve_element(QName(text.as_ref()));
                let ResolveResult::Bound(ns) = ns else {
                    panic!("unbound fault QName");
                };
                values.push((ns.as_ref().to_string(), local.as_ref().to_string()));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    values
}

async fn exercise(transport: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    for op in ["GetDeviceInformation", "SetHostname"] {
        let operation = format!("<d:{op}><d:Name>boundary-612</d:Name></d:{op}>");
        let good = envelope(&operation);
        for (label, xml, code, subcode, reason) in [
            (
                "trailing root",
                format!("{good}<extra/>"),
                "s:Sender",
                Some("ter:WellFormed"),
                "Well-formed Error",
            ),
            (
                "unknown entity",
                good.replace("boundary-612", "&not-defined;"),
                "s:Sender",
                Some("ter:WellFormed"),
                "Well-formed Error",
            ),
            (
                "unbound prefix",
                good.replace("xmlns:d=", "xmlns:other="),
                "s:Sender",
                Some("ter:Namespace"),
                "Namespace Error",
            ),
            (
                "wrong operation",
                envelope("<d:DifferentOperation/>"),
                "s:Sender",
                Some("ter:TagMismatch"),
                "Tag Mismatch",
            ),
            (
                "wrong namespace",
                good.replace(DEVICE, "urn:boundary:wrong"),
                "s:Sender",
                Some("ter:TagMismatch"),
                "Tag Mismatch",
            ),
            (
                "duplicate body",
                good.replace("</s:Envelope>", "<s:Body/></s:Envelope>"),
                "s:Sender",
                Some("ter:InvalidArgs"),
                "Invalid Args",
            ),
            (
                "body text",
                good.replace("<s:Body>", "<s:Body>not-whitespace"),
                "s:Sender",
                Some("ter:InvalidArgs"),
                "Invalid Args",
            ),
            (
                "late header",
                good.replace("</s:Envelope>", "<s:Header/></s:Envelope>"),
                "s:Sender",
                Some("ter:InvalidArgs"),
                "Invalid Args",
            ),
            (
                "empty body with header decoy",
                format!(
                    "<s:Envelope xmlns:s='{SOAP}' xmlns:d='{DEVICE}'><s:Header>{operation}</s:Header><s:Body/></s:Envelope>"
                ),
                "s:Sender",
                Some("ter:InvalidArgs"),
                "Invalid Args",
            ),
            (
                "wrong SOAP version",
                good.replace(SOAP, "http://schemas.xmlsoap.org/soap/envelope/"),
                "s:VersionMismatch",
                None,
                "SOAP version mismatch",
            ),
            (
                "DTD",
                format!("<!DOCTYPE Envelope>{good}"),
                "s:Sender",
                Some("mock:RequestPolicy"),
                "Unsupported mock request construct",
            ),
            (
                "depth limit",
                envelope(&format!(
                    "<d:{op}>{}{}</d:{op}>",
                    "<n>".repeat(65),
                    "</n>".repeat(65)
                )),
                "s:Sender",
                Some("mock:RequestLimit"),
                "Mock request resource limit exceeded",
            ),
            (
                "node limit",
                envelope(&format!("<d:{op}>{}</d:{op}>", "<n/>".repeat(16_384))),
                "s:Sender",
                Some("mock:RequestLimit"),
                "Mock request resource limit exceeded",
            ),
        ] {
            let xml = transport
                .soap_post(url, &format!("{DEVICE}/{op}"), xml)
                .await
                .unwrap();
            assert_eq!(
                find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
                SoapError::Fault {
                    code: code.into(),
                    reason: reason.into(),
                    subcode: subcode.map(str::to_owned),
                    detail: None
                },
                "{op}: {label}"
            );
            assert_eq!(
                serde_json::to_value(&*state.read()).unwrap(),
                before,
                "{op}: {label}"
            );
            assert_eq!(hooks.load(Ordering::SeqCst), 0, "{op}: {label}");
            let expected: Vec<_> = std::iter::once(code)
                .chain(subcode)
                .map(|qname| {
                    let (prefix, local) = qname.split_once(':').unwrap();
                    let ns = match prefix {
                        "s" => SOAP,
                        "ter" => "http://www.onvif.org/ver10/error",
                        "mock" => "urn:oxvif:mock:error",
                        _ => panic!("unexpected test QName prefix"),
                    };
                    (ns.to_owned(), local.to_owned())
                })
                .collect();
            assert_eq!(expanded_fault_values(&xml), expected, "{op}: {label}");
        }
    }
    // A namespace alias is not a different identity. Unknown optional Header
    // content cannot supply operation fields or trigger an unrelated write.
    let xml = format!(
        "<e:Envelope xmlns:e='{SOAP}'><e:Header><x:Probe xmlns:x='urn:boundary:probe'>decoy-612</x:Probe></e:Header><e:Body><GetDeviceInformation xmlns='{DEVICE}'/></e:Body></e:Envelope>"
    );
    let xml = transport
        .soap_post(url, &format!("{DEVICE}/GetDeviceInformation"), xml)
        .await
        .unwrap();
    let response = parse_soap_body(&xml).unwrap();
    assert_eq!(
        find_response(&response, "GetDeviceInformationResponse")
            .unwrap()
            .child("Manufacturer")
            .unwrap()
            .text(),
        "oxvif-mock"
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
    let xml = transport
        .soap_post(
            url,
            &format!("{DEVICE}/SetHostname"),
            envelope("<d:SetHostname><d:Name>boundary-success-612</d:Name></d:SetHostname>"),
        )
        .await
        .unwrap();
    assert_eq!(
        find_response(&parse_soap_body(&xml).unwrap(), "SetHostnameResponse")
            .unwrap()
            .local_name,
        "SetHostnameResponse"
    );
    assert_eq!(state.read().hostname, "boundary-success-612");
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn in_process_static_and_stateful_requests_share_the_parsed_boundary() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = MockTransport::with_state(state);
    exercise(&transport, "http://mock", transport.device(), &hooks).await;
}

#[tokio::test]
async fn in_process_byte_limit_is_not_reported_as_an_xml_syntax_error() {
    let transport = MockTransport::new();
    let before = serde_json::to_value(&*transport.device().read()).unwrap();
    let xml = transport
        .soap_post(
            "http://mock",
            &format!("{DEVICE}/GetDeviceInformation"),
            "x".repeat(2 * 1024 * 1024 + 1),
        )
        .await
        .unwrap();
    assert_eq!(
        find_response(&parse_soap_body(&xml).unwrap(), "unused").unwrap_err(),
        SoapError::Fault {
            code: "s:Sender".into(),
            subcode: Some("mock:RequestLimit".into()),
            reason: "Mock request resource limit exceeded".into(),
            detail: None
        }
    );
    assert_eq!(
        serde_json::to_value(&*transport.device().read()).unwrap(),
        before
    );
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_static_and_stateful_requests_share_the_parsed_boundary() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
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
