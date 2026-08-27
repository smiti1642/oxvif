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

pub use application::{Application, ExecutionOptions};
pub use contract::{
    AgentGuide, ArgumentDescriptor, CommandData, CommandDescriptor, CommandRequest, CommandSuccess,
    CredentialProfileSetRequest, DescribeRequest, DeviceAddRequest, DeviceConnectRequest,
    DeviceCredentialProfileRequest, DeviceCredentialSetRequest, DeviceIdRequest,
    DeviceRenameRequest, DeviceUpdateRequest, DiscoverScanRequest, DiscoverySnapshotShowRequest,
    ErrorEnvelope, GroupCreateRequest, GroupMemberAddRequest, GroupMemberRemoveRequest,
    LiveDeviceInfo, OutputDescriptor, OutputFormat, ResourceIdRequest, ResultMeta, RiskLevel,
    SCHEMA_VERSION, SecretString, SuccessEnvelope, TargetSelector, ViewCreateRequest, Warning,
};
pub use credential::{
    CredentialStore, MemoryCredentialStore, SystemCredentialStore, credential_profile_reference,
    credential_reference,
};
pub use error::{AppError, ErrorCode};
pub use inventory::{
    CredentialProfileView, DeviceFilter, DeviceFilterField, DiscoveryFilter, DiscoveryFilterField,
    DiscoveryRecord, DiscoverySnapshotSummary, DiscoverySnapshotView, GroupMemberView, GroupView,
    NewGroup, NewSavedView, SavedView,
};
pub use output::{render_error, render_success};
pub use registry::{
    DeviceMetadata, DeviceUpdate, DeviceView, NewDevice, REGISTRY_VERSION, RegistryStore,
    normalize_target, validate_device_id,
};
