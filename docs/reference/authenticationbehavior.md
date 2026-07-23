# Authentication Behavior Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/authenticationbehavior/wsdl/authenticationbehavior.wsdl
- **Namespace:** `http://www.onvif.org/ver10/authenticationbehavior/wsdl` (prefix `tab`); PACS common types `pt`.
- **ONVIF Profile:** A
- **oxvif status:** ❌ not implemented. No `src/client/authenticationbehavior.rs`.

Manages **authentication profiles** (which security level applies on which schedule at an access
point) and **security levels** (the set of recognition methods required, e.g. card-then-PIN).
Two entity families, each with the standard PACS list/info/create/set/modify/delete convention.

---

## Operations

### Authentication profiles
| Operation | Req → Resp |
|-----------|------------|
| GetServiceCapabilities | _empty_ → `Capabilities` `tab:ServiceCapabilities` [1] |
| GetAuthenticationProfileInfo | `Token` `pt:ReferenceToken` [1..*] → `AuthenticationProfileInfo` `tab:AuthenticationProfileInfo` [0..*] |
| GetAuthenticationProfileInfoList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `AuthenticationProfileInfo` [0..*] |
| GetAuthenticationProfiles | `Token` `pt:ReferenceToken` [1..*] → `AuthenticationProfile` `tab:AuthenticationProfile` [0..*] |
| GetAuthenticationProfileList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `AuthenticationProfile` [0..*] |
| CreateAuthenticationProfile | `AuthenticationProfile` `tab:AuthenticationProfile` [1] → `Token` `pt:ReferenceToken` [1] |
| SetAuthenticationProfile | `AuthenticationProfile` `tab:AuthenticationProfile` [1] → _empty_ |
| ModifyAuthenticationProfile | `AuthenticationProfile` `tab:AuthenticationProfile` [1] → _empty_ |
| DeleteAuthenticationProfile | `Token` `pt:ReferenceToken` [1] → _empty_ |

### Security levels
| Operation | Req → Resp |
|-----------|------------|
| GetSecurityLevelInfo | `Token` `pt:ReferenceToken` [1..*] → `SecurityLevelInfo` `tab:SecurityLevelInfo` [0..*] |
| GetSecurityLevelInfoList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `SecurityLevelInfo` [0..*] |
| GetSecurityLevels | `Token` `pt:ReferenceToken` [1..*] → `SecurityLevel` `tab:SecurityLevel` [0..*] |
| GetSecurityLevelList | `Limit` [0..1], `StartReference` [0..1] → `NextStartReference` [0..1], `SecurityLevel` [0..*] |
| CreateSecurityLevel | `SecurityLevel` `tab:SecurityLevel` [1] → `Token` `pt:ReferenceToken` [1] |
| SetSecurityLevel | `SecurityLevel` `tab:SecurityLevel` [1] → _empty_ |
| ModifySecurityLevel | `SecurityLevel` `tab:SecurityLevel` [1] → _empty_ |
| DeleteSecurityLevel | `Token` `pt:ReferenceToken` [1] → _empty_ |

Complex types (`tab:AuthenticationProfile`, `tab:AuthenticationProfileInfo`, `tab:SecurityLevel`,
`tab:SecurityLevelInfo`, `tab:ServiceCapabilities`): see authenticationbehavior.wsdl / `types.xsd`.

_Source: authenticationbehavior.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
