# 0.17 發布切點與驗收

[English](release-0.17-cut.md) | [繁體中文](release-0.17-cut_zh.md)

狀態：IN-PROGRESS／尚不可發布。2026-09-11 授權執行。
比較基準：`v0.16.0`；凍結的實作候選：
`f9448e515baec6169f40ec7f38ec2e0fcc752826`（證據補充 `2f92b75`）。
此處凍結的是範圍，不代表 hardening 分支歷史已獲驗收。

| 章節 | 用途 |
| --- | --- |
| [收錄範圍](#收錄範圍) | 有限批次及證據入口 |
| [發布阻擋關卡](#發布阻擋關卡) | 不得延後的條件 |
| [發布文件](#發布文件) | 簡要摘要、完整紀錄及 registry 連結 |
| [施工順序](#施工順序) | 不依賴對話記憶即可接續 |
| [驗證紀錄](#驗證紀錄) | 精確結果及未取得的證據 |

## 收錄範圍

本切點不再增加使用者功能。允許修正下列契約所必需的問題；新發現依嚴重度
分類，不默默擴充範圍。

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
| G01 完整候選審查 | OPEN | 審查相對 v0.16.0 的全部差異，不只 PR #14／#16／#17；確認各收錄批次的讀寫／options／capability／replay 相依完整及遷移方式 |
| G02 資料完整性 K27 | BLOCKED | canonical key 碰撞仍會用不同請求覆蓋錄製內容。須保留不同身分或明確拒絕碰撞，涵蓋 load／save 及公開 lookup；保留機密清理與相同請求替換語意。阻止錯誤 replay 不等於修復儲存 |
| G03 安全及回應完整性 | OPEN | 處理已知機密洩漏、部分寫入、不安全 URL、誤導效果回覆及共用邊界回歸；不得把重大問題改名為後續工作 |
| G04 本機程式及文件 | LOCAL-PASS 基準 | 精確重用未變動程式的證據；修正、改版號或合併後重驗受影響關卡 |
| G05 原生 CI | BLOCKED | 最終候選的 Windows／Linux／macOS 真實結果；先前 dispatch 遭 HTTP 403 拒絕，沒有 run |
| G06 套件及散布 | PARTIAL | 0.16.0 版號的 library dry-run 通過；仍須最終版號的 library／CLI 套件、原生憑證／portable install、source／binary SBOM、checksum 及不發布的 staging |
| G07 人類及 Agent 驗收 | PARTIAL | 已有 Windows synthetic terminal／executable 證據；仍須最終候選的 resize／cancel／input 及實機唯讀 snapshot／diagnose／export／diff 證據；公開證據不含機密或影像 |
| G08 版號及發布連結 | OPEN | 候選驗收後同步 library／CLI 版號；schema v3 宣稱須符合測試；草稿連結固定至最終 tag，遷移警告不可隱藏 |
| G09 RC 及授權 | NOT-RUN | RC 也須明確發布授權；建議觀察 3–7 天，不因日期到期自動通過；取得正式發布同意 |

`tests/mock_replay_key_gaps.rs` 已重現 K27：不同請求縮成一份 fixture，
但 replay 會拒絕替換成錯誤回應。Known-gap 測試綠燈證明缺陷存在，不能結案 G02。

HTTP binding／UTF-8 與其餘 fault／field 語意須在 G01／G03 判定對收錄宣稱的
影響。完整重設計可延後；已證實影響本版的重大失敗不可延後。

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

2026-09-11 切點／文件驗收重跑，實作未變動：

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

本次只修改文件。Rust／lockfile 輸入相同，重用先前 MSRV、strict rustdoc、
XML feature 及外部 corpus 證據，不宣稱本輪重新執行。未新增實機／終端 session、
最終版號套件或安裝驗證。可發布候選備妥前，版號仍為 0.16.0；未改變已發布產物。
