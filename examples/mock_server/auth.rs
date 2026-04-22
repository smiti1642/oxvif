//! WS-Security PasswordDigest authentication for the mock server.
//!
//! Validates the `<wsse:Security>` header in incoming SOAP requests.
//! Only `GetSystemDateAndTime` is exempt (ONVIF spec requires it to be
//! unauthenticated so clients can sync their clock before authenticating).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};

use crate::state::SharedState;
use crate::xml_parse::extract_tag;

/// SOAP actions that do NOT require authentication.
const AUTH_EXEMPT: &[&str] = &["http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime"];

/// Check if the given action requires authentication.
pub fn requires_auth(action: &str) -> bool {
    !AUTH_EXEMPT.contains(&action)
}

/// Validate WS-Security credentials in the SOAP body against the live
/// user table. Each user has their own password; any configured user
/// that produces a matching digest passes.
///
/// Returns `Ok(())` if valid, `Err(reason)` if invalid.
pub fn validate_ws_security(body: &str, state: &SharedState) -> Result<(), String> {
    let username = extract_tag(body, "Username").ok_or_else(|| "Missing Username".to_string())?;
    let digest_b64 =
        extract_tag(body, "Password").ok_or_else(|| "Missing Password digest".to_string())?;
    let nonce_b64 = extract_tag(body, "Nonce").ok_or_else(|| "Missing Nonce".to_string())?;
    let created =
        extract_tag(body, "Created").ok_or_else(|| "Missing Created timestamp".to_string())?;

    let nonce_raw = STANDARD
        .decode(&nonce_b64)
        .map_err(|e| format!("Invalid nonce base64: {e}"))?;

    let password = {
        let s = state.read();
        s.users
            .iter()
            .find(|u| u.username == username)
            .map(|u| u.password.clone())
            .ok_or_else(|| format!("Unknown user: {username}"))?
    };

    // Recompute: SHA-1(nonce_raw || created || password)
    let mut h = Sha1::new();
    h.update(&nonce_raw);
    h.update(created.as_bytes());
    h.update(password.as_bytes());
    let expected = STANDARD.encode(h.finalize());

    if digest_b64 == expected {
        Ok(())
    } else {
        Err(format!("Password digest mismatch for user {username}"))
    }
}

/// Generate a SOAP Fault for authentication failure.
pub fn auth_fault(reason: &str) -> String {
    crate::helpers::soap(
        "",
        &format!(
            r#"<s:Fault>
              <s:Code><s:Value>s:Sender</s:Value>
                <s:Subcode><s:Value>wsse:FailedAuthentication</s:Value></s:Subcode>
              </s:Code>
              <s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason>
            </s:Fault>"#
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PersistentState;

    fn new_state() -> PersistentState {
        PersistentState::for_tests()
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
        format!(
            r#"<wsse:Security>
                <wsse:UsernameToken>
                  <wsse:Username>{username}</wsse:Username>
                  <wsse:Password>{digest_b64}</wsse:Password>
                  <wsse:Nonce>{nonce_b64}</wsse:Nonce>
                  <wsu:Created>{created}</wsu:Created>
                </wsse:UsernameToken>
              </wsse:Security>"#
        )
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
        assert!(validate_ws_security(&body, &s).is_ok());
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
        assert!(validate_ws_security(&body, &s).is_ok());
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
        assert!(validate_ws_security(&body, &s).is_err());
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
        assert!(validate_ws_security(&body, &s).is_err());
    }

    #[test]
    fn unknown_user_fails() {
        let s = new_state();
        let body = r#"<wsse:Username>hacker</wsse:Username>
                      <wsse:Password>x</wsse:Password>
                      <wsse:Nonce>eA==</wsse:Nonce>
                      <wsu:Created>x</wsu:Created>"#;
        let err = validate_ws_security(body, &s).unwrap_err();
        assert!(err.contains("Unknown user"), "got: {err}");
    }

    #[test]
    fn missing_credentials_fails() {
        let s = new_state();
        let body = "<s:Body>no auth here</s:Body>";
        assert!(validate_ws_security(body, &s).is_err());
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
        crate::services::device::handle_create_users(&s, create_body);

        let body = build_digest_body(
            "viewer",
            "viewerpw",
            "2026-04-15T00:00:00Z",
            b"viewer_nonce_20bytes",
        );
        assert!(validate_ws_security(&body, &s).is_ok());
    }
}
