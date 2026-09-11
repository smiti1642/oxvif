# Mock acknowledgment-only 政策開工盤點

[English](mock-fidelity-ack-policy-preflight.md) | [繁體中文](mock-fidelity-ack-policy-preflight_zh.md)

W16／K05，2026-09-11，基準 `a51490c`。此設計於實作前記錄。
目前實作涵蓋 11 項已分類操作。下方範圍／設計／驗證保留首批三項操作的檢查點；
A3 章節記錄另外八項操作的擴充。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 首批分類操作與剩餘工作 |
| [設計](#設計) | 明確的局部政策及相容性邊界 |
| [驗證](#驗證) | 拒絕、opt-in 與優先順序控制 |
| [剩餘 effect stub 批次](#剩餘-effect-stub-批次) | 一次交付八項額外政策分類 |

## 範圍

先分類三條直接空成功 route：Device SetSystemFactoryDefault、Events Unsubscribe
與 Events SetSynchronizationPoint。它們不重設裝置 state、不終止 subscription，
也不加入同步事件。依核准 D2 預設須拒絕。這是專案 fidelity 政策，不宣稱真實裝置
不支援該 ONVIF 操作。其他 auxiliary、reboot、upgrade／restore、subscription 及
search lifecycle stub 仍明列待處理；首批 registry 不代表全部 157 條 route 已驗收。

## 設計

新增 non-exhaustive 公開 `AckOnlyOperation` enum，各 variant 對應精確完整 Action
URI；mock、replay、adapter transport 與 HTTP mock builder 新增
`with_acknowledgment_only(operation)`。重複呼叫只累加選定操作；clone 複製政策，
但保留既有共用裝置 state。沒有全域 legacy 開關，也不接受任意 suffix。

在 synthetic terminal 完成有界 XML／Action／body 身分驗證後、dispatch／effect
前套用政策。預設回傳結構化 Receiver／`mock:UnmodeledEffect` Fault，reason 為固定
文字。Opt-in 僅保留空回應形狀；兩條路徑都不修改 state、不觸發 change hook、
不發出 effect，也不淘汰 replay fixture。保留 fault／auth 與呼叫端 raw adapter 的
優先順序。內建 replay 直接讓這些操作往後傳遞，不做 pre-write invalidation；
acknowledgment 並非已提交的裝置變更。

不新增公開 struct 的必填欄位、不變更現有簽章。政策為 transport／builder 私有設定，
不持久化至 DeviceState。本批不驗證各操作全部欄位、不模擬 subscription 身分／
lifecycle、不新增 receipt tracing、不寫入實機，也不整合 PR #16。該 PR 的 Media
操作須另有 variant／契約；不得與 Events sync 混用。

## 驗證

在舊碼證明三項預設拒絕測試會失敗。兩種 transport 精確斷言 Fault code／subcode／
reason、完整 state 不變與零 change hook。逐項 opt-in，其餘兩項仍拒絕；重複 builder
呼叫可累加，clone 不會回溯啟用其他 clone。斷言 qualified 空 response payload、
一般讀取、不支援／不匹配 Action 及 fault／auth 優先順序。Replay 與 adapter fallback
使用相同政策；自訂 raw response 仍屬呼叫端。誠實更新既有 public action snapshot，
不可讓所有測試一律 opt-in 舊成功。執行完整 workspace／all-feature／no-fail-fast
擾動並還原，再跑五項 gate、strict docs 與清冊。既有 profile XSD corpus 不證明
這些操作契約；外部驗證覆蓋須另行說明。

實作證據：修正前在兩種 transport 觀察到三項原有成功回應
（`1789103471_cargo_test.log`）。完整 workspace／all-feature／no-fail-fast
擾動將單項 opt-in 誤改成全部放行，並恢復 pre-write replay invalidation；
兩項逐選項 transport 測試及 replay invalidation 斷言均失敗
（`1789104095_cargo_test.log`）。擾動已還原。Fixture 使用非預設 hostname，
避免意外 factory reset 隱藏在預設 state 的比較中。HTTP 測試涵蓋一般與啟用
replay 的 builder，分別檢查單項與累加選項。

另行明確執行的 legacy 外部 schema 檢查通過，共 161 份 response：111 份成功
payload、50 份 Fault、1,263 個 anchored node、1,440 個 skipped child、401 個
已檢查 attribute；十類 finding 均為零。三份 opt-in 成功 instance 與新增預設
Fault 同時保留，未放寬 schema resource、coverage floor 或 finding pin。
此形狀檢查不等同於這些操作已通過獨立 Xerces 驗收，也不證明實際效果。

Windows 本機 gate 通過：workspace all-feature 1,224 tests、default 1,127 tests，
各為 33 suites、5 個條件式 ignore；兩種 Clippy、格式、strict default／all-feature
文件與雙語清冊自我測試均通過。清冊維持 159 個 Action 使用點／157 條 route／243 個
直接 reader。前一筆認證 commit `a51490c` 已通過 CI `34564803484`。本批不是 release
驗收；其他 effectful route 尚待分類，因此 W16 維持 PARTIAL。

## 剩餘 effect stub 批次

基準 `6382458`，2026-09-11；於實作前記錄。依核准 D2，一次交付以下已由原始碼
確認的完整 stub 子群。這是政策遷移，不是各操作全部欄位或規範 Fault 的驗收。

| 清冊 ID | 目前 handler／輸入 | 目前回應與未模擬效果 |
| --- | --- | --- |
| `device.SendAuxiliaryCommand` | `device::resp_send_auxiliary_command`，忽略 request 欄位 | `OK`；不執行 auxiliary |
| `ptz.SendAuxiliaryCommand` | `ptz::handle_ptz_send_auxiliary_command`，legacy AuxiliaryData allowlist；忽略 ProfileToken | accepted 文字或 legacy 拒絕；不執行 auxiliary |
| `device.SystemReboot` | `device::resp_system_reboot`，無 request reader | 重啟訊息；不重啟 |
| `device.StartFirmwareUpgrade` | `device::resp_start_firmware_upgrade`，僅 base URL | Upload URI／duration；不接收上傳或升級 |
| `device.StartSystemRestore` | `device::resp_start_system_restore`，僅 base URL | Upload URI／duration；不還原 |
| `events.SubscribeRequest` | `events::resp_subscribe`，僅 base URL | Reference／目前時間；不建立 push subscription 或推送 |
| `events.RenewRequest` | `events::resp_renew`，無 request reader | 目前時間；不延長 lifetime |
| `search.EndSearch` | `recording::resp_end_search`，無 request reader | 目前時間；不終止搜尋 |

八個 handler 目前均不寫入 state，既有 client／session 及 dispatch 身分不變。
在現有政策加入各自 enum variant／完整 Action，於 handler 前預設回覆固定的
`mock:UnmodeledEffect`。Opt-in 保留既有回應投影與 PTZ allowlist，但 reboot
訊息須明確表示未執行重啟。不新增 upload endpoint、subscription、timer 或 search
session。共用 parser 檢查仍優先於政策；raw adapter 仍掌管自己的回應。收件或拒絕
都不得淘汰 replay 資料。不變更公開 struct 必填欄位、snapshot、錯誤型別或 CLI exit。

C01／C07／C08／C10／C11：精確選項、錯誤 service／body、回應欄位、一般拒絕及
既有 typed workflow。C06／C09：非預設 state、零 hook／invalidation 及既有共用
auth／limit 控制。C12：一次整批擾動與最後 gate，不逐操作執行全套。C02–C05 的完整
欄位、token、enum／extension 及 lifecycle 驗證仍未驗收；政策修正不等於升級 legacy
reader。不依專案 fixture 重定義規範 payload 映射；映射維持不變。外部 schema probe
同時新增明確 opt-in 與預設回應，保留既有成功覆蓋。清冊、公開文件及原始碼行為聲明
與本批一起更新。

### A3 實作證據

擴充後的預設拒絕控制在 `6382458` 上，對兩種 transport 的八項既有成功操作均失敗
（`1789107385_cargo_test.log`）。針對性 policy／workflow／action snapshot 共 34 個
測試通過。接著一次完整 workspace／all-feature／no-fail-fast 擾動，同時繞過 reboot
政策、變更 auxiliary 收件內容及 Renew replay 處理（`1789107591_cargo_test.log`）。
預設拒絕／隔離／snapshot 斷言捕捉到 reboot 繞過；typed auxiliary workflow 捕捉到
收件文字變更。Replay 測試先停在 reboot 斷言，因此此組合 run 不獨立證明對 Renew
invalidation 擾動的敏感性。所有擾動均已還原；最後擴充的 replay 測試會確認全部
十一項操作均不改變 invalidation set。

該完整 run 亦發現兩個既有 HTTP maintenance 測試仍假設預設成功，已遷移至明確
逐操作 opt-in，並精確斷言 URI／duration；其針對性重跑通過。這是 fixture 遷移，
不是放寬預設拒絕的理由。未執行實際 upload、restart、push delivery 或 search
termination。前一筆 commit `6382458` 已通過 CI `34566149186`。

還原後的 Windows workspace 最終 gate 通過：all features 為 1,224 passed，
default features 為 1,127 passed；各為 33 suites、5 個條件式 ignore。格式、
兩種 all-target Clippy（warnings denied）、兩種 feature 模式的 strict docs、
雙語清冊自我測試及 `git diff --check` 均通過。清冊維持 159 個 Action 使用點／
157 條 route／243 個直接 reader。

另行明確執行的 legacy 外部 schema 檢查通過，共 169 份 response：111 份成功
payload、58 份 Fault、1,319 個 anchored node、1,464 個 skipped child、409 個
已檢查 attribute；十類 finding 均為零。新增逐操作 opt-in，讓既有成功覆蓋與
預設拒絕同時保留。未變更 schema resource、finding pin 或 coverage floor。
這不等同於八項操作已通過獨立 Xerces 驗收、完整欄位／lifecycle 驗證或實機效果
測試。A3 完成此八項 stub 的政策遷移，累計十一項已分類操作；W16 維持 PARTIAL，
W17 capability 核對仍未完成。本批不包含發布、系統 binary 更新或分支合併。
