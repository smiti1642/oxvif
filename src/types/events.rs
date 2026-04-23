//! Typed response structs for the ONVIF Events service.

use std::collections::HashMap;

use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── PullPointSubscription ─────────────────────────────────────────────────────

/// A pull-point event subscription returned by `CreatePullPointSubscription`.
///
/// Use `reference_url` as the endpoint for `pull_messages`,
/// `renew_subscription`, and `unsubscribe`.
#[derive(Debug, Clone)]
pub struct PullPointSubscription {
    /// Subscription manager endpoint URL.
    pub reference_url: String,
    /// ISO-8601 timestamp when the subscription expires.
    pub termination_time: String,
}

impl PullPointSubscription {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let reference_url = resp
            .path(&["SubscriptionReference", "Address"])
            .map(|n| n.text().to_string())
            .ok_or_else(|| SoapError::missing("SubscriptionReference/Address"))?;
        let termination_time = resp
            .child("TerminationTime")
            .map(|n| n.text().to_string())
            .unwrap_or_default();
        Ok(Self {
            reference_url,
            termination_time,
        })
    }
}

// ── PushSubscription ─────────────────────────────────────────────────────────

/// A WS-BaseNotification push subscription returned by `subscribe`.
///
/// Use `subscription_reference` as the endpoint for
/// [`renew_subscription`](crate::OnvifClient::renew_subscription) and
/// [`unsubscribe`](crate::OnvifClient::unsubscribe).
///
/// The device will POST `Notify` messages to the `consumer_url` you supplied
/// to [`subscribe`](crate::OnvifClient::subscribe). Use
/// [`notification_listener`](crate::notification_listener) to receive them.
#[derive(Debug, Clone)]
pub struct PushSubscription {
    /// Subscription manager endpoint URL — the device's subscription reference.
    pub subscription_reference: String,
    /// ISO-8601 current device time at the moment of subscription.
    pub current_time: String,
    /// ISO-8601 timestamp when the subscription expires.
    pub termination_time: String,
}

impl PushSubscription {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let subscription_reference = resp
            .path(&["SubscriptionReference", "Address"])
            .map(|n| n.text().to_string())
            .ok_or_else(|| SoapError::missing("SubscriptionReference/Address"))?;
        Ok(Self {
            subscription_reference,
            current_time: resp
                .child("CurrentTime")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            termination_time: resp
                .child("TerminationTime")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
        })
    }
}

// ── NotificationMessage ───────────────────────────────────────────────────────

/// A single ONVIF event notification received via `PullMessages`.
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    /// Topic path (e.g. `tns1:VideoSource/MotionAlarm`).
    pub topic: String,
    /// UTC timestamp from the `Message/@UtcTime` attribute.
    pub utc_time: String,
    /// `Message/@PropertyOperation` — typically `Initialized`, `Changed`,
    /// or `Deleted`. Empty when the device omits the attribute.
    pub property_operation: String,
    /// Source `SimpleItem` pairs (e.g. `VideoSourceToken = "VideoSource_1"`).
    pub source: HashMap<String, String>,
    /// Data `SimpleItem` pairs (e.g. `IsMotion = "true"`).
    pub data: HashMap<String, String>,
}

impl NotificationMessage {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Vec<Self> {
        resp.children_named("NotificationMessage")
            .map(Self::from_node)
            .collect()
    }

    fn from_node(node: &XmlNode) -> Self {
        let topic = node
            .child("Topic")
            .map(|n| n.text().trim().to_string())
            .unwrap_or_default();

        // The inner Message element may be wrapped: .../Message/Message[@UtcTime]
        let msg = node
            .path(&["Message", "Message"])
            .or_else(|| node.child("Message"));

        let utc_time = msg
            .and_then(|n| n.attr("UtcTime").map(str::to_string))
            .unwrap_or_default();

        let property_operation = msg
            .and_then(|n| n.attr("PropertyOperation").map(str::to_string))
            .unwrap_or_default();

        let source = msg
            .and_then(|n| n.child("Source"))
            .map(parse_simple_items)
            .unwrap_or_default();

        let data = msg
            .and_then(|n| n.child("Data"))
            .map(parse_simple_items)
            .unwrap_or_default();

        Self {
            topic,
            utc_time,
            property_operation,
            source,
            data,
        }
    }
}

fn parse_simple_items(node: &XmlNode) -> HashMap<String, String> {
    node.children_named("SimpleItem")
        .filter_map(|item| {
            let name = item.attr("Name")?.to_string();
            let value = item.attr("Value").unwrap_or("").to_string();
            Some((name, value))
        })
        .collect()
}

// ── EventProperties ───────────────────────────────────────────────────────────

/// Available event topics advertised by the device.
#[derive(Debug, Clone, Default)]
pub struct EventProperties {
    /// Flattened list of all topic paths (e.g. `VideoSource/MotionAlarm`).
    pub topics: Vec<String>,
}

impl EventProperties {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let topic_set = resp
            .child("TopicSet")
            .ok_or_else(|| SoapError::missing("TopicSet"))?;
        let mut topics = Vec::new();
        flatten_topics(topic_set, String::new(), &mut topics);
        Ok(Self { topics })
    }
}

/// Recursively flatten the TopicSet tree into slash-separated paths.
///
/// Leaf nodes are nodes whose children are all `MessageDescription` elements
/// (or that have no children at all). Intermediate nodes become path segments.
fn flatten_topics(node: &XmlNode, prefix: String, out: &mut Vec<String>) {
    for child in &node.children {
        if child.local_name == "MessageDescription" {
            continue;
        }
        let path = if prefix.is_empty() {
            child.local_name.clone()
        } else {
            format!("{prefix}/{}", child.local_name)
        };
        let has_non_desc = child
            .children
            .iter()
            .any(|n| n.local_name != "MessageDescription");
        if has_non_desc {
            flatten_topics(child, path, out);
        } else {
            out.push(path);
        }
    }
}
