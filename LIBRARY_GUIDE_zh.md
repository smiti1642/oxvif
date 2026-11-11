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
| [HTTP 回應限制](#http-回應限制) | Deadline、有界 body 與 custom transport |
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
oxvif = { version = "0.17", features = ["mock"] }
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
oxvif = "0.17"
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
oxvif = { version = "0.17", features = ["serde"] }
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
| `.build_with_report().await` | 相同初始化，回傳 `(OnvifSession, SessionBuildReport)`；失敗回傳 boxed `SessionBuildFailure`，含原始 error 與 report |
| `session.capabilities()` | 回傳快取的 `&Capabilities`，不執行網路呼叫 |
| `session.client()` | 存取底層 `&OnvifClient` |

`build_with_report` 依序記錄 `clock_read`、`capabilities`、`services_fallback`，
包含耗時毫秒、受控 error class、可取得的 clock offset，以及 service URL 來源
（`configured`、`capabilities`、`services_fallback`、`missing`），不保留
URL／body／Fault 文字。成功的 String transport result 無法辨識 HTTP status；
已知 HTTP error status 則會記錄。Clock／capabilities 失敗仍中止；optional
GetServices 失敗在 report 中可見，但初始化仍成功。既有 URL 不會被覆蓋。

```rust,no_run
# use oxvif::OnvifSession;
# async fn example() {
match OnvifSession::builder("http://camera/onvif/device_service")
    .with_clock_sync().build_with_report().await
{
    Ok((session, report)) => { println!("{:?}", report); let _ = session; }
    Err(failure) => println!("{:?}", failure.report),
}
# }
```

此獨立 report 在 `serde` feature 下可序列化。需要原始 typed cause 時明確檢查
`failure.error`；其中可能包含敏感 wire 資料。`HealthCheck::run_with_session_report`
同樣回傳 `(HealthReport, SessionBuildReport)`，不增加流量或改變既有 HealthReport
shape；HealthReport／captures 仍保留原有診斷細節。

Client trace 改為只記錄固定 operation Action、耗時、serialized request／returned
text byte counts，以及受控 transport error class。收到 String 不代表 SOAP 成功
或特定 HTTP status；此處計數單位是 SOAP exchange，不是 Digest HTTP request。
明確啟用的 capture API 仍保留 wire 證據；選定 credential 去敏不是通用 secret／PII
sanitizer。分享前需審查 capture，原始實機 capture 留在 checkout 外。

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

0.17 起：Media2 幀率限制改用 `f32`，保留小數；已提供但無效的 rate control 會回錯誤。
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

### 依服務識別的媒體讀取

`OnvifSession::media_catalogue` 支援 `MediaSelection::Media1`、`Media2` 與
`Auto`。Auto 先查詢已宣告的 Media2，缺少端點或收到具正確 namespace 的
unsupported-action Fault 時再查 Media1。Catalogue 記錄每次嘗試與端點來源。
認證、transport、端點政策及回應解析錯誤會中止選擇。

```rust
use oxvif::media_workflow::{MediaSelection, StreamRequest};

let catalogue = session.media_catalogue(MediaSelection::Auto).await?;
let profile = catalogue.select(Some(profile_token))?;
let stream = session.media_stream_uri(&profile, StreamRequest::Rtsp).await?;
let snapshot = session.media_snapshot_uri(&profile).await?;
let encoders = session.media_compatible_video_encoders(&profile).await?;
```

Profile handle 保留服務與 opaque token。Profile 和 encoder 維持各服務原生型別，
可讀取來源及 configuration 身分。`profiles_for_source` 依底層 source token
篩選。省略 profile token 時須有唯一 profile。URI 請求支援 RTSP interleaving、
UDP unicast、UDP multicast 和 HTTP tunneling。`media_stream_uri_with_capabilities`
可依選定服務明確回報的能力拒絕不支援的傳輸。播放需另行觀測。

流程預設信任設定的裝置 origin。跨 origin 的媒體服務可透過
`OnvifSessionBuilder::with_endpoint_policy` 明確加入信任。

## PTZ 方法

### 音訊設定與對講

`session.audio_configurations(service)` 明確選擇 Media binding，提供音訊
source、output 和 decoder 的設定與選項。Configuration、profile 與實體
音訊 token 分開處理。Output level 依裝置回報的有號範圍驗證。Decoder
codec 選項描述可接收的格式，設定本身沒有 SOAP codec 欄位。寫入保留
裝置回報的 use count，並獨立讀回。

`talkback` feature 的 `Talkback::channel` 提供 16 幀有界輸入佇列。送入
8 kHz 單聲道、每幀 20 ms 的 PCM，待 PLAY 成功後會透過選定的 PCMU
backchannel 依時序送出 RTP。預設上限為 1,500 幀或 30 秒，另計一秒
TEARDOWN。首版提供單一送出軌道的按鍵對講。

### Analytics 與錄影媒體

`session.analytics().await` 提供 capabilities、已設定及支援的規則與模組。
保留 QName 身分及參數順序，schema URL 只作為資料回傳。可用
`analytics::MetadataDocument::parse` 解析完整的 MetadataStream 並保留
通知與原始限定名稱。

`session.recording_timeline()` 提供摘要、單筆錄影資訊、指定時刻的 track
屬性及錄影事件搜尋。時間需明確帶時區。事件搜尋沿用錄影搜尋的分頁限制、
部分結果、取消與 EndSearch 清理。

`session.recording_exports()` 依裝置既有 storage configuration 啟動匯出，
透過獨立操作讀取進度和取消。`recording-download` feature 可從另外提供的
可信 HTTP(S) URL 有界下載，並以不覆寫方式發佈至指定路徑。SOAP 回報的
檔名只作為遠端資料處理。

### Media2 疊加與隱私遮罩

`session.media2_overlays(video_source_configuration_token)` 使用 Session 的
Media2 端點，並將操作綁定至指定的 video-source configuration。可先讀取
`capabilities`、`osd_options` 或 `mask_options` 取得裝置限制。文字 OSD
支援一般文字與日期／時間設定，影像 OSD 可讀取及移除。隱私遮罩使用
正規化多邊形與裝置宣告的效果及色彩選項。

建立、更新和刪除會先檢查指定的 configuration，再送出一次寫入並獨立
讀回。回覆遺失或無效會回傳 `OutcomeUnknown`。再次寫入前可先讀取
configuration 確認現況。共用 configuration 的 profiles 可能呈現相同
overlay，宣告 `SingleColorOnly` 的裝置也可能同步更新同一來源的遮罩顏色。

### 型別化 PTZ 移動

`ptz_motion_capabilities` 讀取指定 configuration 的預設值、操作空間與 timeout
範圍。將該 profile 對應的證據傳給 `ptz_move`。`PtzMotion` 支援絕對、相對、
連續移動及分軸 Stop。`PtzVector` 的 PanTilt 和 Zoom 都可省略，SOAP 只會
送出提供的軸組。可指定座標空間、速度或連續移動 timeout。

Client 在送出前檢查有限數值及裝置回報的範圍。未回報的限制維持未知，由裝置
驗證。實體移動由明確的 Stop 或裝置 timeout 控制。

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

0.17 起：請查詢目標 Media 服務的 codec options，不可直接互換編碼字串。
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

OSD 序列化採文字欄位順序及 color／persistence attribute，自訂位置座標可完整讀回。標準總量／文字類型配額可直接經 `OnvifClient` 或 `OnvifSession` 取得；舊 child 形式仍可相容讀取。

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
Drop stream 會取消所屬連線；仍須另外取消裝置端訂閱。
兩種 listener 均限制最多 32 條活動連線，讀取請求與寫入 HTTP 確認共用 10 秒期限；
header 上限為 128 KiB，UTF-8 body 上限為 1 MiB。HTTP/1.0 與 HTTP/1.1 POST
必須有唯一的十進位 `Content-Length`；重複長度及 `Transfer-Encoding` 均拒絕。
無效 framing 會嘗試回傳 HTTP 400，且不產生事件；超量或逾時則關閉 socket。
事件佇列滿載時，連線配額保留至事件被消費；佇列交付不計入請求期限。
此最小 listener 不提供 TLS／認證或完整 HTTP server 安全防護。

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

### 保留端點引用的 pull point 與受管理訂閱

`create_referenced_pull_point` 保留地址、依序排列的 reference parameters 與
原始 lease 時間。對應的 Pull、Renew、Unsubscribe 會產生有 namespace 的 SOAP
headers，並加入 WS-Addressing Action 和 MessageID。通知保留 topic bindings、
dialect、依序排列的 Source、Key、Data 及 XML 項目。`to_legacy` 轉換到原有
結構，重複的 simple item 採最後一筆，轉換時會丟棄 reference parameters、
Key 與 XML 項目。

```rust
use oxvif::events::{ManagedEvent, ManagedEventOptions};

let mut subscription = session.managed_events(ManagedEventOptions::default()).await?;
if let Some(Ok(ManagedEvent::Notification(event))) = subscription.next().await {
    let message = event.to_legacy();
}
let outcome = subscription.close().await;
```

Worker 並行拉取與續租。建立時須有可用的裝置時間和到期時間，據以計算本地
單調時鐘的期限。Renew 省略 CurrentTime 時使用最近一次配對的時鐘樣本。
恢復會送出 Gap，表示通知可能遺失或重複。佇列超限會在已排隊的通知送出後
回傳終止錯誤。`close().await` 停止 worker 並有界嘗試 Unsubscribe。Drop
會中止本地工作，遠端訂閱由 lease 到期釋放。

預設要求 60 秒 lease，裝置 pull 等待 15 秒，單次交換上限 20 秒，清理
2 秒，輪詢間隔 100 ms，每頁 32 筆，佇列 128 項與 1 MiB 計量 bytes，
最多替換訂閱兩次、每次 12 秒。Reference parameters 與單頁通知分別受
256 KiB 序列化保留上限約束。解析沿用 production XML bytes、深度與元素
數量限制。佇列 bytes 按序列化通知計量，解析中的回應與佇列管理有各自成本。
`EndpointPolicy` 信任裝置 origin 與明確加入的 origins。內建 addressed
HTTP 路徑直接送到驗證後的 URL。自訂 Transport 負責 HTTP 路由與認證。

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
| `recording_search(search_url, options)` | 不執行 I/O，建立單次使用的 `RecordingSearch` owner |
| `search_recordings_with_report(search_url, options)` | 有界搜尋；失敗時保留部分結果及清理證據 |
| `search_recordings(search_url, max_matches)` | 使用預設限制；只有 Completed 才回傳成功 |
| `get_replay_uri(replay_url, recording_token, stream_type, protocol)` | 取得錄影播放 RTSP URI |

以上三個 helper 也提供 `OnvifSession` 版本，省略 URL 參數。

```rust
use oxvif::RecordingSearchOptions;

match client.search_recordings_with_report(search_url, RecordingSearchOptions::default()).await {
    Ok(report) => {
        assert_eq!(report.stop_reason(), oxvif::RecordingSearchStopReason::Completed);
        // 消耗結果前，獨立檢視 report.cleanup()。
        let recordings = report.into_recordings();
    }
    Err(failure) => {
        // Display 僅含受控摘要；report/cause 可能包含裝置私有資料。
        eprintln!("{failure}");
        let (partial_report, original_cause) = failure.into_parts();
    }
}
```

Report 分別記錄已解析頁數與已接受的 recording。未知狀態，或加入後將超過
item/byte 限制的整頁，都會失敗且不保留該頁；先前接受的頁面仍可從
`RecordingSearchFailure::report()` 取得。保留裝置順序與重複 opaque token。
只有 Completed 回傳 Ok；即使完成，清理失敗或超時仍獨立記錄，並原樣保留
EndSearch 的可選 `Endpoint`。流程不自動重試 RPC。

| 資源 | 預設值／邊界 |
| --- | --- |
| Find 與 polling | 總計 120 秒；transport 本身可能有更短 timeout |
| 已解析頁數／保留項目 | 20／2,000；可選 MaxMatches 也限制保留項目 |
| 保留結果 bytes | 16 MiB；以 record struct 大小加解碼後 UTF-8 欄位長度計算 |
| Poll request | 100 筆、PT5S wait；每次非終態回覆後至少延遲 100 ms |
| 遠端 lease／清理 | 要求 PT60S keepalive／另計兩秒 EndSearch deadline |
| 同時工作／重試 | 每個 owner 一次一個 RPC，不建立 task／零次重試 |
| 回應／送出 XML | 沿用 production 的 4 MiB、depth 64、65,536 elements 限制；送出沿用既有 RPC serialization 與 token escaping |

`RecordingSearchOptions` 提供上述流程限制的 builder/getter；無效正值限制與
xs:int 範圍在 I/O 前即拒絕。保留 bytes 不包含 vector 額外容量、search token
及當前解碼中的 response，因此不是 peak-memory 保證。低階 Find/Poll/End
仍可直接使用，由呼叫端管理生命週期及限制。

需要協作取消時，在 await `owner.run()` 前保留 owner 並複製
`owner.cancellation()`；發出訊號後停止 polling，再執行一次有界清理。
若 timeout/select/abort 從外部丟棄 `run`，請保留 owner 並呼叫
`owner.cancel_and_close().await`。`close` 具冪等性，最多嘗試一次 EndSearch；
若 pending cleanup future 被丟棄，快取 Interrupted，因遠端結果未知。
Drop owner 只發出本地取消，不執行網路 I/O 或建立 task。Find 回覆遺失時沒有
已知 token，只能依賴裝置 lease 到期。若需要部分結果及清理證據，應等待正常
report 回傳。

相容性：`search_recordings` 維持原簽章及原始 Find/poll 錯誤，仍丟棄清理結果；
未完成搜尋現在回傳 `OnvifError::RecordingSearchIncomplete { reason }`。
下游 exhaustive match 必須新增分支，需要部分結果時改用 report API。
這是 exhaustive consumer 的 source-breaking 變更，候選實作需經下一個 minor
版本檢視，不適合 0.17.x patch；本批沒有調整版本或發佈。CLI structured envelope
schema v3 不變；maintenance 以 SEARCH_INCOMPLETE 分類，不重試或偽造 SOAP fault。

---

## 健康檢查（`health` feature）

`HealthCheck` 會產生 Pass/Warn/Fail/Skip 報告與 Profile S/T/G verdict。預設為唯讀，且無法連線的裝置會成為帶有結構化 `CheckError` 的 failing `connect` check，而不會使整個 fleet run 回傳 `Err`。

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

啟用 liveness probe 後，stream check 分別回報 RTSP OPTIONS、認證後的
DESCRIBE 和 SDP codec。`with_stream_options` 可選擇觀察首包。Snapshot
仍有獨立的有界下載與檔頭檢查。錄影檢查會執行 recording search 和 replay
URI resolution。額外的 RTSP／HTTP 連線預設關閉。

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

## HTTP 回應限制

`AuthenticationMode` 支援 `Combined`、`Digest` 與 `WsSecurity`。
提供 credential 時預設同時使用 HTTP Digest 和 SOAP UsernameToken。
可在 Client、Session builder 或 HealthCheck 選擇模式。自訂 Transport
自行管理 HTTP 認證。Session 的 `with_auto_clock_sync` 在使用 UsernameToken
憑證時讀取裝置時鐘。

`Transport::soap_post_with_options` 可指定整次交換期限，內建 HTTP 路徑涵蓋
Digest challenge、重送與讀取 body，並沿用 CA trust 與 response cap。
只實作 `soap_post` 的自訂 Transport 會取得外層 deadline，內部較短的 timeout
仍適用。Exhaustive match 需新增 `TransportError::Deadline` 分支。

`HttpTransport::new()` 對每個 HTTP request（包含 body 讀取）使用十秒 deadline，
並限制累積回應為 4 MiB。Digest 認證可能發出多個 HTTP request；呼叫端可用外層
operation timeout 限制整個交換。Byte 上限涵蓋 HTTP 200／400／500 與其他 status
body，也涵蓋 chunked／未知長度回應，計數在 UTF-8 replacement decoding 之前。

```rust,no_run
use oxvif::transport::{HttpTransport, HttpTransportOptions};
use std::time::Duration;

let transport = HttpTransport::from_options(HttpTransportOptions {
    timeout: Duration::from_secs(30),
    max_response_bytes: 8 * 1024 * 1024,
})?;
# Ok::<(), oxvif::transport::HttpTransportBuildError>(())
```

兩項設定都必須大於零。`with_options`、credentials 與 private CA bundle 不論設定
順序，都會保留彼此設定。超量回傳 `TransportError::ResponseTooLarge { limit }`，
不保留 body，也不應自動重試。下游對公開 error enum 的 exhaustive match 必須
處理新增的 resource 與 invalid-option variant。此上限是工作量設定，不是實測
裝置最大回應或總記憶體保證；custom `Transport` 仍須自行限制網路讀取。

Production SOAP 也在建構 DOM 前檢查完整輸入：UTF-8 text 上限 4 MiB（包含
markup／attributes）、depth 64、65,536 elements。即使使用 custom transport
或較大的 HTTP byte cap，`parse_soap_body` 仍執行這些限制。
`SoapError::ResourceLimit { resource, limit }` 表示不可重試的本地拒絕；exhaustive
error match 必須加入此 variant。格式錯誤、未閉合、多 root、DTD、錯誤 attribute
或 reference 改為回傳 `XmlParse`。XML declaration 需為 version 1.0，encoding
若存在需為 UTF-8。這些檢查不構成 namespace／XSD conformance 驗證。
合法 Unicode、entity、CDATA 內容及 namespace-stripped typed parsing 仍受支援。
`XmlNode::parse_complete` 提供同一有界入口；直接 `XmlNode::parse` 保留無資源上限
的舊寬鬆行為，深層輸入的 recursive consumer／tree destruction 仍有風險。

## 錯誤處理

無狀態 ONVIF RPC 回傳 `Result<T, OnvifError>`；錄影 report API 回傳
`Result<RecordingSearchReport, Box<RecordingSearchFailure>>`：

```rust
pub enum OnvifError {
    Transport(TransportError),
    Soap(SoapError),
    InvalidArgument(String),
    RecordingSearchIncomplete { reason: oxvif::RecordingSearchStopReason },
}
```

```rust
match client.get_capabilities().await {
    Ok(caps) => { /* use caps */ }
    Err(OnvifError::Transport(TransportError::Http(e))) => eprintln!("Network: {e}"),
    Err(OnvifError::Transport(TransportError::HttpStatus { status, body })) => {
        eprintln!("HTTP {status}: {body}");
    }
    Err(OnvifError::Soap(SoapError::Fault { code, reason, .. })) => {
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
oxvif = { version = "0.17", features = ["metamorph"] }
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

`store.summary()` 提供可序列化的 `FixtureSummary` 總數及依完整 Action URI 排序的
`FixtureActionSummary`。計數以 upsert 後實際保存的 exchange 為單位，包含碰撞下保留的
不同請求；`collision_buckets` 計算有多筆請求的 Action/key 群組。`faults` 沿用既有
local-name SOAP parser，計算 Body 下直接的 Fault；`unreadable` 計算 XML／缺少 Body
錯誤，均不代表 schema 或 typed-operation 驗證。Transport 失敗未存入 fixture，須另看
`SweepReport`。摘要不含 raw XML、key、裝置標籤或 Fault reason，但保留呼叫端提供的
Action 字串，分享前仍須檢查；呼叫不存取網路或寫入磁碟。

`FixtureStore::diff_against_synthetic()` 比較 element-path set，結果表示與 oxvif reference mock 的結構差異，**不是 ONVIF schema conformance verdict**。`verify_parsing().await` 則使用 oxvif typed parser，將每項 fixture 分類為 `Parsed`、`Failed`、`Faulted` 或 `Unverified`；`failures()` 刻意排除裝置正常拒絕操作的 `Faulted`。

`compare_corpora` 離線比較兩份凍結 `FixtureCorpus`，與 structural quirk diff、
`verify_parsing`、CLI `config.diff` 分開。配對需要完整 Action 與經有界解析證明的
request identity；canonical key 僅是候選提示。Channel token 保持有意義，Media1／
Media2 不合併；duplicate candidate 即使只存在單側，也保留為 Ambiguous。既有
raw replay 對 malformed request 的 exact escape hatch 不能作為比較身分證明。
比較僅涵蓋保留的 rows，不能推斷失敗／省略的 read 或雙側都缺少的 Action；capture
coverage 請使用 CorpusManifest 回歸驗證。

支援 16 個完整 Action，涵蓋 Device、Media1／Media2、PTZ、Analytics、Search 和 Replay。

| Projection | 比較值 |
| --- | --- |
| DeviceInformation | manufacturer、model、firmware、serial、hardware ID |
| Hostname | 名稱與 DHCP 選擇 |
| Profiles | 唯一 profile token、名稱、所有已建模 configuration／source token bindings |
| EncoderCore | 唯一 config token、名稱、encoding、resolution、可選 bitrate 與 multicast destination |
| StreamUriScheme | RTSP 或 RTSPS，URI 的 authority、path、query 與 userinfo 保持 unexamined |
| OsdOptions／MaskOptions | 選定的配額、效果、位置及幾何限制 |
| AudioOutputOptions | Output token 與有號 level 範圍 |
| AnalyticsSupport | 規則、模組、選項與 metadata capability flags |
| RecordingSummary／PtzSpaces | 錄影數量與限定名稱的 PTZ spaces 數量 |

先以完整 XML 與獨立、有界 `NsReader` guard 檢查 SOAP 1.2 response identity 及選定
expanded names。Device、profile 和 encoder 投影另外共用 typed parse routing。
支援 prefix alias／namespace
character reference；錯誤 namespace 不會因 local-name parser 而變 Equal。選定
scalar／binding 必須存在且不含歧義，此比較支援政策可能比 Library 相容 parser
嚴格；QName-valued `xsi:type` 維持 Unsupported。Encoder destination 另外從經
namespace 檢查的 address 欄位投影，包含 Media2 目前未建模的 destination。

搜尋、pull-point 與匯出生命週期使用有順序的流程證據。`compare_event_windows`
比較兩段明確指定的視窗內的 concrete topic、PropertyOperation 與相對時間。
Source、Key、Data 與 XML 擴充值維持 unexamined。

Equal 僅指宣告的投影；profile fixed、其他 encoder settings（quality、frame rate、
GOP／profile、multicast port／TTL）、vendor extension 與其他未檢查欄位會明列，
不能當作完整驗證。唯一 token 可對齊 reordered lists；缺 token 為 Unparseable、
重複 token 為 Ambiguous。相同 parser 的比較不構成獨立 schema／device conformance。

預設 mask 為空；firmware、semantic tokens、destination 差異保留。明確
`ComparisonMask`（如 `/name`、`/firmware_version`、`/profiles/*/name`、
`/configurations/*/multicast_address`）限定完整 Action，並保存在結果。
作為 list／binding identity 的 token 不可遮罩，無效 mask 明確失敗。

```rust,no_run
use oxvif::metamorph::{ComparisonOptions, FixtureCorpus, compare_corpora};
# async fn compare() -> Result<(), Box<dyn std::error::Error>> {
let options = ComparisonOptions::default();
let left = FixtureCorpus::load("/private/oxvif-captures/reference", options.limits.input)?;
let right = FixtureCorpus::load("/private/oxvif-captures/candidate", options.limits.input)?;
let report = compare_corpora(&left, &right, &options).await?;
for row in &report.rows { println!("{:?}: {:?}", row.outcome, row.changed_fields); }
# Ok(()) }
```

結果分為 Equal／Different／MissingLeft／MissingRight／Ambiguous／Unparseable／
Unsupported／DeviceFault，保留雙側 ordinal、受控 reason、projection、mask、差異
及未檢查路徑；不複製 request／response body、裝置值或任意 Fault reason。
Action／path metadata 仍可能私密，分享前須審查。

預設上限為 1,024 request groups、16,384 identity checks、4,096 field paths、
256 KiB compact JSON，另受 CorpusLimits 保護。Request identity 上限 2 MiB／depth
64／16,384 nodes；response projection 額外限制 16,384 nodes／attributes、256
namespace declarations、256-byte names、1,024-byte raw namespace URI 值，並保留 production
XML 上限。超限回傳明確 error，不會截斷後回報 Equal。
`compare_corpora_with_progress` 計算完成的 request groups，最終成功前均屬暫時進度。

執行 `cargo run --features metamorph --example reference_compare -- <left_dir>
<right_dir>`，stdout 為 deterministic compact JSON，stderr 顯示進度。Exit 0 為非空
selected equality；1 為不同／缺失 request；2 為 unresolved／Fault／empty evidence；
3 為 usage／input／resource refusal。載入保留 duplicate，已被舊 store 覆蓋的 row
無法恢復。

使用 `record_surface_with_evidence` 搭配 `SurfaceSelection`，可保留所有選定 profile／
source 的 param-aware request、逐 operation 的 `SweepReport`，以及獨立、有上限的
transport failure `RecordingEvidence`。既有 `record_surface_with_progress` 回傳
形狀維持不變。Sweep 的 `Failed` 也可能是 SOAP／parser／service 失敗，不可直接
當作 transport failure。Evidence 最多保留 512 次最終 transport failure 與 4 MiB
Action／request 資料；省略或未收集會明確標示，request 去敏仍是選定欄位處理。

在 capture 旁保存 `CorpusManifest`：假名裝置身分、可分享的 vendor／model／firmware、
recording revision、provenance、分享範圍、選定完整 Action、sweep 結果、transport
evidence，以及每個 fixture ordinal 的 `ExpectedExchange`。每筆保存凍結後的完整
sanitized request 與人工核對的 JSON Pointer／值，例如 `/brightness: 45.0`，保留
有意義的 channel token；不可直接複製目前 parser 輸出當作預期答案。

```rust,no_run
use oxvif::metamorph::{CorpusLimits, CorpusManifest, FixtureCorpus, verify_corpus};
# async fn check() -> Result<(), Box<dyn std::error::Error>> {
let limits = CorpusLimits::default();
let corpus = FixtureCorpus::load("/private/oxvif-captures/camera-a", limits)?;
let manifest = CorpusManifest::load("/private/oxvif-captures/camera-a/manifest.json", limits.file_bytes)?;
let report = verify_corpus(&corpus, &manifest).await?;
assert!(report.passed(), "接受 corpus 前先檢視保留的 disposition");
# Ok(()) }
```

`FixtureCorpus` 保留 duplicate row／ordinal，預設每檔 32 MiB、512 fixtures、每個
request／response 4 MiB；既有 `FixtureStore::load` 保留原 upsert 行為。
`verify_corpus` 檢查完整 Action／fixture coverage、精確 request identity 與選定
typed 值。Faulted／Failed／Unverified、缺少欄位斷言、不完整 sweep 或 transport
evidence 都不能通過嚴格驗收；不能只用 `all_parsed()`，也不代表獨立 conformance。

離線執行 `cargo run --features metamorph --example corpus_regression -- <corpus_dir>
<manifest.json>`，輸出 deterministic JSON。Exit 0 表示嚴格驗收通過，1 表示保留
失敗／不完整證據，2 表示輸入缺失、無效或超限；明確選定的 capture 缺檔必須失敗。
原始 capture 與官方 schema 留在 checkout 外，去敏副本分享前仍須人工審查。
實機驗收仍需另外提供 vendor／firmware capture。

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
| `src/recording/` | 有界 search owner、options、取消與 outcome report |
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
