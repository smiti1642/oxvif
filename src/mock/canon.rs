//! Canonicalise a SOAP message for fixture matching and diff (metamorph M1).
//!
//! Normalises — via the namespace-stripped [`XmlNode`](crate::soap::XmlNode)
//! tree, so element/attribute names are prefix-agnostic and `xmlns` declarations
//! drop out — then serialises to a deterministic string with attributes sorted
//! and volatile fields masked. The output is *not* valid XML; it is a stable
//! comparable form (hash it for a fixture key, diff two of them for drift).
//!
//! Masking has two classes (`docs/metamorph.md` D4):
//!
//! - **(a) transport ephemera** — `MessageID`, WS-Security nonce/password,
//!   timestamps, subscription-reference endpoints. They vary every exchange and
//!   never identify the request, so they are masked under **both** projections.
//!   The `Password`/`Nonce` entries mirror the fields the health capture redacts.
//! - **(b) semantic identifiers** — profile / configuration / reference tokens.
//!   They *do* identify the request, so they are **preserved** under `Masking::Key`
//!   (so `GetProfile(token=A)` and `(token=B)` hash apart) and masked only under
//!   `Masking::Value` (so a token echoed back in a response can't defeat matching).
//!
//! Pragmatic, not W3C C14N: prefixes are stripped to local names rather than
//! resolved to namespace URIs, so two *different* namespaces that reuse a local
//! name are treated as one — a non-issue for ONVIF SOAP.

// The `metamorph` feature's ReplayResponder is the production caller of
// `canonicalize` / `Masking`. With only `mock` enabled the module has no
// non-test caller, so silence dead_code just there; under `metamorph` (and the
// `--all-features` gate) real dead-code detection stays on.
#![cfg_attr(not(feature = "metamorph"), allow(dead_code))]

use crate::soap::XmlNode;

/// Placeholder substituted for every masked value.
const MASK: &str = "__MASKED__";

/// Which fields to mask when canonicalising. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Masking {
    /// Mask transport ephemera only (class a); keep semantic identifiers. The
    /// projection whose hash is a fixture lookup key.
    Key,
    /// Mask ephemera *and* identifiers (class a + b). The projection for comparing
    /// a replayed response against the recorded original.
    Value,
}

/// Element local-names whose text is transport ephemera (class a) — masked in
/// both projections. `Password`/`Nonce` mirror the health-capture redaction.
const EPHEMERAL_TEXT: &[&str] = &[
    "MessageID",
    "Nonce",
    "Password",
    "Created",
    "Expires",
    "TerminationTime",
    "CurrentTime",
    "Address", // subscription-reference / endpoint URLs
    "To",      // WS-Addressing destination endpoint (varies by device URL)
];

/// Attribute local-names that are transport ephemera (class a).
const EPHEMERAL_ATTR: &[&str] = &["UtcTime"];

/// Element local-names whose text is a semantic identifier (class b) — masked
/// under `Value`, preserved under `Key`.
const IDENTIFIER_TEXT: &[&str] = &[
    "ProfileToken",
    "ReferenceToken",
    "ConfigurationToken",
    "SourceToken",
];

/// Attribute local-names that are semantic identifiers (class b).
const IDENTIFIER_ATTR: &[&str] = &["token", "ReferenceToken", "CurrentToken"];

/// Canonicalise `xml` under `masking`. On unparseable input, falls back to the
/// whitespace-collapsed raw string (still deterministic, just un-normalised).
pub(crate) fn canonicalize(xml: &str, masking: Masking) -> String {
    match XmlNode::parse(xml) {
        Ok(root) => {
            let mut out = String::with_capacity(xml.len());
            write_node(&mut out, &root, masking);
            out
        }
        Err(_) => xml.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

fn mask_text(local: &str, masking: Masking) -> bool {
    EPHEMERAL_TEXT.contains(&local)
        || (masking == Masking::Value && IDENTIFIER_TEXT.contains(&local))
}

fn mask_attr(local: &str, masking: Masking) -> bool {
    EPHEMERAL_ATTR.contains(&local)
        || (masking == Masking::Value && IDENTIFIER_ATTR.contains(&local))
}

fn write_node(out: &mut String, node: &XmlNode, masking: Masking) {
    out.push('<');
    out.push_str(&node.local_name);

    // Attributes sorted by local name for a stable order.
    let mut attrs: Vec<(&String, &String)> = node.attrs.iter().collect();
    attrs.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(if mask_attr(k, masking) { MASK } else { v });
        out.push('"');
    }
    out.push('>');

    // Leaf text, whitespace-collapsed.
    if let Some(t) = &node.text {
        let collapsed = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            out.push_str(if mask_text(&node.local_name, masking) {
                MASK
            } else {
                &collapsed
            });
        }
    }

    // Children in document order.
    for child in &node.children {
        write_node(out, child, masking);
    }

    out.push_str("</");
    out.push_str(&node.local_name);
    out.push('>');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(xml: &str) -> String {
        canonicalize(xml, Masking::Key)
    }
    fn value(xml: &str) -> String {
        canonicalize(xml, Masking::Value)
    }

    #[test]
    fn value_masks_ephemera_so_timestamp_and_nonce_jitter_collapses() {
        let a = "<E><Created>2026-07-14T00:00:00Z</Created><Nonce>AAA==</Nonce><Body/></E>";
        let b = "<E><Created>1999-01-01T00:00:00Z</Created><Nonce>BBB==</Nonce><Body/></E>";
        assert_eq!(value(a), value(b));
    }

    #[test]
    fn key_masks_ephemera_too() {
        // Ephemera are masked in *both* projections, so a fresh MessageID must
        // not fragment the key.
        let a = "<E><MessageID>uuid:aaaa</MessageID><GetProfiles/></E>";
        let b = "<E><MessageID>uuid:bbbb</MessageID><GetProfiles/></E>";
        assert_eq!(key(a), key(b));
    }

    #[test]
    fn key_preserves_token_element_but_value_masks_it() {
        let a = "<GetProfile><ProfileToken>Profile_1</ProfileToken></GetProfile>";
        let b = "<GetProfile><ProfileToken>Profile_2</ProfileToken></GetProfile>";
        assert_ne!(key(a), key(b), "distinct tokens must yield distinct keys");
        assert_eq!(value(a), value(b), "token must be masked for value compare");
    }

    #[test]
    fn key_preserves_token_attr_but_value_masks_it() {
        let a = r#"<GetProfile token="P1"/>"#;
        let b = r#"<GetProfile token="P2"/>"#;
        assert_ne!(key(a), key(b));
        assert_eq!(value(a), value(b));
    }

    #[test]
    fn prefix_attr_order_and_whitespace_agnostic() {
        let a = r#"<s:Envelope xmlns:s="urn:x"><s:Body>  <trt:GetProfiles a="1" b="2"/></s:Body></s:Envelope>"#;
        let b = "<t:Envelope xmlns:t=\"urn:x\">\n  <t:Body><q:GetProfiles b=\"2\" a=\"1\"/></t:Body>\n</t:Envelope>";
        assert_eq!(key(a), key(b));
    }

    #[test]
    fn key_ignores_wsaddressing_to_endpoint() {
        // The device endpoint (wsa:To) varies between record time and replay
        // time, so it must not fragment the key.
        let a = "<E><Header><To>http://cam-a/onvif</To></Header><GetHostname/></E>";
        let b = "<E><Header><To>http://replay/onvif</To></Header><GetHostname/></E>";
        assert_eq!(key(a), key(b));
    }

    #[test]
    fn unparseable_input_falls_back_without_panicking() {
        let out = canonicalize("not <<< xml at %% all", Masking::Key);
        assert_eq!(out, "not <<< xml at %% all");
    }
}
