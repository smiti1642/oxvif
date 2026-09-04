//! Application and output contracts for the `oxvif` executable.
//!
//! The binary is deliberately thin. Future adapters, including a possible MCP
//! server, can construct the same typed requests and consume the same results
//! without parsing command-line arguments or terminal output.

mod agent;
mod application;
mod contract;
mod credential;
mod describe;
mod error;
mod inventory;
mod output;
mod registry;

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
pub use output::{render_error, render_success};
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
