// ── Recording Service ─────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    FindRecordingResults, RecordingConfiguration, RecordingInformation, RecordingItem,
    RecordingJob, RecordingJobConfiguration, RecordingJobState, xml_escape,
};

impl OnvifClient {
    /// List all recordings stored on the device.
    ///
    /// `recording_url` is obtained from `GetServices` — look for the service
    /// with namespace `http://www.onvif.org/ver10/recording/wsdl`.
    pub async fn get_recordings(
        &self,
        recording_url: &str,
    ) -> Result<Vec<RecordingItem>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/GetRecordings";
        const BODY: &str = "<trc:GetRecordings/>";

        let xml = self.call(recording_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetRecordingsResponse")?;
        RecordingItem::vec_from_xml(resp)
    }

    /// Create a new recording configuration on the device.
    ///
    /// Returns the opaque recording token assigned by the device.
    pub async fn create_recording(
        &self,
        recording_url: &str,
        config: &RecordingConfiguration,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/CreateRecording";
        let retention = if config.maximum_retention_time.is_empty() {
            std::borrow::Cow::Borrowed("PT0S")
        } else {
            xml_escape(&config.maximum_retention_time)
        };
        let body = format!(
            "<trc:CreateRecording>\
               <trc:RecordingConfiguration>\
                 <tt:Source>\
                   <tt:SourceId>{source_id}</tt:SourceId>\
                   <tt:Name>{source_name}</tt:Name>\
                   <tt:Location>{location}</tt:Location>\
                   <tt:Description>{description}</tt:Description>\
                 </tt:Source>\
                 <tt:Content>{content}</tt:Content>\
                 <tt:MaximumRetentionTime>{retention}</tt:MaximumRetentionTime>\
               </trc:RecordingConfiguration>\
             </trc:CreateRecording>",
            source_id = xml_escape(&config.source_id),
            source_name = xml_escape(&config.source_name),
            location = xml_escape(&config.location),
            description = xml_escape(&config.description),
            content = xml_escape(&config.content),
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateRecordingResponse")?;
        resp.child("RecordingToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("RecordingToken").into())
    }

    /// Delete a recording and all its tracks from the device.
    pub async fn delete_recording(
        &self,
        recording_url: &str,
        recording_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/DeleteRecording";
        let body = format!(
            "<trc:DeleteRecording>\
               <trc:RecordingToken>{}</trc:RecordingToken>\
             </trc:DeleteRecording>",
            xml_escape(recording_token)
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteRecordingResponse")?;
        Ok(())
    }

    /// Add a new track to an existing recording.
    ///
    /// Returns the track token assigned by the device.
    ///
    /// - `track_type` — `"Video"`, `"Audio"`, or `"Metadata"`.
    /// - `description` — free-text description of the track.
    pub async fn create_track(
        &self,
        recording_url: &str,
        recording_token: &str,
        track_type: &str,
        description: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/CreateTrack";
        let body = format!(
            "<trc:CreateTrack>\
               <trc:RecordingToken>{rt}</trc:RecordingToken>\
               <trc:TrackConfiguration>\
                 <tt:TrackType>{tt}</tt:TrackType>\
                 <tt:Description>{desc}</tt:Description>\
               </trc:TrackConfiguration>\
             </trc:CreateTrack>",
            rt = xml_escape(recording_token),
            tt = xml_escape(track_type),
            desc = xml_escape(description),
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateTrackResponse")?;
        resp.child("TrackToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("TrackToken").into())
    }

    /// Remove a track from a recording.
    pub async fn delete_track(
        &self,
        recording_url: &str,
        recording_token: &str,
        track_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/DeleteTrack";
        let body = format!(
            "<trc:DeleteTrack>\
               <trc:RecordingToken>{rt}</trc:RecordingToken>\
               <trc:TrackToken>{tt}</trc:TrackToken>\
             </trc:DeleteTrack>",
            rt = xml_escape(recording_token),
            tt = xml_escape(track_token),
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteTrackResponse")?;
        Ok(())
    }

    /// List all recording jobs on the device.
    pub async fn get_recording_jobs(
        &self,
        recording_url: &str,
    ) -> Result<Vec<RecordingJob>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobs";
        const BODY: &str = "<trc:GetRecordingJobs/>";
        let xml = self.call(recording_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetRecordingJobsResponse")?;
        RecordingJob::vec_from_xml(resp)
    }

    /// Create a new recording job that feeds a live stream into a recording.
    ///
    /// Returns the job token assigned by the device.
    pub async fn create_recording_job(
        &self,
        recording_url: &str,
        config: &RecordingJobConfiguration,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/CreateRecordingJob";
        let body = format!(
            "<trc:CreateRecordingJob>{}</trc:CreateRecordingJob>",
            config.to_xml_body()
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "CreateRecordingJobResponse")?;
        resp.child("JobToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("JobToken").into())
    }

    /// Enable or disable a recording job.
    ///
    /// `mode` must be `"Active"` or `"Idle"`.
    pub async fn set_recording_job_mode(
        &self,
        recording_url: &str,
        job_token: &str,
        mode: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/SetRecordingJobMode";
        let body = format!(
            "<trc:SetRecordingJobMode>\
               <trc:JobToken>{jt}</trc:JobToken>\
               <trc:Mode>{mode}</trc:Mode>\
             </trc:SetRecordingJobMode>",
            jt = xml_escape(job_token),
            mode = xml_escape(mode),
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetRecordingJobModeResponse")?;
        Ok(())
    }

    /// Delete a recording job from the device.
    pub async fn delete_recording_job(
        &self,
        recording_url: &str,
        job_token: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/DeleteRecordingJob";
        let body = format!(
            "<trc:DeleteRecordingJob>\
               <trc:JobToken>{}</trc:JobToken>\
             </trc:DeleteRecordingJob>",
            xml_escape(job_token)
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteRecordingJobResponse")?;
        Ok(())
    }

    /// Get the current operational state of a recording job.
    pub async fn get_recording_job_state(
        &self,
        recording_url: &str,
        job_token: &str,
    ) -> Result<RecordingJobState, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/recording/wsdl/GetRecordingJobState";
        let body = format!(
            "<trc:GetRecordingJobState>\
               <trc:JobToken>{}</trc:JobToken>\
             </trc:GetRecordingJobState>",
            xml_escape(job_token)
        );
        let xml = self.call(recording_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetRecordingJobStateResponse")?;
        RecordingJobState::from_xml(resp)
    }

    // ── Search Service ────────────────────────────────────────────────────────────

    /// Start an asynchronous search for recordings.
    ///
    /// Returns a search token string; pass it to
    /// [`get_recording_search_results`](Self::get_recording_search_results).
    ///
    /// - `max_matches` — upper bound on results (`None` = device default).
    /// - `keep_alive_timeout` — how long the search is kept open on the device
    ///   (ISO 8601 duration, e.g. `"PT60S"`).
    pub async fn find_recordings(
        &self,
        search_url: &str,
        max_matches: Option<u32>,
        keep_alive_timeout: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/search/wsdl/FindRecordings";
        let max_el = max_matches
            .map(|m| format!("<tse:MaxMatches>{m}</tse:MaxMatches>"))
            .unwrap_or_default();
        let keep_alive_timeout = xml_escape(keep_alive_timeout);
        let body = format!(
            "<tse:FindRecordings>\
               {max_el}\
               <tse:KeepAliveTime>{keep_alive_timeout}</tse:KeepAliveTime>\
             </tse:FindRecordings>"
        );

        let xml = self.call(search_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "FindRecordingsResponse")?;
        resp.child("SearchToken")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("SearchToken").into())
    }

    /// Retrieve search results for a recording search started by
    /// [`find_recordings`](Self::find_recordings).
    ///
    /// Call repeatedly until `results.search_state == "Completed"`.
    /// Then call [`end_search`](Self::end_search) to release server resources.
    pub async fn get_recording_search_results(
        &self,
        search_url: &str,
        search_token: &str,
        max_results: u32,
        wait_time: &str,
    ) -> Result<FindRecordingResults, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/search/wsdl/GetRecordingSearchResults";
        let search_token = xml_escape(search_token);
        let wait_time = xml_escape(wait_time);
        let body = format!(
            "<tse:GetRecordingSearchResults>\
               <tse:SearchToken>{search_token}</tse:SearchToken>\
               <tse:MaxResults>{max_results}</tse:MaxResults>\
               <tse:WaitTime>{wait_time}</tse:WaitTime>\
             </tse:GetRecordingSearchResults>"
        );

        let xml = self.call(search_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetRecordingSearchResultsResponse")?;
        FindRecordingResults::from_xml(resp)
    }

    /// Release a search session on the device.
    ///
    /// Always call this after you have finished reading results from
    /// [`get_recording_search_results`](Self::get_recording_search_results).
    pub async fn end_search(&self, search_url: &str, search_token: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/search/wsdl/EndSearch";
        let search_token = xml_escape(search_token);
        let body = format!(
            "<tse:EndSearch>\
               <tse:SearchToken>{search_token}</tse:SearchToken>\
             </tse:EndSearch>"
        );

        let xml = self.call(search_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "EndSearchResponse")?;
        Ok(())
    }

    /// Search all recordings and return results in a single call.
    ///
    /// Combines [`find_recordings`](Self::find_recordings),
    /// repeated [`get_recording_search_results`](Self::get_recording_search_results)
    /// polling, and [`end_search`](Self::end_search) into one convenient method.
    ///
    /// - `max_matches` — upper bound on results (`None` = device default).
    ///
    /// Returns a flat `Vec<RecordingInformation>` with all matched recordings.
    pub async fn search_recordings(
        &self,
        search_url: &str,
        max_matches: Option<u32>,
    ) -> Result<Vec<RecordingInformation>, OnvifError> {
        let token = self
            .find_recordings(search_url, max_matches, "PT60S")
            .await?;

        let mut all = Vec::new();
        // Cap polling iterations to prevent unbounded loops when a device
        // never reports `Completed`.  20 rounds × 100 results × PT5S wait
        // covers up to 2 000 recordings with a ~100 s worst-case wall time.
        for _ in 0..20 {
            let results = self
                .get_recording_search_results(search_url, &token, 100, "PT5S")
                .await?;
            all.extend(results.recording_information);
            if results.search_state == "Completed" {
                break;
            }
        }

        let _ = self.end_search(search_url, &token).await;
        Ok(all)
    }

    // ── Replay Service ────────────────────────────────────────────────────────────

    /// Retrieve an RTSP URI for replaying a stored recording.
    ///
    /// - `recording_token` — from [`get_recordings`](Self::get_recordings) or
    ///   [`get_recording_search_results`](Self::get_recording_search_results).
    /// - `stream_type` — `"RTP-Unicast"` or `"RTP-Multicast"`.
    /// - `protocol` — `"RTSP"` or `"RtspOverHttp"`.
    pub async fn get_replay_uri(
        &self,
        replay_url: &str,
        recording_token: &str,
        stream_type: &str,
        protocol: &str,
    ) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/replay/wsdl/GetReplayUri";
        let recording_token = xml_escape(recording_token);
        let stream_type = xml_escape(stream_type);
        let protocol = xml_escape(protocol);
        let body = format!(
            "<trp:GetReplayUri>\
               <trp:StreamSetup>\
                 <tt:Stream>{stream_type}</tt:Stream>\
                 <tt:Transport><tt:Protocol>{protocol}</tt:Protocol></tt:Transport>\
               </trp:StreamSetup>\
               <trp:RecordingToken>{recording_token}</trp:RecordingToken>\
             </trp:GetReplayUri>"
        );

        let xml = self.call(replay_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetReplayUriResponse")?;
        resp.child("Uri")
            .map(|n| n.text().to_string())
            .ok_or_else(|| crate::soap::SoapError::missing("Uri").into())
    }
}
