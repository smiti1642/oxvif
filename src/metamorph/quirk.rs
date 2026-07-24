//! Structural quirk diff: compare a recorded clone against oxvif's synthetic
//! (spec-ideal) mock, per operation, and report where the response *shape*
//! deviates.
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
//! The synthetic mock stands in for "the spec ideal"; it is oxvif's own
//! well-formed response, so a deviation means "the clone's shape differs from
//! what oxvif expects", which is an approximation, not a conformance verdict.

use std::collections::BTreeSet;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Diff every recorded exchange against the synthetic (spec-ideal) mock and
    /// report the structural deviations. See the [module docs](crate::metamorph)
    /// for the structure-only scope.
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
    /// pretty-printed, ephemera-masked, Header-stripped XML — the raw input for
    /// a git-style line diff. `differs` mirrors [`diff_against_synthetic`].
    pub fn diff_details(&self) -> Vec<OperationDiff> {
        self.fixtures()
            .iter()
            .map(|f| {
                let state = MockState::new();
                let synthetic = dispatch(&f.action, BASELINE_BASE, &state, &f.request_raw);
                OperationDiff {
                    action: f.action.clone(),
                    key_canon: f.key_canon.clone(),
                    baseline_xml: pretty_xml(&synthetic, Masking::Key),
                    clone_xml: pretty_xml(&f.response_raw, Masking::Key),
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
            MASK
        } else {
            v.as_str()
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
            MASK
        } else {
            &t
        };
        out.push_str(&open);
        out.push('>');
        out.push_str(shown);
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
}
