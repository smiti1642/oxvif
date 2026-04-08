//! Namespace-aware XML DOM and SOAP response helpers.
//!
//! [`XmlNode`] is a minimal tree built with `quick-xml`. All namespace
//! prefixes are stripped from element and attribute names on parse so that
//! upper layers never need to know whether a device uses `tds:`, `SOAP-ENV:`,
//! or any other prefix.
//!
//! Two SOAP-specific helpers sit on top of the DOM:
//!
//! * [`parse_soap_body`] — parses a full SOAP envelope and returns the
//!   `<s:Body>` node.
//! * [`find_response`] — locates the expected response element inside `Body`,
//!   or converts a `<s:Fault>` into a [`SoapError::Fault`].

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::soap::error::SoapError;

// ── XmlNode ───────────────────────────────────────────────────────────────────

/// A namespace-stripped XML node.
///
/// `local_name` has the namespace prefix removed:
/// `tds:GetCapabilities` → `GetCapabilities`.
/// Attribute keys are stripped the same way; `xmlns:*` declarations are
/// discarded entirely.
#[derive(Debug, Clone, Default)]
pub struct XmlNode {
    pub local_name: String,
    /// Trimmed text content of the element, if any.
    pub text: Option<String>,
    /// Attributes keyed by local name (namespace declarations excluded).
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlNode>,
}

// ── Navigation ────────────────────────────────────────────────────────────────

impl XmlNode {
    /// Return the first direct child whose `local_name` matches.
    pub fn child(&self, local_name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|n| n.local_name == local_name)
    }

    /// Traverse a sequence of `local_name` segments from this node.
    ///
    /// ```
    /// # use oxvif::soap::XmlNode;
    /// let xml = "<A><B><C>hello</C></B></A>";
    /// let root = XmlNode::parse(xml).unwrap();
    /// assert_eq!(root.path(&["B", "C"]).unwrap().text(), "hello");
    /// ```
    pub fn path(&self, segments: &[&str]) -> Option<&XmlNode> {
        segments.iter().try_fold(self, |n, seg| n.child(seg))
    }

    /// Return the trimmed text content, or `""` if absent.
    pub fn text(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }

    /// Return the value of the attribute with the given local name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    /// Iterate over all direct children whose `local_name` matches.
    /// Useful for repeated elements such as multiple `<trt:Profiles>`.
    pub fn children_named<'a>(&'a self, local_name: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children
            .iter()
            .filter(move |n| n.local_name == local_name)
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

impl XmlNode {
    /// Parse a UTF-8 XML string into a node tree.
    ///
    /// Uses an explicit stack instead of recursion; will not overflow on
    /// arbitrarily deep documents.
    pub fn parse(xml: &str) -> Result<Self, SoapError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut stack: Vec<XmlNode> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    stack.push(Self::from_bytes_start(e));
                }

                Ok(Event::Empty(ref e)) => {
                    // Self-closing tag: <Foo/>
                    let node = Self::from_bytes_start(e);
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        return Ok(node);
                    }
                }

                Ok(Event::End(_)) => {
                    let finished = stack
                        .pop()
                        .ok_or_else(|| SoapError::XmlParse("unmatched closing tag".into()))?;
                    if stack.is_empty() {
                        return Ok(finished);
                    }
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(finished);
                    }
                }

                Ok(Event::Text(e)) => {
                    if let Some(node) = stack.last_mut() {
                        let cow = e.xml_content().unwrap_or_default();
                        let trimmed = cow.trim().to_string();
                        if !trimmed.is_empty() {
                            node.text = Some(trimmed);
                        }
                    }
                }

                Ok(Event::CData(ref e)) => {
                    if let Some(node) = stack.last_mut()
                        && let Ok(s) = std::str::from_utf8(e.as_ref())
                    {
                        node.text = Some(s.to_string());
                    }
                }

                Ok(Event::Eof) => {
                    return stack
                        .pop()
                        .ok_or_else(|| SoapError::XmlParse("empty document".into()));
                }

                Err(e) => return Err(SoapError::XmlParse(e.to_string())),

                // Declaration, Comment, PI — ignored
                _ => {}
            }
        }
    }

    fn from_bytes_start(e: &quick_xml::events::BytesStart<'_>) -> Self {
        let local_name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();

        let mut attrs = HashMap::new();
        for attr_result in e.attributes() {
            let Ok(attr) = attr_result else { continue };

            // Drop namespace declarations (xmlns and xmlns:prefix)
            let is_ns_decl = attr.key.as_ref() == b"xmlns"
                || attr.key.prefix().is_some_and(|p| p.as_ref() == b"xmlns");
            if is_ns_decl {
                continue;
            }

            let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
            let value = attr
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_default();
            attrs.insert(key, value);
        }

        XmlNode {
            local_name,
            attrs,
            text: None,
            children: Vec::new(),
        }
    }
}

// ── SOAP helpers ──────────────────────────────────────────────────────────────

/// Parse a SOAP envelope and return the `<s:Body>` node.
///
/// The namespace prefix on `Body` is ignored, so both `s:Body` and
/// `SOAP-ENV:Body` are accepted.
pub fn parse_soap_body(xml: &str) -> Result<XmlNode, SoapError> {
    let mut root = XmlNode::parse(xml)?;
    let idx = root
        .children
        .iter()
        .position(|c| c.local_name == "Body")
        .ok_or(SoapError::MissingBody)?;
    Ok(root.children.swap_remove(idx))
}

/// Find the expected response element inside a `Body` node.
///
/// * If `Body` contains `<s:Fault>`, returns [`SoapError::Fault`] with the
///   structured code and reason (supports both SOAP 1.1 and 1.2 fault formats).
/// * If `expected_tag` is not found, returns [`SoapError::UnexpectedResponse`].
pub fn find_response<'a>(body: &'a XmlNode, expected_tag: &str) -> Result<&'a XmlNode, SoapError> {
    if let Some(fault) = body.child("Fault") {
        // SOAP 1.2: Code/Value + Reason/Text
        // SOAP 1.1 fallback: faultcode + faultstring
        let code = fault
            .path(&["Code", "Value"])
            .or_else(|| fault.child("faultcode"))
            .map(|n| n.text().to_string())
            .unwrap_or_default();

        let reason = fault
            .path(&["Reason", "Text"])
            .or_else(|| fault.child("faultstring"))
            .map(|n| n.text().to_string())
            .unwrap_or_default();

        return Err(SoapError::Fault { code, reason });
    }

    body.child(expected_tag)
        .ok_or_else(|| SoapError::UnexpectedResponse(expected_tag.to_string()))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_element() {
        let node = XmlNode::parse("<Root>hello</Root>").unwrap();
        assert_eq!(node.local_name, "Root");
        assert_eq!(node.text(), "hello");
    }

    #[test]
    fn test_parse_nested() {
        let xml = "<Parent><Child>value</Child></Parent>";
        let root = XmlNode::parse(xml).unwrap();
        assert_eq!(root.local_name, "Parent");
        assert_eq!(root.child("Child").unwrap().text(), "value");
    }

    #[test]
    fn test_parse_self_closing_tag() {
        let xml = "<Root><Empty/><HasText>x</HasText></Root>";
        let root = XmlNode::parse(xml).unwrap();
        assert!(root.child("Empty").is_some());
        assert_eq!(root.child("HasText").unwrap().text(), "x");
    }

    #[test]
    fn test_parse_attribute() {
        let xml = r#"<Node token="abc123" fixed="true"/>"#;
        let node = XmlNode::parse(xml).unwrap();
        assert_eq!(node.attr("token"), Some("abc123"));
        assert_eq!(node.attr("fixed"), Some("true"));
    }

    #[test]
    fn test_strips_namespace_prefix_from_elements() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                        xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
              <s:Body>
                <tds:GetCapabilitiesResponse/>
              </s:Body>
            </s:Envelope>"#;

        let root = XmlNode::parse(xml).unwrap();
        assert_eq!(root.local_name, "Envelope");
        assert!(root.child("Body").is_some());
        assert!(root.path(&["Body", "GetCapabilitiesResponse"]).is_some());
    }

    #[test]
    fn test_strips_prefix_from_attributes() {
        let xml =
            r#"<Profile tt:token="main_profile" xmlns:tt="http://www.onvif.org/ver10/schema"/>"#;
        let node = XmlNode::parse(xml).unwrap();
        assert_eq!(node.attr("token"), Some("main_profile"));
        assert_eq!(node.attrs.get("xmlns"), None);
        assert_eq!(node.attrs.get("tt"), None);
    }

    #[test]
    fn test_xmlns_attributes_are_filtered() {
        let xml = r#"<Root xmlns="http://default.ns" xmlns:foo="http://foo.ns" bar="baz"/>"#;
        let node = XmlNode::parse(xml).unwrap();
        assert_eq!(node.attrs.len(), 1);
        assert_eq!(node.attr("bar"), Some("baz"));
    }

    #[test]
    fn test_path_navigation() {
        let xml = "<A><B><C>deep</C></B></A>";
        let root = XmlNode::parse(xml).unwrap();
        assert_eq!(root.path(&["B", "C"]).unwrap().text(), "deep");
        assert!(root.path(&["B", "X"]).is_none());
    }

    #[test]
    fn test_children_named_iterates_all() {
        let xml = "<List><Item>a</Item><Item>b</Item><Other>x</Other><Item>c</Item></List>";
        let root = XmlNode::parse(xml).unwrap();
        let items: Vec<&str> = root.children_named("Item").map(|n| n.text()).collect();
        assert_eq!(items, ["a", "b", "c"]);
    }

    #[test]
    fn test_parse_soap_body_finds_body() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
              <s:Header/>
              <s:Body>
                <tds:GetCapabilitiesResponse xmlns:tds="http://www.onvif.org/ver10/device/wsdl"/>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        assert_eq!(body.local_name, "Body");
        assert!(body.child("GetCapabilitiesResponse").is_some());
    }

    #[test]
    fn test_parse_soap_body_alternative_prefix() {
        let xml = r#"
            <SOAP-ENV:Envelope xmlns:SOAP-ENV="http://www.w3.org/2003/05/soap-envelope">
              <SOAP-ENV:Body>
                <tds:Response xmlns:tds="http://example.com"/>
              </SOAP-ENV:Body>
            </SOAP-ENV:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        assert_eq!(body.local_name, "Body");
    }

    #[test]
    fn test_parse_soap_body_missing_returns_err() {
        let result = parse_soap_body(r#"<NotAnEnvelope/>"#);
        assert!(matches!(result, Err(SoapError::MissingBody)));
    }

    #[test]
    fn test_find_response_ok() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
              <s:Body>
                <tds:GetCapabilitiesResponse xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
                  <tds:Capabilities/>
                </tds:GetCapabilitiesResponse>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        let resp = find_response(&body, "GetCapabilitiesResponse").unwrap();
        assert_eq!(resp.local_name, "GetCapabilitiesResponse");
        assert!(resp.child("Capabilities").is_some());
    }

    #[test]
    fn test_find_response_soap12_fault() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
              <s:Body>
                <s:Fault>
                  <s:Code><s:Value>s:Sender</s:Value></s:Code>
                  <s:Reason><s:Text xml:lang="en">Sender not Authorized</s:Text></s:Reason>
                </s:Fault>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        let err = find_response(&body, "GetCapabilitiesResponse").unwrap_err();
        assert!(matches!(
            err,
            SoapError::Fault { ref code, ref reason }
            if code == "s:Sender" && reason == "Sender not Authorized"
        ));
    }

    #[test]
    fn test_find_response_soap11_fault() {
        let xml = r#"
            <s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
              <s:Body>
                <s:Fault>
                  <faultcode>s:Client</faultcode>
                  <faultstring>Access Denied</faultstring>
                </s:Fault>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        let err = find_response(&body, "SomeResponse").unwrap_err();
        assert!(matches!(
            err,
            SoapError::Fault { ref code, ref reason }
            if code == "s:Client" && reason == "Access Denied"
        ));
    }

    #[test]
    fn test_find_response_unexpected_tag() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
              <s:Body><tds:WrongResponse/></s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        let err = find_response(&body, "ExpectedResponse").unwrap_err();
        assert!(matches!(
            err,
            SoapError::UnexpectedResponse(ref t) if t == "ExpectedResponse"
        ));
    }

    #[test]
    fn test_real_capabilities_response() {
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                        xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                        xmlns:tt="http://www.onvif.org/ver10/schema">
              <s:Body>
                <tds:GetCapabilitiesResponse>
                  <tds:Capabilities>
                    <tt:Device>
                      <tt:XAddr>http://192.168.1.100/onvif/device_service</tt:XAddr>
                    </tt:Device>
                    <tt:Media>
                      <tt:XAddr>http://192.168.1.100/onvif/media_service</tt:XAddr>
                    </tt:Media>
                  </tds:Capabilities>
                </tds:GetCapabilitiesResponse>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        let resp = find_response(&body, "GetCapabilitiesResponse").unwrap();
        let caps = resp.child("Capabilities").unwrap();

        assert_eq!(
            caps.path(&["Device", "XAddr"]).unwrap().text(),
            "http://192.168.1.100/onvif/device_service"
        );
        assert_eq!(
            caps.path(&["Media", "XAddr"]).unwrap().text(),
            "http://192.168.1.100/onvif/media_service"
        );
    }

    #[test]
    fn test_parse_soap_body_with_header_before_body() {
        // Verify swap_remove correctly extracts Body regardless of child order
        let xml = r#"
            <s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
              <s:Header><wsa:Action xmlns:wsa="http://www.w3.org/2005/08/addressing">urn:test</wsa:Action></s:Header>
              <s:Body>
                <tds:Response xmlns:tds="http://example.com"><tds:Value>42</tds:Value></tds:Response>
              </s:Body>
            </s:Envelope>"#;

        let body = parse_soap_body(xml).unwrap();
        assert_eq!(body.local_name, "Body");
        let resp = body.child("Response").unwrap();
        assert_eq!(resp.child("Value").unwrap().text(), "42");
    }
}
