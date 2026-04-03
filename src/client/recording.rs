// ── Recording Service ─────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{FindRecordingResults, RecordingItem, xml_escape};

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
