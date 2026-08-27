use serde::Serialize;

use crate::AppError;

/// Version of the structured stdout contract.
pub const SCHEMA_VERSION: &str = "1";

/// Presentation format requested by the caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OutputFormat {
    /// Compact, readable terminal output.
    #[default]
    Table,
    /// One pretty-printed JSON document.
    Json,
    /// One compact JSON document per line.
    JsonLines,
}

impl OutputFormat {
    /// Whether the output is intended for machine consumption.
    pub const fn is_structured(self) -> bool {
        matches!(self, Self::Json | Self::JsonLines)
    }
}

/// A request understood by the application layer.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandRequest {
    /// List commands or describe one command.
    Describe(DescribeRequest),
}

impl CommandRequest {
    /// Stable dotted name used in metadata and diagnostics.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Describe(_) => "describe",
        }
    }
}

/// Request payload for [`CommandRequest::Describe`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DescribeRequest {
    /// Dotted command name, or `None` to list the available commands.
    pub command: Option<String>,
}

/// Risk attached to a command in the self-description surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Read,
    Write,
    Dangerous,
}

impl RiskLevel {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Dangerous => "dangerous",
        }
    }
}

/// One command argument exposed through `oxvif describe`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArgumentDescriptor {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub required: bool,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<String>,
}

/// Description of the data returned by a command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputDescriptor {
    #[serde(rename = "type")]
    pub value_type: String,
    pub description: String,
}

/// Self-describing command contract for Agent discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub name: String,
    pub summary: String,
    pub risk: RiskLevel,
    pub authentication_required: bool,
    pub mutates_device: bool,
    pub retryable: bool,
    pub arguments: Vec<ArgumentDescriptor>,
    pub output: OutputDescriptor,
}

/// Typed result variants returned by application commands.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommandData {
    CommandList { commands: Vec<CommandDescriptor> },
    CommandDescription { command: CommandDescriptor },
}

/// A non-fatal condition associated with an otherwise successful result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
}

/// Metadata common to success and failure envelopes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ResultMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub elapsed_ms: u64,
}

/// Successful result before a renderer selects human or machine output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSuccess {
    pub data: CommandData,
    pub warnings: Vec<Warning>,
    pub meta: ResultMeta,
}

/// Stable success envelope for JSON and JSONL output.
#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<'a> {
    pub schema_version: &'static str,
    pub ok: bool,
    pub data: &'a CommandData,
    pub warnings: &'a [Warning],
    pub meta: &'a ResultMeta,
}

impl<'a> From<&'a CommandSuccess> for SuccessEnvelope<'a> {
    fn from(value: &'a CommandSuccess) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: true,
            data: &value.data,
            warnings: &value.warnings,
            meta: &value.meta,
        }
    }
}

/// Stable failure envelope for JSON and JSONL output.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope<'a> {
    pub schema_version: &'static str,
    pub ok: bool,
    pub error: &'a AppError,
    pub meta: &'a ResultMeta,
}

impl<'a> ErrorEnvelope<'a> {
    pub fn new(error: &'a AppError, meta: &'a ResultMeta) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: false,
            error,
            meta,
        }
    }
}
