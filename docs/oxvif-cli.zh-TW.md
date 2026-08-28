# oxvif CLI 使用指南（繁體中文）

[English documentation](oxvif-cli.md)

## 人類使用者快捷操作

第一次設定攝影機時，可以用一個指令完成安全密碼輸入、連線驗證、原生 credential storage
與 current-device 選擇：

```sh
oxvif setup front-door 192.168.1.100 --name "Front Door" --tag entrance --username admin
```

密碼會透過不顯示輸入內容的 terminal prompt 讀取，不會出現在 command argument、registry
或 log。設定完成後，日常操作可以直接使用：

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

直接執行 `oxvif discover` 是安全的一次性掃描，不會儲存 snapshot 或註冊裝置。Shell
completion 可用下列指令產生：

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

0.1 的 ONVIF 操作面是唯讀診斷：可以探索、讀取裝置資訊、Media URI、PTZ 狀態與健康狀態，
但不會修改攝影機設定。新增裝置、Group、View 或 discovery snapshot 只會修改本機 registry。

## 安裝

從 crates.io 安裝：

```sh
cargo install oxvif-cli --locked
oxvif --version
oxvif --help
```

在本 repository 測試尚未發布的版本：

```sh
cargo install --path crates/oxvif-cli --locked
```

套件與執行檔名稱不同是刻意的：Cargo 使用 `oxvif-cli` 避免和 library crate `oxvif` 衝突，
使用者則只需要記住 `oxvif <command>`。

## 最短上手流程

先用固定 ID 儲存一台攝影機，再把密碼送進作業系統的原生 credential store：

```sh
oxvif device add front-door --target 192.168.1.100 --name "Front Door" --tag entrance
oxvif device credential set front-door --username admin --password-stdin
oxvif --device front-door device test
oxvif --device front-door device info --output json
```

`front-door` 是不可變的 canonical device ID；顯示名稱、IP、tag 與快取的裝置資訊之後仍可更新。
密碼不會寫入 `devices.toml`。Windows 使用 Windows Credential Manager，其他平台使用對應的原生
credential store。不要把密碼放在 command argument、URL、版本控制檔案或 log 中。

## 給 Agent 的入口

Agent 在執行裝置操作前，應先讀取目前安裝版本內建的操作規則與 command schema：

```sh
oxvif agent guide --output json
oxvif describe --output json --non-interactive
oxvif describe media.stream-uri --output json --non-interactive
```

0.1 的 structured output schema version 是 3。Agent 應遵守以下原則：

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

Discovery filter 支援 `endpoint`、`uuid`、`type`、`scope`、`xaddr`、`ip-cidr`，以及 enrich 後的
identity 欄位。Filter 格式是 `field[:operator]=value`；operator 包含 `eq`、`neq`、
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
