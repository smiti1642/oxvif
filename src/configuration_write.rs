//! Disposition and read-back evidence for explicit configuration writes.

use crate::{OnvifError, soap::SoapError};

/// Disposition when a configuration write could not be acknowledged.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigurationWriteError {
    /// Preflight stopped before sending a write.
    #[error("configuration preflight failed")]
    Preflight(#[source] OnvifError),
    /// The device returned a qualified SOAP fault.
    #[error("device refused configuration write")]
    Refused(#[source] OnvifError),
    /// The reply could not establish the write's remote outcome. Reconcile by
    /// reading the configuration before sending another write.
    #[error("remote configuration write outcome unknown")]
    OutcomeUnknown(#[source] OnvifError),
}

impl ConfigurationWriteError {
    pub(crate) fn invalid(reason: &'static str) -> Self {
        Self::Preflight(OnvifError::InvalidArgument(reason.into()))
    }

    pub(crate) fn after_send(error: OnvifError) -> Self {
        if matches!(error, OnvifError::Soap(SoapError::Fault { .. })) {
            Self::Refused(error)
        } else {
            Self::OutcomeUnknown(error)
        }
    }
}

/// One acknowledged mutation and its independent read-back observation.
#[derive(Debug)]
pub struct ConfigurationWrite<T> {
    /// Token returned by creation or targeted by update/deletion.
    pub token: String,
    /// Device state observed afterwards. For deletion, `true` means absent.
    pub observed: Result<T, OnvifError>,
}
