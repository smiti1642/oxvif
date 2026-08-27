use std::process::{Command, Output};

use serde_json::Value;

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxvif"))
        .args(arguments)
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
