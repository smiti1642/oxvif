# Snapshot interoperability and authentication repair

[English](snapshot-auth-repair.md) | [繁體中文](snapshot-auth-repair_zh.md)

Status: local repair and verification complete. No version promotion, main-branch merge,
camera configuration change or release publication is authorized by this batch.

| Section | Purpose |
| --- | --- |
| [Scope](#scope) | One shared implementation batch |
| [Acceptance](#acceptance) | Local and real-device evidence |
| [Boundaries](#boundaries) | Explicitly deferred work |

## Scope

- [x] Reproduce Authorization field-name casing failure; title-case diagnostic
  requests downloaded JPEG from all 18 previously failing devices. Two devices
  received single-field controls; this is not yet the repaired CLI acceptance.
- [x] Introduce a bounded downloader shared by CLI and library health.
- [x] Preserve standard unquoted Digest values; no Digest-to-Basic failure fallback.
- [x] Parse case-insensitive scheme/parameter names, quoted lists and multiple
  challenges; ignore unknown qop alternatives, hash empty GET entities for
  auth-int, and bound same-policy stale retries.
- [x] Replace permissive authentication fixtures with a server that computes
  expected digests separately and refuses wrong credentials before sending a
  synthetic, independently decoded JPEG fixture.
- [x] Finish code review, negative cases and full-batch sensitivity campaign.
- [x] Pass all/default workspace tests, both Clippy modes, fmt and both strict
  rustdoc modes. Update bilingual CLI/library/release/approval documentation.
- [x] Build the real candidate CLI, retest the 18 devices and a passing control;
  retain Hanwha's non-image failure rather than changing its configuration.
- [x] Commit the completed repair with exact evidence and remaining release gates.

## Acceptance

2026-09-11: the full sensitivity campaign produced 1,306 passes, two meaningful
assertion failures and five ignored tests. Restored all-feature/default gates
passed 1,308 / 1,198 tests, five ignored each, 41 suites. Both Clippy, strict
rustdoc and fmt modes passed; published CHANGELOG history remains unchanged.

Repaired CLI sampling of the first profile on 18 devices: 17 succeed initially,
one times out at eight seconds and succeeds in one 20-second-budget retry.
The passing control succeeds; Hanwha still refuses non-image output. Both
profiles on the original saved GV-TBL8810 save successfully and independently
decode to 640×360; repeat saves exit 4 with unchanged hashes. See the
[approval packet](release-0.17-approval.md) for boundaries.

The mutation campaign must fail meaningful assertions, not compilation, and use
the full workspace all-feature no-fail-fast command. Restore exactly, then run
the final gates once per cohesive batch. Existing CLI tests continue to cover
timeouts, 16 MiB limits including chunked transfer, private CA verification,
redirect refusal and no-clobber output. New tests cover shared health execution,
correct/incorrect Digest responses, MD5/SHA-256/session/auth-int, bounded stale
retries and unsupported/malformed challenges without Basic downgrade.

Real results must distinguish diagnostic-harness evidence from the final CLI,
JPEG signature recognition from decoding, and first-profile sampling from
all-profile acceptance. Keep credentials in process memory and redact addresses,
profile tokens, URI/query values and authentication headers.

The field-name workaround remains valid HTTP: [RFC 9110 §5.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.1)
defines case-insensitive field names. [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)
defines Digest response syntax. The [ONVIF Media specification](https://www.onvif.org/specs/srv/media/ONVIF-Media-Service-Spec.pdf)
requires JPEG snapshots; [Hanwha's SUNAPI guidance](https://support.hanwhavision.com/hc/en-001/articles/47782352496659-How-to-utilize-SUNAPI-snapshot-command)
describes the MJPEG prerequisite for its snapshot command, not proof that changing
this particular camera would resolve the observed response.

## Boundaries

No automatic MJPEG profile creation or camera-specific CGI rewriting. Hanwha's
missing-MJPEG explanation remains a supported hypothesis, not a verified fix.
Full image decoding is not introduced: existing JPEG/PNG/BMP signature acceptance
remains explicitly documented, with PNG/BMP a non-ONVIF compatibility extension.
General SOAP transport changes and strict SnapshotUri lifetime parsing are
separate work; neither was the demonstrated cause of the 18 HTTP 401 failures.
