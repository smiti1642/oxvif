# Mock acknowledgment-only 政策開工盤點

[English](mock-fidelity-ack-policy-preflight.md) | [繁體中文](mock-fidelity-ack-policy-preflight_zh.md)

W16／K05，2026-09-11，基準 `a51490c`。此設計於實作前記錄。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 首批分類操作與剩餘工作 |
| [設計](#設計) | 明確的局部政策及相容性邊界 |
| [驗證](#驗證) | 拒絕、opt-in 與優先順序控制 |

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
