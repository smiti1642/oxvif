# 0.17 候選版本批次審查

[English](release-0.17-review.md) | [繁體中文](release-0.17-review_zh.md)

狀態：本機審查完成。更新日期：2026-09-12。本文件是執行紀錄，不代表發布核准。
[核准資料](release-0.17-approval_zh.md)及[發布關卡](release-0.17-cut_zh.md)
仍為判定依據。

| 章節 | 用途 |
| --- | --- |
| [清冊](#清冊) | 固定輸入與確切涵蓋範圍 |
| [批次順序](#批次順序) | 已完成的本機審查及保留限制 |
| [A04 人類輸出修正](#a04-人類輸出修正) | 重現、修正及驗證 |
| [V01 編碼器 Replay 覆蓋](#v01-編碼器-replay-覆蓋) | 擴充測試，不是新的產品修正 |
| [A05 終端顯示與 T01 斷言後續](#a05-終端顯示與-t01-斷言後續) | 終端修正、native exit guard 及本機驗收 |
| [T02 精確斷言後續](#t02-精確斷言後續) | 加強既有 driver 及負向控制 |
| [斷言審查界線](#斷言審查界線) | 測試保障及限制 |
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

全部 208 個固定項目現已 reviewed，後續路徑另於 `delta_reviews` 核對；逐檔 blob ID
識別已審查輸入。G01／G03 的本機 source／consumer／assertion／claim 審查結案，
不代表原生平台、staging、實機或全部 W00–W26 驗收。歷史缺陷及測試數量保留於
原始工作卡的具日期紀錄，目前判定以本文件為準。

## 批次順序

以完整服務子群工作，將同一批相關修正整合後再執行完整敏感度與修正後關卡，
不因每個檔案或文件修改重跑整個 workspace。

| 批次 | 固定路徑數 | 已完成的本機審查 | 保留驗收邊界 |
| --- | ---: | --- | --- |
| CLI | 18 | 入口、selector、registry、report、maintenance／manage、navigation、preferences、繪製／生命週期、Agent／schema／descriptor 及受影響測試均對照指南 | Windows debug ConPTY 為有界實測；其餘人工／平台及封裝安裝另驗 |
| Library | 26 | Media1／2、session、type／XML、公開 reexport／example、通知 listener 與消費端均對照 wire／fault／serde／loopback 及遷移斷言 | 快照識別不等於解碼；既有 listener 不保證 Internet 安全；不宣稱普遍實機符合性 |
| Mock | 54 | 共用 request／auth／fault／dispatch／state 及收錄的 profile／PTZ／source／encoder／audio／metadata 路徑、既有消費端、effect 與工作卡已核對 | 僅選定契約；其餘 legacy HTTP／field／Fault 及完整 W00–W26 保留限制 |
| Replay | 7 | Fixture／parse／report collision bucket、adapter／auth／raw chain、committed effect 及跨服務 identity／reference 消費端已對照 K27 與保留控制 | Raw recording 仍由呼叫者負責；降版與併發發布限制保留 |
| Delivery | 19 | Manifest／lockfile、完整 CI／release workflow、schema tool／source ref、source-SPDX、archive／formula 及安裝斷言已審查；本機 native exit 控制通過 | 最終候選原生 CI、散布 staging 及最終版號 package／install 仍須驗收 |
| 文件 | 84 | 公開指南／README／manpage、雙語遷移／發布紀錄、歷史工作卡、本機連結與已發布歷史均核對 | 歷史 external corpus／實機／CI 結果保留精確範圍與版本；不從文字審查推論新執行 |

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

## V01 編碼器 Replay 覆蓋

既有 `mock_video_rate` 已測試無效 rate 的錄製保留、fractional rate 讀回、
Media1 拒絕及實體來源保留。V01 是擴充這些覆蓋，不是修復新重現的產品缺陷。

`mock_video_encoder` 的兩項測試透過 in-process 與 HTTP replay 執行 Media2
encoder 寫入。八種不同 raw marker 先證明錄製命中；拒絕非有限 Quality 後，
完整狀態與全部錄製皆不變。接受整數 rate 設定後，Media1／2 的六種相關
profile／encoder 讀取回傳實際提交的名稱，另保留無關的實體來源及 encoder
options 錄製。輸入為合成控制，不是獨立 schema 驗證或實機 fixture；這兩項
測試未新增對 Media1 寫入入口的驗證。

暫時停用 `VideoEncoderCommitted` 失效分支後，兩項新測試在過期 `GetProfile`
失敗，既有兩項 rate-replay 測試也失敗。這證明共用失效邊界的敏感度，不代表
對六個 Action 各自進行獨立 mutation。最終關卡前已精確還原產品程式。
V01 不修改 ONVIF 方法、解析器、相依套件或產品行為。

| V01 檢查 | 結果 |
| --- | --- |
| 停用失效邏輯，完整 all-features／no-fail-fast | 1,308 通過；四項預期斷言失敗；五項 ignored；41 suites |
| 還原後 all-features／default | 1,312／1,200 通過；零失敗；各五項 ignored、41 suites |
| 靜態檢查 | 兩組 workspace all-target Clippy 禁止警告及 fmt 通過；公開原始碼未變，沿用 A04 strict rustdoc 證據 |
| 產品程式還原 | `src`、CLI crates、manifest／lockfile 及 workflow 相對 `1ea4fff` 無差異；只改測試與審查文件 |

## A05 終端顯示與 T01 斷言後續

2026-09-12，於 `44302c8` 後的 `codex/release-0.17-closure`。全螢幕繪製原本
移除 control 字元，卻仍輸出 Unicode 方向格式字元；新增測試在原始標題斷言失敗。
選單欄位、截斷及換行現在先跳脫顯示資料，再計算寬度，選取資料保留原值。
此項將 A04 延伸至這些繪製路徑，不宣稱全面防止 Unicode 混淆。

測試稽核保留不同 transport、state、replay 及執行檔邊界，將三個重複函式合併至
較強的既有 driver：從 XML 解析 PTZ preset 後精確檢查 JSON／欄位；capabilities
Fault 明確涵蓋有／無 `xml:lang`；seeded literal profile 名稱涵蓋三種讀取及兩種
transport。原生 keyring adapter 共用錯誤遮罩函式，測試得以注入確實包含合成敏感
文字的原始錯誤。

四組局部 perturbation 造成五項斷言失敗：backend 錯誤洩漏、preset Name 改變、
capabilities reason 改變，以及兩種 transport 的 seeded profile 文字改變。
全部復原後，受影響的十項測試通過。這是局部敏感度驗證，不是新一輪全 workspace
mutation campaign。

| 檢查 | 實際結果 |
| --- | --- |
| Workspace all-features／default、locked、no-fail-fast | 1,310／1,198 通過；各五項既有 ignored、41 suites |
| 兩種 workspace all-target Clippy | 禁止警告下通過 |
| 文件／相容性／相依關卡 | 兩種 strict workspace rustdoc、fmt、diff 空白及 Rust 1.88 all-feature／all-target 檢查通過；最新 cargo audit 掃描 410 個鎖定相依，無公告命中 |
| Windows ConPTY、實際 debug 執行檔 | 40 台合成已存裝置；`21G`、詳情／返回、100×24 → 48×12 → 100×24 縮放、profile 查詢／取消、文字編輯及遮罩密碼取消通過 |
| ConPTY snapshot 取消 | loopback proxy 確認 image body 已開始；Esc 後無目的檔／暫存檔、無後續請求，下次操作須重連 |
| 終端與本機狀態 | 正常退出及 Ctrl-C 退出，均還原同一 console 中由 parent 測得的 stdin／stdout mode；已存設定 hash 不變 |
| 執行檔來源 | 最終終端測試使用 debug 複本，SHA-256 `9fbcb9d166ecfc9987647f54051c7bcb17a3c7bd9387a4680c6e7e54606d1f1d`；屬本機 Windows 證據，不是 release archive／安裝驗收 |
| Windows CI smoke | 新增四個即時 exit 檢查；擷取腳本的成功控制及四個模擬 native 失敗均通過，每次失敗即停止後續命令 |
| Windows release smoke／封裝 | 新增十四個即時 native 命令檢查，含 PE 檢查及 archive 產生；各擷取 guard 均通過真正 native exit-0／exit-17 控制，共 28 案，未啟動託管 workflow |
| Packaging 測試位置 | 僅移除 Clippy job 重複呼叫；獨立 Ubuntu／Windows schema-tooling jobs 保留相同 24 項控制，仍為 package prerequisites |
| 最終文件／清冊結案 | 1,360 個本機 Markdown 目標及 670 個 anchor 有效，已發布 CHANGELOG 歷史未變；清冊 self-test 通過：159 route、161 Action site、191 reader。索引操作／來源列及 W00–W26 狀態保留 |

首次 default build 與執行中的 Mock 重疊，遇到 Windows linker LNK1104；終端輔助
程序改用執行檔複本後重跑通過。第一版終端 harness 誤用 HTTP root，導致 transport
失敗；修正為 `/onvif/device` 後通過，未改產品。這兩次失敗不計入驗收。
輸入及本機產物皆為合成資料，未安裝 host CLI，也未新增攝影機掃描。
Rust 修正及測試整併已提交為 `214d989`，workflow guard 為 `210bfc3`。T02 僅改
既有斷言；後續 source comment 與文件修改不改產品行為。

## T02 精確斷言後續

Commit `a5907d3` 加強四份既有測試檔，未增加測試函式或改產品行為。Encoder replay 在完整
state／錄製保留之外，固定 Sender → InvalidArgVal → ConfigModify 及 Quality
reason。Rate replay 於 commit 後讀回已錄製的 bare Media2 GetProfiles：一個 response、
四組 literal token／name 且無 Configurations。Encoder-instance 辨識要求成功的容量
（VSC_1 total 4、VSC_2 total 2），包含各 codec 清單。兩項 generic H264 options
測試固定有序的九組完整解析度 union，包含 480×240。

四組局部擾動造成七項斷言失敗：Quality reason 改變（兩項）、過期 bare profile
名稱（兩項）、錯誤 VSC_1 容量（一項）、union 缺少 480×240（兩項）。全部精確
還原後，17 項受影響測試通過。最終 workspace all-feature 1,310／default 1,198
通過，各五項 ignored、41 suites；兩組 Clippy 通過。這些控制證明加強後斷言的
敏感度，不代表獨立符合性或新一輪全 workspace mutation campaign。

## 斷言審查界線

選定 Mock／schema 及 CLI／navigation 斷言有不同且有價值的重疊保障，不能僅因名稱
相似刪除。

- 部分 profile／PTZ identity differential expected 來自同一實作的另一 instance。
  literal token、不同 head 位置、state 及 hook 提供額外辨識，但不獨立驗證全部語意。
  profile snapshot 競爭測試是有界探測，不是強制交錯的證明。
- Encoder／audio advertised-options 迴圈驗證自洽；其他精確值／列表斷言保護重要
  例子，但迴圈本身不代表完整、無真空案例的 codec 驗證。
- 初始的 encoder Fault detail 及 bare-profile replay 缺口已由 T02 補足；V01 另有
  Type=All profile 讀回保障。共用失效擾動不代表每個 Action 都獨立驗證敏感度。
- CLI schema 檢查本身未固定每個命令的 metadata；部分 maintenance 拒絕仍使用
  `is_err`。Manage 單元 decision table 本身不證明端到端取消，ConPTY 提供額外證據。
- Corpus 匯出不是驗證；ignored structural tests 不算通過。外部 XSD 結果與 lexical、
  semantic、HTTP、實機驗收分開。以上限制不代表新觀察到的產品缺陷。
- T02 補足 encoder-instances 列原本可能只辨識兩個錯誤、未要求成功的缺口。
  Generic encoder-options 的 2592／352 已能識別 VEC_1
  fallback，因為 VEC_1 沒有 352；初次稽核判斷相反，逐值核對 catalogue 後已更正。
  Action snapshot 將成功壓成 `ok`，
  且捨棄 Fault subcode／detail，並非無損 payload oracle；解讀覆蓋數量時須保留限制。

## 交付邊界

G01／G03 對收錄切點為 LOCAL-PASS：所有變更輸入、受影響消費端、斷言與公開宣稱
均已核對，A01–A05 已本機修復。其餘限定產品能力記於 backlog，不偷偷提升為
符合性保證；G05–G09 維持各自判定。維護者啟動 CI 與不發布的
staging 需要 repository 寫入權限，並須記錄精確候選 SHA；指令見
[核准資料](release-0.17-approval_zh.md#維護者操作)。較舊 SHA 的 CI 不涵蓋後續修正或 workflow guard。

保留的版號修改獲准前，library／CLI 維持 0.16.0。本次不合併 master／develop、
建立 tag、發布套件／Release、關閉 PR 或將候選安裝到本機系統。
一般修正 commit 不代表正式 0.17 版號 commit。
