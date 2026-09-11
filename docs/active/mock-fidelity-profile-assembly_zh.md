# Profile 組裝批次

[English](mock-fidelity-profile-assembly.md) | [繁體中文](mock-fidelity-profile-assembly_zh.md)

基準：`0ae5b44`，2026-09-11。W10／W16／W18／W19 子群，不代表全案驗收。

下列數量與下一子群均記錄本次歷史交付；目前候選版本驗收見
[施工檢查表](mock-fidelity-execution-checklist_zh.md)。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 原始碼位置及已審閱決策 |
| [驗收](#驗收) | 整批驗證及剩餘限制 |
| [證據](#證據) | 實際結果 |

## 範圍

延續 [profile 前置檢查](mock-fidelity-profile-preflight_zh.md) 的 13 張來源操作卡。
主要寫入為 Media1／Media2 CreateProfile、DeleteProfile，Media1 的 video source／encoder
Add／Remove，以及 Media2 Add／RemoveConfiguration；讀取包含兩種 profile view、五個已建模
configuration catalogue，以及公告容量的 service capability。來源為 `src/mock/services/media.rs`、
`media2.rs`、共用 `ProfileEntry`、`effect.rs` 及內建 replay。公開 client／session 簽章不變。

重新直接核對 Media1 v24.12 §5.2.1–5.2.5 與 Media2 v26.06 §5.1.1–5.1.5 的初始綁定、
選填改名、All 及容量語意。外部來源筆記保留在 checkout 外，不新增官方 schema 表格或衍生 fixture。

實作決策：由具 namespace 的直接 child 讀取 binding，保留解碼身分；在一次 commit／notification
前驗證完整候選值。Add／create 忽略 All，remove-All 清空已建模 slot，未提供 Name 時保留原值。
同一 slot 的衝突指派須拒絕，不再默默採最後一筆；相同值重複為冪等。維持既有五個 slot，其他類型
明確拒絕，不新增公開必填欄位；將錯誤公告的 Metadata 修正為實際已支援的 PTZ。
建立時遵守既有公告上限 8；匯入較大清單仍可讀取及刪除，但數量未低於上限前不能新增，不截斷 seed。

受影響 catalogue 的引用計數須反映已提交的 profile reference；內建 replay 亦須淘汰因此過期的
configuration read。拒絕不變更 state 或 recording。任意 seed 與完整影音相容性規則，仍與此寫入
契約分開驗收；不虛構實際 encoder 能力或 PTZ 時間保證。

## 驗收

C01–C05：兩種服務／transport、scoped／escaped 身分、重複、缺少、decoy 及錯誤 binding。
C06：初始多項 binding、僅改名、改名加錯誤後筆的 rollback、All／冪等 remove、容量／碰撞／
併發配置、引用計數與單次通知。C07–C08：精確 payload、已審閱 structured Fault 及獨立 QName／
shape 控制。C09–C11：保留共用 auth／limit、跨服務 read、raw ownership、內建 replay 與既有
client／session workflow。C12：針對性舊碼失敗、一次完整擾動、還原後一次五項 gate 及相關外部
檢查。雙語文件與遷移說明一起更新。

完整巢狀 configuration writer／renderer、全部相容組合、HTTP binding、認證 freshness 及未支援
類型的儲存仍屬後續工作，不在本批暗示驗收。不得寫入實機、發布、安裝或合併主分支。

## 證據

首輪組裝測試共三項通過。擴充欄位／replay 控制及遷移 atomicity／known-gap
snapshot 後，三個 suite 共 14 項通過。一次完整 workspace／all-feature／no-fail-fast
擾動（`1789109637_cargo_test.log`）刻意多放行一個 profile，並將受影響引用計數加一。
併發配置斷言捕捉容量超限；組裝、binding、身分、刪除及 replay 的 payload／state
斷言捕捉錯誤計數，容量負向 helper 亦拒絕非預期成功。兩項擾動均精確還原。
同輪另發現 replay 測試仍期待舊平面 Fault，已遷移為精確 structured payload，
不將此預期更新誤列為擾動敏感性。本組合 run 不獨立證明每個其他斷言的敏感性。
最終 Windows workspace gate 使用 locked dependencies，全數通過：formatting、
all-target Clippy 的 all-feature／default 兩種組態（`-D warnings`）、1,230 項
all-feature 測試與 1,131 項 default 測試；兩輪各有五項預定 ignored，共 34 個 suite。
兩種 strict rustdoc 建置亦通過。最終審查移除不必要的 encoder instance replay
相依；重跑的受影響 gate 包含兩種 transport 的精確 recording 保留斷言。
此審查修正未另跑第二輪擾動。

明確選取的舊 schema 檢查通過，finding pin 未變更。當時的外部 profile exporter
產生 40 份 instance，固定版本 strict Xerces 全數驗證通過，外部 corpus 為
`oxvif-profile-corpus-20260911-04`。這些僅涵蓋既有 13 操作 corpus，不包含所有新增
raw 初始 binding、改名、容量與衝突變體。清冊 self-test 通過：159 個 Action site、
157 條 route、238 個直接 reader（223 個位於頂層 test module 前、15 個位於其中），移除五處舊擷取呼叫。
本批不代表 Linux／macOS 原生或 release 驗收。當時交接的下一子群：video source／encoder
configuration 與 options。
