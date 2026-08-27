use std::time::{Duration, Instant};

use crate::{AppError, CommandRequest, CommandSuccess, ResultMeta, describe};

/// Invocation policy shared by CLI and future non-CLI adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOptions {
    pub non_interactive: bool,
    pub timeout: Duration,
    pub retries: u32,
    pub verbosity: u8,
    pub quiet: bool,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            non_interactive: false,
            timeout: Duration::from_secs(10),
            retries: 0,
            verbosity: 0,
            quiet: false,
        }
    }
}

/// Executes typed oxvif CLI commands without depending on terminal parsing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Application;

impl Application {
    pub fn execute(
        &self,
        request: CommandRequest,
        _options: &ExecutionOptions,
    ) -> Result<CommandSuccess, AppError> {
        let started = Instant::now();
        let command_name = request.name();

        let data = match request {
            CommandRequest::Describe(request) => describe::execute(request)?,
        };

        Ok(CommandSuccess {
            data,
            warnings: Vec::new(),
            meta: ResultMeta {
                command: Some(command_name.to_owned()),
                elapsed_ms: elapsed_millis(started),
                ..ResultMeta::default()
            },
        })
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
