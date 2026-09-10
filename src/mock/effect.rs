//! Explicit effects of reviewed synthetic operations. This is not inferred from
//! SOAP text, a write-like name, or the caller-owned persistence notification.

use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    ProfilesChanged,
}

pub(crate) type EffectObserver = Arc<dyn Fn(Effect) + Send + Sync>;

/// Staged migration boundary. Unlisted replay writes retain their legacy policy
/// until their own operation review provides an explicit committed effect.
#[cfg(feature = "metamorph")]
pub(crate) fn tracks_commit(action: &str) -> bool {
    matches!(
        action,
        "http://www.onvif.org/ver10/media/wsdl/DeleteProfile"
            | "http://www.onvif.org/ver20/media/wsdl/DeleteProfile"
            | "http://www.onvif.org/ver10/media/wsdl/CreateProfile"
            | "http://www.onvif.org/ver20/media/wsdl/CreateProfile"
            | "http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration"
            | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration"
            | "http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration"
            | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration"
            | "http://www.onvif.org/ver20/media/wsdl/AddConfiguration"
            | "http://www.onvif.org/ver20/media/wsdl/RemoveConfiguration"
    )
}
