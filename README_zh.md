# oxvif

[English](README.md) | **繁體中文**

[![crates.io](https://img.shields.io/crates/v/oxvif.svg)](https://crates.io/crates/oxvif)
[![docs.rs](https://img.shields.io/docsrs/oxvif)](https://docs.rs/oxvif)
[![downloads](https://img.shields.io/crates/d/oxvif.svg)](https://crates.io/crates/oxvif)
[![license](https://img.shields.io/crates/l/oxvif.svg)](https://github.com/smiti1642/oxvif/blob/master/LICENSE)

oxvif 是用於 [ONVIF](https://www.onvif.org/) IP 攝影機的非同步 Rust client
library 與命令列工具，涵蓋裝置探索、裝置管理、Media1／Media2、PTZ、影像、事件、
錄影、搜尋、重播、健康診斷，以及不依賴實體攝影機的測試工具。

## 為什麼選擇 oxvif

- 以 `tokio` 與 `reqwest` 為基礎，非同步優先。
- 支援 WS-Security `UsernameToken` 與 HTTP Digest authentication。
- 提供可回報錯誤、可指定網路介面的 WS-Discovery API。
- 可選擇自動快取 URL 的高階 `OnvifSession`，或自行控制路由的 `OnvifClient`。
- 內建具狀態的 Mock 裝置與可綁定連接埠的 Mock server。
- 提供 Metamorph 工具，可複製、重播及比較實體攝影機行為。
- 使用純 Rust XML parser，且不含 unsafe code。
- 提供適合人類與 Agent 的唯讀 `oxvif` CLI，並支援 deterministic JSON。

## 選擇操作介面

| 介面 | 適合用途 | 開始使用 |
| --- | --- | --- |
| Rust library | 需要 typed ONVIF access 與完整路由控制的應用程式。 | [快速開始](#快速開始) |
| `oxvif` CLI | 操作人員、診斷、CI、Agent 與 fleet inventory。 | [CLI 概覽](#命令列介面) |
| Mock 裝置 | 不使用實體攝影機也需要 ONVIF 行為的測試。 | [不使用攝影機進行測試](#不使用攝影機進行測試) |

## 安裝

**0.17.0** 提供引導式 CLI 維護及選定 Mock 契約
強化，包含原始碼不相容遷移。參閱[簡要摘要](https://github.com/smiti1642/oxvif/blob/v0.17.0/docs/releases/0.17.0_zh.md)
或[完整變更與遷移](https://github.com/smiti1642/oxvif/blob/v0.17.0/docs/releases/0.17.0-changelog_zh.md)。

在應用程式中加入 oxvif 0.17：

```toml
[dependencies]
oxvif = "0.17"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 快速開始

### `OnvifSession` — 探索並快取 service URL

`OnvifSession` 會在建立 session 時探索並快取 service URL：

```rust
use oxvif::{OnvifError, OnvifSession};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let session = OnvifSession::builder(
        "http://192.168.1.100/onvif/device_service",
    )
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

### `OnvifClient` — 直接控制 service routing

當應用程式需要自行提供每一個 service URL 時，請使用 `OnvifClient`：

```rust
use oxvif::{OnvifClient, OnvifError};

#[tokio::main]
async fn main() -> Result<(), OnvifError> {
    let client = OnvifClient::new(
        "http://192.168.1.100/onvif/device_service",
    )
    .with_credentials("admin", "password");

    let capabilities = client.get_capabilities().await?;
    let media_url = capabilities.media.url.unwrap();
    let profiles = client.get_profiles(&media_url).await?;
    let uri = client.get_stream_uri(&media_url, &profiles[0].token).await?;
    println!("RTSP: {}", uri.uri);
    Ok(())
}
```

[完整 Library 與功能指南](LIBRARY_GUIDE_zh.md)涵蓋兩種介面、裝置探索、所有
service family、錯誤處理與進階功能。產生的
[Rust API 文件](https://docs.rs/oxvif)則是 method 與 type 的權威參考。

## 命令列介面

`oxvif-cli` 套件提供 `oxvif` 執行檔，供人類與 Agent 使用。ONVIF 操作維持唯讀；
可管理本機攝影機清單、儲存快照、匯出及比較設定，不修改攝影機組態。

從 crates.io 安裝 CLI：

```sh
cargo install oxvif-cli --version 0.17.0 --locked
oxvif setup
oxvif manage
```

目前 checkout 可使用 `cargo run -p oxvif-cli -- manage`。
引導式工作區保留探索結果及各攝影機狀態，支援搜尋、已儲存／新增篩選、
Vim 操作、底部狀態列，以及 `?` 設定與按鍵說明。Agent 可透過版本化的
JSON／JSONL 使用相同操作，不會出現互動提示。

安裝及自動化請參閱 [CLI 指南](docs/oxvif-cli_zh.md)；
診斷、快照及設定比較請參閱[維運工作流程](docs/cli-maintenance_zh.md)。
快照不代表影片播放驗證，匯出設定也不是可還原的備份。
APT／Homebrew 暫存安裝驗證不代表官方渠道已收錄。

## 功能概覽

| 領域 | 重點 | 詳細參考 |
| --- | --- | --- |
| 探索與裝置 | WS-Discovery、capability、service、identity、時間、網路、使用者與 I/O。 | [指南](LIBRARY_GUIDE_zh.md#ws-discovery) |
| Media | Media1／Media2 profile、H.264／H.265、audio、stream／snapshot URI 與 video source mode。 | [指南](LIBRARY_GUIDE_zh.md#media-servicemedia1方法) |
| PTZ 與影像 | Move／stop、preset、tour、home、status、exposure、focus、IR cut 與 OSD。 | [指南](LIBRARY_GUIDE_zh.md#ptz-方法) |
| 事件 | Pull-point subscription、renew／unsubscribe 與連續事件串流。 | [指南](LIBRARY_GUIDE_zh.md#events-service-方法) |
| 錄影 | Recording／job 管理、時間／scope 搜尋與 replay URI。 | [指南](LIBRARY_GUIDE_zh.md#recording-service-方法) |
| 診斷 | 選用的 health check、parse-coverage detection 與 conformance 工具。 | [指南](LIBRARY_GUIDE_zh.md#健康檢查health-feature) |
| 測試工具 | Stateful in-process mock、HTTP mock server、fault injection、clone 與 replay。 | [Mock 參考](docs/mock-server_zh.md) |

精確的實作範圍請參閱[各 service 的 operation 表格](OPERATIONS_zh.md)。未列在表格中的
operation，不會被宣稱為已實作。

## 不使用攝影機進行測試

啟用 `mock`，即可讓 client test 使用內建且具狀態的 ONVIF 裝置：

```toml
[dev-dependencies]
oxvif = { version = "0.17", features = ["mock"] }
```

```rust
use std::sync::Arc;
use oxvif::{mock::MockTransport, OnvifClient};

#[tokio::test]
async fn updates_a_mock_camera() {
    let client = OnvifClient::new("http://mock")
        .with_transport(Arc::new(MockTransport::new()));

    client.set_hostname("lab-cam").await.unwrap();
    let hostname = client.get_hostname().await.unwrap();
    assert_eq!(hostname.name.as_deref(), Some("lab-cam"));
}
```

需要 HTTP 端點時，請啟用 `mock-server`（預設繫結 loopback）。0.17 另提供
[可設定的多台啟動器](docs/mock-fleet_zh.md)，可明確啟用 LAN 存取及基本共用 WS-Discovery。完整的路由、
狀態、支援 operation、fault injection 與限制，請參閱
[Mock 裝置參考](docs/mock-server_zh.md)。

## 文件

| 文件 | 用途 |
| --- | --- |
| [Library 與功能指南](LIBRARY_GUIDE_zh.md) | Library 詳細用法、service method、health check、Mock 與 Metamorph。 |
| [Rust API 文件](https://docs.rs/oxvif) | Public type 與 method signature 的權威參考。 |
| [CLI 使用指南](docs/oxvif-cli_zh.md) | 安裝、命令、安全性、fleet workflow、structured output 與 exit code。 |
| [已實作的 operation](OPERATIONS_zh.md) | 各 service 精確的 ONVIF coverage。 |
| [Mock 裝置參考](docs/mock-server_zh.md) | Mock 的完整行為與 fidelity contract。 |
| [音訊／metadata 遷移](docs/audio-metadata_zh.md) | 0.17 Rust／JSON 變更與 mock 限制。 |
| [支援範圍](docs/support_zh.md) | 版本化的平台、安全、相容性與商用宣稱限制。 |
| [Changelog（英文）](CHANGELOG.md) | Release 歷史與目前版本的變更。 |

## 專案狀態

oxvif 適合應用程式開發、診斷、互通性測試與受控 pilot。不同 vendor 與 firmware 的
ONVIF 裝置差異很大，因此相容性宣稱以實測證據為準，不會只根據 protocol profile
推論。歡迎提供經過敏感資料清理的其他實機報告。

0.17 CLI 是唯讀診斷 beta，並提供版本化的 structured-output contract。Release
驗證與平台證據記錄於
[0.17.0 發布說明](https://github.com/smiti1642/oxvif/blob/v0.17.0/docs/releases/0.17.0_zh.md)。

## 貢獻與安全性

提交變更前請閱讀[貢獻指南](CONTRIBUTING_zh.md)。攝影機測試結果請使用
[compatibility report](https://github.com/smiti1642/oxvif/blob/master/.github/ISSUE_TEMPLATE/compatibility.yml)，
並遵循其中的敏感資料清理清單。

安全問題請依照 [SECURITY.md](SECURITY.md) 私下回報，不要建立公開 issue。

## 授權

MIT — 詳見 [LICENSE](LICENSE)。

ONVIF 是 ONVIF, Inc. 的商標。本專案與 ONVIF, Inc. 沒有從屬關係，也未獲其背書。
