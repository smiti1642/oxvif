# Recording Control Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/recording.wsdl
- **Namespace:** `http://www.onvif.org/ver10/recording/wsdl` (prefix `trc`)
- **ONVIF Profile:** G
- **oxvif status:** ◐ implemented in `src/client/recording.rs` (11 of 22 operations)

oxvif covers the create/delete lifecycle for recordings, tracks, and jobs, plus job mode/state.
Unimplemented: the **configuration getters/setters** (recording/track/job), **options**, and the
**export** family (`SetRecordingConfiguration`, `SetTrackConfiguration`,
`GetRecordingOptions`).

> **Real-camera shape (verified GeoVision / Hanwha).** `GetRecordingsResponse`
> contains repeated **`RecordingItem`** (singular — not `RecordingItems`); each
> track carries a `TrackToken` **child element** (not a `@token` attribute) and
> its `TrackType`/`Description` live under the track's `Configuration`. The
> recording / search / replay service URLs are advertised by some cameras only
> via `GetServices`, not the `GetCapabilities` extension; `OnvifSession` falls
> back to `GetServices` so these work regardless.

---

## Operations

| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetRecordings | list recordings + tracks | ✓ | `get_recordings` |
| CreateRecording | create recording | ✓ | `create_recording` |
| DeleteRecording | delete recording | ✓ | `delete_recording` |
| CreateTrack | add track | ✓ | `create_track` |
| DeleteTrack | remove track | ✓ | `delete_track` |
| GetRecordingJobs | list jobs | ✓ | `get_recording_jobs` |
| CreateRecordingJob | create job | ✓ | `create_recording_job` |
| DeleteRecordingJob | delete job | ✓ | `delete_recording_job` |
| SetRecordingJobMode | set job Active/Idle | ✓ | `set_recording_job_mode` |
| GetRecordingJobState | job state | ✓ | `get_recording_job_state` |
| GetServiceCapabilities | recording service capabilities | ✓ | `recording_get_service_capabilities` |
| GetRecordingConfiguration | read recording config | — | — |
| SetRecordingConfiguration | modify recording config | — | — |
| GetRecordingOptions | recording capacity/options | — | — |
| GetTrackConfiguration | read track config | — | — |
| SetTrackConfiguration | modify track config | — | — |
| GetRecordingJobConfiguration | read job config | — | — |
| SetRecordingJobConfiguration | modify job config | — | — |
| ExportRecordedData | export recordings to storage | — | — |
| StopExportRecordedData | stop an export | — | — |
| GetExportRecordedDataState | export progress | — | — |
| OverrideSegmentDuration | adjust segment timing | — | — |

---

## Request / response detail (unimplemented)

Standard `trc`/`tt` shapes — verify config types against recording.wsdl when implementing.

#### GetServiceCapabilities
- **Req:** _(empty)_ · **Resp:** `Capabilities` `trc:Capabilities` [1]

`trc:Capabilities` — all members are **attributes**, all optional. This is the
complete set; it is much larger than the "…" in an earlier revision of this file
suggested, and it is the widest of the nine service-capability types:

| Attribute | Type | Meaning |
|-----------|------|---------|
| `DynamicRecordings` | `xs:boolean` | recordings can be created/deleted at runtime |
| `DynamicTracks` | `xs:boolean` | tracks can be created/deleted at runtime |
| `Encoding` | `tt:StringList` | supported encodings (from `tt:VideoEncoding`/`tt:AudioEncoding`) |
| `MaxRate` | `xs:float` | max rate for a single recording, kbit/s |
| `MaxTotalRate` | `xs:float` | max aggregate rate, kbit/s |
| `MaxRecordings` | `xs:float` | max number of recordings |
| `MaxRecordingJobs` | `xs:int` | max number of recording jobs |
| `Options` | `xs:boolean` | `GetRecordingOptions` / `GetTrackOptions` supported |
| `MetadataRecording` | `xs:boolean` | metadata tracks can be recorded |
| `SupportedExportFileFormats` | `tt:StringAttrList` | export file formats |
| `EventRecording` | `xs:boolean` | event-triggered recording supported |
| `BeforeEventLimit` | `xs:duration` | max pre-event recording time |
| `AfterEventLimit` | `xs:duration` | max post-event recording time |
| `SupportedTargetFormats` | `tt:StringAttrList` | supported export target formats |
| `EncryptionEntryLimit` | `xs:int` | max encryption entries per recording |
| `SupportedEncryptionModes` | `tt:StringAttrList` | supported encryption modes |
| `OverrideSegmentDuration` | `xs:boolean` | export segment duration can be overridden |
| `AsymmetricEncryptionSupported` | `xs:boolean` | asymmetric encryption supported |
| `ScheduledRecording` | `xs:boolean` | scheduled recording supported |
| `OnboardStorage` | `xs:boolean` | onboard storage present — **schema default `true`** |
| `SegmentExport` | `xs:boolean` | segment export supported |

> `OnboardStorage` is the only attribute across all nine service-capability
> types with a schema `default`, and it defaults to **`true`**. A parser that
> maps "absent" to `None`/`false` disagrees with the schema here. Decide
> deliberately which one oxvif reports and say so in the field's doc comment.

#### GetRecordingConfiguration / SetRecordingConfiguration
- **Get Req:** `RecordingToken` `tt:ReferenceToken` [1] · **Resp:** `RecordingConfiguration` `tt:RecordingConfiguration` [1]
- **Set Req:** `RecordingToken` `tt:ReferenceToken` [1]; `RecordingConfiguration` `tt:RecordingConfiguration` [1] · **Resp:** _(empty)_

#### GetRecordingOptions
- **Req:** `RecordingToken` `tt:ReferenceToken` [1] · **Resp:** `Options` `trc:RecordingOptions` [1]
  (Job options: spare jobs / compatible sources; Track options: spare tracks per type).

#### GetTrackConfiguration / SetTrackConfiguration
- **Get Req:** `RecordingToken` [1]; `TrackToken` `tt:ReferenceToken` [1] · **Resp:** `TrackConfiguration` `tt:TrackConfiguration` [1]
- **Set Req:** `RecordingToken` [1]; `TrackToken` [1]; `TrackConfiguration` `tt:TrackConfiguration` [1] · **Resp:** _(empty)_

#### GetRecordingJobConfiguration / SetRecordingJobConfiguration
- **Get Req:** `JobToken` `tt:ReferenceToken` [1] · **Resp:** `JobConfiguration` `tt:RecordingJobConfiguration` [1]
- **Set Req:** `JobToken` [1]; `JobConfiguration` `tt:RecordingJobConfiguration` [1] · **Resp:** `JobConfiguration` `tt:RecordingJobConfiguration` [1]

#### Export family
- **ExportRecordedData** — Req: `SearchScope` `tt:SearchScope` [1]? + `FileFormat` `xs:string` [1] +
  `StorageDestination` `tt:StorageReferencePath` [1]; Resp: `OperationToken` `tt:ReferenceToken` [1],
  `FileNames` `xs:string` [0..*]. _(verify shape in recording.wsdl)_
- **StopExportRecordedData** — Req: `OperationToken` `tt:ReferenceToken` [1]; Resp: _(empty)_.
- **GetExportRecordedDataState** — Req: `OperationToken` [1]; Resp: progress/state.
- **OverrideSegmentDuration** — Req: `RecordingToken` [1] + segment params; Resp: _(empty)_.

Complex types (`tt:RecordingConfiguration`, `tt:TrackConfiguration`, `tt:RecordingJobConfiguration`,
`trc:RecordingOptions`): see recording.wsdl. oxvif already models several in `src/types/recording.rs`.

_Source: recording.wsdl operation list (fetched 2026-05); config shapes are standard ONVIF —
verify against recording.wsdl, especially the export family._
