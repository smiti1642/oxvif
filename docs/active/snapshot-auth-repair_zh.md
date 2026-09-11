# 快照相容性與認證修復

[English](snapshot-auth-repair.md) | [繁體中文](snapshot-auth-repair_zh.md)

狀態：本機修復及驗證完成。本批次不進行版本提升、主分支合併、攝影機設定變更或發布。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 單一共用實作批次 |
| [驗收](#驗收) | 本機與實機證據 |
| [邊界](#邊界) | 明確保留的後續工作 |

## 範圍

- [x] 重現 Authorization 標頭名稱大小寫問題；診斷程式以 Title-Case 向先前失敗的 18 台設備全部取得 JPEG。兩台完成單一標頭控制實驗；尚非修復後 CLI 驗收。
- [x] 建立 CLI 與 library health 共用的有界下載器。
- [x] 保留標準要求的 Digest 參數引號格式，禁止 Digest 失敗後降級 Basic。
- [x] 處理 scheme／參數名稱大小寫、引號清單及多重 challenge；忽略未知 qop 選項、正確計算 auth-int 的空 GET 實體，限制同一安全政策下的 stale 重試。
- [x] 加入獨立計算期望摘要的認證伺服器；先驗證帳密，再傳送已由外部解碼器確認的合成 JPEG fixture。
- [x] 完成原始碼審查、負向案例及完整批次敏感度測試。
- [x] 通過 workspace 全功能／預設測試、兩種 Clippy、fmt 及兩種嚴格 rustdoc；同步中英文 CLI、library、release 與驗收文件。
- [x] 建置正式候選 CLI，複測 18 台問題設備及正常控制組；保留 Hanwha 非圖片失敗，不修改設備設定。
- [x] 完成修復 commit，記錄實際證據及尚未完成的發布閘門。

## 驗收

2026-09-11：完整敏感度批次為 1,306 通過、兩項有效斷言失敗、五項 ignored；
還原後全功能 1,308／預設 1,198 通過，各五項 ignored、41 suites。
兩組 Clippy、嚴格 rustdoc 及 fmt 均通過；已發布 CHANGELOG 歷史保持不變。

修復後 CLI 的 18 台第一個 profile 抽樣：17 台首輪成功，一台在 8 秒逾時後，
單次以 20 秒預算複測成功；正常控制組通過，Hanwha 仍拒絕非影像回應。
原始已儲存 GV-TBL8810 兩個 profile 均保存成功，外部解碼器確認 640×360，
重複保存皆 exit 4 且 hash 不變。詳見[驗收資料](release-0.17-approval_zh.md)。

敏感度實驗須執行完整 workspace、all-features、no-fail-fast，確認失敗來自有效斷言而非編譯。精確還原後，以完整服務批次執行最後閘門。既有 CLI 測試繼續涵蓋逾時、含 chunked 傳輸的 16 MiB 限制、private CA、重新導向拒絕及不可覆寫輸出。新增測試涵蓋 health 共用途徑、正確／錯誤 Digest、MD5／SHA-256／session／auth-int、有界 stale 重試，以及不降級 Basic 的非法或不支援 challenge。

實機結果須區分診斷程式與正式 CLI、JPEG 檔頭辨識與解碼，以及第一個 profile 與全 profile 驗收。憑證僅存於程序記憶體，紀錄不得包含位址、profile token、URI/query 或認證標頭。

欄位名稱相容處理仍符合 HTTP：[RFC 9110 §5.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.1)
定義欄位名稱不區分大小寫；[RFC 7616 §3.4](https://www.rfc-editor.org/rfc/rfc7616.html#section-3.4)
定義 Digest 回應語法。[ONVIF Media 規格](https://www.onvif.org/specs/srv/media/ONVIF-Media-Service-Spec.pdf)
要求 JPEG 快照；[Hanwha SUNAPI 指南](https://support.hanwhavision.com/hc/en-001/articles/47782352496659-How-to-utilize-SUNAPI-snapshot-command)
說明其快照命令的 MJPEG 前提，但不能據此宣稱修改這台攝影機即可修復已觀察到的回應。

## 邊界

不自動建立 MJPEG profile 或改寫廠商 CGI。Hanwha 缺少 MJPEG 的說明仍是有證據支持的假設，不是已驗證修復。本批次不新增完整圖片解碼；保留並明確說明 JPEG／PNG／BMP 的 signature 檢查，PNG／BMP 為非 ONVIF 標準的相容性擴充。一般 SOAP transport 變更與 SnapshotUri 壽命欄位嚴格解析另列後續，兩者均非這 18 台 HTTP 401 的已證實原因。
