use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    task::JoinHandle,
    time::{Instant, sleep_until},
};

use super::{
    EndpointPolicy, EndpointReference, EventLeaseTimes, EventTopicFilter, QualifiedNotification,
    snapshot::SnapshotCache,
};
use crate::{OnvifClient, OnvifError, OnvifSession, soap::SoapError, transport::TransportError};

/// Budgets for a managed pull point. All durations must be positive.
#[derive(Clone)]
pub struct ManagedEventOptions {
    /// Additional explicitly trusted service origins.
    pub endpoints: EndpointPolicy,
    /// Requested initial and renewed lease length.
    pub lease: Duration,
    /// Timeout requested from the device for each pull.
    pub pull_wait: Duration,
    /// Local exchange deadline, including a long poll and authentication.
    pub request_timeout: Duration,
    /// Separate deadline for one unsubscribe attempt.
    pub cleanup_timeout: Duration,
    /// Maximum renewal lead time, also bounded by one third of granted lease.
    pub renew_margin: Duration,
    /// Delay between successful pulls, including immediate empty responses.
    pub poll_interval: Duration,
    /// Maximum notifications requested in a page.
    pub page_messages: u32,
    /// Maximum queued event and gap entries.
    pub queue_messages: usize,
    /// Maximum accounted serialized notification bytes in the queue.
    pub queue_bytes: u32,
    /// Maximum replacement subscriptions over this owner's lifetime.
    pub max_recoveries: u32,
    /// Maximum time for each recovery, including cleanup and creation.
    pub recovery_timeout: Duration,
}

impl Default for ManagedEventOptions {
    fn default() -> Self {
        Self {
            endpoints: EndpointPolicy::default(),
            lease: Duration::from_secs(60),
            pull_wait: Duration::from_secs(15),
            request_timeout: Duration::from_secs(20),
            cleanup_timeout: Duration::from_secs(2),
            renew_margin: Duration::from_secs(10),
            poll_interval: Duration::from_millis(100),
            page_messages: 32,
            queue_messages: 128,
            queue_bytes: 1024 * 1024,
            max_recoveries: 2,
            recovery_timeout: Duration::from_secs(12),
        }
    }
}

impl ManagedEventOptions {
    fn validate(&self) -> Result<(), OnvifError> {
        let durations = [
            self.lease,
            self.pull_wait,
            self.request_timeout,
            self.cleanup_timeout,
            self.renew_margin,
            self.poll_interval,
            self.recovery_timeout,
        ];
        if durations
            .iter()
            .any(|d| d.is_zero() || *d > Duration::from_secs(86_400))
            || self.request_timeout <= self.pull_wait
            || self.page_messages == 0
            || self.page_messages > 4096
            || self.queue_messages == 0
            || self.queue_messages > 65_536
            || self.queue_bytes == 0
            || self.queue_bytes > 64 * 1024 * 1024
        {
            return Err(OnvifError::InvalidArgument(
                "invalid managed event budgets".into(),
            ));
        }
        Ok(())
    }
}

/// Optional topic and delivery behavior for a managed pull point.
#[derive(Clone, Debug, Default)]
pub struct ManagedEventPolicy {
    /// Request one exact ONVIF topic and check it on received notifications.
    pub topic: Option<EventTopicFilter>,
    /// Wait for queue capacity before pulling again, while continuing renewal.
    /// The default stops with `QueueFull` when a queue limit is reached.
    pub wait_for_capacity: bool,
}

/// Terminal outcome of a managed event subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManagedEventStop {
    /// The owner closed the subscription.
    Closed,
    /// The consumer fell behind its configured limits.
    QueueFull,
    /// The device did not supply a usable lease interval.
    InvalidTiming,
    /// An exchange failed permanently.
    RequestFailed,
    /// The recovery count or deadline was exhausted.
    RecoveryExhausted,
    /// The worker exited before supplying a report.
    WorkerStopped,
}

/// Observable lifecycle of a managed subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManagedEventState {
    /// Receiving notifications.
    Active,
    /// Renewing the current lease.
    Renewing,
    /// Replacing the current subscription.
    Recovering,
    /// Unsubscribing and releasing the worker.
    Closing,
    /// Terminal outcome.
    Finished(ManagedEventStop),
}

/// Event delivery or a discontinuity before a replacement subscription.
#[derive(Debug)]
#[non_exhaustive]
pub enum ManagedEvent {
    /// Notification in device order.
    Notification(Box<QualifiedNotification>),
    /// Notifications may have been lost or duplicated across recovery.
    Gap,
    /// Possible loss or duplication, starting at this device UTC checkpoint.
    RecoveryGap {
        /// Device time associated with the previous completed pull or initial lease.
        since: String,
    },
    /// A replacement pull point was established. The gap may still be incomplete.
    RecoveryResumed {
        /// Replacement grant time when it is consistent with the previous clock.
        through: Option<String>,
    },
    /// A retained historical notification received after repositioning the pull point.
    /// This provides no guarantee about the completeness of the recovered interval.
    Recovered(Box<QualifiedNotification>),
    /// A current property state received after requesting synchronization.
    /// Repeated snapshots of the same property value within a synchronization
    /// burst may be coalesced. Changes and later snapshots remain observable.
    Snapshot(Box<QualifiedNotification>),
}

/// Final worker report, including independent remote cleanup evidence.
#[derive(Debug)]
pub struct ManagedEventReport {
    /// Why the worker stopped.
    pub stop: ManagedEventStop,
    /// Typed error from the operation that stopped the worker, if available.
    pub cause: Option<OnvifError>,
    /// Failure from unsubscribing the final known lease, if any.
    pub cleanup_error: Option<OnvifError>,
    /// Number of replacement subscriptions attempted.
    pub recoveries: u32,
    /// Leases whose cleanup could not be confirmed during recovery.
    pub recovery_cleanup_failures: u32,
}

/// Failure to acquire a managed lease, including cleanup of a known reference.
#[derive(Debug, thiserror::Error)]
#[error("managed event subscription could not start")]
pub struct ManagedEventStartError {
    /// Primary request or timing error.
    #[source]
    pub cause: OnvifError,
    /// Cleanup error after an invalid grant, if a reference was acquired.
    pub cleanup_error: Option<OnvifError>,
}

struct QueuedEvent {
    event: ManagedEvent,
    _bytes: Option<OwnedSemaphorePermit>,
}

/// Owns one client pull point and its worker. Use async close for remote cleanup.
pub struct ManagedEventSubscription {
    queue: mpsc::Receiver<QueuedEvent>,
    state: watch::Receiver<ManagedEventState>,
    cancel: watch::Sender<bool>,
    paused: watch::Sender<bool>,
    task: Option<JoinHandle<ManagedEventReport>>,
    report: Option<ManagedEventReport>,
    terminal_delivered: bool,
    pending: Arc<AtomicUsize>,
}

/// A cooperative stop handle that can be used while polling a subscription.
#[derive(Clone)]
pub struct ManagedEventCancellation(watch::Sender<bool>);

impl ManagedEventCancellation {
    /// Stop pulling and attempt to unsubscribe the current lease.
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
}

impl ManagedEventSubscription {
    /// Get a handle to stop the worker without borrowing the consumer.
    pub fn cancellation(&self) -> ManagedEventCancellation {
        ManagedEventCancellation(self.cancel.clone())
    }

    /// Notifications from the last received page that never entered the queue.
    /// Read after close. Loss on the device or across recovery may be unknown.
    pub fn pending_notifications(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }

    /// Current lifecycle state.
    pub fn state(&self) -> ManagedEventState {
        *self.state.borrow()
    }

    /// Pause new pulls while keeping the lease renewed. An existing pull may finish.
    pub fn set_paused(&self, paused: bool) {
        self.paused.send_if_modified(|current| {
            let changed = *current != paused;
            *current = paused;
            changed
        });
    }

    /// Await the next notification, gap or terminal failure. Buffered events
    /// precede the terminal failure, which is delivered once.
    pub async fn next(&mut self) -> Option<Result<ManagedEvent, ManagedEventStop>> {
        if let Some(queued) = self.queue.recv().await {
            return Some(Ok(queued.event));
        }
        if self.terminal_delivered {
            return None;
        }
        self.terminal_delivered = true;
        match self.state() {
            ManagedEventState::Finished(ManagedEventStop::Closed) => None,
            ManagedEventState::Finished(stop) => Some(Err(stop)),
            _ => Some(Err(ManagedEventStop::WorkerStopped)),
        }
    }

    /// Signal cancellation, await unsubscribe and join the worker.
    /// Repeated calls return the same report. An interrupted wait can be resumed.
    pub async fn close(&mut self) -> &ManagedEventReport {
        self.cancel.send_replace(true);
        if let Some(task) = &mut self.task {
            let report = task.await.unwrap_or(ManagedEventReport {
                stop: ManagedEventStop::WorkerStopped,
                cause: None,
                cleanup_error: None,
                recoveries: 0,
                recovery_cleanup_failures: 0,
            });
            self.task = None;
            self.report = Some(report);
        }
        self.report.as_ref().expect("joined worker report")
    }
}

impl Drop for ManagedEventSubscription {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl OnvifClient {
    /// Acquire a pull point and start bounded polling and renewal.
    /// Dropping the returned owner aborts local work. Use close for unsubscribe.
    pub async fn managed_events(
        &self,
        events_url: &str,
        options: ManagedEventOptions,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        self.managed_events_with_policy(events_url, options, ManagedEventPolicy::default())
            .await
    }

    /// Acquire a managed pull point with explicit topic and delivery behavior.
    pub async fn managed_events_with_policy(
        &self,
        events_url: &str,
        options: ManagedEventOptions,
        policy: ManagedEventPolicy,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        let expression = policy
            .topic
            .as_ref()
            .map(|topic| format!("tns1:{}", topic.path()));
        self.start_managed_events(events_url, options, policy, expression, false)
            .await
    }

    /// Observe a pull point and attempt history or state recovery on replacement.
    ///
    /// `filter` is an optional ONVIF ConcreteSet expression, such as
    /// `tns1:VideoSource/MotionAlarm|tns1:Device/Trigger/DigitalInput`.
    /// History is limited to the notifications retained by the device. A recovery
    /// always emits a gap, even when some historical events can be retrieved.
    pub async fn managed_events_recovering(
        &self,
        events_url: &str,
        options: ManagedEventOptions,
        filter: Option<&str>,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        let policy = ManagedEventPolicy {
            wait_for_capacity: true,
            ..ManagedEventPolicy::default()
        };
        self.start_managed_events(events_url, options, policy, filter.map(str::to_owned), true)
            .await
    }

    async fn start_managed_events(
        &self,
        events_url: &str,
        options: ManagedEventOptions,
        policy: ManagedEventPolicy,
        expression: Option<String>,
        recover_history: bool,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        let wrap = |cause| {
            Box::new(ManagedEventStartError {
                cause,
                cleanup_error: None,
            })
        };
        options.validate().map_err(wrap)?;
        let started = Instant::now();
        let point = self
            .create_referenced_pull_point_filtered(
                events_url,
                &crate::types::duration_xml(options.lease),
                expression.as_deref(),
                &options.endpoints,
                options.request_timeout,
            )
            .await
            .map_err(wrap)?;
        let Some(lease) = Lease::from_times(&point.times, started, None) else {
            let cleanup_error = self
                .unsubscribe_referenced_pull_point(
                    &point.reference,
                    &options.endpoints,
                    options.cleanup_timeout,
                )
                .await
                .err();
            return Err(Box::new(ManagedEventStartError {
                cause: invalid_timing(),
                cleanup_error,
            }));
        };
        let (send, queue) = mpsc::channel(options.queue_messages);
        let (cancel, cancelled) = watch::channel(false);
        let (paused, pause) = watch::channel(false);
        let (status, state) = watch::channel(ManagedEventState::Active);
        let pending = Arc::new(AtomicUsize::new(0));
        let worker = Worker {
            client: self.clone(),
            url: events_url.to_owned(),
            options,
            policy,
            expression,
            recover_history,
            pending: pending.clone(),
            send,
            status,
            cancelled,
            pause,
        };
        let cursor = point.times.current_time.unwrap_or_default();
        let task = tokio::spawn(worker.run(point.reference, lease, cursor));
        Ok(ManagedEventSubscription {
            queue,
            state,
            cancel,
            paused,
            task: Some(task),
            report: None,
            terminal_delivered: false,
            pending,
        })
    }
}

impl OnvifSession {
    /// Start a managed event subscription using the resolved Events URL.
    pub async fn managed_events(
        &self,
        options: ManagedEventOptions,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        self.managed_events_with_policy(options, ManagedEventPolicy::default())
            .await
    }

    /// Start a managed subscription with explicit topic and delivery behavior.
    pub async fn managed_events_with_policy(
        &self,
        options: ManagedEventOptions,
        policy: ManagedEventPolicy,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        self.client()
            .managed_events_with_policy(self.managed_events_url()?, options, policy)
            .await
    }

    /// Start history-aware monitoring using the resolved Events service.
    pub async fn managed_events_recovering(
        &self,
        options: ManagedEventOptions,
        filter: Option<&str>,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        self.client()
            .managed_events_recovering(self.managed_events_url()?, options, filter)
            .await
    }

    fn managed_events_url(&self) -> Result<&str, Box<ManagedEventStartError>> {
        self.capabilities().events.url.as_deref().ok_or_else(|| {
            Box::new(ManagedEventStartError {
                cause: SoapError::missing("Events service URL").into(),
                cleanup_error: None,
            })
        })
    }
}

#[derive(Clone, Copy)]
struct Lease {
    observed: Instant,
    sampled: Instant,
    device_time: i128,
    device_expiry: i128,
    expires: Instant,
}

impl Lease {
    fn from_times(
        times: &EventLeaseTimes,
        started: Instant,
        previous: Option<Self>,
    ) -> Option<Self> {
        let (anchor, device_time) = match &times.current_time {
            Some(current) => (started, timestamp(current)?),
            None => {
                let previous = previous?;
                (previous.observed, previous.device_time)
            }
        };
        let device_expiry = timestamp(times.termination_time.as_deref()?)?;
        let remaining = device_expiry.checked_sub(device_time)?;
        let nanos = u64::try_from(remaining).ok()?;
        let expires = anchor.checked_add(Duration::from_nanos(nanos))?;
        (expires > Instant::now()).then_some(Self {
            observed: anchor,
            sampled: started,
            device_time,
            device_expiry,
            expires,
        })
    }
}

pub(super) fn timestamp(value: &str) -> Option<i128> {
    let value = value.trim().strip_suffix('Z')?;
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let (date, clock) = whole.split_once('T')?;
    if date.len() != 10
        || clock.len() != 8
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return None;
    }
    let (year, rest) = date.split_once('-')?;
    let (month, day) = rest.split_once('-')?;
    let (hour, rest) = clock.split_once(':')?;
    let (minute, second) = rest.split_once(':')?;
    if [year, month, day, hour, minute, second]
        .into_iter()
        .any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let (year, month, day, hour, minute, second) = (
        year.parse().ok()?,
        month.parse().ok()?,
        day.parse().ok()?,
        hour.parse().ok()?,
        minute.parse().ok()?,
        second.parse().ok()?,
    );
    let secs = crate::types::civil_to_unix(year, month, day, hour, minute, second);
    if crate::soap::security::unix_secs_to_iso8601(secs).strip_suffix('Z')? != whole {
        return None;
    }
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().ok()? * 10_i128.pow((9 - fraction.len()) as u32)
    };
    Some(i128::from(secs) * 1_000_000_000 + fraction)
}

fn invalid_timing() -> OnvifError {
    SoapError::InvalidValue {
        field: "event lease timing",
        value: "invalid grant".into(),
    }
    .into()
}

async fn cancellation(mut cancel: watch::Receiver<bool>) {
    let _ = cancel.wait_for(|flag| *flag).await;
}

struct Worker {
    client: OnvifClient,
    url: String,
    options: ManagedEventOptions,
    policy: ManagedEventPolicy,
    expression: Option<String>,
    recover_history: bool,
    pending: Arc<AtomicUsize>,
    send: mpsc::Sender<QueuedEvent>,
    status: watch::Sender<ManagedEventState>,
    cancelled: watch::Receiver<bool>,
    pause: watch::Receiver<bool>,
}

#[derive(Clone, Copy)]
enum Delivery {
    Live,
    History { until: i128 },
    Synchronized,
}

enum Exit {
    Closed,
    Full,
    Timing,
    Request(OnvifError),
}

fn recoverable(error: &OnvifError) -> bool {
    match error {
        OnvifError::Transport(TransportError::Deadline) => true,
        OnvifError::Transport(TransportError::Http(error)) => error.is_timeout(),
        OnvifError::Transport(TransportError::HttpStatus { status, .. }) => {
            matches!(status, 408 | 429 | 502 | 503 | 504)
        }
        OnvifError::Soap(SoapError::Fault {
            subcode: Some(code),
            ..
        }) => code == crate::soap::qualified::RESOURCE_UNKNOWN,
        _ => false,
    }
}

impl Worker {
    fn can_recover(&self, error: &OnvifError) -> bool {
        recoverable(error)
            || (self.recover_history
                && matches!(error,
                    OnvifError::Transport(TransportError::Http(error))
                    if error.is_connect() || error.is_request() || error.is_body()
                ))
    }

    async fn run(
        self,
        mut reference: EndpointReference,
        mut lease: Lease,
        mut cursor: String,
    ) -> ManagedEventReport {
        let budget = Arc::new(Semaphore::new(self.options.queue_bytes as usize));
        let mut recoveries = 0;
        let mut cleanup_failures = 0;
        let mut known_reference = true;
        let mut delivery = Delivery::Live;
        let mut resumed = None;
        let (stop, cause) = 'subscription: loop {
            let (clock, changes) = watch::channel(lease);
            self.status.send_replace(ManagedEventState::Active);
            let exit = tokio::select! {
                biased;
                () = cancellation(self.cancelled.clone()) => Exit::Closed,
                exit = self.renew(&reference, &clock, changes) => exit,
                exit = self.pull(&reference, &clock, &budget, &mut cursor, delivery, resumed.take()) => exit,
            };
            let error = match exit {
                Exit::Closed => break (ManagedEventStop::Closed, None),
                Exit::Full => break (ManagedEventStop::QueueFull, None),
                Exit::Timing => break (ManagedEventStop::InvalidTiming, Some(invalid_timing())),
                Exit::Request(error) if self.can_recover(&error) => error,
                Exit::Request(error) => break (ManagedEventStop::RequestFailed, Some(error)),
            };
            if !self.recover_history && recoveries == self.options.max_recoveries {
                break (ManagedEventStop::RecoveryExhausted, Some(error));
            }
            let gap = if self.recover_history {
                ManagedEvent::RecoveryGap {
                    since: cursor.clone(),
                }
            } else {
                ManagedEvent::Gap
            };
            let gap = QueuedEvent {
                event: gap,
                _bytes: None,
            };
            let queued = if self.recover_history {
                tokio::select! {
                    biased;
                    () = cancellation(self.cancelled.clone()) => break (ManagedEventStop::Closed, None),
                    queued = self.enqueue(gap) => queued,
                }
            } else {
                self.send.try_send(gap).is_ok()
            };
            if !queued {
                break (ManagedEventStop::QueueFull, Some(error));
            }
            if recoveries == self.options.max_recoveries {
                break (ManagedEventStop::RecoveryExhausted, Some(error));
            }
            self.status.send_replace(ManagedEventState::Recovering);
            let deadline = Instant::now() + self.options.recovery_timeout;
            let cleanup = self
                .cleanup(
                    &reference,
                    self.options
                        .cleanup_timeout
                        .min(self.options.recovery_timeout),
                )
                .await;
            cleanup_failures += u32::from(cleanup.is_some());
            known_reference = self.recover_history && cleanup.is_some();
            let mut last_error = error;
            let (grant, started) = loop {
                let wait = if self.recover_history && recoveries > 0 {
                    self.options.poll_interval.max(Duration::from_secs(1))
                } else {
                    self.options.poll_interval
                };
                let retry_at = Instant::now() + wait;
                if retry_at >= deadline || recoveries == self.options.max_recoveries {
                    break 'subscription (ManagedEventStop::RecoveryExhausted, Some(last_error));
                }
                tokio::select! {
                    biased;
                    () = cancellation(self.cancelled.clone()) => break 'subscription (ManagedEventStop::Closed, None),
                    () = sleep_until(retry_at) => {}
                }
                if known_reference && Instant::now() < lease.expires {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    known_reference = self
                        .cleanup(&reference, self.options.cleanup_timeout.min(remaining))
                        .await
                        .is_some();
                } else {
                    known_reference = false;
                }
                let started = Instant::now();
                if started >= deadline {
                    break 'subscription (ManagedEventStop::RecoveryExhausted, Some(last_error));
                }
                if *self.cancelled.borrow() {
                    break 'subscription (ManagedEventStop::Closed, None);
                }
                recoveries += 1;
                // A dispatched Create must finish within its budget so cancellation
                // can unsubscribe any reference that the camera returns.
                match self
                    .client
                    .create_referenced_pull_point_filtered(
                        &self.url,
                        &crate::types::duration_xml(self.options.lease),
                        self.expression.as_deref(),
                        &self.options.endpoints,
                        self.options.request_timeout.min(deadline - started),
                    )
                    .await
                {
                    Ok(grant) => break (grant, started),
                    Err(error) => {
                        if !matches!(error, OnvifError::Soap(SoapError::Fault { .. })) {
                            cleanup_failures += 1;
                        }
                        if *self.cancelled.borrow() {
                            break 'subscription (ManagedEventStop::Closed, Some(error));
                        }
                        if !self.recover_history || !self.can_recover(&error) {
                            break 'subscription (ManagedEventStop::RequestFailed, Some(error));
                        }
                        last_error = error;
                    }
                }
            };
            reference = grant.reference;
            known_reference = true;
            let Some(new_lease) = Lease::from_times(&grant.times, started, None) else {
                break (ManagedEventStop::InvalidTiming, Some(invalid_timing()));
            };
            lease = new_lease;
            if self.recover_history {
                let through = grant
                    .times
                    .current_time
                    .clone()
                    .filter(|end| timestamp(end) >= timestamp(&cursor));
                resumed = Some(ManagedEvent::RecoveryResumed { through });
                delivery = tokio::select! {
                    biased;
                    () = cancellation(self.cancelled.clone()) => break (ManagedEventStop::Closed, None),
                    delivery = self.recovery_delivery(&reference, &cursor, lease, deadline) => delivery,
                };
            }
            if !matches!(delivery, Delivery::History { .. }) {
                cursor = grant.times.current_time.unwrap_or_default();
            }
        };
        self.status.send_replace(ManagedEventState::Closing);
        let cleanup_error = if known_reference {
            self.cleanup(&reference, self.options.cleanup_timeout).await
        } else {
            None
        };
        self.status.send_replace(ManagedEventState::Finished(stop));
        ManagedEventReport {
            stop,
            cause,
            cleanup_error,
            recoveries,
            recovery_cleanup_failures: cleanup_failures,
        }
    }

    async fn recovery_delivery(
        &self,
        reference: &EndpointReference,
        cursor: &str,
        lease: Lease,
        deadline: Instant,
    ) -> Delivery {
        // Leave room for normal lease renewal once the replacement starts pulling.
        let deadline = deadline.min(lease.observed + (lease.expires - lease.observed) / 2);
        let remaining = || {
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(3))
        };
        let endpoints = &self.options.endpoints;
        if timestamp(cursor).is_some_and(|from| from < lease.device_time)
            && !remaining().is_zero()
            && self
                .client
                .persistent_notification_storage(&self.url, endpoints, remaining())
                .await
                .unwrap_or(false)
            && !remaining().is_zero()
            && self
                .client
                .seek_referenced_pull_point(reference, cursor, endpoints, remaining())
                .await
                .is_ok()
        {
            return Delivery::History {
                until: lease.device_time,
            };
        }
        if !remaining().is_zero()
            && self
                .client
                .synchronize_referenced_pull_point(reference, endpoints, remaining())
                .await
                .is_ok()
        {
            return Delivery::Synchronized;
        }
        Delivery::Live
    }

    async fn cleanup(
        &self,
        reference: &EndpointReference,
        timeout: Duration,
    ) -> Option<OnvifError> {
        let error = self
            .client
            .unsubscribe_referenced_pull_point(reference, &self.options.endpoints, timeout)
            .await
            .err();
        error.filter(|error| {
            !(self.recover_history
                && matches!(error,
                OnvifError::Soap(SoapError::Fault { subcode: Some(code), .. })
                    if code == crate::soap::qualified::RESOURCE_UNKNOWN))
        })
    }

    async fn renew(
        &self,
        reference: &EndpointReference,
        clock: &watch::Sender<Lease>,
        mut updates: watch::Receiver<Lease>,
    ) -> Exit {
        loop {
            let schedule = |lease: Lease| {
                lease.expires
                    - self
                        .options
                        .renew_margin
                        .min(lease.expires.saturating_duration_since(lease.observed) / 3)
            };
            let mut lease = *updates.borrow_and_update();
            let mut renew_at = schedule(lease);
            loop {
                tokio::select! {
                    biased;
                    () = sleep_until(renew_at) => break,
                    _ = updates.changed() => {
                        let update = *updates.borrow_and_update();
                        renew_at = if update.device_expiry > lease.device_expiry {
                            schedule(update)
                        } else {
                            renew_at.min(schedule(update))
                        };
                        lease = update;
                    }
                }
            }
            let started = Instant::now();
            let timeout = self
                .options
                .request_timeout
                .min(lease.expires.saturating_duration_since(started));
            self.status.send_replace(ManagedEventState::Renewing);
            let times = match self
                .client
                .renew_referenced_pull_point(
                    reference,
                    &crate::types::duration_xml(self.options.lease),
                    &self.options.endpoints,
                    timeout,
                )
                .await
            {
                Ok(times) => times,
                Err(error) => return Exit::Request(error),
            };
            let Some(renewed) = Lease::from_times(&times, started, Some(lease)) else {
                return Exit::Timing;
            };
            if renewed.expires <= lease.expires {
                return Exit::Timing;
            }
            clock.send_if_modified(|current| {
                if current.sampled > started && current.expires >= renewed.expires {
                    return false;
                }
                *current = renewed;
                true
            });
            self.status.send_replace(ManagedEventState::Active);
        }
    }

    async fn pull(
        &self,
        reference: &EndpointReference,
        clock: &watch::Sender<Lease>,
        budget: &Arc<Semaphore>,
        cursor: &mut String,
        mut delivery: Delivery,
        resumed: Option<ManagedEvent>,
    ) -> Exit {
        if let Some(event) = resumed
            && !self
                .enqueue(QueuedEvent {
                    event,
                    _bytes: None,
                })
                .await
        {
            return Exit::Full;
        }
        let wait = crate::types::duration_xml(self.options.pull_wait);
        let mut pause = self.pause.clone();
        let mut connect_retry = false;
        let mut live_cursor = !matches!(delivery, Delivery::History { .. });
        let mut snapshots = matches!(delivery, Delivery::Synchronized)
            .then(|| SnapshotCache::new(self.options.queue_bytes as usize));
        loop {
            if self.policy.wait_for_capacity {
                let Ok(slot) = self.send.reserve().await else {
                    return Exit::Full;
                };
                drop(slot);
                let Ok(byte) = budget.acquire().await else {
                    return Exit::Full;
                };
                drop(byte);
            }
            let _ = pause.wait_for(|paused| !*paused).await;
            let started = Instant::now();
            let mut page = match self
                .client
                .pull_referenced_messages(
                    reference,
                    &wait,
                    self.options.page_messages,
                    &self.options.endpoints,
                    self.options.request_timeout,
                )
                .await
            {
                Ok(page) => {
                    connect_retry = false;
                    page
                }
                Err(OnvifError::Transport(TransportError::Http(error)))
                    if self.recover_history
                        && !connect_retry
                        && error.is_connect()
                        && clock.borrow().expires > Instant::now() + self.options.poll_interval =>
                {
                    connect_retry = true;
                    tokio::time::sleep(self.options.poll_interval).await;
                    continue;
                }
                Err(error) => return Exit::Request(error),
            };
            if let Some(topic) = &self.policy.topic {
                page.notifications.retain(|event| topic.matches(event));
            }
            if matches!(delivery, Delivery::Synchronized) && page.notifications.is_empty() {
                delivery = Delivery::Live;
                snapshots = None;
            }
            // The device response clock can be newer than notifications still
            // queued behind a nonempty page. Seek inclusively from the oldest
            // valid event time in that page, or the response clock if it was empty.
            let checkpoint = if page.notifications.is_empty() {
                page.times.current_time.clone()
            } else if self.recover_history {
                page.notifications
                    .iter()
                    .filter_map(|event| {
                        timestamp(&event.utc_time).map(|time| (time, &event.utc_time))
                    })
                    .min_by_key(|(time, _)| *time)
                    .map(|(_, utc_time)| utc_time.clone())
            } else {
                None
            };
            self.pending
                .store(page.notifications.len(), Ordering::Relaxed);
            let lease = *clock.borrow();
            if started >= lease.sampled {
                let Some(mut observed) = Lease::from_times(&page.times, started, None) else {
                    return Exit::Timing;
                };
                if observed.device_expiry == lease.device_expiry {
                    observed.expires = observed.expires.min(lease.expires);
                }
                clock.send_replace(observed);
            }
            for notification in page.notifications {
                if snapshots
                    .as_mut()
                    .is_some_and(|cache| cache.is_duplicate(&notification))
                {
                    self.pending.fetch_sub(1, Ordering::Relaxed);
                    continue;
                }
                let size = notification.retained_bytes() as u32;
                if size > self.options.queue_bytes {
                    return Exit::Full;
                }
                let permits = if self.policy.wait_for_capacity {
                    budget.clone().acquire_many_owned(size).await.ok()
                } else {
                    budget.clone().try_acquire_many_owned(size).ok()
                };
                let Some(bytes) = permits else {
                    return Exit::Full;
                };
                let event = match delivery {
                    Delivery::History { until } => match timestamp(&notification.utc_time) {
                        Some(time) if time < until => {
                            ManagedEvent::Recovered(Box::new(notification))
                        }
                        Some(_) => {
                            live_cursor = true;
                            ManagedEvent::Notification(Box::new(notification))
                        }
                        None => ManagedEvent::Notification(Box::new(notification)),
                    },
                    Delivery::Synchronized if notification.property_operation == "Initialized" => {
                        ManagedEvent::Snapshot(Box::new(notification))
                    }
                    _ => ManagedEvent::Notification(Box::new(notification)),
                };
                if !self
                    .enqueue(QueuedEvent {
                        event,
                        _bytes: Some(bytes),
                    })
                    .await
                {
                    return Exit::Full;
                }
                self.pending.fetch_sub(1, Ordering::Relaxed);
            }
            if live_cursor
                && let Some(current) = checkpoint
                && timestamp(&current) >= timestamp(cursor)
            {
                *cursor = current;
            }
            tokio::time::sleep(self.options.poll_interval).await;
        }
    }

    async fn enqueue(&self, event: QueuedEvent) -> bool {
        if self.policy.wait_for_capacity {
            self.send.send(event).await.is_ok()
        } else {
            self.send.try_send(event).is_ok()
        }
    }
}
