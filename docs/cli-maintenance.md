# CLI maintenance workflows

[English](cli-maintenance.md) | [繁體中文](cli-maintenance_zh.md)

These features are **unreleased**, available in the development checkout but not
in published 0.16.0 packages. They do not modify camera configuration.

| Section | Purpose |
| --- | --- |
| [Build](#build) | Run without replacing an installed CLI |
| [Guided workspace](#guided-workspace) | Continue maintenance in one terminal interface |
| [Vim-style navigation](#vim-style-navigation) | Counts, relative numbers, modes and reusable core |
| [Line-number settings](#line-number-settings) | Preview, temporary overrides and saved defaults |
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

Maintenance selectors and execution options can precede or follow the command:
`oxvif --timeout 3s diagnose front-door` and
`oxvif diagnose front-door --timeout 3s` are equivalent. This also applies to
`--device`, `--group`, `--view`, `--jobs`, `--retries`, `--clock-sync` and
`--ca-certificate`. Command-local options remain local (notably discovery jobs).
Slow snapshot-save, diagnosis, export and diff operations show elapsed-time text
only on interactive stderr, unless `--quiet` is set. JSON/JSONL and redirected
output never show progress or request interactive input.

## Guided workspace

Horizontal rules separate the heading, content and keyboard hints in menus,
results, input forms and progress screens. Rules fit the terminal width;
very short terminals omit decoration before reducing the visible content.

```sh
oxvif manage
oxvif manage front-door
oxvif manage --target 192.168.1.100 --timeout 3s
```

Use a real terminal with stdin/stdout/stderr attached. `manage` rejects JSON,
redirection, non-interactive and fleet execution before opening the interface.
Select a saved device, explicitly search the network, or enter an address.
Discovery marks saved/new records; direct/new devices remain session-only.
The network results use the same browser as `discover`: `/` starts live search,
`r` toggles saved records, `n` toggles unregistered records, and uppercase `A`
shows all registration states. Registration filters combine with the text query;
`A` preserves that query, while `c` clears it. Enter/Esc finishes search editing;
then Enter selects the highlighted camera and `q`/Esc returns to the chooser.
Selecting a new record does not save it. These filter keys apply to network
results, not the initial saved-camera chooser or action menus.

Within one `manage` session, returning from a selected camera restores the cached
discovery results, text/registration filters, selection and scroll position. Leaving
the browser and choosing **Return to search results (R to rescan)** also reuses them.
In normal list mode, uppercase `R` explicitly rescans; lowercase `r` remains the
saved-record filter. A successful scan replaces the results and resets the view;
cancellation or failure retains the previous results. The title identifies cached
results, which may become stale; exiting `manage` discards this cache.
Standalone `discover` retains its list state while inspecting details or cancelling
the setup form, but submitting setup ends that command. Running `discover` again
performs a new scan; it does not reuse the `manage` session cache.

Camera chooser status, name and ID columns use the longest value in the full list
as their display width, capped at 24 terminal cells per descriptive column, and
center shorter values; addresses remain left-aligned. Full values are available with `i`.
Column positions therefore remain consistent across pages, including Unicode text.
Use **Session credentials (not saved)** if needed; passwords are masked and neither
the registry nor saved credentials are changed. Restart the workspace to reload
credentials modified outside it. Close the workspace before sharing your terminal.

The workspace retains the device and chosen profile while you diagnose, inspect
profiles/device information, save snapshots, export or compare settings. Returning
from an operation or cancelling credentials retains the action selection and
scroll position; returning to the camera chooser also retains its position
(clamped if the registry list has changed). Arrows or
`j`/`k` move, Page Up/Down page, Enter selects, `i` opens item details, and Esc/`q`
returns (at the device chooser it exits). Results scroll and remain available under
**Last result details**. Failed actions do not replace the previous completed result.
Menus, profile selection and scrollable results also accept Ctrl+D / Ctrl+U to move
down/up half a page (rounded down, at least one item or line), stopping at either end.
Input fields retain Ctrl+U to clear text; Page Up/Down still move a full page.
Paths are entered within the same screen, without shell quotes or variable expansion;
use a literal path and an existing parent directory. Existing destinations are refused.
Long input scrolls to keep the caret visible; Left/Right/Home/End edit the path.
Destination validation retains the entered path for correction instead of clearing it.

Sessions expire after 60 seconds; failures/cancellation invalidate them. The interface
announces reuse or reconnect. A cached session is not a liveness claim. Esc/Ctrl-C
can cancel pending network work; a completed output file may already exist, so inspect
the destination before retrying. Closing `manage` normally returns 0; each operation
displays its own exit status. Automation must use individual commands, not this UI.

Profile details add camera-reported encoding, resolution and configured FPS limit.
Missing values display `not provided`; `details_status=query_failed` identifies a
failed optional metadata lookup. No main/substream roles or measured FPS are inferred.
Metadata uses one additional bounded encoder-configuration operation, with the same
SOAP retry policy. A metadata failure does not discard the usable profile list.

## Vim-style navigation

Manage menus, standalone profile selection, Discovery and read-only detail/result
viewers share one navigation core. This is a Vim-inspired subset, not an editor.

| Keys | Action in navigation mode |
| --- | --- |
| `j` / `k`, Down / Up | Move one item or display line. |
| `7j`, `3k`, count + Down / Up | Move the specified number of items or lines. |
| `gg`, Home | First entry. A single `g` waits for the second key. |
| `G`, End | Last entry; in text viewers, show the final page. |
| `21G`, `21gg` | Jump to ordinal 21 in the current list or wrapped text. |
| PgUp / PgDown, Ctrl+U / Ctrl+D | Full/half-page movement; counts multiply the movement. |
| Esc | Cancel a pending sequence first; otherwise return/cancel. |
| Enter, `i`, `q` | Existing select/details/return actions when no prefix is pending. |
| `?` | Open line-number settings when no prefix is pending. |

By default, the gutter uses **hybrid relative numbers**: the selected row (`>`) has its absolute
one-based ordinal; other rows show distance from it. A `7` below the selection can
be reached with `7j`. These are not immutable device IDs or original discovery record
numbers (the separate `RECORD` column). Filtering restarts the ordinal space.
Long text uses its first visible wrapped line as the anchor, not an editable cursor;
`21G` positions line 21 at the top where possible. Resizing may reflow text and change
its line numbers. Empty lists have no selectable row and show position `0/0`.

The bottom status line displays `NORMAL`, `INPUT`, `SEARCH`, `BUSY` or `SETTINGS`. Navigation
status includes pending keys and current item/line position, for example
`NORMAL | [12g] | item 21/40 | numbers:hybrid`. The reverse-video status bar is
fixed to the bottom row, separate from key hints. Pending input appears only
while a sequence is active; spare space shows screen/device context. BUSY shows
elapsed time, not a guessed completion percentage. Very short terminals prioritize
content; a one-row terminal omits the status bar. `^D` / `^U` in the footer mean Ctrl+D / Ctrl+U.
Counts are limited to six digits. No typing timeout is imposed. Unsupported sequences
such as `3i`, `gq` or count+Enter are cancelled with a hint, without opening an item
or leaving the screen. Ctrl+C remains immediate cancellation/exit.

Search and text/password/path input do not interpret navigation keys: `123ggjk`
remains text, and Ctrl+U clears it. Navigation prefixes reset on screen/mode/filter
changes, resize or a recognized paste event. In navigation mode, recognized paste
events are ignored rather than executed. Unix terminals supporting bracketed paste
have that mode enabled/restored with the terminal session. **Windows' current native
key-event backend and terminals that send paste as ordinary keystrokes cannot
distinguish paste from typing. Do not paste commands into navigation screens.**
This limitation does not change literal typing in input fields.

Discovery preserves `h` / `l` page aliases and `/`, `r`, `n`, `A` filtering controls.
In development builds, its old single `g` binding is replaced by `gg`; Home remains
an immediate alternative. Published 0.16.0 packages have the older behavior.
Small screens reduce decoration/gutter detail before losing the selected row; long
camera names are truncated before column alignment so other identities remain visible.
Cancelling a profile preflight returns to the action menu: dismissing its cancellation
message does not start a second network operation. A genuine profile lookup failure
may still allow diagnosis of other stages.

The reusable implementation is `crates/oxvif-cli/src/navigation.rs`, independent of
crossterm, ONVIF, async work and camera data. Its exact source can be tested without
Cargo: `rustc --edition 2024 --test crates/oxvif-cli/src/navigation.rs -o navigation-tests`
(use an `.exe` output on Windows), then run the resulting test binary. It currently
ships inside the CLI, not as a separately published crate or stable public API.

## Line-number settings

This development-build feature applies to manage menus, standalone profile selection,
Discovery lists/details and text results. It does not affect plain tables or JSON/JSONL.

| Mode | Selected row or first visible text line | Other rows |
| --- | --- | --- |
| `absolute` | One-based ordinal | One-based ordinal |
| `relative` | `0` | Distance from the selection/anchor |
| `hybrid` (default) | One-based ordinal | Distance from the selection/anchor |
| `off` | `>` marker only | No navigation numbers |

```sh
oxvif manage --line-numbers absolute
oxvif --line-numbers off discover
```

In a navigation screen, press `?`. Use `j`/`k` or arrows to preview a mode; the
sample below the choices appears when there is enough terminal height. Enter applies
it for the current process, `s` saves and applies it as the default, and Esc/`q`
cancels without changing anything. A pending number/`g` prefix must be cancelled
first. In SEARCH and input fields, `?` remains literal text; BUSY has no settings
shortcut. The status line shows `numbers:<mode>` when space permits.

Initial precedence is `--line-numbers` → saved preference → `hybrid`. Interactive
Apply can subsequently change this process's mode; it does not write a preference.
Save explicitly replaces the default for future invocations, including when the
current process started with an override. Reopening a menu in the same process
retains the applied mode. External preference edits are loaded by a new process.

The separate `ui-preferences.json` file is in the directory reported by
`oxvif config path` (`OXVIF_CONFIG_DIR` also applies):

```json
{"line_numbers": "absolute"}
```

Saving uses a separate lock and atomic replacement, preserving unknown JSON fields.
A failed save leaves the active mode unchanged and shows an error. Malformed files
are not silently replaced; use `--line-numbers hybrid` to enter a temporary session
and repair the file before saving. Non-interactive/structured commands never read
UI preferences, so even a damaged UI file does not block Agent commands.

Changing this display setting does not alter `7j`, `21G`, filtering, device IDs,
Discovery's original `RECORD` column, or machine output. `off` retains the selection
marker and status position. Text may reflow when its gutter changes; the display-line
anchor is clamped, not preserved as a logical-text bookmark.

## Snapshot download

Snapshot downloads and library health probes now share one bounded HTTP core.
HTTP/1.1 uses title-case field names for firmware that mishandles lowercase
Authorization; standard unquoted Digest parameters are preserved. Multiple
challenges, case-insensitive scheme/parameter names and auth-int for empty GET
entities are handled. A same-policy stale nonce may be retried once within the
total deadline. Digest failure never triggers Basic fallback.

Signature recognition is not full decoding or ONVIF certification. ONVIF snapshots
are JPEG; accepted PNG/BMP files are compatibility extensions. An HTTP 200 text
error remains a failure: inspect camera snapshot/profile configuration, but the
CLI never creates an MJPEG profile or rewrites a vendor CGI automatically.

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

Digest Authorization preserves the generated unquoted `qop`, `algorithm` and `nc`
parameters as required by [RFC 7616 section 3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4).
Challenge-header quoting rules must not be applied to the client response. An
HTTP 401 still requires investigation of credentials and camera HTTP permissions;
successful ONVIF SOAP calls alone do not establish snapshot access.

Snapshot URLs must use HTTP(S), match the device target hostname/IP exactly
(ports may differ), have no userinfo or fragment, and never downgrade an HTTPS
device target to HTTP. Redirects and environment HTTP proxies are not used for
image downloads. Alternate hosts, redirected services and embedded credentials
require operator investigation; the CLI does not bypass these safety checks.

## Layered diagnosis

```sh
oxvif diagnose front-door --profile Profile_1
oxvif diagnose front-door
oxvif diagnose front-door --profile Profile_1 -v
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
failures. Only a unique profile is selected automatically. For a single device in
an interactive terminal, omitted `--profile` opens a paginated name/token selector:
arrows or `j`/`k`, Page Up/Down, Home/End, Enter to select, Esc/`q`/Ctrl-C to cancel.
It reuses the existing session and profile query. Cancellation retains completed
stages and exits `20`. An explicitly unknown token never opens a fallback menu.
Agent, redirected and fleet calls require an explicit token when ambiguous;
candidates remain in the report. Independent checks continue after later failures.

Human output starts with completion and status counts, followed by actionable
failures. Use `-v` for all stages and millisecond timings. Playback limitations are
summarized once in the compact report; they are not successful checks.

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
Human comparisons display field/before/after rows (`<missing>` is distinct from
`null`). Complete equal comparisons say `No configuration changes.`; incomplete
comparisons never claim overall equality.

Failed/unsupported sections appear in `incomparable_sections`; `matches` is `null`
rather than falsely claiming equality. Complete comparisons return exit `0` even
when settings differ: inspect `matches` and `changes`.

## Automation contract

Both human and Agent calls use shared typed requests. Use explicit selectors,
`--output json` or `jsonl`, and `--non-interactive`. Run `describe` against the
installed executable; the embedded Agent guide is version 8 in this checkout.
The base stdout envelope remains schema version 3. New operations return
`device_diagnostic`; fleet diagnosis uses `fleet_diagnostic` or JSONL
`fleet_item` records followed by `fleet_summary`.

Diagnosis adds `summary` counts (`passed`, `failed`, `unsupported`, `not_tested`)
and nullable `selected_profile`. Profile query/selection data includes `candidates`
with `name` and `token`; the original `profiles` token list remains available.
Untested stages include `not_tested_reason`: `prerequisite_failed` for dependent
checks, or `not_implemented` for deferred playback checks.

For compatibility, the selection stage retains `error_code` =
`PROFILE_SELECTION_REQUIRED`. Inspect its `data.reason_code` for precise handling:

| Reason | Meaning |
| --- | --- |
| `PROFILE_SELECTION_REQUIRED` | Multiple candidates; supply a token |
| `PROFILE_NOT_FOUND` | Requested or adapter-returned token was not found |
| `PROFILE_QUERY_FAILED` | Profile query did not produce usable data |
| `NO_PROFILES_AVAILABLE` | Successful query returned an empty list |
| `PROFILE_SELECTION_CANCELLED` | Human cancelled; earlier checks retained |
| `PROFILE_INTERACTION_FAILED` | Terminal adapter failed; earlier checks retained |

These fields are additive; schema version 3 and existing exit meanings are unchanged.

`assessment` adds `primary_issue`, `additional_issues`, `blocked_checks` and
`limitations`. Each issue includes `stage`, `code`, `observed`, `suggested_action`
and `certainty=observed_failure_not_root_cause`. The primary issue is the first
failed stage, not a proven root cause; a timeout is not proof of bad credentials.
Human summaries show the same assessment. `media.profiles` retains its array shape
and existing fields, adding nullable `video` and `details_status` to each record;
diagnosis candidates contain the same metadata. `video.source` explicitly labels
the values as `device_configuration_not_measured`.

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
   Exercise menu selection/cancellation, terminal resizing, `-v`, a redirected
   report and JSON without `--non-interactive`; automation must never prompt.
4. Export twice without configuration changes and compare; then compare against
   an authorized, independently changed setting. Verify the expected path changes.
5. Exercise unsupported settings and mixed online/offline groups; inspect exit
   codes and JSON/JSONL reports from an Agent as well as human-readable output.
6. Run native Windows/macOS/Linux CI and installation checks before release.
7. In `manage`, choose **Search network for cameras** with both saved and new
   devices available. Search by saved ID with `/`, then test `r`, `n`, `A` and
   `c`; verify empty-result recovery and that Enter selects the displayed camera.
   Selecting a new camera must not register it. Check status-bar separation,
   half pages, resizing, literal Unicode/IME input, credential cancellation and
   retained menu position. Check terminal restoration after Esc, `q` and Ctrl+C.

Local mock tests do not establish real-camera interoperability or three-platform
acceptance. See the [implementation plan](https://github.com/smiti1642/oxvif/blob/ddec9ecfc69d503c54c487c28feb64fdad32dca5/docs/active/cli-maintenance-workflows-plan.md)
for current evidence and remaining gates.
