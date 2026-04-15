use crate::helpers::soap;

// ── Recording responses ──────────────────────────────────────────────────────

pub fn resp_recordings() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:GetRecordingsResponse>
          <trc:RecordingItems>
            <trc:RecordingToken>Rec_001</trc:RecordingToken>
            <trc:Configuration>
              <tt:Source>
                <tt:SourceId>rtsp://mock/live</tt:SourceId>
                <tt:Name>MockCamera</tt:Name>
                <tt:Location>Lab</tt:Location>
                <tt:Description>Mock recording</tt:Description>
              </tt:Source>
              <tt:Content>Normal</tt:Content>
              <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
            </trc:Configuration>
          </trc:RecordingItems>
          <trc:RecordingItems>
            <trc:RecordingToken>Rec_002</trc:RecordingToken>
            <trc:Configuration>
              <tt:Source>
                <tt:Name>MockCamera</tt:Name>
              </tt:Source>
              <tt:Content></tt:Content>
              <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
            </trc:Configuration>
          </trc:RecordingItems>
        </trc:GetRecordingsResponse>"#,
    )
}

pub fn resp_create_recording() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:CreateRecordingResponse>
          <trc:RecordingToken>Rec_new</trc:RecordingToken>
        </trc:CreateRecordingResponse>"#,
    )
}

pub fn resp_create_track() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:CreateTrackResponse>
          <trc:TrackToken>Track_new</trc:TrackToken>
        </trc:CreateTrackResponse>"#,
    )
}

pub fn resp_recording_jobs() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:GetRecordingJobsResponse>
          <trc:JobItem>
            <trc:JobToken>Job_001</trc:JobToken>
            <trc:JobConfiguration>
              <tt:RecordingToken>Rec_001</tt:RecordingToken>
              <tt:Mode>Active</tt:Mode>
              <tt:Priority>1</tt:Priority>
              <tt:Source>
                <tt:SourceToken>
                  <tt:Token>Profile_1</tt:Token>
                </tt:SourceToken>
              </tt:Source>
            </trc:JobConfiguration>
          </trc:JobItem>
        </trc:GetRecordingJobsResponse>"#,
    )
}

pub fn resp_create_recording_job() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:CreateRecordingJobResponse>
          <trc:JobToken>Job_new</trc:JobToken>
        </trc:CreateRecordingJobResponse>"#,
    )
}

pub fn resp_recording_job_state() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:GetRecordingJobStateResponse>
          <trc:State>
            <tt:RecordingToken>Rec_001</tt:RecordingToken>
            <tt:State>Active</tt:State>
          </trc:State>
        </trc:GetRecordingJobStateResponse>"#,
    )
}

// ── Search responses ─────────────────────────────────────────────────────────

pub fn resp_find_recordings() -> String {
    soap(
        r#"xmlns:tse="http://www.onvif.org/ver10/search/wsdl""#,
        r#"<tse:FindRecordingsResponse>
          <tse:SearchToken>search_mock_001</tse:SearchToken>
        </tse:FindRecordingsResponse>"#,
    )
}

pub fn resp_recording_search_results() -> String {
    soap(
        r#"xmlns:tse="http://www.onvif.org/ver10/search/wsdl""#,
        r#"<tse:GetRecordingSearchResultsResponse>
          <tse:SearchState>Completed</tse:SearchState>
          <tse:RecordingInformation>
            <tt:RecordingToken>Rec_001</tt:RecordingToken>
            <tt:Source>
              <tt:Name>MockCamera</tt:Name>
            </tt:Source>
            <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
            <tt:LatestRecording>2026-04-01T00:00:00Z</tt:LatestRecording>
            <tt:Content>Motion event</tt:Content>
            <tt:RecordingStatus>Stopped</tt:RecordingStatus>
          </tse:RecordingInformation>
        </tse:GetRecordingSearchResultsResponse>"#,
    )
}

// ── Replay responses ─────────────────────────────────────────────────────────

pub fn resp_replay_uri() -> String {
    soap(
        r#"xmlns:trp="http://www.onvif.org/ver10/replay/wsdl""#,
        r#"<trp:GetReplayUriResponse>
          <trp:Uri>rtsp://127.0.0.1:554/mock/replay/Rec_001</trp:Uri>
        </trp:GetReplayUriResponse>"#,
    )
}
