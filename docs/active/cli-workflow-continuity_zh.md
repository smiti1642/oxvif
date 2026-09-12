# CLI 工作流程連續性計畫

[English](cli-workflow-continuity.md) | [繁體中文](cli-workflow-continuity_zh.md)

狀態：2026-09-13 已授權施工。原始碼基準為
`71f0f437f60781393db98ab58dd95eee22fc1d90`，分支 `feat/basic-mock-fleet`。
本計畫依實際程式檢查撰寫，不以先前對話摘要代替證據。
不包含版號變更、系統安裝、發布、啟動 CI 或合併主分支。

| 章節 | 用途 |
| --- | --- |
| [原始碼對照](#原始碼對照) | 已確認缺口與現有基礎 |
| [行為契約](#行為契約) | 狀態、安全與導覽決策 |
| [施工批次](#施工批次) | 依序實作與驗收 |
| [驗證與文件](#驗證與文件) | 關卡與交付 |
| [進度](#進度) | 執行後填入證據 |

## 原始碼對照

行號指上述基準，修改後以函式名稱定位。

| 缺口 | 實際程式位置 | 必要修改 |
| --- | --- | --- |
| Manage 無法新增探索到的設備 | [manage.rs](../../crates/oxvif-cli/src/manage.rs) 的 `choose_device`（304）；[interactive.rs](../../crates/oxvif-cli/src/interactive.rs) 的 `BrowserState::handle_key`（1331）、`DiscoverySelection` | 區分 Enter 選取與 a 明確新增 |
| 獨立新增提交後退出，驗證失敗亦然 | [main.rs](../../crates/oxvif-cli/src/main.rs) 的 `execute_and_emit`（1115）、`setup_discovered_device`（1175）；`browse_discovery`、`SetupForm::finish` | 新增期間保留終端、可重試表單與探索結果 |
| 切換設備丟失狀態 | `manage::run`（73–82）每次重新建立 context/profile/result/viewport | 工作階段擁有各設備工作區 |
| 已存清單不能搜尋 | `choose_device` 呼叫不處理 query 的 `Panel::menu_with_view`（735） | 可重用且保留原始索引的篩選選單 |
| Profile 選單回到首列 | `manage::run`（167）呼叫建立預設 viewport 的 `Panel::menu` | 每次讀取後按 token 還原位置 |
| 直接失敗無法重看 | `manage::execute`（252）將錯誤縮成 `WorkflowOutcome::Failed` | 失敗／取消證據與最後完成結果分別保存 |
| 位址修正丟失文字，比較無法沿用匯出 | `choose_device` 位址分支、`destination`（276）、`run` 比較分支 | 非機密輸入草稿及明確的最近匯出選項 |
| 重開報告位置歸零且無搜尋 | `Panel::show`（792）使用區域 `offset = 0` | 呼叫端持有可搜尋文字檢視 |

必須重用並保留的基礎：

- [application.rs](../../crates/oxvif-cli/src/application.rs) 的
  `Application::preflight_setup`、`CommandRequest::DeviceSetup` 執行、
  `rollback_setup`、`discovery_devices`（UUID／正規化位址的登錄投影）。
  現有 setup 先驗證，再同步保存本機資料，已有 rollback 證據測試；
  不重做寫入流程或身分比對演算法。
- [maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs) 的 `ManagedDevice`、
  `set_credentials`、`disconnect`、60 秒連線到期、`ManagedAction`、
  輸出檔不覆寫與 `read_baseline`。
- [interactive.rs](../../crates/oxvif-cli/src/interactive.rs) 的終端 RAII、
  差異重繪、表單密碼遮蔽／清除、文字輸入、`DiscoverySelectionView` 及情境式 Vim 導覽。
- 現有測試：application 的 setup 成功／驗證失敗／既有憑證衝突／競爭回復／
  回復不完整；maintenance 的 session／不覆寫／診斷取消；interactive 的探索
  狀態、篩選、Unicode、待完成按鍵、縮放及表單遮蔽。

## 行為契約

1. Manage Enter 僅選取，不儲存；`a` 明確開啟新增。獨立探索維持 Enter/a 新增。
   成功／失敗／取消均留在同一終端。表單明示提交將驗證並保存設備與憑證；
   manage 不修改全域目前設備，獨立探索維持既有 setup 選為目前設備的行為。
2. 重試保留 ID、帳號，清除已提交密碼，顯示實際 application 錯誤。不自動重試
   本機寫入、不覆寫、不略過 TLS、不改用其他憑證。提交前取消不寫入；
   執行中取消先核對 registry 狀態再允許重試，不把不完整回復說成乾淨取消。
3. 新增後以 registry 重新投影快取，不重播 multicast。保留 query、登錄篩選及
   紀錄身分。在 NEW 篩選下，新存紀錄會消失；顯示成功確認，選下一個有效位置，
   不擅自清除篩選。
4. 各設備工作區持有 ManagedDevice、Profile、操作／Profile 位置、最後完成結果、
   最新失敗／取消、報告檢視及非機密路徑草稿。已存設備以 ID 加正規化 target
   識別，直接設備以正規化 target 識別；不只按 IP 共用，也不默認合併兩者憑證。
   保留連線到期規則。最多保留 256 個 context，超過時明確要求重開，
   不靜默逐出既有狀態。
5. 除明確提交 setup 外，憑證僅留記憶體；切換不複製別台密碼，退出清除全部 context。
   已存設備的 target／憑證設定變更時，重用前使舊狀態失效。外部修改原生密碼庫
   的內容仍須重開工作區，維持既有文件規則。
6. 已存清單以 ID、名稱、target、tags 做不分大小寫搜尋。零筆符合時，網路搜尋與
   直接輸入仍可用。Registry 改變時按身分而非索引還原；不恢復未完成 Vim 前綴或
   搜尋編輯模式。Query 是一般文字，不是正規表示式。
7. Profile 仍即時讀取；讀取後還原仍存在的 token。Token 消失時清除舊選取並要求
   明確選擇，既有單 Profile 便利行為除外；讀取失敗不擅自改用另一 token。
8. 最後完成結果與最新失敗／取消分開列選項。報告重開保留 query／位置，新內容
   重設檢視，標示為歷史證據而非即時狀態。
9. 位址／路徑無效時回同一個有內容的編輯框；一般返回再進入保留非機密草稿，
   密碼不作草稿。比較可選本設備最後成功寫出的匯出或手動路徑；執行時重新驗證，
   不把快照路徑當設定清單。
10. 不改 ONVIF 協議／解析／Mock 或 Agent envelope。人類新增重用 Agent 已有的
    typed setup command。

## 施工批次

### B1 — 連續探索新增

- [ ] 將 main 中退出到 shell 的 setup 接線改為可重用同畫面流程，Panel 持有終端，
  共用 Application setup。
- [ ] 新增 manage a/add；獨立探索每次新增後可繼續。
- [ ] 重用本機登錄投影；成功／失敗／取消／重複 ID／無效 XAddr 保留身分與篩選。
- [ ] 擴充既有 UI/application 測試，涵蓋明確寫入、密碼清除、登錄刷新、
  待完成／搜尋按鍵不誤觸新增。
- [ ] 集中執行 CLI 相關測試後提交完整批次。

### B2 — 設備工作區及可搜尋清單

- [ ] 將每次選取的區域變數改為有界、按身分保存的工作區；
  已存身分／設定變更使舊資料失效，不跨設備共用。
- [ ] 可搜尋已存清單，保留固定操作列及原始索引。
- [ ] 還原 Profile token／位置；成功重新讀取後清除不存在的 token。
- [ ] 測試 A→B→A、相同 IP 不同身分、256 個 context 邊界、零符合、Vim 文字查詢、
  registry 變更與 Profile 移除／重排。
- [ ] 集中執行 CLI 相關測試後提交。

### B3 — 結果、修正與檔案流程

- [ ] 直接錯誤／取消與完成資料分開保存。
- [ ] 可重用、由呼叫端持有的報告文字搜尋／閱讀位置。
- [ ] 保留位址／輸出／比較草稿，輸入錯誤在同框修正。
- [ ] 確認成功匯出的檔案可選為比較基準；重新驗證刪除、格式錯誤或變更的檔案，
  維持不覆寫規則。
- [ ] 測試舊成功／新失敗分離、取消、文字無符合／Unicode／縮放、修正輸入、
  匯出後比較與遺失／無效基準。
- [ ] 集中執行 CLI 相關測試後提交。

### B4 — 整合、文件與交付

- [ ] B1–B3 後集中執行一次 CLI package suite 與 CLI 全 target Clippy／fmt。
  重用 application／maintenance 證據；除非跨入 ONVIF 核心或 Mock，
  不重跑整套 1300 多項 Mock。
- [ ] 用隔離 registry 與 mock fixture 執行 Windows ConPTY 真實終端流程：
  新增／修正／下一台、manage 搜尋／新增／操作／返回、A→B→A、
  256 台清單搜尋、Profile 消失、重看失敗、匯出比較、退出與終端恢復。
  UX 驗收不使用正式憑證或攝影機寫入。
- [ ] 分別記錄失敗／限制與通過證據；編譯不等於終端驗收。Linux/macOS
  runtime 仍待 CI／人工。
- [ ] 更新成對 CLI 指南／維運文件、根目錄 Unreleased changelog、完整 0.17
  changelog、active 驗收與發布切點。歷史計數及已發布 0.16 不改；
  文件提交驗證後才刷新公開入口的固定版本連結。
- [ ] 分批 commit／push 功能分支，交付精確 commit 與人工檢查。
  不合併、不打 tag、不發布、不安裝系統 release。

## 驗證與文件

每批在最近的既有測試模組新增情境測試，避免大量重複案例。網路 fixture 使用
隔離 loopback 埠、設定及假憑證；原生密碼庫成功測試維持明確 opt-in，
環境略過須如實記錄。先單元測試再終端流程，保留去識別且可重跑的終端工具。

驗收需要狀態斷言與端到端 UI 接線：單獨證明 viewport 可以保存，不能證明
呼叫端真的重用。新增公開行為須有成對英文／繁中文件、互相切換連結；
長文件須有章節連結表格。

## 進度

- 計畫完成：已檢查上述基準與各程式責任區。
- B1–B4 尚未驗收；每批執行後填入證據及 commit。
