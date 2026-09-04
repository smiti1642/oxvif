use serde::Serialize;
use zeroize::Zeroize;

use crate::{
    AppError, CredentialProfileView, DeviceUpdate, DeviceView, DiscoveryDeviceView,
    DiscoveryFilter, DiscoveryResultSummary, DiscoverySnapshotResult, DiscoverySnapshotSummary,
    GroupView, NewDevice, NewGroup, NewSavedView, SavedView,
};

/// Version of the structured stdout contract.
pub const SCHEMA_VERSION: &str = "3";

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

macro_rules! command_ids {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// Exhaustive identity for every public CLI command path, including
        /// human-friendly aliases that map to canonical application requests.
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub enum CommandId {
            $($variant),+
        }

        impl CommandId {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Canonical typed application operation for a public command.
            pub const fn canonical(self) -> Self {
                match self {
                    Self::Auth => Self::DeviceCredentialSet,
                    Self::Info => Self::DeviceInfo,
                    Self::Test => Self::DeviceTest,
                    Self::Health => Self::HealthCheck,
                    Self::Profiles => Self::MediaProfiles,
                    Self::Stream => Self::MediaStreamUri,
                    Self::Snapshot => Self::MediaSnapshotUri,
                    Self::Devices => Self::DeviceList,
                    Self::Groups => Self::GroupList,
                    Self::Views => Self::ViewList,
                    other => other,
                }
            }
        }
    };
}

command_ids! {
    AgentGuide => "agent.guide",
    AgentPrompt => "agent.prompt",
    Describe => "describe",
    ConfigPath => "config.path",
    ConfigValidate => "config.validate",
    Setup => "setup",
    Auth => "auth",
    Info => "info",
    Test => "test",
    Health => "health",
    Profiles => "profiles",
    Stream => "stream",
    Snapshot => "snapshot",
    Devices => "devices",
    Groups => "groups",
    Views => "views",
    Completion => "completion",
    DeviceAdd => "device.add",
    DeviceList => "device.list",
    DeviceShow => "device.show",
    DeviceUpdate => "device.update",
    DeviceRename => "device.rename",
    DeviceRemove => "device.remove",
    DeviceImport => "device.import",
    DeviceCredentialSet => "device.credential.set",
    DeviceCredentialDelete => "device.credential.delete",
    DeviceCredentialUseProfile => "device.credential.use-profile",
    CredentialProfileSet => "credential.profile.set",
    CredentialProfileList => "credential.profile.list",
    CredentialProfileShow => "credential.profile.show",
    CredentialProfileDelete => "credential.profile.delete",
    GroupCreate => "group.create",
    GroupList => "group.list",
    GroupShow => "group.show",
    GroupDelete => "group.delete",
    GroupMemberAdd => "group.member.add",
    GroupMemberRemove => "group.member.remove",
    ViewCreate => "view.create",
    ViewList => "view.list",
    ViewShow => "view.show",
    ViewEvaluate => "view.evaluate",
    ViewDelete => "view.delete",
    DiscoverScan => "discover.scan",
    DiscoverRefresh => "discover.refresh",
    DiscoverEnrich => "discover.enrich",
    DiscoverSnapshots => "discover.snapshots",
    DiscoverList => "discover.list",
    DiscoverRemove => "discover.remove",
    Use => "use",
    Current => "current",
    DeviceTest => "device.test",
    DeviceInfo => "device.info",
    DeviceCapabilities => "device.capabilities",
    DeviceServices => "device.services",
    MediaProfiles => "media.profiles",
    MediaStreamUri => "media.stream-uri",
    MediaSnapshotUri => "media.snapshot-uri",
    PtzStatus => "ptz.status",
    PtzPresets => "ptz.presets",
    HealthCheck => "health.check",
    DeviceRefresh => "device.refresh",
}

/// A request understood by the application layer.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandRequest {
    AgentGuide,
    AgentPrompt,
    /// List commands or describe one command.
    Describe(DescribeRequest),
    DeviceSetup(DeviceSetupRequest),
    DeviceAdd(DeviceAddRequest),
    DeviceList,
    DeviceShow(DeviceIdRequest),
    DeviceUpdate(DeviceUpdateRequest),
    DeviceRename(DeviceRenameRequest),
    DeviceRemove(DeviceIdRequest),
    DeviceImport(DeviceImportRequest),
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
    ViewEvaluate(ViewEvaluateRequest),
    ViewDelete(ResourceIdRequest),
    DiscoverScan(DiscoverScanRequest),
    DiscoveryRefresh(DiscoveryRefreshRequest),
    DiscoveryEnrich(DiscoveryEnrichRequest),
    DiscoverySnapshotList,
    DiscoverySnapshotShow(DiscoverySnapshotShowRequest),
    DiscoverySnapshotRemove(ResourceIdRequest),
    ConfigPath,
    ConfigValidate,
    Use(DeviceIdRequest),
    Current,
    DeviceTest(DeviceConnectRequest),
    DeviceInfo(DeviceConnectRequest),
    DeviceCapabilities(DeviceConnectRequest),
    DeviceServices(DeviceConnectRequest),
    MediaProfiles(DeviceConnectRequest),
    MediaStreamUri(ProfileConnectRequest),
    MediaSnapshotUri(ProfileConnectRequest),
    PtzStatus(ProfileConnectRequest),
    PtzPresets(ProfileConnectRequest),
    HealthCheck(DeviceConnectRequest),
    DeviceRefresh(DeviceIdRequest),
}

impl CommandRequest {
    /// Canonical application identity for this typed request.
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::AgentGuide => CommandId::AgentGuide,
            Self::AgentPrompt => CommandId::AgentPrompt,
            Self::Describe(_) => CommandId::Describe,
            Self::DeviceSetup(_) => CommandId::Setup,
            Self::DeviceAdd(_) => CommandId::DeviceAdd,
            Self::DeviceList => CommandId::DeviceList,
            Self::DeviceShow(_) => CommandId::DeviceShow,
            Self::DeviceUpdate(_) => CommandId::DeviceUpdate,
            Self::DeviceRename(_) => CommandId::DeviceRename,
            Self::DeviceRemove(_) => CommandId::DeviceRemove,
            Self::DeviceImport(_) => CommandId::DeviceImport,
            Self::DeviceCredentialSet(_) => CommandId::DeviceCredentialSet,
            Self::DeviceCredentialDelete(_) => CommandId::DeviceCredentialDelete,
            Self::DeviceCredentialUseProfile(_) => CommandId::DeviceCredentialUseProfile,
            Self::CredentialProfileSet(_) => CommandId::CredentialProfileSet,
            Self::CredentialProfileList => CommandId::CredentialProfileList,
            Self::CredentialProfileShow(_) => CommandId::CredentialProfileShow,
            Self::CredentialProfileDelete(_) => CommandId::CredentialProfileDelete,
            Self::GroupCreate(_) => CommandId::GroupCreate,
            Self::GroupList => CommandId::GroupList,
            Self::GroupShow(_) => CommandId::GroupShow,
            Self::GroupDelete(_) => CommandId::GroupDelete,
            Self::GroupMemberAdd(_) => CommandId::GroupMemberAdd,
            Self::GroupMemberRemove(_) => CommandId::GroupMemberRemove,
            Self::ViewCreate(_) => CommandId::ViewCreate,
            Self::ViewList => CommandId::ViewList,
            Self::ViewShow(_) => CommandId::ViewShow,
            Self::ViewEvaluate(_) => CommandId::ViewEvaluate,
            Self::ViewDelete(_) => CommandId::ViewDelete,
            Self::DiscoverScan(_) => CommandId::DiscoverScan,
            Self::DiscoveryRefresh(_) => CommandId::DiscoverRefresh,
            Self::DiscoveryEnrich(_) => CommandId::DiscoverEnrich,
            Self::DiscoverySnapshotList => CommandId::DiscoverSnapshots,
            Self::DiscoverySnapshotShow(_) => CommandId::DiscoverList,
            Self::DiscoverySnapshotRemove(_) => CommandId::DiscoverRemove,
            Self::ConfigPath => CommandId::ConfigPath,
            Self::ConfigValidate => CommandId::ConfigValidate,
            Self::Use(_) => CommandId::Use,
            Self::Current => CommandId::Current,
            Self::DeviceTest(_) => CommandId::DeviceTest,
            Self::DeviceInfo(_) => CommandId::DeviceInfo,
            Self::DeviceCapabilities(_) => CommandId::DeviceCapabilities,
            Self::DeviceServices(_) => CommandId::DeviceServices,
            Self::MediaProfiles(_) => CommandId::MediaProfiles,
            Self::MediaStreamUri(_) => CommandId::MediaStreamUri,
            Self::MediaSnapshotUri(_) => CommandId::MediaSnapshotUri,
            Self::PtzStatus(_) => CommandId::PtzStatus,
            Self::PtzPresets(_) => CommandId::PtzPresets,
            Self::HealthCheck(_) => CommandId::HealthCheck,
            Self::DeviceRefresh(_) => CommandId::DeviceRefresh,
        }
    }

    /// Stable dotted name used in metadata and diagnostics.
    pub const fn name(&self) -> &'static str {
        self.command_id().as_str()
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

#[derive(Debug, Eq, PartialEq)]
pub struct DeviceSetupRequest {
    pub device: NewDevice,
    pub username: String,
    pub password: SecretString,
    pub verify: bool,
    pub set_current: bool,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportMode {
    Plan,
    Apply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceImportRequest {
    pub snapshot_id: String,
    pub filters: Vec<DiscoveryFilter>,
    pub group_id: Option<String>,
    pub credential_profile: Option<String>,
    pub tags: Vec<String>,
    pub overrides: Vec<crate::DiscoveryImportOverride>,
    pub mode: ImportMode,
    pub expected_fingerprint: Option<String>,
}

#[derive(Clone, Eq, PartialEq)]
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

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
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
pub struct ViewEvaluateRequest {
    pub id: String,
    pub explain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverScanRequest {
    pub snapshot_id: Option<String>,
    pub interfaces: Vec<String>,
    pub filters: Vec<DiscoveryFilter>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryRefreshRequest {
    pub id: String,
    pub interfaces: Vec<String>,
    pub filters: Vec<DiscoveryFilter>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryEnrichRequest {
    pub id: String,
    pub credential_profile: String,
    pub filters: Vec<DiscoveryFilter>,
    pub jobs: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySnapshotShowRequest {
    pub id: String,
    pub filters: Vec<DiscoveryFilter>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TargetSelector {
    pub device: Option<String>,
    pub target: Option<String>,
    pub group: Option<String>,
    pub view: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceConnectRequest {
    pub selector: TargetSelector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileConnectRequest {
    pub selector: TargetSelector,
    pub profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FleetItemError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FleetDiagnosticItem {
    pub device_id: String,
    pub selected_by: String,
    pub target: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<FleetItemError>,
    pub elapsed_ms: u64,
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
    pub possible_errors: Vec<String>,
    pub examples: Vec<String>,
}

/// One exhaustive command identity paired with its complete public contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub id: CommandId,
    pub descriptor: CommandDescriptor,
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
    DeviceSetup {
        device: DeviceView,
        verified: bool,
        current: bool,
    },
    DeviceRemoved {
        id: String,
    },
    DeviceImport {
        applied: bool,
        plan: crate::DiscoveryImportPlan,
        devices: Vec<DeviceView>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        explanation: Option<crate::ViewExplanation>,
    },
    DiscoverySnapshotList {
        snapshots: Vec<DiscoverySnapshotSummary>,
    },
    DiscoverySnapshotRecord {
        action: String,
        snapshot: DiscoverySnapshotResult,
    },
    DiscoveryScan {
        devices: Vec<DiscoveryDeviceView>,
        summary: DiscoveryResultSummary,
        saved_snapshot: Option<DiscoverySnapshotSummary>,
        interfaces: Vec<String>,
    },
    DiscoveryEnrichment {
        snapshot: DiscoverySnapshotSummary,
        attempted: usize,
        enriched: usize,
        failed: usize,
    },
    ConfigStatus {
        config_dir: String,
        registry_file: String,
        snapshots_dir: String,
        validated: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        device_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_count: Option<usize>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        orphaned_snapshot_files: Vec<String>,
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
    DeviceDiagnostic {
        operation: String,
        device_id: Option<String>,
        target: String,
        result: serde_json::Value,
    },
    FleetDiagnostic {
        operation: String,
        selection_kind: String,
        selection_id: String,
        total: usize,
        succeeded: usize,
        failed: usize,
        items: Vec<FleetDiagnosticItem>,
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

impl CommandSuccess {
    pub fn exit_code(&self) -> u8 {
        match &self.data {
            CommandData::FleetDiagnostic {
                succeeded, failed, ..
            } if *succeeded > 0 && *failed > 0 => 6,
            _ => 0,
        }
    }
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
            ok: value.exit_code() == 0,
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
