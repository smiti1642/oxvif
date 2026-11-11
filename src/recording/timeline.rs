use std::{fmt, str::FromStr, sync::Arc};

use crate::{
    OnvifClient, OnvifError, OnvifSession,
    events::{QualifiedElement, QualifiedNotification, notification},
    scoped_rpc::{SCHEMA, ScopedService, boolean, field, required, scalar, text, token},
    soap::{
        SoapError,
        qualified::{EXTENSION_BYTES, invalid, valid_qname},
    },
    transport::EndpointPolicy,
    types::{RecordingInformation, xml_escape},
};

use super::{
    RecordingSearchCancellation, RecordingSearchCleanup, RecordingSearchOptions,
    RecordingSearchStopReason,
    search::{SearchOwner, SearchPage, SearchQuery, SearchReportData},
};

const SEARCH: &str = "http://www.onvif.org/ver10/search/wsdl";

/// A timezone-qualified XML date-time with an unambiguous instant.
#[derive(Clone, Debug)]
pub struct RecordingTimestamp {
    wire: String,
    instant: time::OffsetDateTime,
}

impl FromStr for RecordingTimestamp {
    type Err = OnvifError;

    fn from_str(wire: &str) -> Result<Self, Self::Err> {
        let invalid =
            || OnvifError::InvalidArgument("expected a timezone-qualified ONVIF date-time".into());
        if wire.len() > 40
            || wire.as_bytes().get(10) != Some(&b'T')
            || wire.as_bytes().get(17..19) == Some(b"60")
            || wire.ends_with('z')
        {
            return Err(invalid());
        }
        let instant =
            time::OffsetDateTime::parse(wire, &time::format_description::well_known::Rfc3339)
                .map_err(|_| invalid())?;
        if instant.offset().whole_seconds().unsigned_abs() > 14 * 3600 {
            return Err(invalid());
        }
        Ok(Self {
            wire: wire.into(),
            instant,
        })
    }
}

impl RecordingTimestamp {
    /// Validated original date-time spelling sent to the device.
    pub fn as_str(&self) -> &str {
        &self.wire
    }

    /// Comparable instant, regardless of the original timezone offset.
    pub fn instant(&self) -> time::OffsetDateTime {
        self.instant
    }
}

/// A source filter of the Search service, separate from a recording token.
#[derive(Clone, Debug)]
pub struct RecordingSource {
    /// Source token, typically a receiver or video source.
    pub token: String,
    /// Optional source-type URI. The ONVIF default is Receiver.
    pub kind: Option<String>,
}

/// Explicit subset of recordings and sources. Empty lists select all recordings.
#[derive(Clone, Debug, Default)]
pub struct RecordingScope {
    /// Include any of these recording tokens.
    pub recordings: Vec<String>,
    /// Include any of these sources.
    pub sources: Vec<RecordingSource>,
}

impl RecordingScope {
    pub(super) fn fields(&self) -> Result<String, OnvifError> {
        if self.recordings.len().saturating_add(self.sources.len()) > 128 {
            return Err(OnvifError::InvalidArgument(
                "recording scope exceeds 128 entries".into(),
            ));
        }
        let mut fields = String::new();
        for source in &self.sources {
            token(&source.token)?;
            let kind = if let Some(kind) = &source.kind {
                uri(kind)?;
                format!(" Type=\"{}\"", xml_escape(kind))
            } else {
                String::new()
            };
            fields.push_str(&format!(
                "<tt:IncludedSources{kind}>{}</tt:IncludedSources>",
                field("tt:Token", &source.token)
            ));
        }
        for recording in &self.recordings {
            token(recording)?;
            fields.push_str(&field("tt:IncludedRecordings", recording));
        }
        Ok(fields)
    }
}

fn uri(value: &str) -> Result<(), OnvifError> {
    if value.len() > 1024
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
        || reqwest::Url::parse(value).is_err()
    {
        return Err(OnvifError::InvalidArgument("invalid ONVIF URI".into()));
    }
    Ok(())
}

/// One concrete WS-Topics path in an explicitly bound topic namespace.
#[derive(Clone, Debug)]
pub struct EventTopic {
    namespace: String,
    path: String,
}

impl EventTopic {
    /// Construct a concrete path such as `RuleEngine/CellMotionDetector/Motion`.
    pub fn new(namespace: &str, path: &str) -> Result<Self, OnvifError> {
        uri(namespace)?;
        if path.len() > 1024
            || path
                .split('/')
                .any(|part| part.contains(':') || !valid_qname(part))
        {
            return Err(OnvifError::InvalidArgument(
                "invalid concrete event topic".into(),
            ));
        }
        Ok(Self {
            namespace: namespace.into(),
            path: path.into(),
        })
    }

    fn xml(&self) -> String {
        format!(
            "<wsnt:TopicExpression xmlns:wsnt=\"http://docs.oasis-open.org/wsn/b-2\" xmlns:topic=\"{}\" Dialect=\"http://docs.oasis-open.org/wsn/t-1/TopicExpression/Concrete\">topic:{}</wsnt:TopicExpression>",
            xml_escape(&self.namespace),
            xml_escape(&self.path)
        )
    }
}

/// Typed time, scope and event filters for FindEvents.
#[derive(Clone, Debug)]
pub struct EventSearchRequest {
    /// Start time.
    pub start: RecordingTimestamp,
    /// Optional end. An earlier instant requests a backward search.
    pub end: Option<RecordingTimestamp>,
    /// Recording and source filters.
    pub scope: RecordingScope,
    /// Optional concrete topic filter.
    pub topic: Option<EventTopic>,
    /// Include virtual events representing the start state.
    pub include_start_state: bool,
}

/// Device-wide extent and count for a recording timeline.
#[derive(Debug, Clone)]
pub struct RecordingSummary {
    /// Earliest device-reported data time.
    pub from: RecordingTimestamp,
    /// Latest device-reported data time.
    pub until: RecordingTimestamp,
    /// Number of recordings.
    pub count: u32,
}

/// Recording information with its qualified source and track fields retained.
#[derive(Debug, Clone)]
pub struct TimelineRecording {
    /// Common recording information.
    pub information: RecordingInformation,
    /// Original qualified XML, including source, tracks and extensions.
    pub xml: QualifiedElement,
}

/// Track attributes effective over a specific time span.
#[derive(Debug, Clone)]
pub struct RecordingMediaAttributes {
    /// Recording token.
    pub recording_token: String,
    /// Start of the effective span.
    pub from: RecordingTimestamp,
    /// End of the effective span.
    pub until: RecordingTimestamp,
    /// Qualified TrackAttributes in device order, with media-specific fields.
    pub tracks: Vec<QualifiedElement>,
}

/// A rich recorded event and the identity of its origin when available.
#[derive(Debug, Clone)]
pub struct RecordedEvent {
    /// Recording identity, if associated.
    pub recording_token: Option<String>,
    /// Track identity, if associated.
    pub track_token: Option<String>,
    /// Device-reported event time.
    pub time: RecordingTimestamp,
    /// Whether this represents a virtual start-state event.
    pub start_state: bool,
    /// Notification with qualified, ordered items.
    pub notification: QualifiedNotification,
}

/// Trusted Search service used for recording timeline queries.
pub struct RecordingTimeline {
    service: Arc<ScopedService>,
}

impl OnvifClient {
    /// Bind timeline reads to a Search endpoint on the configured origin.
    pub fn recording_timeline(&self, endpoint: &str) -> Result<RecordingTimeline, OnvifError> {
        let url = EndpointPolicy::default().check(self.device_url(), endpoint)?;
        Ok(RecordingTimeline::new(self, url))
    }
}

impl OnvifSession {
    /// Bind timeline reads to the session's resolved Search endpoint.
    pub fn recording_timeline(&self) -> Result<RecordingTimeline, OnvifError> {
        let client = self.client();
        let url = self
            .endpoint_policy
            .check(client.device_url(), self.search_url()?)?;
        Ok(RecordingTimeline::new(client, url))
    }
}

impl RecordingTimeline {
    fn new(client: &OnvifClient, url: reqwest::Url) -> Self {
        Self {
            service: Arc::new(ScopedService {
                client: client.direct(),
                url,
                namespace: SEARCH,
            }),
        }
    }

    /// Fetch the device's aggregate timeline extent.
    pub async fn summary(&self) -> Result<RecordingSummary, OnvifError> {
        let response = self
            .service
            .call("GetRecordingSummary", "GetRecordingSummaryResponse", "")
            .await?;
        let summary = required(&response, SEARCH, "Summary")?;
        let (from, until) = span(summary, "DataFrom", "DataUntil")?;
        Ok(RecordingSummary {
            from,
            until,
            count: scalar(summary, SCHEMA, "NumberRecordings")?,
        })
    }

    /// Read one recording's basic information, source and tracks.
    pub async fn information(&self, recording: &str) -> Result<TimelineRecording, OnvifError> {
        token(recording)?;
        let response = self
            .service
            .call(
                "GetRecordingInformation",
                "GetRecordingInformationResponse",
                &field("RecordingToken", recording),
            )
            .await?;
        let node = required(&response, SEARCH, "RecordingInformation")?;
        if node.serialized_len() > EXTENSION_BYTES {
            return Err(invalid("recording information exceeds retention cap").into());
        }
        required(node, SCHEMA, "Source")?;
        text(node, SCHEMA, "Content")?;
        text(node, SCHEMA, "RecordingStatus")?;
        let information =
            RecordingInformation::from_xml(&node.legacy_projection(&[SEARCH, SCHEMA]));
        if information.recording_token != recording {
            return Err(invalid("recording information did not match requested token").into());
        }
        Ok(TimelineRecording {
            information,
            xml: node.clone(),
        })
    }

    /// Read track attributes effective at a time, scoped to up to 128 recordings.
    pub async fn media_attributes(
        &self,
        recordings: &[String],
        at: &RecordingTimestamp,
    ) -> Result<Vec<RecordingMediaAttributes>, OnvifError> {
        if recordings.len() > 128 {
            return Err(OnvifError::InvalidArgument(
                "media attributes scope exceeds 128 recordings".into(),
            ));
        }
        let mut fields = String::new();
        for recording in recordings {
            token(recording)?;
            fields.push_str(&field("RecordingTokens", recording));
        }
        fields.push_str(&field("Time", at.as_str()));
        let response = self
            .service
            .call("GetMediaAttributes", "GetMediaAttributesResponse", &fields)
            .await?;
        if response.serialized_len() > EXTENSION_BYTES {
            return Err(invalid("media attributes exceed retention cap").into());
        }
        response
            .children()
            .map(|node| {
                if node.namespace() != SEARCH || node.local_name() != "MediaAttributes" {
                    return Err(invalid("unexpected media attributes entry").into());
                }
                let recording_token = text(node, SCHEMA, "RecordingToken")?;
                token(&recording_token)?;
                if !recordings.is_empty() && !recordings.contains(&recording_token) {
                    return Err(invalid("media attributes included an unrelated recording").into());
                }
                let (from, until) = span(node, "From", "Until")?;
                let tracks = node
                    .children()
                    .filter(|node| {
                        node.namespace() == SCHEMA && node.local_name() == "TrackAttributes"
                    })
                    .cloned()
                    .collect();
                Ok(RecordingMediaAttributes {
                    recording_token,
                    from,
                    until,
                    tracks,
                })
            })
            .collect()
    }

    /// Construct a single-use event search with the same budgets and cleanup as
    /// the recording search owner. Construction performs no network I/O.
    pub fn event_search(
        &self,
        request: EventSearchRequest,
        options: RecordingSearchOptions,
    ) -> Result<EventSearch, OnvifError> {
        let mut fields = field("StartPoint", request.start.as_str());
        if let Some(end) = request.end {
            fields.push_str(&field("EndPoint", end.as_str()));
        }
        fields.push_str(&format!("<Scope>{}</Scope><SearchFilter>{}</SearchFilter><IncludeStartState>{}</IncludeStartState>",
            request.scope.fields()?, request.topic.as_ref().map(EventTopic::xml).unwrap_or_default(), request.include_start_state));
        SearchOwner::new(
            EventQuery {
                service: self.service.clone(),
                fields,
            },
            options,
        )
        .map(EventSearch)
    }
}

fn time(node: &QualifiedElement, name: &'static str) -> Result<RecordingTimestamp, SoapError> {
    text(node, SCHEMA, name)?
        .trim()
        .parse()
        .map_err(|_| invalid("invalid recording date-time"))
}

fn span(
    node: &QualifiedElement,
    from: &'static str,
    until: &'static str,
) -> Result<(RecordingTimestamp, RecordingTimestamp), SoapError> {
    let from = time(node, from)?;
    let until = time(node, until)?;
    if from.instant() > until.instant() {
        return Err(invalid("reversed recording time range"));
    }
    Ok((from, until))
}

/// Single-use owner for a recorded event search.
pub struct EventSearch(SearchOwner<EventQuery>);

/// Result evidence from a bounded recorded event search.
pub struct EventSearchReport(SearchReportData<RecordedEvent>);

impl EventSearchReport {
    /// Results retained in device page order.
    pub fn events(&self) -> &[RecordedEvent] {
        &self.0.items
    }

    /// Number of parsed result pages.
    pub fn pages(&self) -> usize {
        self.0.pages
    }

    /// Estimated retained bytes charged to the search budget.
    pub fn retained_bytes(&self) -> usize {
        self.0.retained_bytes
    }

    /// Why the search returned.
    pub fn stop_reason(&self) -> RecordingSearchStopReason {
        self.0.stop_reason
    }

    /// Independent EndSearch outcome.
    pub fn cleanup(&self) -> &RecordingSearchCleanup {
        &self.0.cleanup
    }
}

impl fmt::Debug for EventSearchReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSearchReport")
            .field("events", &self.0.items.len())
            .field("pages", &self.0.pages)
            .field("reason", &self.0.stop_reason)
            .finish_non_exhaustive()
    }
}

/// A stopped or failed event search with partial results and cleanup evidence.
#[derive(Debug, thiserror::Error)]
#[error("recorded event search stopped: {reason}", reason = .report.stop_reason())]
pub struct EventSearchFailure {
    /// Partial report.
    pub report: EventSearchReport,
    /// Original Find, poll or parsing error, when applicable.
    #[source]
    pub cause: Option<OnvifError>,
}

impl EventSearch {
    /// Cooperative cancellation handle.
    pub fn cancellation(&self) -> RecordingSearchCancellation {
        self.0.cancellation()
    }

    /// Find and collect bounded event pages, then attempt EndSearch.
    pub async fn run(&mut self) -> Result<EventSearchReport, Box<EventSearchFailure>> {
        self.0
            .run()
            .await
            .map(EventSearchReport)
            .map_err(|(data, cause)| {
                Box::new(EventSearchFailure {
                    report: EventSearchReport(data),
                    cause,
                })
            })
    }

    /// Cancel and clean up a suspended or dropped run future.
    pub async fn cancel_and_close(&mut self) -> Arc<RecordingSearchCleanup> {
        self.0.cancel_and_close().await
    }
}

struct EventQuery {
    service: Arc<ScopedService>,
    fields: String,
}

impl SearchQuery for EventQuery {
    type Item = RecordedEvent;

    async fn find(&self, max_matches: Option<u32>, keep_alive: &str) -> Result<String, OnvifError> {
        let mut fields = self.fields.clone();
        if let Some(matches) = max_matches {
            fields.push_str(&field("MaxMatches", &matches.to_string()));
        }
        fields.push_str(&field("KeepAliveTime", keep_alive));
        let response = self
            .service
            .call("FindEvents", "FindEventsResponse", &fields)
            .await?;
        let search_token = text(&response, SEARCH, "SearchToken")?;
        token(&search_token)?;
        Ok(search_token)
    }

    async fn page(
        &self,
        search_token: &str,
        size: u32,
        wait: &str,
    ) -> Result<SearchPage<Self::Item>, OnvifError> {
        let fields = field("SearchToken", search_token)
            + &field("MaxResults", &size.to_string())
            + &field("WaitTime", wait);
        let response = self
            .service
            .call(
                "GetEventSearchResults",
                "GetEventSearchResultsResponse",
                &fields,
            )
            .await?;
        let list = required(&response, SEARCH, "ResultList")?;
        let search_state = text(list, SCHEMA, "SearchState")?;
        let mut items = Vec::new();
        for event in list
            .children()
            .filter(|node| node.namespace() == SCHEMA && node.local_name() == "Result")
        {
            if items.len() == size as usize {
                return Err(invalid("event search exceeded requested page size").into());
            }
            items.push(parse_event(event)?);
        }
        Ok(SearchPage {
            search_state,
            items,
        })
    }

    async fn end(&self, search_token: &str) -> Result<Option<String>, OnvifError> {
        let response = self
            .service
            .call(
                "EndSearch",
                "EndSearchResponse",
                &field("SearchToken", search_token),
            )
            .await?;
        response
            .one(SEARCH, "Endpoint")?
            .map(QualifiedElement::scalar)
            .transpose()
            .map_err(Into::into)
    }

    fn item_bytes(item: &RecordedEvent) -> usize {
        std::mem::size_of::<RecordedEvent>()
            + item.notification.retained_bytes()
            + item.recording_token.as_ref().map_or(0, String::len)
            + item.track_token.as_ref().map_or(0, String::len)
            + item.time.as_str().len()
    }
}

fn parse_event(node: &QualifiedElement) -> Result<RecordedEvent, OnvifError> {
    let identity = |name| {
        let value = text(node, SCHEMA, name)?;
        if value.is_empty() {
            return Ok::<_, OnvifError>(None);
        }
        token(&value)?;
        Ok(Some(value))
    };
    Ok(RecordedEvent {
        recording_token: identity("RecordingToken")?,
        track_token: identity("TrackToken")?,
        time: time(node, "Time")?,
        start_state: boolean(&text(node, SCHEMA, "StartStateEvent")?)?,
        notification: notification(required(node, SCHEMA, "Event")?)?,
    })
}
