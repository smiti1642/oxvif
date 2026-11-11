use crate::{
    OnvifClient, OnvifError,
    soap::{
        SoapError,
        qualified::{self, QualifiedElement},
    },
    types::xml_escape,
};

pub(crate) const SCHEMA: &str = "http://www.onvif.org/ver10/schema";
pub(crate) const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";

pub(crate) struct ScopedService {
    pub(crate) client: OnvifClient,
    pub(crate) url: reqwest::Url,
    pub(crate) namespace: &'static str,
}

impl ScopedService {
    pub(crate) async fn call(
        &self,
        operation: &'static str,
        response: &'static str,
        fields: &str,
    ) -> Result<QualifiedElement, OnvifError> {
        let action = format!("{}/{operation}", self.namespace);
        let body = format!(
            "<{operation} xmlns=\"{}\">{fields}</{operation}>",
            self.namespace
        );
        let xml = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.client.call(self.url.as_str(), &action, &body),
        )
        .await
        .map_err(|_| crate::transport::TransportError::Deadline)??;
        qualified::response(&xml, self.namespace, response)
    }
}

pub(crate) fn field(name: &str, value: &str) -> String {
    format!("<{name}>{}</{name}>", xml_escape(value))
}

pub(crate) fn required<'a>(
    node: &'a QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<&'a QualifiedElement, SoapError> {
    node.one(ns, name)?.ok_or_else(|| SoapError::missing(name))
}

pub(crate) fn text(
    node: &QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<String, SoapError> {
    required(node, ns, name)?.scalar()
}

pub(crate) fn scalar<T: std::str::FromStr>(
    node: &QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<T, SoapError> {
    text(node, ns, name)?
        .trim()
        .parse()
        .map_err(|_| qualified::invalid(name))
}

pub(crate) fn token(value: &str) -> Result<(), OnvifError> {
    if value.trim().is_empty()
        || value.chars().take(65).count() > 64
        || value.chars().any(char::is_control)
    {
        return Err(OnvifError::InvalidArgument(
            "invalid ONVIF reference token".into(),
        ));
    }
    Ok(())
}

pub(crate) fn boolean(value: &str) -> Result<bool, SoapError> {
    match value.trim() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(qualified::invalid("invalid XML boolean")),
    }
}
