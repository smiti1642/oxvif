//! Source-selected PTZ identity controls with project-authored values, not schema fixtures.
#![cfg(feature = "mock")]

use oxvif::{
    mock::{MockState, MockTransport, state::DeviceState},
    soap::{SoapError, XmlNode, find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const NS: &str = "http://www.onvif.org/ver20/ptz/wsdl";
const TOKEN: &str = " profile &amp; <one> \"'\r\n\tend ";
const ENCODED: &str = " profile &amp;amp; &lt;one&gt; &quot;&apos;&#13;&#10;&#9;end ";

// All current users of require_profile/require_head, not a protocol catalogue.
const CASES: &[(&str, &str)] = &[
    ("GetStatus", ""),
    ("GetPresets", ""),
    ("SetPreset", "<p:PresetName>identity-preset</p:PresetName>"),
    ("RemovePreset", "<p:PresetToken>Preset_1</p:PresetToken>"),
    ("GotoPreset", "<p:PresetToken>Preset_1</p:PresetToken>"),
    (
        "AbsoluteMove",
        "<p:Position><tt:Zoom x='0.43'/></p:Position>",
    ),
    (
        "RelativeMove",
        "<p:Translation><tt:Zoom x='0.11'/></p:Translation>",
    ),
    (
        "ContinuousMove",
        "<p:Velocity><tt:Zoom x='0.21'/></p:Velocity>",
    ),
    ("Stop", ""),
    ("GotoHomePosition", ""),
    ("SetHomePosition", ""),
    ("GetCompatibleConfigurations", ""),
    ("GetPresetTours", ""),
    (
        "GetPresetTour",
        "<p:PresetTourToken>Tour_1</p:PresetTourToken>",
    ),
    ("GetPresetTourOptions", ""),
    ("CreatePresetTour", ""),
    (
        "ModifyPresetTour",
        "<p:PresetTour token='Tour_1'><tt:Name>identity-tour</tt:Name></p:PresetTour>",
    ),
    (
        "OperatePresetTour",
        "<p:PresetTourToken>Tour_1</p:PresetTourToken><p:Operation>Start</p:Operation>",
    ),
    (
        "RemovePresetTour",
        "<p:PresetTourToken>Tour_1</p:PresetTourToken>",
    ),
];

fn seeded() -> DeviceState {
    let mut device = DeviceState::default();
    let tour = device.ptz.channel("PTZNode_1").unwrap().tours[0].clone();
    device.ptz.channel_mut("PTZNode_1").zoom = 0.17;
    let second = device.ptz.channel_mut("PTZNode_2");
    second.zoom = 0.71;
    second.tours = vec![tour];
    device
}

async fn request(
    transport: &dyn Transport,
    url: &str,
    op: &str,
    field: &str,
    extra: &str,
) -> String {
    transport.soap_post(url, &format!("{NS}/{op}"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:p='{NS}' xmlns:tt='http://www.onvif.org/ver10/schema' xmlns:x='urn:decoy'><s:Body><p:{op}>{field}{extra}</p:{op}></s:Body></s:Envelope>"
    )).await.unwrap()
}

fn stable_response(mut node: XmlNode) -> serde_json::Value {
    if node.local_name == "UtcTime" {
        node.text = Some("dynamic-clock".into());
    }
    serde_json::json!([
        node.local_name,
        node.text,
        node.attrs,
        node.children
            .into_iter()
            .map(stable_response)
            .collect::<Vec<_>>()
    ])
}

async fn exercise(transport: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    for ordinary in ["Profile_1", "Profile_3"] {
        for (op, extra) in CASES {
            let baseline = seeded();
            state.modify(|device| {
                *device = baseline.clone();
                device
                    .profiles
                    .profiles
                    .iter_mut()
                    .find(|p| p.token == ordinary)
                    .unwrap()
                    .token = TOKEN.into();
            });
            let other_node = if ordinary == "Profile_1" {
                "PTZNode_2"
            } else {
                "PTZNode_1"
            };
            let other_before = serde_json::to_value(state.read().ptz.channel(other_node)).unwrap();
            let notifications = hooks.load(Ordering::SeqCst);
            let control = MockTransport::with_state(MockState::with_state(baseline));
            let expected = request(
                &control,
                "http://mock",
                op,
                &format!("<p:ProfileToken>{ordinary}</p:ProfileToken>"),
                extra,
            )
            .await;
            find_response(
                &parse_soap_body(&expected).unwrap(),
                &format!("{op}Response"),
            )
            .unwrap();
            let observed = request(
                transport,
                url,
                op,
                &format!("<p:ProfileToken>{ENCODED}</p:ProfileToken>"),
                extra,
            )
            .await;
            assert_eq!(
                stable_response(parse_soap_body(&observed).unwrap()),
                stable_response(parse_soap_body(&expected).unwrap()),
                "decoded identity: {ordinary}/{op}: {observed}"
            );
            let mut actual = state.read().clone();
            assert_eq!(
                serde_json::to_value(actual.ptz.channel(other_node)).unwrap(),
                other_before,
                "other head changed: {ordinary}/{op}"
            );
            let writes = !matches!(
                *op,
                "GetStatus"
                    | "GetPresets"
                    | "GetCompatibleConfigurations"
                    | "GetPresetTours"
                    | "GetPresetTour"
                    | "GetPresetTourOptions"
                    | "Stop"
            );
            assert_eq!(
                hooks.load(Ordering::SeqCst),
                notifications + usize::from(writes),
                "successful hook: {ordinary}/{op}"
            );
            if *op == "AbsoluteMove" {
                let node = if ordinary == "Profile_1" {
                    "PTZNode_1"
                } else {
                    "PTZNode_2"
                };
                assert_eq!(actual.ptz.channel(node).unwrap().zoom, 0.43);
            }
            actual
                .profiles
                .profiles
                .iter_mut()
                .find(|p| p.token == TOKEN)
                .unwrap()
                .token = ordinary.into();
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(&*control.device().read()).unwrap(),
                "full state: {ordinary}/{op}"
            );
        }
    }

    // A direct, correctly qualified field wins over foreign/nested decoys.
    state.modify(|device| *device = seeded());
    for (token, zoom) in [("Profile_1", "0.17"), ("Profile_3", "0.71")] {
        let field = format!(
            "<x:ProfileToken>Profile_4</x:ProfileToken><x:Extension><p:ProfileToken>Profile_4</p:ProfileToken></x:Extension><p:ProfileToken>{token}</p:ProfileToken>"
        );
        let response = request(transport, url, "GetStatus", &field, "").await;
        let body = parse_soap_body(&response).unwrap();
        let status = find_response(&body, "GetStatusResponse").unwrap();
        assert_eq!(
            status
                .path(&["PTZStatus", "Position", "Zoom"])
                .unwrap()
                .attr("x"),
            Some(zoom)
        );
    }

    for (op, extra) in CASES {
        for field in [
            "<p:ProfileToken>Profile_1</p:ProfileToken><p:ProfileToken>Profile_3</p:ProfileToken>",
            "<p:ProfileToken><p:Nested>Profile_1</p:Nested></p:ProfileToken>",
        ] {
            let before = serde_json::to_value(&*state.read()).unwrap();
            let notifications = hooks.load(Ordering::SeqCst);
            let response = request(transport, url, op, field, extra).await;
            let body = parse_soap_body(&response).unwrap();
            assert_eq!(
                find_response(&body, &format!("{op}Response")).unwrap_err(),
                SoapError::Fault {
                    code: "s:Sender".into(),
                    subcode: Some("ter:InvalidArgs".into()),
                    reason: "Invalid Args".into(),
                    detail: None,
                },
                "{op}: {response}"
            );
            assert_eq!(
                serde_json::to_value(&*state.read()).unwrap(),
                before,
                "refused state: {op}"
            );
            assert_eq!(
                hooks.load(Ordering::SeqCst),
                notifications,
                "refused hook: {op}"
            );
        }
    }

    for (field, code, reason) in [
        (
            "",
            "env:Sender",
            "NoProfileToken-STATUS-5601: every PTZ operation is per-profile",
        ),
        (
            "<p:ProfileToken/>",
            "env:Sender",
            "NoProfileToken-STATUS-5601: every PTZ operation is per-profile",
        ),
        (
            "<x:ProfileToken>Profile_1</x:ProfileToken>",
            "env:Sender",
            "NoProfileToken-STATUS-5601: every PTZ operation is per-profile",
        ),
        (
            "<x:Extension><p:ProfileToken>Profile_1</p:ProfileToken></x:Extension>",
            "env:Sender",
            "NoProfileToken-STATUS-5601: every PTZ operation is per-profile",
        ),
        (
            "<p:ProfileToken>not&#32;stored&amp;&lt;head&gt;&#13;&#10;&#9;end</p:ProfileToken>",
            "ter:NoProfile",
            "NoSuchProfile-STATUS-5601: not stored&<head>\r\n\tend",
        ),
    ] {
        let before = serde_json::to_value(&*state.read()).unwrap();
        let notifications = hooks.load(Ordering::SeqCst);
        let response = request(transport, url, "GetStatus", field, "").await;
        assert_eq!(
            find_response(&parse_soap_body(&response).unwrap(), "GetStatusResponse").unwrap_err(),
            SoapError::Fault {
                code: code.into(),
                subcode: None,
                reason: reason.into(),
                detail: None,
            }
        );
        assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
        assert_eq!(hooks.load(Ordering::SeqCst), notifications);
    }

    // Prefix aliases, default namespace, header decoys and exactly-once CDATA.
    state.modify(|device| device.profiles.profiles[0].token = "literal &amp; <head>".into());
    let response = transport.soap_post(url, &format!("{NS}/GetStatus"), format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:p='{NS}'><s:Header><p:ProfileToken>Profile_4</p:ProfileToken></s:Header><s:Body><GetStatus xmlns='{NS}'><ProfileToken><![CDATA[literal &amp; <head>]]></ProfileToken></GetStatus></s:Body></s:Envelope>"
    )).await.unwrap();
    let body = parse_soap_body(&response).unwrap();
    assert_eq!(
        find_response(&body, "GetStatusResponse")
            .unwrap()
            .path(&["PTZStatus", "Position", "Zoom"])
            .unwrap()
            .attr("x"),
        Some("0.17")
    );
}

#[tokio::test]
async fn in_process_ptz_uses_scoped_literal_profile_identity() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let transport = MockTransport::with_state(state);
    exercise(&transport, "http://mock", transport.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_ptz_uses_scoped_literal_profile_identity() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = hooks.clone();
    let server = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    exercise(
        &oxvif::transport::HttpTransport::new(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}
