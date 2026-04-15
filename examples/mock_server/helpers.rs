//! SOAP envelope helpers and action extraction.

use axum::http::HeaderMap;

/// Wrap a body fragment in a SOAP 1.2 envelope.
pub fn soap(extra_ns: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tt="http://www.onvif.org/ver10/schema" {extra_ns}><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

/// Return an empty `<prefix:Tag/>` response (for void write operations).
pub fn resp_empty(prefix: &str, tag: &str) -> String {
    soap("", &format!("<{prefix}:{tag}/>"))
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
pub fn extract_action(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get("content-type")?.to_str().ok()?;
    let action_part = ct.split(';').find(|s| s.trim().starts_with("action="))?;
    let raw = action_part.trim().strip_prefix("action=")?;
    Some(raw.trim_matches('"').to_string())
}
