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
        .create_profile(
            TARGET,
            "Corpus-Media1\r\n\tend",
            Some(" Corpus &amp;\r\n\tend "),
        )
        .await
        .unwrap();
    assert_eq!(created.token, " Corpus &amp;\r\n\tend ");
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
    let error = client
        .create_profile(TARGET, "empty-policy-corpus", Some(""))
        .await
        .unwrap_err();
    assert_empty_policy(error, "s:Sender");
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
    capture.mock.device().modify(|state| {
        let mut invalid = state.profiles.profiles[0].clone();
        invalid.token.clear();
        invalid.name = "legacy-empty-corpus".into();
        state.profiles.profiles.push(invalid);
    });
    let before_reads = serde_json::to_value(&*capture.mock.device().read()).unwrap();
    for media2 in [false, true] {
        let error = if media2 {
            client.get_profiles_media2(TARGET).await.unwrap_err()
        } else {
            client.get_profiles(TARGET).await.unwrap_err()
        };
        assert_empty_policy(error, "s:Receiver");
        capture
            .exchanges
            .lock()
            .unwrap()
            .last_mut()
            .unwrap()
            .expected_fault = true;
        assert_eq!(
            serde_json::to_value(&*capture.mock.device().read()).unwrap(),
            before_reads
        );
    }
    drop(client);
    Arc::try_unwrap(capture.exchanges)
        .unwrap_or_else(|_| panic!("capture still shared"))
        .into_inner()
        .unwrap()
}

async fn rate_exchanges() -> Vec<Exchange> {
    let capture = Capture {
        mock: MockTransport::new(),
        exchanges: Arc::default(),
    };
    let client = OnvifClient::new(TARGET).with_transport(Arc::new(capture.clone()));
    let mut config = client
        .get_video_encoder_configuration_media2(TARGET, "VEC_1")
        .await
        .unwrap();
    assert_eq!(config.rate_control.as_ref().unwrap().frame_rate_limit, 25.0);
    config.rate_control.as_mut().unwrap().frame_rate_limit = 12.5;
    client
        .set_video_encoder_configuration_media2(TARGET, &config)
        .await
        .unwrap();
    assert_eq!(
        capture.mock.device().read().video_encoders[0].frame_rate_limit,
        12.5
    );
    let actual = client
        .get_video_encoder_configuration_media2(TARGET, "VEC_1")
        .await
        .unwrap();
    assert_eq!(actual.rate_control.as_ref().unwrap().frame_rate_limit, 12.5);
    let error = client
        .get_video_encoder_configuration(TARGET, "VEC_1")
        .await
        .unwrap_err();
    let OnvifError::Soap(error) = error else {
        panic!("expected Media1 representation Fault")
    };
    assert_eq!(
        error,
        SoapError::Fault {
            code: "s:Receiver".into(),
            reason: "Encoder frame rate cannot be represented by Media1; use Media2".into(),
            subcode: Some("mock:RequestPolicy".into()),
            detail: None
        }
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    config.rate_control.as_mut().unwrap().frame_rate_limit = 25.0;
    client
        .set_video_encoder_configuration_media2(TARGET, &config)
        .await
        .unwrap();
    let actual = client
        .get_video_encoder_configuration(TARGET, "VEC_1")
        .await
        .unwrap();
    assert_eq!(actual.rate_control.as_ref().unwrap().frame_rate_limit, 25);
    // A source-authored, structurally valid negative value bypasses the client
    // preflight to exercise the synthetic refusal and unchanged state.
    let before = serde_json::to_value(&*capture.mock.device().read()).unwrap();
    let previous = capture
        .exchanges
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.action.ends_with("/SetVideoEncoderConfiguration"))
        .unwrap()
        .request
        .clone();
    let xml = capture
        .soap_post(
            TARGET,
            "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration",
            previous.replace(
                "<tt:FrameRateLimit>12.5</tt:FrameRateLimit>",
                "<tt:FrameRateLimit>-1</tt:FrameRateLimit>",
            ),
        )
        .await
        .unwrap();
    let body = parse_soap_body(&xml).unwrap();
    let fault = body.child("Fault").unwrap();
    assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), "s:Sender");
    assert_eq!(
        fault.path(&["Code", "Subcode", "Value"]).unwrap().text(),
        "ter:InvalidArgs"
    );
    assert_eq!(
        fault.path(&["Reason", "Text"]).unwrap().text(),
        "Invalid encoder FrameRateLimit"
    );
    assert_eq!(
        serde_json::to_value(&*capture.mock.device().read()).unwrap(),
        before
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    drop(client);
    Arc::try_unwrap(capture.exchanges)
        .unwrap_or_else(|_| panic!("capture still shared"))
        .into_inner()
        .unwrap()
}

#[tokio::test]
async fn captures_fractional_rate_and_explicit_media1_limit() {
    let exchanges = rate_exchanges().await;
    assert_eq!(exchanges.len(), 7);
    assert_eq!(exchanges.iter().filter(|e| e.expected_fault).count(), 2);
    assert_eq!(
        exchanges
            .iter()
            .map(|e| &e.action)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
}

async fn source_exchanges() -> Vec<Exchange> {
    let capture = Capture {
        mock: MockTransport::new(),
        exchanges: Arc::default(),
    };
    const TOKEN: &str = "Corpus & <source> 北";
    capture.mock.device().modify(|state| {
        state.video_source_configs[0].token = TOKEN.into();
        for profile in &mut state.profiles.profiles {
            if profile.video_source_config_token.as_deref() == Some("VSC_1") {
                profile.video_source_config_token = Some(TOKEN.into());
            }
        }
    });
    let client = OnvifClient::new(TARGET).with_transport(Arc::new(capture.clone()));
    let sensors = client.get_video_sources(TARGET).await.unwrap();
    assert_eq!(sensors.len(), 2);
    let configs = client
        .get_video_source_configurations(TARGET)
        .await
        .unwrap();
    assert_eq!(configs[0].token, TOKEN);
    let mut config = client
        .get_video_source_configuration(TARGET, TOKEN)
        .await
        .unwrap();
    assert_eq!(config.source_token, "VS_1");
    let options = client
        .get_video_source_configuration_options(TARGET, TOKEN)
        .await
        .unwrap();
    assert_eq!(options.bounds_range.unwrap().width_range.max, 2592);
    config.name = "Corpus source & <改名>".into();
    config.bounds.width = 640;
    config.bounds.height = 360;
    client
        .set_video_source_configuration(TARGET, &config)
        .await
        .unwrap();
    assert_eq!(
        capture.mock.device().read().video_source_configs[0].width,
        640
    );
    let options = client
        .get_video_source_configuration_options(TARGET, TOKEN)
        .await
        .unwrap();
    assert_eq!(options.bounds_range.unwrap().width_range.max, 2592);
    let configs = client
        .get_video_source_configurations_media2(TARGET)
        .await
        .unwrap();
    assert_eq!(configs[0].name, config.name);
    config.source_token = "VS_2".into();
    config.bounds.width = 3000;
    config.bounds.height = 2000;
    client
        .set_video_source_configuration_media2(TARGET, &config)
        .await
        .unwrap();
    assert_eq!(
        capture.mock.device().read().video_source_configs[0].width,
        1280
    );
    let options = client
        .get_video_source_configuration_options_media2(TARGET, TOKEN)
        .await
        .unwrap();
    assert_eq!(options.source_tokens, ["VS_2"]);
    let configs = client
        .get_video_source_configurations_media2(TARGET)
        .await
        .unwrap();
    assert_eq!(
        (configs[0].bounds.width, configs[0].bounds.height),
        (1280, 720)
    );
    let before = serde_json::to_value(&*capture.mock.device().read()).unwrap();
    let error = client
        .get_video_source_configuration(TARGET, "absent-source-941")
        .await
        .unwrap_err();
    assert_delete_fault(
        error,
        "ter:InvalidArgVal",
        "Source configuration not found: absent-source-941",
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    let error = client
        .get_video_source_configuration_options_media2(TARGET, "absent-source-942")
        .await
        .unwrap_err();
    assert_delete_fault(
        error,
        "ter:InvalidArgVal",
        "Source configuration not found: absent-source-942",
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    config.bounds.x = 7;
    let error = client
        .set_video_source_configuration_media2(TARGET, &config)
        .await
        .unwrap_err();
    assert_delete_fault(
        error,
        "ter:InvalidArgVal",
        "Invalid mock source setting: Bounds/@x (only zero origin is modeled)",
    );
    capture
        .exchanges
        .lock()
        .unwrap()
        .last_mut()
        .unwrap()
        .expected_fault = true;
    for version in ["ver10", "ver20"] {
        let ns = format!("http://www.onvif.org/{version}/media/wsdl");
        let op = "GetVideoSourceConfigurationOptions";
        let xml = capture.soap_post(TARGET, &format!("{ns}/{op}"), format!("<s:Envelope xmlns:s='{SOAP}' xmlns:m='{ns}'><s:Body><m:{op}/></s:Body></s:Envelope>")).await.unwrap();
        let body = parse_soap_body(&xml).unwrap();
        assert_eq!(
            body.children[0]
                .path(&["Options", "BoundsRange", "WidthRange", "Max"])
                .unwrap()
                .text(),
            "1280"
        );
    }
    assert_eq!(
        serde_json::to_value(&*capture.mock.device().read()).unwrap(),
        before
    );
    drop(client);
    Arc::try_unwrap(capture.exchanges)
        .unwrap_or_else(|_| panic!("capture still shared"))
        .into_inner()
        .unwrap()
}

#[tokio::test]
async fn captures_source_batch_with_write_readback_and_refusals() {
    let exchanges = source_exchanges().await;
    assert_eq!(exchanges.len(), 15);
    assert_eq!(exchanges.iter().filter(|e| e.expected_fault).count(), 3);
    assert_eq!(
        exchanges
            .iter()
            .map(|e| &e.action)
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
    for e in exchanges.iter().filter(|e| e.expected_fault) {
        let body = parse_soap_body(&e.response).unwrap();
        let fault = body.child("Fault").unwrap();
        assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), "s:Sender");
        assert_eq!(
            fault.path(&["Code", "Subcode", "Value"]).unwrap().text(),
            "ter:InvalidArgVal"
        );
        assert_eq!(
            fault
                .path(&["Code", "Subcode", "Subcode", "Value"])
                .unwrap()
                .text(),
            if e.action.ends_with("/SetVideoSourceConfiguration") {
                "ter:ConfigModify"
            } else {
                "ter:NoConfig"
            }
        );
    }
}

fn assert_empty_policy(error: OnvifError, code: &str) {
    let OnvifError::Soap(fault) = error else {
        panic!("expected an explicit mock policy Fault");
    };
    assert_eq!(
        fault,
        SoapError::Fault {
            code: code.into(),
            reason: "The mock does not support empty profile tokens".into(),
            subcode: Some("mock:RequestPolicy".into()),
            detail: None,
        }
    );
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
    assert_eq!(exchanges.len(), 20);
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
        7
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
            if !exchange.action.ends_with("/DeleteProfile") {
                let expected_code = if exchange.action.ends_with("/CreateProfile") {
                    "s:Sender"
                } else {
                    assert!(exchange.action.ends_with("/GetProfiles"));
                    "s:Receiver"
                };
                assert_eq!(code.child("Value").unwrap().text(), expected_code);
                let category = code.child("Subcode").unwrap();
                assert_eq!(
                    category.child("Value").unwrap().text(),
                    "mock:RequestPolicy"
                );
                assert!(category.child("Subcode").is_none());
                assert_eq!(
                    body.path(&["Fault", "Reason", "Text"]).unwrap().text(),
                    "The mock does not support empty profile tokens"
                );
                assert!(
                    exchange
                        .response
                        .contains("xmlns:mock=\"urn:oxvif:mock:error\"")
                );
                continue;
            }
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
async fn export_reviewed_batches_for_independent_validation() {
    let directory = std::env::var_os("OXVIF_MOCK_CORPUS")
        .expect("OXVIF_MOCK_CORPUS is required; missing output is not an export pass");
    let mut exchanges = profile_exchanges().await;
    exchanges.extend(source_exchanges().await);
    exchanges.extend(rate_exchanges().await);
    export_external(Path::new(&directory), &exchanges)
        .expect("external corpus export succeeds without overwriting");
    eprintln!(
        "Exported {} synthetic-only client exchanges / {} XML instances; not a conformance pass",
        exchanges.len(),
        exchanges.len() * 2
    );
}
