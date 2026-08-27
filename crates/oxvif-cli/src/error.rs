use serde::Serialize;

/// Stable machine-readable error categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    InvalidArgument,
    CommandNotFound,
    SerializationFailed,
    Internal,
}

impl ErrorCode {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidArgument => 2,
            Self::CommandNotFound => 3,
            Self::SerializationFailed | Self::Internal => 70,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::CommandNotFound => "COMMAND_NOT_FOUND",
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

    pub fn serialization_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::SerializationFailed,
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
