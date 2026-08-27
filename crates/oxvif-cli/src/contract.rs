use serde::Serialize;

use crate::{
    AppError, CredentialProfileView, DeviceUpdate, DeviceView, DiscoveryFilter,
    DiscoverySnapshotSummary, DiscoverySnapshotView, GroupView, NewDevice, NewGroup, NewSavedView,
    SavedView,
};

/// Version of the structured stdout contract.
pub const SCHEMA_VERSION: &str = "2";

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
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandRequest {
    AgentGuide,
    AgentPrompt,
    /// List commands or describe one command.
    Describe(DescribeRequest),
    DeviceAdd(DeviceAddRequest),
    DeviceList,
    DeviceShow(DeviceIdRequest),
    DeviceUpdate(DeviceUpdateRequest),
    DeviceRename(DeviceRenameRequest),
    DeviceRemove(DeviceIdRequest),
    DeviceCredentialSet(DeviceCredentialSetRequest),
    DeviceCredentialDelete(DeviceIdRequest),
    DeviceCredentialUseProfile(DeviceCredentialProfileRequest),
    CredentialProfileSet(CredentialProfileSetRequest),
    CredentialProfileList,
    CredentialProfileShow(ResourceIdRequest),
    CredentialProfileDelete(ResourceIdRequest),
    GroupCreate(GroupCreateRequest),
    GroupList,
    GroupShow(ResourceIdRequest),
    GroupDelete(ResourceIdRequest),
    GroupMemberAdd(GroupMemberAddRequest),
    GroupMemberRemove(GroupMemberRemoveRequest),
    ViewCreate(ViewCreateRequest),
    ViewList,
    ViewShow(ResourceIdRequest),
    ViewEvaluate(ResourceIdRequest),
    ViewDelete(ResourceIdRequest),
    DiscoverScan(DiscoverScanRequest),
    DiscoverySnapshotList,
    DiscoverySnapshotShow(DiscoverySnapshotShowRequest),
    DiscoverySnapshotRemove(ResourceIdRequest),
    Use(DeviceIdRequest),
    Current,
    DeviceTest(DeviceConnectRequest),
    DeviceInfo(DeviceConnectRequest),
    DeviceRefresh(DeviceIdRequest),
}

impl CommandRequest {
    /// Stable dotted name used in metadata and diagnostics.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AgentGuide => "agent.guide",
            Self::AgentPrompt => "agent.prompt",
            Self::Describe(_) => "describe",
            Self::DeviceAdd(_) => "device.add",
            Self::DeviceList => "device.list",
            Self::DeviceShow(_) => "device.show",
            Self::DeviceUpdate(_) => "device.update",
            Self::DeviceRename(_) => "device.rename",
            Self::DeviceRemove(_) => "device.remove",
            Self::DeviceCredentialSet(_) => "device.credential.set",
            Self::DeviceCredentialDelete(_) => "device.credential.delete",
            Self::DeviceCredentialUseProfile(_) => "device.credential.use-profile",
            Self::CredentialProfileSet(_) => "credential.profile.set",
            Self::CredentialProfileList => "credential.profile.list",
            Self::CredentialProfileShow(_) => "credential.profile.show",
            Self::CredentialProfileDelete(_) => "credential.profile.delete",
            Self::GroupCreate(_) => "group.create",
            Self::GroupList => "group.list",
            Self::GroupShow(_) => "group.show",
            Self::GroupDelete(_) => "group.delete",
            Self::GroupMemberAdd(_) => "group.member.add",
            Self::GroupMemberRemove(_) => "group.member.remove",
            Self::ViewCreate(_) => "view.create",
            Self::ViewList => "view.list",
            Self::ViewShow(_) => "view.show",
            Self::ViewEvaluate(_) => "view.evaluate",
            Self::ViewDelete(_) => "view.delete",
            Self::DiscoverScan(_) => "discover.scan",
            Self::DiscoverySnapshotList => "discover.snapshots",
            Self::DiscoverySnapshotShow(_) => "discover.list",
            Self::DiscoverySnapshotRemove(_) => "discover.remove",
            Self::Use(_) => "use",
            Self::Current => "current",
            Self::DeviceTest(_) => "device.test",
            Self::DeviceInfo(_) => "device.info",
            Self::DeviceRefresh(_) => "device.refresh",
        }
    }
}

/// Request payload for [`CommandRequest::Describe`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DescribeRequest {
    /// Dotted command name, or `None` to list the available commands.
    pub command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAddRequest {
    pub device: NewDevice,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceUpdateRequest {
    pub id: String,
    pub update: DeviceUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRenameRequest {
    pub id: String,
    pub name: String,
}

#[derive(Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(secret: impl Into<String>) -> Result<Self, AppError> {
        let secret = secret.into();
        if secret.is_empty() {
            Err(AppError::invalid_argument("Password must not be empty."))
        } else {
            Ok(Self(secret))
        }
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeviceCredentialSetRequest {
    pub id: String,
    pub username: String,
    pub password: SecretString,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCredentialProfileRequest {
    pub device_id: String,
    pub profile_id: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CredentialProfileSetRequest {
    pub id: String,
    pub username: String,
    pub password: SecretString,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupCreateRequest {
    pub group: NewGroup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMemberAddRequest {
    pub group_id: String,
    pub device_id: String,
    pub alias: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMemberRemoveRequest {
    pub group_id: String,
    pub alias: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCreateRequest {
    pub view: NewSavedView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverScanRequest {
    pub snapshot_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySnapshotShowRequest {
    pub id: String,
    pub filters: Vec<DiscoveryFilter>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TargetSelector {
    pub device: Option<String>,
    pub target: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceConnectRequest {
    pub selector: TargetSelector,
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
    AgentGuide {
        guide: AgentGuide,
    },
    AgentPrompt {
        prompt: String,
    },
    CommandList {
        commands: Vec<CommandDescriptor>,
    },
    CommandDescription {
        command: CommandDescriptor,
    },
    DeviceList {
        devices: Vec<DeviceView>,
        current_device: Option<String>,
    },
    DeviceRecord {
        action: String,
        device: DeviceView,
    },
    DeviceRemoved {
        id: String,
    },
    CurrentDevice {
        device: Option<DeviceView>,
    },
    CredentialUpdated {
        action: String,
        device: DeviceView,
    },
    CredentialProfileList {
        profiles: Vec<CredentialProfileView>,
    },
    CredentialProfileRecord {
        action: String,
        profile: CredentialProfileView,
    },
    GroupList {
        groups: Vec<GroupView>,
    },
    GroupRecord {
        action: String,
        group: GroupView,
    },
    ViewList {
        views: Vec<SavedView>,
    },
    ViewRecord {
        action: String,
        view: SavedView,
    },
    ViewEvaluation {
        view: SavedView,
        devices: Vec<DeviceView>,
    },
    DiscoverySnapshotList {
        snapshots: Vec<DiscoverySnapshotSummary>,
    },
    DiscoverySnapshotRecord {
        action: String,
        snapshot: DiscoverySnapshotView,
    },
    ResourceRemoved {
        resource: String,
        id: String,
    },
    DeviceTest {
        device_id: Option<String>,
        target: String,
        authenticated: bool,
        information: LiveDeviceInfo,
    },
    DeviceInformation {
        device_id: Option<String>,
        target: String,
        information: LiveDeviceInfo,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LiveDeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

/// Versioned operational rules embedded in the installed binary for Agents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentGuide {
    pub guide_version: &'static str,
    pub cli_version: &'static str,
    pub schema_version: &'static str,
    pub rules: Vec<&'static str>,
    pub recommended_workflow: Vec<&'static str>,
    pub security_requirements: Vec<&'static str>,
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
    pub selected_by: Option<String>,
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
