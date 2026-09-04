# oxvif CLI 使用指南（繁體中文）

[English](oxvif-cli.md) | **繁體中文**

## 快速索引

| 章節 | 內容 |
| --- | --- |
| [人類使用者快捷操作](#人類使用者快捷操作) | 最短的互動式設定與日常操作。 |
| [安裝](#安裝) | crates.io、source 與原生 package 安裝指引。 |
| [最短上手流程](#最短上手流程) | 建立裝置、儲存憑證與首次診斷。 |
| [Retry 與診斷輸出](#retry-與診斷輸出) | Timeout、重試、TLS 與安全 log。 |
| [給 Agent 的入口](#給-agent-的入口) | 內建指南、descriptor 與自動化規則。 |
| [探索內網裝置](#探索內網裝置) | Interface、snapshot、filter 與 enrichment。 |
| [批次匯入](#從-snapshot-批次匯入) | 可審查的 plan/apply 與 fingerprint。 |
| [Group 與 View](#用-group-與-view-管理大量攝影機) | 靜態與動態 fleet 選擇。 |
| [唯讀診斷](#唯讀診斷) | Device、Media、PTZ 與 health。 |
| [Fleet 診斷](#fleet-診斷) | 有界並行、排序與彙總結果。 |
| [輸出與 exit code](#輸出格式與-exit-code) | JSON/JSONL contract 與程序狀態。 |
| [環境變數](#常用環境變數) | Credential 與設定路徑輸入。 |

## 人類使用者快捷操作

第一次設定攝影機時，可以用一個指令完成安全密碼輸入、連線驗證、原生 credential storage
與 current-device 選擇：

```sh
oxvif setup 192.168.1.100 --name "Front Door"
```

CLI 會建議不可變 ID `front-door`，再透過 terminal prompt 讀取 ONVIF username 與不回顯的
密碼；秘密不會出現在 command argument、registry 或 log。設定完成後，日常操作可以直接使用：

```sh
oxvif info
oxvif test
oxvif health
oxvif profiles
oxvif stream
oxvif snapshot
```

不切換 current device 時，可傳入精確的 canonical ID 或 `group/local-alias`：

```sh
oxvif info front-door
oxvif profiles taipei-f1/cam-023
oxvif health --group taipei-f1 --jobs 16
```

`stream` 與 `snapshot` 只有一個 profile 時會自動選擇；有多個 profile 時，人類終端會顯示
選單，`--non-interactive` 則會要求明確傳入 `--profile`。Agent 仍應使用完整 canonical
command、明確 selector、structured output 與 `--non-interactive`。

直接執行 `oxvif discover` 是安全的一次性掃描；人類終端會開啟可分頁、搜尋並加入裝置的
互動介面，但不會自動註冊任何裝置。直接執行 `oxvif setup` 也會先進入同一個探索介面。
Shell completion 可用下列指令產生：

```sh
oxvif completion bash
oxvif completion zsh
oxvif completion fish
oxvif completion powershell
```

`oxvif-cli` 是 [`oxvif`](../README.md) ONVIF client library 的命令列操作介面；
套件名稱是 `oxvif-cli`，安裝後的執行檔名稱則是 `oxvif`。它同時服務兩類使用者：

- 人類使用者可以用名稱、Group、View 與表格輸出管理大量攝影機。
- Agent 與自動化程式可以使用可描述的命令、JSON/JSONL、明確的 selector 與穩定的 exit code。

0.16 的 ONVIF 操作面是唯讀診斷：可以探索、讀取裝置資訊、Media URI、PTZ 狀態與健康狀態，
但不會修改攝影機設定。新增裝置、Group、View 或 discovery snapshot 只會修改本機 registry。

## 安裝

從 crates.io 安裝 0.16：

```sh
cargo install oxvif-cli --locked
oxvif --version
```

Repository contributor 可以改為安裝目前的 workspace 版本：

```sh
cargo install --path crates/oxvif-cli --locked
oxvif --version
oxvif --help
```

APT 與 Homebrew package 只透過專案
[`README`](../README.md#command-line-interface) 明確列出、且已完成獨立安裝／移除驗證的
channel 發布。原生 channel 尚未列出時，請使用 crates.io 或對應 GitHub Release 所附且可
核對 checksum 的 portable artifact。平台驗證證據記錄於
[0.16.0 release notes](releases/0.16.0.md#pre-release-verification)。

套件與執行檔名稱不同是刻意的：Cargo 使用 `oxvif-cli` 避免和 library crate `oxvif` 衝突，
使用者則只需要記住 `oxvif <command>`。

## 最短上手流程

互動式終端的最短流程是：

```sh
oxvif setup 192.168.1.100
```

若未指定 `--id`，CLI 會依 display name 或 target 建議固定 ID。完全不知道 IP 時可直接執行
`oxvif setup`，從探索介面選取裝置。Agent 與 unattended script 必須明確提供 target、ID、
structured output 與 `--non-interactive`：

```sh
oxvif setup 192.168.1.100 --id front-door --username admin --password-stdin --output json --non-interactive
```

先用固定 ID 儲存一台攝影機，再把密碼送進作業系統的原生 credential store：

```sh
oxvif device add front-door --target 192.168.1.100 --name "Front Door" --tag entrance
oxvif device credential set front-door --username admin --password-stdin
oxvif --device front-door device test
oxvif --device front-door device info --output json
```

`front-door` 是不可變的 canonical device ID；顯示名稱、IP、tag 與快取的裝置資訊之後仍可更新。
密碼不會寫入 `devices.toml`。原生儲存分別使用 Windows Credential Manager、macOS Keychain，
以及目前 D-Bus session 中的 Linux Secret Service。後端不存在、鎖定或拒絕存取時會回傳
`CREDENTIAL_UNAVAILABLE`，不會降級為明文 credential 檔案。headless Linux 或 container
可由可信任環境注入 `OXVIF_USERNAME`、`OXVIF_PASSWORD` 做不持久化的暫時性操作。
原生憑證生命週期合約已在 Windows x64、macOS Intel／Apple Silicon，以及 Ubuntu
x86_64／aarch64 CI 通過；Linux CI 也另外驗證缺少 D-Bus session 時會以
`CREDENTIAL_UNAVAILABLE` 安全失敗。
CLI 自己建立或從 credential store 讀出的密碼 buffer 會在 drop 時清零；但環境區塊、作業系統 API、
allocator、crash dump 與底層 protocol library 仍可能保留副本，因此這不是「所有記憶體副本立即消失」
的保證。應把執行中的 process 與 diagnostic dump 視為敏感資料，並使用短生命週期、最小權限帳號。
不要把密碼放在 command argument、URL、版本控制檔案或 log 中。

預設 registry 目錄在 Windows 是 `%APPDATA%\oxvif`，Linux 是
`$XDG_CONFIG_HOME/oxvif`（未設定時為 `$HOME/.config/oxvif`），macOS 是
`$HOME/Library/Application Support/oxvif`。備份或還原前必須先停止所有可能寫入的
oxvif process，完整複製包含 `devices.toml`、`devices.lock` 與 `snapshots/` 的整個目錄，
並保留舊目錄作 rollback。可先用 `oxvif config path` 確認路徑，再用
`oxvif config validate --output json --non-interactive` 解析 registry 與每個 indexed
snapshot。未被索引的 `snapshots/*.json` 只會產生 `ORPHANED_SNAPSHOT_FILE` warning，
不會自動刪除；清理前必須先查核備份與 registry 歷史。

## Retry 與診斷輸出

`--timeout` 是每次網路嘗試的上限。`--retries` 只會以有上限的 backoff
重試暫時性 transport failure；認證拒絕、無效輸入、確定性的 SOAP fault、parse
或 schema failure 不會重試。Discovery 只會重試失敗的已選介面，不會重跑已成功的介面。

`--clock-sync auto`（預設）會在有 credential 時先讀取攝影機時間，只調整 client
端 WS-Security timestamp offset；`always` 對所有 session 執行，`never` 則停用。
任何 policy 都不會修改攝影機的時鐘。

使用 private CA 的 HTTPS 攝影機可重複傳入 `--ca-certificate <FILE>`，每個檔案可為
單張 PEM certificate 或 bundle。CLI 會把它們加入平台 trust roots，並一致套用於
setup/refresh、單機診斷、health、discovery enrichment 與 fleet。格式錯誤、空 bundle
或包含 private key 的檔案會在連線前被拒絕；certificate chain 與 hostname verification
仍維持啟用，CLI 不提供 insecure bypass。`-vv` 只顯示 bundle 數量，不輸出憑證內容。

`-v` 會把 sanitized command、timeout、retry policy 與完成時間寫到 stderr；`-vv`
會再顯示最大 attempt 數與 timeout scope。JSON/JSONL stdout 不會混入 log、prompt
或 color，也不會輸出 password、authorization、WS-Security material 或 URI userinfo。

## 給 Agent 的入口

Agent 在執行裝置操作前，應先讀取目前安裝版本內建的操作規則與 command schema：

```sh
oxvif agent guide --output json
oxvif describe --output json --non-interactive
oxvif describe media.stream-uri --output json --non-interactive
```

0.16 的 structured output schema version 是 3。Agent 應遵守以下原則：

1. 使用明確的 `--device`、`--group`、`--view` 或 command-level `--target`，不要依賴目前選取的裝置。
2. 使用 `--output json` 或 `--output jsonl`，並加上 `--non-interactive`。
3. 先用 `describe <command>` 確認參數、風險與輸出，再產生呼叫。
4. 透過原生 credential store 或受控環境變數提供秘密，絕不將密碼寫進參數或輸出。
5. Fleet exit code `6` 代表部分成功，必須逐筆讀取結果和最後的 summary，而不是把整批視為失敗。

`oxvif help`、`oxvif --help` 與子命令 help 也會提示 Agent 前往 `agent guide`。

## 探索內網裝置

一次性掃描不會寫入 registry；加入 `--save` 才會保存 snapshot：

```sh
oxvif --timeout 3s discover scan
oxvif --timeout 3s discover scan --save factory-scan
oxvif discover snapshots
oxvif discover list factory-scan --filter ip-cidr=192.168.1.0/24
oxvif --timeout 3s discover refresh factory-scan
```

在互動式終端中，`discover` 會開啟最多每頁 12 筆的裝置瀏覽器；終端高度不足時會自動減少。
`j`／`k` 或上下方向鍵移動，`h`／`l` 或 Page Up／Page Down 翻頁，`g`／`G` 跳到第一／最後
一筆，`/` 進入即時搜尋，`c` 清除搜尋，`i` 開啟選取裝置的可捲動詳細資訊頁，Enter 或 `a`
對選取且尚未註冊的裝置執行安全
setup，`q`、Esc 或 Ctrl-C 離開。`r` 切換只看已記錄裝置，`n` 切換只看尚未記錄的裝置
（包含 incomplete），`A` 恢復全部。詳細頁沿用 `j`／`k`、`h`／`l` 與 `g`／`G` 捲動，
按 `i` 或 Esc 返回清單。

選擇加入裝置後，Device ID、使用者名稱與遮蔽密碼會在同一個 terminal 畫面的內嵌表單輸入。
Tab 或上下方向鍵切換欄位，Enter 前進或送出，Ctrl-U 清除目前欄位，Esc 則不儲存並返回探索清單。

互動掃描進行時，oxvif 會每秒在同一行更新已耗費時間。瀏覽器採用同步 terminal frame 與
逐行覆寫，降低移動及篩選時的畫面閃爍。

JSON、JSONL、redirected output 與 `--non-interactive` 不會啟動互動介面，而是輸出固定排序且
使用 Unicode 顯示寬度對齊的結果。每筆結果明確標示 `SAVED`、`NEW` 或 `INCOMPLETE`，
並在已記錄時顯示 oxvif device ID。已註冊裝置與沒有可用 XAddr 的紀錄不能重複加入。
oxvif 會直接解析 ONVIF scopes 已公開的 manufacturer、hardware/model、firmware 與 serial；
未提供的值顯示為 `Not advertised`。需要經裝置確認的正式 identity 時，再使用下方的
snapshot enrichment。

Agent 會取得相同語意的 structured fields：每筆具有 `registration_status`，已保存裝置另有
`registered_device_id`；`summary` 則包含 total、matched、saved、new 與 incomplete 數量。
`discover scan`、`discover list` 與 `discover refresh` 都接受 `--query <TEXT>`，其大小寫不敏感
的比對邏輯與互動瀏覽器的 `/` 完全相同，涵蓋 endpoint UUID、types、scopes、所有 XAddr、
manufacturer、model、firmware、serial number、registration alias 與已保存 device ID：

```sh
oxvif --timeout 3s discover scan --filter registration=unregistered --output json --non-interactive
oxvif discover list factory-scan --filter registration=saved --query loading-dock --output json --non-interactive
```

Registration filter 接受 `saved`／`registered`、`new`、`unregistered`（new 加 incomplete）及
`incomplete`。Scan／refresh 的 filter 只限制當次回傳結果；搭配 `--save` 或 refresh 時，
snapshot 仍保存完整掃描；`--query` 同樣只限制當次回傳內容，不會裁切保存的 snapshot。
Registration 是查詢當下的 registry 狀態，不適用於 `device import`；
import plan 本身已有 `already_present` disposition。

`--interface` 可以重複使用，值可以是本機網路介面名稱或 IPv4 位址：

```sh
oxvif --timeout 3s discover scan --interface Ethernet --interface 192.168.1.20 --save factory-scan
```

`discover refresh` 會原子替換 snapshot 的紀錄；snapshot 的 `generation` 會遞增，舊的 import
fingerprint 會失效。探索本身不會自動將任何攝影機加入 named-device registry。

若需要依廠牌、型號或序號篩選，可以先用 credential profile enrich snapshot：

```sh
oxvif credential profile set factory-admin --username admin --password-stdin
oxvif discover enrich factory-scan --credential-profile factory-admin --filter ip-cidr=192.168.1.0/24 --jobs 16
```

Discovery filter 支援 `registration`、`endpoint`、`uuid`、`type`、`scope`、`xaddr`、
`ip-cidr`，以及 scopes 公開或 enrich 後的 identity 欄位。Filter 格式是
`field[:operator]=value`；operator 包含 `eq`、`neq`、
`contains`、`prefix` 與 `in`。

## 從 snapshot 批次匯入

大量裝置使用 plan/apply 流程。先檢查唯讀 plan，再用完全相同的條件和 fingerprint 套用：

```sh
oxvif device import --from factory-scan --filter manufacturer=GeoVision --group taipei-f1 --credential-profile factory-admin --tag discovered --plan --output json
oxvif device import --from factory-scan --filter manufacturer=GeoVision --group taipei-f1 --credential-profile factory-admin --tag discovered --apply --expect-plan sha256:...
```

請從最新 plan 的 structured output 複製完整 `sha256:...`。Snapshot generation、filter、Group、
tag、credential profile 或 override 改變後，都必須重新 review plan；CLI 會拒絕不相符的 apply。

特殊裝置 ID 或 Group-local alias 可以放在不含秘密的 versioned JSON override 中，並在 plan 和
apply 都傳入同一份檔案：

```json
{
  "version": 1,
  "devices": [
    {
      "endpoint": "urn:uuid:...",
      "id": "loading-bay",
      "alias": "cam-042"
    }
  ]
}
```

```sh
oxvif device import --from factory-scan --overrides overrides.json --plan --output json
```

## 用 Group 與 View 管理大量攝影機

Group 是明確成員清單，適合樓層、廠區或維運責任區。每個成員可以有 Group 內唯一的 alias：

```sh
oxvif group create taipei-f1 --name "Taipei F1"
oxvif group member add taipei-f1 front-door --alias cam-023
oxvif device show taipei-f1/cam-023
```

`taipei-f1/cam-023` 永遠精確解析成一個 canonical device，適合在數百台設備中避免同名歧義。

View 是動態 filter，會依目前 device metadata 即時計算成員：

```sh
oxvif view create outdoor-geovision --filter tag=outdoor --filter manufacturer:contains=GeoVision --match all
oxvif view evaluate outdoor-geovision --explain --output json
```

Device/View filter 欄位包含 `id`、`name`、`target`、`uuid`、`manufacturer`、`model`、
`firmware`、`serial`、`tag` 與 `ip-cidr`。`--match all` 是預設值；也可以用
`--match any`。需要知道某台裝置為何被選中時，使用 `view evaluate --explain`。

## 唯讀診斷

對單台已儲存裝置執行診斷：

```sh
oxvif --device front-door device capabilities --output json
oxvif --device front-door device services --output json
oxvif --device front-door media profiles --output json
oxvif --device front-door media stream-uri --profile Profile_1 --output json
oxvif --device front-door media snapshot-uri --profile Profile_1 --output json
oxvif --device front-door ptz status --profile Profile_1 --output json
oxvif --device front-door ptz presets --profile Profile_1 --output json
oxvif --timeout 20s --device front-door health check --output json
```

回傳的 stream 與 snapshot URL 會移除 URI userinfo。預設 health check 不會執行 write
round-trip、額外的影像存活抓取或 raw exchange capture。

不先儲存裝置也可以直接使用 endpoint；帳密由安全注入的 `OXVIF_USERNAME` 與
`OXVIF_PASSWORD` 提供：

```sh
oxvif device info --target 192.168.1.100 --output json --non-interactive
oxvif media profiles --target 192.168.1.100 --output json --non-interactive
```

`--device`、`--group` 與 `--view` 是 root selector，放在 command 前；`--target` 屬於個別
診斷 command，放在 command 後。

## Fleet 診斷

Group 或 View 可以直接成為診斷目標：

```sh
oxvif --group taipei-f1 --jobs 16 health check --output jsonl --non-interactive
oxvif --view outdoor-geovision media profiles --output json --non-interactive
```

Fleet 預設同時執行 16 個工作，`--jobs` 上限是 64，結果固定依 canonical device ID 排序。
JSONL 每台裝置輸出一筆 `fleet_item`，最後再輸出一筆 `fleet_summary`，適合串流處理大量設備。

- Exit `0`：全部成功。
- Exit `6`：部分成功；保留成功結果並檢查失敗項目。
- Exit `20` 且 error code 為 `FLEET_FAILED`：全部失敗。

## 輸出格式與 exit code

`--output table` 是預設的人類輸出；`json` 適合單一 structured result；`jsonl` 適合 fleet
串流。Structured error 會在 stdout 保持合法 JSON/JSONL，診斷訊息則不應污染資料流。

| Exit code | 意義 |
| ---: | --- |
| `0` | 成功 |
| `2` | 參數錯誤 |
| `3` | command、device 或 resource 不存在 |
| `4` | resource 衝突、已存在或 import plan 不相符 |
| `5` | 缺少 target selector |
| `6` | Fleet 部分成功 |
| `10` | Config 或 registry 無法使用 |
| `11` | Credential 無法使用 |
| `20` | 裝置連線、探索或 Fleet 全部失敗 |
| `70` | Serialization 或內部錯誤 |

程式應同時檢查 exit code 與 structured error 的 `code`，不要解析人類訊息文字。

## 常用環境變數

| 變數 | 用途 |
| --- | --- |
| `OXVIF_CONFIG_DIR` | 指定 registry 目錄，適合測試、container 與隔離的 Agent session |
| `OXVIF_DEVICE` | 提供預設 device selector；Agent 仍建議顯式傳入 `--device` |
| `OXVIF_USERNAME` | Direct target 或暫時 automation 的使用者名稱 |
| `OXVIF_PASSWORD` | Direct target 或暫時 automation 的密碼；只應由受控環境注入 |

互動式人類工作階段可以用 `oxvif use front-door` 和 `oxvif current`；Agent、CI 與排程工作
應避免 ambient state，明確傳入 selector、output format、timeout 與 non-interactive mode。

## 查詢下一個指令

CLI 本身是與安裝版本同步的權威參考：

```sh
oxvif --help
oxvif device --help
oxvif describe --output json
oxvif describe health.check --output json
oxvif agent guide --output json
```

Package 細節另見 [`crates/oxvif-cli/README.md`](../crates/oxvif-cli/README.md)。
