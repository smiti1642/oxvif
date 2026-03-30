use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SoapError {
    #[error("XML parse error: {0}")]
    XmlParse(String),

    #[error("Missing <s:Body> in SOAP response")]
    MissingBody,

    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    /// 回應 tag 不符合預期（可能是裝置回傳了非預期的操作）
    #[error("Expected response tag '{0}' not found in Body")]
    UnexpectedResponse(String),

    /// 裝置回傳 SOAP Fault
    #[error("SOAP fault [{code}]: {reason}")]
    Fault { code: String, reason: String },

    #[error("Invalid value '{value}' for field '{field}'")]
    InvalidValue { field: &'static str, value: String },
}

impl SoapError {
    pub fn missing(field: &'static str) -> Self {
        Self::MissingField(field)
    }

    pub fn invalid(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidValue {
            field,
            value: value.into(),
        }
    }
}
