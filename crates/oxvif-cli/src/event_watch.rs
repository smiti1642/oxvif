use std::{collections::BTreeMap, future::Future, io, time::Duration};

use oxvif::events::{
    EventTopicFilter, ManagedEvent, ManagedEventOptions, ManagedEventPolicy, ManagedEventStop,
    ManagedEventSubscription, QualifiedNotification,
};
use serde::Serialize;
use tokio::time::{Instant, MissedTickBehavior, interval_at, sleep, timeout};

use crate::{AppError, Application, ExecutionOptions, ResultMeta, TargetSelector, maintenance};

const MAX_TOPIC_GROUPS: usize = 12;
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(4);
const SUMMARY_INTERVAL: Duration = Duration::from_secs(1);

/// A bounded event observation on one selected camera.
#[derive(Debug, Eq, PartialEq)]
pub struct WatchEventsRequest {
    pub selector: TargetSelector,
    pub duration: Duration,
    pub topic: Option<EventTopicFilter>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EventWatchCounts {
    pub delivered: u64,
    pub gaps: u64,
    pub topics: BTreeMap<String, u64>,
    pub other_topics: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventWatchEnd {
    DurationElapsed,
    Interrupted,
    QueueFull,
    InvalidTiming,
    RequestFailed,
    RecoveryExhausted,
    WorkerStopped,
    OutputTimeout,
    OutputFailed,
    DrainTimeout,
}

impl EventWatchEnd {
    fn normal(self) -> bool {
        matches!(self, Self::DurationElapsed | Self::Interrupted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventWatchGap {
    SubscriptionRecovery,
    QueueFull,
    SubscriptionStopped,
    UndeliveredAtStop,
}

/// Updates are consumed in order. Progress is a bounded presentation snapshot.
pub enum EventWatchUpdate {
    Notification {
        sequence: u64,
        event: Box<QualifiedNotification>,
    },
    Gap {
        after_sequence: u64,
        reason: EventWatchGap,
    },
    Progress(EventWatchCounts),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventWatchSummary {
    pub reason: EventWatchEnd,
    pub complete: bool,
    pub counts: EventWatchCounts,
    pub known_undelivered: u64,
    pub recoveries: u32,
    pub cleanup_failed: bool,
}

pub struct EventWatchResult {
    pub summary: EventWatchSummary,
    pub meta: ResultMeta,
    /// Whether it is safe to attempt another record on this output sink.
    pub output_available: bool,
}

impl EventWatchResult {
    pub fn exit_code(&self) -> u8 {
        if !self.summary.reason.normal() {
            20
        } else if !self.summary.complete || self.summary.cleanup_failed {
            6
        } else {
            0
        }
    }
}

impl Application {
    /// Open, observe and clean up a single managed event subscription.
    /// The observation deadline starts after the device grants the subscription.
    pub async fn watch_events(
        &self,
        request: WatchEventsRequest,
        options: &ExecutionOptions,
        cancel: impl Future<Output = ()>,
        emit: impl AsyncFnMut(&ResultMeta, EventWatchUpdate) -> io::Result<()>,
    ) -> Result<EventWatchResult, AppError> {
        if request.duration.is_zero() || request.duration > Duration::from_secs(3600) {
            return Err(AppError::invalid_argument(
                "Event watch duration must be greater than zero and at most 60 minutes.",
            ));
        }
        let pull_wait = (options.timeout / 2).min(Duration::from_secs(15));
        if pull_wait.is_zero() || options.timeout > Duration::from_secs(86_400) {
            return Err(AppError::invalid_argument(
                "Event watch requires a positive pull wait and a request timeout of at most 24 hours.",
            ));
        }
        let resolved = self.resolve_target(request.selector)?;
        let invocation_start = Instant::now();
        let mut cancel = std::pin::pin!(cancel);
        let session = tokio::select! {
            biased;
            () = &mut cancel => {
                return Ok(interrupted_start(watch_meta(resolved), invocation_start, false));
            }
            session = maintenance::connect_session(&resolved, options) => session?,
        };
        let meta = watch_meta(resolved);
        tokio::select! {
            biased;
            () = &mut cancel => return Ok(interrupted_start(meta, invocation_start, false)),
            () = std::future::ready(()) => {}
        }
        let event_options = ManagedEventOptions {
            pull_wait,
            request_timeout: options.timeout,
            ..ManagedEventOptions::default()
        };
        let mut subscribe = std::pin::pin!(session.managed_events_with_policy(
            event_options,
            ManagedEventPolicy {
                topic: request.topic,
                wait_for_capacity: true,
            },
        ));
        // Once creation has been polled, the camera may already have granted
        // a lease. Keep waiting for its bounded response so we can clean up.
        let (subscription, interrupted) = tokio::select! {
            biased;
            result = &mut subscribe => (result, false),
            () = &mut cancel => (subscribe.await, true),
        };
        let subscription = match subscription {
            Ok(subscription) => subscription,
            Err(_) if interrupted => return Ok(interrupted_start(meta, invocation_start, true)),
            Err(error) => {
                let message = if error.cleanup_error.is_some() {
                    "Could not start event watch. Subscription cleanup could not be confirmed."
                } else {
                    "Could not start event watch. Check the Events service, credentials and topic."
                };
                return Err(AppError::device_operation_failed(message, false));
            }
        };
        Ok(observe(
            subscription,
            request.duration,
            async {
                if !interrupted {
                    cancel.await;
                }
            },
            meta,
            invocation_start,
            emit,
        )
        .await)
    }
}

fn watch_meta(resolved: crate::application::ResolvedTarget) -> ResultMeta {
    ResultMeta {
        command: Some("events.watch".into()),
        device_id: resolved.device_id,
        selected_by: resolved.selected_by,
        target: Some(resolved.target),
        elapsed_ms: 0,
    }
}

fn interrupted_start(
    mut meta: ResultMeta,
    invocation_start: Instant,
    cleanup_failed: bool,
) -> EventWatchResult {
    meta.elapsed_ms = elapsed(invocation_start);
    EventWatchResult {
        summary: EventWatchSummary {
            reason: EventWatchEnd::Interrupted,
            complete: !cleanup_failed,
            counts: EventWatchCounts::default(),
            known_undelivered: 0,
            recoveries: 0,
            cleanup_failed,
        },
        meta,
        output_available: true,
    }
}

async fn observe(
    mut subscription: ManagedEventSubscription,
    duration: Duration,
    cancel: impl Future<Output = ()>,
    mut meta: ResultMeta,
    invocation_start: Instant,
    mut emit: impl AsyncFnMut(&ResultMeta, EventWatchUpdate) -> io::Result<()>,
) -> EventWatchResult {
    let cancel_subscription = subscription.cancellation();
    let mut state = Delivery::default();
    let (requested_end, output_error, drain_elapsed) = {
        let mut consume = std::pin::pin!(consume(
            &mut subscription,
            &mut state,
            &mut meta,
            invocation_start,
            &mut emit,
        ));
        let mut cancel = std::pin::pin!(cancel);
        let exit = tokio::select! {
            biased;
            () = &mut cancel => Ok(EventWatchEnd::Interrupted),
            () = sleep(duration) => Ok(EventWatchEnd::DurationElapsed),
            result = &mut consume => Err(result.err()),
        };
        match exit {
            Ok(reason) => {
                cancel_subscription.cancel();
                match timeout(DRAIN_TIMEOUT, consume).await {
                    Ok(result) => (Some(reason), result.err(), false),
                    Err(_) => (Some(reason), None, true),
                }
            }
            Err(output_error) => (None, output_error, false),
        }
    };
    // Waiting for the worker to resolve Create and clean up a replacement
    // lease can outlast draining the output. An idle reader leaves stdout usable.
    let output_error = if drain_elapsed && state.write_pending {
        Some(EventWatchEnd::DrainTimeout)
    } else {
        output_error
    };
    finish_observation(
        subscription,
        state,
        meta,
        invocation_start,
        requested_end,
        output_error,
        &mut emit,
    )
    .await
}

#[derive(Default)]
struct Delivery {
    received: u64,
    counts: EventWatchCounts,
    write_pending: bool,
}

impl Delivery {
    async fn write(
        &mut self,
        operation: impl Future<Output = io::Result<()>>,
    ) -> Result<(), EventWatchEnd> {
        self.write_pending = true;
        let result = write_update(operation).await;
        self.write_pending = false;
        result
    }
}

async fn consume(
    subscription: &mut ManagedEventSubscription,
    state: &mut Delivery,
    meta: &mut ResultMeta,
    invocation_start: Instant,
    emit: &mut impl AsyncFnMut(&ResultMeta, EventWatchUpdate) -> io::Result<()>,
) -> Result<(), EventWatchEnd> {
    let mut tick = interval_at(Instant::now() + SUMMARY_INTERVAL, SUMMARY_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        let item = tokio::select! {
            biased;
            _ = tick.tick() => {
                meta.elapsed_ms = elapsed(invocation_start);
                state.write(emit(meta, EventWatchUpdate::Progress(state.counts.clone()))).await?;
                continue;
            }
            item = subscription.next() => item,
        };
        meta.elapsed_ms = elapsed(invocation_start);
        match item {
            Some(Ok(ManagedEvent::Notification(event))) => {
                state.received += 1;
                let topic = event
                    .topic_expression()
                    .unwrap_or_else(|| "(unknown)".into());
                let topic: String = topic
                    .chars()
                    .take(120)
                    .map(|c| if c.is_control() { '�' } else { c })
                    .collect();
                let sequence = state.counts.delivered + 1;
                state
                    .write(emit(
                        meta,
                        EventWatchUpdate::Notification { sequence, event },
                    ))
                    .await?;
                state.counts.delivered = sequence;
                if let Some(count) = state.counts.topics.get_mut(&topic) {
                    *count += 1;
                } else if state.counts.topics.len() < MAX_TOPIC_GROUPS {
                    state.counts.topics.insert(topic, 1);
                } else {
                    state.counts.other_topics += 1;
                }
            }
            Some(Ok(ManagedEvent::Gap)) => {
                state.counts.gaps += 1;
                state
                    .write(emit(
                        meta,
                        EventWatchUpdate::Gap {
                            after_sequence: state.counts.delivered,
                            reason: EventWatchGap::SubscriptionRecovery,
                        },
                    ))
                    .await?;
            }
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return Ok(()),
        }
    }
}

async fn finish_observation(
    mut subscription: ManagedEventSubscription,
    mut state: Delivery,
    mut meta: ResultMeta,
    invocation_start: Instant,
    requested_end: Option<EventWatchEnd>,
    mut output_error: Option<EventWatchEnd>,
    emit: &mut impl AsyncFnMut(&ResultMeta, EventWatchUpdate) -> io::Result<()>,
) -> EventWatchResult {
    let report = subscription.close().await;
    let stop = report.stop;
    let cleanup_failed = report.cleanup_error.is_some() || report.recovery_cleanup_failures > 0;
    let recoveries = report.recoveries;
    let mut known_undelivered =
        state.received - state.counts.delivered + subscription.pending_notifications() as u64;
    while let Some(item) = subscription.next().await {
        match item {
            Ok(ManagedEvent::Notification(_)) => known_undelivered += 1,
            Ok(ManagedEvent::Gap) => state.counts.gaps += 1,
            _ => {}
        }
    }
    let subscription_end = match stop {
        ManagedEventStop::Closed => requested_end.unwrap_or(EventWatchEnd::WorkerStopped),
        ManagedEventStop::QueueFull => EventWatchEnd::QueueFull,
        ManagedEventStop::InvalidTiming => EventWatchEnd::InvalidTiming,
        ManagedEventStop::RequestFailed => EventWatchEnd::RequestFailed,
        ManagedEventStop::RecoveryExhausted => EventWatchEnd::RecoveryExhausted,
        _ => EventWatchEnd::WorkerStopped,
    };
    let gap = match subscription_end {
        EventWatchEnd::QueueFull => Some(EventWatchGap::QueueFull),
        reason if !reason.normal() && output_error.is_none() => {
            Some(EventWatchGap::SubscriptionStopped)
        }
        _ if known_undelivered > 0 => Some(EventWatchGap::UndeliveredAtStop),
        _ => None,
    };
    meta.elapsed_ms = elapsed(invocation_start);
    if let Some(reason) = gap {
        state.counts.gaps += 1;
        if output_error.is_none() {
            output_error = write_update(emit(
                &meta,
                EventWatchUpdate::Gap {
                    after_sequence: state.counts.delivered,
                    reason,
                },
            ))
            .await
            .err();
        }
    }
    EventWatchResult {
        summary: EventWatchSummary {
            reason: output_error.unwrap_or(subscription_end),
            complete: output_error.is_none() && subscription_end.normal() && state.counts.gaps == 0,
            counts: state.counts,
            known_undelivered,
            recoveries,
            cleanup_failed,
        },
        meta,
        output_available: output_error.is_none(),
    }
}

async fn write_update(
    operation: impl Future<Output = io::Result<()>>,
) -> Result<(), EventWatchEnd> {
    match timeout(WRITE_TIMEOUT, operation).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(EventWatchEnd::OutputFailed),
        Err(_) => Err(EventWatchEnd::OutputTimeout),
    }
}

fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvif::{
        OnvifClient,
        transport::{Transport, TransportError},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::sync::Notify;

    #[derive(Default)]
    struct ReplacementTransport {
        creates: AtomicUsize,
        unsubscribes: AtomicUsize,
        replacing: Notify,
        reply: Notify,
    }

    #[async_trait::async_trait]
    impl Transport for ReplacementTransport {
        async fn soap_post(
            &self,
            _url: &str,
            action: &str,
            _body: String,
        ) -> Result<String, TransportError> {
            let response = if action.ends_with("/CreatePullPointSubscriptionRequest") {
                if self.creates.fetch_add(1, Ordering::Relaxed) != 0 {
                    self.replacing.notify_one();
                    self.reply.notified().await;
                }
                "<e:CreatePullPointSubscriptionResponse><e:SubscriptionReference><a:Address>http://camera.test/subscription</a:Address></e:SubscriptionReference><n:CurrentTime>2050-01-01T00:00:00Z</n:CurrentTime><n:TerminationTime>2050-01-01T00:01:00Z</n:TerminationTime></e:CreatePullPointSubscriptionResponse>"
            } else if action.ends_with("/PullMessagesRequest") {
                return Err(TransportError::HttpStatus {
                    status: 503,
                    body: String::new(),
                });
            } else {
                assert!(action.ends_with("/UnsubscribeRequest"), "{action}");
                self.unsubscribes.fetch_add(1, Ordering::Relaxed);
                "<n:UnsubscribeResponse/>"
            };
            Ok(format!(
                r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://www.w3.org/2005/08/addressing" xmlns:n="http://docs.oasis-open.org/wsn/b-2" xmlns:e="http://www.onvif.org/ver10/events/wsdl"><s:Body>{response}</s:Body></s:Envelope>"#
            ))
        }
    }

    #[tokio::test]
    async fn delayed_replacement_cleanup_still_allows_the_final_summary() {
        let transport = Arc::new(ReplacementTransport::default());
        let subscription = OnvifClient::new("http://camera.test/device")
            .with_transport(transport.clone())
            .managed_events("http://camera.test/events", ManagedEventOptions::default())
            .await
            .unwrap();
        let stopping = Notify::new();
        let (result, ()) = tokio::join!(
            observe(
                subscription,
                Duration::from_secs(30),
                async {
                    transport.replacing.notified().await;
                    stopping.notify_one();
                },
                ResultMeta::default(),
                Instant::now(),
                async |_, _| Ok(()),
            ),
            async {
                stopping.notified().await;
                sleep(DRAIN_TIMEOUT + Duration::from_millis(100)).await;
                transport.reply.notify_one();
            },
        );
        assert!(result.output_available);
        assert_eq!(result.summary.reason, EventWatchEnd::Interrupted);
        assert_eq!(result.summary.counts.gaps, 1);
        assert!(!result.summary.cleanup_failed);
        assert_eq!(transport.unsubscribes.load(Ordering::Relaxed), 2);
    }
}
