//! Typed adapter arguments and raw fallback are distinct contracts.
#![cfg(feature = "metamorph")]

use async_trait::async_trait;
use oxvif::{
    OnvifClient,
    metamorph::{
        AdapterResponder, AdapterResult, AdapterTransport, DeviceAdapter, DeviceIdentity, PtzVector,
    },
    mock::{MockState, RequestCtx, Responder},
    transport::Transport,
};
use std::sync::{Arc, Mutex};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
const PTZ: &str = "http://www.onvif.org/ver20/ptz/wsdl";
const DEVICE: &str = "http://www.onvif.org/ver10/device/wsdl";
const TT: &str = "http://www.onvif.org/ver10/schema";
const RAW: &str = "intentionally non-XML raw response-945";

#[derive(Default)]
struct Probe {
    calls: Mutex<Vec<(String, String, [f32; 3])>>,
    raw: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl DeviceAdapter for Probe {
    fn identity(&self) -> DeviceIdentity {
        self.calls
            .lock()
            .unwrap()
            .push(("identity".into(), String::new(), [0.0; 3]));
        DeviceIdentity {
            manufacturer: "identity-945".into(),
            ..Default::default()
        }
    }
    fn stream_uri(&self, profile: &str) -> Option<String> {
        self.calls
            .lock()
            .unwrap()
            .push(("stream".into(), profile.into(), [0.0; 3]));
        Some("rtsp://adapter.example.test/stream-945".into())
    }
    async fn continuous_move(&self, profile: &str, velocity: PtzVector) -> AdapterResult {
        self.calls.lock().unwrap().push((
            "move".into(),
            profile.into(),
            [velocity.pan, velocity.tilt, velocity.zoom],
        ));
        AdapterResult::Handled
    }
    async fn respond_raw(&self, op: &str, body: &str) -> Option<String> {
        self.raw.lock().unwrap().push((op.into(), body.into()));
        Some(RAW.into())
    }
}

fn operation(ns: &str, op: &str, fields: &str) -> String {
    format!("<m:{op} xmlns:m='{ns}' xmlns:tt='{TT}'>{fields}</m:{op}>")
}

#[tokio::test]
async fn typed_adapter_preserves_client_identity_and_velocity() {
    let probe = Arc::new(Probe::default());
    let transport = Arc::new(AdapterTransport::new(probe.clone()));
    let client = OnvifClient::new("http://adapter").with_transport(transport.clone());
    assert_eq!(
        client.get_device_info().await.unwrap().manufacturer,
        "identity-945"
    );
    for profile in ["head-one", " head &amp;\r\n\t尾 "] {
        assert_eq!(
            client
                .get_stream_uri("http://adapter", profile)
                .await
                .unwrap()
                .uri,
            "rtsp://adapter.example.test/stream-945"
        );
        assert_eq!(
            client
                .get_stream_uri_media2("http://adapter", profile)
                .await
                .unwrap(),
            "rtsp://adapter.example.test/stream-945"
        );
        client
            .ptz_continuous_move("http://adapter", profile, 0.31, -0.47, 0.63)
            .await
            .unwrap();
        let calls = probe.calls.lock().unwrap();
        let tail = &calls[calls.len() - 3..];
        assert_eq!(
            tail,
            &[
                ("stream".into(), profile.into(), [0.0; 3]),
                ("stream".into(), profile.into(), [0.0; 3]),
                ("move".into(), profile.into(), [0.31, -0.47, 0.63])
            ]
        );
    }
    assert!(probe.raw.lock().unwrap().is_empty());
    for ns in [M1, M2] {
        let body = operation(
            ns,
            "GetStreamUri",
            "<m:ProfileToken><![CDATA[ CDATA &amp; identity ]]></m:ProfileToken>",
        );
        let xml = transport
            .soap_post("http://adapter", &format!("{ns}/GetStreamUri"), body)
            .await
            .unwrap();
        assert!(xml.contains("stream-945"));
        assert_eq!(
            probe.calls.lock().unwrap().last().unwrap().1,
            " CDATA &amp; identity "
        );
    }
    let fields = "<x:ProfileToken xmlns:x='urn:foreign'>wrong</x:ProfileToken><m:Extension><m:ProfileToken>nested</m:ProfileToken></m:Extension><m:ProfileToken> head&#x20;&amp;&#x9;two </m:ProfileToken><m:Velocity><x:PanTilt xmlns:x='urn:foreign' x='0.9' y='0.9'/><tt:PanTilt x='&#x20;0.31 ' y='-0.47'/><tt:Zoom x='0.63'/></m:Velocity>";
    let body = format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope'><s:Header><h:ProfileToken xmlns:h='{PTZ}'>header</h:ProfileToken></s:Header><s:Body>{}</s:Body></s:Envelope>",
        operation(PTZ, "ContinuousMove", fields)
    );
    let xml = transport
        .soap_post("http://adapter", &format!("{PTZ}/ContinuousMove"), body)
        .await
        .unwrap();
    assert!(xml.contains("ContinuousMoveResponse"));
    assert_eq!(
        probe.calls.lock().unwrap().last(),
        Some(&("move".into(), " head &\ttwo ".into(), [0.31, -0.47, 0.63]))
    );
    assert_eq!(probe.calls.lock().unwrap().len(), 10);
    assert!(probe.raw.lock().unwrap().is_empty());
}

#[tokio::test]
async fn typed_adapter_declines_ambiguous_requests_without_touching_raw_fallback() {
    let probe = Arc::new(Probe::default());
    let responder = AdapterResponder::new(probe.clone());
    let state = MockState::new();
    let before = serde_json::to_value(&*state.read()).unwrap();
    let mut cases = Vec::new();
    for (ns, op) in [
        (DEVICE, "GetDeviceInformation"),
        (M1, "GetStreamUri"),
        (M2, "GetStreamUri"),
        (PTZ, "ContinuousMove"),
    ] {
        let body = operation(ns, op, "<m:ProfileToken>p</m:ProfileToken>");
        cases.push((format!("urn:wrong/{op}"), body.clone()));
        cases.push((
            format!("urn:wrong/{op}"),
            operation("urn:wrong", op, "<m:ProfileToken>p</m:ProfileToken>"),
        ));
        cases.push((op.into(), body.clone()));
        cases.push((format!("{ns}/{op}"), operation("urn:wrong", op, "")));
        cases.push((format!("{ns}/{op}"), "<broken".into()));
        cases.push((format!("{ns}/{op}"), format!("{body}{body}")));
        cases.push((format!("{ns}/{op}"), format!("<!DOCTYPE m:{op}>{body}")));
    }
    cases.push((
        format!("{DEVICE}/GetDeviceInformation"),
        format!(
            "{}{}",
            " ".repeat(2 * 1024 * 1024),
            operation(DEVICE, "GetDeviceInformation", "")
        ),
    ));
    for ns in [M1, M2, PTZ] {
        let op = if ns == PTZ {
            "ContinuousMove"
        } else {
            "GetStreamUri"
        };
        for fields in [
            "",
            "<m:ProfileToken/>",
            "<m:ProfileToken>one</m:ProfileToken><m:ProfileToken>two</m:ProfileToken>",
            "<m:ProfileToken><m:Nested>one</m:Nested></m:ProfileToken>",
            "<x:ProfileToken xmlns:x='urn:foreign'>one</x:ProfileToken>",
            "<m:Extension><m:ProfileToken>one</m:ProfileToken></m:Extension>",
        ] {
            cases.push((format!("{ns}/{op}"), operation(ns, op, fields)));
        }
    }
    let axes = "<tt:PanTilt x='0.31' y='-0.47'/><tt:Zoom x='0.63'/>";
    for fields in [
        String::new(),
        "<m:Velocity/>".into(),
        "<m:Velocity><tt:PanTilt x='0.31' y='-0.47'/></m:Velocity>".into(),
        "<m:Velocity><tt:PanTilt x='0.31'/><tt:Zoom x='0.63'/></m:Velocity>".into(),
        format!("<m:Velocity>{axes}</m:Velocity><m:Velocity>{axes}</m:Velocity>"),
        format!("<m:Velocity>{axes}<tt:Zoom x='0.7'/></m:Velocity>"),
        format!("<m:Velocity>{}</m:Velocity>", axes.replace("0.31", "NaN")),
        format!("<m:Velocity>{}</m:Velocity>", axes.replace("0.63", "1e100")),
        format!("<m:Velocity>{}</m:Velocity>", axes.replace("x='0.63'", "x='0.63' space='urn:space'")),
        format!("<m:Velocity>{axes}</m:Velocity><m:Timeout>PT1S</m:Timeout>"),
        "<m:Velocity><x:PanTilt xmlns:x='urn:foreign' x='0.31' y='-0.47'/><tt:Zoom x='0.63'/></m:Velocity>".into(),
    ] {
        cases.push((format!("{PTZ}/ContinuousMove"), operation(PTZ, "ContinuousMove", &format!("<m:ProfileToken>p</m:ProfileToken>{fields}"))));
    }
    for (action, body) in cases {
        let ctx = RequestCtx {
            action: &action,
            body: &body,
            base: "http://adapter",
            state: &state,
        };
        assert_eq!(
            responder.respond(&ctx).await.as_deref(),
            Some(RAW),
            "{action}: {body}"
        );
        assert!(
            probe.calls.lock().unwrap().is_empty(),
            "declined requests cannot drive typed hooks"
        );
        assert_eq!(
            probe.raw.lock().unwrap().pop(),
            Some((action.rsplit('/').next().unwrap().into(), body))
        );
    }
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
}
