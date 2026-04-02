use super::xml_str;
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── RecordingSourceInformation ────────────────────────────────────────────────

/// Identifies the physical source of a recording.
#[derive(Debug, Clone, Default)]
pub struct RecordingSourceInformation {
    /// Unique source identifier URI.
    pub source_id: String,
    /// Human-readable name for the source.
    pub name: String,
    /// Physical location description.
    pub location: String,
    /// Free-text description.
    pub description: String,
}

impl RecordingSourceInformation {
    fn from_xml(node: &XmlNode) -> Self {
        Self {
            source_id: xml_str(node, "SourceId").unwrap_or_default(),
            name: xml_str(node, "Name").unwrap_or_default(),
            location: xml_str(node, "Location").unwrap_or_default(),
            description: xml_str(node, "Description").unwrap_or_default(),
        }
    }
}

// ── RecordingTrack ────────────────────────────────────────────────────────────

/// A single track (video, audio, or metadata) within a recording.
#[derive(Debug, Clone)]
pub struct RecordingTrack {
    /// Opaque track token.
    pub token: String,
    /// Track type: `"Video"`, `"Audio"`, or `"Metadata"`.
    pub track_type: String,
    /// Free-text description.
    pub description: String,
}

// ── RecordingItem ─────────────────────────────────────────────────────────────

/// A recording entry returned by `get_recordings`.
#[derive(Debug, Clone)]
pub struct RecordingItem {
    /// Opaque recording token; pass to `get_replay_uri`.
    pub token: String,
    /// Source device or stream this recording originated from.
    pub source: RecordingSourceInformation,
    /// Free-text content description.
    pub content: String,
    /// ISO-8601 timestamp of the earliest recorded frame.
    pub earliest_recording: Option<String>,
    /// ISO-8601 timestamp of the latest recorded frame.
    pub latest_recording: Option<String>,
    /// Recording lifecycle state: `"Initiated"`, `"Recording"`, `"Stopped"`,
    /// `"Removing"`, or `"Removed"`.
    pub recording_status: String,
    /// Tracks contained in this recording.
    pub tracks: Vec<RecordingTrack>,
}

impl RecordingItem {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("RecordingItems")
            .map(|item| {
                let token = item
                    .attr("Token")
                    .or_else(|| item.attr("token"))
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("RecordingItem/@Token"))?
                    .to_string();

                let source = item
                    .child("RecordingInformation")
                    .and_then(|ri| ri.child("Source"))
                    .map(RecordingSourceInformation::from_xml)
                    .unwrap_or_default();

                let ri = item.child("RecordingInformation");

                let tracks = item
                    .child("Tracks")
                    .map(|tracks_node| {
                        tracks_node
                            .children_named("Track")
                            .map(|t| RecordingTrack {
                                token: t.attr("token").unwrap_or_default().to_string(),
                                track_type: xml_str(t, "TrackType").unwrap_or_default(),
                                description: xml_str(t, "Description").unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Self {
                    token,
                    source,
                    content: ri
                        .and_then(|r| r.child("Content"))
                        .map(|n| n.text().to_string())
                        .unwrap_or_default(),
                    earliest_recording: ri
                        .and_then(|r| r.child("EarliestRecording"))
                        .map(|n| n.text().to_string()),
                    latest_recording: ri
                        .and_then(|r| r.child("LatestRecording"))
                        .map(|n| n.text().to_string()),
                    recording_status: ri
                        .and_then(|r| r.child("RecordingStatus"))
                        .map(|n| n.text().to_string())
                        .unwrap_or_default(),
                    tracks,
                })
            })
            .collect()
    }
}

// ── RecordingInformation ──────────────────────────────────────────────────────

/// Summary of a recording returned by `get_recording_search_results`.
#[derive(Debug, Clone)]
pub struct RecordingInformation {
    /// Opaque recording token; pass to `get_replay_uri`.
    pub recording_token: String,
    /// Human-readable source name.
    pub source_name: String,
    /// ISO-8601 timestamp of the earliest recorded frame.
    pub earliest_recording: Option<String>,
    /// ISO-8601 timestamp of the latest recorded frame.
    pub latest_recording: Option<String>,
    /// Free-text content description.
    pub content: String,
    /// Recording status: `"Initiated"`, `"Recording"`, `"Stopped"`, etc.
    pub recording_status: String,
}

impl RecordingInformation {
    fn from_xml(node: &XmlNode) -> Self {
        let src = node.child("Source").unwrap_or(node);
        Self {
            recording_token: node
                .child("RecordingToken")
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            source_name: xml_str(src, "Name").unwrap_or_default(),
            earliest_recording: node
                .child("EarliestRecording")
                .map(|n| n.text().to_string()),
            latest_recording: node.child("LatestRecording").map(|n| n.text().to_string()),
            content: xml_str(node, "Content").unwrap_or_default(),
            recording_status: xml_str(node, "RecordingStatus").unwrap_or_default(),
        }
    }
}

// ── FindRecordingResults ──────────────────────────────────────────────────────

/// Results returned by `get_recording_search_results`.
#[derive(Debug, Clone)]
pub struct FindRecordingResults {
    /// Search state: `"Queued"`, `"Searching"`, `"Completed"`, or `"Unknown"`.
    pub search_state: String,
    /// Recording entries found so far.
    pub recording_information: Vec<RecordingInformation>,
}

impl FindRecordingResults {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        Ok(Self {
            search_state: resp
                .child("SearchState")
                .map(|n| n.text().to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            recording_information: resp
                .children_named("RecordingInformation")
                .map(RecordingInformation::from_xml)
                .collect(),
        })
    }
}
