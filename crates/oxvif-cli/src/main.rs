use std::{
    env,
    ffi::OsString,
    io::{self, Read},
    process::ExitCode,
    time::Duration,
};

use clap::{ArgAction, Parser, Subcommand, ValueEnum, error::ErrorKind};
use oxvif_cli::{
    AppError, Application, CommandRequest, DescribeRequest, DeviceAddRequest, DeviceConnectRequest,
    DeviceCredentialSetRequest, DeviceIdRequest, DeviceRenameRequest, DeviceUpdate,
    DeviceUpdateRequest, ExecutionOptions, NewDevice, OutputFormat, ResultMeta, SecretString,
    TargetSelector, render_error, render_success,
};
use tokio::time::Instant;

#[derive(Debug, Parser)]
#[command(
    name = "oxvif",
    version,
    about = "Human- and Agent-friendly ONVIF camera operations"
)]
struct Cli {
    /// Select terminal, JSON, or newline-delimited JSON output.
    #[arg(long, value_enum, default_value_t, global = true)]
    output: CliOutputFormat,

    /// Select a saved device by immutable ID.
    #[arg(long, global = true)]
    device: Option<String>,

    /// Never prompt or open a GUI; fail when required input is missing.
    #[arg(long, global = true)]
    non_interactive: bool,

    /// Bound command execution (supported units: ms, s, m).
    #[arg(
        long,
        default_value = "10s",
        value_parser = parse_duration,
        global = true
    )]
    timeout: Duration,

    /// Number of retries allowed for retryable failures.
    #[arg(long, default_value_t = 0, global = true)]
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
    /// Select the current device for interactive commands.
    Use { id: String },
    /// Show the current interactive device selection.
    Current,
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
        /// Use a direct ONVIF URL, hostname, or IP without saving it.
        #[arg(long)]
        target: Option<String>,
    },
    /// Read live information and update cached registry metadata.
    Refresh { id: String },
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
    let options = ExecutionOptions {
        non_interactive: cli.non_interactive,
        timeout: cli.timeout,
        retries: cli.retries,
        verbosity: cli.verbose,
        quiet: cli.quiet,
    };
    let request = match build_request(cli.command, cli.device) {
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
                0
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
) -> Result<CommandRequest, AppError> {
    let selector = |target| TargetSelector {
        device: selected_device.clone(),
        target,
    };

    match command {
        Commands::Describe { command } => Ok(CommandRequest::Describe(DescribeRequest { command })),
        Commands::Use { id } => Ok(CommandRequest::Use(DeviceIdRequest { id })),
        Commands::Current => Ok(CommandRequest::Current),
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
            DeviceCommands::Credential { command } => match command {
                CredentialCommands::Set {
                    id,
                    username,
                    password_stdin,
                } => {
                    let username = username
                        .or_else(|| env::var("OXVIF_USERNAME").ok())
                        .ok_or_else(|| {
                            AppError::invalid_argument("Provide --username or set OXVIF_USERNAME.")
                        })?;
                    let password = if password_stdin {
                        read_password_from_stdin()?
                    } else {
                        env::var("OXVIF_PASSWORD").map_err(|_| {
                            AppError::invalid_argument(
                                "Pass --password-stdin or set OXVIF_PASSWORD.",
                            )
                        })?
                    };
                    Ok(CommandRequest::DeviceCredentialSet(
                        DeviceCredentialSetRequest {
                            id,
                            username,
                            password: SecretString::new(password)?,
                        },
                    ))
                }
                CredentialCommands::Delete { id } => {
                    Ok(CommandRequest::DeviceCredentialDelete(DeviceIdRequest {
                        id,
                    }))
                }
            },
            DeviceCommands::Test { id, target } => {
                let mut selector = selector(target);
                if let Some(id) = id {
                    if selector.device.is_some() || selector.target.is_some() {
                        return Err(AppError::invalid_argument(
                            "A positional device ID cannot be combined with --device or --target.",
                        ));
                    }
                    selector.device = Some(id);
                }
                Ok(CommandRequest::DeviceTest(DeviceConnectRequest {
                    selector,
                }))
            }
            DeviceCommands::Info { target } => {
                Ok(CommandRequest::DeviceInfo(DeviceConnectRequest {
                    selector: selector(target),
                }))
            }
            DeviceCommands::Refresh { id } => {
                Ok(CommandRequest::DeviceRefresh(DeviceIdRequest { id }))
            }
        },
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
}
