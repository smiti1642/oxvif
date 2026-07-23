//! Persona C template: skin one fixed RTSP stream as an ONVIF device.
//!
//! Implement [`DeviceAdapter`] for a device that only speaks RTSP and drive a
//! normal `OnvifClient` against it — `GetDeviceInformation` and `GetStreamUri`
//! come from the adapter; everything else (profiles, capabilities) falls
//! through to the synthetic mock. A standard NVR / Frigate can ingest this as
//! an ONVIF camera.
//!
//! ```text
//! cargo run --example metamorph_adapter --features metamorph
//! ```

use std::sync::Arc;

use oxvif::OnvifClient;
use oxvif::metamorph::{AdapterTransport, DeviceAdapter, DeviceIdentity};

/// A skin over one fixed RTSP URL — the whole adapter is these two methods.
struct RtspCam {
    rtsp: String,
}

#[async_trait::async_trait]
impl DeviceAdapter for RtspCam {
    fn identity(&self) -> DeviceIdentity {
        DeviceIdentity {
            manufacturer: "Acme".into(),
            model: "RTSP-Skin".into(),
            firmware_version: "1.0".into(),
            serial_number: "SN-0001".into(),
            hardware_id: "HW-0001".into(),
        }
    }

    fn stream_uri(&self, _profile: &str) -> Option<String> {
        Some(self.rtsp.clone())
    }
}

#[tokio::main]
async fn main() {
    let adapter = Arc::new(RtspCam {
        rtsp: "rtsp://192.168.1.77:554/Streaming/Channels/101".into(),
    });
    let client =
        OnvifClient::new("http://adapter").with_transport(Arc::new(AdapterTransport::new(adapter)));

    let info = client.get_device_info().await.unwrap();
    println!("device: {} {}", info.manufacturer, info.model);

    // Profiles come from the synthetic scaffolding; the stream URI is the real one.
    let profiles = client.get_profiles("http://adapter/media").await.unwrap();
    let uri = client
        .get_stream_uri("http://adapter/media", &profiles[0].token)
        .await
        .unwrap();
    println!("profile {} → {}", profiles[0].token, uri.uri);
}
