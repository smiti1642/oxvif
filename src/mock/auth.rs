//! WS-Security PasswordDigest authentication for the mock server.
//!
//! Validates the `<wsse:Security>` header in incoming SOAP requests.
//! Scoped PasswordDigest checking with bounded freshness and nonce replay
//! protection. User-level authorization and full WS-Security are not modeled.
//! Only `GetSystemDateAndTime` is exempt (ONVIF spec requires it to be
//! unauthenticated so clients can sync their clock before authenticating).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::mock::request::{Node, Request};
use crate::mock::state::SharedState;

const WSSE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const WSU: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
const DIGEST_TYPE: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest";
const BASE64_TYPE: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary";
const INVALID_HEADER: &str = "Invalid WS-Security header";
const MAX_AGE: u64 = 300;
const FUTURE_SKEW: u64 = 60;
const MAX_NONCE_BYTES: usize = 256;
const MAX_NONCES: usize = 4096;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReplayCache {
    nonces: BTreeMap<Vec<u8>, u64>,
    accepted_clock: u64,
    #[cfg(test)]
    now_override: Option<u64>,
}

impl ReplayCache {
    fn now(&self) -> u64 {
        #[cfg(test)]
        if let Some(now) = self.now_override {
            return now.max(self.accepted_clock);
        }
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .max(self.accepted_clock)
    }

    fn admit(&mut self, nonce: Vec<u8>, created: u64) -> Result<(), String> {
        let now = self.now();
        if created < now.saturating_sub(MAX_AGE) {
            return Err("Stale Created timestamp".into());
        }
        if created > now.saturating_add(FUTURE_SKEW) {
            return Err("Future Created timestamp".into());
        }
        if self.nonces.get(&nonce).is_some_and(|end| *end >= now) {
            return Err("Reused WS-Security nonce".into());
        }
        if self.nonces.values().filter(|end| **end >= now).count() >= MAX_NONCES {
            return Err("WS-Security nonce cache capacity exceeded".into());
        }
        // All refusals precede mutation; never evict a live nonce for capacity.
        self.nonces.retain(|_, end| *end >= now);
        self.nonces
            .insert(nonce, now.max(created).saturating_add(MAX_AGE));
        self.accepted_clock = now;
        Ok(())
    }
}

fn created_seconds(value: &str) -> Option<u64> {
    let value = value.trim_matches([' ', '\t', '\r', '\n']);
    if value.len() != 20 || !value.is_ascii() {
        return None;
    }
    if &value[4..5] != "-"
        || &value[7..8] != "-"
        || &value[10..11] != "T"
        || &value[13..14] != ":"
        || &value[16..17] != ":"
        || &value[19..] != "Z"
    {
        return None;
    }
    let y = value[..4].parse::<i32>().ok()?;
    let m = value[5..7].parse::<i32>().ok()?;
    let d = value[8..10].parse::<i32>().ok()?;
    let h = value[11..13].parse::<i32>().ok()?;
    let min = value[14..16].parse::<i32>().ok()?;
    let sec = value[17..19].parse::<i32>().ok()?;
    if !(1970..=9999).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=31).contains(&d)
        || !(0..24).contains(&h)
        || !(0..60).contains(&min)
        || !(0..60).contains(&sec)
    {
        return None;
    }
    let time = crate::types::civil_to_unix(y, m, d, h, min, sec);
    (crate::soap::security::unix_secs_to_iso8601(time) == value).then_some(time as u64)
}

/// SOAP actions that do NOT require authentication.
const AUTH_EXEMPT: &[&str] = &["http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime"];

/// Check if the given action requires authentication.
pub fn requires_auth(action: &str) -> bool {
    !AUTH_EXEMPT.contains(&action)
}

/// Validate scoped WS-Security credentials in the SOAP Header against the live
/// user table. Each user has their own password; any configured user
/// that produces a matching digest, fresh timestamp and unused nonce passes.
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
    if nonce_raw.len() > MAX_NONCE_BYTES {
        return Err("WS-Security nonce exceeds mock limit".into());
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

    if supplied.as_slice() != expected.as_slice() {
        return Err("Password digest mismatch".into());
    }
    let created = created_seconds(created).ok_or("Invalid Created timestamp")?;
    state
        .auth_replay
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .admit(nonce_raw, created)
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
        let state = MockState::for_tests();
        state.auth_replay.lock().unwrap().now_override = created_seconds("2026-04-15T00:00:00Z");
        state
    }

    #[test]
    fn freshness_boundaries_and_rejections_preserve_cache() {
        let now = 1_704_067_200;
        for delta in [-301i64, -300, 0, 60, 61] {
            let mut cache = ReplayCache {
                now_override: Some(now),
                ..Default::default()
            };
            let before = cache.clone();
            let result = cache.admit(vec![1], (now as i64 + delta) as u64);
            if (-300..=60).contains(&delta) {
                assert_eq!(result, Ok(()));
                let committed = cache.clone();
                assert_eq!(
                    cache.admit(vec![1], (now as i64 + delta) as u64),
                    Err("Reused WS-Security nonce".into())
                );
                assert_eq!(cache, committed);
            } else {
                assert_eq!(
                    result,
                    Err(if delta < 0 {
                        "Stale Created timestamp"
                    } else {
                        "Future Created timestamp"
                    }
                    .into())
                );
                assert_eq!(cache, before);
            }
        }
    }

    #[test]
    fn future_nonce_retention_and_clock_rollback_are_fail_closed() {
        let now = 1_704_067_200;
        let mut cache = ReplayCache {
            now_override: Some(now),
            ..Default::default()
        };
        cache.admit(vec![1], now + 60).unwrap();
        cache.now_override = Some(now + 360);
        assert_eq!(
            cache.admit(vec![1], now + 60),
            Err("Reused WS-Security nonce".into())
        );
        cache.now_override = Some(now + 361);
        cache.admit(vec![2], now + 361).unwrap();
        assert!(!cache.nonces.contains_key(&vec![1]));
        cache.now_override = Some(now);
        let before = cache.clone();
        assert_eq!(
            cache.admit(vec![1], now + 60),
            Err("Stale Created timestamp".into())
        );
        assert_eq!(cache, before);
    }

    #[test]
    fn capacity_preserves_live_nonces_and_recovers_after_expiry() {
        let now = 1_704_067_200;
        let mut cache = ReplayCache {
            now_override: Some(now),
            ..Default::default()
        };
        for i in 0..MAX_NONCES {
            cache.admit(i.to_be_bytes().to_vec(), now).unwrap();
        }
        let before = cache.clone();
        assert_eq!(
            cache.admit(vec![99], now),
            Err("WS-Security nonce cache capacity exceeded".into())
        );
        assert_eq!(cache, before);
        cache.now_override = Some(now + 301);
        cache.admit(vec![99], now + 301).unwrap();
        assert_eq!(cache.nonces.len(), 1);
    }

    #[test]
    fn timestamp_policy_checks_calendar_and_preserves_digest_whitespace() {
        assert_eq!(
            created_seconds(" \t2024-02-29T00:00:00Z\r\n"),
            Some(1_709_164_800)
        );
        for value in [
            "2023-02-29T00:00:00Z",
            "2024-04-31T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:00:60Z",
            "2024-01-01T00:00:00.1Z",
            "2024-01-01T00:00:00+00:00",
            "北024-01-01T00:00:00Z",
        ] {
            assert_eq!(created_seconds(value), None, "{value}");
        }
        let state = new_state();
        let xml = build_digest_body(
            "admin",
            "admin",
            " 2026-04-15T00:00:00Z ",
            b"whitespace-nonce",
        );
        assert_eq!(validate_ws_security(&xml, &state), Ok(()));
    }

    #[test]
    fn failed_credentials_never_reserve_a_nonce_and_replay_is_cross_user() {
        let state = new_state();
        let created = "2026-04-15T00:00:00Z";
        let before = state.auth_replay.lock().unwrap().clone();
        let bad = build_digest_body("admin", "wrong", created, b"same-nonce");
        assert_eq!(
            validate_ws_security(&bad, &state),
            Err("Password digest mismatch".into())
        );
        assert_eq!(*state.auth_replay.lock().unwrap(), before);
        let good = build_digest_body("admin", "admin", created, b"same-nonce");
        validate_ws_security(&good, &state).unwrap();
        let other_user = build_digest_body("operator", "operator", created, b"same-nonce");
        assert_eq!(
            validate_ws_security(&other_user, &state),
            Err("Reused WS-Security nonce".into())
        );
        assert_eq!(validate_ws_security(&good, &new_state()), Ok(()));
        let before = state.auth_replay.lock().unwrap().clone();
        let oversized = build_digest_body("admin", "admin", created, &[1; MAX_NONCE_BYTES + 1]);
        assert_eq!(
            validate_ws_security(&oversized, &state),
            Err("WS-Security nonce exceeds mock limit".into())
        );
        let invalid_time = build_digest_body("admin", "admin", "not-a-time", b"invalid-time");
        assert_eq!(
            validate_ws_security(&invalid_time, &state),
            Err("Invalid Created timestamp".into())
        );
        assert_eq!(*state.auth_replay.lock().unwrap(), before);
    }

    #[test]
    fn concurrent_replay_admission_accepts_exactly_one() {
        let state = std::sync::Arc::new(new_state());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let mut threads = Vec::new();
        for _ in 0..16 {
            let state = state.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                let body = build_digest_body(
                    "admin",
                    "admin",
                    "2026-04-15T00:00:00Z",
                    b"concurrent-nonce",
                );
                barrier.wait();
                validate_ws_security(&body, &state)
            }));
        }
        let results = threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| **r == Err("Reused WS-Security nonce".into()))
                .count(),
            15
        );
        assert_eq!(state.auth_replay.lock().unwrap().nonces.len(), 1);
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
