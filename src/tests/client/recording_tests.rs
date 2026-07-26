//! Unit tests for the Recording / Search / Replay methods on `OnvifClient`
//! (`src/client/recording.rs`).

use super::*;
use crate::tests::common::*;

// ── get_recordings ────────────────────────────────────────────────────────────

fn get_recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trc:GetRecordingsResponse>
              <trc:RecordingItem>
                <tt:RecordingToken>rec_001</tt:RecordingToken>
                <tt:Configuration>
                  <tt:Source>
                    <tt:SourceId>urn:uuid:source-1</tt:SourceId>
                    <tt:Name>Channel 1</tt:Name>
                    <tt:Location>Entrance</tt:Location>
                    <tt:Description>Front door camera</tt:Description>
                  </tt:Source>
                  <tt:Content>Motion event</tt:Content>
                  <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
                </tt:Configuration>
                <tt:Tracks>
                  <tt:Track>
                    <tt:TrackToken>VIDEO001</tt:TrackToken>
                    <tt:Configuration>
                      <tt:TrackType>Video</tt:TrackType>
                      <tt:Description>videoTrack</tt:Description>
                    </tt:Configuration>
                  </tt:Track>
                </tt:Tracks>
              </trc:RecordingItem>
            </trc:GetRecordingsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_recordings_parses_item() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_recordings_xml()));

    let recs = client
        .get_recordings("http://192.168.1.1/onvif/recording")
        .await
        .unwrap();

    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].token, "rec_001");
    assert_eq!(recs[0].source.name, "Channel 1");
    assert_eq!(recs[0].content, "Motion event");
    assert_eq!(recs[0].tracks.len(), 1);
    assert_eq!(recs[0].tracks[0].token, "VIDEO001");
    assert_eq!(recs[0].tracks[0].track_type, "Video");
}

#[tokio::test]
async fn test_get_recordings_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
          <s:Body>
            <trc:GetRecordingsResponse>
              <trc:RecordingItem>
                <!-- no RecordingToken — should trigger missing-field error -->
              </trc:RecordingItem>
            </trc:GetRecordingsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_recordings("http://192.168.1.1/onvif/recording")
        .await
        .unwrap_err();

    assert_missing_field(err, "RecordingItem/RecordingToken");
}

// ── Real-camera regression (GeoVision GV-GBLF4813, ONVIF v25.6) ───────────────
//
// Captured raw from the device. These shapes — singular `RecordingItem` with
// `tt:`-namespaced fields, `Tracks/Track/TrackToken/Configuration`, and a
// `ResultList`-wrapped search response — are the official ONVIF schema and
// previously parsed to empty under oxvif's mock-matching (but spec-wrong)
// parsers. Keep them verbatim so the parsers can never silently regress.

fn get_recordings_geovision_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trc:GetRecordingsResponse>
              <trc:RecordingItem>
                <tt:RecordingToken>tokenRecording1</tt:RecordingToken>
                <tt:Configuration>
                  <tt:Source>
                    <tt:SourceId>SourceId_1</tt:SourceId>
                    <tt:Name>IpCamera_1</tt:Name>
                    <tt:Location>Location</tt:Location>
                    <tt:Description>videoSource</tt:Description>
                    <tt:Address>http://www.onvif.org/ver10/schema/Profile</tt:Address>
                  </tt:Source>
                  <tt:Content>recordingContent</tt:Content>
                  <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
                </tt:Configuration>
                <tt:Tracks>
                  <tt:Track><tt:TrackToken>VIDEO001</tt:TrackToken><tt:Configuration><tt:TrackType>Video</tt:TrackType><tt:Description>videoTrack</tt:Description></tt:Configuration></tt:Track>
                  <tt:Track><tt:TrackToken>AUDIO001</tt:TrackToken><tt:Configuration><tt:TrackType>Audio</tt:TrackType><tt:Description>audioTrack</tt:Description></tt:Configuration></tt:Track>
                  <tt:Track><tt:TrackToken>META001</tt:TrackToken><tt:Configuration><tt:TrackType>Metadata</tt:TrackType><tt:Description>metaTrack</tt:Description></tt:Configuration></tt:Track>
                </tt:Tracks>
              </trc:RecordingItem>
            </trc:GetRecordingsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_recordings_geovision_real() {
    let client = OnvifClient::new("http://192.0.2.10/onvif/device_service")
        .with_transport(mock(get_recordings_geovision_xml()));

    let recs = client
        .get_recordings("http://192.0.2.10/onvif/Recording")
        .await
        .unwrap();

    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].token, "tokenRecording1");
    assert_eq!(recs[0].source.name, "IpCamera_1");
    assert_eq!(recs[0].tracks.len(), 3);
    assert_eq!(recs[0].tracks[0].token, "VIDEO001");
    assert_eq!(recs[0].tracks[0].track_type, "Video");
    assert_eq!(recs[0].tracks[1].token, "AUDIO001");
    assert_eq!(recs[0].tracks[2].track_type, "Metadata");
}

#[tokio::test]
async fn test_search_results_geovision_resultlist_queued() {
    // The camera nests SearchState inside <tse:ResultList>; oxvif previously
    // read it as a direct child and fell back to "Unknown", which made the
    // poll loop never complete.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tse="http://www.onvif.org/ver10/search/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tse:GetRecordingSearchResultsResponse>
              <tse:ResultList>
                <tt:SearchState>Queued</tt:SearchState>
              </tse:ResultList>
            </tse:GetRecordingSearchResultsResponse>
          </s:Body>
        </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.0.2.10/onvif/device_service").with_transport(mock(xml));

    let results = client
        .get_recording_search_results("http://192.0.2.10/onvif/SearchRecording", "t", 50, "PT5S")
        .await
        .unwrap();

    assert_eq!(results.search_state, "Queued");
}

// ── find_recordings / get_recording_search_results / end_search ───────────────

fn find_recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tse="http://www.onvif.org/ver10/search/wsdl">
          <s:Body>
            <tse:FindRecordingsResponse>
              <tse:SearchToken>search_abc123</tse:SearchToken>
            </tse:FindRecordingsResponse>
          </s:Body>
        </s:Envelope>"#
}

fn recording_search_results_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tse="http://www.onvif.org/ver10/search/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tse:GetRecordingSearchResultsResponse>
              <tse:ResultList>
                <tt:SearchState>Completed</tt:SearchState>
                <tt:RecordingInformation>
                  <tt:RecordingToken>rec_001</tt:RecordingToken>
                  <tt:Source>
                    <tt:Name>Channel 1</tt:Name>
                  </tt:Source>
                  <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
                  <tt:LatestRecording>2026-01-02T00:00:00Z</tt:LatestRecording>
                  <tt:Content>Motion event</tt:Content>
                  <tt:RecordingStatus>Stopped</tt:RecordingStatus>
                </tt:RecordingInformation>
              </tse:ResultList>
            </tse:GetRecordingSearchResultsResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_find_recordings_returns_token() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(find_recordings_xml()));

    let token = client
        .find_recordings("http://192.168.1.1/onvif/search", None, "PT60S")
        .await
        .unwrap();

    assert_eq!(token, "search_abc123");
}

#[tokio::test]
async fn test_find_recordings_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><tse:FindRecordingsResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .find_recordings("http://192.168.1.1/onvif/search", None, "PT60S")
        .await
        .unwrap_err();

    assert_missing_field(err, "SearchToken");
}

#[tokio::test]
async fn test_get_recording_search_results_parses_completed() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(recording_search_results_xml()));

    let results = client
        .get_recording_search_results(
            "http://192.168.1.1/onvif/search",
            "search_abc123",
            100,
            "PT5S",
        )
        .await
        .unwrap();

    assert_eq!(results.search_state, "Completed");
    assert_eq!(results.recording_information.len(), 1);
    assert_eq!(results.recording_information[0].recording_token, "rec_001");
    assert_eq!(results.recording_information[0].source_name, "Channel 1");
}

#[tokio::test]
async fn test_end_search_ok() {
    let xml = empty_response_xml("EndSearchResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .end_search("http://192.168.1.1/onvif/search", "search_abc123")
        .await
        .unwrap();

    assert!(captured.lock().unwrap().body.contains("search_abc123"));
}

// ── get_replay_uri ────────────────────────────────────────────────────────────

fn get_replay_uri_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:trp="http://www.onvif.org/ver10/replay/wsdl">
          <s:Body>
            <trp:GetReplayUriResponse>
              <trp:Uri>rtsp://192.168.1.1/replay/rec_001</trp:Uri>
            </trp:GetReplayUriResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_replay_uri_returns_rtsp() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_replay_uri_xml()));

    let uri = client
        .get_replay_uri(
            "http://192.168.1.1/onvif/replay",
            "rec_001",
            "RTP-Unicast",
            "RTSP",
        )
        .await
        .unwrap();

    assert_eq!(uri, "rtsp://192.168.1.1/replay/rec_001");
}

#[tokio::test]
async fn test_get_replay_uri_missing_uri_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                   <s:Body><trp:GetReplayUriResponse/></s:Body>
                 </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));

    let err = client
        .get_replay_uri(
            "http://192.168.1.1/onvif/replay",
            "rec_001",
            "RTP-Unicast",
            "RTSP",
        )
        .await
        .unwrap_err();

    assert_missing_field(err, "Uri");
}

#[tokio::test]
async fn test_get_recordings_parses_track_times_and_address() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingsResponse>
             <trc:RecordingItem>
               <tt:RecordingToken>Rec_001</tt:RecordingToken>
               <tt:Configuration>
                 <tt:Source>
                   <tt:SourceId>urn:uuid:camera-001</tt:SourceId>
                   <tt:Name>Camera 1</tt:Name>
                   <tt:Location>Entrance</tt:Location>
                   <tt:Description>Front door</tt:Description>
                   <tt:Address>rtsp://192.168.1.50/stream</tt:Address>
                 </tt:Source>
                 <tt:Content>Normal</tt:Content>
                 <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
               </tt:Configuration>
               <tt:Tracks>
                 <tt:Track>
                   <tt:TrackToken>Track_V1</tt:TrackToken>
                   <tt:Configuration>
                     <tt:TrackType>Video</tt:TrackType>
                     <tt:Description>Main video</tt:Description>
                   </tt:Configuration>
                   <tt:DataFrom>2024-01-01T00:00:00Z</tt:DataFrom>
                   <tt:DataTo>2024-01-02T00:00:00Z</tt:DataTo>
                 </tt:Track>
               </tt:Tracks>
             </trc:RecordingItem>
           </trc:GetRecordingsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let recs = client
        .get_recordings("http://192.168.1.1/onvif/recording_service")
        .await
        .unwrap();
    assert_eq!(
        recs[0].source.address.as_deref(),
        Some("rtsp://192.168.1.50/stream")
    );
    let track = &recs[0].tracks[0];
    assert_eq!(track.data_from.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(track.data_to.as_deref(), Some("2024-01-02T00:00:00Z"));
}

// ── Direction-1: Profile G recording write operations ─────────────────────────

#[tokio::test]
async fn test_create_recording_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingResponse>
             <trc:RecordingToken>Rec_007</trc:RecordingToken>
           </trc:CreateRecordingResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let token = client
        .create_recording(
            "http://192.168.1.1/onvif/recording_service",
            &crate::types::RecordingConfiguration {
                source_name: "Camera A".into(),
                source_id: "urn:uuid:cam-a".into(),
                location: "Entrance".into(),
                description: "Front door cam".into(),
                content: "Normal".into(),
                maximum_retention_time: "P30D".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(token, "Rec_007");
}

#[tokio::test]
async fn test_create_recording_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingResponse/>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client
        .create_recording(
            "http://192.168.1.1/onvif/recording_service",
            &crate::types::RecordingConfiguration {
                source_name: "Camera A".into(),
                source_id: "urn:uuid:cam-a".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert_missing_field(err, "RecordingToken");
}

#[tokio::test]
async fn test_create_track_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateTrackResponse>
             <trc:TrackToken>Track_V2</trc:TrackToken>
           </trc:CreateTrackResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let token = client
        .create_track(
            "http://192.168.1.1/onvif/recording_service",
            "Rec_001",
            "Video",
            "Main video track",
        )
        .await
        .unwrap();
    assert_eq!(token, "Track_V2");
}

#[tokio::test]
async fn test_create_track_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateTrackResponse/>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client
        .create_track(
            "http://192.168.1.1/onvif/recording_service",
            "Rec_001",
            "Video",
            "",
        )
        .await
        .unwrap_err();
    assert_missing_field(err, "TrackToken");
}

#[tokio::test]
async fn test_get_recording_jobs_parses_fields() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingJobsResponse>
             <trc:JobItem>
               <trc:JobToken>Job_001</trc:JobToken>
               <trc:JobConfiguration>
                 <tt:RecordingToken>Rec_001</tt:RecordingToken>
                 <tt:Mode>Active</tt:Mode>
                 <tt:Priority>2</tt:Priority>
                 <tt:Source>
                   <tt:SourceToken>
                     <tt:Token>Profile_1</tt:Token>
                   </tt:SourceToken>
                 </tt:Source>
               </trc:JobConfiguration>
             </trc:JobItem>
           </trc:GetRecordingJobsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let jobs = client
        .get_recording_jobs("http://192.168.1.1/onvif/recording_service")
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].token, "Job_001");
    assert_eq!(jobs[0].recording_token, "Rec_001");
    assert_eq!(jobs[0].mode, "Active");
    assert_eq!(jobs[0].priority, 2);
    assert_eq!(jobs[0].source_token, "Profile_1");
}

#[tokio::test]
async fn test_get_recording_jobs_missing_job_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:GetRecordingJobsResponse>
             <trc:JobItem/>
           </trc:GetRecordingJobsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client
        .get_recording_jobs("http://192.168.1.1/onvif/recording_service")
        .await
        .unwrap_err();
    assert_missing_field(err, "RecordingJob/JobToken");
}

#[tokio::test]
async fn test_create_recording_job_returns_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingJobResponse>
             <trc:JobToken>Job_new</trc:JobToken>
           </trc:CreateRecordingJobResponse>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let config = RecordingJobConfiguration {
        recording_token: "Rec_001".into(),
        mode: "Active".into(),
        priority: 1,
        source_token: "Profile_1".into(),
    };
    let token = client
        .create_recording_job("http://192.168.1.1/onvif/recording_service", &config)
        .await
        .unwrap();
    assert_eq!(token, "Job_new");
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Rec_001"));
    assert!(c.body.contains("Active"));
    assert!(c.body.contains("Profile_1"));
}

#[tokio::test]
async fn test_set_recording_job_mode_sends_correct_body() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:SetRecordingJobModeResponse/>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_recording_job_mode(
            "http://192.168.1.1/onvif/recording_service",
            "Job_001",
            "Idle",
        )
        .await
        .unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Job_001"));
    assert!(c.body.contains("Idle"));
}

#[tokio::test]
async fn test_get_recording_job_state_parses_active_state() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <trc:GetRecordingJobStateResponse>
             <trc:State>
               <tt:RecordingToken>Rec_001</tt:RecordingToken>
               <tt:State>Active</tt:State>
             </trc:State>
           </trc:GetRecordingJobStateResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let state = client
        .get_recording_job_state("http://192.168.1.1/onvif/recording_service", "Job_001")
        .await
        .unwrap();
    assert_eq!(state.recording_token, "Rec_001");
    assert_eq!(state.active_state, "Active");
}

#[tokio::test]
async fn test_get_recording_job_state_missing_state_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:GetRecordingJobStateResponse>
           </trc:GetRecordingJobStateResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client
        .get_recording_job_state("http://192.168.1.1/onvif/recording_service", "Job_001")
        .await
        .unwrap_err();
    assert_missing_field(err, "GetRecordingJobStateResponse/State");
}

#[tokio::test]
async fn test_create_recording_job_xml_escapes_token() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body>
           <trc:CreateRecordingJobResponse>
             <trc:JobToken>Job_safe</trc:JobToken>
           </trc:CreateRecordingJobResponse>
         </s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    let config = RecordingJobConfiguration {
        recording_token: "Rec<&>".into(),
        mode: "Active".into(),
        priority: 1,
        source_token: "Profile_1".into(),
    };
    client
        .create_recording_job("http://192.168.1.1/onvif/recording_service", &config)
        .await
        .unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("Rec&lt;&amp;&gt;"));
}

// ── delete_recording / delete_track / delete_recording_job ───────────────────

#[tokio::test]
async fn test_delete_recording_ok() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body><trc:DeleteRecordingResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .delete_recording("http://192.168.1.1/onvif/recording_service", "Rec_001")
        .await
        .unwrap();
    assert!(captured.lock().unwrap().body.contains("Rec_001"));
}

#[tokio::test]
async fn test_delete_recording_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Receiver", "NoSuchRecording-delete-8821");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .delete_recording("http://192.168.1.1/onvif/recording_service", "bad_token")
        .await
        .unwrap_err();
    assert_fault(err, "env:Receiver", "NoSuchRecording-delete-8821");
}

#[tokio::test]
async fn test_delete_track_ok() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body><trc:DeleteTrackResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .delete_track(
            "http://192.168.1.1/onvif/recording_service",
            "Rec_001",
            "Track_V1",
        )
        .await
        .unwrap();
    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Rec_001"));
    assert!(body.contains("Track_V1"));
}

#[tokio::test]
async fn test_delete_track_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Sender", "NoSuchTrack-delete-4417");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .delete_track("http://192.168.1.1/onvif/recording_service", "bad", "bad")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchTrack-delete-4417");
}

#[tokio::test]
async fn test_delete_recording_job_ok() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body><trc:DeleteRecordingJobResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .delete_recording_job("http://192.168.1.1/onvif/recording_service", "Job_001")
        .await
        .unwrap();
    assert!(captured.lock().unwrap().body.contains("Job_001"));
}

#[tokio::test]
async fn test_delete_recording_job_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Receiver", "NoSuchJob-delete-9052");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .delete_recording_job("http://192.168.1.1/onvif/recording_service", "bad")
        .await
        .unwrap_err();
    assert_fault(err, "env:Receiver", "NoSuchJob-delete-9052");
}

// ── Missing negative tests for existing methods ───────────────────────────────

#[tokio::test]
async fn test_create_recording_job_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:trc="http://www.onvif.org/ver10/recording/wsdl">
         <s:Body><trc:CreateRecordingJobResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let config = RecordingJobConfiguration {
        recording_token: "Rec_001".into(),
        mode: "Active".into(),
        priority: 1,
        source_token: "Profile_1".into(),
    };
    let err = client
        .create_recording_job("http://192.168.1.1/onvif/recording_service", &config)
        .await
        .unwrap_err();
    assert_missing_field(err, "JobToken");
}

#[tokio::test]
async fn test_set_recording_job_mode_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Sender", "InvalidJobMode-3160");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .set_recording_job_mode(
            "http://192.168.1.1/onvif/recording_service",
            "bad_job",
            "Active",
        )
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "InvalidJobMode-3160");
}

#[tokio::test]
async fn test_get_recording_search_results_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Receiver", "NoSuchSearchToken-results-7735");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .get_recording_search_results("http://192.168.1.1/onvif/search", "bad_token", 10, "PT5S")
        .await
        .unwrap_err();
    assert_fault(err, "env:Receiver", "NoSuchSearchToken-results-7735");
}

#[tokio::test]
async fn test_end_search_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Sender", "NoSuchSearchToken-end-2648");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .end_search("http://192.168.1.1/onvif/search", "bad_token")
        .await
        .unwrap_err();
    assert_fault(err, "env:Sender", "NoSuchSearchToken-end-2648");
}

// ── search_recordings ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_recordings_propagates_find_error() {
    // The wrapper's first call is `find_recordings`, so the fault it surfaces
    // must be that call's fault verbatim — not a repackaged one.
    let xml = make_soap_fault_xml("env:Receiver", "ActionNotSupported-find-5093");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let err = client
        .search_recordings("http://192.168.1.1/onvif/search", None)
        .await
        .unwrap_err();
    assert_fault(err, "env:Receiver", "ActionNotSupported-find-5093");
}

#[tokio::test]
async fn test_search_recordings_returns_empty_on_completed_no_results() {
    // Single XML served for every call. find_recordings parses SearchToken;
    // subsequent get_recording_search_results sees SearchState=Completed with
    // no RecordingInformation children; end_search silently ignores the mismatch.
    // The wrapper must return Ok(vec![]).
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tse="http://www.onvif.org/ver10/search/wsdl">
         <s:Body>
           <tse:FindRecordingsResponse>
             <tse:SearchToken>tok_xyz</tse:SearchToken>
           </tse:FindRecordingsResponse>
           <tse:GetRecordingSearchResultsResponse>
             <tse:SearchState>Completed</tse:SearchState>
           </tse:GetRecordingSearchResultsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let results = client
        .search_recordings("http://192.168.1.1/onvif/search", None)
        .await
        .unwrap();
    assert!(results.is_empty());
}
