use crate::soap::security::WsSecurityToken;
use std::fmt::Write;

// ── Namespace 清單（唯一需要維護的地方）─────────────────────────────────────────

/// 所有 ONVIF SOAP 請求共用的 namespace 宣告。
/// 要新增服務只需在這裡加一行。
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
];

// ── SoapEnvelope ───────────────────────────────────────────────────────────────

pub struct SoapEnvelope {
    security: Option<WsSecurityToken>,
    /// `<s:Body>` 內的 XML 內容（由呼叫者提供）
    body_content: String,
    /// `<wsa:To>` 位址（可選，部分裝置需要）
    wsa_to: Option<String>,
}

impl SoapEnvelope {
    pub fn new(body_content: String) -> Self {
        Self {
            security: None,
            body_content,
            wsa_to: None,
        }
    }

    pub fn with_security(mut self, token: WsSecurityToken) -> Self {
        self.security = Some(token);
        self
    }

    pub fn with_wsa_to(mut self, to: impl Into<String>) -> Self {
        self.wsa_to = Some(to.into());
        self
    }

    /// 建構完整的 SOAP Envelope XML 字串
    pub fn build(self) -> String {
        let mut out = String::with_capacity(2048);

        // XML 宣告
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);

        // <s:Envelope> 開頭，帶上所有 namespace
        out.push_str("<s:Envelope");
        for (prefix, uri) in NAMESPACES {
            write!(out, r#" xmlns:{prefix}="{uri}""#).unwrap();
        }
        out.push('>');

        // <s:Header>（只有 security 或 wsa:To 時才輸出）
        let has_header = self.security.is_some() || self.wsa_to.is_some();
        if has_header {
            out.push_str("<s:Header>");
            if let Some(to) = &self.wsa_to {
                write!(out, "<wsa:To>{to}</wsa:To>").unwrap();
            }
            if let Some(sec) = &self.security {
                sec.write_xml(&mut out);
            }
            out.push_str("</s:Header>");
        }

        // <s:Body>
        out.push_str("<s:Body>");
        out.push_str(&self.body_content);
        out.push_str("</s:Body>");

        out.push_str("</s:Envelope>");
        out
    }
}

// ── 單元測試 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soap::security::WsSecurityToken;

    // 解析輔助：用 quick-xml 驗證 XML 可被正確解析
    fn parse_ok(xml: &str) -> bool {
        crate::soap::xml::XmlNode::parse(xml).is_ok()
    }

    // ── 基本結構 ───────────────────────────────────────────────────────────────

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

    // ── Namespace ──────────────────────────────────────────────────────────────

    #[test]
    fn test_required_namespaces_present() {
        let xml = SoapEnvelope::new(String::new()).build();
        // 核心 namespace 必須存在
        assert!(xml.contains(r#"xmlns:s="http://www.w3.org/2003/05/soap-envelope""#));
        assert!(xml.contains(r#"xmlns:tt="http://www.onvif.org/ver10/schema""#));
        assert!(xml.contains(r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#));
        assert!(xml.contains(r#"xmlns:wsse=""#));
        assert!(xml.contains(r#"xmlns:wsu=""#));
    }

    // ── Header 輸出規則 ────────────────────────────────────────────────────────

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

    // ── 完整 round-trip 驗證 ───────────────────────────────────────────────────

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

        // XML 必須可被解析
        assert!(parse_ok(&envelope));

        // Body 必須可找到
        let body = parse_soap_body(&envelope).unwrap();
        assert_eq!(body.local_name, "Body");

        // request body 節點可被導覽
        let req = find_response(&body, "GetCapabilities").unwrap();
        assert_eq!(req.child("Category").unwrap().text(), "All");
    }
}
