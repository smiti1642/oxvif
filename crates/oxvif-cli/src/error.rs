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
    MissingTarget,
    ConfigUnavailable,
    RegistryIo,
    RegistryCorrupt,
    RegistryVersionUnsupported,
    CredentialUnavailable,
    DeviceConnectionFailed,
    DiscoveryFailed,
    SerializationFailed,
    Internal,
}

impl ErrorCode {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidArgument => 2,
            Self::CommandNotFound | Self::DeviceNotFound | Self::ResourceNotFound => 3,
            Self::DeviceAlreadyExists | Self::ResourceAlreadyExists | Self::ResourceInUse => 4,
            Self::MissingTarget => 5,
            Self::ConfigUnavailable
            | Self::RegistryIo
            | Self::RegistryCorrupt
            | Self::RegistryVersionUnsupported => 10,
            Self::CredentialUnavailable => 11,
            Self::DeviceConnectionFailed | Self::DiscoveryFailed => 20,
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
            Self::MissingTarget => "MISSING_TARGET",
            Self::ConfigUnavailable => "CONFIG_UNAVAILABLE",
            Self::RegistryIo => "REGISTRY_IO",
            Self::RegistryCorrupt => "REGISTRY_CORRUPT",
            Self::RegistryVersionUnsupported => "REGISTRY_VERSION_UNSUPPORTED",
            Self::CredentialUnavailable => "CREDENTIAL_UNAVAILABLE",
            Self::DeviceConnectionFailed => "DEVICE_CONNECTION_FAILED",
            Self::DiscoveryFailed => "DISCOVERY_FAILED",
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
            suggested_action: Some("Run `oxvif device list` to list saved devices.".to_owned()),
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

    pub fn missing_target() -> Self {
        Self {
            code: ErrorCode::MissingTarget,
            message: "No device target was selected.".to_owned(),
            retryable: false,
            suggested_action: Some(
                "Pass --device/--target, set OXVIF_DEVICE, or run `oxvif use <id>`.".to_owned(),
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
                "Set the device credential again with `oxvif device credential set`.".to_owned(),
            ),
        }
    }

    pub fn device_connection_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::DeviceConnectionFailed,
            message: message.into(),
            retryable: true,
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

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}
