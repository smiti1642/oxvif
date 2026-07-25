//! Selectable ONVIF read-surface for the clone sweep.
//!
//! The recorder ([`record_standard_surface`](super::record_standard_surface))
//! drives a set of **non-destructive `Get*` operations** against a live camera
//! and lands the responses in a [`FixtureStore`](super::FixtureStore). This
//! module lets a caller choose *which* operations to drive, at two levels of
//! granularity:
//!
//! - **[`SurfaceGroup`]** — a coarse service zone (identity, network, media,
//!   PTZ, imaging, events, media2). The common "pick which areas to clone" UI.
//! - **[`SurfaceOp`]** — a single operation. The fine-grained level a
//!   professional tester needs to reproduce a model-specific quirk that only
//!   appears on one command.
//!
//! Many per-token reads depend on a prior list read (`GetStreamUri` needs the
//! profile tokens from `GetProfiles`). That relationship is exposed via
//! [`SurfaceOp::requires`] so a UI can render the surface as a dependency tree
//! and light up a parent when a child is ticked. At drive time the prerequisite
//! is always run (and, through the recording tap, captured) so the resulting
//! clone can actually be replayed — a `GetStreamUri` fixture is useless without
//! the `GetProfiles` that yields its token.
//!
//! Media2 operations are ordinary [`SurfaceOp`]s in the [`SurfaceGroup::Media2`]
//! zone; on a device that only speaks Media1 they simply fail best-effort and
//! are skipped, so there is no harm in leaving them selected.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::OnvifSession;

/// A coarse service zone — the top level of the selectable read surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SurfaceGroup {
    /// Device identity & service discovery.
    Identity,
    /// Network read surface (interfaces, DNS, NTP, …).
    Network,
    /// Media1: profiles, stream/snapshot URIs, encoder/source configs, OSD, audio.
    Media,
    /// Pan/tilt/zoom configurations, nodes, presets, status.
    Ptz,
    /// Imaging (exposure/focus) settings and options, per video source.
    Imaging,
    /// Event topology.
    Events,
    /// Media2 service (Profile T/S cameras).
    Media2,
}

impl SurfaceGroup {
    /// Every zone, in a stable display order.
    pub const ALL: &'static [SurfaceGroup] = &[
        SurfaceGroup::Identity,
        SurfaceGroup::Network,
        SurfaceGroup::Media,
        SurfaceGroup::Ptz,
        SurfaceGroup::Imaging,
        SurfaceGroup::Events,
        SurfaceGroup::Media2,
    ];

    /// Human-readable zone name for a selection UI.
    pub fn label(self) -> &'static str {
        match self {
            SurfaceGroup::Identity => "Identity & discovery",
            SurfaceGroup::Network => "Network",
            SurfaceGroup::Media => "Media",
            SurfaceGroup::Ptz => "PTZ",
            SurfaceGroup::Imaging => "Imaging",
            SurfaceGroup::Events => "Events",
            SurfaceGroup::Media2 => "Media2",
        }
    }

    /// The operations that make up this zone.
    pub fn ops(self) -> Vec<SurfaceOp> {
        SurfaceOp::ALL
            .iter()
            .copied()
            .filter(|o| o.group() == self)
            .collect()
    }
}

// Defines `SurfaceOp` plus its `ALL`, `group`, and `action_name` from a single
// list, so the three stay in sync when an operation is added.
macro_rules! surface_ops {
    ($( $variant:ident => ($group:ident, $name:literal) ),+ $(,)?) => {
        /// A single ONVIF read operation the sweep can drive.
        ///
        /// Per-token operations (e.g. [`SurfaceOp::GetStreamUri`]) are driven
        /// once per discovered token; the token source is given by
        /// [`SurfaceOp::requires`]. The derived ordering follows declaration
        /// order in [`SurfaceOp::ALL`], giving reports a stable sort.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[non_exhaustive]
        pub enum SurfaceOp { $( $variant ),+ }

        impl SurfaceOp {
            /// Every operation the sweep knows about, in a stable order.
            pub const ALL: &'static [SurfaceOp] = &[ $( SurfaceOp::$variant ),+ ];

            /// The service zone this operation belongs to.
            pub fn group(self) -> SurfaceGroup {
                match self { $( SurfaceOp::$variant => SurfaceGroup::$group ),+ }
            }

            /// The ONVIF operation name, for display in a selection UI.
            pub fn action_name(self) -> &'static str {
                match self { $( SurfaceOp::$variant => $name ),+ }
            }
        }
    };
}

surface_ops! {
    // Identity & discovery.
    GetDeviceInformation => (Identity, "GetDeviceInformation"),
    GetSystemDateAndTime => (Identity, "GetSystemDateAndTime"),
    GetServices => (Identity, "GetServices"),
    GetHostname => (Identity, "GetHostname"),
    GetScopes => (Identity, "GetScopes"),
    GetUsers => (Identity, "GetUsers"),

    // Network.
    GetNetworkInterfaces => (Network, "GetNetworkInterfaces"),
    GetNetworkProtocols => (Network, "GetNetworkProtocols"),
    GetDns => (Network, "GetDNS"),
    GetNtp => (Network, "GetNTP"),
    GetNetworkDefaultGateway => (Network, "GetNetworkDefaultGateway"),

    // Media1.
    GetProfiles => (Media, "GetProfiles"),
    GetProfile => (Media, "GetProfile"),
    GetStreamUri => (Media, "GetStreamUri"),
    GetSnapshotUri => (Media, "GetSnapshotUri"),
    GetVideoSources => (Media, "GetVideoSources"),
    GetVideoSourceConfigurations => (Media, "GetVideoSourceConfigurations"),
    GetVideoSourceConfiguration => (Media, "GetVideoSourceConfiguration"),
    GetVideoSourceConfigurationOptions => (Media, "GetVideoSourceConfigurationOptions"),
    GetVideoEncoderConfigurations => (Media, "GetVideoEncoderConfigurations"),
    GetVideoEncoderConfiguration => (Media, "GetVideoEncoderConfiguration"),
    GetVideoEncoderConfigurationOptions => (Media, "GetVideoEncoderConfigurationOptions"),
    GetOsds => (Media, "GetOSDs"),
    GetOsd => (Media, "GetOSD"),
    GetOsdOptions => (Media, "GetOSDOptions"),
    GetAudioSources => (Media, "GetAudioSources"),
    GetAudioSourceConfigurations => (Media, "GetAudioSourceConfigurations"),
    GetAudioEncoderConfigurations => (Media, "GetAudioEncoderConfigurations"),
    GetAudioEncoderConfiguration => (Media, "GetAudioEncoderConfiguration"),
    GetAudioEncoderConfigurationOptions => (Media, "GetAudioEncoderConfigurationOptions"),

    // PTZ.
    GetPtzConfigurations => (Ptz, "GetConfigurations"),
    GetPtzConfiguration => (Ptz, "GetConfiguration"),
    GetPtzConfigurationOptions => (Ptz, "GetConfigurationOptions"),
    GetPtzNodes => (Ptz, "GetNodes"),
    GetPtzNode => (Ptz, "GetNode"),
    GetPtzPresets => (Ptz, "GetPresets"),
    GetPtzStatus => (Ptz, "GetStatus"),
    GetPtzCompatibleConfigurations => (Ptz, "GetCompatibleConfigurations"),

    // Imaging.
    GetImagingSettings => (Imaging, "GetImagingSettings"),
    GetImagingOptions => (Imaging, "GetOptions"),
    GetImagingMoveOptions => (Imaging, "GetMoveOptions"),
    GetImagingStatus => (Imaging, "GetStatus"),

    // Events.
    GetEventProperties => (Events, "GetEventProperties"),

    // Media2.
    GetProfilesMedia2 => (Media2, "GetProfiles"),
    GetStreamUriMedia2 => (Media2, "GetStreamUri"),
    GetSnapshotUriMedia2 => (Media2, "GetSnapshotUri"),
    GetVideoSourceConfigurationsMedia2 => (Media2, "GetVideoSourceConfigurations"),
    GetVideoSourceConfigurationOptionsMedia2 => (Media2, "GetVideoSourceConfigurationOptions"),
    GetVideoEncoderConfigurationsMedia2 => (Media2, "GetVideoEncoderConfigurations"),
    GetVideoEncoderConfigurationMedia2 => (Media2, "GetVideoEncoderConfiguration"),
    GetVideoEncoderConfigurationOptionsMedia2 => (Media2, "GetVideoEncoderConfigurationOptions"),
    GetVideoEncoderInstancesMedia2 => (Media2, "GetVideoEncoderInstances"),
}

impl SurfaceOp {
    /// The list read this operation needs first, if any — its token source.
    ///
    /// `GetStreamUri` returns `Some(GetProfiles)` because it must be driven once
    /// per profile token. A UI renders these as parent → child edges; the driver
    /// always runs the prerequisite so the recorded clone is replayable.
    /// Top-level list reads (`GetProfiles`, `GetVideoSources`, …) return `None`.
    pub fn requires(self) -> Option<SurfaceOp> {
        use SurfaceOp::*;
        Some(match self {
            GetProfile
            | GetStreamUri
            | GetSnapshotUri
            | GetPtzPresets
            | GetPtzStatus
            | GetPtzCompatibleConfigurations => GetProfiles,
            GetVideoSourceConfiguration | GetVideoSourceConfigurationOptions => {
                GetVideoSourceConfigurations
            }
            GetVideoEncoderConfiguration | GetVideoEncoderConfigurationOptions => {
                GetVideoEncoderConfigurations
            }
            GetOsd | GetOsdOptions => GetOsds,
            GetAudioEncoderConfiguration | GetAudioEncoderConfigurationOptions => {
                GetAudioEncoderConfigurations
            }
            GetPtzConfiguration | GetPtzConfigurationOptions => GetPtzConfigurations,
            GetPtzNode => GetPtzNodes,
            GetImagingSettings | GetImagingOptions | GetImagingMoveOptions | GetImagingStatus => {
                GetVideoSources
            }
            GetStreamUriMedia2 | GetSnapshotUriMedia2 => GetProfilesMedia2,
            GetVideoEncoderConfigurationMedia2
            | GetVideoEncoderConfigurationOptionsMedia2
            | GetVideoEncoderInstancesMedia2 => GetVideoEncoderConfigurationsMedia2,
            _ => return None,
        })
    }
}

/// A chosen set of [`SurfaceOp`]s to drive — exactly what the user ticked.
///
/// Store the user's literal picks here; the driver expands prerequisites
/// ([`SurfaceOp::requires`]) internally, so a selection of just
/// [`SurfaceOp::GetStreamUri`] still yields a replayable clone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SurfaceSelection(HashSet<SurfaceOp>);

impl SurfaceSelection {
    /// The empty selection.
    pub fn none() -> Self {
        Self::default()
    }

    /// Every known operation.
    pub fn all() -> Self {
        Self(SurfaceOp::ALL.iter().copied().collect())
    }

    /// The recommended default sweep: every non-destructive read across all
    /// zones. This is what [`drive_standard_surface`] uses.
    pub fn recommended() -> Self {
        Self::all()
    }

    /// A selection built from whole zones.
    pub fn from_groups(groups: &[SurfaceGroup]) -> Self {
        let mut set = HashSet::new();
        for g in groups {
            set.extend(g.ops());
        }
        Self(set)
    }

    /// Builder: add one operation.
    pub fn with(mut self, op: SurfaceOp) -> Self {
        self.0.insert(op);
        self
    }

    /// Builder: add a whole zone.
    pub fn with_group(mut self, group: SurfaceGroup) -> Self {
        self.0.extend(group.ops());
        self
    }

    /// Add an operation in place.
    pub fn insert(&mut self, op: SurfaceOp) {
        self.0.insert(op);
    }

    /// Remove an operation in place.
    pub fn remove(&mut self, op: SurfaceOp) {
        self.0.remove(&op);
    }

    /// Whether `op` was picked (does **not** account for prerequisites).
    pub fn contains(&self, op: SurfaceOp) -> bool {
        self.0.contains(&op)
    }

    /// Number of picked operations.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing is picked.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate the picked operations (unordered).
    pub fn iter(&self) -> impl Iterator<Item = SurfaceOp> + '_ {
        self.0.iter().copied()
    }

    /// The picks plus every transitive prerequisite — what the driver runs.
    fn effective(&self) -> HashSet<SurfaceOp> {
        let mut eff = HashSet::new();
        for op in self.iter() {
            let mut cur = op;
            eff.insert(cur);
            while let Some(req) = cur.requires() {
                if !eff.insert(req) {
                    break;
                }
                cur = req;
            }
        }
        eff
    }
}

/// What happened to a selected [`SurfaceOp`] during a sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OpOutcome {
    /// The operation ran and at least one response was captured.
    Recorded,
    /// The operation was attempted but the device errored on every attempt
    /// (SOAP fault, malformed response, or unsupported service).
    Failed,
    /// The prerequisite list read succeeded but yielded no tokens (e.g. a fixed
    /// camera with no PTZ profiles), so this per-token operation had nothing to
    /// run against. Not a bug — the device simply has no such path.
    SkippedNoData,
    /// The prerequisite list read itself failed, so this operation could not be
    /// reached (its token source was unavailable).
    SkippedPrerequisite,
}

impl OpOutcome {
    /// Whether this outcome means the operation was left uncaptured.
    pub fn is_skipped(self) -> bool {
        matches!(
            self,
            OpOutcome::SkippedNoData | OpOutcome::SkippedPrerequisite
        )
    }
}

/// Per-operation result of a sweep — which selected operations were captured,
/// which the device errored on, and which were skipped (and why).
///
/// This is the "hard prerequisite" feedback: a per-token operation only reports
/// [`OpOutcome::Recorded`] when its token source actually produced data, so a
/// tester can tell "this device has no such path" ([`OpOutcome::SkippedNoData`])
/// apart from "the command itself broke" ([`OpOutcome::Failed`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SweepReport {
    outcomes: HashMap<SurfaceOp, OpOutcome>,
}

impl SweepReport {
    /// Record an attempt: upgrades to [`OpOutcome::Recorded`] on any success,
    /// otherwise leaves/sets [`OpOutcome::Failed`].
    fn observe(&mut self, op: SurfaceOp, ok: bool) {
        let entry = self.outcomes.entry(op).or_insert(OpOutcome::Failed);
        if ok {
            *entry = OpOutcome::Recorded;
        }
    }

    /// The outcome of `op`, or `None` if it was not part of the sweep.
    pub fn outcome(&self, op: SurfaceOp) -> Option<OpOutcome> {
        self.outcomes.get(&op).copied()
    }

    /// Every `(op, outcome)` pair, sorted by operation for stable output.
    pub fn entries(&self) -> Vec<(SurfaceOp, OpOutcome)> {
        let mut v: Vec<_> = self.outcomes.iter().map(|(&k, &o)| (k, o)).collect();
        v.sort_by_key(|(op, _)| *op);
        v
    }

    /// Operations whose outcome matches `outcome`, sorted.
    pub fn with_outcome(&self, outcome: OpOutcome) -> Vec<SurfaceOp> {
        let mut v: Vec<_> = self
            .outcomes
            .iter()
            .filter(|&(_, &o)| o == outcome)
            .map(|(&op, _)| op)
            .collect();
        v.sort();
        v
    }

    /// Operations that were captured.
    pub fn recorded(&self) -> Vec<SurfaceOp> {
        self.with_outcome(OpOutcome::Recorded)
    }

    /// Operations left uncaptured (either skip reason), sorted.
    pub fn skipped(&self) -> Vec<SurfaceOp> {
        let mut v: Vec<_> = self
            .outcomes
            .iter()
            .filter(|(_, o)| o.is_skipped())
            .map(|(&op, _)| op)
            .collect();
        v.sort();
        v
    }

    /// Whether every swept operation was captured (nothing failed or skipped).
    pub fn is_complete(&self) -> bool {
        self.outcomes.values().all(|o| *o == OpOutcome::Recorded)
    }
}

/// Progress of a surface sweep — one event per selected operation.
///
/// Emitted by [`drive_surface_with_progress`] and
/// [`record_surface_with_progress`](super::record_surface_with_progress).
///
/// **The unit is a [`SurfaceOp`], not an HTTP request.** A per-token operation
/// such as [`SurfaceOp::GetStreamUri`] contributes exactly *one* event no matter
/// how many profile tokens the device turns out to have — the request count is
/// unknowable before `GetProfiles` answers, whereas the operation count is known
/// up front, which is what a determinate progress bar needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepProgress {
    /// The operation this event is for.
    pub op: SurfaceOp,
    /// Operations resolved so far, counting this one — `1..=total`.
    pub done: usize,
    /// The number of **selected operations after prerequisite expansion**
    /// ([`SurfaceOp::requires`]) — i.e. everything the driver will resolve.
    /// Known before the sweep starts. Not a request count.
    pub total: usize,
}

/// Drive the recommended default read surface (every zone) against `session`.
///
/// Equivalent to [`drive_surface`] with [`SurfaceSelection::recommended`].
pub async fn drive_standard_surface(session: &OnvifSession) -> SweepReport {
    drive_surface(session, &SurfaceSelection::recommended()).await
}

/// Drive the chosen [`SurfaceSelection`] against `session`, returning a
/// [`SweepReport`] of what each operation did.
///
/// Prerequisite list reads (per [`SurfaceOp::requires`]) are run automatically
/// so per-token operations have their tokens. Every call is best-effort — a
/// device that lacks a service, profile, or token is simply skipped, and the
/// report says which and why. When `session`'s transport is a
/// [`RecordingTransport`](super::RecordingTransport), each successful exchange —
/// prerequisites included — lands in its store.
pub async fn drive_surface(session: &OnvifSession, selection: &SurfaceSelection) -> SweepReport {
    drive_surface_with_progress(session, selection, |_| {}).await
}

/// Accumulates a [`SweepReport`] while emitting one [`SweepProgress`] per
/// selected operation. Deliberately named so that `rep.observe(..)` in the
/// driver body reads and behaves exactly as it did before progress existed.
struct SweepRun<'a> {
    rep: SweepReport,
    done: usize,
    total: usize,
    progress: &'a (dyn Fn(SweepProgress) + Send + Sync),
}

impl SweepRun<'_> {
    /// Record an attempt, ticking progress the **first** time this operation is
    /// touched. A per-token operation is observed once per token but reported
    /// once; the payload carries no outcome because at first touch the outcome
    /// is still provisional (a later token can upgrade it to `Recorded`).
    fn observe(&mut self, op: SurfaceOp, ok: bool) {
        let first = !self.rep.outcomes.contains_key(&op);
        self.rep.observe(op, ok);
        if first {
            self.tick(op);
        }
    }

    fn tick(&mut self, op: SurfaceOp) {
        self.done += 1;
        (self.progress)(SweepProgress {
            op,
            done: self.done,
            total: self.total,
        });
    }
}

/// [`drive_surface`], reporting progress as it goes.
///
/// `progress` is invoked **once per selected operation** (after prerequisite
/// expansion): when the operation is first attempted, or — for one gated out by
/// a prerequisite that failed or yielded no tokens — when it is resolved as
/// skipped at the end. [`done`](SweepProgress::done) counts operations resolved,
/// so the first call carries `done == 1` and the last `done == total`.
///
/// Attempted operations are reported in the driver's fixed order; the trailing
/// skip-resolution batch is unordered (it walks the expanded selection's set),
/// so a UI should treat the op as a label, not as a position.
///
/// [`total`](SweepProgress::total) is the size of the prerequisite-expanded
/// selection, known before the first request goes out. It is **not** a request
/// count: per-token operations issue one request per token and that count only
/// becomes known once the device answers the list read. See [`SweepProgress`].
///
/// The callback is `Fn + Send + Sync` so it can be a closure that pushes into a
/// channel shared with a UI thread, and so the returned future stays `Send`.
///
/// ```no_run
/// # async fn run(session: &oxvif::OnvifSession) {
/// use oxvif::metamorph::{SurfaceSelection, drive_surface_with_progress};
/// let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
/// let report = drive_surface_with_progress(session, &SurfaceSelection::all(), move |p| {
///     let _ = tx.send(p);
/// })
/// .await;
/// # let _ = report;
/// # }
/// ```
pub async fn drive_surface_with_progress(
    session: &OnvifSession,
    selection: &SurfaceSelection,
    progress: impl Fn(SweepProgress) + Send + Sync,
) -> SweepReport {
    use SurfaceOp::*;
    let eff = selection.effective();
    let want = |op: SurfaceOp| eff.contains(&op);
    let mut rep = SweepRun {
        rep: SweepReport::default(),
        done: 0,
        total: eff.len(),
        progress: &progress,
    };

    // Identity & discovery.
    if want(GetDeviceInformation) {
        rep.observe(
            GetDeviceInformation,
            session.get_device_info().await.is_ok(),
        );
    }
    if want(GetSystemDateAndTime) {
        rep.observe(
            GetSystemDateAndTime,
            session.get_system_date_and_time().await.is_ok(),
        );
    }
    if want(GetServices) {
        rep.observe(GetServices, session.get_services().await.is_ok());
    }
    if want(GetHostname) {
        rep.observe(GetHostname, session.get_hostname().await.is_ok());
    }
    if want(GetScopes) {
        rep.observe(GetScopes, session.get_scopes().await.is_ok());
    }
    if want(GetUsers) {
        rep.observe(GetUsers, session.get_users().await.is_ok());
    }

    // Network.
    if want(GetNetworkInterfaces) {
        rep.observe(
            GetNetworkInterfaces,
            session.get_network_interfaces().await.is_ok(),
        );
    }
    if want(GetNetworkProtocols) {
        rep.observe(
            GetNetworkProtocols,
            session.get_network_protocols().await.is_ok(),
        );
    }
    if want(GetDns) {
        rep.observe(GetDns, session.get_dns().await.is_ok());
    }
    if want(GetNtp) {
        rep.observe(GetNtp, session.get_ntp().await.is_ok());
    }
    if want(GetNetworkDefaultGateway) {
        rep.observe(
            GetNetworkDefaultGateway,
            session.get_network_default_gateway().await.is_ok(),
        );
    }

    // Media1: profiles + per-profile stream / snapshot / PTZ-per-profile reads.
    if want(GetProfiles) {
        match session.get_profiles().await {
            Ok(profiles) => {
                rep.observe(GetProfiles, true);
                for p in &profiles {
                    if want(GetProfile) {
                        rep.observe(GetProfile, session.get_profile(&p.token).await.is_ok());
                    }
                    if want(GetStreamUri) {
                        rep.observe(GetStreamUri, session.get_stream_uri(&p.token).await.is_ok());
                    }
                    if want(GetSnapshotUri) {
                        rep.observe(
                            GetSnapshotUri,
                            session.get_snapshot_uri(&p.token).await.is_ok(),
                        );
                    }
                    if want(GetPtzPresets) {
                        rep.observe(
                            GetPtzPresets,
                            session.ptz_get_presets(&p.token).await.is_ok(),
                        );
                    }
                    if want(GetPtzStatus) {
                        rep.observe(GetPtzStatus, session.ptz_get_status(&p.token).await.is_ok());
                    }
                    if want(GetPtzCompatibleConfigurations) {
                        rep.observe(
                            GetPtzCompatibleConfigurations,
                            session
                                .ptz_get_compatible_configurations(&p.token)
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetProfiles, false),
        }
    }

    // Video source / encoder configurations + per-token options.
    if want(GetVideoSourceConfigurations) {
        match session.get_video_source_configurations().await {
            Ok(cfgs) => {
                rep.observe(GetVideoSourceConfigurations, true);
                for c in &cfgs {
                    if want(GetVideoSourceConfiguration) {
                        rep.observe(
                            GetVideoSourceConfiguration,
                            session
                                .get_video_source_configuration(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                    if want(GetVideoSourceConfigurationOptions) {
                        rep.observe(
                            GetVideoSourceConfigurationOptions,
                            session
                                .get_video_source_configuration_options(Some(&c.token))
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetVideoSourceConfigurations, false),
        }
    }
    if want(GetVideoEncoderConfigurations) {
        match session.get_video_encoder_configurations().await {
            Ok(cfgs) => {
                rep.observe(GetVideoEncoderConfigurations, true);
                for c in &cfgs {
                    if want(GetVideoEncoderConfiguration) {
                        rep.observe(
                            GetVideoEncoderConfiguration,
                            session
                                .get_video_encoder_configuration(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                    if want(GetVideoEncoderConfigurationOptions) {
                        rep.observe(
                            GetVideoEncoderConfigurationOptions,
                            session
                                .get_video_encoder_configuration_options(Some(&c.token))
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetVideoEncoderConfigurations, false),
        }
    }

    // OSD.
    if want(GetOsds) {
        match session.get_osds(None).await {
            Ok(osds) => {
                rep.observe(GetOsds, true);
                for o in &osds {
                    if want(GetOsd) {
                        rep.observe(GetOsd, session.get_osd(&o.token).await.is_ok());
                    }
                    if want(GetOsdOptions) {
                        rep.observe(
                            GetOsdOptions,
                            session
                                .get_osd_options(&o.video_source_config_token)
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetOsds, false),
        }
    }

    // Audio.
    if want(GetAudioSources) {
        rep.observe(GetAudioSources, session.get_audio_sources().await.is_ok());
    }
    if want(GetAudioSourceConfigurations) {
        rep.observe(
            GetAudioSourceConfigurations,
            session.get_audio_source_configurations().await.is_ok(),
        );
    }
    if want(GetAudioEncoderConfigurations) {
        match session.get_audio_encoder_configurations().await {
            Ok(cfgs) => {
                rep.observe(GetAudioEncoderConfigurations, true);
                for c in &cfgs {
                    if want(GetAudioEncoderConfiguration) {
                        rep.observe(
                            GetAudioEncoderConfiguration,
                            session
                                .get_audio_encoder_configuration(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                    if want(GetAudioEncoderConfigurationOptions) {
                        rep.observe(
                            GetAudioEncoderConfigurationOptions,
                            session
                                .get_audio_encoder_configuration_options(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetAudioEncoderConfigurations, false),
        }
    }

    // PTZ configurations & nodes.
    if want(GetPtzConfigurations) {
        match session.ptz_get_configurations().await {
            Ok(cfgs) => {
                rep.observe(GetPtzConfigurations, true);
                for c in &cfgs {
                    if want(GetPtzConfiguration) {
                        rep.observe(
                            GetPtzConfiguration,
                            session.ptz_get_configuration(&c.token).await.is_ok(),
                        );
                    }
                    if want(GetPtzConfigurationOptions) {
                        rep.observe(
                            GetPtzConfigurationOptions,
                            session
                                .ptz_get_configuration_options(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetPtzConfigurations, false),
        }
    }
    if want(GetPtzNodes) {
        match session.ptz_get_nodes().await {
            Ok(nodes) => {
                rep.observe(GetPtzNodes, true);
                for n in &nodes {
                    if want(GetPtzNode) {
                        rep.observe(GetPtzNode, session.ptz_get_node(&n.token).await.is_ok());
                    }
                }
            }
            Err(_) => rep.observe(GetPtzNodes, false),
        }
    }

    // Imaging, per physical video source (source list is a Media-zone read).
    if want(GetVideoSources) {
        match session.get_video_sources().await {
            Ok(sources) => {
                rep.observe(GetVideoSources, true);
                for s in &sources {
                    if want(GetImagingSettings) {
                        rep.observe(
                            GetImagingSettings,
                            session.get_imaging_settings(&s.token).await.is_ok(),
                        );
                    }
                    if want(GetImagingOptions) {
                        rep.observe(
                            GetImagingOptions,
                            session.get_imaging_options(&s.token).await.is_ok(),
                        );
                    }
                    if want(GetImagingMoveOptions) {
                        rep.observe(
                            GetImagingMoveOptions,
                            session.imaging_get_move_options(&s.token).await.is_ok(),
                        );
                    }
                    if want(GetImagingStatus) {
                        rep.observe(
                            GetImagingStatus,
                            session.imaging_get_status(&s.token).await.is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetVideoSources, false),
        }
    }

    // Event topology.
    if want(GetEventProperties) {
        rep.observe(
            GetEventProperties,
            session.get_event_properties().await.is_ok(),
        );
    }

    // Media2 — best-effort; on a Media1-only device these fail and are skipped.
    if want(GetProfilesMedia2) {
        match session.get_profiles_media2().await {
            Ok(profiles2) => {
                rep.observe(GetProfilesMedia2, true);
                for p in &profiles2 {
                    if want(GetStreamUriMedia2) {
                        rep.observe(
                            GetStreamUriMedia2,
                            session.get_stream_uri_media2(&p.token).await.is_ok(),
                        );
                    }
                    if want(GetSnapshotUriMedia2) {
                        rep.observe(
                            GetSnapshotUriMedia2,
                            session.get_snapshot_uri_media2(&p.token).await.is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetProfilesMedia2, false),
        }
    }
    if want(GetVideoSourceConfigurationsMedia2) {
        rep.observe(
            GetVideoSourceConfigurationsMedia2,
            session
                .get_video_source_configurations_media2()
                .await
                .is_ok(),
        );
    }
    if want(GetVideoSourceConfigurationOptionsMedia2) {
        rep.observe(
            GetVideoSourceConfigurationOptionsMedia2,
            session
                .get_video_source_configuration_options_media2(None)
                .await
                .is_ok(),
        );
    }
    if want(GetVideoEncoderConfigurationsMedia2) {
        match session.get_video_encoder_configurations_media2().await {
            Ok(cfgs) => {
                rep.observe(GetVideoEncoderConfigurationsMedia2, true);
                for c in &cfgs {
                    if want(GetVideoEncoderConfigurationMedia2) {
                        rep.observe(
                            GetVideoEncoderConfigurationMedia2,
                            session
                                .get_video_encoder_configuration_media2(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                    if want(GetVideoEncoderConfigurationOptionsMedia2) {
                        rep.observe(
                            GetVideoEncoderConfigurationOptionsMedia2,
                            session
                                .get_video_encoder_configuration_options_media2(Some(&c.token))
                                .await
                                .is_ok(),
                        );
                    }
                    if want(GetVideoEncoderInstancesMedia2) {
                        rep.observe(
                            GetVideoEncoderInstancesMedia2,
                            session
                                .get_video_encoder_instances_media2(&c.token)
                                .await
                                .is_ok(),
                        );
                    }
                }
            }
            Err(_) => rep.observe(GetVideoEncoderConfigurationsMedia2, false),
        }
    }

    // Resolve every selected op the driver never attempted: it was gated out by
    // its prerequisite. Classify why from the prerequisite's own outcome. All
    // `requires()` targets are top-level list reads (depth-1 tree), so a single
    // pass suffices.
    for op in eff.iter().copied() {
        if rep.rep.outcomes.contains_key(&op) {
            continue;
        }
        let outcome = match op.requires().and_then(|parent| rep.rep.outcome(parent)) {
            Some(OpOutcome::Recorded) => OpOutcome::SkippedNoData,
            _ => OpOutcome::SkippedPrerequisite,
        };
        rep.rep.outcomes.insert(op, outcome);
        rep.tick(op);
    }

    rep.rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OnvifSession;
    use crate::mock::MockTransport;
    use crate::transport::{Transport, TransportError};
    use async_trait::async_trait;
    use std::sync::Arc;

    #[test]
    fn every_op_belongs_to_exactly_one_zone_and_all_zones_partition_all() {
        let mut from_groups: Vec<SurfaceOp> =
            SurfaceGroup::ALL.iter().flat_map(|g| g.ops()).collect();
        from_groups.sort();
        let mut all = SurfaceOp::ALL.to_vec();
        all.sort();
        assert_eq!(
            from_groups, all,
            "zones must partition SurfaceOp::ALL exactly"
        );
    }

    #[test]
    fn requires_forms_a_depth_one_tree() {
        // The driver's single-pass skip classification assumes every prerequisite
        // is itself a top-level list read (no grandparents).
        for &op in SurfaceOp::ALL {
            if let Some(parent) = op.requires() {
                assert_eq!(
                    parent.requires(),
                    None,
                    "{:?}'s prerequisite {:?} must be top-level",
                    op,
                    parent
                );
            }
        }
    }

    #[test]
    fn effective_pulls_in_prerequisites() {
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);
        let eff = sel.effective();
        assert!(eff.contains(&SurfaceOp::GetStreamUri));
        assert!(
            eff.contains(&SurfaceOp::GetProfiles),
            "selecting GetStreamUri must pull in its GetProfiles prerequisite"
        );
        // The literal selection stays minimal — expansion is driver-only.
        assert!(!sel.contains(SurfaceOp::GetProfiles));
    }

    #[test]
    fn from_groups_selects_whole_zone() {
        let sel = SurfaceSelection::from_groups(&[SurfaceGroup::Network]);
        assert!(sel.contains(SurfaceOp::GetDns));
        assert!(!sel.contains(SurfaceOp::GetProfiles));
    }

    async fn mock_session(transport: Arc<dyn Transport>) -> OnvifSession {
        OnvifSession::builder("http://mock/onvif/device_service")
            .with_transport(transport)
            .build()
            .await
            .expect("session over mock should build")
    }

    #[tokio::test]
    async fn narrow_selection_records_only_selected_plus_prerequisites() {
        let session = mock_session(Arc::new(MockTransport::new())).await;
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);
        let report = drive_surface(&session, &sel).await;

        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(OpOutcome::Recorded)
        );
        assert_eq!(
            report.outcome(SurfaceOp::GetProfiles),
            Some(OpOutcome::Recorded),
            "prerequisite must be driven and reported"
        );
        // An unselected op is absent from the report entirely.
        assert_eq!(report.outcome(SurfaceOp::GetHostname), None);
    }

    /// Delegates to a [`MockTransport`] but fails one action, to drive the
    /// prerequisite-skip classification deterministically.
    struct FailAction {
        inner: MockTransport,
        fail_suffix: &'static str,
    }

    #[async_trait]
    impl Transport for FailAction {
        async fn soap_post(
            &self,
            url: &str,
            action: &str,
            body: String,
        ) -> Result<String, TransportError> {
            if action.ends_with(self.fail_suffix) {
                return Err(TransportError::HttpStatus {
                    status: 500,
                    body: String::new(),
                });
            }
            self.inner.soap_post(url, action, body).await
        }
    }

    #[tokio::test]
    async fn failed_prerequisite_marks_children_skipped() {
        // GetProfiles fails; GetStreamUri (which needs it) can't be reached.
        let transport = Arc::new(FailAction {
            inner: MockTransport::new(),
            fail_suffix: "/GetProfiles",
        });
        let session = mock_session(transport).await;
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);
        let report = drive_surface(&session, &sel).await;

        assert_eq!(
            report.outcome(SurfaceOp::GetProfiles),
            Some(OpOutcome::Failed)
        );
        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(OpOutcome::SkippedPrerequisite),
            "a child whose token source failed is SkippedPrerequisite, not Failed"
        );
        assert!(!report.is_complete());
    }

    /// A saved selection reloads as the same set — the "remember what the user
    /// ticked" round trip a UI relies on.
    #[test]
    fn surface_selection_json_roundtrip() {
        let sel = SurfaceSelection::none()
            .with(SurfaceOp::GetStreamUri)
            .with(SurfaceOp::GetProfiles);
        let json = serde_json::to_string(&sel).unwrap();
        let back: SurfaceSelection = serde_json::from_str(&json).unwrap();

        let mut before: Vec<_> = sel.iter().collect();
        let mut after: Vec<_> = back.iter().collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }

    /// `SweepReport` keys its map by `SurfaceOp`, so serialising it depends on
    /// the enum rendering as a JSON *object key* (a string) rather than a
    /// struct. Guards that specifically — it is the one shape that would fail
    /// at runtime while still compiling.
    #[test]
    fn sweep_report_json_roundtrip_uses_string_keys() {
        let mut report = SweepReport::default();
        report.observe(SurfaceOp::GetProfiles, true);
        report.observe(SurfaceOp::GetStreamUri, false);
        report
            .outcomes
            .insert(SurfaceOp::GetPtzPresets, OpOutcome::SkippedNoData);

        let json = serde_json::to_string(&report).unwrap();
        assert!(
            json.contains("\"GetProfiles\":\"Recorded\""),
            "op should be a string key, json was: {json}"
        );

        let back: SweepReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries(), report.entries());
    }

    // ── progress ──────────────────────────────────────────────────────────────

    /// Drain a sweep's progress events, sorted the way a caller sees them.
    fn sweep_events(report_events: Vec<SweepProgress>) -> (Vec<usize>, Vec<SurfaceOp>) {
        let dones = report_events.iter().map(|p| p.done).collect();
        let mut ops: Vec<SurfaceOp> = report_events.iter().map(|p| p.op).collect();
        ops.sort();
        (dones, ops)
    }

    /// `total` is the **selected operations after prerequisite expansion**, and a
    /// per-token operation ticks once however many tokens the device returns —
    /// the mock serves two profiles, so `GetStreamUri` is driven twice but
    /// reported once.
    #[tokio::test]
    async fn progress_totals_the_expanded_selection_and_ticks_once_per_op() {
        let session = mock_session(Arc::new(MockTransport::new())).await;
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);
        assert_eq!(sel.len(), 1, "the user ticked one op");
        assert_eq!(sel.effective().len(), 2, "expansion adds GetProfiles");

        let seen = std::sync::Mutex::new(Vec::new());
        let report =
            drive_surface_with_progress(&session, &sel, |p| seen.lock().unwrap().push(p)).await;
        let seen = seen.into_inner().unwrap();

        assert!(
            seen.iter().all(|p| p.total == 2),
            "total is the expanded selection size, not a request count: {seen:?}"
        );
        let (dones, ops) = sweep_events(seen);
        assert_eq!(dones, vec![1, 2], "one tick per op, ending at total");
        assert_eq!(ops, vec![SurfaceOp::GetProfiles, SurfaceOp::GetStreamUri]);

        // Both really were driven, and GetStreamUri ran per profile token — the
        // event count deliberately does not follow the request count.
        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(OpOutcome::Recorded)
        );
        assert!(
            session.get_profiles().await.expect("mock profiles").len() > 1,
            "the mock must serve several profiles for this test to mean anything"
        );
    }

    /// An operation gated out by a failed prerequisite is never attempted, so it
    /// ticks in the final skip-resolution pass instead — `done` still reaches
    /// `total`.
    #[tokio::test]
    async fn progress_ticks_operations_resolved_as_skipped() {
        let transport = Arc::new(FailAction {
            inner: MockTransport::new(),
            fail_suffix: "/GetProfiles",
        });
        let session = mock_session(transport).await;
        let sel = SurfaceSelection::none().with(SurfaceOp::GetStreamUri);

        let seen = std::sync::Mutex::new(Vec::new());
        let report =
            drive_surface_with_progress(&session, &sel, |p| seen.lock().unwrap().push(p)).await;
        let seen = seen.into_inner().unwrap();

        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(OpOutcome::SkippedPrerequisite)
        );
        let (dones, ops) = sweep_events(seen);
        assert_eq!(dones, vec![1, 2], "a skipped op still ticks exactly once");
        assert_eq!(ops, vec![SurfaceOp::GetProfiles, SurfaceOp::GetStreamUri]);
    }

    /// Every op of a full sweep ticks exactly once, and `drive_surface` — which
    /// delegates with a no-op callback — produces the identical report.
    #[tokio::test]
    async fn full_sweep_ticks_every_selected_op_exactly_once() {
        let session = mock_session(Arc::new(MockTransport::new())).await;
        let sel = SurfaceSelection::recommended();
        let total = sel.effective().len();

        let seen = std::sync::Mutex::new(Vec::new());
        let with = drive_surface_with_progress(&session, &sel, |p| seen.lock().unwrap().push(p))
            .await
            .entries();
        let seen = seen.into_inner().unwrap();

        let (dones, mut ops) = sweep_events(seen);
        assert_eq!(dones, (1..=total).collect::<Vec<_>>());
        ops.dedup();
        assert_eq!(ops.len(), total, "no operation ticks twice");
        assert_eq!(
            total,
            SurfaceOp::ALL.len(),
            "the recommended selection is everything"
        );

        let without = drive_surface(&session, &sel).await.entries();
        assert_eq!(with, without, "the no-progress form is unchanged");
    }

    /// The Dioxus-desktop shape: the callback is a closure that sends into a
    /// `tokio::sync::mpsc::UnboundedSender`, and the future stays `Send` so it
    /// can be spawned. Compile-checked, not assumed.
    #[tokio::test]
    async fn progress_callback_accepts_an_mpsc_sender() {
        fn assert_send<T: Send>(t: T) -> T {
            t
        }

        let session = mock_session(Arc::new(MockTransport::new())).await;
        let sel = SurfaceSelection::none().with(SurfaceOp::GetHostname);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SweepProgress>();

        let report = assert_send(drive_surface_with_progress(&session, &sel, move |p| {
            let _ = tx.send(p);
        }))
        .await;
        assert_eq!(
            report.outcome(SurfaceOp::GetHostname),
            Some(OpOutcome::Recorded)
        );

        let got = rx.recv().await.expect("one progress event");
        assert_eq!(
            (got.op, got.done, got.total),
            (SurfaceOp::GetHostname, 1, 1)
        );
        assert!(rx.recv().await.is_none(), "sender dropped with the future");
    }

    // ── NET 3: `is_complete()` on a non-empty report ──────────────────────────
    //
    // Stage 2 changes what `is_complete()` returns for an *empty* report, so
    // that case is deliberately left unpinned here. The non-empty semantics
    // below must survive the change untouched.

    /// A report in which every swept operation was `Recorded` is complete.
    #[test]
    fn is_complete_is_true_when_every_non_empty_outcome_is_recorded() {
        let mut report = SweepReport::default();
        report.observe(SurfaceOp::GetProfiles, true);
        report.observe(SurfaceOp::GetStreamUri, true);
        report.observe(SurfaceOp::GetHostname, true);

        assert_eq!(report.entries().len(), 3, "the report must be non-empty");
        assert!(report.is_complete());
        let recorded = report.recorded();
        assert_eq!(recorded.len(), 3);
        for op in [
            SurfaceOp::GetProfiles,
            SurfaceOp::GetStreamUri,
            SurfaceOp::GetHostname,
        ] {
            assert!(recorded.contains(&op), "{op:?} missing from {recorded:?}");
            assert_eq!(report.outcome(op), Some(OpOutcome::Recorded));
        }
        assert!(report.skipped().is_empty());
    }

    /// A single `Failed` outcome makes the whole report incomplete.
    #[test]
    fn is_complete_is_false_when_any_outcome_failed() {
        let mut report = SweepReport::default();
        report.observe(SurfaceOp::GetProfiles, true);
        report.observe(SurfaceOp::GetStreamUri, false);

        assert_eq!(
            report.outcome(SurfaceOp::GetStreamUri),
            Some(OpOutcome::Failed)
        );
        assert!(!report.is_complete());
        assert_eq!(
            report.skipped(),
            Vec::<SurfaceOp>::new(),
            "Failed is not a skip"
        );
    }

    /// Either skip reason also makes the report incomplete.
    #[test]
    fn is_complete_is_false_when_any_outcome_is_skipped() {
        for skipped in [OpOutcome::SkippedNoData, OpOutcome::SkippedPrerequisite] {
            let mut report = SweepReport::default();
            report.observe(SurfaceOp::GetProfiles, true);
            report.outcomes.insert(SurfaceOp::GetPtzPresets, skipped);

            assert_eq!(report.outcome(SurfaceOp::GetPtzPresets), Some(skipped));
            assert!(skipped.is_skipped());
            assert!(
                !report.is_complete(),
                "{skipped:?} must not count as a completed sweep"
            );
            assert_eq!(report.skipped(), vec![SurfaceOp::GetPtzPresets]);
        }
    }
}
