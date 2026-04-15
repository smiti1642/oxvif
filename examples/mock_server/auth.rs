//! WS-Security PasswordDigest authentication for the mock server.
//!
//! Validates the `<wsse:Security>` header in incoming SOAP requests.
//! Only `GetSystemDateAndTime` is exempt (ONVIF spec requires it to be
//! unauthenticated so clients can sync their clock before authenticating).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};

use crate::xml_parse::extract_tag;

/// Credentials accepted by the mock server.
const MOCK_USERNAME: &str = "admin";
const MOCK_PASSWORD: &str = "admin";

/// SOAP actions that do NOT require authentication.
const AUTH_EXEMPT: &[&str] = &[
    "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime",
];

/// Check if the given action requires authentication.
pub fn requires_auth(action: &str) -> bool {
    !AUTH_EXEMPT.contains(&action)
}

/// Validate WS-Security credentials in the SOAP body.
///
/// Returns `Ok(())` if valid, `Err(reason)` if invalid.
pub fn validate_ws_security(body: &str) -> Result<(), String> {
    let username = extract_tag(body, "Username")
        .ok_or_else(|| "Missing Username".to_string())?;
    let digest_b64 = extract_tag(body, "Password")
        .ok_or_else(|| "Missing Password digest".to_string())?;
    let nonce_b64 = extract_tag(body, "Nonce")
        .ok_or_else(|| "Missing Nonce".to_string())?;
    let created = extract_tag(body, "Created")
        .ok_or_else(|| "Missing Created timestamp".to_string())?;

    if username != MOCK_USERNAME {
        return Err(format!("Unknown user: {username}"));
    }

    // Decode nonce from base64
    let nonce_raw = STANDARD
        .decode(&nonce_b64)
        .map_err(|e| format!("Invalid nonce base64: {e}"))?;

    // Recompute: SHA-1(nonce_raw || created || password)
    let mut h = Sha1::new();
    h.update(&nonce_raw);
    h.update(created.as_bytes());
    h.update(MOCK_PASSWORD.as_bytes());
    let expected = STANDARD.encode(h.finalize());

    if digest_b64 == expected {
        Ok(())
    } else {
        Err("Password digest mismatch".to_string())
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
    fn valid_digest_passes() {
        // Build a valid digest manually
        let nonce_raw = b"test_nonce_20_bytes!";
        let created = "2026-04-15T00:00:00Z";
        let nonce_b64 = STANDARD.encode(nonce_raw);

        let mut h = Sha1::new();
        h.update(nonce_raw);
        h.update(created.as_bytes());
        h.update(MOCK_PASSWORD.as_bytes());
        let digest_b64 = STANDARD.encode(h.finalize());

        let body = format!(
            r#"<wsse:Security>
                <wsse:UsernameToken>
                  <wsse:Username>{MOCK_USERNAME}</wsse:Username>
                  <wsse:Password>{digest_b64}</wsse:Password>
                  <wsse:Nonce>{nonce_b64}</wsse:Nonce>
                  <wsu:Created>{created}</wsu:Created>
                </wsse:UsernameToken>
              </wsse:Security>"#
        );

        assert!(validate_ws_security(&body).is_ok());
    }

    #[test]
    fn wrong_password_fails() {
        let nonce_raw = b"test_nonce_20_bytes!";
        let created = "2026-04-15T00:00:00Z";
        let nonce_b64 = STANDARD.encode(nonce_raw);

        // Compute digest with WRONG password
        let mut h = Sha1::new();
        h.update(nonce_raw);
        h.update(created.as_bytes());
        h.update(b"wrong_password");
        let digest_b64 = STANDARD.encode(h.finalize());

        let body = format!(
            r#"<wsse:Security>
                <wsse:UsernameToken>
                  <wsse:Username>{MOCK_USERNAME}</wsse:Username>
                  <wsse:Password>{digest_b64}</wsse:Password>
                  <wsse:Nonce>{nonce_b64}</wsse:Nonce>
                  <wsu:Created>{created}</wsu:Created>
                </wsse:UsernameToken>
              </wsse:Security>"#
        );

        assert!(validate_ws_security(&body).is_err());
    }

    #[test]
    fn wrong_username_fails() {
        let body = r#"<wsse:Username>hacker</wsse:Username>
                      <wsse:Password>x</wsse:Password>
                      <wsse:Nonce>x</wsse:Nonce>
                      <wsu:Created>x</wsu:Created>"#;
        let err = validate_ws_security(body).unwrap_err();
        assert!(err.contains("Unknown user"));
    }

    #[test]
    fn missing_credentials_fails() {
        let body = "<s:Body>no auth here</s:Body>";
        assert!(validate_ws_security(body).is_err());
    }
}
