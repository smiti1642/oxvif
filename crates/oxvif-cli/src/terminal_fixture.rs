//! Opt-in isolated terminal fixture, driven by packaging/test_cli_workflow_terminal.py.
//! It does not use the system registry, native credential store or LAN discovery.
use oxvif_cli::{
    Application, DiscoveryDeviceView, DiscoveryRecord, DiscoveryRegistrationStatus,
    DiscoveryResultSummary, ExecutionOptions, MemoryCredentialStore, NewDevice, RegistryStore,
};
use std::{io::IsTerminal, sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires a real terminal; run packaging/test_cli_workflow_terminal.py"]
async fn workflow_terminal_fixture() {
    assert!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "fixture needs a terminal"
    );
    let directory = std::path::PathBuf::from(
        std::env::var("OXVIF_TERMINAL_FIXTURE_DIR").expect("explicit isolated fixture directory"),
    );
    let mode = std::env::var("OXVIF_TERMINAL_FIXTURE_MODE").expect("explicit fixture mode");
    assert!(
        !directory.join("devices.toml").exists(),
        "refuse existing registry"
    );
    crate::ui_settings::configure(&directory, Some(crate::ui_settings::LineNumbers::Absolute))
        .unwrap();
    let credentials = Arc::new(MemoryCredentialStore::default());
    let registry = RegistryStore::at(&directory);
    let app = Application::with_stores(registry.clone(), credentials);
    let a = Arc::new(
        oxvif::mock::MockServer::builder()
            .enforce_auth(true)
            .start()
            .await
            .unwrap(),
    );
    let b = oxvif::mock::MockServer::builder()
        .enforce_auth(true)
        .start()
        .await
        .unwrap();
    let c = oxvif::mock::MockServer::builder()
        .enforce_auth(true)
        .start()
        .await
        .unwrap();
    let stalled = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let stalled_url = format!(
        "http://{}/onvif/device_service",
        stalled.local_addr().unwrap()
    );
    let control_server = a.clone();
    let control_directory = directory.clone();
    let controls = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if control_directory.join("clear-profiles").is_file() {
                control_server
                    .device()
                    .modify(|state| state.profiles.profiles.clear());
                std::fs::remove_file(control_directory.join("clear-profiles")).unwrap();
                std::fs::write(control_directory.join("profiles-cleared"), b"ok").unwrap();
            }
        }
    });
    let records = [
        ("fixture-a", a.device_url()),
        ("fixture-b", b.device_url()),
        ("fixture-c", c.device_url()),
    ]
    .into_iter()
    .map(|(id, target)| DiscoveryDeviceView {
        record: DiscoveryRecord {
            endpoint: format!("urn:uuid:{id}"),
            xaddrs: vec![target.into()],
            types: Vec::new(),
            scopes: Vec::new(),
            manufacturer: Some("Fixture".into()),
            model: Some(id.into()),
            firmware_version: None,
            serial_number: None,
        },
        registration_status: DiscoveryRegistrationStatus::New,
        registered_device_id: None,
    })
    .collect::<Vec<_>>();
    let options = ExecutionOptions {
        timeout: Duration::from_secs(2),
        ..Default::default()
    };
    match mode.as_str() {
        "discover" => {
            let summary = DiscoveryResultSummary {
                total_count: 3,
                matched_count: 3,
                saved_count: 0,
                new_count: 3,
                incomplete_count: 0,
            };
            crate::interactive::browse_discovery(&app, &options, &records, &summary)
                .await
                .unwrap();
            assert!(registry.get("added-one").unwrap().has_credentials);
            assert!(registry.get("added-two").unwrap().has_credentials);
            assert_eq!(registry.list().unwrap().0.len(), 2);
            assert_eq!(registry.current().unwrap().unwrap().id, "added-two");
        }
        "manage" => {
            for index in 0..256 {
                registry
                    .add(NewDevice {
                        id: format!("cam{index:03}"),
                        name: Some(format!("Camera {index:03}")),
                        target: match index {
                            255 => a.device_url().to_owned(),
                            254 => b.device_url().to_owned(),
                            253 => stalled_url.clone(),
                            _ => format!("http://127.0.0.1:9/onvif/camera-{index}"),
                        },
                        tags: vec!["fixture".into()],
                    })
                    .unwrap();
            }
            crate::manage::run_fixture(&app, &options, records)
                .await
                .unwrap();
            assert!(registry.get("added-three").unwrap().has_credentials);
            assert!(
                registry.current().unwrap().is_none(),
                "manage setup must not change global current selection"
            );
            assert!(
                !registry.get("cam255").unwrap().has_credentials,
                "temporary credentials must not persist"
            );
            assert!(directory.join("inventory.json").is_file());
        }
        _ => panic!("unsupported fixture mode"),
    }
    let contents = std::fs::read_to_string(directory.join("devices.toml")).unwrap();
    assert!(!contents.contains("wrong-password-fixture"));
    controls.abort();
    let _ = controls.await;
    Arc::try_unwrap(a).ok().unwrap().shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
    println!("TERMINAL_FIXTURE_PASS");
}
