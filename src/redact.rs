//! Strip credentials out of SOAP exchanges before they are written to disk.
//!
//! Three recorders capture live traffic — [`crate::fixtures::CapturingTransport`],
//! `health::capture` and `metamorph::FixtureStore` — and every one of them has
//! the same problem: an authenticated ONVIF request carries a WS-Security
//! `UsernameToken`, and a `GetStreamUri` response can carry
//! `rtsp://user:pass@host/…`. Whatever lands on disk tends to end up in a bug
//! report or a git repository.
//!
//! The two transforms live here so all three share one implementation and one
//! set of tests. Crate-internal: the recorders apply them, callers do not.
//!
//! Compiled only for the features that have a recorder (`mock` covers
//! `metamorph`, which enables it).

/// Blank the text of the WS-Security `Password` and `Nonce` elements in a
/// request envelope oxvif emitted, so a captured request can't be used to
/// recover the credential. Targets the exact tags oxvif writes
/// (`wsse:Password` / `wsse:Nonce`, see [`crate::soap::WsSecurityToken`]); all
/// other content — including `Username` and `Created` — is left intact so the
/// exchange stays useful for debugging.
pub(crate) fn redact_credentials(xml: &str) -> String {
    let mut out = xml.to_string();
    for (open, close) in [
        ("<wsse:Password", "</wsse:Password>"),
        ("<wsse:Nonce", "</wsse:Nonce>"),
    ] {
        out = blank_between(&out, open, close);
    }
    out
}

/// Replace the text between every `open`…`>` and its following `close` with
/// `[redacted]`, preserving both tags. `open` is matched up to the first `>`
/// (tolerating attributes on the open tag).
fn blank_between(xml: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(op) = rest.find(open) {
        let Some(gt) = rest[op..].find('>') else {
            break;
        };
        let open_end = op + gt + 1; // just past the open tag's '>'
        let Some(cl_rel) = rest[open_end..].find(close) else {
            break;
        };
        let close_abs = open_end + cl_rel;
        out.push_str(&rest[..open_end]);
        out.push_str("[redacted]");
        out.push_str(close);
        rest = &rest[close_abs + close.len()..];
    }
    out.push_str(rest);
    out
}

/// Strip `user:pass@` credential userinfo from every URL in `xml` (e.g. a
/// `GetStreamUri` response's `rtsp://user:pass@host/…` → `rtsp://host/…`), so no
/// stream / snapshot credential lands on disk. Targets the `scheme://userinfo@`
/// form where the userinfo contains a `:` — a user/password pair; a bare
/// `user@host` (no password) is left alone. The replayed URI then carries no
/// credential, which is the correct shape (RTSP auth is negotiated separately).
///
/// The module is compiled for `mock` **or** `health`, but only the recorders use
/// this one (`fixtures.rs` and `metamorph/fixture.rs`, both `mock`); `health`
/// needs only [`redact_credentials`]. Without the gate this is dead code under
/// `--features health` alone — a warning invisible to the quality gate, which
/// only lints `--all-features` and no-features.
#[cfg(feature = "mock")]
pub(crate) fn scrub_url_userinfo(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let bytes = xml.as_bytes();
    let mut i = 0;
    while i < xml.len() {
        if xml[i..].starts_with("://") {
            out.push_str("://");
            i += 3;
            // Scan a userinfo candidate up to '@' or a URL delimiter.
            let start = i;
            let mut j = i;
            let mut saw_colon = false;
            let mut at = None;
            while j < xml.len() {
                match bytes[j] {
                    b'@' => {
                        at = Some(j);
                        break;
                    }
                    b'/' | b'?' | b'#' | b'<' | b'>' | b'"' | b'\'' | b' ' | b'\t' | b'\r'
                    | b'\n' => break,
                    b':' => {
                        saw_colon = true;
                        j += 1;
                    }
                    b if b.is_ascii() => j += 1,
                    // Non-ASCII byte: not URL userinfo — stop (keeps `j` on a
                    // char boundary, since every prior byte was ASCII).
                    _ => break,
                }
            }
            match (at, saw_colon) {
                // `scheme://user:pass@…` → drop the userinfo and the '@'.
                (Some(at_pos), true) => i = at_pos + 1,
                // No credential pair — keep the scanned segment verbatim.
                _ => {
                    out.push_str(&xml[start..j]);
                    i = j;
                }
            }
        } else {
            let ch = xml[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_password_and_nonce_but_keeps_username() {
        let req = "<wsse:Security><wsse:UsernameToken>\
             <wsse:Username>admin</wsse:Username>\
             <wsse:Password Type=\"...#PasswordDigest\">SECRETDIGEST==</wsse:Password>\
             <wsse:Nonce EncodingType=\"...Base64Binary\">SECRETNONCE==</wsse:Nonce>\
             <wsu:Created>2026-07-14T00:00:00Z</wsu:Created>\
           </wsse:UsernameToken></wsse:Security>";
        let out = redact_credentials(req);
        assert!(!out.contains("SECRETDIGEST=="), "password leaked: {out}");
        assert!(!out.contains("SECRETNONCE=="), "nonce leaked: {out}");
        assert!(out.contains(">[redacted]</wsse:Password>"));
        assert!(out.contains(">[redacted]</wsse:Nonce>"));
        // Non-secret context is preserved.
        assert!(out.contains("<wsse:Username>admin</wsse:Username>"));
        assert!(out.contains("2026-07-14T00:00:00Z"));
    }

    #[test]
    #[cfg(feature = "mock")]
    fn scrub_url_userinfo_targets_only_credential_pairs() {
        // A user:password pair is stripped, host/path kept.
        assert_eq!(scrub_url_userinfo("rtsp://u:p@h/x"), "rtsp://h/x");
        // A bare userinfo (no password) is left alone.
        assert_eq!(
            scrub_url_userinfo("http://user@host/x"),
            "http://user@host/x"
        );
        // A host:port colon is not mistaken for userinfo.
        assert_eq!(scrub_url_userinfo("http://host:554/x"), "http://host:554/x");
        // Surrounding markup is preserved; only the pair is removed.
        assert_eq!(
            scrub_url_userinfo("<Uri>rtsp://a:b@h:554/s</Uri>"),
            "<Uri>rtsp://h:554/s</Uri>"
        );
    }
}
