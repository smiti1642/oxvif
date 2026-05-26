# WS-Discovery

> Reference for implementing oxvif — not part of the crate.

- **Spec:** WS-Discovery 1.1 (OASIS) over SOAP-over-UDP; ONVIF Core §7.
- **Multicast:** `239.255.255.250:3702` (IPv4). Namespaces: `http://docs.oasis-open.org/ws-dd/ns/discovery/2009/01`
  (prefix `d`/`wsd`), ONVIF discovery extensions `dn`.
- **oxvif status:** ◐ implemented in `src/discovery.rs` — active Probe + passive Hello/Bye listen.

Unlike the other services this is **not** a request/response WSDL operation set; it is a set of
SOAP-over-UDP message types exchanged via multicast/unicast.

---

## Message types

| Message | Direction | Purpose | oxvif | API |
|---------|-----------|---------|:----:|-----|
| Probe | client → multicast | search for devices by Types/Scopes | ✓ | `discovery::probe` / `probe_unicast` |
| ProbeMatches | device → client | response to Probe (XAddrs, Scopes, Types) | ✓ | parsed into `DiscoveredDevice` |
| Hello | device → multicast | device announces it joined the network | ✓ | `discovery::listen` (`DiscoveryEvent::Hello`) |
| Bye | device → multicast | device announces it is leaving | ✓ | `discovery::listen` (`DiscoveryEvent::Bye`) |
| Resolve | client → multicast | resolve an EndpointReference to XAddrs | — | — |
| ResolveMatches | device → client | response to Resolve | — | — |

---

## Message field reference (for the unimplemented Resolve)

- **Resolve** — header `wsa:Action` = `…/Resolve`; body `d:Resolve` → `wsa:EndpointReference`
  (`wsa:Address` `xs:anyURI` [1]). Sent to the multicast group.
- **ResolveMatches** — body `d:ResolveMatches` → `d:ResolveMatch` [0..1]:
  `wsa:EndpointReference` [1], `d:Types` [0..1], `d:Scopes` [0..1], `d:XAddrs` `xs:anyURI`-list [1],
  `d:MetadataVersion` `xs:unsignedInt` [1].

**`DiscoveredDevice`** (oxvif, `src/discovery.rs`) — `endpoint`, `types` `Vec<String>`,
`scopes` `Vec<String>`, `xaddrs` `Vec<String>`. Probe/ProbeMatches and Hello/Bye are already
modelled there; that code is the source of truth.

_Source: WS-Discovery 1.1 spec + ONVIF Core §7 (stable). Resolve fields are standard WS-Discovery._
