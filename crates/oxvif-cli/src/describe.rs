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
        credential_descriptor(
            "device.credential.set",
            "Store a device password in the native OS credential store.",
        ),
        credential_descriptor(
            "device.credential.delete",
            "Delete a device password from the native OS credential store.",
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
    }
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
            || name.starts_with("device.refresh"),
        arguments,
        output: OutputDescriptor {
            value_type: "object".to_owned(),
            description: "A typed command result inside the stable oxvif envelope.".to_owned(),
        },
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

        assert_eq!(commands.len(), 14);
        assert_eq!(commands[0].name, "describe");
    }

    #[test]
    fn unknown_command_is_typed_error() {
        let error = execute(DescribeRequest {
            command: Some("media.stream-uri".to_owned()),
        })
        .expect_err("unimplemented command should fail");

        assert_eq!(error.code, crate::ErrorCode::CommandNotFound);
        assert!(!error.retryable);
    }
}
