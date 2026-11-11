use crate::OnvifError;
use reqwest::Url;

/// Explicit trust for service and subscription origins.
///
/// The configured device origin is allowed by default. An additional origin
/// must be supplied explicitly. HTTPS to HTTP downgrade is refused.
#[derive(Clone, Default)]
pub struct EndpointPolicy {
    allowed: Vec<Url>,
}

impl EndpointPolicy {
    /// Allow an additional origin selected by the caller.
    pub fn allow_origin(mut self, url: &str) -> Result<Self, OnvifError> {
        self.allowed.push(endpoint_url(url)?);
        Ok(self)
    }

    pub(crate) fn check(&self, device: &str, destination: &str) -> Result<Url, OnvifError> {
        let device = endpoint_url(device)?;
        let dest = endpoint_url(destination)?;
        let origin = dest.origin();
        if (device.scheme() == "https" && dest.scheme() == "http")
            || (origin != device.origin() && !self.allowed.iter().any(|url| url.origin() == origin))
        {
            return Err(OnvifError::InvalidArgument(
                "SOAP endpoint origin is not trusted".into(),
            ));
        }
        Ok(dest)
    }
}

fn endpoint_url(value: &str) -> Result<Url, OnvifError> {
    if value
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err(OnvifError::InvalidArgument(
            "invalid HTTP(S) SOAP endpoint".into(),
        ));
    }
    Url::parse(value)
        .ok()
        .filter(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        })
        .ok_or_else(|| OnvifError::InvalidArgument("invalid HTTP(S) SOAP endpoint".into()))
}
