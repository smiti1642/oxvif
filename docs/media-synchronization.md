# Media synchronization

[English](media-synchronization.md) | [繁體中文](media-synchronization_zh.md)

Available in the next release; not part of the published 0.16.0 API.

| Section | Purpose |
| --- | --- |
| [Client and session](#client-and-session) | Request a profile-associated synchronization point |
| [Mock behavior](#mock-behavior) | Explicit receipt-only testing |
| [Verification limits](#verification-limits) | What a successful response proves |

## Client and session

| Service | Client method | Session method |
| --- | --- | --- |
| Media1 | `media_set_synchronization_point(media_url, profile_token)` | `media_set_synchronization_point(profile_token)` |
| Media2 | `set_synchronization_point_media2(media_url, profile_token)` | `set_synchronization_point_media2(profile_token)` |

Choose the intended profile explicitly. These methods request synchronization of
its associated streams, including a video intra frame and applicable metadata
status refresh. They are distinct from Events `set_synchronization_point`, which
addresses a subscription rather than a media profile. Sessions do not silently
switch to a different service. Transport, SOAP and response-shape errors propagate.

```rust
// Requires an established OnvifSession and a selected profile token.
session.media_set_synchronization_point(&profile.token).await?;
// For a device exposing Media2, use the Media2 method instead:
session.set_synchronization_point_media2(&profile.token).await?;
```

## Mock behavior

The mock has no video encoder or RTP/metadata delivery pipeline. Both methods
refuse by default with `s:Receiver` / `mock:UnmodeledEffect`. For receipt-only tests,
explicitly select the exact service operation:

```rust
use oxvif::mock::{AckOnlyOperation, MockTransport};
let transport = MockTransport::new()
    .with_acknowledgment_only(AckOnlyOperation::MediaSynchronizationPoint);
// Media2SynchronizationPoint is a separate selection; Events is unaffected.
```

Opt-in still validates request identity and the unique scoped profile token.
Missing/ambiguous/mislocated selectors are rejected; an unknown profile returns
a structured fault. Refusals and receipts leave state, hooks, event queues and
recorded reads unchanged. Replay does not return recorded write acknowledgments;
it applies the same synthetic policy. Explicit fault injection remains separate.

## Verification limits

Client wire tests, HTTP/in-process mock tests and external XML validation check
requests, replies and policy, not delivered media. A SOAP acknowledgment alone
does not establish that an I-frame or metadata refresh was observed. Real-stream
acceptance needs an explicitly authorized request and observation of that stream.
See the [integration evidence](active/contributor-pr-integration-plan.md).
