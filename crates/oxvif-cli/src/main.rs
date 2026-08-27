use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use clap::{ArgAction, Parser, Subcommand, ValueEnum, error::ErrorKind};
use oxvif_cli::{
    AppError, Application, CommandRequest, CredentialProfileSetRequest, DescribeRequest,
    DeviceAddRequest, DeviceConnectRequest, DeviceCredentialProfileRequest,
    DeviceCredentialSetRequest, DeviceFilter, DeviceIdRequest, DeviceImportRequest,
    DeviceRenameRequest, DeviceUpdate, DeviceUpdateRequest, DiscoverScanRequest,
    DiscoveryEnrichRequest, DiscoveryFilter, DiscoveryImportOverride, DiscoveryImportOverrides,
    DiscoveryRefreshRequest, DiscoverySnapshotShowRequest, ExecutionOptions, GroupCreateRequest,
    GroupMemberAddRequest, GroupMemberRemoveRequest, ImportMode, MatchMode, NewDevice, NewGroup,
    NewSavedView, OutputFormat, ProfileConnectRequest, ResourceIdRequest, ResultMeta, SecretString,
    TargetSelector, ViewCreateRequest, render_error, render_success,
};
use tokio::time::Instant;

const AGENT_HELP: &str = "AI AGENTS:\n  Run `oxvif agent guide --output json` before operating devices.\n  Use structured output, --non-interactive, and an explicit device selector.\n  Never place passwords in command arguments, output, or logs.";

#[derive(Debug, Parser)]
#[command(
    name = "oxvif",
    version,
    about = "Human- and Agent-friendly ONVIF camera operations",
    after_help = AGENT_HELP
)]
struct Cli {
    /// Select terminal, JSON, or newline-delimited JSON output.
    #[arg(long, value_enum, default_value_t, global = true)]
    output: CliOutputFormat,

    /// Select a saved device by immutable ID.
    #[arg(long)]
    device: Option<String>,

    /// Select every explicit member of a static Group for a fleet diagnostic.
    #[arg(long, conflicts_with_all = ["device", "view"])]
    group: Option<String>,

    /// Select every current match of a dynamic View for a fleet diagnostic.
    #[arg(long, conflicts_with_all = ["device", "group"])]
    view: Option<String>,

    /// Bound concurrent work for Group/View diagnostics (default 16, maximum 64).
    #[arg(long)]
    jobs: Option<usize>,

    /// Never prompt or open a GUI; fail when required input is missing.
    #[arg(long, global = true)]
    non_interactive: bool,

    /// Bound command execution (supported units: ms, s, m).
    #[arg(
        long,
        default_value = "10s",
        value_parser = parse_duration,
    )]
    timeout: Duration,

    /// Number of retries allowed for retryable failures.
    #[arg(long, default_value_t = 0)]
    retries: u32,

    /// Increase diagnostic verbosity; repeat for more detail.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress non-essential diagnostics.
    #[arg(short, long, conflicts_with = "verbose", global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show version-matched operational guidance for AI Agents.
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// List implemented commands or describe one command.
    Describe {
        /// Stable dotted command name, for example `device.info`.
        command: Option<String>,
    },
    /// Manage saved devices and perform device-level operations.
    Device {
        #[command(subcommand)]
        command: DeviceCommands,
    },
    /// Inspect media profiles and obtain read-only media URIs.
    Media {
        #[command(subcommand)]
        command: MediaCommands,
    },
    /// Inspect PTZ state and presets without moving the camera.
    Ptz {
        #[command(subcommand)]
        command: PtzCommands,
    },
    /// Run read-only device health diagnostics.
    Health {
        #[command(subcommand)]
        command: HealthCommands,
    },
    /// Manage static groups and Group-local device aliases.
    Group {
        #[command(subcommand)]
        command: GroupCommands,
    },
    /// Manage saved dynamic filters over registered devices.
    View {
        #[command(subcommand)]
        command: ViewCommands,
    },
    /// Manage reusable credentials stored outside the registry.
    Credential {
        #[command(subcommand)]
        command: CredentialRootCommands,
    },
    /// Scan for ONVIF devices and manage named discovery snapshots.
    Discover {
        #[command(subcommand)]
        command: DiscoverCommands,
    },
    /// Select the current device for interactive commands.
    Use { id: String },
    /// Show the current interactive device selection.
    Current,
}

#[derive(Debug, Subcommand)]
enum AgentCommands {
    /// Return the versioned Agent operation guide.
    Guide,
    /// Print a compact prompt suitable for an Agent's instructions.
    Prompt,
}

#[derive(Debug, Subcommand)]
enum DeviceCommands {
    /// Save a new device under an immutable ID.
    Add {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long = "target")]
        add_target: String,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// List all saved devices.
    List,
    /// Show one saved device without revealing its password.
    Show { id: String },
    /// Update a saved device display name, target, or tags.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long = "target")]
        updated_target: Option<String>,
        #[arg(long = "tag", conflicts_with = "clear_tags")]
        tags: Vec<String>,
        #[arg(long)]
        clear_tags: bool,
    },
    /// Change the display name while preserving the immutable ID.
    Rename {
        id: String,
        #[arg(long)]
        name: String,
    },
    /// Remove a saved device and its stored credential.
    Remove { id: String },
    /// Plan or atomically apply devices from a discovery snapshot.
    Import {
        #[arg(long = "from")]
        snapshot: String,
        #[arg(long = "filter")]
        filters: Vec<DiscoveryFilter>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        credential_profile: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Read versioned, secret-free ID/alias overrides from a JSON file.
        #[arg(long, value_name = "FILE", conflicts_with = "overrides_stdin")]
        overrides: Option<PathBuf>,
        /// Read versioned, secret-free ID/alias overrides from stdin.
        #[arg(long, conflicts_with = "overrides")]
        overrides_stdin: bool,
        #[arg(long, conflicts_with = "apply", required_unless_present = "apply")]
        plan: bool,
        #[arg(long, conflicts_with = "plan", required_unless_present = "plan")]
        apply: bool,
        #[arg(long, requires = "apply")]
        expect_plan: Option<String>,
    },
    /// Store or delete a device password in the OS credential store.
    Credential {
        #[command(subcommand)]
        command: CredentialCommands,
    },
    /// Verify connectivity and authentication.
    Test {
        id: Option<String>,
        /// Use a direct ONVIF URL, hostname, or IP without saving it.
        #[arg(long)]
        target: Option<String>,
    },
    /// Read live ONVIF device information.
    Info {
        /// Saved device selector: a global ID or group/local-alias.
        id: Option<String>,
        /// Use a direct ONVIF URL, hostname, or IP without saving it.
        #[arg(long)]
        target: Option<String>,
    },
    /// Read the device's advertised ONVIF capabilities.
    Capabilities {
        /// Saved device selector: a global ID or group/local-alias.
        id: Option<String>,
        /// Use a direct ONVIF URL, hostname, or IP without saving it.
        #[arg(long)]
        target: Option<String>,
    },
    /// List all ONVIF service endpoints advertised by the device.
    Services {
        /// Saved device selector: a global ID or group/local-alias.
        id: Option<String>,
        /// Use a direct ONVIF URL, hostname, or IP without saving it.
        #[arg(long)]
        target: Option<String>,
    },
    /// Read live information and update cached registry metadata.
    Refresh { id: String },
}

#[derive(Debug, Subcommand)]
enum MediaCommands {
    /// List Media1 profiles.
    Profiles {
        #[arg(long)]
        target: Option<String>,
    },
    /// Get the RTSP URI for one media profile.
    StreamUri {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        target: Option<String>,
    },
    /// Get the snapshot URI for one media profile.
    SnapshotUri {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum PtzCommands {
    /// Read current PTZ position and movement state.
    Status {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        target: Option<String>,
    },
    /// List stored PTZ presets without moving the camera.
    Presets {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum HealthCommands {
    /// Run the default read-only health and conformance checks.
    Check {
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum GroupCommands {
    /// Create an empty static Group.
    Create {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// List static Groups.
    List,
    /// Show one Group and its explicit members.
    Show { id: String },
    /// Delete a Group without deleting its devices.
    Delete { id: String },
    /// Add or remove Group members.
    Member {
        #[command(subcommand)]
        command: GroupMemberCommands,
    },
}

#[derive(Debug, Subcommand)]
enum GroupMemberCommands {
    /// Add a device with an alias unique inside this Group.
    Add {
        group_id: String,
        device_id: String,
        #[arg(long)]
        alias: String,
    },
    /// Remove a member by its Group-local alias.
    Remove { group_id: String, alias: String },
}

#[derive(Debug, Subcommand)]
enum ViewCommands {
    /// Save a dynamic device filter.
    Create {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long = "filter", required = true)]
        filters: Vec<DeviceFilter>,
        #[arg(long = "match", default_value = "all")]
        match_mode: MatchMode,
    },
    /// List saved dynamic Views.
    List,
    /// Show one View definition.
    Show { id: String },
    /// Evaluate a View against current registered-device metadata.
    Evaluate {
        id: String,
        /// Include per-filter match counts in the result.
        #[arg(long)]
        explain: bool,
    },
    /// Delete a saved View.
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
enum CredentialRootCommands {
    /// Manage reusable credential profiles.
    Profile {
        #[command(subcommand)]
        command: CredentialProfileCommands,
    },
}

#[derive(Debug, Subcommand)]
enum CredentialProfileCommands {
    /// Create or update a reusable credential profile.
    Set {
        id: String,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password_stdin: bool,
    },
    /// List credential profiles without exposing secrets.
    List,
    /// Show one credential profile without exposing its secret.
    Show { id: String },
    /// Delete an unused credential profile and its native secret.
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
enum DiscoverCommands {
    /// Run WS-Discovery and save a deterministic named snapshot.
    Scan {
        #[arg(long)]
        save: Option<String>,
        /// Limit multicast discovery to an interface name or IPv4 address.
        #[arg(long = "interface")]
        interfaces: Vec<String>,
    },
    /// Re-run discovery and atomically replace an existing named snapshot.
    Refresh {
        snapshot: String,
        /// Limit multicast discovery to an interface name or IPv4 address.
        #[arg(long = "interface")]
        interfaces: Vec<String>,
    },
    /// Authenticate discovered devices and cache their identity metadata.
    Enrich {
        snapshot: String,
        #[arg(long)]
        credential_profile: String,
        #[arg(long = "filter")]
        filters: Vec<DiscoveryFilter>,
        #[arg(long, default_value_t = 16)]
        jobs: usize,
    },
    /// List records from one snapshot, optionally filtering them.
    List {
        snapshot: String,
        #[arg(long = "filter")]
        filters: Vec<DiscoveryFilter>,
    },
    /// List named discovery snapshots.
    Snapshots,
    /// Remove a named discovery snapshot.
    Remove { snapshot: String },
}

#[derive(Debug, Subcommand)]
enum CredentialCommands {
    /// Store a password in the native OS credential store.
    Set {
        id: String,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password_stdin: bool,
    },
    /// Delete a password from the native OS credential store.
    Delete { id: String },
    /// Assign a reusable credential profile to a device.
    UseProfile { id: String, profile: String },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum CliOutputFormat {
    #[default]
    Table,
    Json,
    #[value(name = "jsonl")]
    JsonLines,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(value: CliOutputFormat) -> Self {
        match value {
            CliOutputFormat::Table => Self::Table,
            CliOutputFormat::Json => Self::Json,
            CliOutputFormat::JsonLines => Self::JsonLines,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    ExitCode::from(run(env::args_os().collect()).await)
}

async fn run(arguments: Vec<OsString>) -> u8 {
    let requested_format = requested_output(&arguments);
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            let exit_code = error.exit_code();
            let _ = error.print();
            return u8::try_from(exit_code).unwrap_or(0);
        }
        Err(error) => {
            let app_error = AppError::invalid_argument(error.to_string().trim().to_owned());
            emit_error(requested_format, &app_error, None);
            return app_error.exit_code();
        }
    };

    let format = OutputFormat::from(cli.output);
    let set_selected = cli.group.is_some() || cli.view.is_some();
    if cli.jobs.is_some() && !set_selected {
        let error = AppError::invalid_argument("--jobs requires --group or --view.");
        emit_error(format, &error, None);
        return error.exit_code();
    }
    let jobs = cli.jobs.unwrap_or(16);
    if !(1..=64).contains(&jobs) {
        let error = AppError::invalid_argument("--jobs must be between 1 and 64.");
        emit_error(format, &error, None);
        return error.exit_code();
    }
    let options = ExecutionOptions {
        non_interactive: cli.non_interactive,
        timeout: cli.timeout,
        retries: cli.retries,
        verbosity: cli.verbose,
        quiet: cli.quiet,
        jobs,
    };
    let request = match build_request(cli.command, cli.device, cli.group, cli.view) {
        Ok(request) => request,
        Err(error) => {
            emit_error(format, &error, None);
            return error.exit_code();
        }
    };
    let command_name = request.name();
    let started = Instant::now();
    let application = match Application::system() {
        Ok(application) => application,
        Err(error) => {
            emit_error(format, &error, Some(command_name));
            return error.exit_code();
        }
    };

    match application.execute(request, &options).await {
        Ok(success) => match render_success(format, &success) {
            Ok(rendered) => {
                println!("{rendered}");
                success.exit_code()
            }
            Err(error) => {
                emit_error(format, &error, Some(command_name));
                error.exit_code()
            }
        },
        Err(error) => {
            let meta = ResultMeta {
                command: Some(command_name.to_owned()),
                elapsed_ms: elapsed_millis(started),
                ..ResultMeta::default()
            };
            emit_error_with_meta(format, &error, &meta);
            error.exit_code()
        }
    }
}

fn build_request(
    command: Commands,
    selected_device: Option<String>,
    selected_group: Option<String>,
    selected_view: Option<String>,
) -> Result<CommandRequest, AppError> {
    let selector = |target| TargetSelector {
        device: selected_device.clone(),
        target,
        group: selected_group.clone(),
        view: selected_view.clone(),
    };

    let request = match command {
        Commands::Agent { command } => Ok(match command {
            AgentCommands::Guide => CommandRequest::AgentGuide,
            AgentCommands::Prompt => CommandRequest::AgentPrompt,
        }),
        Commands::Describe { command } => Ok(CommandRequest::Describe(DescribeRequest { command })),
        Commands::Use { id } => Ok(CommandRequest::Use(DeviceIdRequest { id })),
        Commands::Current => Ok(CommandRequest::Current),
        Commands::Group { command } => match command {
            GroupCommands::Create { id, name } => {
                Ok(CommandRequest::GroupCreate(GroupCreateRequest {
                    group: NewGroup { id, name },
                }))
            }
            GroupCommands::List => Ok(CommandRequest::GroupList),
            GroupCommands::Show { id } => Ok(CommandRequest::GroupShow(ResourceIdRequest { id })),
            GroupCommands::Delete { id } => {
                Ok(CommandRequest::GroupDelete(ResourceIdRequest { id }))
            }
            GroupCommands::Member { command } => match command {
                GroupMemberCommands::Add {
                    group_id,
                    device_id,
                    alias,
                } => Ok(CommandRequest::GroupMemberAdd(GroupMemberAddRequest {
                    group_id,
                    device_id,
                    alias,
                })),
                GroupMemberCommands::Remove { group_id, alias } => Ok(
                    CommandRequest::GroupMemberRemove(GroupMemberRemoveRequest { group_id, alias }),
                ),
            },
        },
        Commands::View { command } => match command {
            ViewCommands::Create {
                id,
                name,
                filters,
                match_mode,
            } => Ok(CommandRequest::ViewCreate(ViewCreateRequest {
                view: NewSavedView {
                    id,
                    name,
                    filters,
                    match_mode,
                },
            })),
            ViewCommands::List => Ok(CommandRequest::ViewList),
            ViewCommands::Show { id } => Ok(CommandRequest::ViewShow(ResourceIdRequest { id })),
            ViewCommands::Evaluate { id, explain } => Ok(CommandRequest::ViewEvaluate(
                oxvif_cli::ViewEvaluateRequest { id, explain },
            )),
            ViewCommands::Delete { id } => Ok(CommandRequest::ViewDelete(ResourceIdRequest { id })),
        },
        Commands::Credential { command } => match command {
            CredentialRootCommands::Profile { command } => match command {
                CredentialProfileCommands::Set {
                    id,
                    username,
                    password_stdin,
                } => {
                    let (username, password) = credential_input(username, password_stdin)?;
                    Ok(CommandRequest::CredentialProfileSet(
                        CredentialProfileSetRequest {
                            id,
                            username,
                            password,
                        },
                    ))
                }
                CredentialProfileCommands::List => Ok(CommandRequest::CredentialProfileList),
                CredentialProfileCommands::Show { id } => {
                    Ok(CommandRequest::CredentialProfileShow(ResourceIdRequest {
                        id,
                    }))
                }
                CredentialProfileCommands::Delete { id } => {
                    Ok(CommandRequest::CredentialProfileDelete(ResourceIdRequest {
                        id,
                    }))
                }
            },
        },
        Commands::Discover { command } => match command {
            DiscoverCommands::Scan { save, interfaces } => {
                Ok(CommandRequest::DiscoverScan(DiscoverScanRequest {
                    snapshot_id: save,
                    interfaces,
                }))
            }
            DiscoverCommands::Refresh {
                snapshot,
                interfaces,
            } => Ok(CommandRequest::DiscoveryRefresh(DiscoveryRefreshRequest {
                id: snapshot,
                interfaces,
            })),
            DiscoverCommands::Enrich {
                snapshot,
                credential_profile,
                filters,
                jobs,
            } => {
                if !(1..=64).contains(&jobs) {
                    return Err(AppError::invalid_argument(
                        "--jobs must be between 1 and 64.",
                    ));
                }
                Ok(CommandRequest::DiscoveryEnrich(DiscoveryEnrichRequest {
                    id: snapshot,
                    credential_profile,
                    filters,
                    jobs,
                }))
            }
            DiscoverCommands::List { snapshot, filters } => Ok(
                CommandRequest::DiscoverySnapshotShow(DiscoverySnapshotShowRequest {
                    id: snapshot,
                    filters,
                }),
            ),
            DiscoverCommands::Snapshots => Ok(CommandRequest::DiscoverySnapshotList),
            DiscoverCommands::Remove { snapshot } => {
                Ok(CommandRequest::DiscoverySnapshotRemove(ResourceIdRequest {
                    id: snapshot,
                }))
            }
        },
        Commands::Media { command } => match command {
            MediaCommands::Profiles { target } => {
                Ok(CommandRequest::MediaProfiles(DeviceConnectRequest {
                    selector: selector(target),
                }))
            }
            MediaCommands::StreamUri { profile, target } => {
                Ok(CommandRequest::MediaStreamUri(ProfileConnectRequest {
                    selector: selector(target),
                    profile,
                }))
            }
            MediaCommands::SnapshotUri { profile, target } => {
                Ok(CommandRequest::MediaSnapshotUri(ProfileConnectRequest {
                    selector: selector(target),
                    profile,
                }))
            }
        },
        Commands::Ptz { command } => match command {
            PtzCommands::Status { profile, target } => {
                Ok(CommandRequest::PtzStatus(ProfileConnectRequest {
                    selector: selector(target),
                    profile,
                }))
            }
            PtzCommands::Presets { profile, target } => {
                Ok(CommandRequest::PtzPresets(ProfileConnectRequest {
                    selector: selector(target),
                    profile,
                }))
            }
        },
        Commands::Health { command } => match command {
            HealthCommands::Check { target } => {
                Ok(CommandRequest::HealthCheck(DeviceConnectRequest {
                    selector: selector(target),
                }))
            }
        },
        Commands::Device { command } => match command {
            DeviceCommands::Add {
                id,
                name,
                add_target,
                tags,
            } => Ok(CommandRequest::DeviceAdd(DeviceAddRequest {
                device: NewDevice {
                    id,
                    name,
                    target: add_target,
                    tags,
                },
            })),
            DeviceCommands::List => Ok(CommandRequest::DeviceList),
            DeviceCommands::Show { id } => Ok(CommandRequest::DeviceShow(DeviceIdRequest { id })),
            DeviceCommands::Update {
                id,
                name,
                updated_target,
                tags,
                clear_tags,
            } => Ok(CommandRequest::DeviceUpdate(DeviceUpdateRequest {
                id,
                update: DeviceUpdate {
                    name,
                    target: updated_target,
                    tags: if clear_tags || !tags.is_empty() {
                        Some(tags)
                    } else {
                        None
                    },
                },
            })),
            DeviceCommands::Rename { id, name } => {
                Ok(CommandRequest::DeviceRename(DeviceRenameRequest {
                    id,
                    name,
                }))
            }
            DeviceCommands::Remove { id } => {
                Ok(CommandRequest::DeviceRemove(DeviceIdRequest { id }))
            }
            DeviceCommands::Import {
                snapshot,
                filters,
                group,
                credential_profile,
                tags,
                overrides,
                overrides_stdin,
                plan: _,
                apply,
                expect_plan,
            } => {
                if apply && expect_plan.is_none() {
                    return Err(AppError::invalid_argument(
                        "`device import --apply` requires --expect-plan from a fresh plan.",
                    ));
                }
                Ok(CommandRequest::DeviceImport(DeviceImportRequest {
                    snapshot_id: snapshot,
                    filters,
                    group_id: group,
                    credential_profile,
                    tags,
                    overrides: read_import_overrides(overrides.as_deref(), overrides_stdin)?,
                    mode: if apply {
                        ImportMode::Apply
                    } else {
                        ImportMode::Plan
                    },
                    expected_fingerprint: expect_plan,
                }))
            }
            DeviceCommands::Credential { command } => match command {
                CredentialCommands::Set {
                    id,
                    username,
                    password_stdin,
                } => {
                    let (username, password) = credential_input(username, password_stdin)?;
                    Ok(CommandRequest::DeviceCredentialSet(
                        DeviceCredentialSetRequest {
                            id,
                            username,
                            password,
                        },
                    ))
                }
                CredentialCommands::Delete { id } => {
                    Ok(CommandRequest::DeviceCredentialDelete(DeviceIdRequest {
                        id,
                    }))
                }
                CredentialCommands::UseProfile { id, profile } => Ok(
                    CommandRequest::DeviceCredentialUseProfile(DeviceCredentialProfileRequest {
                        device_id: id,
                        profile_id: profile,
                    }),
                ),
            },
            DeviceCommands::Test { id, target } => {
                let selector = selector_with_positional(selector(target), id)?;
                Ok(CommandRequest::DeviceTest(DeviceConnectRequest {
                    selector,
                }))
            }
            DeviceCommands::Info { id, target } => {
                let selector = selector_with_positional(selector(target), id)?;
                Ok(CommandRequest::DeviceInfo(DeviceConnectRequest {
                    selector,
                }))
            }
            DeviceCommands::Capabilities { id, target } => {
                let selector = selector_with_positional(selector(target), id)?;
                Ok(CommandRequest::DeviceCapabilities(DeviceConnectRequest {
                    selector,
                }))
            }
            DeviceCommands::Services { id, target } => {
                let selector = selector_with_positional(selector(target), id)?;
                Ok(CommandRequest::DeviceServices(DeviceConnectRequest {
                    selector,
                }))
            }
            DeviceCommands::Refresh { id } => {
                Ok(CommandRequest::DeviceRefresh(DeviceIdRequest { id }))
            }
        },
    }?;

    if (selected_device.is_some() || selected_group.is_some() || selected_view.is_some())
        && !matches!(
            request,
            CommandRequest::DeviceTest(_)
                | CommandRequest::DeviceInfo(_)
                | CommandRequest::DeviceCapabilities(_)
                | CommandRequest::DeviceServices(_)
                | CommandRequest::MediaProfiles(_)
                | CommandRequest::MediaStreamUri(_)
                | CommandRequest::MediaSnapshotUri(_)
                | CommandRequest::PtzStatus(_)
                | CommandRequest::PtzPresets(_)
                | CommandRequest::HealthCheck(_)
        )
    {
        return Err(AppError::invalid_argument(
            "Root --device/--group/--view is accepted only by commands that operate on selected devices.",
        ));
    }
    Ok(request)
}

fn selector_with_positional(
    mut selector: TargetSelector,
    id: Option<String>,
) -> Result<TargetSelector, AppError> {
    if let Some(id) = id {
        if selector.device.is_some()
            || selector.target.is_some()
            || selector.group.is_some()
            || selector.view.is_some()
        {
            return Err(AppError::invalid_argument(
                "A positional device selector cannot be combined with --device, --target, --group, or --view.",
            ));
        }
        selector.device = Some(id);
    }
    Ok(selector)
}

fn read_password_from_stdin() -> Result<String, AppError> {
    let mut password = String::new();
    io::stdin()
        .read_to_string(&mut password)
        .map_err(|error| AppError::invalid_argument(format!("Failed to read password: {error}")))?;
    while password.ends_with(['\r', '\n']) {
        password.pop();
    }
    if password.is_empty() {
        Err(AppError::invalid_argument(
            "Password stdin was empty; no credential was stored.",
        ))
    } else {
        Ok(password)
    }
}

fn read_import_overrides(
    path: Option<&Path>,
    from_stdin: bool,
) -> Result<Vec<DiscoveryImportOverride>, AppError> {
    let contents = match (path, from_stdin) {
        (Some(path), false) => fs::read(path).map_err(|error| {
            AppError::invalid_argument(format!(
                "Failed to read import overrides from {}: {error}",
                path.display()
            ))
        })?,
        (None, true) => {
            let mut contents = Vec::new();
            io::stdin().read_to_end(&mut contents).map_err(|error| {
                AppError::invalid_argument(format!(
                    "Failed to read import overrides from stdin: {error}"
                ))
            })?;
            contents
        }
        (None, false) => return Ok(Vec::new()),
        (Some(_), true) => {
            return Err(AppError::invalid_argument(
                "Use only one of --overrides or --overrides-stdin.",
            ));
        }
    };
    const MAX_OVERRIDE_BYTES: usize = 1024 * 1024;
    if contents.len() > MAX_OVERRIDE_BYTES {
        return Err(AppError::invalid_argument(
            "Import overrides exceed the 1 MiB input limit.",
        ));
    }
    let document: DiscoveryImportOverrides =
        serde_json::from_slice(&contents).map_err(|error| {
            AppError::invalid_argument(format!("Invalid import override JSON: {error}"))
        })?;
    if document.version != 1 {
        return Err(AppError::invalid_argument(format!(
            "Unsupported import override version {}; expected version 1.",
            document.version
        )));
    }
    Ok(document.devices)
}

fn credential_input(
    username: Option<String>,
    password_stdin: bool,
) -> Result<(String, SecretString), AppError> {
    let username = username
        .or_else(|| env::var("OXVIF_USERNAME").ok())
        .ok_or_else(|| AppError::invalid_argument("Provide --username or set OXVIF_USERNAME."))?;
    let password = if password_stdin {
        read_password_from_stdin()?
    } else {
        env::var("OXVIF_PASSWORD").map_err(|_| {
            AppError::invalid_argument("Pass --password-stdin or set OXVIF_PASSWORD.")
        })?
    };
    Ok((username, SecretString::new(password)?))
}

fn emit_error(format: OutputFormat, error: &AppError, command: Option<&str>) {
    let meta = ResultMeta {
        command: command.map(str::to_owned),
        ..ResultMeta::default()
    };
    emit_error_with_meta(format, error, &meta);
}

fn emit_error_with_meta(format: OutputFormat, error: &AppError, meta: &ResultMeta) {
    match render_error(format, error, meta) {
        Ok(rendered) if format.is_structured() => println!("{rendered}"),
        Ok(rendered) => eprintln!("{rendered}"),
        Err(serialization_error) => eprintln!("{serialization_error}"),
    }
}

fn requested_output(arguments: &[OsString]) -> OutputFormat {
    let mut arguments = arguments
        .iter()
        .filter_map(|argument| argument.to_str())
        .peekable();
    while let Some(argument) = arguments.next() {
        if let Some(value) = argument.strip_prefix("--output=") {
            return parse_output_hint(value);
        }
        if argument == "--output" {
            return arguments
                .next()
                .map_or(OutputFormat::Table, parse_output_hint);
        }
    }
    OutputFormat::Table
}

fn parse_output_hint(value: &str) -> OutputFormat {
    match value {
        "json" => OutputFormat::Json,
        "jsonl" => OutputFormat::JsonLines,
        _ => OutputFormat::Table,
    }
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let split_at = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split_at);
    let number = number
        .parse::<u64>()
        .map_err(|_| "duration must begin with a positive integer".to_owned())?;
    if number == 0 {
        return Err("duration must be greater than zero".to_owned());
    }

    match unit {
        "ms" => Ok(Duration::from_millis(number)),
        "s" | "" => Ok(Duration::from_secs(number)),
        "m" => number
            .checked_mul(60)
            .map(Duration::from_secs)
            .ok_or_else(|| "duration is too large".to_owned()),
        _ => Err("duration unit must be one of: ms, s, m".to_owned()),
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_durations() {
        assert_eq!(parse_duration("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(parse_duration("10s"), Ok(Duration::from_secs(10)));
        assert_eq!(parse_duration("2m"), Ok(Duration::from_secs(120)));
    }

    #[test]
    fn rejects_zero_and_unknown_units() {
        assert!(parse_duration("0s").is_err());
        assert!(parse_duration("10h").is_err());
    }

    #[test]
    fn secret_debug_output_is_redacted() {
        let secret = SecretString::new("do-not-print").expect("secret should construct");
        assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
    }

    #[test]
    fn reads_versioned_import_overrides() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("overrides.json");
        fs::write(
            &path,
            r#"{"version":1,"devices":[{"endpoint":"uuid:camera","id":"front-door"}]}"#,
        )
        .expect("fixture should write");

        let overrides = read_import_overrides(Some(&path), false).expect("overrides should parse");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].id.as_deref(), Some("front-door"));
    }

    #[test]
    fn rejects_unknown_import_override_version() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("overrides.json");
        fs::write(&path, r#"{"version":2,"devices":[]}"#).expect("fixture should write");

        let error = read_import_overrides(Some(&path), false).expect_err("version must fail");
        assert_eq!(error.code, oxvif_cli::ErrorCode::InvalidArgument);
    }
}
