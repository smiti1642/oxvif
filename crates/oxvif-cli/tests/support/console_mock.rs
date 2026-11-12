//! Run the same mock camera as the CLI's event tests for the OxDM integration suite.
use std::io::{self, BufRead, Write};

use oxvif::mock::{MockServer, state::PendingIoEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1);
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
