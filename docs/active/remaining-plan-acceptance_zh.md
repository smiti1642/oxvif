# 剩餘計劃驗收輸入

[English](remaining-plan-acceptance.md) | [繁體中文](remaining-plan-acceptance_zh.md)

2026-09-22。這是既有計劃的輔助清單，不新增 milestone；排程與狀態以
[backlog](post-0.17-backlog_zh.md) 為準。本地工作直接 commit 到 `master`，
本次不包含 PR、發布或外部上架。

2026-09-23 狀態核對：至 `2d430d8` 的上一輪選定本地交付**已完成（DONE）**。
本清單仍管理下方未完成輸入，狀態維持 **ACTIVE**；不代表所有上層計劃均已完成。
見[結案核對](post-0.17-backlog_zh.md#結案核對2026-09-23)。

| 章節 | 用途 |
| --- | --- |
| [工作處置](#工作處置) | 每個 F 家族的下一個驗收邊界 |
| [決策輸入](#決策輸入) | 延後產品範圍的具體提案 |
| [原生證據](#原生證據) | 執行入口與仍需操作員的項目 |

## 工作處置

| 家族 | 本地工作／下一個驗收邊界 |
| --- | --- |
| F01／F02／F03 | [RS1](../done/mock-fidelity-read-selectors_zh.md) 修正四個 Media／PTZ／Recording selector 及共用輸出。OS1 已建模 OSD CRUD 完成；URI／source mode、運動／Imaging、Device／DeviceIO、錄影／搜尋生命週期仍是可繼續實作的工作，不是都受硬體阻擋；下一批仍先做 W01 操作卡。 |
| F04／F06 | [EP2](../done/mock-fidelity-event-lifecycle_zh.md) 以 private endpoint adapter 完成有限訂閱生命週期，保留公開 RequestCtx 建構。Push、property 同步、完整 topic grammar、規範 WSNT Fault detail 仍待實作；其餘 capability／replay 驗收保持開放。 |
| F05 | AF1 freshness／replay protection 已交付；HTTP binding、roles、structured／raw injection 仍需各自驗收。 |
| F07 | RS1／EP1 擴充選定操作 capture 與獨立驗證，保留 payload anchor；較廣 wildcard／QName／negative／fuzz 仍未完成，獨立資源留在 checkout 外。 |
| F08 | 下方已準備產品切片契約；RTSP／playback、批次檔案匯出、相機寫入、MCP、crate 拆分仍待產品決策。 |
| F09 | 既有 artifact／staging 可驗證；下方公開 APT／tap 擁有者、簽章、復原資料尚缺，原生生命週期與上架未完成。 |
| F10 | `snapshot_probe` 用正式 signature 函式提供 bounded 離線證據，不接受修改後的圖片。仍需去敏感的失敗相機 body／transport metadata，才能確定實際根因。 |
| F11 | Windows `cb25276` 的 loopback 64／256 member、同數併發讀取通過，身分與回應全數確認、零失敗；startup／total 分別 360／932 ms、1547／2378 ms。這是一次 smoke，非效能上限、記憶體量測、LAN／VMS 或長時間 soak。 |
| F12 | 重跑 Windows ConPTY discover／manage／resize；runner 改為等待完整重繪並給 fixture 初始化獨立時限。人工 IME 與原生 Linux／macOS 仍未完成。 |
| F13 | Windows 的隔離原生探針通過 3.6.3 ↔ 4.2.0；正式仍用 3.6.3。其他 OS、鎖定／拒絕／不可用 store 與重連未完成。 |
| F14 | 下方已準備 M4／M7 transition／comparison 契約，不宣稱已實作 persona 切換或 reference-value 等價。 |
| F15 | Health 人類輸出 `--details` 完成。R2 observability／durability、descriptor reachability／example 及商用驗收仍待程式／設計／證據；原生工具支援驗收，不等於 R5 結案。 |

## 決策輸入

以下是具體候選切片，並非默默核准新增產品範圍。

- **相機寫入：**首選單一裝置 hostname。Preview 記錄裝置身分、目前值與新值，不含憑證；apply 重讀身分／值，拒絕過期 preview，最多發送一次寫入。送出後逾時代表結果未知，先 readback 才考慮重試；取消不保證 rollback。測試 stale identity／value、auth failure、commit 前後斷線、readback 失敗與復原。不要擴展至帳密、網路、firmware 或多機寫入；仍須 shipping 決策及權限 UX。
- **批次檔案：**候選為固定明確裝置選取的既有唯讀 inventory，限制併發、使用全新目錄、唯一檔名及逐項 outcome manifest。不覆蓋既有檔案；部分失敗保留成功輸出並列出失敗／取消項目。先決定選取／snapshot 語意再新增 parser path。
- **RTSP／playback 與 navigation 拆分：**先選 decoder 授權／平台／資源上限及可觀察的播放成功條件；navigation 等第二個 consumer、package／API／version 確認前維持 internal。
- **M7：**候選 reference 為明確提供的 fixture store，只比對不含歧義的 exact operation／request identity，不能只 join legacy canonical key。Missing／ambiguous／unparseable／unsupported 要與 equal／different 分開；保留 raw evidence、明確 volatile mask，不能隱藏語意 ID 改變。獨立案例包含同結構不同值、namespace 改變、key collision、單邊缺漏、parse failure、僅 timestamp 改變。此比較不能證明 fixture 就是正確實機 reference。
- **M4：**候選為從已驗證 synthetic／replay 設定啟動新 server，成功後才交換 owner handle；失敗保留舊 instance，handover 後明確停舊端點。不保證 same-port 無縫切換、隱式 persistence、無驗證遠端管理或寫入 replay。實作前先決定 local owner API 或受驗證 control endpoint。
- **公開發行：**補 APT base URL、suite／architecture、owner／recovery contact、簽章 fingerprint／expiry／rotation、保留的 downgrade 版本；tap repo owner 與每種架構 formula／bottle hash。具備真實值後驗證 install → upgrade → downgrade／pin → remove、破壞／過期簽章拒絕及 registry／credential 保留政策。CI-only key／staging URL 不能直接變成正式預設。

## 原生證據

Keyring 探針刻意寫入唯一 synthetic account：

```text
python -X utf8 packaging/check_keyring_migration.py --run-native
```

它建立隔離 Cargo 專案、固定 keyring 版本、不改正式 lockfile，驗證 v3 create →
v4 read／update → v3 rollback read → v4 delete → v4 create → v3 read／delete →
兩版均不存在。一般錯誤返回亦會 cleanup；程序／OS 被強制終止仍可打斷清理，
原生驗收宜使用可拋棄 session。不列舉既有帳號，不輸出 raw backend error／value。
Windows 使用 native-store 1.1.0；正式選 facade 或 explicit store 仍未決定。

Snapshot 離線入口：

```text
cargo run --example snapshot_probe --features health -- /path/to/captured-body
```

只回報位元組數、signature、尾端 CR／LF 數、診斷性去尾後 signature、
`decoded=false`；最多讀 16 MiB 加一個 guard byte，來源不變。這不是 decoder
或 image-acceptance 政策改變。分享資料排除 URL／query、cookie、authorization；
HTTP status、content type、長度、auth mode 另外去敏感記錄。目前沒有失敗實機樣本。

每種 OS／terminal 的操作員紀錄：

| 必要欄位 | 需取得證據 |
| --- | --- |
| 環境 | commit、binary hash、OS／build、terminal／version、鍵盤 layout、IME／version |
| 字面輸入 | search／name／path／password 的數字、`gg`、`j/k`、Unicode；組字確認／取消不能觸發 navigation |
| Navigation | pending count、escape、selection 保持、filter／mode 切換 |
| Resize | 窄／大視窗、完整重繪、cursor／selection、底部 status |
| 退出／復原 | Ctrl+C、operation cancellation、返回畫面、terminal restoration |
| 結果 | 逐項 pass／fail／not-run、去敏感 repro；不能以注入 ASCII 代替人工 IME |

Windows 自動測試使用 ignored 目錄中的 pywinpty 3.0.5、pyte 0.8.2、wcwidth 0.8.4。
第一輪 manage startup／resize 在 build 負載下揭露 harness 時序假設；單獨重跑
manage／resize 通過，修正後保留原畫面、選取與復原斷言。LAN 仍需獲授權介面
與目標 VMS；本批未改網路設定或遠端裝置狀態。

修正 runner 後的最終 Windows ConPTY 完整執行：discover、manage、resize 三種模式一起通過。Snapshot 離線 example 單元測試亦通過；人工原生 IME 仍為 not-run。

Terminal 主機為 Windows 10 build 19045；測試 CLI fixture 執行檔 SHA-256：
`692f987b6daed5b26a1df60c5104edc98243a1057902fcd8d075d5b987480996`。
Snapshot 探針另通過實際 binary 的來源保持、超量拒絕、缺少／目錄輸入及 path
去敏感檢查。最終合併 workspace 驗收：全功能 1,351、預設 1,235，各七项
ignored；兩種 all-target Clippy 與格式通過。26 項 packaging 控制、明確執行的
snapshot example、178-instance 外部 corpus、native credential、ConPTY／capacity
為額外獨立證據，不加進 workspace 總數。

交付 commit：`4a9d0a5` health details、`cb25276` RS1、`af1489f` EP1、
`c747297` snapshot 探針、`6093d94` keyring 探針、`32d6f49` terminal runner。
表中的較大實作仍未完成；本清單記錄逐份計劃的本地可行工作及輸入準備，
不代表所有未來可寫的程式都已完成。

OS1（2026-09-23）：[有限 OSD CRUD](../done/mock-fidelity-osd-crud_zh.md) 完成 scoped candidate、每來源原子配額、綁定拒絕及只在 commit 後通知持久化／replay。Client 座標／顏色／persistence XML 與配額解析已修正。選定外部 corpus 為 198 instance／56 operation。背景色、暫存文字及任意 write extension 明確拒絕；URI／source mode 與其餘 W10 仍未完成。

AF1（2026-09-23）：[有界認證時間／nonce 防重用](../done/mock-fidelity-auth-freshness_zh.md) 已交付；nonce 原子 admission、過期／未來／重用拒絕不觸發 hook，時鐘倒退不重開已淘汰區間。外部 corpus 224 XML／60 Actions，AF1 request Security header 去除後匯出。Roles、HTTP binding、structured／raw injection 仍為 Q02 工作。
