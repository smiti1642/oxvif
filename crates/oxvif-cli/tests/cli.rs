use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

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
fn describe_json_has_stable_envelope() {
    let output = run(&["describe", "--output", "json", "--non-interactive"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["schema_version"], "1");
    assert_eq!(document["ok"], true);
    assert_eq!(document["data"]["kind"], "command_list");
    assert_eq!(document["data"]["commands"][0]["name"], "describe");
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
        "media.stream-uri",
        "--output",
        "json",
        "--non-interactive",
    ]);

    assert_eq!(output.status.code(), Some(3));
    let document: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(document["schema_version"], "1");
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
