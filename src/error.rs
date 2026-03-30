//! Top-level error type for the oxvif library.
//!
//! All public async methods on [`OnvifClient`](crate::client::OnvifClient)
//! return `Result<T, OnvifError>`. Callers can match on the two variants to
//! distinguish transport-level failures (network, TLS) from SOAP-level
//! failures (malformed response, SOAP Fault from the device).

use thiserror::Error;

use crate::soap::SoapError;
use crate::transport::TransportError;

// ── OnvifError ────────────────────────────────────────────────────────────────

/// The unified error type returned by every ONVIF operation.
///
/// # Variants
///
/// * [`Transport`](OnvifError::Transport) — a network or TLS error occurred
///   before a response was received, or the server replied with an unexpected
///   HTTP status code (anything other than 200 or 500).
///
/// * [`Soap`](OnvifError::Soap) — a response was received but could not be
///   parsed, a required field was missing, or the device returned a
///   `<s:Fault>` element.
#[derive(Debug, Error)]
pub enum OnvifError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Soap(#[from] SoapError),
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_transport_http_status() {
        let t_err = TransportError::HttpStatus {
            status: 401,
            body: "Unauthorized".into(),
        };
        let err: OnvifError = t_err.into();
        assert!(matches!(err, OnvifError::Transport(_)));
        assert!(err.to_string().contains("401"));
    }

    #[test]
    fn test_from_soap_missing_body() {
        let err: OnvifError = SoapError::MissingBody.into();
        assert!(matches!(err, OnvifError::Soap(_)));
        assert!(err.to_string().contains("Body"));
    }

    #[test]
    fn test_from_soap_fault_contains_code_and_reason() {
        let err: OnvifError = SoapError::Fault {
            code: "s:Sender".into(),
            reason: "Not Authorized".into(),
        }
        .into();
        let msg = err.to_string();
        assert!(msg.contains("s:Sender"));
        assert!(msg.contains("Not Authorized"));
    }

    #[test]
    fn test_from_soap_missing_field() {
        let err: OnvifError = SoapError::missing("Capabilities").into();
        assert!(matches!(
            err,
            OnvifError::Soap(SoapError::MissingField("Capabilities"))
        ));
    }

    #[test]
    fn test_display_transport_wraps_message() {
        let err: OnvifError = TransportError::HttpStatus {
            status: 503,
            body: "Service Unavailable".into(),
        }
        .into();
        assert!(err.to_string().contains("503"));
    }
}
