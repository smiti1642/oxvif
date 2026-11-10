//! Verify a private frozen corpus against reviewed parameter-aware expectations.
//!
//! `cargo run --features metamorph --example corpus_regression -- <corpus_dir> <manifest.json>`
//! Inputs must be explicitly supplied; missing captures fail. Keep raw captures
//! outside the checkout. Exit 0: strict selected regression passed; 1: retained
//! failure/incomplete evidence; 2: usage, IO, JSON or resource-limit refusal.
//! This runs offline and does not establish independent protocol conformance.
use oxvif::metamorph::{CorpusLimits, CorpusManifest, FixtureCorpus, verify_corpus};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    ExitCode::from(Box::pin(run()).await)
}

async fn run() -> u8 {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        eprintln!("usage: corpus_regression <corpus_dir> <manifest.json>");
        return 2;
    }
    let limits = CorpusLimits::default();
    let corpus = match FixtureCorpus::load(&arguments[0], limits) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("corpus input refused: {error}");
            return 2;
        }
    };
    let manifest = match CorpusManifest::load(&arguments[1], limits.file_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("manifest input refused: {error}");
            return 2;
        }
    };
    let report = match verify_corpus(&corpus, &manifest).await {
        Ok(report) => report,
        Err(error) => {
            eprintln!("regression input refused: {error}");
            return 2;
        }
    };
    eprintln!("checked {} frozen expectations", report.rows.len());
    match serde_json::to_string(&report) {
        Ok(json) => println!("{json}"),
        Err(_) => {
            eprintln!("could not serialize regression report");
            return 2;
        }
    }
    if report.passed() { 0 } else { 1 }
}
