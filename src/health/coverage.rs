//! Parse-coverage: compare what the parser extracted against what the device
//! actually returned, to catch silent **list-emptying** — the bug class behind
//! the Profile G recording and ImagingOptions fixes (a parser looking for the
//! wrong element name returns 0 items with no error).
//!
//! ## Scope / limits
//!
//! This catches list-emptying for the curated operations below (including
//! wrapper-nested lists, via an explicit item path). It does **not** catch
//! scalar field-defaulting — e.g. a single optional range silently parsed to
//! `None` — which is guarded by committed fixtures + the `conformance` example
//! instead. The item element names below are the **spec** names: for a
//! compliant device the raw count equals the parsed count (no warning); if a
//! parser later regresses to a wrong name, its parsed count drops while the raw
//! count holds, and this check warns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::report::{Category, CheckResult};
use crate::OnvifSession;
use crate::soap::{find_response, parse_soap_body};
use crate::transport::{Transport, TransportError};

/// In-memory transport tap recording the latest raw response per SOAP action
/// (keyed by the action's last URL segment, e.g. `GetProfiles`).
pub(super) struct CoverageTransport {
    inner: Arc<dyn Transport>,
    seen: Mutex<HashMap<String, String>>,
}

impl CoverageTransport {
    pub(super) fn new(inner: Arc<dyn Transport>) -> Self {
        Self {
            inner,
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn raw_for(&self, action_suffix: &str) -> Option<String> {
        self.seen.lock().unwrap().get(action_suffix).cloned()
    }
}

#[async_trait]
impl Transport for CoverageTransport {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let resp = self.inner.soap_post(url, action, body).await?;
        let suffix = action.rsplit('/').next().unwrap_or(action).to_string();
        self.seen.lock().unwrap().insert(suffix, resp.clone());
        Ok(resp)
    }
}

/// Count item elements in a captured `*Response`, navigating `item_path` from
/// the response node (the last segment is counted via `children_named`; any
/// earlier segments are followed via `child`, which handles wrappers like
/// `ResultList`). Returns `None` if the response/path is absent.
fn count_items(raw: &str, response_name: &str, item_path: &[&str]) -> Option<usize> {
    let body = parse_soap_body(raw).ok()?;
    let resp = find_response(&body, response_name).ok()?;
    let (last, mids) = item_path.split_last()?;
    let mut node = resp;
    for seg in mids {
        node = node.child(seg)?;
    }
    Some(node.children_named(last).count())
}

/// Parse-coverage check (see module docs). Re-runs a curated set of list
/// operations and warns when the device returned more items than the parser
/// extracted. Self-contained: each comparison uses the raw of its own call, so
/// it is race-free regardless of the other concurrent checks. The extra
/// read-only round-trips are the cost of keeping coverage isolated from the
/// functional checks.
pub(super) async fn coverage(s: &OnvifSession, tap: &CoverageTransport) -> Vec<CheckResult> {
    let mut out = Vec::new();

    let mut compare = |id: &'static str, op: &str, item_path: &[&str], parsed: usize| {
        let Some(raw) = tap.raw_for(op) else {
            return;
        };
        if let Some(m) = count_items(&raw, &format!("{op}Response"), item_path) {
            if m > parsed {
                out.push(CheckResult::warn(
                    id,
                    Category::Coverage,
                    format!("parsed {parsed} of {m} items — possible parser gap"),
                    format!("{parsed}/{m}"),
                ));
            }
        }
    };

    if let Ok(v) = s.get_profiles().await {
        compare("coverage_profiles", "GetProfiles", &["Profiles"], v.len());
    }
    if let Ok(v) = s.get_video_encoder_configurations().await {
        compare(
            "coverage_video_encoders",
            "GetVideoEncoderConfigurations",
            &["Configurations"],
            v.len(),
        );
    }
    if let Ok(v) = s.get_users().await {
        compare("coverage_users", "GetUsers", &["User"], v.len());
    }
    if let Ok(v) = s.get_network_interfaces().await {
        compare(
            "coverage_network_interfaces",
            "GetNetworkInterfaces",
            &["NetworkInterfaces"],
            v.len(),
        );
    }
    if let Ok(v) = s.ptz_get_nodes().await {
        compare("coverage_ptz_nodes", "GetNodes", &["PTZNode"], v.len());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::count_items;

    fn wrap(body: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                 xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                 xmlns:tse="http://www.onvif.org/ver10/search/wsdl"
                 xmlns:tt="http://www.onvif.org/ver10/schema">
               <s:Body>{body}</s:Body></s:Envelope>"#
        )
    }

    #[test]
    fn counts_flat_list() {
        let raw = wrap(
            "<trt:GetProfilesResponse>\
               <trt:Profiles token=\"a\"/><trt:Profiles token=\"b\"/>\
             </trt:GetProfilesResponse>",
        );
        assert_eq!(
            count_items(&raw, "GetProfilesResponse", &["Profiles"]),
            Some(2)
        );
    }

    #[test]
    fn counts_wrapper_nested_list() {
        let raw = wrap(
            "<tse:GetRecordingSearchResultsResponse><tse:ResultList>\
               <tt:SearchState>Completed</tt:SearchState>\
               <tt:RecordingInformation/><tt:RecordingInformation/>\
             </tse:ResultList></tse:GetRecordingSearchResultsResponse>",
        );
        assert_eq!(
            count_items(
                &raw,
                "GetRecordingSearchResultsResponse",
                &["ResultList", "RecordingInformation"]
            ),
            Some(2)
        );
    }

    #[test]
    fn empty_and_missing() {
        let empty = wrap("<trt:GetProfilesResponse/>");
        assert_eq!(
            count_items(&empty, "GetProfilesResponse", &["Profiles"]),
            Some(0)
        );
        // Wrong response name → None (never a false gap).
        assert_eq!(count_items(&empty, "GetUsersResponse", &["User"]), None);
    }
}
