//! Exact typed-field regression evidence over frozen parameter-aware captures.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CorpusError, CorpusLimits, FixtureCorpus, OpOutcome, ParseStatus, RecordingEvidence, SurfaceOp,
};

/// Provenance and sharing decisions supplied by the corpus owner, not inferred.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusIdentity {
    /// Pseudonymous device label.
    pub pseudonym: String,
    /// Vendor, if permitted to retain.
    pub vendor: Option<String>,
    /// Model, if permitted to retain.
    pub model: Option<String>,
    /// Firmware, if permitted to retain.
    pub firmware: Option<String>,
    /// Actual recording source revision; do not invent a revision for old captures.
    pub recording_commit: String,
    /// How the input was produced (e.g. synthetic, device capture, reviewed copy).
    pub provenance: String,
    /// Explicit permitted sharing scope. This is a declaration, not a sanitizer.
    pub sharing_scope: String,
}

/// One exact frozen request and the typed values a reviewer expects from it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedExchange {
    /// Zero-based ordinal in the frozen fixtures file, including duplicates.
    pub ordinal: usize,
    /// Full Action URI, never an operation suffix.
    pub action: String,
    /// Exact sanitized request from this frozen recording; meaningful tokens remain.
    pub request_raw: String,
    /// JSON Pointer -> exact typed value. Empty pointer asserts the whole value.
    /// At least one assertion is required; parser success alone is insufficient.
    pub fields: BTreeMap<String, Value>,
}

/// Per-operation sweep outcome; failed transport observations are separate below.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepObservation {
    /// Selected operation, including expanded prerequisites.
    pub operation: SurfaceOp,
    /// Driver result; Failed can include SOAP/parser/service failures.
    pub outcome: OpOutcome,
}

/// Reviewable expectations stored beside a private, frozen `fixtures.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusManifest {
    /// Currently 1; incompatible versions fail explicitly.
    pub version: u32,
    /// Recording identity and provenance.
    pub identity: CorpusIdentity,
    /// Full Actions expected in this corpus, including recorded initialization.
    pub selected_actions: Vec<String>,
    /// Actual per-operation sweep disposition; not inferred from parser success.
    pub sweep: Vec<SweepObservation>,
    /// Actual opt-in transport observations, including any explicit omissions.
    pub transport: RecordingEvidence,
    /// One expectation per stored row, with exact request identity and values.
    pub exchanges: Vec<ExpectedExchange>,
}

impl CorpusManifest {
    /// Load an explicitly selected manifest with an actual-byte budget.
    pub fn load(path: impl AsRef<Path>, max_bytes: usize) -> Result<Self, CorpusError> {
        let bytes = super::corpus::read_bounded(path.as_ref(), max_bytes)?;
        let manifest: Self =
            serde_json::from_slice(&bytes).map_err(|_| CorpusError::InvalidJson)?;
        if manifest.version != 1 {
            return Err(CorpusError::InvalidJson);
        }
        Ok(manifest)
    }
}

/// Controlled result for one frozen exchange. Fault/refusal is not parser success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionOutcome {
    /// Exact identity and all selected typed fields matched.
    Passed,
    /// Parsed, but one or more expected values differed or were absent.
    FieldMismatch,
    /// Expected ordinal, Action or frozen request did not match.
    IdentityMismatch,
    /// Typed parser rejected the response.
    Failed,
    /// Device declined with a SOAP Fault.
    Faulted,
    /// No typed parser was mapped.
    Unverified,
    /// No explicit typed field assertions were supplied.
    NoAssertions,
}

/// Ordinal-based regression summary without response/Fault/error text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionRow {
    /// Frozen fixture ordinal.
    pub ordinal: usize,
    /// Controlled verdict.
    pub outcome: RegressionOutcome,
    /// Expected JSON pointers whose selected values did not match.
    pub differing_fields: Vec<String>,
}

/// Regression evidence, not independent protocol or device conformance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionReport {
    /// One result per expectation, in manifest order.
    pub rows: Vec<RegressionRow>,
    /// Coverage/metadata errors, represented as controlled labels.
    pub coverage_errors: Vec<String>,
    /// Transport failures observed by the recording tap, separate from SOAP Faults.
    pub transport_failed: usize,
    /// Recording events omitted due to the evidence budget.
    pub transport_omitted: usize,
    /// Transport collection was disabled or unavailable.
    pub transport_unavailable: bool,
    /// Selected sweep operations that failed or were skipped.
    pub incomplete_operations: usize,
}

impl RegressionReport {
    /// Strict local acceptance: complete nonempty coverage, explicit values,
    /// all rows passed, all selected sweep operations recorded and no transport
    /// failure/omission. This deliberately does not call ParseReport::all_parsed.
    pub fn passed(&self) -> bool {
        !self.rows.is_empty()
            && self.coverage_errors.is_empty()
            && self
                .rows
                .iter()
                .all(|row| row.outcome == RegressionOutcome::Passed)
            && self.transport_failed == 0
            && self.transport_omitted == 0
            && !self.transport_unavailable
            && self.incomplete_operations == 0
    }
}

/// Verify exact typed values for every frozen parameter-aware exchange.
/// The default CorpusLimits also apply to caller-constructed inputs.
/// Raw request equality establishes which frozen exchange was reviewed; it does
/// not prove namespace/schema conformance. Faulted/Failed/Unverified remain
/// distinct and none can produce a passing regression report.
pub async fn verify_corpus(
    corpus: &FixtureCorpus,
    manifest: &CorpusManifest,
) -> Result<RegressionReport, CorpusError> {
    corpus.validate(CorpusLimits::default())?;
    if manifest.exchanges.len() > 512
        || manifest.selected_actions.len() > 512
        || manifest.sweep.len() > 512
    {
        return Err(CorpusError::Limit {
            resource: "manifest rows",
            limit: 512,
        });
    }
    let mut report = RegressionReport {
        rows: Vec::new(),
        coverage_errors: Vec::new(),
        transport_failed: manifest.transport.failures.len(),
        transport_omitted: manifest.transport.omitted,
        transport_unavailable: manifest.transport.unavailable,
        incomplete_operations: manifest
            .sweep
            .iter()
            .filter(|entry| entry.outcome != OpOutcome::Recorded)
            .count(),
    };
    let identity = &manifest.identity;
    if manifest.version != 1
        || identity.pseudonym.is_empty()
        || identity.recording_commit.is_empty()
        || identity.provenance.is_empty()
        || identity.sharing_scope.is_empty()
        || corpus.device != identity.pseudonym
    {
        report.coverage_errors.push("identity_or_provenance".into());
    }
    if manifest.sweep.is_empty() {
        report.coverage_errors.push("missing_sweep".into());
    }
    let selected = manifest.selected_actions.iter().collect::<BTreeSet<_>>();
    let recorded = corpus
        .fixtures
        .iter()
        .map(|row| &row.action)
        .collect::<BTreeSet<_>>();
    if selected.len() != manifest.selected_actions.len()
        || selected != recorded
        || selected.is_empty()
    {
        report.coverage_errors.push("action_coverage".into());
    }
    let mut seen = BTreeSet::new();
    for expected in &manifest.exchanges {
        if expected.fields.len() > 64 || expected.fields.keys().any(|pointer| pointer.len() > 1024)
        {
            return Err(CorpusError::Limit {
                resource: "field assertions",
                limit: 64,
            });
        }
        let mut row = RegressionRow {
            ordinal: expected.ordinal,
            outcome: RegressionOutcome::IdentityMismatch,
            differing_fields: Vec::new(),
        };
        let unique = seen.insert(expected.ordinal);
        let fixture = corpus.fixtures.get(expected.ordinal).filter(|fixture| {
            unique
                && fixture.action == expected.action
                && fixture.request_raw == expected.request_raw
        });
        if let Some(fixture) = fixture {
            let (status, _, value) =
                super::parse::parse_check(&fixture.action, &fixture.response_raw).await;
            row.outcome = match status {
                ParseStatus::Failed => RegressionOutcome::Failed,
                ParseStatus::Faulted => RegressionOutcome::Faulted,
                ParseStatus::Unverified => RegressionOutcome::Unverified,
                ParseStatus::Parsed if expected.fields.is_empty() => {
                    RegressionOutcome::NoAssertions
                }
                ParseStatus::Parsed => {
                    for (pointer, expected_value) in &expected.fields {
                        if value.as_ref().and_then(|value| value.pointer(pointer))
                            != Some(expected_value)
                        {
                            row.differing_fields.push(pointer.clone());
                        }
                    }
                    if row.differing_fields.is_empty() {
                        RegressionOutcome::Passed
                    } else {
                        RegressionOutcome::FieldMismatch
                    }
                }
            };
        }
        report.rows.push(row);
    }
    if seen.len() != corpus.fixtures.len()
        || seen.iter().any(|ordinal| *ordinal >= corpus.fixtures.len())
    {
        report.coverage_errors.push("fixture_coverage".into());
    }
    Ok(report)
}
