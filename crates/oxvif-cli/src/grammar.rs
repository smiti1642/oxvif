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
    arguments: Vec<Argument<'a>>,
    subcommands: Vec<Node<'a>>,
}

#[derive(Serialize)]
struct Console {
    target_flag: &'static str,
    output: &'static str,
    control: Option<Control>,
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
    let console = match id {
        Some(CommandId::DeviceInfo | CommandId::MediaProfiles) => Some(Console {
            target_flag: "--target",
            output: "json",
            control: None,
        }),
        Some(CommandId::EventsWatch) => Some(Console {
            target_flag: "--target",
            output: "jsonl",
            control: Some(Control {
                kind: "stdin_stop_v1",
                flag: "--control-stdin",
                // Default Create request, subscription cleanup, drain and final writes.
                stop_grace_ms: 20_000,
            }),
        }),
        _ => None,
    };
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
        arguments,
        subcommands,
    }
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
            (Some(CommandId::MediaStreamUri | CommandId::MediaSnapshotUri), "profile") => {
                Some("media_profile")
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde_json::Value;

    fn verify(node: &Value, path: &mut Vec<String>) {
        if node["console"].is_object() {
            let args = path.iter().map(String::as_str).chain([
                node["console"]["target_flag"].as_str().unwrap(),
                "http://[::1]:8080/custom",
                "--non-interactive",
                "--output",
                node["console"]["output"].as_str().unwrap(),
            ]);
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
