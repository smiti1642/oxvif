use crate::{
    AppError, ArgumentDescriptor, CommandData, CommandDescriptor, CommandId, CommandSpec,
    DescribeRequest, OutputDescriptor, RiskLevel,
};

pub(crate) fn execute(request: DescribeRequest) -> Result<CommandData, AppError> {
    let commands = descriptors();

    match request.command {
        Some(name) => commands
            .into_iter()
            .find(|command| command.name == name)
            .map(|command| CommandData::CommandDescription { command })
            .ok_or_else(|| AppError::command_not_found(&name)),
        None => Ok(CommandData::CommandList { commands }),
    }
}

pub(crate) fn descriptors() -> Vec<CommandDescriptor> {
    specs().into_iter().map(|spec| spec.descriptor).collect()
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    let commands = vec![
        read_descriptor(
            "agent.guide",
            "Return version-matched Agent operation and security rules.",
        ),
        read_descriptor(
            "agent.prompt",
            "Return a compact prompt for an Agent operating oxvif.",
        ),
        describe_descriptor(),
        read_descriptor("config.path", "Return resolved local registry paths."),
        read_descriptor(
            "config.validate",
            "Parse and validate the registry and retained discovery snapshots.",
        ),
        descriptor(
            "setup",
            "Securely register, authenticate, verify, and select one device.",
            RiskLevel::Write,
            true,
            false,
            vec![
                required("id", "string"),
                required("target", "url | host"),
                optional("name", "string"),
                optional("tag", "string[]"),
                optional("username", "string"),
                optional("password-stdin", "boolean"),
                optional("no-verify", "boolean"),
                optional("no-use", "boolean"),
            ],
        ),
        local_write_descriptor(
            "auth",
            "Securely set a saved device credential, prompting in an interactive terminal.",
            vec![
                required("id", "string"),
                optional("username", "string"),
                optional("password-stdin", "boolean"),
            ],
        ),
        device_read_descriptor("info", "Human shortcut for device.info."),
        device_read_descriptor("test", "Human shortcut for device.test."),
        device_read_descriptor("health", "Human shortcut for health.check."),
        device_read_descriptor("profiles", "Human shortcut for media.profiles."),
        quick_profile_descriptor("stream", "Human shortcut for media.stream-uri."),
        quick_profile_descriptor("snapshot", "Human shortcut for media.snapshot-uri."),
        read_descriptor("devices", "Human shortcut for device.list."),
        read_descriptor("groups", "Human shortcut for group.list."),
        read_descriptor("views", "Human shortcut for view.list."),
        descriptor(
            "completion",
            "Generate a completion script for Bash, Zsh, Fish, or PowerShell.",
            RiskLevel::Read,
            false,
            false,
            vec![argument_with_values(
                "shell",
                "string",
                true,
                &["bash", "zsh", "fish", "powershell"],
            )],
        ),
        registry_descriptor(
            "device.add",
            "Save a new device under an immutable machine-safe ID.",
            vec![
                required("id", "string"),
                required("target", "url | host"),
                optional("name", "string"),
                optional("tag", "string[]"),
            ],
        ),
        read_descriptor(
            "device.list",
            "List saved devices and the current selection.",
        ),
        local_read_descriptor(
            "device.show",
            "Show one saved device without revealing its password.",
            vec![required("id", "string")],
        ),
        registry_descriptor(
            "device.update",
            "Update a saved device target, display name, or tags.",
            vec![
                required("id", "string"),
                optional("target", "url | host"),
                optional("name", "string"),
                optional("tag", "string[]"),
                optional("clear-tags", "boolean"),
            ],
        ),
        registry_descriptor(
            "device.rename",
            "Change a device display name without changing its immutable ID.",
            vec![required("id", "string"), required("name", "string")],
        ),
        registry_descriptor(
            "device.remove",
            "Remove a saved device and its stored credential.",
            vec![required("id", "string")],
        ),
        import_descriptor(),
        local_write_descriptor(
            "device.credential.set",
            "Store a device password in the native OS credential store.",
            vec![
                required("id", "string"),
                optional("username", "string"),
                optional("password-stdin", "boolean"),
            ],
        ),
        credential_descriptor(
            "device.credential.delete",
            "Delete a device password from the native OS credential store.",
        ),
        local_write_descriptor(
            "device.credential.use-profile",
            "Assign a reusable credential profile to a saved device.",
            vec![required("id", "string"), required("profile", "string")],
        ),
        local_write_descriptor(
            "credential.profile.set",
            "Create or update a reusable native credential profile.",
            vec![
                required("id", "string"),
                optional("username", "string"),
                optional("password-stdin", "boolean"),
            ],
        ),
        read_descriptor(
            "credential.profile.list",
            "List credential profiles without exposing secrets.",
        ),
        local_read_descriptor(
            "credential.profile.show",
            "Show one credential profile without exposing its secret.",
            vec![required("id", "string")],
        ),
        credential_descriptor(
            "credential.profile.delete",
            "Delete an unused credential profile and its native secret.",
        ),
        registry_descriptor(
            "group.create",
            "Create an empty static device Group.",
            vec![required("id", "string"), optional("name", "string")],
        ),
        read_descriptor("group.list", "List static device Groups."),
        local_read_descriptor(
            "group.show",
            "Show a Group and its explicit members.",
            vec![required("id", "string")],
        ),
        registry_descriptor(
            "group.delete",
            "Delete a Group without deleting its member devices.",
            vec![required("id", "string")],
        ),
        registry_descriptor(
            "group.member.add",
            "Add one canonical device under a Group-local alias.",
            vec![
                required("group_id", "string"),
                required("device_id", "string"),
                required("alias", "string"),
            ],
        ),
        registry_descriptor(
            "group.member.remove",
            "Remove one member by its Group-local alias.",
            vec![required("group_id", "string"), required("alias", "string")],
        ),
        registry_descriptor(
            "view.create",
            "Create a dynamic View from typed device filters.",
            vec![
                required("id", "string"),
                optional("name", "string"),
                argument_with_values(
                    "filter",
                    "field[:operator]=value[]",
                    true,
                    &[
                        "id",
                        "name",
                        "target",
                        "uuid",
                        "manufacturer",
                        "model",
                        "firmware",
                        "serial",
                        "tag",
                        "ip-cidr",
                        "eq",
                        "neq",
                        "contains",
                        "prefix",
                        "in",
                    ],
                ),
                argument_with_values("match", "string", false, &["all", "any"]),
            ],
        ),
        read_descriptor("view.list", "List dynamic Views."),
        local_read_descriptor(
            "view.show",
            "Show one dynamic View definition.",
            vec![required("id", "string")],
        ),
        descriptor(
            "view.evaluate",
            "Evaluate a View against current registered-device metadata.",
            RiskLevel::Read,
            false,
            false,
            vec![required("id", "string"), optional("explain", "boolean")],
        ),
        registry_descriptor(
            "view.delete",
            "Delete a dynamic View.",
            vec![required("id", "string")],
        ),
        descriptor(
            "discover.scan",
            "Run ephemeral WS-Discovery; optionally save a named snapshot.",
            RiskLevel::Write,
            false,
            false,
            vec![
                optional("save", "string"),
                optional("interface", "interface name | IPv4[]"),
            ],
        ),
        descriptor(
            "discover.refresh",
            "Re-run discovery and atomically replace an existing named snapshot.",
            RiskLevel::Write,
            false,
            false,
            vec![
                required("snapshot", "string"),
                optional("interface", "interface name | IPv4[]"),
            ],
        ),
        enrich_descriptor(),
        read_descriptor("discover.snapshots", "List named discovery snapshots."),
        read_descriptor(
            "discover.list",
            "List and filter records in one discovery snapshot.",
        ),
        registry_descriptor(
            "discover.remove",
            "Remove one named discovery snapshot.",
            vec![required("snapshot", "string")],
        ),
        registry_descriptor(
            "use",
            "Select the current device for interactive human commands.",
            vec![required("id", "string")],
        ),
        read_descriptor("current", "Show the current interactive device selection."),
        device_read_descriptor("device.test", "Verify connectivity and authentication."),
        device_read_descriptor("device.info", "Read live ONVIF device information."),
        device_read_descriptor(
            "device.capabilities",
            "Read the device's advertised ONVIF capabilities.",
        ),
        device_read_descriptor(
            "device.services",
            "List all ONVIF service endpoints advertised by the device.",
        ),
        device_read_descriptor("media.profiles", "List Media1 profiles."),
        profile_read_descriptor(
            "media.stream-uri",
            "Get the credential-sanitized RTSP URI for one media profile.",
        ),
        profile_read_descriptor(
            "media.snapshot-uri",
            "Get the credential-sanitized snapshot URI for one media profile.",
        ),
        profile_read_descriptor(
            "ptz.status",
            "Read current PTZ position and movement state.",
        ),
        profile_read_descriptor(
            "ptz.presets",
            "List stored PTZ presets without moving the camera.",
        ),
        device_read_descriptor(
            "health.check",
            "Run default read-only health and conformance checks.",
        ),
        local_read_descriptor(
            "device.refresh",
            "Read live device information and update cached registry metadata.",
            vec![required("id", "string")],
        ),
    ];
    assert_eq!(
        commands.len(),
        CommandId::ALL.len(),
        "command descriptor catalogue length drifted"
    );
    for (descriptor, id) in commands.iter().zip(CommandId::ALL) {
        assert_eq!(
            descriptor.name,
            id.as_str(),
            "command descriptor ordering or identity drifted"
        );
    }
    commands
        .into_iter()
        .zip(CommandId::ALL)
        .map(|(mut descriptor, id)| {
            descriptor.name = id.as_str().to_owned();
            CommandSpec {
                id: *id,
                descriptor,
            }
        })
        .collect()
}

fn describe_descriptor() -> CommandDescriptor {
    CommandDescriptor {
        name: "describe".to_owned(),
        summary: "List implemented commands or describe one command as structured data.".to_owned(),
        risk: RiskLevel::Read,
        authentication_required: false,
        mutates_device: false,
        retryable: false,
        arguments: vec![ArgumentDescriptor {
            name: "command".to_owned(),
            value_type: "string".to_owned(),
            required: false,
            description: "Stable dotted command name; omit it to list commands.".to_owned(),
            allowed_values: Vec::new(),
        }],
        output: OutputDescriptor {
            value_type: "command_list | command_description".to_owned(),
            description: "Implemented command descriptors and their machine-readable contracts."
                .to_owned(),
        },
        possible_errors: vec!["COMMAND_NOT_FOUND".to_owned()],
        examples: vec!["oxvif describe device.info --output json".to_owned()],
    }
}

fn import_descriptor() -> CommandDescriptor {
    let mut command = descriptor(
        "device.import",
        "Plan or atomically apply devices from a discovery snapshot.",
        RiskLevel::Write,
        false,
        false,
        vec![
            required("from", "discovery snapshot ID"),
            optional("filter", "field=value[]"),
            optional("group", "group ID"),
            optional("credential-profile", "credential profile ID"),
            optional("tag", "string[]"),
            optional("overrides", "versioned JSON file"),
            optional("overrides-stdin", "versioned JSON on stdin"),
            optional("plan", "boolean"),
            optional("apply", "boolean"),
            optional("expect-plan", "sha256 fingerprint"),
        ],
    );
    command
        .possible_errors
        .push("IMPORT_PLAN_MISMATCH".to_owned());
    command.examples = vec![
        "oxvif device import --from scan --plan --output json".to_owned(),
        "oxvif device import --from scan --apply --expect-plan sha256:... --output json".to_owned(),
    ];
    command
}

fn enrich_descriptor() -> CommandDescriptor {
    let mut command = descriptor(
        "discover.enrich",
        "Authenticate snapshot records and cache device identity metadata.",
        RiskLevel::Write,
        true,
        false,
        vec![
            required("snapshot", "string"),
            required("credential-profile", "credential profile ID"),
            optional("filter", "field=value[]"),
            optional("jobs", "integer 1..64"),
        ],
    );
    command.examples = vec![
        "oxvif discover enrich scan --credential-profile factory-admin --jobs 16 --output json"
            .to_owned(),
    ];
    command
}

fn read_descriptor(name: &str, summary: &str) -> CommandDescriptor {
    descriptor(name, summary, RiskLevel::Read, false, false, Vec::new())
}

fn local_read_descriptor(
    name: &str,
    summary: &str,
    arguments: Vec<ArgumentDescriptor>,
) -> CommandDescriptor {
    descriptor(name, summary, RiskLevel::Read, false, false, arguments)
}

fn registry_descriptor(
    name: &str,
    summary: &str,
    arguments: Vec<ArgumentDescriptor>,
) -> CommandDescriptor {
    descriptor(name, summary, RiskLevel::Write, false, false, arguments)
}

fn local_write_descriptor(
    name: &str,
    summary: &str,
    arguments: Vec<ArgumentDescriptor>,
) -> CommandDescriptor {
    descriptor(name, summary, RiskLevel::Write, false, false, arguments)
}

fn credential_descriptor(name: &str, summary: &str) -> CommandDescriptor {
    descriptor(
        name,
        summary,
        RiskLevel::Write,
        false,
        false,
        vec![required("id", "string")],
    )
}

fn device_read_descriptor(name: &str, summary: &str) -> CommandDescriptor {
    descriptor(
        name,
        summary,
        RiskLevel::Read,
        true,
        false,
        vec![
            optional("device", "string"),
            optional("target", "url | host"),
            optional("group", "static Group ID"),
            optional("view", "dynamic View ID"),
            optional("jobs", "integer 1..64"),
        ],
    )
}

fn profile_read_descriptor(name: &str, summary: &str) -> CommandDescriptor {
    let mut command = device_read_descriptor(name, summary);
    command
        .arguments
        .push(required("profile", "media profile token"));
    command
}

fn quick_profile_descriptor(name: &str, summary: &str) -> CommandDescriptor {
    let mut command = device_read_descriptor(name, summary);
    command
        .arguments
        .push(optional("profile", "media profile token"));
    command
}

fn descriptor(
    name: &str,
    summary: &str,
    risk: RiskLevel,
    authentication_required: bool,
    mutates_device: bool,
    arguments: Vec<ArgumentDescriptor>,
) -> CommandDescriptor {
    CommandDescriptor {
        name: name.to_owned(),
        summary: summary.to_owned(),
        risk,
        authentication_required,
        mutates_device,
        retryable: name.starts_with("device.test")
            || name.starts_with("device.info")
            || name.starts_with("device.refresh")
            || name.starts_with("device.capabilities")
            || name.starts_with("device.services")
            || name.starts_with("media.")
            || name.starts_with("ptz.")
            || name == "health.check"
            || matches!(
                name,
                "setup" | "info" | "test" | "health" | "profiles" | "stream" | "snapshot"
            )
            || name == "discover.scan"
            || name == "discover.refresh",
        arguments,
        output: output_descriptor(name),
        possible_errors: possible_errors(name),
        examples: vec![command_example(name).to_owned()],
    }
}

fn required(name: &str, value_type: &str) -> ArgumentDescriptor {
    argument(name, value_type, true)
}

fn optional(name: &str, value_type: &str) -> ArgumentDescriptor {
    argument(name, value_type, false)
}

fn argument(name: &str, value_type: &str, required: bool) -> ArgumentDescriptor {
    ArgumentDescriptor {
        name: name.to_owned(),
        value_type: value_type.to_owned(),
        required,
        description: argument_description(name, required).to_owned(),
        allowed_values: argument_allowed_values(name),
    }
}

fn output_descriptor(name: &str) -> OutputDescriptor {
    let (value_type, description) = match name {
        "agent.guide" => ("agent_guide", "Version-matched Agent rules and workflow."),
        "agent.prompt" => ("agent_prompt", "A compact Agent operating prompt."),
        "completion" => (
            "completion_script",
            "A raw shell completion script on stdout.",
        ),
        "config.path" | "config.validate" => (
            "config_status",
            "Resolved local-state paths and optional validation counts.",
        ),
        "setup" => (
            "device_setup",
            "The saved device and verification/current-selection state.",
        ),
        "auth" | "device.credential.set" | "device.credential.delete" => (
            "credential_updated",
            "The updated credential assignment without secret material.",
        ),
        "device.credential.use-profile" => (
            "device_record",
            "The device with its reusable credential-profile assignment.",
        ),
        "info" | "device.info" => (
            "device_information | fleet_diagnostic",
            "Live device identity for one target or deterministic fleet items and summary.",
        ),
        "test" | "device.test" => (
            "device_test | fleet_diagnostic",
            "Connectivity/authentication result for one target or deterministic fleet items and summary.",
        ),
        "health"
        | "profiles"
        | "stream"
        | "snapshot"
        | "device.capabilities"
        | "device.services"
        | "media.profiles"
        | "media.stream-uri"
        | "media.snapshot-uri"
        | "ptz.status"
        | "ptz.presets"
        | "health.check" => (
            "device_diagnostic | fleet_diagnostic",
            "A tagged diagnostic result for one target or deterministic fleet items and summary.",
        ),
        "devices" | "device.list" => ("device_list", "Saved devices and current selection."),
        "groups" | "group.list" => ("group_list", "Saved static Groups."),
        "views" | "view.list" => ("view_list", "Saved dynamic Views."),
        "device.add" | "device.show" | "device.update" | "device.rename" => (
            "device_record",
            "One redacted saved-device record and action.",
        ),
        "device.remove" => ("device_removed", "The immutable ID that was removed."),
        "device.import" => (
            "device_import",
            "A deterministic import plan or applied result.",
        ),
        "credential.profile.set" | "credential.profile.show" => (
            "credential_profile_record",
            "One redacted reusable credential-profile record and action.",
        ),
        "credential.profile.delete" => (
            "resource_removed",
            "The removed reusable credential-profile ID.",
        ),
        "credential.profile.list" => (
            "credential_profile_list",
            "Reusable credential profiles without secret material.",
        ),
        "group.create"
        | "group.show"
        | "group.delete"
        | "group.member.add"
        | "group.member.remove" => ("group_record", "One static Group record and action."),
        "view.create" | "view.show" | "view.delete" => {
            ("view_record", "One dynamic View record and action.")
        }
        "view.evaluate" => (
            "view_evaluation",
            "The View, matching devices, and optional match explanation.",
        ),
        "discover.scan" | "discover.refresh" => (
            "discovery_scan",
            "Deterministically merged discovery records, interfaces, and optional snapshot.",
        ),
        "discover.enrich" => (
            "discovery_enrichment",
            "Snapshot enrichment attempted/succeeded/failed counts.",
        ),
        "discover.snapshots" => (
            "discovery_snapshot_list",
            "Saved discovery snapshot summaries.",
        ),
        "discover.list" => (
            "discovery_snapshot_record",
            "One filtered discovery snapshot without secret material.",
        ),
        "discover.remove" => ("resource_removed", "The removed discovery snapshot ID."),
        "use" | "current" => (
            "current_device",
            "The current interactive device selection.",
        ),
        "device.refresh" => (
            "device_record",
            "The saved device with refreshed identity metadata and any identity-change warning.",
        ),
        _ => (
            "typed_command_data",
            "Tagged command data inside the stable oxvif envelope.",
        ),
    };
    OutputDescriptor {
        value_type: value_type.to_owned(),
        description: description.to_owned(),
    }
}

fn possible_errors(name: &str) -> Vec<String> {
    let errors: &[&str] = if matches!(
        name,
        "agent.guide" | "agent.prompt" | "completion" | "config.path"
    ) {
        &["INVALID_ARGUMENT"]
    } else if name == "config.validate" {
        &[
            "CONFIG_UNAVAILABLE",
            "REGISTRY_IO",
            "REGISTRY_CORRUPT",
            "REGISTRY_VERSION_UNSUPPORTED",
        ]
    } else if name == "describe" {
        &["COMMAND_NOT_FOUND"]
    } else if name.starts_with("discover.") {
        &[
            "INVALID_ARGUMENT",
            "RESOURCE_NOT_FOUND",
            "RESOURCE_ALREADY_EXISTS",
            "REGISTRY_IO",
            "CREDENTIAL_UNAVAILABLE",
            "DISCOVERY_FAILED",
            "DEVICE_CONNECTION_FAILED",
        ]
    } else if matches!(
        name,
        "info"
            | "test"
            | "health"
            | "profiles"
            | "stream"
            | "snapshot"
            | "setup"
            | "device.test"
            | "device.info"
            | "device.capabilities"
            | "device.services"
            | "media.profiles"
            | "media.stream-uri"
            | "media.snapshot-uri"
            | "ptz.status"
            | "ptz.presets"
            | "health.check"
            | "device.refresh"
    ) {
        &[
            "INVALID_ARGUMENT",
            "MISSING_TARGET",
            "DEVICE_NOT_FOUND",
            "RESOURCE_NOT_FOUND",
            "CREDENTIAL_UNAVAILABLE",
            "DEVICE_CONNECTION_FAILED",
            "FLEET_FAILED",
        ]
    } else {
        &[
            "INVALID_ARGUMENT",
            "DEVICE_NOT_FOUND",
            "RESOURCE_NOT_FOUND",
            "RESOURCE_ALREADY_EXISTS",
            "RESOURCE_IN_USE",
            "REGISTRY_IO",
            "REGISTRY_CORRUPT",
            "CREDENTIAL_UNAVAILABLE",
        ]
    };
    errors.iter().map(|error| (*error).to_owned()).collect()
}

fn argument_description(name: &str, required: bool) -> &'static str {
    match name {
        "id" => "Immutable machine-safe resource ID.",
        "target" => {
            "Device host, IP address, or ONVIF device-service URL; URL userinfo is forbidden."
        }
        "device" => "Saved device ID or Group-local alias; conflicts with group and view.",
        "group" => {
            "Static Group ID selecting all explicit members; conflicts with device and view."
        }
        "view" => "Dynamic View ID selecting all current matches; conflicts with device and group.",
        "jobs" => "Maximum concurrent fleet items, from 1 through 64.",
        "profile" => "Opaque media profile token returned by media.profiles.",
        "username" => {
            "Credential username; the password is read from a prompt or stdin, never this argument."
        }
        "password-stdin" => {
            "Read exactly one password line from stdin without echoing or logging it."
        }
        "interface" => {
            "Local discovery interface name or IPv4 address; repeat to select more than one."
        }
        "snapshot" => "Immutable discovery snapshot ID.",
        "filter" => "Repeatable field/operator/value filter documented by the command help.",
        "mode" => "Import conflict-handling mode.",
        "apply" => {
            "Apply a previously reviewed local-state plan instead of returning a dry-run plan."
        }
        "expect-plan" => "Required plan fingerprint when applying, preventing stale-plan mutation.",
        "explain" => "Include per-device View match reasoning.",
        "command" => "Stable dotted command name; omit it to list commands.",
        _ if required => "Required command argument; see the value type and command help.",
        _ => "Optional command argument; see the value type and command help.",
    }
}

fn argument_allowed_values(name: &str) -> Vec<String> {
    match name {
        "mode" => ["skip-existing", "fail-on-conflict"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn command_example(name: &str) -> &'static str {
    match name {
        "agent.guide" => "oxvif agent guide --output json --non-interactive",
        "agent.prompt" => "oxvif agent prompt --output json --non-interactive",
        "describe" => "oxvif describe device.info --output json --non-interactive",
        "setup" => "oxvif setup front-door 192.0.2.10 --username admin --no-verify",
        "auth" => "oxvif auth front-door --username admin",
        "info" => "oxvif info front-door --output json --non-interactive",
        "test" => "oxvif test front-door --output json --non-interactive",
        "health" => "oxvif --device front-door health --output json --non-interactive",
        "profiles" => "oxvif profiles front-door --output json --non-interactive",
        "stream" => "oxvif stream front-door --profile profile-1 --output json --non-interactive",
        "snapshot" => {
            "oxvif snapshot front-door --profile profile-1 --output json --non-interactive"
        }
        "devices" => "oxvif devices --output json --non-interactive",
        "groups" => "oxvif groups --output json --non-interactive",
        "views" => "oxvif views --output json --non-interactive",
        "completion" => "oxvif completion bash",
        "config.path" => "oxvif config path --output json --non-interactive",
        "config.validate" => "oxvif config validate --output json --non-interactive",
        "device.add" => "oxvif device add front-door --target 192.0.2.10 --output json",
        "device.list" => "oxvif device list --output json --non-interactive",
        "device.show" => "oxvif device show front-door --output json --non-interactive",
        "device.update" => "oxvif device update front-door --name 'Front Door' --output json",
        "device.rename" => "oxvif device rename front-door --name 'Entry Camera' --output json",
        "device.remove" => "oxvif device remove front-door --output json",
        "device.import" => {
            "oxvif device import --from factory-scan --plan --output json --non-interactive"
        }
        "device.credential.set" => {
            "oxvif device credential set front-door --username admin --password-stdin"
        }
        "device.credential.delete" => "oxvif device credential delete front-door --output json",
        "device.credential.use-profile" => {
            "oxvif device credential use-profile front-door factory-admin --output json"
        }
        "credential.profile.set" => {
            "oxvif credential profile set factory-admin --username admin --password-stdin"
        }
        "credential.profile.list" => {
            "oxvif credential profile list --output json --non-interactive"
        }
        "credential.profile.show" => {
            "oxvif credential profile show factory-admin --output json --non-interactive"
        }
        "credential.profile.delete" => {
            "oxvif credential profile delete factory-admin --output json"
        }
        "group.create" => "oxvif group create factory --name 'Factory cameras' --output json",
        "group.list" => "oxvif group list --output json --non-interactive",
        "group.show" => "oxvif group show factory --output json --non-interactive",
        "group.delete" => "oxvif group delete factory --output json",
        "group.member.add" => {
            "oxvif group member add factory front-door --alias cam-001 --output json"
        }
        "group.member.remove" => "oxvif group member remove factory front-door --output json",
        "view.create" => "oxvif view create outdoor-cameras --filter tag=outdoor --output json",
        "view.list" => "oxvif view list --output json --non-interactive",
        "view.show" => "oxvif view show online-cameras --output json --non-interactive",
        "view.evaluate" => {
            "oxvif view evaluate online-cameras --explain --output json --non-interactive"
        }
        "view.delete" => "oxvif view delete online-cameras --output json",
        "discover.scan" => {
            "oxvif --timeout 3s discover scan --save factory-scan --output json --non-interactive"
        }
        "discover.refresh" => {
            "oxvif --timeout 3s discover refresh factory-scan --output json --non-interactive"
        }
        "discover.enrich" => {
            "oxvif discover enrich factory-scan --credential-profile factory-admin --output json --non-interactive"
        }
        "discover.snapshots" => "oxvif discover snapshots --output json --non-interactive",
        "discover.list" => "oxvif discover list factory-scan --output json --non-interactive",
        "discover.remove" => "oxvif discover remove factory-scan --output json",
        "use" => "oxvif use front-door --output json",
        "current" => "oxvif current --output json --non-interactive",
        "device.test" => "oxvif --device front-door device test --output json --non-interactive",
        "device.info" => "oxvif --device front-door device info --output json --non-interactive",
        "device.capabilities" => {
            "oxvif --device front-door device capabilities --output json --non-interactive"
        }
        "device.services" => {
            "oxvif --device front-door device services --output json --non-interactive"
        }
        "media.profiles" => {
            "oxvif --device front-door media profiles --output json --non-interactive"
        }
        "media.stream-uri" => {
            "oxvif --device front-door media stream-uri --profile profile-1 --output json --non-interactive"
        }
        "media.snapshot-uri" => {
            "oxvif --device front-door media snapshot-uri --profile profile-1 --output json --non-interactive"
        }
        "ptz.status" => {
            "oxvif --device front-door ptz status --profile profile-1 --output json --non-interactive"
        }
        "ptz.presets" => {
            "oxvif --device front-door ptz presets --profile profile-1 --output json --non-interactive"
        }
        "health.check" => "oxvif --device front-door health check --output json --non-interactive",
        "device.refresh" => "oxvif device refresh front-door --output json --non-interactive",
        _ => "oxvif describe --output json --non-interactive",
    }
}

fn argument_with_values(
    name: &str,
    value_type: &str,
    required: bool,
    allowed_values: &[&str],
) -> ArgumentDescriptor {
    let mut argument = argument(name, value_type, required);
    argument.allowed_values = allowed_values
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    argument
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn command_ids_are_unique_and_round_trip_every_descriptor() {
        let names = CommandId::ALL
            .iter()
            .map(|id| id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), CommandId::ALL.len());
        for id in CommandId::ALL {
            assert_eq!(CommandId::from_name(id.as_str()), Some(*id));
        }
        assert_eq!(
            descriptors()
                .into_iter()
                .map(|descriptor| descriptor.name)
                .collect::<Vec<_>>(),
            CommandId::ALL
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn lists_only_implemented_commands() {
        let CommandData::CommandList { commands } =
            execute(DescribeRequest::default()).expect("describe should succeed")
        else {
            panic!("expected command list");
        };

        assert_eq!(commands.len(), 61);
        assert_eq!(commands[0].name, "agent.guide");
        assert!(commands.iter().any(|command| command.name == "setup"));
        assert!(commands.iter().any(|command| command.name == "stream"));
        assert!(commands.iter().all(|command| !command.examples.is_empty()));
        assert!(
            commands
                .iter()
                .all(|command| command.output.value_type != "object")
        );
        assert!(commands.iter().all(|command| {
            command.arguments.iter().all(|argument| {
                !argument
                    .description
                    .starts_with("Required command argument.")
            })
        }));
    }

    #[test]
    fn unknown_command_is_typed_error() {
        let error = execute(DescribeRequest {
            command: Some("device.factory-reset".to_owned()),
        })
        .expect_err("unimplemented command should fail");

        assert_eq!(error.code, crate::ErrorCode::CommandNotFound);
        assert!(!error.retryable);
    }
}
