//! Configurable multi-camera test fleet. Run with `--help` for commands.
//! Requires `mock-server`; not part of the installed oxvif CLI.

mod config;

use config::Manifest;
use std::path::Path;

const HELP: &str = "Mock Fleet (test service; no RTSP)\n\
  mock_fleet_serve init <manifest.toml> [--count 4] [--base-port 18080]\n\
  mock_fleet_serve check <manifest.toml>\n\
  mock_fleet_serve serve <manifest.toml>\n\
init never overwrites files; check does not bind ports.\n\
Default: loopback HTTP, no discovery, authentication not enforced.\n\
Edit the manifest to explicitly enable lab-network access. State changes are not saved.";

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("Mock Fleet: {error}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        println!("{HELP}");
        return Ok(());
    }
    let Some(path) = args.get(1) else {
        return Err("A manifest path is required; use --help".into());
    };
    match args[0].as_str() {
        "init" => {
            let (mut count, mut port) = (4, 18080);
            let (mut seen_count, mut seen_port) = (false, false);
            for option in args[2..].chunks(2) {
                let [key, value] = option else {
                    return Err("Every init option needs a value".into());
                };
                match key.as_str() {
                    "--count" if !seen_count => {
                        count = value.parse().map_err(|_| "Invalid count")?;
                        seen_count = true;
                    }
                    "--base-port" if !seen_port => {
                        port = value.parse().map_err(|_| "Invalid base port")?;
                        seen_port = true;
                    }
                    _ => return Err("Unknown or repeated init option; use --help".into()),
                }
            }
            Manifest::generated(count, port)?.write_new(Path::new(path))?;
            println!("Created manifest for {count} cameras. Edit it, then run check and serve.");
            Ok(())
        }
        "check" if args.len() == 2 => {
            let manifest = Manifest::load(Path::new(path))?;
            let states = manifest.prepare(Path::new(path))?;
            println!(
                "Valid manifest: {} cameras; no listeners started. Authentication is not enforced by this runner.",
                states.len()
            );
            Ok(())
        }
        "serve" if args.len() == 2 => {
            Err("Serving is not available in the configuration-only implementation batch".into())
        }
        _ => Err("Unknown command or unexpected arguments; use --help".into()),
    }
}
