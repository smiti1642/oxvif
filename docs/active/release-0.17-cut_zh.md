# 0.17 發布切點與驗收

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

狀態：IN-PROGRESS／尚不可發布。2026-09-11 授權執行。
2026-09-13 更新：[工作流程連續性](cli-workflow-continuity_zh.md) 已通過針對性 CLI
測試與隔離 Windows 終端。本差異須納入最終候選審查／CI，不以歷史 workspace
計數代替目前關卡。

2026-09-12 更新：使用者重新開啟 CLI 範圍，納入 manage 搜尋篩選、F12 狀態列
設計及選單位置保留，見 [CLI 重新驗收](cli-0.17-reentry_zh.md)。既有驗收及 CI
僅保留為歷史證據，不代表已驗證本次變更。
後續[基礎 Mock Fleet](mock-fleet-basic-plan_zh.md) 已實作並具本機證據，但發布
收錄仍待決定。目前開發分支為 `feat/basic-mock-fleet`（本次文件整理前為
`32fcc77`），包含 CLI `dcbff41`；查核時遠端 master／develop 均為 `5a1821b`。
比較基準：`v0.16.0`；凍結的範圍基準：
`f9448e515baec6169f40ec7f38ec2e0fcc752826`（證據補充 `2f92b75`）。
此處記錄原凍結範圍，不代表 hardening 分支歷史已獲驗收。
下列 K27 修正接續此基準，屬於已授權的資料完整性範圍。

[正式版號前審查資料](release-0.17-approval_zh.md) 追蹤使用者要求的四項工作、
A01／A02 修正及[共用快照修復](snapshot-auth-repair_zh.md)、維護者 staging 操作及
留待核准的版號修改。先前本機完整候選／安全審查僅對其記錄基準結案；CLI
重新驗收、安裝與最終發布仍依各自關卡判定。

| 章節 | 用途 |
| --- | --- |
| [收錄範圍](#收錄範圍) | 有限批次及證據入口 |
| [發布阻擋關卡](#發布阻擋關卡) | 不得延後的條件 |
| [發布文件](#發布文件) | 簡要摘要、完整紀錄及 registry 連結 |
| [施工順序](#施工順序) | 不依賴對話記憶即可接續 |
| [驗證紀錄](#驗證紀錄) | 精確結果及未取得的證據 |

## 收錄範圍

原切點排除新增使用者功能，之後使用者明確授權上述 CLI 重新納入作為例外。
其餘新發現依嚴重度分類，不默默擴充範圍。Mock Fleet 雖已存在於開發分支，
但在另行決定前仍不屬於已確認的發布範圍。

| ID | 收錄行為 | 原始碼與驗證入口 |
| --- | --- | --- |
| R01 | CLI 維護、manage、profile 資訊、人類／Agent 對齊 | `crates/oxvif-cli/src/{maintenance,manage,interactive,describe,agent,output}.rs`；CLI unit／executable tests；[維護驗收](../cli-maintenance_zh.md#人工驗收) |
| R02 | Vim 核心、行號設定、排版與取消 | `navigation.rs`、`ui_settings.rs`、`interactive.rs`；`tests/navigation_core.rs`、`tests/cli.rs`；[既有終端證據](cli-vim-navigation-plan_zh.md) |
| R03 | 已完成的共用 XML／Action／Fault／auth 邊界及 literal identity | W03–W08 已實作子群、P1／P2、PTZ1、A1；`request`、`dispatch`、`fault`、`auth`、共用 escaping；request／identity／auth／adapter 測試 |
| R04 | Media profile 建立／讀取／刪除／binding、source、rate、encoder、audio／metadata | E1、PA1、VS1、rate 修正、VE1、AM1；[操作清冊](mock-fidelity-operation-ledger_zh.md)、服務工作卡及 profile／source／rate／encoder／audio 測試；涵蓋雙服務及雙 transport |
| R05 | 選定的原子快照／hook 及 committed replay 相依 | W18／W19 已實作子群；`state.rs`、`responder.rs`、`metamorph/replay.rs`；reentrant hook、profile／read／replay 測試 |
| R06 | 十三個已分類的僅收件確認操作，包含 Media sync | A2／A3 加 B16；`policy.rs`、`mock_ack_policy`、`mock_media_sync` 及工作卡；不宣稱硬體效果 |
| R07 | Notification peer 包裝、listener 及相容性 | B17；`client/events.rs`、公開通知型別、`notification_origin` 測試 |
| R08 | 相依套件、XML 相容性、source SBOM 及驗證工具 | B14 加已收錄的相依維護；lockfile、schema 工具、release workflow、下游 XML 測試、source SPDX 控制 |

這些是選定契約，不是 W00–W26 或服務全部操作結案。未修改的 handler 仍可能
受共用 parser／fault 影響；須審查收錄 helper 的全部下游。不得宣稱
「Mock 全面 hardening 完成」或「所有 ONVIF 行為均已驗證」。

## 發布阻擋關卡

| Gate | 目前狀態 | 結案條件 |
| --- | --- | --- |
| G01 完整候選審查 | 基準 LOCAL-PASS；目前差異 OPEN | 208 個路徑及已記錄差異的審查只適用於[審查紀錄](release-0.17-review_zh.md)的輸入 hash；須對最終候選核對後續 CLI 及任何收錄的 Fleet 變更，測試通過不自動擴充固定審查清冊 |
| G02 資料完整性 K27 | LOCAL-PASS | 碰撞群組在 record／load／save 及完整請求 replay 中保留不同請求；key-only 歧義回傳 None；等價請求仍替換，去憑證維持指定格式。報告群組保留各列。見下方 K27 證據；G05 以獨立 CI 證據判定 |
| G03 安全及回應完整性 | 基準 LOCAL-PASS；目前差異 OPEN | A01–A05 保留已修復狀態及斷言證據；須核對後續終端行為與任何收錄的 Fleet LAN／控制端點／探索暴露。已知限制與本機測試不代表整體候選安全驗收 |
| G04 本機程式及文件 | LOCAL-PASS，Windows 開發批次 | B4 全功能 1,316 通過／0 失敗／6 跳過，預設 1,204／0／6；兩組 Clippy／strict rustdoc 及 fmt 通過，見[精確證據及排除](mock-fleet-basic-plan_zh.md#本機證據)。先前 Rust 1.88／schema 證據為歷史，不是新執行；改版號、移除範圍或合併後重驗受影響關卡 |
| G05 原生 CI | 歷史通過；目前待執行 | [Run 34672460802](https://github.com/smiti1642/oxvif/actions/runs/34672460802) 在 fed6777 通過全部 27 個 job／五種原生目標；不涵蓋後續 CLI／Fleet runtime 或 CI 步驟修改，最終候選須另行執行並記錄 |
| G06 套件及散布 | PARTIAL | fed6777 的 Package/docs 通過：library package 驗證、CLI package 檔案清單、archive 控制及文件。清單不等於 CLI package／安裝驗證；仍須最終版號套件、portable install、SBOM／checksum 及不發布的 staging |
| G07 人類及 Agent 驗收 | PARTIAL | CLI dcbff41 Windows ConPTY 涵蓋 manage 探索文字／紀錄篩選、正確選取、縮放、底部狀態及選單保留；B4 另驗四台 CLI 讀取／重啟／清理。剩餘導覽／輸入法／其他平台及原生 LAN／VMS 驗收仍明列；歷史實機證據不是新執行 |
| G08 版號及發布連結 | OPEN | 候選驗收後同步 library／CLI 版號；schema v3 宣稱須符合測試；草稿連結固定至最終 tag，遷移警告不可隱藏 |
| G09 RC 及授權 | NOT-RUN | RC 也須明確發布授權；建議觀察 3–7 天，不因日期到期自動通過；取得正式發布同意 |

`tests/mock_replay_key_gaps.rs` 現在斷言資料保留，不再斷言遺失：十組碰撞在
record／load／save、公開 lookup、報告及雙 replay transport 中保留兩份請求。
舊版已覆蓋資料無法恢復；降版後舊讀取器仍可能合併格式未變的 JSON。
詳見[儲存與報告遷移](../replay-storage_zh.md)。

A01 修復無效 HTTP UTF-8 處理；其餘 HTTP binding 與 fault／field 語意已對收錄
宣稱完成判定，保留 backlog 明列的子集合限制。若後續證實影響本切點的重大失敗，
仍須阻擋發布；本機審查不豁免新發現。

## 發布文件

- `docs/releases/0.17.0.md`／`_zh.md`：使用者導向的簡要摘要，沿用目前
  release workflow 的 `--notes-file`，不必改 workflow 或繞過權限。
- `docs/releases/0.17.0-changelog.md`／`_zh.md`：完整分類紀錄、不相容變更、
  遷移範例、驗證範圍及限制。
- `CHANGELOG.md`：精簡 Unreleased 摘要並連到完整紀錄；不改寫已發布歷史。
- 根目錄及 CLI 套件 README：開發中只放簡短「下一版」連結。發布前兩個 crates.io
  頁面均以絕對 GitHub URL 連至真實 release tag 下的完整紀錄；crates.io 不會自動
  展示 repository 的 changelog。
- 安裝指令在可發布版本備妥前仍維持已發布的 0.16。草稿保留
  `Status: Unreleased`，使既有發布契約阻擋提早發布。
- 最終連結使用 `blob/v0.17.0/...`，不指向 master／develop 或臨時分支；
  RC 連結須使用自己的真實 RC tag。

## 施工順序

下列第 1–3 步保留原始施工順序。目前候選應先決定 Fleet 發布歸屬、核對後續
G01／G03 差異，再依更新的[維護者操作](release-0.17-approval_zh.md#維護者操作)
對精確版本執行 CI／staging。若從發布移除 Fleet，也會改變受測輸入，須重驗
受影響部分，不能直接沿用含 Fleet 的統計。

1. 提交切點、雙語完整／摘要文件及[後續清單](post-0.17-backlog_zh.md)。
2. 以既有批次 suites 及有限獨立控制重驗收錄契約；先記錄失敗再修正，不把已知
   缺陷測試當成修復證據。
3. 分批修復 G02 及 G01／G03 的發現，保留貢獻者署名；不擴張成全部服務遷移。
4. 修復 workflow 權限或請維護者為精確候選啟動 CI。不得改 trigger、開 PR 或
   更換憑證繞過 403。
5. 本機及託管驗收後準備真正的 RC／版號，重驗受影響的套件／連結／CLI 版號
   關卡；staging 明確使用 `publish=false`。
6. 完成 G07／G09 後請求發布授權。測試通過不代表可以合併主分支、建 tag、
   publish 至 crates.io 或安裝到使用者系統。

[後續清單](post-0.17-backlog_zh.md) 排程剩餘工作，不將歷史 PARTIAL／TODO
標記完成。本切點負責發布排程；操作清冊與證據工作卡仍是技術事實來源。

## 驗證紀錄

基準證據見[社群整合紀錄](contributor-pr-integration-plan_zh.md#執行紀錄)：
all-features 1,298、workspace default 1,190 通過，各五項 ignored；兩組 Clippy、
fmt、strict docs、Rust 1.88、獨立 feature 及 encoding 關閉／開啟控制通過。
選定外部 corpus 有 160 份 XML、80 次交換、46 個操作、21 次拒絕，不代表全程式
conformance。Library package dry-run 通過，未上傳。以上不能結案儲存缺陷、
新版套件、原生 CI 或實機關卡。

2026-09-11 `e661781` 切點／文件驗收重跑，實作未變動
（K27 修正前的歷史基準）：

| 檢查 | 觀察結果 |
| --- | --- |
| Workspace all-features／default、no-fail-fast | 1,298／1,190 通過，各五項 ignored、41 suites |
| 兩組 workspace Clippy 及 fmt | 通過 |
| 包裝／schema 工具 unit 控制 | 外部固定版本 Python 環境中 24 項通過 |
| Inventory self-test | 通過；159 routes、161 Action site、191 reader site |
| Cargo audit | 通過；掃描 410 個 locked dependencies |
| Markdown | 466 個本機目標通過；檢查新發布文件的本機 anchor；已發布 CHANGELOG 歷史未變動 |
| 既有 release note 使用端 | 原始碼確認讀取短版版本檔；未修改 workflow 或遠端發布 |
| K27 | 既有可執行回歸仍重現儲存覆蓋；G02 維持 BLOCKED |
| CI dispatch | 再次執行仍回傳 HTTP 403，沒有新 run；G05 維持 BLOCKED |

該基準提交只修改文件。Rust／lockfile 輸入相同，重用先前 MSRV、strict rustdoc、
XML feature 及外部 corpus 證據，不宣稱本輪重新執行。未新增實機／終端 session、
最終版號套件或安裝驗證。可發布候選備妥前，版號仍為 0.16.0；未改變已發布產物。

### K27 碰撞保留修正

2026-09-11，接續 `e661781`。儲存索引改為碰撞群組，只有去憑證後等價的請求才能
替換。完整請求 lookup 選取唯一匹配，key-only lookup 拒絕歧義；legacy key
清理不再合併不同請求。`QuirkDiff` 將歧義列完整保留為未配對觀察值，不丟棄資料
或虛構對應。

碰撞回歸包含十組虛構請求，涵蓋空白、namespace／structure／attribute 邊界、
body 與未限定 namespace 的 header 欄位、mixed content、trailing root 及
QName binding。各組驗證不同回應、保存／載入／原檔不變、同請求替換、報告列及
進度、key-only 拒絕與 in-process／HTTP replay。Qualified ephemera 及一般
token／Action 正向控制仍通過。Legacy URL 清理另有相同請求合併及不同請求保留控制。

一次 workspace 全量 all-features、`--no-fail-fast` 擾動，暫時恢復同 key 替換
及報告 map 覆蓋；恰有三項在實際斷言失敗，而非編譯失敗：

- `metamorph::fixture::tests::legacy_key_cleanup_preserves_distinct_colliding_requests`
- `metamorph::quirk::tests::colliding_report_rows_are_retained_without_an_invented_pairing`
- `legacy_key_collisions_preserve_both_recordings_and_replay_identity`

擾動執行共有 1,297 通過、三項失敗、五項 ignored，涵蓋 41 suites。碰撞迴圈在
第一組即失敗，不宣稱每條 XML 比較規則均有獨立擾動敏感度。兩處擾動均精確還原後
才執行下列關卡。

| 檢查 | K27 結果 |
| --- | --- |
| Workspace all-features／default、no-fail-fast | 1,300／1,190 通過，各五項 ignored、41 suites |
| Workspace Clippy all／default 與 fmt | 通過 |
| Rust 1.88 workspace all-features／all-targets | 通過 |
| Library 獨立 feature | default、health、mock、mock-server、metamorph、metamorph-server、serde 的 Clippy 均通過 |
| Strict workspace rustdoc all／default | `-D warnings` 通過 |
| Markdown | 已檢查修改文件的 310 個本機檔案目標及 18 個導覽 anchor；正規化換行後，已發布 CHANGELOG 內容不變 |
| 遠端權限 | 唯讀檢查顯示 `viewerPermission=READ`；未再次 dispatch、換憑證或繞過限制；G05 仍阻擋 |

未修改 ONVIF 方法、SOAP parser、相依套件、workflow 或 CLI 實作。既有 XML
feature-unification 及外部 corpus 證據僅重用未變動輸入，不宣稱本輪重跑 schema。
本輪未執行實機、原生平台、最終版號套件或安裝驗收。G02 本機結案；G01／G03
完整候選與安全審查、G05–G09 仍依上表待完成。

### 原生 CI 維運測試後續修正

維護者啟動的 [run 34594415559](https://github.com/smiti1642/oxvif/actions/runs/34594415559)
測試 `111c185`。Windows、Linux x64／ARM 與 macOS ARM test job 通過；macOS
Intel 的 `chunked_snapshot_limit_is_enforced_without_content_length`、
`diagnostic_picker_retains_evidence_and_never_falls_back`、
`fleet_diagnose_retains_partial_and_total_failure_evidence` 及
`managed_session_reuses_expires_and_preserves_no_clobber` 失敗。Package/docs
因此略過；原生憑證、CLI smoke 及選定外部 schema job 通過。這是部分原生證據，
不是發布關卡通過。

原始維運測試在本機 19 項全部通過。四個失敗均使用共用的兩秒功能網路預算，
但舊斷言未呈現實際錯誤／診斷章節，因此 runner 資源競爭目前只是工作假設，
並非已證實的 macOS 失敗原因。

後續只修改 `maintenance.rs` 的 `cfg(test)` 模組：

- 功能測試採有限的 30 秒預算，fixture server 等待連線亦同；明確的 10 毫秒
  SOAP 與 100 毫秒 snapshot deadline 測試保留各自預算。
- 延遲 HTTP 與 SOAP fixture 超過舊兩秒上限，仍須回傳精確圖片 bytes／type
  及具有識別性的 SOAP 階段結果。
- Chunked 大小測試要求精確的 16 MiB 拒絕訊息及不可重試分類，而非任意錯誤；
  stalled 測試必須帶有超時證據。
- Export 完整性、diff 結果與 picker／fleet 失敗顯示結構化證據，保留不覆寫、
  選定 profile、exit code、session reuse／expiry、fault 及取消斷言。
  未略過任何測試，產品 timeout、圖片限制、重試及 CLI 輸出皆未改動。

修正版聚焦測試 20 項通過。一次 workspace 全量 all-features、`--no-fail-fast`
擾動再恢復兩秒預算、放過額外一個圖片 byte，並將兩處超時訊息替換為無關的可重試
錯誤。恰有三項斷言失敗：延遲進度、chunked 上限及 stalled snapshot 測試
（1,298 通過、三項失敗、五項 ignored，共 41 suites）。延遲測試先在 SOAP 階段
斷言失敗，不代表後續每個斷言均有獨立擾動證據。正式關卡前已精確還原全部暫時
改動；原生驗收仍須針對此修正版啟動新 run，不是重跑舊 commit。

精確還原後的最終本機關卡：

| 檢查 | 結果 |
| --- | --- |
| Workspace all-features／default、locked、no-fail-fast | 1,301／1,191 通過，各五項既有 ignored、41 suites |
| Workspace Clippy all-features／default、all-targets | `-D warnings` 通過 |
| 格式與 diff 空白 | 通過 |
| 產品程式碼邊界 | `maintenance.rs` 的 `cfg(test)` 之前內容與 `111c185` 相同，未改產品行為 |
| 文件 | 44 個本機 Markdown 檔案目標通過，已發布 CHANGELOG 內容未變 |
| 託管驗收 | 仍須新 run；目前 GitHub CLI 權限檢查為 READ |

本次測試修正不宣稱新增原生平台、套件／安裝、實機、MSRV 或外部 schema 執行。
先前產品程式碼證據仍屬歷史，不能據此結案 G05 或其他尚未完成的發布關卡。

### 正式版號提交前的驗收進度

上方要求重跑的敘述為歷史紀錄。維護者的
[run 34597167495](https://github.com/smiti1642/oxvif/actions/runs/34597167495)
在 `3eccfd15274d4e978501634e4155477760502b6b` 全部 27 個 job 通過，包含
Windows、Linux x64／ARM、macOS Intel／ARM 的測試、smoke 及原生憑證。
Package/docs 驗證 library，但 CLI 僅列出套件檔案；不代表後續修正或散布安裝已驗收。

審查發現 A01：HTTP 的 `from_utf8_lossy` 將無效位元組改為 U+FFFD，並成功建立
不同名稱的 profile。新增的 HTTP 入口檢查在 responder chain 前，以 HTTP 400／
Sender／mock:RequestPolicy 拒絕無效 UTF-8；未重設計 Content-Type、宣告的
charset 或一般 fault status mapping。修正前的完整 workspace all-feature
敏感度驗證有 1,301 通過、兩項斷言失敗、五項 ignored（41 suites）：profile
測試確實觀察到狀態被修改；另一項觀察到 armed-fault 路徑回傳 200。
修正後 all-feature 1,303／default 1,193 通過，各五項 ignored；兩組 workspace
Clippy 及 strict rustdoc 通過。五種無效位元組拒絕、合法中文／U+FFFD roundtrip
及 armed fault 保留均通過。

使用者要求停在正式 0.17 版號／發布 commit 之前。目前版號維持 0.16.0；
這些檢查不授權建立 tag、發布或合併主分支。
