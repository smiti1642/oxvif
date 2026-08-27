use crate::{
    AppError, ArgumentDescriptor, CommandData, CommandDescriptor, DescribeRequest,
    OutputDescriptor, RiskLevel,
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

fn descriptors() -> Vec<CommandDescriptor> {
    vec![
        read_descriptor(
            "agent.guide",
            "Return version-matched Agent operation and security rules.",
        ),
        read_descriptor(
            "agent.prompt",
            "Return a compact prompt for an Agent operating oxvif.",
        ),
        describe_descriptor(),
        registry_descriptor(
            "device.add",
            "Save a new device under an immutable machine-safe ID.",
            vec![required("id", "string"), required("target", "url | host")],
        ),
        read_descriptor(
            "device.list",
            "List saved devices and the current selection.",
        ),
        read_descriptor(
            "device.show",
            "Show one saved device without revealing its password.",
        ),
        registry_descriptor(
            "device.update",
            "Update a saved device target, display name, or tags.",
            vec![required("id", "string")],
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
        credential_descriptor(
            "device.credential.set",
            "Store a device password in the native OS credential store.",
        ),
        credential_descriptor(
            "device.credential.delete",
            "Delete a device password from the native OS credential store.",
        ),
        credential_descriptor(
            "device.credential.use-profile",
            "Assign a reusable credential profile to a saved device.",
        ),
        credential_descriptor(
            "credential.profile.set",
            "Create or update a reusable native credential profile.",
        ),
        read_descriptor(
            "credential.profile.list",
            "List credential profiles without exposing secrets.",
        ),
        read_descriptor(
            "credential.profile.show",
            "Show one credential profile without exposing its secret.",
        ),
        credential_descriptor(
            "credential.profile.delete",
            "Delete an unused credential profile and its native secret.",
        ),
        registry_descriptor(
            "group.create",
            "Create an empty static device Group.",
            vec![required("id", "string")],
        ),
        read_descriptor("group.list", "List static device Groups."),
        read_descriptor("group.show", "Show a Group and its explicit members."),
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
        read_descriptor("view.show", "Show one dynamic View definition."),
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
        device_read_descriptor(
            "device.refresh",
            "Read live device information and update cached registry metadata.",
        ),
    ]
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

fn registry_descriptor(
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
            || name == "discover.scan"
            || name == "discover.refresh",
        arguments,
        output: OutputDescriptor {
            value_type: "object".to_owned(),
            description: "A typed command result inside the stable oxvif envelope.".to_owned(),
        },
        possible_errors: vec![
            "INVALID_ARGUMENT".to_owned(),
            "RESOURCE_NOT_FOUND".to_owned(),
            "CREDENTIAL_UNAVAILABLE".to_owned(),
            "DEVICE_CONNECTION_FAILED".to_owned(),
            "DISCOVERY_FAILED".to_owned(),
        ],
        examples: Vec::new(),
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
        description: if required {
            "Required command argument.".to_owned()
        } else {
            "Optional command argument.".to_owned()
        },
        allowed_values: Vec::new(),
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

    #[test]
    fn lists_only_implemented_commands() {
        let CommandData::CommandList { commands } =
            execute(DescribeRequest::default()).expect("describe should succeed")
        else {
            panic!("expected command list");
        };

        assert_eq!(commands.len(), 47);
        assert_eq!(commands[0].name, "agent.guide");
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
