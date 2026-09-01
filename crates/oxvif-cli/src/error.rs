use serde::Serialize;

/// Stable machine-readable error categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    InvalidArgument,
    CommandNotFound,
    DeviceNotFound,
    DeviceAlreadyExists,
    ResourceNotFound,
    ResourceAlreadyExists,
    ResourceInUse,
    ImportPlanMismatch,
    MissingTarget,
    ConfigUnavailable,
    RegistryIo,
    RegistryCorrupt,
    RegistryVersionUnsupported,
    CredentialUnavailable,
    DeviceConnectionFailed,
    DiscoveryFailed,
    FleetFailed,
    SerializationFailed,
    Internal,
}

impl ErrorCode {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidArgument => 2,
            Self::CommandNotFound | Self::DeviceNotFound | Self::ResourceNotFound => 3,
            Self::DeviceAlreadyExists
            | Self::ResourceAlreadyExists
            | Self::ResourceInUse
            | Self::ImportPlanMismatch => 4,
            Self::MissingTarget => 5,
            Self::ConfigUnavailable
            | Self::RegistryIo
            | Self::RegistryCorrupt
            | Self::RegistryVersionUnsupported => 10,
            Self::CredentialUnavailable => 11,
            Self::DeviceConnectionFailed | Self::DiscoveryFailed | Self::FleetFailed => 20,
            Self::SerializationFailed | Self::Internal => 70,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::CommandNotFound => "COMMAND_NOT_FOUND",
            Self::DeviceNotFound => "DEVICE_NOT_FOUND",
            Self::DeviceAlreadyExists => "DEVICE_ALREADY_EXISTS",
            Self::ResourceNotFound => "RESOURCE_NOT_FOUND",
            Self::ResourceAlreadyExists => "RESOURCE_ALREADY_EXISTS",
            Self::ResourceInUse => "RESOURCE_IN_USE",
            Self::ImportPlanMismatch => "IMPORT_PLAN_MISMATCH",
            Self::MissingTarget => "MISSING_TARGET",
            Self::ConfigUnavailable => "CONFIG_UNAVAILABLE",
            Self::RegistryIo => "REGISTRY_IO",
            Self::RegistryCorrupt => "REGISTRY_CORRUPT",
            Self::RegistryVersionUnsupported => "REGISTRY_VERSION_UNSUPPORTED",
            Self::CredentialUnavailable => "CREDENTIAL_UNAVAILABLE",
            Self::DeviceConnectionFailed => "DEVICE_CONNECTION_FAILED",
            Self::DiscoveryFailed => "DISCOVERY_FAILED",
            Self::FleetFailed => "FLEET_FAILED",
            Self::SerializationFailed => "SERIALIZATION_FAILED",
            Self::Internal => "INTERNAL",
        }
    }
}

/// Error returned by the application or presentation layer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

impl AppError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidArgument,
            message: message.into(),
            retryable: false,
            suggested_action: Some("Run `oxvif --help` or `oxvif describe`.".to_owned()),
        }
    }

    pub fn command_not_found(command: &str) -> Self {
        Self {
            code: ErrorCode::CommandNotFound,
            message: format!("No implemented command named `{command}`."),
            retryable: false,
            suggested_action: Some(
                "Run `oxvif describe --output json` to list implemented commands.".to_owned(),
            ),
        }
    }

    pub fn device_not_found(id: &str) -> Self {
        Self {
            code: ErrorCode::DeviceNotFound,
            message: format!("No saved device has ID `{id}`."),
            retryable: false,
            suggested_action: Some("Run `oxvif devices` to list saved devices.".to_owned()),
        }
    }

    pub fn device_exists(id: &str) -> Self {
        Self {
            code: ErrorCode::DeviceAlreadyExists,
            message: format!("A saved device already has ID `{id}`."),
            retryable: false,
            suggested_action: Some(
                "Choose another immutable ID or run `oxvif device update`.".to_owned(),
            ),
        }
    }

    pub fn resource_not_found(kind: &str, id: &str) -> Self {
        Self {
            code: ErrorCode::ResourceNotFound,
            message: format!("No {kind} has ID `{id}`."),
            retryable: false,
            suggested_action: Some(format!("List saved {kind}s and choose an existing ID.")),
        }
    }

    pub fn resource_exists(kind: &str, id: &str) -> Self {
        Self {
            code: ErrorCode::ResourceAlreadyExists,
            message: format!("A {kind} already has ID `{id}`."),
            retryable: false,
            suggested_action: Some("Choose another immutable ID.".to_owned()),
        }
    }

    pub fn resource_in_use(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ResourceInUse,
            message: message.into(),
            retryable: false,
            suggested_action: Some(
                "Remove the references before deleting this resource.".to_owned(),
            ),
        }
    }

    pub fn import_plan_mismatch(expected: &str, actual: &str) -> Self {
        Self {
            code: ErrorCode::ImportPlanMismatch,
            message: format!(
                "Import plan fingerprint mismatch: expected `{expected}`, current plan is `{actual}`."
            ),
            retryable: false,
            suggested_action: Some(
                "Run `device import --plan` again, review it, then apply its fingerprint."
                    .to_owned(),
            ),
        }
    }

    pub fn missing_target() -> Self {
        Self {
            code: ErrorCode::MissingTarget,
            message: "No device target was selected.".to_owned(),
            retryable: false,
            suggested_action: Some(
                "Pass --device/--target/--group/--view, set OXVIF_DEVICE, or run `oxvif use <id>`."
                    .to_owned(),
            ),
        }
    }

    pub fn fleet_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::FleetFailed,
            message: message.into(),
            retryable: true,
            suggested_action: Some(
                "Inspect device credentials/connectivity or retry with a narrower Group/View."
                    .to_owned(),
            ),
        }
    }

    pub fn config_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ConfigUnavailable,
            message: message.into(),
            retryable: false,
            suggested_action: Some("Set OXVIF_CONFIG_DIR to a writable directory.".to_owned()),
        }
    }

    pub fn registry_io(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::RegistryIo,
            message: message.into(),
            retryable: true,
            suggested_action: Some(
                "Check directory permissions and whether another process holds the registry lock."
                    .to_owned(),
            ),
        }
    }

    pub fn registry_corrupt(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::RegistryCorrupt,
            message: message.into(),
            retryable: false,
            suggested_action: Some(
                "Repair or restore devices.toml; oxvif will not overwrite it.".to_owned(),
            ),
        }
    }

    pub fn registry_version(found: u32, supported: u32) -> Self {
        Self {
            code: ErrorCode::RegistryVersionUnsupported,
            message: format!(
                "Registry schema version {found} is unsupported; this build supports {supported}."
            ),
            retryable: false,
            suggested_action: Some("Upgrade oxvif-cli or migrate the registry.".to_owned()),
        }
    }

    pub fn credential_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::CredentialUnavailable,
            message: message.into(),
            retryable: false,
            suggested_action: Some(
                "Run `oxvif auth <DEVICE> --username <USERNAME>` to set it securely.".to_owned(),
            ),
        }
    }

    /// Report a native secret-store failure without forwarding backend text.
    ///
    /// Platform APIs may include account identifiers or other sensitive context
    /// in their errors, so callers intentionally provide only the attempted
    /// operation here.
    pub fn credential_backend_unavailable(operation: &str) -> Self {
        Self {
            code: ErrorCode::CredentialUnavailable,
            message: format!("The native credential backend could not {operation} the secret."),
            retryable: false,
            suggested_action: Some(native_credential_suggested_action().to_owned()),
        }
    }

    pub fn device_connection_failed(message: impl Into<String>) -> Self {
        Self::device_operation_failed(message, true)
    }

    pub fn device_operation_failed(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: ErrorCode::DeviceConnectionFailed,
            message: message.into(),
            retryable,
            suggested_action: Some(
                "Verify target reachability, credentials, and device clock synchronization."
                    .to_owned(),
            ),
        }
    }

    pub fn discovery_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::DiscoveryFailed,
            message: message.into(),
            retryable: true,
            suggested_action: Some(
                "Check local network interfaces, multicast routing, and firewall policy."
                    .to_owned(),
            ),
        }
    }

    pub fn serialization_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::SerializationFailed,
            message: message.into(),
            retryable: false,
            suggested_action: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.into(),
            retryable: false,
            suggested_action: None,
        }
    }

    pub const fn exit_code(&self) -> u8 {
        self.code.exit_code()
    }
}

#[cfg(windows)]
const fn native_credential_suggested_action() -> &'static str {
    "Check that Windows Credential Manager is available. For one-shot automation, use OXVIF_USERNAME and OXVIF_PASSWORD; oxvif never creates a plaintext fallback."
}

#[cfg(target_os = "macos")]
const fn native_credential_suggested_action() -> &'static str {
    "Unlock the login Keychain and allow this terminal session to use it. For one-shot automation, use OXVIF_USERNAME and OXVIF_PASSWORD; oxvif never creates a plaintext fallback."
}

#[cfg(target_os = "linux")]
const fn native_credential_suggested_action() -> &'static str {
    "Start and unlock a Secret Service provider in this D-Bus session. For one-shot automation, use OXVIF_USERNAME and OXVIF_PASSWORD; oxvif never creates a plaintext fallback."
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
const fn native_credential_suggested_action() -> &'static str {
    "Use OXVIF_USERNAME and OXVIF_PASSWORD for one-shot automation; oxvif never creates a plaintext credential fallback."
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}
