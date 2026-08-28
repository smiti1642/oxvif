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
        OutputFormat::JsonLines => render_json_lines(success),
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
        CommandData::DeviceSetup {
            device,
            verified,
            current,
        } => format!(
            "Device setup complete.\n{}\nConnection: {}\nCurrent device: {}\n\nNext: oxvif info",
            render_device(device),
            if *verified {
                "verified"
            } else {
                "not verified"
            },
            if *current {
                device.id.as_str()
            } else {
                "unchanged"
            }
        ),
        CommandData::DeviceRemoved { id } => format!("Device `{id}` removed."),
        CommandData::DeviceImport {
            applied,
            plan,
            devices,
        } => {
            let mut output = format!(
                "Import {}.\nSnapshot: {}\nFingerprint: {}\nCreate: {} | Existing: {} | Filtered: {} | Conflicts: {}",
                if *applied { "applied" } else { "plan" },
                plan.snapshot_id,
                plan.fingerprint,
                plan.create_count,
                plan.already_present_count,
                plan.filtered_out_count,
                plan.conflict_count
            );
            for proposal in &plan.proposals {
                let _ = write!(
                    output,
                    "\n  {:?} | {} | {}",
                    proposal.disposition,
                    proposal.device_id.as_deref().unwrap_or("-"),
                    proposal.target.as_deref().unwrap_or("-")
                );
                if !proposal.reasons.is_empty() {
                    let _ = write!(output, " | {}", proposal.reasons.join("; "));
                }
            }
            if *applied {
                let _ = write!(output, "\nImported devices: {}", devices.len());
            }
            output
        }
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
                let mut output =
                    String::from("ID               GENERATION  DEVICES  SAVED_AT_UNIX_MS\n");
                for snapshot in snapshots {
                    let _ = writeln!(
                        output,
                        "{:<16} {:<11} {:<8} {}",
                        snapshot.id,
                        snapshot.generation,
                        snapshot.device_count,
                        snapshot.saved_at_unix_ms
                    );
                }
                output
            }
        }
        CommandData::DiscoverySnapshotRecord {
            action,
            snapshot,
            registrations,
        } => {
            let mut output = format!(
                "Discovery snapshot {action}.\nID: {}\nGeneration: {}\nInterfaces: {}\nDevices: {}\nSaved at (Unix ms): {}",
                snapshot.id,
                snapshot.generation,
                if snapshot.interfaces.is_empty() {
                    "(unknown)".to_owned()
                } else {
                    snapshot.interfaces.join(", ")
                },
                snapshot.devices.len(),
                snapshot.saved_at_unix_ms
            );
            append_discovery_table(&mut output, &snapshot.devices, registrations);
            output
        }
        CommandData::DiscoveryScan {
            devices,
            saved_snapshot,
            interfaces,
            registrations,
        } => {
            let mut output = format!("Discovery found {} device(s).", devices.len());
            if !interfaces.is_empty() {
                let _ = write!(output, "\nInterfaces: {}", interfaces.join(", "));
            }
            if let Some(snapshot) = saved_snapshot {
                let _ = write!(
                    output,
                    "\nSaved snapshot: {} (generation {})",
                    snapshot.id, snapshot.generation
                );
            }
            append_discovery_table(&mut output, devices, registrations);
            if let Some(snapshot) = saved_snapshot {
                let _ = write!(
                    output,
                    "\n\nNext:\n  Filter: oxvif discover list {} --filter ip-cidr=<CIDR>\n  Enrich: oxvif discover enrich {} --credential-profile <PROFILE>",
                    snapshot.id, snapshot.id
                );
            } else {
                output.push_str(
                    "\n\nNext:\n  Save a reusable scan: oxvif discover scan --save <SNAPSHOT>",
                );
            }
            output
        }
        CommandData::DiscoveryEnrichment {
            snapshot,
            attempted,
            enriched,
            failed,
        } => format!(
            "Discovery snapshot enriched.\nID: {}\nDevices: {}\nAttempted: {} | Enriched: {} | Failed: {}",
            snapshot.id, snapshot.device_count, attempted, enriched, failed
        ),
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
        CommandData::DeviceDiagnostic {
            operation,
            device_id,
            target,
            result,
        } => format!(
            "Operation: {operation}\nDevice ID: {}\nTarget: {target}\nResult:\n{}",
            device_id.as_deref().unwrap_or("(direct target)"),
            serde_json::to_string_pretty(result)
                .unwrap_or_else(|_| "(result serialization failed)".to_owned())
        ),
        CommandData::FleetDiagnostic {
            operation,
            selection_kind,
            selection_id,
            total,
            succeeded,
            failed,
            items,
        } => {
            let mut output = format!(
                "Operation: {operation}\nSelection: {selection_kind} `{selection_id}`\nTotal: {total} | Succeeded: {succeeded} | Failed: {failed}\nSTATUS | DEVICE | SELECTED BY | TARGET"
            );
            for item in items {
                let _ = write!(
                    output,
                    "\n{} | {} | {} | {}",
                    if item.ok { "OK" } else { "FAILED" },
                    item.device_id,
                    item.selected_by,
                    item.target
                );
                if let Some(error) = &item.error {
                    let _ = write!(output, " | {}: {}", error.code, error.message);
                }
            }
            output
        }
    };

    while rendered.ends_with('\n') {
        rendered.pop();
    }
    rendered
}

fn append_discovery_table(
    output: &mut String,
    devices: &[crate::DiscoveryRecord],
    registrations: &std::collections::BTreeMap<String, String>,
) {
    if devices.is_empty() {
        return;
    }
    output.push_str("\n\n# | ADDRESS | MANUFACTURER | MODEL | REGISTERED | ENDPOINT");
    for (index, device) in devices.iter().enumerate() {
        let address = device
            .xaddrs
            .iter()
            .find_map(|xaddr| {
                url::Url::parse(xaddr)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
            })
            .unwrap_or_else(|| "(no address)".to_owned());
        let registered = registrations
            .get(&device.endpoint)
            .map_or("—", String::as_str);
        let _ = write!(
            output,
            "\n{} | {} | {} | {} | {} | {}",
            index + 1,
            address,
            device.manufacturer.as_deref().unwrap_or("—"),
            device.model.as_deref().unwrap_or("—"),
            registered,
            device.endpoint
        );
    }
}

fn render_json_lines(success: &CommandSuccess) -> Result<String, AppError> {
    let CommandData::FleetDiagnostic {
        operation,
        selection_kind,
        selection_id,
        total,
        succeeded,
        failed,
        items,
    } = &success.data
    else {
        return serde_json::to_string(&SuccessEnvelope::from(success))
            .map_err(|error| AppError::serialization_failed(error.to_string()));
    };

    let mut lines = Vec::with_capacity(items.len() + 1);
    for item in items {
        let document = serde_json::json!({
            "schema_version": crate::SCHEMA_VERSION,
            "ok": item.ok,
            "data": {
                "kind": "fleet_item",
                "operation": operation,
                "selection_kind": selection_kind,
                "selection_id": selection_id,
                "item": item,
            },
            "warnings": [],
            "meta": {
                "command": success.meta.command,
                "device_id": item.device_id,
                "selected_by": item.selected_by,
                "target": item.target,
                "elapsed_ms": item.elapsed_ms,
            }
        });
        lines.push(
            serde_json::to_string(&document)
                .map_err(|error| AppError::serialization_failed(error.to_string()))?,
        );
    }
    let summary = serde_json::json!({
        "schema_version": crate::SCHEMA_VERSION,
        "ok": *failed == 0,
        "data": {
            "kind": "fleet_summary",
            "operation": operation,
            "selection_kind": selection_kind,
            "selection_id": selection_id,
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
        },
        "warnings": success.warnings,
        "meta": success.meta,
    });
    lines.push(
        serde_json::to_string(&summary)
            .map_err(|error| AppError::serialization_failed(error.to_string()))?,
    );
    Ok(lines.join("\n"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_registration_is_human_context_not_structured_schema() {
        let record = crate::DiscoveryRecord {
            endpoint: "urn:uuid:camera".to_owned(),
            types: Vec::new(),
            scopes: Vec::new(),
            xaddrs: vec!["http://192.168.1.20/onvif/device_service".to_owned()],
            manufacturer: Some("Example".to_owned()),
            model: Some("Cam".to_owned()),
            firmware_version: None,
            serial_number: None,
        };
        let success = CommandSuccess {
            data: CommandData::DiscoveryScan {
                devices: vec![record],
                saved_snapshot: None,
                interfaces: vec!["Ethernet".to_owned()],
                registrations: std::collections::BTreeMap::from([(
                    "urn:uuid:camera".to_owned(),
                    "front-door".to_owned(),
                )]),
            },
            warnings: Vec::new(),
            meta: ResultMeta::default(),
        };

        let table = render_success(OutputFormat::Table, &success).expect("table output");
        assert!(table.contains("front-door"));
        assert!(table.contains("192.168.1.20"));
        let json = render_success(OutputFormat::Json, &success).expect("json output");
        assert!(!json.contains("front-door"));
        assert!(!json.contains("registrations"));
    }
}
