//! Param-aware fixture store for Persona B (record / replay).
//!
//! A [`FixtureStore`] is the set of recorded SOAP exchanges for one device,
//! keyed by the pair **(SOAP action, canonical (a)-masked request)** (see
//! [`crate::mock::canon`]). Neither half suffices alone:
//!
//! - Keying on the canonicalised request — not the bare action name, as the
//!   older [`FixtureTransport`](crate::FixtureTransport) does — is what lets
//!   `GetProfile(token=A)` and `GetProfile(token=B)` coexist, while volatile
//!   transport fields (MessageID, nonce, timestamps) never fragment the key.
//! - Keying *also* on the action is what keeps two services apart. The
//!   canonicaliser strips prefixes to local names and masks the endpoint URL as
//!   ephemera, so Media1's `<trt:GetProfiles/>` and Media2's
//!   `<tr2:GetProfiles/>` share one `key_canon`; before 0.14 the second
//!   overwrote the first and reads of one service were answered from the other.
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
    /// The canonical, (a)-masked request — half of the lookup key (`action` is
    /// the other half), and human-readable.
    pub key_canon: String,
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The request envelope as recorded, with WS-Security `Password`/`Nonce`
    /// blanked and any `user:pass@` URL credential stripped, so nothing secret
    /// lands on disk.
    pub request_raw: String,
    /// The device's response envelope, stored for faithful replay — with any
    /// `user:pass@` URL credential (e.g. an `rtsp://` stream URI) stripped so no
    /// credential lands on disk.
    pub response_raw: String,
}

/// Progress of a per-fixture pass over a [`FixtureStore`] — one event per
/// recorded exchange examined.
///
/// Emitted by [`FixtureStore::verify_parsing_with_progress`] and
/// [`FixtureStore::diff_against_synthetic_with_progress`]. The unit of work is
/// one recorded [`Fixture`], identified by the same `(action, key_canon)` pair
/// the reports are keyed on, so a UI can highlight the row it is working on.
///
/// [`FixtureStore::verify_parsing_with_progress`]: FixtureStore::verify_parsing_with_progress
/// [`FixtureStore::diff_against_synthetic_with_progress`]: FixtureStore::diff_against_synthetic_with_progress
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureProgress {
    /// The SOAP action URI of the fixture just examined.
    pub action: String,
    /// The canonical, ephemera-masked request of the fixture just examined.
    pub key_canon: String,
    /// Fixtures completed so far, counting this one — `1..=total`.
    pub done: usize,
    /// Total fixtures in the store, known before the pass starts.
    pub total: usize,
}

/// On-disk shape of a device's fixture set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    device: String,
    fixtures: Vec<Fixture>,
}

/// An in-memory set of [`Fixture`]s for one device, indexed by
/// `(action, key_canon)`.
#[derive(Debug, Clone, Default)]
pub struct FixtureStore {
    device: String,
    fixtures: Vec<Fixture>,
    /// `(action, key_canon)` → index into `fixtures`. Both halves are needed:
    /// the key alone collides across services (Media1 and Media2 `GetProfiles`
    /// canonicalise identically), the action alone collides across params.
    index: HashMap<(String, String), usize>,
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

    /// Record one exchange: derive the canonical key from `request_raw`, scrub
    /// every credential (WS-Security `Password`/`Nonce` in the request, plus any
    /// `user:pass@` URL credential in either envelope), and upsert (last write
    /// wins per `(action, key_canon)` pair — so re-recording the same operation
    /// replaces it, while a different action sharing the key is kept apart).
    pub fn record(&mut self, action: &str, request_raw: &str, response_raw: &str) {
        let key_canon = canonicalize(request_raw, Masking::Key);
        self.insert(Fixture {
            key_canon,
            action: action.to_string(),
            request_raw: scrub_url_userinfo(&redact_credentials(request_raw)),
            response_raw: scrub_url_userinfo(response_raw),
        });
    }

    /// Look up the exchange one `action` answered for a canonical request key.
    ///
    /// Both halves are load-bearing. The canonicaliser keeps only local names
    /// and masks the endpoint URL as transport ephemera, so Media1's
    /// `<trt:GetProfiles/>` and Media2's `<tr2:GetProfiles/>` produce the *same*
    /// `key_canon`; only the action tells them apart.
    pub fn lookup(&self, action: &str, key_canon: &str) -> Option<&Fixture> {
        self.index
            .get(&(action.to_string(), key_canon.to_string()))
            .map(|&i| &self.fixtures[i])
    }

    /// Look up by canonical key alone, ignoring the action — the pre-0.14
    /// behaviour, kept for one release.
    ///
    /// **This function cannot be made correct, so it is not a rename of
    /// [`lookup`](Self::lookup).** Two different SOAP actions can share one
    /// canonical request body: Media1's `<trt:GetProfiles/>` and Media2's
    /// `<tr2:GetProfiles/>` canonicalise identically, because prefixes are
    /// stripped to local names and the endpoint URL is masked as transport
    /// ephemera. With no action to disambiguate with, this returns the *first*
    /// fixture matching the key in insertion order — which for a store holding
    /// both services may be the other service's exchange. That envelope parses
    /// successfully and yields wrong data, silently. Pass the action instead.
    #[deprecated(
        since = "0.14.0",
        note = "cannot disambiguate two actions that share one canonical request body (e.g. ver10 and ver20 GetProfiles), so it may silently return the wrong service's exchange; pass the action to `lookup(action, key_canon)`"
    )]
    pub fn lookup_by_key(&self, key_canon: &str) -> Option<&Fixture> {
        self.fixtures.iter().find(|f| f.key_canon == key_canon)
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
        let key = (f.action.clone(), f.key_canon.clone());
        if let Some(&i) = self.index.get(&key) {
            self.fixtures[i] = f;
        } else {
            let i = self.fixtures.len();
            self.index.insert(key, i);
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

/// Strip `user:pass@` credential userinfo from every URL in `xml` (e.g. a
/// `GetStreamUri` response's `rtsp://user:pass@host/…` → `rtsp://host/…`), so no
/// stream / snapshot credential lands on disk. Targets the `scheme://userinfo@`
/// form where the userinfo contains a `:` — a user/password pair; a bare
/// `user@host` (no password) is left alone. The replayed URI then carries no
/// credential, which is the correct shape (RTSP auth is negotiated separately).
fn scrub_url_userinfo(xml: &str) -> String {
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
        assert_eq!(
            store.lookup("act/GetProfile", &key_a).unwrap().response_raw,
            "<respA/>"
        );
        assert_eq!(
            store.lookup("act/GetProfile", &key_b).unwrap().response_raw,
            "<respB/>"
        );
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
        assert_eq!(
            store.lookup("act/GetHostname", &key).unwrap().response_raw,
            "<r2/>"
        );
    }

    #[test]
    fn record_redacts_wssecurity_credentials() {
        let mut store = FixtureStore::new("dev");
        let req = "<Envelope><Header><wsse:Password Type=\"..#PasswordDigest\">SECRET==\
                   </wsse:Password><wsse:Nonce>NONCE==</wsse:Nonce></Header>\
                   <Body><GetHostname/></Body></Envelope>";
        store.record("act/GetHostname", req, "<r/>");
        let key = canonicalize(req, Masking::Key);
        let stored = &store.lookup("act/GetHostname", &key).unwrap().request_raw;
        assert!(!stored.contains("SECRET=="), "password leaked: {stored}");
        assert!(!stored.contains("NONCE=="), "nonce leaked: {stored}");
        assert!(stored.contains(">[redacted]</wsse:Password>"));
    }

    #[test]
    fn record_scrubs_url_credentials_in_stream_uri() {
        let mut store = FixtureStore::new("dev");
        let req = "<Envelope><Body><GetStreamUri/></Body></Envelope>";
        let resp = "<Envelope><Body><GetStreamUriResponse><Uri>\
                    rtsp://admin:s3cr3t@10.0.0.5:554/Streaming/Channels/101\
                    </Uri></GetStreamUriResponse></Body></Envelope>";
        store.record("act/GetStreamUri", req, resp);
        let key = canonicalize(req, Masking::Key);
        let stored = &store.lookup("act/GetStreamUri", &key).unwrap().response_raw;
        assert!(!stored.contains("s3cr3t"), "password leaked: {stored}");
        assert!(!stored.contains("admin:"), "userinfo leaked: {stored}");
        assert!(
            stored.contains("rtsp://10.0.0.5:554/Streaming/Channels/101"),
            "host/path must survive: {stored}"
        );
    }

    #[test]
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
        assert_eq!(
            loaded.lookup("act/GetHostname", &key).unwrap().response_raw,
            "<r/>"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── NET 4: invariants Stage 3 must preserve ───────────────────────────────

    /// Ephemera de-duplication, checked on the whole `Fixture` rather than just
    /// the store length: two recordings of the same operation that differ only
    /// in `MessageID` must collapse onto one entry whose `action` and
    /// `key_canon` survive, with the later response winning.
    ///
    /// Complements `ephemera_jitter_does_not_fragment_the_key` above, which
    /// pins the length and last-write-wins but not the retained metadata.
    #[test]
    fn ephemera_dedup_keeps_one_fixture_with_its_action_and_key() {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        let req1 = "<Envelope><Header><MessageID>uuid:aaa</MessageID></Header>\
                    <Body><GetHostname/></Body></Envelope>";
        let req2 = "<Envelope><Header><MessageID>uuid:bbb</MessageID></Header>\
                    <Body><GetHostname/></Body></Envelope>";

        // Both requests canonicalise to the same key — that is *why* they merge.
        let key = canonicalize(req1, Masking::Key);
        assert_eq!(key, canonicalize(req2, Masking::Key));
        assert!(
            !key.contains("uuid:aaa") && !key.contains("uuid:bbb"),
            "the MessageID must be masked out of the key: {key}"
        );

        let mut store = FixtureStore::new("dev");
        store.record(ACTION, req1, "<r1/>");
        store.record(ACTION, req2, "<r2/>");

        assert_eq!(store.len(), 1);
        assert_eq!(store.fixtures().len(), 1);
        let f = &store.fixtures()[0];
        assert_eq!(f.action, ACTION);
        assert_eq!(f.key_canon, key);
        assert_eq!(f.response_raw, "<r2/>", "last write wins");
        assert!(
            f.request_raw.contains("uuid:bbb"),
            "the retained request is the later one: {}",
            f.request_raw
        );
        assert_eq!(store.lookup(ACTION, &key).unwrap().response_raw, "<r2/>");
    }

    /// `save` → `load` round-trip over several fixtures: device label, count,
    /// insertion order, and every fixture's `action` / `key_canon` /
    /// `request_raw` / `response_raw` come back byte-identical.
    #[test]
    fn save_then_load_preserves_every_fixture_and_its_action() {
        const GET_HOSTNAME: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        const GET_PROFILE: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfile";

        let dir = tmp_dir("actions");
        let mut store = FixtureStore::new("acme-cam");
        store.record(
            GET_HOSTNAME,
            "<Envelope><Body><GetHostname/></Body></Envelope>",
            "<HostnameResponse/>",
        );
        store.record(GET_PROFILE, GET_PROFILE_A, "<respA/>");
        store.record(GET_PROFILE, GET_PROFILE_B, "<respB/>");
        store.save(&dir).unwrap();

        let loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(loaded.device(), "acme-cam");
        assert_eq!(loaded.len(), 3);
        assert_eq!(
            loaded
                .fixtures()
                .iter()
                .map(|f| f.action.as_str())
                .collect::<Vec<_>>(),
            vec![GET_HOSTNAME, GET_PROFILE, GET_PROFILE],
            "actions survive the round-trip, in insertion order"
        );
        assert_eq!(
            loaded.fixtures().len(),
            store.fixtures().len(),
            "no fixture is dropped"
        );
        for (before, after) in store.fixtures().iter().zip(loaded.fixtures()) {
            assert_eq!(before.key_canon, after.key_canon);
            assert_eq!(before.action, after.action);
            assert_eq!(before.request_raw, after.request_raw);
            assert_eq!(before.response_raw, after.response_raw);
        }

        // The reloaded index still resolves each distinct request separately.
        assert_eq!(
            loaded
                .lookup(GET_PROFILE, &canonicalize(GET_PROFILE_A, Masking::Key))
                .unwrap()
                .response_raw,
            "<respA/>"
        );
        assert_eq!(
            loaded
                .lookup(GET_PROFILE, &canonicalize(GET_PROFILE_B, Masking::Key))
                .unwrap()
                .response_raw,
            "<respB/>"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── D1: the store key is (action, key_canon) ──────────────────────────────

    const MEDIA1_GET_PROFILES: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
    const MEDIA2_GET_PROFILES: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";

    /// Media1's `GetProfiles`, as `src/client/media.rs` builds it.
    const MEDIA1_PROFILES_REQ: &str = "<Envelope><Header><To>http://cam/onvif/Media</To></Header>\
                                       <Body><trt:GetProfiles/></Body></Envelope>";
    /// Media2's `GetProfiles`, as `src/client/media2.rs` builds it. Differs from
    /// Media1 only in the prefix and the endpoint — both of which the
    /// canonicaliser removes.
    const MEDIA2_PROFILES_REQ: &str = "<Envelope><Header><To>http://cam/onvif/Media2</To></Header>\
                                       <Body><tr2:GetProfiles/></Body></Envelope>";

    const MEDIA1_PROFILES_RESP: &str = "<Envelope><Body><trt:GetProfilesResponse>\
                                        <Profiles token=\"media1-profile\"/>\
                                        </trt:GetProfilesResponse></Body></Envelope>";
    const MEDIA2_PROFILES_RESP: &str = "<Envelope><Body><tr2:GetProfilesResponse>\
                                        <Profiles token=\"media2-profile\"/>\
                                        </tr2:GetProfilesResponse></Body></Envelope>";

    /// Premise guard for the three tests below: Media1's and Media2's
    /// `GetProfiles` really do canonicalise to one key, so the collision they
    /// exercise is *constructed*, not assumed. The canonicaliser keeps only
    /// local names (`trt:`/`tr2:` drop out) and masks `To` as transport
    /// ephemera, which removes the one field that distinguished the services.
    #[test]
    fn media1_and_media2_get_profiles_share_one_canonical_key() {
        let key1 = canonicalize(MEDIA1_PROFILES_REQ, Masking::Key);
        let key2 = canonicalize(MEDIA2_PROFILES_REQ, Masking::Key);
        assert_eq!(
            key1, key2,
            "the two services' requests must canonicalise identically"
        );
        assert!(
            !key1.contains("trt") && !key1.contains("tr2") && !key1.contains("cam"),
            "neither the prefix nor the endpoint survives into the key: {key1}"
        );
    }

    /// D1: two actions whose canonical request bodies match must be two
    /// fixtures, each resolving to *its own* response. Asserting only the length
    /// would pass on a store that kept two entries but crossed the responses.
    #[test]
    fn two_actions_sharing_a_canonical_body_are_two_fixtures() {
        let key = canonicalize(MEDIA1_PROFILES_REQ, Masking::Key);
        assert_eq!(key, canonicalize(MEDIA2_PROFILES_REQ, Masking::Key));

        let mut store = FixtureStore::new("dev");
        store.record(
            MEDIA1_GET_PROFILES,
            MEDIA1_PROFILES_REQ,
            MEDIA1_PROFILES_RESP,
        );
        store.record(
            MEDIA2_GET_PROFILES,
            MEDIA2_PROFILES_REQ,
            MEDIA2_PROFILES_RESP,
        );

        assert_eq!(
            store.len(),
            2,
            "one canonical key under two actions is two exchanges, not one"
        );
        assert_eq!(
            store
                .lookup(MEDIA1_GET_PROFILES, &key)
                .unwrap()
                .response_raw,
            MEDIA1_PROFILES_RESP,
            "Media1 must resolve to the Media1 envelope"
        );
        assert_eq!(
            store
                .lookup(MEDIA2_GET_PROFILES, &key)
                .unwrap()
                .response_raw,
            MEDIA2_PROFILES_RESP,
            "Media2 must resolve to the Media2 envelope"
        );
        assert!(
            store
                .lookup("http://www.onvif.org/ver10/device/wsdl/GetProfiles", &key)
                .is_none(),
            "an action that was never recorded must miss, even on a known key"
        );
    }

    /// The on-disk format needs no migration: `Fixture` already carries
    /// `action`, and `load` rebuilds the index by re-`insert`ing, so a store
    /// holding both colliding fixtures survives a round-trip intact.
    #[test]
    fn save_then_load_keeps_both_actions_that_share_one_key() {
        let dir = tmp_dir("collision");
        let key = canonicalize(MEDIA1_PROFILES_REQ, Masking::Key);

        let mut store = FixtureStore::new("acme-cam");
        store.record(
            MEDIA1_GET_PROFILES,
            MEDIA1_PROFILES_REQ,
            MEDIA1_PROFILES_RESP,
        );
        store.record(
            MEDIA2_GET_PROFILES,
            MEDIA2_PROFILES_REQ,
            MEDIA2_PROFILES_RESP,
        );
        store.save(&dir).unwrap();

        let loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(loaded.len(), 2, "both exchanges survive save + load");
        assert_eq!(
            loaded
                .lookup(MEDIA1_GET_PROFILES, &key)
                .unwrap()
                .response_raw,
            MEDIA1_PROFILES_RESP
        );
        assert_eq!(
            loaded
                .lookup(MEDIA2_GET_PROFILES, &key)
                .unwrap()
                .response_raw,
            MEDIA2_PROFILES_RESP
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pins the documented ambiguity of the deprecated shim: it matches on the
    /// key alone, so with both services recorded it hands a Media2 caller the
    /// Media1 exchange. This is *why* the shim cannot be described as a rename.
    #[test]
    #[allow(deprecated)]
    fn lookup_by_key_returns_the_first_match_and_so_can_be_the_wrong_action() {
        let key = canonicalize(MEDIA1_PROFILES_REQ, Masking::Key);

        let mut store = FixtureStore::new("dev");
        store.record(
            MEDIA1_GET_PROFILES,
            MEDIA1_PROFILES_REQ,
            MEDIA1_PROFILES_RESP,
        );
        store.record(
            MEDIA2_GET_PROFILES,
            MEDIA2_PROFILES_REQ,
            MEDIA2_PROFILES_RESP,
        );

        let hit = store.lookup_by_key(&key).unwrap();
        assert_eq!(
            hit.action, MEDIA1_GET_PROFILES,
            "the shim returns the first fixture matching the key"
        );
        assert_eq!(hit.response_raw, MEDIA1_PROFILES_RESP);
        assert_ne!(
            hit.response_raw, MEDIA2_PROFILES_RESP,
            "a Media2 caller using the shim gets Media1's envelope - the bug it cannot fix"
        );
    }
}
