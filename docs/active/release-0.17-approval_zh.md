# 0.17 發布核准資料

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

更新日期：2026-09-14。使用者已授權準備 0.17 版號／文件，不等於對外發布授權。
目前收錄已合併的 CLI 與基礎 Mock Fleet；master／develop 在準備前均為
`d3ac1b6`。目前分支為 `codex/release-0.17-finalize`。
本頁歷史證據保留原版本；目前關卡以[切點](release-0.17-cut_zh.md)及
[收尾紀錄](release-0.17-finalization_zh.md)為準。

| 章節 | 用途 |
| --- | --- |
| [使用者要求的四項工作](#使用者要求的四項工作) | 目前狀態 |
| [審查發現](#審查發現) | 歷史修復及限制 |
| [已執行檢查](#已執行檢查) | 指定版本證據 |
| [審查結案](#審查結案) | 本次差異審查 |
| [維護者操作](#維護者操作) | CI／staging |
| [版號修改清單](#版號修改清單) | 準備成果 |
| [核准邊界](#核准邊界) | 真正發布前停止 |

<a id="四項工作"></a>

## 使用者要求的四項工作

| 工作 | 目前狀態 |
| --- | --- |
| 1. 候選／安全審查 | 基準及本次有限 CLI／Fleet 差異 LOCAL-PASS；非整體 ONVIF 認證 |
| 2. 套件、安裝與人工驗收 | 0.17 兩個套件本機驗證、27 CI jobs、17 staging jobs 通過；使用者回報人工 PASS；精確結果見收尾紀錄 |
| 3. 版號、連結與文件 | 兩套件 0.17.0；公開英／繁中指南同步；日期及 Unreleased 防護保留 |
| 4. 使用者確認 | 準備已授權；對外發布另行確認 |

下列發現及驗證表為歷史、指定版本的紀錄；其中舊「待執行」狀態不覆蓋上表。

## 審查發現

| ID | 觀察 | 處理方式 |
| --- | --- | --- |
| A01 | HTTP lossy UTF-8 decoding 將 FF 位元組轉成 U+FFFD，成功建立不同名稱的 profile | 6135e72 修復：responder 前回傳 HTTP 400／Sender／mock:RequestPolicy。五類無效位元組、合法 Unicode、完整狀態／hook 保留及 queued fault 控制均本機通過 |
| A02 | Snapshot Authorization 把 `qop=auth` 改成帶引號值 | 移除改寫，斷言未加引號 qop／algorithm／nc 及精確 URI；依據 [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)，不增加設備特例或認證降級 |
| A03 | 已儲存攝影機的兩個 profile 在 A02 修正前後皆回 HTTP 401；擴大測試另發現 18 台類似失敗 | 已本機修復：HTTP/1.1 欄位名稱採 Title-Case，處理韌體錯誤區分大小寫；CLI／health 共用 Digest 核心並保留認證與目的地安全限制。原始攝影機兩個 profile 已保存及解碼成功。見[修復證據](snapshot-auth-repair_zh.md)；託管發布閘門仍未完成 |
| A04 | 人類輸出會原樣送出攝影機／profile 文字、詳細報告與錯誤提示中的終端控制序列 | 已本機修復：在人類報告邊界及單行選單／上下文欄位轉義 C0／C1 與雙向文字格式控制字元；JSON／JSONL 資料值不變。報告仍允許 LF／TAB 排版，不代表消除所有多行顯示歧義 |
| A05 | 全螢幕繪製仍輸出方向格式控制字元 | 214d989 已修復：寬度／截斷／換行／cell 排版前轉義，選取資料保留原值；修正前斷言失敗 |
| C01 | Windows native 失敗可能被後續成功命令掩蓋 | 210bfc3 已修復：四個 CI、十四個 release exit guard；本機正負控制通過，fed6777 託管 CI 已通過，release workflow staging 仍待執行 |

A01 不代表一般 HTTP binding／charset／fault status 稽核完成；僅 A02 並未解決 A03。
下列 A03 結果僅涵蓋受測設備與 profile，不代表普遍相容性；剩餘項目仍須明列於
[後續清單](post-0.17-backlog_zh.md)。

## 已執行檢查

| 證據 | 結果及限制 |
| --- | --- |
| 最新合併開發批次，6cf345c | 全功能 1,316 通過／0 失敗／6 跳過；預設 1,204／0／6，各 41 suites。兩組 Clippy／strict rustdoc 及 fmt 通過。三個範例測試與 64／256 台 loopback 容量測試另行通過，見[六項排除及邊界](mock-fleet-basic-plan_zh.md#跳過測試明細)；沒有新增 MSRV／schema／原生 CI 結果 |
| CLI 重新驗收，dcbff41 | 全功能／預設 1,311／1,199 通過，零失敗、各五項跳過。Windows ConPTY 驗證實際 manage 探索文字／紀錄篩選、正確已存／session-only 選取、狀態列、選單位置、縮放及取消／恢復，見[範圍](cli-0.17-reentry_zh.md#驗收) |
| Fleet 終端機，B4 | 兩輪四台前景 serve；CLI 核對每台設定身分，Ctrl+C 零退出、埠可重新繫結、manifest hash 不變。僅限 loopback；原生 multicast／VMS 未驗收 |
| CLI 重新納入前的歷史本機結案，2026-09-12 | Workspace all-feature／default：1,310／1,198 通過，各五項 ignored、41 suites；兩組 all-target Clippy、strict rustdoc、fmt 及 Rust 1.88 all-target／all-feature 檢查通過。[A05／T01／T02 及 native exit 證據](release-0.17-review_zh.md#a05-終端顯示與-t01-斷言後續) |
| Windows debug ConPTY | 40 台合成裝置，manage／detail／profile／input／password／resize、已開始的 snapshot 取消、正常／Ctrl-C 退出的精確 console mode 恢復均通過；設定 hash 不變。未新增攝影機掃描或 host 安裝；G07 剩餘範圍如下 |
| V01 擴充 encoder replay，2026-09-12 | 停用失效邏輯後 1,308 通過／四項斷言失敗／五項 ignored，包含兩項新測試及兩項既有 rate 測試。還原後 all-feature 1,312／default 1,200 通過，各五項 ignored、41 suites。產品程式與 1ea4fff 相同；見[範圍](release-0.17-review_zh.md#v01-編碼器-replay-覆蓋)。此為歷史 V01 結果；最終本機結案記於下方 |
| A04 人類輸出修正，2026-09-12 | 修正前 1,308 通過／兩項斷言失敗／五項 ignored；修正後 all-feature 1,310／default 1,200 通過，各五項 ignored、41 suites；兩組 Clippy 與 strict rustdoc 通過。實際 debug 執行檔保留 JSON／JSONL 原值及錯誤 exit 3，人類 stderr 已轉義。見[批次紀錄](release-0.17-review_zh.md#a04-人類輸出修正)；不是全螢幕終端驗收 |
| 3eccfd15274d4e978501634e4155477760502b6b 託管 CI | [Run 34597167495](https://github.com/smiti1642/oxvif/actions/runs/34597167495)：全部 27 個 job、五種原生目標通過；不涵蓋後續修正 |
| A01 敏感度驗證 | 完整 workspace all-features、no-fail-fast：1,301 通過／兩項斷言失敗／五項 ignored；實際觀察到非預期狀態修改 |
| A01 修正後關卡 | all-feature 1,303／default 1,193 通過，各五項 ignored、41 suites；兩組 Clippy、strict rustdoc、fmt、本機連結及發布歷史檢查通過 |
| A02 敏感度驗證 | 完整 workspace all-features、no-fail-fast：1,302 通過／一項斷言失敗／五項 ignored；輸出的 quoted qop 使 wire-format 斷言失敗 |
| A02 修正後關卡 | all-feature 1,303／default 1,193 通過，各五項 ignored、41 suites；兩組 workspace Clippy 通過。公開文件未變，沿用 A01 strict rustdoc 結果；本次格式／連結檢查隨提交記錄 |
| 新執行的相依檢查 | cargo audit：410 個 locked package，未回報弱點。cargo outdated 列出 reqwest 0.13.5、tokio-rustls 0.26.5、toml 1.1.6 及 dev-only dirs 7；keyring 4 遷移產生 obsolete-feature 警告。未改 lockfile；升級須獨立審查，不在發布前自動更新 |
| Windows x64 實機 | GeoVision_2 GV-TBL8810、firmware V111_2025_12_09；info 與兩個 profile 查詢 exit 0 |
| 共用修復前：兩個 profile 的實機 diagnose | exit 20、complete=false：五階段通過，snapshot_fetch 回 401，RTSP transport／decode 為 not_tested。失敗仍保留部分證據，不宣稱播放成功 |
| 共用修復前：人類表格／Agent 版號 | 純文字表格同樣 exit 20，指出 401／部分結果／播放限制；不是互動終端測試。實際執行的 guide 回報 CLI 0.16.0、schema 3、guide 8 |
| 實機 export／diff | 八個 section 通過；export exit 0／complete=true；再次寫同一路徑為 exit 4／RESOURCE_ALREADY_EXISTS，檔案 hash 不變；live diff exit 0／complete=true／matches=true、零差異 |
| 隱私邊界 | 僅執行唯讀攝影機操作；baseline 保留於 Git 外的私人暫存目錄。本文件不記錄 IP、序號、憑證、URI、Digest challenge 或影像 |
| 先前文件／清冊 | fmt 與 diff 空白通過；檢查 123 個本機 Markdown 目標，已發布 CHANGELOG 歷史未變。清冊 self-test 通過：159 routes、161 Action sites、191 readers；不代表規範或完整相依圖驗收 |

以上實機列為修復前基準；本批次更新證據如下：

| 共用修復，Windows x64 | 結果及限制 |
| --- | --- |
| 候選 CLI SHA-256 | `9F2856CC727596791945A2DCC3F0243E3FB1ECA2CB09D054B5C0E894FEA1DF97`；一般本機 debug 建置，非已發布 release binary |
| 完整敏感度測試 | 故意破壞送出的 Digest response：1,306 通過／兩項斷言失敗／五項 ignored，41 suites；精確還原正式原始碼，並非僅編譯失敗 |
| 還原後閘門 | all-feature 1,308／default 1,198 通過，各五項 ignored、41 suites；兩組 workspace Clippy、strict rustdoc 及 fmt 通過 |
| 先前失敗設備 | 18 台各抽樣第一個 profile：17 台在 8 秒預算下成功；一台逾時，單次改用 20 秒預算複測成功。18 台皆取得 JPEG signature，但不能將首輪描述為 18/18 |
| 正常控制組／Hanwha | 控制組持續成功；Hanwha XND-C6083RV 在 20 秒複測後仍被判定非影像，不自動建立 MJPEG profile 或修改 CGI |
| 原始已儲存 GV-TBL8810 | 兩個 profile 的 diagnose 皆 exit 0／complete=true，保存 exit 0；System.Drawing 獨立解碼確認 640×360 圖片。重複保存 exit 4，各檔案 hash 保持不變 |
| 隱私／解讀 | 兩張影像保留於 Git 外私人暫存目錄，未改攝影機設定。RTSP transport／video decode 仍為 not_tested；快照抽樣不代表全品牌或全部 profile 相容 |

公開證據不包含 IP、憑證、URI/query、認證標頭或實機影像。
本機修復結果不取代託管、互動終端或散布套件 staging 驗收。

<a id="審查結案清單"></a>

## 審查結案

原 208 路徑清冊保留原 blob 證據；`5a1821b..d3ac1b6` 的 CLI／Fleet
差異另記於[收尾審查](release-0.17-finalization_zh.md#差異審查)，不因測試通過
默默擴充清冊。已知子集合、快照格式、錄製隱私及 listener 限制仍見 backlog。

## 維護者操作

使用授權的 `smiti1642` 帳號。以下只執行 CI 與暫存 Actions artifacts，不建立
tag、不上傳 crates.io、不建立 GitHub Release，也不安裝至使用者系統。

```powershell
$candidateBranch = 'codex/release-0.17-finalize'
git fetch origin $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'Fetch failed' }
$candidate = git rev-parse "origin/$candidateBranch"
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve candidate' }
gh workflow run ci.yml --ref $candidateBranch
if ($LASTEXITCODE -ne 0) { throw 'CI dispatch failed' }
gh workflow run release.yml --ref $candidateBranch -f tag=$candidate -F publish=false -F prerelease=true
if ($LASTEXITCODE -ne 0) { throw 'Staging dispatch failed' }
```

記錄精確 SHA 及 URL。要求五種原生套件、credential、checksum、兩類 SBOM、
APT install/remove、Homebrew install/bottle/reinstall 全部通過；不代表已上官方通路。

<a id="正式版號修改清單"></a>

## 版號修改清單

已同步 workspace／CLI 相依／lockfile 為 0.17.0，更新 README、指南、man page、
簡要 Release 及完整 Changelog。Agent schema 3／guide 8 不因套件改版而遞增。
本機 workspace 打包使用 Cargo 暫存 registry，確實編譯打包後的 CLI 及 library；
實際發布仍須先讓 library 在 crates.io 可取得，再發布 CLI。

## 核准邊界

CI 與 staging 通過後提出精確證據及限制，停止等待對外發布同意。
核准後才能完成日期／狀態、cargo publish、tag 及 GitHub Release。
PR #14／#16／#17 已透過保留貢獻者資訊的改寫整合，不是 GitHub 原 PR merge；
不得重複整合。先前 CLI／Fleet 分支已合併且清理，不再使用舊分支操作指令。
