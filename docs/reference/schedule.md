# Schedule Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/schedule/wsdl/schedule.wsdl
- **Namespace:** `http://www.onvif.org/ver10/schedule/wsdl` (prefix `tsc`); PACS common types `pt`.
- **ONVIF Profile:** A
- **oxvif status:** ❌ not implemented. No `src/client/schedule.rs`.

Manages **schedules** (recurring time windows, iCalendar-based) and **special day groups**
(holidays/exceptions). Two parallel entity families, each with the standard PACS
list/info/create/set/modify/delete convention; schedules also expose a state query.

---

## Operations

### Schedules
| Operation | Req → Resp |
|-----------|------------|
| GetServiceCapabilities | _empty_ → `Capabilities` `tsc:ServiceCapabilities` [1] |
| GetScheduleState | `Token` `pt:ReferenceToken` [1] → `ScheduleState` `tsc:ScheduleState` [1] |
| GetScheduleInfo | `Token` `pt:ReferenceToken` [1..*] → `ScheduleInfo` `tsc:ScheduleInfo` [0..*] |
| GetScheduleInfoList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `ScheduleInfo` [0..*] |
| GetSchedules | `Token` `pt:ReferenceToken` [1..*] → `Schedule` `tsc:Schedule` [0..*] |
| GetScheduleList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `Schedule` [0..*] |
| CreateSchedule | `Schedule` `tsc:Schedule` [1] → `Token` `pt:ReferenceToken` [1] |
| SetSchedule | `Schedule` `tsc:Schedule` [1] → _empty_ |
| ModifySchedule | `Schedule` `tsc:Schedule` [1] → _empty_ |
| DeleteSchedule | `Token` `pt:ReferenceToken` [1] → _empty_ |

### Special day groups
| Operation | Req → Resp |
|-----------|------------|
| GetSpecialDayGroupInfo | `Token` `pt:ReferenceToken` [1..*] → `SpecialDayGroupInfo` `tsc:SpecialDayGroupInfo` [0..*] |
| GetSpecialDayGroupInfoList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `SpecialDayGroupInfo` [0..*] |
| GetSpecialDayGroups | `Token` `pt:ReferenceToken` [1..*] → `SpecialDayGroup` `tsc:SpecialDayGroup` [0..*] |
| GetSpecialDayGroupList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `SpecialDayGroup` [0..*] |
| CreateSpecialDayGroup | `SpecialDayGroup` `tsc:SpecialDayGroup` [1] → `Token` `pt:ReferenceToken` [1] |
| SetSpecialDayGroup | `SpecialDayGroup` `tsc:SpecialDayGroup` [1] → _empty_ |
| ModifySpecialDayGroup | `SpecialDayGroup` `tsc:SpecialDayGroup` [1] → _empty_ |
| DeleteSpecialDayGroup | `Token` `pt:ReferenceToken` [1] → _empty_ |

Complex types (`tsc:Schedule` — holds iCalendar `Standard`/`SpecialDays`, `tsc:ScheduleState`,
`tsc:ScheduleInfo`, `tsc:SpecialDayGroup`): see schedule.wsdl / `types.xsd`.

_Source: schedule.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
