# Access Rules Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/accessrules/wsdl/accessrules.wsdl
- **Namespace:** `http://www.onvif.org/ver10/accessrules/wsdl` (prefix `tar`); PACS common types `pt`.
- **ONVIF Profile:** A
- **oxvif status:** ❌ not implemented. No `src/client/accessrules.rs`.

Manages **access profiles** — the rules binding credentials to access points/areas on schedules.
Single-entity CRUD with the standard PACS list/info/create/set/modify/delete convention.

---

## Operations

| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetServiceCapabilities | service capabilities | _empty_ → `Capabilities` `tar:ServiceCapabilities` [1] |
| GetAccessProfileInfo | info by tokens | `Token` `pt:ReferenceToken` [1..*] → `AccessProfileInfo` `tar:AccessProfileInfo` [0..*] |
| GetAccessProfileInfoList | paginated info list | `Limit` `xs:int` [0..1], `StartReference` `xs:string` [0..1] → `NextStartReference` [0..1], `AccessProfileInfo` [0..*] |
| GetAccessProfiles | full by tokens | `Token` `pt:ReferenceToken` [1..*] → `AccessProfile` `tar:AccessProfile` [0..*] |
| GetAccessProfileList | paginated full list | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `AccessProfile` [0..*] |
| CreateAccessProfile | create (device token) | `AccessProfile` `tar:AccessProfile` [1] → `Token` `pt:ReferenceToken` [1] |
| ModifyAccessProfile | modify | `AccessProfile` `tar:AccessProfile` [1] → _empty_ |
| SetAccessProfile | create/replace (client token) | `AccessProfile` `tar:AccessProfile` [1] → _empty_ |
| DeleteAccessProfile | delete | `Token` `pt:ReferenceToken` [1] → _empty_ |

**`tar:AccessProfile`** — attr `token` `pt:ReferenceToken`; `Name` `pt:Name` [1];
`Description` `pt:Description` [0..1]; `AccessPolicy` `tar:AccessPolicy` [0..*]
(each: `ScheduleToken` `pt:ReferenceToken`, `Entity`/`EntityType`, `AccessPointToken`).
Full structure: see accessrules.wsdl / `types.xsd`.

_Source: accessrules.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
