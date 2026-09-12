//! Human workflow adapter. All device work stays in the shared application layer.
use std::{collections::BTreeMap, path::PathBuf};

use oxvif_cli::{
    AppError, Application, CommandData, CommandRequest, CommandSuccess, DiscoverScanRequest,
    DiscoveryDeviceView, ExecutionOptions, ManagedAction, ManagedDevice, OutputFormat,
    SecretString, TargetSelector, profile_label, render_success_with_details,
};

use crate::interactive::{DiscoverySelection, DiscoverySelectionView, MenuSearch, Panel, TextView};
use crate::navigation::Viewport;

enum WorkflowOutcome {
    Completed(Box<CommandSuccess>),
    Failed,
    Cancelled,
}

#[derive(Default)]
struct DeviceChooser {
    menu: MenuSearch,
    search: Option<SearchResults>,
    browsing_search: bool,
    address_draft: String,
}

const MAX_WORKSPACES: usize = 256;

struct DeviceWorkspace {
    context: ManagedDevice,
    saved: Option<oxvif_cli::DeviceView>,
    profile: Option<String>,
    last: Option<CommandSuccess>,
    action_view: Viewport,
    profile_view: Viewport,
    result_view: TextView,
    issue: IssueLog,
    paths: PathDrafts,
}

enum RecentIssue {
    Failed(AppError),
    Cancelled,
}

#[derive(Default)]
struct IssueLog {
    latest: Option<RecentIssue>,
    view: TextView,
}

impl IssueLog {
    fn record(&mut self, issue: RecentIssue) {
        self.latest = Some(issue);
        self.view = TextView::default();
    }

    fn text(&self) -> String {
        match &self.latest {
            Some(RecentIssue::Failed(error)) => format!("{}\n{}\n{}", error.code.as_str(), error.message, error.suggested_action.as_deref().unwrap_or("Check the target, ONVIF permissions or session credentials. No automatic alternate profile or TLS bypass was used.")),
            Some(RecentIssue::Cancelled) => "Operation cancelled. Earlier completed result retained. Next operation reconnects. If saving a file, inspect the destination before trying again; completed files are never overwritten.".into(),
            None => "No failure or cancellation recorded for this camera.".into(),
        }
    }
}

#[derive(Default)]
struct PathDrafts {
    snapshot: String,
    export: String,
    compare: String,
    last_export: Option<PathBuf>,
}

impl PathDrafts {
    fn record_export(&mut self, result: &CommandSuccess) {
        if let CommandData::DeviceDiagnostic {
            operation, result, ..
        } = &result.data
            && operation == "config.export"
            && let Some(path) = result["saved_to"].as_str()
        {
            self.last_export = Some(PathBuf::from(path));
        }
    }
}

#[derive(Default)]
struct Workspaces(BTreeMap<String, DeviceWorkspace>);

impl Workspaces {
    fn select(
        &mut self,
        app: &Application,
        selector: TargetSelector,
        options: &ExecutionOptions,
    ) -> Result<&mut DeviceWorkspace, AppError> {
        let saved = selector
            .device
            .as_ref()
            .map(|id| app.registry().get(id))
            .transpose()?;
        let key = if let Some(device) = &saved {
            format!("saved:{}", device.id)
        } else {
            format!(
                "direct:{}",
                oxvif_cli::normalize_target(
                    selector
                        .target
                        .as_deref()
                        .ok_or_else(AppError::missing_target)?
                )?
            )
        };
        let changed = self.0.get(&key).is_some_and(|state| state.saved != saved);
        if changed {
            self.0.remove(&key);
        }
        if !self.0.contains_key(&key) {
            if self.0.len() >= MAX_WORKSPACES {
                return Err(AppError::invalid_argument(
                    "This workspace retains 256 cameras. Restart manage to select another camera; existing state has not been discarded.",
                ));
            }
            self.0.insert(
                key.clone(),
                DeviceWorkspace {
                    context: app.manage_device(selector, options)?,
                    saved,
                    profile: None,
                    last: None,
                    action_view: Viewport::default(),
                    profile_view: Viewport::default(),
                    result_view: TextView::default(),
                    issue: IssueLog::default(),
                    paths: PathDrafts::default(),
                },
            );
        }
        Ok(self.0.get_mut(&key).expect("inserted workspace"))
    }
}

fn restore_profile(
    profile: &mut Option<String>,
    view: &mut Viewport,
    profiles: &[serde_json::Value],
) {
    if let Some(index) = profile.as_ref().and_then(|token| {
        profiles
            .iter()
            .position(|p| p["token"].as_str() == Some(token))
    }) {
        view.selected = index;
    } else {
        *profile = None;
        *view = Viewport::default();
    }
}

struct SearchResults {
    devices: Vec<DiscoveryDeviceView>,
    view: DiscoverySelectionView,
}

impl DeviceChooser {
    fn replace_search(&mut self, devices: Vec<DiscoveryDeviceView>) {
        self.search = Some(SearchResults {
            devices,
            view: DiscoverySelectionView::default(),
        });
    }
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
    run_inner(app, initial, options, DeviceChooser::default()).await
}

#[cfg(test)]
pub(crate) async fn run_fixture(
    app: &Application,
    options: &ExecutionOptions,
    devices: Vec<DiscoveryDeviceView>,
) -> Result<(), AppError> {
    let mut chooser = DeviceChooser::default();
    chooser.replace_search(devices);
    run_inner(app, TargetSelector::default(), options, chooser).await
}

async fn run_inner(
    app: &Application,
    initial: TargetSelector,
    options: &ExecutionOptions,
    mut chooser: DeviceChooser,
) -> Result<(), AppError> {
    let mut panel = Panel::enter()?;
    let mut workspaces = Workspaces::default();
    let mut next = if initial.device.is_some() || initial.target.is_some() {
        Some(initial)
    } else {
        None
    };
    loop {
        let selector = match next.take() {
            Some(selector) => selector,
            None => match choose_device(&mut panel, app, options, &mut chooser).await? {
                Some(selector) => selector,
                None => return Ok(()),
            },
        };
        let label = selector
            .device
            .clone()
            .or(selector.target.clone())
            .unwrap_or_default();
        let state = match workspaces.select(app, selector, options) {
            Ok(state) => state,
            Err(error) => {
                panel.show("Cannot select device", &error.message)?;
                continue;
            }
        };
        let DeviceWorkspace {
            context,
            profile,
            last,
            action_view,
            profile_view,
            result_view,
            issue,
            paths,
            ..
        } = state;
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
                "Latest failure / cancellation",
            ]
            .map(str::to_owned);
            let Some(choice) = panel.menu_with_view(&title, &choices, &[], action_view)? else {
                break;
            };
            if choice == 9 {
                break;
            }
            if choice == 5 {
                panel.show_with_view(
                    "Last completed result",
                    &match last.as_ref() {
                        Some(result) => {
                            render_success_with_details(OutputFormat::Table, result, true)?
                        }
                        None => {
                            "No completed result yet. Choose Diagnose camera or Device information."
                                .into()
                        }
                    },
                    result_view,
                )?;
                continue;
            }
            if choice == 10 {
                panel.show_with_view(
                    "Latest failure / cancellation (historical)",
                    &issue.text(),
                    &mut issue.view,
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
                let outcome = execute(&mut panel, context, ManagedAction::Profiles, issue).await?;
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
                    restore_profile(profile, profile_view, profiles);
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
                            panel.menu_with_view(
                                "Select profile (camera configuration, not measured FPS)",
                                &labels,
                                &details,
                                profile_view,
                            )?
                        };
                        if let Some(index) = selected {
                            *profile = profiles[index]["token"].as_str().map(str::to_owned);
                        } else {
                            continue;
                        }
                    }
                    if choice == 1 {
                        *last = Some(*result);
                        *result_view = TextView::default();
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
                    let Some(token) = profile.as_ref() else {
                        panel.show(
                            "Snapshot unavailable",
                            "Select an available media profile first.",
                        )?;
                        continue;
                    };
                    let Some(save) = destination(
                        &mut panel,
                        "Save snapshot (format is not converted)",
                        &mut paths.snapshot,
                    )?
                    else {
                        continue;
                    };
                    ManagedAction::Snapshot {
                        profile: token.clone(),
                        save,
                    }
                }
                3 => {
                    let Some(save) = destination(
                        &mut panel,
                        "Export settings (not a restorable backup)",
                        &mut paths.export,
                    )?
                    else {
                        continue;
                    };
                    ManagedAction::Export { save }
                }
                4 => {
                    let Some(path) = comparison(&mut panel, paths)? else {
                        continue;
                    };
                    ManagedAction::Diff { against: path }
                }
                6 => ManagedAction::Info,
                _ => continue,
            };
            if let WorkflowOutcome::Completed(result) =
                execute(&mut panel, context, action, issue).await?
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
                paths.record_export(&result);
                *last = Some(*result);
                *result_view = TextView::default();
            }
        }
    }
}

async fn execute(
    panel: &mut Panel,
    context: &mut ManagedDevice,
    action: ManagedAction,
    issue: &mut IssueLog,
) -> Result<WorkflowOutcome, AppError> {
    let status = if context.needs_connection() {
        "Connecting / reconnecting (new, expired or previous failure)"
    } else {
        "Reusing session; running live request"
    };
    match panel.wait(status, context.execute(action)).await? {
        Some(Ok(result)) => Ok(WorkflowOutcome::Completed(Box::new(result))),
        Some(Err(error)) => {
            issue.record(RecentIssue::Failed(error));
            panel.show("Operation failed; earlier result retained", &issue.text())?;
            Ok(WorkflowOutcome::Failed)
        }
        None => {
            context.disconnect();
            issue.record(RecentIssue::Cancelled);
            panel.show("Operation cancelled", &issue.text())?;
            Ok(WorkflowOutcome::Cancelled)
        }
    }
}

fn destination(
    panel: &mut Panel,
    title: &str,
    draft: &mut String,
) -> Result<Option<PathBuf>, AppError> {
    loop {
        let Some(path) = panel.input_with_draft(
            title,
            "New output path; parent directory must exist. No quotes:",
            draft,
        )?
        else {
            return Ok(None);
        };
        let path = PathBuf::from(path.trim());
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

fn comparison(panel: &mut Panel, paths: &mut PathDrafts) -> Result<Option<PathBuf>, AppError> {
    if let Some(path) = &paths.last_export {
        let choices = vec![
            format!("Use last successful export: {}", path.display()),
            "Enter another inventory path".into(),
        ];
        match panel.menu("Choose comparison baseline", &choices, &[])? {
            Some(0) => paths.compare = path.display().to_string(),
            Some(_) => {}
            None => return Ok(None),
        }
    }
    loop {
        let Some(path) = panel.input_with_draft(
            "Compare settings",
            "Existing inventory path, without surrounding quotes:",
            &mut paths.compare,
        )?
        else {
            return Ok(None);
        };
        let request = oxvif_cli::ConfigDiffRequest {
            selector: TargetSelector::default(),
            against: PathBuf::from(path.trim()),
        };
        match request.preflight() {
            Ok(()) => return Ok(Some(request.against)),
            Err(error) => panel.show("Invalid baseline; correct the path", &error.message)?,
        }
    }
}

async fn choose_device(
    panel: &mut Panel,
    app: &Application,
    options: &ExecutionOptions,
    chooser: &mut DeviceChooser,
) -> Result<Option<TargetSelector>, AppError> {
    loop {
        if chooser.browsing_search {
            if let Some(results) = &mut chooser.search {
                results.devices = app.refresh_discovery_registration(&results.devices)?;
                match panel.select_discovered_device(&results.devices, &mut results.view)? {
                    DiscoverySelection::Back => {
                        chooser.browsing_search = false;
                        continue;
                    }
                    DiscoverySelection::Select(index) => {
                        let device = &results.devices[index];
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
                        panel.show("No usable address", "This discovery record has no valid ONVIF address. Go back and use Enter device address, or press R to rescan.")?;
                        continue;
                    }
                    DiscoverySelection::Rescan => {}
                    DiscoverySelection::Add(index) => {
                        panel
                            .onboard_device(app, options, &results.devices[index].record, false)
                            .await?;
                        continue;
                    }
                }
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
                    chooser.replace_search(devices);
                }
                Some(Err(error)) => panel.show(
                    if chooser.search.is_some() {
                        "Discovery failed; previous results retained"
                    } else {
                        "Discovery failed"
                    },
                    &error.message,
                )?,
                None => {}
            }
            // Only a completed scan replaces the results. Back and failed/cancelled
            // rescans retain this manage session's data, filter and viewport.
            if chooser.search.is_none() {
                chooser.browsing_search = false;
            }
            continue;
        }
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
            if chooser.search.is_some() {
                "Return to search results (R to rescan)".into()
            } else {
                "Search network for cameras".into()
            },
            "Enter device address (session only)".into(),
        ]);
        let mut details = devices
            .iter()
            .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
            .collect::<Vec<_>>();
        details.extend([
            "Search network or return to cached results".into(),
            "Connect by address without saving".into(),
        ]);
        let mut identities = devices
            .iter()
            .map(|device| {
                (
                    format!("device:{}", device.id),
                    format!(
                        "{}\n{}\n{}\n{}",
                        device.id,
                        device.name,
                        device.target,
                        device.tags.join("\n")
                    ),
                )
            })
            .collect::<Vec<_>>();
        identities.extend([
            ("action:search".into(), String::new()),
            ("action:address".into(), String::new()),
        ]);
        let Some(index) = panel.searchable_menu(
            "Choose camera | Esc/q: exit manage",
            &choices,
            &details,
            &identities,
            devices.len(),
            &mut chooser.menu,
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
            while let Some(target) = panel.input_with_draft(
                "Connect without saving a device",
                "IP address, hostname or ONVIF device URL:",
                &mut chooser.address_draft,
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
        chooser.browsing_search = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_issue_does_not_replace_completed_data_or_invent_an_export() {
        let completed = |operation: &str, data| CommandSuccess {
            data: CommandData::DeviceDiagnostic {
                operation: operation.into(),
                device_id: None,
                target: "http://192.0.2.1/onvif/device_service".into(),
                result: data,
            },
            warnings: Vec::new(),
            meta: Default::default(),
        };
        let last = completed(
            "config.export",
            serde_json::json!({"saved_to":"inventory.json","complete":true}),
        );
        let mut paths = PathDrafts::default();
        paths.record_export(&last);
        assert_eq!(paths.last_export, Some(PathBuf::from("inventory.json")));
        let mut issue = IssueLog::default();
        let error = AppError::invalid_argument("broken baseline");
        issue.record(RecentIssue::Failed(error.clone()));
        assert!(matches!(&issue.latest, Some(RecentIssue::Failed(retained)) if retained == &error));
        assert!(issue.text().contains("broken baseline"));
        paths.record_export(&completed(
            "media.snapshot-save",
            serde_json::json!({"saved_to":"snapshot.jpg"}),
        ));
        paths.record_export(&completed(
            "config.export",
            serde_json::json!({"complete":false}),
        ));
        assert_eq!(paths.last_export, Some(PathBuf::from("inventory.json")));
        issue.record(RecentIssue::Cancelled);
        assert!(issue.text().contains("cancelled"));
        assert!(matches!(last.data, CommandData::DeviceDiagnostic { .. }));
    }

    #[test]
    fn workspaces_retain_device_state_without_cross_identity_reuse_and_are_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let registry = oxvif_cli::RegistryStore::at(directory.path());
        let app = Application::with_stores(
            registry.clone(),
            std::sync::Arc::new(oxvif_cli::MemoryCredentialStore::default()),
        );
        let options = ExecutionOptions::default();
        for id in ["a", "b"] {
            registry
                .add(oxvif_cli::NewDevice {
                    id: id.into(),
                    name: None,
                    target: "192.0.2.1".into(),
                    tags: Vec::new(),
                })
                .unwrap();
        }
        let selector = |id: &str| TargetSelector {
            device: Some(id.into()),
            ..Default::default()
        };
        let mut workspaces = Workspaces::default();
        {
            let a = workspaces.select(&app, selector("a"), &options).unwrap();
            a.profile = Some("token-a".into());
            a.action_view = Viewport {
                selected: 7,
                top: 3,
            };
            a.context
                .set_credentials("account-a".into(), SecretString::new("fake-a").unwrap());
        }
        assert!(
            workspaces
                .select(&app, selector("b"), &options)
                .unwrap()
                .profile
                .is_none()
        );
        let a = workspaces.select(&app, selector("a"), &options).unwrap();
        assert_eq!(a.profile.as_deref(), Some("token-a"));
        assert_eq!(
            a.action_view,
            Viewport {
                selected: 7,
                top: 3
            }
        );
        let direct = |n| TargetSelector {
            target: Some(format!("http://192.0.2.1:{n}/onvif/device_service")),
            ..Default::default()
        };
        assert!(
            workspaces
                .select(&app, direct(80), &options)
                .unwrap()
                .profile
                .is_none()
        );
        registry
            .update(
                "a",
                oxvif_cli::DeviceUpdate {
                    target: Some("192.0.2.2".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            workspaces
                .select(&app, selector("a"), &options)
                .unwrap()
                .profile
                .is_none()
        );
        for port in 10000..10253 {
            workspaces.select(&app, direct(port), &options).unwrap();
        }
        assert_eq!(workspaces.0.len(), MAX_WORKSPACES);
        assert!(workspaces.select(&app, direct(11000), &options).is_err());
        assert!(workspaces.select(&app, selector("b"), &options).is_ok());
        assert_eq!(workspaces.0.len(), MAX_WORKSPACES);
    }

    #[test]
    fn profile_restore_tracks_tokens_not_indices_and_clears_removed_tokens() {
        let mut token = Some("chosen".into());
        let mut view = Viewport {
            selected: 0,
            top: 0,
        };
        restore_profile(
            &mut token,
            &mut view,
            &[
                serde_json::json!({"token":"other"}),
                serde_json::json!({"token":"chosen"}),
            ],
        );
        assert_eq!(view.selected, 1);
        assert_eq!(token.as_deref(), Some("chosen"));
        restore_profile(&mut token, &mut view, &[]);
        assert!(token.is_none());
        assert_eq!(view, Viewport::default());
    }

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
