# CLI 維運工作流程

[English](cli-maintenance.md) | [繁體中文](cli-maintenance_zh.md)

以下功能**尚未發布**，僅適用於開發中的原始碼，不包含在已發布的 0.16.0 套件中。
所有操作均不修改攝影機設定。

| 章節 | 用途 |
| --- | --- |
| [建置](#建置) | 不替換已安裝 CLI 的執行方式 |
| [引導式工作區](#引導式工作區) | 在同一終端介面連續執行維運 |
| [Vim 風格導航](#vim-風格導航) | 數字移動、相對行號、模式及可重用核心 |
| [行號設定](#行號設定) | 預覽、單次覆寫及保存預設值 |
| [快照下載](#快照下載) | 安全保存圖片 |
| [分層診斷](#分層診斷) | 檢查 ONVIF 與圖片傳輸 |
| [設定盤點](#設定盤點) | 匯出與比較設定 |
| [自動化契約](#自動化契約) | 輸出、限制及退出碼 |
| [人工驗收](#人工驗收) | 發布前的實機檢查 |

## 建置

```sh
cargo build -p oxvif-cli --locked
```

下列範例的 `oxvif` 請替換為 `./target/debug/oxvif`；PowerShell 使用
`.\target\debug\oxvif.exe`。建置不會替換已安裝執行檔。選擇器、憑證注入、私人 CA
與全域選項請參閱 [CLI 使用指南](oxvif-cli_zh.md)。產生檔案的工作流程限單台設備；
`diagnose` 也接受既有 Group／View 選擇器。

維運選擇器及執行選項可放在指令前後，例如 `oxvif --timeout 3s diagnose front-door`
與 `oxvif diagnose front-door --timeout 3s` 等效。此規則也適用於 `--device`、
`--group`、`--view`、`--jobs`、`--retries`、`--clock-sync`、`--ca-certificate`；
指令專屬選項仍維持原作用範圍，例如 discovery 的 jobs。快照保存、診斷、匯出及比較
較慢時，僅在互動終端的 stderr 顯示經過時間；`--quiet` 可停用。JSON／JSONL 與
重新導向輸出不顯示進度，也不要求互動輸入。

## 引導式工作區

選單、結果、輸入表單及進度畫面使用水平分隔線，區分標題、內容與鍵盤操作提示。
分隔線配合終端寬度；視窗高度不足時優先省略裝飾，保留可見內容。

```sh
oxvif manage
oxvif manage front-door
oxvif manage --target 192.168.1.100 --timeout 3s
```

stdin／stdout／stderr 均須連接真實終端。`manage` 會拒絕 JSON、重新導向、非互動
及批次呼叫，不開啟介面。可選擇已存設備、明確啟動網路搜尋或直接輸入位址。
搜尋結果標示已存／新設備；直接連線及新設備僅限本次工作階段，不自動保存。
網路搜尋結果與 `discover` 共用瀏覽器：`/` 啟動即時文字搜尋、`r` 切換已儲存
篩選、`n` 切換未儲存篩選，大寫 `A` 顯示所有紀錄狀態。狀態篩選與文字條件
共同生效；`A` 保留文字條件，`c` 清除文字條件。Enter／Esc 結束搜尋編輯後，
Enter 選取目前攝影機，`q`／Esc 返回設備選單。選取新紀錄不會自動儲存。
按 `a` 可在同畫面的表單明確驗證並保存新探索設備及其憑證；manage 新增不修改
全域目前設備。已存設備選單也支援 `/` 與 `c`，比對 ID、名稱、位址與 tags；
零筆符合時仍可選網路搜尋／直接輸入。登錄狀態篩選僅適用於探索結果。

在同一次 `manage` 工作階段，從已選攝影機返回時，會還原快取的探索結果、文字與
登錄狀態篩選、選取及捲動位置。離開瀏覽器後選擇 **Return to search results
(R to rescan)** 也會重用上述狀態。在清單的一般模式按大寫 `R` 才重新掃描；
小寫 `r` 仍為已儲存篩選。成功掃描會取代結果並重設檢視；取消或失敗則保留舊結果。
標題會註明結果來自快取，內容可能已非最新狀態；退出 `manage` 即清除本次快取。
Manage 與獨立 `discover` 新增後均返回快取清單，可接續新增下一台。驗證失敗時
保留 ID／帳號、清除已提交密碼，供明確重試。本機登錄投影不重新掃描；
NEW 篩選會隱藏剛儲存的紀錄。獨立新增維持既有選為目前設備的行為。
再次執行 `discover` 仍會重新掃描，不重用 `manage` 工作階段快取。

設備選單的狀態、名稱及 ID 欄以整份清單中最長值的顯示寬度對齊，較短值在欄內
置中；各描述欄最多占 24 個終端字元格，位址維持靠左。跨頁欄位位置一致，並支援
Unicode 文字欄寬；按 `i` 可檢視完整值。
需要認證時使用 **Session credentials (not saved)**；密碼遮蔽顯示，不修改設備
清單或既有憑證。若在外部修改憑證，請重開工作區以重新載入。分享終端前應關閉工作區。

在同一程序切換 A → B → A，會還原 A 的暫存憑證、Profile、選單位置與歷史結果。
已存與直接設備的身分分開處理，即使位址相同也不共用。已存設備紀錄變更會使其
舊工作區失效；原生密碼庫內容變更仍須重開。最多保留 256 個不同設備 context，
超過時提示重開，不靜默移除既有狀態。退出即清除記憶體 context。Profile 即時
讀取後按原 token 還原；token 已消失時清除舊選取。

工作區保留設備及已選 profile，可連續診斷、檢視 profiles／設備資訊、保存快照、
匯出或比較設定。操作完成或取消憑證輸入後保留操作選項與捲動位置；返回設備選單
亦保留位置（設備清單變更時會限制於目前有效範圍）。方向鍵或 `j`／`k` 移動，
Page Up／Down 翻頁，Enter 選取，`i`
檢視項目詳情，Esc／`q` 返回；在設備選擇頁則退出。結果可捲動，並可透過
**Last result details** 再次檢視。失敗操作不取代先前完成的結果。
**Latest failure / cancellation** 另外保存最近一次直接錯誤或取消。上述報告支援
`/` 不分大小寫的一般文字逐行篩選，以及 `c` 清除；重開保留 query／捲動位置，
新證據則重設檢視。搜尋編輯中 Enter／Esc 先結束編輯，不直接離開報告。
選單、Profile 選擇及可捲動結果另支援 Ctrl+D／Ctrl+U，向下／向上移動半頁
（向下取整，至少一筆或一行），抵達邊界時停止。輸入欄位的 Ctrl+U 仍清空文字；
Page Up／Down 維持整頁移動。
路徑在同一畫面輸入，不加 shell 引號，也不展開變數；請輸入實際路徑並使用已存在
的父目錄。已存在的目的檔會被拒絕。
長輸入會水平捲動以保持游標可見，可用 Left／Right／Home／End 編輯。
目的檔驗證失敗後保留原輸入供修正，不清空重填。
位址與路徑取消後再開也會保留非機密草稿，無效值回原框修正；密碼不作草稿。
**Compare settings** 可選本設備最近成功寫出的匯出作為預填基準，或手動輸入路徑；
即時比較前重新驗證。檔案遺失或格式錯誤會返回編輯框，不把快照當設定清單基準。

Session 最長保留 60 秒，操作失敗或取消後失效；介面標示重用或重新連線，不將
快取視為在線證明。網路等待時可按 Esc／Ctrl-C 取消；完成的輸出檔可能已存在，
重試前請先確認目的檔。正常關閉 `manage` 回傳 0，各操作退出碼在結果頁獨立顯示。
自動化應使用個別命令，而非此互動介面。

Profile 詳情新增設備回報的編碼、解析度及設定 FPS 上限。缺值顯示 `not provided`，
`details_status=query_failed` 表示選用的中繼資料查詢失敗。不推測主副串流角色，
不將設定值視為實測 FPS。補充資料使用一次有界影像編碼設定操作，遵循 SOAP 重試
政策；補充查詢失敗不丟棄已取得的 profile 清單。

## Vim 風格導航

Manage 選單、獨立 Profile 選擇、Discovery 及唯讀詳情／結果檢視器共用一套導航
核心。這是 Vim 風格子集，不是文字編輯器。

| 按鍵 | 導航模式中的操作 |
| --- | --- |
| `j`／`k`、下／上方向鍵 | 移動一筆或一個顯示行。 |
| `7j`、`3k`、數字加下／上方向鍵 | 移動指定筆數或行數。 |
| `gg`、Home | 到第一筆；單按 `g` 等待第二鍵。 |
| `G`、End | 到最後一筆；文字檢視器則顯示最後一頁。 |
| `21G`、`21gg` | 到目前清單或換行後文字的第 21 筆／行。 |
| PgUp／PgDown、Ctrl+U／Ctrl+D | 整頁／半頁移動；數字前綴乘上移動量。 |
| Esc | 先取消未完成序列；否則返回／取消。 |
| Enter、`i`、`q` | 沒有前綴時，維持選取／詳情／返回等既有操作。 |
| `?` | 沒有前綴時開啟行號設定。 |

行號預設採**混合式相對數字**：`>` 選取列顯示從 1 起算的絕對序號，其他列顯示與選取列
的距離；選取列下方標示 `7` 的項目可用 `7j` 到達。這些數字不是不可變設備 ID 或
原始探索紀錄編號（另列於 `RECORD` 欄）。篩選後重新計算導航序號。
長文字以最上方可見顯示行為基準，不建立編輯游標；`21G` 在可行時把第 21 行放在
頂端。視窗尺寸改變可能重新換行並改變行號。空清單沒有可選列，位置顯示 `0/0`。

底部狀態列標示 `NORMAL`、`INPUT`、`SEARCH`、`BUSY` 或 `SETTINGS`；導航時另顯示待完成按鍵及
目前筆數／行數，例如 `NORMAL | [12g] | item 21/40 | numbers:hybrid`。反白狀態列固定於
最底列，與操作提示分離；僅於按鍵序列尚未完成時顯示待完成輸入，剩餘空間顯示
畫面／設備上下文。BUSY 顯示經過時間，不推估完成百分比。極矮視窗優先保留內容；
僅一列高時省略狀態列。頁尾的 `^D`／`^U` 表示
Ctrl+D／Ctrl+U。數字最多六位，不設輸入時限。`3i`、`gq` 或數字加 Enter 等不支援
序列會取消並顯示提示，不開啟項目或離開畫面。Ctrl+C 仍立即取消／退出。

搜尋及文字／密碼／路徑輸入不解析導航鍵：`123ggjk` 保持原文字，Ctrl+U 清空輸入。
切換畫面／模式／篩選、調整大小或收到可辨識的貼上事件時，會清除導航前綴。
導航模式忽略可辨識的貼上事件，不執行其中內容；支援 bracketed paste 的 Unix
終端會隨工作階段啟用及還原該模式。**目前 Windows 原生按鍵後端，以及將貼上送成
一般按鍵的終端，無法區分貼上與打字。請勿在導航畫面貼上命令。** 此限制不改變
輸入欄位的文字輸入行為。

Discovery 保留 `h`／`l` 翻頁別名與 `/`、`r`、`n`、`A` 篩選操作。開發版的單鍵 `g`
已改為 `gg`，Home 仍可直接到最前方；已發布的 0.16.0 套件維持舊行為。
小視窗優先減少裝飾／行號細節，保留選取列；超長攝影機名稱先截斷再對齊，避免其他
設備身分被擠出畫面。取消 Profile 前置查詢會回到動作選單，關閉取消訊息不會再啟動
另一個網路操作；真正的 Profile 查詢失敗則仍可繼續診斷其他階段。

可重用實作位於 `crates/oxvif-cli/src/navigation.rs`，不依賴 crossterm、ONVIF、
非同步工作或攝影機資料。可不透過 Cargo 測試同一份原始碼：
`rustc --edition 2024 --test crates/oxvif-cli/src/navigation.rs -o navigation-tests`
（Windows 輸出名稱加 `.exe`），再執行測試檔。目前核心隨 CLI 提供，尚非獨立發布的
crate 或穩定公開 API。

## 行號設定

此開發版功能適用於 manage 選單、獨立 Profile 選擇、Discovery 清單／詳情與文字結果，
不影響一般表格或 JSON／JSONL。

| 模式 | 選取列或最上方可見文字行 | 其他列 |
| --- | --- | --- |
| `absolute` | 從 1 起算的絕對序號 | 絕對序號 |
| `relative` | `0` | 與選取列／基準行的距離 |
| `hybrid`（預設） | 絕對序號 | 與選取列／基準行的距離 |
| `off` | 僅保留 `>` 標記 | 不顯示導航行號 |

```sh
oxvif manage --line-numbers absolute
oxvif --line-numbers off discover
```

在導航畫面按 `?`，使用 `j`／`k` 或方向鍵預覽模式；終端高度足夠時，選項下方顯示
範例。Enter 套用至目前程序，`s` 儲存並套用為預設值，Esc／`q` 取消且不變更。
若有數字／`g` 前綴，須先取消；SEARCH 及輸入欄位中的 `?` 仍為一般文字，BUSY
不提供設定快捷鍵。狀態列在寬度足夠時顯示 `numbers:<mode>`。

啟動時的優先順序為 `--line-numbers` → 已保存偏好 → `hybrid`。之後在互動介面
套用其他模式，可改變目前程序的顯示，但不寫入偏好。儲存則明確更新日後啟動的預設值，
即使本次啟動時使用覆寫選項亦然。同一程序重新開啟選單會保留已套用模式；外部修改
偏好檔後，須啟動新程序才會載入。

獨立的 `ui-preferences.json` 位於 `oxvif config path` 回報的目錄
（亦遵循 `OXVIF_CONFIG_DIR`）：

```json
{"line_numbers": "absolute"}
```

儲存使用獨立檔案鎖及原子替換，保留未知 JSON 欄位；失敗時顯示錯誤，且目前模式不變。
格式損壞的檔案不會被默默覆蓋；可用 `--line-numbers hybrid` 進入暫時工作階段，
修復檔案後再儲存。非互動／結構化命令不讀取 UI 偏好，因此損壞的 UI 檔不會阻擋 Agent。

此設定只改變顯示，不改變 `7j`、`21G`、篩選、設備 ID、Discovery 原始 `RECORD` 欄
或機器輸出。`off` 仍保留選取標記與狀態位置。行號欄寬改變時，文字可能重新換行；
顯示行基準會限制在有效範圍內，但不保證保留相同的邏輯文字位置。

## 快照下載

快照下載與 library health probe 現在共用同一個有界 HTTP 核心。HTTP/1.1
採用首字母大寫的標頭名稱，以相容錯誤處理小寫 Authorization 的韌體；Digest
參數仍遵循標準引號格式。支援多重 challenge、scheme／參數名稱大小寫，以及
空 GET 實體的 auth-int。同一安全政策下的 stale nonce 最多重試一次，包含在
總期限內；Digest 失敗不會觸發 Basic 降級。

檔頭辨識不等於完整解碼或 ONVIF 認證。ONVIF 快照要求 JPEG；接受 PNG／BMP
屬相容性擴充。HTTP 200 的純文字錯誤仍判定失敗，應檢查攝影機快照及 profile
設定；CLI 不會自動建立 MJPEG profile 或改寫廠商 CGI。

```sh
oxvif snapshot front-door --profile Profile_1 --save front-door.jpg
oxvif media snapshot-save --target 192.168.1.100 --profile Profile_1 --save front-door.jpg --output json --non-interactive
```

未指定 `--save` 時，`snapshot` 仍僅回傳 URI。快捷指令沿用唯一 profile 自動選取
或互動選取行為；正式指令必須明確指定 profile。目的檔不得已存在，且父目錄必須
已存在。完整下載後才透過暫存檔發布目的檔，不覆寫既有檔案。正常失敗會清理暫存檔；
強制終止程序或斷電不在清理保證範圍內。

結果包含 `saved_to`、`bytes`、`image_type`、`profile` 與 `validation`，不包含
圖片位元組或下載 URL。不進行轉碼，實際格式可能與指定副檔名不同。JPEG／PNG／BMP
特徵檢查可拒絕常見 HTML 登入頁與過短回應，但**不等同完整影像解碼**或確認拍攝
內容。本文上限為 16 MiB，包含分塊傳輸。圖片屬敏感資料，請選用具存取控制的目錄。

HTTP 認證與本文傳輸共用單次 `--timeout` 時限。SOAP 階段遵循 `--retries`，圖片
下載不自動重試。支援私人 CA 且不停用主機名稱驗證。HTTP 認證採挑戰方式，優先
使用 Digest，僅在明確提供 Basic 時使用 Basic，Digest 被拒絕後不降級至 Basic。
HTTP 上的 Basic 不加密憑證，應使用 HTTPS 或適當受信任的網路。

Digest Authorization 保留產生器輸出的未加引號 `qop`、`algorithm` 及 `nc`，
符合 [RFC 7616 第 3.4 節](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)。
不可將 challenge header 的引號規則套用至 client 回覆。HTTP 401 仍須確認憑證
與攝影機 HTTP 權限；ONVIF SOAP 呼叫成功不代表已取得 snapshot 存取權。

快照 URL 僅接受 HTTP(S)，主機名稱／IP 必須與設備目標完全相同（連接埠可不同），
不得含 userinfo 或 fragment，且 HTTPS 設備目標不得降級至 HTTP。圖片下載不跟隨
重新導向，也不使用環境變數設定的 HTTP proxy。其他主機、重新導向服務與內嵌憑證
應由操作者確認，不自動繞過安全檢查。

## 分層診斷

```sh
oxvif diagnose front-door --profile Profile_1
oxvif diagnose front-door
oxvif diagnose front-door --profile Profile_1 -v
oxvif diagnose --target 192.168.1.100 --profile Profile_1 --output json --non-interactive
oxvif diagnose --group taipei-f1 --profile Profile_1 --jobs 8 --output jsonl --non-interactive
```

已實作階段包括 session 建立、設備資訊、Media1 profiles、串流 URI、快照 URI 及
實際圖片下載（不保存）。Session 建立包含網路／TLS 與 ONVIF 交握，不分別量測
DNS、TCP 與 TLS。各階段提供穩定名稱、狀態、毫秒耗時、錯誤分類與後續建議。
不輸出原始 SOAP fault、下載 URL 或本文。

狀態為 `pass`、`fail`、`unsupported`、`not_tested`。設備明確回報不支援操作的
fault 會與格式錯誤回應及傳輸失敗區分。僅在唯一 profile 時自動選取。單台設備的
互動終端未提供 `--profile` 且有多個選項時，會顯示名稱與 token 的分頁選單：
方向鍵或 `j`／`k` 移動，Page Up／Down 翻頁，Home／End 跳至首尾，Enter 選取，
Esc／`q`／Ctrl-C 取消。沿用既有 session 與 profile 查詢；取消保留已完成階段，
退出碼為 `20`。明確指定不存在的 token 不會改開選單或自動替代。Agent、重新導向
與批次呼叫遇到多個選項時必須提供 token，報告保留候選資料。後續失敗不丟棄先前
結果，獨立檢查仍會繼續。

人類輸出先顯示完成狀態與計數，再列出可採取行動的失敗原因。使用 `-v` 展開所有
階段及毫秒耗時。精簡報告統一說明播放驗證限制，不將尚未測試項目視為通過。

`rtsp_transport` 與 `video_decode` 明確標示為 `not_tested`，`playback_verified`
固定為 `false`。取得 URI 不表示 RTSP 連線／認證、封包傳輸、影格率或播放成功；
選用的外部解碼器整合仍列為後續工作。

各 SOAP 階段分別套用每次嘗試的 `--timeout` 與 `--retries`，整體工作流程可能
需要數個逾時週期。`complete` 僅涵蓋已實作檢查，不包括尚未實作的播放驗證；
退出碼為 `0` 時仍應檢查各階段狀態。

## 設定盤點

```sh
oxvif config export front-door --save baseline.json
oxvif config diff front-door --against baseline.json --output json --non-interactive
```

兩個指令也接受 `--target` 或 `--device`，在網路操作前拒絕 Group／View 選擇器。
既有 `config path`／`config validate` 仍處理本機 CLI 狀態；export／diff 則讀取
攝影機的主機名稱、NTP、DNS、網路介面、協定、閘道、Media1 profiles 與影像編碼設定。

獨立基準檔包含 `inventory_version: 1`、設備 `identity` 與具名 `sections`（狀態、
耗時、建議及選用資料），不是 stdout envelope 或 discovery snapshot。檔案上限
為 4 MiB，不覆寫既有檔案。不完整匯出仍保存供調查，並標示 `complete=false`。
不收集密碼、即時媒體 URL、日誌或持續變動的時鐘讀值；檔案仍含敏感網路拓樸與
設備身分資訊。

這是有限範圍的盤點，**不是可還原備份**。沒有 apply／restore 指令，亦不涵蓋未
列出的設定。比較要求製造商、型號、序號及硬體身分一致，且序號不得為空。僅比較
區段資料，不比較耗時。具識別欄位的記錄清單會排序，DNS／NTP 偏好順序則保留。
差異採 JSON Pointer 路徑與 `before`、`after`、`before_present`、`after_present`，
不存在的欄位與明確 JSON `null` 會分別表示。
人類輸出採欄位／前值／後值排列，以 `<missing>` 與 `null` 區分不存在與空值。
完整且相同的比較顯示 `No configuration changes.`；不完整比較不宣稱整體一致。

失敗／不支援區段列入 `incomparable_sections`，`matches` 為 `null`，不誤判相同。
完整比較即使發現設定不同仍回傳退出碼 `0`，呼叫端應檢查 `matches` 與 `changes`。

## 自動化契約

人類與 Agent 使用共用的型別化請求。自動化應指定明確目標、`--output json` 或
`jsonl`，以及 `--non-interactive`。以實際執行檔的 `describe` 查詢能力；目前開發版
內建 Agent guide 為版本 8。基礎 stdout envelope 維持 schema 版本 3。新增操作
回傳 `device_diagnostic`；批次診斷使用 `fleet_diagnostic` 或 JSONL `fleet_item`
記錄，最後附上 `fleet_summary`。

診斷新增 `summary` 計數（`passed`、`failed`、`unsupported`、`not_tested`）與可為
null 的 `selected_profile`。Profile 查詢／選取資料提供含 `name`、`token` 的
`candidates`，原有 `profiles` token 清單仍保留。未測試階段以 `not_tested_reason`
區分 `prerequisite_failed`（先決條件失敗）與 `not_implemented`（尚未實作播放檢查）。

為維持相容，選取階段的 `error_code` 仍為 `PROFILE_SELECTION_REQUIRED`；精確原因
請讀取該階段的 `data.reason_code`：

| 原因 | 意義 |
| --- | --- |
| `PROFILE_SELECTION_REQUIRED` | 多個候選項目，需提供 token |
| `PROFILE_NOT_FOUND` | 指定或介面回傳的 token 不存在 |
| `PROFILE_QUERY_FAILED` | Profile 查詢未取得可用資料 |
| `NO_PROFILES_AVAILABLE` | 查詢成功，但清單為空 |
| `PROFILE_SELECTION_CANCELLED` | 使用者取消，保留先前檢查 |
| `PROFILE_INTERACTION_FAILED` | 終端介面失敗，保留先前檢查 |

以上欄位均為增補；schema 版本 3 與既有退出碼意義維持不變。

`assessment` 新增 `primary_issue`、`additional_issues`、`blocked_checks` 及
`limitations`。每項問題包含 `stage`、`code`、`observed`、`suggested_action`
及 `certainty=observed_failure_not_root_cause`。主要問題是最先失敗的階段，不是
已證實的根本原因；逾時不代表密碼錯誤。人類摘要使用同一份判讀資料。
`media.profiles` 保留原陣列及欄位，各筆新增可為 null 的 `video` 與
`details_status`；診斷候選資料提供相同中繼資料。`video.source` 明確標示
`device_configuration_not_measured`，不表示實測播放品質。

| 結果 | 退出碼 | 說明 |
| --- | ---: | --- |
| 快照／匯出／診斷完成 | 0 | 檔案資訊或報告，仍須檢查限制 |
| 比較完整，不論相同或不同 | 0 | 檢查 `matches` 與 `changes` |
| 診斷或盤點失敗／不完整 | 20 | `ok=false`，報告可保留於 `data.result` 而非頂層 `error` |
| 批次診斷部分成功 | 6 | 保留所有逐台結果 |
| 批次診斷全部失敗 | 20 | 與既有診斷不同，仍保留報告 |
| 目的檔已存在 | 4 | 不覆寫 |
| 基準檔／身分無效或選擇器不支援 | 2 | 不執行比較或替換檔案 |

其他既有憑證、檔案 I/O 與連線錯誤碼仍適用。為避免目的檔衝突，檔案工作流程限單台
設備。診斷應檢查 `failed`、`complete` 與各階段狀態，不可假設 `ok=false` 一定含
頂層 `error`。失敗的 fleet 項目可同時保留錯誤摘要與階段報告。既有 URI 查詢、
PTZ 與 health 執行行為不變。由於 `snapshot` 現在可以選擇保存檔案，指令描述採
保守分類，標示本機寫入風險且不可自動重試整個指令；`mutates_device` 仍為 false。
`media snapshot-uri` 維持明確的唯讀操作。

## 人工驗收

僅使用經授權的目標與測試輸出目錄，不在公開 issue 貼上憑證、攝影機畫面或未遮蔽
的設定盤點。

1. 從實機下載並開啟圖片，確認格式符合 `image_type`；使用相同檔名再執行一次，
   確認原檔未改動。
2. 測試挑戰式認證及私人 CA HTTPS 攝影機，確認錯誤 CA／主機名稱與不安全 URL
   會被拒絕，且不洩漏憑證。
3. 使用正確／錯誤憑證與多個 profiles 執行診斷，確認保留的階段結果符合實機行為，
   而非被誤讀為播放驗證。
   測試選單選取／取消、終端縮放、`-v`、重新導向，以及未加 `--non-interactive`
   的 JSON 呼叫；自動化不得出現互動提示。
4. 設定不變時匯出並比較，再針對經授權且已獨立修改的設定比較，確認預期路徑有差異。
5. 測試不支援設定及混合上線／離線群組，以 Agent 與人類輸出分別核對退出碼及報告。
6. 發布前完成 Windows／macOS／Linux 原生 CI 與安裝檢查。
7. 在同時有已存與新設備的環境，於 `manage` 選擇 **Search network for cameras**。
   以 `/` 搜尋已存 ID，再測試 `r`、`n`、`A` 與 `c`，確認空結果可恢復，且 Enter
   選取畫面所示攝影機。選取新攝影機不得自動註冊。檢查狀態列與提示分離、半頁
   操作、視窗縮放、一般 Unicode／IME 輸入、取消憑證輸入及選單位置保留，並確認
   Esc、`q` 與 Ctrl+C 後終端狀態正確恢復。

本機 mock 測試不代表實機互通性或三平台驗收。現有證據與待驗項目請參閱
[實作計畫](https://github.com/smiti1642/oxvif/blob/ddec9ecfc69d503c54c487c28feb64fdad32dca5/docs/active/cli-maintenance-workflows-plan_zh.md)。
