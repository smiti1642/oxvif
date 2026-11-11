use super::*;
use oxvif::{
    OnvifError, OnvifSession,
    media_workflow::{MediaAttemptOutcome, MediaSelection, MediaService, MediaUri, StreamRequest},
    session::{ServiceUrlSource, SessionErrorClass},
    soap::SoapError,
};

const CAPS: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
const SERVICES: &str = "http://www.onvif.org/ver10/device/wsdl/GetServices";
const M1_PROFILES: &str = "http://www.onvif.org/ver10/media/wsdl/GetProfiles";
const M2_PROFILES: &str = "http://www.onvif.org/ver20/media/wsdl/GetProfiles";
const M2_STREAM: &str = "http://www.onvif.org/ver20/media/wsdl/GetStreamUri";
const M2_SNAPSHOT: &str = "http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri";
const M2_ENCODERS: &str = "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations";
const M1_COMPATIBLE: &str =
    "http://www.onvif.org/ver10/media/wsdl/GetCompatibleVideoEncoderConfigurations";
const M1_URL: &str = "http://camera.test/media1";
const M2_URL: &str = "http://camera.test/media2";

fn initialize(media1: bool, media2: Option<&str>) -> Vec<(&'static str, Reply)> {
    let caps = if media1 {
        format!("<t:Media><t:XAddr>{M1_URL}</t:XAddr></t:Media>")
    } else {
        String::new()
    };
    let services = media2.map(|url| format!("<d:Service><d:Namespace>http://www.onvif.org/ver20/media/wsdl</d:Namespace><d:XAddr>{url}</d:XAddr></d:Service>")).unwrap_or_default();
    vec![
        (
            CAPS,
            reply(&format!(
                "<d:GetCapabilitiesResponse><d:Capabilities>{caps}</d:Capabilities></d:GetCapabilitiesResponse>"
            )),
        ),
        (
            SERVICES,
            reply(&format!(
                "<d:GetServicesResponse>{services}</d:GetServicesResponse>"
            )),
        ),
    ]
}

async fn session(script: &Arc<Script>) -> OnvifSession {
    OnvifSession::builder(DEVICE)
        .with_transport(script.clone())
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn media2_catalogue_routes_opaque_profile_tokens_and_retains_native_identity() {
    let mut steps = initialize(false, Some(M2_URL));
    steps.extend([
        (M2_PROFILES, reply("<x:GetProfilesResponse xmlns:x=\"urn:other\"/><m2:GetProfilesResponse><m2:Profiles token=\"right\"><m2:Name>Duplicate name</m2:Name><m2:Configurations><m2:VideoSource token=\"right-cfg\"><t:SourceToken>right-source</t:SourceToken></m2:VideoSource><m2:VideoEncoder token=\"right-encoder\"/></m2:Configurations></m2:Profiles><m2:Profiles token=\"left&amp;aux\"><m2:Name>Duplicate name</m2:Name><m2:Configurations><m2:VideoSource token=\"left-cfg\"><t:SourceToken>left-source</t:SourceToken></m2:VideoSource><m2:VideoEncoder token=\"left-encoder\"/></m2:Configurations></m2:Profiles></m2:GetProfilesResponse>")),
        (M2_STREAM, reply("<m2:GetStreamUriResponse><m2:Uri>rtsp://camera.test/left</m2:Uri></m2:GetStreamUriResponse>")),
        (M2_SNAPSHOT, reply("<m2:GetSnapshotUriResponse><m2:Uri>http://camera.test/left.jpg</m2:Uri></m2:GetSnapshotUriResponse>")),
    ]);
    let script = Script::new(steps);
    let session = session(&script).await;
    let catalogue = session.media_catalogue(MediaSelection::Auto).await.unwrap();
    assert_eq!(catalogue.service, MediaService::Media2);
    assert_eq!(
        catalogue.endpoint_source,
        ServiceUrlSource::ServicesFallback
    );
    assert_eq!(
        catalogue.attempts[0].outcome,
        MediaAttemptOutcome::Succeeded
    );
    assert!(
        matches!(catalogue.select(None), Err(OnvifError::InvalidArgument(message)) if message == "a unique media profile token is required")
    );
    let left = catalogue.profiles_for_source("left-source").next().unwrap();
    assert_eq!(left.encoder_token(), Some("left-encoder"));
    assert_eq!(left.token(), "left&aux");
    let handle = catalogue.select(Some(left.token())).unwrap();
    assert!(
        matches!(session.media_stream_uri(&handle, StreamRequest::UdpUnicast).await.unwrap(), MediaUri::Media2(uri) if uri == "rtsp://camera.test/left")
    );
    assert_eq!(
        session.media_snapshot_uri(&handle).await.unwrap().uri(),
        "http://camera.test/left.jpg"
    );
    let calls = script.calls();
    assert!(calls[2].body.contains("<tr2:Type>All</tr2:Type>"));
    assert!(
        calls[3]
            .body
            .contains("<tr2:Protocol>RtspUnicast</tr2:Protocol>")
    );
    assert!(
        calls[3]
            .body
            .contains("<tr2:ProfileToken>left&amp;aux</tr2:ProfileToken>")
    );
    assert!(
        calls[4]
            .body
            .contains("<tr2:ProfileToken>left&amp;aux</tr2:ProfileToken>")
    );
    assert!(calls[2..].iter().all(|call| call.url == M2_URL));
    script.assert_done();
}

#[tokio::test]
async fn auto_falls_back_only_after_missing_or_qualified_unsupported() {
    for supported in [false, true] {
        let mut steps = initialize(true, supported.then_some(M2_URL));
        if supported {
            steps.push((M2_PROFILES, reply("<s:Fault><s:Code><s:Value>s:Sender</s:Value><s:Subcode><s:Value>a:ActionNotSupported</s:Value></s:Subcode></s:Code><s:Reason><s:Text xml:lang=\"en\">not-implemented-fixture</s:Text></s:Reason></s:Fault>")));
        }
        steps.push((M1_PROFILES, reply("<m:GetProfilesResponse><m:Profiles token=\"only\"><t:Name>Media One</t:Name></m:Profiles></m:GetProfilesResponse>")));
        let script = Script::new(steps);
        let catalogue = session(&script)
            .await
            .media_catalogue(MediaSelection::Auto)
            .await
            .unwrap();
        assert_eq!(catalogue.service, MediaService::Media1);
        assert_eq!(
            catalogue.attempts[0].outcome,
            if supported {
                MediaAttemptOutcome::Unsupported
            } else {
                MediaAttemptOutcome::NotAdvertised
            }
        );
        assert_eq!(
            catalogue.attempts[1].outcome,
            MediaAttemptOutcome::Succeeded
        );
        assert_eq!(catalogue.select(None).unwrap().token(), "only");
        assert_eq!(script.calls().last().unwrap().url, M1_URL);
        script.assert_done();
    }
}

#[tokio::test]
async fn auto_does_not_treat_auth_parse_or_origin_errors_as_unsupported() {
    let cases = [
        (
            Some(M2_URL),
            Some(Reply::Status(401)),
            SessionErrorClass::Authentication,
        ),
        (
            Some(M2_URL),
            Some(Reply::Xml("<broken>".into())),
            SessionErrorClass::XmlSyntax,
        ),
        (
            Some("http://other.test/media2"),
            None,
            SessionErrorClass::InvalidArgument,
        ),
    ];
    for (url, response, expected) in cases {
        let mut steps = initialize(true, url);
        if let Some(response) = response {
            steps.push((M2_PROFILES, response));
        }
        let script = Script::new(steps);
        let failure = session(&script)
            .await
            .media_catalogue(MediaSelection::Auto)
            .await
            .unwrap_err();
        assert_eq!(failure.attempts.len(), 1);
        assert_eq!(
            failure.attempts[0].outcome,
            MediaAttemptOutcome::Failed(expected)
        );
        assert!(!script.calls().iter().any(|call| call.action == M1_PROFILES));
        script.assert_done();
    }
    let mut steps = initialize(true, Some(M2_URL));
    steps.push((M2_PROFILES, reply("<s:Fault><s:Code><s:Value>s:Sender</s:Value><s:Subcode><s:Value xmlns:q=\"urn:other\">q:ActionNotSupported</s:Value></s:Subcode></s:Code><s:Reason><s:Text xml:lang=\"en\">foreign-fixture</s:Text></s:Reason></s:Fault>")));
    let script = Script::new(steps);
    let failure = session(&script)
        .await
        .media_catalogue(MediaSelection::Auto)
        .await
        .unwrap_err();
    assert!(
        matches!(failure.cause, OnvifError::Soap(SoapError::Fault {subcode: Some(code), reason, ..}) if code == "{urn:other}ActionNotSupported" && reason == "foreign-fixture")
    );
    assert_eq!(failure.attempts.len(), 1);
}

#[tokio::test]
async fn compatible_encoder_queries_keep_service_and_capability_provenance() {
    use oxvif::media_workflow::{MediaEncoders, StreamCapabilities};
    let mut steps = initialize(true, Some(M2_URL));
    steps.extend([
        (M1_PROFILES, reply("<m:GetProfilesResponse><m:Profiles token=\"p-one\"/></m:GetProfilesResponse>")),
        (M1_COMPATIBLE, reply("<m:GetCompatibleVideoEncoderConfigurationsResponse><m:Configurations token=\"encoder-one\"><t:Encoding>H264</t:Encoding><t:Resolution><t:Width>640</t:Width><t:Height>480</t:Height></t:Resolution></m:Configurations></m:GetCompatibleVideoEncoderConfigurationsResponse>")),
        (M2_PROFILES, reply("<m2:GetProfilesResponse><m2:Profiles token=\"p-two\"/></m2:GetProfilesResponse>")),
        (M2_ENCODERS, reply("<m2:GetVideoEncoderConfigurationsResponse><m2:Configurations token=\"encoder-two\"><t:Encoding>H265</t:Encoding><t:Resolution><t:Width>1920</t:Width><t:Height>1080</t:Height></t:Resolution></m2:Configurations></m2:GetVideoEncoderConfigurationsResponse>")),
    ]);
    let script = Script::new(steps);
    let session = session(&script).await;
    let one = session
        .media_catalogue(MediaSelection::Media1)
        .await
        .unwrap()
        .select(None)
        .unwrap();
    assert!(
        matches!(session.media_compatible_video_encoders(&one).await.unwrap(), MediaEncoders::Media1(configs) if configs[0].token == "encoder-one" && configs[0].resolution.width == 640)
    );
    let two = session
        .media_catalogue(MediaSelection::Media2)
        .await
        .unwrap()
        .select(None)
        .unwrap();
    let caps = oxvif::Media2StreamingCapabilities {
        rtp_multicast: Some(false),
        ..Default::default()
    };
    let error = session
        .media_stream_uri_with_capabilities(
            &two,
            StreamRequest::UdpMulticast,
            StreamCapabilities::Media2(&caps),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, OnvifError::InvalidArgument(message) if message == "stream transport was reported unavailable")
    );
    assert!(
        matches!(session.media_compatible_video_encoders(&two).await.unwrap(), MediaEncoders::Media2(configs) if configs[0].token == "encoder-two" && configs[0].resolution.width == 1920)
    );
    let calls = script.calls();
    assert!(
        calls[3]
            .body
            .contains("<trt:ProfileToken>p-one</trt:ProfileToken>")
    );
    assert!(
        calls[5]
            .body
            .contains("<tr2:ProfileToken>p-two</tr2:ProfileToken>")
    );
    script.assert_done();
}
