# Analytics Service

> Reference for implementing oxvif — not part of the crate. Mirrors the official ONVIF WSDL.

- **WSDL:** https://www.onvif.org/ver20/analytics/wsdl/analytics.wsdl
- **Namespace:** `http://www.onvif.org/ver20/analytics/wsdl` (prefix `tan`)
- **Shared schema:** `http://www.onvif.org/ver10/schema/onvif.xsd` (prefix `tt`)
- **ONVIF Profile:** T (Profile M for metadata/analytics consumers)
- **oxvif status:** ❌ not implemented (ROADMAP short-term). No `src/client/analytics.rs` yet.

Two portTypes: **AnalyticsEngine** (module CRUD) and **RuleEngine** (rule CRUD).

---

## Operations

| Operation | Port | Purpose | In → Out | oxvif | method |
|-----------|------|---------|----------|:----:|--------|
| GetServiceCapabilities | Engine | analytics service capabilities | `GetServiceCapabilities` → `…Response` | — | — |
| GetSupportedAnalyticsModules | Engine | list supported modules for a config | `GetSupportedAnalyticsModules` → `…Response` | — | — |
| GetAnalyticsModules | Engine | list assigned modules | `GetAnalyticsModules` → `…Response` | — | — |
| CreateAnalyticsModules | Engine | add modules to a config | `CreateAnalyticsModules` → `…Response` | — | — |
| ModifyAnalyticsModules | Engine | modify existing modules | `ModifyAnalyticsModules` → `…Response` | — | — |
| DeleteAnalyticsModules | Engine | remove modules from a config | `DeleteAnalyticsModules` → `…Response` | — | — |
| GetAnalyticsModuleOptions | Engine | options for supported modules | `GetAnalyticsModuleOptions` → `…Response` | — | — |
| GetSupportedMetadata | Engine | metadata a module emits | `GetSupportedMetadata` → `…Response` | — | — |
| GetSupportedRules | Rule | list supported rules for a config | `GetSupportedRules` → `…Response` | — | — |
| GetRules | Rule | list assigned rules | `GetRules` → `…Response` | — | — |
| CreateRules | Rule | add rules to a config | `CreateRules` → `…Response` | — | — |
| ModifyRules | Rule | modify existing rules | `ModifyRules` → `…Response` | — | — |
| DeleteRules | Rule | remove rules by name | `DeleteRules` → `…Response` | — | — |
| GetRuleOptions | Rule | options for supported rules | `GetRuleOptions` → `…Response` | — | — |

All operations are unimplemented; full request/response detail follows.

---

## Request / response detail

Cardinality: `[1]` required, `[0..1]` optional, `[0..*]` repeated, `[1..*]` one-or-more. `(attr)` = XML attribute.

### RuleEngine

#### GetSupportedRules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `SupportedRules` `tt:SupportedRules` [1]

#### GetRules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `Rule` `tt:Config` [0..*]

#### CreateRules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `Rule` `tt:Config` [1..*]
- **Resp:** _(empty)_

#### ModifyRules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `Rule` `tt:Config` [1..*]
- **Resp:** _(empty)_

#### DeleteRules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `RuleName` `xs:string` [1..*]
- **Resp:** _(empty)_

#### GetRuleOptions
- **Req:** `RuleType` `xs:QName` [0..1]; `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `RuleOptions` `tan:ConfigOptions` [0..*]

### AnalyticsEngine

#### GetServiceCapabilities
- **Req:** _(empty)_
- **Resp:** `Capabilities` `tan:Capabilities` [1] — attrs `RuleSupport`, `AnalyticsModuleSupport`, `CellBasedSceneDescriptionSupported`, `RuleOptionsSupported`, `AnalyticsModuleOptionsSupported`, `SupportedMetadata` (all bool/optional)

#### GetSupportedAnalyticsModules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `SupportedAnalyticsModules` `tt:SupportedAnalyticsModules` [1]

#### GetAnalyticsModules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `AnalyticsModule` `tt:Config` [0..*]

#### CreateAnalyticsModules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `AnalyticsModule` `tt:Config` [1..*]
- **Resp:** _(empty)_

#### ModifyAnalyticsModules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `AnalyticsModule` `tt:Config` [1..*]
- **Resp:** _(empty)_

#### DeleteAnalyticsModules
- **Req:** `ConfigurationToken` `tt:ReferenceToken` [1]; `AnalyticsModuleName` `xs:string` [1..*]
- **Resp:** _(empty)_

#### GetAnalyticsModuleOptions
- **Req:** `Type` `xs:QName` [0..1]; `ConfigurationToken` `tt:ReferenceToken` [1]
- **Resp:** `Options` `tan:ConfigOptions` [0..*]

#### GetSupportedMetadata
- **Req:** `Type` `xs:QName` [0..1]
- **Resp:** `AnalyticsModule` `tan:MetadataInfo` [0..*]

_Source: analytics.wsdl `<wsdl:types>` (fetched 2026-05)._

---

## Schema types

Workhorse types `tt:Config`, `tt:ItemList`, `tt:ConfigDescription`, `tt:ItemListDescription`,
`tt:ReferenceToken` are defined once in [types.md](types.md) — analytics reuses them directly.

Analytics-specific:

- **`tt:SupportedRules`** / **`tt:SupportedAnalyticsModules`** — describe what a config accepts:
  `RuleContentSchemaLocation` / `AnalyticsModuleContentSchemaLocation` `xs:anyURI` [0..*], plus
  `RuleDescription` / `AnalyticsModuleDescription` `tt:ConfigDescription` [0..*].
- **`tan:ConfigOptions`** — option descriptor for one rule/module type: attrs `RuleType`/`Name`
  `xs:QName`; child `tt:ItemListDescription`. (See analytics.wsdl `<wsdl:types>`.)
- **`tan:MetadataInfo`** — metadata a module emits; see analytics.wsdl `<wsdl:types>`.

_Source: analytics.wsdl + onvif.xsd v25.12; `*Description` detail kept as pointer — verify against onvif.xsd when implementing._
