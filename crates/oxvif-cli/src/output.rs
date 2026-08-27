use std::fmt::Write;

use crate::{
    AppError, CommandData, CommandSuccess, ErrorEnvelope, OutputFormat, ResultMeta, SuccessEnvelope,
};

/// Render a successful command according to the caller's selected policy.
pub fn render_success(format: OutputFormat, success: &CommandSuccess) -> Result<String, AppError> {
    match format {
        OutputFormat::Table => Ok(render_human(success)),
        OutputFormat::Json => serde_json::to_string_pretty(&SuccessEnvelope::from(success))
            .map_err(|error| AppError::serialization_failed(error.to_string())),
        OutputFormat::JsonLines => serde_json::to_string(&SuccessEnvelope::from(success))
            .map_err(|error| AppError::serialization_failed(error.to_string())),
    }
}

/// Render an application or argument error with the same schema as all future
/// command failures.
pub fn render_error(
    format: OutputFormat,
    error: &AppError,
    meta: &ResultMeta,
) -> Result<String, AppError> {
    match format {
        OutputFormat::Table => {
            let mut rendered = format!("error[{}]: {}", error.code.as_str(), error.message);
            if let Some(suggested_action) = &error.suggested_action {
                let _ = write!(rendered, "\nhint: {suggested_action}");
            }
            Ok(rendered)
        }
        OutputFormat::Json => serde_json::to_string_pretty(&ErrorEnvelope::new(error, meta))
            .map_err(|serialization_error| {
                AppError::serialization_failed(serialization_error.to_string())
            }),
        OutputFormat::JsonLines => {
            serde_json::to_string(&ErrorEnvelope::new(error, meta)).map_err(|serialization_error| {
                AppError::serialization_failed(serialization_error.to_string())
            })
        }
    }
}

fn render_human(success: &CommandSuccess) -> String {
    let mut rendered = match &success.data {
        CommandData::CommandList { commands } => {
            let mut output = String::from("COMMAND   RISK   AUTH  SUMMARY\n");
            for command in commands {
                let _ = writeln!(
                    output,
                    "{:<9} {:<6} {:<5} {}",
                    command.name,
                    command.risk.as_str(),
                    yes_no(command.authentication_required),
                    command.summary
                );
            }
            output
        }
        CommandData::CommandDescription { command } => {
            let mut output = format!(
                "Command: {}\nSummary: {}\nRisk: {}\nAuthentication: {}\nMutates device: {}\nRetryable: {}\n",
                command.name,
                command.summary,
                command.risk.as_str(),
                yes_no(command.authentication_required),
                yes_no(command.mutates_device),
                yes_no(command.retryable)
            );
            output.push_str("Arguments:\n");
            if command.arguments.is_empty() {
                output.push_str("  (none)\n");
            } else {
                for argument in &command.arguments {
                    let requirement = if argument.required {
                        "required"
                    } else {
                        "optional"
                    };
                    let _ = writeln!(
                        output,
                        "  {}: {} ({}) — {}",
                        argument.name, argument.value_type, requirement, argument.description
                    );
                }
            }
            let _ = writeln!(
                output,
                "Output: {} — {}",
                command.output.value_type, command.output.description
            );
            output
        }
        CommandData::DeviceList {
            devices,
            current_device,
        } => {
            if devices.is_empty() {
                String::from("No saved devices.")
            } else {
                let mut output =
                    String::from("CURRENT  ID               NAME                 TARGET\n");
                for device in devices {
                    let current = if current_device.as_deref() == Some(&device.id) {
                        "*"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        output,
                        "{:<7}  {:<16} {:<20} {}",
                        current, device.id, device.name, device.target
                    );
                }
                output
            }
        }
        CommandData::DeviceRecord { action, device } => {
            format!("Device {action}.\n{}", render_device(device))
        }
        CommandData::DeviceRemoved { id } => format!("Device `{id}` removed."),
        CommandData::CurrentDevice { device } => match device {
            Some(device) => format!("Current device:\n{}", render_device(device)),
            None => String::from("No current device selected."),
        },
        CommandData::CredentialUpdated { action, device } => format!(
            "Credential {action} for `{}` (username: {}).",
            device.id,
            device.username.as_deref().unwrap_or("none")
        ),
        CommandData::DeviceTest {
            device_id,
            target,
            authenticated,
            information,
        } => format!(
            "Connection successful.\nDevice ID: {}\nTarget: {target}\nAuthenticated: {}\n{}",
            device_id.as_deref().unwrap_or("(direct target)"),
            yes_no(*authenticated),
            render_live_information(information)
        ),
        CommandData::DeviceInformation {
            device_id,
            target,
            information,
        } => format!(
            "Device ID: {}\nTarget: {target}\n{}",
            device_id.as_deref().unwrap_or("(direct target)"),
            render_live_information(information)
        ),
    };

    while rendered.ends_with('\n') {
        rendered.pop();
    }
    rendered
}

fn render_device(device: &crate::DeviceView) -> String {
    let mut output = format!(
        "ID: {}\nName: {}\nTarget: {}\nUsername: {}\nCredentials: {}",
        device.id,
        device.name,
        device.target,
        device.username.as_deref().unwrap_or("none"),
        yes_no(device.has_credentials)
    );
    if !device.tags.is_empty() {
        let _ = write!(output, "\nTags: {}", device.tags.join(", "));
    }
    if let Some(manufacturer) = &device.manufacturer {
        let _ = write!(output, "\nManufacturer: {manufacturer}");
    }
    if let Some(model) = &device.model {
        let _ = write!(output, "\nModel: {model}");
    }
    if let Some(firmware) = &device.firmware_version {
        let _ = write!(output, "\nFirmware: {firmware}");
    }
    output
}

fn render_live_information(information: &crate::LiveDeviceInfo) -> String {
    format!(
        "Manufacturer: {}\nModel: {}\nFirmware: {}\nSerial: {}\nHardware ID: {}",
        information.manufacturer,
        information.model,
        information.firmware_version,
        information.serial_number,
        information.hardware_id
    )
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
