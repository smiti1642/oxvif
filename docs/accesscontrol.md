# Access Control Service (PACS)

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/pacs/accesscontrol.wsdl
- **Namespace:** `http://www.onvif.org/ver10/accesscontrol/wsdl` (prefix `tac`); common PACS types
  use `pt` (`http://www.onvif.org/ver10/pacs`).
- **ONVIF Profile:** C (physical access control), D (configuration)
- **oxvif status:** ❌ not implemented. No `src/client/accesscontrol.rs`.

Manages **access points** (a controlled passage direction, e.g. a door reader) and **areas**.
Most entities follow the PACS list/info/CRUD convention with pagination
(`Limit` + `StartReference` → `NextStartReference`).

---

## Operations

### Access points
| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetServiceCapabilities | service capabilities | _empty_ → `Capabilities` `tac:ServiceCapabilities` [1] |
| GetAccessPointInfoList | paginated info list | `Limit` `xs:int` [0..1], `StartReference` `xs:string` [0..1] → `NextStartReference` `xs:string` [0..1], `AccessPointInfo` `tac:AccessPointInfo` [0..*] |
| GetAccessPointInfo | info by tokens | `Token` `pt:ReferenceToken` [1..*] → `AccessPointInfo` `tac:AccessPointInfo` [0..*] |
| GetAccessPointList | paginated full list | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `AccessPoint` `tac:AccessPoint` [0..*] |
| GetAccessPoints | by tokens | `Token` `pt:ReferenceToken` [1..*] → `AccessPoint` `tac:AccessPoint` [0..*] |
| CreateAccessPoint | create (device token) | `AccessPoint` `tac:AccessPoint` [1] → `Token` `pt:ReferenceToken` [1] |
| SetAccessPoint | create/replace (client token) | `AccessPoint` `tac:AccessPoint` [1] → _empty_ |
| ModifyAccessPoint | modify | `AccessPoint` `tac:AccessPoint` [1] → _empty_ |
| DeleteAccessPoint | delete | `Token` `pt:ReferenceToken` [1] → _empty_ |
| SetAccessPointAuthenticationProfile | assign auth profile | `Token` [1], `AuthenticationProfileToken` `pt:ReferenceToken` [1] → _empty_ |
| DeleteAccessPointAuthenticationProfile | clear auth profile | `Token` [1] → _empty_ |

### Areas
| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetAreaInfoList | paginated info list | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `AreaInfo` `tac:AreaInfo` [0..*] |
| GetAreaInfo | info by tokens | `Token` `pt:ReferenceToken` [1..*] → `AreaInfo` `tac:AreaInfo` [0..*] |
| GetAreaList | paginated full list | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `Area` `tac:Area` [0..*] |
| GetAreas | by tokens | `Token` `pt:ReferenceToken` [1..*] → `Area` `tac:Area` [0..*] |
| CreateArea | create (device token) | `Area` `tac:Area` [1] → `Token` `pt:ReferenceToken` [1] |
| SetArea | create/replace | `Area` `tac:Area` [1] → _empty_ |
| ModifyArea | modify | `Area` `tac:Area` [1] → _empty_ |
| DeleteArea | delete | `Token` `pt:ReferenceToken` [1] → _empty_ |

### State & control
| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetAccessPointState | enabled/disabled state | `Token` `pt:ReferenceToken` [1] → `AccessPointState` `tac:AccessPointState` [1] |
| EnableAccessPoint | enable | `Token` `pt:ReferenceToken` [1] → _empty_ |
| DisableAccessPoint | disable | `Token` `pt:ReferenceToken` [1] → _empty_ |
| ExternalAuthorization | external grant/deny | `AccessPointToken` `pt:ReferenceToken` [1], `CredentialToken` `pt:ReferenceToken` [0..1], `Reason` `xs:string` [0..1], `Decision` `tac:Decision` (`Granted|Denied`) [1] → _empty_ |
| Feedback | reader feedback/indication | `AccessPointToken` [1], `FeedbackType` `xs:string` [1], `RecognitionType` `xs:string` [0..*], `TextMessage` `xs:string` [0..1] → _empty_ |

Complex types (`tac:AccessPointInfo`, `tac:AccessPoint`, `tac:AreaInfo`, `tac:Area`,
`tac:AccessPointState`, `tac:ServiceCapabilities`): see accesscontrol.wsdl / `types.xsd`.

_Source: accesscontrol.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
