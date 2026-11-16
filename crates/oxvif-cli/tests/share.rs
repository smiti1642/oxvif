use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

const ACCOUNT: &str = "share-canary-account";
const PASSWORD: &str = "share-canary-password";
const PATH_SECRET: &str = "share-canary-private-folder";
const DEVICE_TEXT: &str = "share-canary-device-response";
const QUERY: &str = "share-canary-uri-query";
const ENCODED_PASSWORD: &str = "c2hhcmUtY2FuYXJ5LXBhc3N3b3Jk";

fn cli(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxvif"));
    command
        .env("OXVIF_CONFIG_DIR", directory)
        .env_remove("OXVIF_DEVICE")
        .env_remove("OXVIF_USERNAME")
        .env_remove("OXVIF_PASSWORD")
        .env("RUST_BACKTRACE", "1");
    command
}

fn assert_no_secrets(text: &str, secrets: &[&str]) {
    use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
    for secret in [
        ACCOUNT,
        PASSWORD,
        PATH_SECRET,
        DEVICE_TEXT,
        QUERY,
        ENCODED_PASSWORD,
    ]
    .into_iter()
    .chain(secrets.iter().copied())
    {
        assert!(
            !text.contains(secret)
                && !text.contains(&utf8_percent_encode(secret, NON_ALPHANUMERIC).to_string())
                && !percent_decode_str(text)
                    .decode_utf8_lossy()
                    .contains(secret),
            "share output contains a fixture canary"
        );
    }
}

fn report(output: Output, code: i32, secrets: &[&str]) -> Value {
    assert_eq!(output.status.code(), Some(code));
    assert_no_secrets(std::str::from_utf8(&output.stderr).unwrap(), secrets);
    assert!(output.stderr.is_empty());
    assert_no_secrets(std::str::from_utf8(&output.stdout).unwrap(), secrets);
    let parsed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_no_secrets(&parsed.to_string(), secrets);
    let schema: Value = serde_json::from_str(include_str!(
        "../schema/shareable-diagnostic-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(&parsed),
        "share report fails its schema: {parsed}"
    );
    assert_eq!(parsed["exit_code"], code);
    parsed
}

fn check<'a>(document: &'a Value, stage: &str) -> &'a Value {
    document["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["stage"] == stage)
        .unwrap()
}

#[test]
fn share_errors_do_not_echo_command_line_or_ca_path() {
    let temp = tempfile::tempdir().unwrap();
    for args in [
        vec!["diagnose", "--share", "--profile", ACCOUNT, "--unknown"],
        vec!["diagnose", "--share", "--timeout", PASSWORD],
        vec!["diagnose", "--share=bad", "--unknown"],
        vec!["config", "path", "--share"],
        vec![
            "diagnose", "--share", "--output", "jsonl", "--target", QUERY,
        ],
        vec!["diagnose", "--share", "--group", ACCOUNT],
    ] {
        let document = report(cli(temp.path()).args(args).output().unwrap(), 2, &[]);
        assert_eq!(document["primary_issue"]["reason"], "invalid_input");
        assert_eq!(document["complete"], false);
    }
    let ca = temp.path().join(PATH_SECRET);
    fs::write(
        &ca,
        format!("-----BEGIN PRIVATE KEY-----\n{PASSWORD}\n-----END PRIVATE KEY-----"),
    )
    .unwrap();
    let output = cli(temp.path())
        .args([
            "diagnose",
            "--share",
            "--target",
            "127.0.0.1",
            "--ca-certificate",
        ])
        .arg(&ca)
        .output()
        .unwrap();
    let document = report(output, 2, &["127.0.0.1"]);
    assert_eq!(document["primary_issue"]["reason"], "invalid_ca");
    let output = cli(temp.path())
        .args(["diagnose", "--", "--share"])
        .output()
        .unwrap();
    assert!(
        serde_json::from_slice::<Value>(&output.stdout).is_err(),
        "a positional value enabled share mode"
    );
}

#[test]
fn config_share_preserves_files_and_omits_paths_and_parser_input() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join(PATH_SECRET);
    let snapshots = directory.join("snapshots");
    fs::create_dir_all(&snapshots).unwrap();
    let orphan = snapshots.join(format!("{ACCOUNT}.json"));
    fs::write(&orphan, PASSWORD).unwrap();
    let registry = directory.join("devices.toml");
    for (source, code, reason) in [
        ("version = 3\n", 0, "unindexed_snapshots"),
        ("version = 999\n", 10, "registry_version"),
        ("version = [share-canary-password\n", 10, "registry_corrupt"),
    ] {
        fs::write(&registry, source).unwrap();
        let output = cli(&directory)
            .args(["config", "validate", "--share", "-vv"])
            .output()
            .unwrap();
        let document = report(output, code, &[]);
        if code == 0 {
            assert_eq!(document["outcome"], "completed_with_warnings");
            let orphaned = check(&document, "orphaned_snapshots");
            assert_eq!(orphaned["count"], 1);
            assert_eq!(orphaned["reason"], reason);
        } else {
            assert_eq!(document["primary_issue"]["reason"], reason);
            assert_eq!(check(&document, "indexed_snapshots")["status"], "blocked");
        }
        assert_eq!(fs::read_to_string(&registry).unwrap(), source);
        assert_eq!(fs::read_to_string(&orphan).unwrap(), PASSWORD);
    }
    let file = fs::File::create(&registry).unwrap();
    file.set_len(16 * 1024 * 1024 + 1).unwrap();
    let output = cli(&directory)
        .args(["config", "validate", "--share"])
        .output()
        .unwrap();
    assert_eq!(
        report(output, 10, &[])["primary_issue"]["reason"],
        "scan_limit"
    );
}

#[test]
fn share_checks_indexed_snapshots_without_exposing_their_identity() {
    let temp = tempfile::tempdir().unwrap();
    let registry = oxvif_cli::RegistryStore::at(temp.path());
    registry
        .save_discovery_snapshot(ACCOUNT, Vec::new())
        .unwrap();
    let file = temp
        .path()
        .join("snapshots")
        .join(format!("{ACCOUNT}.json"));
    let original = fs::read(&file).unwrap();
    let run = || {
        cli(temp.path())
            .args(["config", "validate", "--share"])
            .output()
            .unwrap()
    };
    assert_eq!(
        check(&report(run(), 0, &[]), "indexed_snapshots")["count"],
        1
    );
    for bad in [
        format!("{{{PASSWORD}"),
        String::from_utf8(original.clone())
            .unwrap()
            .replace(ACCOUNT, DEVICE_TEXT),
    ] {
        fs::write(&file, &bad).unwrap();
        let document = report(run(), 10, &[]);
        assert_eq!(document["primary_issue"]["stage"], "indexed_snapshots");
        assert_eq!(document["primary_issue"]["reason"], "registry_corrupt");
        assert_eq!(fs::read_to_string(&file).unwrap(), bad);
    }
    fs::remove_file(file).unwrap();
    let document = report(run(), 10, &[]);
    assert_eq!(document["primary_issue"]["stage"], "indexed_snapshots");
    assert_eq!(document["primary_issue"]["reason"], "snapshot_missing");
    assert_eq!(check(&document, "orphaned_snapshots")["status"], "blocked");
}

#[tokio::test]
async fn share_distinguishes_missing_credentials_and_unavailable_backend() {
    use oxvif_cli::{
        AppError, Application, CredentialStore, DiagnoseRequest, ExecutionOptions,
        MemoryCredentialStore, RegistryStore, SecretString, TargetSelector, share,
    };
    use std::sync::Arc;

    struct Unavailable;
    impl CredentialStore for Unavailable {
        fn set(&self, _: &str, _: &str) -> Result<(), AppError> {
            panic!("diagnose must not write credentials");
        }
        fn get(&self, _: &str) -> Result<Option<SecretString>, AppError> {
            Err(AppError::credential_unavailable(PASSWORD))
        }
        fn delete(&self, _: &str) -> Result<(), AppError> {
            panic!("diagnose must not delete credentials");
        }
    }
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("devices.toml"), format!(
        "version = 3\n[devices.{ACCOUNT}]\nname = \"{DEVICE_TEXT}\"\ntarget = \"http://127.0.0.1/\"\nusername = \"{ACCOUNT}\"\ncredential_ref = \"{QUERY}\"\n"
    )).unwrap();
    let backends: [(Arc<dyn CredentialStore>, _); 2] = [
        (
            Arc::new(MemoryCredentialStore::default()),
            "credential_missing",
        ),
        (Arc::new(Unavailable), "credential_store_unavailable"),
    ];
    for (credentials, reason) in backends {
        let application = Application::with_stores(RegistryStore::at(temp.path()), credentials);
        let mut report = share::Report::new(share::Command::Diagnose);
        let request = DiagnoseRequest {
            selector: TargetSelector {
                device: Some(ACCOUNT.into()),
                ..Default::default()
            },
            profile: None,
        };
        let code = application
            .diagnose_shared(request, &ExecutionOptions::default(), &mut report)
            .await;
        assert_eq!(code, 11);
        let mut bytes = Vec::new();
        report.write(code, &mut bytes).unwrap();
        assert_no_secrets(std::str::from_utf8(&bytes).unwrap(), &["127.0.0.1"]);
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["primary_issue"]["stage"], "credential_store");
        assert_eq!(parsed["primary_issue"]["reason"], reason);
        assert_eq!(check(&parsed, "onvif_session")["status"], "blocked");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnose_share_uses_same_checks_without_sharing_device_data() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    let temp = tempfile::tempdir().unwrap();
    let target = format!("{}?token={QUERY}", server.device_url());
    let diagnose = || {
        cli(temp.path())
            .args([
                "--media-service",
                "auto",
                "diagnose",
                "--share",
                "--json",
                "-vv",
                "--target",
                &target,
                "--profile",
                "Profile_1",
                "--clock-sync",
                "always",
            ])
            .env("OXVIF_USERNAME", ACCOUNT)
            .env("OXVIF_PASSWORD", PASSWORD)
            .output()
            .unwrap()
    };
    let secrets = [
        server.device_url(),
        "127.0.0.1",
        "Profile_1",
        "MockCam-1080p",
        "MOCK-0001",
    ];
    let passed = report(diagnose(), 0, &secrets);
    assert_eq!(passed["credentials_configured"], true);
    assert_eq!(passed["session_attempts"], 1);
    assert_eq!(check(&passed, "snapshot_fetch")["status"], "passed");
    assert_eq!(check(&passed, "snapshot_destination")["status"], "passed");
    assert_eq!(check(&passed, "snapshot_signature")["status"], "passed");
    assert_eq!(check(&passed, "rtsp_transport")["status"], "not_enabled");
    assert_eq!(check(&passed, "video_decode")["status"], "not_implemented");
    assert_eq!(passed["complete"], true);
    assert!(passed["primary_issue"].is_null());

    server.inject_fault("GetCapabilities", "s:Receiver", DEVICE_TEXT);
    let failed = report(diagnose(), 20, &secrets);
    assert_eq!(failed["primary_issue"]["stage"], "capabilities");
    assert_eq!(failed["primary_issue"]["reason"], "soap_fault");
    assert_eq!(check(&failed, "media_profiles")["status"], "blocked");
    assert_eq!(failed["complete"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn share_reports_http_auth_rejection_without_response_material() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!(
        "http://{}/onvif?token={QUERY}",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 8192];
        let _ = stream.read(&mut request).await.unwrap();
        stream.write_all(format!(
            "HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: {}\r\nX-Private: {ACCOUNT}\r\n\r\n{DEVICE_TEXT}",
            DEVICE_TEXT.len()
        ).as_bytes()).await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let output = cli(temp.path())
        .args(["diagnose", "--share", "--target", &target])
        .output()
        .unwrap();
    server.await.unwrap();
    let document = report(output, 20, &["127.0.0.1"]);
    assert_eq!(document["primary_issue"]["stage"], "capabilities");
    assert_eq!(
        document["primary_issue"]["reason"],
        "remote_authorization_rejected"
    );
    assert_eq!(document["primary_issue"]["http_status"], 403);
}

#[test]
fn share_canary_assertion_detects_a_leak() {
    assert!(std::panic::catch_unwind(|| assert_no_secrets(PASSWORD, &[])).is_err());
    assert!(std::panic::catch_unwind(|| assert_no_secrets(ENCODED_PASSWORD, &[])).is_err());
}

#[test]
fn share_rejects_embedded_credentials_and_missing_account_without_echoing_them() {
    let temp = tempfile::tempdir().unwrap();
    let url = format!("http://{ACCOUNT}:{PASSWORD}@127.0.0.1/onvif?token={QUERY}");
    let output = cli(temp.path())
        .args(["diagnose", "--share", "--target", &url])
        .output()
        .unwrap();
    assert_eq!(
        report(output, 2, &["127.0.0.1"])["primary_issue"]["stage"],
        "target"
    );
    let output = cli(temp.path())
        .args(["diagnose", "--share", "--target", "127.0.0.1"])
        .env("OXVIF_PASSWORD", PASSWORD)
        .output()
        .unwrap();
    let document = report(output, 11, &["127.0.0.1"]);
    assert_eq!(document["primary_issue"]["reason"], "credential_incomplete");
    assert_eq!(document["primary_issue"]["action"], "check_credential_pair");
    assert_eq!(check(&document, "snapshot_fetch")["status"], "blocked");
}

#[tokio::test]
async fn cancelled_diagnosis_produces_a_partial_report() {
    use oxvif_cli::{
        Application, DiagnoseRequest, ExecutionOptions, MemoryCredentialStore, RegistryStore,
        TargetSelector, share,
    };
    let directory = tempfile::tempdir().unwrap();
    let application = Application::with_stores(
        RegistryStore::at(directory.path()),
        std::sync::Arc::new(MemoryCredentialStore::default()),
    );
    let (_sender, receiver) = tokio::sync::watch::channel(true);
    let mut report = share::Report::new(share::Command::Diagnose);
    report.watch_cancellation(receiver);
    let code = application
        .diagnose_shared(
            DiagnoseRequest {
                selector: TargetSelector {
                    target: Some("http://127.0.0.1:1/onvif".into()),
                    ..Default::default()
                },
                profile: None,
            },
            &ExecutionOptions::default(),
            &mut report,
        )
        .await;
    assert_eq!(code, 130);
    let mut bytes = Vec::new();
    report.write(code, &mut bytes).unwrap();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed["complete"], false);
    assert_eq!(parsed["primary_issue"]["reason"], "cancelled");
    assert_eq!(parsed["primary_issue"]["action"], "retry");
}

#[cfg(unix)]
#[test]
fn unwritable_stdout_reports_a_fixed_message() {
    let temp = tempfile::tempdir().unwrap();
    let output = cli(temp.path())
        .args(["config", "validate", "--share"])
        .stdout(fs::File::open("/dev/null").unwrap())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(70));
    assert_eq!(output.stderr, b"Could not write diagnostic report.\n");
}
