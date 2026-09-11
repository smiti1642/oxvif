# oxvif 函式庫與功能指南

[English](LIBRARY_GUIDE.md) | **繁體中文**

本文件詳細說明 oxvif Rust 函式庫、ONVIF service 操作介面、健康檢查、Mock 裝置與 Metamorph 工具。若只需要安裝方式與最短的 client 建立流程，請先閱讀[專案 README](README_zh.md)。

## 快速導覽

| 章節 | 內容 |
| --- | --- |
| [架構概覽](#架構概覽) | Discovery、session、client 與各 service family 的關係 |
| [快速開始](#快速開始) | 最小 `OnvifSession`、direct client 與 mock 範例 |
| [安裝](#安裝) | Library dependency 與 feature 指引 |
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
oxvif = { version = "0.16", features = ["mock"] }
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
oxvif = "0.16"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

---

## 命令列介面

Workspace 內另有可獨立發布的 `oxvif-cli` package，安裝後的 executable 名稱為 `oxvif`。它為 terminal user 與 Agent 提供唯讀 ONVIF 診斷及 fleet 管理能力，包括 named device、discovery snapshot、Group、View、原生 OS credential storage、deterministic JSON/JSONL、typed error 與穩定 exit code。

安裝、命令、安全行為、自動化契約與 exit code 請參閱[完整 CLI 指南](docs/oxvif-cli_zh.md)。從 crates.io 安裝 0.1 CLI：

```sh
cargo install oxvif-cli --locked
oxvif --version
```

Repository contributor 可改為安裝目前的 checkout：

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --help
oxvif agent guide --output json
oxvif setup front-door 192.168.1.100 --name "Front Door" --tag entrance
oxvif --device front-door device info --output json --non-interactive
```

Registry 不儲存密碼。持久化憑證使用 Windows Credential Manager、macOS Keychain 或 Linux Secret Service；自動化可使用 `--password-stdin` 或受信任的 process environment。私有 HTTPS 裝置可重複指定 `--ca-certificate <FILE>` PEM bundle，且不會停用 certificate-chain 或 hostname verification。

APT 與 Homebrew package 只透過專案 README 明確列出、且已完成獨立安裝驗證的 channel 發布。原生 package channel 尚未列出時，請使用 crates.io 或對應 GitHub Release 所附且可核對 checksum 的 portable artifact。

---

## Serde 支援（`serde` feature）

啟用 `serde` 後，`oxvif::types` 的所有 response type，以及 `discovery::probe` 等 API 回傳的 `DiscoveredDevice`、`DiscoveryEvent`，都會 derive `Serialize` 與 `Deserialize`。

```toml
[dependencies]
oxvif = { version = "0.16", features = ["serde"] }
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

參數應傳 literal string，不要預先 escape XML。共用序列化會 escape markup，並以
character reference 表示 CR、LF、tab 資料，避免 XML text／attribute normalization
改變字元。各欄位有效性與裝置檢查仍適用。包含這些資料字元的 bytewise fixture
需採用新的 reference 表示；公開 signature 與一般值不變。此修正不包含仍略過
共用 serializer 或使用 legacy token extraction 的 mock 路徑。

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

`media_set_synchronization_point(url, profile_token)`（亦提供 `OnvifSession`
wrapper）請求所選 profile 的關聯串流同步。Service 選擇、Mock 僅收件確認政策及
真實串流驗證限制，詳見 [Media synchronization 指南](docs/media-synchronization_zh.md)。

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

`set_synchronization_point_media2(url, profile_token)` 提供對應 Media2 請求、
Session wrapper 及獨立 Mock opt-in，不是 Events subscription 同步操作；參閱
[Media synchronization 指南](docs/media-synchronization_zh.md)。

Media2 是 Media1 的後繼介面，原生支援 H.265，且 encoder configuration 結構較扁平。所有方法使用 `media2_url`。

尚未發布：Media2 幀率限制改用 `f32`，保留小數；已提供但無效的 rate control 會回錯誤。
詳見 [Rust／JSON 遷移與 Media1 mock 政策](docs/media2-frame-rate_zh.md)。
Mock 現在原子驗證完整 encoder 寫入，並共用 options／write 限制；capacity 使用
source configuration token。詳見 [encoder 契約](docs/mock-server_zh.md#622-encoder-configuration-契約)。

| 特性 | Media1 | Media2 |
|---|---|---|
| H.265 | `Other(String)` | `VideoEncoding::H265` |
| Encoder config | Nested `H264`/`H265` struct | `gov_length` 與 `profile` 是 top-level field，wire 上為 attribute |
| `GetStreamUri` response | `<MediaUri>` wrapper | `<Uri>` string |
| Write | 需要 `<ForcePersistence>true` | 無 `ForcePersistence` |

| 方法 | 回傳／用途 |
|---|---|
| `get_profiles_media2(url)` | `Vec<MediaProfile2>`；列出 profile 與關聯 configuration（`Type=All`） |
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

尚未發布版本：請查詢目標 Media 服務的 codec options，不可直接互換編碼字串。
音訊 options 保留全部重複 Items。Metadata 改用結構化 multicast 與必要的
session timeout；舊 Rust／JSON 欄位須明確遷移。詳見[音訊／metadata 遷移](docs/audio-metadata_zh.md)。

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

### 推播通知來源

使用 `notification_listener_with_peer(bind_addr).await?`，可先完成 bind 再訂閱，
接收 `ReceivedNotification { message, peer }`。Bind 錯誤直接回傳；`message` 仍是
原本的 `NotificationMessage`。同步 `notification_listener` 保留原本 payload 與 signature。

```rust
use futures::StreamExt as _;
let mut notifications = oxvif::notification_listener_with_peer(
    "127.0.0.1:8080".parse()?
).await?;
while let Some(received) = notifications.next().await {
    println!("{}: {}", received.peer, received.message.topic);
}
```

Peer 來自 TCP socket，而非 ONVIF XML 或代理 header。Port 是此連線的來源 port，
不是攝影機服務 port。NAT／proxy 可能隱藏原始裝置，不能視為已驗證的攝影機身分。
包裝型別不 derive serde，讓位址匯出保持明確選擇，既有事件 JSON 不變。
Drop stream 會取消所屬連線；仍須另外取消裝置端訂閱。此最小 listener 不提供
TLS／認證或完整 HTTP server 安全防護。

### Pull-point 訂閱

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

啟用 liveness probe 後，stream URI 會進行 RTSP `OPTIONS`，snapshot URI 會抓取有界圖片資料並檢查檔頭（不是完整解碼），Profile G 會執行 recording search 與 replay URI resolution。這些行為會建立額外連線，因此預設關閉。

快照使用與 CLI 共用的 `health::snapshot` 核心：限同一設備主機，不重新導向、
不使用 proxy、不降級 HTTPS，限制 16 MiB 及總期限。Digest 失敗不改送 Basic；
HTTP/1.1 首字母大寫標頭改善韌體相容性，但不變更標準 Digest 參數。
可透過 `with_snapshot_options` 設定快照期限及額外 CA 根憑證；SOAP transport
另行設定。PNG／BMP 辨識為相容性擴充，不代表 ONVIF JPEG 合規或成功解碼。

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

Profile 建立遵守公告容量；Media2 初始 binding、改名及 All 選擇會原子提交，
受影響引用計數與 replay read 跟隨 commit 更新。不截斷匯入 fixture，詳見
[profile 組裝及限制](docs/mock-server_zh.md#63-profile)。

Source configuration 寫入會先驗證完整已建模值，再原子提交；options 使用 sensor 範圍，
不依目前 crop。非零原點與未建模 extension 明確拒絕，詳見
[source 設定與 selector](docs/mock-server_zh.md#621-source-configuration-契約)。

內建 mock 可路由 oxvif 實作的 159 個 SOAP action。已建模的寫入會保留於記憶體，並由對應 getter 反映；路由覆蓋不代表每項效果都已建模。已分類的 reset、auxiliary、maintenance、subscription／synchronization 及結束搜尋 stub，若僅需確認收到請求，必須明確逐項 opt-in acknowledgment-only；詳見 [mock fidelity 政策](docs/mock-server_zh.md#135-明確的-acknowledgment-only-政策)。

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
oxvif = { version = "0.16", features = ["metamorph"] }
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

錄製資料會移除支援格式的 WS-Security `Password`／`Nonce` 與 literal URL `user:pass@`。
Canonical key 也會在 entity decoding 後清除 URL 帳密，replay 使用相同的去憑證 key。
載入舊檔僅清理記憶體中的 key；需明確呼叫 save 才會保存修正，只有同一 key 群組內
等價的請求採最後一筆資料。分享前仍須檢查 capture 與舊副本；這些指定格式的轉換並不保證能偵測
自訂欄位、任意裝置資料或 malformed XML 中的所有秘密。

儲存層保留 legacy key 碰撞下的不同請求。Replay 由群組選出唯一的 scoped XML 身分，
沒有匹配才轉入 synthetic。完全相同的 raw recording 仍可 replay；非同一原文的
malformed XML、mixed content 或未解析 `xsi:type` 不視為等價。支援一般 namespace
別名、decoded scalar text 與選定的 qualified SOAP-header ephemera。此防護不是
完整 protocol validation，也無法恢復舊版已覆蓋的錄製。JSON 格式不變，但降版後舊讀取器
可能再次合併資料。僅提供 key 的 `lookup` 在歧義時回傳 `None`；請改用 `lookup_request`。
詳見[錄製儲存與報告遷移](docs/replay-storage_zh.md)。

`record_surface` 可配合 `SurfaceSelection` 只錄製指定 service group，並以 `SweepReport` 回報 recorded、failed 與 skipped operation。Replay read 會逐 byte 使用錄製 response；write 則轉入 synthetic `DeviceState`，依已提交的 profile effect 或尚未遷移的 legacy family 政策使 fixture 失效。完整依賴追蹤仍待完成。

`FixtureStore::diff_against_synthetic()` 比較 element-path set，結果表示與 oxvif reference mock 的結構差異，**不是 ONVIF schema conformance verdict**。`verify_parsing().await` 則使用 oxvif typed parser，將每項 fixture 分類為 `Parsed`、`Failed`、`Faulted` 或 `Unverified`；`failures()` 刻意排除裝置正常拒絕操作的 `Faulted`。

長時間 sweep 可使用 `_with_progress` variant。`SweepProgress::total` 計算展開 prerequisite 後的 operation 數，而不是 HTTP request 數；每個 operation 不論執行或 skip 都只推進一次。

`DeviceAdapter` trait 可替 RTSP-only 等非 ONVIF 裝置提供 ONVIF skin。只需實作 `identity` 與 `stream_uri`；profile、capabilities 與 services 由 synthetic mock 補足，`continuous_move` 與 `snapshot` 為可選 hook。

Typed hook 要求完整且支援的 Action 與對應 qualified operation；profile 身分
解碼後不裁切空白。缺少、重複、巢狀或路由不一致的身分欄位不會進入 typed hook。
ContinuousMove 僅接受兩組座標軸、有限座標值，且無明確 space 或 Timeout；目前
`PtzVector` 無法保留省略的軸或這些選項。其他形式以原始 bytes 交由 `respond_raw`，
再交由 synthetic fallback。Raw adapter 自行負責這些請求的驗證。這是 typed API
限制，不是 ONVIF 無效性判定；此處尚未驗證完整 StreamSetup／Protocol 或實際效果。

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
