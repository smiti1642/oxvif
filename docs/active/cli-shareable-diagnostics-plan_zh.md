# oxvif 0.18 可安全分享的 CLI 診斷報告

2026-11-13 依本地 `master` 的 `64ce580` 走訪程式。0.18 候選實作已通過 Windows 本地 CLI 回歸與 PowerShell 重新導向驗證。Linux、macOS 及多廠牌實機驗收仍待執行。

## 使用流程與範圍

建議在原指令上加 `--share`。診斷單台攝影機或檢查本機設定後，stdout 產生一份可獨立閱讀的 JSON 報告，即使檢查失敗也可保存。報告先列出失敗的階段、可確認的事實和最有幫助的下一步，再列其他結果和未執行項目。

```sh
oxvif diagnose --device CAMERA --non-interactive --share > diagnose.json
oxvif config validate --share > config-validation.json
```

`--share` 預設非互動，只支援單一 JSON 文件。`--output json` 可明確指定；表格、JSONL、Group／View 及其他命令在本版回報安全的用法錯誤。選擇攝影機時要求明確的 ID 或 `--target`，並沿用原本的 timeout、重試、SOAP 認證、Media1／Media2 和選用的 RTSP 探測。每次執行沿用一份檢查結果。兩個命令的一般模式維持現行輸出與結束碼。

報告本版使用 stdout 交付，方便管線或 shell 重新導向。文件須說明重新導向由 shell 處理，應使用新檔名，並在分享前確認檔案可解析。分享模式沒有上傳、攝影機設定寫入、憑證修改、登錄檔修復或額外連線探測。

## 現有路徑與可重用證據

| 路徑 | 實際行為與分享邊界 |
| --- | --- |
| [CLI 入口與前置錯誤](../../crates/oxvif-cli/src/main.rs#L938)、[執行與輸出](../../crates/oxvif-cli/src/main.rs#L1188) | Clap 解析、CA 檔案、`Application::system`、UI 設定及選擇器錯誤可能早於診斷執行。Clap 錯誤目前會轉成字串。一般流程可用 `-vv` 收集階段／重試並寫到 stderr。分享模式必須在這些分支輸出前確立。 |
| [目標與憑證解析](../../crates/oxvif-cli/src/application.rs#L1057)、[憑證後端](../../crates/oxvif-cli/src/credential.rs#L30) | 解析目標與密碼會早於建立 ONVIF Session。原生儲存的底層錯誤已捨棄原文，但缺少憑證的錯誤可能含裝置 ID 和憑證參照。原生儲存不可用、條目不存在和帳密組合缺欄位目前可能共用 `CREDENTIAL_UNAVAILABLE`。 |
| [ONVIF、媒體與快照診斷](../../crates/oxvif-cli/src/maintenance.rs#L700)、[診斷階段](../../crates/oxvif-cli/src/maintenance.rs#L884) | 現有 `Step` 有狀態、耗時、錯誤碼、提示與可變的 `data`。現行結果可含 Profile token／候選資料，外層可含目標與選擇器。`assess` 已整理首個失敗、其他失敗和限制，但部分跳過項目與結果的 `complete` 需要沿既有語義理解，例如 `complete` 在加入 RTSP 結果前計算。 |
| [Session report](../../src/session/report.rs#L57)、[Session 初始化](../../src/session.rs#L176)、[詳細輸出](../../crates/oxvif-cli/src/diagnostics.rs#L131) | 已有時鐘、Capabilities、GetServices 回退的型別化階段、結果、錯誤類別及服務 URL 來源；來源是 enum。CLI 目前主要把它轉成 `-vv` 文字，失敗時將原始 `OnvifError` 傳回。分享報告可在轉成文字前取用階段證據。 |
| [快照下載](../../src/health/snapshot.rs#L37)、[RTSP probe](../../src/stream_probe.rs#L53) | 快照已限制目標、認證、回應大小及逾時，但 `SnapshotError` 主要保留顯示文字和少數布林分類。RTSP 已提供型別化階段，可區分 TLS、I/O、逾時、拒絕與不支援。RTSP OPTIONS 的 401 表示服務可達，不能當成認證失敗；封包或圖片簽章檢查不等同解碼成功。 |
| [設定檢查](../../crates/oxvif-cli/src/application.rs#L253)、[登錄與快照讀取](../../crates/oxvif-cli/src/registry.rs#L1123) | 檢查登錄檔、索引中的快照，以及未索引檔案。現行流程可能在首個失敗停止。成功結果含完整路徑，錯誤可能含檔名、TOML／JSON 解析原文。此命令本身不讀 Keychain 或連線，設定通過不能推論憑證和裝置也通過。 |
| [AppError](../../crates/oxvif-cli/src/error.rs#L76)、[錯誤 renderer](../../crates/oxvif-cli/src/output.rs#L80)、[ResultMeta](../../crates/oxvif-cli/src/contract.rs#L717) | 結構化錯誤有穩定碼、重試提示與結束碼，但 `message`、`suggested_action` 是自由文字。JSON renderer 會照常序列化，終端 escape 只處理顯示。既有 [SOAP capture 遮罩](../../src/redact.rs#L21) 保留 Username，[URI sanitizer](../../crates/oxvif-cli/src/application.rs#L1740) 主要移除 URL userinfo。這些轉換各有用途，分享輸出仍需自己的欄位白名單。 |

## 報告的資料與程式邊界

1. **在來源保留可判讀的證據。** `diagnose` 共用原本的選擇器、憑證查找、Session 與探測。從這些邊界保留型別化事實，供現有 renderer 和分享投影各自使用。以 CLI 內部的結構補上失敗發生的位置，不增加 `AppError` 的必填公開欄位。透過 `build_with_report` 取得同次嘗試的 Session 階段，保留最終嘗試與有限次數摘要，成功重試與非必要回退警告不升格成主要失敗。
2. **抽出共用的本機設定檢查。** 讓 `config validate` 及分享模式使用同一套版本、參照及快照驗證規則。同次檢查讀取一份登錄索引，再驗證索引指向的檔案及統計孤立檔。普通模式維持首個錯誤與既有呈現。分享模式輸出已取得的階段和數量；因首個錯誤或預算停止的工作標示為未完成。用有界讀取和掃描限制處理分享模式的本機檔案，並驗證相容的正常登錄檔不被誤拒。權限不足或資料同時被改動時保留證據所支持的狀態。
3. **將分享資料限制為封閉型別。** 新增獨立 `ShareReportV1` 與 JSON Schema。包括 `kind`、報告版本、固定命令識別、CLI 版本、OS／架構、受控選項、整體狀態、結束碼、階段列表和主要問題。每階段只放固定 stage／status／reason／action code，及允許的布林、計數、耗時、HTTP 狀態、服務來源等有界資料。OS 和版本來自編譯資訊。人類說明及命令範例由固定模板產生，任何目標值用固定佔位符。分享型別禁止通用 `Value`、自由文字 extension、`AppError`、`Step.data`、一般 `ResultMeta` 和原始診斷文字。未分類錯誤映射到固定的未知原因和下一步。
4. **讓所有分享分支走同一輸出邊界。** 在一般 Clap 錯誤與前置初始化前，用經測試的參數辨識鎖定有效 `--share` 旗標，處理 `--` 結束旗標及旗標值等情況。進入分享模式後，解析錯誤、前置錯誤、執行失敗和輸出錯誤均用受控訊息；`-v`／`-vv` 不另吐一般診斷。禁止 fallback 到 `error.to_string()`、一般 renderer 或任意來源的 stderr。檢查依賴的日誌、進度與 panic handler，避免它們繞過 renderer 印出原文。先在記憶體序列化有大小上限的完整報告，再寫入 stdout。標準輸出寫入失敗時只允許固定 stderr 訊息，傳回非零，文件可能不完整。版本變更時，新增欄位與代碼需逐項評估資料來源並更新報告契約。

### 分享檔允許與排除的資訊

| 可保留 | 排除 |
| --- | --- |
| 目標選擇方式，例如已儲存裝置或直接目標；是否配置憑證的受控狀態；SOAP 認證及 Media 選項 | 帳號、密碼、憑證參照與 Profile 名稱、環境變數值、argv、Authorization／Cookie、WS-Security UsernameToken、Digest challenge 與 nonce、PEM 或金鑰內容 |
| 固定的服務名稱、來源、階段、結果、耗時、嘗試次數和成功／失敗計數 | IP／hostname、完整或部分 URL、userinfo、query／fragment、裝置名稱／ID／序號、Profile token、檔案或目錄路徑、未索引檔名、非固定 header |
| 固定代碼及可操作的固定建議 | 原始 SOAP／XML／SDP、封包、圖片、登錄檔、快照檔、日誌、Clap 參數原文、OS／HTTP／Keychain 錯誤文字 |

### 首要問題怎麼選，下一步怎麼寫

依檢查依賴關係選出第一個**可證實且會阻擋後續**的失敗，並保留互相獨立的其他失敗。狀態至少有通過、失敗、明確不支援、因前置失敗而跳過、使用者未啟用。報告分別呈現執行完整度與檢查結果，讓未啟用 RTSP 和已啟用但失敗有明確區別。

| 階段 | 報告應指出的證據和動作 |
| --- | --- |
| 輸入、登錄檔、探索快照 | 先指出缺少目標、路徑無法使用、登錄版本不相容、解析／參照錯誤，或已索引快照無法讀取。依原因建議補上目標、用 `config path` 在本機確認位置、檢查權限或比對備份。孤立檔只列數量並提示在本機檢查。 |
| 本機憑證／遠端認證 | 本機後端無法存取時，提供平台對應的解鎖／開啟方式；參照存在而找不到條目時，提示在本機重新設定憑證；HTTP 401／403 或已分類的 SOAP 拒絕時，提示驗證帳號權限和認證模式。只有實際取得裝置時間與偏差，才提供關於時鐘偏差的觀察。 |
| TLS／傳輸／Session | CA 輸入不合規可在本機驗證階段報告；RTSPS 的明確 TLS 結果可單獨標記。一般 SOAP `Transport` 常合併 DNS、TCP、TLS 與 HTTP，只報「傳輸失敗，細因未知」，提供可逐項檢查的動作。分別保留逾時、已知 HTTP 狀態及時鐘／Capabilities／Services 階段；取得穩定的型別化訊號前，避免解析 reqwest 字串猜測 TLS 或 DNS。 |
| Media／Profile | 區分裝置回報不支援、查詢失敗、沒有 Profile、指定的 Profile 不存在和需要選擇。建議在本機執行 `profiles` 並使用合適的 `--profile`，或在有證據時檢查 Media1／Media2；用受控值說明是否觀察到回退。 |
| Snapshot／RTSP | 分開列出 URI 取得、目的地驗證、下載、簽章與 RTSP 階段。若快照需要更精確的分類，在 `SnapshotError` 保留型別化原因及已知狀態碼，再用原有 renderer 顯示。RTSP OPTIONS 的 401 維持可達，後續認證拒絕才列為失敗。無第一個封包或 TEARDOWN 失敗各有對應建議，且報告明列未做影像解碼。 |

分享模式使用版本化的獨立報告契約，普通 JSON v3／JSONL 不變。既有命令結束碼仍代表其原來的成功／失敗規則；報告另列各階段觀察和執行完整度，從既有退出政策取得 `exit_code`。例如裝置故障得到有效的報告時，程序仍可以回傳 20；若使用者以 shell 重新導向，不能用結束碼判斷檔案是否可讀。

例如本機憑證參照存在、儲存後端可用，但查無該條目。報告應指出主要問題是「本機憑證缺少」，下一步是在本機重新設定該攝影機的憑證，並標示 ONVIF、Media 和快照因前置失敗而跳過。這份報告保留結束碼 11，不帶裝置 ID 或憑證參照。

## 施工與驗收

| 順序 | 交付 | 可驗證的完成條件 |
| --- | --- | --- |
| 1 | 先定 `ShareReportV1`、JSON Schema、階段／原因／動作表與輸出邊界 | 每個 JSON 物件都有封閉的欄位規則；未知分類有安全 fallback。解析、前置錯誤、序列化及 stdout 寫入失敗也走分享邊界。 |
| 2 | 接上 `config validate` 和目標／本機憑證的早期失敗 | 驗證空設定、有效設定、無權限、版本不相容、損毀 TOML、遺失／損毀／身份不符的快照及孤立檔。設定檔與快照內容逐位元組不變；config 路徑不連網、不查憑證庫。使用可注入的 CredentialStore 模擬無項目、後端不可用及帳密缺欄，驗證建議各自對應。 |
| 3 | 接上單機診斷、重試／Session、Media、快照及 RTSP | 用 MockServer 和現有受控 HTTP／RTSP fixtures 注入失敗。涵蓋 401／403、SOAP Fault、時鐘、Capabilities、回退、逾時、媒體選擇、快照被拒／非圖片、RTSPS TLS 錯誤及無第一個封包。核對先決條件、主要問題、下一步和退出碼；統計請求確認沒有第二輪診斷，沒選的 RTSP 模式沒有額外流量。 |
| 4 | 分享安全、相容性與文件 | 執行下列安全矩陣，普通模式的既有 schema、結束碼與提示維持。增加獨立報告 Schema 驗證，補雙語 CLI 指南和 0.18 發行說明；Windows PowerShell、Linux 和 macOS 驗證實際 binary 的 JSON 重新導向。 |

### 敏感資料測試矩陣

- 為每個來源放不同的合成 canary。涵蓋環境帳密、儲存帳號／密碼／參照、含私人名稱的路徑、裝置／Profile 值、URL userinfo 與 query、PEM 路徑及檔案內容。Mock 回應再將 canary 放入 HTTP body、SOAP Fault／Detail／未知 XML、Snapshot URI、RTSP SDP／錯誤或回應 header，確認設備回應不能把字串塞進分享檔。
- 驗證成功、部分失敗和前置失敗。另跑缺參數、錯誤旗標、錯誤 CA、native store 錯誤、損毀設定、無法寫 stdout、`-vv`、取消與預算耗盡。以合成 canary 覆蓋命令列錯誤回顯和支援範例中出現的設定值。stdout、stderr 與重新導向的最終檔都檢查。
- 同時檢查原始 bytes 及解析後的 JSON 值，涵蓋 JSON escaping、URL percent-encoding、XML escaping、Base64 和已知測試 Digest／Nonce。使用嚴格 Schema 與欄位值域斷言確認安全證據仍在。另以測試限定的反例注入原文，確認測試會失敗，防止只因未執行到帶秘密的分支而假通過。
- 重用 [現有 CLI 整合測試](../../crates/oxvif-cli/tests/cli.rs#L894)、[Schema 測試](../../crates/oxvif-cli/tests/cli.rs#L1271) 與 [跨平台 CI](../../.github/workflows/ci.yml#L173)。至少驗證一份可讓首次使用者定位並依建議處理的失敗報告，以及一份通過報告；首次使用者只能讀報告和一般 CLI 說明。多廠牌的實體相容性依既有驗收計畫追蹤。
