use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::soap::error::SoapError;

// ── 核心資料結構 ────────────────────────────────────────────────────────────────

/// Namespace-aware 的 XML 節點。
///
/// local_name 已剝除 prefix（`tds:GetCapabilities` → `GetCapabilities`），
/// 因此上層程式碼完全不需要知道裝置用哪種 prefix。
#[derive(Debug, Clone, Default)]
pub struct XmlNode {
    pub local_name: String,
    /// 元素的文字內容（trimmed）
    pub text: Option<String>,
    /// 屬性，key 也是 local name（xmlns:* 宣告已過濾）
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlNode>,
}

// ── 導覽 ────────────────────────────────────────────────────────────────────────

impl XmlNode {
    /// 以 local name 找第一個直接子節點
    pub fn child(&self, local_name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|n| n.local_name == local_name)
    }

    /// 多層路徑存取：`node.path(&["Body", "GetCapabilitiesResponse"])`
    pub fn path(&self, segments: &[&str]) -> Option<&XmlNode> {
        segments.iter().try_fold(self, |n, seg| n.child(seg))
    }

    /// 文字內容，找不到時回傳 `""`（方便鏈式呼叫時不用 unwrap）
    pub fn text(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }

    /// 屬性值
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    /// 列舉所有同名的直接子節點（ONVIF 常有重複元素，例如多個 Profile）
    pub fn children_named<'a>(&'a self, local_name: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children
            .iter()
            .filter(move |n| n.local_name == local_name)
    }
}

// ── 解析 ────────────────────────────────────────────────────────────────────────

impl XmlNode {
    /// 從 XML 字串解析成節點樹。
    /// 使用堆疊模擬遞迴，不會 stack overflow。
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
                    // 自閉合標籤 <Foo/>
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
                    stack.last_mut().unwrap().children.push(finished);
                }

                Ok(Event::Text(ref e)) => {
                    if let Some(node) = stack.last_mut() {
                        let cow = e.unescape().unwrap_or_default();
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

                // Declaration, Comment, PI 等直接忽略
                _ => {}
            }
        }
    }

    fn from_bytes_start(e: &quick_xml::events::BytesStart<'_>) -> Self {
        let local_name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();

        let mut attrs = HashMap::new();
        for attr_result in e.attributes() {
            let Ok(attr) = attr_result else { continue };

            // 過濾 namespace 宣告（xmlns 與 xmlns:prefix）
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

// ── SOAP 專用輔助函式 ───────────────────────────────────────────────────────────

/// 解析 SOAP Envelope，回傳 `<s:Body>` 節點（忽略 prefix）
pub fn parse_soap_body(xml: &str) -> Result<XmlNode, SoapError> {
    let root = XmlNode::parse(xml)?;
    root.child("Body").cloned().ok_or(SoapError::MissingBody)
}

/// 在 Body 節點中找到預期的回應節點。
/// - 若 Body 含 `<s:Fault>` 則回傳 `Err(SoapError::Fault)`
/// - 若找不到 `expected_tag` 則回傳 `Err(SoapError::UnexpectedResponse)`
pub fn find_response<'a>(body: &'a XmlNode, expected_tag: &str) -> Result<&'a XmlNode, SoapError> {
    if let Some(fault) = body.child("Fault") {
        // SOAP 1.2: Code/Value + Reason/Text
        // SOAP 1.1: faultcode + faultstring（備援）
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

// ── 單元測試 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 基礎解析 ──────────────────────────────────────────────────────────────

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
        let child = root.child("Child").unwrap();
        assert_eq!(child.text(), "value");
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

    // ── Namespace prefix 剝除 ─────────────────────────────────────────────────

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
        // prefix 不同，但 local_name 都是乾淨的
        assert_eq!(root.local_name, "Envelope");
        assert!(root.child("Body").is_some());
        assert!(root.path(&["Body", "GetCapabilitiesResponse"]).is_some());
    }

    #[test]
    fn test_strips_prefix_from_attributes() {
        let xml =
            r#"<Profile tt:token="main_profile" xmlns:tt="http://www.onvif.org/ver10/schema"/>"#;
        let node = XmlNode::parse(xml).unwrap();
        // tt:token → key 是 "token"，xmlns:tt 已被過濾
        assert_eq!(node.attr("token"), Some("main_profile"));
        assert_eq!(node.attrs.get("xmlns"), None);
        assert_eq!(node.attrs.get("tt"), None);
    }

    #[test]
    fn test_xmlns_attributes_are_filtered() {
        let xml = r#"<Root xmlns="http://default.ns" xmlns:foo="http://foo.ns" bar="baz"/>"#;
        let node = XmlNode::parse(xml).unwrap();
        assert_eq!(node.attrs.len(), 1); // 只有 bar
        assert_eq!(node.attr("bar"), Some("baz"));
    }

    // ── 導覽 ─────────────────────────────────────────────────────────────────

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

    // ── SOAP 輔助 ─────────────────────────────────────────────────────────────

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
        // 某些裝置用 SOAP-ENV: 而非 s:
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
        let xml = r#"<NotAnEnvelope/>"#;
        let result = parse_soap_body(xml);
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
        // SOAP 1.1 格式備援（部分舊裝置）
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
        assert!(matches!(err, SoapError::UnexpectedResponse(ref t) if t == "ExpectedResponse"));
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
}
