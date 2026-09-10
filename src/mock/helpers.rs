//! SOAP envelope helpers and action extraction.

#[cfg(feature = "mock-server")]
use axum::http::HeaderMap;

/// Wrap a body fragment in a SOAP 1.2 envelope.
pub fn soap(extra_ns: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tt="http://www.onvif.org/ver10/schema" {extra_ns}><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

/// The namespace URI each prefix the mock emits is bound to.
///
/// Kept here rather than at the call sites because [`resp_empty`] is the only
/// helper that builds an element from a bare prefix string.
fn namespace_for(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "tds" => "http://www.onvif.org/ver10/device/wsdl",
        "trt" => "http://www.onvif.org/ver10/media/wsdl",
        "tr2" => "http://www.onvif.org/ver20/media/wsdl",
        "tptz" => "http://www.onvif.org/ver20/ptz/wsdl",
        "timg" => "http://www.onvif.org/ver20/imaging/wsdl",
        "trc" => "http://www.onvif.org/ver10/recording/wsdl",
        "tse" => "http://www.onvif.org/ver10/search/wsdl",
        "trp" => "http://www.onvif.org/ver10/replay/wsdl",
        "tev" => "http://www.onvif.org/ver10/events/wsdl",
        "wsnt" => "http://docs.oasis-open.org/wsn/b-2",
        _ => return None,
    })
}

/// Return an empty `<prefix:Tag/>` response (for void write operations).
///
/// **The prefix is declared on the envelope.** Until 0.15 it was not: this
/// emitted `<tds:SetHostnameResponse/>` inside an envelope declaring only `s`
/// and `tt`, so `tds` was an *unbound prefix* and the document was not
/// namespace-well-formed. A conforming parser must reject it.
///
/// Nothing in this crate noticed, because `find_response` matches on local
/// name and quick-xml does not enforce prefix binding — but an external ONVIF
/// client (gSOAP and friends resolve prefixes strictly) sees a hard parse
/// error. 53 call sites across nine prefixes were affected, roughly a third of
/// the operations the mock answers. `no_response_declares_an_attribute_twice`
/// and `every_response_binds_the_prefixes_it_uses` in `dispatch.rs` are the
/// standing guards.
pub fn resp_empty(prefix: &str, tag: &str) -> String {
    let ns = match namespace_for(prefix) {
        Some(uri) => format!("xmlns:{prefix}=\"{uri}\""),
        // An unknown prefix is a programming error in the mock, not something
        // a caller can trigger. Emitting it undeclared would be the old bug,
        // so fail loudly in tests and degrade to a bare local name otherwise.
        None => {
            debug_assert!(false, "resp_empty: no namespace registered for `{prefix}`");
            return soap("", &format!("<{tag}/>"));
        }
    };
    soap(&ns, &format!("<{prefix}:{tag}/>"))
}

/// Return a legacy flat SOAP 1.2 Fault with escaped text and bound known QNames.
///
/// This compatibility helper does not manufacture an ONVIF subcode hierarchy.
/// Operation-specific structured-fault migration is tracked separately.
pub fn resp_soap_fault(code: &str, reason: &str) -> String {
    let code = crate::types::xml_escape(code);
    let reason = crate::types::xml_escape(reason);
    soap(
        r#"xmlns:env="http://www.w3.org/2003/05/soap-envelope" xmlns:ter="http://www.onvif.org/ver10/error""#,
        &format!(
            r#"<s:Fault><s:Code><s:Value>{code}</s:Value></s:Code><s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason></s:Fault>"#
        ),
    )
}

/// Extract the SOAPAction URI from the Content-Type header.
///
/// SOAP 1.2 puts the action in the Content-Type header:
/// `application/soap+xml; charset=utf-8; action="http://..."`
#[cfg(feature = "mock-server")]
pub fn extract_action(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get("content-type")?.to_str().ok()?;
    let action_part = ct.split(';').find(|s| s.trim().starts_with("action="))?;
    let raw = action_part.trim().strip_prefix("action=")?;
    Some(raw.trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::{
        NsReader,
        events::Event,
        name::{QName, ResolveResult},
    };

    #[test]
    fn fault_text_is_literal_not_markup() {
        let reason = "sensor <& \"北門\"> </s:Text><injected/>";
        let xml = resp_soap_fault("env:Sender", reason);
        let body = crate::soap::parse_soap_body(&xml).unwrap();
        let fault = body.child("Fault").unwrap();
        let text = fault.path(&["Reason", "Text"]).unwrap();
        assert_eq!(text.text(), reason);
        assert!(text.children.is_empty());
        assert_eq!(fault.child("Reason").unwrap().children.len(), 1);
        assert!(!xml.contains("<injected/>"));
    }

    #[test]
    fn fault_code_text_cannot_insert_elements() {
        let xml = resp_soap_fault("s:Receiver</s:Value><injected/>", "literal code");
        assert!(!xml.contains("<injected/>"));
        let body = crate::soap::parse_soap_body(&xml).unwrap();
        assert_eq!(
            body.child("Fault")
                .unwrap()
                .path(&["Code", "Value"])
                .unwrap()
                .text(),
            "s:Receiver</s:Value><injected/>"
        );
    }

    #[test]
    fn fault_qnames_have_bindings_in_value_scope() {
        for (code, namespace) in [
            ("env:Sender", "http://www.w3.org/2003/05/soap-envelope"),
            ("s:Receiver", "http://www.w3.org/2003/05/soap-envelope"),
            ("ter:NoProfile", "http://www.onvif.org/ver10/error"),
        ] {
            let xml = resp_soap_fault(code, "binding probe");
            let mut reader = NsReader::from_str(&xml);
            let mut checked = false;
            loop {
                match reader.read_event().unwrap() {
                    Event::Text(text) if text.as_ref() == code => {
                        let (resolved, _) = reader.resolver().resolve_element(QName(code));
                        match resolved {
                            ResolveResult::Bound(ns) => assert_eq!(ns.as_ref(), namespace),
                            other => panic!("unbound fault code {code}: {other:?}"),
                        }
                        checked = true;
                    }
                    Event::Eof => break,
                    _ => {}
                }
            }
            assert!(checked, "fault code was not inspected: {code}");
        }
    }
}
