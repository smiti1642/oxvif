//! Shared read-only retry policy. Callers retain their own attempt units.
use oxvif::{OnvifError, transport::TransportError};
use std::time::Duration;

pub(crate) fn is_retryable_onvif_error(error: &OnvifError) -> bool {
    match error {
        OnvifError::Transport(error) => is_retryable_transport_error(error),
        OnvifError::Soap(_)
        | OnvifError::InvalidArgument(_)
        | OnvifError::RecordingSearchIncomplete { .. } => false,
    }
}

pub(crate) fn is_retryable_transport_error(error: &TransportError) -> bool {
    match error {
        TransportError::Http(error) => {
            error.is_timeout() || error.is_connect() || error.is_body() || error.is_request()
        }
        TransportError::HttpStatus { status, .. } => {
            matches!(*status, 408 | 425 | 429 | 502 | 503 | 504)
        }
        TransportError::ResponseTooLarge { .. } => false,
    }
}

pub(crate) fn retry_delay(attempt: u32, discriminator: &str) -> Duration {
    let exponent = attempt.min(4);
    let base_ms = 100_u64.saturating_mul(1_u64 << exponent);
    let hash = discriminator.bytes().fold(0_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(byte))
    });
    let jitter_ms = hash.wrapping_add(u64::from(attempt)) % 51;
    Duration::from_millis((base_ms + jitter_ms).min(2_000))
}
