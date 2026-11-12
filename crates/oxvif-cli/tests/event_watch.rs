use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use oxvif::{
    OnvifSession,
    events::EndpointPolicy,
    mock::{MockServer, state::PendingIoEvent},
};
use serde_json::Value;
use tokio::time::{sleep, timeout};

fn command(config: &Path, target: &str, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxvif"));
    command
        .args(["events", "watch", "--target", target, "--non-interactive"])
        .args(arguments)
        .env("OXVIF_CONFIG_DIR", config)
        .env_remove("OXVIF_DEVICE")
        .env_remove("OXVIF_USERNAME")
        .env_remove("OXVIF_PASSWORD");
    command
}

fn records(bytes: &[u8]) -> Vec<Value> {
    let schema =
        serde_json::from_str(include_str!("../schema/oxvif-envelope.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| {
            let value = serde_json::from_str(line).unwrap();
            assert!(validator.is_valid(&value), "invalid event envelope {value}");
            value
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filtered_watch_emits_ordered_jsonl_and_a_timed_summary() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    let output = command(
        config.path(),
        server.device_url(),
        &[
            "--duration",
            "1s",
            "--topic",
            "VideoSource/MotionAlarm",
            "--jsonl",
        ],
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let records = records(&output.stdout);
    let (summary, notifications) = records.split_last().unwrap();
    assert!(!notifications.is_empty());
    for (index, line) in notifications.iter().enumerate() {
        assert_eq!(line["data"]["kind"], "event_notification");
        assert_eq!(line["data"]["sequence"], index + 1);
        assert_eq!(
            line["data"]["event"]["topic"],
            "tns1:VideoSource/MotionAlarm"
        );
        assert_eq!(
            line["data"]["event"]["topic_namespace"],
            "http://www.onvif.org/ver10/topics"
        );
    }
    assert_eq!(summary["data"]["kind"], "event_watch_summary");
    assert_eq!(summary["data"]["summary"]["reason"], "duration_elapsed");
    assert_eq!(summary["data"]["summary"]["complete"], true);
    assert_eq!(
        summary["data"]["summary"]["counts"]["delivered"],
        notifications.len()
    );
    assert_eq!(summary["data"]["summary"]["cleanup_failed"], false);

    let invalid = command(
        config.path(),
        server.device_url(),
        &["--topic", "other:Invalid", "--jsonl"],
    )
    .output()
    .unwrap();
    assert_eq!(
        records_for_error(&invalid.stdout)["error"]["code"],
        "INVALID_ARGUMENT"
    );
}

fn records_for_error(bytes: &[u8]) -> Value {
    let mut values = records(bytes);
    assert_eq!(values.len(), 1);
    values.pop().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdin_stop_and_eof_share_the_graceful_cleanup_path() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    for (send_line, fail_cleanup) in [(true, false), (false, false), (true, true)] {
        let mut invocation = command(
            config.path(),
            server.device_url(),
            &["--duration", "30s", "--jsonl", "--control-stdin"],
        );
        invocation.stdin(Stdio::piped());
        let mut child = WatchProcess::start(invocation);
        let (mut output, first) = child.first_line().await;
        assert_eq!(
            records(first.as_bytes())[0]["data"]["kind"],
            "event_notification"
        );
        if fail_cleanup {
            server.inject_fault("UnsubscribeRequest", "ter:NotAuthorized", "cleanup fixture");
        }
        let mut stdin = child.0.stdin.take().unwrap();
        if send_line {
            stdin.write_all(b"ignore\r\nstop\r\n").unwrap();
        } else {
            drop(stdin);
        }
        let rest = tokio::task::spawn_blocking(move || {
            let mut rest = Vec::new();
            output.read_to_end(&mut rest).unwrap();
            rest
        });
        assert_eq!(
            child.wait(Duration::from_secs(8)).await.code(),
            Some(if fail_cleanup { 6 } else { 0 })
        );
        let lines = records(&rest.await.unwrap());
        assert_eq!(
            lines
                .iter()
                .filter(|line| line["data"]["kind"] == "event_watch_summary")
                .count(),
            1
        );
        let summary = &lines.last().unwrap()["data"]["summary"];
        assert_eq!(summary["reason"], "interrupted");
        assert_eq!(summary["complete"], true);
        assert_eq!(summary["cleanup_failed"], fail_cleanup);
    }
}

#[tokio::test]
async fn stop_interrupts_a_connection_that_has_not_yet_granted_a_subscription() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}/onvif/device", listener.local_addr().unwrap());
    let config = tempfile::tempdir().unwrap();
    let mut invocation = command(
        config.path(),
        &target,
        &["--duration", "30s", "--jsonl", "--control-stdin"],
    );
    invocation.stdin(Stdio::piped());
    let mut child = WatchProcess::start(invocation);
    // Keep the connection open without sending a response. Stop must win
    // while session initialization is still waiting on the camera.
    let (_connection, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("CLI should connect to the fixture")
        .unwrap();
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"stop\n")
        .unwrap();
    assert!(child.wait(Duration::from_secs(3)).await.success());
    let mut output = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    let lines = records(&output);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["data"]["kind"], "event_watch_summary");
    let summary = &lines[0]["data"]["summary"];
    assert_eq!(summary["reason"], "interrupted");
    assert_eq!(summary["counts"]["delivered"], 0);
    assert_eq!(summary["complete"], true);
    assert_eq!(summary["cleanup_failed"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_waits_for_an_in_flight_create_then_releases_the_granted_lease() {
    let (granted, lease_committed) = tokio::sync::oneshot::channel();
    let (reply, permit_reply) = mpsc::channel();
    let gate = Mutex::new(Some((granted, permit_reply)));
    let server = MockServer::builder()
        .on_change(Arc::new(move |_| {
            // The mock fires the first change hook after committing Create and
            // before sending its HTTP response.
            let permit = gate.lock().unwrap().take();
            if let Some((granted, permit_reply)) = permit {
                let _ = granted.send(());
                tokio::task::block_in_place(|| {
                    let _ = permit_reply.recv_timeout(Duration::from_secs(5));
                });
            }
        }))
        .start()
        .await
        .unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut invocation = command(
        config.path(),
        server.device_url(),
        &["--duration", "30s", "--jsonl", "--control-stdin"],
    );
    invocation.stdin(Stdio::piped());
    let mut child = WatchProcess::start(invocation);
    timeout(Duration::from_secs(5), lease_committed)
        .await
        .unwrap()
        .unwrap();
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"stop\n")
        .unwrap();
    sleep(Duration::from_millis(150)).await;
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "stop abandoned the pending Create reply"
    );
    reply.send(()).unwrap();

    assert!(child.wait(Duration::from_secs(5)).await.success());
    let mut output = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    let lines = records(&output);
    let summary = &lines.last().unwrap()["data"]["summary"];
    assert_eq!(summary["reason"], "interrupted");
    assert_eq!(summary["cleanup_failed"], false);
    assert_all_subscription_slots_available(&server).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finite_watch_exits_with_the_control_pipe_still_open() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut invocation = command(
        config.path(),
        server.device_url(),
        &["--duration", "1s", "--control-stdin"],
    );
    invocation.stdin(Stdio::piped());
    let mut child = WatchProcess::start(invocation);
    let _open_pipe = child.0.stdin.take().unwrap();
    assert!(child.wait(Duration::from_secs(8)).await.success());
    let mut output = String::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert!(output.contains("DurationElapsed"), "{output}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_interrupt_still_stops_a_watch_without_stdin_control() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut child = WatchProcess::start(command(
        config.path(),
        server.device_url(),
        &["--duration", "30s", "--jsonl"],
    ));
    let (mut output, first) = child.first_line().await;
    assert_eq!(
        records(first.as_bytes())[0]["data"]["kind"],
        "event_notification"
    );
    assert!(
        Command::new("kill")
            .args(["-s", "INT", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let rest = tokio::task::spawn_blocking(move || {
        let mut rest = Vec::new();
        output.read_to_end(&mut rest).unwrap();
        rest
    });
    assert!(child.wait(Duration::from_secs(5)).await.success());
    let lines = records(&rest.await.unwrap());
    let summary = &lines.last().unwrap()["data"]["summary"];
    assert_eq!(summary["reason"], "interrupted");
    assert_eq!(summary["cleanup_failed"], false);
    assert_all_subscription_slots_available(&server).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_subscription_reports_an_explicit_jsonl_gap() {
    let server = MockServer::start().await.unwrap();
    server.inject_fault(
        "PullMessagesRequest",
        "ter:NotAuthorized",
        "private fixture detail",
    );
    let config = tempfile::tempdir().unwrap();
    let output = command(
        config.path(),
        server.device_url(),
        &["--duration", "4s", "--jsonl"],
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(20), "{output:?}");
    let lines = records(&output.stdout);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["data"]["kind"], "event_gap");
    assert_eq!(lines[0]["data"]["reason"], "subscription_stopped");
    assert!(lines[0]["data"]["device_loss_count"].is_null());
    assert_eq!(lines[1]["data"]["summary"]["complete"], false);
    assert_eq!(lines[1]["data"]["summary"]["cleanup_failed"], false);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private fixture detail"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn event_burst_updates_a_bounded_terminal_summary() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut child = WatchProcess::start(command(
        config.path(),
        server.device_url(),
        &["--duration", "4s"],
    ));
    let (mut output, first) = child.first_line().await;
    assert!(first.starts_with("Events "));
    burst(&server, 96, "");
    assert!(child.wait(Duration::from_secs(8)).await.success());
    let mut rest = String::new();
    output.read_to_string(&mut rest).unwrap();
    let final_line = rest.lines().last().unwrap();
    assert!(final_line.contains("DurationElapsed"), "{rest}");
    assert!(final_line.contains("DigitalInput 96"), "{rest}");
    assert!(rest.lines().count() <= 6, "{rest}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blocked_stdout_times_out_and_releases_the_subscription() {
    let server = MockServer::start().await.unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut child = WatchProcess::start(command(
        config.path(),
        server.device_url(),
        &["--duration", "15s", "--jsonl"],
    ));
    let (_unread_pipe, first) = child.first_line().await;
    assert_eq!(
        serde_json::from_str::<Value>(&first).unwrap()["data"]["kind"],
        "event_notification"
    );
    burst(&server, 120, &"x".repeat(6500));
    assert_eq!(child.wait(Duration::from_secs(9)).await.code(), Some(20));
    let mut stderr = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.contains("OutputTimeout"), "{stderr}");
    assert!(
        stderr.contains("Subscription cleanup confirmed"),
        "{stderr}"
    );

    assert_all_subscription_slots_available(&server).await;
}

async fn assert_all_subscription_slots_available(server: &MockServer) {
    // Opening all four slots proves the CLI's lease is gone on the camera.
    let session = OnvifSession::builder(server.device_url())
        .build()
        .await
        .unwrap();
    let client = session.client();
    let events_url = session.capabilities().events.url.as_deref().unwrap();
    let mut points = Vec::new();
    for _ in 0..4 {
        points.push(
            client
                .create_referenced_pull_point(
                    events_url,
                    "PT60S",
                    &EndpointPolicy::default(),
                    Duration::from_secs(2),
                )
                .await
                .unwrap(),
        );
    }
    for point in points {
        client
            .unsubscribe_referenced_pull_point(
                &point.reference,
                &EndpointPolicy::default(),
                Duration::from_secs(2),
            )
            .await
            .unwrap();
    }
}

fn burst(server: &MockServer, count: usize, padding: &str) {
    server.device().modify(|device| {
        device
            .pending_io_events
            .extend((0..count).map(|index| PendingIoEvent {
                kind: "DigitalInput",
                token: format!("input-{index}{padding}"),
                logical_state: if index % 2 == 0 { "active" } else { "inactive" }.into(),
            }));
    });
}

struct WatchProcess(Child);

impl WatchProcess {
    fn start(mut command: Command) -> Self {
        Self(
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        )
    }

    async fn first_line(&mut self) -> (BufReader<ChildStdout>, String) {
        let mut reader = BufReader::new(self.0.stdout.take().unwrap());
        let (reader, line) = timeout(
            Duration::from_secs(10),
            tokio::task::spawn_blocking(move || {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                (reader, line)
            }),
        )
        .await
        .expect("watch should emit while it is running")
        .unwrap();
        assert!(
            self.0.try_wait().unwrap().is_none(),
            "watch exited before its first line"
        );
        (reader, line)
    }

    async fn wait(&mut self, budget: Duration) -> ExitStatus {
        timeout(budget, async {
            loop {
                if let Some(status) = self.0.try_wait().unwrap() {
                    return status;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("watch should exit within its output and cleanup budgets")
    }
}

impl Drop for WatchProcess {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
