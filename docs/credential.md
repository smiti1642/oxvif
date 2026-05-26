# Credential Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/credential/wsdl/credential.wsdl
- **Namespace:** `http://www.onvif.org/ver10/credential/wsdl` (prefix `tcr`); PACS common types `pt`.
- **ONVIF Profile:** C
- **oxvif status:** ❌ not implemented. No `src/client/credential.rs`.

Manages **credentials** (cards, PINs, biometrics) and their identifiers, access-profile bindings,
and white/black lists. CRUD uses the PACS list/info/create/set/modify/delete convention with
pagination.

---

## Operations

### Credential CRUD & state
| Operation | Req → Resp |
|-----------|------------|
| GetServiceCapabilities | _empty_ → `Capabilities` `tcr:ServiceCapabilities` [1] |
| GetSupportedFormatTypes | `CredentialIdentifierTypeName` `xs:string` [1] → `FormatTypeInfo` `tcr:CredentialIdentifierFormatTypeInfo` [1..*] |
| GetCredentialInfo | `Token` `pt:ReferenceToken` [1..*] → `CredentialInfo` `tcr:CredentialInfo` [0..*] |
| GetCredentialInfoList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `CredentialInfo` [0..*] |
| GetCredentials | `Token` `pt:ReferenceToken` [1..*] → `Credential` `tcr:Credential` [0..*] |
| GetCredentialList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `Credential` [0..*] |
| CreateCredential | `Credential` `tcr:Credential` [1], `State` `tcr:CredentialState` [1] → `Token` `pt:ReferenceToken` [1] |
| ModifyCredential | `Credential` `tcr:Credential` [1] → _empty_ |
| SetCredential | `CredentialData` `tcr:CredentialData` [1] → _empty_ |
| DeleteCredential | `Token` `pt:ReferenceToken` [1] → _empty_ |
| GetCredentialState | `Token` `pt:ReferenceToken` [1] → `State` `tcr:CredentialState` [1] |
| EnableCredential | `Token` [1], `Reason` `pt:Name` [0..1] → _empty_ |
| DisableCredential | `Token` [1], `Reason` `pt:Name` [0..1] → _empty_ |
| ResetAntipassbackViolation | `CredentialToken` `pt:ReferenceToken` [1] → _empty_ |

### Identifiers & access profiles
| Operation | Req → Resp |
|-----------|------------|
| GetCredentialIdentifiers | `CredentialToken` [1] → `CredentialIdentifier` `tcr:CredentialIdentifier` [0..*] |
| SetCredentialIdentifier | `CredentialToken` [1], `CredentialIdentifier` `tcr:CredentialIdentifier` [1] → _empty_ |
| DeleteCredentialIdentifier | `CredentialToken` [1], `CredentialIdentifierTypeName` `pt:Name` [1] → _empty_ |
| GetCredentialAccessProfiles | `CredentialToken` [1] → `CredentialAccessProfile` `tcr:CredentialAccessProfile` [0..*] |
| SetCredentialAccessProfiles | `CredentialToken` [1], `CredentialAccessProfile` [1..*] → _empty_ |
| DeleteCredentialAccessProfiles | `CredentialToken` [1], `AccessProfileToken` `pt:ReferenceToken` [1..*] → _empty_ |

### White / black lists
| Operation | Req → Resp |
|-----------|------------|
| GetWhitelist / GetBlacklist | `Limit` [0..1], `StartReference` [0..1], `IdentifierType` `xs:string` [0..1], `FormatType` `xs:string` [0..1], `Value` `xs:hexBinary` [0..1] → `NextStartReference` [0..1], `Identifier` `tcr:CredentialIdentifierItem` [0..*] |
| AddToWhitelist / AddToBlacklist | `Identifier` `tcr:CredentialIdentifierItem` [1..*] → _empty_ |
| RemoveFromWhitelist / RemoveFromBlacklist | `Identifier` `tcr:CredentialIdentifierItem` [1..*] → _empty_ |
| DeleteWhitelist / DeleteBlacklist | _empty_ → _empty_ |

Complex types (`tcr:Credential`, `tcr:CredentialData`, `tcr:CredentialInfo`, `tcr:CredentialState`,
`tcr:CredentialIdentifier`, `tcr:CredentialAccessProfile`, `tcr:CredentialIdentifierItem`):
see credential.wsdl / `types.xsd`.

_Source: credential.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
