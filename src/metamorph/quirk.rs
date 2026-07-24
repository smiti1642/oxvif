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
//! **not** reported — only shape drift is. Value / type-level quirks are the
//! deeper, still-unbuilt half of M7 (see `docs/active/metamorph.md`).
//!
//! The synthetic mock stands in for "the spec ideal"; it is oxvif's own
//! well-formed response, so a deviation means "the clone's shape differs from
//! what oxvif expects", which is an approximation, not a conformance verdict.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

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
        walk(child, &path, set);
    }
    set.insert(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorph::FixtureStore;

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
