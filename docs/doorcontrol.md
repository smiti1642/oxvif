# Door Control Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/pacs/doorcontrol.wsdl
- **Namespace:** `http://www.onvif.org/ver10/doorcontrol/wsdl` (prefix `tdc`); PACS common types `pt`.
- **ONVIF Profile:** C
- **oxvif status:** ❌ not implemented. No `src/client/doorcontrol.rs`.

Models physical **doors** and their lock/access actions. CRUD follows the PACS
list/info/create/set/modify/delete convention; the rest are momentary/latched lock commands that
all take just a door `Token` and return empty.

---

## Operations

### CRUD & state
| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetServiceCapabilities | service capabilities | _empty_ → `Capabilities` `tdc:ServiceCapabilities` [1] |
| GetDoorInfoList | paginated info list | `Limit` `xs:int` [0..1], `StartReference` `xs:string` [0..1] → `NextStartReference` [0..1], `DoorInfo` `tdc:DoorInfo` [0..*] |
| GetDoorInfo | info by tokens | `Token` `pt:ReferenceToken` [1..*] → `DoorInfo` `tdc:DoorInfo` [0..*] |
| GetDoorList | paginated full list | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `Door` `tdc:Door` [0..*] |
| GetDoors | by tokens | `Token` `pt:ReferenceToken` [1..*] → `Door` `tdc:Door` [0..*] |
| CreateDoor | create (device token) | `Door` `tdc:Door` [1] → `Token` `pt:ReferenceToken` [1] |
| SetDoor | create/replace (client token) | `Door` `tdc:Door` [1] → _empty_ |
| ModifyDoor | modify | `Door` `tdc:Door` [1] → _empty_ |
| DeleteDoor | delete | `Token` `pt:ReferenceToken` [1] → _empty_ |
| GetDoorState | current door state | `Token` `pt:ReferenceToken` [1] → `DoorState` `tdc:DoorState` [1] |

### Lock / access commands (all `Token` `pt:ReferenceToken` [1] → _empty_, unless noted)
| Operation | Purpose |
|-----------|---------|
| AccessDoor | momentary access; extra req `UseExtendedTime` `xs:boolean` [0..1], `AccessTime` `xs:duration` [0..1], `OpenTooLongTime` `xs:duration` [0..1], `PreAlarmTime` `xs:duration` [0..1], `Extension` `tdc:AccessDoorExtension` [0..1] |
| LockDoor | lock |
| UnlockDoor | unlock |
| BlockDoor | block momentary access |
| LockDownDoor | lock down (latched until release) |
| LockDownReleaseDoor | release lock-down |
| LockOpenDoor | hold open (latched until release) |
| LockOpenReleaseDoor | release lock-open |
| DoubleLockDoor | engage all locks |

Complex types (`tdc:DoorInfo`, `tdc:Door`, `tdc:DoorState`, `tdc:ServiceCapabilities`,
`tdc:AccessDoorExtension`): see doorcontrol.wsdl / `types.xsd`.

_Source: doorcontrol.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
