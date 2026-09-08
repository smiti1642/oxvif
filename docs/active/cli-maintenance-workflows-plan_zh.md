# CLI 維運工作流程計畫

[English](cli-maintenance-workflows-plan.md) | [繁體中文](cli-maintenance-workflows-plan_zh.md)

狀態：本機實作完成；實機及跨平台原生發布驗收待執行。以 0.16.0 為基礎，功能尚未發布。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 工作流程邊界 |
| [實作項目](#實作項目) | 依序交付內容 |
| [驗收](#驗收) | 必要驗證 |
| [驗證證據與待驗項目](#驗證證據與待驗項目) | 結果與交接 |

## 範圍

透過共用應用層提供快照下載、分層診斷，以及攝影機設定的唯讀盤點與比較。
保留既有 URI 查詢與本機 `config path` / `config validate`。人類與非互動呼叫端
具有相同能力；JSON 不包含圖片二進位資料。

本輪不修改設備、不自動掃描網路、不在未指定測試目標的情況下操作實機，也不發布
套件、修改 Release 或替換已安裝執行檔。攝影機畫面與設定盤點均屬敏感本機檔案。

## 實作項目

1. [x] 快照：新增 `snapshot --save` 與正式指令 `media snapshot-save`，沿用 profile
   選擇並支援直接目標。拒絕既有目的檔，完整下載後才原子發布檔案。限制大小與時間、
   支援私人 CA 及經挑戰的 Basic/Digest 認證。在送出憑證前拒絕重新導向、內嵌憑證、
   不同主機的快照 URL 與 HTTPS 降級。不記錄可能含秘密查詢參數的 URL 或回應內文。
2. [x] 診斷：ONVIF/session、身分資訊、profiles、串流 URI 與快照下載各階段分別限時，
   回報通過、失敗、不支援或未測試，以及耗時、穩定階段名稱與後續建議。保留部分證據，
   有失敗檢查時回傳非零退出碼。RTSP 傳輸與影片解碼明確標示未測試；外部解碼器整合
   延後，不將本輪診斷宣稱為播放驗證。
3. [x] 設定：`config export` / `config diff` 收集主機名稱、NTP、DNS、網路介面、
   協定、閘道、Media1 profiles 與影像編碼設定。使用版本化盤點、分區錯誤、穩定排序
   與 JSON pointer 差異。不收集密碼、即時 URI、日誌或持續變動的時鐘。驗證基準檔
   格式與設備身分；不完整區段不得判定相同。匯出不是可還原備份，不覆寫既有檔案。
4. [x] 契約與 UX：更新指令描述、help、範例、結構化 envelope、表格、退出碼、Agent
   指南及 schema 測試。診斷支援既有 Group/View；產生檔案的工作流程初期限單一設備，
   明確拒絕 fleet 選擇器，避免檔案衝突。
5. [x] 文件：同步更新中英文 CLI 與維運指南、CLI crate 英文 README、精簡根目錄 README 連結、Changelog
   Unreleased、文件索引及本計畫的驗證證據，保留雙向語言切換與表格導覽。

## 驗收

- 本機 mock 攝影機／HTTP 伺服器測試：成功圖片、認證、非圖片或損壞回應、超大及
  分塊回應、逾時、重新導向、不同主機、降級、既有目的檔與取消後清理。
- 後續診斷失敗時保留先前結果；多個 profiles 時不任意選取；fleet 與 JSON 失敗語意一致。
- 盤點：排序穩定、實際欄位變更、缺少／不支援區段、身分或版本不符、基準檔讀取上限、
  不含秘密等測試。
- Parser、指令描述、schema 對齊與人類／Agent 指令測試。
- `cargo fmt --all --check`；workspace 預設與全功能 clippy／測試。未執行的實機或
  非 Windows 驗證標示待驗收，不將 mock 通過視為硬體驗收。

## 驗證證據與待驗項目

2026-09-08 本機 Windows x64 驗證：

以下記錄初始實作的驗證。後續 UX 修正及更新後的測試數量請參閱
[UX 改善計畫](cli-maintenance-ux-plan_zh.md)。

- `cargo test --workspace --all-features`：1,095 項通過、4 項既有條件式忽略，包含
  函式庫與執行檔共 19 項新增工作流程測試。
- `cargo test --workspace`：1,015 項通過、4 項既有條件式忽略。
- Workspace 預設／全功能 clippy（`-D warnings`）：通過。
- Rust 1.88 workspace／all-targets／all-features 檢查：通過。
- CLI rustdoc（警告視為錯誤）：通過。
- `cargo audit`：掃描 411 個依賴套件，未回報已知漏洞。
- CLI 套件清單包含新增實作與更新的 schema；已檢查八份指南的本機 Markdown
  連結。套件清單檢查不等同發布或安裝驗收。
- 涵蓋正反向 HTTP、Basic／Digest 挑戰認證、私人 CA、錯誤主機名稱 TLS、分塊
  大小限制、取消、不覆寫、階段重試、mock 快照、匯出／比較、不完整區段、身分
  不符、profile 歧義、fleet 部分／全部失敗與執行檔／schema 測試。

網路／TLS 診斷包含於 session 建立，並非獨立 DNS／TCP／TLS 探測。HTTP 下載
採單次總時限且不重試，SOAP 階段採每次嘗試限時及有界重試。快照僅驗證檔案特徵，
不執行完整解碼。DNS／NTP 偏好順序保留，具識別欄位的記錄則排序。本輪未連線
實機、未變更 Release／tag／已安裝執行檔，也未修改攝影機設定。

發布前仍須完成：

- [ ] 依中英文維運指南執行經授權的實機驗收。
- [ ] macOS／Linux 與 Windows 原生候選版本 CI／安裝驗證。
- [ ] 發布前與操作者確認版本及 Release 核准。

RTSP 傳輸／解碼器整合、批次檔案匯出、事件與設備寫入不在本輪實作範圍內。
