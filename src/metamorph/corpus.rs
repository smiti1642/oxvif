//! Bounded, ordinal-preserving input for offline regression and comparison.
use std::{fs::File, io::Read, path::Path};

use serde::{Deserialize, Serialize};

use super::Fixture;

/// Limits applied before offline input is parsed or copied.
#[derive(Clone, Copy, Debug)]
pub struct CorpusLimits {
    /// Actual file bytes, including JSON metadata. Default: 32 MiB.
    pub file_bytes: usize,
    /// Retained exchange rows. Default: 512.
    pub fixtures: usize,
    /// Bytes of either raw request or raw response. Default: 4 MiB.
    pub xml_bytes: usize,
}

impl Default for CorpusLimits {
    fn default() -> Self {
        Self {
            file_bytes: 32 * 1024 * 1024,
            fixtures: 512,
            xml_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Controlled offline input failures; no raw XML or JSON value is retained.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CorpusError {
    /// Input file could not be read. The original IO cause remains available.
    #[error("cannot read corpus input")]
    Io(#[from] std::io::Error),
    /// Invalid JSON or incompatible input shape.
    #[error("invalid corpus JSON")]
    InvalidJson,
    /// A configured input limit is zero or cannot be represented.
    #[error("invalid corpus limit")]
    InvalidLimit,
    /// A byte or row limit refused input before further processing.
    #[error("corpus {resource} limit exceeded ({limit})")]
    Limit {
        /// Controlled resource name.
        resource: &'static str,
        /// Configured maximum.
        limit: usize,
    },
}

/// The existing `fixtures.json` shape, preserving every on-disk row and ordinal.
/// Unlike FixtureStore loading, this does not upsert equivalent requests. Already
/// overwritten recordings cannot be recovered. Raw captures remain sensitive.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureCorpus {
    /// Caller-supplied device label, not a verified physical identity.
    #[serde(default)]
    pub device: String,
    /// Frozen insertion order, including duplicate candidates.
    pub fixtures: Vec<Fixture>,
}

impl FixtureCorpus {
    /// Read `<directory>/fixtures.json` with actual byte and row limits.
    /// Explicitly selected missing files fail; there is no skip or network access.
    pub fn load(directory: impl AsRef<Path>, limits: CorpusLimits) -> Result<Self, CorpusError> {
        limits.check()?;
        let bytes = read_bounded(&directory.as_ref().join("fixtures.json"), limits.file_bytes)?;
        let corpus: Self = serde_json::from_slice(&bytes).map_err(|_| CorpusError::InvalidJson)?;
        corpus.validate(limits)?;
        Ok(corpus)
    }

    /// Apply the same row/string budget to caller-provided in-memory inputs.
    pub fn validate(&self, limits: CorpusLimits) -> Result<(), CorpusError> {
        limits.check()?;
        if self.fixtures.len() > limits.fixtures {
            return Err(CorpusError::Limit {
                resource: "fixtures",
                limit: limits.fixtures,
            });
        }
        let mut retained = self.device.len();
        for row in &self.fixtures {
            if row.request_raw.len() > limits.xml_bytes || row.response_raw.len() > limits.xml_bytes
            {
                return Err(CorpusError::Limit {
                    resource: "XML bytes",
                    limit: limits.xml_bytes,
                });
            }
            for text in [
                &row.action,
                &row.key_canon,
                &row.request_raw,
                &row.response_raw,
            ] {
                retained = retained.checked_add(text.len()).ok_or(CorpusError::Limit {
                    resource: "retained bytes",
                    limit: limits.file_bytes,
                })?;
            }
        }
        if retained > limits.file_bytes {
            return Err(CorpusError::Limit {
                resource: "retained bytes",
                limit: limits.file_bytes,
            });
        }
        Ok(())
    }
}

impl CorpusLimits {
    fn check(self) -> Result<(), CorpusError> {
        if self.file_bytes == 0
            || self.fixtures == 0
            || self.xml_bytes == 0
            || self.file_bytes.checked_add(1).is_none()
        {
            return Err(CorpusError::InvalidLimit);
        }
        Ok(())
    }
}

pub(super) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, CorpusError> {
    let count = limit
        .checked_add(1)
        .filter(|_| limit > 0)
        .ok_or(CorpusError::InvalidLimit)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(count as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(CorpusError::Limit {
            resource: "file bytes",
            limit,
        });
    }
    Ok(bytes)
}
