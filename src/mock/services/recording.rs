//! Recording, Search and Replay — three ONVIF services over one `RecordingState`.
//!
//! Until 0.15 every one of these was a static fixture: `CreateRecording`
//! answered `Rec_new` and `GetRecordings` never listed it, `DeleteRecording` was
//! an unconditional empty success that removed nothing, and
//! `GetRecordingJobState` returned the same state for every job token. The same
//! shape as the reported Media2 `CreateProfile` bug, in a different service —
//! `docs/active/mock-audit-2026-07.md` §4.2.
//!
//! The consequence went past the mock. `HealthCheck::with_liveness_probes(true)`
//! runs a real chain — `get_recordings` → `search_recordings` → `get_replay_uri`
//! on the *first token the search returned* (`src/health/checks.rs`) — but
//! against canned fixtures none of those links were coupled: the token came from
//! a literal and the replay handler ignored it. The chain now has to hold
//! together, because `GetReplayUri` faults on a token that names no recording.

use crate::mock::helpers::{resp_empty, resp_soap_fault, soap};
use crate::mock::state::{RecordingEntry, RecordingJobEntry, RecordingTrackEntry, SharedState};
use crate::mock::xml_parse::extract_tag;

const TRC: &str = r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#;
const TSE: &str = r#"xmlns:tse="http://www.onvif.org/ver10/search/wsdl""#;
const TRP: &str = r#"xmlns:trp="http://www.onvif.org/ver10/replay/wsdl""#;

// ── Rendering ────────────────────────────────────────────────────────────────

fn render_recording(r: &RecordingEntry) -> String {
    let tracks: String = r
        .tracks
        .iter()
        .map(|t| {
            format!(
                "<tt:Track>\
                   <tt:TrackToken>{token}</tt:TrackToken>\
                   <tt:Configuration>\
                     <tt:TrackType>{ty}</tt:TrackType>\
                     <tt:Description>{desc}</tt:Description>\
                   </tt:Configuration>\
                 </tt:Track>",
                token = t.token,
                ty = t.track_type,
                desc = t.description,
            )
        })
        .collect();
    // `tt:GetRecordingsResponseItem/Tracks` is [1] and `tt:GetTracksResponseList`
    // declares `Track` as [0..*], so a recording holding nothing sends the
    // wrapper *empty* rather than omitting it. Until 0.15.0 the wrapper was
    // dropped whenever the list was — that is `Rec_002` and every freshly
    // created recording — which is a shape no conformant device produces.
    format!(
        r#"<trc:RecordingItem>
          <tt:RecordingToken>{token}</tt:RecordingToken>
          <tt:Configuration>
            {source}
            <tt:Content>{content}</tt:Content>
            <tt:MaximumRetentionTime>{retention}</tt:MaximumRetentionTime>
          </tt:Configuration>
          <tt:Tracks>{tracks}</tt:Tracks>
        </trc:RecordingItem>"#,
        token = r.token,
        source = render_source(r),
        content = r.content,
        retention = r.maximum_retention_time,
    )
}

/// `tt:RecordingSourceInformation`, rendered once for both getters.
///
/// All five members are `minOccurs=1` and go out in schema order — `SourceId`,
/// `Name`, `Location`, `Description`, `Address`. Two things this fixes:
///
/// - `GetRecordingSearchResults` sent `Name` alone, dropping four required
///   members that `RecordingState` was already holding. Rendering from state
///   rather than re-deciding per response is what keeps the two getters from
///   disagreeing about one recording.
/// - `Address` had no field at all, so `CreateRecording` read it out of the
///   request and discarded it while the client kept parsing it.
///
/// An entry with no address sends the element **empty** rather than omitting
/// it: the member is required, and `RecordingSourceInformation::address`
/// filters empty text to `None`, so "the device did not say" stays observable.
/// `Rec_001` carries one and `Rec_002` does not, which is what makes that
/// distinction assertable.
fn render_source(r: &RecordingEntry) -> String {
    format!(
        "<tt:Source>\
           <tt:SourceId>{source_id}</tt:SourceId>\
           <tt:Name>{name}</tt:Name>\
           <tt:Location>{location}</tt:Location>\
           <tt:Description>{description}</tt:Description>\
           <tt:Address>{address}</tt:Address>\
         </tt:Source>",
        source_id = r.source_id,
        name = r.source_name,
        location = r.location,
        description = r.description,
        address = r.address,
    )
}

/// `JobItem` is `trc:` — it is declared locally in `recording.wsdl`. Its two
/// children are **`tt:`**, because `JobItem` is typed
/// `tt:GetRecordingJobsResponseItem`, a complexType declared in `onvif.xsd`.
///
/// This runs the *opposite* way to the rest of the 0.15.0 namespace sweep,
/// where the fix was `tt:` → a service namespace. A uniform "push it into the
/// service namespace" pass would have broken these two rows while fixing the
/// other thirteen.
///
/// And the same two names go the other way one operation over:
/// `CreateRecordingJobResponse` declares its own `JobToken` and
/// `JobConfiguration` locally, so **those are `trc:`** and
/// `handle_create_recording_job` is right to emit `trc:JobToken`. Same names,
/// same service, different namespace — match on the declaration, never on the
/// name.
fn render_job(j: &RecordingJobEntry) -> String {
    format!(
        r#"<trc:JobItem>
          <tt:JobToken>{token}</tt:JobToken>
          <tt:JobConfiguration>
            <tt:RecordingToken>{rt}</tt:RecordingToken>
            <tt:Mode>{mode}</tt:Mode>
            <tt:Priority>{priority}</tt:Priority>
            <tt:Source>
              <tt:SourceToken>
                <tt:Token>{src}</tt:Token>
              </tt:SourceToken>
            </tt:Source>
          </tt:JobConfiguration>
        </trc:JobItem>"#,
        token = j.token,
        rt = j.recording_token,
        mode = j.mode,
        priority = j.priority,
        src = j.source_token,
    )
}

// ── Recording responses ──────────────────────────────────────────────────────

pub fn resp_recordings(state: &SharedState) -> String {
    let items: String = state
        .read()
        .recording
        .recordings
        .iter()
        .map(render_recording)
        .collect();
    soap(
        TRC,
        &format!("<trc:GetRecordingsResponse>{items}</trc:GetRecordingsResponse>"),
    )
}

pub fn handle_create_recording(state: &SharedState, body: &str) -> String {
    let cfg = extract_tag(body, "RecordingConfiguration").unwrap_or_default();
    let source = extract_tag(&cfg, "Source").unwrap_or_default();

    let token = state.modify_returning(|s| {
        let id = s.recording.next_recording_id;
        s.recording.next_recording_id += 1;
        let token = format!("Rec_{id:03}");
        s.recording.recordings.push(RecordingEntry {
            token: token.clone(),
            source_id: extract_tag(&source, "SourceId").unwrap_or_default(),
            source_name: extract_tag(&source, "Name").unwrap_or_default(),
            location: extract_tag(&source, "Location").unwrap_or_default(),
            description: extract_tag(&source, "Description").unwrap_or_default(),
            address: extract_tag(&source, "Address").unwrap_or_default(),
            content: extract_tag(&cfg, "Content").unwrap_or_default(),
            maximum_retention_time: extract_tag(&cfg, "MaximumRetentionTime")
                .unwrap_or_else(|| "PT0S".into()),
            // A recording exists before it holds anything. `CreateTrack` fills
            // it in, which is the pair `tests/mock_roundtrip.rs` asks about.
            tracks: Vec::new(),
            earliest: String::new(),
            latest: String::new(),
            status: "Initiated".into(),
        });
        eprintln!("    [STATE] recording created: {token}");
        token
    });

    soap(
        TRC,
        &format!(
            "<trc:CreateRecordingResponse>\
               <trc:RecordingToken>{token}</trc:RecordingToken>\
             </trc:CreateRecordingResponse>"
        ),
    )
}

pub fn handle_delete_recording(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "RecordingToken").unwrap_or_default();
    let removed = state.modify_returning(|s| {
        let before = s.recording.recordings.len();
        s.recording.recordings.retain(|r| r.token != token);
        // A recording's jobs go with it — a job pointing at nothing is not a
        // state a device would report.
        s.recording.jobs.retain(|j| j.recording_token != token);
        before != s.recording.recordings.len()
    });
    if !removed {
        return resp_soap_fault(
            "ter:NoRecording",
            &format!("NoSuchRecording-DELREC-5701: {token}"),
        );
    }
    eprintln!("    [STATE] recording deleted: {token}");
    resp_empty("trc", "DeleteRecordingResponse")
}

pub fn handle_create_track(state: &SharedState, body: &str) -> String {
    let recording = extract_tag(body, "RecordingToken").unwrap_or_default();
    let cfg = extract_tag(body, "TrackConfiguration").unwrap_or_default();
    let track_type = extract_tag(&cfg, "TrackType").unwrap_or_else(|| "Video".into());
    let description = extract_tag(&cfg, "Description").unwrap_or_default();

    let token = state.modify_returning(|s| {
        let id = s.recording.next_track_id;
        if !s.recording.recordings.iter().any(|r| r.token == recording) {
            return None;
        }
        s.recording.next_track_id += 1;
        let token = format!("TRACK{id:03}");
        if let Some(r) = s
            .recording
            .recordings
            .iter_mut()
            .find(|r| r.token == recording)
        {
            r.tracks.push(RecordingTrackEntry {
                token: token.clone(),
                track_type,
                description,
            });
        }
        eprintln!("    [STATE] track created on {recording}: {token}");
        Some(token)
    });

    match token {
        Some(token) => soap(
            TRC,
            &format!(
                "<trc:CreateTrackResponse>\
                   <trc:TrackToken>{token}</trc:TrackToken>\
                 </trc:CreateTrackResponse>"
            ),
        ),
        None => resp_soap_fault(
            "ter:NoRecording",
            &format!("NoSuchRecording-CREATETRACK-5702: {recording}"),
        ),
    }
}

pub fn handle_delete_track(state: &SharedState, body: &str) -> String {
    let recording = extract_tag(body, "RecordingToken").unwrap_or_default();
    let track = extract_tag(body, "TrackToken").unwrap_or_default();
    let removed = state.modify_returning(|s| {
        let Some(r) = s
            .recording
            .recordings
            .iter_mut()
            .find(|r| r.token == recording)
        else {
            return false;
        };
        let before = r.tracks.len();
        r.tracks.retain(|t| t.token != track);
        before != r.tracks.len()
    });
    if !removed {
        return resp_soap_fault(
            "ter:NoTrack",
            &format!("NoSuchTrack-DELTRACK-5703: {recording}/{track}"),
        );
    }
    eprintln!("    [STATE] track deleted from {recording}: {track}");
    resp_empty("trc", "DeleteTrackResponse")
}

pub fn resp_recording_jobs(state: &SharedState) -> String {
    let items: String = state.read().recording.jobs.iter().map(render_job).collect();
    soap(
        TRC,
        &format!("<trc:GetRecordingJobsResponse>{items}</trc:GetRecordingJobsResponse>"),
    )
}

pub fn handle_create_recording_job(state: &SharedState, body: &str) -> String {
    let cfg = extract_tag(body, "JobConfiguration").unwrap_or_default();
    let recording_token = extract_tag(&cfg, "RecordingToken").unwrap_or_default();
    let mode = extract_tag(&cfg, "Mode").unwrap_or_else(|| "Idle".into());
    let priority = extract_tag(&cfg, "Priority")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let source_token = extract_tag(&cfg, "Token").unwrap_or_default();

    let token = state.modify_returning(|s| {
        if !s
            .recording
            .recordings
            .iter()
            .any(|r| r.token == recording_token)
        {
            return None;
        }
        let id = s.recording.next_job_id;
        s.recording.next_job_id += 1;
        let token = format!("Job_{id:03}");
        s.recording.jobs.push(RecordingJobEntry {
            token: token.clone(),
            recording_token,
            mode,
            priority,
            source_token,
        });
        eprintln!("    [STATE] recording job created: {token}");
        Some(token)
    });

    match token {
        Some(token) => soap(
            TRC,
            &format!(
                "<trc:CreateRecordingJobResponse>\
                   <trc:JobToken>{token}</trc:JobToken>\
                 </trc:CreateRecordingJobResponse>"
            ),
        ),
        None => resp_soap_fault(
            "ter:NoRecording",
            "NoSuchRecording-CREATEJOB-5704: a job must name an existing recording",
        ),
    }
}

pub fn handle_set_recording_job_mode(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "JobToken").unwrap_or_default();
    let mode = extract_tag(body, "Mode").unwrap_or_default();
    if mode != "Active" && mode != "Idle" {
        return resp_soap_fault(
            "ter:InvalidArgVal",
            &format!("BadJobMode-SETJOBMODE-5705: {mode}"),
        );
    }
    let found = state.modify_returning(|s| {
        if let Some(j) = s.recording.jobs.iter_mut().find(|j| j.token == token) {
            j.mode = mode.clone();
            eprintln!("    [STATE] recording job {token} -> {mode}");
            true
        } else {
            false
        }
    });
    if !found {
        return resp_soap_fault("ter:NoJob", &format!("NoSuchJob-SETJOBMODE-5706: {token}"));
    }
    resp_empty("trc", "SetRecordingJobModeResponse")
}

pub fn handle_delete_recording_job(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "JobToken").unwrap_or_default();
    let removed = state.modify_returning(|s| {
        let before = s.recording.jobs.len();
        s.recording.jobs.retain(|j| j.token != token);
        before != s.recording.jobs.len()
    });
    if !removed {
        return resp_soap_fault("ter:NoJob", &format!("NoSuchJob-DELJOB-5707: {token}"));
    }
    eprintln!("    [STATE] recording job deleted: {token}");
    resp_empty("trc", "DeleteRecordingJobResponse")
}

/// Per **job token** — with one seeded job this was indistinguishable from a
/// constant, which is why the default state ships two in different modes.
///
/// `tt:RecordingJobState/State` is `Active` / `PartiallyActive` / `Idle`, and
/// the mock reports the job's own mode: it has no partially-started jobs.
pub fn resp_recording_job_state(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "JobToken").unwrap_or_default();
    let job = state
        .read()
        .recording
        .jobs
        .iter()
        .find(|j| j.token == token)
        .cloned();
    let Some(job) = job else {
        return resp_soap_fault("ter:NoJob", &format!("NoSuchJob-JOBSTATE-5708: {token}"));
    };
    soap(
        TRC,
        &format!(
            "<trc:GetRecordingJobStateResponse>\
               <trc:State>\
                 <tt:RecordingToken>{rt}</tt:RecordingToken>\
                 <tt:State>{st}</tt:State>\
               </trc:State>\
             </trc:GetRecordingJobStateResponse>",
            rt = job.recording_token,
            st = job.mode,
        ),
    )
}

// ── Search responses ─────────────────────────────────────────────────────────

pub fn resp_find_recordings() -> String {
    soap(
        TSE,
        r#"<tse:FindRecordingsResponse>
          <tse:SearchToken>search_mock_001</tse:SearchToken>
        </tse:FindRecordingsResponse>"#,
    )
}

/// The search returns whatever recordings exist now.
///
/// The mock keeps no per-search cursor: `FindRecordings` hands out one token and
/// this renders the whole list against it. That is a **declared** simplification
/// — a real device pages and expires searches — and it is stated here rather
/// than left to be inferred, per audit §6.
///
/// A recording created through `CreateRecording` has no time bounds yet, so its
/// `Earliest`/`Latest` are omitted rather than faked; both are optional in
/// `tt:RecordingInformation`.
pub fn resp_recording_search_results(state: &SharedState) -> String {
    let items: String = state
        .read()
        .recording
        .recordings
        .iter()
        .map(|r| {
            let bounds = if r.earliest.is_empty() {
                String::new()
            } else {
                format!(
                    "<tt:EarliestRecording>{}</tt:EarliestRecording>\
                     <tt:LatestRecording>{}</tt:LatestRecording>",
                    r.earliest, r.latest
                )
            };
            format!(
                r#"<tt:RecordingInformation>
                  <tt:RecordingToken>{token}</tt:RecordingToken>
                  {source}
                  {bounds}
                  <tt:Content>{content}</tt:Content>
                  <tt:RecordingStatus>{status}</tt:RecordingStatus>
                </tt:RecordingInformation>"#,
                token = r.token,
                source = render_source(r),
                content = r.content,
                status = r.status,
            )
        })
        .collect();
    soap(
        TSE,
        &format!(
            r#"<tse:GetRecordingSearchResultsResponse>
          <tse:ResultList>
            <tt:SearchState>Completed</tt:SearchState>
            {items}
          </tse:ResultList>
        </tse:GetRecordingSearchResultsResponse>"#
        ),
    )
}

/// `EndSearchResponse` is **not** an empty response.
///
/// `search.wsdl` declares one required child, `Endpoint`, an `xs:dateTime`
/// naming the point in time the search reached before it was released. The
/// mock answered with `resp_empty` until 0.15.0, which is a body no conformant
/// device sends — and nothing noticed, because `end_search` returns `()` and
/// only checks that the response element is present.
///
/// The clock is `soap::security::unix_secs_to_iso8601`, the same conversion
/// `GetSystemDateAndTime` and `PTZStatus/UtcTime` use, so the mock has one
/// clock rather than three.
pub fn resp_end_search() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let endpoint = crate::soap::security::unix_secs_to_iso8601(now as i64);
    soap(
        TSE,
        &format!(
            "<tse:EndSearchResponse><tse:Endpoint>{endpoint}</tse:Endpoint></tse:EndSearchResponse>"
        ),
    )
}

// ── Replay responses ─────────────────────────────────────────────────────────

/// Per **recording token**: the URI names the recording being replayed, so a
/// caller can tell which one it asked for. Unknown tokens fault — a replay URI
/// for a recording that does not exist is not something a device can hand out.
pub fn resp_replay_uri(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "RecordingToken").unwrap_or_default();
    if !state
        .read()
        .recording
        .recordings
        .iter()
        .any(|r| r.token == token)
    {
        return resp_soap_fault(
            "ter:NoRecording",
            &format!("NoSuchRecording-REPLAY-5709: {token}"),
        );
    }
    soap(
        TRP,
        &format!(
            "<trp:GetReplayUriResponse>\
               <trp:Uri>rtsp://127.0.0.1:554/mock/replay/{token}</trp:Uri>\
             </trp:GetReplayUriResponse>"
        ),
    )
}

// ── GetServiceCapabilities (recording / search / replay) ─────────────────────

/// `trc:Capabilities` — the widest of the nine, 21 attributes.
///
/// `OnboardStorage` is **deliberately omitted**. It is the only attribute
/// across all nine service-capability types carrying a schema `default`, and
/// that default is `true`, so a parser that maps absent to `false` disagrees
/// with the schema. Leaving it out here is what makes that disagreement
/// reachable from a test.
///
/// `MaxRecordings` is **`2.5`, not `2`** — deliberately fractional. It is
/// `xs:float` in the schema despite reading like a count, and an integral
/// fixture value passes just as well against a parser that typed it `u32`, so
/// the fraction is the only thing pinning the type.
pub fn resp_recording_service_capabilities() -> String {
    soap(
        TRC,
        r#"<trc:GetServiceCapabilitiesResponse>
          <trc:Capabilities DynamicRecordings="true"
                            DynamicTracks="true"
                            Encoding="H264 AAC"
                            MaxRate="4096"
                            MaxTotalRate="8192"
                            MaxRecordings="2.5"
                            MaxRecordingJobs="2"
                            Options="false"
                            MetadataRecording="false"
                            EventRecording="false"
                            ScheduledRecording="false"
                            SegmentExport="false"/>
        </trc:GetServiceCapabilitiesResponse>"#,
    )
}

/// `tse:Capabilities`. Four booleans, no children.
pub fn resp_search_service_capabilities() -> String {
    soap(
        TSE,
        r#"<tse:GetServiceCapabilitiesResponse>
          <tse:Capabilities MetadataSearch="false"
                            GeneralStartEvents="false"
                            NLSearch="false"
                            ImageSearch="false"/>
        </tse:GetServiceCapabilitiesResponse>"#,
    )
}

/// `trp:Capabilities`.
///
/// `SessionTimeoutRange` is a `tt:FloatList` — a whitespace-separated
/// min/max pair carried in the *attribute*, not a `Min`/`Max` child element.
pub fn resp_replay_service_capabilities() -> String {
    soap(
        TRP,
        r#"<trp:GetServiceCapabilitiesResponse>
          <trp:Capabilities ReversePlayback="false"
                            SessionTimeoutRange="1.0 600.0"
                            RTP_RTSP_TCP="true"/>
        </trp:GetServiceCapabilitiesResponse>"#,
    )
}
