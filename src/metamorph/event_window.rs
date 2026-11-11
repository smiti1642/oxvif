//! Offline comparison of two ordered, explicitly delimited event observations.

use std::time::Duration;

use crate::{
    events::QualifiedNotification, recording::RecordingTimestamp, soap::qualified::valid_qname,
};

const MAX_EVENTS: usize = 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;

/// A half-open observation window, with notifications in capture order.
pub struct EventWindow<'a> {
    /// Inclusive start instant.
    pub start: &'a RecordingTimestamp,
    /// Exclusive end instant.
    pub end: &'a RecordingTimestamp,
    /// Qualified notifications from one capture.
    pub notifications: &'a [QualifiedNotification],
}

/// A difference at the indicated position in an ordered comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventWindowDifference {
    /// Zero-based ordinal within both windows.
    pub index: usize,
    /// One side has no event at this ordinal.
    pub missing: bool,
    /// Concrete expanded topic, dialect or PropertyOperation differs.
    pub identity: bool,
    /// Time relative to each window's start differs beyond the tolerance.
    pub timing: bool,
}

/// Selected event evidence, excluding private values and raw XML.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventWindowComparison {
    /// Number of events on the left.
    pub left: usize,
    /// Number of events on the right.
    pub right: usize,
    /// Differences in ascending capture order.
    pub differences: Vec<EventWindowDifference>,
}

/// A window could not provide the selected timing or identity evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EventWindowError {
    /// End must be strictly after start.
    #[error("invalid event observation window")]
    InvalidWindow,
    /// Event time was invalid, outside the window, or out of capture order.
    #[error("invalid event time at ordinal {0}")]
    InvalidTime(usize),
    /// No usable concrete, namespace-resolved topic was available.
    #[error("unexamined event topic at ordinal {0}")]
    UnexaminedTopic(usize),
    /// More than 1,024 events or 4 MiB of accounted bytes in a window.
    #[error("event observation window exceeds limits")]
    Limit,
}

/// Compare topic identity, PropertyOperation and relative time by ordinal.
/// Equal timestamps preserve caller order. Topics must be concrete paths
/// with a bound root. Source, Key, Data and XML extension values are left
/// unexamined. The two windows may start at different instants.
pub fn compare_event_windows(
    left: EventWindow<'_>,
    right: EventWindow<'_>,
    tolerance: Duration,
) -> Result<EventWindowComparison, EventWindowError> {
    let left = evidence(left)?;
    let right = evidence(right)?;
    let mut differences = Vec::new();
    for index in 0..left.len().max(right.len()) {
        let (missing, identity, timing) = match (left.get(index), right.get(index)) {
            (Some(a), Some(b)) => (
                false,
                a.identity != b.identity,
                (a.relative_time - b.relative_time).unsigned_abs() > tolerance.as_nanos(),
            ),
            _ => (true, false, false),
        };
        if missing || identity || timing {
            differences.push(EventWindowDifference {
                index,
                missing,
                identity,
                timing,
            });
        }
    }
    Ok(EventWindowComparison {
        left: left.len(),
        right: right.len(),
        differences,
    })
}

struct Evidence<'a> {
    identity: (&'a str, String, &'a str, &'a str),
    relative_time: i128,
}

fn evidence<'a>(window: EventWindow<'a>) -> Result<Vec<Evidence<'a>>, EventWindowError> {
    let start = window.start.instant().unix_timestamp_nanos();
    let end = window.end.instant().unix_timestamp_nanos();
    if start >= end {
        return Err(EventWindowError::InvalidWindow);
    }
    if window.notifications.len() > MAX_EVENTS
        || window.notifications.iter().fold(0usize, |bytes, event| {
            bytes.saturating_add(event.retained_bytes())
        }) > MAX_BYTES
    {
        return Err(EventWindowError::Limit);
    }
    let mut prior = start;
    window
        .notifications
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let at: RecordingTimestamp = event
                .utc_time
                .parse()
                .map_err(|_| EventWindowError::InvalidTime(index))?;
            let at = at.instant().unix_timestamp_nanos();
            if at < prior || at >= end {
                return Err(EventWindowError::InvalidTime(index));
            }
            prior = at;
            let identity = topic(event).ok_or(EventWindowError::UnexaminedTopic(index))?;
            Ok(Evidence {
                identity,
                relative_time: at - start,
            })
        })
        .collect()
}

fn topic(event: &QualifiedNotification) -> Option<(&str, String, &str, &str)> {
    let topic = event.topic.as_ref()?;
    let dialect = topic.attr("", "Dialect")?;
    if !matches!(
        dialect,
        "http://docs.oasis-open.org/wsn/t-1/TopicExpression/Concrete"
            | "http://www.onvif.org/ver10/tev/topicExpression/ConcreteSet"
    ) {
        return None;
    }
    let path = topic.scalar().ok()?;
    let (prefix, names) = path.trim().split_once(':')?;
    if prefix.contains('/')
        || !valid_qname(prefix)
        || names
            .split('/')
            .any(|name| name.contains(':') || !valid_qname(name))
    {
        return None;
    }
    let ns = topic.namespace_for_prefix(prefix)?;
    Some((ns, names.to_owned(), dialect, &event.property_operation))
}
