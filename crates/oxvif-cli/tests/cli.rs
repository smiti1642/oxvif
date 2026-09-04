use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

use oxvif_cli::{DiscoveryRecord, NewDevice, NewGroup, RegistryStore};
use serde_json::Value;

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxvif"))
        .args(arguments)
        .output()
        .expect("oxvif binary should run")
}

fn run_isolated(arguments: &[&str], config_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxvif"))
        .args(arguments)
        .env("OXVIF_CONFIG_DIR", config_dir)
        .env_remove("OXVIF_DEVICE")
        .env_remove("OXVIF_USERNAME")
        .env_remove("OXVIF_PASSWORD")
        .output()
        .expect("oxvif binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

#[test]
fn describe_has_readable_terminal_output() {
    let output = run(&["describe"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("COMMAND"));
    assert!(stdout(&output).contains("describe"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn root_help_routes_agents_to_the_embedded_guide() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("AI AGENTS"));
    assert!(stdout(&output).contains("oxvif agent guide --output json"));
    assert!(stdout(&output).contains("--group"));
    assert!(stdout(&output).contains("--view"));
    assert!(stdout(&output).contains("--jobs"));
    assert!(stdout(&output).contains("--clock-sync"));
    assert!(stdout(&output).contains("--ca-certificate"));
    assert!(stdout(&output).contains("setup"));
    assert!(stdout(&output).contains("stream"));
    assert!(stdout(&output).contains("completion"));
}

#[test]
fn every_first_level_command_has_focused_help() {
    for command in [
        "setup",
        "auth",
        "info",
        "test",
        "health",
        "profiles",
        "stream",
        "snapshot",
        "list",
        "devices",
        "groups",
        "views",
        "agent",
        "describe",
        "device",
        "media",
        "ptz",
        "group",
        "view",
        "credential",
        "discover",
        "config",
        "completion",
        "use",
        "current",
    ] {
        let output = run(&[command, "--help"]);
        assert!(output.status.success(), "{command}: {}", stderr(&output));
        let help = stdout(&output);
        assert!(help.contains("Usage:"), "{command}: {help}");
        assert!(help.contains(command), "{command}: {help}");
    }
}

#[test]
fn setup_help_uses_target_first_onboarding() {
    let output = run(&["setup", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let help = stdout(&output);
    assert!(help.contains("setup [OPTIONS] [TARGET]"));
    assert!(help.contains("--id <ID>"));
    assert!(!help.contains("<ID> <TARGET>"));
}

#[test]
fn automated_setup_requires_target_and_explicit_id() {
    let directory = tempfile::tempdir().expect("temp directory");
    let without_id = Command::new(env!("CARGO_BIN_EXE_oxvif"))
        .args([
            "setup",
            "192.0.2.10",
            "--output",
            "json",
            "--non-interactive",
        ])
        .env("OXVIF_CONFIG_DIR", directory.path())
        .output()
        .expect("setup should run");
    assert_eq!(without_id.status.code(), Some(2));
    assert!(stdout(&without_id).contains("Provide --id"));

    let without_target = Command::new(env!("CARGO_BIN_EXE_oxvif"))
        .args(["setup", "--output", "json", "--non-interactive"])
        .env("OXVIF_CONFIG_DIR", directory.path())
        .output()
        .expect("setup should run");
    assert_eq!(without_target.status.code(), Some(2));
    assert!(stdout(&without_target).contains("requires an interactive terminal"));
}

#[test]
fn human_inventory_alias_and_json_shorthand_work_end_to_end() {
    let directory = tempfile::tempdir().expect("temp directory");
    RegistryStore::at(directory.path())
        .add(NewDevice {
            id: "front-door".to_owned(),
            name: Some("Front Door".to_owned()),
            target: "192.0.2.10".to_owned(),
            tags: Vec::new(),
        })
        .expect("device should add");

    let output = run_isolated(&["devices", "--json"], directory.path());
    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["schema_version"], "3");
    assert_eq!(document["data"]["kind"], "device_list");
    assert_eq!(document["data"]["devices"][0]["id"], "front-door");

    let list = run_isolated(&["list", "--json"], directory.path());
    assert!(list.status.success(), "{}", stderr(&list));
    let listed: Value = serde_json::from_slice(&list.stdout).expect("stdout should be JSON");
    assert_eq!(listed["schema_version"], document["schema_version"]);
    assert_eq!(listed["data"], document["data"]);
    assert_eq!(listed["meta"]["command"], "device.list");
}

#[test]
fn non_interactive_quick_command_does_not_use_current_device() {
    let directory = tempfile::tempdir().expect("temp directory");
    let registry = RegistryStore::at(directory.path());
    registry
        .add(NewDevice {
            id: "front-door".to_owned(),
            name: None,
            target: "192.0.2.10".to_owned(),
            tags: Vec::new(),
        })
        .expect("device should add");
    registry
        .set_current("front-door")
        .expect("current device should set");

    let output = run_isolated(&["info", "--json", "--non-interactive"], directory.path());
    assert_eq!(output.status.code(), Some(5));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["error"]["code"], "MISSING_TARGET");
}

#[test]
fn completion_is_generated_without_application_output_envelopes() {
    let output = run(&["completion", "powershell"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let script = stdout(&output);
    assert!(script.contains("Register-ArgumentCompleter"));
    assert!(!script.contains("schema_version"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn completion_rejects_structured_output_instead_of_ignoring_it() {
    let output = run(&["completion", "bash", "--output", "json"]);
    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
    assert!(
        document["error"]["message"]
            .as_str()
            .expect("message")
            .contains("raw shell script")
    );
}

#[test]
fn config_path_and_validate_are_structured_and_read_only() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = run_isolated(
        &["config", "path", "--output", "json", "--non-interactive"],
        directory.path(),
    );
    assert!(path.status.success(), "{}", stderr(&path));
    let document: Value = serde_json::from_slice(&path.stdout).expect("path should be JSON");
    assert_eq!(document["data"]["kind"], "config_status");
    assert_eq!(document["data"]["validated"], false);
    assert!(
        document["data"]["registry_file"]
            .as_str()
            .expect("registry path")
            .ends_with("devices.toml")
    );

    let validate = run_isolated(
        &[
            "config",
            "validate",
            "--output",
            "json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(validate.status.success(), "{}", stderr(&validate));
    let document: Value =
        serde_json::from_slice(&validate.stdout).expect("validation should be JSON");
    assert_eq!(document["data"]["validated"], true);
    assert_eq!(document["data"]["device_count"], 0);
    assert_eq!(document["data"]["snapshot_count"], 0);
}

#[test]
fn config_validate_refuses_a_corrupt_registry() {
    let directory = tempfile::tempdir().expect("temp directory");
    fs::write(directory.path().join("devices.toml"), "not = [valid toml")
        .expect("corrupt fixture should write");
    let output = run_isolated(
        &[
            "config",
            "validate",
            "--output",
            "json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert_eq!(output.status.code(), Some(10));
    let document: Value = serde_json::from_slice(&output.stdout).expect("error should be JSON");
    assert_eq!(document["error"]["code"], "REGISTRY_CORRUPT");
}

#[test]
fn config_validate_reports_orphaned_snapshots_without_deleting_them() {
    let directory = tempfile::tempdir().expect("temp directory");
    let snapshots = directory.path().join("snapshots");
    fs::create_dir_all(&snapshots).expect("snapshot directory should create");
    let orphan = snapshots.join("orphan.json");
    fs::write(&orphan, "{}").expect("orphan fixture should write");

    let output = run_isolated(
        &[
            "config",
            "validate",
            "--output",
            "json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(orphan.exists(), "validation must not delete an orphan");
    let document: Value = serde_json::from_slice(&output.stdout).expect("result should be JSON");
    assert_eq!(document["warnings"][0]["code"], "ORPHANED_SNAPSHOT_FILE");
    assert_eq!(
        document["data"]["orphaned_snapshot_files"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn fleet_jobs_require_a_bounded_set_selector() {
    let output = run(&[
        "--jobs",
        "3",
        "device",
        "capabilities",
        "--target",
        "127.0.0.1",
        "--output",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
    assert!(
        document["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--jobs")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fleet_jsonl_is_deterministic_and_partial_exit_is_six() {
    let server = oxvif::mock::MockServer::start()
        .await
        .expect("mock server should start");
    let directory = tempfile::tempdir().expect("temp directory");
    let registry = RegistryStore::at(directory.path());
    for (id, target) in [
        ("camera-a", server.device_url()),
        ("camera-b", "http://127.0.0.1:9/onvif/device_service"),
    ] {
        registry
            .add(NewDevice {
                id: id.to_owned(),
                name: None,
                target: target.to_owned(),
                tags: Vec::new(),
            })
            .expect("device should add");
    }
    registry
        .create_group(NewGroup {
            id: "fleet".to_owned(),
            name: None,
        })
        .expect("group should create");
    registry
        .add_group_member("fleet", "camera-b", "cam-002")
        .expect("member B should add");
    registry
        .add_group_member("fleet", "camera-a", "cam-001")
        .expect("member A should add");

    let output = run_isolated(
        &[
            "--timeout",
            "2s",
            "--group",
            "fleet",
            "--jobs",
            "2",
            "device",
            "capabilities",
            "--output",
            "jsonl",
            "--non-interactive",
        ],
        directory.path(),
    );

    assert_eq!(output.status.code(), Some(6), "{}", stderr(&output));
    let lines = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
        .collect::<Vec<_>>();
    let schema: Value = serde_json::from_str(include_str!("../schema/oxvif-envelope.schema.json"))
        .expect("envelope schema should be JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema should compile");
    assert!(lines.iter().all(|line| validator.is_valid(line)));
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["data"]["item"]["device_id"], "camera-a");
    assert_eq!(lines[1]["data"]["item"]["device_id"], "camera-b");
    assert_eq!(lines[2]["data"]["kind"], "fleet_summary");
    assert_eq!(lines[2]["data"]["succeeded"], 1);
    assert_eq!(lines[2]["data"]["failed"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn mock_human_diagnostics_use_purpose_built_renderers() {
    let server = oxvif::mock::MockServer::start()
        .await
        .expect("mock server should start");
    let target = server.device_url();
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["device", "capabilities", "--target", target],
            &["SERVICE | AVAILABLE | URL"],
        ),
        (
            &["device", "services", "--target", target],
            &["SERVICE | VERSION | URL"],
        ),
        (
            &["media", "profiles", "--target", target],
            &["TOKEN | NAME | FIXED | VIDEO | AUDIO | PTZ"],
        ),
        (
            &[
                "ptz",
                "status",
                "--profile",
                "Profile_1",
                "--target",
                target,
            ],
            &["POSITION | PAN", "MOVEMENT | PAN/TILT"],
        ),
        (
            &[
                "ptz",
                "presets",
                "--profile",
                "Profile_1",
                "--target",
                target,
            ],
            &["TOKEN | NAME | PAN | TILT | ZOOM"],
        ),
        (
            &["--timeout", "20s", "health", "check", "--target", target],
            &["Health:", "Passed:"],
        ),
    ];

    for (arguments, expected) in cases {
        let output = run(arguments);
        assert!(
            output.status.success(),
            "{:?}: {}",
            arguments,
            stderr(&output)
        );
        let rendered = stdout(&output);
        for fragment in *expected {
            assert!(rendered.contains(fragment), "{:?}: {rendered}", arguments);
        }
        assert!(
            !rendered.contains("Result:\n{"),
            "{:?}: {rendered}",
            arguments
        );
    }
}

#[test]
fn registry_subcommand_help_omits_connection_only_options() {
    let output = run(&["group", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let help = stdout(&output);
    assert!(!help.contains("--device"));
    assert!(!help.contains("--timeout"));
    assert!(!help.contains("--retries"));
}

#[test]
fn discovery_and_view_help_expose_fleet_controls() {
    let discovery = run(&["discover", "scan", "--help"]);
    assert!(discovery.status.success(), "{}", stderr(&discovery));
    let help = stdout(&discovery);
    assert!(help.contains("--interface"));
    assert!(help.contains("--save"));
    assert!(help.contains("--filter"));
    assert!(help.contains("--query"));

    let view = run(&["view", "evaluate", "--help"]);
    assert!(view.status.success(), "{}", stderr(&view));
    assert!(stdout(&view).contains("--explain"));

    let import = run(&["device", "import", "--help"]);
    assert!(import.status.success(), "{}", stderr(&import));
    let help = stdout(&import);
    assert!(help.contains("--expect-plan"));
    assert!(help.contains("--apply"));

    let enrich = run(&["discover", "enrich", "--help"]);
    assert!(enrich.status.success(), "{}", stderr(&enrich));
    assert!(stdout(&enrich).contains("--credential-profile"));
}

#[test]
fn discovery_list_exposes_and_filters_registration_status_for_agents() {
    let directory = tempfile::tempdir().expect("temp directory");
    let store = RegistryStore::at(directory.path());
    store
        .add(NewDevice {
            id: "saved-camera".to_owned(),
            name: Some("Saved camera".to_owned()),
            target: "http://192.0.2.120/onvif/device_service".to_owned(),
            tags: Vec::new(),
        })
        .expect("device should save");
    let record = |suffix: u8| DiscoveryRecord {
        endpoint: format!("urn:uuid:camera-{suffix}"),
        types: Vec::new(),
        scopes: (suffix == 121)
            .then(|| "onvif://www.onvif.org/location/loading-dock".to_owned())
            .into_iter()
            .collect(),
        xaddrs: vec![format!("http://192.0.2.{suffix}/onvif/device_service")],
        manufacturer: None,
        model: None,
        firmware_version: None,
        serial_number: None,
    };
    store
        .save_discovery_snapshot("scan", vec![record(120), record(121)])
        .expect("snapshot should save");

    let saved = run_isolated(
        &[
            "discover",
            "list",
            "scan",
            "--filter",
            "registration=saved",
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(saved.status.success(), "{}", stderr(&saved));
    let saved: Value = serde_json::from_slice(&saved.stdout).expect("saved list should be JSON");
    assert_eq!(saved["data"]["snapshot"]["summary"]["total_count"], 2);
    assert_eq!(saved["data"]["snapshot"]["summary"]["matched_count"], 1);
    assert_eq!(
        saved["data"]["snapshot"]["devices"][0]["registration_status"],
        "saved"
    );
    assert_eq!(
        saved["data"]["snapshot"]["devices"][0]["registered_device_id"],
        "saved-camera"
    );

    let unregistered = run_isolated(
        &[
            "discover",
            "list",
            "scan",
            "--filter",
            "registration=unregistered",
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(unregistered.status.success(), "{}", stderr(&unregistered));
    let unregistered: Value =
        serde_json::from_slice(&unregistered.stdout).expect("new list should be JSON");
    assert_eq!(
        unregistered["data"]["snapshot"]["devices"][0]["registration_status"],
        "new"
    );
    assert!(
        unregistered["data"]["snapshot"]["devices"][0]
            .get("registered_device_id")
            .is_none()
    );

    let queried = run_isolated(
        &[
            "discover",
            "list",
            "scan",
            "--query",
            "LOADING-DOCK",
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(queried.status.success(), "{}", stderr(&queried));
    let queried: Value =
        serde_json::from_slice(&queried.stdout).expect("queried list should be JSON");
    assert_eq!(queried["data"]["snapshot"]["summary"]["matched_count"], 1);
    assert_eq!(
        queried["data"]["snapshot"]["devices"][0]["endpoint"],
        "urn:uuid:camera-121"
    );
}

#[test]
fn describe_json_has_stable_envelope() {
    let output = run(&["describe", "--output", "json", "--non-interactive"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["schema_version"], "3");
    assert_eq!(document["ok"], true);
    assert_eq!(document["data"]["kind"], "command_list");
    assert_eq!(document["data"]["commands"][0]["name"], "agent.guide");
    assert_eq!(document["data"]["commands"][0]["risk"], "read");
    assert_eq!(document["meta"]["command"], "describe");
    assert!(stderr(&output).is_empty());
}

#[test]
fn published_schemas_validate_success_error_and_all_descriptors() {
    let envelope_schema: Value =
        serde_json::from_str(include_str!("../schema/oxvif-envelope.schema.json"))
            .expect("envelope schema should be JSON");
    let envelope = jsonschema::validator_for(&envelope_schema).expect("schema should compile");

    let success = run(&["describe", "--output", "json", "--non-interactive"]);
    assert!(success.status.success(), "{}", stderr(&success));
    let success_document: Value =
        serde_json::from_slice(&success.stdout).expect("success should be JSON");
    assert!(envelope.is_valid(&success_document));

    let failure = run(&[
        "describe",
        "not.a.command",
        "--output",
        "json",
        "--non-interactive",
    ]);
    assert_eq!(failure.status.code(), Some(3));
    let failure_document: Value =
        serde_json::from_slice(&failure.stdout).expect("failure should be JSON");
    assert!(envelope.is_valid(&failure_document));

    let descriptor_schema: Value =
        serde_json::from_str(include_str!("../schema/command-descriptor.schema.json"))
            .expect("descriptor schema should be JSON");
    let descriptor = jsonschema::validator_for(&descriptor_schema).expect("schema should compile");
    for command in success_document["data"]["commands"]
        .as_array()
        .expect("commands")
    {
        assert!(
            descriptor.is_valid(command),
            "invalid descriptor: {command}"
        );
    }

    let mut wrong_version = success_document;
    wrong_version["schema_version"] = Value::String("999".to_owned());
    assert!(!envelope.is_valid(&wrong_version));
}

#[test]
fn verbose_diagnostics_use_stderr_without_corrupting_json_stdout() {
    let output = run(&[
        "-vv",
        "--retries",
        "2",
        "--clock-sync",
        "never",
        "describe",
        "describe",
        "--output",
        "json",
        "--non-interactive",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["ok"], true);
    let diagnostics = stderr(&output);
    assert!(diagnostics.contains("command=describe"));
    assert!(diagnostics.contains("output=json"));
    assert!(diagnostics.contains("max_attempts=3"));
    assert!(diagnostics.contains("clock_sync=never"));
    assert!(diagnostics.contains("status=ok"));
    assert!(!diagnostics.contains("password"));
    assert!(!diagnostics.contains("authorization"));
}

#[test]
fn verbose_diagnostics_reject_url_credentials_without_exposing_secrets() {
    let directory = tempfile::tempdir().expect("temp directory");
    for verbosity in ["-v", "-vv"] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxvif"))
            .args([
                verbosity,
                "setup",
                "leak-test",
                "http://url-user:url-secret@127.0.0.1/onvif/device_service",
                "--username",
                "environment-user",
                "--no-verify",
                "--non-interactive",
                "--output",
                "json",
            ])
            .env("OXVIF_CONFIG_DIR", directory.path())
            .env("OXVIF_PASSWORD", "environment-secret")
            .output()
            .expect("oxvif binary should run");

        assert_eq!(output.status.code(), Some(2));
        let combined = format!("{}\n{}", stdout(&output), stderr(&output));
        for secret in [
            "url-user",
            "url-secret",
            "environment-user",
            "environment-secret",
            "PasswordDigest",
            "Authorization",
        ] {
            assert!(
                !combined.contains(secret),
                "{verbosity} diagnostics exposed {secret}: {combined}"
            );
        }
        let document: Value =
            serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
        assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
    }
}

#[test]
fn ca_certificate_inputs_fail_early_without_exposing_file_contents() {
    let directory = tempfile::tempdir().expect("temp directory");
    let malformed = directory.path().join("malformed-ca.pem");
    fs::write(&malformed, "not a certificate").expect("fixture should write");
    let private_key = directory.path().join("private-key.pem");
    fs::write(
        &private_key,
        "-----BEGIN PRIVATE KEY-----\ndo-not-expose-this\n-----END PRIVATE KEY-----",
    )
    .expect("fixture should write");
    let missing = directory.path().join("missing-ca.pem");

    for path in [missing, malformed, private_key] {
        let output = run_isolated(
            &[
                "--ca-certificate",
                path.to_str().expect("fixture path should be UTF-8"),
                "describe",
                "--output",
                "json",
                "--non-interactive",
            ],
            directory.path(),
        );
        assert_eq!(output.status.code(), Some(2));
        let document: Value =
            serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
        assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
        let combined = format!("{}\n{}", stdout(&output), stderr(&output));
        assert!(!combined.contains("do-not-expose-this"));
    }
}

#[test]
fn describes_one_command() {
    let output = run(&["--output=json", "describe", "describe"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["data"]["kind"], "command_description");
    assert_eq!(document["data"]["command"]["name"], "describe");
    assert_eq!(document["data"]["command"]["mutates_device"], false);
}

#[test]
fn unknown_described_command_is_structured_error() {
    let output = run(&[
        "describe",
        "device.factory-reset",
        "--output",
        "json",
        "--non-interactive",
    ]);

    assert_eq!(output.status.code(), Some(3));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["schema_version"], "3");
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "COMMAND_NOT_FOUND");
    assert_eq!(document["error"]["retryable"], false);
    assert_eq!(document["meta"]["command"], "describe");
    assert!(stderr(&output).is_empty());
}

#[test]
fn argument_parser_errors_are_json_when_requested() {
    let output = run(&["--output", "json", "--timeout", "later", "describe"]);

    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
    assert!(stderr(&output).is_empty());
}

#[test]
fn jsonl_is_one_complete_line() {
    let output = run(&["describe", "--output", "jsonl"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let rendered = stdout(&output);
    assert_eq!(rendered.lines().count(), 1);
    let document: Value = serde_json::from_str(rendered.trim()).expect("line should be JSON");
    assert_eq!(document["ok"], true);
}

#[test]
fn agent_guide_and_prompt_are_embedded_and_versioned() {
    let guide = run(&["agent", "guide", "--output", "json", "--non-interactive"]);
    assert!(guide.status.success(), "{}", stderr(&guide));
    let document: Value = serde_json::from_slice(&guide.stdout).expect("guide should be JSON");
    assert_eq!(document["schema_version"], "3");
    assert_eq!(document["data"]["kind"], "agent_guide");
    assert_eq!(document["data"]["guide"]["guide_version"], "5");
    assert!(
        document["data"]["guide"]["security_requirements"]
            .as_array()
            .is_some_and(|rules| !rules.is_empty())
    );

    let prompt = run(&["agent", "prompt"]);
    assert!(prompt.status.success(), "{}", stderr(&prompt));
    assert!(stdout(&prompt).contains("--non-interactive"));
}

#[test]
fn irrelevant_root_device_selector_is_rejected() {
    let directory = tempfile::tempdir().expect("temp directory");
    let output = run_isolated(
        &["--device", "camera", "group", "list", "--output=json"],
        directory.path(),
    );
    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("error should be JSON");
    assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
}

#[test]
fn named_device_registry_round_trips_through_cli() {
    let directory = tempfile::tempdir().expect("temp directory");

    let add = run_isolated(
        &[
            "device",
            "add",
            "front-door",
            "--name",
            "Front Door",
            "--target",
            "192.168.1.20",
            "--tag",
            "outdoor",
            "--output",
            "json",
        ],
        directory.path(),
    );
    assert!(add.status.success(), "{}", stderr(&add));
    let added: Value = serde_json::from_slice(&add.stdout).expect("add should return JSON");
    assert_eq!(added["data"]["kind"], "device_record");
    assert_eq!(added["data"]["device"]["id"], "front-door");
    assert_eq!(
        added["data"]["device"]["target"],
        "http://192.168.1.20/onvif/device_service"
    );

    let select = run_isolated(&["use", "front-door"], directory.path());
    assert!(select.status.success(), "{}", stderr(&select));

    let current = run_isolated(&["current", "--output", "json"], directory.path());
    assert!(current.status.success(), "{}", stderr(&current));
    let current: Value =
        serde_json::from_slice(&current.stdout).expect("current should return JSON");
    assert_eq!(current["data"]["device"]["id"], "front-door");

    let rename = run_isolated(
        &["device", "rename", "front-door", "--name", "Main Entrance"],
        directory.path(),
    );
    assert!(rename.status.success(), "{}", stderr(&rename));

    let list = run_isolated(&["device", "list", "--output=json"], directory.path());
    assert!(list.status.success(), "{}", stderr(&list));
    let list: Value = serde_json::from_slice(&list.stdout).expect("list should return JSON");
    assert_eq!(list["data"]["devices"][0]["name"], "Main Entrance");
    assert_eq!(list["data"]["current_device"], "front-door");
    assert_eq!(list["data"]["devices"][0]["has_credentials"], false);

    let registry = fs::read_to_string(directory.path().join("devices.toml"))
        .expect("registry should be readable");
    assert!(!registry.to_ascii_lowercase().contains("password"));

    let remove = run_isolated(&["device", "remove", "front-door"], directory.path());
    assert!(remove.status.success(), "{}", stderr(&remove));
    let current = run_isolated(&["current", "--output=json"], directory.path());
    let current: Value =
        serde_json::from_slice(&current.stdout).expect("current should return JSON");
    assert!(current["data"]["device"].is_null());
}

#[test]
fn invalid_device_id_is_a_structured_error() {
    let directory = tempfile::tempdir().expect("temp directory");
    let output = run_isolated(
        &[
            "device",
            "add",
            "Front Door",
            "--target",
            "192.168.1.20",
            "--output",
            "json",
        ],
        directory.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("error should be JSON");
    assert_eq!(document["error"]["code"], "INVALID_ARGUMENT");
    assert!(!directory.path().join("devices.toml").exists());
}

#[test]
fn concurrent_registry_writers_do_not_lose_devices() {
    let directory = tempfile::tempdir().expect("temp directory");
    let mut children = Vec::new();
    for index in 0..8 {
        let id = format!("camera-{index}");
        let target = format!("192.168.1.{}", index + 20);
        let child = Command::new(env!("CARGO_BIN_EXE_oxvif"))
            .args(["device", "add", &id, "--target", &target])
            .env("OXVIF_CONFIG_DIR", directory.path())
            .env_remove("OXVIF_DEVICE")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("concurrent oxvif process should start");
        children.push(child);
    }
    for mut child in children {
        assert!(child.wait().expect("child should finish").success());
    }

    let list = run_isolated(&["device", "list", "--output=json"], directory.path());
    assert!(list.status.success(), "{}", stderr(&list));
    let list: Value = serde_json::from_slice(&list.stdout).expect("list should return JSON");
    assert_eq!(list["data"]["devices"].as_array().unwrap().len(), 8);
    let registry = fs::read_to_string(directory.path().join("devices.toml"))
        .expect("registry should remain readable");
    assert_eq!(registry.matches("[devices.camera-").count(), 8);
}

#[test]
fn group_alias_and_dynamic_view_work_through_cli() {
    let directory = tempfile::tempdir().expect("temp directory");
    for (id, ip, tag) in [
        ("global-a", "192.168.20.21", "outdoor"),
        ("global-b", "192.168.20.22", "indoor"),
    ] {
        let output = run_isolated(
            &["device", "add", id, "--target", ip, "--tag", tag],
            directory.path(),
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }

    let group = run_isolated(
        &["group", "create", "taipei-f1", "--name", "Taipei F1"],
        directory.path(),
    );
    assert!(group.status.success(), "{}", stderr(&group));
    let member = run_isolated(
        &[
            "group",
            "member",
            "add",
            "taipei-f1",
            "global-a",
            "--alias",
            "cam-023",
        ],
        directory.path(),
    );
    assert!(member.status.success(), "{}", stderr(&member));

    let selected = run_isolated(&["use", "taipei-f1/cam-023"], directory.path());
    assert!(selected.status.success(), "{}", stderr(&selected));
    let current = run_isolated(&["current", "--output=json"], directory.path());
    let current: Value = serde_json::from_slice(&current.stdout).expect("current should be JSON");
    assert_eq!(current["data"]["device"]["id"], "global-a");

    let view = run_isolated(
        &["view", "create", "outdoor", "--filter", "tag=outdoor"],
        directory.path(),
    );
    assert!(view.status.success(), "{}", stderr(&view));
    let evaluated = run_isolated(
        &["view", "evaluate", "outdoor", "--output=json"],
        directory.path(),
    );
    assert!(evaluated.status.success(), "{}", stderr(&evaluated));
    let evaluated: Value =
        serde_json::from_slice(&evaluated.stdout).expect("View result should be JSON");
    assert_eq!(evaluated["data"]["kind"], "view_evaluation");
    assert_eq!(evaluated["data"]["devices"].as_array().unwrap().len(), 1);
    assert_eq!(evaluated["data"]["devices"][0]["id"], "global-a");

    let remove = run_isolated(&["device", "remove", "global-a"], directory.path());
    assert!(remove.status.success(), "{}", stderr(&remove));
    let group = run_isolated(
        &["group", "show", "taipei-f1", "--output=json"],
        directory.path(),
    );
    let group: Value = serde_json::from_slice(&group.stdout).expect("Group should be JSON");
    assert_eq!(
        group["data"]["group"]["members"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn device_import_cli_requires_and_applies_reviewed_fingerprint() {
    let directory = tempfile::tempdir().expect("temp directory");
    let store = RegistryStore::at(directory.path());
    store
        .save_discovery_snapshot(
            "scan",
            vec![DiscoveryRecord {
                endpoint: "uuid:cli-camera".to_owned(),
                types: Vec::new(),
                scopes: vec!["onvif://www.onvif.org/name/CLI%20Camera".to_owned()],
                xaddrs: vec!["http://192.0.2.120/onvif/device_service".to_owned()],
                manufacturer: None,
                model: None,
                firmware_version: None,
                serial_number: None,
            }],
        )
        .expect("snapshot should save");

    let plan = run_isolated(
        &[
            "device",
            "import",
            "--from",
            "scan",
            "--filter",
            "endpoint=uuid:cli-camera",
            "--plan",
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(plan.status.success(), "{}", stderr(&plan));
    let document: Value = serde_json::from_slice(&plan.stdout).expect("plan should be JSON");
    assert_eq!(document["data"]["kind"], "device_import");
    assert_eq!(document["data"]["plan"]["create_count"], 1);
    assert!(store.list().expect("registry should load").0.is_empty());
    let fingerprint = document["data"]["plan"]["fingerprint"]
        .as_str()
        .expect("fingerprint should exist")
        .to_owned();

    let stale = run_isolated(
        &[
            "device",
            "import",
            "--from",
            "scan",
            "--filter",
            "endpoint=uuid:cli-camera",
            "--apply",
            "--expect-plan",
            "sha256:stale",
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert_eq!(stale.status.code(), Some(4));
    let stale: Value = serde_json::from_slice(&stale.stdout).expect("error should be JSON");
    assert_eq!(stale["error"]["code"], "IMPORT_PLAN_MISMATCH");
    assert!(store.list().expect("registry should load").0.is_empty());

    let apply = run_isolated(
        &[
            "device",
            "import",
            "--from",
            "scan",
            "--filter",
            "endpoint=uuid:cli-camera",
            "--apply",
            "--expect-plan",
            &fingerprint,
            "--output=json",
            "--non-interactive",
        ],
        directory.path(),
    );
    assert!(apply.status.success(), "{}", stderr(&apply));
    let document: Value = serde_json::from_slice(&apply.stdout).expect("apply should be JSON");
    assert_eq!(document["data"]["applied"], true);
    assert_eq!(document["data"]["devices"][0]["id"], "cam-cli-camera");
    assert_eq!(store.list().expect("registry should load").0.len(), 1);

    let missing_fingerprint = run_isolated(
        &[
            "device",
            "import",
            "--from",
            "scan",
            "--filter",
            "endpoint=uuid:cli-camera",
            "--apply",
            "--output=json",
        ],
        directory.path(),
    );
    assert_eq!(missing_fingerprint.status.code(), Some(2));
}
