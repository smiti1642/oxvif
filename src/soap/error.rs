//! SOAP-layer error types.
//!
//! [`SoapError`] covers everything that can go wrong after a successful HTTP
//! response has been received: XML parse failures, missing required elements,
//! and SOAP Fault responses from the device.

use thiserror::Error;

/// Errors arising from SOAP response parsing or device-reported faults.
#[derive(Debug, Error, PartialEq)]
pub enum SoapError {
    /// The raw response bytes could not be parsed as XML.
    #[error("XML parse error: {0}")]
    XmlParse(String),

    /// The response did not contain a `<s:Body>` element.
    #[error("Missing <s:Body> in SOAP response")]
    MissingBody,

    /// A required XML element or attribute was absent in the response.
    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    /// The response body did not contain the expected operation response tag.
    /// This may indicate the device sent a different operation response.
    #[error("Expected response tag '{0}' not found in Body")]
    UnexpectedResponse(String),

    /// The device returned a `<s:Fault>` with a structured code and reason.
    #[error("SOAP fault [{code}]: {reason}")]
    Fault { code: String, reason: String },

    /// A field was present but its value could not be interpreted.
    #[error("Invalid value '{value}' for field '{field}'")]
    InvalidValue { field: &'static str, value: String },
}

impl SoapError {
    /// Convenience constructor for [`SoapError::MissingField`].
    pub fn missing(field: &'static str) -> Self {
        Self::MissingField(field)
    }

    /// Convenience constructor for [`SoapError::InvalidValue`].
    pub fn invalid(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidValue {
            field,
            value: value.into(),
        }
    }
}
