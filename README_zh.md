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

在應用程式中加入 oxvif 0.16：

```toml
[dependencies]
oxvif = "0.16"
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

可獨立發布的 `oxvif-cli` package 會安裝名為 `oxvif` 的執行檔。0.16 版的 ONVIF
操作面刻意限制為唯讀：操作人員與 Agent 可以探索裝置、維護本機 inventory、檢查
device／media／PTZ 狀態，並執行 deterministic health 與 fleet diagnostics，
但不會修改攝影機設定。

從 crates.io 安裝 CLI：

```sh
cargo install oxvif-cli --locked
oxvif --version
```

Repository contributor 也可以安裝目前的 checkout：

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --help
oxvif setup 192.168.1.100
oxvif list
oxvif info
```

密碼會保存在 Windows Credential Manager、macOS Keychain 或 Linux Secret Service，
而不是 device registry。Private HTTPS trust anchor 可以明確加入，且不需要停用憑證或
hostname 驗證。

安裝方式、命令、安全行為、fleet workflow、structured output 與 exit code 請參閱
[完整 CLI 使用指南](docs/oxvif-cli_zh.md)。

Discovery 會將每筆結果標示為 `SAVED`、`NEW` 或 `INCOMPLETE`；終端使用者與 Agent
都能依相同狀態篩選即時掃描及已保存的 snapshot。終端瀏覽器可按 `i` 開啟可捲動的
完整資訊頁；`/` 與 Agent 使用的 `--query` 則共用相同的跨欄位搜尋語意。

`oxvif list` 會用本機快取資料列出所有已保存攝影機，不會主動連線攝影機；Agent 與
automation 可使用 canonical form：`oxvif device list --output json --non-interactive`。

原生 APT 與 Homebrew package 只會在完成獨立安裝驗證後，透過本 README 明確列出的
channel 發布。在這些 channel 列出前，請使用 crates.io，或使用對應 GitHub Release
所附且可核對 checksum 的 portable artifact。

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
oxvif = { version = "0.16", features = ["mock"] }
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

當其他 process 需要真正的 HTTP port 時，請啟用 `mock-server` feature。完整的路由、
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
| [支援範圍](docs/support_zh.md) | 版本化的平台、安全、相容性與商用宣稱限制。 |
| [Changelog（英文）](CHANGELOG.md) | Release 歷史與目前版本的變更。 |

## 專案狀態

oxvif 適合應用程式開發、診斷、互通性測試與受控 pilot。不同 vendor 與 firmware 的
ONVIF 裝置差異很大，因此相容性宣稱以實測證據為準，不會只根據 protocol profile
推論。歡迎提供經過敏感資料清理的其他實機報告。

0.16 CLI 是唯讀診斷 beta，並提供版本化的 structured-output contract。Release
驗證與平台證據記錄於
[0.16.0 release note（英文）](https://github.com/smiti1642/oxvif/blob/master/docs/releases/0.16.0.md)。

## 貢獻與安全性

提交變更前請閱讀 [CONTRIBUTING.md](CONTRIBUTING.md)。攝影機測試結果請使用
[compatibility report](https://github.com/smiti1642/oxvif/blob/master/.github/ISSUE_TEMPLATE/compatibility.yml)，
並遵循其中的敏感資料清理清單。

安全問題請依照 [SECURITY.md](SECURITY.md) 私下回報，不要建立公開 issue。

## 授權

MIT — 詳見 [LICENSE](LICENSE)。

ONVIF 是 ONVIF, Inc. 的商標。本專案與 ONVIF, Inc. 沒有從屬關係，也未獲其背書。
