//! Persona C — put an ONVIF skin on a non-ONVIF device (metamorph M5).
//!
//! Implement [`DeviceAdapter`] for a device that only speaks RTSP (or a private
//! protocol) and get a working ONVIF device: [`AdapterResponder`] translates
//! ONVIF operations into adapter calls, and everything the adapter doesn't
//! override falls through to the synthetic mock — so the ONVIF scaffolding
//! (profiles, capabilities, services) comes for free and the adapter only
//! supplies what is real about the device.
//!
//! Minimum viable set: [`DeviceAdapter::identity`] + [`DeviceAdapter::stream_uri`]
//! is enough for a standard NVR / Frigate to ingest the device as an ONVIF
//! camera. [`DeviceAdapter::continuous_move`] is an optional hook (default:
//! unsupported → falls through). See `examples/metamorph_adapter.rs`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::mock::fault_injection::FaultInjector;
use crate::mock::helpers::{resp_empty, soap};
use crate::mock::responder::{Chain, RequestCtx, Responder};
use crate::mock::state::MockState;
use crate::soap::XmlNode;
use crate::transport::{Transport, TransportError};
use crate::types::xml_escape;

/// Advertised device identity — the `GetDeviceInformation` fields.
#[derive(Debug, Clone, Default)]
pub struct DeviceIdentity {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub serial_number: String,
    pub hardware_id: String,
}

/// A PTZ continuous-move velocity (each component nominally in `-1.0..=1.0`).
#[derive(Debug, Clone, Copy, Default)]
pub struct PtzVector {
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
}

/// Outcome of an optional adapter hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterResult {
    /// The adapter handled the operation.
    Handled,
    /// The adapter does not implement this operation — fall through to synthetic.
    Unsupported,
}

/// Implement this for a non-ONVIF device to get a working ONVIF device.
///
/// Only [`identity`](Self::identity) and [`stream_uri`](Self::stream_uri) are
/// required; the rest default to "unsupported" and fall through to the
/// synthetic mock.
#[async_trait]
pub trait DeviceAdapter: Send + Sync {
    /// Advertised identity (`GetDeviceInformation`, and later discovery).
    fn identity(&self) -> DeviceIdentity;

    /// The real media URI for a profile (the stream being skinned). `None`
    /// falls through to the synthetic mock's stream URI.
    fn stream_uri(&self, profile: &str) -> Option<String>;

    /// Drive a real PTZ. Default: unsupported (falls through). Return
    /// [`AdapterResult::Handled`] once actioned.
    async fn continuous_move(&self, _profile: &str, _velocity: PtzVector) -> AdapterResult {
        AdapterResult::Unsupported
    }

    /// Grab a snapshot as JPEG bytes. Reserved for a future server layer that
    /// serves the bytes over HTTP; the SOAP-level [`AdapterResponder`] does not
    /// call it. Default: none.
    async fn snapshot(&self) -> Option<Vec<u8>> {
        None
    }
}

/// Chain responder that answers ONVIF operations from a [`DeviceAdapter`],
/// deferring everything it doesn't handle to the next responder (synthetic).
///
/// Spliced ahead of the synthetic terminal via [`Chain::mock_with_extra`].
pub struct AdapterResponder {
    adapter: Arc<dyn DeviceAdapter>,
}

impl AdapterResponder {
    /// A responder over `adapter`.
    pub fn new(adapter: Arc<dyn DeviceAdapter>) -> Self {
        Self { adapter }
    }
}

#[async_trait]
impl Responder for AdapterResponder {
    async fn respond(&self, ctx: &RequestCtx<'_>) -> Option<String> {
        let op = ctx.action.rsplit('/').next().unwrap_or(ctx.action);
        match op {
            "GetDeviceInformation" => Some(device_information(&self.adapter.identity())),
            "GetStreamUri" => {
                let profile = profile_token(ctx.body).unwrap_or_default();
                let uri = self.adapter.stream_uri(&profile)?;
                Some(stream_uri_response(ctx.action, &uri))
            }
            "ContinuousMove" => {
                let profile = ctx_profile(ctx.body).unwrap_or_default();
                let velocity = ptz_velocity(ctx.body);
                match self.adapter.continuous_move(&profile, velocity).await {
                    AdapterResult::Handled => Some(resp_empty("tptz", "ContinuousMoveResponse")),
                    AdapterResult::Unsupported => None,
                }
            }
            _ => None,
        }
    }
}

fn device_information(id: &DeviceIdentity) -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        &format!(
            r#"<tds:GetDeviceInformationResponse>
          <tds:Manufacturer>{}</tds:Manufacturer>
          <tds:Model>{}</tds:Model>
          <tds:FirmwareVersion>{}</tds:FirmwareVersion>
          <tds:SerialNumber>{}</tds:SerialNumber>
          <tds:HardwareId>{}</tds:HardwareId>
        </tds:GetDeviceInformationResponse>"#,
            xml_escape(&id.manufacturer),
            xml_escape(&id.model),
            xml_escape(&id.firmware_version),
            xml_escape(&id.serial_number),
            xml_escape(&id.hardware_id),
        ),
    )
}

/// Build the `GetStreamUriResponse` in the shape matching the requested
/// service: Media2 (`ver20`) is a flat `<Uri>`; Media1 wraps it in `<MediaUri>`.
fn stream_uri_response(action: &str, uri: &str) -> String {
    let uri = xml_escape(uri);
    if action.contains("/ver20/") {
        soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            &format!(
                r#"<tr2:GetStreamUriResponse><tr2:Uri>{uri}</tr2:Uri></tr2:GetStreamUriResponse>"#
            ),
        )
    } else {
        soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                r#"<trt:GetStreamUriResponse><trt:MediaUri>
              <tt:Uri>{uri}</tt:Uri>
              <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
              <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
              <tt:Timeout>PT0S</tt:Timeout>
            </trt:MediaUri></trt:GetStreamUriResponse>"#
            ),
        )
    }
}

/// Extract `Body/GetStreamUri/ProfileToken`.
fn profile_token(body: &str) -> Option<String> {
    let root = XmlNode::parse(body).ok()?;
    root.path(&["Body", "GetStreamUri", "ProfileToken"])
        .map(|n| n.text().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract `Body/ContinuousMove/ProfileToken`.
fn ctx_profile(body: &str) -> Option<String> {
    let root = XmlNode::parse(body).ok()?;
    root.path(&["Body", "ContinuousMove", "ProfileToken"])
        .map(|n| n.text().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the PanTilt (x/y) and Zoom (x) velocity from a `ContinuousMove`.
fn ptz_velocity(body: &str) -> PtzVector {
    let Ok(root) = XmlNode::parse(body) else {
        return PtzVector::default();
    };
    let velocity = root.path(&["Body", "ContinuousMove", "Velocity"]);
    let attr_f = |node: Option<&XmlNode>, child: &str, attr: &str| {
        node.and_then(|n| n.child(child))
            .and_then(|c| c.attr(attr))
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    PtzVector {
        pan: attr_f(velocity, "PanTilt", "x"),
        tilt: attr_f(velocity, "PanTilt", "y"),
        zoom: attr_f(velocity, "Zoom", "x"),
    }
}

/// Base URL the adapter device uses when it must emit absolute URLs.
const ADAPTER_BASE: &str = "http://metamorph-adapter";

/// In-process ONVIF device backed by a [`DeviceAdapter`]: a [`Transport`] whose
/// chain answers from the adapter and falls back to synthetic `DeviceState` for
/// everything the adapter doesn't override.
///
/// ```no_run
/// use std::sync::Arc;
/// use oxvif::OnvifClient;
/// use oxvif::metamorph::{AdapterTransport, DeviceAdapter, DeviceIdentity};
///
/// # struct MyCam;
/// # #[async_trait::async_trait]
/// # impl DeviceAdapter for MyCam {
/// #   fn identity(&self) -> DeviceIdentity { DeviceIdentity::default() }
/// #   fn stream_uri(&self, _p: &str) -> Option<String> { Some("rtsp://…".into()) }
/// # }
/// let client = OnvifClient::new("http://adapter")
///     .with_transport(Arc::new(AdapterTransport::new(Arc::new(MyCam))));
/// ```
#[derive(Clone)]
pub struct AdapterTransport {
    state: Arc<MockState>,
    faults: Arc<FaultInjector>,
    adapter: Arc<dyn DeviceAdapter>,
    enforce_auth: bool,
}

impl AdapterTransport {
    /// An adapter device over `adapter`, with fresh synthetic state and auth off.
    pub fn new(adapter: Arc<dyn DeviceAdapter>) -> Self {
        Self {
            state: Arc::new(MockState::new()),
            faults: Arc::new(FaultInjector::new()),
            adapter,
            enforce_auth: false,
        }
    }

    /// Access the synthetic fallback device state.
    pub fn device(&self) -> &MockState {
        &self.state
    }
}

#[async_trait]
impl Transport for AdapterTransport {
    async fn soap_post(
        &self,
        _url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let responder = AdapterResponder::new(self.adapter.clone());
        let chain = Chain::mock_with_extra(
            self.faults.clone(),
            self.enforce_auth,
            vec![Box::new(responder)],
        );
        let ctx = RequestCtx {
            action,
            base: ADAPTER_BASE,
            body: &body,
            state: &self.state,
        };
        Ok(chain.respond(&ctx).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OnvifClient;

    /// A minimal skin over one fixed RTSP stream — the Persona C template.
    struct RtspCam {
        rtsp: String,
    }

    #[async_trait]
    impl DeviceAdapter for RtspCam {
        fn identity(&self) -> DeviceIdentity {
            DeviceIdentity {
                manufacturer: "Acme".to_string(),
                model: "RTSP-Skin".to_string(),
                firmware_version: "1.0".to_string(),
                serial_number: "SN-1".to_string(),
                hardware_id: "HW-1".to_string(),
            }
        }
        fn stream_uri(&self, _profile: &str) -> Option<String> {
            Some(self.rtsp.clone())
        }
    }

    fn client() -> OnvifClient {
        let adapter = Arc::new(RtspCam {
            rtsp: "rtsp://10.0.0.9:554/stream1".to_string(),
        });
        OnvifClient::new("http://adapter").with_transport(Arc::new(AdapterTransport::new(adapter)))
    }

    #[tokio::test]
    async fn identity_comes_from_the_adapter() {
        let info = client().get_device_info().await.unwrap();
        assert_eq!(info.manufacturer, "Acme");
        assert_eq!(info.model, "RTSP-Skin");
    }

    #[tokio::test]
    async fn stream_uri_passes_through_the_real_rtsp_url() {
        let c = client();
        // Profiles come from the synthetic fallback (the adapter doesn't override).
        let profiles = c.get_profiles("http://adapter/media").await.unwrap();
        assert!(
            !profiles.is_empty(),
            "synthetic profiles fill the scaffolding"
        );
        let uri = c
            .get_stream_uri("http://adapter/media", &profiles[0].token)
            .await
            .unwrap();
        assert_eq!(uri.uri, "rtsp://10.0.0.9:554/stream1");
    }

    #[tokio::test]
    async fn unhandled_ops_fall_through_to_synthetic() {
        // GetHostname isn't overridden → synthetic answers it (no error).
        let h = client().get_hostname().await.unwrap();
        assert!(h.name.is_some());
    }

    #[test]
    fn ptz_velocity_parses_pan_tilt_zoom() {
        let body = r#"<s:Envelope><s:Body><ContinuousMove>
            <ProfileToken>p0</ProfileToken>
            <Velocity><PanTilt x="0.5" y="-0.25"/><Zoom x="0.1"/></Velocity>
        </ContinuousMove></s:Body></s:Envelope>"#;
        let v = ptz_velocity(body);
        assert_eq!(v.pan, 0.5);
        assert_eq!(v.tilt, -0.25);
        assert_eq!(v.zoom, 0.1);
    }
}
