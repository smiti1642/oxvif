//! Compare selected reference values in two private frozen corpora, offline.
//!
//! `cargo run --features metamorph --example reference_compare -- <left_dir> <right_dir>`
//! Both contain `fixtures.json`; loading is bounded and preserves duplicate rows.
//! Progress goes to stderr and deterministic compact JSON to stdout.
//! Exit 0: nonempty selected projections equal; 1: different/missing requests;
//! 2: unresolved, device refusal or empty evidence; 3: usage/input/resource error.
//! Equal covers only the declared projection; inspect unexamined fields and masks.
use oxvif::metamorph::{
    ComparisonOptions, ComparisonOutcome, FixtureCorpus, compare_corpora_with_progress,
};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    ExitCode::from(Box::pin(run()).await)
}

async fn run() -> u8 {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("usage: reference_compare <left_dir> <right_dir>");
        return 3;
    }
    let options = ComparisonOptions::default();
    let load = |path| FixtureCorpus::load(path, options.limits.input);
    let (left, right) = match (load(&args[0]), load(&args[1])) {
        (Ok(left), Ok(right)) => (left, right),
        _ => {
            eprintln!("comparison input unavailable or refused");
            return 3;
        }
    };
    let report = match compare_corpora_with_progress(&left, &right, &options, |event| {
        eprintln!(
            "compared request group {}/{}: {:?}",
            event.completed, event.total, event.outcome
        )
    })
    .await
    {
        Ok(report) => report,
        Err(error) => {
            eprintln!("comparison refused: {error}");
            return 3;
        }
    };
    match serde_json::to_string(&report) {
        Ok(json) => println!("{json}"),
        Err(_) => {
            eprintln!("cannot serialize comparison report");
            return 3;
        }
    }
    if report.all_equal() {
        0
    } else if report.rows.is_empty()
        || report.rows.iter().any(|row| {
            !matches!(
                row.outcome,
                ComparisonOutcome::Equal
                    | ComparisonOutcome::Different
                    | ComparisonOutcome::MissingLeft
                    | ComparisonOutcome::MissingRight
            )
        })
    {
        2
    } else {
        1
    }
}
