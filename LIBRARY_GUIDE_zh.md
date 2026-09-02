# oxvif 函式庫與功能指南

[English](LIBRARY_GUIDE.md) | **繁體中文**

本文件詳細說明 oxvif Rust 函式庫、ONVIF service 操作介面、健康檢查、Mock 裝置與 Metamorph 工具。若只需要安裝方式與最短的 client 建立流程，請先閱讀[專案 README](README_zh.md)。

## 快速導覽

| 章節 | 內容 |
| --- | --- |
| [架構概覽](#架構概覽) | Discovery、session、client 與各 service family 的關係 |
| [快速開始](#快速開始) | 最小 `OnvifSession`、direct client 與 mock 範例 |
| [安裝](#安裝) | Library dependency 與未發布版本指引 |
| [命令列介面](#命令列介面) | CLI 概覽與完整手冊連結 |
| [Serde 支援](#serde-支援serde-feature) | Response 與 discovery type 的 JSON serialization |
| [`OnvifSession`](#onvifsession) / [`OnvifClient`](#onvifclient) | Service URL cache、builder、constructor 與 accessor |
| [WS-Discovery](#ws-discovery) | Multicast discovery 與結果欄位 |
| [Device 與 Media](#device-service-方法) | Device、Media1、Media2 與 audio 操作 |
| [PTZ、Imaging 與 OSD](#ptz-方法) | 攝影機移動、影像與 overlay configuration API |
| [Events 與 Recording](#events-service-方法) | Pull event、recording、search 與 replay |
| [健康檢查](#健康檢查health-feature) | 診斷行為、選項與報告解讀 |
| [錯誤處理](#錯誤處理) | Typed failure 與處理方式 |
| [無實機測試](#不使用實機進行測試) | Mock transport、mock server 與 fault injection |
| [Metamorph](#metamorphmetamorph--metamorph-server-feature) | Clone、replay、compare 與 adapter |
| [內建範例](#執行內建範例) | Repository 中可執行範例的命令 |
| [已實作操作](#已實作的-onvif-操作) | 各 service 的 operation coverage table |

---

## 架構概覽

oxvif 是用於 IP 攝影機（Profile S/T/G）的非同步 [ONVIF](https://www.onvif.org/) client，涵蓋 discovery、device、media、PTZ、imaging、events 與 recording workflow。

```text
UDP multicast ──► discovery::probe() ──► Vec<DiscoveredDevice>
                                                  │
                                                  ▼ XAddr
                      OnvifSession ─── caches service URLs, delegates every call
                           │
SOAP/HTTP ──────►  OnvifClient ──► Device    (capabilities, hostname, NTP, reboot)
                               ──► Media1    (profiles, RTSP/snapshot URIs, video + audio configs)
                               ──► Media2    (H.265, metadata, audio, video source modes)
                               ──► PTZ       (move, stop, presets, home, status, configurations, nodes)
                               ──► Imaging   (brightness, contrast, exposure, IR cut, focus move/stop)
                               ──► OSD       (create, read, update, delete on-screen display elements)
                               ──► Events    (subscribe, pull, renew, unsubscribe, continuous stream)
                               ──► Recording (list, create/delete recordings and recording jobs)
                               ──► Search    (find recordings by time/scope)
                               ──► Replay    (RTSP URI for playback)
```

主要特性：

- 以 `tokio` 與 `reqwest` 為基礎的 async-first API。
- WS-Security `UsernameToken` / `PasswordDigest` 與 HTTP Digest Authentication（RFC 7616）。
- 透過 UDP multicast `239.255.255.250:3702` 執行 WS-Discovery。
- 可替換 transport，並提供 `mock` / `mock-server` feature 供無實機測試。
- `metamorph` / `metamorph-server` 可錄製並離線重播實機行為、比較 response shape，或將非 ONVIF 裝置包裝成 ONVIF。
- 不使用 unsafe code；XML 由 pure Rust `quick-xml` 解析。
- `health` feature 提供可程式化的裝置健康檢查與 parse-coverage 偵測。

---

## 快速開始

oxvif 提供兩種主要使用方式，請依路由控制需求選擇。

### `OnvifSession`：自動快取 service URL

```rust
use oxvif::{OnvifSession, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password")
        .with_clock_sync()
        .build()
        .await?;

    let profiles = session.get_profiles().await?;
    let uri = session.get_stream_uri(&profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

### `OnvifClient`：由呼叫端管理 service URL

```rust
use oxvif::{OnvifClient, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let client = OnvifClient::new("http://192.168.1.100/onvif/device_service")
        .with_credentials("admin", "password");

    let caps = client.get_capabilities().await?;
    let media_url = caps.media.url.unwrap();
    let profiles = client.get_profiles(&media_url).await?;
    let uri = client.get_stream_uri(&media_url, &profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

`OnvifSession` 在 `build()` 時呼叫一次 `GetCapabilities` 並快取 service URL，各方法不需要 URL 參數。`OnvifClient` 為 stateless，呼叫端需自行傳入 URL，因此可完整控制每次呼叫的路由。

### 測試：不需要攝影機

```toml
[dev-dependencies]
oxvif = { version = "0.15", features = ["mock"] }
```

```rust
use std::sync::Arc;
use oxvif::{OnvifClient, mock::MockTransport};

#[tokio::test]
async fn talks_to_a_mock_camera() {
    let client = OnvifClient::new("http://mock")
        .with_transport(Arc::new(MockTransport::new()));

    client.set_hostname("lab-cam").await.unwrap();
    let h = client.get_hostname().await.unwrap();
    assert_eq!(h.name.as_deref(), Some("lab-cam"));
}
```

需要實際 bound port 時，請啟用 `mock-server` 並使用 `MockServer::start()`。

---

## 安裝

```toml
[dependencies]
oxvif = "0.15"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

`develop` 分支正在準備 oxvif 0.16.0。在該版本公開並完成獨立驗證前，crates.io consumer 應使用 0.15，或以明確 source revision 評估 0.16。

---

## 命令列介面

Workspace 內另有可獨立發布的 `oxvif-cli` package，安裝後的 executable 名稱為 `oxvif`。它為 terminal user 與 Agent 提供唯讀 ONVIF 診斷及 fleet 管理能力，包括 named device、discovery snapshot、Group、View、原生 OS credential storage、deterministic JSON/JSONL、typed error 與穩定 exit code。

安裝、命令、安全行為、自動化契約與 exit code 請參閱[完整 CLI 指南](docs/oxvif-cli_zh.md)。0.1 尚未公開發布；目前可由 checkout 安裝評估版本：

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --help
oxvif agent guide --output json
oxvif setup front-door 192.168.1.100 --name "Front Door" --tag entrance
oxvif --device front-door device info --output json --non-interactive
```

Registry 不儲存密碼。持久化憑證使用 Windows Credential Manager、macOS Keychain 或 Linux Secret Service；自動化可使用 `--password-stdin` 或受信任的 process environment。私有 HTTPS 裝置可重複指定 `--ca-certificate <FILE>` PEM bundle，且不會停用 certificate-chain 或 hostname verification。

APT 與 Homebrew packaging path 已通過不發布的三平台 staging，但目前尚無公開 APT repository 或 Homebrew tap。正式安裝來源只會在公開並驗證後列入 README。

---

## Serde 支援（`serde` feature）

啟用 `serde` 後，`oxvif::types` 的所有 response type，以及 `discovery::probe` 等 API 回傳的 `DiscoveredDevice`、`DiscoveryEvent`，都會 derive `Serialize` 與 `Deserialize`。

```toml
[dependencies]
oxvif = { version = "0.15", features = ["serde"] }
```

```rust
let profiles = session.get_profiles().await?;
println!("{}", serde_json::to_string_pretty(&profiles)?);

let round_tripped: Vec<oxvif::MediaProfile> = serde_json::from_str(&json)?;
```

Field name 使用 Rust 原生 `snake_case`，未設定 `rename_all`。此 feature 為 opt-in，停用時不增加 dependency 成本。

---

## `OnvifSession`

`OnvifSession` 建立時呼叫 `GetCapabilities`，在內部快取 service URL，並將所有 operation 暴露為不需 URL 參數的方法。

### 建立 session

```rust
let session = OnvifSession::builder("http://192.168.1.100/onvif/device_service")
    .with_credentials("admin", "password")
    .with_clock_sync()
    .build()
    .await?;

let caps = session.capabilities();
let profiles = session.get_profiles().await?;
let uri = session.get_stream_uri(&profiles[0].token).await?;
```

| 方法 | 說明 |
|---|---|
| `OnvifSession::builder(device_url)` | 建立 session builder |
| `.with_credentials(username, password)` | 啟用 WS-Security `UsernameToken` |
| `.with_clock_sync()` | 先呼叫 `GetSystemDateAndTime` 並套用 UTC offset，避免 clock skew 導致驗證失敗 |
| `.with_transport(transport)` | 替換 HTTP transport，適合單元測試 |
| `.build().await` | 建立連線、選擇性同步時間、取得 capabilities 並回傳 session |
| `session.capabilities()` | 回傳快取的 `&Capabilities`，不執行網路呼叫 |
| `session.client()` | 存取底層 `&OnvifClient` |

---

## `OnvifClient`

`OnvifClient` 為 stateless 且 clone 成本低，可由 `Arc` 跨 thread 共用。Service URL 由呼叫端透過 `get_capabilities()` 或 `get_services()` 取得並管理。

| 方法 | 說明 |
|---|---|
| `OnvifClient::new(device_url)` | 使用 device service URL 建立 client |
| `.with_credentials(username, password)` | 啟用 WS-Security `UsernameToken` |
| `.with_utc_offset(offset_secs: i64)` | 調整 WS-Security timestamp |
| `.with_transport(Arc<dyn Transport>)` | 替換預設 HTTP transport |

```rust
let client = OnvifClient::new("http://192.168.1.100/onvif/device_service");
let dt = client.get_system_date_and_time().await?;
let client = client
    .with_credentials("admin", "secret")
    .with_utc_offset(dt.utc_offset_secs());
```

---

## WS-Discovery

```rust
use std::time::Duration;
use oxvif::discovery;

let devices = discovery::probe(Duration::from_secs(3)).await;
for d in &devices {
    println!("Found: {}", d.endpoint);
    for addr in &d.xaddrs {
        println!("  XAddr: {addr}");
    }
}
```

| 欄位 | 類型 | 說明 |
|---|---|---|
| `endpoint` | `String` | 唯一 endpoint URN |
| `types` | `Vec<String>` | WS-Discovery type |
| `scopes` | `Vec<String>` | ONVIF name、location、hardware 等 scope |
| `xaddrs` | `Vec<String>` | Device service URL，可傳入 `OnvifClient::new` |

`probe` 發生 I/O error 時回傳空 `Vec`，不會 panic。

---

## Device Service 方法

### Capabilities 與服務探索

`get_capabilities()` 應作為第一個呼叫，用來取得 service endpoint 與 feature flag。每個 service 另有自身的 `GetServiceCapabilities`：

| 方法 | 回傳型別 |
|---|---|
| `device_get_service_capabilities()` | `DeviceServiceCapabilities` |
| `media_get_service_capabilities(media_url)` | `MediaServiceCapabilities` |
| `media2_get_service_capabilities(media2_url)` | `Media2ServiceCapabilities` |
| `ptz_get_service_capabilities(ptz_url)` | `PtzServiceCapabilities` |
| `imaging_get_service_capabilities(imaging_url)` | `ImagingServiceCapabilities` |
| `events_get_service_capabilities(events_url)` | `EventsServiceCapabilities` |
| `recording_get_service_capabilities(recording_url)` | `RecordingServiceCapabilities` |
| `search_get_service_capabilities(search_url)` | `SearchServiceCapabilities` |
| `replay_get_service_capabilities(replay_url)` | `ReplayServiceCapabilities` |

Service capability 中的 flag 使用 `Option<bool>`：`None` 表示裝置未宣告，`Some(false)` 表示明確不支援，不應合併處理。List-valued attribute 使用 `Vec<_>`，缺少時為空 list。

若 `caps.media2_url` 等欄位為 `None`，請以 `get_services()` 尋找對應的 `OnvifService`。`OnvifSession` 會自動以此方式補齊 Recording、Search、Replay 與 DeviceIO URL。

### 裝置管理方法

| 方法 | 說明 |
|---|---|
| `get_system_date_and_time()` / `set_system_date_and_time(req)` | 讀寫裝置時間、timezone 與 DST |
| `get_device_info()` | 取得 manufacturer、model、firmware、serial number |
| `get_hostname()` / `set_hostname(name)` | 讀寫 hostname |
| `get_ntp()` / `set_ntp(from_dhcp, servers)` | 讀寫 NTP 設定 |
| `system_reboot()` | 啟動重開機並回傳資訊訊息 |
| `get_scopes()` / `set_scopes(scopes)` | 讀取或替換 configurable scope |
| `get_users()` | 列出帳號與 access level |
| `create_users(users)` / `delete_users(usernames)` / `set_user(...)` | 帳號管理 |
| `get_network_interfaces()` / `set_network_interfaces(...)` | 網路介面與 IPv4 設定 |
| `get_network_protocols()` / `set_network_protocols(...)` | HTTP、HTTPS、RTSP 與 port 設定 |
| `get_dns()` / `set_dns(...)` | DNS 與 DHCP 設定 |
| `get_network_default_gateway()` / `set_network_default_gateway(...)` | IPv4 default gateway |
| `get_discovery_mode()` / `set_discovery_mode(mode)` | `Discoverable` / `NonDiscoverable` |
| `get_system_log(log_type)` | 讀取 `System` 或 `Access` log |
| `get_system_uris()` | 取得 support、backup 等 download URI |
| `set_system_factory_default(default_type)` | `Hard` 或 `Soft` factory reset |
| `start_firmware_upgrade()` / `start_system_restore()` | 取得 upload URI 與時間資訊 |
| `get_relay_outputs()` / `set_relay_output_state(...)` / `set_relay_output_settings(...)` | Relay output 操作 |
| `get_digital_inputs(deviceio_url)` | DeviceIO endpoint 的 digital input |
| `get_storage_configurations()` / `set_storage_configuration(...)` | SD/NAS storage configuration |

`get_digital_inputs` 必須使用 DeviceIO endpoint。若 `Capabilities.device_io.url` 為 `None`，請由 `get_services()` 尋找 `OnvifService::is_device_io()`；`OnvifSession::get_digital_inputs()` 會自動執行 fallback。

---

## Media Service（Media1）方法

所有 Media1 方法均使用 `caps.media.url`。

### Profile 與 stream

| 方法 | 回傳 | 說明 |
|---|---|---|
| `get_profiles(media_url)` | `Vec<MediaProfile>` | 列出全部 profile |
| `get_profile(media_url, token)` | `MediaProfile` | 依 token 取得 profile |
| `create_profile(media_url, name, token)` | `MediaProfile` | 建立空 profile |
| `delete_profile(media_url, token)` | `()` | 刪除非 fixed profile |
| `add_video_encoder_configuration(...)` / `remove_video_encoder_configuration(...)` | `()` | 綁定／解除 encoder configuration |
| `add_video_source_configuration(...)` / `remove_video_source_configuration(...)` | `()` | 綁定／解除 source configuration |
| `get_stream_uri(media_url, profile_token)` | `StreamUri` | 取得 RTSP URI |
| `get_snapshot_uri(media_url, profile_token)` | `SnapshotUri` | 取得 snapshot URI |

### Video configuration

| 方法 | 說明 |
|---|---|
| `get_video_sources(media_url)` | Physical video input |
| `get_video_source_configurations(media_url)` / `get_video_source_configuration(media_url, token)` | 讀取 crop/position configuration |
| `set_video_source_configuration(media_url, config)` | 寫回 video source configuration |
| `get_video_source_configuration_options(media_url, token)` | 有效 bounds range |
| `get_video_encoder_configurations(media_url)` / `get_video_encoder_configuration(media_url, token)` | 讀取 codec、resolution、bitrate configuration |
| `set_video_encoder_configuration(media_url, config)` | 寫回 encoder configuration |
| `get_video_encoder_configuration_options(media_url, token)` | 有效 resolution、bitrate、fps range |

```rust
let mut enc = client.get_video_encoder_configuration(media_url, &token).await?;
if let Some(rc) = enc.rate_control.as_mut() {
    rc.bitrate_limit = 2048;
}
client.set_video_encoder_configuration(media_url, &enc).await?;
```

---

## Media2 方法

Media2 是 Media1 的後繼介面，原生支援 H.265，且 encoder configuration 結構較扁平。所有方法使用 `media2_url`。

| 特性 | Media1 | Media2 |
|---|---|---|
| H.265 | `Other(String)` | `VideoEncoding::H265` |
| Encoder config | Nested `H264`/`H265` struct | `gov_length` 與 `profile` 是 top-level field，wire 上為 attribute |
| `GetStreamUri` response | `<MediaUri>` wrapper | `<Uri>` string |
| Write | 需要 `<ForcePersistence>true` | 無 `ForcePersistence` |

| 方法 | 回傳／用途 |
|---|---|
| `get_profiles_media2(url)` | `Vec<MediaProfile2>` |
| `create_profile_media2(url, name)` / `delete_profile_media2(url, token)` | 建立／刪除 profile |
| `get_stream_uri_media2(url, token)` / `get_snapshot_uri_media2(url, token)` | Stream / snapshot URI |
| `get_video_source_configurations_media2(url)` / `set_video_source_configuration_media2(url, config)` | Video source configuration |
| `get_video_source_configuration_options_media2(url, token)` | Video source options |
| `get_video_encoder_configurations_media2(url)` / `get_video_encoder_configuration_media2(url, token)` | H.265-capable encoder configuration |
| `set_video_encoder_configuration_media2(url, config)` | 寫入 encoder configuration |
| `get_video_encoder_configuration_options_media2(url, token)` | Encoder options |
| `get_video_encoder_instances_media2(url, config_token)` | Encoder capacity |
| `add_configuration_media2(...)` / `remove_configuration_media2(...)` | 綁定／解除 profile configuration |
| `get_metadata_configurations_media2(...)` / `set_metadata_configuration_media2(...)` | Metadata configuration |
| `get_metadata_configuration_options_media2(...)` | Metadata options |
| `get_audio_source_configurations_media2(url)` | Audio source configuration |
| `get_audio_encoder_configurations_media2(url)` / `set_audio_encoder_configuration_media2(url, config)` | Audio encoder configuration |
| `get_audio_encoder_configuration_options_media2(url, token)` | Audio options |
| `get_audio_output_configurations_media2(url)` / `get_audio_decoder_configurations_media2(url)` | Audio output / decoder |
| `get_video_source_modes_media2(url, source_token)` / `set_video_source_mode_media2(...)` | Sensor mode；setter 回傳是否需要 reboot |

---

## PTZ 方法

所有 PTZ 方法使用 `caps.ptz_url`。ONVIF normalized range 通常為 pan/tilt `[-1.0, 1.0]`、zoom `[0.0, 1.0]`。

| 方法 | 說明 |
|---|---|
| `ptz_absolute_move(...)` / `ptz_relative_move(...)` / `ptz_continuous_move(...)` | Absolute、relative 或 continuous movement |
| `ptz_stop(ptz_url, profile_token)` | 停止移動 |
| `ptz_get_presets(...)` / `ptz_goto_preset(...)` | 列出／前往 preset |
| `ptz_set_preset(...)` / `ptz_remove_preset(...)` | 儲存／刪除 preset |
| `ptz_get_status(...)` | Position 與 move state |
| `ptz_get_configurations(...)` / `ptz_get_configuration(...)` | PTZ configuration |
| `ptz_set_configuration(...)` / `ptz_get_configuration_options(...)` | 寫入 configuration 與查詢 options |
| `ptz_get_nodes(...)` / `ptz_get_compatible_configurations(...)` | Node capability 與 profile-compatible configuration |
| `ptz_goto_home_position(...)` / `ptz_set_home_position(...)` | Home position |
| `ptz_get_preset_tours(...)` / `ptz_get_preset_tour(...)` | Preset tour |
| `ptz_get_preset_tour_options(...)` | Tour capability |
| `ptz_create_preset_tour(...)` / `ptz_modify_preset_tour(...)` | 建立／修改 tour |
| `ptz_operate_preset_tour(...)` / `ptz_remove_preset_tour(...)` | 啟停、暫停或刪除 tour |
| `ptz_send_auxiliary_command(...)` | 依 profile 執行 wiper、washer、IR lamp 等命令 |

`PtzPresetTour::token` 為 `Option<String>`，符合 schema 中 optional `@token`。`PtzPresetTourPresetDetail` 是 enum，表示 `PresetToken`、`Home` 或 explicit `Position` 三選一。State 與 direction enum 均保留 `Unknown(String)`，以承接廠商 extension。

PTZ service 的 `ptz_send_auxiliary_command(ptz_url, profile_token, data)` 與 Device service 的 `send_auxiliary_command(command)` 是不同 ONVIF operation。接受值由廠商定義，應先由 `device_get_service_capabilities().misc.auxiliary_commands` 探索。

---

## Audio Service 方法

| 方法 | 回傳／用途 |
|---|---|
| `get_audio_sources(media_url)` | `Vec<AudioSource>` physical input |
| `get_audio_source_configurations(media_url)` | Source configuration |
| `get_audio_encoder_configurations(media_url)` / `get_audio_encoder_configuration(media_url, token)` | Encoder configuration |
| `set_audio_encoder_configuration(media_url, config)` | 寫回 configuration |
| `get_audio_encoder_configuration_options(media_url, token)` | Encoding、bitrate 與 sample-rate option |

`AudioEncoding` variant 為 `G711`、`G726`、`Aac` 與 `Other(String)`。

---

## Imaging Service 方法

所有方法使用 `caps.imaging_url`，並要求 `video_source_token`。

| 方法 | 說明 |
|---|---|
| `get_imaging_settings(...)` / `set_imaging_settings(...)` | Brightness、contrast、IR cut、white balance、exposure 等設定 |
| `get_imaging_options(...)` | 各設定的有效範圍 |
| `imaging_get_status(...)` / `imaging_get_move_options(...)` | Focus position、state 與 move range |
| `imaging_move(..., FocusMove)` / `imaging_stop(...)` | Absolute、relative、continuous focus movement 與停止 |

---

## OSD Service 方法

OSD 用於在 video stream 上疊加文字或影像，並使用 Media1 URL。

| 方法 | 回傳／用途 |
|---|---|
| `get_osds(media_url, config_token)` | `Vec<OsdConfiguration>`；`None` 表示全部 |
| `get_osd(media_url, osd_token)` | 單一 OSD |
| `set_osd(media_url, osd)` | 更新 OSD |
| `create_osd(media_url, osd)` | 建立並回傳 token |
| `delete_osd(media_url, osd_token)` | 刪除 OSD |
| `get_osd_options(media_url, config_token)` | 支援的 type 與 position option |

`OsdConfiguration` 包含 `token`、`video_source_config_token`、`type_`、`position`、可選 `text_string` 與 `image_path`。`OsdTextString` 可設定 plain text、日期／時間格式、font size、顏色與背景。

---

## Events Service 方法

ONVIF Events 使用 pull-point subscription model：

```rust
let props = client.get_event_properties(&events_url).await?;
let sub = client.create_pull_point_subscription(
    &events_url, None, Some("PT60S")
).await?;
let msgs = client.pull_messages(&sub.reference_url, "PT5S", 50).await?;
let new_time = client.renew_subscription(&sub.reference_url, "PT60S").await?;
client.unsubscribe(&sub.reference_url).await?;
```

`set_synchronization_point(subscription_url)` 要求裝置重新傳送各 property topic 的目前狀態。`event_stream` 將 polling loop 包裝為無限 async `Stream`；請以 `StreamExt::take`、`select!` 或 cancellation 限制生命週期，結束時呼叫 `unsubscribe`。

`NotificationMessage` 包含 `topic`、`utc_time`、`source: HashMap<String, String>` 與 `data: HashMap<String, String>`。

---

## Recording Service 方法

Recording、Search 與 Replay URL 通常由 `get_services()` 取得。

| 方法 | 回傳／用途 |
|---|---|
| `get_recordings(recording_url)` | `Vec<RecordingItem>` |
| `create_recording(recording_url, config)` / `delete_recording(...)` | 建立／刪除 recording |
| `create_track(...)` / `delete_track(...)` | 建立／刪除 track |
| `get_recording_jobs(recording_url)` | `Vec<RecordingJob>` |
| `create_recording_job(...)` / `delete_recording_job(...)` | 建立／刪除 job |
| `set_recording_job_mode(...)` | 設定 `Active` 或 `Idle` |
| `get_recording_job_state(...)` | 讀取 active state |

`RecordingItem` 包含 token、source、content 與 track；track 可為 `Video`、`Audio` 或 `Metadata`，並可含 ISO-8601 time bound。

---

## Search 與 Replay Service 方法

| 方法 | 說明 |
|---|---|
| `find_recordings(search_url, max_matches, keep_alive)` | 啟動非同步搜尋並回傳 search token |
| `get_recording_search_results(search_url, token, max_results, wait_time)` | 輪詢至 `search_state == "Completed"` |
| `end_search(search_url, token)` | 釋放 search session |
| `get_replay_uri(replay_url, recording_token, stream_type, protocol)` | 取得錄影播放 RTSP URI |

---

## 健康檢查（`health` feature）

`HealthCheck` 會產生 Pass/Warn/Fail/Skip 報告與 Profile S/T/G verdict。預設為唯讀，且無法連線的裝置會成為 failing `connect` check，而不會使整個 fleet run 回傳 `Err`。

```bash
cargo run --example healthcheck --features health,mock-server -- --mock
```

```rust
use oxvif::health::HealthCheck;

let report = HealthCheck::new("http://192.168.1.100/onvif/device_service")
    .with_credentials("admin", "password")
    .run()
    .await;
println!("{report}");
if !report.ok() {
    std::process::exit(1);
}
```

| Builder | 增加的檢查 |
|---|---|
| `.with_credentials(user, pass)` | WS-Security 與 HTTP Digest |
| `.with_clock_sync(true)` | 先同步裝置時間 |
| `.with_liveness_probes(true)` | 實際抓取 snapshot、連線 RTSP port、執行 Profile G flow |
| `.with_write_checks(true)` | 執行一次非破壞性 write round-trip |
| `.with_force_unsupported(true)` | 探測裝置未公告的常見 service URL |
| `.with_capture(true)` | 保存失敗 SOAP exchange，並移除 credential material |

`report.to_json()` / `to_json_pretty()` 可供 CI 使用，`report.diff(&previous)` 可比較前次 baseline。`CheckResult::error` 提供 structured `ErrorClass`、ONVIF subcode、fault code、reason 與 detail；`ProfileAssessment` 將 genuine failure 與無法驗證區分為 `missing` 與 `unverified`。

啟用 liveness probe 後，stream URI 會進行 RTSP `OPTIONS`，snapshot URI 會實際抓取並驗證 image bytes，Profile G 會執行 recording search 與 replay URI resolution。這些行為會建立額外連線，因此預設關閉。

九項 `service_caps_*` check 會詢問各 service 的 `GetServiceCapabilities`；`service_caps_self_consistent` 比較 device-level 與 service-level 重複宣告的 24 個 attribute。Device-level 為 `true` 而 service-level 為 `false` 時回報 contradiction `Warn`；反向情況只計數不警告，因為 device-level 缺少 element 也可能被解析為 `false`。

Parse coverage 會比較 list operation 的原始 XML item 數與 parser 結果數，可偵測因 element name 錯誤造成的 silent drop。Scalar field defaulting 仍應使用 `conformance` 範例搭配實機驗證：

```sh
cargo run --example conformance --features mock -- devices.txt
```

---

## 錯誤處理

所有 API 方法回傳 `Result<T, OnvifError>`：

```rust
pub enum OnvifError {
    Transport(TransportError),
    Soap(SoapError),
}
```

```rust
match client.get_capabilities().await {
    Ok(caps) => { /* use caps */ }
    Err(OnvifError::Transport(TransportError::Http(e))) => eprintln!("Network: {e}"),
    Err(OnvifError::Transport(TransportError::HttpStatus { status, body })) => {
        eprintln!("HTTP {status}: {body}");
    }
    Err(OnvifError::Soap(SoapError::Fault { code, reason })) => {
        eprintln!("SOAP Fault [{code}]: {reason}");
    }
    Err(e) => eprintln!("Other: {e}"),
}
```

HTTP 500 會先視為 transport success，讓 SOAP layer 解析 `<s:Fault>` 詳細內容。

---

## 不使用實機進行測試

內建 mock 是 stateful ONVIF 裝置，涵蓋 oxvif 實作的 157 個 SOAP action；`Set` 會持久存在記憶體，後續 `Get` 可觀察變更。

| | `MockTransport`（`mock`） | `MockServer`（`mock-server`） |
|---|---|---|
| 接線方式 | 注入 client | 連線至實際 bound port |
| 驗證範圍 | 呼叫端程式與 parser | 另包含 HTTP transport 與 WS-Security |
| 適用情境 | 單元測試 | Integration test 與外部工具 |

```rust
let mock = MockTransport::new();
mock.device().modify(|s| s.hostname = "seeded-cam".into());
mock.inject_fault("GetProfiles", "ter:NotAuthorized", "denied");

let client = OnvifClient::new("http://mock")
    .with_transport(Arc::new(mock.clone()));
assert!(client.get_profiles("http://mock/media").await.is_err());
```

兩種 mock 預設都不要求驗證。`MockTransport::with_auth()` 與 `MockServer::builder().enforce_auth(true)` 可啟用 WS-Security。完整的 fixture、錯誤與限制請參閱 [Mock ONVIF 裝置參考](docs/mock-server_zh.md)。

| API | 說明 |
|---|---|
| `MockTransport::new()` / `with_state(MockState)` | 建立程序內 transport |
| `.device()` / `.inject_fault(...)` / `.clear_faults()` | 狀態、single-shot fault 與 queue 控制 |
| `MockServer::start().await` | 在 ephemeral port 啟動 server |
| `MockServer::builder()` | 設定 port、initial state、change hook、auth 與 replay |
| `MockState::read()` / `modify(...)` / `modify_returning(...)` | 讀寫共享 `DeviceState` |
| `MockState::set_on_change(hook)` | 每次 mutation 後執行持久化 callback |

Standalone 範例預設使用 port 18080，並將 state 持久化至 `~/.oxvif/mock_device.toml`：

```sh
cargo run --example mock_server --features mock-server
```

---

## Metamorph（`metamorph` / `metamorph-server` feature）

Metamorph 可將實機的 read surface 錄製為可離線重播的 clone，也可比較 clone 與 reference mock 的 response shape，或以 ONVIF adapter 包裝非 ONVIF 裝置。`metamorph-server` 在此基礎上加入 bound-port server。

```toml
[dev-dependencies]
oxvif = { version = "0.15", features = ["metamorph"] }
```

```rust
use std::sync::Arc;
use oxvif::OnvifClient;
use oxvif::metamorph::{FixtureStore, MetamorphTransport, record_standard_surface};

let clone = record_standard_surface(
    "http://192.168.1.100/onvif/device_service",
    Some(("admin", "password")),
    "hikvision-ds2cd",
).await?;
clone.save("clones/hikvision-ds2cd")?;

let store = FixtureStore::load("clones/hikvision-ds2cd")?;
let client = OnvifClient::new("http://replay")
    .with_transport(Arc::new(MetamorphTransport::new(store)));
let info = client.get_device_info().await?;
```

保存的 clone 會移除 WS-Security `Password` / `Nonce` 與 URL 中的 `user:pass@`。`record_surface` 可配合 `SurfaceSelection` 只錄製指定 service group，並以 `SweepReport` 回報 recorded、failed 與 skipped operation。Replay read 會逐 byte 使用錄製 response；write 則轉入 synthetic `DeviceState` 並使同 family fixture 失效，使 `Set → Get` 仍可 round-trip。

`FixtureStore::diff_against_synthetic()` 比較 element-path set，結果表示與 oxvif reference mock 的結構差異，**不是 ONVIF schema conformance verdict**。`verify_parsing().await` 則使用 oxvif typed parser，將每項 fixture 分類為 `Parsed`、`Failed`、`Faulted` 或 `Unverified`；`failures()` 刻意排除裝置正常拒絕操作的 `Faulted`。

長時間 sweep 可使用 `_with_progress` variant。`SweepProgress::total` 計算展開 prerequisite 後的 operation 數，而不是 HTTP request 數；每個 operation 不論執行或 skip 都只推進一次。

`DeviceAdapter` trait 可替 RTSP-only 等非 ONVIF 裝置提供 ONVIF skin。只需實作 `identity` 與 `stream_uri`；profile、capabilities 與 services 由 synthetic mock 補足，`continuous_move` 與 `snapshot` 為可選 hook。

---

## 執行內建範例

```sh
cp .env.example .env
cargo run --example camera -- full-workflow
cargo run --example camera -- session
cargo run --example camera -- device-info
cargo run --example camera -- stream-uris
cargo run --example camera -- ptz-presets
cargo run --example camera -- imaging-focus
cargo run --example camera -- events
cargo run --example camera -- recording
cargo run --example camera -- network-config
cargo run --example camera -- healthcheck
```

也可直接指定裝置：

```sh
cargo run --example camera -- --ip 192.168.1.100 --auth admin:password device-info
```

Mock、conformance 與 Metamorph：

```sh
cargo run --example mock_server --features mock-server
cargo run --example conformance --features mock -- devices.txt
cargo run --example metamorph_record --features metamorph -- \
    http://192.168.1.100/onvif/device_service admin password clones/hikvision-ds2cd
cargo run --example metamorph_serve --features metamorph-server -- clones/hikvision-ds2cd
cargo run --example metamorph_adapter --features metamorph
```

含憑證的 device list 與 `.env` 不得提交至版本控制。

---

## 專案結構

| 路徑 | 職責 |
|---|---|
| `src/client/` | Device、Media1/2、PTZ、Imaging、Events、Recording client method |
| `src/session.rs` | 快取 service URL 的 `OnvifSession` |
| `src/discovery.rs` | WS-Discovery UDP multicast probe |
| `src/transport.rs` | `Transport` trait 與 `HttpTransport` |
| `src/soap/` | SOAP envelope、security、XML parser 與 error |
| `src/types/` | 各 service 的 public request / response type |
| `src/mock/` | 程序內與 bound-port mock ONVIF 裝置 |
| `src/health/` | 健康與 conformance 自我診斷 |
| `src/metamorph/` | Fixture recording、replay、adapter 與 quirk diff |
| `examples/` | 實機、mock、health、conformance 與 Metamorph 範例 |

---

## 已實作的 ONVIF 操作

請參閱 [OPERATIONS_zh.md](OPERATIONS_zh.md)，其中依九項 service 與 WS-Discovery 列出完整涵蓋範圍。本文件前述各節則提供方法簽章、語意與使用範例。

---

## Changelog

版本歷程請參閱 [CHANGELOG.md](CHANGELOG.md)。

## 貢獻、安全性與支援

本機 gate 與實機資料遮蔽規則請參閱 [CONTRIBUTING.md](CONTRIBUTING.md)；安全性問題請依 [SECURITY.md](SECURITY.md) 進行非公開回報；社群互動應遵循 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。受支援版本、平台與相容性聲明限制定義於[支援政策](docs/support_zh.md)。

## 授權

MIT，適用於 oxvif 自有原始碼。

## 商標與 ONVIF 聲明

ONVIF® 是 ONVIF, Inc. 的商標。oxvif 是獨立社群專案，**與 ONVIF 無隸屬、背書或認證關係**。「ONVIF」名稱僅用於說明本專案實作的通訊協定。oxvif 未通過 ONVIF conformance program，也不提出 ONVIF Profile conformance 聲明；`health` 與 `conformance` 是非官方自我診斷工具，不是官方 ONVIF Device Test Tool。
