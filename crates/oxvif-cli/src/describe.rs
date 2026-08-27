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
    vec![describe_descriptor()]
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

        assert_eq!(commands.len(), 1);
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
