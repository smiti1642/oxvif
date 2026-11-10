//! Bounded reference-value comparison, separate from structural quirk diff.
use serde::{Deserialize, Serialize};

use super::{CorpusError, CorpusLimits, Fixture, FixtureCorpus};

/// Explicit disposition for one proven request group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComparisonOutcome {
    /// Declared selected typed projection matched; unexamined fields remain listed.
    Equal,
    /// At least one unmasked selected value differed.
    Different,
    /// Only the right input contained this request identity.
    MissingLeft,
    /// Only the left input contained this request identity.
    MissingRight,
    /// Duplicate request candidates or repeated response tokens prevent alignment.
    Ambiguous,
    /// Selected response identity/namespace/typed data was unusable.
    Unparseable,
    /// Request identity or selected projection cannot be established.
    Unsupported,
    /// A namespace-identified response contained a typed SOAP Fault.
    DeviceFault,
}

/// Controlled reasons; never raw XML, Fault reason or parser error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComparisonReason {
    /// Projection values matched.
    SelectedValuesEqual,
    /// Projection values differed.
    SelectedValuesDiffer,
    /// The request was absent on one side.
    MissingRequest,
    /// More than one candidate belongs to this identity.
    DuplicateRequests,
    /// A request was malformed, unsupported or did not match its full Action.
    RequestIdentity,
    /// No projection is mapped for this exact full Action.
    UnmappedAction,
    /// Response syntax, expanded identity, selected namespace or scalar was invalid.
    ResponseProjection,
    /// QName-valued xsi:type cannot be resolved by this comparison projection.
    UnresolvedQName,
    /// Repeated response identities cannot be aligned.
    DuplicateTokens,
    /// The typed parser recognized a SOAP Fault after namespace guarding.
    DeviceFault,
}

/// Initial selected projection. These are not complete protocol models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComparisonProjection {
    /// Manufacturer/model/firmware/serial/hardware identity fields.
    DeviceInformation,
    /// Hostname and DHCP selection.
    Hostname,
    /// Profile token, name and modeled configuration/source token bindings.
    Profiles,
    /// Configuration token/name/encoding/resolution, optional bitrate and
    /// namespace-checked multicast destination (including Media2's unmodeled value).
    EncoderCore,
}

/// Explicit selected-field mask scoped to an exact full Action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonMask {
    /// One of the six supported full Action URIs.
    pub action: String,
    /// Declared projection pointer (e.g. /name or /profiles/*/name).
    /// Semantic list identity tokens cannot be masked.
    pub field: String,
}

/// Bounds applied before pairing or retaining comparison output.
#[derive(Clone, Copy, Debug)]
pub struct ComparisonLimits {
    /// Input corpus byte/row/XML limits.
    pub input: CorpusLimits,
    /// Maximum output request groups. Default: 1,024.
    pub pairs: usize,
    /// Maximum semantic request comparisons. Default: 16,384.
    pub identity_checks: usize,
    /// Total retained changed/unexamined field paths. Default: 4,096.
    pub field_paths: usize,
    /// Compact serialized report bytes. Default: 256 KiB.
    pub report_bytes: usize,
}
impl Default for ComparisonLimits {
    fn default() -> Self {
        Self {
            input: CorpusLimits::default(),
            pairs: 1024,
            identity_checks: 16384,
            field_paths: 4096,
            report_bytes: 256 * 1024,
        }
    }
}

/// Comparison policy. No fields are masked by default.
#[derive(Clone, Debug, Default)]
pub struct ComparisonOptions {
    /// Input, work and retained-output budgets.
    pub limits: ComparisonLimits,
    /// Explicit field exclusions, validated and recorded in each applicable row.
    pub masks: Vec<ComparisonMask>,
}

/// Explicit refusal; no partial report is returned as Equal.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ComparisonError {
    /// Invalid or oversized corpus input.
    #[error(transparent)]
    Input(#[from] CorpusError),
    /// A zero work/output budget is invalid.
    #[error("invalid comparison limit")]
    InvalidLimit,
    /// Unmapped action, unknown field or semantic token mask.
    #[error("invalid comparison field mask")]
    InvalidMask,
    /// Work or output limit refused the comparison.
    #[error("comparison {resource} limit exceeded ({limit})")]
    Limit {
        /// Controlled budget identity.
        resource: &'static str,
        /// Configured maximum.
        limit: usize,
    },
}

/// One output row, containing ordinals and field paths rather than wire values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonRow {
    /// Exact full Action; canonical fixture keys are not exposed as identities.
    pub action: String,
    /// Zero-based left ordinals, including duplicate candidates.
    pub left: Vec<usize>,
    /// Zero-based right ordinals, including duplicate candidates.
    pub right: Vec<usize>,
    /// Explicit disposition.
    pub outcome: ComparisonOutcome,
    /// Controlled reason.
    pub reason: ComparisonReason,
    /// Declared selected projection, if mapped.
    pub projection: Option<ComparisonProjection>,
    /// Every applicable explicit mask, sorted and deduplicated.
    pub masks: Vec<String>,
    /// Changed projection paths, with no device values.
    pub changed_fields: Vec<String>,
    /// Left wire subtrees/attributes outside the declared projection.
    pub unexamined_left: Vec<String>,
    /// Right wire subtrees/attributes outside the declared projection.
    pub unexamined_right: Vec<String>,
}

/// Offline selected-value evidence, not independent ONVIF conformance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonReport {
    /// Stable order by full Action and input ordinals.
    pub rows: Vec<ComparisonRow>,
}
impl ComparisonReport {
    /// True only for a nonempty report of Equal selected projections.
    /// Unexamined fields may remain even when this returns true.
    /// It cannot infer failed reads or Actions absent from both inputs.
    pub fn all_equal(&self) -> bool {
        !self.rows.is_empty()
            && self
                .rows
                .iter()
                .all(|row| row.outcome == ComparisonOutcome::Equal)
    }
}

/// Completed request groups, not HTTP requests or physical devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComparisonProgress {
    /// Completed output rows.
    pub completed: usize,
    /// Total rows known after bounded pairing.
    pub total: usize,
    /// Latest controlled outcome.
    pub outcome: ComparisonOutcome,
}

struct Group<'a> {
    representative: &'a Fixture,
    left: Vec<usize>,
    right: Vec<usize>,
    supported: bool,
}

pub(super) struct PathBudget {
    pub count: usize,
    pub bytes: usize,
}

/// Compare frozen corpora using full Action plus proven bounded request identity.
/// A canonical key only prioritizes candidate checks. Invalid raw replay escape
/// hatches never prove identity; duplicate candidates remain Ambiguous.
pub async fn compare_corpora(
    left: &FixtureCorpus,
    right: &FixtureCorpus,
    options: &ComparisonOptions,
) -> Result<ComparisonReport, ComparisonError> {
    compare_corpora_with_progress(left, right, options, |_| {}).await
}

/// The same bounded comparison with one progress event per retained request group.
/// Limit failure returns Err rather than a truncated success report.
/// Progress is provisional; acceptance requires the final successful report.
pub async fn compare_corpora_with_progress(
    left: &FixtureCorpus,
    right: &FixtureCorpus,
    options: &ComparisonOptions,
    progress: impl Fn(ComparisonProgress) + Send + Sync,
) -> Result<ComparisonReport, ComparisonError> {
    let limits = options.limits;
    left.validate(limits.input)?;
    right.validate(limits.input)?;
    if limits.pairs == 0
        || limits.identity_checks == 0
        || limits.field_paths == 0
        || limits.report_bytes == 0
    {
        return Err(ComparisonError::InvalidLimit);
    }
    super::projection::validate_masks(&options.masks)?;
    let mut groups: Vec<Group<'_>> = Vec::new();
    let mut checks = 0;
    for (is_left, corpus) in [(true, left), (false, right)] {
        for (ordinal, fixture) in corpus.fixtures.iter().enumerate() {
            if fixture.action.len() > limits.report_bytes {
                return Err(ComparisonError::Limit {
                    resource: "report bytes",
                    limit: limits.report_bytes,
                });
            }
            if fixture.request_raw.len() > 2 * 1024 * 1024 {
                return Err(ComparisonError::Limit {
                    resource: "request XML bytes",
                    limit: 2 * 1024 * 1024,
                });
            }
            let supported =
                crate::mock::recording_identity_supported(&fixture.request_raw, &fixture.action);
            let mut found = None;
            if supported {
                let mut candidates = groups
                    .iter()
                    .enumerate()
                    .filter(|(_, group)| {
                        group.supported && group.representative.action == fixture.action
                    })
                    .map(|(index, group)| {
                        (group.representative.key_canon != fixture.key_canon, index)
                    })
                    .collect::<Vec<_>>();
                candidates.sort_unstable();
                for (_, index) in candidates {
                    checks += 1;
                    if checks > limits.identity_checks {
                        return Err(ComparisonError::Limit {
                            resource: "request identity checks",
                            limit: limits.identity_checks,
                        });
                    }
                    tokio::task::yield_now().await;
                    if crate::mock::recording_comparable(
                        &groups[index].representative.request_raw,
                        &fixture.request_raw,
                    ) {
                        found = Some(index);
                        break;
                    }
                }
            }
            let index = if let Some(index) = found {
                index
            } else {
                if groups.len() == limits.pairs {
                    return Err(ComparisonError::Limit {
                        resource: "request groups",
                        limit: limits.pairs,
                    });
                }
                groups.push(Group {
                    representative: fixture,
                    left: Vec::new(),
                    right: Vec::new(),
                    supported,
                });
                groups.len() - 1
            };
            if is_left {
                groups[index].left.push(ordinal);
            } else {
                groups[index].right.push(ordinal);
            }
            tokio::task::yield_now().await;
        }
    }
    groups.sort_by(|a, b| {
        (&a.representative.action, &a.left, &a.right).cmp(&(
            &b.representative.action,
            &b.left,
            &b.right,
        ))
    });
    let total = groups.len();
    let mut report = ComparisonReport { rows: Vec::new() };
    let mut paths = PathBudget { count: 0, bytes: 0 };
    let mut retained = b"{\"rows\":[]}".len();
    if retained > limits.report_bytes {
        return Err(ComparisonError::Limit {
            resource: "report bytes",
            limit: limits.report_bytes,
        });
    }
    for group in groups {
        use ComparisonOutcome as O;
        use ComparisonReason as R;
        let (outcome, reason) = if !group.supported {
            (O::Unsupported, R::RequestIdentity)
        } else if group.left.len() > 1 || group.right.len() > 1 {
            (O::Ambiguous, R::DuplicateRequests)
        } else if group.left.is_empty() {
            (O::MissingLeft, R::MissingRequest)
        } else if group.right.is_empty() {
            (O::MissingRight, R::MissingRequest)
        } else {
            (O::Unsupported, R::UnmappedAction)
        };
        let projection = super::projection::projection(&group.representative.action);
        let mut row = ComparisonRow {
            action: group.representative.action.clone(),
            left: group.left,
            right: group.right,
            outcome,
            reason,
            projection,
            masks: Vec::new(),
            changed_fields: Vec::new(),
            unexamined_left: Vec::new(),
            unexamined_right: Vec::new(),
        };
        if group.supported && row.left.len() == 1 && row.right.len() == 1 {
            super::projection::compare_pair(
                &left.fixtures[row.left[0]],
                &right.fixtures[row.right[0]],
                options,
                &mut row,
                &mut paths,
            )
            .await?;
        }
        let bytes = serde_json::to_vec(&row).map_err(|_| ComparisonError::InvalidLimit)?;
        retained = retained
            .saturating_add(bytes.len())
            .saturating_add(usize::from(!report.rows.is_empty()));
        if retained > limits.report_bytes {
            return Err(ComparisonError::Limit {
                resource: "report bytes",
                limit: limits.report_bytes,
            });
        }
        let outcome = row.outcome;
        report.rows.push(row);
        progress(ComparisonProgress {
            completed: report.rows.len(),
            total,
            outcome,
        });
    }
    Ok(report)
}
