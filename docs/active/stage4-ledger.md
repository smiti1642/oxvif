# Stage 4 coverage ledger — `oxvif`

> **Scope was widened after this ledger was written — read this before working
> from it.** Where the text below says the locked scope is **48** methods, the
> decision of 2026-07-26 is **64**: the 48 *plus* the 16 methods whose negative
> asserts the variant with no payload (`Fault { .. }`, `MissingField(_)`). Same
> defect as the 20 `is_err`-only rows — a negative test that does not assert what
> went wrong — and the cheapest rows here. The measurements in this file are
> unaffected; only the line drawn through them moved. Authority is
> [`refactor-2026-07.md`](refactor-2026-07.md) §2.2.

**Ref analysed:** `c903816360f3a420fe9dcdb871317dd789b17341`
(`refactor/2026-07` tip, = "docs(active): stage 3 step 2 verdict; C18; no deprecation cycle for the shim").
`git status --porcelain` empty at start **and** at end. Analysis ran in a private
locked worktree checked out detached at that SHA, so nothing could move underneath
it (C11b).

> **Worktree correction, read this first.** The worktree handed to this task was
> created from **`5789f41`** — a stale `develop` commit that predates the whole
> programme. It has no `src/tests/client/`, no `src/tests/common.rs`, no
> `tests/mock_action_snapshot.rs` and no `docs/active/refactor-2026-07.md`. A
> ledger built there would describe a tree Stage 4 will never touch — the exact
> failure C11b exists to prevent, inverted. The worktree was therefore
> `git checkout --detach refactor/2026-07`'d to `c903816` before any measurement.
> No file was edited, added or deleted; nothing was committed.

## Definitions used (fixed before measuring)

- **positive `yes`** — calls the method and asserts on the *outcome*: fields of the
  returned value, or (write/void) `c.action` / `c.body` from `RecordingTransport`.
- **positive `weak`** — reaches the method but asserts nothing beyond "did not
  error" (`.unwrap()` alone, `.expect(...)` alone, `assert!(res.is_ok())`).
- **positive `no`** — no test asserts anything about this method's outcome.
- **negative `yes`** — drives an error and asserts variant **and** payload
  (field path, fault code/reason, or message content).
- **negative `hollow`** — reaches an error path but asserts less than that.
  Three sub-flavours, always named in the evidence column because they need
  different amounts of Stage 4 work:
  - `[is_err-only]` — `assert!(res.is_err())` / `matches!(res, Err(_))` and nothing else.
    **This is the class §2.2 counted as "the 21 hollow negatives".**
  - `[outer-variant-only]` — `matches!(err, OnvifError::Soap(_))` or
    `OnvifError::Transport(_)`. Names the outer enum arm; every parse failure in
    the crate satisfies it, so it discriminates almost nothing.
  - `[inner-variant-no-payload]` — e.g. `OnvifError::Soap(SoapError::MissingField(_))`:
    names both arms but not *which* field.
- **negative `n/a`** — the method is infallible (returns `Self` or `&str`, not
  `Result`). Listed separately in §5, never counted in the work set.
- **snapshot-only** — `yes` means the method appears in
  `tests/mock_action_snapshot.rs`'s `EXPECTED` table. Per C6 this is a **call
  site, not coverage**; it is tracked so a method that looks green is not mistaken
  for a tested one.

## Universe — reconciled two ways (C12)

| | count |
|---|---|
| Grep tool, line-anchored `^\s*pub (async )?fn \w+` over `src/client/` | **149** |
| Independent token parser (comment/string-stripped, newline-tolerant, `.claude`-free) | **150** |

The two disagree by exactly one, and the difference is fully explained: the token
parser's `pub` pattern also accepts `pub(crate)`, so it additionally counts
`pub(crate) async fn call` at `src/client/mod.rs:144` — an internal SOAP helper,
not public API. Excluding it both methods give **149 `pub` fns in `src/client/`**.

149 = 1 free function (`notification_listener`, `src/client/events.rs:321`) +
**148 `OnvifClient` methods**. That is §2.2's 148, independently re-derived.

Third, independent cross-check: the snapshot net's hand-written `EXPECTED` table
holds **141** rows (matching C12's stated 141), and
`148 − 141 = 7` is exactly `{new, with_credentials, with_transport, with_utc_offset,
device_url, event_stream, search_recordings}` — the 7 methods the snapshot cannot
or does not drive. Three independent counts, no residual.

| service file | methods | of which `pub async fn` |
|---|---|---|
| `device.rs` | 38 | 38 |
| `events.rs` | 8 | 7 (`event_stream` is sync, returns a `Stream`) |
| `imaging.rs` | 7 | 7 |
| `media.rs` | 31 | 31 |
| `media2.rs` | 26 | 26 |
| `mod.rs` | 5 | 0 (builders/accessor) |
| `ptz.rs` | 18 | 18 |
| `recording.rs` | 15 | 15 |
| **total** | **148** | **142** |

The task's stated universe ("every `pub async fn`") is 142. The ledger covers all
**148**, because §2.2's locked scope explicitly names `event_stream` (hollow) and
`with_utc_offset` / `device_url` (no call site) — dropping the 6 non-async methods
would silently drop three of Stage 4's own named targets.

---

## Ledger

| service | method | positive | negative | snapshot-only | evidence |
|---|---|---|---|---|---|
| device | create_users | yes | no | yes | `test_create_users_sends_correct_body` (asserts `c.action` + `<tt:Username>`) |
| device | delete_users | yes | hollow | yes | pos `test_delete_users_sends_correct_body`; neg `test_delete_users_transport_error` — `assert!(matches!(err, OnvifError::Transport(_)))` `[outer-variant-only]`, and transport-level, not protocol-level |
| device | get_capabilities | yes | yes | yes | pos `test_get_capabilities_returns_correct_urls`; neg `test_get_capabilities_soap_fault_returns_err` asserts `code == "s:Sender"`, plus `test_get_capabilities_http_error_returns_err` asserts `HttpStatus { status: 401, .. }` |
| device | get_device_info | yes | no | yes | `test_get_device_info_returns_correct_fields` |
| device | get_digital_inputs | yes | hollow | yes | pos `test_get_digital_inputs_returns_fields`; neg `test_get_digital_inputs_missing_token_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_discovery_mode | yes | yes | yes | pos `get_discovery_mode_pins_action_body_and_parsed_value` (+ `test_get_discovery_mode_returns_value`); neg `get_discovery_mode_without_the_element_is_a_missing_field_error` asserts `field == "GetDiscoveryModeResponse/DiscoveryMode"`, + empty/whitespace twins + `get_discovery_mode_propagates_a_soap_fault` (code+reason). **Fixed by Stage 2 (`ddfde44`)** |
| device | get_dns | yes | hollow | yes | pos `test_get_dns_returns_servers`; neg `test_get_dns_missing_dns_information_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_hostname | yes | no | yes | `test_get_hostname_returns_name_and_flag`, `test_get_hostname_uses_device_url` |
| device | get_network_default_gateway | yes | hollow | yes | pos `test_get_network_default_gateway_returns_address`; neg `test_get_network_default_gateway_missing_node_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_network_interfaces | yes | hollow | yes | pos `test_get_network_interfaces_returns_fields`; neg `test_get_network_interfaces_missing_token_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_network_protocols | yes | no | yes | `test_get_network_protocols_returns_list` |
| device | get_ntp | yes | no | yes | `test_get_ntp_returns_servers` |
| device | get_relay_outputs | yes | hollow | yes | pos `test_get_relay_outputs_returns_fields`; neg `test_get_relay_outputs_missing_token_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_scopes | yes | no | yes | `test_get_scopes_returns_uris` |
| device | get_services | **no** | **no** | yes | no call site in any unit test (Grep-verified with a passing control). Only `tests/mock_action_snapshot.rs:101,260` |
| device | get_storage_configurations | yes | hollow | yes | pos `test_get_storage_configurations_returns_fields`; neg `test_get_storage_configurations_missing_token_returns_err` — `assert!(matches!(err, OnvifError::Soap(crate::soap::SoapError::MissingField(_))))` `[inner-variant-no-payload]` — closest of all the hollows to a real negative; needs only the field string |
| device | get_system_date_and_time | **no** | **no** | yes | no unit test. `tests/mock_workflow.rs:35` calls it as `s.get_system_date_and_time().await.unwrap();` with no assertion, and that file is gated `#![cfg(feature = "mock-server")]` |
| device | get_system_log | yes | hollow | yes | pos `test_get_system_log_returns_string`; neg `test_get_system_log_missing_system_log_returns_err` — `assert!(matches!(err, OnvifError::Soap(_)))` `[outer-variant-only]` |
| device | get_system_uris | yes | no | yes | `test_get_system_uris_returns_fields` |
| device | get_users | yes | no | yes | `test_get_users_returns_list` |
| device | send_auxiliary_command | yes | no | yes | `test_send_auxiliary_command_returns_response`, `test_send_auxiliary_command_escapes_input` |
| device | set_discovery_mode | yes | no | yes | `test_set_discovery_mode_sends_correct_body` (action + body) |
| device | set_dns | yes | no | yes | `test_set_dns_sends_correct_body` |
| device | set_hostname | yes | no | yes | `test_set_hostname_sends_name` |
| device | set_network_default_gateway | yes | yes | yes | pos `test_set_network_default_gateway_sends_addresses`; neg `test_set_network_default_gateway_soap_fault` — `assert!(err.to_string().contains("Action not supported"))`, i.e. asserts the fault *reason* |
| device | set_network_interfaces | yes | no | yes | `test_set_network_interfaces_sends_ipv4_body` (+ ipv6/mtu + reboot-needed variants) |
| device | set_network_protocols | yes | no | yes | `test_set_network_protocols_sends_correct_body` |
| device | set_ntp | yes | no | yes | `test_set_ntp_sends_from_dhcp_false_and_servers`, `test_set_ntp_from_dhcp_true_sends_no_servers` |
| device | set_relay_output_settings | yes | no | yes | `test_set_relay_output_settings_sends_correct_body` |
| device | set_relay_output_state | yes | no | yes | `test_set_relay_output_state_sends_correct_body` |
| device | set_scopes | yes | **hollow** | yes | pos `test_set_scopes_sends_scope_elements`, `test_set_scopes_xml_escapes_value`; neg `test_set_scopes_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| device | set_storage_configuration | yes | no | yes | `test_set_storage_configuration_sends_correct_body` |
| device | set_system_date_and_time | yes | **hollow** | yes | pos `test_set_system_date_and_time_manual_sends_utc_fields`, `..._ntp_omits_utc_element`; neg `test_set_system_date_and_time_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| device | set_system_factory_default | yes | no | yes | `test_set_system_factory_default_sends_correct_body` |
| device | set_user | yes | no | yes | `test_set_user_sends_correct_body` |
| device | start_firmware_upgrade | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:134,369` |
| device | start_system_restore | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:135,370` |
| device | system_reboot | yes | no | yes | `test_system_reboot_returns_message`, `test_system_reboot_uses_device_url` |
| events | create_pull_point_subscription | yes | no | yes | `test_create_pull_point_subscription_returns_reference_url` (+ 2 body tests) |
| events | event_stream | yes | **hollow** | no | pos `test_event_stream_yields_notification_messages` (asserts `msg.topic`); neg `test_event_stream_error_on_bad_response` — `assert!(result.is_err());` `[is_err-only]` — **§2.2 target**. Not in the snapshot table |
| events | get_event_properties | yes | no | yes | `test_get_event_properties_flattens_topics` |
| events | pull_messages | yes | no | yes | `test_pull_messages_parses_notification`, `test_pull_messages_empty_returns_empty_vec` |
| events | renew_subscription | yes | no | yes | `test_renew_subscription_returns_new_termination_time`, `..._uses_oasis_action_uri` |
| events | set_synchronization_point | yes | no | yes | `test_set_synchronization_point_ok` (asserts action contains) |
| events | subscribe | yes | yes | yes | pos `test_subscribe_parses_push_subscription`; neg `test_subscribe_soap_fault_returns_error` asserts `.to_string().contains("InvalidConsumerReference")` — payload asserted |
| events | unsubscribe | yes | no | yes | `test_unsubscribe_uses_oasis_action_uri` |
| imaging | get_imaging_options | yes | no | yes | `test_get_imaging_options_parses_ranges_and_modes`, GeoVision/Hikvision Min*/Max* regressions |
| imaging | get_imaging_settings | yes | no | yes | `test_get_imaging_settings_parses_all_fields` (+ backlight, focus/wdr) |
| imaging | imaging_get_move_options | yes | hollow | yes | pos `test_imaging_get_move_options_parses_ranges`; neg `test_imaging_get_move_options_missing_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` |
| imaging | imaging_get_status | yes | hollow | yes | pos `test_imaging_get_status_parses_focus`; neg `test_imaging_get_status_missing_status_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` |
| imaging | imaging_move | yes | yes | yes | pos `test_imaging_move_sends_absolute_body`; neg `test_imaging_move_soap_fault_returns_err` asserts `code == "s:Sender" && reason == "ter:NoFocus"` |
| imaging | imaging_stop | yes | yes | yes | pos `test_imaging_stop_sends_source_token_and_action`; neg `test_imaging_stop_soap_fault_returns_err` asserts code+reason. **The one method Stage 1a cleared (C3)** |
| imaging | set_imaging_settings | yes | no | yes | `test_set_imaging_settings_serialises_fields` |
| media | add_video_encoder_configuration | yes | yes | yes | pos `test_add_video_encoder_configuration_ok`; neg `..._soap_fault_returns_err` asserts code+reason |
| media | add_video_source_configuration | yes | yes | yes | pos `test_add_video_source_configuration_ok`; neg `..._soap_fault_returns_err` asserts code+reason |
| media | create_osd | yes | no | yes | `test_create_osd_returns_token` |
| media | create_profile | yes | no | yes | `test_create_profile_returns_profile`, `..._with_token_sends_token` |
| media | delete_osd | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:178,559` |
| media | delete_profile | yes | no | yes | `test_delete_profile_sends_token` |
| media | get_audio_encoder_configuration | yes | no | yes | `test_get_audio_encoder_configuration_parses_channels` |
| media | get_audio_encoder_configuration_options | yes | no | yes | `test_get_audio_encoder_configuration_options_ok` |
| media | get_audio_encoder_configurations | yes | no | yes | `test_get_audio_encoder_configurations_ok` |
| media | get_audio_source_configurations | yes | no | yes | `test_get_audio_source_configurations_ok` |
| media | get_audio_sources | yes | no | yes | `test_get_audio_sources_ok` |
| media | get_osd | yes | no | yes | `test_get_osd_parses_colors_and_persistence`; also `mock_get_osd_response_parses_via_client` |
| media | get_osd_options | yes | hollow | yes | pos `test_get_osd_options_parses_max_and_types`; neg `test_get_osd_options_missing_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]`. Extra positives live in `session_tests.rs` (vendor-extension enrichment ×3) |
| media | get_osds | yes | hollow | yes | pos `test_get_osds_parses_configuration`, `..._sends_configuration_token_element`; neg `test_get_osds_missing_token_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` |
| media | get_profile | yes | **hollow** | yes | pos `test_get_profile_returns_correct_fields`; neg `test_get_profile_missing_token_returns_err` — `assert!(result.is_err(), "expected Err when profile token attribute is absent")` `[is_err-only]` — **§2.2 target** |
| media | get_profiles | yes | **hollow** | yes | pos `test_get_profiles_returns_all_profiles`, `..._parses_config_tokens`; **two** hollow negatives: `test_get_profiles_missing_token_returns_err` — `assert!(result.is_err(), "expected Err when profile token is missing")` and `test_get_profiles_malformed_xml_returns_err` — `assert!(result.is_err(), "expected Err on malformed XML")`, both `[is_err-only]` — **§2.2's "get_profiles (×2 sites)"** |
| media | get_snapshot_uri | **no** | **no** | yes | no unit test. `mock_workflow.rs:62` asserts `!...uri.is_empty()` but is feature-gated `mock-server`; snapshot row at `:157` |
| media | get_stream_uri | yes | **hollow** | yes | pos `test_get_stream_uri_returns_rtsp_url`, `..._embeds_profile_token_in_body`, `..._escapes_profile_token`; neg `test_get_stream_uri_missing_uri_returns_err` — `assert!(result.is_err(), "expected Err when Uri element is missing")` `[is_err-only]` — **§2.2 target** |
| media | get_video_encoder_configuration | yes | no | yes | `test_get_video_encoder_configuration_single`, `..._parses_multicast`, `..._parses_guaranteed_frame_rate` |
| media | get_video_encoder_configuration_options | yes | no | yes | `test_get_video_encoder_configuration_options_parses_h264` |
| media | get_video_encoder_configurations | yes | no | yes | `test_get_video_encoder_configurations_returns_all` |
| media | get_video_source_configuration | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:167`. Note the *plural* `get_video_source_configurations` is tested — easy to mistake one for the other |
| media | get_video_source_configuration_options | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:169,500` |
| media | get_video_source_configurations | yes | no | yes | `test_get_video_source_configurations_returns_all` |
| media | get_video_sources | yes | no | yes | `test_get_video_sources_returns_correct_fields` |
| media | remove_video_encoder_configuration | yes | yes | yes | pos `test_remove_video_encoder_configuration_ok`; neg `..._soap_fault_returns_err` asserts code+reason |
| media | remove_video_source_configuration | yes | yes | yes | pos `test_remove_video_source_configuration_ok`; neg `..._soap_fault_returns_err` asserts code+reason |
| media | set_audio_encoder_configuration | yes | yes | yes | **tests live in `media2_tests.rs`**, mod `request_body_shapes`: pos `set_audio_encoder_configuration_media1_emits_trt_configuration` (exact fragment + action); neg `set_audio_encoder_configuration_media1_soap_fault_returns_fault` asserts code+reason |
| media | set_osd | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:176,534` |
| media | set_video_encoder_configuration | yes | yes | yes | pos `set_video_encoder_configuration_media1_body_is_exact` (**in `media2_tests.rs`**); neg `test_set_video_encoder_configuration_rejects_h265_via_media1` (in `media_tests.rs`) matches `OnvifError::InvalidArgument(msg)` and asserts `msg.contains("H265")` + Media2 hint. A gate error, not a device error — but variant+payload are asserted |
| media | set_video_source_configuration | yes | no | yes | pos `set_video_source_configuration_media1_body_is_exact` (**in `media2_tests.rs`**) |
| media2 | add_configuration_media2 | yes | no | yes | `test_add_configuration_media2_sends_type_and_token` |
| media2 | create_profile_media2 | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:198,661` |
| media2 | delete_profile_media2 | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:199,666` |
| media2 | get_audio_decoder_configurations_media2 | yes | no | yes | `test_get_audio_decoder_configurations_parses_response` |
| media2 | get_audio_encoder_configuration_options_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:207,713` |
| media2 | get_audio_encoder_configurations_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:206` |
| media2 | get_audio_output_configurations_media2 | yes | no | yes | `test_get_audio_output_configurations_parses_response` |
| media2 | get_audio_source_configurations_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:205,703` |
| media2 | get_metadata_configuration_options_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:204,698` |
| media2 | get_metadata_configurations_media2 | yes | hollow | yes | pos `test_get_metadata_configurations_parses_response`; neg `test_get_metadata_configurations_missing_token_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` |
| media2 | get_profiles_media2 | yes | no | yes | `test_get_profiles_media2_returns_correct_fields`, `..._parses_audio_ptz_tokens` |
| media2 | get_snapshot_uri_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:189,603` |
| media2 | get_stream_uri_media2 | yes | no | yes | `test_get_stream_uri_media2_returns_string` |
| media2 | get_video_encoder_configuration_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:194` |
| media2 | get_video_encoder_configuration_options_media2 | yes | no | yes | `test_get_video_encoder_configuration_options_media2_parses_options` |
| media2 | get_video_encoder_configurations_media2 | yes | no | yes | `test_get_video_encoder_configurations_media2_parses_h265` |
| media2 | get_video_encoder_instances_media2 | yes | no | yes | `test_get_video_encoder_instances_parses_total` |
| media2 | get_video_source_configuration_options_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:192,625` |
| media2 | get_video_source_configurations_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:190` |
| media2 | get_video_source_modes_media2 | yes | no | yes | `test_get_video_source_modes_parses_response` |
| media2 | remove_configuration_media2 | yes | no | yes | `test_remove_configuration_media2_sends_type_and_token` |
| media2 | set_audio_encoder_configuration_media2 | yes | yes | yes | pos `set_audio_encoder_configuration_media2_emits_tr2_configuration` (exact fragment; **flipped by Stage 1b `573168a`**); neg `set_audio_encoder_configuration_media2_soap_fault_returns_fault` asserts code+reason |
| media2 | set_metadata_configuration_media2 | **no** | **no** | yes | no call site in any unit test. Only `mock_action_snapshot.rs:203` |
| media2 | set_video_encoder_configuration_media2 | yes | no | yes | `set_video_encoder_configuration_media2_body_is_exact` (exact fragment + action) |
| media2 | set_video_source_configuration_media2 | yes | no | yes | `set_video_source_configuration_media2_body_is_exact` |
| media2 | set_video_source_mode_media2 | yes | no | yes | `test_set_video_source_mode_sends_tokens` (asserts returned `reboot` + body) |
| mod | device_url | **no** | n/a | no | **no unit-test call site at all**; only `mock_workflow.rs` (feature-gated), as an argument to `OnvifSession::builder`. Infallible `&str` getter → no negative possible. Confirms §2.2 |
| mod | new | yes | n/a | n/a | exercised as `OnvifClient::new(...)` in essentially every test; infallible constructor |
| mod | with_credentials | yes | n/a | n/a | `test_credentials_add_ws_security_header` asserts `<wsse:Username>admin</wsse:Username>` in the emitted body; `test_ws_security_escapes_username`; `test_no_credentials_omits_security_header` is its complement. Infallible builder |
| mod | with_transport | yes | n/a | n/a | 248 call sites across the test suite; its effect is what every mock-based assertion rests on. Infallible builder |
| mod | with_utc_offset | **no** | n/a | no | **no call site anywhere in the test suite** (Grep-verified with a passing control). Infallible builder → no negative possible. Confirms §2.2 |
| ptz | ptz_absolute_move | **no** | **no** | yes | no unit test. `mock_workflow.rs:104` (feature-gated) moves then asserts via `ptz_get_status`; snapshot row `:214` |
| ptz | ptz_continuous_move | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:216,762` |
| ptz | ptz_get_compatible_configurations | yes | no | yes | `test_ptz_get_compatible_configurations_sends_profile_token` (len + token + body); `mock_get_compatible_configurations_response_parses_via_client` |
| ptz | ptz_get_configuration | yes | no | yes | `test_ptz_get_configuration_parses_default_spaces` |
| ptz | ptz_get_configuration_options | yes | no | yes | `test_ptz_get_configuration_options_ok` |
| ptz | ptz_get_configurations | yes | **hollow** | yes | pos `test_ptz_get_configurations_ok`; neg `test_ptz_get_configurations_missing_token_returns_err` — `assert!(result.is_err());` `[is_err-only]` — **§2.2 target** |
| ptz | ptz_get_node | yes | no | yes | `test_ptz_get_node_parses_response`, `test_ptz_get_node_sends_token` |
| ptz | ptz_get_nodes | yes | **hollow** | yes | pos `test_ptz_get_nodes_ok`, `..._parses_spaces`; neg `test_ptz_get_nodes_missing_token_returns_err` — `assert!(result.is_err());` `[is_err-only]` — **§2.2 target** |
| ptz | ptz_get_presets | **no** | **no** | yes | no unit test — **the C6 datum**: breaking its response tag left all `client::ptz` tests green. `mock_workflow.rs:101` asserts non-empty but is feature-gated; snapshot row `:218` |
| ptz | ptz_get_status | yes | no | yes | `test_ptz_get_status_parses_position_and_move_status`, `..._no_position_is_none`, `..._parses_utc_time`, `..._parses_error`, `..._no_error_is_none` |
| ptz | ptz_goto_home_position | yes | no | yes | `test_ptz_goto_home_position_ok` (action + body) |
| ptz | ptz_goto_preset | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:219,769` |
| ptz | ptz_relative_move | **no** | **no** | yes | no call site in any unit test (Grep-verified). Only `mock_action_snapshot.rs:215,757` |
| ptz | ptz_remove_preset | yes | no | yes | `test_ptz_remove_preset_embeds_tokens` |
| ptz | ptz_set_configuration | yes | no | yes | `test_ptz_set_configuration_ok` (action + body) |
| ptz | ptz_set_home_position | yes | no | yes | `test_ptz_set_home_position_ok` (action + body) |
| ptz | ptz_set_preset | yes | no | yes | `test_ptz_set_preset_returns_token`, `..._embeds_name_and_optional_token`, `..._without_name_or_token` |
| ptz | ptz_stop | **weak** | **no** | yes | **tests live in `session_tests.rs`, not `ptz_tests.rs`.** `test_ptz_stop_delegates_ok` is the whole body: `session.ptz_stop("Profile_1").await.unwrap();` — no assertion at all, so `weak` not `yes`. `test_missing_ptz_url_returns_error` asserts `MissingField(_)` but that is `OnvifSession`'s URL resolver failing *before* the client method runs, so it is not a negative for `OnvifClient::ptz_stop` |
| recording | create_recording | yes | **hollow** | yes | pos `test_create_recording_returns_token`; neg `test_create_recording_missing_token_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | create_recording_job | yes | **hollow** | yes | pos `test_create_recording_job_returns_token` (+ `..._xml_escapes_token`); neg `test_create_recording_job_missing_token_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | create_track | yes | **hollow** | yes | pos `test_create_track_returns_token`; neg `test_create_track_missing_token_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | delete_recording | yes | **hollow** | yes | pos `test_delete_recording_ok`; neg `test_delete_recording_soap_fault_returns_err` — `assert!(res.is_err());` against a real `make_soap_fault_xml` `[is_err-only]` — **§2.2 target** |
| recording | delete_recording_job | yes | **hollow** | yes | pos `test_delete_recording_job_ok`; neg `test_delete_recording_job_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | delete_track | yes | **hollow** | yes | pos `test_delete_track_ok`; neg `test_delete_track_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | end_search | yes | **hollow** | yes | pos `test_end_search_ok`; neg `test_end_search_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | find_recordings | yes | hollow | yes | pos `test_find_recordings_returns_token`; neg `test_find_recordings_missing_token_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` — **not** in §2.2's 21 |
| recording | get_recording_job_state | yes | **hollow** | yes | pos `test_get_recording_job_state_parses_active_state`; neg `test_get_recording_job_state_missing_state_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | get_recording_jobs | yes | **hollow** | yes | pos `test_get_recording_jobs_parses_fields`; neg `test_get_recording_jobs_missing_job_token_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | get_recording_search_results | yes | **hollow** | yes | pos `test_get_recording_search_results_parses_completed`, `test_search_results_geovision_resultlist_queued`; neg `test_get_recording_search_results_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |
| recording | get_recordings | yes | hollow | yes | pos `test_get_recordings_parses_item`, `..._geovision_real`, `..._parses_track_times_and_address`; neg `test_get_recordings_missing_token_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` — **not** in §2.2's 21 |
| recording | get_replay_uri | yes | hollow | yes | pos `test_get_replay_uri_returns_rtsp`; neg `test_get_replay_uri_missing_uri_returns_err` — `assert!(matches!(err, crate::error::OnvifError::Soap(_)))` `[outer-variant-only]` — **not** in §2.2's 21 |
| recording | search_recordings | yes | **hollow** | no | pos `test_search_recordings_returns_empty_on_completed_no_results` (asserts `results.is_empty()`); neg `test_search_recordings_propagates_find_error` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target**. Not in the snapshot table (it is a multi-call wrapper) |
| recording | set_recording_job_mode | yes | **hollow** | yes | pos `test_set_recording_job_mode_sends_correct_body`; neg `test_set_recording_job_mode_soap_fault_returns_err` — `assert!(res.is_err());` `[is_err-only]` — **§2.2 target** |

---

## 1. Totals

| metric | count |
|---|---|
| **Methods total** | **148** |
| positive `yes` | 120 |
| positive `weak` | 1 (`ptz_stop`) |
| positive `no` (**zero-positive**) | 27 |
| negative `yes` | 13 |
| negative `hollow` — `[is_err-only]` | **20 methods / 21 sites** |
| negative `hollow` — `[outer-variant-only]` + `[inner-variant-no-payload]` | 16 |
| negative `no` (**zero-negative**) | 94 |
| negative `n/a` (infallible) | 5 |
| **Fully compliant today** (real positive **and** real negative) | **13** |

Cross-foots: `120 + 1 + 27 = 148`; `13 + 20 + 16 + 94 + 5 = 148`.

### Stage 4 work set

**Under §2.2's locked scope** ("the zero-coverage methods + the hollow negatives"),
re-derived at `c903816`:

| component | methods |
|---|---|
| no real positive (27 zero + 1 weak `ptz_stop`) | **28** |
| negative is `[is_err-only]` hollow | **20** |
| overlap between the two | 0 (the 28 have no negative at all; the 20 all have real positives) |
| **locked-scope work set** | **48 methods** (49 test sites — `get_profiles` has two hollow sites) |

**Under full CLAUDE.md compliance** (every method needs a real positive *and* a
real negative): **132 methods**. `148 − 13 already compliant − 3 exempt`
(`new`, `with_credentials`, `with_transport`: infallible **and** already positively
tested). The other 84 methods beyond the locked scope are the `partial` class —
real positive, negative missing or variant-only — which §2.2 defers to §8.
Decomposition of the 132: 28 need a positive, 36 need a negative upgraded,
94 need a negative written from scratch (minus the 26 double-counted methods that
need both), i.e. `28 + 36 + 94 − 26 = 132`.

---

## 2. Reconciliation against the rumoured 26 + 21 = 47

**Verdict: the shape holds, the numbers move to 28 + 20 = 48 methods (49 sites).**
The rumour was not wildly wrong — but neither half is exactly reproducible, and
the two halves are wrong in opposite directions.

### The "21 hollow" → 20 methods / 21 sites

§2.2's own enumeration reads: *"`ptz_get_configurations`, `ptz_get_nodes`,
`get_profiles` (×2 sites), `get_profile`, `get_stream_uri`, `set_scopes`,
`set_system_date_and_time`, `event_stream`, and all twelve recording ones."*
Counted as written that is 8 named + `get_profiles` twice + 12 = **21**. Every one
of those 21 sites is still present and still `assert!(res.is_err())` at `c903816`
— **I confirmed all 21 individually**; none was fixed by Stages 1a/1b/2/3.

The 21 is a count of **test sites**, not methods: `get_profiles` contributes two
(`test_get_profiles_missing_token_returns_err` and
`test_get_profiles_malformed_xml_returns_err`). By method the figure is **20**.
Both numbers are correct for their own unit; Stage 4 should size itself by 21
(edits to make) and report by 20 (methods cleared).

**Three further hollow negatives §2.2 missed**, all in recording:
`get_recordings`, `find_recordings`, `get_replay_uri`. They assert
`matches!(err, OnvifError::Soap(_))` rather than bare `is_err()`, so they fall
outside §2.2's literal criterion — but they discriminate almost nothing, and
§2.2's own stated rationale ("nine feed a real `make_soap_fault_xml` and then
assert only `is_err()` … turning a Fault into an `UnexpectedResponse` or
`MissingField` would leave all of them green") applies to them verbatim.
**Recommendation: fold them in. Recording's dangerous cluster is 15, not 12.**

### The "26 zero-coverage" → 28 methods with no real positive

I count **27** methods with neither a positive nor a negative, plus `ptz_stop`
which has a *weak* positive only — **28** methods lacking a real positive.
The full list, so the next agent can diff rather than re-derive:

- **device (4):** `get_services`, `get_system_date_and_time`, `start_firmware_upgrade`, `start_system_restore`
- **media (5):** `get_snapshot_uri`, `get_video_source_configuration`, `get_video_source_configuration_options`, `set_osd`, `delete_osd`
- **media2 (11):** `get_snapshot_uri_media2`, `get_video_source_configurations_media2`, `get_video_source_configuration_options_media2`, `get_video_encoder_configuration_media2`, `create_profile_media2`, `delete_profile_media2`, `set_metadata_configuration_media2`, `get_metadata_configuration_options_media2`, `get_audio_source_configurations_media2`, `get_audio_encoder_configurations_media2`, `get_audio_encoder_configuration_options_media2`
- **ptz (5):** `ptz_absolute_move`, `ptz_relative_move`, `ptz_continuous_move`, `ptz_get_presets`, `ptz_goto_preset`
- **mod (2):** `with_utc_offset`, `device_url`
- **weak-only (1):** `ptz_stop`

**Method-by-method movement vs §2.2:**

- `ptz_stop` — §2.2 lists it among PTZ's six zero-coverage methods. It is not
  zero: `src/tests/session_tests.rs:498` `test_ptz_stop_delegates_ok` calls it and
  `.unwrap()`s. **But it asserts nothing**, so it is `weak`, and Stage 4 must still
  write a real positive (`RecordingTransport` action+body). Net effect on the work
  set: none. Net effect on the *description*: "no test at all" is wrong; "no test
  that would notice if the body were empty" is right. The tests being in
  `session_tests.rs` rather than `ptz_tests.rs` is why an earlier survey scoped to
  `src/tests/client/` would have missed it.
- `with_utc_offset`, `device_url` — §2.2 flags these separately ("no call site in
  any test at all"), which reads as *outside* the 26. Confirmed exactly: still zero
  call sites. They are in my 28. If §2.2's 26 excluded them, that alone is the
  26 → 28 gap.
- `imaging_stop` — §2.2/C3 record it as the single method Stage 1a cleared.
  Confirmed: now `yes`/`yes`, fully covered.
- Nothing else moved. Stages 1a/1b/2/3 cleared exactly **two** ledger rows between
  them: `imaging_stop` (1a, → covered) and `get_discovery_mode` (2, → covered, with
  the field path asserted). Stage 1b upgraded
  `set_audio_encoder_configuration_media2`'s positive from a bug-pin to a correct
  pin but did not change its classification (already covered). Stage 3 touched
  `src/metamorph/` only and moved no row.

### The unreconcilable part: §2.2's "covered 32 / partial 90 / zero 26"

I get **covered 13**, not 32. The gap is a definitional one, and it is worth
stating plainly because it changes what "compliant" means:

- Strict reading (this ledger; the task's definition — variant **and** payload):
  **13 covered**.
- Lenient reading (any assertion naming an error variant counts):
  `13 + 16 = 29` covered.

29 is close to §2.2's 32 but does not equal it, and I cannot close the last 3
without §2.2's raw per-method ledger — which does not exist in the repo and which
C11b tells us was measured against a moving tree while Stage 1a was in flight.
**Recommendation: treat 32/90/26 as superseded by this table rather than trying to
reconcile it.** The consequence for §2.2's closing claim is material: "full
CLAUDE.md compliance would be 116 methods" becomes **132** under the strict
reading Stage 4 is being asked to apply.

---

## 3. Per-service breakdown (for batching)

| service | methods | zero-pos | weak-pos | hollow `[is_err-only]` | hollow (variant-only) | real neg | **locked-scope work** | full-compliance work |
|---|---|---|---|---|---|---|---|---|
| device | 38 | 4 | 0 | 2 | 8 | 3 | **6** | 35 |
| events | 8 | 0 | 0 | 1 | 0 | 1 | **1** | 7 |
| imaging | 7 | 0 | 0 | 0 | 2 | 2 | **0** | 5 |
| media | 31 | 5 | 0 | 3 | 2 | 6 | **8** | 25 |
| media2 | 26 | 11 | 0 | 0 | 1 | 1 | **11** | 25 |
| mod | 5 | 2 | 0 | 0 | 0 | 0 (5 n/a) | **2** | 2 |
| ptz | 18 | 5 | 1 | 2 | 0 | 0 | **8** | 18 |
| recording | 15 | 0 | 0 | 12 | 3 | 0 | **12** | 15 |
| **total** | **148** | **27** | **1** | **20** | **16** | **13** | **48** | **132** |

Suggested agent-sized batches for the locked scope, chosen so no two agents share
a file (C11) — note `media` and `media2` **must not** be split across agents,
because three Media1 setters have their tests in `media2_tests.rs`:

1. **recording** — 12 hollow upgrades, one file, mechanically uniform. Largest
   single win and the one §2.2 calls most dangerous. Do this first.
2. **media2 + media** — 19 (11 zero-positive + 8), one agent, two files that are
   entangled.
3. **ptz** — 8 (5 zero-positive + 1 weak + 2 hollow). Whole service currently has
   **zero** compliant methods; §2.2's claim confirmed.
4. **device + mod** — 8 (4 zero-positive + 2 hollow + 2 builder positives).
5. **events + imaging** — 1 (`event_stream`'s hollow negative). Trivial; fold into
   another batch.

---

## 4. Ambiguities resolved, and how

1. **`matches!(err, OnvifError::Soap(_))` — hollow or real?** 40 % of all negatives
   in the crate are this shape. It asserts a variant, so it is not literally
   "hollow" under the task's wording; it asserts no payload, so it is not a real
   negative either. **Resolved:** classified `hollow`, sub-flavoured
   `[outer-variant-only]` in every evidence cell, and counted separately (16) from
   §2.2's `[is_err-only]` class (20). This keeps §2.2's locked scope reproducible
   while making the larger population visible. Anyone preferring the lenient
   reading can move all 16 to `yes` and recompute from the table without re-reading
   source.
2. **`get_storage_configurations`** asserts `SoapError::MissingField(_)` — two
   variant levels, still no field string. Sub-flavoured
   `[inner-variant-no-payload]`. It is one `assert_eq!` away from compliant and is
   the cheapest single upgrade in the ledger.
3. **Session-only tests.** `ptz_stop`, and extra positives for `get_osd_options`,
   live in `session_tests.rs`. `OnvifSession` delegates 1:1, so these *do* exercise
   the client method — not double-counted, but credited where they assert an
   outcome. `test_missing_*_url_returns_error` (8 tests) are **not** credited to any
   client method: they assert `OnvifSession`'s URL resolver fails before the client
   is reached.
4. **`mock_workflow.rs` assertions.** Unlike the snapshot net, it *does* assert
   outcomes (`ptz_get_presets` non-empty, `get_snapshot_uri` non-empty). Not
   credited as positives: it is gated `#![cfg(feature = "mock-server")]`, so a plain
   `cargo test` never compiles it, and it drives `OnvifSession` end-to-end rather
   than pinning a request or a parse. Recorded in the evidence column so Stage 4
   knows a partial net exists.
5. **`set_video_encoder_configuration`'s negative** is an `InvalidArgument` client-side
   gate (H265-via-Media1), not a device error. It asserts variant + message content,
   so it counts as a real negative — but it never reaches the transport. Flagged
   because a reader looking for a SOAP-Fault negative will not find one.
6. **Session mirror gaps (side observation, as asked).** Measured, not estimated:
   `src/session.rs` exposes 149 `pub` fns and mirrors **144 of the 148** client
   methods. Exactly **four have no session mirror**:
   - `get_capabilities` — the one substantive gap, and deliberate: the session
     fetches capabilities once in `build()` and exposes the cached value through
     `capabilities()`, so re-fetching is not offered.
   - `new`, `with_utc_offset`, `device_url` — superseded by the session's own
     builder (`builder()`/`build()`/`with_clock_sync()`).

   `with_credentials` and `with_transport` **are** mirrored, on the builder.
   `search_recordings` and `event_stream` are both mirrored. The 5 session-only
   names are `builder`, `build`, `with_clock_sync`, `client`, `capabilities`.
   Note that `with_utc_offset` and `device_url` are therefore untested *and*
   unmirrored — they are reachable only through `OnvifClient` directly.

---

## 5. `not-applicable` list

Methods that cannot have a negative test. **These must not appear in the Stage 4
work set as negatives**, and three of them are already fully compliant.

| method | why no negative is possible | still needs a positive? |
|---|---|---|
| `OnvifClient::new` | infallible constructor, returns `Self`; no fallible input | no — exercised by ~250 tests |
| `with_credentials` | infallible builder, returns `Self` | no — `test_credentials_add_ws_security_header` asserts the emitted header |
| `with_transport` | infallible builder, returns `Self` | no — 248 call sites; every mock assertion depends on it |
| `with_utc_offset` | infallible builder, returns `Self` | **yes** — zero call sites; needs a test asserting the offset reaches `<wsu:Created>` |
| `device_url` | infallible `&str` getter | **yes** — zero unit-test call sites; a one-line round-trip assertion |

No method's only failure mode is transport-level, so nothing else qualifies for
this list. `delete_users`'s sole negative *is* transport-level
(`ErrorTransport { status: 500 }`) but the method can also return SOAP faults and
parse errors, so it stays in the work set.
