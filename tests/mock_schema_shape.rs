//! Does the mock emit XML the ONVIF schema actually declares?
//!
//! ## Why this exists
//!
//! The mock writes XML as hand-built strings, so it can emit a document no
//! schema allows and **all five gate lines stay green**. Seven instances have
//! been found so far, every one of them by a human reading a schema file by
//! hand:
//!
//! | found | the mock emitted | the schema says |
//! |---|---|---|
//! | 0.15.0 | `tt:AFModes` at the top of the focus options | that name is real, but only under `Extension` |
//! | 0.15.0 | `BitrateRange` at the top level of the options | only under `Extension` |
//! | 0.15.0 | Media1 options flat | wrapper + repeated entry |
//! | 0.15.0 | `DefaultAbsolutePanTiltPositionSpace` | `…Pant…`, double `t` |
//! | 0.15.0 | `tt:ScopeAttribute` | a different name entirely |
//! | 0.15.0 | Media2 `Audio` | `AudioEncoder` — **this one was a client bug** |
//! | 0.15.0 | Media2 `<tt:GovLength>` / `<tt:Profile>` elements | `xs:attribute` on the configuration — **also a client bug** |
//!
//! Three of those were put back one at a time and each turned this test red on
//! the assertion (`8091892`, schema set of run 3):
//!
//! ```text
//! tt:AFModes          UNKNOWN-CHILD  6 → 7      UNKNOWN-NAME unchanged
//! PanTilt spelling    UNKNOWN-CHILD  6 → 10     UNKNOWN-NAME 12 → 16
//! tr2:Audio           MISSING-REQ.  23 → 22     UNKNOWN-NAME 12 → 13
//! ```
//!
//! Two things in that table are worth keeping. **`AFModes` moves
//! `UNKNOWN-CHILD`, not `UNKNOWN-NAME`** — the name is a real ONVIF element at
//! a deeper level, so it is an `Extension`-nesting defect rather than a
//! misspelling, and `docs/active/schema-shape-plan-2026-08.md` said otherwise
//! until this run measured it. And **`tr2:Audio` leaves the total at 63 while
//! moving two kinds**, which is why [`PINS`] is per-kind: a single total would
//! have let this release's client bug back in silently.
//!
//! Six for six by hand is not a strategy, and nothing says the class is
//! exhausted. Worse, **no other test in this repository can see any of it**:
//! `XmlNode` is namespace-stripped (`src/soap/xml.rs`) and every lookup matches
//! the local name only, so oxvif's own parser is namespace-blind and
//! order-independent. A response with every element in the wrong namespace, in
//! the wrong order, parses identically. `tests/mock_roundtrip.rs` and
//! `tests/mock_token_discrimination.rs` go through the client, so neither could
//! ever have caught one.
//!
//! ## Why it is `#[ignore]`d, and what that costs
//!
//! The ONVIF schema set is © ONVIF 2008-2025. The maintainer's decision
//! (`docs/active/schema-shape-plan-2026-08.md` §4, D2) is that **nothing derived
//! from it enters this repository** — not the files, not a generated index, not
//! a derived fixture, and **not a schema fact hardcoded here**. A
//! `const REQUIRED: &[&str] = &["TLS1.1", …]` in this file would be the same
//! redistribution wearing a different extension.
//!
//! So every element name, cardinality and sequence below is read at run time
//! from a directory outside the working tree. This file contains namespace URIs
//! and its own logic; nothing else.
//!
//! ```sh
//! OXVIF_ONVIF_SCHEMA=/path/to/schema \
//!   cargo test --features mock --test mock_schema_shape -- --ignored --nocapture
//! ```
//!
//! The directory needs the service WSDLs plus `onvif.xsd` and `common.xsd`;
//! `onvif.xsd` alone anchors 19% of the output and is not the schema. Fetching
//! `b-2.xsd` as well is what made the three events findings visible at all.
//!
//! **The cost, stated plainly: this check can silently stop being run.** Not in
//! CI, not for a contributor, not for the maintainer on a machine where the
//! directory moved. Two things make that survivable, and both are load-bearing:
//!
//! - the skip path **prints why**, so a run that checked nothing never looks
//!   like a run that passed;
//! - `CLAUDE.md`'s publishing checklist is the only thing that makes it happen.
//!   That is weaker than a gate line, and is written down as weaker.
//!
//! ## What it cannot see
//!
//! - **Values.** Ranges, enumerations, lexical spaces. A structural index
//!   carries names and cardinality.
//! - **An empty element whose children are all optional.** `<tt:SupportedPTZSpaces/>`
//!   was a real 0.15.0 defect and is schema-valid. Shape checking answers *is
//!   this well-formed*, never *does it mean anything*.
//! - **Whatever the corpus does not reach.** A third of the responses are SOAP
//!   faults, because the operation needs a body this file does not supply. Those
//!   contribute no shape evidence, and [`PAYLOAD_FLOOR`] is what stops that
//!   third quietly becoming two thirds.
#![cfg(feature = "mock")]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxvif::mock::MockTransport;
use oxvif::transport::Transport;

// ── Namespace URIs. The only external constants this file is allowed. ────────

const XS: &str = "http://www.w3.org/2001/XMLSchema";
const WSDL: &str = "http://schemas.xmlsoap.org/wsdl/";
const SOAP_ENV: &str = "http://www.w3.org/2003/05/soap-envelope";

// ── Pins ─────────────────────────────────────────────────────────────────────

/// Distinct findings per kind, against the schema set described in the lab's
/// `NOTES.md` run 3.
///
/// **These are pins, not targets.** A fix lowers one and the test says so; a
/// regression raises one and the test says that too. Never edit a number to
/// make a run green — read the diff the failure prints first.
///
/// Quoted in `docs/active/mock-schema-conformance-2026-08.md` §1 and in the
/// lab's `NOTES.md` (run 3). Change one here and both are wrong.
///
/// Movement so far:
///
/// - `8091892` … `1e92fbc` — 16 / 23 / **12** / 6 / 6, the baseline this file
///   landed with.
/// - §5.0, `GetDigitalInputs` to the DeviceIO service — `UNKNOWN-NAME` 12 → 11.
///   The one row was the `DigitalInputs` *child*; the response root itself was
///   the corpus's single **unanchored non-fault root** and contributed nothing
///   to any count, because an unanchored root is not judged at all.
///
///   Measured, and the reason the anchoring assertion below exists: putting
///   only the wrapper's namespace back leaves all five numbers here at
///   16 / 23 / 11 / 6 / 6 and the run still fails — on the anchoring assertion.
///   **The pins do not see a wrong-service defect.** They saw one row of this
///   one by luck, because the mock happened to render the child in the same
///   wrong namespace.
/// - §5.1, namespace correctness — `WRONG-NS` 16 → **0**, no other kind moved.
///   Four families, and three of them could not have been fixed by any rule
///   about element *names*: storage `tt:` → `tds:`, Media2 `tt:` → `tr2:`,
///   recording `trc:` → **`tt:`**, events `tev:`/`wstop:` → **`wsnt:`**.
///   `CurrentTime` and `TerminationTime` are `wsnt:` on
///   `CreatePullPointSubscriptionResponse` and `tev:` on
///   `PullMessagesResponse` — same two names, same WSDL, two namespaces.
///
///   **Zero is the weakest pin here, not the strongest.** A careless edit can
///   leave it green by making a *different* mistake: a name undeclared anywhere
///   reports `UNKNOWN-NAME`, and a name undeclared *by its parent* reports
///   `UNKNOWN-CHILD`. Only a name that the parent declares in some other
///   namespace lands in this bucket.
/// - §5.2 + §5.3, the imaging slice — `MISSING-REQUIRED` 23 → 21, `UNKNOWN-NAME`
///   11 → 8, `UNKNOWN-CHILD` 6 → 4, `ORDER` 6 → 5. Eight rows, and **seven of
///   them were one defect**: `GetMoveOptions` rendered the focus ranges as
///   `PositionSpace` / `SpeedSpace`, borrowing PTZ's *space* vocabulary for a
///   type that has none, where the schema declares a plain `Position` and
///   `Speed`. One wrong name therefore reported in three kinds at once — the
///   name is undeclared (`UNKNOWN-NAME`), the parent does not accept it
///   (`UNKNOWN-CHILD`), and the real required child is then absent
///   (`MISSING-REQUIRED`). The eighth row is `GetOptions`, which emitted
///   `tt:ImagingOptions20`'s ten children in a plausible but invented order.
///
///   Putting the four `…Space` names back moves exactly those three kinds and
///   leaves `ORDER` at 5, so the two halves of this bucket are independently
///   pinned. **A one-name defect that moves three counters is the reason a
///   per-kind pin is not merely more precise than a total — a total would have
///   let a compensating pair of edits through.**
/// - §5.4, the two `GetProfiles` responses — `MISSING-REQUIRED` 21 → 16,
///   `ORDER` 5 → 3, `UNKNOWN-NAME` 8 → **9**.
///
///   The five required-member rows were **one decision**: Media2 rendered each
///   configuration as `<tr2:VideoSource token="…"/>`, and `tr2:ConfigurationSet`
///   types every member as the *whole* configuration — `VideoSource` is
///   `tt:VideoSourceConfiguration`, the same type `tt:Profile` inlines. So one
///   renderer omitted the required members of five different types at once.
///   The two order rows are unrelated to it and to each other: `tt:Profile` and
///   `tr2:ConfigurationSet` are different types in different schemas that happen
///   to agree on interleaving audio source between the two video members, and
///   each was derived on its own. Perturbed one at a time: token references back
///   moves `MISSING-REQUIRED` 16 → 21 and leaves `ORDER` at 3; either order back
///   moves `ORDER` 3 → 4 and nothing else.
///
///   **`UNKNOWN-NAME` went up, and that was the honest number.** Inlining the
///   video encoder reuses `render_video_encoder`, the same helper
///   `GetVideoEncoderConfigurations` uses, so the `tt:Profile` element it emitted
///   appeared at a second path — one more distinct row for a defect already
///   counted once. Taking the second copy instead was rejected: a duplicated
///   renderer that can drift from the list getter is the failure mode
///   `CLAUDE.md` step 5b exists for, and is worse than a counted defect. Both
///   rows closed together in §5.5, as that entry predicted they would.
/// - §5.5, `GovLength` / `Profile` on the Media2 encoder — `UNKNOWN-NAME`
///   9 → **7**, no other kind moved. `tt:VideoEncoder2Configuration` declares
///   both as `xs:attribute`; `VideoEncoderConfiguration2` parsed and emitted
///   them as child elements and the mock rendered them that way to match, so
///   this was a **client** defect, the second one this sweep has found.
///
///   **The two rows that closed were both `Profile`, and the count is weaker
///   evidence than it looks.** Only `Profile` was ever visible here:
///
///   - `GovLength` moved **nothing**, in either direction. The name is a real
///     `tt:` element on `H264Configuration` and `Mpeg4Configuration`, so
///     `UNKNOWN-NAME` never fired; and `tt:VideoEncoder2Configuration` carries
///     an `xs:any`, which sets `Ty::wild` and suppresses `UNKNOWN-CHILD` for
///     the whole type. A wildcarded type is a blind spot for *every* misplaced
///     child whose name exists somewhere in the namespace.
///   - Nothing here reads attributes at all. A row disappearing proves the
///     element is gone, **not** that the attribute that replaced it is spelled
///     right or carried at all. `tests/mock_workflow.rs`'s
///     `media2_encoder_gov_length_and_profile_are_attributes` is what asserts
///     that, by driving the client against the mock and reading the values.
/// - §5.6, the whole `GetCapabilities` tree — `MISSING-REQUIRED` 16 → **11**,
///   `UNKNOWN-NAME` 7 → **6**, `ORDER` and `UNKNOWN-CHILD` unmoved. Six rows,
///   two independent halves, perturbed separately.
///
///   The five required-member rows were five *different* types each rendered
///   down to the members oxvif's parser happens to read — `SecurityCapabilities`
///   missing four, `SystemCapabilities` one, `EventCapabilities` one,
///   `RecordingCapabilities` five, `SearchCapabilities` one. Putting all five
///   back moves `MISSING-REQUIRED` 11 → 16 and nothing else. **Adding required
///   members opened no new row**, which is not automatic: every added element is
///   a fresh chance at a wrong namespace or position, and `SupportedVersions`
///   carries two required children of its own.
///
///   The sixth row was `Device/Security/UsernameToken`, and it is the one place
///   here where the count is the *weaker* evidence. Removing it moves
///   `UNKNOWN-NAME` 6 → 7 and proves only that the element is gone. What it
///   cannot show is that dropping it was right rather than a rename:
///   `UsernameToken` is declared **only** as an `xs:attribute` on
///   `tds:SecurityCapabilities`, never as an element anywhere in any of the
///   fifteen files, and it is not reachable through `SecurityCapabilitiesExtension`
///   or `…Extension2` either. So there was no element to rename it to, and the
///   fact belongs to the operation that already carries it. `src/health/`'s
///   cross-check had paired the two names across the two types and is what
///   asserts the consequence.
/// - §5.7, the Media2 metadata family — `MISSING-REQUIRED` 11 → **7**,
///   `ORDER` 3 → **1**, `UNKNOWN-CHILD` 4 → **3**, `UNKNOWN-NAME` 6 → **5**.
///   Eight rows, two renderers in `src/mock/services/media2.rs`, perturbed as
///   two independent halves.
///
///   `render_metadata` carried six of the eight: `Analytics` before `PTZStatus`
///   (one row per seeded configuration, one cause), and `Multicast` /
///   `SessionTimeout` absent. Putting it back moves `MISSING-REQUIRED` 7 → 11
///   and `ORDER` 1 → 3, nothing else. **`tt:MetadataConfiguration/Multicast` is
///   `[1]`**, and so are the `Address`, `Port`, `TTL` and `AutoStart` inside it;
///   the mock had emitted the block only for a configuration with an address, on
///   a comment claiming it was optional. Only `tt:IPAddress/IPv4Address` is, so
///   an unconfigured entry sends the block and omits the address — which is what
///   keeps `MetadataConfiguration::multicast_address` observable as `None`.
///
///   `resp_metadata_configuration_options` carried the other two, and they are
///   one element: `Extension/AnalyticsSupported`, undeclared **and** not
///   accepted by its parent. Putting it back moves `UNKNOWN-CHILD` 3 → 4 and
///   `UNKNOWN-NAME` 5 → 6; putting the empty `<tt:PTZStatusFilterOptions/>` back
///   moves `MISSING-REQUIRED` alone. As with `UsernameToken`, the count proves
///   only that the element is gone. What settles it is that `AnalyticsSupported`
///   is declared **nowhere** in the schema set, as element or attribute, and
///   `tt:MetadataConfigurationOptionsExtension` declares exactly
///   `CompressionType` and a further `Extension` — so there was nothing to
///   rename it to, and the fact it stated is `GetCapabilities`'
///   `tt:AnalyticsCapabilities/AnalyticsModuleSupport`, a different operation.
///   `metadata_configs_differ_on_every_field` in `tests/mock_workflow.rs` is
///   what asserts the two required booleans that replaced it, per token.
/// - §5.8, the two Media2 *options* types — `UNKNOWN-NAME` 5 → **4**,
///   `UNKNOWN-CHILD` 3 → **2**, `MISSING-REQUIRED` and `ORDER` unmoved. Two rows
///   closed, and **six members were misclassified to close them**, which is the
///   widest gap between what this file measures and what the work was.
///
///   `tt:VideoEncoder2ConfigurationOptions` declares exactly four child elements
///   — `Encoding`, `QualityRange`, `ResolutionsAvailable`, `BitrateRange` — and
///   everything else as `xs:attribute`. `VideoEncoderOptions2` read
///   `GovLengthRange`, `FrameRatesSupported` and `ProfilesSupported` as
///   elements, so all three were empty or `None` from every conformant device;
///   it also carried a `frame_rate_range` field for an element the type does not
///   declare at any level. `tt:VideoSourceConfigurationOptions` declares
///   `MaximumNumberOfProfiles` as an `xs:attribute`, and `max_limit` read an
///   element. Both were client defects — the seventh and eighth of the sweep.
///
///   **Only one of the six was ever visible here, and the count is the weakest
///   evidence in this list.** Nothing in this file reads attributes at all, so
///   a closed row proves the element is gone and says nothing about whether the
///   attribute that replaced it is spelled right or read at all. On top of that:
///
///   - `GovLengthRange` and `FrameRateRange` moved **nothing**. Both are real
///     `tt:` elements on `H264Options` / `Mpeg4Options` — the *Media1* options
///     types, which genuinely declare their ranges as elements and were
///     correct — so `UNKNOWN-NAME` could not fire; and
///     `tt:VideoEncoder2ConfigurationOptions` carries an `xs:any`, which sets
///     `Ty::wild` and suppresses `UNKNOWN-CHILD` for the whole type. This is
///     §5.5's blind spot again, on the sibling type.
///   - `FrameRatesSupported` moved nothing either, for a third reason: the mock
///     had never emitted it under any spelling.
///   - `ProfilesSupported` was the one visible name, undeclared as an element
///     anywhere (Media1 says `H264ProfilesSupported`).
///   - `MaximumNumberOfProfiles` reported as `UNKNOWN-CHILD` and never as
///     `UNKNOWN-NAME`, because it *is* an element on the unrelated
///     `tt:ProfileCapabilities`, and `tt:VideoSourceConfigurationOptions` has no
///     `xs:any` to suppress the child rule.
///
///   Two of the attributes are `xs:list`-typed, so the fix changed the parse's
///   *cardinality*: `tt:StringAttrList` and `tt:FloatList` are each
///   `<xs:list itemType="…"/>`, one attribute for the whole collection.
///   `media2_encoder_options_lists_are_attributes` and
///   `video_source_options_max_profiles_is_an_attribute` in
///   `tests/mock_workflow.rs` are what assert the six members, by reading the
///   values back through the client; reverting either side reddens both.
const PINS: &[(&str, usize)] = &[
    ("WRONG-NS", 0),
    ("MISSING-REQUIRED", 7),
    ("UNKNOWN-NAME", 4),
    ("UNKNOWN-CHILD", 2),
    ("ORDER", 1),
];

/// Floors on what the run actually covered.
///
/// Without these the whole check passes vacuously: an index that loaded nothing
/// declares nothing missing, and a corpus of faults has no elements to judge.
/// Measured 1314 / 608 / 105 at `8091892`; the floors sit below that so an
/// ONVIF release with a few more types does not fail the build, but a schema
/// directory half-copied does.
const TYPE_FLOOR: usize = 1_200;
const ANCHORED_FLOOR: usize = 550;
/// Responses carrying a payload rather than a SOAP fault.
const PAYLOAD_FLOOR: usize = 100;

// ── A minimal namespace-aware XML tree ───────────────────────────────────────
//
// `oxvif::soap::XmlNode` cannot be used here: it strips namespaces, which is
// precisely the property under test. quick-xml resolves element names against
// in-scope declarations, but not QName *values* like `type="tt:IntRange"`, so
// the prefix map is collected per file and applied by hand.

type Qn = (String, String);

#[derive(Debug)]
struct Node {
    ns: String,
    local: String,
    attrs: Vec<(String, String)>,
    kids: Vec<Node>,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn xs_kids<'a>(&'a self, local: &'a str) -> impl Iterator<Item = &'a Node> {
        self.kids
            .iter()
            .filter(move |k| k.ns == XS && k.local == local)
    }
}

struct Raw {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<Raw>,
}

/// Parse to a raw tree plus the file's flat prefix map.
///
/// Flat rather than scoped because these documents declare every prefix on the
/// root element; a scoped map would be more code for no measured difference.
fn parse(xml: &str) -> Result<(Raw, HashMap<String, String>), String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut prefixes: HashMap<String, String> = HashMap::new();
    let mut stack: Vec<Raw> = Vec::new();
    let mut root: Option<Raw> = None;

    fn place(stack: &mut [Raw], root: &mut Option<Raw>, n: Raw) {
        match stack.last_mut() {
            Some(p) => p.kids.push(n),
            None => *root = Some(n),
        }
    }

    // `Attribute::unescape_value` is `#[cfg(not(feature = "encoding"))]`, and
    // the dev-dependency turns `encoding` on precisely so the test build
    // matches a downstream crate that does — see docs/dependency-pitfalls.md.
    // The decoder-taking form is the one that exists in both builds.
    let decoder = reader.decoder();

    fn start(
        e: &quick_xml::events::BytesStart,
        px: &mut HashMap<String, String>,
        decoder: quick_xml::encoding::Decoder,
    ) -> Raw {
        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        let mut attrs = Vec::new();
        for a in e.attributes().flatten() {
            let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            let v = a
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            if k == "xmlns" {
                px.insert(String::new(), v.clone());
            } else if let Some(p) = k.strip_prefix("xmlns:") {
                px.insert(p.to_string(), v.clone());
            }
            attrs.push((k, v));
        }
        Raw {
            name,
            attrs,
            kids: Vec::new(),
        }
    }

    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            quick_xml::events::Event::Start(e) => stack.push(start(&e, &mut prefixes, decoder)),
            quick_xml::events::Event::Empty(e) => {
                let n = start(&e, &mut prefixes, decoder);
                place(&mut stack, &mut root, n);
            }
            quick_xml::events::Event::End(_) => {
                let n = stack.pop().ok_or("unbalanced end tag")?;
                place(&mut stack, &mut root, n);
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    root.map(|r| (r, prefixes)).ok_or("no root element".into())
}

fn resolve_tree(r: Raw, px: &HashMap<String, String>) -> Node {
    let (prefix, local) = match r.name.split_once(':') {
        Some((p, l)) => (p, l),
        None => ("", r.name.as_str()),
    };
    Node {
        ns: px.get(prefix).cloned().unwrap_or_default(),
        local: local.to_string(),
        attrs: r.attrs,
        kids: r.kids.into_iter().map(|k| resolve_tree(k, px)).collect(),
    }
}

fn qname(raw: &str, px: &HashMap<String, String>, default_ns: &str) -> Option<Qn> {
    if raw.is_empty() {
        return None;
    }
    Some(match raw.split_once(':') {
        Some((p, l)) => (px.get(p).cloned().unwrap_or_default(), l.to_string()),
        None => (default_ns.to_string(), raw.to_string()),
    })
}

// ── The index ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Child {
    ns: String,
    name: String,
    ty: Option<Qn>,
    min: u32,
    /// The `xs:choice` this child belongs to, if any. Members of one choice
    /// share a position in the parent's sequence and are satisfied by *one* of
    /// them being present — walking a choice as a sequence made the checker
    /// demand every alternative at once, which was a false positive on the one
    /// response that uses one.
    group: Option<u32>,
}

#[derive(Default)]
struct Ty {
    base: Option<Qn>,
    kids: Vec<Child>,
    /// The type, or a base of it, has an `xs:any`. A wildcarded type legally
    /// accepts unknown children, so "unknown child" cannot be reported for it.
    wild: bool,
    /// choice id -> its `minOccurs`.
    groups: BTreeMap<u32, u32>,
}

#[derive(Default)]
struct Index {
    types: HashMap<Qn, Ty>,
    globals: HashMap<Qn, Option<Qn>>,
    declared: HashSet<Qn>,
    known_ns: HashSet<String>,
    next_group: u32,
}

fn min_occurs(n: &Node) -> u32 {
    match n.attr("minOccurs") {
        None => 1,
        Some(v) => v.parse().unwrap_or(0),
    }
}

/// A readable name for an inline `complexType`.
///
/// The event-service responses are almost entirely inline types, so this is
/// exactly where an opaque serial number made a message useless.
fn anon_name(owner: &str, local: &str) -> String {
    if owner.is_empty() {
        format!("<{local}>")
    } else {
        format!("<{owner}/{local}>")
    }
}

type Px = HashMap<String, String>;

impl Index {
    fn load_schema_node(&mut self, sch: &Node, px: &Px) {
        let tns = sch.attr("targetNamespace").unwrap_or("").to_string();
        let efd = sch
            .attr("elementFormDefault")
            .unwrap_or("unqualified")
            .to_string();
        self.known_ns.insert(tns.clone());

        // Global elements first: an `xs:element ref=` may point at one.
        for el in sch.xs_kids("element") {
            let Some(nm) = el.attr("name") else { continue };
            let nm = nm.to_string();
            let mut ty = el.attr("type").and_then(|v| qname(v, px, &tns));
            if ty.is_none()
                && let Some(inline) = el.xs_kids("complexType").next()
            {
                let qn = (tns.clone(), anon_name("", &nm));
                let t = self.parse_type(inline, px, &tns, &efd, &nm);
                self.types.insert(qn.clone(), t);
                ty = Some(qn);
            }
            self.globals.insert((tns.clone(), nm.clone()), ty);
            self.declared.insert((tns.clone(), nm));
        }
        for ct in sch.xs_kids("complexType") {
            if let Some(nm) = ct.attr("name") {
                let nm = nm.to_string();
                let t = self.parse_type(ct, px, &tns, &efd, &nm);
                self.types.insert((tns.clone(), nm), t);
            }
        }
    }

    fn parse_type(&mut self, node: &Node, px: &Px, tns: &str, efd: &str, owner: &str) -> Ty {
        let mut t = Ty::default();
        self.walk_type(node, px, tns, efd, owner, None, &mut t);
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_type(
        &mut self,
        n: &Node,
        px: &Px,
        tns: &str,
        efd: &str,
        owner: &str,
        group: Option<u32>,
        t: &mut Ty,
    ) {
        for c in &n.kids {
            if c.ns != XS {
                continue;
            }
            match c.local.as_str() {
                // The anonymous type of an enclosing xs:element; `add_element`
                // handles it, and descending here would flatten it into the
                // parent.
                "complexType" | "annotation" => {}
                "extension" => {
                    t.base = c.attr("base").and_then(|v| qname(v, px, tns));
                    self.walk_type(c, px, tns, efd, owner, group, t);
                }
                "any" => t.wild = true,
                "choice" => {
                    self.next_group += 1;
                    let g = self.next_group;
                    t.groups
                        .insert(g, u32::from(c.attr("minOccurs") != Some("0")));
                    self.walk_type(c, px, tns, efd, owner, Some(g), t);
                }
                "element" => self.add_element(c, px, tns, efd, owner, group, t),
                _ => self.walk_type(c, px, tns, efd, owner, group, t),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_element(
        &mut self,
        c: &Node,
        px: &Px,
        tns: &str,
        efd: &str,
        owner: &str,
        group: Option<u32>,
        t: &mut Ty,
    ) {
        if let Some(r) = c.attr("ref") {
            let Some((ns, local)) = qname(r, px, tns) else {
                return;
            };
            let ty = self
                .globals
                .get(&(ns.clone(), local.clone()))
                .cloned()
                .flatten();
            self.declared.insert((ns.clone(), local.clone()));
            t.kids.push(Child {
                ns,
                name: local,
                ty,
                min: min_occurs(c),
                group,
            });
            return;
        }
        let local = c.attr("name").unwrap_or("").to_string();
        let ns = if efd == "qualified" {
            tns.to_string()
        } else {
            String::new()
        };
        let mut ty = c.attr("type").and_then(|v| qname(v, px, tns));
        if ty.is_none()
            && let Some(inline) = c.xs_kids("complexType").next()
        {
            let qn = (tns.to_string(), anon_name(owner, &local));
            let sub_owner = format!("{owner}/{local}");
            let sub = self.parse_type(inline, px, tns, efd, &sub_owner);
            self.types.insert(qn.clone(), sub);
            ty = Some(qn);
        }
        self.declared.insert((ns.clone(), local.clone()));
        t.kids.push(Child {
            ns,
            name: local,
            ty,
            min: min_occurs(c),
            group,
        });
    }

    fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (raw, px) = parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let root = resolve_tree(raw, &px);
        if root.ns == XS && root.local == "schema" {
            self.load_schema_node(&root, &px);
        } else {
            // wsdl:definitions — the response wrapper elements live in each
            // service WSDL's inline schema, not in onvif.xsd.
            for tnode in root
                .kids
                .iter()
                .filter(|k| k.ns == WSDL && k.local == "types")
            {
                for sch in tnode.xs_kids("schema") {
                    self.load_schema_node(sch, &px);
                }
            }
        }
        Ok(())
    }

    /// Children, wildcard and choice groups for a type, following `xs:extension`.
    fn resolve(&self, ty: Option<&Qn>) -> (Vec<Child>, bool, BTreeMap<u32, u32>) {
        fn go(
            ix: &Index,
            ty: Option<&Qn>,
            seen: &mut HashSet<Qn>,
        ) -> (Vec<Child>, bool, BTreeMap<u32, u32>) {
            let Some(q) = ty else {
                return (Vec::new(), false, BTreeMap::new());
            };
            if seen.contains(q) {
                return (Vec::new(), false, BTreeMap::new());
            }
            let Some(t) = ix.types.get(q) else {
                return (Vec::new(), false, BTreeMap::new());
            };
            seen.insert(q.clone());
            let (mut kids, mut wild, mut groups) = (Vec::new(), t.wild, t.groups.clone());
            if let Some(b) = &t.base {
                let (bk, bw, bg) = go(ix, Some(b), seen);
                kids.extend(bk);
                wild = wild || bw;
                groups.extend(bg);
            }
            kids.extend(t.kids.iter().cloned());
            (kids, wild, groups)
        }
        go(self, ty, &mut HashSet::new())
    }
}

// ── Findings ─────────────────────────────────────────────────────────────────

struct Finding {
    doc: String,
    kind: &'static str,
    msg: String,
}

#[derive(Default)]
struct Run {
    findings: Vec<Finding>,
    anchored: usize,
    unanchored_children: usize,
    /// Roots with no global element declaration. SOAP faults belong here and
    /// are not defects; anything else needs explaining before it is dismissed.
    unanchored_roots: Vec<(String, String, String)>,
    /// Elements a WRONG-NS row already explains, so the name check stays quiet.
    ns_explained: HashSet<(String, String, String, String)>,
}

impl Run {
    fn add(&mut self, doc: &str, kind: &'static str, msg: String) {
        self.findings.push(Finding {
            doc: doc.to_string(),
            kind,
            msg,
        });
    }

    fn check_anchored(&mut self, ix: &Index, doc: &str, node: &Node, ty: &Qn, path: &str) {
        let (kids, wild, groups) = ix.resolve(Some(ty));
        let by: HashMap<Qn, &Child> = kids
            .iter()
            .map(|k| ((k.ns.clone(), k.name.clone()), k))
            .collect();
        let mut by_local: HashMap<&str, &Child> = HashMap::new();
        for k in &kids {
            by_local.entry(k.name.as_str()).or_insert(k);
        }
        let order: Vec<Qn> = kids
            .iter()
            .map(|k| (k.ns.clone(), k.name.clone()))
            .collect();

        // A choice's members share the position of the first of them, so an
        // alternative standing where a sibling was declared is not an ordering
        // violation.
        let mut first_of_group: HashMap<u32, usize> = HashMap::new();
        let mut pos: HashMap<Qn, usize> = HashMap::new();
        for (i, k) in kids.iter().enumerate() {
            if let Some(g) = k.group {
                first_of_group.entry(g).or_insert(i);
            }
            let p = k
                .group
                .and_then(|g| first_of_group.get(&g).copied())
                .unwrap_or(i);
            pos.entry((k.ns.clone(), k.name.clone())).or_insert(p);
        }

        let obs: Vec<Qn> = node
            .kids
            .iter()
            .map(|c| (c.ns.clone(), c.local.clone()))
            .collect();
        self.anchored += 1;

        // Wrong namespace — the parent declares this local name, elsewhere. One
        // row covers all three symptoms it used to produce.
        let mut satisfied: HashSet<Qn> = HashSet::new();
        let mut resolved: HashMap<Qn, Qn> = HashMap::new();
        let mut unknown: BTreeSet<String> = BTreeSet::new();
        for o in &obs {
            if by.contains_key(o) || !ix.known_ns.contains(&o.0) {
                continue;
            }
            match by_local.get(o.1.as_str()) {
                Some(want) => {
                    self.add(
                        doc,
                        "WRONG-NS",
                        format!(
                            "{path}/{} — emitted in {}, {} declares it in {}",
                            o.1, o.0, ty.1, want.ns
                        ),
                    );
                    satisfied.insert((want.ns.clone(), want.name.clone()));
                    resolved.insert(o.clone(), (want.ns.clone(), want.name.clone()));
                    self.ns_explained.insert((
                        doc.to_string(),
                        path.to_string(),
                        o.0.clone(),
                        o.1.clone(),
                    ));
                }
                None => {
                    unknown.insert(o.1.clone());
                }
            }
        }
        if !unknown.is_empty() && !wild {
            let names: Vec<&String> = unknown.iter().collect();
            self.add(
                doc,
                "UNKNOWN-CHILD",
                format!("{path}: {names:?} not declared by {}", ty.1),
            );
        }

        let mut prev: i64 = -1;
        for o in &obs {
            let key = resolved.get(o).unwrap_or(o);
            let Some(&i) = pos.get(key) else { continue };
            if (i as i64) < prev {
                let emitted: Vec<&str> = obs
                    .iter()
                    .map(|x| resolved.get(x).unwrap_or(x))
                    .filter(|x| pos.contains_key(*x))
                    .map(|x| x.1.as_str())
                    .collect();
                let declared: Vec<&str> = order.iter().map(|x| x.1.as_str()).collect();
                self.add(
                    doc,
                    "ORDER",
                    format!(
                        "{path} ({}): emitted {emitted:?}, schema {declared:?}",
                        ty.1
                    ),
                );
                break;
            }
            prev = i as i64;
        }

        let present: HashSet<&Qn> = obs.iter().chain(satisfied.iter()).collect();
        let mut missing: BTreeSet<String> = BTreeSet::new();
        for k in &kids {
            if k.group.is_none() && k.min >= 1 && !present.contains(&(k.ns.clone(), k.name.clone()))
            {
                missing.insert(k.name.clone());
            }
        }
        for (g, gmin) in &groups {
            if *gmin < 1 {
                continue;
            }
            let members: Vec<&Child> = kids.iter().filter(|k| k.group == Some(*g)).collect();
            if !members.is_empty()
                && !members
                    .iter()
                    .any(|k| present.contains(&(k.ns.clone(), k.name.clone())))
            {
                let names: Vec<&str> = members.iter().map(|k| k.name.as_str()).collect();
                missing.insert(format!("one of {}", names.join("|")));
            }
        }
        if !missing.is_empty() {
            let names: Vec<&String> = missing.iter().collect();
            self.add(
                doc,
                "MISSING-REQUIRED",
                format!("{path} ({}): {names:?}", ty.1),
            );
        }

        for c in &node.kids {
            let key = (c.ns.clone(), c.local.clone());
            let child = by
                .get(&key)
                .or_else(|| resolved.get(&key).and_then(|k| by.get(k)));
            match child
                .and_then(|k| k.ty.as_ref())
                .filter(|t| ix.types.contains_key(*t))
            {
                Some(t) => {
                    let t = t.clone();
                    self.check_anchored(ix, doc, c, &t, &format!("{path}/{}", c.local));
                }
                None => self.unanchored_children += 1,
            }
        }
    }

    fn check_names(&mut self, ix: &Index, doc: &str, node: &Node, path: &str) {
        for c in &node.kids {
            let key = (c.ns.clone(), c.local.clone());
            if ix.known_ns.contains(&c.ns)
                && !ix.declared.contains(&key)
                && !self.ns_explained.contains(&(
                    doc.to_string(),
                    path.to_string(),
                    c.ns.clone(),
                    c.local.clone(),
                ))
            {
                self.add(
                    doc,
                    "UNKNOWN-NAME",
                    format!("{path}/{} — not declared in {}", c.local, c.ns),
                );
            }
            self.check_names(ix, doc, c, &format!("{path}/{}", c.local));
        }
    }
}

// ── The corpus, generated in process ─────────────────────────────────────────
//
// Same extraction `mock_handles_every_action_the_client_can_send` uses: read
// the action URIs straight out of the client sources, so an operation added
// tomorrow is checked without anyone remembering this file.

const CLIENT_SOURCES: &[(&str, &str)] = &[
    ("device", include_str!("../src/client/device.rs")),
    ("events", include_str!("../src/client/events.rs")),
    ("imaging", include_str!("../src/client/imaging.rs")),
    ("media", include_str!("../src/client/media.rs")),
    ("media2", include_str!("../src/client/media2.rs")),
    ("ptz", include_str!("../src/client/ptz.rs")),
    ("recording", include_str!("../src/client/recording.rs")),
];

fn action_uris(src: &str) -> Vec<&str> {
    const STARTS: [&str; 2] = ["\"http://www.onvif.org/", "\"http://docs.oasis-open.org/"];
    let mut out = Vec::new();
    for start in STARTS {
        let mut rest = src;
        while let Some(i) = rest.find(start) {
            let after = &rest[i + 1..];
            let Some(end) = after.find('"') else { break };
            out.push(&after[..end]);
            rest = &after[end..];
        }
    }
    out
}

fn is_action(uri: &str) -> bool {
    let tail = uri.rsplit('/').next().unwrap_or("");
    tail.starts_with(|c: char| c.is_ascii_uppercase())
        && tail.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Bodies for the operations that need a token before they will answer with a
/// payload rather than a fault. These are the mock's *own* seeded tokens.
///
/// Every operation missing from here answers with a fault and contributes no
/// shape evidence — see [`PAYLOAD_FLOOR`].
fn body_for(op: &str) -> &'static str {
    match op {
        "GetVideoEncoderConfigurationOptions" | "GetVideoEncoderConfiguration" => {
            "<ConfigurationToken>VEC_1</ConfigurationToken>"
        }
        "GetVideoSourceConfigurationOptions" | "GetVideoSourceConfiguration" => {
            "<ConfigurationToken>VSC_1</ConfigurationToken>"
        }
        "GetAudioEncoderConfigurationOptions" | "GetAudioEncoderConfiguration" => {
            "<ConfigurationToken>AEC_1</ConfigurationToken>"
        }
        "GetAudioSourceConfiguration" => "<ConfigurationToken>ASC_1</ConfigurationToken>",
        "GetMetadataConfiguration" | "GetMetadataConfigurationOptions" => {
            "<ConfigurationToken>MetaConf_1</ConfigurationToken>"
        }
        "GetProfile" => "<ProfileToken>Profile_1</ProfileToken>",
        "GetStreamUri" | "GetSnapshotUri" => {
            "<ProfileToken>Profile_1</ProfileToken><Token>Profile_1</Token>"
        }
        "GetConfiguration" => "<PTZConfigurationToken>PTZConfig_1</PTZConfigurationToken>",
        "GetConfigurationOptions" => "<ConfigurationToken>PTZConfig_1</ConfigurationToken>",
        "GetNode" => "<NodeToken>PTZNode_1</NodeToken>",
        "GetCompatibleConfigurations" | "GetPresets" | "GetPresetTours" => {
            "<ProfileToken>Profile_1</ProfileToken>"
        }
        "GetPresetTour" => {
            "<ProfileToken>Profile_1</ProfileToken><PresetTourToken>Tour_1</PresetTourToken>"
        }
        "GetPresetTourOptions" => "<ProfileToken>Profile_1</ProfileToken>",
        "GetOSDs" | "GetOSDOptions" => "<ConfigurationToken>VSC_1</ConfigurationToken>",
        "GetOSD" => "<OSDToken>OSD_1</OSDToken>",
        "GetRecordingJobState" => "<JobToken>Job_001</JobToken>",
        "GetReplayUri" => "<RecordingToken>Rec_001</RecordingToken>",
        "GetRecordingSearchResults" => "<SearchToken>Search_1</SearchToken>",
        _ => "",
    }
}

/// Imaging and PTZ status take a source or profile token under several names.
fn extra_body(op: &str) -> &'static str {
    match op {
        "GetImagingSettings" | "GetOptions" | "GetMoveOptions" | "GetServiceCapabilities" => {
            "<VideoSourceToken>VS_1</VideoSourceToken>"
        }
        "GetStatus" => {
            "<VideoSourceToken>VS_1</VideoSourceToken><ProfileToken>Profile_1</ProfileToken>"
        }
        _ => "",
    }
}

// ── The test ─────────────────────────────────────────────────────────────────

fn schema_files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("xsd") | Some("wsdl")
            )
        })
        .collect();
    out.sort();
    out
}

#[tokio::test]
#[ignore = "needs the ONVIF schema set; see the module docs and CLAUDE.md's publishing checklist"]
async fn mock_output_matches_the_onvif_schema() {
    let Ok(dir) = std::env::var("OXVIF_ONVIF_SCHEMA") else {
        // Loud on purpose: a run that checked nothing must not read like a pass.
        eprintln!(
            "SKIPPED — OXVIF_ONVIF_SCHEMA is unset, so no schema was read and \
             NOTHING was checked.\n\
             Point it at a directory holding the ONVIF service WSDLs plus \
             onvif.xsd and common.xsd.\n\
             Nothing schema-derived is committed to this repository \
             (docs/active/schema-shape-plan-2026-08.md §4, D2)."
        );
        return;
    };
    let dir = PathBuf::from(&dir);
    let files = schema_files(&dir);
    // Set but wrong is a failure, not a skip: the run was asked for.
    assert!(
        files.len() >= 8,
        "OXVIF_ONVIF_SCHEMA={} holds {} .xsd/.wsdl files; the set needs at least \
         the six service WSDLs plus onvif.xsd and common.xsd. onvif.xsd alone \
         anchors 19% of the output and is not the schema.",
        dir.display(),
        files.len()
    );

    let mut ix = Index::default();
    // Twice: an `xs:element ref=` may point at a global declared in a file
    // loaded later. The second pass overwrites with identical values.
    for _ in 0..2 {
        for f in &files {
            ix.load_file(f).expect("schema file parses");
        }
    }
    assert!(
        ix.types.len() >= TYPE_FLOOR,
        "index holds {} types, floor is {TYPE_FLOOR}. An index that loads \
         little declares little missing, so every check below would pass \
         vacuously.",
        ix.types.len()
    );

    // Corpus: one response per action the client can send.
    let transport = MockTransport::new();
    let mut docs: Vec<(String, String)> = Vec::new();
    for (service, src) in CLIENT_SOURCES {
        for uri in action_uris(src) {
            if !is_action(uri) {
                continue;
            }
            let op = uri.rsplit('/').next().unwrap_or("");
            let body = format!("{}{}", body_for(op), extra_body(op));
            let xml = transport
                .soap_post("http://mock", uri, body)
                .await
                .unwrap_or_else(|e| panic!("{service}/{op}: mock transport failed: {e}"));
            docs.push((format!("{service}__{op}"), xml));
        }
    }

    let mut run = Run::default();
    let mut payloads = 0;
    for (name, xml) in &docs {
        let (raw, px) = parse(xml).unwrap_or_else(|e| panic!("{name}: {e}"));
        let root = resolve_tree(raw, &px);
        let Some(body) = root
            .kids
            .iter()
            .find(|k| k.ns == SOAP_ENV && k.local == "Body")
        else {
            continue;
        };
        for resp in &body.kids {
            if resp.local != "Fault" {
                payloads += 1;
            }
            let key = (resp.ns.clone(), resp.local.clone());
            // Anchored first: it fills ns_explained, which the name check reads.
            match ix.globals.get(&key).cloned().flatten() {
                Some(ty) if ix.types.contains_key(&ty) => {
                    run.check_anchored(&ix, name, resp, &ty, &resp.local)
                }
                _ => run
                    .unanchored_roots
                    .push((name.clone(), resp.ns.clone(), resp.local.clone())),
            }
            run.check_names(&ix, name, resp, &resp.local);
        }
    }

    // ── Coverage floors, before any finding is believed ──────────────────────
    assert!(
        payloads >= PAYLOAD_FLOOR,
        "only {payloads} of {} responses carried a payload rather than a SOAP \
         fault (floor {PAYLOAD_FLOOR}). A fault contributes no shape evidence, \
         so this check silently shrinks as operations start refusing the bodies \
         in `body_for`.",
        docs.len()
    );
    assert!(
        run.anchored >= ANCHORED_FLOOR,
        "only {} subtrees anchored (floor {ANCHORED_FLOOR}). Anchoring needs the \
         response wrapper element, which lives in the service WSDL — a missing \
         WSDL shows up here rather than as findings.",
        run.anchored
    );

    // ── Report ──────────────────────────────────────────────────────────────
    // Rolled up: identical rows across documents collapse, keeping the count.
    // Without this one wrong-namespace element reads as three defects.
    let mut rolled: BTreeMap<(&str, String), Vec<String>> = BTreeMap::new();
    for f in &run.findings {
        rolled
            .entry((f.kind, f.msg.clone()))
            .or_default()
            .push(f.doc.clone());
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (kind, _) in rolled.keys() {
        *counts.entry(kind).or_default() += 1;
    }

    let faults: Vec<_> = run
        .unanchored_roots
        .iter()
        .filter(|r| r.2 == "Fault")
        .collect();
    let other: BTreeSet<String> = run
        .unanchored_roots
        .iter()
        .filter(|r| r.2 != "Fault")
        .map(|r| format!("{} ({})", r.2, r.1))
        .collect();

    println!(
        "schema: {} files, {} types, {} declared elements, {} namespaces",
        files.len(),
        ix.types.len(),
        ix.declared.len(),
        ix.known_ns.len()
    );
    println!(
        "corpus: {} responses, {} with a payload, {} roots anchored, {} faults, \
         {} children skipped",
        docs.len(),
        payloads,
        run.anchored,
        faults.len(),
        run.unanchored_children
    );
    if !other.is_empty() {
        println!(
            "  unanchored non-fault roots: {}",
            other.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    println!(
        "findings: {} raw, {} distinct {:?}",
        run.findings.len(),
        rolled.len(),
        counts
    );
    for ((kind, msg), where_) in &rolled {
        let at = if where_.len() == 1 {
            where_[0].clone()
        } else {
            format!("{} responses", where_.len())
        };
        println!("  {kind:<16} [{at}] {msg}");
    }

    // ── Every non-fault root must anchor ─────────────────────────────────────
    //
    // A root that anchors to no global element is a response the schema set
    // does not declare *in the namespace the mock put it in*. Once the schema
    // set is complete that means one of two things, and both are defects:
    // the element name is wrong, or — the interesting one — the operation is
    // addressed to the wrong service.
    //
    // This is a stronger claim than the pins and is asserted separately,
    // because it is not a count: it went from "one, cause unknown" to zero, and
    // the one was `GetDigitalInputs` sent to device management, which
    // `deviceio.wsdl` declares and `devicemgmt.wsdl` does not. It cost a single
    // `UNKNOWN-NAME` row, so the pins alone would have made it look like the
    // smallest finding in the set rather than the only client-facing one.
    //
    // Faults are excluded: there is no soap-envelope schema in the set, so all
    // 50 of them are unanchored by construction.
    assert!(
        other.is_empty(),
        "\n{} response root(s) anchor to no declared element: {}\n\
         Each is either a misnamed response or an operation sent to the wrong \
         service — check which WSDL declares the element before assuming the \
         schema set is incomplete.",
        other.len(),
        other.iter().cloned().collect::<Vec<_>>().join(", ")
    );

    // ── Pins ────────────────────────────────────────────────────────────────
    let got: Vec<(&str, usize)> = PINS
        .iter()
        .map(|(k, _)| (*k, counts.get(k).copied().unwrap_or(0)))
        .collect();
    let unpinned: Vec<&str> = counts
        .keys()
        .filter(|k| !PINS.iter().any(|(p, _)| p == *k))
        .copied()
        .collect();
    assert!(
        unpinned.is_empty(),
        "finding kinds with no pin: {unpinned:?}. Add them to PINS with the \
         count this run produced, and say in the commit why they appeared."
    );
    assert_eq!(
        got,
        PINS.to_vec(),
        "\nthe distinct finding counts moved.\n\
         Lower is a fix — update PINS in the same commit as the fix, and update \
         `docs/active/mock-schema-conformance-2026-08.md` §1 and the lab's \
         NOTES.md run 3, which both quote these numbers.\n\
         Higher is a regression — read the rows printed above before touching \
         PINS.\n\
         Different schema release — the counts are pinned against one set; say \
         so rather than editing them silently."
    );
}
