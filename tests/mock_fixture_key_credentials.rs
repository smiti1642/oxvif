//! Synthetic credential controls, not ONVIF schema fixtures or real recordings.
#![cfg(feature = "metamorph")]

use oxvif::{
    metamorph::{FixtureStore, MetamorphTransport},
    transport::Transport,
};

const ACTION: &str = "urn:fixture-key/GetResource";
const RESPONSE: &str = "<recorded-resource-927/>";

fn request(authority: &str) -> String {
    format!("<GetResource><Uri>rtsp://{authority}/path</Uri></GetResource>")
}

fn store() -> FixtureStore {
    let mut store = FixtureStore::new("synthetic-key-credentials-927");
    store.record(
        ACTION,
        &request("probe:secret-927@camera.invalid"),
        RESPONSE,
    );
    let fixture = &store.fixtures()[0];
    for value in [
        &fixture.key_canon,
        &fixture.request_raw,
        &fixture.response_raw,
    ] {
        assert!(
            !value.contains("secret-927"),
            "fixture retained synthetic credential"
        );
    }
    assert!(fixture.key_canon.contains("rtsp://camera.invalid/path"));
    store
}

async fn verify(transport: &dyn Transport, base: &str) {
    for authority in [
        "camera.invalid",
        "probe:secret-927@camera.invalid",
        "other:rotated-928@camera.invalid",
    ] {
        assert_eq!(
            transport
                .soap_post(base, ACTION, request(authority))
                .await
                .unwrap(),
            RESPONSE
        );
    }
    let different = transport
        .soap_post(base, ACTION, request("other.invalid"))
        .await
        .unwrap();
    assert_ne!(
        different, RESPONSE,
        "a different host must not reuse the recording"
    );
}

#[tokio::test]
async fn credential_free_key_is_used_by_in_process_replay() {
    verify(&MetamorphTransport::new(store()), "http://metamorph").await;
}

#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn credential_free_key_is_used_by_http_replay() {
    let server = oxvif::mock::MockServer::builder()
        .port(0)
        .replay(store())
        .start()
        .await
        .unwrap();
    verify(&oxvif::transport::HttpTransport::new(), server.device_url()).await;
}
