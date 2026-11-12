use std::{
    fmt::Write as _,
    io::{self, BufRead, Write},
    thread,
    time::Duration,
};

use oxvif::events::{EventItem, QualifiedNotification};
use oxvif_cli::{
    AppError, Application, EventWatchCounts, EventWatchResult, EventWatchUpdate, ExecutionOptions,
    OutputFormat, ResultMeta, SCHEMA_VERSION, WatchEventsRequest,
};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

const FINAL_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) async fn run(
    application: &Application,
    request: WatchEventsRequest,
    options: &ExecutionOptions,
    format: OutputFormat,
    control_stdin: bool,
) -> u8 {
    let control = if control_stdin {
        let (stop, receiver) = oneshot::channel();
        // A blocking stdin read must not keep the Tokio runtime alive on
        // natural completion, when the parent may still hold the pipe open.
        if thread::Builder::new()
            .name("oxvif-event-control".into())
            .spawn(move || {
                read_control(io::stdin().lock());
                let _ = stop.send(());
            })
            .is_err()
        {
            let error = AppError::internal("Could not start the event control reader.");
            crate::emit_error(format, &error, Some("events.watch"));
            return error.exit_code();
        }
        Some(receiver)
    } else {
        None
    };
    let mut writer = match LineWriter::stdout() {
        Ok(writer) => writer,
        Err(_) => {
            let error = AppError::internal("Could not start the event output writer.");
            crate::emit_error(format, &error, Some("events.watch"));
            return error.exit_code();
        }
    };
    let result = application
        .watch_events(
            request,
            options,
            async {
                tokio::select! {
                    () = async {
                        if tokio::signal::ctrl_c().await.is_err() {
                            std::future::pending::<()>().await;
                        }
                    } => {}
                    () = async {
                        if let Some(control) = control {
                            let _ = control.await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {}
                }
            },
            async |meta, update| {
                if let Some(line) = update_line(format, options.quiet, meta, update) {
                    writer.write(line).await?;
                }
                Ok(())
            },
        )
        .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            crate::emit_error(format, &error, Some("events.watch"));
            return error.exit_code();
        }
    };
    if result.output_available
        && matches!(
            tokio::time::timeout(
                FINAL_WRITE_TIMEOUT,
                writer.write(summary_line(format, &result))
            )
            .await,
            Ok(Ok(()))
        )
    {
        return result.exit_code();
    }
    if let Ok(mut stderr) = LineWriter::start(|line| write_line(io::stderr().lock(), line)) {
        let line = format!(
            "Event watch output unavailable. {}",
            summary_line(OutputFormat::Table, &result)
        );
        let _ = tokio::time::timeout(FINAL_WRITE_TIMEOUT, stderr.write(line)).await;
    }
    20
}

fn read_control(mut input: impl BufRead) {
    let mut line = [0; 5];
    let mut len = 0;
    loop {
        let bytes = match input.fill_buf() {
            Ok([]) | Err(_) => return,
            Ok(bytes) => bytes,
        };
        for &byte in bytes {
            if byte == b'\n' {
                if len == 4 && &line[..4] == b"stop" || len == 5 && &line == b"stop\r" {
                    return;
                }
                len = 0;
            } else if len < line.len() {
                line[len] = byte;
                len += 1;
            } else {
                len = line.len() + 1;
            }
        }
        let read = bytes.len();
        input.consume(read);
    }
}

fn update_line(
    format: OutputFormat,
    quiet: bool,
    meta: &ResultMeta,
    update: EventWatchUpdate,
) -> Option<String> {
    match update {
        EventWatchUpdate::Notification { sequence, event } if format == OutputFormat::JsonLines => {
            Some(envelope(
                true,
                meta,
                json!({
                    "kind": "event_notification",
                    "sequence": sequence,
                    "event": event_data(&event),
                }),
            ))
        }
        EventWatchUpdate::Gap {
            after_sequence,
            reason,
        } if format == OutputFormat::JsonLines => Some(envelope(
            false,
            meta,
            json!({
                "kind": "event_gap",
                "after_sequence": after_sequence,
                "reason": reason,
                "device_loss_count": null,
            }),
        )),
        EventWatchUpdate::Gap {
            after_sequence,
            reason,
        } if format == OutputFormat::Table => Some(format!(
            "Event gap after {after_sequence} delivered ({reason:?})."
        )),
        EventWatchUpdate::Progress(counts) if format == OutputFormat::Table && !quiet => {
            Some(counts_line(&counts))
        }
        _ => None,
    }
}

fn event_data(event: &QualifiedNotification) -> Value {
    let topic = event.topic_expression();
    let topic_namespace = event.topic.as_ref().and_then(|node| {
        let root = topic.as_deref()?.split('/').next()?;
        let prefix = root.split_once(':').map_or("", |(prefix, _)| prefix);
        node.namespace_for_prefix(prefix)
    });
    let dialect = event.topic.as_ref().and_then(|node| {
        node.attributes()
            .iter()
            .find(|attr| attr.namespace().is_empty() && attr.local_name() == "Dialect")
            .map(|attr| attr.value())
    });
    json!({
        "topic": topic,
        "topic_namespace": topic_namespace,
        "topic_dialect": dialect,
        "utc_time": event.utc_time,
        "property_operation": event.property_operation,
        "source": items(&event.source),
        "key": items(&event.key),
        "data": items(&event.data),
    })
}

fn items(items: &[EventItem]) -> Vec<Value> {
    items
        .iter()
        .map(|item| match item {
            EventItem::Simple { name, value } => {
                json!({"kind": "simple", "name": name, "value": value})
            }
            EventItem::Element(element) => json!({"kind": "element", "xml": element.to_xml()}),
            _ => json!({"kind": "unknown"}),
        })
        .collect()
}

fn summary_line(format: OutputFormat, result: &EventWatchResult) -> String {
    let summary = &result.summary;
    if format.is_structured() {
        return envelope(
            result.exit_code() == 0,
            &result.meta,
            json!({"kind": "event_watch_summary", "summary": summary}),
        );
    }
    format!(
        "Event watch ended ({:?}). {} Known undelivered {}. Subscription cleanup {}.",
        summary.reason,
        counts_line(&summary.counts),
        summary.known_undelivered,
        if summary.cleanup_failed {
            "unconfirmed"
        } else {
            "confirmed"
        },
    )
}

fn counts_line(counts: &EventWatchCounts) -> String {
    let mut text = format!("Events {} · gaps {}", counts.delivered, counts.gaps);
    for (topic, count) in &counts.topics {
        let _ = write!(text, " · {topic} {count}");
    }
    if counts.other_topics > 0 {
        let _ = write!(text, " · other {}", counts.other_topics);
    }
    text
}

fn envelope(ok: bool, meta: &ResultMeta, data: Value) -> String {
    json!({
        "schema_version": SCHEMA_VERSION,
        "ok": ok,
        "data": data,
        "warnings": [],
        "meta": meta,
    })
    .to_string()
}

struct LineWriter(mpsc::Sender<Line>);

struct Line {
    text: String,
    written: oneshot::Sender<io::Result<()>>,
}

impl LineWriter {
    fn stdout() -> io::Result<Self> {
        Self::start(|line| write_line(io::stdout().lock(), line))
    }

    fn start(mut output: impl FnMut(&str) -> io::Result<()> + Send + 'static) -> io::Result<Self> {
        let (send, mut receive) = mpsc::channel::<Line>(1);
        // A synchronous pipe write can block indefinitely. A dedicated thread
        // keeps the runtime and unsubscribe live and does not delay process exit.
        thread::Builder::new()
            .name("oxvif-event-output".into())
            .spawn(move || {
                while let Some(line) = receive.blocking_recv() {
                    let result = output(&line.text);
                    let failed = result.is_err();
                    let _ = line.written.send(result);
                    if failed {
                        break;
                    }
                }
            })?;
        Ok(Self(send))
    }

    async fn write(&mut self, text: String) -> io::Result<()> {
        let (written, acknowledgement) = oneshot::channel();
        self.0
            .send(Line { text, written })
            .await
            .map_err(io::Error::other)?;
        acknowledgement.await.map_err(io::Error::other)?
    }
}

fn write_line(mut stream: impl Write, line: &str) -> io::Result<()> {
    writeln!(stream, "{line}")?;
    stream.flush()
}
