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
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["data"]["item"]["device_id"], "camera-a");
    assert_eq!(lines[1]["data"]["item"]["device_id"], "camera-b");
    assert_eq!(lines[2]["data"]["kind"], "fleet_summary");
    assert_eq!(lines[2]["data"]["succeeded"], 1);
    assert_eq!(lines[2]["data"]["failed"], 1);
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
    assert_eq!(document["data"]["guide"]["guide_version"], "3");
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
