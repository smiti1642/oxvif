//! Qualified Analytics and RuleEngine reads and metadata XML parsing.

use crate::{
    OnvifClient, OnvifError, OnvifSession,
    events::{EventItem, QualifiedElement, QualifiedNotification, notification},
    scoped_rpc::{SCHEMA, ScopedService, boolean, field, required, token},
    soap::{
        SoapError,
        qualified::{self, EXTENSION_BYTES, invalid},
    },
    transport::EndpointPolicy,
};

const ANALYTICS: &str = "http://www.onvif.org/ver20/analytics/wsdl";
const WSNT: &str = "http://docs.oasis-open.org/wsn/b-2";

/// Client for a trusted Analytics service.
pub struct AnalyticsService {
    service: ScopedService,
}

impl OnvifClient {
    /// Bind Analytics reads to a service on the configured device origin.
    pub fn analytics(&self, endpoint: &str) -> Result<AnalyticsService, OnvifError> {
        let url = EndpointPolicy::default().check(self.device_url(), endpoint)?;
        Ok(AnalyticsService::new(self, url))
    }
}

impl OnvifSession {
    /// Resolve Analytics from capabilities or a GetServices listing.
    pub async fn analytics(&self) -> Result<AnalyticsService, OnvifError> {
        let client = self.client();
        let endpoint = match &self.capabilities().analytics.url {
            Some(url) => url.clone(),
            None => {
                client
                    .direct()
                    .get_services()
                    .await?
                    .into_iter()
                    .find(|service| service.namespace == ANALYTICS)
                    .ok_or_else(|| SoapError::missing("Analytics service URL"))?
                    .url
            }
        };
        let url = self.endpoint_policy.check(client.device_url(), &endpoint)?;
        Ok(AnalyticsService::new(client, url))
    }
}

/// Analytics and RuleEngine support advertised by the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsSupport {
    /// Rules interface is supported.
    pub rules: Option<bool>,
    /// Analytics modules interface is supported.
    pub modules: Option<bool>,
    /// Device can return rule options.
    pub rule_options: Option<bool>,
    /// Device can return module options.
    pub module_options: Option<bool>,
    /// Device exposes supported metadata descriptions.
    pub supported_metadata: Option<bool>,
    /// Original bounded capabilities, including vendor fields.
    pub xml: QualifiedElement,
}

/// Expanded XML name for a rule, module or parameter type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnalyticsName {
    /// Namespace resolved in scope, or empty for an unqualified name.
    pub namespace: String,
    /// Local name, independent of a device's chosen prefix.
    pub local_name: String,
}

/// Rule or analytics module family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsKind {
    /// RuleEngine rule.
    Rule,
    /// Analytics module.
    Module,
}

/// An ordered value or retained vendor element in a configuration parameter list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyticsParameter {
    /// Standard simple or qualified XML-valued item.
    Value(EventItem),
    /// Vendor or extension element kept as received.
    Extension(QualifiedElement),
}

/// A configured rule or module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsConfiguration {
    /// Instance name.
    pub name: String,
    /// Expanded rule or module type.
    pub kind: AnalyticsName,
    /// Parameter values in document order.
    pub parameters: Vec<AnalyticsParameter>,
    /// Original bounded configuration, including vendor fields.
    pub xml: QualifiedElement,
}

/// One parameter from the supported type description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyticsParameterDescription {
    /// A standard simple or XML-valued item description.
    Item {
        /// Parameter name.
        name: String,
        /// QName of the value type.
        value_type: AnalyticsName,
        /// Whether the value contains XML rather than a simple scalar.
        element: bool,
    },
    /// Vendor description retained for a caller that understands it.
    Extension(QualifiedElement),
}

/// A declared module or rule type with typed parameter descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsDescription {
    /// Expanded type identity.
    pub kind: AnalyticsName,
    /// Parameter descriptors in document order.
    pub parameters: Vec<AnalyticsParameterDescription>,
    /// Original descriptor, including event declarations and extensions.
    pub xml: QualifiedElement,
}

/// Device support for one configuration and family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsCatalogue {
    /// Optional number of instances the device can create.
    pub limit: Option<u32>,
    /// Schema locations advertised by the device. They are never fetched.
    pub schema_locations: Vec<String>,
    /// Type descriptions in document order.
    pub descriptions: Vec<AnalyticsDescription>,
    /// Extension fields on the catalogue itself.
    pub extensions: Vec<QualifiedElement>,
}

impl AnalyticsService {
    fn new(client: &OnvifClient, url: reqwest::Url) -> Self {
        Self {
            service: ScopedService {
                client: client.direct(),
                url,
                namespace: ANALYTICS,
            },
        }
    }

    /// Read the service's explicit capability flags.
    pub async fn capabilities(&self) -> Result<AnalyticsSupport, OnvifError> {
        let response = self
            .service
            .call(
                "GetServiceCapabilities",
                "GetServiceCapabilitiesResponse",
                "",
            )
            .await?;
        let caps = required(&response, ANALYTICS, "Capabilities")?;
        check_size(caps)?;
        let flag = |name| caps.attr("", name).map(boolean).transpose();
        Ok(AnalyticsSupport {
            rules: flag("RuleSupport")?,
            modules: flag("AnalyticsModuleSupport")?,
            rule_options: flag("RuleOptionsSupported")?,
            module_options: flag("AnalyticsModuleOptionsSupported")?,
            supported_metadata: flag("SupportedMetadata")?,
            xml: caps.clone(),
        })
    }

    /// Read configured rules or modules for a Media analytics configuration token.
    pub async fn configurations(
        &self,
        kind: AnalyticsKind,
        configuration: &str,
    ) -> Result<Vec<AnalyticsConfiguration>, OnvifError> {
        token(configuration)?;
        let (op, reply, item) = match kind {
            AnalyticsKind::Rule => ("GetRules", "GetRulesResponse", "Rule"),
            AnalyticsKind::Module => (
                "GetAnalyticsModules",
                "GetAnalyticsModulesResponse",
                "AnalyticsModule",
            ),
        };
        let response = self
            .service
            .call(op, reply, &field("ConfigurationToken", configuration))
            .await?;
        check_size(&response)?;
        response
            .children()
            .map(|child| {
                if child.namespace() != ANALYTICS || child.local_name() != item {
                    return Err(invalid("unexpected analytics configuration entry").into());
                }
                parse_configuration(child).map_err(Into::into)
            })
            .collect()
    }

    /// Read supported type names, parameter descriptors and schema locations.
    pub async fn supported(
        &self,
        kind: AnalyticsKind,
        configuration: &str,
    ) -> Result<AnalyticsCatalogue, OnvifError> {
        token(configuration)?;
        let (op, reply, list, schema, item) = match kind {
            AnalyticsKind::Rule => (
                "GetSupportedRules",
                "GetSupportedRulesResponse",
                "SupportedRules",
                "RuleContentSchemaLocation",
                "RuleDescription",
            ),
            AnalyticsKind::Module => (
                "GetSupportedAnalyticsModules",
                "GetSupportedAnalyticsModulesResponse",
                "SupportedAnalyticsModules",
                "AnalyticsModuleContentSchemaLocation",
                "AnalyticsModuleDescription",
            ),
        };
        let response = self
            .service
            .call(op, reply, &field("ConfigurationToken", configuration))
            .await?;
        let catalogue = required(&response, ANALYTICS, list)?;
        check_size(catalogue)?;
        let limit = catalogue
            .attr("", "Limit")
            .map(|value| {
                value
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| invalid("invalid analytics instance limit"))
            })
            .transpose()?;
        let mut result = AnalyticsCatalogue {
            limit,
            schema_locations: Vec::new(),
            descriptions: Vec::new(),
            extensions: Vec::new(),
        };
        let mut names = std::collections::HashSet::new();
        for child in catalogue.children() {
            match (child.namespace(), child.local_name()) {
                (SCHEMA, name) if name == schema => result.schema_locations.push(child.scalar()?),
                (SCHEMA, name) if name == item => {
                    let description = parse_description(child)?;
                    if !names.insert(description.kind.clone()) {
                        return Err(invalid("duplicate analytics type").into());
                    }
                    result.descriptions.push(description);
                }
                _ => result.extensions.push(child.clone()),
            }
        }
        Ok(result)
    }
}

/// Bounded ONVIF metadata XML with original frame and extension order preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataDocument {
    /// Original XML tree, including frames, vendor fields and QName bindings.
    pub xml: QualifiedElement,
    /// Standard event notifications in document order.
    pub notifications: Vec<QualifiedNotification>,
}

impl MetadataDocument {
    /// Parse one complete MetadataStream document. Packet assembly is a separate layer.
    pub fn parse(xml: &str) -> Result<Self, SoapError> {
        if xml.len() > EXTENSION_BYTES {
            return Err(invalid("metadata document retention limit exceeded"));
        }
        let root = qualified::parse(xml)?;
        if root.namespace() != SCHEMA || root.local_name() != "MetadataStream" {
            return Err(invalid("expected ONVIF MetadataStream"));
        }
        check_size(&root)?;
        let notifications = root
            .children()
            .filter(|node| node.namespace() == SCHEMA && node.local_name() == "Event")
            .flat_map(QualifiedElement::children)
            .filter(|node| node.namespace() == WSNT && node.local_name() == "NotificationMessage")
            .map(notification)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            xml: root,
            notifications,
        })
    }
}

fn check_size(element: &QualifiedElement) -> Result<(), SoapError> {
    if element.serialized_len() > EXTENSION_BYTES {
        return Err(invalid("analytics retention limit exceeded"));
    }
    Ok(())
}

fn name<'a>(node: &'a QualifiedElement, field: &'static str) -> Result<&'a str, SoapError> {
    node.attr("", field)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SoapError::missing(field))
}

fn qname(node: &QualifiedElement, attribute: &'static str) -> Result<AnalyticsName, SoapError> {
    let lexical = name(node, attribute)?.trim();
    if !qualified::valid_qname(lexical) {
        return Err(invalid("invalid analytics QName"));
    }
    let (namespace, local_name) = match lexical.split_once(':') {
        Some(("xml", local)) => ("http://www.w3.org/XML/1998/namespace", local),
        Some((prefix, local)) => (
            node.namespace_for_prefix(prefix)
                .ok_or_else(|| invalid("unbound analytics QName"))?,
            local,
        ),
        None => (node.namespace_for_prefix("").unwrap_or_default(), lexical),
    };
    Ok(AnalyticsName {
        namespace: namespace.into(),
        local_name: local_name.into(),
    })
}

fn parse_configuration(node: &QualifiedElement) -> Result<AnalyticsConfiguration, SoapError> {
    let parameters = required(node, SCHEMA, "Parameters")?
        .children()
        .map(|item| {
            Ok(match (item.namespace(), item.local_name()) {
                (SCHEMA, "SimpleItem") => AnalyticsParameter::Value(EventItem::Simple {
                    name: name(item, "Name")?.into(),
                    value: item
                        .attr("", "Value")
                        .ok_or_else(|| SoapError::missing("Value"))?
                        .into(),
                }),
                (SCHEMA, "ElementItem") => {
                    name(item, "Name")?;
                    AnalyticsParameter::Value(EventItem::Element(item.clone()))
                }
                _ => AnalyticsParameter::Extension(item.clone()),
            })
        })
        .collect::<Result<Vec<_>, SoapError>>()?;
    Ok(AnalyticsConfiguration {
        name: name(node, "Name")?.into(),
        kind: qname(node, "Type")?,
        parameters,
        xml: node.clone(),
    })
}

fn parse_description(node: &QualifiedElement) -> Result<AnalyticsDescription, SoapError> {
    let mut names = std::collections::HashSet::new();
    let parameters = required(node, SCHEMA, "Parameters")?
        .children()
        .map(|item| match (item.namespace(), item.local_name()) {
            (SCHEMA, "SimpleItemDescription" | "ElementItemDescription") => {
                let name = name(item, "Name")?;
                if !names.insert(name) {
                    return Err(invalid("duplicate analytics parameter name"));
                }
                Ok(AnalyticsParameterDescription::Item {
                    name: name.into(),
                    value_type: qname(item, "Type")?,
                    element: item.local_name() == "ElementItemDescription",
                })
            }
            _ => Ok(AnalyticsParameterDescription::Extension(item.clone())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AnalyticsDescription {
        kind: qname(node, "Name")?,
        parameters,
        xml: node.clone(),
    })
}
