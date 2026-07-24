//! Param-aware fixture store for Persona B (record / replay).
//!
//! A [`FixtureStore`] is the set of recorded SOAP exchanges for one device,
//! keyed by the **canonical, (a)-masked request** (see [`crate::mock::canon`]).
//! Keying on the canonicalised request — not the bare action name, as the older
//! [`FixtureTransport`](crate::FixtureTransport) does — is what lets
//! `GetProfile(token=A)` and `GetProfile(token=B)` coexist, while volatile
//! transport fields (MessageID, nonce, timestamps) never fragment the key.
//!
//! On disk it is a single `fixtures.json` per device directory
//! (`<vendor>-<model>/fixtures.json`); [`FixtureStore::load`] pulls the whole
//! set into memory and each [`lookup`](FixtureStore::lookup) is a hash hit.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::mock::canon::{Masking, canonicalize};

/// File name of the fixture set inside a device directory.
const FIXTURES_FILE: &str = "fixtures.json";

/// One recorded request/response exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    /// The canonical, (a)-masked request — the lookup key (also human-readable).
    pub key_canon: String,
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The request envelope as recorded, with WS-Security `Password`/`Nonce`
    /// blanked so no credential lands on disk.
    pub request_raw: String,
    /// The device's response envelope, stored verbatim for faithful replay.
    pub response_raw: String,
}

/// On-disk shape of a device's fixture set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    device: String,
    fixtures: Vec<Fixture>,
}

/// An in-memory set of [`Fixture`]s for one device, indexed by canonical key.
#[derive(Debug, Clone, Default)]
pub struct FixtureStore {
    device: String,
    fixtures: Vec<Fixture>,
    /// `key_canon` → index into `fixtures`.
    index: HashMap<String, usize>,
}

impl FixtureStore {
    /// An empty store labelled `device` (e.g. `"hikvision-ds2cd"`).
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            device: device.into(),
            fixtures: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Load `<dir>/fixtures.json` into memory.
    pub fn load(dir: impl AsRef<Path>) -> io::Result<Self> {
        let path = dir.as_ref().join(FIXTURES_FILE);
        let text = std::fs::read_to_string(&path)?;
        let on_disk: OnDisk = serde_json::from_str(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut store = Self::new(on_disk.device);
        for f in on_disk.fixtures {
            store.insert(f);
        }
        Ok(store)
    }

    /// Write the store to `<dir>/fixtures.json` (pretty-printed), creating the
    /// directory if needed.
    pub fn save(&self, dir: impl AsRef<Path>) -> io::Result<()> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let on_disk = OnDisk {
            device: self.device.clone(),
            fixtures: self.fixtures.clone(),
        };
        let text = serde_json::to_string_pretty(&on_disk)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(dir.join(FIXTURES_FILE), text)
    }

    /// Record one exchange: derive the canonical key from `request_raw`, blank
    /// the request's credentials, and upsert (last write wins per key).
    pub fn record(&mut self, action: &str, request_raw: &str, response_raw: &str) {
        let key_canon = canonicalize(request_raw, Masking::Key);
        self.insert(Fixture {
            key_canon,
            action: action.to_string(),
            request_raw: redact_credentials(request_raw),
            response_raw: response_raw.to_string(),
        });
    }

    /// Look up the exchange for a canonical request key.
    pub fn lookup(&self, key_canon: &str) -> Option<&Fixture> {
        self.index.get(key_canon).map(|&i| &self.fixtures[i])
    }

    /// The device label this set was recorded for.
    pub fn device(&self) -> &str {
        &self.device
    }

    /// The recorded exchanges, in insertion order — for rendering the clone's
    /// contents or driving analysis such as [`diff_against_synthetic`].
    ///
    /// [`diff_against_synthetic`]: FixtureStore::diff_against_synthetic
    pub fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }

    /// Number of stored exchanges.
    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    /// Whether the store holds no exchanges.
    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }

    fn insert(&mut self, f: Fixture) {
        if let Some(&i) = self.index.get(&f.key_canon) {
            self.fixtures[i] = f;
        } else {
            let i = self.fixtures.len();
            self.index.insert(f.key_canon.clone(), i);
            self.fixtures.push(f);
        }
    }
}

/// Blank the text of the WS-Security `Password` and `Nonce` elements in a
/// recorded request, so no credential lands on disk. The recorded request is
/// oxvif's own envelope, so the exact tags are `wsse:Password` / `wsse:Nonce`.
///
/// This deliberately mirrors `health::capture::redact_credentials`; it is
/// duplicated (a few lines) rather than shared so the `metamorph` feature does
/// not pull in `health`.
fn redact_credentials(xml: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const GET_PROFILE_A: &str =
        "<Envelope><Body><GetProfile><ProfileToken>A</ProfileToken></GetProfile></Body></Envelope>";
    const GET_PROFILE_B: &str =
        "<Envelope><Body><GetProfile><ProfileToken>B</ProfileToken></GetProfile></Body></Envelope>";

    #[test]
    fn param_aware_key_keeps_distinct_tokens_apart() {
        let mut store = FixtureStore::new("dev");
        store.record("act/GetProfile", GET_PROFILE_A, "<respA/>");
        store.record("act/GetProfile", GET_PROFILE_B, "<respB/>");
        assert_eq!(store.len(), 2, "distinct tokens must not collide");

        let key_a = canonicalize(GET_PROFILE_A, Masking::Key);
        let key_b = canonicalize(GET_PROFILE_B, Masking::Key);
        assert_eq!(store.lookup(&key_a).unwrap().response_raw, "<respA/>");
        assert_eq!(store.lookup(&key_b).unwrap().response_raw, "<respB/>");
    }

    #[test]
    fn ephemera_jitter_does_not_fragment_the_key() {
        let mut store = FixtureStore::new("dev");
        let req1 = "<Envelope><Header><MessageID>uuid:aaa</MessageID></Header>\
                    <Body><GetHostname/></Body></Envelope>";
        let req2 = "<Envelope><Header><MessageID>uuid:bbb</MessageID></Header>\
                    <Body><GetHostname/></Body></Envelope>";
        store.record("act/GetHostname", req1, "<r1/>");
        store.record("act/GetHostname", req2, "<r2/>");
        assert_eq!(
            store.len(),
            1,
            "a fresh MessageID must not create a new entry"
        );
        // Last write wins.
        let key = canonicalize(req2, Masking::Key);
        assert_eq!(store.lookup(&key).unwrap().response_raw, "<r2/>");
    }

    #[test]
    fn record_redacts_wssecurity_credentials() {
        let mut store = FixtureStore::new("dev");
        let req = "<Envelope><Header><wsse:Password Type=\"..#PasswordDigest\">SECRET==\
                   </wsse:Password><wsse:Nonce>NONCE==</wsse:Nonce></Header>\
                   <Body><GetHostname/></Body></Envelope>";
        store.record("act/GetHostname", req, "<r/>");
        let key = canonicalize(req, Masking::Key);
        let stored = &store.lookup(&key).unwrap().request_raw;
        assert!(!stored.contains("SECRET=="), "password leaked: {stored}");
        assert!(!stored.contains("NONCE=="), "nonce leaked: {stored}");
        assert!(stored.contains(">[redacted]</wsse:Password>"));
    }

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir(label: &str) -> std::path::PathBuf {
        let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!(
            "oxvif-metamorph-{}-{}-{label}",
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tmp_dir("roundtrip");
        let mut store = FixtureStore::new("acme-cam");
        store.record(
            "act/GetHostname",
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            "<r/>",
        );
        store.save(&dir).unwrap();

        let loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(loaded.device(), "acme-cam");
        assert_eq!(loaded.len(), 1);
        let key = canonicalize(
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            Masking::Key,
        );
        assert_eq!(loaded.lookup(&key).unwrap().response_raw, "<r/>");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
