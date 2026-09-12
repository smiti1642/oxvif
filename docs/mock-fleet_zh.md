# 可設定的 Mock Fleet

[English](mock-fleet.md) | [繁體中文](mock-fleet_zh.md)

此為開發中功能，尚未包含於已發布的 0.16.0 套件。它以單一前景程序執行多台
獨立的 ONVIF 測試設備，使用同一個 IPv4 位址及不同 HTTP 埠。啟動器是 Rust
範例，不是已安裝 `oxvif` CLI 的子命令；不提供 RTSP 串流或正式環境安全邊界。

| 章節 | 內容 |
| --- | --- |
| [快速開始](#快速開始) | 產生、驗證及啟動四台攝影機 |
| [設定](#設定) | 穩定身分及初始狀態 |
| [LAN 探索](#lan-探索) | 明確指定網路介面及暴露範圍 |
| [生命週期與限制](#生命週期與限制) | 清理、持久化及協議邊界 |
| [驗證](#驗證) | 可重跑檢查及剩餘驗收 |
| [參考資料](#參考資料) | 實作計畫及協議來源 |

## 快速開始

在包含此範例的原始碼目錄中執行：

```sh
cargo run --example mock_fleet_serve --features mock-server -- init lab.toml --count 4 --base-port 18080
cargo run --example mock_fleet_serve --features mock-server -- check lab.toml
cargo run --example mock_fleet_serve --features mock-server -- serve lab.toml
```

`init` 拒絕覆寫既有檔案。`check` 驗證設定但不開啟 listener。`serve` 列出實際
設備數、探索／認證狀態及各設備 URL，然後持續執行至按下 Ctrl+C。預設僅允許
本機 HTTP 存取，且不啟用探索。例如可在另一個終端機執行：

```sh
oxvif device info --target http://127.0.0.1:18080/onvif/device --json --non-interactive
```

請以啟動器實際輸出的 URL 為準。已安裝的 CLI 可以查詢此範例，不需要啟用
`mock-server` feature。修改設定前先停止程序；重新使用同一份 manifest，
即可在重啟後保留設備身分。

## 設定

`init --count 64` 或 `--count 256` 可產生較大的 manifest，但不代表承諾 VMS
容量。數量須為 1–256，連續埠範圍須介於 1–65535。每筆 `[[devices]]` 都是
實際設備設定，不另設執行期間的範本展開機制。

| 欄位 | 意義 |
| --- | --- |
| 根層 `schema_version` | 必填；目前為 `1` |
| 根層 `bind_ip` | 本機 HTTP IPv4 介面；預設 `127.0.0.1` |
| 根層 `advertise_ip` | 選填的明確本機 IPv4 位址；wildcard 繫結時必填 |
| 根層 `discovery` | 預設 `false`；須明確啟用共用 responder |
| 根層 `discovery_interface` | 啟用探索時必填；須等於公告的非 loopback 本機位址 |
| 設備 `id` | 唯一 ASCII 識別字，亦作為初始 hostname |
| 設備 `uuid` | 儲存在設定檔中的唯一 UUID，作為探索端點身分 |
| 設備 `port` | 唯一且固定的 HTTP 埠 |
| 設備 `name` | 經編碼後透過 ONVIF name scope 公告的名稱 |
| 設備 `manufacturer`、`model`、`serial_number` | 裝置資訊身分；序號必須唯一 |
| 設備 `scopes` | 選填的絕對 scope URI；會公告，但不支援 scope 篩選比對 |
| 設備 `state_file` | 選填的既有 `DeviceState` TOML；相對路徑以 manifest 所在目錄為基準 |

Manifest 拒絕未知欄位、無效／重複身分及無效網路組合。Manifest 與每份引用的
狀態檔上限均為 4 MiB。引用檔不存在或格式錯誤時直接失敗，不回退為出廠設定。
狀態檔使用既有的 `DeviceState` parser。

未指定 `state_file` 時使用出廠狀態。載入後，manifest 會覆蓋 hostname、廠商、
型號、序號與 scopes（包含產生的 name scope），其他載入欄位保留。輸入檔案
均為唯讀；執行期間的 ONVIF 寫入只改變該成員的記憶體，重啟後不保留。

## LAN 探索

僅限隔離且已授權的測試網路。啟動器**不強制認證**，暴露 HTTP 亦會暴露 Mock
控制端點。它不會修改防火牆或替主機新增位址。

修改產生檔案中、**第一個 `[[devices]]` 表格之前**的根層設定。下列文件示範
位址須替換為測試介面已配置的 IPv4 位址；不要重複附加既有 TOML 欄位：

```toml
schema_version = 1
bind_ip = "192.0.2.10"
advertise_ip = "192.0.2.10"
discovery = true
discovery_interface = "192.0.2.10"
```

先執行 `check`，再執行 `serve`。單一 UDP 3702 listener 會以各自的 UUID 及
實際裝置 URL 公告已就緒的 HTTP 成員。埠衝突會使啟動失敗並清理整個 Fleet，
不會終止其他程序。公告 IP 必須明確且屬於本機；指定明確繫結位址時兩者須相同，
使用 `0.0.0.0` 繫結時則必須指定公告位址。

Responder 支援基本、未帶 scope 篩選的 WS-Discovery Probe，包含具 namespace
的 NetworkVideoTransmitter／Device type 篩選。**非空 Scopes 或明確指定的
scope 比對屬性會被拒絕。** 若 VMS 使用 scoped Probe，可能無法找到設備，
應另測手動 URL 加入流程。僅以 IP 識別設備的 VMS 也可能將同 IP Fleet 合併。

## 生命週期與限制

- 啟動前驗證完整設定。HTTP 繫結或共用探索啟動失敗時，關閉已建立的成員並回傳
  非零退出碼。
- Ctrl+C 先停止探索，再等待 HTTP 關閉。不支援熱重載、動態成員、自動狀態回寫
  或程序監管。
- 共用探索限制封包為 16 KiB，逐成員回覆並抑制近期重複 Probe。這不是完整
  WS-Discovery 相容實作：未實作 scope 比對、Hello／Bye、Resolve 或持久化開機計數。
- 既有 `mock_server` 持久化範例與短暫執行的 `mock_fleet` 範例維持原行為。
  單台 `MockServerBuilder::discoverable` 仍採 best-effort 與舊比對規則，不共用
  listener；共用且啟動失敗即停止的路徑請使用 `FleetBuilder::discoverable`。
- 函式庫使用者可透過 `FleetBuilder::member` 設定成員、`Fleet::device_urls`
  取得端點，並等待 `Fleet::shutdown`。HTTP 預設仍為 loopback。
- 不提供 RTSP／影片產生、HTTP Digest、多品牌 Metamorph 設定、多主機 IP 管理
  或長時間穩定性保證。其他 Mock 真實度限制仍適用，詳見 [Mock 參考](mock-server_zh.md)。

## 驗證

已記錄的 Windows 全功能批次為 **1,316 通過、0 失敗、6 項跳過**；預設功能批次為
**1,204 通過、0 失敗、6 項跳過**。`ignored` 表示未執行，不是失敗，也不能計入
通過。容量測試已另外執行並通過，其餘五項未於本批次執行。前置條件及兩個未驗證
文件範例詳見[六項跳過明細](active/mock-fleet-basic-plan_zh.md#跳過測試明細)。
上述數字不代表原生 LAN multicast、VMS 或跨平台驗收完成。

聚焦設定及生命週期測試涵蓋無效輸入、防覆寫、獨立狀態、HTTP／UDP 埠占用回復、
共用 Probe 的精確身分／URL、重複抑制及清理。選用容量測試在一般執行中刻意忽略：

```sh
cargo test --features mock-server --lib configured_fleet_capacity_smoke -- --ignored --nocapture
```

CI 測試矩陣另以 `cargo test --example mock_fleet_serve --all-features --locked`
執行範例內的設定測試；一般 workspace 測試不會自動執行 example 內的測試。

已記錄的 Windows 本機執行先後啟動 64、256 台設備，收集不同的 loopback unicast
公告，並以期限限制執行 64／256 個同時裝置資訊讀取。此測試不量測原生 multicast、
VMS 加入、串流、持續負載或記憶體／CPU 峰值，詳見
[實作證據](active/mock-fleet-basic-plan_zh.md#本機證據)。

人工驗收請先在已授權介面啟動四台，確認 VMS 顯示四個不同身分、可連至全部 URL、
重啟後保留身分，並在 Ctrl+C／重啟後重新連線。之後才增加至 64 或 256 台，同時
記錄 VMS 連線數、失敗及主機資源。原生 LAN／VMS 與 Linux／macOS 驗收不由
本機單元測試取代。

## 參考資料

本次有界探索實作已對照
[ONVIF Core §7.3](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf) 及
[WS-Discovery April 2005 §2.4、§5.3 與 Appendix I](https://specs.xmlsoap.org/ws/2005/04/discovery/ws-discovery.pdf)。
這些參考不代表簡化 responder 已取得認證；上述 scope 與生命週期缺口為首階段
明確排除的範圍。
