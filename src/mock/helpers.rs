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

/// Return a SOAP 1.2 Fault.
pub fn resp_soap_fault(code: &str, reason: &str) -> String {
    soap(
        "",
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
