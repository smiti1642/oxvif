# 0.17 發布切點之後的工作

[English](post-0.17-backlog.md) | [繁體中文](post-0.17-backlog_zh.md)

狀態：PARTIAL（部分完成）— 0.17.0 發布後的持續工作，於 2026-09-23 核對；不承諾實作或發布日期。
已完成的 [發布切點](../done/release-0.17-cut_zh.md#發布阻擋關卡) 與
[發布證據](../done/release-0.17-finalization_zh.md) 已歸檔。
結案不代表下列廣泛工作已完成，也不會將未執行的檢查視為通過。

| 章節 | 用途 |
| --- | --- |
| [校準後的工作歸屬](#校準後的工作歸屬) | 目前處置、唯一負責計畫及完成條件 |
| [結案核對](#結案核對2026-09-23) | 已完成批次與仍須保留 active 的計劃 |
| [延後批次](#延後批次) | 剩餘範圍與原始編號 |
| [重新納入規則](#重新納入規則) | 何時成為本版阻擋 |
| [保留追蹤](#保留追蹤) | 不遺失技術工作卡 |

## 校準後的工作歸屬

校準日期：2026-09-23；核對至 `2d430d8`。這是工作盤點，不承諾全部納入 0.18。
原 16 組 active 文件均已核對；相依套件整合／分組驗證結案後為 15 組；本次 G3 結案後剩 14 組。
其中多份是同一 Mock 計畫的清冊與 preflight，不能當成獨立功能重複排程。

區分四種處置：**待實作**、**待證據**（已有實作，但指定驗收未留存）、
**待決策／延後**、**持續維護參考**。W12–W15 的 TODO 是待補 hardening 稽核，
不表示相關服務沒有 handler；路由數也不是缺陷數。

| ID | 目前處置／唯一負責計畫 | 下一步與完成條件 |
| --- | --- | --- |
| F01 | 待實作：W10，[execution checklist](mock-fidelity-execution-checklist_zh.md)；profile preflight 為佐證 | OS1 已建模 OSD CRUD 完成；選其餘 URI／source mode／binding 或明確擴充 OSD 範圍，補 field／Fault／state／replay 工作卡與兩個 transport／獨立 instance 驗收；不重做 PA1／VS1／VE1／AM1／OS1。 |
| F02 | 待實作：W11／W12，同一 checklist | RS1 GetNode／GetConfiguration selector 已完成。補其餘 PTZ space／效果及 per-source Imaging／focus 契約；測兩個不同 head／source、限制與拒絕不改狀態，不宣稱真實移動效果。 |
| F03 | 待實作：W13／W14，同一 checklist | 分服務子群稽核 Device／DeviceIO 與 Recording／Search／Replay 生命週期，含 cascade、timeout／termination 及錯誤後狀態；沿用既有 handler 為基準。 |
| F04 | 待實作：W15，同一 checklist | EP2 已交付有限訂閱生命週期；其餘 push、property 同步、完整 topic grammar、WSNT Fault detail 待完成；立即合成 timing 限制明示。 |
| F05 | 待實作：W07–W09；[pipeline](mock-fidelity-pipeline-preflight_zh.md) | 通知連線／期限／framing 限制已交付，見下方。其餘為較廣的 mock HTTP binding、freshness／replay、roles 及 fault injection；不重做已交付 scoped auth。 |
| F06 | 待實作：W16–W19；pipeline 記相依 | 選剩餘 capability／mutation／read 組合，驗證 atomic commit、拒絕保留與 replay 可見性。K27 儲存保留已完成；未來 key-format 重設計另列決策。 |
| F07 | 待實作／證據：W20–W23；[schema preflight](mock-fidelity-schema-preflight_zh.md) | RS1／EP1／OS1／EP2 已將選定 corpus 擴充至 216 instance／59 operation，並通過本機獨立驗證。繼續補其餘覆蓋，保留 anchor、固定外部資源、錯誤控制與有界 fuzz／property 種子。0.17 選定 CI 為另筆歷史證據。 |
| F08 | 待決策／延後：[CLI roadmap](oxvif-cli-plan.md)；[navigation](cli-vim-navigation-plan_zh.md) 管 M6 | decoder／playback、批次匯出、controlled writes、crate 拆分分別選定有限交付與介面／權限／復原驗收；不自動成為 0.18 blocker。 |
| F09 | 待營運／證據／申請：[distribution](oxvif-cli-three-platform-distribution-plan.md) | 唯讀渠道盤點已完成，確認缺少正式 owner／URL／金鑰資料。補齊後隔離驗證 install／upgrade／downgrade／remove；0.17 artifacts／staging 已完成，官方收錄另需證據。 |
| F10 | 待證據：快照調查，[修復紀錄](../done/snapshot-auth-repair_zh.md) | 取得去敏且可重現的非圖片／JPEG 回應，再測有界辨識；保留目的地／認證／TLS／大小／不覆寫規則。CR／LF 僅是合成差異，尚非 Hanwha 根因。 |
| F11 | 待證據：[Fleet](mock-fleet-basic-plan_zh.md)；廣泛 discovery 延後 | 記錄獲授權 OS／interface／VMS 的四台 identity、endpoint、隔離與關閉重啟。完整 scopes／Hello／Bye／Resolve、混合 persona 與持續負載另作實作／驗收。 |
| F12 | 待證據：navigation | 補原生 terminal／人工 IME 組字、取消與復原紀錄；Windows resize 已有證據，一般 CI／注入按鍵不能取代缺少的平台／輸入法驗收。 |
| F13 | 相依維護：[審查紀錄](../dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22) | 已審查相依更新及 rustls 安全公告修補已整合並驗證。Windows keyring 3.6.3 ↔ 4.2.0 原生相容性探針已完成；其他 OS 與鎖定／拒絕／不可用／復原案例仍待驗收。正式維持 3.6.3，發布另列。分組重用／不重複驗證已完成。 |
| F14 | 待決策／實作：[Metamorph](metamorph.md)，[已結案 clone note](../done/metamorph-clone-in-oxdm.md) 記錄 G3 | G1／G3 已完成、G2 已被替代；G3 已補離線摘要與回歸測試，尚未發布。剩餘 M4 persona／control transition 或 M7 reference-value 比對與驗收。 |
| F15 | 待實作／證據：[CLI hardening](oxvif-cli-release-hardening-plan.md) | R2 型別化重試政策、R3 health details 與 R4 六種結果 envelope 矩陣已完成。剩餘 observability／clock-skew／durability、原生終端驗收及 descriptor reachability／可執行 example。R5 多廠牌／soak／簽章／支援／復原仍開放；首次 package／發布已完成。 |

Mock 主計畫管政策，execution checklist 管 W 狀態，operation ledger／source audit
是持續維護的參考，profile／pipeline／schema preflight 管工作卡與相依證據。
同一要求只排一次；W26 限定整合 DONE，W24／W25 全案 PARTIAL，但 0.17 子集已交付。

本次核對 source／tests 與 0.17 最終紀錄，沒有新執行攝影機、原生終端、外部 schema 或發布驗證。
校準提交 `c35704f` 已通過 workspace gate：全功能 1,327／預設 1,215 項測試，
各 7 ignored、41 個 suites，以及兩種 all-target Clippy 與格式檢查。
inventory checker 及拒絕控制亦通過；這些檢查不擴張既有硬體或外部 schema 驗收範圍。
F13 的新相依查核與本地整合證據記於
[相依審查](../dependency-pitfalls.md#post-017--reviewed-maintenance-batch-2026-09-22)。
遠端檢查僅涵蓋具名 PR revision，不代表其後本地修改的原生平台驗收。

F09 唯讀渠道盤點完成：提議 tap 以目前權限查詢回傳 404；repo 只有暫存 APT
staging，尚無具名正式 URL／金鑰。公開渠道負責者、簽章／復原政策與原生生命週期
驗收仍列於 [distribution D](oxvif-cli-three-platform-distribution-plan.md#read-only-channel-inventory-2026-09-22)。

## 結案核對（2026-09-23）

上一輪選定的本地工作**已完成（DONE）**，提交與有限驗收見
[驗收清單](remaining-plan-acceptance_zh.md)。該清單仍管理待補驗收輸入，
因此保留 active；它是輔助資料，不是第十五組計劃。

相依維護計劃、G3 clone 整合、RS1 read selector 與 EP1 event pull 批次
均已位於 `docs/done/`，內容明確標示 DONE。本次沒有其他 active 計劃滿足
整份結案條件，因此未移動部分完成的計劃，也未將其標示為全部完成。
Fleet 仍缺 LAN／VMS 證據；導覽仍缺原生終端／IME 證據；Mock 強化、CLI
runtime／descriptor、散布及 Metamorph M4／M7 仍有上表的實作、決策或驗收工作。
上層計劃仍為 active，不代表要重排已完成的子項目。

## 延後批次

2026-09-22 完成的接近結案項目：

- **F15／R2 重試政策：**修正 Health 初始連線錯誤遺失結構化分類，補齊
  Health／enrichment 重試上限、恢復與取消測試；成功 scan 不重跑。
  Observability、clock-skew 與 registry durability 仍開放。


- **F14／G3：**離線 clone 摘要與回歸測試完成；[整合計畫](../done/metamorph-clone-in-oxdm.md)
  已於 `24bf983` 歸檔。M4／M7 仍開放，新 API 尚未發布。
- **F15／R4 §9.2：**[六種結果的 envelope 驗證](oxvif-cli-release-hardening-plan.md#envelope-acceptance-closed-2026-09-22)
  以 JSON／JSONL 實際執行 12 次 CLI，核對退出碼、model 值、Fleet 排序／計數、
  result／error 邊界與 stdout／stderr，並以負向控制確認 schema 會拒絕錯誤資料。
  描述器完整性、runtime 與商用驗收仍開放。

| ID | 範圍 | 原始追蹤／前置條件 |
| --- | --- | --- |
| F01 | OS1 以外的 OSD extension、URI／source mode 及不支援的 configuration／binding 語意 | W10；保留已接受 Media 子群，逐項分類其餘操作 |
| F02 | 超出 profile identity 的 PTZ configuration／space、movement／preset／home／tour 效果；Imaging 欄位及 focus 建模 | W11／W12；不宣稱現有 profile 檢查已模擬移動 |
| F03 | 完整 Device／DeviceIO／network／user／relay 契約與 Recording／Search／Replay 生命週期 | W13／W14；不改 host network；硬體效果另行授權 |
| F04 | Events filter、subscription、queue 隔離、renew／expiry／termination 及傳送建模 | W15；Media sync 不等於 Events sync；保留目前僅收件確認的預設拒絕 |
| F05 | 更完整 HTTP binding、WSSE freshness／replay protection、角色及 fault injection 重設計 | W07–W09；先完成本版安全／完整性判定；raw hook 保留明確逃生出口 |
| F06 | 全程式 capability 一致性、併發／rollback、replay 相依及超出 K27 碰撞保留修正的 key-format 重設計 | W16–W19；K27 儲存保留已在候選修復，並未延後；見[遷移](../replay-storage_zh.md) |
| F07 | 其餘 schema／QName／wildcard 控制、更廣 corpus 及 fuzz／property 覆蓋 | W20–W23；保留選定獨立 corpus 關卡，不宣稱全部操作覆蓋 |
| F08 | RTSP decoder／playback、批次檔案匯出、CLI 攝影機寫入及獨立導覽 crate | 既有 CLI 維護／導覽計畫；寫入前先建立威脅模型與權限 UX |
| F09 | 官方 Homebrew Core、Debian／Ubuntu 與 Windows 社群套件渠道申請 | 散布計畫；目前原生包裝驗證仍是發布關卡，產出 artifact 不代表已提交申請 |
| F10 | 有界 snapshot 圖片相容性調查 | 保留 A03 的目的地／認證／大小／不覆寫政策；取得去敏、可重現回應後才修改圖片接受條件 |
| F11（基本功能已於 0.17 發布） | 剩餘 Mock Fleet 探索功能與驗收 | [基礎 Fleet 證據](mock-fleet-basic-plan_zh.md)與[操作指南](../mock-fleet_zh.md)。完整 scope 比對、Hello／Bye／Resolve、原生 LAN／VMS 驗收及 Metamorph 混合仍未完成 |
| F12（狀態列已於 0.17 發布） | 剩餘原生終端／輸入法驗收 | [CLI 重新驗收](../done/cli-0.17-reentry_zh.md) 已歸檔；未驗證的平台／輸入法項目繼續留在 [導覽計畫](cli-vim-navigation-plan_zh.md)，發布不代表新增終端驗證 |
| F13 | 後續相依批次與 keyring 遷移審查 | 分組驗收已完成，見上方校準工作歸屬 |
| F14 | Metamorph M4／M7 | G1／G3 已交付、G2 已被替代，不重開 recorder 抽取 |
| F15 | 剩餘 CLI runtime／descriptor 與商用試行強化 | 首次診斷版已交付，分批選定 R2–R5 工作 |

F10 調查處置，2026-09-12：使用者提供的 ONVIF Device Manager 原始碼走
GetSnapshotUri → HTTP stream download → WPF image decode，此路徑沒有 RTSP
fallback。合成 JPEG 尾端附 CR／LF 後，獨立 Windows decoder 可以解碼，oxvif 的
末端 EOI signature 檢查則拒絕。這證明一種相容性差異，尚未證明是觀察到的 Hanwha
非圖片回應原因。未移植 GPL 程式；後續須獨立實作，不採用寬鬆憑證驗證、redirect、
proxy 或含憑證 URL 正規化作為解法。既有成功的唯讀 snapshot／diagnose 證據保留於
[快照修復紀錄](../done/snapshot-auth-repair_zh.md)。

F05 通知子群（2026-09-22）：兩種 listener API 現在限制最多 32 條活動連線、
請求讀取／確認共用 10 秒期限、128 KiB header 與 1 MiB UTF-8 body。
HTTP POST 必須有唯一的十進位 Content-Length；重複長度及 Transfer-Encoding
均拒絕。佇列交付保留連線配額，且不計入請求期限。常設測試涵蓋不完整 header／body
逾時、超量與恢復後的精確來源、錯誤 framing／UTF-8 拒絕，以及 header 大小邊界。
Peer wrapper 仍是連線資料，不是認證。TLS、chunked decoding 與較廣的 W07–W09
mock HTTP／auth／fault 工作仍未完成；此有界 listener 子群不證明公開端點適用性。
驗證：14 項 listener 測試通過；移除期限或放行重複長度時，對應常設測試確實失敗。
兩種 workspace Clippy／test 通過（全功能 1,341／預設 1,225，各 7 ignored），
格式、strict rustdoc、inventory 控制與本地 Markdown 連結亦通過。

不因 outdated 報告列出版本，就將新的相依 major 升級加入 0.17。
安全公告修復與相容性修正仍依發布阻擋政策處理。

## 重新納入規則

若可重現證據顯示資料遺失、機密暴露、不安全副作用、收錄契約中的誤導成功，
或共用修改破壞既有行為，該項返回本版。先於 G01／G02／G03 記錄精確案例、
影響、修正／回復／拒絕方案及驗收檢查，再決定處置。
僅缺少選用功能本身不構成阻擋。

## 保留追蹤

保留 [W00–W26](mock-fidelity-execution-checklist_zh.md)、C01–C12 及 K findings
的既有證據和真實 PARTIAL／TODO 狀態。R01–R08 收錄完成子群，不代表完整里程碑。
W00–W06 基礎及 W24／W25 發布關卡仍屬本版驗收，不整批延後。
後續施工須沿用操作工作卡、批次節奏及雙語要求，不依賴對話記憶重建範圍。

F15／R3，2026-09-22：`health check --details` 與 `health --details` 已能在人類輸出顯示所有檢查、狀態、原因／細節與耗時。JSON／JSONL 保持完整資料，`-v` 維持診斷詳細度用途。Renderer 與實際執行檔驗收通過，兩種 workspace Clippy／test 亦通過（全功能 1,343、預設 1,227，各七項 ignored）。其他 R3 終端驗收仍未完成。

RS1（2026-09-22）：[四個 scoped read selector](../done/mock-fidelity-read-selectors_zh.md) 的有限 W04 批次完成，涵蓋 Media／PTZ／Recording；共用輸出修正 escape 與 OSD 結構。選定 corpus 擴充至 170 instance／50 operation，本機 strict Xerces 通過。完整操作、生命週期、fault 及 capability 仍未完成。

EP1（2026-09-22）：[event pull 一致性](../done/mock-fidelity-event-pull_zh.md) 對 queued IO 套用既有 lexical filter，並原子取得事件／filter。併發、reentrant 控制及四組 client capture 補充 RS1；每份 subscription 的生命週期與完整 topic 語意仍未完成。

2026-09-22 後續：[剩餘計劃驗收輸入](remaining-plan-acceptance_zh.md) 記錄新的 Windows capacity／terminal／credential 證據、離線 snapshot 探針與仍缺少的平台／產品／發行條件。它支援既有工作家族，不新增計劃群組。

OS1（2026-09-23）：[有限 OSD CRUD](../done/mock-fidelity-osd-crud_zh.md) 完成 scoped candidate、每來源原子配額、綁定拒絕及只在 commit 後通知持久化／replay。Client 座標／顏色／persistence XML 與配額解析已修正。選定外部 corpus 為 198 instance／56 operation。背景色、暫存文字及任意 write extension 明確拒絕；URI／source mode 與其餘 W10 仍未完成。

EP2（2026-09-23）：[有限 pull-point 生命週期](../done/mock-fidelity-event-lifecycle_zh.md) 完成獨立 endpoint／filter／queue、原子配置、expiry／renew／unsubscribe，保留公開 RequestCtx 建構。外部 corpus 為 216 XML／59 Actions；push、property 同步、完整 topic grammar、規範 WSNT Fault detail 仍為 W15 工作。
