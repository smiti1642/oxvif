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
        CommandData::AgentGuide { guide } => {
            let mut output = format!(
                "oxvif Agent operation guide v{}\nCLI version: {}\nSchema version: {}\n\nRules:\n",
                guide.guide_version, guide.cli_version, guide.schema_version
            );
            for rule in &guide.rules {
                let _ = writeln!(output, "- {rule}");
            }
            output.push_str("\nRecommended workflow:\n");
            for step in &guide.recommended_workflow {
                let _ = writeln!(output, "- {step}");
            }
            output.push_str("\nSecurity requirements:\n");
            for requirement in &guide.security_requirements {
                let _ = writeln!(output, "- {requirement}");
            }
            output
        }
        CommandData::AgentPrompt { prompt } => prompt.clone(),
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
                let mut output = String::from("CURRENT | ID | NAME | TARGET\n");
                for device in devices {
                    let current = if current_device.as_deref() == Some(&device.id) {
                        "*"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        output,
                        "{} | {} | {} | {}",
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
        CommandData::CredentialProfileList { profiles } => {
            if profiles.is_empty() {
                String::from("No credential profiles.")
            } else {
                let mut output =
                    String::from("ID               USERNAME             CREDENTIALS\n");
                for profile in profiles {
                    let _ = writeln!(
                        output,
                        "{:<16} {:<20} {}",
                        profile.id,
                        profile.username,
                        yes_no(profile.has_credentials)
                    );
                }
                output
            }
        }
        CommandData::CredentialProfileRecord { action, profile } => format!(
            "Credential profile {action}.\nID: {}\nUsername: {}\nCredentials: {}",
            profile.id,
            profile.username,
            yes_no(profile.has_credentials)
        ),
        CommandData::GroupList { groups } => {
            if groups.is_empty() {
                String::from("No groups.")
            } else {
                let mut output = String::from("ID               NAME                 MEMBERS\n");
                for group in groups {
                    let _ = writeln!(
                        output,
                        "{:<16} {:<20} {}",
                        group.id,
                        group.name,
                        group.members.len()
                    );
                }
                output
            }
        }
        CommandData::GroupRecord { action, group } => {
            let mut output = format!(
                "Group {action}.\nID: {}\nName: {}\nMembers: {}",
                group.id,
                group.name,
                group.members.len()
            );
            for member in &group.members {
                let _ = write!(
                    output,
                    "\n  {}/{} -> {}",
                    group.id, member.alias, member.device_id
                );
            }
            output
        }
        CommandData::ViewList { views } => {
            if views.is_empty() {
                String::from("No views.")
            } else {
                let mut output = String::from("ID               NAME                 FILTERS\n");
                for view in views {
                    let _ = writeln!(
                        output,
                        "{:<16} {:<20} {}",
                        view.id,
                        view.name,
                        view.filters.len()
                    );
                }
                output
            }
        }
        CommandData::ViewRecord { action, view } => format!(
            "View {action}.\nID: {}\nName: {}\nMatch: {:?}\nFilters: {}",
            view.id,
            view.name,
            view.match_mode,
            format_device_filters(&view.filters)
        ),
        CommandData::ViewEvaluation {
            view,
            devices,
            explanation,
        } => {
            let mut output = format!("View `{}` matched {} device(s).\n", view.id, devices.len());
            if let Some(explanation) = explanation {
                let _ = writeln!(
                    output,
                    "Evaluated: {} | Match mode: {:?}",
                    explanation.evaluated_devices, view.match_mode
                );
                for item in &explanation.filters {
                    let _ = writeln!(
                        output,
                        "  {:?}:{:?}={} | matched {} | excluded {}",
                        item.filter.field,
                        item.filter.operator,
                        item.filter.value,
                        item.matched_devices,
                        item.unmatched_devices
                    );
                }
            }
            for device in devices {
                let _ = writeln!(output, "{}  {}  {}", device.id, device.name, device.target);
            }
            output
        }
        CommandData::DiscoverySnapshotList { snapshots } => {
            if snapshots.is_empty() {
                String::from("No discovery snapshots.")
            } else {
                let mut output = String::from("ID               DEVICES  SAVED_AT_UNIX_MS\n");
                for snapshot in snapshots {
                    let _ = writeln!(
                        output,
                        "{:<16} {:<8} {}",
                        snapshot.id, snapshot.device_count, snapshot.saved_at_unix_ms
                    );
                }
                output
            }
        }
        CommandData::DiscoverySnapshotRecord { action, snapshot } => {
            let mut output = format!(
                "Discovery snapshot {action}.\nID: {}\nDevices: {}\nSaved at (Unix ms): {}",
                snapshot.id,
                snapshot.devices.len(),
                snapshot.saved_at_unix_ms
            );
            for device in &snapshot.devices {
                let _ = write!(
                    output,
                    "\n  {}  {}",
                    device.endpoint,
                    device.xaddrs.first().map_or("(no XAddr)", String::as_str)
                );
            }
            output
        }
        CommandData::DiscoveryScan {
            devices,
            saved_snapshot,
            interfaces,
        } => {
            let mut output = format!("Discovery found {} device(s).", devices.len());
            if !interfaces.is_empty() {
                let _ = write!(output, "\nInterfaces: {}", interfaces.join(", "));
            }
            if let Some(snapshot) = saved_snapshot {
                let _ = write!(output, "\nSaved snapshot: {}", snapshot.id);
            }
            for device in devices {
                let _ = write!(
                    output,
                    "\n  {}  {}",
                    device.endpoint,
                    device.xaddrs.first().map_or("(no XAddr)", String::as_str)
                );
            }
            output
        }
        CommandData::ResourceRemoved { resource, id } => {
            format!("{resource} `{id}` removed.")
        }
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
        "ID: {}\nName: {}\nTarget: {}\nUsername: {}\nCredential source: {}\nCredential availability: {}",
        device.id,
        device.name,
        device.target,
        device.username.as_deref().unwrap_or("none"),
        device.credential_source.as_deref().unwrap_or("none"),
        device.credential_availability
    );
    if !device.tags.is_empty() {
        let _ = write!(output, "\nTags: {}", device.tags.join(", "));
    }
    if let Some(profile) = &device.credential_profile {
        let _ = write!(output, "\nCredential profile: {profile}");
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

fn format_device_filters(filters: &[crate::DeviceFilter]) -> String {
    filters
        .iter()
        .map(|filter| {
            let field = serde_json::to_value(filter.field)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{:?}", filter.field));
            let operator = serde_json::to_value(filter.operator)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{:?}", filter.operator));
            if operator == "eq" {
                format!("{field}={}", filter.value)
            } else {
                format!("{field}:{operator}={}", filter.value)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
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
