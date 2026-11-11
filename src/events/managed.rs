use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    task::JoinHandle,
    time::{Instant, sleep_until},
};

use super::{EndpointPolicy, EndpointReference, EventLeaseTimes, QualifiedNotification};
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
    /// Earlier unsubscribe failures encountered during recovery.
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
    task: Option<JoinHandle<ManagedEventReport>>,
    report: Option<ManagedEventReport>,
    terminal_delivered: bool,
}

impl ManagedEventSubscription {
    /// Current lifecycle state.
    pub fn state(&self) -> ManagedEventState {
        *self.state.borrow()
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
        let wrap = |cause| {
            Box::new(ManagedEventStartError {
                cause,
                cleanup_error: None,
            })
        };
        options.validate().map_err(wrap)?;
        let started = Instant::now();
        let point = self
            .create_referenced_pull_point(
                events_url,
                &crate::types::duration_xml(options.lease),
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
        let (status, state) = watch::channel(ManagedEventState::Active);
        let worker = Worker {
            client: self.clone(),
            url: events_url.to_owned(),
            options,
            send,
            status,
            cancelled,
        };
        let task = tokio::spawn(worker.run(point.reference, lease));
        Ok(ManagedEventSubscription {
            queue,
            state,
            cancel,
            task: Some(task),
            report: None,
            terminal_delivered: false,
        })
    }
}

impl OnvifSession {
    /// Start a managed event subscription using the resolved Events URL.
    pub async fn managed_events(
        &self,
        options: ManagedEventOptions,
    ) -> Result<ManagedEventSubscription, Box<ManagedEventStartError>> {
        let url = self.capabilities().events.url.as_deref().ok_or_else(|| {
            Box::new(ManagedEventStartError {
                cause: SoapError::missing("Events service URL").into(),
                cleanup_error: None,
            })
        })?;
        self.client().managed_events(url, options).await
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

fn timestamp(value: &str) -> Option<i128> {
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
    send: mpsc::Sender<QueuedEvent>,
    status: watch::Sender<ManagedEventState>,
    cancelled: watch::Receiver<bool>,
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
    async fn run(self, mut reference: EndpointReference, mut lease: Lease) -> ManagedEventReport {
        let budget = Arc::new(Semaphore::new(self.options.queue_bytes as usize));
        let mut recoveries = 0;
        let mut cleanup_failures = 0;
        let mut known_reference = true;
        let (stop, cause) = loop {
            let (clock, changes) = watch::channel(lease);
            self.status.send_replace(ManagedEventState::Active);
            let exit = tokio::select! {
                biased;
                () = cancellation(self.cancelled.clone()) => Exit::Closed,
                exit = self.renew(&reference, &clock, changes) => exit,
                exit = self.pull(&reference, &clock, &budget) => exit,
            };
            let error = match exit {
                Exit::Closed => break (ManagedEventStop::Closed, None),
                Exit::Full => break (ManagedEventStop::QueueFull, None),
                Exit::Timing => break (ManagedEventStop::InvalidTiming, Some(invalid_timing())),
                Exit::Request(error) if recoverable(&error) => error,
                Exit::Request(error) => break (ManagedEventStop::RequestFailed, Some(error)),
            };
            if recoveries == self.options.max_recoveries {
                break (ManagedEventStop::RecoveryExhausted, Some(error));
            }
            if self
                .send
                .try_send(QueuedEvent {
                    event: ManagedEvent::Gap,
                    _bytes: None,
                })
                .is_err()
            {
                break (ManagedEventStop::QueueFull, Some(error));
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
            known_reference = false;
            let wait = self.options.poll_interval;
            let started = Instant::now() + wait;
            if started >= deadline {
                break (ManagedEventStop::RecoveryExhausted, Some(error));
            }
            recoveries += 1;
            let grant = tokio::select! {
                biased;
                () = cancellation(self.cancelled.clone()) => break (ManagedEventStop::Closed, None),
                grant = async {
                    sleep_until(started).await;
                    self.client.create_referenced_pull_point(&self.url, &crate::types::duration_xml(self.options.lease),
                        &self.options.endpoints, self.options.request_timeout.min(deadline.saturating_duration_since(Instant::now()))).await
                } => grant,
            };
            let grant = match grant {
                Ok(grant) => grant,
                Err(error) => break (ManagedEventStop::RequestFailed, Some(error)),
            };
            reference = grant.reference;
            known_reference = true;
            let Some(new_lease) = Lease::from_times(&grant.times, started, None) else {
                break (ManagedEventStop::InvalidTiming, Some(invalid_timing()));
            };
            lease = new_lease;
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

    async fn cleanup(
        &self,
        reference: &EndpointReference,
        timeout: Duration,
    ) -> Option<OnvifError> {
        self.client
            .unsubscribe_referenced_pull_point(reference, &self.options.endpoints, timeout)
            .await
            .err()
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
    ) -> Exit {
        let wait = crate::types::duration_xml(self.options.pull_wait);
        loop {
            let started = Instant::now();
            let page = match self
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
                Ok(page) => page,
                Err(error) => return Exit::Request(error),
            };
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
                let Ok(bytes) = budget
                    .clone()
                    .try_acquire_many_owned(notification.retained_bytes() as u32)
                else {
                    return Exit::Full;
                };
                if self
                    .send
                    .try_send(QueuedEvent {
                        event: ManagedEvent::Notification(Box::new(notification)),
                        _bytes: Some(bytes),
                    })
                    .is_err()
                {
                    return Exit::Full;
                }
            }
            tokio::time::sleep(self.options.poll_interval).await;
        }
    }
}
