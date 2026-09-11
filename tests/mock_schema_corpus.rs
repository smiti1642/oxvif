//! Project-generated client/mock exchanges, not schema-derived fixtures.
//! The export is opt-in and external-only. Capturing an exchange is not a
//! conformance pass; an independent validator must inspect the emitted bytes.
#![cfg(feature = "mock")]

use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use oxvif::{
    OnvifClient, OnvifError,
    mock::MockTransport,
    soap::{SoapError, parse_soap_body},
    transport::{Transport, TransportError},
};

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const TARGET: &str = "http://mock";

struct Exchange {
    action: String,
    request: String,
    response: String,
    expected_fault: bool,
}

#[derive(Clone)]
struct Capture {
    mock: MockTransport,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
}

#[async_trait]
impl Transport for Capture {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let response = self.mock.soap_post(url, action, body.clone()).await?;
        self.exchanges.lock().unwrap().push(Exchange {
            action: action.into(),
            request: body,
            response: response.clone(),
            expected_fault: false,
        });
        Ok(response)
    }
}

fn rejected_absent_profile(error: OnvifError) {
    assert_delete_fault(
        error,
        "ter:InvalidArgVal",
        "Profile not found: Corpus-absent",
    );
}

fn assert_delete_fault(error: OnvifError, subcode: &str, reason: &str) {
    let OnvifError::Soap(fault) = &error else {
        panic!("expected a SOAP rejection, not a transport/parser failure");
    };
    assert_eq!(
        fault,
        &SoapError::Fault {
            code: "s:Sender".into(),
            reason: reason.into(),
            subcode: Some(subcode.into()),
            detail: None,
        }
    );
    #[cfg(feature = "health")]
    {
        let check = oxvif::health::CheckError::from(&error);
        assert_eq!(check.class, oxvif::health::ErrorClass::SoapFault);
        assert_eq!(check.subcode.as_deref(), Some(subcode));
        assert_eq!(check.reason, reason);
        assert!(!check.is_auth());
    }
}

async fn profile_exchanges() -> Vec<Exchange> {
    let capture = Capture {
        mock: MockTransport::new(),
        exchanges: Arc::default(),
    };
    let client = OnvifClient::new(TARGET).with_transport(Arc::new(capture.clone()));
    let before = capture.mock.device().read().profiles.profiles.len();
    let profiles = client.get_profiles(TARGET).await.unwrap();
    assert_eq!(profiles.len(), before);
    let fetched = client
        .get_profile(TARGET, &profiles[0].token)
        .await
        .unwrap();
    assert_eq!(fetched.token, profiles[0].token);
    let created = client
        .create_profile(TARGET, "Corpus-Media1\r\n\tend", Some("Corpus-1"))
        .await
        .unwrap();
    assert_eq!(created.token, "Corpus-1");
    assert_eq!(created.name, "Corpus-Media1\r\n\tend");
    let source = capture.mock.device().read().video_source_configs[0]
        .token
        .clone();
    let encoder = capture.mock.device().read().video_encoders[0].token.clone();
    client
        .add_video_source_configuration(TARGET, &created.token, &source)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_source_config_token
            .as_deref(),
        Some(source.as_str())
    );
    client
        .remove_video_source_configuration(TARGET, &created.token)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_source_config_token,
        None
    );
    client
        .add_video_encoder_configuration(TARGET, &created.token, &encoder)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_encoder_config_token
            .as_deref(),
        Some(encoder.as_str())
    );
    client
        .remove_video_encoder_configuration(TARGET, &created.token)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_encoder_config_token,
        None
    );
    client.delete_profile(TARGET, &created.token).await.unwrap();
    assert!(
        !capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .iter()
            .any(|p| p.token == created.token)
    );

    let second_view = client.get_profiles_media2(TARGET).await.unwrap();
    assert_eq!(second_view.len(), before);
    let second = client
        .create_profile_media2(TARGET, "Corpus-Media2\r\n\tend")
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .name,
        "Corpus-Media2\r\n\tend"
    );
    client
        .add_configuration_media2(TARGET, &second, "VideoEncoder", &encoder)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_encoder_config_token
            .as_deref(),
        Some(encoder.as_str())
    );
    client
        .remove_configuration_media2(TARGET, &second, "VideoEncoder", &encoder)
        .await
        .unwrap();
    assert_eq!(
        capture
            .mock
            .device()
            .read()
            .profiles
            .profiles
            .last()
            .unwrap()
            .video_encoder_config_token,
        None
    );
    client.delete_profile_media2(TARGET, &second).await.unwrap();
    assert_eq!(capture.mock.device().read().profiles.profiles.len(), before);
    rejected_absent_profile(
        client
            .delete_profile(TARGET, "Corpus-absent")
            .await
            .unwrap_err(),
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    rejected_absent_profile(
        client
            .delete_profile_media2(TARGET, "Corpus-absent")
            .await
            .unwrap_err(),
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    let fixed = profiles[0].token.clone();
    assert!(capture.mock.device().read().profiles.profiles[0].fixed);
    let before_refusal = serde_json::to_value(&*capture.mock.device().read()).unwrap();
    for media2 in [false, true] {
        let error = if media2 {
            client
                .delete_profile_media2(TARGET, &fixed)
                .await
                .unwrap_err()
        } else {
            client.delete_profile(TARGET, &fixed).await.unwrap_err()
        };
        assert_delete_fault(
            error,
            "ter:Action",
            &format!("Cannot delete fixed profile: {fixed}"),
        );
        capture
            .exchanges
            .lock()
            .unwrap()
            .last_mut()
            .unwrap()
            .expected_fault = true;
        assert_eq!(
            serde_json::to_value(&*capture.mock.device().read()).unwrap(),
            before_refusal
        );
    }
    drop(client);
    Arc::try_unwrap(capture.exchanges)
        .unwrap_or_else(|_| panic!("capture still shared"))
        .into_inner()
        .unwrap()
}

fn export_external(
    directory: &Path,
    exchanges: &[Exchange],
) -> Result<(), Box<dyn std::error::Error>> {
    if exchanges.is_empty() {
        return Err("refusing to export an empty corpus".into());
    }
    if exchanges
        .iter()
        .any(|exchange| exchange.request.contains("UsernameToken"))
    {
        return Err("refusing to export credential-bearing requests".into());
    }
    if !directory.is_absolute() || directory.exists() {
        return Err("corpus destination must be a new absolute directory".into());
    }
    let parent = directory
        .parent()
        .ok_or("missing destination parent")?
        .canonicalize()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    if parent.starts_with(&repository) || repository.starts_with(&parent) {
        return Err("corpus destination must be outside the repository and its ancestors".into());
    }
    let target = parent.join(directory.file_name().ok_or("missing destination name")?);
    std::fs::create_dir(&target)?;
    let mut cases = Vec::new();
    for (index, exchange) in exchanges.iter().enumerate() {
        let (namespace, operation) = exchange
            .action
            .rsplit_once('/')
            .ok_or("invalid captured action")?;
        for (direction, bytes) in [
            ("request", &exchange.request),
            ("response", &exchange.response),
        ] {
            let file = format!("case-{index:03}-{direction}.xml");
            let payload = if direction == "request" {
                format!("{{{namespace}}}{operation}")
            } else if exchange.expected_fault {
                format!("{{{SOAP}}}Fault")
            } else {
                format!("{{{namespace}}}{operation}Response")
            };
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target.join(&file))?;
            std::io::Write::write_all(&mut output, bytes.as_bytes())?;
            cases.push(
                serde_json::json!({"file": file, "root": format!("{{{SOAP}}}Envelope"),
                "payload_path": [format!("{{{SOAP}}}Body"), payload], "action": exchange.action,
                "direction": direction}),
            );
        }
    }
    let mut index = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target.join("cases.json"))?;
    serde_json::to_writer_pretty(
        &mut index,
        &serde_json::json!({"format": 1, "cases": cases}),
    )?;
    Ok(())
}

#[tokio::test]
async fn captures_first_profile_batch_from_real_client_calls_without_network() {
    let exchanges = profile_exchanges().await;
    assert_eq!(exchanges.len(), 17);
    let actions: BTreeSet<_> = exchanges
        .iter()
        .map(|exchange| exchange.action.clone())
        .collect();
    let expected: BTreeSet<_> = [
        ("ver10", "GetProfiles"),
        ("ver10", "GetProfile"),
        ("ver10", "CreateProfile"),
        ("ver10", "DeleteProfile"),
        ("ver10", "AddVideoSourceConfiguration"),
        ("ver10", "RemoveVideoSourceConfiguration"),
        ("ver10", "AddVideoEncoderConfiguration"),
        ("ver10", "RemoveVideoEncoderConfiguration"),
        ("ver20", "GetProfiles"),
        ("ver20", "CreateProfile"),
        ("ver20", "DeleteProfile"),
        ("ver20", "AddConfiguration"),
        ("ver20", "RemoveConfiguration"),
    ]
    .into_iter()
    .map(|(version, op)| format!("http://www.onvif.org/{version}/media/wsdl/{op}"))
    .collect();
    assert_eq!(
        actions, expected,
        "keep the corpus aligned with the first 13 source-audit cards"
    );
    assert_eq!(
        exchanges
            .iter()
            .filter(|exchange| exchange.expected_fault)
            .count(),
        4
    );
    for exchange in exchanges {
        assert!(exchange.request.contains("Envelope"));
        assert!(exchange.response.contains("Envelope"));
        assert!(
            !exchange.request.contains("UsernameToken"),
            "the fixture must not capture credentials"
        );
        if exchange.expected_fault {
            let body = parse_soap_body(&exchange.response).unwrap();
            let code = body.path(&["Fault", "Code"]).unwrap();
            assert_eq!(code.child("Value").unwrap().text(), "s:Sender");
            let category = code.child("Subcode").unwrap();
            let leaf = category.child("Subcode").unwrap();
            let expected = if exchange.request.contains("Corpus-absent") {
                ("ter:InvalidArgVal", "ter:NoProfile")
            } else {
                ("ter:Action", "ter:DeletionOfFixedProfile")
            };
            assert_eq!(category.child("Value").unwrap().text(), expected.0);
            assert_eq!(leaf.child("Value").unwrap().text(), expected.1);
            assert!(leaf.child("Subcode").is_none());
        }
    }
}

#[test]
fn corpus_export_rejects_empty_relative_existing_and_checkout_destinations() {
    let empty = export_external(Path::new("not-an-export"), &[]).unwrap_err();
    assert_eq!(empty.to_string(), "refusing to export an empty corpus");
    let fixture = [Exchange {
        action: "urn:example/Probe".into(),
        request: "<request/>".into(),
        response: "<response/>".into(),
        expected_fault: false,
    }];
    assert_eq!(
        export_external(Path::new("not-an-export"), &fixture)
            .unwrap_err()
            .to_string(),
        "corpus destination must be a new absolute directory"
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        export_external(repository, &fixture)
            .unwrap_err()
            .to_string(),
        "corpus destination must be a new absolute directory"
    );
    assert_eq!(
        export_external(&repository.join("must-not-create-corpus"), &fixture)
            .unwrap_err()
            .to_string(),
        "corpus destination must be outside the repository and its ancestors"
    );
    assert!(!repository.join("must-not-create-corpus").exists());
    let mut credential_fixture = fixture;
    credential_fixture[0].request = "<s:UsernameToken/>".into();
    assert_eq!(
        export_external(Path::new("not-an-export"), &credential_fixture)
            .unwrap_err()
            .to_string(),
        "refusing to export credential-bearing requests"
    );
}

#[tokio::test]
#[ignore = "explicit external-only export; set OXVIF_MOCK_CORPUS to a new absolute directory"]
async fn export_first_profile_batch_for_independent_validation() {
    let directory = std::env::var_os("OXVIF_MOCK_CORPUS")
        .expect("OXVIF_MOCK_CORPUS is required; missing output is not an export pass");
    let exchanges = profile_exchanges().await;
    export_external(Path::new(&directory), &exchanges)
        .expect("external corpus export succeeds without overwriting");
    eprintln!(
        "Exported {} synthetic-only client exchanges / {} XML instances; not a conformance pass",
        exchanges.len(),
        exchanges.len() * 2
    );
}
