//! WS-Security PasswordDigest authentication for the mock server.
//!
//! Validates the `<wsse:Security>` header in incoming SOAP requests.
//! Scoped PasswordDigest checking only: no freshness, nonce reuse prevention,
//! user-level authorization or full WS-Security processing is implemented.
//! Only `GetSystemDateAndTime` is exempt (ONVIF spec requires it to be
//! unauthenticated so clients can sync their clock before authenticating).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};

use crate::mock::request::{Node, Request};
use crate::mock::state::SharedState;

const WSSE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const WSU: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
const DIGEST_TYPE: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest";
const BASE64_TYPE: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary";
const INVALID_HEADER: &str = "Invalid WS-Security header";

/// SOAP actions that do NOT require authentication.
const AUTH_EXEMPT: &[&str] = &["http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime"];

/// Check if the given action requires authentication.
pub fn requires_auth(action: &str) -> bool {
    !AUTH_EXEMPT.contains(&action)
}

/// Validate scoped WS-Security credentials in the SOAP Header against the live
/// user table. Each user has their own password; any configured user
/// that produces a matching digest passes.
///
/// Returns `Ok(())` if valid, `Err(reason)` if invalid.
pub fn validate_ws_security(body: &str, state: &SharedState) -> Result<(), String> {
    let request = Request::parse(body).map_err(|_| INVALID_HEADER)?;
    let header = request
        .header()
        .map_err(|_| INVALID_HEADER)?
        .ok_or("Missing Username")?;
    let security = header
        .child(WSSE, "Security")
        .map_err(|_| INVALID_HEADER)?
        .ok_or("Missing Username")?;
    if let Some(role) = security.attribute("http://www.w3.org/2003/05/soap-envelope", "role")
        && role != "http://www.w3.org/2003/05/soap-envelope/role/ultimateReceiver"
    {
        return Err("Unsupported WS-Security role".into());
    }
    let token = security
        .child(WSSE, "UsernameToken")
        .map_err(|_| INVALID_HEADER)?
        .ok_or("Missing Username")?;
    let username = field(token, WSSE, "Username", "Missing Username")?;
    let digest_b64 = field(token, WSSE, "Password", "Missing Password digest")?;
    let password_node = token
        .child(WSSE, "Password")
        .map_err(|_| INVALID_HEADER)?
        .ok_or("Missing Password digest")?;
    if password_node.attribute("", "Type") != Some(DIGEST_TYPE) {
        return Err("Unsupported WS-Security password type".into());
    }
    let nonce_b64 = field(token, WSSE, "Nonce", "Missing Nonce")?;
    let nonce_node = token
        .child(WSSE, "Nonce")
        .map_err(|_| INVALID_HEADER)?
        .ok_or("Missing Nonce")?;
    if nonce_node
        .attribute("", "EncodingType")
        .is_some_and(|kind| kind != BASE64_TYPE)
    {
        return Err("Unsupported nonce encoding".into());
    }
    let created = field(token, WSU, "Created", "Missing Created timestamp")?;

    let nonce_raw = STANDARD
        .decode(base64_text(nonce_b64))
        .map_err(|_| "Invalid nonce base64")?;
    if nonce_raw.is_empty() {
        return Err("Invalid nonce base64".into());
    }

    let password = {
        let s = state.read();
        s.users
            .iter()
            .find(|u| u.username == username)
            .map(|u| u.password.clone())
            .ok_or("Unknown user")?
    };

    // Recompute: SHA-1(nonce_raw || created || password)
    let mut h = Sha1::new();
    h.update(&nonce_raw);
    h.update(created.as_bytes());
    h.update(password.as_bytes());
    let expected = h.finalize();
    let supplied = STANDARD
        .decode(base64_text(digest_b64))
        .map_err(|_| "Invalid password digest base64")?;

    if supplied.as_slice() == expected.as_slice() {
        Ok(())
    } else {
        Err("Password digest mismatch".into())
    }
}

fn field<'a>(
    token: &'a Node,
    ns: &str,
    name: &str,
    missing: &'static str,
) -> Result<&'a str, String> {
    let value = token
        .child(ns, name)
        .map_err(|_| INVALID_HEADER)?
        .ok_or(missing)?
        .scalar_text()
        .map_err(|_| INVALID_HEADER)?;
    if value.is_empty() {
        return Err(missing.into());
    }
    Ok(value)
}

fn base64_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'))
        .collect()
}

/// Generate a SOAP Fault for authentication failure.
pub fn auth_fault(reason: &str) -> String {
    use super::fault::{Code, FAILED_AUTHENTICATION, Fault};
    Fault::new(Code::Sender, &[FAILED_AUTHENTICATION], reason).to_xml()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::state::MockState;

    fn new_state() -> MockState {
        MockState::for_tests()
    }

    #[test]
    fn auth_fault_preserves_literal_reason_without_inserting_children() {
        let reason = "auth-952 <marker/> &amp; 北";
        let xml = auth_fault(reason);
        let body = crate::soap::parse_soap_body(&xml).unwrap();
        let text = body.path(&["Fault", "Reason", "Text"]).unwrap();
        assert_eq!(text.text(), reason);
        assert!(text.children.is_empty());
        assert_eq!(
            crate::soap::find_response(&body, "unused").unwrap_err(),
            crate::soap::SoapError::Fault {
                code: "s:Sender".into(),
                reason: reason.into(),
                subcode: Some("wsse:FailedAuthentication".into()),
                detail: None,
            }
        );
        #[cfg(feature = "health")]
        {
            let error =
                crate::OnvifError::Soap(crate::soap::find_response(&body, "unused").unwrap_err());
            let check = crate::health::CheckError::from(&error);
            assert_eq!(check.class, crate::health::ErrorClass::SoapFault);
            assert_eq!(check.fault_code.as_deref(), Some("s:Sender"));
            assert_eq!(check.subcode.as_deref(), Some("wsse:FailedAuthentication"));
            assert_eq!(check.reason, reason);
            assert!(check.is_auth());
        }
    }

    #[test]
    fn auth_fault_binds_the_existing_subcode_in_its_value_scope() {
        use quick_xml::{
            NsReader,
            events::Event,
            name::{QName, ResolveResult},
        };
        let xml = auth_fault("binding-429");
        let mut reader = NsReader::from_str(&xml);
        let mut checked = false;
        loop {
            match reader.read_event().unwrap() {
                Event::Text(text) if text.as_ref() == "wsse:FailedAuthentication" => {
                    let (resolved, local) = reader.resolver().resolve_element(QName(text.as_ref()));
                    assert_eq!(local.as_ref(), "FailedAuthentication");
                    assert!(
                        matches!(resolved, ResolveResult::Bound(ns) if ns.as_ref() ==
                        "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd"),
                        "authentication subcode has no correct binding: {resolved:?}"
                    );
                    checked = true;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert!(checked, "authentication subcode was not inspected");
    }

    fn build_digest_body(
        username: &str,
        password: &str,
        created: &str,
        nonce_raw: &[u8],
    ) -> String {
        let nonce_b64 = STANDARD.encode(nonce_raw);
        let mut h = Sha1::new();
        h.update(nonce_raw);
        h.update(created.as_bytes());
        h.update(password.as_bytes());
        let digest_b64 = STANDARD.encode(h.finalize());
        crate::soap::SoapEnvelope::new("<tds:GetDeviceInformation/>".into())
            .with_security(crate::soap::WsSecurityToken::from_parts(
                username,
                &digest_b64,
                &nonce_b64,
                created,
            ))
            .build()
    }

    #[test]
    fn exempt_action_does_not_require_auth() {
        assert!(!requires_auth(
            "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime"
        ));
    }

    #[test]
    fn normal_action_requires_auth() {
        assert!(requires_auth(
            "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation"
        ));
    }

    #[test]
    fn valid_digest_for_admin_passes() {
        let s = new_state();
        let body = build_digest_body(
            "admin",
            "admin",
            "2026-04-15T00:00:00Z",
            b"nonce_admin_20bytes!",
        );
        assert_eq!(validate_ws_security(&body, &s), Ok(()));
    }

    #[test]
    fn valid_digest_for_operator_passes() {
        // Per-user auth: operator's own password should authenticate them.
        let s = new_state();
        let body = build_digest_body(
            "operator",
            "operator",
            "2026-04-15T00:00:00Z",
            b"nonce_op_20bytes_x!!",
        );
        assert_eq!(validate_ws_security(&body, &s), Ok(()));
    }

    #[test]
    fn operator_cannot_use_admin_password() {
        let s = new_state();
        // Digest built with operator's *name* but admin's password.
        let body = build_digest_body(
            "operator",
            "admin",
            "2026-04-15T00:00:00Z",
            b"nonce_cross_20byts!!",
        );
        assert_eq!(
            validate_ws_security(&body, &s).unwrap_err(),
            "Password digest mismatch"
        );
    }

    #[test]
    fn wrong_password_fails() {
        let s = new_state();
        let body = build_digest_body(
            "admin",
            "wrong",
            "2026-04-15T00:00:00Z",
            b"test_nonce_20_bytes!",
        );
        assert_eq!(
            validate_ws_security(&body, &s).unwrap_err(),
            "Password digest mismatch"
        );
    }

    #[test]
    fn unknown_user_fails() {
        let s = new_state();
        let body = build_digest_body(
            "hacker",
            "unused",
            "2026-04-15T00:00:00Z",
            b"unknown-user-nonce",
        );
        assert_eq!(validate_ws_security(&body, &s).unwrap_err(), "Unknown user");
    }

    #[test]
    fn missing_credentials_fails() {
        let s = new_state();
        let body = crate::soap::SoapEnvelope::new("<tds:GetDeviceInformation/>".into()).build();
        assert_eq!(
            validate_ws_security(&body, &s).unwrap_err(),
            "Missing Username"
        );
    }

    #[test]
    fn created_user_can_authenticate() {
        let s = new_state();
        // Create a new user via the handler, then try to auth with their creds.
        let create_body = r#"<tds:CreateUsers><tds:User>
            <tt:Username>viewer</tt:Username>
            <tt:Password>viewerpw</tt:Password>
            <tt:UserLevel>User</tt:UserLevel>
          </tds:User></tds:CreateUsers>"#;
        crate::mock::services::device::handle_create_users(&s, create_body);

        let body = build_digest_body(
            "viewer",
            "viewerpw",
            "2026-04-15T00:00:00Z",
            b"viewer_nonce_20bytes",
        );
        assert_eq!(validate_ws_security(&body, &s), Ok(()));
    }
}
