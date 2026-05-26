# Search Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/search.wsdl
- **Namespace:** `http://www.onvif.org/ver10/search/wsdl` (prefix `tse`)
- **ONVIF Profile:** G
- **oxvif status:** ◐ implemented in `src/client/recording.rs` (3 of 14 operations)

oxvif covers the recording-search session (find → poll → end). Unimplemented: summary/info/media
attributes and the **event / PTZ-position / metadata** search sessions (ROADMAP medium-term:
`FindEvents` + `GetEventSearchResults`, `FindPTZPosition` + `GetPTZPositionSearchResults`).

---

## Operations

| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| FindRecordings | start recording search | ✓ | `find_recordings` |
| GetRecordingSearchResults | poll recording results | ✓ | `get_recording_search_results` |
| EndSearch | end a search session | ✓ | `end_search` |
| GetServiceCapabilities | search service capabilities | — | — |
| GetRecordingSummary | overview of all recorded data | — | — |
| GetRecordingInformation | details of one recording | — | — |
| GetMediaAttributes | track attributes at a time | — | — |
| FindEvents | start event search | — | — |
| GetEventSearchResults | poll event results | — | — |
| FindPTZPosition | start PTZ-position search | — | — |
| GetPTZPositionSearchResults | poll PTZ-position results | — | — |
| FindMetadata | start metadata search | — | — |
| GetMetadataSearchResults | poll metadata results | — | — |
| GetSearchState | search session status (deprecated) | — | — |

(`search_recordings` is an oxvif convenience helper wrapping find → poll → end.)

---

## Request / response patterns (unimplemented)

All search sessions follow the same two-step shape (mirrors the implemented `FindRecordings` flow):

- **`Find<X>`** — Req: `Scope` `tt:SearchScope` [1] (+ `<X>Filter` [0..1]); `MaxMatches` `xs:int` [0..1];
  `KeepAliveTime` `xs:duration` [1]. Resp: `SearchToken` `tt:JobToken` [1].
- **`Get<X>SearchResults`** — Req: `SearchToken` `tt:JobToken` [1]; `MinResults` `xs:int` [0..1];
  `MaxResults` `xs:int` [0..1]; `WaitTime` `xs:duration` [0..1].
  Resp: `ResultList` `tt:FindStateInformation` + the per-type result array (see below).

Per-operation specifics:

- **GetRecordingSummary** — Req: _(empty)_; Resp: `Summary` `tt:RecordingSummary` [1]
  (DataFrom/DataUntil dateTime, NumberRecordings int).
- **GetRecordingInformation** — Req: `RecordingToken` `tt:ReferenceToken` [1];
  Resp: `RecordingInformation` `tt:RecordingInformation` [1].
- **GetMediaAttributes** — Req: `RecordingTokens` `tt:ReferenceToken` [0..*]; `Time` `xs:dateTime` [1];
  Resp: `MediaAttributes` `tt:MediaAttributes` [0..*].
- **FindEvents** — extra Req fields: `StartPoint` `xs:dateTime` [1]; `EndPoint` `xs:dateTime` [0..1];
  `SearchFilter` `tt:SearchFilter` [0..1]; `IncludeStartState` `xs:boolean` [1].
  `GetEventSearchResults` Resp `ResultList`: `Result` `tt:FindEventResult` [0..*].
- **FindPTZPosition** — extra Req: `StartPoint`/`EndPoint` dateTime; `SearchFilter` `tt:PTZPositionFilter`.
  Resp `Result` `tt:FindPTZPositionResult` [0..*].
- **FindMetadata** — extra Req: `MetadataFilter` `tt:MetadataFilter`.
  Resp `Result` `tt:FindMetadataResult` [0..*].
- **GetSearchState** — Req: `SearchToken` `tt:JobToken` [1]; Resp: `State` `tt:SearchState` [1] (deprecated).

Complex types (`tt:SearchScope`, `tt:RecordingSummary`, `tt:FindEventResult`,
`tt:FindPTZPositionResult`, `tt:SearchFilter`): see search.wsdl / onvif.xsd.

_Source: search.wsdl operation list (fetched 2026-05); session field shapes are standard ONVIF —
verify against search.wsdl when implementing._
