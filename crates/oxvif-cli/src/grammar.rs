use std::collections::HashMap;

use clap::{Arg, ArgAction, Command, CommandFactory};
use oxvif_cli::{AppError, CommandDescriptor, CommandId, OutputFormat, SCHEMA_VERSION};
use serde::Serialize;
use serde_json::json;

use crate::Cli;

const GRAMMAR_VERSION: u32 = 1;

pub(crate) fn emit(format: OutputFormat) -> u8 {
    if format != OutputFormat::Json {
        let error = AppError::invalid_argument("describe --grammar requires --output json.");
        crate::emit_error(format, &error, Some("describe"));
        return error.exit_code();
    }
    println!(
        "{}",
        json!({
            "schema_version": SCHEMA_VERSION,
            "ok": true,
            "warnings": [],
            "meta": {"command": "describe", "elapsed_ms": 0},
            "data": {
                "kind": "command_grammar",
                "grammar_version": GRAMMAR_VERSION,
                "cli_version": env!("CARGO_PKG_VERSION"),
                "event_schema_version": SCHEMA_VERSION,
                "root": grammar(),
            },
        })
    );
    0
}

#[derive(Serialize)]
struct Node<'a> {
    name: &'a str,
    aliases: Vec<&'a str>,
    about: Option<String>,
    command_id: Option<&'static str>,
    description: Option<&'a CommandDescriptor>,
    console: Option<Console>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_terminal: Option<&'static str>,
    arguments: Vec<Argument<'a>>,
    subcommands: Vec<Node<'a>>,
}

#[derive(Serialize)]
struct Console {
    target_flag: Option<&'static str>,
    output: &'static str,
    control: Option<Control>,
    #[serde(skip_serializing_if = "Option::is_none")]
    share_report_version: Option<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    root_options: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    excluded_options: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_change: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_input: Option<&'static str>,
}

#[derive(Serialize)]
struct Control {
    kind: &'static str,
    flag: &'static str,
    stop_grace_ms: u32,
}

#[derive(Serialize)]
struct Argument<'a> {
    id: &'a str,
    long: Option<&'a str>,
    short: Option<char>,
    long_aliases: Vec<&'a str>,
    short_aliases: Vec<char>,
    position: Option<usize>,
    global: bool,
    required: bool,
    min_values: usize,
    max_values: Option<usize>,
    repeatable: bool,
    values: Vec<String>,
    defaults: Vec<String>,
    help: Option<String>,
    conflicts: Vec<&'a str>,
    value_source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    help_id: Option<&'static str>,
}

fn grammar() -> serde_json::Value {
    let descriptions = oxvif_cli::command_specs()
        .into_iter()
        .map(|spec| (spec.id.as_str(), spec.descriptor))
        .collect();
    let mut root = Cli::command();
    root.build();
    json!(node(&root, &mut Vec::new(), &descriptions))
}

fn node<'a>(
    command: &'a Command,
    path: &mut Vec<&'a str>,
    descriptions: &'a HashMap<&str, CommandDescriptor>,
) -> Node<'a> {
    let id = match path.as_slice() {
        ["discover"] => Some(CommandId::DiscoverScan),
        _ => CommandId::from_name(&path.join(".")),
    };
    let console = id.and_then(console);
    let arguments = command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set() && (path.is_empty() || !arg.is_global_set()))
        .map(|arg| argument(command, arg, id))
        .collect();
    let subcommands = command
        .get_subcommands()
        .filter(|child| !child.is_hide_set() && child.get_name() != "help")
        .map(|child| {
            path.push(child.get_name());
            let result = node(child, path, descriptions);
            path.pop();
            result
        })
        .collect();
    Node {
        name: command.get_name(),
        aliases: command.get_visible_aliases().collect(),
        about: command.get_about().map(ToString::to_string),
        command_id: id.map(CommandId::as_str),
        description: id.and_then(|id| descriptions.get(id.as_str())),
        console,
        external_terminal: (id == Some(CommandId::Manage)).then_some("manage_v1"),
        arguments,
        subcommands,
    }
}

fn console(id: CommandId) -> Option<Console> {
    use CommandId::*;

    let credential_input = matches!(
        id,
        Auth | DeviceCredentialSet | CredentialProfileSet | Inspect | Setup
    )
    .then_some("password_stdin_v1");
    let workspace_change = match id {
        DeviceAdd
        | DeviceUpdate
        | DeviceRename
        | DeviceRefresh
        | DeviceCredentialUseProfile
        | GroupCreate
        | GroupMemberAdd
        | ViewCreate
        | DiscoverRefresh
        | DiscoverEnrich
        | Auth
        | DeviceCredentialSet
        | CredentialProfileSet
        | Setup
        | Use => Some("write"),
        DeviceRemove
        | DeviceCredentialDelete
        | GroupDelete
        | GroupMemberRemove
        | ViewDelete
        | CredentialProfileDelete
        | DiscoverRemove => Some("remove"),
        _ => None,
    };
    let camera = match id {
        DeviceTest | DeviceInfo | DeviceCapabilities | DeviceServices | MediaProfiles
        | MediaStreamUri | MediaSnapshotUri | MediaSnapshotSave | PtzStatus | PtzPresets
        | HealthCheck | Diagnose | ConfigExport | ConfigDiff | EventsWatch => true,
        AgentGuide
        | AgentPrompt
        | List
        | Devices
        | Groups
        | Views
        | Info
        | Test
        | Profiles
        | Stream
        | Snapshot
        | DeviceList
        | DeviceShow
        | DeviceImport
        | Completion
        | GroupList
        | GroupShow
        | ViewList
        | ViewShow
        | ViewEvaluate
        | CredentialProfileList
        | CredentialProfileShow
        | DiscoverScan
        | DiscoverList
        | DiscoverSnapshots
        | ConfigPath
        | ConfigValidate
        | Describe
        | Inspect
        | Current
        | Health => false,
        _ if workspace_change.is_some() => false,
        _ => return None,
    };
    let watch = id == EventsWatch;
    let mut root_options = Vec::new();
    if camera || id == Inspect {
        root_options.extend(["clock_sync", "soap_auth", "ca_certificates"]);
    }
    if (camera && !watch) || matches!(id, Health | Info | Test | Profiles | Stream | Snapshot) {
        root_options.push("device");
    }
    if matches!(
        id,
        DeviceTest
            | DeviceInfo
            | DeviceCapabilities
            | DeviceServices
            | MediaProfiles
            | MediaStreamUri
            | MediaSnapshotUri
            | PtzStatus
            | PtzPresets
            | Health
            | HealthCheck
            | Diagnose
    ) {
        root_options.extend(["group", "view", "jobs"]);
    }
    if matches!(
        id,
        MediaProfiles
            | MediaStreamUri
            | MediaSnapshotUri
            | MediaSnapshotSave
            | Diagnose
            | ConfigExport
            | ConfigDiff
            | Stream
            | Snapshot
            | Profiles
            | Inspect
    ) {
        root_options.push("media_service");
    }
    if id == Diagnose {
        root_options.push("rtsp_probe");
    }
    if matches!(id, DiscoverScan | DiscoverRefresh | DiscoverEnrich) {
        root_options.push("timeout");
    }
    if id == Inspect {
        root_options.extend(["timeout", "retries"]);
    }
    Some(Console {
        target_flag: camera.then_some("--target"),
        output: if watch {
            "jsonl"
        } else if id == Completion {
            "table"
        } else {
            "json"
        },
        control: watch.then_some(Control {
            kind: "stdin_stop_v1",
            flag: "--control-stdin",
            // Default Create request, subscription cleanup, drain and final writes.
            stop_grace_ms: 20_000,
        }),
        share_report_version: matches!(id, Diagnose | ConfigValidate).then_some(1),
        root_options,
        // These options require a dedicated console flow or belong to the native CLI.
        excluded_options: match id {
            Inspect => vec!["password"],
            Snapshot => vec!["save"],
            Describe => vec!["grammar"],
            DeviceImport => vec!["overrides_stdin"],
            Completion => vec!["output", "json", "jsonl"],
            // The desktop coordinates its two event surfaces by normalized ONVIF URL.
            EventsWatch => vec!["id"],
            _ => Vec::new(),
        },
        workspace_change,
        credential_input,
    })
}

fn argument<'a>(command: &'a Command, arg: &'a Arg, id: Option<CommandId>) -> Argument<'a> {
    let range = arg
        .get_num_args()
        .expect("a built Clap argument has a value range");
    let max_values = range.max_values();
    Argument {
        id: arg.get_id().as_str(),
        long: arg.get_long(),
        short: arg.get_short(),
        long_aliases: arg.get_visible_aliases().unwrap_or_default(),
        short_aliases: arg.get_visible_short_aliases().unwrap_or_default(),
        position: arg.get_index(),
        global: arg.is_global_set(),
        required: arg.is_required_set(),
        min_values: range.min_values(),
        max_values: (max_values != usize::MAX).then_some(max_values),
        repeatable: matches!(arg.get_action(), ArgAction::Append | ArgAction::Count),
        values: arg
            .get_possible_values()
            .into_iter()
            .filter(|value| !value.is_hide_set())
            .map(|value| value.get_name().to_owned())
            .collect(),
        defaults: arg
            .get_default_values()
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect(),
        help: arg.get_help().map(ToString::to_string),
        conflicts: command
            .get_arg_conflicts_with(arg)
            .into_iter()
            .map(|arg| arg.get_id().as_str())
            .collect(),
        value_source: match (id, arg.get_id().as_str()) {
            (Some(CommandId::EventsWatch), "topic") => Some("event_topic"),
            (
                Some(
                    CommandId::MediaStreamUri
                    | CommandId::MediaSnapshotUri
                    | CommandId::MediaSnapshotSave
                    | CommandId::PtzStatus
                    | CommandId::PtzPresets
                    | CommandId::Diagnose,
                ),
                "profile",
            ) => Some("media_profile"),
            _ => None,
        },
        value_kind: match (id, arg.get_id().as_str()) {
            (_, "ca_certificates" | "against" | "overrides") => Some("input_file"),
            (
                Some(CommandId::ConfigExport | CommandId::MediaSnapshotSave | CommandId::Snapshot),
                "save",
            ) => Some("output_file"),
            (_, "duration" | "timeout") => Some("duration"),
            (_, "jobs") => Some("concurrency"),
            _ => None,
        },
        help_id: (id == Some(CommandId::DeviceImport) && arg.get_id().as_str() == "group")
            .then_some("import_group")
            .or_else(|| {
                (id == Some(CommandId::DiscoverScan) && arg.get_id().as_str() == "save")
                    .then_some("discovery_snapshot_name")
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde_json::Value;

    fn verify(node: &Value, path: &mut Vec<String>) {
        if node["console"].is_object() {
            let mut args: Vec<_> = path.iter().map(String::as_str).collect();
            if let Some(options) = node["console"]["root_options"].as_array() {
                for option in options {
                    assert!(matches!(
                        option.as_str(),
                        Some(
                            "clock_sync"
                                | "soap_auth"
                                | "ca_certificates"
                                | "media_service"
                                | "rtsp_probe"
                                | "device"
                                | "group"
                                | "view"
                                | "jobs"
                                | "timeout"
                                | "retries"
                        )
                    ));
                }
            }
            if let Some(flag) = node["console"]["target_flag"].as_str() {
                args.extend([flag, "http://[::1]:8080/custom"]);
            }
            let mut args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            if node["command_id"] == "device.import" {
                args.push("--plan".to_owned());
            }
            for argument in node["arguments"].as_array().unwrap() {
                if argument["required"] == true {
                    if let Some(long) = argument["long"].as_str() {
                        args.push(format!("--{long}"));
                        args.push(
                            if long == "filter" {
                                "tag=outdoor"
                            } else {
                                "sample"
                            }
                            .to_owned(),
                        );
                    } else {
                        args.push(
                            argument["values"]
                                .as_array()
                                .and_then(|values| values.first())
                                .and_then(Value::as_str)
                                .unwrap_or("sample-id")
                                .to_owned(),
                        );
                    }
                }
            }
            args.extend(
                [
                    "--non-interactive",
                    "--output",
                    node["console"]["output"].as_str().unwrap(),
                ]
                .map(str::to_owned),
            );
            if node["console"]["share_report_version"] == 1 {
                let mut share_args = args.clone();
                share_args.push("--share".to_owned());
                assert!(Cli::try_parse_from(share_args).unwrap().share);
            }
            let parsed = Cli::try_parse_from(args).unwrap();
            assert_eq!(
                node["command_id"],
                crate::surface_command_id(&parsed.command).as_str()
            );
            assert!(node["description"]["examples"].as_array().is_some());
        }
        for child in node["subcommands"].as_array().unwrap() {
            path.push(child["name"].as_str().unwrap().to_owned());
            verify(child, path);
            path.pop();
        }
    }

    #[test]
    fn exported_console_commands_match_the_actual_parser() {
        verify(&grammar(), &mut vec!["oxvif".to_owned()]);
    }
}
