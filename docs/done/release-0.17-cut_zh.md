# 0.17 發布切點與驗收

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

**2026-09-22 歸檔：此文件的限定交付範圍已隨 0.17.0 發布。**
結案依據見 [最終驗證與發布紀錄](release-0.17-finalization_zh.md)。下方保留各批次當時的版本、授權、待驗收項目與測試限制，並非目前發布狀態；歸檔不代表新增實機、原生終端或 VMS 驗證。尚未完成的廣泛工作由 [後續待辦](../active/post-0.17-backlog_zh.md) 與 active 主計畫繼續追蹤。

狀態：已授權對外發布。更新日期：2026-09-14。
維護者已發布兩個 crate，並授權 v0.17.0 tag 及 GitHub Release。下列準備清單
保留原執行順序；等待核准的停止條件已解除，發布結果以收尾紀錄為準。
使用者回報 CI 與人工驗收通過後，已授權版號及文件準備。本切點收錄已合併的
CLI 工作流程及基礎 Mock Fleet；本機系統安裝不在本次授權範圍。

| 章節 | 用途 |
| --- | --- |
| [收錄範圍](#收錄範圍) | 有限發布內容 |
| [發布阻擋關卡](#發布阻擋關卡) | 目前狀態 |
| [發布文件](#發布文件) | 版號與文件 |
| [施工順序](#施工順序) | 剩餘工作 |
| [驗證紀錄](#驗證紀錄) | 指定版本的歷史證據 |

## 收錄範圍

R01–R08 維持[審查](release-0.17-review_zh.md)及
[完整變更紀錄](../releases/0.17.0-changelog_zh.md)列出的 CLI、共用 XML／認證、
選定 Media／Mock 狀態、僅收件操作、通知 listener、相依套件及 replay 完整性。
後續授權範圍包含 manage 工作流程連續性、適應視窗的列表、情境按鍵說明及
[基礎 Mock Fleet](../mock-fleet_zh.md)。
不宣稱全服務擬真、ONVIF 認證、RTSP 串流、品牌模擬或 64／256 台 VMS 長時間穩定性。

## 發布阻擋關卡

| Gate | 目前狀態 | 證據及邊界 |
| --- | --- | --- |
| G01 候選審查 | 有限切點與 CLI／Fleet 差異 LOCAL-PASS | 見[收尾審查](release-0.17-finalization_zh.md#差異審查)；不默默擴充原 208 路徑清冊 |
| G02 Replay／資料完整性 | LOCAL-PASS | K27 保留碰撞及遷移警告；無法恢復舊版已覆蓋資料 |
| G03 安全及回應完整性 | 受審範圍 LOCAL-PASS | A01–A05 及後續 CLI／Fleet 審查；HTTP／listener／錄製限制仍明列 |
| G04 本機程式及文件 | 版號準備檢查通過 | Workspace check、strict rustdoc、打包及版本／help／連結檢查；舊完整測試計數仍屬歷史 |
| G05 原生 CI | PASS，0.17 候選 9305f5d | [全部 27 jobs](https://github.com/smiti1642/oxvif/actions/runs/34813217979) 通過 |
| G06 套件／散布 | PASS，0.17 不發布的 staging | [17 個驗證 jobs](https://github.com/smiti1642/oxvif/actions/runs/34813220307) 通過；兩個套件本機驗證、五平台 artifacts、APT／Homebrew 安裝、SBOM 及 checksum 均通過；非官方通路收錄 |
| G07 人類／Agent | 有限自動驗證及使用者回報人工 PASS | Discover／manage／resize ConPTY、schema 3／guide 8；不推定其他平台、設備或 VMS 覆蓋 |
| G08 版號及文件 | 已準備 | 兩套件／相依／lockfile 均為 0.17.0；雙語文件及固定 tag 連結；日期／發布狀態刻意保留待核准 |
| G09 發布授權 | 已授權 | 維護者於 2026-09-14 發布兩個 crate；已授權 v0.17.0 tag 及正式 GitHub Release，不另發 RC |

## 發布文件

- 根目錄及 CLI README 使用準備中的 0.17 範例、未發布提示及最終 tag 絕對連結；
  保留兩種 library Quick Start。
- 簡要 Release 導向完整技術／遷移 Changelog。
- 既有英／繁中公開指南同步收錄行為及限制；正確的 0.16 歷史比較不取代。
- 保留 `Status: Unreleased` 及 `## [0.17.0] - Unreleased` 發布防護。
  未來的 `blob/v0.17.0` 連結僅經本機結構驗證，不宣稱已上線。
- 後續發布流程見[核准資料](release-0.17-approval_zh.md)。

## 施工順序

第 1–3 步已完成。準備分支在 fast-forward 整合至 master／develop 後移除；
下一步為取得對外發布核准。

1. 在 `codex/release-0.17-finalize` 提交版號與文件準備。
2. 對精確 SHA 執行 CI 及手動、不發布的 release staging。
3. 記錄結果，必要關卡通過後才同步主分支。
4. 提出已知限制並取得明確發布同意。
5. 核准後才完成日期／狀態、先發布 library 再發布相依 CLI，建立核准 tag 及
   Release。若程式或發布工具再修改，重驗受影響項目。

## 驗證紀錄

以下保留歷史證據，不是目前關卡表。舊分支、版號、CI 阻擋及待授權敘述只適用於
當時輸入；目前結果以[收尾紀錄](release-0.17-finalization_zh.md)為準。

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
