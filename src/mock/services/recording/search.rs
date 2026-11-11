//! W14 bounded recording search snapshots and cursors.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::Instant;

use crate::{
    mock::{
        fault::{Code, Fault, INVALID_ARG_VAL, INVALID_TOKEN, MOCK_REQUEST_POLICY},
        helpers::soap,
        policy::{AckOnlyOperation, AckOnlyPolicy},
        request::{Node, RequestError},
        services::events::lifecycle::duration as whole_duration,
        state::{RecordingEntry, SharedState},
    },
    scoped_rpc::{SCHEMA, field},
};

const NS: &str = "http://www.onvif.org/ver10/search/wsdl";
const MAX_SESSIONS: usize = 32;
const MAX_ROWS: usize = 256;
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const MAX_LEASE: Duration = Duration::from_secs(3600);

pub(crate) struct SearchRuntime {
    instance: u64,
    next: u64,
    entries: BTreeMap<String, Search>,
}

impl Default for SearchRuntime {
    fn default() -> Self {
        Self {
            instance: rand::random(),
            next: 0,
            entries: BTreeMap::new(),
        }
    }
}

struct Search {
    rows: Vec<String>,
    bytes: usize,
    cursor: usize,
    lease: Duration,
    expires: Instant,
}

impl Search {
    fn state(&self) -> &'static str {
        if self.cursor == self.rows.len() {
            "Completed"
        } else {
            "Searching"
        }
    }
}

#[cfg(feature = "metamorph")]
pub(crate) fn is_lifecycle(action: &str) -> bool {
    action.rsplit_once('/').is_some_and(|(ns, name)| {
        ns == NS
            && matches!(
                name,
                "FindRecordings"
                    | "GetRecordingSearchResults"
                    | "FindEvents"
                    | "GetEventSearchResults"
                    | "FindPTZPosition"
                    | "GetPTZPositionSearchResults"
                    | "FindMetadata"
                    | "GetMetadataSearchResults"
                    | "GetSearchState"
                    | "EndSearch"
            )
    })
}

pub(crate) fn handle(
    op: &str,
    state: &SharedState,
    operation: &Node,
    policy: &AckOnlyPolicy,
) -> String {
    if op == "EndSearch" && policy.enabled(AckOnlyOperation::SearchEnd) {
        return super::resp_end_search();
    }
    let result = match op {
        "FindRecordings" => find(state, operation),
        "GetRecordingSearchResults" => poll(state, operation),
        "GetSearchState" => get_state(state, operation),
        "EndSearch" => end(state, operation),
        _ => unreachable!("fixed W14 operation"),
    };
    result.unwrap_or_else(|fault| fault)
}

fn find(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, NS, &["Scope", "MaxMatches", "KeepAliveTime"])?;
    let scope = operation
        .child(NS, "Scope")
        .map_err(RequestError::to_fault)?
        .ok_or_else(|| RequestError::MissingField.to_fault())?;
    let selection = recording_scope(scope)?;
    let limit = positive(operation, "MaxMatches")?.unwrap_or(MAX_ROWS + 1);
    let lease = optional(operation, "KeepAliveTime")?
        .and_then(duration)
        .filter(|lease| !lease.is_zero() && *lease <= MAX_LEASE)
        .ok_or_else(|| invalid("invalid recording search lease"))?;
    let mut rows = Vec::new();
    let mut bytes = 0;
    {
        let device = state.read();
        for record in device
            .recording
            .recordings
            .iter()
            .filter(|record| selection.is_empty() || selection.contains(&record.token))
            .take(limit)
        {
            if rows.len() == MAX_ROWS {
                return Err(invalid("recording search row limit"));
            }
            let row = render(record)?;
            bytes += row.len();
            if bytes > MAX_SNAPSHOT_BYTES {
                return Err(invalid("recording search snapshot limit"));
            }
            rows.push(row);
        }
    }
    let now = Instant::now();
    let mut searches = state.searches.lock().unwrap();
    searches.entries.retain(|_, item| item.expires > now);
    let used = searches
        .entries
        .values()
        .map(|item| item.bytes)
        .sum::<usize>();
    if searches.entries.len() == MAX_SESSIONS || used + bytes > MAX_TOTAL_BYTES {
        return Err(invalid("recording search capacity reached"));
    }
    let next = searches
        .next
        .checked_add(1)
        .ok_or_else(|| invalid("recording search token limit"))?;
    let token = format!("search_{:016x}_{next:016x}", searches.instance);
    searches.entries.insert(
        token.clone(),
        Search {
            rows,
            bytes,
            cursor: 0,
            lease,
            expires: now + lease,
        },
    );
    searches.next = next;
    Ok(response("FindRecordings", &field("SearchToken", &token)))
}

fn recording_scope(scope: &Node) -> Result<BTreeSet<String>, String> {
    shape(scope, SCHEMA, &["IncludedRecordings"])?;
    let mut tokens = BTreeSet::new();
    for (index, (ns, name, node)) in scope.element_children().enumerate() {
        if ns != SCHEMA
            || name != "IncludedRecordings"
            || index >= 128
            || node.expanded_attributes().next().is_some()
        {
            return Err(invalid("unsupported recording search scope"));
        }
        let token = node.scalar_text().map_err(RequestError::to_fault)?;
        identity(token)?;
        if !tokens.insert(token.to_owned()) {
            return Err(invalid("duplicate recording scope token"));
        }
    }
    Ok(tokens)
}

fn poll(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(
        operation,
        NS,
        &["SearchToken", "MinResults", "MaxResults", "WaitTime"],
    )?;
    let token = search_token(operation)?;
    let min = positive(operation, "MinResults")?.unwrap_or(0);
    let max = positive(operation, "MaxResults")?.unwrap_or(MAX_ROWS);
    if min > max
        || optional(operation, "WaitTime")?
            .is_some_and(|value| duration(value).is_none_or(|duration| duration > MAX_LEASE))
    {
        return Err(invalid("invalid recording search result bounds"));
    }
    let mut searches = state.searches.lock().unwrap();
    let now = Instant::now();
    let search = live(&mut searches, &token, now)?;
    let end = search.cursor.saturating_add(max).min(search.rows.len());
    let page = search.rows[search.cursor..end].concat();
    search.cursor = end;
    search.expires = now + search.lease;
    Ok(response(
        "GetRecordingSearchResults",
        &format!(
            "<ResultList><tt:SearchState>{}</tt:SearchState>{page}</ResultList>",
            search.state()
        ),
    ))
}

fn get_state(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, NS, &["SearchToken"])?;
    let token = search_token(operation)?;
    let mut searches = state.searches.lock().unwrap();
    let now = Instant::now();
    let search = live(&mut searches, &token, now)?;
    search.expires = now + search.lease;
    Ok(response("GetSearchState", &field("State", search.state())))
}

fn end(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, NS, &["SearchToken"])?;
    let token = search_token(operation)?;
    let mut searches = state.searches.lock().unwrap();
    live(&mut searches, &token, Instant::now())?;
    searches.entries.remove(&token);
    // The WSDL requires Endpoint. FindRecordings has no temporal endpoint.
    Ok(super::resp_end_search())
}

fn live<'a>(
    runtime: &'a mut SearchRuntime,
    token: &str,
    now: Instant,
) -> Result<&'a mut Search, String> {
    runtime.entries.retain(|_, search| search.expires > now);
    runtime.entries.get_mut(token).ok_or_else(|| {
        Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, INVALID_TOKEN],
            "Unknown or expired recording search token",
        )
        .to_xml()
    })
}

fn search_token(operation: &Node) -> Result<String, String> {
    let token =
        optional(operation, "SearchToken")?.ok_or_else(|| RequestError::MissingField.to_fault())?;
    identity(token)?;
    Ok(token.into())
}

fn positive(operation: &Node, name: &str) -> Result<Option<usize>, String> {
    optional(operation, name)?
        .map(|value| {
            value
                .trim()
                .parse::<i32>()
                .ok()
                .filter(|value| *value > 0)
                .map(|value| value as usize)
                .ok_or_else(|| invalid("invalid recording search count"))
        })
        .transpose()
}

fn shape(node: &Node, namespace: &str, fields: &[&str]) -> Result<(), String> {
    if node.expanded_attributes().next().is_some() {
        return Err(RequestError::InvalidAttribute.to_fault());
    }
    node.check_child_sequence(namespace, fields)
        .map_err(RequestError::to_fault)
}

fn optional<'a>(node: &'a Node, name: &str) -> Result<Option<&'a str>, String> {
    node.child(NS, name)
        .map_err(RequestError::to_fault)?
        .map(|node| {
            if node.expanded_attributes().next().is_some() {
                return Err(RequestError::InvalidAttribute.to_fault());
            }
            node.scalar_text().map_err(RequestError::to_fault)
        })
        .transpose()
}

fn identity(value: &str) -> Result<(), String> {
    crate::scoped_rpc::token(value).map_err(|_| invalid("invalid recording search token"))
}

fn duration(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Some(seconds) = whole_duration(value) {
        return Some(Duration::from_secs(seconds));
    }
    let (seconds, fraction) = value
        .strip_prefix("PT")?
        .strip_suffix('S')?
        .split_once('.')?;
    if seconds.is_empty()
        || fraction.is_empty()
        || fraction.len() > 9
        || !seconds
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(Duration::new(
        seconds.parse().ok()?,
        fraction.parse::<u32>().ok()? * 10u32.pow((9 - fraction.len()) as u32),
    ))
}

fn render(record: &RecordingEntry) -> Result<String, String> {
    let fields = [
        ("tt:SourceId", &record.source_id),
        ("tt:Name", &record.source_name),
        ("tt:Location", &record.location),
        ("tt:Description", &record.description),
        ("tt:Address", &record.address),
        ("tt:RecordingToken", &record.token),
        ("tt:Content", &record.content),
        ("tt:RecordingStatus", &record.status),
        ("tt:EarliestRecording", &record.earliest),
        ("tt:LatestRecording", &record.latest),
    ];
    if fields.iter().any(|(_, value)| {
        value.len() > 8192
            || value.chars().any(|ch| {
                ch.is_control() && !matches!(ch, '\r' | '\n' | '\t')
                    || matches!(ch, '\u{fffe}' | '\u{ffff}')
            })
    }) {
        return Err(invalid("invalid recording search snapshot value"));
    }
    let mut result = "<tt:RecordingInformation>".to_owned();
    result.push_str(&field("tt:RecordingToken", &record.token));
    result.push_str("<tt:Source>");
    for (name, value) in fields.iter().take(5) {
        result.push_str(&field(name, value));
    }
    result.push_str("</tt:Source>");
    for (name, value) in fields.iter().skip(8).filter(|(_, value)| !value.is_empty()) {
        result.push_str(&field(name, value));
    }
    result.push_str(&field("tt:Content", &record.content));
    result.push_str(&field("tt:RecordingStatus", &record.status));
    result.push_str("</tt:RecordingInformation>");
    Ok(result)
}

fn response(op: &str, fields: &str) -> String {
    soap(
        "",
        &format!("<{op}Response xmlns=\"{NS}\">{fields}</{op}Response>"),
    )
}

fn invalid(reason: &str) -> String {
    Fault::new(Code::Sender, &[MOCK_REQUEST_POLICY], reason).to_xml()
}
