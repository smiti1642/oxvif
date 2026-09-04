use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};
use clap_complete::{Shell, generate};
use oxvif::transport::HttpTransport;
use oxvif_cli::{
    AppError, Application, ClockSyncPolicy, CommandData, CommandId, CommandRequest,
    CredentialProfileSetRequest, DescribeRequest, DeviceAddRequest, DeviceConnectRequest,
    DeviceCredentialProfileRequest, DeviceCredentialSetRequest, DeviceFilter, DeviceIdRequest,
    DeviceImportRequest, DeviceRenameRequest, DeviceSetupRequest, DeviceUpdate,
    DeviceUpdateRequest, DiscoverScanRequest, DiscoveryEnrichRequest, DiscoveryFilter,
    DiscoveryImportOverride, DiscoveryImportOverrides, DiscoveryRefreshRequest,
    DiscoverySnapshotShowRequest, ExecutionOptions, GroupCreateRequest, GroupMemberAddRequest,
    GroupMemberRemoveRequest, ImportMode, MatchMode, NewDevice, NewGroup, NewSavedView,
    OutputFormat, ProfileConnectRequest, ResourceIdRequest, ResultMeta, SecretString,
    TargetSelector, ViewCreateRequest, normalize_target, render_error, render_success,
};
use tokio::time::Instant;

mod interactive;

use interactive::{BrowserAction, DiscoverySetup, await_discovery, browse_discovery};

const AGENT_HELP: &str = "AI AGENTS:\n  Run `oxvif agent guide --output json` before operating devices.\n  Use structured output, --non-interactive, and an explicit device selector.\n  Never place passwords in command arguments, output, or logs.";

trait Prompt {
    fn text(&self, label: &str) -> Result<String, AppError>;
    fn password(&self, label: &str) -> Result<String, AppError>;
    fn select(&self, label: &str, choices: &[String]) -> Result<usize, AppError>;
}

struct SystemPrompt;

impl SystemPrompt {
    fn ensure_terminal() -> Result<(), AppError> {
        if io::stdin().is_terminal() && io::stderr().is_terminal() {
            Ok(())
        } else {
            Err(AppError::invalid_argument(
                "Interactive input requires a terminal; provide explicit input or use --non-interactive.",
            ))
        }
    }
}

impl Prompt for SystemPrompt {
    fn text(&self, label: &str) -> Result<String, AppError> {
        Self::ensure_terminal()?;
        eprint!("{label}");
        io::stderr().flush().map_err(|error| {
            AppError::invalid_argument(format!("Failed to display prompt: {error}"))
        })?;
        let mut value = String::new();
        io::stdin().read_line(&mut value).map_err(|error| {
            AppError::invalid_argument(format!("Failed to read input: {error}"))
        })?;
        Ok(value.trim().to_owned())
    }

    fn password(&self, label: &str) -> Result<String, AppError> {
        Self::ensure_terminal()?;
        rpassword::prompt_password(label).map_err(|error| {
            AppError::invalid_argument(format!("Failed to read password: {error}"))
        })
    }

    fn select(&self, label: &str, choices: &[String]) -> Result<usize, AppError> {
        Self::ensure_terminal()?;
        eprintln!("{label}");
        for (index, choice) in choices.iter().enumerate() {
            eprintln!("  {}. {choice}", index + 1);
        }
        let answer = self.text("Selection [1]: ")?;
        let selected = if answer.is_empty() {
            1
        } else {
            answer.parse::<usize>().map_err(|_| {
                AppError::invalid_argument("Profile selection must be a displayed number.")
            })?
        };
        if !(1..=choices.len()).contains(&selected) {
            return Err(AppError::invalid_argument(format!(
                "Profile selection must be between 1 and {}.",
                choices.len()
            )));
        }
        Ok(selected - 1)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "oxvif",
    version,
    about = "Human- and Agent-friendly ONVIF camera operations",
    after_help = AGENT_HELP
)]
struct Cli {
    /// Select terminal, JSON, or newline-delimited JSON output.
    #[arg(long, value_enum, global = true, conflicts_with_all = ["json", "jsonl"])]
    output: Option<CliOutputFormat>,

    /// Shorthand for --output json.
    #[arg(long, global = true, conflicts_with_all = ["output", "jsonl"])]
    json: bool,

    /// Shorthand for --output jsonl.
    #[arg(long, global = true, conflicts_with_all = ["output", "json"])]
    jsonl: bool,

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

    /// Bound each network attempt (supported units: ms, s, m).
    #[arg(
        long,
        default_value = "10s",
        value_parser = parse_duration,
    )]
    timeout: Duration,

    /// Number of retries for transient transport failures.
    #[arg(long, default_value_t = 0)]
    retries: u32,

    /// Device-clock synchronization policy for WS-Security timestamps.
    #[arg(long, value_enum, default_value_t = CliClockSyncPolicy::Auto)]
    clock_sync: CliClockSyncPolicy,

    /// Add a PEM CA certificate or bundle to the platform trust roots; repeatable.
    #[arg(long = "ca-certificate", value_name = "FILE")]
    ca_certificates: Vec<PathBuf>,

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
    /// Securely register, authenticate, verify, and select a device.
    Setup {
        /// Device host, IP address, or ONVIF device-service URL.
        target: Option<String>,
        /// Immutable ID used to select this saved device; suggested interactively when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Human-readable display name.
        #[arg(long)]
        name: Option<String>,
        /// Searchable tag; repeat to assign more than one.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// ONVIF username; prompted when omitted in an interactive terminal.
        #[arg(long)]
        username: Option<String>,
        /// Read the password from stdin instead of prompting securely.
        #[arg(long)]
        password_stdin: bool,
        /// Save without a live authentication check; may retain an unreachable or incorrectly authenticated device.
        #[arg(long)]
        no_verify: bool,
        /// Do not make the new device the current interactive device.
        #[arg(long)]
        no_use: bool,
    },
    /// Securely set a saved device credential.
    Auth {
        /// Exact saved-device ID or Group-local alias.
        id: String,
        /// ONVIF username; prompted when omitted in an interactive terminal.
        #[arg(long)]
        username: Option<String>,
        /// Read the password from stdin instead of prompting securely.
        #[arg(long)]
        password_stdin: bool,
    },
    /// Show device information using an optional saved-device selector.
    Info {
        /// Exact saved-device ID or Group-local alias; otherwise use the current device interactively.
        id: Option<String>,
    },
    /// Verify device connectivity using an optional saved-device selector.
    Test {
        /// Exact saved-device ID or Group-local alias; otherwise use the current device interactively.
        id: Option<String>,
    },
    /// List media profiles using an optional saved-device selector.
    Profiles {
        /// Exact saved-device ID or Group-local alias; otherwise use the current device interactively.
        id: Option<String>,
    },
    /// Get a stream URI, selecting a profile interactively when needed.
    Stream {
        /// Exact saved-device ID or Group-local alias; otherwise use the current device interactively.
        id: Option<String>,
        /// Exact media profile token; prompted when omitted and multiple profiles exist.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Get a snapshot URI, selecting a profile interactively when needed.
    Snapshot {
        /// Exact saved-device ID or Group-local alias; otherwise use the current device interactively.
        id: Option<String>,
        /// Exact media profile token; prompted when omitted and multiple profiles exist.
        #[arg(long)]
        profile: Option<String>,
    },
    /// List saved IP cameras and their cached identity information.
    List,
    /// List saved devices.
    Devices,
    /// List static Groups.
    Groups,
    /// List dynamic Views.
    Views,
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
        command: Option<HealthCommands>,
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
    /// Scan for ONVIF devices; human terminals open a paged, filterable browser.
    Discover {
        #[command(subcommand)]
        command: Option<DiscoverCommands>,
    },
    /// Inspect and validate the local oxvif registry location.
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Generate a shell completion script on stdout.
    Completion {
        #[arg(value_enum)]
        shell: Shell,
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
enum ConfigCommands {
    /// Print the resolved config, registry, and snapshot paths.
    Path,
    /// Parse and validate the registry and every indexed discovery snapshot.
    Validate,
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
        /// Saved device selector: a global ID or group/local-alias.
        id: Option<String>,
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
        /// Filter returned records; repeat for AND semantics.
        #[arg(long = "filter")]
        filters: Vec<DiscoveryFilter>,
        /// Case-insensitive search of identity, addressing, registration, type, and scope fields.
        #[arg(long)]
        query: Option<String>,
    },
    /// Re-run discovery and atomically replace an existing named snapshot.
    Refresh {
        snapshot: String,
        /// Limit multicast discovery to an interface name or IPv4 address.
        #[arg(long = "interface")]
        interfaces: Vec<String>,
        /// Filter returned records; the complete refreshed snapshot is still saved.
        #[arg(long = "filter")]
        filters: Vec<DiscoveryFilter>,
        /// Case-insensitive search of identity, addressing, registration, type, and scope fields.
        #[arg(long)]
        query: Option<String>,
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
        /// Case-insensitive search of identity, addressing, registration, type, and scope fields.
        #[arg(long)]
        query: Option<String>,
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

fn surface_command_id(command: &Commands) -> CommandId {
    match command {
        Commands::Setup { .. } => CommandId::Setup,
        Commands::Auth { .. } => CommandId::Auth,
        Commands::Info { .. } => CommandId::Info,
        Commands::Test { .. } => CommandId::Test,
        Commands::Profiles { .. } => CommandId::Profiles,
        Commands::Stream { .. } => CommandId::Stream,
        Commands::Snapshot { .. } => CommandId::Snapshot,
        Commands::List => CommandId::List,
        Commands::Devices => CommandId::Devices,
        Commands::Groups => CommandId::Groups,
        Commands::Views => CommandId::Views,
        Commands::Agent { command } => match command {
            AgentCommands::Guide => CommandId::AgentGuide,
            AgentCommands::Prompt => CommandId::AgentPrompt,
        },
        Commands::Describe { .. } => CommandId::Describe,
        Commands::Device { command } => match command {
            DeviceCommands::Add { .. } => CommandId::DeviceAdd,
            DeviceCommands::List => CommandId::DeviceList,
            DeviceCommands::Show { .. } => CommandId::DeviceShow,
            DeviceCommands::Update { .. } => CommandId::DeviceUpdate,
            DeviceCommands::Rename { .. } => CommandId::DeviceRename,
            DeviceCommands::Remove { .. } => CommandId::DeviceRemove,
            DeviceCommands::Import { .. } => CommandId::DeviceImport,
            DeviceCommands::Credential { command } => match command {
                CredentialCommands::Set { .. } => CommandId::DeviceCredentialSet,
                CredentialCommands::Delete { .. } => CommandId::DeviceCredentialDelete,
                CredentialCommands::UseProfile { .. } => CommandId::DeviceCredentialUseProfile,
            },
            DeviceCommands::Test { .. } => CommandId::DeviceTest,
            DeviceCommands::Info { .. } => CommandId::DeviceInfo,
            DeviceCommands::Capabilities { .. } => CommandId::DeviceCapabilities,
            DeviceCommands::Services { .. } => CommandId::DeviceServices,
            DeviceCommands::Refresh { .. } => CommandId::DeviceRefresh,
        },
        Commands::Media { command } => match command {
            MediaCommands::Profiles { .. } => CommandId::MediaProfiles,
            MediaCommands::StreamUri { .. } => CommandId::MediaStreamUri,
            MediaCommands::SnapshotUri { .. } => CommandId::MediaSnapshotUri,
        },
        Commands::Ptz { command } => match command {
            PtzCommands::Status { .. } => CommandId::PtzStatus,
            PtzCommands::Presets { .. } => CommandId::PtzPresets,
        },
        Commands::Health { command: None } => CommandId::Health,
        Commands::Health {
            command: Some(HealthCommands::Check { .. }),
        } => CommandId::HealthCheck,
        Commands::Group { command } => match command {
            GroupCommands::Create { .. } => CommandId::GroupCreate,
            GroupCommands::List => CommandId::GroupList,
            GroupCommands::Show { .. } => CommandId::GroupShow,
            GroupCommands::Delete { .. } => CommandId::GroupDelete,
            GroupCommands::Member { command } => match command {
                GroupMemberCommands::Add { .. } => CommandId::GroupMemberAdd,
                GroupMemberCommands::Remove { .. } => CommandId::GroupMemberRemove,
            },
        },
        Commands::View { command } => match command {
            ViewCommands::Create { .. } => CommandId::ViewCreate,
            ViewCommands::List => CommandId::ViewList,
            ViewCommands::Show { .. } => CommandId::ViewShow,
            ViewCommands::Evaluate { .. } => CommandId::ViewEvaluate,
            ViewCommands::Delete { .. } => CommandId::ViewDelete,
        },
        Commands::Credential {
            command: CredentialRootCommands::Profile { command },
        } => match command {
            CredentialProfileCommands::Set { .. } => CommandId::CredentialProfileSet,
            CredentialProfileCommands::List => CommandId::CredentialProfileList,
            CredentialProfileCommands::Show { .. } => CommandId::CredentialProfileShow,
            CredentialProfileCommands::Delete { .. } => CommandId::CredentialProfileDelete,
        },
        Commands::Discover { command: None } => CommandId::DiscoverScan,
        Commands::Discover {
            command: Some(command),
        } => match command {
            DiscoverCommands::Scan { .. } => CommandId::DiscoverScan,
            DiscoverCommands::Refresh { .. } => CommandId::DiscoverRefresh,
            DiscoverCommands::Enrich { .. } => CommandId::DiscoverEnrich,
            DiscoverCommands::List { .. } => CommandId::DiscoverList,
            DiscoverCommands::Snapshots => CommandId::DiscoverSnapshots,
            DiscoverCommands::Remove { .. } => CommandId::DiscoverRemove,
        },
        Commands::Config { command } => match command {
            ConfigCommands::Path => CommandId::ConfigPath,
            ConfigCommands::Validate => CommandId::ConfigValidate,
        },
        Commands::Completion { .. } => CommandId::Completion,
        Commands::Use { .. } => CommandId::Use,
        Commands::Current => CommandId::Current,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum CliOutputFormat {
    #[default]
    Table,
    Json,
    #[value(name = "jsonl")]
    JsonLines,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum CliClockSyncPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

impl From<CliClockSyncPolicy> for ClockSyncPolicy {
    fn from(value: CliClockSyncPolicy) -> Self {
        match value {
            CliClockSyncPolicy::Auto => Self::Auto,
            CliClockSyncPolicy::Always => Self::Always,
            CliClockSyncPolicy::Never => Self::Never,
        }
    }
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
    let arguments = normalize_human_arguments(arguments);
    let requested_format = requested_output(&arguments);
    let mut cli = match Cli::try_parse_from(&arguments) {
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

    let format = selected_output(cli.output, cli.json, cli.jsonl);
    let _invoked_command_id = surface_command_id(&cli.command);
    if let Commands::Completion { shell } = &cli.command {
        if format.is_structured() {
            let error = AppError::invalid_argument(
                "completion writes a raw shell script and cannot be combined with --output, --json, or --jsonl.",
            );
            emit_error(format, &error, Some("completion"));
            return error.exit_code();
        }
        generate_completion(*shell);
        return 0;
    }

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
    let ca_certificates = match load_ca_certificates(&cli.ca_certificates) {
        Ok(certificates) => certificates,
        Err(error) => {
            emit_error(format, &error, None);
            return error.exit_code();
        }
    };
    let options = ExecutionOptions {
        non_interactive: cli.non_interactive,
        timeout: cli.timeout,
        retries: cli.retries,
        clock_sync: cli.clock_sync.into(),
        ca_certificates,
        verbosity: cli.verbose,
        quiet: cli.quiet,
        jobs,
    };
    let application = match Application::system() {
        Ok(application) => application,
        Err(error) => {
            emit_error(format, &error, None);
            return error.exit_code();
        }
    };
    let prompt = SystemPrompt;
    if matches!(cli.command, Commands::Setup { target: None, .. }) {
        if !interactive_terminal_available(format, &options) {
            let error = AppError::invalid_argument(
                "`oxvif setup` without a target requires an interactive terminal; provide a target and --id for automation.",
            );
            emit_error(format, &error, Some("setup"));
            return error.exit_code();
        }
        return execute_and_emit(
            &application,
            CommandRequest::DiscoverScan(DiscoverScanRequest {
                snapshot_id: None,
                interfaces: Vec::new(),
                filters: Vec::new(),
                query: None,
            }),
            &options,
            format,
            false,
            true,
            &prompt,
        )
        .await;
    }
    if let Err(error) =
        prepare_human_command(&mut cli.command, cli.non_interactive, format, &prompt)
    {
        emit_error(format, &error, None);
        return error.exit_code();
    }
    if let Err(error) = preflight_human_command(&cli.command, &application) {
        emit_error(format, &error, None);
        return error.exit_code();
    }
    let implicit_human_context = quick_command_uses_ambient_device(
        &cli.command,
        cli.device.as_deref(),
        cli.group.as_deref(),
        cli.view.as_deref(),
    );
    let request = match build_request(
        cli.command,
        cli.device,
        cli.group,
        cli.view,
        cli.non_interactive,
        &prompt,
    ) {
        Ok(request) => request,
        Err(error) => {
            emit_error(format, &error, None);
            return error.exit_code();
        }
    };
    let browse_after_discovery = interactive_terminal_available(format, &options)
        && matches!(request, CommandRequest::DiscoverScan(_));
    execute_and_emit(
        &application,
        request,
        &options,
        format,
        implicit_human_context,
        browse_after_discovery,
        &prompt,
    )
    .await
}

async fn execute_and_emit(
    application: &Application,
    request: CommandRequest,
    options: &ExecutionOptions,
    format: OutputFormat,
    implicit_human_context: bool,
    browse_after_discovery: bool,
    prompt: &dyn Prompt,
) -> u8 {
    let command_name = request.name();
    let started = Instant::now();
    emit_verbose_start(options, format, command_name);

    let request = match choose_profile_if_needed(request, application, options, prompt).await {
        Ok(request) => request,
        Err(error) => {
            emit_verbose_error(options, command_name, &error, started);
            emit_error(format, &error, Some(command_name));
            return error.exit_code();
        }
    };
    let command_name = request.name();

    let show_discovery_progress = interactive_terminal_available(format, options)
        && !options.quiet
        && matches!(
            &request,
            CommandRequest::DiscoverScan(_) | CommandRequest::DiscoveryRefresh(_)
        );
    let execution = application.execute(request, options);
    let result = if show_discovery_progress {
        await_discovery(execution).await
    } else {
        execution.await
    };

    match result {
        Ok(success) => {
            if browse_after_discovery
                && let CommandData::DiscoveryScan {
                    devices, summary, ..
                } = &success.data
                && !devices.is_empty()
            {
                match browse_discovery(devices, summary) {
                    Ok(BrowserAction::Quit) => {
                        println!(
                            "Discovery browser closed. Found {} device(s).",
                            summary.total_count
                        );
                        emit_verbose_success(options, command_name, &success, started);
                        return success.exit_code();
                    }
                    Ok(BrowserAction::Add(setup)) => {
                        emit_verbose_success(options, command_name, &success, started);
                        return setup_discovered_device(
                            application,
                            *setup,
                            options,
                            format,
                            prompt,
                        )
                        .await;
                    }
                    Err(error) => {
                        emit_verbose_error(options, command_name, &error, started);
                        emit_error(format, &error, Some(command_name));
                        return error.exit_code();
                    }
                }
            }

            match render_success(format, &success) {
                Ok(rendered) => {
                    if format == OutputFormat::Table
                        && implicit_human_context
                        && !options.quiet
                        && let (Some(device_id), Some(target)) = (
                            success.meta.device_id.as_deref(),
                            success.meta.target.as_deref(),
                        )
                    {
                        println!("Using device: {device_id} ({target})\n\n{rendered}");
                    } else {
                        println!("{rendered}");
                    }
                    emit_verbose_success(options, command_name, &success, started);
                    success.exit_code()
                }
                Err(error) => {
                    emit_verbose_error(options, command_name, &error, started);
                    emit_error(format, &error, Some(command_name));
                    error.exit_code()
                }
            }
        }
        Err(error) => {
            let meta = ResultMeta {
                command: Some(command_name.to_owned()),
                elapsed_ms: elapsed_millis(started),
                ..ResultMeta::default()
            };
            emit_verbose_error(options, command_name, &error, started);
            emit_error_with_meta(format, &error, &meta);
            error.exit_code()
        }
    }
}

async fn setup_discovered_device(
    application: &Application,
    setup: DiscoverySetup,
    options: &ExecutionOptions,
    format: OutputFormat,
    prompt: &dyn Prompt,
) -> u8 {
    let Some(target) = setup
        .device
        .xaddrs
        .iter()
        .find(|target| normalize_target(target).is_ok())
        .cloned()
    else {
        let error = AppError::invalid_argument(
            "The selected discovery record has no usable device-service address.",
        );
        emit_error(format, &error, Some("setup"));
        return error.exit_code();
    };
    let device = NewDevice {
        id: setup.id,
        name: None,
        target,
        tags: Vec::new(),
    };
    if let Err(error) = application.preflight_setup(&device) {
        emit_error(format, &error, Some("setup"));
        return error.exit_code();
    }
    let request = CommandRequest::DeviceSetup(DeviceSetupRequest {
        device,
        username: setup.username,
        password: setup.password,
        verify: true,
        set_current: true,
    });
    Box::pin(execute_and_emit(
        application,
        request,
        options,
        format,
        false,
        false,
        prompt,
    ))
    .await
}

fn interactive_terminal_available(format: OutputFormat, options: &ExecutionOptions) -> bool {
    format == OutputFormat::Table
        && !options.non_interactive
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && io::stderr().is_terminal()
}

fn emit_verbose_start(options: &ExecutionOptions, format: OutputFormat, command: &str) {
    if options.verbosity == 0 {
        return;
    }
    eprintln!(
        "debug: command={command} output={} timeout_ms={} retries={}",
        output_name(format),
        options.timeout.as_millis(),
        options.retries
    );
    if options.verbosity > 1 {
        eprintln!(
            "debug: retry_policy=transient_transport max_attempts={} timeout_scope=per_attempt clock_sync={} custom_ca_bundles={}",
            options.retries.saturating_add(1),
            clock_sync_name(options.clock_sync),
            options.ca_certificates.len()
        );
    }
}

fn emit_verbose_success(
    options: &ExecutionOptions,
    command: &str,
    success: &oxvif_cli::CommandSuccess,
    started: Instant,
) {
    if options.verbosity > 0 {
        eprintln!(
            "debug: command={command} status=ok exit_code={} elapsed_ms={} warnings={}",
            success.exit_code(),
            elapsed_millis(started),
            success.warnings.len()
        );
    }
}

fn emit_verbose_error(
    options: &ExecutionOptions,
    command: &str,
    error: &AppError,
    started: Instant,
) {
    if options.verbosity > 0 {
        eprintln!(
            "debug: command={command} status=error code={} retryable={} elapsed_ms={}",
            error.code.as_str(),
            error.retryable,
            elapsed_millis(started)
        );
    }
}

fn output_name(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Table => "table",
        OutputFormat::Json => "json",
        OutputFormat::JsonLines => "jsonl",
    }
}

fn clock_sync_name(policy: ClockSyncPolicy) -> &'static str {
    match policy {
        ClockSyncPolicy::Auto => "auto",
        ClockSyncPolicy::Always => "always",
        ClockSyncPolicy::Never => "never",
    }
}

fn build_request(
    command: Commands,
    selected_device: Option<String>,
    selected_group: Option<String>,
    selected_view: Option<String>,
    non_interactive: bool,
    prompt: &dyn Prompt,
) -> Result<CommandRequest, AppError> {
    let selector = |target| TargetSelector {
        device: selected_device.clone(),
        target,
        group: selected_group.clone(),
        view: selected_view.clone(),
    };

    let request = match command {
        Commands::Setup {
            target,
            id,
            name,
            tags,
            username,
            password_stdin,
            no_verify,
            no_use,
        } => {
            reject_root_selector(
                selected_device.as_deref(),
                selected_group.as_deref(),
                selected_view.as_deref(),
                "setup",
            )?;
            let (username, password) =
                human_credential_input(username, password_stdin, non_interactive, prompt)?;
            let target = target.ok_or_else(|| {
                AppError::invalid_argument(
                    "Interactive setup discovery requires a terminal; provide a target.",
                )
            })?;
            let id = id.ok_or_else(|| {
                AppError::invalid_argument(
                    "Provide --id under --non-interactive or accept the interactive suggestion.",
                )
            })?;
            Ok(CommandRequest::DeviceSetup(DeviceSetupRequest {
                device: NewDevice {
                    id,
                    name,
                    target,
                    tags,
                },
                username,
                password,
                verify: !no_verify,
                set_current: !no_use,
            }))
        }
        Commands::Auth {
            id,
            username,
            password_stdin,
        } => {
            let (username, password) =
                human_credential_input(username, password_stdin, non_interactive, prompt)?;
            Ok(CommandRequest::DeviceCredentialSet(
                DeviceCredentialSetRequest {
                    id,
                    username,
                    password,
                },
            ))
        }
        Commands::Info { id } => {
            let selector = quick_selector(selector(None), id, non_interactive)?;
            Ok(CommandRequest::DeviceInfo(DeviceConnectRequest {
                selector,
            }))
        }
        Commands::Test { id } => {
            let selector = quick_selector(selector(None), id, non_interactive)?;
            Ok(CommandRequest::DeviceTest(DeviceConnectRequest {
                selector,
            }))
        }
        Commands::Profiles { id } => {
            let selector = quick_selector(selector(None), id, non_interactive)?;
            Ok(CommandRequest::MediaProfiles(DeviceConnectRequest {
                selector,
            }))
        }
        Commands::Stream { id, profile } => {
            let selector = quick_selector(selector(None), id, non_interactive)?;
            Ok(CommandRequest::MediaStreamUri(ProfileConnectRequest {
                selector,
                profile: profile.unwrap_or_default(),
            }))
        }
        Commands::Snapshot { id, profile } => {
            let selector = quick_selector(selector(None), id, non_interactive)?;
            Ok(CommandRequest::MediaSnapshotUri(ProfileConnectRequest {
                selector,
                profile: profile.unwrap_or_default(),
            }))
        }
        Commands::List => Ok(CommandRequest::DeviceList),
        Commands::Devices => Ok(CommandRequest::DeviceList),
        Commands::Groups => Ok(CommandRequest::GroupList),
        Commands::Views => Ok(CommandRequest::ViewList),
        Commands::Agent { command } => Ok(match command {
            AgentCommands::Guide => CommandRequest::AgentGuide,
            AgentCommands::Prompt => CommandRequest::AgentPrompt,
        }),
        Commands::Describe { command } => Ok(CommandRequest::Describe(DescribeRequest { command })),
        Commands::Config { command } => Ok(match command {
            ConfigCommands::Path => CommandRequest::ConfigPath,
            ConfigCommands::Validate => CommandRequest::ConfigValidate,
        }),
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
            None => Ok(CommandRequest::DiscoverScan(DiscoverScanRequest {
                snapshot_id: None,
                interfaces: Vec::new(),
                filters: Vec::new(),
                query: None,
            })),
            Some(command) => match command {
                DiscoverCommands::Scan {
                    save,
                    interfaces,
                    filters,
                    query,
                } => Ok(CommandRequest::DiscoverScan(DiscoverScanRequest {
                    snapshot_id: save,
                    interfaces,
                    filters,
                    query,
                })),
                DiscoverCommands::Refresh {
                    snapshot,
                    interfaces,
                    filters,
                    query,
                } => Ok(CommandRequest::DiscoveryRefresh(DiscoveryRefreshRequest {
                    id: snapshot,
                    interfaces,
                    filters,
                    query,
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
                DiscoverCommands::List {
                    snapshot,
                    filters,
                    query,
                } => Ok(CommandRequest::DiscoverySnapshotShow(
                    DiscoverySnapshotShowRequest {
                        id: snapshot,
                        filters,
                        query,
                    },
                )),
                DiscoverCommands::Snapshots => Ok(CommandRequest::DiscoverySnapshotList),
                DiscoverCommands::Remove { snapshot } => {
                    Ok(CommandRequest::DiscoverySnapshotRemove(ResourceIdRequest {
                        id: snapshot,
                    }))
                }
            },
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
            None => Ok(CommandRequest::HealthCheck(DeviceConnectRequest {
                selector: quick_selector(selector(None), None, non_interactive)?,
            })),
            Some(HealthCommands::Check { id, target }) => {
                let selector = selector_with_positional(selector(target), id)?;
                if non_interactive
                    && selector.device.is_none()
                    && selector.target.is_none()
                    && selector.group.is_none()
                    && selector.view.is_none()
                {
                    return Err(AppError::missing_target());
                }
                Ok(CommandRequest::HealthCheck(DeviceConnectRequest {
                    selector,
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
        Commands::Completion { .. } => {
            return Err(AppError::internal(
                "Completion generation should be handled before application dispatch.",
            ));
        }
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

fn preflight_human_command(command: &Commands, application: &Application) -> Result<(), AppError> {
    match command {
        Commands::Setup {
            id,
            target,
            name,
            tags,
            ..
        } => match (id, target) {
            (Some(id), Some(target)) => application.preflight_setup(&NewDevice {
                id: id.clone(),
                name: name.clone(),
                target: target.clone(),
                tags: tags.clone(),
            }),
            _ => Ok(()),
        },
        Commands::Auth { id, .. } => {
            let canonical_id = application.registry().resolve_device_selector(id)?;
            application.registry().get(&canonical_id).map(|_| ())
        }
        _ => Ok(()),
    }
}

fn prepare_human_command(
    command: &mut Commands,
    non_interactive: bool,
    format: OutputFormat,
    prompt: &dyn Prompt,
) -> Result<(), AppError> {
    let Commands::Setup {
        target: Some(target),
        id,
        name,
        ..
    } = command
    else {
        return Ok(());
    };
    if id.is_some() {
        return Ok(());
    }
    if non_interactive || format.is_structured() {
        return Err(AppError::invalid_argument(
            "Provide --id for setup under --non-interactive or structured output.",
        ));
    }

    let suggestion = suggested_device_id(target, name.as_deref())?;
    let answer = prompt.text(&format!("Device ID [{suggestion}]: "))?;
    *id = Some(if answer.trim().is_empty() {
        suggestion
    } else {
        answer.trim().to_owned()
    });
    Ok(())
}

fn suggested_device_id(target: &str, name: Option<&str>) -> Result<String, AppError> {
    let normalized = normalize_target(target)?;
    let parsed = url::Url::parse(&normalized)
        .map_err(|error| AppError::invalid_argument(format!("Invalid target: {error}")))?;
    let preferred = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(slugify_device_id);
    Ok(preferred.unwrap_or_else(|| {
        slugify_device_id(&format!("camera-{}", parsed.host_str().unwrap_or("device")))
            .unwrap_or_else(|| "camera".to_owned())
    }))
}

fn slugify_device_id(source: &str) -> Option<String> {
    let mut id = String::with_capacity(source.len());
    let mut separator = false;
    for character in source.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || character == '_' {
            if separator && !id.is_empty() {
                id.push('-');
            }
            id.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    (!id.is_empty()).then_some(id)
}

fn reject_root_selector(
    device: Option<&str>,
    group: Option<&str>,
    view: Option<&str>,
    command: &str,
) -> Result<(), AppError> {
    if device.is_some() || group.is_some() || view.is_some() {
        Err(AppError::invalid_argument(format!(
            "`{command}` does not accept --device, --group, or --view."
        )))
    } else {
        Ok(())
    }
}

fn human_credential_input(
    username: Option<String>,
    password_stdin: bool,
    non_interactive: bool,
    prompt: &dyn Prompt,
) -> Result<(String, SecretString), AppError> {
    let username = match username.or_else(|| env::var("OXVIF_USERNAME").ok()) {
        Some(username) => username,
        None if non_interactive => {
            return Err(AppError::invalid_argument(
                "Provide --username or OXVIF_USERNAME under --non-interactive.",
            ));
        }
        None => prompt.text("Username: ")?,
    };
    let username = username.trim().to_owned();
    if username.is_empty() {
        return Err(AppError::invalid_argument("Username must not be empty."));
    }

    let password = if password_stdin {
        read_password_from_stdin()?
    } else if let Ok(password) = env::var("OXVIF_PASSWORD") {
        password
    } else if non_interactive {
        return Err(AppError::invalid_argument(
            "Pass --password-stdin or set OXVIF_PASSWORD under --non-interactive.",
        ));
    } else {
        prompt.password("Password: ")?
    };
    Ok((username, SecretString::new(password)?))
}

fn selected_output(output: Option<CliOutputFormat>, json: bool, jsonl: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if jsonl {
        OutputFormat::JsonLines
    } else {
        output.unwrap_or_default().into()
    }
}

fn quick_command_uses_ambient_device(
    command: &Commands,
    selected_device: Option<&str>,
    selected_group: Option<&str>,
    selected_view: Option<&str>,
) -> bool {
    if selected_device.is_some() || selected_group.is_some() || selected_view.is_some() {
        return false;
    }
    match command {
        Commands::Info { id }
        | Commands::Test { id }
        | Commands::Profiles { id }
        | Commands::Stream { id, .. }
        | Commands::Snapshot { id, .. } => id.is_none(),
        Commands::Health { command: None } => true,
        Commands::Health {
            command: Some(HealthCommands::Check { id, target }),
        } => id.is_none() && target.is_none(),
        _ => false,
    }
}

async fn choose_profile_if_needed(
    request: CommandRequest,
    application: &Application,
    options: &ExecutionOptions,
    prompt: &dyn Prompt,
) -> Result<CommandRequest, AppError> {
    enum Kind {
        Stream,
        Snapshot,
    }

    let (selector, kind) = match &request {
        CommandRequest::MediaStreamUri(request) if request.profile.is_empty() => {
            (request.selector.clone(), Kind::Stream)
        }
        CommandRequest::MediaSnapshotUri(request) if request.profile.is_empty() => {
            (request.selector.clone(), Kind::Snapshot)
        }
        _ => return Ok(request),
    };
    if selector.group.is_some() || selector.view.is_some() {
        return Err(AppError::invalid_argument(
            "Fleet stream/snapshot operations require an explicit --profile.",
        ));
    }

    let profiles = application
        .execute(
            CommandRequest::MediaProfiles(DeviceConnectRequest {
                selector: selector.clone(),
            }),
            options,
        )
        .await?;
    let CommandData::DeviceDiagnostic { result, .. } = profiles.data else {
        return Err(AppError::internal(
            "Media profile selection returned an unexpected result.",
        ));
    };
    let values = result
        .as_array()
        .ok_or_else(|| AppError::internal("Media profile selection did not return an array."))?;
    let profiles = values
        .iter()
        .filter_map(|value| {
            let token = value.get("token")?.as_str()?.to_owned();
            let name = value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("(unnamed)")
                .to_owned();
            Some((token, name))
        })
        .collect::<Vec<_>>();
    if profiles.is_empty() {
        return Err(AppError::invalid_argument(
            "The selected device exposes no usable media profiles.",
        ));
    }
    let profile = if profiles.len() == 1 {
        profiles[0].0.clone()
    } else if options.non_interactive {
        return Err(AppError::invalid_argument(format!(
            "Multiple media profiles are available; pass --profile with one of: {}.",
            profiles
                .iter()
                .map(|(token, _)| token.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    } else {
        let choices = profiles
            .iter()
            .map(|(token, name)| format!("{name} ({token})"))
            .collect::<Vec<_>>();
        profiles[prompt.select("Select a media profile:", &choices)?]
            .0
            .clone()
    };

    Ok(match kind {
        Kind::Stream => CommandRequest::MediaStreamUri(ProfileConnectRequest { selector, profile }),
        Kind::Snapshot => {
            CommandRequest::MediaSnapshotUri(ProfileConnectRequest { selector, profile })
        }
    })
}

fn generate_completion(shell: Shell) {
    write_completion(shell, &mut io::stdout());
}

fn write_completion<W: Write>(shell: Shell, writer: &mut W) {
    let mut command = Cli::command();
    let binary_name = command.get_name().to_owned();
    generate(shell, &mut command, binary_name, writer);
}

fn normalize_human_arguments(mut arguments: Vec<OsString>) -> Vec<OsString> {
    let mut index = 1usize;
    while index < arguments.len() {
        let argument = arguments[index].to_string_lossy();
        if matches!(
            argument.as_ref(),
            "--non-interactive" | "--json" | "--jsonl" | "--quiet" | "-q"
        ) || argument.starts_with("-v")
        {
            index += 1;
            continue;
        }
        if matches!(
            argument.as_ref(),
            "--output"
                | "--device"
                | "--group"
                | "--view"
                | "--jobs"
                | "--timeout"
                | "--retries"
                | "--clock-sync"
                | "--ca-certificate"
        ) {
            index += 2;
            continue;
        }
        if argument.starts_with("--") && argument.contains('=') {
            index += 1;
            continue;
        }
        break;
    }

    // Preserve the pre-0.16 preview syntax (`setup <ID> <TARGET>`) while the
    // public and help-facing form uses `setup <TARGET> --id <ID>`.
    if arguments
        .get(index)
        .is_some_and(|argument| argument == "setup")
        && arguments
            .get(index + 1)
            .is_some_and(|argument| !argument.to_string_lossy().starts_with('-'))
        && arguments
            .get(index + 2)
            .is_some_and(|argument| !argument.to_string_lossy().starts_with('-'))
    {
        let legacy_id = arguments.remove(index + 1);
        arguments.insert(index + 2, OsString::from("--id"));
        arguments.insert(index + 3, legacy_id);
    }

    if arguments
        .get(index)
        .is_some_and(|argument| argument == "health")
    {
        let has_explicit_subcommand = arguments
            .get(index + 1)
            .is_some_and(|argument| argument == "check" || argument == "help");
        if !has_explicit_subcommand {
            arguments.insert(index + 1, OsString::from("check"));
        }

        let mut moved = Vec::new();
        let mut scan = index + 2;
        while scan < arguments.len() {
            let (takes_value, inline_value) = {
                let argument = arguments[scan].to_string_lossy();
                let takes_value = matches!(
                    argument.as_ref(),
                    "--device"
                        | "--group"
                        | "--view"
                        | "--jobs"
                        | "--timeout"
                        | "--retries"
                        | "--clock-sync"
                        | "--ca-certificate"
                );
                let inline_value = [
                    "--device=",
                    "--group=",
                    "--view=",
                    "--jobs=",
                    "--timeout=",
                    "--retries=",
                    "--clock-sync=",
                    "--ca-certificate=",
                ]
                .iter()
                .any(|prefix| argument.starts_with(prefix));
                (takes_value, inline_value)
            };
            if takes_value && scan + 1 < arguments.len() {
                moved.push(arguments.remove(scan));
                moved.push(arguments.remove(scan));
            } else if inline_value {
                moved.push(arguments.remove(scan));
            } else {
                scan += 1;
            }
        }
        arguments.splice(index..index, moved);
    }
    arguments
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

fn quick_selector(
    selector: TargetSelector,
    id: Option<String>,
    non_interactive: bool,
) -> Result<TargetSelector, AppError> {
    let selector = selector_with_positional(selector, id)?;
    if non_interactive
        && selector.device.is_none()
        && selector.target.is_none()
        && selector.group.is_none()
        && selector.view.is_none()
    {
        Err(AppError::missing_target())
    } else {
        Ok(selector)
    }
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
        if argument == "--json" {
            return OutputFormat::Json;
        }
        if argument == "--jsonl" {
            return OutputFormat::JsonLines;
        }
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

fn load_ca_certificates(paths: &[PathBuf]) -> Result<Vec<Vec<u8>>, AppError> {
    let mut bundles = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let bundle = fs::read(path).map_err(|error| {
            AppError::invalid_argument(format!(
                "CA certificate file #{} could not be read: {}.",
                index + 1,
                error.kind()
            ))
        })?;
        bundles.push(bundle);
    }
    if !bundles.is_empty() {
        HttpTransport::new()
            .with_root_certificates_pem(&bundles)
            .map_err(|error| {
                AppError::invalid_argument(format!("Invalid --ca-certificate input: {error}"))
            })?;
    }
    Ok(bundles)
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedPrompt;

    impl Prompt for FixedPrompt {
        fn text(&self, _label: &str) -> Result<String, AppError> {
            Ok("admin".to_owned())
        }

        fn password(&self, _label: &str) -> Result<String, AppError> {
            Ok("prompt-secret".to_owned())
        }

        fn select(&self, _label: &str, _choices: &[String]) -> Result<usize, AppError> {
            Ok(0)
        }
    }

    fn parsed_request(arguments: &[&str]) -> Result<CommandRequest, AppError> {
        let arguments = normalize_human_arguments(arguments.iter().map(OsString::from).collect());
        let cli = Cli::try_parse_from(arguments).expect("arguments should parse");
        build_request(
            cli.command,
            cli.device,
            cli.group,
            cli.view,
            cli.non_interactive,
            &FixedPrompt,
        )
    }

    #[test]
    fn every_catalogue_example_parses_to_its_declared_command_id() {
        for descriptor in oxvif_cli::command_descriptors() {
            let example = descriptor.examples.first().expect("catalogue example");
            let arguments = shlex::split(example).expect("example should have valid shell quoting");
            let arguments =
                normalize_human_arguments(arguments.into_iter().map(OsString::from).collect());
            let cli = Cli::try_parse_from(arguments).unwrap_or_else(|error| {
                panic!("example for {} did not parse: {error}", descriptor.name)
            });
            assert_eq!(
                surface_command_id(&cli.command).canonical(),
                CommandId::from_name(&descriptor.name)
                    .expect("catalogue identity")
                    .canonical(),
                "example mapped to a different command for {}",
                descriptor.name
            );
        }
    }

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
    fn setup_uses_target_first_and_explicit_id_for_automation() {
        let request = parsed_request(&[
            "oxvif",
            "setup",
            "192.0.2.10",
            "--id",
            "front-door",
            "--username",
            "admin",
            "--no-verify",
        ])
        .expect("setup should build");
        let CommandRequest::DeviceSetup(request) = request else {
            panic!("expected setup request");
        };
        assert_eq!(request.device.id, "front-door");
        assert_eq!(request.device.target, "192.0.2.10");
    }

    #[test]
    fn interactive_setup_suggests_a_human_device_id() {
        assert_eq!(
            suggested_device_id("192.168.1.100", None).expect("IP should normalize"),
            "camera-192-168-1-100"
        );
        assert_eq!(
            suggested_device_id("camera.example.test", Some("Front Door"))
                .expect("host should normalize"),
            "front-door"
        );
        assert_eq!(
            suggested_device_id("192.168.1.100", Some("前門攝影機"))
                .expect("non-ASCII name should fall back to the host"),
            "camera-192-168-1-100"
        );
    }

    #[test]
    fn non_interactive_setup_requires_an_explicit_id() {
        let mut command = Commands::Setup {
            target: Some("192.0.2.10".to_owned()),
            id: None,
            name: None,
            tags: Vec::new(),
            username: Some("admin".to_owned()),
            password_stdin: true,
            no_verify: false,
            no_use: false,
        };
        let error = prepare_human_command(&mut command, true, OutputFormat::Table, &FixedPrompt)
            .expect_err("automation should require --id");
        assert_eq!(error.code, oxvif_cli::ErrorCode::InvalidArgument);
        assert!(error.message.contains("--id"));
    }

    #[test]
    fn preview_setup_syntax_remains_compatible() {
        let normalized = normalize_human_arguments(
            [
                "oxvif",
                "setup",
                "front-door",
                "192.0.2.10",
                "--username",
                "admin",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
        );
        let parsed = Cli::try_parse_from(normalized).expect("legacy syntax should normalize");
        let Commands::Setup { target, id, .. } = parsed.command else {
            panic!("expected setup command");
        };
        assert_eq!(target.as_deref(), Some("192.0.2.10"));
        assert_eq!(id.as_deref(), Some("front-door"));
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

    #[test]
    fn human_quick_commands_map_to_canonical_requests() {
        let cases = [
            (&["oxvif", "info", "front-door"][..], "device.info"),
            (&["oxvif", "test", "front-door"][..], "device.test"),
            (&["oxvif", "profiles", "front-door"][..], "media.profiles"),
            (
                &["oxvif", "stream", "front-door", "--profile", "main"][..],
                "media.stream-uri",
            ),
            (
                &["oxvif", "snapshot", "front-door", "--profile", "main"][..],
                "media.snapshot-uri",
            ),
            (&["oxvif", "health", "front-door"][..], "health.check"),
            (&["oxvif", "devices"][..], "device.list"),
            (&["oxvif", "groups"][..], "group.list"),
            (&["oxvif", "views"][..], "view.list"),
        ];

        for (arguments, expected) in cases {
            assert_eq!(
                parsed_request(arguments)
                    .expect("quick command should build")
                    .name(),
                expected
            );
        }
    }

    #[test]
    fn human_positional_selector_conflicts_with_root_selector() {
        let error = parsed_request(&["oxvif", "--device", "other", "info", "front-door"])
            .expect_err("conflicting selectors must fail");
        assert_eq!(error.code, oxvif_cli::ErrorCode::InvalidArgument);
    }

    #[test]
    fn bare_discover_is_an_ephemeral_scan() {
        let request = parsed_request(&["oxvif", "discover"]).expect("discover should build");
        let CommandRequest::DiscoverScan(request) = request else {
            panic!("expected discovery scan");
        };
        assert_eq!(request.snapshot_id, None);
        assert!(request.interfaces.is_empty());
    }

    #[test]
    fn health_shortcut_inserts_the_canonical_check_subcommand() {
        let arguments = normalize_human_arguments(
            ["oxvif", "--timeout", "20s", "health", "front-door"]
                .into_iter()
                .map(OsString::from)
                .collect(),
        );
        assert_eq!(arguments[4], "check");
        assert_eq!(arguments[5], "front-door");
    }

    #[test]
    fn health_shortcut_accepts_fleet_options_after_the_action() {
        let arguments = normalize_human_arguments(
            ["oxvif", "health", "--group", "fleet", "--jobs", "2"]
                .into_iter()
                .map(OsString::from)
                .collect(),
        );
        let cli = Cli::try_parse_from(arguments).expect("health fleet syntax should parse");
        assert_eq!(cli.group.as_deref(), Some("fleet"));
        assert_eq!(cli.jobs, Some(2));
        let request = build_request(
            cli.command,
            cli.device,
            cli.group,
            cli.view,
            cli.non_interactive,
            &FixedPrompt,
        )
        .expect("health fleet request should build");
        let CommandRequest::HealthCheck(request) = request else {
            panic!("expected health request");
        };
        assert_eq!(request.selector.group.as_deref(), Some("fleet"));
    }

    #[test]
    fn output_shorthands_select_structured_formats() {
        let json = Cli::try_parse_from(["oxvif", "--json", "devices"]).expect("json parse");
        assert_eq!(
            selected_output(json.output, json.json, json.jsonl),
            OutputFormat::Json
        );
        let jsonl = Cli::try_parse_from(["oxvif", "devices", "--jsonl"]).expect("jsonl parse");
        assert_eq!(
            selected_output(jsonl.output, jsonl.json, jsonl.jsonl),
            OutputFormat::JsonLines
        );
    }

    #[test]
    fn non_interactive_quick_command_requires_an_explicit_selector() {
        let error = parsed_request(&["oxvif", "info", "--non-interactive"])
            .expect_err("ambient state must not be used by unattended shortcuts");
        assert_eq!(error.code, oxvif_cli::ErrorCode::MissingTarget);
    }

    #[tokio::test]
    async fn quick_stream_selects_a_profile_before_dispatch() {
        let server = oxvif::mock::MockServer::start()
            .await
            .expect("mock server should start");
        let directory = tempfile::tempdir().expect("temp directory");
        let application = Application::with_stores(
            oxvif_cli::RegistryStore::at(directory.path()),
            std::sync::Arc::new(oxvif_cli::MemoryCredentialStore::default()),
        );
        let request = CommandRequest::MediaStreamUri(ProfileConnectRequest {
            selector: TargetSelector {
                target: Some(server.device_url().to_owned()),
                ..TargetSelector::default()
            },
            profile: String::new(),
        });

        let request = choose_profile_if_needed(
            request,
            &application,
            &ExecutionOptions {
                timeout: Duration::from_secs(20),
                ..ExecutionOptions::default()
            },
            &FixedPrompt,
        )
        .await
        .expect("profile should be selected");
        let CommandRequest::MediaStreamUri(request) = request else {
            panic!("expected stream request");
        };
        assert!(!request.profile.is_empty());
    }

    #[test]
    fn completion_scripts_are_generated_for_supported_shells() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
            let mut output = Vec::new();
            write_completion(shell, &mut output);
            let output = String::from_utf8(output).expect("completion must be UTF-8");
            assert!(output.contains("oxvif"));
            assert!(output.contains("info"));
        }
    }
}
