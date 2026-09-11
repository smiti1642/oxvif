//! Source-authored rate-control regressions, independent of the mock renderer.
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use oxvif::{
    OnvifClient, OnvifError,
    soap::SoapError,
    transport::{Transport, TransportError},
};

const URL: &str = "http://offline-rate-test";
const READ: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
const FPS: &str = "Configuration/RateControl/FrameRateLimit";
const BITRATE: &str = "Configuration/RateControl/BitrateLimit";
type Calls = Arc<Mutex<Vec<(String, String)>>>;

struct Answer {
    xml: String,
    calls: Calls,
}

#[async_trait]
impl Transport for Answer {
    async fn soap_post(
        &self,
        _: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        self.calls.lock().unwrap().push((action.into(), body));
        Ok(self.xml.clone())
    }
}

fn answer(xml: String) -> (OnvifClient, Calls) {
    let calls = Arc::default();
    let client = OnvifClient::new(URL).with_transport(Arc::new(Answer {
        xml,
        calls: Arc::clone(&calls),
    }));
    (client, calls)
}

fn response(rate: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:m="http://www.onvif.org/ver20/media/wsdl" xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body><m:GetVideoEncoderConfigurationsResponse><m:Configurations token="rate-947" GovLength="25" Profile="Main"><tt:Name>Rate test</tt:Name><tt:UseCount>0</tt:UseCount><tt:Encoding>H264</tt:Encoding><tt:Resolution><tt:Width>640</tt:Width><tt:Height>360</tt:Height></tt:Resolution>{rate}<tt:Quality>5</tt:Quality></m:Configurations></m:GetVideoEncoderConfigurationsResponse></s:Body></s:Envelope>"#
    )
}

fn rate(fps: &str) -> String {
    format!(
        "<tt:RateControl><tt:FrameRateLimit>{fps}</tt:FrameRateLimit><tt:BitrateLimit>1024</tt:BitrateLimit></tt:RateControl>"
    )
}

#[tokio::test]
async fn fractional_rate_survives_public_client_read() {
    for (wire, expected) in [
        ("25", 25.0_f64),
        ("12.5", 12.5),
        ("0", 0.0),
        ("29.97", f64::from(29.97_f32)),
        ("1.25E1", 12.5),
    ] {
        let (client, calls) = answer(response(&rate(wire)));
        let values = client
            .get_video_encoder_configurations_media2(URL)
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].token, "rate-947");
        let rc = values[0].rate_control.as_ref().unwrap();
        assert_eq!(f64::from(rc.frame_rate_limit), expected, "wire={wire}");
        assert_eq!(rc.bitrate_limit, 1024);
        assert_eq!(calls.lock().unwrap()[0].0, READ);
    }
}

#[tokio::test]
async fn malformed_rate_is_not_zero() {
    for value in ["bad-fps-948", "", "NaN", "INF", "-INF", "1e100", "-1"] {
        let (client, _) = answer(response(&rate(value)));
        let error = client
            .get_video_encoder_configurations_media2(URL)
            .await
            .unwrap_err();
        assert_parse(error, SoapError::invalid(FPS, value));
    }
}

fn assert_parse(error: OnvifError, expected: SoapError) {
    let OnvifError::Soap(error) = error else {
        panic!("expected SOAP value error")
    };
    assert_eq!(error, expected);
}

#[tokio::test]
async fn absent_rate_differs_from_incomplete_or_ambiguous_rate() {
    let (client, _) = answer(response(""));
    let configs = client
        .get_video_encoder_configurations_media2(URL)
        .await
        .unwrap();
    assert_eq!(configs[0].token, "rate-947");
    assert!(configs[0].rate_control.is_none());
    for (body, expected) in [
        ("<tt:RateControl/>".into(), SoapError::missing(FPS)),
        (
            "<tt:RateControl><tt:FrameRateLimit>12.5</tt:FrameRateLimit></tt:RateControl>".into(),
            SoapError::missing(BITRATE),
        ),
        (
            rate("12.5").replace("1024", "invalid-949"),
            SoapError::invalid(BITRATE, "invalid-949"),
        ),
        (
            rate("12.5").replace("1024", "4294967296"),
            SoapError::invalid(BITRATE, "4294967296"),
        ),
        (
            rate("<tt:Nested>12.5</tt:Nested>"),
            SoapError::invalid(FPS, "non-scalar value"),
        ),
        (
            rate("12.5").replace(
                "</tt:FrameRateLimit>",
                "</tt:FrameRateLimit><tt:FrameRateLimit>25</tt:FrameRateLimit>",
            ),
            SoapError::invalid(FPS, "duplicate field"),
        ),
        (
            format!("{}{}", rate("12.5"), rate("25")),
            SoapError::invalid("Configuration/RateControl", "duplicate field"),
        ),
    ] {
        let (client, _) = answer(response(&body));
        assert_parse(
            client
                .get_video_encoder_configurations_media2(URL)
                .await
                .unwrap_err(),
            expected,
        );
    }
}

async fn configuration() -> oxvif::VideoEncoderConfiguration2 {
    let (client, _) = answer(response(&rate("25")));
    client
        .get_video_encoder_configurations_media2(URL)
        .await
        .unwrap()
        .remove(0)
}

#[tokio::test]
async fn rate_write_preserves_fraction_and_rejects_invalid_before_transport() {
    let response = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:m="http://www.onvif.org/ver20/media/wsdl"><s:Body><m:SetVideoEncoderConfigurationResponse/></s:Body></s:Envelope>"#;
    let mut config = configuration().await;
    config.rate_control.as_mut().unwrap().frame_rate_limit = 12.5;
    let (client, calls) = answer(response.into());
    client
        .set_video_encoder_configuration_media2(URL, &config)
        .await
        .unwrap();
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0,
            "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration"
        );
        assert!(
            calls[0]
                .1
                .contains("<tt:FrameRateLimit>12.5</tt:FrameRateLimit>")
        );
        assert!(
            calls[0]
                .1
                .contains("<tt:BitrateLimit>1024</tt:BitrateLimit>")
        );
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
        config.rate_control.as_mut().unwrap().frame_rate_limit = value;
        let (client, calls) = answer(response.into());
        assert_parse(
            client
                .set_video_encoder_configuration_media2(URL, &config)
                .await
                .unwrap_err(),
            SoapError::invalid(FPS, value.to_string()),
        );
        assert!(
            calls.lock().unwrap().is_empty(),
            "invalid rate reached transport"
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn rate_json_accepts_old_integers_and_preserves_fraction() {
    let mut value: oxvif::VideoRateControl2 =
        serde_json::from_str(r#"{"frame_rate_limit":25,"bitrate_limit":1024}"#).unwrap();
    assert_eq!(value.frame_rate_limit, 25.0);
    value.frame_rate_limit = 12.5;
    let json = serde_json::to_value(&value).unwrap();
    assert_eq!(json["frame_rate_limit"].as_f64(), Some(12.5));
    assert_eq!(json["bitrate_limit"].as_u64(), Some(1024));
    let restored: oxvif::VideoRateControl2 = serde_json::from_value(json).unwrap();
    assert_eq!(restored.frame_rate_limit, 12.5);
    for rate in [f32::NAN, f32::INFINITY, -1.0] {
        value.frame_rate_limit = rate;
        assert_eq!(
            serde_json::to_value(&value).unwrap_err().to_string(),
            "frame rate must be finite and nonnegative"
        );
    }
    let error = serde_json::from_str::<oxvif::VideoRateControl2>(
        r#"{"frame_rate_limit":-1,"bitrate_limit":1024}"#,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("frame rate must be finite and nonnegative"),
        "{error}"
    );
}
