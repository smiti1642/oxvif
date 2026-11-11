//! Invocation-local metadata for stderr adapters. Never retains wire data.
use oxvif::SessionBuildReport;
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};

const MAX_DEVICES: usize = 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Default)]
struct Buffer {
    entries: Vec<String>,
    omitted: usize,
}
#[derive(Default)]
struct Log {
    devices: BTreeMap<String, Buffer>,
    omitted_devices: usize,
}
#[derive(Clone)]
pub(crate) struct Context {
    log: Arc<Mutex<Log>>,
    key: String,
}
tokio::task_local! { static CURRENT: Context; }

/// Run an invocation with bounded metadata capture. Returned lines contain only
/// controlled labels/numbers, sorted by device identity with pseudonymous indices.
/// This does not alter command results or the v3 stdout envelope. Fleet workers
/// explicitly inherit this scope; unrelated concurrent invocations stay separate.
pub fn capture_diagnostics<T>(
    enabled: bool,
    future: impl Future<Output = T>,
) -> impl Future<Output = (T, Vec<String>)> {
    // TaskLocalFuture otherwise retains the full command future in multiple
    // branches, which can exceed the Windows main-thread stack in debug builds.
    capture_boxed(enabled, Box::pin(future))
}

async fn capture_boxed<T>(enabled: bool, future: impl Future<Output = T>) -> (T, Vec<String>) {
    if !enabled {
        return (future.await, Vec::new());
    }
    let context = Context {
        log: Arc::new(Mutex::new(Log::default())),
        key: String::new(),
    };
    let result = CURRENT.scope(context.clone(), future).await;
    let Ok(log) = context.log.lock() else {
        return (result, vec!["debug: diagnostics=unavailable".into()]);
    };
    let mut lines = Vec::new();
    for (index, buffer) in log.devices.values().enumerate() {
        for entry in &buffer.entries {
            lines.push(format!("debug: device_index={index} {entry}"));
        }
        if buffer.omitted > 0 {
            lines.push(format!(
                "debug: device_index={index} omitted_entries={}",
                buffer.omitted
            ));
        }
    }
    if log.omitted_devices > 0 {
        lines.push(format!(
            "debug: omitted_device_events={}",
            log.omitted_devices
        ));
    }
    (result, lines)
}

pub(crate) fn context() -> Option<Context> {
    CURRENT.try_with(Clone::clone).ok()
}

#[cfg(test)]
pub(crate) fn contains_entry(entry: &str) -> bool {
    let context = context().expect("diagnostics capture must be enabled");
    let log = context.log.lock().expect("diagnostics lock");
    log.devices
        .get(&context.key)
        .is_some_and(|buffer| buffer.entries.iter().any(|item| item == entry))
}

pub(crate) async fn device_scope<T>(
    context: Option<Context>,
    key: String,
    future: impl Future<Output = T>,
) -> T {
    match context {
        Some(mut context) => {
            context.key = key.chars().take(128).collect();
            CURRENT.scope(context, future).await
        }
        None => future.await,
    }
}

fn record(entry: String) {
    let _ = CURRENT.try_with(|context| {
        if let Ok(mut log) = context.log.lock() {
            if !log.devices.contains_key(&context.key) && log.devices.len() == MAX_DEVICES {
                // Retain the lexicographically first keys regardless of worker
                // completion order. Evicted entries contribute to the count.
                let largest = log.devices.keys().next_back().cloned();
                if let Some(largest) = largest.filter(|key| context.key < *key) {
                    if let Some(buffer) = log.devices.remove(&largest) {
                        log.omitted_devices = log
                            .omitted_devices
                            .saturating_add(buffer.entries.len())
                            .saturating_add(buffer.omitted);
                    }
                } else {
                    log.omitted_devices = log.omitted_devices.saturating_add(1);
                    return;
                }
            }
            let buffer = log.devices.entry(context.key.clone()).or_default();
            if buffer.entries.len() < MAX_ENTRIES {
                buffer.entries.push(entry);
            } else {
                buffer.omitted = buffer.omitted.saturating_add(1);
            }
        }
    });
}

pub(crate) fn session(report: &SessionBuildReport) {
    if context().is_none() {
        return;
    }
    for stage in &report.stages {
        record(format!(
            "session_stage={:?} outcome={:?} elapsed_ms={} error_class={:?} http_status={:?}",
            stage.stage, stage.outcome, stage.elapsed_ms, stage.error_class, stage.http_status
        ));
    }
    record(format!("clock_offset_secs={:?}", report.clock_offset_secs));
    for service in &report.services {
        record(format!(
            "service={:?} url_source={:?}",
            service.service, service.source
        ));
    }
}

pub(crate) fn attempt(
    unit: &'static str,
    attempt: u32,
    class: &'static str,
    delay: Option<std::time::Duration>,
) {
    if context().is_none() {
        return;
    }
    record(format!(
        "operation_unit={unit} attempt={} retry_class={class} retry_delay_ms={}",
        u64::from(attempt) + 1,
        delay.map_or(0, |delay| delay.as_millis())
    ));
}

pub(crate) async fn initialize(
    builder: oxvif::session::OnvifSessionBuilder,
) -> Result<oxvif::OnvifSession, oxvif::OnvifError> {
    match builder.build_with_report().await {
        Ok((value, report)) => {
            session(&report);
            Ok(value)
        }
        Err(failure) => {
            session(&failure.report);
            Err(failure.error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invocation_buffers_sort_workers_bound_output_and_isolate_private_keys() {
        let (value, lines) = capture_diagnostics(true, async {
            let mut workers = tokio::task::JoinSet::new();
            for (key, unit, count) in [("private-b", "second", 1), ("private-a", "first", 140)] {
                let context = context();
                workers.spawn(device_scope(context, key.into(), async move {
                    for attempt_index in 0..count {
                        attempt(unit, attempt_index, "succeeded", None);
                    }
                }));
            }
            while let Some(result) = workers.join_next().await {
                result.unwrap();
            }
            123
        })
        .await;
        assert_eq!(value, 123);
        assert_eq!(lines.len(), 130);
        assert!(lines[0].contains("device_index=0 operation_unit=first attempt=1"));
        assert!(lines[127].contains("attempt=128"));
        assert_eq!(lines[128], "debug: device_index=0 omitted_entries=12");
        assert!(lines[129].contains("device_index=1 operation_unit=second"));
        assert!(!lines.join("\n").contains("private-"));
        let (_, empty) = capture_diagnostics(true, async {}).await;
        assert!(empty.is_empty());
        let (_, disabled) = capture_diagnostics(false, async {
            attempt("disabled", 0, "succeeded", None);
        })
        .await;
        assert!(disabled.is_empty());
    }

    #[tokio::test]
    async fn over_capacity_devices_keep_the_same_sorted_projection_in_either_order() {
        let mut results = Vec::new();
        for reverse in [false, true] {
            let (_, lines) = capture_diagnostics(true, async {
                let mut keys = (0..1026).collect::<Vec<_>>();
                if reverse {
                    keys.reverse();
                }
                for key in keys {
                    device_scope(context(), format!("{key:04}"), async {
                        attempt("read", key, "succeeded", None);
                    })
                    .await;
                }
            })
            .await;
            assert_eq!(lines.len(), 1025);
            assert!(lines[1023].contains("device_index=1023 operation_unit=read attempt=1024"));
            assert_eq!(lines[1024], "debug: omitted_device_events=2");
            results.push(lines);
        }
        assert_eq!(results[0], results[1]);
    }
}
