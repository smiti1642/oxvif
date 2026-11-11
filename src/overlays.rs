//! Media2 OSD and privacy masks on a selected video-source configuration.
//!
//! Mutations use a fresh preflight and one write. Successful responses carry
//! a separate bounded read-back result. Devices may normalize values between
//! these calls. Concurrent callers can still modify the same configuration.

mod mask;
mod osd;

pub use mask::{
    Mask, MaskColor, MaskColorOptions, MaskColorRange, MaskKind, MaskOptions, MaskPoint,
    MaskSettings,
};
#[cfg(feature = "mock")]
pub(crate) use mask::{
    mask_body, parse_settings as parse_mask_settings, validate as validate_mask,
};

use crate::{
    OnvifClient, OnvifError, OnvifSession,
    media_workflow::MediaService,
    scoped_rpc::{MEDIA2, ScopedService, field, text, token},
    soap::{
        SoapError,
        qualified::{QualifiedElement, invalid},
    },
    transport::EndpointPolicy,
};

/// Media2 operations scoped to one video-source configuration.
pub struct Media2Overlays {
    service: ScopedService,
    configuration: String,
}

impl OnvifClient {
    /// Bind overlay operations to a video-source configuration on the device origin.
    pub fn media2_overlays(
        &self,
        endpoint: &str,
        configuration: &str,
    ) -> Result<Media2Overlays, OnvifError> {
        let url = EndpointPolicy::default().check(self.device_url(), endpoint)?;
        Media2Overlays::new(self, url, configuration)
    }
}

impl OnvifSession {
    /// Bind overlay operations to the session's resolved, trusted Media2 endpoint.
    pub fn media2_overlays(&self, configuration: &str) -> Result<Media2Overlays, OnvifError> {
        Media2Overlays::new(
            self.client(),
            self.trusted_media(MediaService::Media2)?,
            configuration,
        )
    }
}

/// Disposition of an unacknowledged overlay write.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OverlayWriteError {
    /// Preflight stopped before sending a write.
    #[error("overlay preflight failed")]
    Preflight(#[source] OnvifError),
    /// The device returned a qualified SOAP fault.
    #[error("device refused overlay write")]
    Refused(#[source] OnvifError),
    /// The reply could not establish the write's remote outcome. Reconcile by
    /// reading the configuration before sending another write.
    #[error("remote overlay write outcome unknown")]
    OutcomeUnknown(#[source] OnvifError),
}

/// One acknowledged mutation and its independent read-back observation.
#[derive(Debug)]
pub struct OverlayWrite<T> {
    /// Token returned by creation or targeted by update/deletion.
    pub token: String,
    /// Device state observed afterwards. For deletion, `true` means absent.
    pub observed: Result<T, OnvifError>,
}

impl Media2Overlays {
    fn new(
        client: &OnvifClient,
        url: reqwest::Url,
        configuration: &str,
    ) -> Result<Self, OnvifError> {
        token(configuration)?;
        Ok(Self {
            service: ScopedService {
                client: client.direct(),
                url,
                namespace: MEDIA2,
            },
            configuration: configuration.into(),
        })
    }

    /// Video-source configuration token (distinct from a source or profile token).
    pub fn configuration_token(&self) -> &str {
        &self.configuration
    }

    /// Read Media2's advertised OSD and mask capability flags.
    pub async fn capabilities(
        &self,
    ) -> Result<crate::types::Media2ServiceCapabilities, OnvifError> {
        self.service
            .client
            .media2_get_service_capabilities(self.service.url.as_str())
            .await
    }

    fn configuration_field(&self) -> String {
        field("ConfigurationToken", &self.configuration)
    }

    async fn write(
        &self,
        op: &'static str,
        response: &'static str,
        body: &str,
    ) -> Result<QualifiedElement, OverlayWriteError> {
        self.service
            .call(op, response, body)
            .await
            .map_err(|error| {
                if matches!(error, OnvifError::Soap(SoapError::Fault { .. })) {
                    OverlayWriteError::Refused(error)
                } else {
                    OverlayWriteError::OutcomeUnknown(error)
                }
            })
    }

    fn created_token(
        response: &QualifiedElement,
        name: &'static str,
    ) -> Result<String, OverlayWriteError> {
        let read = || {
            let value = text(response, MEDIA2, name)?;
            token(&value)?;
            Ok(value)
        };
        read().map_err(OverlayWriteError::OutcomeUnknown)
    }
}

fn preflight_error(reason: &'static str) -> OverlayWriteError {
    OverlayWriteError::Preflight(OnvifError::InvalidArgument(reason.into()))
}

fn entries<T>(
    response: &QualifiedElement,
    name: &'static str,
    scope: &str,
    parse: impl Fn(&QualifiedElement) -> Result<T, OnvifError>,
    identity: impl Fn(&T) -> (&str, &str),
) -> Result<Vec<T>, OnvifError> {
    let mut entries = Vec::new();
    let mut tokens = std::collections::HashSet::new();
    for node in response.children() {
        if node.namespace() != MEDIA2 || node.local_name() != name || entries.len() == 1024 {
            return Err(invalid("unexpected or excessive overlay entries").into());
        }
        let entry = parse(node)?;
        let (token, configuration) = identity(&entry);
        if configuration != scope || !tokens.insert(token.to_owned()) {
            return Err(invalid(
                "overlay response has an unrelated configuration or duplicate token",
            )
            .into());
        }
        entries.push(entry);
    }
    Ok(entries)
}

pub(crate) fn shape(
    node: &QualifiedElement,
    namespace: &str,
    children: &[&str],
    attrs: &[&str],
) -> Result<(), SoapError> {
    if node
        .children()
        .any(|item| item.namespace() != namespace || !children.contains(&item.local_name()))
        || node
            .attributes()
            .iter()
            .any(|attr| !attr.namespace().is_empty() || !attrs.contains(&attr.local_name()))
        || node.content().iter().any(|item| match item {
            crate::soap::qualified::QualifiedContent::Text(text) => !text.trim().is_empty(),
            _ => false,
        })
    {
        return Err(invalid("unmodeled overlay field"));
    }
    Ok(())
}

fn boolean(value: &str) -> Result<bool, SoapError> {
    match value.trim() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(invalid("invalid overlay boolean")),
    }
}
