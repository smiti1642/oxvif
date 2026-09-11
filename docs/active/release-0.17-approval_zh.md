# 0.17 正式版號提交前審查資料

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

狀態：IN-PROGRESS／尚未核准。更新日期：2026-09-12。
本資料補充[發布切點](release-0.17-cut_zh.md)，不取代其中的阻擋關卡。
使用者要求停在正式 0.17 版號／發布 commit 前。允許一般修正及證據提交；
本次驗收不執行版號升級、主分支合併、tag、發布、PR 關閉或本機系統安裝。

| 章節 | 用途 |
| --- | --- |
| [四項工作](#四項工作) | 區分各種證據的目前狀態 |
| [審查發現](#審查發現) | 修正與未結案結果 |
| [已執行檢查](#已執行檢查) | 本機、託管與實機觀察 |
| [審查結案清單](#審查結案清單) | 已完成的本機審查及剩餘發布關卡 |
| [維護者操作](#維護者操作) | 不發布的 CI 與安裝 staging |
| [正式版號修改清單](#正式版號修改清單) | 留待核准後提交的修改 |
| [核准邊界](#核准邊界) | 正式版號提交前的條件 |

## 四項工作

| 工作 | 狀態 | 剩餘項目 |
| --- | --- | --- |
| 1. 完整候選／安全審查 | LOCAL-PASS | 208 個固定路徑及後續差異均核對受影響消費端／斷言／宣稱；A01–A05 已修復，T01／T02 已加強，詳見下方結案紀錄 |
| 2. 套件安裝與人類／實機驗收 | PARTIAL | 3eccfd1 原生 CI 通過；之後修正須重跑。實機 export／diff 與修復後快照驗收通過，限制如下；Hanwha 非影像回應仍為限制。有界 Windows ConPTY 通過；其餘人工／平台驗收與散布 staging 待驗 |
| 3. 版號、連結與文件 | 已準備，尚未升版 | 雙語發布紀錄及本清單已更新；實際版號仍為 0.16.0，核准後才套用版號修改清單 |
| 4. 使用者確認 | 尚未請求正式提交核准 | 正式版號 commit 前呈現最終證據及風險；發布須另行授權 |

## 審查發現

| ID | 觀察 | 處理方式 |
| --- | --- | --- |
| A01 | HTTP lossy UTF-8 decoding 將 FF 位元組轉成 U+FFFD，成功建立不同名稱的 profile | 6135e72 修復：responder 前回傳 HTTP 400／Sender／mock:RequestPolicy。五類無效位元組、合法 Unicode、完整狀態／hook 保留及 queued fault 控制均本機通過 |
| A02 | Snapshot Authorization 把 `qop=auth` 改成帶引號值 | 移除改寫，斷言未加引號 qop／algorithm／nc 及精確 URI；依據 [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)，不增加設備特例或認證降級 |
| A03 | 已儲存攝影機的兩個 profile 在 A02 修正前後皆回 HTTP 401；擴大測試另發現 18 台類似失敗 | 已本機修復：HTTP/1.1 欄位名稱採 Title-Case，處理韌體錯誤區分大小寫；CLI／health 共用 Digest 核心並保留認證與目的地安全限制。原始攝影機兩個 profile 已保存及解碼成功。見[修復證據](snapshot-auth-repair_zh.md)；託管發布閘門仍未完成 |
| A04 | 人類輸出會原樣送出攝影機／profile 文字、詳細報告與錯誤提示中的終端控制序列 | 已本機修復：在人類報告邊界及單行選單／上下文欄位轉義 C0／C1 與雙向文字格式控制字元；JSON／JSONL 資料值不變。報告仍允許 LF／TAB 排版，不代表消除所有多行顯示歧義 |
| A05 | 全螢幕繪製仍輸出方向格式控制字元 | 214d989 已修復：寬度／截斷／換行／cell 排版前轉義，選取資料保留原值；修正前斷言失敗 |
| C01 | Windows native 失敗可能被後續成功命令掩蓋 | 210bfc3 已修復：四個 CI、十四個 release exit guard；本機正負控制通過，仍須最終託管執行 |

A01 不代表一般 HTTP binding／charset／fault status 稽核完成；僅 A02 並未解決 A03。
下列 A03 結果僅涵蓋受測設備與 profile，不代表普遍相容性；剩餘項目仍須明列於
[後續清單](post-0.17-backlog_zh.md)。

## 已執行檢查

| 證據 | 結果及限制 |
| --- | --- |
| 最終本機結案，2026-09-12 | Workspace all-feature／default：1,310／1,198 通過，各五項 ignored、41 suites；兩組 all-target Clippy、strict rustdoc、fmt 及 Rust 1.88 all-target／all-feature 檢查通過。[A05／T01／T02 及 native exit 證據](release-0.17-review_zh.md#a05-終端顯示與-t01-斷言後續) |
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

## 審查結案清單

G01／G03 對 R01–R08 為 LOCAL-PASS。[逐檔清冊](release-0.17-review-ledger.json)
列出 `v0.16.0..b7bc881` 的 208 個 reviewed 路徑，後續差異另行審查。
各已審輸入有 blob ID，清冊本身以所屬 commit 識別。
[批次審查](release-0.17-review_zh.md#批次順序) 六組記錄原始碼、受影響消費端、
斷言及公開宣稱的核對。

較早的 `v0.16.0..3eccfd1` 比較包含 200 檔、新增 39,614 行、刪除 3,007 行，
屬歷史檢查點，不是最終審查輸入。A01–A05 修正、T01／T02 測試品質、CI／release
guard 控制及雙語宣稱校正已完成本機可處理的發現；本次審查未留下收錄切點內
已知且未處理的重大 blocker。

這是有界工程審查，不是獨立全程式或 ONVIF 認證。完整 HTTP／field／Fault 語意、
沿用 listener 安全、raw recording 隱私限制、併發 replay 可見性及 snapshot 格式
相容性，仍於[後續清單](post-0.17-backlog_zh.md) 明列範圍。
歷史外部 corpus／實機證據只適用於相符輸入。G05／G06 須以精確新候選執行原生 CI
及不發布 staging；G07 仍部分完成，G08／G09 保留核准邊界。

## 維護者操作

目前 GitHub CLI 帳號僅有 repository 讀取權限。不得修改 trigger、換帳號或
開 workaround PR 來啟動 workflow。一般修正提交推送後，維護者可執行：

```powershell
git fetch origin codex/release-0.17-closure
$candidate = git rev-parse origin/codex/release-0.17-closure
gh workflow run ci.yml --ref codex/release-0.17-closure
gh workflow run release.yml --ref codex/release-0.17-closure -f tag=$candidate -F publish=false -F prerelease=true
```

記錄每個 run URL 及實際 checkout SHA；dispatch 後分支移動不可默默改變驗收候選。
Staging 的 tag 輸入使用 filename-safe commit SHA，不使用含斜線的分支名稱。
此輸入雖名為 tag，指令不會建立 Git tag；`publish` 必須保持 false。

須驗證 Windows／Linux／macOS archives、checksum、source／binary SBOM、
Debian package install／remove、兩種 Linux 架構的暫時簽章 APT repository 安裝，
以及兩種 Mac 架構的 Homebrew formula／bottle install／reinstall。
這些安裝發生於 CI runner，不修改使用者機器。目前 0.16 版號的 staging 不可取代
最終 0.17 package 驗證；官方渠道收錄及正式 APT signing key 仍是獨立工作。

本次有界 Windows ConPTY 已驗收上述 manage／profile／input／resize、開始後的
snapshot 取消及 console 恢復。先前 Vim／discovery 證據保留原始版本；仍須完成
最終候選的 discover／diagnose navigation、filter／數字／gg／G／Ctrl-D／U／行號
矩陣及其他平台人工驗收，因此 G07 保持 PARTIAL。實機 snapshot 成功沿用已授權
設備的證據，不表示本次重新掃描設備群。

## 正式版號修改清單

以下保留至另行明確核准的版號 commit：

| 位置 | 修改／檢查 |
| --- | --- |
| Cargo.toml、crates/oxvif-cli/Cargo.toml、Cargo.lock | 同步 workspace library／CLI 與 CLI library dependency 為 0.17.0；驗 metadata 與 package 相依解析 |
| CHANGELOG.md、docs/releases/0.17.0{,_zh}.md 及完整 changelog 雙語檔 | 核准後才改 draft status／日期；更新索引與最終 tag 連結，維持簡要摘要與完整遷移分離 |
| README 雙語檔、CLI package README | 將下一版提示改成 CLI 導向發布摘要及絕對 blob/v0.17.0 連結；保留 OnvifSession 與 OnvifClient 兩種 Quick Start |
| LIBRARY_GUIDE 雙語檔、src/lib.rs | 更新安裝範例與公開遷移說明；保留正確的歷史比較引用 |
| docs/oxvif-cli、docs/cli-maintenance、docs/support 雙語檔與 packaging/oxvif.1 | 同步支援版號、指令、唯讀診斷限制、導航與實際通過的安裝渠道 |
| CLI guide／describe／schema | 分別驗 CLI version=0.17.0、Agent guide version 與 schema version；不因 package 升版就改 schema |
| 套件驗證 | 依真正發布相依鏈驗證兩個 package；CLI --list 或 workspace build 不足。Library 必須先可取得，才能發布依賴它的 CLI |
| 最終凍結候選 | 重驗受影響本機／原生／staging／版號／連結關卡並保留 hash；合併衝突解決後重新驗證 |

不可機械式替換所有 0.16 引用：已發布歷史、遷移比較及固定測試資料可能刻意保留舊版。

## 核准邊界

正式版號 commit 前，提供已完成的 G01／G03、精確本機與託管結果、staging 證據、
人類／實機驗收、已解決的 A03、預定版號／連結 diff 及明確限制。
缺少任何必要關卡時必須照實標示，不得宣稱四項全部完成。

PR #14／#16／#17 仍為 open。候選已包含調整後且保留署名的實作；不可再次合併
舊實作，也不可當作 master／develop 已包含這些工作而關閉 PR。
主分支整合及 PR 結案須在核准後處理，與發布授權分開。
