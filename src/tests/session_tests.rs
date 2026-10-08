use super::*;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::tests::common::ErrorTransport;
use crate::transport::TransportError;

struct ReportScript {
    responses: Mutex<std::collections::VecDeque<Result<String, TransportError>>>,
    requests: Mutex<Vec<(String, String)>>,
}

impl ReportScript {
    fn new(responses: Vec<Result<String, TransportError>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl Transport for ReportScript {
    async fn soap_post(
        &self,
        _: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        self.requests.lock().unwrap().push((action.into(), body));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unplanned initialization request")
    }
}

fn source(report: &SessionBuildReport, service: SessionService) -> ServiceUrlSource {
    report
        .services
        .iter()
        .find(|entry| entry.service == service)
        .unwrap()
        .source
}

#[tokio::test]
async fn report_build_preserves_legacy_traffic_and_records_skipped_stages() {
    let old = ReportScript::new(vec![Ok(caps_full_xml().into())]);
    let new = ReportScript::new(vec![Ok(caps_full_xml().into())]);
    OnvifSession::builder("http://configured")
        .with_transport(old.clone())
        .build()
        .await
        .unwrap();
    let (_, report) = OnvifSession::builder("http://configured")
        .with_transport(new.clone())
        .build_with_report()
        .await
        .unwrap();
    let actions = |script: &ReportScript| {
        script
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|(action, _)| action.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(actions(&old), actions(&new));
    assert_eq!(
        actions(&new),
        ["http://www.onvif.org/ver10/device/wsdl/GetCapabilities"]
    );
    assert_eq!(
        report
            .stages
            .iter()
            .map(|stage| stage.stage)
            .collect::<Vec<_>>(),
        [
            SessionStage::ClockRead,
            SessionStage::Capabilities,
            SessionStage::ServicesFallback
        ]
    );
    assert_eq!(
        report
            .stages
            .iter()
            .map(|stage| stage.outcome)
            .collect::<Vec<_>>(),
        [
            SessionStageOutcome::Skipped,
            SessionStageOutcome::Succeeded,
            SessionStageOutcome::Skipped
        ]
    );
    assert_eq!(report.stages[0].elapsed_ms, 0);
    assert!(report.stages[1].elapsed_ms >= 5);
    assert!(
        report
            .stages
            .iter()
            .all(|stage| stage.http_status.is_none() && stage.error_class.is_none())
    );
    assert_eq!(report.clock_offset_secs, None);
    assert_eq!(report.services.len(), 10);
    assert_eq!(
        source(&report, SessionService::Device),
        ServiceUrlSource::Configured
    );
    assert_eq!(
        source(&report, SessionService::Media2),
        ServiceUrlSource::Capabilities
    );
    assert!(
        report
            .services
            .iter()
            .filter(|entry| entry.service != SessionService::Device)
            .all(|entry| entry.source == ServiceUrlSource::Capabilities)
    );
}

#[tokio::test]
async fn report_clock_offsets_are_applied_before_capabilities_in_either_builder_order() {
    use crate::soap::{XmlNode, security::unix_secs_to_iso8601};
    for (delta, transport_first) in [(-3600, false), (3600, true)] {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let device_time = now + delta;
        let stamp = unix_secs_to_iso8601(device_time);
        let response = format!(
            "<Envelope><Body><GetSystemDateAndTimeResponse><SystemDateAndTime><UTCDateTime><Time><Hour>{}</Hour><Minute>{}</Minute><Second>{}</Second></Time><Date><Year>{}</Year><Month>{}</Month><Day>{}</Day></Date></UTCDateTime></SystemDateAndTime></GetSystemDateAndTimeResponse></Body></Envelope>",
            &stamp[11..13],
            &stamp[14..16],
            &stamp[17..19],
            &stamp[..4],
            &stamp[5..7],
            &stamp[8..10]
        );
        let script = ReportScript::new(vec![Ok(response), Ok(caps_full_xml().into())]);
        let builder = OnvifSession::builder("http://configured").with_clock_sync();
        let builder = if transport_first {
            builder
                .with_transport(script.clone())
                .with_credentials("report-user", "report-password")
        } else {
            builder
                .with_credentials("report-user", "report-password")
                .with_transport(script.clone())
        };
        let (_, report) = builder.build_with_report().await.unwrap();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let offset = report.clock_offset_secs.unwrap();
        assert!((device_time - after..=delta).contains(&offset));
        assert_eq!(offset.signum(), delta.signum());
        assert_eq!(report.stages[0].outcome, SessionStageOutcome::Succeeded);
        let requests = script.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].0,
            "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime"
        );
        assert_eq!(
            requests[1].0,
            "http://www.onvif.org/ver10/device/wsdl/GetCapabilities"
        );
        let envelope = XmlNode::parse_complete(&requests[1].1).unwrap();
        let created = envelope
            .path(&["Header", "Security", "UsernameToken", "Created"])
            .unwrap()
            .text();
        assert!(
            (device_time..=device_time + 2).any(|time| created == unix_secs_to_iso8601(time)),
            "clock offset did not reach WS-Security"
        );
        assert_eq!(
            envelope
                .path(&["Header", "Security", "UsernameToken", "Username"])
                .unwrap()
                .text(),
            "report-user"
        );
    }
}

#[tokio::test]
async fn mandatory_report_failure_retains_original_cause_and_stops_followup_calls() {
    for clock in [false, true] {
        let script = ReportScript::new(vec![
            Err(TransportError::HttpStatus {
                status: 403,
                body: "credential-shaped-private-body".into(),
            }),
            Ok(caps_full_xml().into()),
        ]);
        let builder = OnvifSession::builder("http://user:password@configured/?token=private")
            .with_transport(script.clone());
        let failure = (if clock {
            builder.with_clock_sync()
        } else {
            builder
        })
        .build_with_report()
        .await
        .err()
        .unwrap();
        assert_eq!(failure.to_string(), "Session initialization failed");
        assert!(
            matches!(failure.error, OnvifError::Transport(TransportError::HttpStatus { status: 403, ref body }) if body == "credential-shaped-private-body")
        );
        let failed_index = if clock { 0 } else { 1 };
        assert_eq!(
            failure.report.stages[failed_index].outcome,
            SessionStageOutcome::Failed
        );
        assert_eq!(
            failure.report.stages[failed_index].error_class,
            Some(SessionErrorClass::Authentication)
        );
        assert_eq!(failure.report.stages[failed_index].http_status, Some(403));
        assert!(
            failure.report.stages[failed_index + 1..]
                .iter()
                .all(|stage| stage.outcome == SessionStageOutcome::NotAttempted
                    && stage.elapsed_ms == 0)
        );
        assert_eq!(script.requests.lock().unwrap().len(), 1);
        assert_eq!(script.responses.lock().unwrap().len(), 1);
        let projection = format!("{:?}", failure.report);
        for secret in ["password", "private-body", "configured", "token="] {
            assert!(!projection.contains(secret));
        }
    }
}

#[tokio::test]
async fn fallback_report_keeps_capability_urls_and_distinguishes_missing_services() {
    let caps = caps_full_xml().replace(
        "<tt:Media2>    <tt:XAddr>http://cam/onvif/media2</tt:XAddr>    </tt:Media2>",
        "",
    );
    assert!(!caps.contains("/onvif/media2"));
    let services = "<Envelope><Body><GetServicesResponse><Service><Namespace>http://www.onvif.org/ver10/recording/wsdl</Namespace><XAddr>http://different/recording</XAddr></Service><Service><Namespace>http://www.onvif.org/ver20/media/wsdl</Namespace><XAddr>http://user:pass@fallback/?query=secret</XAddr></Service></GetServicesResponse></Body></Envelope>";
    for fallback in [
        Ok(services.into()),
        Ok("<Envelope><Body><GetServicesResponse/></Body></Envelope>".into()),
        Err(TransportError::HttpStatus {
            status: 503,
            body: "private-fallback-error".into(),
        }),
    ] {
        let succeeded = fallback.is_ok();
        let filled = fallback
            .as_ref()
            .is_ok_and(|xml: &String| xml.contains("<Service>"));
        let script = ReportScript::new(vec![Ok(caps.clone()), fallback]);
        let (session, report) = OnvifSession::builder("http://configured")
            .with_transport(script.clone())
            .build_with_report()
            .await
            .unwrap();
        assert_eq!(
            session.capabilities().recording.url.as_deref(),
            Some("http://cam/onvif/recording")
        );
        assert_eq!(
            source(&report, SessionService::Recording),
            ServiceUrlSource::Capabilities
        );
        assert_eq!(
            source(&report, SessionService::Media2),
            if filled {
                ServiceUrlSource::ServicesFallback
            } else {
                ServiceUrlSource::Missing
            }
        );
        assert_eq!(
            session.capabilities().media2.url.as_deref(),
            if filled {
                Some("http://user:pass@fallback/?query=secret")
            } else {
                None
            }
        );
        assert_eq!(
            report.stages[2].outcome,
            if succeeded {
                SessionStageOutcome::Succeeded
            } else {
                SessionStageOutcome::Failed
            }
        );
        assert_eq!(
            report.stages[2].http_status,
            if succeeded { None } else { Some(503) }
        );
        assert_eq!(
            report.stages[2].error_class,
            if succeeded {
                None
            } else {
                Some(SessionErrorClass::Transport)
            }
        );
        assert_eq!(script.requests.lock().unwrap().len(), 2);
        for secret in ["user:pass", "query=", "fallback-error"] {
            assert!(!format!("{report:?}").contains(secret));
        }
        #[cfg(feature = "serde")]
        {
            let encoded = serde_json::to_string(&report).unwrap();
            assert_eq!(
                serde_json::from_str::<SessionBuildReport>(&encoded).unwrap(),
                report
            );
            for secret in ["user:pass", "query=", "fallback-error"] {
                assert!(!encoded.contains(secret));
            }
        }
    }
}

#[tokio::test]
async fn successful_clock_without_utc_reports_unknown_offset() {
    let script = ReportScript::new(vec![Ok("<Envelope><Body><GetSystemDateAndTimeResponse><SystemDateAndTime/></GetSystemDateAndTimeResponse></Body></Envelope>".into()), Ok(caps_full_xml().into())]);
    let (_, report) = OnvifSession::builder("http://configured")
        .with_clock_sync()
        .with_transport(script.clone())
        .build_with_report()
        .await
        .unwrap();
    assert_eq!(report.stages[0].outcome, SessionStageOutcome::Succeeded);
    assert_eq!(report.clock_offset_secs, None);
    assert_eq!(script.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn fault_report_classifies_device_refusal_without_echoing_reason_or_http_success_status() {
    let fault = "<Envelope><Body><Fault><Code><Value>s:Sender</Value></Code><Reason><Text>password=report-secret http://user:pass@host/?query=secret</Text></Reason></Fault></Body></Envelope>";
    let script = ReportScript::new(vec![Ok(fault.into())]);
    let failure = OnvifSession::builder("http://configured")
        .with_transport(script)
        .build_with_report()
        .await
        .err()
        .unwrap();
    assert_eq!(
        failure.report.stages[1].error_class,
        Some(SessionErrorClass::DeviceFault)
    );
    assert_eq!(failure.report.stages[1].http_status, None);
    assert_eq!(
        failure.report.stages[1].outcome,
        SessionStageOutcome::Failed
    );
    assert!(
        matches!(failure.error, OnvifError::Soap(SoapError::Fault { ref reason, .. }) if reason == "password=report-secret http://user:pass@host/?query=secret")
    );
    for secret in ["report-secret", "user:pass", "query="] {
        assert!(!format!("{:?}", failure.report).contains(secret));
    }
}

#[tokio::test]
async fn media_sync_session_routes_services_and_reports_missing_urls() {
    for media2 in [false, true] {
        let response = crate::tests::common::empty_response_xml("SetSynchronizationPointResponse");
        let (transport, captured) = crate::tests::common::RecordingTransport::new(&response);
        let mut caps = Capabilities::default();
        caps.media.url = Some("http://media1.invalid/service".into());
        caps.media2.url = Some("http://media2.invalid/service".into());
        let session = OnvifSession {
            client: OnvifClient::new("http://device.invalid").with_transport(transport),
            caps,
        };
        if media2 {
            session
                .set_synchronization_point_media2("target")
                .await
                .unwrap();
        } else {
            session
                .media_set_synchronization_point("target")
                .await
                .unwrap();
        }
        {
            let call = captured.lock().unwrap();
            assert_eq!(
                call.url,
                if media2 {
                    "http://media2.invalid/service"
                } else {
                    "http://media1.invalid/service"
                }
            );
            assert_eq!(
                call.action,
                format!(
                    "http://www.onvif.org/{}/media/wsdl/SetSynchronizationPoint",
                    if media2 { "ver20" } else { "ver10" }
                )
            );
            assert!(call.body.contains(">target</"));
        }
        let session = OnvifSession {
            caps: Capabilities::default(),
            ..session
        };
        let error = if media2 {
            session
                .set_synchronization_point_media2("target")
                .await
                .unwrap_err()
        } else {
            session
                .media_set_synchronization_point("target")
                .await
                .unwrap_err()
        };
        crate::tests::common::assert_missing_field(
            error,
            if media2 {
                "Media2 service URL"
            } else {
                "Media service URL"
            },
        );
    }
}

// ── SequenceTransport: returns responses in order ─────────────────────────

struct SequenceTransport {
    responses: Mutex<Vec<String>>,
}

impl SequenceTransport {
    fn new(responses: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.iter().map(|s| s.to_string()).rev().collect()),
        })
    }
}

#[async_trait]
impl Transport for SequenceTransport {
    async fn soap_post(
        &self,
        _url: &str,
        _action: &str,
        _body: String,
    ) -> Result<String, TransportError> {
        let mut stack = self.responses.lock().unwrap();
        stack.pop().map(Ok).unwrap_or_else(|| {
            Err(TransportError::HttpStatus {
                status: 503,
                body: "no more responses".into(),
            })
        })
    }
}

// ── ErrorTransport: always fails ─────────────────────────────────────────
//
// Shared with the client tests — see `src/tests/common.rs`.

// ── XML fixtures ──────────────────────────────────────────────────────────

/// Full capabilities response including all service URLs (Media, PTZ, Imaging,
/// Events, DeviceIO, Recording, Search, Replay, Media2).
///
/// **It must stay complete**, because `session_with` relies on it: the builder
/// falls back to `GetServices` when any of those URLs is missing, which
/// consumes a second scripted response and every delegate test then runs out
/// one short. That is exactly what happened when `DeviceIO` joined the
/// fallback set and this fixture had not.
fn caps_full_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device>   <tt:XAddr>http://cam/onvif/device</tt:XAddr>    </tt:Device>
            <tt:Media>    <tt:XAddr>http://cam/onvif/media</tt:XAddr>     </tt:Media>
            <tt:PTZ>      <tt:XAddr>http://cam/onvif/ptz</tt:XAddr>       </tt:PTZ>
            <tt:Imaging>  <tt:XAddr>http://cam/onvif/imaging</tt:XAddr>   </tt:Imaging>
            <tt:Events>   <tt:XAddr>http://cam/onvif/events</tt:XAddr>    </tt:Events>
            <tt:Extension>
              <tt:DeviceIO>  <tt:XAddr>http://cam/onvif/deviceio</tt:XAddr>  </tt:DeviceIO>
              <tt:Recording> <tt:XAddr>http://cam/onvif/recording</tt:XAddr> </tt:Recording>
              <tt:Search>    <tt:XAddr>http://cam/onvif/search</tt:XAddr>    </tt:Search>
              <tt:Replay>    <tt:XAddr>http://cam/onvif/replay</tt:XAddr>    </tt:Replay>
              <tt:Media2>    <tt:XAddr>http://cam/onvif/media2</tt:XAddr>    </tt:Media2>
            </tt:Extension>
          </tds:Capabilities>
        </tds:GetCapabilitiesResponse>
      </s:Body>
    </s:Envelope>"#
}

/// Minimal capabilities response — only a Device URL, no service URLs.
fn caps_device_only_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device><tt:XAddr>http://cam/onvif/device</tt:XAddr></tt:Device>
          </tds:Capabilities>
        </tds:GetCapabilitiesResponse>
      </s:Body>
    </s:Envelope>"#
}

fn system_date_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <tds:GetSystemDateAndTimeResponse>
          <tds:SystemDateAndTime>
            <tt:DateTimeType>NTP</tt:DateTimeType>
            <tt:DaylightSavings>false</tt:DaylightSavings>
            <tt:TimeZone><tt:TZ>UTC</tt:TZ></tt:TimeZone>
            <tt:UTCDateTime>
              <tt:Time><tt:Hour>12</tt:Hour><tt:Minute>0</tt:Minute><tt:Second>0</tt:Second></tt:Time>
              <tt:Date><tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>2</tt:Day></tt:Date>
            </tt:UTCDateTime>
          </tds:SystemDateAndTime>
        </tds:GetSystemDateAndTimeResponse>
      </s:Body>
    </s:Envelope>"#
}

fn profiles_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <trt:GetProfilesResponse>
          <trt:Profiles token="Profile_1" fixed="true">
            <tt:Name>mainStream</tt:Name>
          </trt:Profiles>
        </trt:GetProfilesResponse>
      </s:Body>
    </s:Envelope>"#
}

fn stream_uri_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <trt:GetStreamUriResponse>
          <trt:MediaUri>
            <tt:Uri>rtsp://cam:554/stream1</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>
      </s:Body>
    </s:Envelope>"#
}

fn ptz_stop_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
      <s:Body><tptz:StopResponse/></s:Body>
    </s:Envelope>"#
}

fn imaging_settings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:Brightness>50</tt:Brightness>
            <tt:Contrast>50</tt:Contrast>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>
      </s:Body>
    </s:Envelope>"#
}

fn recordings_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:trc="http://www.onvif.org/ver10/recording/wsdl"
                  xmlns:tt="http://www.onvif.org/ver10/schema">
      <s:Body>
        <trc:GetRecordingsResponse>
          <trc:RecordingItem>
            <tt:RecordingToken>rec1</tt:RecordingToken>
            <tt:Configuration>
              <tt:Source><tt:Name>Camera 1</tt:Name></tt:Source>
              <tt:Content>Normal</tt:Content>
              <tt:MaximumRetentionTime>PT0S</tt:MaximumRetentionTime>
            </tt:Configuration>
          </trc:RecordingItem>
        </trc:GetRecordingsResponse>
      </s:Body>
    </s:Envelope>"#
}

fn subscribe_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:wsnt="http://docs.oasis-open.org/wsn/b-2"
                  xmlns:wsa="http://www.w3.org/2005/08/addressing">
      <s:Body>
        <wsnt:SubscribeResponse>
          <wsnt:SubscriptionReference>
            <wsa:Address>http://cam/onvif/events/push_sub_1</wsa:Address>
          </wsnt:SubscriptionReference>
          <wsnt:CurrentTime>2026-04-02T12:00:00Z</wsnt:CurrentTime>
          <wsnt:TerminationTime>2026-04-02T12:01:00Z</wsnt:TerminationTime>
        </wsnt:SubscribeResponse>
      </s:Body>
    </s:Envelope>"#
}

fn soap_fault_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
      <s:Body>
        <s:Fault>
          <s:Code><s:Value>s:Sender</s:Value></s:Code>
          <s:Reason><s:Text xml:lang="en">Not Authorized</s:Text></s:Reason>
        </s:Fault>
      </s:Body>
    </s:Envelope>"#
}

// ── Helper: build a session from a sequence of SOAP responses ─────────────
// The first response is always for GetCapabilities (caps_full_xml).

async fn session_with(method_responses: &[&str]) -> OnvifSession {
    let mut responses = vec![caps_full_xml()];
    responses.extend_from_slice(method_responses);
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(SequenceTransport::new(&responses))
        .build()
        .await
        .expect("session build failed")
}

// ── Builder tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_builder_stores_capabilities() {
    let session = session_with(&[]).await;
    let caps = session.capabilities();
    assert_eq!(caps.media.url.as_deref(), Some("http://cam/onvif/media"));
    assert_eq!(caps.ptz.url.as_deref(), Some("http://cam/onvif/ptz"));
    assert_eq!(
        caps.imaging.url.as_deref(),
        Some("http://cam/onvif/imaging")
    );
    assert_eq!(caps.events.url.as_deref(), Some("http://cam/onvif/events"));
    assert_eq!(
        caps.recording.url.as_deref(),
        Some("http://cam/onvif/recording")
    );
    assert_eq!(caps.search.url.as_deref(), Some("http://cam/onvif/search"));
    assert_eq!(caps.replay.url.as_deref(), Some("http://cam/onvif/replay"));
    assert_eq!(caps.media2.url.as_deref(), Some("http://cam/onvif/media2"));
    assert_eq!(
        caps.device_io.url.as_deref(),
        Some("http://cam/onvif/deviceio")
    );
}

#[tokio::test]
async fn test_builder_with_clock_sync_calls_date_time_first() {
    // clock sync → GetCapabilities: two responses required, in that order
    let transport = SequenceTransport::new(&[system_date_xml(), caps_full_xml()]);
    let session = OnvifSession::builder("http://cam/onvif/device")
        .with_clock_sync()
        .with_transport(transport)
        .build()
        .await
        .expect("session with clock sync failed");

    // Session must be functional after the two-call init sequence
    assert!(session.capabilities().media.url.is_some());
}

#[tokio::test]
async fn test_builder_without_clock_sync_uses_one_call() {
    // Without clock sync only GetCapabilities is called
    let transport = SequenceTransport::new(&[caps_full_xml()]);
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(transport)
        .build()
        .await
        .expect("build without clock sync failed");
}

#[tokio::test]
async fn test_builder_transport_error_propagates() {
    let result = OnvifSession::builder("http://cam/onvif/device")
        .with_transport(Arc::new(ErrorTransport { status: 503 }))
        .build()
        .await;

    assert!(matches!(result, Err(OnvifError::Transport(_))));
}

#[tokio::test]
async fn test_builder_soap_fault_propagates() {
    let transport = SequenceTransport::new(&[soap_fault_xml()]);
    let result = OnvifSession::builder("http://cam/onvif/device")
        .with_transport(transport)
        .build()
        .await;

    assert!(matches!(
        result,
        Err(OnvifError::Soap(crate::soap::SoapError::Fault { .. }))
    ));
}

// ── client() / capabilities() accessors ──────────────────────────────────

#[tokio::test]
async fn test_client_accessor_returns_underlying_client() {
    let session = session_with(&[]).await;
    // Verify the client is callable (it will return transport error since no
    // more responses, but the accessor itself must work)
    let _client_ref = session.client();
}

#[tokio::test]
async fn test_capabilities_accessor_is_the_cached_value() {
    let session = session_with(&[]).await;
    let caps = session.capabilities();
    // Smoke check that the cached caps are the parsed full caps fixture
    assert_eq!(caps.device.url.as_deref(), Some("http://cam/onvif/device"));
}

// ── Missing URL errors ────────────────────────────────────────────────────
// Each test builds a session with only a Device URL so the relevant
// service URL resolver returns Err rather than Ok.

async fn session_device_only() -> OnvifSession {
    OnvifSession::builder("http://cam/onvif/device")
        .with_transport(SequenceTransport::new(&[caps_device_only_xml()]))
        .build()
        .await
        .expect("device-only session build failed")
}

#[tokio::test]
async fn test_missing_media_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_profiles().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_ptz_url_returns_error() {
    let session = session_device_only().await;
    let err = session.ptz_stop("tok").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_imaging_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_imaging_settings("src_tok").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_events_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_event_properties().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_events_url_subscribe_returns_error() {
    let session = session_device_only().await;
    let err = session
        .subscribe("http://consumer/notify", None, Some("PT60S"))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_recording_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_recordings().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_search_url_returns_error() {
    let session = session_device_only().await;
    let err = session.find_recordings(None, "PT60S").await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

#[tokio::test]
async fn test_missing_replay_url_returns_error() {
    let session = session_device_only().await;
    let err = session
        .get_replay_uri("tok", "RTP-Unicast", "RTSP")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

fn svc(ns: &str, url: &str) -> crate::types::OnvifService {
    crate::types::OnvifService {
        namespace: ns.to_string(),
        url: url.to_string(),
        version_major: 2,
        version_minor: 0,
    }
}

#[test]
fn fill_missing_service_urls_fills_missing_from_get_services() {
    let services = vec![
        svc(
            "http://www.onvif.org/ver10/device/wsdl",
            "http://cam/onvif/device",
        ),
        svc(
            "http://www.onvif.org/ver10/recording/wsdl",
            "http://cam/onvif/Recording",
        ),
        svc(
            "http://www.onvif.org/ver10/search/wsdl",
            "http://cam/onvif/SearchRecording",
        ),
        svc(
            "http://www.onvif.org/ver10/replay/wsdl",
            "http://cam/onvif/Replay",
        ),
    ];

    let mut caps = Capabilities::default();
    fill_missing_service_urls(&mut caps, &services);

    assert_eq!(
        caps.recording.url.as_deref(),
        Some("http://cam/onvif/Recording")
    );
    assert_eq!(
        caps.search.url.as_deref(),
        Some("http://cam/onvif/SearchRecording")
    );
    assert_eq!(caps.replay.url.as_deref(), Some("http://cam/onvif/Replay"));
}

/// DeviceIO is filled from `GetServices` under **either** spelling of the
/// namespace segment. `deviceio.wsdl` writes `targetNamespace="…/deviceIO/…"`
/// and spells every soapAction `…/deviceio/…`, so firmware copies both; an
/// exact match would find the endpoint on only some devices and
/// `get_digital_inputs` would then fail with a missing-URL error on the rest.
#[test]
fn fill_missing_service_urls_matches_deviceio_in_either_casing() {
    for ns in [
        "http://www.onvif.org/ver10/deviceIO/wsdl",
        "http://www.onvif.org/ver10/deviceio/wsdl",
    ] {
        let mut caps = Capabilities::default();
        fill_missing_service_urls(&mut caps, &[svc(ns, "http://cam/onvif/DeviceIO")]);
        assert_eq!(
            caps.device_io.url.as_deref(),
            Some("http://cam/onvif/DeviceIO"),
            "namespace {ns} did not resolve to the DeviceIO endpoint"
        );
    }

    // And it does not swallow the device-management service, whose namespace
    // differs from DeviceIO's by those two letters alone.
    let mut caps = Capabilities::default();
    fill_missing_service_urls(
        &mut caps,
        &[svc(
            "http://www.onvif.org/ver10/device/wsdl",
            "http://cam/onvif/device",
        )],
    );
    assert_eq!(caps.device_io.url, None);
}

#[test]
fn fill_missing_service_urls_does_not_override_existing() {
    let services = vec![svc(
        "http://www.onvif.org/ver10/recording/wsdl",
        "http://cam/onvif/FromServices",
    )];

    let mut caps = Capabilities::default();
    caps.recording.url = Some("http://cam/onvif/FromCapabilities".to_string());
    fill_missing_service_urls(&mut caps, &services);

    // The GetCapabilities URL wins; GetServices must not clobber it.
    assert_eq!(
        caps.recording.url.as_deref(),
        Some("http://cam/onvif/FromCapabilities")
    );
}

/// A device that advertises no DeviceIO endpoint cannot answer
/// `GetDigitalInputs` at all — the session must say which URL is missing rather
/// than fall back to the device service, which is where this was sent until
/// 0.15 and where a real camera has no such operation.
///
/// Asserts the field path, not just the variant: `session_device_only` is
/// missing every service URL, so a bare `MissingField(_)` would pass on any of
/// them and prove nothing about DeviceIO.
#[tokio::test]
async fn test_missing_device_io_url_returns_error_naming_deviceio() {
    let session = session_device_only().await;
    let err = session.get_digital_inputs().await.unwrap_err();
    match err {
        OnvifError::Soap(crate::soap::SoapError::MissingField(f)) => {
            assert_eq!(f, "DeviceIO service URL");
        }
        other => panic!("expected MissingField(\"DeviceIO service URL\"), got {other:?}"),
    }
}

#[tokio::test]
async fn test_missing_media2_url_returns_error() {
    let session = session_device_only().await;
    let err = session.get_profiles_media2().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

// ── Delegate method tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_get_profiles_delegates_and_returns_results() {
    let session = session_with(&[profiles_xml()]).await;
    let profiles = session.get_profiles().await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].token, "Profile_1");
    assert_eq!(profiles[0].name, "mainStream");
}

#[tokio::test]
async fn test_get_stream_uri_delegates_and_returns_uri() {
    let session = session_with(&[stream_uri_xml()]).await;
    let uri = session.get_stream_uri("Profile_1").await.unwrap();
    assert_eq!(uri.uri, "rtsp://cam:554/stream1");
}

#[tokio::test]
async fn test_ptz_stop_delegates_ok() {
    let session = session_with(&[ptz_stop_xml()]).await;
    session.ptz_stop("Profile_1").await.unwrap();
}

#[tokio::test]
async fn test_get_imaging_settings_delegates_ok() {
    let session = session_with(&[imaging_settings_xml()]).await;
    let settings = session.get_imaging_settings("VideoSource_1").await.unwrap();
    assert_eq!(settings.brightness, Some(50.0));
}

#[tokio::test]
async fn test_get_recordings_delegates_and_returns_items() {
    let session = session_with(&[recordings_xml()]).await;
    let recs = session.get_recordings().await.unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].token, "rec1");
    assert_eq!(recs[0].content, "Normal");
}

#[tokio::test]
async fn test_subscribe_delegates_and_returns_reference() {
    let session = session_with(&[subscribe_response_xml()]).await;
    let sub = session
        .subscribe("http://consumer/notify", None, Some("PT60S"))
        .await
        .unwrap();
    assert_eq!(
        sub.subscription_reference,
        "http://cam/onvif/events/push_sub_1"
    );
}

#[tokio::test]
async fn test_delegate_soap_fault_propagates() {
    let session = session_with(&[soap_fault_xml()]).await;
    let err = session.get_profiles().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::Fault { .. })
    ));
}

#[tokio::test]
async fn test_get_device_info_uses_device_url() {
    // Device methods bypass service URL caching and go directly to device_url
    let device_info_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
      <s:Body>
        <tds:GetDeviceInformationResponse>
          <tds:Manufacturer>TestCorp</tds:Manufacturer>
          <tds:Model>Cam-X</tds:Model>
          <tds:FirmwareVersion>1.0</tds:FirmwareVersion>
          <tds:SerialNumber>SN001</tds:SerialNumber>
          <tds:HardwareId>0x01</tds:HardwareId>
        </tds:GetDeviceInformationResponse>
      </s:Body>
    </s:Envelope>"#;

    let session = session_with(&[device_info_xml]).await;
    let info = session.get_device_info().await.unwrap();
    assert_eq!(info.manufacturer, "TestCorp");
    assert_eq!(info.model, "Cam-X");
}

// ── OSD options vendor-extension enrichment ──────────────────────────────
//
// OnvifSession::get_osd_options must (a) parse the spec-strict shape
// and (b) layer in vendor extensions that OnvifClient deliberately
// ignores. These tests pin both behaviours.

#[tokio::test]
async fn test_get_osd_options_enriches_per_type_quotas_from_attrs() {
    // Genetec / late-Hikvision shape: count + per-type quotas live as
    // attributes on <MaximumNumberOfOSDs>, element body empty.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDOptionsResponse>
              <trt:OSDOptions>
                <tt:MaximumNumberOfOSDs Total="8" Plain="7" DateAndTime="1" Date="1" Time="1"/>
                <tt:Type>Text</tt:Type>
              </trt:OSDOptions>
            </trt:GetOSDOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let session = session_with(&[xml]).await;
    let opts = session.get_osd_options("vsc_1").await.unwrap();

    assert_eq!(opts.max_osd, 8, "Total= attribute should fill max_osd");
    assert_eq!(opts.max_per_text_type.get("Plain"), Some(&7));
    assert_eq!(opts.max_per_text_type.get("DateAndTime"), Some(&1));
    assert_eq!(opts.max_per_text_type.get("Date"), Some(&1));
    assert_eq!(opts.max_per_text_type.get("Time"), Some(&1));
}

#[tokio::test]
async fn test_get_osd_options_parses_conformant_position_options() {
    // The conformant shape: `tt:OSDConfigurationOptions/PositionOption` is
    // `type="xs:string" maxOccurs="unbounded"`, so one element per position.
    // This test was named `..._enriches_flat_position_options` and its comment
    // called this the "Genetec / some-Dahua shape" until 0.15 — it is the
    // spec's, and the strict parser reads it directly now, with no enrichment.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDOptionsResponse>
              <trt:OSDOptions>
                <tt:MaximumNumberOfOSDs>4</tt:MaximumNumberOfOSDs>
                <tt:PositionOption>UpperLeft</tt:PositionOption>
                <tt:PositionOption>UpperRight</tt:PositionOption>
                <tt:PositionOption>LowerLeft</tt:PositionOption>
                <tt:PositionOption>LowerRight</tt:PositionOption>
              </trt:OSDOptions>
            </trt:GetOSDOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let session = session_with(&[xml]).await;
    let opts = session.get_osd_options("vsc_1").await.unwrap();

    assert_eq!(
        opts.position_types,
        vec!["UpperLeft", "UpperRight", "LowerLeft", "LowerRight"]
    );
}

#[tokio::test]
async fn test_get_osd_options_wrapper_shape_still_parses() {
    // The `<PositionOption>` wrapper holding `<Type>` children. This test was
    // named `..._strict_shape_still_parses` until 0.15, when the schema settled
    // which of the two shapes is the spec's: it is the other one. The wrapper
    // is kept as a vendor tolerance, so this must still parse — via
    // `apply_vendor_extensions`, which is the session path only.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <trt:GetOSDOptionsResponse>
              <trt:OSDOptions>
                <tt:MaximumNumberOfOSDs>4</tt:MaximumNumberOfOSDs>
                <tt:Type>Text</tt:Type>
                <tt:PositionOption>
                  <tt:Type>UpperLeft</tt:Type>
                  <tt:Type>UpperRight</tt:Type>
                </tt:PositionOption>
              </trt:OSDOptions>
            </trt:GetOSDOptionsResponse>
          </s:Body>
        </s:Envelope>"#;
    let session = session_with(&[xml]).await;
    let opts = session.get_osd_options("vsc_1").await.unwrap();

    assert_eq!(opts.max_osd, 4);
    assert_eq!(opts.position_types, vec!["UpperLeft", "UpperRight"]);
    assert!(opts.max_per_text_type.is_empty());
}
