//! SOAP 1.2 envelope builder.
//!
//! [`SoapEnvelope`] constructs a fully-namespaced `<s:Envelope>` string ready
//! to be posted over HTTP. All ONVIF-relevant XML namespace prefixes are
//! declared on the root element so that body fragments can use any prefix
//! without declaring it locally.
//!
//! `<s:Header>` is emitted only when a [`WsSecurityToken`] or a `<wsa:To>`
//! address is provided; otherwise the envelope has no header element.

use crate::soap::security::WsSecurityToken;
use std::fmt::Write;

/// Escape the five predefined XML entities in `s`.
fn xml_escape_url(s: &str) -> std::borrow::Cow<'_, str> {
    if s.bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        std::borrow::Cow::Owned(
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;"),
        )
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

// ── Namespace declarations ────────────────────────────────────────────────────

/// All ONVIF-relevant XML namespace prefix→URI pairs declared on every
/// envelope. Add new service namespaces here; no other change is required.
const NAMESPACES: &[(&str, &str)] = &[
    ("s", "http://www.w3.org/2003/05/soap-envelope"),
    ("enc", "http://www.w3.org/2003/05/soap-encoding"),
    ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ("xsd", "http://www.w3.org/2001/XMLSchema"),
    ("wsa", "http://www.w3.org/2005/08/addressing"),
    (
        "wsse",
        "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd",
    ),
    (
        "wsu",
        "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd",
    ),
    ("wsnt", "http://docs.oasis-open.org/wsn/b-2"),
    ("tt", "http://www.onvif.org/ver10/schema"),
    ("tds", "http://www.onvif.org/ver10/device/wsdl"),
    ("trt", "http://www.onvif.org/ver10/media/wsdl"),
    ("tr2", "http://www.onvif.org/ver20/media/wsdl"),
    ("tev", "http://www.onvif.org/ver10/events/wsdl"),
    ("tptz", "http://www.onvif.org/ver20/ptz/wsdl"),
    ("timg", "http://www.onvif.org/ver20/imaging/wsdl"),
    ("tan", "http://www.onvif.org/ver20/analytics/wsdl"),
    ("ter", "http://www.onvif.org/ver10/error"),
    ("trc", "http://www.onvif.org/ver10/recording/wsdl"),
    ("tse", "http://www.onvif.org/ver10/search/wsdl"),
    ("trp", "http://www.onvif.org/ver10/replay/wsdl"),
];

// ── SoapEnvelope ──────────────────────────────────────────────────────────────

/// Builder for a SOAP 1.2 envelope.
///
/// # Example
///
/// ```
/// use oxvif::soap::{SoapEnvelope, WsSecurityToken};
///
/// let token = WsSecurityToken::from_parts("admin", "digest==", "nonce==", "2024-01-01T00:00:00Z");
/// let xml = SoapEnvelope::new("<tds:GetCapabilities/>".into())
///     .with_security(token)
///     .build();
///
/// assert!(xml.contains("<wsse:Security>"));
/// assert!(xml.contains("<s:Body>"));
/// ```
pub struct SoapEnvelope {
    /// Optional WS-Security UsernameToken placed in `<s:Header>`.
    security: Option<WsSecurityToken>,
    /// Raw XML fragment placed inside `<s:Body>`.
    body_content: String,
    /// Optional `<wsa:To>` address placed in `<s:Header>`.
    wsa_to: Option<String>,
}

impl SoapEnvelope {
    /// Create an envelope with the given body fragment.
    pub fn new(body_content: String) -> Self {
        Self {
            security: None,
            body_content,
            wsa_to: None,
        }
    }

    /// Attach a WS-Security `UsernameToken` to the SOAP header.
    pub fn with_security(mut self, token: WsSecurityToken) -> Self {
        self.security = Some(token);
        self
    }

    /// Set the `<wsa:To>` WS-Addressing destination in the SOAP header.
    /// Some devices require this to match the service endpoint URL.
    pub fn with_wsa_to(mut self, to: impl Into<String>) -> Self {
        self.wsa_to = Some(to.into());
        self
    }

    /// Serialise the envelope to a UTF-8 XML string.
    pub fn build(self) -> String {
        let mut out = String::with_capacity(2048);

        out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);

        // Root element with all namespace declarations
        out.push_str("<s:Envelope");
        for (prefix, uri) in NAMESPACES {
            write!(out, r#" xmlns:{prefix}="{uri}""#).unwrap();
        }
        out.push('>');

        // Header — emitted only when there is content to put inside
        let has_header = self.security.is_some() || self.wsa_to.is_some();
        if has_header {
            out.push_str("<s:Header>");
            if let Some(to) = &self.wsa_to {
                write!(out, "<wsa:To>{}</wsa:To>", xml_escape_url(to)).unwrap();
            }
            if let Some(sec) = &self.security {
                sec.write_xml(&mut out);
            }
            out.push_str("</s:Header>");
        }

        out.push_str("<s:Body>");
        out.push_str(&self.body_content);
        out.push_str("</s:Body>");
        out.push_str("</s:Envelope>");

        out
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soap::security::WsSecurityToken;

    fn parse_ok(xml: &str) -> bool {
        crate::soap::xml::XmlNode::parse(xml).is_ok()
    }

    #[test]
    fn test_build_produces_valid_xml() {
        let xml = SoapEnvelope::new("<tds:GetCapabilities/>".to_string()).build();
        assert!(parse_ok(&xml), "produced XML should be well-formed");
    }

    #[test]
    fn test_envelope_starts_with_xml_declaration() {
        let xml = SoapEnvelope::new(String::new()).build();
        assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    }

    #[test]
    fn test_envelope_root_element() {
        let xml = SoapEnvelope::new(String::new()).build();
        assert!(xml.contains("<s:Envelope"));
        assert!(xml.ends_with("</s:Envelope>"));
    }

    #[test]
    fn test_body_content_preserved() {
        let body = r#"<tds:GetCapabilities><tds:Category>All</tds:Category></tds:GetCapabilities>"#;
        let xml = SoapEnvelope::new(body.to_string()).build();
        assert!(xml.contains(body));
        assert!(xml.contains("<s:Body>"));
        assert!(xml.contains("</s:Body>"));
    }

    #[test]
    fn test_required_namespaces_present() {
        let xml = SoapEnvelope::new(String::new()).build();
        assert!(xml.contains(r#"xmlns:s="http://www.w3.org/2003/05/soap-envelope""#));
        assert!(xml.contains(r#"xmlns:tt="http://www.onvif.org/ver10/schema""#));
        assert!(xml.contains(r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#));
        assert!(xml.contains(r#"xmlns:wsse=""#));
        assert!(xml.contains(r#"xmlns:wsu=""#));
    }

    #[test]
    fn test_no_header_when_no_security_no_wsa() {
        let xml = SoapEnvelope::new(String::new()).build();
        assert!(!xml.contains("<s:Header>"));
    }

    #[test]
    fn test_header_present_with_security() {
        let token = WsSecurityToken::from_parts("admin", "digest", "nonce", "2024-01-01T00:00:00Z");
        let xml = SoapEnvelope::new(String::new())
            .with_security(token)
            .build();
        assert!(xml.contains("<s:Header>"));
        assert!(xml.contains("</s:Header>"));
        assert!(xml.contains("<wsse:Security>"));
    }

    #[test]
    fn test_header_contains_wsa_to() {
        let xml = SoapEnvelope::new(String::new())
            .with_wsa_to("http://192.168.1.100/onvif/device_service")
            .build();
        assert!(xml.contains("<s:Header>"));
        assert!(xml.contains("<wsa:To>http://192.168.1.100/onvif/device_service</wsa:To>"));
    }

    #[test]
    fn test_security_fields_in_xml() {
        let token = WsSecurityToken::from_parts(
            "operator",
            "Zm9vYmFy", // base64("foobar")
            "bm9uY2U=", // base64("nonce")
            "2024-06-15T08:00:00Z",
        );
        let xml = SoapEnvelope::new(String::new())
            .with_security(token)
            .build();
        assert!(xml.contains("<wsse:Username>operator</wsse:Username>"));
        assert!(xml.contains(">Zm9vYmFy</wsse:Password>"));
        assert!(xml.contains(">bm9uY2U=</wsse:Nonce>"));
        assert!(xml.contains(">2024-06-15T08:00:00Z</wsu:Created>"));
    }

    #[test]
    fn test_full_envelope_is_parseable_and_navigable() {
        use crate::soap::xml::{find_response, parse_soap_body};

        let token = WsSecurityToken::from_parts("admin", "d", "n", "2024-01-01T00:00:00Z");
        let envelope = SoapEnvelope::new(
            "<tds:GetCapabilities><tds:Category>All</tds:Category></tds:GetCapabilities>"
                .to_string(),
        )
        .with_security(token)
        .build();

        assert!(parse_ok(&envelope));

        let body = parse_soap_body(&envelope).unwrap();
        assert_eq!(body.local_name, "Body");

        let req = find_response(&body, "GetCapabilities").unwrap();
        assert_eq!(req.child("Category").unwrap().text(), "All");
    }
}
