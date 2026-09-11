//! Structured ordinary SOAP Fault serialization. Raw adversarial responses stay
//! outside this path. Operation-specific ONVIF hierarchies must be reviewed at
//! their call sites; this module does not infer them from flat error strings.

use super::helpers::soap;

#[derive(Clone, Copy)]
pub(super) enum Code {
    Sender,
    Receiver,
    VersionMismatch,
}

/// Trusted, compile-time fault names, not request-supplied names. The normal
/// mock vocabulary uses ASCII NCNames. QName-valued request parsing and arbitrary
/// injected/vendor QNames are separate contracts.
#[derive(Clone, Copy)]
pub(super) struct FaultQName {
    prefix: &'static str,
    namespace: &'static str,
    local: &'static str,
}

impl FaultQName {
    /// Use in a `const` declaration: an invalid definition fails compilation.
    const fn new(prefix: &'static str, namespace: &'static str, local: &'static str) -> Self {
        const fn ncname(s: &str) -> bool {
            let bytes = s.as_bytes();
            if bytes.is_empty() || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
                return false;
            }
            let mut i = 1;
            while i < bytes.len() {
                if !(bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b'.')) {
                    return false;
                }
                i += 1;
            }
            true
        }
        assert!(ncname(prefix) && ncname(local) && !namespace.is_empty());
        // A declaration on s:Value must not rebind its own element prefix or
        // the XML reserved prefixes. Other prefixes can be rebound per Value.
        assert!(!matches!(prefix.as_bytes(), b"s" | b"xml" | b"xmlns"));
        Self {
            prefix,
            namespace,
            local,
        }
    }
}

pub(super) const FAILED_AUTHENTICATION: FaultQName = FaultQName::new(
    "wsse",
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd",
    "FailedAuthentication",
);

const ONVIF_ERROR: &str = "http://www.onvif.org/ver10/error";
pub(super) const INVALID_ARG_VAL: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "InvalidArgVal");
pub(super) const ACTION: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "Action");
pub(super) const NO_PROFILE: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "NoProfile");
pub(super) const DELETION_OF_FIXED_PROFILE: FaultQName =
    FaultQName::new("ter", ONVIF_ERROR, "DeletionOfFixedProfile");
pub(super) const WELL_FORMED: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "WellFormed");
pub(super) const NAMESPACE: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "Namespace");
pub(super) const TAG_MISMATCH: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "TagMismatch");
pub(super) const INVALID_ARGS: FaultQName = FaultQName::new("ter", ONVIF_ERROR, "InvalidArgs");
// Project-specific limits/policy, not ONVIF hardware capacity claims.
pub(super) const MOCK_REQUEST_LIMIT: FaultQName =
    FaultQName::new("mock", "urn:oxvif:mock:error", "RequestLimit");
pub(super) const MOCK_REQUEST_POLICY: FaultQName =
    FaultQName::new("mock", "urn:oxvif:mock:error", "RequestPolicy");
pub(super) const MOCK_UNMODELED_EFFECT: FaultQName =
    FaultQName::new("mock", "urn:oxvif:mock:error", "UnmodeledEffect");

pub(super) struct Fault<'a> {
    code: Code,
    subcodes: &'a [FaultQName],
    reason: &'a str,
}

impl<'a> Fault<'a> {
    pub(super) fn new(code: Code, subcodes: &'a [FaultQName], reason: &'a str) -> Self {
        Self {
            code,
            subcodes,
            reason,
        }
    }

    pub(super) fn to_xml(&self) -> String {
        let code = match self.code {
            Code::Sender => "s:Sender",
            Code::Receiver => "s:Receiver",
            Code::VersionMismatch => "s:VersionMismatch",
        };
        let mut body = format!("<s:Fault><s:Code><s:Value>{code}</s:Value>");
        for name in self.subcodes {
            let FaultQName {
                prefix,
                namespace,
                local,
            } = name;
            body.push_str(&format!(
                "<s:Subcode><s:Value xmlns:{prefix}=\"{}\">{prefix}:{local}</s:Value>",
                crate::types::xml_escape(namespace),
            ));
        }
        for _ in self.subcodes {
            body.push_str("</s:Subcode>");
        }
        body.push_str("</s:Code><s:Reason><s:Text xml:lang=\"en\">");
        // XML-invalid control characters must not corrupt an ordinary fault.
        // Replace them, escape once, and retain literal CR via a character ref.
        let clean: String = self
            .reason
            .chars()
            .map(|c| match c {
                '\t'
                | '\n'
                | '\r'
                | '\u{20}'..='\u{d7ff}'
                | '\u{e000}'..='\u{fffd}'
                | '\u{10000}'..='\u{10ffff}' => c,
                _ => '\u{fffd}',
            })
            .collect();
        body.push_str(&crate::types::xml_escape(&clean));
        body.push_str("</s:Text></s:Reason></s:Fault>");
        soap("", &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soap::{SoapError, find_response, parse_soap_body};
    use quick_xml::{
        NsReader,
        events::Event,
        name::{QName, ResolveResult},
    };

    #[test]
    fn structured_fault_keeps_order_and_rebinds_each_qname_at_its_value() {
        const OUTER: FaultQName = FaultQName::new("f", "urn:outer", "Category");
        const INNER: FaultQName = FaultQName::new("f", "urn:inner", "Specific");
        let xml = Fault::new(Code::Sender, &[OUTER, INNER], "nested-918 <&> 北").to_xml();
        let mut reader = NsReader::from_str(&xml);
        let mut depth = 0;
        let mut values = Vec::new();
        loop {
            match reader.read_event().unwrap() {
                Event::Start(e) if e.local_name().as_ref() == "Subcode" => depth += 1,
                Event::End(e) if e.local_name().as_ref() == "Subcode" => depth -= 1,
                Event::Text(t)
                    if matches!(t.as_ref(), "s:Sender" | "f:Category" | "f:Specific") =>
                {
                    let (ns, local) = reader.resolver().resolve_element(QName(t.as_ref()));
                    let ResolveResult::Bound(ns) = ns else {
                        panic!("unbound QName {t:?}")
                    };
                    values.push((ns.as_ref().to_string(), local.as_ref().to_string(), depth));
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(
            values,
            [
                (
                    "http://www.w3.org/2003/05/soap-envelope".into(),
                    "Sender".into(),
                    0
                ),
                ("urn:outer".into(), "Category".into(), 1),
                ("urn:inner".into(), "Specific".into(), 2),
            ]
        );
        assert_eq!(depth, 0);
        // Preserve the existing public contract: subcode is the FIRST level,
        // not the deepest one; do not hide an API change inside mock work.
        let body = parse_soap_body(&xml).unwrap();
        assert_eq!(
            find_response(&body, "unused").unwrap_err(),
            SoapError::Fault {
                code: "s:Sender".into(),
                reason: "nested-918 <&> 北".into(),
                subcode: Some("f:Category".into()),
                detail: None,
            }
        );
        #[cfg(feature = "health")]
        {
            let error = crate::OnvifError::Soap(find_response(&body, "unused").unwrap_err());
            let check = crate::health::CheckError::from(&error);
            assert_eq!(check.class, crate::health::ErrorClass::SoapFault);
            assert_eq!(check.subcode.as_deref(), Some("f:Category"));
            assert_eq!(check.reason, "nested-918 <&> 北");
            assert!(!check.is_auth());
        }
    }

    #[test]
    fn structured_receiver_fault_replaces_illegal_xml_characters_not_valid_text() {
        let xml = Fault::new(Code::Receiver, &[], "literal &amp;\0\u{fffe} 北\rtext").to_xml();
        let body = parse_soap_body(&xml).unwrap();
        assert_eq!(
            find_response(&body, "unused").unwrap_err(),
            SoapError::Fault {
                code: "s:Receiver".into(),
                reason: "literal &amp;\u{fffd}\u{fffd} 北\rtext".into(),
                subcode: None,
                detail: None,
            }
        );
        assert!(!xml.contains('\0'));
        assert!(xml.contains("&#13;"));
    }
}
