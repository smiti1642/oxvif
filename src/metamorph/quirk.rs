//! Structural quirk diff: compare a recorded clone against oxvif's synthetic
//! **reference** mock, per operation, and report where the response *shape*
//! deviates. The baseline is oxvif's own well-formed response, **not** the ONVIF
//! WSDL/XSD schema — see the caveat at the end.
//!
//! For each recorded [`Fixture`](super::fixture::Fixture), the fixture's own
//! request is replayed through the synthetic [`dispatch`] to produce the
//! baseline oxvif would emit, then the two responses' **element-path sets** are
//! diffed. A path present in the clone but not the baseline (or vice versa) is a
//! structural quirk — an extra vendor element, a field oxvif's mock omits, a
//! missing block.
//!
//! ## Scope — structure, not values
//!
//! This compares *which element paths exist*, not their text. A different
//! `Manufacturer` value (`"Hikvision"` vs `"oxvif-mock"`) is expected and is
//! **not** reported — only shape drift is. The SOAP `Header` subtree
//! (WS-Addressing plumbing a real device echoes but the baseline omits) is
//! excluded, so the diff reflects response *Body* shape. Value / type-level
//! quirks are the deeper, still-unbuilt half of M7 (see
//! `docs/active/metamorph.md`).
//!
//! ## Caveat — the baseline is oxvif's mock, not the ONVIF schema
//!
//! The baseline is oxvif's synthetic response, **not** the ONVIF WSDL/XSD. So a
//! deviation means "the clone's shape differs from what oxvif emits/expects",
//! which is a useful proxy (it tracks whether oxvif will parse the device
//! correctly) but **not** a schema-conformance verdict. In particular the mock
//! is *minimal*: it omits many spec-*optional* elements, so a camera that
//! includes an optional element shows it as `only_in_clone` and one that omits
//! one oxvif happens to emit shows it as `only_in_synthetic` — neither is
//! necessarily a spec violation. A true spec baseline would require validating
//! against the XSD (element presence + cardinality), which oxvif does not do.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::mock::canon::{MASK, Masking, mask_attr, mask_text};
use crate::mock::dispatch::dispatch;
use crate::mock::state::MockState;
use crate::soap::XmlNode;

use super::fixture::FixtureStore;

/// Base URL handed to the synthetic dispatcher when producing the baseline. Only
/// affects absolute URLs in the response *text*, which the structural diff
/// ignores — so its exact value is irrelevant.
const BASELINE_BASE: &str = "http://baseline";

/// One operation whose clone response deviates structurally from the synthetic
/// baseline. Empty `only_in_*` vectors never appear here — a fixture with no
/// drift is omitted from the [`QuirkReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationQuirk {
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The canonical, ephemera-masked request (the fixture key) — identifies the
    /// exact call, including its `token=` params.
    pub key_canon: String,
    /// Element paths present in the clone's response but not the synthetic
    /// baseline (e.g. a vendor extension oxvif's mock does not emit). Paths are
    /// prefix-agnostic, slash-joined local names (`Envelope/Body/…`).
    pub only_in_clone: Vec<String>,
    /// Element paths the synthetic baseline emits but the clone's response lacks.
    pub only_in_synthetic: Vec<String>,
}

/// The result of diffing a whole clone against the synthetic baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuirkReport {
    /// The device label the clone was recorded for.
    pub device: String,
    /// How many recorded exchanges were compared.
    pub compared: usize,
    /// The operations that deviated structurally, in the store's insertion order.
    pub quirks: Vec<OperationQuirk>,
}

impl QuirkReport {
    /// Whether the clone matched the synthetic baseline everywhere (no drift).
    pub fn is_empty(&self) -> bool {
        self.quirks.is_empty()
    }

    /// Serialise the report as a compact single-line JSON string — the on-disk
    /// form for a saved quirk baseline.
    pub fn to_json(&self) -> String {
        // serde_json on the fully-serializable QuirkReport — infallible.
        serde_json::to_string(self).expect("QuirkReport is fully serializable")
    }

    /// Serialise the report as pretty-printed JSON (indented, line-separated).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("QuirkReport is fully serializable")
    }

    /// Compare this report against an earlier baseline and report only what
    /// changed — the regression-tracking view. Answers "did this firmware update
    /// change the device's quirks?" and "are these two same-model cameras
    /// quirk-identical?".
    ///
    /// ```no_run
    /// # fn run() -> std::io::Result<()> {
    /// use oxvif::metamorph::{FixtureStore, QuirkReport};
    /// let baseline: QuirkReport =
    ///     serde_json::from_str(&std::fs::read_to_string("quirks-baseline.json")?).unwrap();
    /// let now = FixtureStore::load("tests/fixtures/hikvision-ds2cd")?.diff_against_synthetic();
    /// let d = now.diff(&baseline);
    /// if !d.is_empty() {
    ///     println!("{}", serde_json::to_string_pretty(&d).unwrap());
    /// }
    /// # Ok(()) }
    /// ```
    pub fn diff(&self, prev: &QuirkReport) -> QuirkDiff {
        QuirkDiff::compute(prev, self)
    }
}

// ── QuirkDiff ─────────────────────────────────────────────────────────────────

/// Differences between two [`QuirkReport`]s — what a device's structural quirks
/// gained, lost, or shifted between two runs.
///
/// A quirk's identity is the `(action, key_canon)` pair — the join key the
/// [module docs](crate::metamorph) share with [`ParseVerdict`] and
/// [`OperationDiff`]. `action` alone will not do: one action can have many
/// fixtures distinguished only by their `token=` params. `key_canon` alone is in
/// fact unique *within* one store (it is [`FixtureStore`]'s index key), but the
/// pair keeps the identity aligned with the other reports and stays correct when
/// the two reports being compared come from different stores.
///
/// Entries are ordered by that pair and every path list is sorted, so two runs
/// over identical input serialise byte-identically.
///
/// [`ParseVerdict`]: super::ParseVerdict
///
/// Computed via [`QuirkReport::diff`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuirkDiff {
    /// Operations that drift now but did not in the baseline — a newly quirky
    /// operation, carried whole so the paths are readable without a join.
    pub appeared: Vec<OperationQuirk>,
    /// Operations that drifted in the baseline but match the synthetic mock now
    /// — carried as they appeared in the baseline.
    pub resolved: Vec<OperationQuirk>,
    /// Operations that drift in *both* reports but whose deviating path sets
    /// moved. See [`ChangedQuirk`].
    pub changed: Vec<ChangedQuirk>,
}

/// One entry in [`QuirkDiff::changed`]: an operation quirky in both reports,
/// with the paths that entered or left each side of its deviation.
///
/// The four vectors are set differences against the baseline's corresponding
/// vector, so a path that moved from `only_in_synthetic` to `only_in_clone`
/// shows up in both `clone_only_added` and `synthetic_only_removed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedQuirk {
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The canonical, ephemera-masked request (the fixture key).
    pub key_canon: String,
    /// Paths in [`OperationQuirk::only_in_clone`] now but not in the baseline's.
    pub clone_only_added: Vec<String>,
    /// Paths in the baseline's [`OperationQuirk::only_in_clone`] but not now.
    pub clone_only_removed: Vec<String>,
    /// Paths in [`OperationQuirk::only_in_synthetic`] now but not in the baseline's.
    pub synthetic_only_added: Vec<String>,
    /// Paths in the baseline's [`OperationQuirk::only_in_synthetic`] but not now.
    pub synthetic_only_removed: Vec<String>,
}

impl QuirkDiff {
    /// `true` when the two reports describe the same quirks — nothing appeared,
    /// nothing resolved, and no deviation changed shape.
    pub fn is_empty(&self) -> bool {
        self.appeared.is_empty() && self.resolved.is_empty() && self.changed.is_empty()
    }

    fn compute(prev: &QuirkReport, now: &QuirkReport) -> Self {
        let prev_by_key = index_by_key(prev);
        let now_by_key = index_by_key(now);

        let mut appeared = Vec::new();
        let mut changed = Vec::new();

        for (key, q) in &now_by_key {
            match prev_by_key.get(key) {
                None => appeared.push((*q).clone()),
                Some(p) => {
                    let entry = ChangedQuirk {
                        action: q.action.clone(),
                        key_canon: q.key_canon.clone(),
                        clone_only_added: path_diff(&q.only_in_clone, &p.only_in_clone),
                        clone_only_removed: path_diff(&p.only_in_clone, &q.only_in_clone),
                        synthetic_only_added: path_diff(&q.only_in_synthetic, &p.only_in_synthetic),
                        synthetic_only_removed: path_diff(
                            &p.only_in_synthetic,
                            &q.only_in_synthetic,
                        ),
                    };
                    if !(entry.clone_only_added.is_empty()
                        && entry.clone_only_removed.is_empty()
                        && entry.synthetic_only_added.is_empty()
                        && entry.synthetic_only_removed.is_empty())
                    {
                        changed.push(entry);
                    }
                }
            }
        }

        let resolved = prev_by_key
            .iter()
            .filter(|(key, _)| !now_by_key.contains_key(*key))
            .map(|(_, q)| (*q).clone())
            .collect();

        Self {
            appeared,
            resolved,
            changed,
        }
    }
}

/// Index a report's quirks on their identity — the `(action, key_canon)` *pair*,
/// since `key_canon` alone collides across actions. The `BTreeMap` also fixes the
/// diff's output order deterministically.
fn index_by_key(r: &QuirkReport) -> BTreeMap<(&str, &str), &OperationQuirk> {
    r.quirks
        .iter()
        .map(|q| ((q.action.as_str(), q.key_canon.as_str()), q))
        .collect()
}

/// The paths in `a` absent from `b`, sorted and deduplicated — so the result is
/// independent of the order the two reports happened to store their paths in.
fn path_diff(a: &[String], b: &[String]) -> Vec<String> {
    let a: BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let b: BTreeSet<&str> = b.iter().map(String::as_str).collect();
    a.difference(&b).map(|p| (*p).to_string()).collect()
}

/// Side-by-side diff material for one operation: the baseline and clone responses
/// rendered as aligned, pretty-printed XML, ready for a line-level (git-style)
/// diff. Produced by [`FixtureStore::diff_details`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDiff {
    /// The SOAP action URI this exchange answered.
    pub action: String,
    /// The canonical, ephemera-masked request (the fixture key).
    pub key_canon: String,
    /// oxvif's synthetic baseline response — the **left** side of the diff.
    pub baseline_xml: String,
    /// The real camera's recorded response — the **right** side of the diff.
    pub clone_xml: String,
    /// Whether the two differ structurally (same test as the quirk report).
    pub differs: bool,
}

impl FixtureStore {
    /// Diff every recorded exchange against the synthetic reference mock and
    /// report the structural deviations. See the [module docs](crate::metamorph)
    /// for the structure-only scope and the baseline caveat (it is oxvif's mock,
    /// not the ONVIF schema).
    ///
    /// ```no_run
    /// # fn run() -> std::io::Result<()> {
    /// use oxvif::metamorph::FixtureStore;
    /// let store = FixtureStore::load("tests/fixtures/hikvision-ds2cd")?;
    /// let report = store.diff_against_synthetic();
    /// for q in &report.quirks {
    ///     println!("{}: +{:?} -{:?}", q.action, q.only_in_clone, q.only_in_synthetic);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn diff_against_synthetic(&self) -> QuirkReport {
        let mut quirks = Vec::new();
        for f in self.fixtures() {
            // A fresh synthetic device answers the fixture's own request.
            let state = MockState::new();
            let synthetic = dispatch(&f.action, BASELINE_BASE, &state, &f.request_raw);

            let clone_paths = element_paths(&f.response_raw);
            let synth_paths = element_paths(&synthetic);
            let only_in_clone: Vec<String> =
                clone_paths.difference(&synth_paths).cloned().collect();
            let only_in_synthetic: Vec<String> =
                synth_paths.difference(&clone_paths).cloned().collect();

            if !only_in_clone.is_empty() || !only_in_synthetic.is_empty() {
                quirks.push(OperationQuirk {
                    action: f.action.clone(),
                    key_canon: f.key_canon.clone(),
                    only_in_clone,
                    only_in_synthetic,
                });
            }
        }
        QuirkReport {
            device: self.device().to_string(),
            compared: self.fixtures().len(),
            quirks,
        }
    }

    /// Per-operation side-by-side material: for every recorded exchange, the
    /// synthetic baseline and the clone response rendered as aligned,
    /// pretty-printed, Header-stripped XML — the raw input for a git-style line
    /// diff. Instance-specific values (transport ephemera, tokens, and
    /// IPv4/IPv6/MAC literals, including IPs inside URLs) are normalised to a
    /// placeholder, so a line that differs only in such a value doesn't show as
    /// a change. `differs` mirrors [`Self::diff_against_synthetic`].
    pub fn diff_details(&self) -> Vec<OperationDiff> {
        self.fixtures()
            .iter()
            .map(|f| {
                let state = MockState::new();
                let synthetic = dispatch(&f.action, BASELINE_BASE, &state, &f.request_raw);
                OperationDiff {
                    action: f.action.clone(),
                    key_canon: f.key_canon.clone(),
                    baseline_xml: pretty_xml(&synthetic, Masking::Value),
                    clone_xml: pretty_xml(&f.response_raw, Masking::Value),
                    differs: element_paths(&f.response_raw) != element_paths(&synthetic),
                }
            })
            .collect()
    }
}

/// Render `xml` as indented, one-element-per-line XML — volatile fields masked
/// per `masking`, the top-level SOAP `Header` dropped — an aligned form for a
/// line diff. Unparseable input is returned unchanged.
fn pretty_xml(xml: &str, masking: Masking) -> String {
    match XmlNode::parse(xml) {
        Ok(root) => {
            let mut out = String::new();
            pretty_node(&mut out, &root, 0, masking, true);
            out
        }
        Err(_) => xml.to_string(),
    }
}

fn pretty_node(out: &mut String, node: &XmlNode, depth: usize, masking: Masking, is_root: bool) {
    let indent = "  ".repeat(depth);
    let mut open = format!("{indent}<{}", node.local_name);
    let mut attrs: Vec<(&String, &String)> = node.attrs.iter().collect();
    attrs.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in attrs {
        let val = if mask_attr(k, masking) {
            MASK.to_string()
        } else {
            normalize_instance_values(v)
        };
        open.push_str(&format!(" {k}=\"{val}\""));
    }

    // Drop the top-level SOAP Header subtree, matching the structural diff.
    let children: Vec<&XmlNode> = node
        .children
        .iter()
        .filter(|c| !(is_root && c.local_name == "Header"))
        .collect();

    let text = node
        .text
        .as_deref()
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty());

    if children.is_empty() && text.is_none() {
        out.push_str(&open);
        out.push_str("/>\n");
    } else if children.is_empty() {
        let t = text.unwrap();
        let shown = if mask_text(&node.local_name, masking) {
            MASK.to_string()
        } else {
            normalize_instance_values(&t)
        };
        out.push_str(&open);
        out.push('>');
        out.push_str(&shown);
        out.push_str(&format!("</{}>\n", node.local_name));
    } else {
        out.push_str(&open);
        out.push_str(">\n");
        for c in children {
            pretty_node(out, c, depth + 1, masking, false);
        }
        out.push_str(&format!("{indent}</{}>\n", node.local_name));
    }
}

/// Normalise instance-specific values that differ per device but aren't a
/// structural quirk — IPv4/IPv6/MAC literals (in bare values *and* inside URLs
/// like `XAddr`) — to a stable placeholder, so a line that differs only in such
/// a value collapses to an equal (un-highlighted) row in the side-by-side diff.
/// Token identifiers are handled separately by [`Masking::Value`].
fn normalize_instance_values(s: &str) -> String {
    normalize_ipv6(&normalize_ipv4(s))
}

/// Replace every dotted-quad IPv4 literal (each octet 0–255) with `x.x.x.x`.
fn normalize_ipv4(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < s.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let run = &s[start..i];
            out.push_str(if is_ipv4(run) { "x.x.x.x" } else { run });
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn is_ipv4(run: &str) -> bool {
    let parts: Vec<&str> = run.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u16>().is_ok_and(|n| n <= 255))
}

/// Replace IPv6 / MAC-style literals (hex groups joined by `:`) with `x:x`.
/// Guarded so plain times like `12:34:56` (no hex letter, no `::`) are left
/// alone. Runs after [`normalize_ipv4`], whose `x.x.x.x` output has no colons.
fn normalize_ipv6(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if is_hexish(bytes[i]) {
            let start = i;
            while i < s.len() && is_hexish(bytes[i]) {
                i += 1;
            }
            let run = &s[start..i];
            out.push_str(if looks_like_ipv6_or_mac(run) {
                "x:x"
            } else {
                run
            });
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn is_hexish(c: u8) -> bool {
    c.is_ascii_hexdigit() || c == b':'
}

fn looks_like_ipv6_or_mac(run: &str) -> bool {
    if !run.contains(':') {
        return false;
    }
    // A hex *letter* (a–f) or a `::` distinguishes an address/MAC from a plain
    // numeric time like `12:34:56`.
    run.contains("::")
        || run
            .bytes()
            .any(|b| b.is_ascii_hexdigit() && !b.is_ascii_digit())
}

/// The set of element paths in `xml` — prefix-agnostic, slash-joined local names
/// (`Envelope/Body/GetHostnameResponse/Name`). Repeated siblings collapse to one
/// path; only presence matters. Unparseable input yields the empty set.
fn element_paths(xml: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    if let Ok(root) = XmlNode::parse(xml) {
        walk(&root, "", &mut set);
    }
    set
}

fn walk(node: &XmlNode, prefix: &str, set: &mut BTreeSet<String>) {
    let path = if prefix.is_empty() {
        node.local_name.clone()
    } else {
        format!("{prefix}/{}", node.local_name)
    };
    for child in &node.children {
        // Skip the SOAP `Header` subtree (WS-Addressing plumbing — MessageID,
        // To, RelatesTo …). A real device echoes it; the synthetic baseline
        // emits none, so keeping it would flag *every* operation and bury the
        // real Body-shape differences. `prefix.is_empty()` ⇒ `node` is the root
        // envelope, so this only drops the top-level Header, not a same-named
        // element deeper in the body.
        if prefix.is_empty() && child.local_name == "Header" {
            continue;
        }
        walk(child, &path, set);
    }
    set.insert(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorph::FixtureStore;

    #[test]
    fn normalize_masks_addresses_but_not_times_or_versions() {
        // IPv4 in a bare value and inside a URL.
        assert_eq!(normalize_instance_values("192.168.1.5"), "x.x.x.x");
        assert_eq!(
            normalize_instance_values("rtsp://10.0.0.9:554/live"),
            "rtsp://x.x.x.x:554/live"
        );
        // IPv6 and MAC.
        assert_eq!(normalize_instance_values("fe80::1"), "x:x");
        assert_eq!(normalize_instance_values("00:1A:2B:3C:4D:5E"), "x:x");
        // A plain numeric time and a 3-part version are left alone.
        assert_eq!(normalize_instance_values("12:34:56"), "12:34:56");
        assert_eq!(normalize_instance_values("5.1.2"), "5.1.2");
    }

    #[test]
    fn diff_details_render_aligned_masked_header_stripped_bodies() {
        let action = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        let req = "<Envelope><Body><GetHostname/></Body></Envelope>";
        let state = MockState::new();
        let synthetic = dispatch(action, BASELINE_BASE, &state, req);

        // Clone = the synthetic body, but wrapped with a SOAP Header the
        // baseline lacks — after Header-stripping the two renders must match.
        let env = synthetic.find("Envelope").expect("SOAP Envelope root");
        let gt = env + synthetic[env..].find('>').expect("open tag closes");
        let clone = format!(
            "{}<Header><To>http://cam/onvif</To></Header>{}",
            &synthetic[..=gt],
            &synthetic[gt + 1..]
        );
        let mut store = FixtureStore::new("clone");
        store.record(action, req, &clone);

        let details = store.diff_details();
        assert_eq!(details.len(), 1);
        let d = &details[0];
        assert!(!d.differs, "clone == synthetic body → no drift: {d:?}");
        assert_eq!(
            d.baseline_xml, d.clone_xml,
            "identical bodies must render identically"
        );
        assert!(
            d.clone_xml.contains("<GetHostnameResponse"),
            "pretty body present: {}",
            d.clone_xml
        );
        assert!(
            !d.clone_xml.contains("Header") && !d.clone_xml.contains("<To>"),
            "SOAP Header subtree must be dropped: {}",
            d.clone_xml
        );
        assert!(d.clone_xml.contains('\n'), "multi-line pretty output");
    }

    #[test]
    fn soap_header_subtree_is_ignored() {
        // A real device echoes a WS-Addressing SOAP Header the synthetic mock
        // never emits; it must not register as a quirk on every operation.
        let action = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        let req = "<Envelope><Body><GetHostname/></Body></Envelope>";
        let state = MockState::new();
        let synthetic = dispatch(action, BASELINE_BASE, &state, req);

        // Clone = the synthetic Body, but wrapped with an extra SOAP Header.
        let env = synthetic.find("Envelope").expect("SOAP Envelope root");
        let gt = env + synthetic[env..].find('>').expect("open tag closes");
        let with_header = format!(
            "{}<Header><To>http://cam/onvif</To><MessageID>uuid:x</MessageID></Header>{}",
            &synthetic[..=gt],
            &synthetic[gt + 1..]
        );
        let mut store = FixtureStore::new("clone");
        store.record(action, req, &with_header);
        let report = store.diff_against_synthetic();
        assert!(
            report.is_empty(),
            "a SOAP Header must not count as a structural quirk: {report:?}"
        );
    }

    #[test]
    fn element_paths_are_prefix_agnostic_and_nested() {
        let paths = element_paths("<s:E xmlns:s='urn:x'><Body><Foo><Bar/></Foo></Body></s:E>");
        assert!(paths.contains("E"), "root: {paths:?}");
        assert!(paths.contains("E/Body/Foo"), "nested: {paths:?}");
        assert!(paths.contains("E/Body/Foo/Bar"), "leaf: {paths:?}");
    }

    #[test]
    fn matching_shape_is_clean_and_extra_element_is_flagged() {
        let action = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        let req = "<Envelope><Body><GetHostname/></Body></Envelope>";
        let state = MockState::new();
        let synthetic = dispatch(action, BASELINE_BASE, &state, req);

        // Clone == synthetic → identical shape, no quirk.
        let mut store = FixtureStore::new("clone");
        store.record(action, req, &synthetic);
        let report = store.diff_against_synthetic();
        assert_eq!(report.compared, 1);
        assert!(
            report.is_empty(),
            "identical shape must not be a quirk: {report:?}"
        );

        // Clone with an extra element the baseline lacks: insert it as the first
        // child of the Envelope, prefix-robustly, right after the Envelope
        // opening tag (skipping any `<?xml?>` prolog, so it stays single-rooted).
        let env = synthetic.find("Envelope").expect("SOAP Envelope root");
        let gt = env + synthetic[env..].find('>').expect("open tag closes");
        let quirky = format!(
            "{}<VendorExtension>x</VendorExtension>{}",
            &synthetic[..=gt],
            &synthetic[gt + 1..]
        );
        let mut store2 = FixtureStore::new("clone");
        store2.record(action, req, &quirky);
        let report2 = store2.diff_against_synthetic();
        assert_eq!(report2.quirks.len(), 1, "one drifting op: {report2:?}");
        let q = &report2.quirks[0];
        assert!(
            q.only_in_clone
                .iter()
                .any(|p| p.ends_with("VendorExtension")),
            "extra element should be only_in_clone: {q:?}"
        );
        assert!(
            q.only_in_synthetic.is_empty(),
            "baseline lacks nothing the clone has here: {q:?}"
        );
    }

    // ── to_json / diff ────────────────────────────────────────────────────────

    /// A quirk with the given identity and deviating path sets.
    fn quirk(action: &str, key: &str, clone: &[&str], synth: &[&str]) -> OperationQuirk {
        OperationQuirk {
            action: action.to_string(),
            key_canon: key.to_string(),
            only_in_clone: clone.iter().map(|s| s.to_string()).collect(),
            only_in_synthetic: synth.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn report(quirks: Vec<OperationQuirk>) -> QuirkReport {
        QuirkReport {
            device: "cam".to_string(),
            compared: 4,
            quirks,
        }
    }

    const GET_PROFILES: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
    const GET_HOSTNAME: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";

    #[test]
    fn to_json_round_trips() {
        let r = report(vec![
            quirk(GET_PROFILES, "k1", &["E/Body/P/Vendor"], &[]),
            quirk(GET_HOSTNAME, "k2", &[], &["E/Body/H/Name"]),
        ]);
        let json = r.to_json();

        let back: QuirkReport = serde_json::from_str(&json).expect("to_json emits valid JSON");
        assert_eq!(back.device, r.device);
        assert_eq!(back.compared, r.compared);
        assert_eq!(back.quirks, r.quirks);
        // Re-serialising the round-tripped report reproduces the same bytes.
        assert_eq!(back.to_json(), json);
    }

    #[test]
    fn to_json_pretty_is_indented() {
        let r = report(vec![quirk(GET_PROFILES, "k1", &["E/Body/P/Vendor"], &[])]);
        let compact = r.to_json();
        let pretty = r.to_json_pretty();

        assert_ne!(pretty, compact);
        assert!(pretty.contains('\n'), "pretty JSON is line-separated");
        assert!(pretty.contains("\n  \""), "pretty JSON is indented");
        assert!(!compact.contains('\n'), "compact JSON is single-line");
        // Both encode the same document.
        let a: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let b: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn diff_is_empty_for_identical_reports() {
        let r = report(vec![
            quirk(GET_PROFILES, "k1", &["E/Body/P/Vendor"], &["E/Body/P/Gone"]),
            quirk(GET_HOSTNAME, "k2", &[], &["E/Body/H/Name"]),
        ]);
        let d = r.diff(&r.clone());
        assert!(d.is_empty(), "same quirks → no diff: {d:?}");
        assert_eq!(d, QuirkDiff::compute(&r, &r));
    }

    #[test]
    fn diff_flags_appeared_quirk() {
        let prev = report(vec![]);
        let now = report(vec![quirk(GET_PROFILES, "k1", &["E/Body/P/Vendor"], &[])]);

        let d = now.diff(&prev);
        assert!(!d.is_empty());
        assert_eq!(d.appeared.len(), 1, "{d:?}");
        assert_eq!(d.appeared[0].action, GET_PROFILES);
        assert_eq!(d.appeared[0].only_in_clone, ["E/Body/P/Vendor"]);
        assert!(d.resolved.is_empty() && d.changed.is_empty(), "{d:?}");
    }

    #[test]
    fn diff_flags_resolved_quirk() {
        let prev = report(vec![quirk(GET_PROFILES, "k1", &["E/Body/P/Vendor"], &[])]);
        let now = report(vec![]);

        let d = now.diff(&prev);
        assert!(!d.is_empty());
        assert_eq!(d.resolved.len(), 1, "{d:?}");
        assert_eq!(d.resolved[0].key_canon, "k1");
        assert_eq!(
            d.resolved[0].only_in_clone,
            ["E/Body/P/Vendor"],
            "resolved carries the baseline's paths: {d:?}"
        );
        assert!(d.appeared.is_empty() && d.changed.is_empty(), "{d:?}");
    }

    #[test]
    fn diff_flags_changed_path_sets() {
        // Same operation, quirky in both — but one path moved from the
        // synthetic-only side to the clone-only side, and another was dropped.
        let prev = report(vec![quirk(
            GET_PROFILES,
            "k1",
            &["E/Body/P/Old"],
            &["E/Body/P/Moved"],
        )]);
        let now = report(vec![quirk(
            GET_PROFILES,
            "k1",
            &["E/Body/P/Moved"],
            &["E/Body/P/Fresh"],
        )]);

        let d = now.diff(&prev);
        assert!(d.appeared.is_empty() && d.resolved.is_empty(), "{d:?}");
        assert_eq!(d.changed.len(), 1, "{d:?}");
        let c = &d.changed[0];
        assert_eq!(c.action, GET_PROFILES);
        assert_eq!(c.key_canon, "k1");
        assert_eq!(c.clone_only_added, ["E/Body/P/Moved"]);
        assert_eq!(c.clone_only_removed, ["E/Body/P/Old"]);
        assert_eq!(c.synthetic_only_added, ["E/Body/P/Fresh"]);
        assert_eq!(c.synthetic_only_removed, ["E/Body/P/Moved"]);
    }

    #[test]
    fn diff_keys_on_action_and_key_canon_pair() {
        // Two fixtures of the *same* action distinguished only by their
        // `token=` params: they are distinct quirks, never merged.
        let prev = report(vec![quirk(
            GET_PROFILES,
            "GetProfile token=A",
            &["E/Body/P/A"],
            &[],
        )]);
        let now = report(vec![quirk(
            GET_PROFILES,
            "GetProfile token=B",
            &["E/Body/P/B"],
            &[],
        )]);

        let d = now.diff(&prev);
        assert_eq!(d.appeared.len(), 1, "token=B is new: {d:?}");
        assert_eq!(d.appeared[0].key_canon, "GetProfile token=B");
        assert_eq!(d.resolved.len(), 1, "token=A is gone: {d:?}");
        assert_eq!(d.resolved[0].key_canon, "GetProfile token=A");
        assert!(
            d.changed.is_empty(),
            "different key_canon ⇒ never a path change: {d:?}"
        );

        // And the mirror trap: the same key_canon under a *different* action is
        // likewise distinct, not a changed path set.
        let prev2 = report(vec![quirk(GET_PROFILES, "k1", &["E/Body/X"], &[])]);
        let now2 = report(vec![quirk(GET_HOSTNAME, "k1", &["E/Body/Y"], &[])]);
        let d2 = now2.diff(&prev2);
        assert_eq!(d2.appeared.len(), 1, "{d2:?}");
        assert_eq!(d2.resolved.len(), 1, "{d2:?}");
        assert!(d2.changed.is_empty(), "{d2:?}");
    }

    #[test]
    fn diff_output_order_is_deterministic() {
        let a = quirk(GET_PROFILES, "k2", &["E/Body/b", "E/Body/a"], &[]);
        let b = quirk(GET_HOSTNAME, "k1", &["E/Body/z"], &[]);
        let prev = report(vec![]);

        let d1 = report(vec![a.clone(), b.clone()]).diff(&prev);
        let d2 = report(vec![b, a]).diff(&prev);
        assert_eq!(d1, d2, "insertion order must not leak into the diff");

        // Path lists inside a changed entry are sorted, not source-ordered.
        let base = report(vec![quirk(GET_PROFILES, "k1", &[], &[])]);
        let now = report(vec![quirk(GET_PROFILES, "k1", &["E/b", "E/a"], &[])]);
        assert_eq!(now.diff(&base).changed[0].clone_only_added, ["E/a", "E/b"]);
    }
}
