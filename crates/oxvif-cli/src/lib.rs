//! Application and output contracts for the `oxvif` executable.
//!
//! The binary is deliberately thin. Future adapters, including a possible MCP
//! server, can construct the same typed requests and consume the same results
//! without parsing command-line arguments or terminal output.
//!
//! Maintenance requests add bounded snapshot downloads, layered diagnostic
//! reports and read-only configuration inventory/diff. These workflows do not
//! mutate cameras. File-producing requests refuse existing destinations; failed
//! diagnostic reports may retain data while `CommandSuccess::exit_code()` is 20.
//! Snapshot signatures and URI lookups do not establish video playback, and
//! configuration inventories are not restorable backups.
//! Human adapters may supply a profile picker to diagnosis without repeating the
//! session handshake. Structured reports retain legacy error categories alongside
//! additive selection reasons, candidates, status counts and untested reasons.

mod agent;
mod application;
mod contract;
mod credential;
mod describe;
mod error;
mod inventory;
mod maintenance;
mod output;
mod registry;

pub use maintenance::{
    ConfigDiffRequest, ConfigExportRequest, DiagnoseRequest, ManagedAction, ManagedDevice,
    ProfileChoice, ProfilePicker, SnapshotSaveRequest,
};

pub use application::{Application, ClockSyncPolicy, ExecutionOptions};
pub use contract::{
    AgentGuide, ArgumentDescriptor, CommandData, CommandDescriptor, CommandId, CommandRequest,
    CommandSpec, CommandSuccess, CredentialProfileSetRequest, DescribeRequest, DeviceAddRequest,
    DeviceConnectRequest, DeviceCredentialProfileRequest, DeviceCredentialSetRequest,
    DeviceIdRequest, DeviceImportRequest, DeviceRenameRequest, DeviceSetupRequest,
    DeviceUpdateRequest, DiscoverScanRequest, DiscoveryEnrichRequest, DiscoveryRefreshRequest,
    DiscoverySnapshotShowRequest, ErrorEnvelope, FleetDiagnosticItem, FleetItemError,
    GroupCreateRequest, GroupMemberAddRequest, GroupMemberRemoveRequest, ImportMode,
    LiveDeviceInfo, OutputDescriptor, OutputFormat, ProfileConnectRequest, ResourceIdRequest,
    ResultMeta, RiskLevel, SCHEMA_VERSION, SecretString, SuccessEnvelope, TargetSelector,
    ViewCreateRequest, ViewEvaluateRequest, Warning,
};
pub use credential::{
    CredentialStore, MemoryCredentialStore, SystemCredentialStore, credential_profile_reference,
    credential_reference,
};
pub use error::{AppError, ErrorCode};
pub use inventory::{
    CredentialProfileView, DeviceFilter, DeviceFilterField, DiscoveryDeviceView, DiscoveryFilter,
    DiscoveryFilterField, DiscoveryImportOverride, DiscoveryImportOverrides, DiscoveryImportPlan,
    DiscoveryImportProposal, DiscoveryRecord, DiscoveryRegistrationStatus, DiscoveryResultSummary,
    DiscoverySnapshotResult, DiscoverySnapshotSummary, DiscoverySnapshotView, FilterExplanation,
    FilterOperator, GroupMemberView, GroupView, ImportDisposition, MatchMode, NewGroup,
    NewSavedView, SavedView, ViewExplanation, discovery_query_matches,
};
pub use output::{profile_label, render_error, render_success, render_success_with_details};
pub use registry::{
    DeviceMetadata, DeviceUpdate, DeviceView, NewDevice, REGISTRY_VERSION, RegistryStore,
    normalize_target, validate_device_id,
};

/// Return the complete, drift-checked public command catalogue.
pub fn command_descriptors() -> Vec<CommandDescriptor> {
    describe::descriptors()
}

/// Return the exhaustive command catalogue with stable identities.
pub fn command_specs() -> Vec<CommandSpec> {
    describe::specs()
}
