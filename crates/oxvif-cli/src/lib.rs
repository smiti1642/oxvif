//! Application and output contracts for the `oxvif` executable.
//!
//! The binary is deliberately thin. Future adapters, including a possible MCP
//! server, can construct the same typed requests and consume the same results
//! without parsing command-line arguments or terminal output.

mod application;
mod contract;
mod describe;
mod error;
mod output;

pub use application::{Application, ExecutionOptions};
pub use contract::{
    ArgumentDescriptor, CommandData, CommandDescriptor, CommandRequest, CommandSuccess,
    DescribeRequest, ErrorEnvelope, OutputDescriptor, OutputFormat, ResultMeta, RiskLevel,
    SCHEMA_VERSION, SuccessEnvelope, Warning,
};
pub use error::{AppError, ErrorCode};
pub use output::{render_error, render_success};
