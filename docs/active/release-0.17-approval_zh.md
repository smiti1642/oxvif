# 0.17 正式版號提交前審查資料

[English](release-0.17-approval.md) | [繁體中文](release-0.17-approval_zh.md)

狀態：IN-PROGRESS／尚未核准。更新日期：2026-09-11。
本資料補充[發布切點](release-0.17-cut_zh.md)，不取代其中的阻擋關卡。
使用者要求停在正式 0.17 版號／發布 commit 前。允許一般修正及證據提交；
本次驗收不執行版號升級、主分支合併、tag、發布、PR 關閉或本機系統安裝。

| 章節 | 用途 |
| --- | --- |
| [四項工作](#四項工作) | 區分各種證據的目前狀態 |
| [審查發現](#審查發現) | 修正與未結案結果 |
| [已執行檢查](#已執行檢查) | 本機、託管與實機觀察 |
| [審查結案清單](#審查結案清單) | 剩餘原始碼審查 |
| [維護者操作](#維護者操作) | 不發布的 CI 與安裝 staging |
| [正式版號修改清單](#正式版號修改清單) | 留待核准後提交的修改 |
| [核准邊界](#核准邊界) | 正式版號提交前的條件 |

## 四項工作

| 工作 | 狀態 | 剩餘項目 |
| --- | --- | --- |
| 1. 完整候選／安全審查 | IN-PROGRESS | A01、A02、A03 已本機修復；須完成下方全部差異及相依使用者的結案 |
| 2. 套件安裝與人類／實機驗收 | PARTIAL | 3eccfd1 原生 CI 通過；之後修正須重跑。實機 export／diff 與修復後快照驗收通過，限制如下；Hanwha 非影像回應仍為限制。最終終端與散布 staging 待驗 |
| 3. 版號、連結與文件 | 已準備，尚未升版 | 雙語發布紀錄及本清單已更新；實際版號仍為 0.16.0，核准後才套用版號修改清單 |
| 4. 使用者確認 | 尚未請求正式提交核准 | 正式版號 commit 前呈現最終證據及風險；發布須另行授權 |

## 審查發現

| ID | 觀察 | 處理方式 |
| --- | --- | --- |
| A01 | HTTP lossy UTF-8 decoding 將 FF 位元組轉成 U+FFFD，成功建立不同名稱的 profile | 6135e72 修復：responder 前回傳 HTTP 400／Sender／mock:RequestPolicy。五類無效位元組、合法 Unicode、完整狀態／hook 保留及 queued fault 控制均本機通過 |
| A02 | Snapshot Authorization 把 `qop=auth` 改成帶引號值 | 移除改寫，斷言未加引號 qop／algorithm／nc 及精確 URI；依據 [RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)，不增加設備特例或認證降級 |
| A03 | 已儲存攝影機的兩個 profile 在 A02 修正前後皆回 HTTP 401；擴大測試另發現 18 台類似失敗 | 已本機修復：HTTP/1.1 欄位名稱採 Title-Case，處理韌體錯誤區分大小寫；CLI／health 共用 Digest 核心並保留認證與目的地安全限制。原始攝影機兩個 profile 已保存及解碼成功。見[修復證據](snapshot-auth-repair_zh.md)；託管發布閘門仍未完成 |

A01 不代表一般 HTTP binding／charset／fault status 稽核完成；僅 A02 並未解決 A03。
下列 A03 結果僅涵蓋受測設備與 profile，不代表普遍相容性；剩餘項目仍須明列於
[後續清單](post-0.17-backlog_zh.md)。

## 已執行檢查

| 證據 | 結果及限制 |
| --- | --- |
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

精確審查基準為 `git diff v0.16.0 3eccfd1`：200 個檔案變更、新增 39,614 行、
刪除 3,007 行；之後的修正與證據提交須另計差異。清冊或綠色測試不等於全部差異審查。

| 群組 | 目前證據 | 最終結案要求 |
| --- | --- | --- |
| CLI application／maintenance／manage、navigation／preferences、Agent／exit 契約 | 已檢視產品入口，已有回歸及上述實機控制 | 一起完成 main／interactive／output／registry／schema／descriptor 差異審查；檢查終端生命週期、控制字元、明確 selector 及本機檔案副作用 |
| Library Media1／2、session、notification peer、XML／type 遷移 | 社群與操作工作卡；已檢視 additive peer wrapper 及遷移入口 | 將公開方法／型別／reexport／example／相容文件全部對照最終差異；不以局部修正宣稱完整 XML 驗證 |
| Mock request／auth／fault／dispatch 及服務 handler | 既有操作清冊與雙語批次證據；A01 獨立重現 | 確認每個收錄批次的讀寫／options／capability／replay 關聯、受 shared helper 影響的未修改 handler，並結案其餘 HTTP／fault 語意判定 |
| Metamorph storage／report／replay／adapter | K27 修復及目前資料保留測試 | 完成其餘 fixture／parse／canonicalization／adapter／replay 差異與降版文件審查；粗粒度 invalidation 仍為明確子集合 |
| 相依、workflow 及 packaging | 原生 CI 證據；已檢視 CI／release／SBOM／formula 修改 | 驗證 source／tooling ref、staging artifact、checksum、source／binary SBOM 區別及安裝後指令；完成 schema verifier／工具來源審查 |
| 文件與測試斷言 | 雙語發布／遷移紀錄及歷史證據保留 | 核對 README／指南／support／manpage 的目前宣稱，區分 synthetic、獨立 corpus 與實機；每個變更檔案均須歸入已審查群組，不能只看列出的入口 |

上述結案前，G01／G03 維持 open。只有輸入完全相同時，才可重用既有批次原始碼
審查與外部 corpus 結果；不得表示成新執行的全程式獨立稽核。

## 維護者操作

目前 GitHub CLI 帳號僅有 repository 讀取權限。不得修改 trigger、換帳號或
開 workaround PR 來啟動 workflow。一般修正提交推送後，維護者可執行：

```powershell
git fetch origin codex/contributor-pr-integration
$candidate = git rev-parse origin/codex/contributor-pr-integration
gh workflow run ci.yml --ref codex/contributor-pr-integration
gh workflow run release.yml --ref codex/contributor-pr-integration -f tag=$candidate -F publish=false -F prerelease=true
```

記錄每個 run URL 及實際 checkout SHA；dispatch 後分支移動不可默默改變驗收候選。
Staging 的 tag 輸入使用 filename-safe commit SHA，不使用含斜線的分支名稱。
此輸入雖名為 tag，指令不會建立 Git tag；`publish` 必須保持 false。

須驗證 Windows／Linux／macOS archives、checksum、source／binary SBOM、
Debian package install／remove、兩種 Linux 架構的暫時簽章 APT repository 安裝，
以及兩種 Mac 架構的 Homebrew formula／bottle install／reinstall。
這些安裝發生於 CI runner，不修改使用者機器。目前 0.16 版號的 staging 不可取代
最終 0.17 package 驗證；官方渠道收錄及正式 APT signing key 仍是獨立工作。

人工終端仍須驗收 manage／discover／diagnose 的 resize、filter、數字／gg／G、
Ctrl-D／U、行號模式、輸入／密碼取消與畫面恢復。以授權帳號驗證 snapshot 成功；
不要在 issue、commit 或對話中分享密碼。

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
