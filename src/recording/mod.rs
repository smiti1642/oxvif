//! Owned, bounded recording workflows over the stateless service RPCs.
//!
//! [`RecordingSearch`] keeps partial results and cleanup evidence separate from
//! completion. It never spawns a task or retries a failed RPC. Explicit async
//! close is needed for remote cleanup after an externally dropped run future;
//! dropping the owner only cancels local work. A lost Find reply can leave an
//! unknown remote token until the device's lease expires.

mod export;
mod search;
mod timeline;

#[cfg(feature = "recording-download")]
pub mod download;

pub use export::{
    ExportFileProgress, ExportJob, ExportPollError, ExportPollOptions, ExportProgress,
    ExportRequest, ExportService,
};

pub use timeline::{
    EventSearch, EventSearchFailure, EventSearchReport, EventSearchRequest, EventTopic,
    RecordedEvent, RecordingMediaAttributes, RecordingScope, RecordingSource, RecordingSummary,
    RecordingTimeline, RecordingTimestamp, TimelineRecording,
};

pub use search::{
    RecordingSearch, RecordingSearchCancellation, RecordingSearchCleanup, RecordingSearchFailure,
    RecordingSearchOptions, RecordingSearchReport, RecordingSearchStopReason,
};
