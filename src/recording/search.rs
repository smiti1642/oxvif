use std::{fmt, future::Future, sync::Arc, time::Duration};
use tokio::time::{Instant, sleep, sleep_until, timeout};

use crate::{OnvifClient, OnvifError, types::RecordingInformation};

/// Why a bounded recording search stopped. Only `Completed` is complete success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordingSearchStopReason {
    /// The device explicitly reported Completed within every result budget.
    Completed,
    /// The workflow's total Find/poll deadline elapsed.
    Deadline,
    /// The maximum number of successfully parsed result pages was received.
    PageLimit,
    /// A page would exceed the retained item limit or requested MaxMatches.
    ItemLimit,
    /// A page would exceed the accounted retained-result byte limit.
    ByteLimit,
    /// The cancellation handle was signalled.
    Cancelled,
    /// The find request failed before a usable token was acquired.
    FindFailed,
    /// A result request or its response parser failed.
    PollFailed,
    /// The device reported a state other than Queued, Searching or Completed.
    UnknownState,
    /// Options were rejected before network access.
    InvalidOptions,
    /// This single-use owner was already run or closed.
    AlreadyUsed,
}

impl fmt::Display for RecordingSearchStopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Completed => "completed",
            Self::Deadline => "deadline",
            Self::PageLimit => "page limit",
            Self::ItemLimit => "item limit",
            Self::ByteLimit => "byte limit",
            Self::Cancelled => "cancelled",
            Self::FindFailed => "find request failed",
            Self::PollFailed => "result request failed",
            Self::UnknownState => "unrecognized search state",
            Self::InvalidOptions => "invalid options",
            Self::AlreadyUsed => "owner already used",
        })
    }
}

/// Configurable workflow budgets, independent of a transport's shorter timeout.
///
/// Defaults: 120 seconds total, two seconds for cleanup, 20 pages, 2,000 items,
/// 16 MiB accounted retained bytes, 100 results/page, PT5S wait, PT60S keepalive
/// and 100 ms between nonterminal responses. No automatic retries occur.
///
/// Retained bytes charge each record's struct size plus UTF-8 field lengths.
/// This is an accounting limit, not a total allocator/peak-memory guarantee:
/// one decoded response and vector spare capacity also exist. Production XML
/// parsing retains its separate byte/depth/node limits.
#[derive(Clone, Debug)]
pub struct RecordingSearchOptions {
    total_timeout: Duration,
    cleanup_timeout: Duration,
    max_pages: usize,
    max_items: usize,
    max_retained_bytes: usize,
    page_size: u32,
    wait_time: Duration,
    keep_alive: Duration,
    min_poll_interval: Duration,
    max_matches: Option<u32>,
}

impl Default for RecordingSearchOptions {
    fn default() -> Self {
        Self {
            total_timeout: Duration::from_secs(120),
            cleanup_timeout: Duration::from_secs(2),
            max_pages: 20,
            max_items: 2_000,
            max_retained_bytes: 16 * 1024 * 1024,
            page_size: 100,
            wait_time: Duration::from_secs(5),
            keep_alive: Duration::from_secs(60),
            min_poll_interval: Duration::from_millis(100),
            max_matches: None,
        }
    }
}

impl RecordingSearchOptions {
    /// Set the total Find/poll budget; cleanup has its own separate budget.
    pub fn with_total_timeout(mut self, value: Duration) -> Self {
        self.total_timeout = value;
        self
    }
    /// Set the cleanup attempt deadline.
    pub fn with_cleanup_timeout(mut self, value: Duration) -> Self {
        self.cleanup_timeout = value;
        self
    }
    /// Set positive page, retained item and accounted retained byte limits.
    pub fn with_limits(mut self, pages: usize, items: usize, bytes: usize) -> Self {
        self.max_pages = pages;
        self.max_items = items;
        self.max_retained_bytes = bytes;
        self
    }
    /// Set requested MaxResults per page (positive and representable as xs:int).
    pub fn with_page_size(mut self, value: u32) -> Self {
        self.page_size = value;
        self
    }
    /// Set the device's long-poll wait. Zero is allowed with the minimum cadence.
    pub fn with_wait_time(mut self, value: Duration) -> Self {
        self.wait_time = value;
        self
    }
    /// Set the requested positive remote search keepalive duration.
    pub fn with_keep_alive(mut self, value: Duration) -> Self {
        self.keep_alive = value;
        self
    }
    /// Set a positive delay between nonterminal replies to avoid busy polling.
    pub fn with_min_poll_interval(mut self, value: Duration) -> Self {
        self.min_poll_interval = value;
        self
    }
    /// Set optional Find MaxMatches; it also caps retained results.
    pub fn with_max_matches(mut self, value: Option<u32>) -> Self {
        self.max_matches = value;
        self
    }
    /// Total Find/poll timeout.
    pub fn total_timeout(&self) -> Duration {
        self.total_timeout
    }
    /// Separate cleanup timeout.
    pub fn cleanup_timeout(&self) -> Duration {
        self.cleanup_timeout
    }
    /// Maximum parsed result pages.
    pub fn max_pages(&self) -> usize {
        self.max_pages
    }
    /// Maximum retained records, before an optional smaller MaxMatches.
    pub fn max_items(&self) -> usize {
        self.max_items
    }
    /// Maximum accounted retained bytes.
    pub fn max_retained_bytes(&self) -> usize {
        self.max_retained_bytes
    }
    /// Requested results per page.
    pub fn page_size(&self) -> u32 {
        self.page_size
    }
    /// Requested device wait duration.
    pub fn wait_time(&self) -> Duration {
        self.wait_time
    }
    /// Requested remote keepalive duration.
    pub fn keep_alive(&self) -> Duration {
        self.keep_alive
    }
    /// Minimum delay after a nonterminal response.
    pub fn min_poll_interval(&self) -> Duration {
        self.min_poll_interval
    }
    /// Requested Find MaxMatches.
    pub fn max_matches(&self) -> Option<u32> {
        self.max_matches
    }

    fn validate(&self) -> Result<(), OnvifError> {
        let now = Instant::now();
        if self.total_timeout.is_zero()
            || self.cleanup_timeout.is_zero()
            || self.keep_alive.is_zero()
            || self.min_poll_interval.is_zero()
            || now.checked_add(self.total_timeout).is_none()
            || now.checked_add(self.cleanup_timeout).is_none()
            || now.checked_add(self.min_poll_interval).is_none()
            || self.max_pages == 0
            || self.max_items == 0
            || self.max_retained_bytes == 0
            || self.page_size == 0
            || self.page_size > i32::MAX as u32
            || self
                .max_matches
                .is_some_and(|n| n == 0 || n > i32::MAX as u32)
        {
            return Err(OnvifError::InvalidArgument(
                "Invalid recording search budgets or xs:int result limits".into(),
            ));
        }
        Ok(())
    }
}

/// Cloneable, one-way cancellation signal; it never owns or spawns a task.
#[derive(Clone, Debug)]
pub struct RecordingSearchCancellation(tokio::sync::watch::Sender<bool>);

impl Default for RecordingSearchCancellation {
    fn default() -> Self {
        Self(tokio::sync::watch::channel(false).0)
    }
}

impl RecordingSearchCancellation {
    /// Signal cooperative cancellation. A running workflow then attempts cleanup.
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    /// Whether cancellation has been signalled.
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }
    async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|value| *value).await;
    }
}

/// Outcome of the owner's single remote cleanup attempt.
///
/// A timeout/interrupted reply does not prove that the device failed to close.
/// Error and endpoint values can contain device data; routine logs should use
/// controlled status metadata rather than formatting this whole value.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecordingSearchCleanup {
    /// No usable token was received. A lost Find response may still leave a lease.
    NoKnownToken,
    /// EndSearch returned successfully; the optional endpoint value is preserved.
    Released {
        /// Device-provided EndSearch endpoint time, if present.
        endpoint: Option<String>,
    },
    /// EndSearch failed; the original typed cause is retained.
    Failed(OnvifError),
    /// The separate cleanup deadline elapsed.
    TimedOut,
    /// The close future was externally dropped during its one cleanup attempt.
    Interrupted,
}

/// Results retained within the budgets and the independent cleanup outcome.
#[derive(Debug)]
pub struct RecordingSearchReport {
    recordings: Vec<RecordingInformation>,
    pages: usize,
    retained_bytes: usize,
    stop_reason: RecordingSearchStopReason,
    cleanup: Arc<RecordingSearchCleanup>,
}

impl RecordingSearchReport {
    /// Accepted records in device order, without token deduplication.
    pub fn recordings(&self) -> &[RecordingInformation] {
        &self.recordings
    }
    /// Consume the report to obtain accepted records.
    pub fn into_recordings(self) -> Vec<RecordingInformation> {
        self.recordings
    }
    /// Successfully parsed pages, including a page refused by a retention limit.
    pub fn pages(&self) -> usize {
        self.pages
    }
    /// Accounted bytes of accepted records only.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Why polling stopped; only Completed establishes completion.
    pub fn stop_reason(&self) -> RecordingSearchStopReason {
        self.stop_reason
    }
    /// The cleanup outcome, separate from polling completion or its error.
    pub fn cleanup(&self) -> &RecordingSearchCleanup {
        &self.cleanup
    }
}

/// Search failure with accepted partial results and separate cleanup evidence.
///
/// Display uses a controlled stop reason. The original cause and report can
/// contain device data; explicitly review them before logging or sharing.
#[derive(Debug)]
pub struct RecordingSearchFailure {
    report: RecordingSearchReport,
    cause: Option<OnvifError>,
}

impl RecordingSearchFailure {
    /// Partial result and cleanup report.
    pub fn report(&self) -> &RecordingSearchReport {
        &self.report
    }
    /// Original Find/poll/local-validation error, when one exists.
    pub fn cause(&self) -> Option<&OnvifError> {
        self.cause.as_ref()
    }
    /// Consume the failure into its report and original cause.
    pub fn into_parts(self) -> (RecordingSearchReport, Option<OnvifError>) {
        (self.report, self.cause)
    }
    pub(crate) fn invalid_options(error: OnvifError) -> Box<Self> {
        Self::before_start(RecordingSearchStopReason::InvalidOptions, error)
    }
    pub(crate) fn before_start(reason: RecordingSearchStopReason, error: OnvifError) -> Box<Self> {
        Box::new(Self {
            report: RecordingSearchReport {
                recordings: Vec::new(),
                pages: 0,
                retained_bytes: 0,
                stop_reason: reason,
                cleanup: Arc::new(RecordingSearchCleanup::NoKnownToken),
            },
            cause: Some(error),
        })
    }
    pub(crate) fn into_legacy_error(self) -> OnvifError {
        self.cause.unwrap_or(OnvifError::RecordingSearchIncomplete {
            reason: self.report.stop_reason,
        })
    }
}

impl fmt::Display for RecordingSearchFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Recording search stopped: {}", self.report.stop_reason)
    }
}
impl std::error::Error for RecordingSearchFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|error| error as _)
    }
}

/// Single-use recording search owner. Construction performs no network I/O.
///
/// `run` returns Ok only for Completed; successful completion may still carry a
/// failed cleanup outcome. Rejected pages are never partially retained. Explicit
/// close is idempotent and attempts EndSearch at most once, even if its future is
/// dropped. Dropping `run` leaves the owner available for `cancel_and_close`.
/// Dropping the owner cancels locally only: it performs no async cleanup or spawn.
pub struct RecordingSearch(SearchOwner<RecordingQuery>);

impl RecordingSearch {
    pub(crate) fn new(
        client: OnvifClient,
        url: &str,
        options: RecordingSearchOptions,
    ) -> Result<Self, OnvifError> {
        SearchOwner::new(
            RecordingQuery {
                client,
                url: url.into(),
            },
            options,
        )
        .map(Self)
    }

    /// Obtain a signal usable concurrently while `run` borrows this owner.
    pub fn cancellation(&self) -> RecordingSearchCancellation {
        self.0.cancellation()
    }

    /// Effective workflow budgets.
    pub fn options(&self) -> &RecordingSearchOptions {
        self.0.options()
    }

    /// Find, poll within budgets, and attempt bounded cleanup on normal return.
    /// An externally dropped future needs explicit `cancel_and_close`.
    pub async fn run(&mut self) -> Result<RecordingSearchReport, Box<RecordingSearchFailure>> {
        let report = |data: SearchReportData<RecordingInformation>| RecordingSearchReport {
            recordings: data.items,
            pages: data.pages,
            retained_bytes: data.retained_bytes,
            stop_reason: data.stop_reason,
            cleanup: data.cleanup,
        };
        self.0.run().await.map(report).map_err(|(data, cause)| {
            Box::new(RecordingSearchFailure {
                report: report(data),
                cause: cause.map(|error| *error),
            })
        })
    }

    /// End a known remote search once. Subsequent calls return the cached outcome.
    pub async fn close(&mut self) -> Arc<RecordingSearchCleanup> {
        self.0.close().await
    }

    /// Signal cancellation and explicitly close a suspended or dropped run.
    pub async fn cancel_and_close(&mut self) -> Arc<RecordingSearchCleanup> {
        self.0.cancel_and_close().await
    }
}

pub(crate) trait SearchQuery {
    type Item;

    fn find(
        &self,
        max_matches: Option<u32>,
        keep_alive: &str,
    ) -> impl Future<Output = Result<String, OnvifError>> + Send;
    fn page(
        &self,
        token: &str,
        size: u32,
        wait: &str,
    ) -> impl Future<Output = Result<SearchPage<Self::Item>, OnvifError>> + Send;
    fn end(&self, token: &str) -> impl Future<Output = Result<Option<String>, OnvifError>> + Send;
    fn item_bytes(item: &Self::Item) -> usize;
}

pub(crate) struct SearchPage<T> {
    pub(crate) search_state: String,
    pub(crate) items: Vec<T>,
}

pub(crate) struct SearchReportData<T> {
    pub(crate) items: Vec<T>,
    pub(crate) pages: usize,
    pub(crate) retained_bytes: usize,
    pub(crate) stop_reason: RecordingSearchStopReason,
    pub(crate) cleanup: Arc<RecordingSearchCleanup>,
}

type SearchOutcome<T> = Result<SearchReportData<T>, (SearchReportData<T>, Option<Box<OnvifError>>)>;

struct RecordingQuery {
    client: OnvifClient,
    url: String,
}

impl SearchQuery for RecordingQuery {
    type Item = RecordingInformation;

    async fn find(&self, max_matches: Option<u32>, keep_alive: &str) -> Result<String, OnvifError> {
        self.client
            .find_recordings(&self.url, max_matches, keep_alive)
            .await
    }

    async fn page(
        &self,
        token: &str,
        size: u32,
        wait: &str,
    ) -> Result<SearchPage<Self::Item>, OnvifError> {
        let page = self
            .client
            .get_recording_search_results(&self.url, token, size, wait)
            .await?;
        Ok(SearchPage {
            search_state: page.search_state,
            items: page.recording_information,
        })
    }

    async fn end(&self, token: &str) -> Result<Option<String>, OnvifError> {
        self.client.end_search_endpoint(&self.url, token).await
    }

    fn item_bytes(item: &Self::Item) -> usize {
        record_bytes(item)
    }
}

pub(crate) struct SearchOwner<Q: SearchQuery> {
    query: Q,
    options: RecordingSearchOptions,
    cancellation: RecordingSearchCancellation,
    started: bool,
    closed: bool,
    token: Option<String>,
    cleanup: Option<Arc<RecordingSearchCleanup>>,
    recordings: Vec<Q::Item>,
    pages: usize,
    retained_bytes: usize,
}

impl<Q: SearchQuery> SearchOwner<Q> {
    pub(crate) fn new(query: Q, options: RecordingSearchOptions) -> Result<Self, OnvifError> {
        options.validate()?;
        Ok(Self {
            query,
            options,
            cancellation: RecordingSearchCancellation::default(),
            started: false,
            closed: false,
            token: None,
            cleanup: None,
            recordings: Vec::new(),
            pages: 0,
            retained_bytes: 0,
        })
    }
    /// Obtain a signal usable concurrently while `run` borrows this owner.
    pub(crate) fn cancellation(&self) -> RecordingSearchCancellation {
        self.cancellation.clone()
    }
    /// Effective workflow budgets.
    pub(crate) fn options(&self) -> &RecordingSearchOptions {
        &self.options
    }
    /// Find, poll within budgets, and attempt bounded cleanup on normal return.
    ///
    /// # Errors
    /// Returns partial results and the original error on Find/poll failure, or a
    /// controlled budget/cancellation/state reason. An externally dropped future
    /// does not execute its remaining cleanup; call `cancel_and_close` on the owner.
    pub(crate) async fn run(&mut self) -> SearchOutcome<Q::Item> {
        if self.started || self.closed {
            let cleanup = self.close().await;
            return Err((
                self.take_report(RecordingSearchStopReason::AlreadyUsed, cleanup),
                None,
            ));
        }
        self.started = true;
        let outcome = match Instant::now().checked_add(self.options.total_timeout) {
            Some(deadline) => self.collect(deadline).await,
            None => Err((RecordingSearchStopReason::Deadline, None)),
        };
        let cleanup = self.close().await;
        let (reason, cause) = match outcome {
            Ok(()) => (RecordingSearchStopReason::Completed, None),
            Err(failure) => failure,
        };
        let report = self.take_report(reason, cleanup);
        if reason == RecordingSearchStopReason::Completed {
            Ok(report)
        } else {
            Err((report, cause.map(Box::new)))
        }
    }
    /// Close local ownership and attempt remote EndSearch once within its budget.
    ///
    /// The cached outcome makes repeated calls idempotent. Cancellation cannot
    /// skip this cleanup budget. If this close future is dropped, Interrupted is
    /// cached instead of silently retrying an operation of unknown remote outcome.
    pub(crate) async fn close(&mut self) -> Arc<RecordingSearchCleanup> {
        self.closed = true;
        self.cancellation.cancel();
        if let Some(outcome) = &self.cleanup {
            return outcome.clone();
        }
        let outcome = if let Some(token) = self.token.take() {
            self.cleanup = Some(Arc::new(RecordingSearchCleanup::Interrupted));
            match timeout(self.options.cleanup_timeout, self.query.end(&token)).await {
                Ok(Ok(endpoint)) => RecordingSearchCleanup::Released { endpoint },
                Ok(Err(error)) => RecordingSearchCleanup::Failed(error),
                Err(_) => RecordingSearchCleanup::TimedOut,
            }
        } else {
            RecordingSearchCleanup::NoKnownToken
        };
        let outcome = Arc::new(outcome);
        self.cleanup = Some(outcome.clone());
        outcome
    }
    /// Signal cancellation and explicitly close a suspended/dropped run.
    pub(crate) async fn cancel_and_close(&mut self) -> Arc<RecordingSearchCleanup> {
        self.cancellation.cancel();
        self.close().await
    }

    async fn collect(&mut self, deadline: Instant) -> SearchResult<()> {
        let keep_alive = crate::types::duration_xml(self.options.keep_alive);
        self.token = Some(
            self.wait(
                deadline,
                self.query.find(self.options.max_matches, &keep_alive),
                RecordingSearchStopReason::FindFailed,
            )
            .await?,
        );
        let Some(token) = self.token.as_deref() else {
            return Err((RecordingSearchStopReason::FindFailed, None));
        };
        let token = token.to_owned();
        let wait_time = crate::types::duration_xml(self.options.wait_time);
        let item_limit = self
            .options
            .max_matches
            .map_or(self.options.max_items, |n| {
                self.options.max_items.min(n as usize)
            });
        loop {
            let page = self
                .wait(
                    deadline,
                    self.query.page(&token, self.options.page_size, &wait_time),
                    RecordingSearchStopReason::PollFailed,
                )
                .await?;
            self.pages += 1;
            if !matches!(
                page.search_state.as_str(),
                "Queued" | "Searching" | "Completed"
            ) {
                return Err((RecordingSearchStopReason::UnknownState, None));
            }
            if page.items.len() > item_limit.saturating_sub(self.recordings.len()) {
                return Err((RecordingSearchStopReason::ItemLimit, None));
            }
            let cost = page
                .items
                .iter()
                .fold(0usize, |sum, item| sum.saturating_add(Q::item_bytes(item)));
            if cost
                > self
                    .options
                    .max_retained_bytes
                    .saturating_sub(self.retained_bytes)
            {
                return Err((RecordingSearchStopReason::ByteLimit, None));
            }
            self.retained_bytes += cost;
            self.recordings.extend(page.items);
            if page.search_state == "Completed" {
                return Ok(());
            }
            if self.pages >= self.options.max_pages {
                return Err((RecordingSearchStopReason::PageLimit, None));
            }
            self.wait(
                deadline,
                async {
                    sleep(self.options.min_poll_interval).await;
                    Ok(())
                },
                RecordingSearchStopReason::PollFailed,
            )
            .await?;
        }
    }
    async fn wait<T>(
        &self,
        deadline: Instant,
        future: impl Future<Output = Result<T, OnvifError>>,
        stage: RecordingSearchStopReason,
    ) -> SearchResult<T> {
        tokio::select! {
            biased;
            () = self.cancellation.cancelled() => Err((RecordingSearchStopReason::Cancelled, None)),
            () = sleep_until(deadline) => Err((RecordingSearchStopReason::Deadline, None)),
            result = future => result.map_err(|error| (stage, Some(error))),
        }
    }
    fn take_report(
        &mut self,
        reason: RecordingSearchStopReason,
        cleanup: Arc<RecordingSearchCleanup>,
    ) -> SearchReportData<Q::Item> {
        SearchReportData {
            items: std::mem::take(&mut self.recordings),
            pages: std::mem::take(&mut self.pages),
            retained_bytes: std::mem::take(&mut self.retained_bytes),
            stop_reason: reason,
            cleanup,
        }
    }
}

impl<Q: SearchQuery> Drop for SearchOwner<Q> {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

type SearchResult<T> = Result<T, (RecordingSearchStopReason, Option<OnvifError>)>;

fn record_bytes(item: &RecordingInformation) -> usize {
    [
        Some(item.recording_token.as_str()),
        Some(item.source_name.as_str()),
        item.earliest_recording.as_deref(),
        item.latest_recording.as_deref(),
        Some(item.content.as_str()),
        Some(item.recording_status.as_str()),
    ]
    .into_iter()
    .flatten()
    .fold(std::mem::size_of::<RecordingInformation>(), |sum, value| {
        sum.saturating_add(value.len())
    })
}
