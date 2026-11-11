use super::*;
use oxvif::{
    OnvifError,
    ptz_motion::{PanTiltVector, PtzMotion, PtzStopAxes, PtzVector, ZoomVector},
};

const URL: &str = "http://camera.test/ptz";
const CONFIG: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration";
const OPTIONS: &str = "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions";
const ABS: &str = "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove";
const REL: &str = "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove";
const CONT: &str = "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove";
const STOP: &str = "http://www.onvif.org/ver20/ptz/wsdl/Stop";

fn config(token: &str, node: &str, defaults: &str) -> Reply {
    reply(&format!(
        "<p:GetConfigurationResponse><p:PTZConfiguration token=\"{token}\"><t:Name>{token}</t:Name><t:UseCount>1</t:UseCount><t:NodeToken>{node}</t:NodeToken>{defaults}</p:PTZConfiguration></p:GetConfigurationResponse>"
    ))
}
fn options(spaces: &str) -> Reply {
    reply(&format!(
        "<p:GetConfigurationOptionsResponse><p:PTZConfigurationOptions><t:Spaces>{spaces}</t:Spaces><t:PTZTimeout><t:Min>PT0.5S</t:Min><t:Max>PT4S</t:Max></t:PTZTimeout></p:PTZConfigurationOptions></p:GetConfigurationOptionsResponse>"
    ))
}
fn zoom(x: f32, space: Option<&str>) -> PtzVector {
    PtzVector {
        zoom: Some(ZoomVector {
            x,
            space: space.map(str::to_owned),
        }),
        pan_tilt: None,
    }
}

#[tokio::test]
async fn typed_motion_checks_reported_axes_defaults_and_bounds_before_io() {
    let script = Script::new(vec![
        (
            CONFIG,
            config(
                "zoom-config",
                "zoom-head",
                "<t:DefaultAbsoluteZoomPositionSpace>urn:zoom:wide</t:DefaultAbsoluteZoomPositionSpace>",
            ),
        ),
        (
            OPTIONS,
            options(
                "<t:AbsoluteZoomPositionSpace><t:URI>urn:zoom:narrow</t:URI><t:XRange><t:Min>0</t:Min><t:Max>1</t:Max></t:XRange></t:AbsoluteZoomPositionSpace><t:AbsoluteZoomPositionSpace><t:URI>urn:zoom:wide</t:URI><t:XRange><t:Min>0</t:Min><t:Max>5</t:Max></t:XRange></t:AbsoluteZoomPositionSpace><t:ZoomSpeedSpace><t:URI>urn:zoom:speed</t:URI><t:XRange><t:Min>0</t:Min><t:Max>2</t:Max></t:XRange></t:ZoomSpeedSpace><t:ContinuousZoomVelocitySpace><t:URI>urn:zoom:velocity</t:URI></t:ContinuousZoomVelocitySpace>",
            ),
        ),
        (
            CONFIG,
            config(
                "pan-config",
                "pan-head",
                "<t:DefaultRelativePanTiltTranslationSpace>urn:pan:relative</t:DefaultRelativePanTiltTranslationSpace>",
            ),
        ),
        (
            OPTIONS,
            options(
                "<t:RelativePanTiltTranslationSpace><t:URI>urn:pan:relative</t:URI><t:XRange><t:Min>-2</t:Min><t:Max>2</t:Max></t:XRange><t:YRange><t:Min>-4</t:Min><t:Max>4</t:Max></t:YRange></t:RelativePanTiltTranslationSpace>",
            ),
        ),
        (ABS, reply("<p:AbsoluteMoveResponse/>")),
        (REL, reply("<p:RelativeMoveResponse/>")),
        (CONT, reply("<p:ContinuousMoveResponse/>")),
        (STOP, reply("<p:StopResponse/>")),
    ]);
    let client = script.client();
    let zoom_caps = client
        .ptz_motion_capabilities(URL, "zoom-config")
        .await
        .unwrap();
    let pan_caps = client
        .ptz_motion_capabilities(URL, "pan-config")
        .await
        .unwrap();
    assert_eq!(zoom_caps.configuration().node_token, "zoom-head");
    assert_eq!(pan_caps.configuration().node_token, "pan-head");
    let ranges: Vec<_> = zoom_caps.reported_spaces().unwrap().collect();
    assert!(
        ranges
            .iter()
            .any(|(kind, range)| *kind == "ContinuousZoomVelocitySpace" && range.x.is_none())
    );
    let pan = PtzVector {
        pan_tilt: Some(PanTiltVector {
            x: 2.0,
            y: -4.0,
            space: None,
        }),
        zoom: None,
    };
    let move_to = |position| PtzMotion::Absolute {
        position,
        speed: None,
    };
    for (motion, expected) in [
        (
            move_to(pan.clone()),
            "PTZ operation does not support this axis",
        ),
        (
            move_to(zoom(5.1, None)),
            "PTZ vector is outside reported space bounds",
        ),
        (
            move_to(zoom(1.1, Some("urn:zoom:narrow"))),
            "PTZ vector is outside reported space bounds",
        ),
        (
            move_to(zoom(0.5, Some("urn:unreported"))),
            "PTZ space was not reported for this axis",
        ),
        (
            move_to(zoom(f32::NAN, None)),
            "invalid PTZ coordinate or space URI",
        ),
        (
            move_to(PtzVector::default()),
            "PTZ vector requires at least one axis group",
        ),
        (
            PtzMotion::Continuous {
                velocity: zoom(1.0, None),
                timeout: Some(Duration::from_secs(5)),
            },
            "PTZ motion timeout is outside reported bounds",
        ),
    ] {
        let error = client
            .ptz_move(URL, "zoom-profile", &motion, Some(&zoom_caps))
            .await
            .unwrap_err();
        assert!(matches!(error, OnvifError::InvalidArgument(message) if message == expected));
    }
    assert_eq!(script.calls().len(), 4);
    client
        .ptz_move(
            URL,
            "zoom&profile",
            &PtzMotion::Absolute {
                position: zoom(5.0, None),
                speed: Some(zoom(2.0, Some("urn:zoom:speed"))),
            },
            Some(&zoom_caps),
        )
        .await
        .unwrap();
    client
        .ptz_move(
            URL,
            "pan-profile",
            &PtzMotion::Relative {
                translation: pan,
                speed: None,
            },
            Some(&pan_caps),
        )
        .await
        .unwrap();
    client
        .ptz_move(
            URL,
            "zoom&profile",
            &PtzMotion::Continuous {
                velocity: zoom(20.0, None),
                timeout: Some(Duration::from_millis(1500)),
            },
            Some(&zoom_caps),
        )
        .await
        .unwrap();
    client
        .ptz_move(
            URL,
            "zoom&profile",
            &PtzMotion::Stop(PtzStopAxes::Zoom),
            None,
        )
        .await
        .unwrap();
    let calls = script.calls();
    assert!(
        calls[1]
            .body
            .contains("<tptz:ConfigurationToken>zoom-config</tptz:ConfigurationToken>")
    );
    assert!(
        calls[3]
            .body
            .contains("<tptz:ConfigurationToken>pan-config</tptz:ConfigurationToken>")
    );
    assert!(
        calls[4]
            .body
            .contains("<tptz:ProfileToken>zoom&amp;profile</tptz:ProfileToken>")
    );
    assert!(
        calls[4]
            .body
            .contains("<tptz:Position><tt:Zoom x=\"5\"/></tptz:Position>")
    );
    assert!(
        calls[4]
            .body
            .contains("<tptz:Speed><tt:Zoom x=\"2\" space=\"urn:zoom:speed\"/></tptz:Speed>")
    );
    assert!(!calls[4].body.contains("<tt:PanTilt"));
    assert!(
        calls[5]
            .body
            .contains("<tptz:Translation><tt:PanTilt x=\"2\" y=\"-4\"/></tptz:Translation>")
    );
    assert!(!calls[5].body.contains("<tt:Zoom"));
    assert!(
        calls[6]
            .body
            .contains("<tptz:Timeout>PT1.5S</tptz:Timeout>")
    );
    assert!(
        calls[7]
            .body
            .contains("<tptz:PanTilt>false</tptz:PanTilt><tptz:Zoom>true</tptz:Zoom>")
    );
    assert!(calls.iter().all(|call| call.url == URL));
    script.assert_done();
}

#[cfg(feature = "mock")]
#[tokio::test]
async fn typed_zoom_moves_only_the_selected_mock_head_and_preserves_other_axes() {
    let transport = Arc::new(oxvif::mock::MockTransport::new());
    let client = OnvifClient::new(DEVICE).with_transport(transport);
    let profiles = client
        .get_profiles("http://camera.test/onvif/media_service")
        .await
        .unwrap();
    let profile = profiles.iter().find(|p| p.token == "Profile_1").unwrap();
    let peer = profiles.iter().find(|p| p.token == "Profile_3").unwrap();
    assert_ne!(profile.ptz_config_token, peer.ptz_config_token);
    let url = "http://camera.test/onvif/ptz_service";
    client
        .ptz_absolute_move(url, &profile.token, 0.2, -0.3, 0.4)
        .await
        .unwrap();
    let before = client.ptz_get_status(url, &profile.token).await.unwrap();
    let peer_before = client.ptz_get_status(url, &peer.token).await.unwrap();
    let caps = client
        .ptz_motion_capabilities(url, profile.ptz_config_token.as_deref().unwrap())
        .await
        .unwrap();
    client
        .ptz_move(
            url,
            &profile.token,
            &PtzMotion::Absolute {
                position: zoom(0.7, None),
                speed: None,
            },
            Some(&caps),
        )
        .await
        .unwrap();
    let after = client.ptz_get_status(url, &profile.token).await.unwrap();
    let peer_after = client.ptz_get_status(url, &peer.token).await.unwrap();
    assert_eq!(
        (before.pan, before.tilt, before.zoom),
        (Some(0.2), Some(-0.3), Some(0.4))
    );
    assert_eq!(
        (after.pan, after.tilt, after.zoom),
        (Some(0.2), Some(-0.3), Some(0.7))
    );
    assert_eq!(
        (peer_after.pan, peer_after.tilt, peer_after.zoom),
        (peer_before.pan, peer_before.tilt, peer_before.zoom)
    );
}
