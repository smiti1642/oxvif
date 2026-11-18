use super::{EventItem, QualifiedNotification};

const MAX_PROPERTIES: usize = 256;

/// Latest snapshots for the current synchronization burst.
pub(super) struct SnapshotCache {
    properties: Vec<(Topic, QualifiedNotification)>,
    bytes: usize,
    byte_limit: usize,
}

#[derive(PartialEq, Eq)]
struct Topic {
    namespace: Option<String>,
    path: String,
}

impl Topic {
    fn from_notification(event: &QualifiedNotification) -> Option<Self> {
        let element = event.topic.as_ref()?;
        let expression = event.topic_expression()?;
        if expression.is_empty() {
            return None;
        }
        let (prefix, path) = expression.split_once(':').unwrap_or(("", &expression));
        if !path.contains([':', '|'])
            && !path.chars().any(char::is_whitespace)
            && let Some(namespace) = element.namespace_for_prefix(prefix)
        {
            return Some(Self {
                namespace: Some(namespace.to_owned()),
                path: path.to_owned(),
            });
        }
        Some(Self {
            namespace: None,
            path: expression,
        })
    }
}

impl SnapshotCache {
    pub(super) fn new(byte_limit: usize) -> Self {
        Self {
            properties: Vec::new(),
            bytes: 0,
            byte_limit,
        }
    }

    /// Track the latest value for a property, with a bounded cache. If the
    /// property cannot be tracked, keep delivering its notifications.
    pub(super) fn is_duplicate(&mut self, event: &QualifiedNotification) -> bool {
        let Some(topic) = Topic::from_notification(event) else {
            return false;
        };
        let initialized = event.property_operation == "Initialized";
        if let Some(index) = self.properties.iter().position(|(cached_topic, previous)| {
            *cached_topic == topic
                && (topic.namespace.is_some() || previous.topic == event.topic)
                && same_items(&previous.source, &event.source)
                && same_items(&previous.key, &event.key)
        }) {
            let (_, previous) = &self.properties[index];
            if same_items(&previous.data, &event.data) {
                match event.property_operation.as_str() {
                    "Initialized" => return true,
                    // The live notification is still delivered. Repeating the
                    // same value doesn't invalidate the snapshot already seen.
                    "Changed" => return false,
                    _ => {}
                }
            }
            let (removed_topic, removed) = self.properties.swap_remove(index);
            self.bytes -= retained_bytes(&removed_topic, &removed);
        }

        let size = retained_bytes(&topic, event);
        if initialized
            && self.properties.len() < MAX_PROPERTIES
            && size <= self.byte_limit - self.bytes
        {
            self.properties.push((topic, event.clone()));
            self.bytes += size;
        }
        false
    }
}

fn retained_bytes(topic: &Topic, event: &QualifiedNotification) -> usize {
    event.retained_bytes() + topic.path.len() + topic.namespace.as_ref().map_or(0, String::len)
}

fn same_items(left: &[EventItem], right: &[EventItem]) -> bool {
    if left == right {
        return true;
    }
    if left.len() != right.len() {
        return false;
    }
    // Simple fields with unique names can arrive in a different order. Keep
    // exact comparison for repeated names and XML items with mixed content.
    match (sorted_simple_items(left), sorted_simple_items(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn sorted_simple_items(items: &[EventItem]) -> Option<Vec<(&str, &str)>> {
    let mut fields: Vec<_> = items
        .iter()
        .map(|item| match item {
            EventItem::Simple { name, value } => Some((name.as_str(), value.as_str())),
            EventItem::Element(_) => None,
        })
        .collect::<Option<_>>()?;
    fields.sort_unstable();
    (!fields.windows(2).any(|pair| pair[0].0 == pair[1].0)).then_some(fields)
}
