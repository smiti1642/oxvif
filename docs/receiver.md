# Receiver Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/receiver.wsdl
- **Namespace:** `http://www.onvif.org/ver10/receiver/wsdl` (prefix `trv`)
- **ONVIF Profile:** — (NVR-side; pairs with Recording)
- **oxvif status:** ❌ not implemented (ROADMAP medium-term). No `src/client/receiver.rs`.

A Receiver is an RTSP **input** sink on an NVR/device that pulls a stream from a remote source —
the producer side of a recording job. Small, regular CRUD service.

---

## Operations & detail

| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetServiceCapabilities | service capabilities | _empty_ → `Capabilities` `trv:Capabilities` [1] |
| GetReceivers | list all receivers | _empty_ → `Receivers` `tt:Receiver` [0..*] |
| GetReceiver | one receiver | `ReceiverToken` `tt:ReferenceToken` [1] → `Receiver` `tt:Receiver` [1] |
| CreateReceiver | create a receiver | `Configuration` `tt:ReceiverConfiguration` [1] → `Receiver` `tt:Receiver` [1] |
| DeleteReceiver | delete an idle receiver | `ReceiverToken` `tt:ReferenceToken` [1] → _empty_ |
| ConfigureReceiver | reconfigure a receiver | `ReceiverToken` [1], `Configuration` `tt:ReceiverConfiguration` [1] → _empty_ |
| SetReceiverMode | set mode without other changes | `ReceiverToken` [1], `Mode` `tt:ReceiverMode` [1] (`AutoConnect|AlwaysConnect|NeverConnect`) → _empty_ |
| GetReceiverState | connection state | `ReceiverToken` [1] → `ReceiverState` `tt:ReceiverStateInformation` [1] |

**`tt:ReceiverConfiguration`** — `Mode` `tt:ReceiverMode` [1]; `MediaUri` `xs:anyURI` [1];
`StreamSetup` `tt:StreamSetup` [1].
**`tt:Receiver`** — attr `Token` `tt:ReferenceToken`; child `Configuration` `tt:ReceiverConfiguration` [1].
**`tt:ReceiverStateInformation`** — `State` `tt:ReceiverState` (`NotConnected|Connecting|Connected`) [1];
`AutoCreated` `xs:boolean` [1].

_Source: receiver.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
