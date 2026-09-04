use std::fmt::Write;

use unicode_width::UnicodeWidthStr;

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
        CommandData::DiscoverySnapshotRecord { action, snapshot } => {
            let mut output = format!(
                "Discovery snapshot {action}.\nID: {}\nGeneration: {}\nInterfaces: {}\nDevices: {}/{} matched | Saved: {} | New: {} | Incomplete: {}\nSaved at (Unix ms): {}",
                snapshot.id,
                snapshot.generation,
                if snapshot.interfaces.is_empty() {
                    "(unknown)".to_owned()
                } else {
                    snapshot.interfaces.join(", ")
                },
                snapshot.summary.matched_count,
                snapshot.summary.total_count,
                snapshot.summary.saved_count,
                snapshot.summary.new_count,
                snapshot.summary.incomplete_count,
                snapshot.saved_at_unix_ms
            );
            append_discovery_table(&mut output, &snapshot.devices);
            output
        }
        CommandData::DiscoveryScan {
            devices,
            summary,
            saved_snapshot,
            interfaces,
        } => {
            let mut output = format!(
                "Discovery found {} device(s); showing {}.\nSaved: {} | New: {} | Incomplete: {}",
                summary.total_count,
                summary.matched_count,
                summary.saved_count,
                summary.new_count,
                summary.incomplete_count
            );
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
            append_discovery_table(&mut output, devices);
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
        CommandData::ConfigStatus {
            config_dir,
            registry_file,
            snapshots_dir,
            validated,
            device_count,
            snapshot_count,
            orphaned_snapshot_files,
        } => {
            let mut output = format!(
                "Config directory: {config_dir}\nRegistry file: {registry_file}\nSnapshots directory: {snapshots_dir}"
            );
            if *validated {
                let _ = write!(
                    output,
                    "\nValidation: valid\nDevices: {} | Snapshots: {}",
                    device_count.unwrap_or(0),
                    snapshot_count.unwrap_or(0)
                );
            } else {
                output.push_str("\nValidation: not requested");
            }
            if !orphaned_snapshot_files.is_empty() {
                let _ = write!(
                    output,
                    "\nOrphaned snapshot files (reported only):\n- {}",
                    orphaned_snapshot_files.join("\n- ")
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
        CommandData::DeviceDiagnostic {
            operation,
            device_id,
            target,
            result,
        } => format!(
            "Operation: {operation}\nDevice ID: {}\nTarget: {target}\n{}",
            device_id.as_deref().unwrap_or("(direct target)"),
            render_diagnostic_result(operation, result)
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

fn render_diagnostic_result(operation: &str, result: &serde_json::Value) -> String {
    match operation {
        "media.profiles" => render_profiles(result),
        "device.capabilities" => render_capabilities(result),
        "device.services" => render_services(result),
        "ptz.status" => render_ptz_status(result),
        "ptz.presets" => render_ptz_presets(result),
        "health.check" => render_health(result),
        "media.stream-uri" | "media.snapshot-uri" => render_scalar_object(result),
        _ => format!(
            "Result:\n{}",
            serde_json::to_string_pretty(result)
                .unwrap_or_else(|_| "(result serialization failed)".to_owned())
        ),
    }
}

fn render_profiles(result: &serde_json::Value) -> String {
    let Some(profiles) = result.as_array() else {
        return render_shape_mismatch(result);
    };
    if profiles.is_empty() {
        return "No media profiles reported.".to_owned();
    }
    let mut output = String::from("TOKEN | NAME | FIXED | VIDEO | AUDIO | PTZ");
    for profile in profiles {
        let _ = write!(
            output,
            "\n{} | {} | {} | {} | {} | {}",
            string_field(profile, "token"),
            string_field(profile, "name"),
            yes_no(
                profile
                    .get("fixed")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            ),
            present_field(
                profile,
                &["video_source_config_token", "video_encoder_token"]
            ),
            present_field(profile, &["audio_source_token", "audio_encoder_token"]),
            present_field(profile, &["ptz_config_token"]),
        );
    }
    output
}

fn render_capabilities(result: &serde_json::Value) -> String {
    let Some(capabilities) = result.as_object() else {
        return render_shape_mismatch(result);
    };
    let mut services = capabilities.iter().collect::<Vec<_>>();
    services.sort_by_key(|(name, _)| *name);
    let mut output = String::from("SERVICE | AVAILABLE | URL");
    for (name, capability) in services {
        let url = capability
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-");
        let available = url != "-";
        let _ = write!(output, "\n{name} | {} | {url}", yes_no(available));
    }
    output
}

fn render_services(result: &serde_json::Value) -> String {
    let Some(services) = result.as_array() else {
        return render_shape_mismatch(result);
    };
    if services.is_empty() {
        return "No ONVIF services reported.".to_owned();
    }
    let mut output = String::from("SERVICE | VERSION | URL");
    for service in services {
        let namespace = string_field(service, "namespace");
        let name = namespace
            .trim_end_matches('/')
            .rsplit('/')
            .nth(1)
            .unwrap_or(namespace);
        let major = service
            .get("version_major")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let minor = service
            .get("version_minor")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let _ = write!(
            output,
            "\n{name} | {major}.{minor} | {}",
            string_field(service, "url")
        );
    }
    output
}

fn render_ptz_status(result: &serde_json::Value) -> String {
    let Some(status) = result.as_object() else {
        return render_shape_mismatch(result);
    };
    format!(
        "POSITION | PAN {} | TILT {} | ZOOM {}\nMOVEMENT | PAN/TILT {} | ZOOM {}\nUTC time: {}\nDevice error: {}",
        number_or_dash(status.get("pan")),
        number_or_dash(status.get("tilt")),
        number_or_dash(status.get("zoom")),
        value_or_dash(status.get("pan_tilt_status")),
        value_or_dash(status.get("zoom_status")),
        value_or_dash(status.get("utc_time")),
        value_or_dash(status.get("error")),
    )
}

fn render_ptz_presets(result: &serde_json::Value) -> String {
    let Some(presets) = result.as_array() else {
        return render_shape_mismatch(result);
    };
    if presets.is_empty() {
        return "No PTZ presets reported.".to_owned();
    }
    let mut output = String::from("TOKEN | NAME | PAN | TILT | ZOOM");
    for preset in presets {
        let pan_tilt = preset.get("pan_tilt").and_then(serde_json::Value::as_array);
        let pan = pan_tilt.and_then(|values| values.first());
        let tilt = pan_tilt.and_then(|values| values.get(1));
        let _ = write!(
            output,
            "\n{} | {} | {} | {} | {}",
            string_field(preset, "token"),
            string_field(preset, "name"),
            number_or_dash(pan),
            number_or_dash(tilt),
            number_or_dash(preset.get("zoom")),
        );
    }
    output
}

fn render_health(result: &serde_json::Value) -> String {
    let Some(summary) = result.get("summary") else {
        return render_shape_mismatch(result);
    };
    let mut output = format!(
        "Health: {}\nPassed: {} | Warned: {} | Failed: {} | Skipped: {}",
        if result
            .get("healthy")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            "HEALTHY"
        } else {
            "UNHEALTHY"
        },
        integer_or_zero(summary.get("passed")),
        integer_or_zero(summary.get("warned")),
        integer_or_zero(summary.get("failed")),
        integer_or_zero(summary.get("skipped")),
    );
    let issues = result
        .pointer("/report/checks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|check| {
            matches!(
                check
                    .pointer("/status/kind")
                    .and_then(serde_json::Value::as_str),
                Some("warn" | "fail")
            )
        })
        .collect::<Vec<_>>();
    if !issues.is_empty() {
        output.push_str("\n\nSTATUS | CATEGORY | CHECK | REASON");
        for check in issues {
            let kind = check
                .pointer("/status/kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_ascii_uppercase();
            let _ = write!(
                output,
                "\n{} | {} | {} | {}",
                kind,
                value_or_dash(check.get("category")),
                string_field(check, "id"),
                value_or_dash(check.pointer("/status/reason")),
            );
        }
    }
    output
}

fn render_scalar_object(result: &serde_json::Value) -> String {
    let Some(fields) = result.as_object() else {
        return render_shape_mismatch(result);
    };
    fields
        .iter()
        .map(|(name, value)| format!("{}: {}", name.replace('_', " "), value_or_dash(Some(value))))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_shape_mismatch(result: &serde_json::Value) -> String {
    format!(
        "Result:\n{}",
        serde_json::to_string_pretty(result)
            .unwrap_or_else(|_| "(result serialization failed)".to_owned())
    )
}

fn string_field<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
}

fn present_field(value: &serde_json::Value, fields: &[&str]) -> &'static str {
    yes_no(
        fields
            .iter()
            .any(|field| value.get(*field).is_some_and(|value| !value.is_null())),
    )
}

fn number_or_dash(value: Option<&serde_json::Value>) -> String {
    value
        .filter(|value| !value.is_null())
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_owned())
}

fn value_or_dash(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(value)) if !value.is_empty() => value.clone(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => "-".to_owned(),
    }
}

fn integer_or_zero(value: Option<&serde_json::Value>) -> u64 {
    value.and_then(serde_json::Value::as_u64).unwrap_or(0)
}

fn append_discovery_table(output: &mut String, devices: &[crate::DiscoveryDeviceView]) {
    if devices.is_empty() {
        return;
    }
    let headers = ["#", "STATUS", "ADDRESS", "DEVICE", "SAVED AS", "ENDPOINT"];
    let mut rows = Vec::with_capacity(devices.len());
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
        let device_name = match (device.manufacturer.as_deref(), device.model.as_deref()) {
            (Some(manufacturer), Some(model)) => format!("{manufacturer} {model}"),
            (Some(manufacturer), None) => manufacturer.to_owned(),
            (None, Some(model)) => model.to_owned(),
            (None, None) => "Not advertised".to_owned(),
        };
        rows.push([
            (index + 1).to_string(),
            device.registration_status.as_str().to_ascii_uppercase(),
            address,
            device_name,
            device
                .registered_device_id
                .as_deref()
                .unwrap_or("—")
                .to_owned(),
            if device.endpoint.trim().is_empty() {
                "—".to_owned()
            } else {
                device.endpoint.clone()
            },
        ]);
    }
    output.push_str("\n\n");
    output.push_str(&render_columns(&headers, &rows));
}

fn render_columns<const N: usize>(headers: &[&str; N], rows: &[[String; N]]) -> String {
    let mut widths: [usize; N] =
        std::array::from_fn(|index| UnicodeWidthStr::width(headers[index]));
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(value.as_str()));
        }
    }

    let mut output = String::new();
    for (index, header) in headers.iter().enumerate() {
        if index > 0 {
            output.push_str(" | ");
        }
        if index + 1 == N {
            output.push_str(header);
        } else {
            push_padded(&mut output, header, widths[index]);
        }
    }
    for row in rows {
        output.push('\n');
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                output.push_str(" | ");
            }
            if index + 1 == N {
                output.push_str(value);
            } else {
                push_padded(&mut output, value, widths[index]);
            }
        }
    }
    output
}

fn push_padded(output: &mut String, value: &str, width: usize) {
    output.push_str(value);
    output.push_str(&" ".repeat(width.saturating_sub(UnicodeWidthStr::width(value))));
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
    fn discovery_registration_is_shared_by_human_and_structured_output() {
        let record = crate::DiscoveryRecord {
            endpoint: "urn:uuid:camera".to_owned(),
            types: Vec::new(),
            scopes: Vec::new(),
            xaddrs: vec!["http://192.168.1.20/onvif/device_service".to_owned()],
            manufacturer: Some("範例廠商".to_owned()),
            model: Some("攝影機".to_owned()),
            firmware_version: None,
            serial_number: None,
        };
        let device = crate::DiscoveryDeviceView::new(record, Some("front-door".to_owned()));
        let summary = crate::DiscoveryResultSummary::new(1, std::slice::from_ref(&device));
        let success = CommandSuccess {
            data: CommandData::DiscoveryScan {
                devices: vec![device],
                summary,
                saved_snapshot: None,
                interfaces: vec!["Ethernet".to_owned()],
            },
            warnings: Vec::new(),
            meta: ResultMeta::default(),
        };

        let table = render_success(OutputFormat::Table, &success).expect("table output");
        assert!(table.contains("front-door"));
        assert!(table.contains("192.168.1.20"));
        let table_lines = table
            .lines()
            .skip_while(|line| !line.starts_with("# "))
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(table_lines.len(), 2);
        let header_widths = table_lines[0]
            .split(" | ")
            .take(5)
            .map(UnicodeWidthStr::width)
            .collect::<Vec<_>>();
        let row_widths = table_lines[1]
            .split(" | ")
            .take(5)
            .map(UnicodeWidthStr::width)
            .collect::<Vec<_>>();
        assert_eq!(header_widths, row_widths);
        let json = render_success(OutputFormat::Json, &success).expect("json output");
        assert!(json.contains("front-door"));
        assert!(json.contains("registration_status"));
        assert!(json.contains("saved_count"));
        assert!(!json.contains("registrations"));
    }

    #[test]
    fn diagnostic_profiles_have_a_purpose_built_table() {
        let success = CommandSuccess {
            data: CommandData::DeviceDiagnostic {
                operation: "media.profiles".to_owned(),
                device_id: Some("front-door".to_owned()),
                target: "http://192.0.2.10/onvif/device_service".to_owned(),
                result: serde_json::json!([{
                    "token": "profile-1",
                    "name": "Main stream",
                    "fixed": true,
                    "video_encoder_token": "video-1",
                    "audio_encoder_token": null,
                    "ptz_config_token": "ptz-1"
                }]),
            },
            warnings: Vec::new(),
            meta: ResultMeta::default(),
        };

        let table = render_success(OutputFormat::Table, &success).expect("table output");
        assert!(table.contains("TOKEN | NAME | FIXED | VIDEO | AUDIO | PTZ"));
        assert!(table.contains("profile-1 | Main stream | yes | yes | no | yes"));
        assert!(!table.contains("\"video_encoder_token\""));
    }

    #[test]
    fn diagnostic_health_summarizes_and_lists_only_issues() {
        let result = serde_json::json!({
            "healthy": false,
            "summary": {"passed": 3, "warned": 1, "failed": 1, "skipped": 2},
            "report": {"checks": [
                {"id": "connect", "category": "Connectivity", "status": {"kind": "pass"}},
                {"id": "clock", "category": "Time", "status": {"kind": "warn", "reason": "clock skew"}},
                {"id": "profiles", "category": "Media", "status": {"kind": "fail", "reason": "SOAP fault"}}
            ]}
        });

        let table = render_health(&result);
        assert!(table.contains("Health: UNHEALTHY"));
        assert!(table.contains("WARN | Time | clock | clock skew"));
        assert!(table.contains("FAIL | Media | profiles | SOAP fault"));
        assert!(!table.contains("connect"));
    }
}
