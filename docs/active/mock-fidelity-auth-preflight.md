# Mock authentication preflight

[English](mock-fidelity-auth-preflight.md) | [繁體中文](mock-fidelity-auth-preflight_zh.md)

W08/W02/W06, 2026-09-11, baseline `9f390d1`. Design recorded before implementation.
This is a bounded authentication-parser migration, not production security acceptance.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | Reader, state and public compatibility |
| [Policy](#policy) | Supported token projection and exclusions |
| [Acceptance](#acceptance) | Load-bearing regression and downstream checks |

## Scope

`auth::validate_ws_security` currently searches the entire raw XML by four local
names. It can accept credentials in Body/foreign elements and trim identities.
Migrate onto the bounded request tree without changing auth-off defaults, exact
GetSystemDateAndTime exemption, fault/auth/raw/replay order, or public error shape.
Normal missing-credential diagnostics remain stable; structural/unsupported-token
failures are explicit and must never echo username, digest, nonce or raw XML.
Device-user writers, role authorization and shared credential-update transactions
are separate W13/W18 work.

## Policy

Read one qualified SOAP 1.2 Header/Security/UsernameToken path with unique direct
scalar Username/Password/Nonce/Created fields. Decode once and preserve identity
and timestamp text. Require explicit PasswordDigest Type; absent Type is not a
digest declaration. Permit absent or supported base64 Nonce EncodingType. Refuse
duplicate/nested credentials and unsupported SOAP recipient roles. Preserve exact
username lookup; do not normalize persisted users. No external entity resolution.
Base64 lexical whitespace may be normalized for decoding, not username/Created.

This mock still does not implement timestamp freshness, nonce replay caches,
PasswordText, HTTP Digest, full WS-Security header processing or user-level
authorization. Do not advertise those guarantees or alter policy defaults to
simulate them. Retain Sender/wsse:FailedAuthentication and CLI classification;
private diagnostic reasons may become more specific without reflecting input.
Reference review: OASIS UsernameToken 1.1.1 §3.1 and ONVIF Core v24.12 §§5.9.4–5.9.5;
normative notes remain external, not packaged schema-derived fixtures.

## Acceptance

Both transports: real client positive with distinct users, decoded special/space
identity; raw prefix/CDATA/header controls; Body/foreign/nested/duplicate credentials,
wrong/absent Type, bad EncodingType/base64, malformed/bounded input and mismatched
digest negatives. Assert exact fault payload, unchanged full state/hooks and no
credential markers in response. Verify auth-off and exact exemption controls,
fault-before-auth ordering, live user-table lookup and CLI human/Agent contracts.
Replace touched hollow unit assertions with exact reasons. Reproduce before fixes,
run unfiltered all-feature/no-fail-fast mutation, restore, run all five gates,
strict docs and paired inventory. Do not claim external auth-instance acceptance
from the existing 40-profile-instance corpus.

Implementation checkpoint: `Request::header` scopes authentication independently
of synthetic Action/body handling; `auth::field` enforces unique decoded scalar
credentials. Password/nonce declarations and supported recipient role are checked
before the live user-table lookup. Required nonce bytes must be nonempty; base64
whitespace normalization does not alter Username/Created. Public error shape and
ordinary missing-credential CLI behavior remain unchanged. Private unknown-user,
nonce-decode and digest-mismatch reasons no longer reflect request data.

Old-code reproduction: both new transport tests failed for the special/space
identity (`1789102024_cargo_test.log`). In a full workspace/all-feature/no-fail-fast
mutation run, accepting the first duplicate Security header failed both transport
refusal assertions; changing the mismatch reason failed two strengthened unit
assertions (`1789102471_cargo_test.log`). Both mutations were restored. Negative
fixture generation now asserts it actually changed the request. Oversized HTTP
requests retain the pre-existing transport `413`; in-process requests map the
parser limit to an authentication Fault. They are separately asserted, not equalized.

The old fragment unit fixtures now use qualified SoapEnvelope/UsernameToken
requests and exact success/error payloads. Both-transport controls also preserve
base64 whitespace/default nonce encoding, literal Created text, raw prefix/CDATA
variants, ignored foreign decoys and live password changes. Old timestamps and
repeated valid tokens intentionally remain accepted; no anti-replay claim is made.

Verified on Windows: workspace all-feature tests 1,217 passed and default tests
1,121 passed, each with 5 conditional ignores across 32 suites; both Clippy gates,
formatting, strict default/all-feature rustdocs and inventory self-tests passed.
The inventory now has 159 Action sites, 157 routes and 243 direct readers
(228 production, 15 test, 71 symbols). These are local results, not release acceptance.
The preceding adapter commit `9f390d1` passed CI run `34563256364`.
