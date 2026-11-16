//! Run the same mock camera as the CLI's event tests for the OxDM integration suite.
use std::io::{self, BufRead, Write};

use oxvif::mock::{MockServer, state::PendingIoEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1);
    if mode.as_deref() == Some("--terminal-fixture") {
        use std::io::IsTerminal;
        let isolated = ["OXVIF_USERNAME", "OXVIF_PASSWORD", "OXVIF_DEVICE"]
            .iter()
            .all(|name| std::env::var_os(name).is_none());
        let result = serde_json::json!({
            "stdin": io::stdin().is_terminal(),
            "stdout": io::stdout().is_terminal(),
            "stderr": io::stderr().is_terminal(),
            "isolated": isolated,
            "args": std::env::args().skip(3).collect::<Vec<_>>(),
        });
        let destination = std::env::args_os().nth(2).expect("fixture output file");
        std::fs::write(destination, serde_json::to_vec(&result)?)?;
        return Ok(());
    }
    if mode.as_deref() == Some("--credential-pipe-fixture") {
        use std::io::Read;
        let mut password = zeroize::Zeroizing::new(String::new());
        io::stdin().read_to_string(&mut password)?;
        let username = std::env::var("OXVIF_USERNAME")?;
        let argv = std::env::args().collect::<Vec<_>>();
        if !argv.iter().any(|arg| arg == "--password-stdin")
            || argv
                .iter()
                .any(|arg| arg.contains(username.as_str()) || arg.contains(password.as_str()))
            || std::env::var_os("OXVIF_PASSWORD").is_some()
            || username != "synthetic-account"
            || password.as_str() != "synthetic-password"
        {
            std::process::exit(3);
        }
        println!("{username}");
        eprintln!("{}", password.as_str());
        return Ok(());
    }
    if mode.as_deref() == Some("--seed-import") {
        let store = oxvif_cli::RegistryStore::system()?;
        store.save_discovery_snapshot(
            "console-fixture",
            vec![oxvif_cli::DiscoveryRecord {
                endpoint: "uuid:console-camera".into(),
                xaddrs: vec!["http://192.0.2.120/onvif/device_service".into()],
                types: Vec::new(),
                scopes: Vec::new(),
                manufacturer: None,
                model: None,
                firmware_version: None,
                serial_number: None,
            }],
        )?;
        return Ok(());
    }
    if mode.as_deref() == Some("--echo-argv") {
        let arguments: Vec<_> = std::env::args().skip(2).collect();
        serde_json::to_writer(io::stdout(), &arguments)?;
        return Ok(());
    }
    if matches!(mode.as_deref(), Some("--silent-exit" | "--ignore-stop")) {
        println!(
            r#"{{"schema_version":"3","data":{{"kind":"event_notification","sequence":1,"event":{{"topic":"VideoSource/MotionAlarm"}}}}}}"#
        );
        io::stdout().flush()?;
        if mode.as_deref() == Some("--ignore-stop") {
            std::future::pending::<()>().await;
        }
        return Ok(());
    }
    let server = MockServer::builder()
        .enforce_auth(matches!(mode.as_deref(), Some("--auth" | "--auth-empty")))
        .start()
        .await?;
    if mode.as_deref() == Some("--auth-empty") {
        server.device().modify(|device| {
            device
                .users
                .iter_mut()
                .find(|user| user.username == "admin")
                .expect("the fixture has an admin user")
                .password
                .clear();
        });
    }
    println!("{}", server.device_url());
    io::stdout().flush()?;
    for line in io::stdin().lock().lines() {
        match line?.as_str() {
            "burst" => server.device().modify(|device| {
                device
                    .pending_io_events
                    .extend((0..96).map(|index| PendingIoEvent {
                        kind: "DigitalInput",
                        token: format!("console-input-{index}"),
                        logical_state: "active".into(),
                    }));
            }),
            "fail_pull" => server.inject_fault(
                "PullMessagesRequest",
                "ter:NotAuthorized",
                "console fixture",
            ),
            _ => {}
        }
    }
    Ok(())
}
