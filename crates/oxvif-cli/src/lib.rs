//! Application and output contracts for the `oxvif` executable.
//!
//! The binary is deliberately thin. Future adapters, including a possible MCP
//! server, can construct the same typed requests and consume the same results
//! without parsing command-line arguments or terminal output.

mod application;
mod contract;
mod credential;
mod describe;
mod error;
mod output;
mod registry;

pub use application::{Application, ExecutionOptions};
pub use contract::{
    ArgumentDescriptor, CommandData, CommandDescriptor, CommandRequest, CommandSuccess,
    DescribeRequest, DeviceAddRequest, DeviceConnectRequest, DeviceCredentialSetRequest,
    DeviceIdRequest, DeviceRenameRequest, DeviceUpdateRequest, ErrorEnvelope, LiveDeviceInfo,
    OutputDescriptor, OutputFormat, ResultMeta, RiskLevel, SCHEMA_VERSION, SecretString,
    SuccessEnvelope, TargetSelector, Warning,
};
pub use credential::{
    CredentialStore, MemoryCredentialStore, SystemCredentialStore, credential_reference,
};
pub use error::{AppError, ErrorCode};
pub use output::{render_error, render_success};
pub use registry::{
    DeviceMetadata, DeviceUpdate, DeviceView, NewDevice, REGISTRY_VERSION, RegistryStore,
    normalize_target, validate_device_id,
};
