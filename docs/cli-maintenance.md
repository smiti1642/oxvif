# CLI maintenance workflows

[English](cli-maintenance.md) | [繁體中文](cli-maintenance_zh.md)

These features are **unreleased**, available in the development checkout but not
in published 0.16.0 packages. They do not modify camera configuration.

| Section | Purpose |
| --- | --- |
| [Build](#build) | Run without replacing an installed CLI |
| [Snapshot download](#snapshot-download) | Save an image safely |
| [Layered diagnosis](#layered-diagnosis) | Inspect ONVIF and image delivery |
| [Configuration inventory](#configuration-inventory) | Export and compare settings |
| [Automation contract](#automation-contract) | Outputs, limits and exit codes |
| [Manual acceptance](#manual-acceptance) | Real-device checks before release |

## Build

```sh
cargo build -p oxvif-cli --locked
```

Use `./target/debug/oxvif` or `.\target\debug\oxvif.exe` in PowerShell in place of
`oxvif` below. Building does not replace installed binaries. For selectors,
credential injection, private CA roots and global options, see the
[CLI guide](oxvif-cli.md). File-producing workflows require one device; `diagnose`
also accepts existing Group/View selectors.

## Snapshot download

```sh
oxvif snapshot front-door --profile Profile_1 --save front-door.jpg
oxvif media snapshot-save --target 192.168.1.100 --profile Profile_1 --save front-door.jpg --output json --non-interactive
```

Without `--save`, `snapshot` still returns a URI. The shortcut retains existing
unique-profile/interactive selection behavior; the canonical command requires an
explicit profile. The destination must not exist and its parent directory must
exist. A temporary file is published without overwriting only after a complete
download. Normal failures clean up temporary files; abrupt process termination
or power failure is not a cleanup guarantee.

Results include `saved_to`, `bytes`, `image_type`, `profile` and `validation`, not
image bytes or the download URL. No transcoding occurs: the actual format may
differ from your filename extension. JPEG/PNG/BMP signature checks reject common
HTML/login and short responses, but are **not full image decoding** or confirmation
that the scene is correct. The response body is limited to 16 MiB, including
chunked responses. Images are sensitive; choose an access-controlled destination.

One `--timeout` budget covers HTTP authentication and body transfer. SOAP stages
honor `--retries`; image downloads do not automatically retry. Private CA bundles
are honored without disabling hostname verification. Authentication is
challenge-based: Digest is preferred over explicitly offered Basic, and rejected
Digest is not downgraded to Basic. Basic over HTTP does not encrypt credentials;
use HTTPS or an appropriately trusted network.

Snapshot URLs must use HTTP(S), match the device target hostname/IP exactly
(ports may differ), have no userinfo or fragment, and never downgrade an HTTPS
device target to HTTP. Redirects and environment HTTP proxies are not used for
image downloads. Alternate hosts, redirected services and embedded credentials
require operator investigation; the CLI does not bypass these safety checks.

## Layered diagnosis

```sh
oxvif diagnose front-door --profile Profile_1
oxvif diagnose --target 192.168.1.100 --profile Profile_1 --output json --non-interactive
oxvif diagnose --group taipei-f1 --profile Profile_1 --jobs 8 --output jsonl --non-interactive
```

The implemented stages are session establishment, device information, Media1
profiles, stream URI, snapshot URI and actual image download (not saved).
Session establishment includes network/TLS and ONVIF handshake; it does not
separately measure DNS, TCP and TLS. Each stage records a stable name, status,
elapsed milliseconds, error category and next-step guidance. Raw SOAP faults and
download URLs/bodies are withheld.

Statuses are `pass`, `fail`, `unsupported` and `not_tested`. Explicit device
unsupported-action faults are distinguished from malformed replies or transport
failures. Only a unique profile is selected automatically. Multiple profiles
require `--profile`; available tokens are retained in the report without guessing.
Later failures preserve earlier results, and independent checks continue.

`rtsp_transport` and `video_decode` remain explicitly `not_tested` and
`playback_verified` is always `false`. URI retrieval does not establish RTSP
connectivity/authentication, packet delivery, frame rate or playback. Optional
external-decoder integration remains deferred.

Each SOAP stage applies its own per-attempt `--timeout` and `--retries`, so the
whole workflow may take several timeout periods. `complete` covers implemented
checks, not deferred playback verification. Inspect statuses even on exit `0`.

## Configuration inventory

```sh
oxvif config export front-door --save baseline.json
oxvif config diff front-door --against baseline.json --output json --non-interactive
```

Both commands accept `--target` or `--device`; Group/View selectors are rejected
before network access. Existing `config path` / `config validate` still operate
on local CLI state. Export/diff read the camera's hostname, NTP, DNS, interfaces,
protocols, gateway, Media1 profiles and video encoder configurations.

The standalone baseline contains `inventory_version: 1`, camera `identity` and
named `sections` (status, timing, guidance and optional data). It is neither a
stdout envelope nor a discovery snapshot. Files are limited to 4 MiB and never
overwritten. Partial exports are saved for investigation with `complete=false`.
Passwords, live media URLs, logs and a ticking clock are not collected; the file
still contains sensitive topology and identity information.

This is a limited inventory, **not a restorable backup**. There is no apply or
restore command, and unlisted settings are not covered. Comparison requires a
matching manufacturer/model/serial/hardware identity and a nonempty serial number.
It compares section data, not elapsed time. Keyed record collections are sorted;
DNS/NTP preference ordering is preserved. Differences use JSON Pointer paths with
`before`, `after`, `before_present` and `after_present`; an absent field differs
from an explicit JSON `null`.

Failed/unsupported sections appear in `incomparable_sections`; `matches` is `null`
rather than falsely claiming equality. Complete comparisons return exit `0` even
when settings differ: inspect `matches` and `changes`.

## Automation contract

Both human and Agent calls use shared typed requests. Use explicit selectors,
`--output json` or `jsonl`, and `--non-interactive`. Run `describe` against the
installed executable; the embedded Agent guide is version 6 in this checkout.
The base stdout envelope remains schema version 3. New operations return
`device_diagnostic`; fleet diagnosis uses `fleet_diagnostic` or JSONL
`fleet_item` records followed by `fleet_summary`.

| Outcome | Exit | Result |
| --- | ---: | --- |
| Completed snapshot/export/diagnosis | 0 | File metadata or report; inspect limitations |
| Complete comparison, equal or different | 0 | Inspect `matches` and `changes` |
| Failed/incomplete diagnosis or inventory | 20 | `ok=false`; retained report may be in `data.result`, not top-level `error` |
| Partly successful fleet diagnosis | 6 | Retain all per-device results |
| Entire fleet diagnosis failed | 20 | Retain reports, unlike existing all-failed diagnostics |
| Existing destination | 4 | No overwrite |
| Invalid baseline/identity or unsupported selector | 2 | No comparison/file replacement |

Other established credential, file-I/O and connection error codes still apply.
File workflows are single-device to prevent output collisions. For diagnosis,
inspect `failed`, `complete` and stage statuses; do not require a top-level error
object whenever `ok=false`. Failed fleet items may retain both an error summary
and a stage report. The legacy URI-only, PTZ and health execution is unchanged.
Because `snapshot` now optionally saves files, its descriptor conservatively
declares local write risk and no whole-command automatic retry; `mutates_device`
remains false. `media snapshot-uri` remains an unambiguous read-only operation.

## Manual acceptance

Use authorized targets and disposable output directories; do not paste credentials,
camera images or unredacted inventory into public issues.

1. Download and open an image on a real camera; compare actual format with
   `image_type`. Repeat with an existing filename and verify it is unchanged.
2. Test challenged authentication and a private-CA HTTPS camera. Confirm that
   invalid CA/hostname and unsafe download URLs fail without leaking credentials.
3. Run diagnosis with correct/incorrect credentials and multiple profiles;
   verify retained stages match observed device behavior, not a playback claim.
4. Export twice without configuration changes and compare; then compare against
   an authorized, independently changed setting. Verify the expected path changes.
5. Exercise unsupported settings and mixed online/offline groups; inspect exit
   codes and JSON/JSONL reports from an Agent as well as human-readable output.
6. Run native Windows/macOS/Linux CI and installation checks before release.

Local mock tests do not establish real-camera interoperability or three-platform
acceptance. See the [implementation plan](https://github.com/smiti1642/oxvif/blob/master/docs/active/cli-maintenance-workflows-plan.md)
for current evidence and remaining gates.
