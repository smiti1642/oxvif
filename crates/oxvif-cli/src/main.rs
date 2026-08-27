use std::{
    env,
    ffi::OsString,
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::{ArgAction, Parser, Subcommand, ValueEnum, error::ErrorKind};
use oxvif_cli::{
    AppError, Application, CommandRequest, DescribeRequest, ExecutionOptions, OutputFormat,
    ResultMeta, render_error, render_success,
};

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
        /// Stable dotted command name, for example `media.stream-uri`.
        command: Option<String>,
    },
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

fn main() -> ExitCode {
    ExitCode::from(run(env::args_os().collect()))
}

fn run(arguments: Vec<OsString>) -> u8 {
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
    let request = match cli.command {
        Commands::Describe { command } => CommandRequest::Describe(DescribeRequest { command }),
    };
    let command_name = request.name();
    let started = Instant::now();

    match Application.execute(request, &options) {
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
}
