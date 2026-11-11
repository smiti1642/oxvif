//! Qualified pull-point references and notification data.

use std::time::Duration;

use reqwest::Url;

use crate::soap::qualified::{self, ADDRESSING, EXTENSION_BYTES, SOAP, response};
pub use crate::soap::qualified::{QualifiedAttribute, QualifiedContent, QualifiedElement};
use crate::{
    NotificationMessage, OnvifClient, OnvifError, soap::SoapError, transport::SoapRequestOptions,
    types::xml_escape,
};
mod managed;
pub use managed::*;

const EVENTS: &str = "http://www.onvif.org/ver10/events/wsdl";
const NOTIFICATION: &str = "http://docs.oasis-open.org/wsn/b-2";
const SCHEMA: &str = "http://www.onvif.org/ver10/schema";

pub use crate::transport::EndpointPolicy;

/// A subscription endpoint and its ordered, qualified reference parameters.
#[derive(Clone)]
pub struct EndpointReference {
    address: String,
    parameters: Vec<QualifiedElement>,
}

impl EndpointReference {
    /// Device-advertised destination after URL validation.
    pub fn address(&self) -> &str {
        self.address.as_str()
    }
    /// Parameters in device order. These values may contain private data.
    pub fn parameters(&self) -> &[QualifiedElement] {
        &self.parameters
    }
}

/// Raw lease timestamps from the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLeaseTimes {
    /// Device clock at the response, when present.
    pub current_time: Option<String>,
    /// Granted expiry, when present.
    pub termination_time: Option<String>,
}

/// A created pull point with its qualified address and granted timing.
#[derive(Clone)]
pub struct ReferencedPullPoint {
    /// Address and routing parameters for subsequent requests.
    pub reference: EndpointReference,
    /// Initial lease timestamps.
    pub times: EventLeaseTimes,
}

impl ReferencedPullPoint {
    /// Project to the legacy address-only structure, discarding parameters.
    pub fn to_legacy(&self) -> crate::PullPointSubscription {
        crate::PullPointSubscription {
            reference_url: self.reference.address().to_owned(),
            termination_time: self.times.termination_time.clone().unwrap_or_default(),
        }
    }
}

/// Ordered ONVIF message item.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventItem {
    /// String-valued item.
    Simple {
        /// Item name.
        name: String,
        /// Device value.
        value: String,
    },
    /// XML-valued item, including its qualified descendants.
    Element(QualifiedElement),
}

/// Qualified notification with ordered Source, Key and Data items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedNotification {
    /// Topic expression, dialect attribute and bindings in scope.
    pub topic: Option<QualifiedElement>,
    /// Device UTC timestamp.
    pub utc_time: String,
    /// Property operation value.
    pub property_operation: String,
    /// Source identity items.
    pub source: Vec<EventItem>,
    /// Key items.
    pub key: Vec<EventItem>,
    /// Data items.
    pub data: Vec<EventItem>,
    retained_bytes: usize,
}

impl QualifiedNotification {
    /// Project to legacy simple items. Duplicate names use the last value.
    /// Keys, qualified topic identity and XML items are discarded.
    pub fn to_legacy(&self) -> NotificationMessage {
        fn simple(items: &[EventItem]) -> std::collections::HashMap<String, String> {
            items
                .iter()
                .filter_map(|item| match item {
                    EventItem::Simple { name, value } => Some((name.clone(), value.clone())),
                    EventItem::Element(_) => None,
                })
                .collect()
        }
        NotificationMessage {
            topic: self
                .topic
                .as_ref()
                .and_then(|t| t.scalar().ok())
                .unwrap_or_default()
                .trim()
                .to_owned(),
            utc_time: self.utc_time.clone(),
            property_operation: self.property_operation.clone(),
            source: simple(&self.source),
            data: simple(&self.data),
        }
    }

    /// Accounted serialized bytes before projection to typed items.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

/// A qualified PullMessages response.
#[derive(Debug, Clone)]
pub struct ReferencedEventPage {
    /// Device clock and expiry observed with this page.
    pub times: EventLeaseTimes,
    /// Notifications in device order.
    pub notifications: Vec<QualifiedNotification>,
}

fn required<'a>(
    node: &'a QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<&'a QualifiedElement, SoapError> {
    node.one(ns, name)?.ok_or_else(|| SoapError::missing(name))
}

fn scalar(
    node: &QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<Option<String>, SoapError> {
    node.one(ns, name)?
        .map(QualifiedElement::scalar)
        .transpose()
}

fn times(node: &QualifiedElement, ns: &str) -> Result<EventLeaseTimes, SoapError> {
    Ok(EventLeaseTimes {
        current_time: scalar(node, ns, "CurrentTime")?,
        termination_time: scalar(node, ns, "TerminationTime")?,
    })
}

fn parse_reference(
    node: &QualifiedElement,
    device: &str,
    issuer: &Url,
    policy: &EndpointPolicy,
) -> Result<EndpointReference, OnvifError> {
    let address = required(node, ADDRESSING, "Address")?.scalar()?;
    let address = address.trim();
    let destination = policy.check(device, address)?;
    if issuer.scheme() == "https" && destination.scheme() == "http" {
        return Err(OnvifError::InvalidArgument(
            "event reference downgrades HTTPS".into(),
        ));
    }
    let mut parameters = Vec::new();
    let mut bytes = 0;
    if let Some(parent) = node.one(ADDRESSING, "ReferenceParameters")? {
        for element in parent.children() {
            let ns = element.namespace();
            if ns.is_empty()
                || ns == SOAP
                || ns == ADDRESSING
                || ns == "http://schemas.xmlsoap.org/soap/envelope/"
                || ns == "http://www.w3.org/2000/09/xmldsig#"
                || ns.starts_with("http://docs.oasis-open.org/wss/")
            {
                return Err(qualified::invalid("reserved reference parameter namespace").into());
            }
            bytes += element.serialized_len();
            if parameters.len() == 64 || bytes > EXTENSION_BYTES {
                return Err(
                    qualified::invalid("reference parameter retention limit exceeded").into(),
                );
            }
            parameters.push(element.clone());
        }
    }
    Ok(EndpointReference {
        address: address.to_owned(),
        parameters,
    })
}

pub(crate) fn notification(node: &QualifiedElement) -> Result<QualifiedNotification, SoapError> {
    let retained_bytes = node.serialized_len();
    if retained_bytes > EXTENSION_BYTES {
        return Err(qualified::invalid("notification retention limit exceeded"));
    }
    fn items(node: &QualifiedElement, name: &'static str) -> Result<Vec<EventItem>, SoapError> {
        let Some(list) = node.one(SCHEMA, name)? else {
            return Ok(Vec::new());
        };
        list.children()
            .filter(|item| item.namespace() == SCHEMA)
            .filter_map(|item| match item.local_name() {
                "SimpleItem" => Some((|| {
                    Ok(EventItem::Simple {
                        name: item
                            .attr("", "Name")
                            .ok_or_else(|| SoapError::missing("SimpleItem/Name"))?
                            .to_owned(),
                        value: item
                            .attr("", "Value")
                            .ok_or_else(|| SoapError::missing("SimpleItem/Value"))?
                            .to_owned(),
                    })
                })()),
                "ElementItem" => Some(Ok(EventItem::Element(item.clone()))),
                _ => None,
            })
            .collect()
    }
    let message = required(required(node, NOTIFICATION, "Message")?, SCHEMA, "Message")?;
    Ok(QualifiedNotification {
        topic: node.one(NOTIFICATION, "Topic")?.cloned(),
        utc_time: message.attr("", "UtcTime").unwrap_or_default().to_owned(),
        property_operation: message
            .attr("", "PropertyOperation")
            .unwrap_or_default()
            .to_owned(),
        source: items(message, "Source")?,
        key: items(message, "Key")?,
        data: items(message, "Data")?,
        retained_bytes,
    })
}

impl OnvifClient {
    /// Create a pull point retaining WS-Addressing reference parameters.
    pub async fn create_referenced_pull_point(
        &self,
        events_url: &str,
        initial_termination: &str,
        policy: &EndpointPolicy,
        timeout: Duration,
    ) -> Result<ReferencedPullPoint, OnvifError> {
        let url = policy.check(self.device_url(), events_url)?;
        let xml = self.call_addressed(url.as_str(),
            "http://www.onvif.org/ver10/events/wsdl/EventPortType/CreatePullPointSubscriptionRequest",
            &format!("<tev:CreatePullPointSubscription><tev:InitialTerminationTime>{}</tev:InitialTerminationTime></tev:CreatePullPointSubscription>", xml_escape(initial_termination)),
            &[], SoapRequestOptions::with_timeout(timeout)).await?;
        let node = response(&xml, EVENTS, "CreatePullPointSubscriptionResponse")?;
        Ok(ReferencedPullPoint {
            reference: parse_reference(
                required(&node, EVENTS, "SubscriptionReference")?,
                self.device_url(),
                &url,
                policy,
            )?,
            times: times(&node, NOTIFICATION)?,
        })
    }

    /// Pull qualified notifications using the full subscription reference.
    pub async fn pull_referenced_messages(
        &self,
        reference: &EndpointReference,
        wait_time: &str,
        max_messages: u32,
        policy: &EndpointPolicy,
        timeout: Duration,
    ) -> Result<ReferencedEventPage, OnvifError> {
        if max_messages == 0 || max_messages > i32::MAX as u32 {
            return Err(OnvifError::InvalidArgument(
                "invalid event message limit".into(),
            ));
        }
        policy.check(self.device_url(), reference.address())?;
        let xml = self.call_addressed(reference.address(),
            "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest",
            &format!("<tev:PullMessages><tev:Timeout>{}</tev:Timeout><tev:MessageLimit>{max_messages}</tev:MessageLimit></tev:PullMessages>", xml_escape(wait_time)),
            reference.parameters(), SoapRequestOptions::with_timeout(timeout)).await?;
        let node = response(&xml, EVENTS, "PullMessagesResponse")?;
        let messages = node.children().filter(|child| {
            child.namespace() == NOTIFICATION && child.local_name() == "NotificationMessage"
        });
        let mut notifications = Vec::new();
        let mut bytes = 0;
        for message in messages {
            if notifications.len() == max_messages as usize {
                return Err(qualified::invalid("event response exceeds message limit").into());
            }
            let message = notification(message)?;
            bytes += message.retained_bytes;
            if bytes > EXTENSION_BYTES {
                return Err(qualified::invalid("event page retention limit exceeded").into());
            }
            notifications.push(message);
        }
        Ok(ReferencedEventPage {
            times: times(&node, EVENTS)?,
            notifications,
        })
    }

    /// Renew an addressed pull point and retain both lease timestamps.
    pub async fn renew_referenced_pull_point(
        &self,
        reference: &EndpointReference,
        termination: &str,
        policy: &EndpointPolicy,
        timeout: Duration,
    ) -> Result<EventLeaseTimes, OnvifError> {
        policy.check(self.device_url(), reference.address())?;
        let xml = self
            .call_addressed(
                reference.address(),
                "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest",
                &format!(
                    "<wsnt:Renew><wsnt:TerminationTime>{}</wsnt:TerminationTime></wsnt:Renew>",
                    xml_escape(termination)
                ),
                reference.parameters(),
                SoapRequestOptions::with_timeout(timeout),
            )
            .await?;
        Ok(times(
            &response(&xml, NOTIFICATION, "RenewResponse")?,
            NOTIFICATION,
        )?)
    }

    /// Unsubscribe using the full reference, within the supplied deadline.
    pub async fn unsubscribe_referenced_pull_point(
        &self,
        reference: &EndpointReference,
        policy: &EndpointPolicy,
        timeout: Duration,
    ) -> Result<(), OnvifError> {
        policy.check(self.device_url(), reference.address())?;
        let xml = self
            .call_addressed(
                reference.address(),
                "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest",
                "<wsnt:Unsubscribe/>",
                reference.parameters(),
                SoapRequestOptions::with_timeout(timeout),
            )
            .await?;
        response(&xml, NOTIFICATION, "UnsubscribeResponse")?;
        Ok(())
    }
}
