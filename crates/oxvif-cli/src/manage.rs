//! Human workflow adapter. All device work stays in the shared application layer.
use std::path::PathBuf;

use oxvif_cli::{
    AppError, Application, CommandData, CommandRequest, CommandSuccess, DiscoverScanRequest,
    ExecutionOptions, ManagedAction, ManagedDevice, OutputFormat, SecretString, TargetSelector,
    profile_label, render_success_with_details,
};

use crate::interactive::Panel;
use crate::navigation::Viewport;

enum WorkflowOutcome {
    Completed(Box<CommandSuccess>),
    Failed,
    Cancelled,
}

fn proceed_after_profiles(choice: usize, outcome: &WorkflowOutcome) -> bool {
    match outcome {
        WorkflowOutcome::Completed(_) => true,
        WorkflowOutcome::Failed => choice == 0,
        WorkflowOutcome::Cancelled => false,
    }
}

pub(crate) async fn run(
    app: &Application,
    initial: TargetSelector,
    options: &ExecutionOptions,
) -> Result<(), AppError> {
    let mut panel = Panel::enter()?;
    let mut camera_view = Viewport::default();
    let mut next = if initial.device.is_some() || initial.target.is_some() {
        Some(initial)
    } else {
        None
    };
    loop {
        let selector = match next.take() {
            Some(selector) => selector,
            None => match choose_device(&mut panel, app, options, &mut camera_view).await? {
                Some(selector) => selector,
                None => return Ok(()),
            },
        };
        let label = selector
            .device
            .clone()
            .or(selector.target.clone())
            .unwrap_or_default();
        let mut context = match app.manage_device(selector, options) {
            Ok(context) => context,
            Err(error) => {
                panel.show("Cannot select device", &error.message)?;
                continue;
            }
        };
        let mut profile: Option<String> = None;
        let mut last: Option<CommandSuccess> = None;
        let mut action_view = Viewport::default();
        loop {
            let title = format!(
                "{label} | Profile: {} | {}",
                profile.as_deref().unwrap_or("not selected"),
                if context.needs_connection() {
                    "New connection needed"
                } else {
                    "Session cached (not a live status)"
                }
            );
            let choices = [
                "Diagnose camera",
                "Profiles / choose stream",
                "Save snapshot",
                "Export settings",
                "Compare settings",
                "Last result details",
                "Device information",
                "Session credentials (not saved)",
                "Reconnect on next operation",
                "Change device",
            ]
            .map(str::to_owned);
            let Some(choice) = panel.menu_with_view(&title, &choices, &[], &mut action_view)?
            else {
                break;
            };
            if choice == 9 {
                break;
            }
            if choice == 5 {
                panel.show(
                    "Last completed result",
                    &match &last {
                        Some(result) => {
                            render_success_with_details(OutputFormat::Table, result, true)?
                        }
                        None => {
                            "No completed result yet. Choose Diagnose camera or Device information."
                                .into()
                        }
                    },
                )?;
                continue;
            }
            if choice == 7 {
                if let Some(user) =
                    panel.input("Session credentials (not saved)", "ONVIF username:")?
                    && let Some(password) = panel.password()?
                {
                    context.set_credentials(user, SecretString::new(password)?);
                    panel.show("Credentials updated for this workspace only", "The next operation will establish a new session. Saved credentials were not changed.")?;
                }
                continue;
            }
            if choice == 8 {
                context.disconnect();
                continue;
            }
            if choice == 1 || ((choice == 0 || choice == 2) && profile.is_none()) {
                let outcome = execute(&mut panel, &mut context, ManagedAction::Profiles).await?;
                // A failed optional lookup may still yield useful diagnosis, but a
                // user's cancellation must never fall through into another request.
                if !proceed_after_profiles(choice, &outcome) {
                    continue;
                }
                if let WorkflowOutcome::Completed(result) = outcome {
                    let CommandData::DeviceDiagnostic { result: data, .. } = &result.data else {
                        unreachable!()
                    };
                    let profiles = data
                        .as_array()
                        .ok_or_else(|| AppError::internal("Invalid profile result."))?;
                    if profiles.is_empty() {
                        panel.show("No media profiles", "The camera returned no profiles. Diagnosis remains available to inspect other stages.")?;
                    } else {
                        let labels = profiles.iter().map(profile_label).collect::<Vec<_>>();
                        let details = profiles
                            .iter()
                            .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
                            .collect::<Vec<_>>();
                        let selected = if profiles.len() == 1 {
                            Some(0)
                        } else {
                            panel.menu(
                                "Select profile (camera configuration, not measured FPS)",
                                &labels,
                                &details,
                            )?
                        };
                        if let Some(index) = selected {
                            profile = profiles[index]["token"].as_str().map(str::to_owned);
                        } else {
                            continue;
                        }
                    }
                    if choice == 1 {
                        last = Some(*result);
                    }
                }
                if choice == 1 {
                    continue;
                }
            }
            let action = match choice {
                0 => ManagedAction::Diagnose {
                    profile: profile.clone(),
                },
                2 => {
                    let Some(token) = &profile else {
                        panel.show(
                            "Snapshot unavailable",
                            "Select an available media profile first.",
                        )?;
                        continue;
                    };
                    let Some(save) =
                        destination(&mut panel, "Save snapshot (format is not converted)")?
                    else {
                        continue;
                    };
                    ManagedAction::Snapshot {
                        profile: token.clone(),
                        save,
                    }
                }
                3 => {
                    let Some(save) =
                        destination(&mut panel, "Export settings (not a restorable backup)")?
                    else {
                        continue;
                    };
                    ManagedAction::Export { save }
                }
                4 => {
                    let Some(path) = panel.input(
                        "Compare settings",
                        "Existing inventory path, without surrounding quotes:",
                    )?
                    else {
                        continue;
                    };
                    ManagedAction::Diff {
                        against: PathBuf::from(path.trim()),
                    }
                }
                6 => ManagedAction::Info,
                _ => continue,
            };
            if let WorkflowOutcome::Completed(result) =
                execute(&mut panel, &mut context, action).await?
            {
                let title = format!(
                    "Operation finished | exit {} | Enter: continue",
                    result.exit_code()
                );
                panel.show(
                    &title,
                    &render_success_with_details(OutputFormat::Table, &result, false)?.replace(
                        "Use -v for stage details and timings.",
                        "Open Last result details for stage details and timings.",
                    ),
                )?;
                last = Some(*result);
            }
        }
    }
}

async fn execute(
    panel: &mut Panel,
    context: &mut ManagedDevice,
    action: ManagedAction,
) -> Result<WorkflowOutcome, AppError> {
    let status = if context.needs_connection() {
        "Connecting / reconnecting (new, expired or previous failure)"
    } else {
        "Reusing session; running live request"
    };
    match panel.wait(status, context.execute(action)).await? {
        Some(Ok(result)) => Ok(WorkflowOutcome::Completed(Box::new(result))),
        Some(Err(error)) => {
            panel.show("Operation failed; earlier result retained", &format!("{}\n{}\nCheck the target, ONVIF permissions or session credentials. No automatic alternate profile or TLS bypass was used.", error.code.as_str(), error.message))?;
            Ok(WorkflowOutcome::Failed)
        }
        None => {
            context.disconnect();
            panel.show("Operation cancelled", "Earlier completed result retained. Next operation reconnects. If saving a file, inspect the destination before trying again; completed files are never overwritten.")?;
            Ok(WorkflowOutcome::Cancelled)
        }
    }
}

fn destination(panel: &mut Panel, title: &str) -> Result<Option<PathBuf>, AppError> {
    let mut initial = String::new();
    loop {
        let Some(path) = panel.input_with_initial(
            title,
            "New output path; parent directory must exist. No quotes:",
            &initial,
        )?
        else {
            return Ok(None);
        };
        initial = path;
        let path = PathBuf::from(initial.trim());
        let request = oxvif_cli::SnapshotSaveRequest {
            selector: TargetSelector::default(),
            profile: "preflight".into(),
            save: path.clone(),
        };
        match request.preflight() {
            Ok(()) => return Ok(Some(path)),
            Err(error) => panel.show(
                "Choose another destination (nothing written)",
                &error.message,
            )?,
        }
    }
}

async fn choose_device(
    panel: &mut Panel,
    app: &Application,
    options: &ExecutionOptions,
    view: &mut Viewport,
) -> Result<Option<TargetSelector>, AppError> {
    loop {
        let (devices, _) = app.registry().list()?;
        let mut choices = super::interactive::aligned_menu_rows(
            &devices
                .iter()
                .map(|d| {
                    [
                        d.name.clone(),
                        d.id.clone(),
                        format!("{} (saved)", d.target),
                    ]
                })
                .collect::<Vec<_>>(),
        );
        choices.extend([
            "Search network for cameras".into(),
            "Enter device address (session only)".into(),
        ]);
        let details = devices
            .iter()
            .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
            .collect::<Vec<_>>();
        let Some(index) = panel.menu_with_view(
            "Choose camera | Esc/q: exit manage",
            &choices,
            &details,
            view,
        )?
        else {
            return Ok(None);
        };
        if let Some(device) = devices.get(index) {
            return Ok(Some(TargetSelector {
                device: Some(device.id.clone()),
                ..Default::default()
            }));
        }
        if index == devices.len() + 1 {
            if let Some(target) = panel.input(
                "Connect without saving a device",
                "IP address, hostname or ONVIF device URL:",
            )? {
                match oxvif_cli::normalize_target(target.trim()) {
                    Ok(target) => {
                        return Ok(Some(TargetSelector {
                            target: Some(target),
                            ..Default::default()
                        }));
                    }
                    Err(error) => panel.show("Invalid device address", &error.message)?,
                }
            }
            continue;
        }
        let scan = CommandRequest::DiscoverScan(DiscoverScanRequest {
            snapshot_id: None,
            interfaces: Vec::new(),
            filters: Vec::new(),
            query: None,
        });
        match panel
            .wait(
                "Discovering cameras (Esc cancels)",
                app.execute(scan, options),
            )
            .await?
        {
            Some(Ok(result)) => {
                let CommandData::DiscoveryScan { devices, .. } = result.data else {
                    unreachable!()
                };
                if let Some(index) = panel.select_discovered_device(&devices)? {
                    let device = &devices[index];
                    if let Some(id) = &device.registered_device_id {
                        return Ok(Some(TargetSelector {
                            device: Some(id.clone()),
                            ..Default::default()
                        }));
                    }
                    if let Some(target) = device
                        .record
                        .xaddrs
                        .iter()
                        .find_map(|s| oxvif_cli::normalize_target(s).ok())
                    {
                        return Ok(Some(TargetSelector {
                            target: Some(target),
                            ..Default::default()
                        }));
                    }
                    panel.show("No usable address", "This discovery record has no valid ONVIF address. Try Enter device address.")?;
                }
            }
            Some(Err(error)) => panel.show("Discovery failed", &error.message)?,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_profile_preflight_never_starts_diagnosis_or_snapshot() {
        for choice in [0, 1, 2] {
            assert!(!proceed_after_profiles(choice, &WorkflowOutcome::Cancelled));
        }
        assert!(proceed_after_profiles(0, &WorkflowOutcome::Failed));
        assert!(!proceed_after_profiles(1, &WorkflowOutcome::Failed));
        assert!(!proceed_after_profiles(2, &WorkflowOutcome::Failed));
    }
}
