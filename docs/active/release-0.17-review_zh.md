# 0.17 候選版本批次審查

[English](release-0.17-review.md) | [繁體中文](release-0.17-review_zh.md)

狀態：IN-PROGRESS。更新日期：2026-09-12。本文件是執行紀錄，不代表發布核准。
[核准資料](release-0.17-approval_zh.md)及[發布關卡](release-0.17-cut_zh.md)
仍為判定依據。

| 章節 | 用途 |
| --- | --- |
| [清冊](#清冊) | 固定輸入與確切涵蓋範圍 |
| [批次順序](#批次順序) | 結案前剩餘工作 |
| [A04 人類輸出修正](#a04-人類輸出修正) | 重現、修正及驗證 |
| [交付邊界](#交付邊界) | 維護者 CI 與保留操作 |

## 清冊

[機器可讀清冊](release-0.17-review-ledger.json) 列出
`git diff --name-only v0.16.0 b7bc881` 的全部 208 個變更路徑。
狀態描述的是**本輪審查**，不代表過去實作或測試的完成程度。
`pending` 表示尚未在此記錄；`in_progress` 記錄確切檢視範圍，
但仍未結案。不得將測試通過數或已讀檔案數轉成發布完成百分比。

逐檔適用的原始碼差異、受影響使用者、斷言及公開宣稱完成核對後，
才可改為 `reviewed`。只有輸入版本完全相符時，才能沿用既有操作工作卡證據；
新修改的程式須審查差異。固定清冊之後的新增檔案列於 `delta_paths`，
包含本紀錄及其翻譯。

## 批次順序

以完整服務子群工作，將同一批相關修正整合後再執行完整敏感度與修正後關卡，
不因每個檔案或文件修改重跑整個 workspace。

| 批次 | 固定路徑數 | 本輪已檢視 | 剩餘結案要求 |
| --- | ---: | --- | --- |
| CLI | 18 | application／contract／registry／main／descriptor／Agent／schema 差異；maintenance／manage／navigation／preferences 產品程式；互動繪製／生命週期；人類輸出邊界 | 其餘測試差異與未修改使用者；執行檔與最終互動終端驗收；套件說明 |
| Library | 26 | Media1／2／session／type／XML 差異與通知 listener，包含沿用的 HTTP reader | 公開遷移／reexport／範例及受影響讀寫；核對先前快照證據的精確版本；通知限制與斷言 |
| Mock | 54 | scoped request tree、認證、receipt policy、responder 產品程式及 server／transport 差異 | dispatch／fault／shared-state 與收錄的各服務子群；對照工作卡核對讀寫／options／capability／replay 及既有 helper 使用者 |
| Replay | 7 | 已有先前 K27 證據；不宣稱本輪結案 | fixture／adapter／parse／quirk／replay／report 全部差異；碰撞保留、成功修改後失效及降版相容 |
| Delivery | 19 | 已有先前 CI／package 證據；不宣稱本輪結案 | source／tool 版本、workflow 輸入、schema 工具、SBOM／checksum／安裝斷言及新一輪原生／staging 結果 |
| 文件 | 84 | 發布切點／核准／後續清單邊界與 A04 宣稱差異 | 核對指南／README／manpage 最終宣稱、雙語連結與已發布歷史；不僅因說明文字重跑歷史測試 |

數量代表檔案分類，不是獨立風險單位；共用 helper 可能跨越多組。
收錄範圍內的安全問題須修復或明確阻擋發布，其餘產品功能留在
[後續清單](post-0.17-backlog_zh.md)。

## A04 人類輸出修正

人類表格報告原本會原樣送出資料中的 ESC／OSC／CSI、歸位、C1 及
雙向文字格式控制字元。本次在 `render_success`、詳細報告完成邊界及
`render_error` 修正，單行 profile 選項與隱含裝置上下文欄位使用
`terminal_field`。跳脫表示僅供顯示，不作為攝影機請求的輸入。

JSON／JSONL 保留原始值。人類報告排版保留 LF／TAB，因此不保證每個資料欄位
皆為單行，不宣稱全面防止 Unicode 混淆，也不新增對全部全螢幕輸入／繪製路徑的
安全保證。A04 不改攝影機協議、憑證、重試、檔案寫入語意或 schema 版號。

| 檢查，Windows x64 | 結果 |
| --- | --- |
| 產品修正前 | 完整 workspace all-features／no-fail-fast：1,308 通過、兩項預期斷言失敗、五項 ignored、41 suites |
| 敏感度斷言 | `human_output_neutralizes_terminal_commands_without_changing_json` 及 `verbose_stages_and_error_hints_cannot_emit_terminal_commands`；並非僅編譯失敗 |
| 修正後 all-features／default | 1,310／1,200 通過；零失敗，各五項 ignored、各 41 suites |
| 靜態／文件關卡 | 兩組 workspace all-target Clippy 禁止警告、兩組 strict rustdoc、fmt 與 diff 空白檢查通過 |
| 實際 debug 執行檔 | 使用含 ESC／BEL／方向控制字元的合成未知命令：人類 stderr 錯誤已轉義；JSON／JSONL stdout 可解析回原文；皆 exit 3，結構化模式 stderr 為空 |
| 執行檔檢查範圍 | 僅本機 `describe` metadata；無攝影機請求及系統安裝，不是互動終端驗收 |

修正前執行證明新增斷言能抓到原始行為。執行檔檢查補充單元測試，
不取代原生 CI。原始 log 為本機證據，不提交攝影機資料。

## 交付邊界

剩餘逐檔項目核對完成前，G01／G03 維持 open。維護者啟動 CI 與不發布的
staging 需要 repository 寫入權限，並須記錄精確候選 SHA；指令見
[核准資料](release-0.17-approval_zh.md#維護者操作)。較舊 SHA 的 CI 不涵蓋 A04。

正式版號前審查未完成時，library／CLI 維持 0.16.0。本次不合併 master／develop、
建立 tag、發布套件／Release、關閉 PR 或將候選安裝到本機系統。
一般修正 commit 不代表正式 0.17 版號 commit。
