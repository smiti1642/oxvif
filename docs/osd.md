# OSD (On-Screen Display)

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).
> OSD is not a separate service — the operations live in the Media1 (`trt`) and Media2 (`tr2`)
> WSDLs. oxvif implements OSD via **Media1**.

- **WSDL:** https://www.onvif.org/ver10/media/wsdl/media.wsdl (also `ver20/media/wsdl`)
- **ONVIF Profile:** T
- **oxvif status:** ✅ fully implemented in `src/client/media.rs` (Media1 variants).

---

## Operations

| Operation | Purpose | oxvif (Media1) | method |
|-----------|---------|:----:|--------|
| GetOSDs | list OSD elements | ✓ | `get_osds` |
| GetOSD | single OSD by token | ✓ | `get_osd` |
| CreateOSD | create OSD, returns token | ✓ | `create_osd` |
| SetOSD | update OSD | ✓ | `set_osd` |
| DeleteOSD | delete OSD | ✓ | `delete_osd` |
| GetOSDOptions | valid OSD types/positions | ✓ | `get_osd_options` |

The Media2 (`tr2`) WSDL defines the same six operations; oxvif does not call them separately
(Media1 coverage is sufficient). If a device only exposes OSD via Media2, these would need
`tr2`-namespaced variants.

---

## Notes for implementers

- `OnvifSession::get_osd_options` (`src/session.rs`) enriches the spec-strict
  `OnvifClient::get_osd_options` result with **vendor extensions**: per-text-type quotas stashed as
  attributes on `<MaximumNumberOfOSDs>` (Genetec / late-Hikvision) and the flat `<PositionOption>`
  shape (some Dahua). Keep `OnvifClient` spec-pure; do enrichment at the session layer.
- Field structures (`tt:OSDConfiguration`, `tt:OSDTextConfiguration`, `tt:OSDColor`,
  `tt:OSDConfigurationOptions`) are already modelled in `src/types/osd.rs` — that code is the
  source of truth, so no field expansion is duplicated here.

_Source: media.wsdl operation list (fetched 2026-05); oxvif types in `src/types/osd.rs`._
