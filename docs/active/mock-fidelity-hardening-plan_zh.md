# Mock 行為可信度強化計畫

[English](mock-fidelity-hardening-plan.md) | [繁體中文](mock-fidelity-hardening-plan_zh.md)

狀態：D1–D3 已於 2026-09-10 核准；第一批實作已完成本機驗證，整套計畫仍進行中。
日期：2026-09-10。Repository 基準：`9dccf9d`。
起因：[PR #16](https://github.com/smiti1642/oxvif/pull/16) 的 `dc69e9a`
版本審查；本計畫不假設該貢獻已合併。

| 章節 | 用途 |
| --- | --- |
| [目標與邊界](#目標與邊界) | 範圍及排除事項 |
| [證據與相依事項](#證據與相依事項) | 已觀察事實與待驗證風險 |
| [待決策項目](#待決策項目) | 相容性、模擬政策及 schema 取得方式 |
| [架構](#架構) | 解析、Fault、行為分類及獨立驗證 |
| [里程碑](#里程碑) | 實作順序與驗收條件 |
| [驗證與完成條件](#驗證與完成條件) | 測試門檻與證據要求 |
| [文件與交付](#文件與交付) | 公開文件及發布邊界 |
| [決策紀錄](#決策紀錄) | 維護者核准與執行狀態 |
| [第一批實作](#第一批實作) | 範圍、回歸證據及剩餘工作 |
| [施工文件](#施工文件) | 原始碼對照檢查表、逐操作追蹤及接續起點 |
| [來源稽核檢查點](#來源稽核檢查點) | W00 來源核對、W01／W02 進度與已重現缺口 |

## 目標與邊界

使 synthetic mock 成為可靠且限制明確的測試替身。Client 與 mock 成功完成
往返，不得作為協定正確性的唯一證據。

- 修正目前已路由服務操作的 request 識別、文字處理、synthetic Fault 序列化
  與可觀察行為宣告。
- 維持 `MockTransport` 與 `MockServer` 的行為一致；分別驗證 HTTP 狀態與
  process 內 SOAP 行為。
- 以明確的 injection 保留異常設備回應測試，不以正常 synthetic 路徑的偶然
  偏差達成相容性測試。
- 延伸既有 round-trip、token-discrimination 與 schema 稽核，不重新建立一套
  重複機制；歷史驗收數字不視為本次基準。

不包含：補齊全部 ONVIF 方法、產生 RTP／視訊、模擬攝影機時間行為、修改 CLI
導覽、批次升級依賴、自動操作實機寫入、ONVIF 認證或發布 Release。
不得正規化已錄製的 Metamorph／replay fixture；其偏差可能正是測試對象。
公開 client 錯誤行為的變更必須接受相容性審查，不得附帶於 mock 重構中悄悄修改。

## 證據與相依事項

| 觀察 | 證據與對計畫的影響 |
| --- | --- |
| PR #16 拒絕含 XML 特殊字元的既有 token | 已於該 PR 透過公開 API 重現。新增 handler 將尚未解碼的文字與儲存值比對。實作分支具備該操作後才移入回歸案例，不自行納入整個 PR。 |
| 字串擷取是共用技術債 | `src/mock/xml_parse.rs` 依 local name 擷取片段並修剪文字，並非以操作為範圍的 XML parser。替換前盤點所有 caller；部分 caller 需要子樹而非解碼文字。 |
| Fault 建構採非結構化字串 | `src/mock/helpers.rs` 直接插入 code／reason；mock 指南已記錄 QName binding 偏差。修正行為時一併列出下游 assertion 的變更。 |
| Client parser 不能作為獨立驗證器 | `src/soap/xml.rs` 移除 namespace、修剪文字，且只公開一層 subcode。不得直接作為嚴格 mock parser，也不得默默改變公開錯誤契約。 |
| 既有 schema check 存在檢查缺口 | 基線的 `tests/mock_schema_shape.rs` 排除 Fault，且使用共用 prefix map。[W20 檢查點](mock-fidelity-schema-preflight_zh.md) 已修正 scope resolution 並納入 Fault 結構；完整 XSD 驗證及 SOAP HTTP 行為仍是獨立工作。 |
| 先前測試未攔截 token 缺陷 | PR head 的 830 個 library tests、28 個 workflow／action-snapshot tests 及 formatting 通過；並非完整 workspace、schema、三平台或實際串流驗收。實作時重新量測。 |

實作驗證的一手參考來源：
[ONVIF 2026 年 6 月規格目錄](https://www.onvif.org/profiles/specifications/specification-history/june-2026/)、
[Media1](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)、
[Media2](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)、
[Media1 WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl)、
[Media2 WSDL](https://www.onvif.org/ver20/media/wsdl/media.wsdl)、
[SOAP messaging](https://www.w3.org/TR/soap12-part1/) 與
[SOAP bindings](https://www.w3.org/TR/soap12-part2/)。
各操作另選適用的 Service／Core 文件。於外部驗證 manifest 記錄來源 URL、
取得日期、版本及雜湊；不得假設可變動的 WSDL URL 代表不可變的版本。

維持既有 schema 不隨專案散布的決策：官方 schema、產生的 schema index、
schema 衍生 fixture 或硬編碼 schema 表格不得加入 repository／package。
一般 XML 測試使用專案自行設計的合成名稱；規範檢查於執行時讀取外部資源。
這是專案邊界，不是新的法律判斷。

## 待決策項目

維護者已於 2026-09-10 核准以下三項建議。它們是實作目標，不代表所有預設值
已完成切換。

| ID | 決策 | 建議 | 取捨及受阻範圍 |
| --- | --- | --- | --- |
| D1 | 正常 synthetic 行為與 Fault 輸出的切換時機 | 以下一個 minor release 為目標，預設採修正後行為並附遷移文件。舊版／異常回應僅透過明確 injection 保留；未證實下游需求前，不新增全域 legacy mode。 | 比對 Fault 字串或依賴寬鬆 request 的既有測試可能需要修改。影響預設切換及最終公開 API／版本選擇，不影響盤點與新增測試。若只發 patch，須縮小為維持相容性的修正範圍。 |
| D2 | 未模擬實際效果的操作如何回應 | 預設拒絕；workflow 測試可逐項明確啟用 acknowledgment-only stub。已記錄限制的靜態讀取 fixture 可保留。 | 部分目前成功的 workflow 需要 opt-in 設定。Stub 可記錄收到請求，但不能證明實際效果。影響相關操作的行為切換；盤點與分類仍可進行。 |
| D3 | 如何使 schema 驗收可強制執行 | 核准專用 CI 驗證 job，在 checkout 外使用固定版本的外部資源；保留無 schema 的離線開發測試。發布驗收必須具有成功的 schema 證據。 | 變更歷史上的手動驗證政策。官方資源不重新散布、不放入 build artifact、不隨 package 附帶。缺檔、缺工具、雜湊不符或下載失敗必須阻擋該 job，不能以 skipped 通過。若不採 CI，改要求維護者執行同等 gate，並明示 CI 不涵蓋 schema。 |

盤點 call site、設計回歸案例及同步文件不需新增產品決策。Parser 內部設計、
測試檔案位置與服務遷移批次屬工程判斷。只有盤點證實需要超出 D1 的公開 API
破壞，或超出 D3 的新依賴／資源散布需求時，才另行提請決策。

## 架構

### Request 解析

在適用範圍內使用既有 XML dependency，建立 mock 私有 request 表示，保留
namespace scope 與解碼後文字。在 synthetic request 邊界解析一次，依操作
定位直接 child，不搜尋整份訊息中第一個相同 local name。

- 分離文字、attribute 與子樹存取；不得機械式將所有 `extract_tag` 改成文字解碼。
- 保留 XML 正規化後的 token 內容；只依特定型別欄位套用空白規則，不全面 trim。
- 比對 operation、Action 與 service namespace。Prefix 拼字可不同，namespace
  身分不得被忽略。
- 拒絕 malformed document、多餘 root、未 binding prefix 及不適當的必填欄位
  重複；不得只因不熟悉內容就拒絕合法的重複成員或 extension。
- 不解析外部 entity、不載入 DTD。兩個入口均限制輸入大小、深度及 node 數，
  公開限制值前先量測。
- 分別盤點 auth、canonicalization 與 replay 相依性；不得將 synthetic
  正規化置於 replay 之前，以免抹除錄製的設備偏差。

### Fault 建構及消費端

以內部結構表示 fault code QName、有順序的巢狀 subcode、reason 與選擇性
detail。安全序列化文字，並在實際 scope 中 binding 所有 QName。
逐操作對照適用參考資料確認錯誤映射；不得由任意 `ter:*` 字串推測整個階層。

一併檢查 `fault_injection.rs`、`responder.rs`、`server.rs`、`transport.rs`、
client Fault parsing 與 CLI error classification。一般 synthetic 錯誤與刻意
malformed／raw injection 必須分離。可行時保留公開 injection 的呼叫形式；
行為遷移依 D1 記錄。

目前 client 僅公開一層 subcode。引入巢狀 Fault 後，選擇維持有效錯誤分類所需
的最小相容方案，並獨立檢查 wire 原文。不得附帶重新定義既有 `subcode` 欄位，
或未告知就增加公開 enum 欄位。

HTTP status、content type、Action 處理及 authentication failure 應對照 SOAP
binding／Core 參考資料。XML Fault 正確不代表 `MockServer` 已驗證完成；
不得假設目前 HTTP 行為正確。

### 行為可信度宣告

維護一份以 service 加 operation 為 key、涵蓋所有 routed action 的專案行為
清單。記錄行為分類、token 測試、state／effect 測試、已知限制及文件 anchor；
它是專案行為清單，不是複製的 schema catalogue。可行時共用於測試覆蓋檢查
與公開 mock 表格。

分類：已建模、靜態讀取 fixture、僅確認請求、未支援。
區分請求被接受、狀態改變及外部可觀察效果。依 D2，僅確認請求必須逐操作啟用；
被拒絕的操作不得修改狀態。如加入 receipt tracing，應有容量上限、各 instance
隔離、可清除、不持久化，且不包含憑證或原始 envelope。它只能證明收到請求；
同步點追蹤不得宣稱 frame 已送達。

### 獨立驗證

分離三層：client request／response tests、直接 mock wire tests、外部 schema
validation。Round trip 為補充，不取代任一層；外部 validator 不得共用 mock／
client parser。

先強化既有 structural checker，包含具 scope 的 namespace resolution 與
如實的覆蓋報告；再於驗證環境評估成熟的外部 XSD validator，不增加 library
runtime dependency。分別驗證 SOAP envelope 與 service payload root，避免
wildcard 讓未檢查 payload 的結果看似通過。成功 payload 與 Fault 都需驗證。
Schema-valid 的輸出仍需語意測試。

透過外部固定版本 catalogue 解析 import，驗證過程禁止任意網路解析。
依 D3，以受限的 fetch step 準備資源，不使用 repository secret 或具特權的
fork-PR 執行。Log／artifact 只包含去識別化結果及來源雜湊，不包含 schema 內容。

## 里程碑

目前沒有里程碑已完整驗收；下方第一批實作包含已完成的子項目。
每個可獨立驗證的單位分段 commit，不將所有里程碑合為一次提交。
確切工作編號及逐操作狀態以後方施工文件為準。下列 commit scope 僅供規劃參考。

| 階段 | 工作與主要位置 | 驗收條件 |
| --- | --- | --- |
| M0 — 盤點與基準 | `src/mock/**`、既有 property tests、`src/soap/**`、CLI Fault 消費端及公開指南。記錄行為分類、parser／Fault caller、相容性風險及 PR #16 整合狀態。 | 每個 routed operation 均列入；觀察有重現案例或明示尚未驗證；相關行為變更前記錄 D1–D3。歷史數字必須重新量測。 |
| M1 — 回歸基礎 | 一般 XML 測試、直接 mock request 及獨立 client 負向測試；盤點舊 fragment 型態的測試 probe。 | Escaped／Unicode／空白 token、attribute、CDATA、namespace 遮蔽、誤導性巢狀欄位、malformed document 與 Fault escaping 案例，能因預期的舊行為失敗，控制案例正常。不得附帶 schema 衍生 fixture 表格。 |
| M2 — 結構化 Fault | 私有 builder、分批遷移 synthetic caller、明確 injection 邊界、消費端分類與 HTTP 契約檢查；可拆為 `fix(mock)`／`test(soap)`。 | 遷移的 Fault 結構與映射具獨立證據；缺少／未知輸入不修改狀態；舊 injection 用法有遷移說明或明確相容途徑。不任意重映射 error code。 |
| M3 — Request parser 遷移 | 私有 namespace-aware request view；先 Media1／Media2，再 PTZ／Imaging，最後 Device／Recording／Search／Replay／Events 及其餘共用路徑。 | 各批次具 token discrimination 與 request-path tests，跨服務狀態保持一致。所有舊擷取 caller 均已遷移，或明確隔離在正常 synthetic 路徑之外。兩個入口一致拒絕 malformed request。 |
| M4 — 行為分類政策 | 行為清單、D2 政策、必要操作的 receipt tracing、state／effect tests 及 advertised capability 稽核。 | 依核准政策，未建模效果的 acknowledgment-only 操作不得預設默默成功。靜態讀取與 opt-in stub 已記錄。Capability、模擬行為與測試宣告不互相矛盾。 |
| M5 — 外部驗證 gate | `tests/mock_schema_shape.rs`、驗證工具及 D3 核准後的 CI job；擴充 request／success／Fault corpus、parser scope checks 及完整 validator 交叉檢查。 | 缺少必要資源不得通過；各 corpus 類別具覆蓋量測。錯誤 namespace、缺少必要內容或損壞 Fault 結構的擾動會使測試失敗。Opaque／unresolved／wildcard 覆蓋獨立呈現，不算完整驗證。 |
| M6 — 整合與文件 | 完整回歸、feature／平台檢查、雙語指南及 release 遷移／證據草稿。 | 同一記錄 commit 上所有核准里程碑通過；限制與未執行項目清楚；不由 mock 測試推論認證或實際串流。Release 仍須依授權發布。 |

M2、M3 按服務拆分遷移，不一次全面重寫。共用 helper 變更若揭露 client 缺陷，
另加回歸與審查；不得放寬 mock，只為恢復缺乏依據的綠燈測試。

## 驗證與完成條件

- 保留 `CLAUDE.md` 的五項 per-commit gate：formatting、有／無 all features
  的 clippy、有／無 all features 的測試。使用 locked dependencies 以便重現；
  整合與 CLI 消費端驗證執行完整 workspace gate。
- 執行既有 workflow、action snapshot、round-trip、token discrimination、
  multi-sensor、Media1／Media2 agreement、service capability、
  replay／canonicalization 與 XML compatibility suites。先確認語意再更新預期。
- 寫入正向測試須斷言 request payload 與可觀察的已建模狀態；負向測試斷言
  特定錯誤 payload，不只使用 `is_err()`。
- 在隔離 checkout 進行受控擾動：繞過 token 選擇、略過解碼、破壞 namespace
  scope 或攤平 Fault 結構。確認測試敏感性後精確還原；批次執行使用
  `--no-fail-fast` 與 all features。
- Windows、Linux、macOS 執行不依賴 schema 的測試；外部驗證 gate 在已記錄
  的驗證環境執行。僅 cross-compile 或未執行的平台不得宣稱 native coverage。
- 驗證適用 feature 組合的 docs 與 doctests；發布前執行 per-feature warning
  sweep。公開 API／MSRV 變動須執行專案規定的額外相容性檢查。
- 分開呈現 passed、failed、blocked、not run。缺少 schema 不構成符合規格的
  證據。去識別化報告記錄 commit、toolchain、features、OS、命令、corpus
  coverage 與外部資源雜湊。

完成代表所有 routed operation 均有明確分類，正常 synthetic parsing／Fault
路徑符合核准契約，遷移行為具非空殼測試，且必要 gate 通過。保留的例外須有
具名範圍及理由，不得隱藏於總計數或更新後的 pin 中。

## 文件與交付

隨實作同步更新：

- `docs/mock-server.md` 與 `_zh.md`：request strictness、Fault／injection
  契約、行為分類、stub 政策、transport 邊界及遷移說明。
- `LIBRARY_GUIDE.md`／`_zh.md`、`OPERATIONS.md`／`_zh.md`、
  `src/mock/mod.rs`、`src/lib.rs` 及受影響公開方法文件：正確的行為與支援宣告。
- `README.md`／`_zh.md`：只有定位變動才調整簡介與連結，不擴充為技術手冊。
  CLI 指南僅於可觀察錯誤輸出或既有範例需要遷移時更新。
- `CHANGELOG.md`：精簡的 unreleased 行為變更及遷移指引；版本確定後，詳細
  去識別化證據放入對應 release record。不得改寫已發布歷史，造成當時已修復的
  錯誤印象。
- `docs/README.md`、本計畫及相關歷史 audit 狀態註記：更新導覽與交叉連結，
  不覆蓋歷史量測。

最初的規劃僅建立雙語草案及索引；維護者後續已授權實作，並允許以 CLI 輔助
實機驗證。該測試授權不包含實機寫入；除非另有同意，僅執行 discovery 與唯讀
檢查。實作 commit 依核准決策及專案 gate 執行。
撰寫本計畫不包含合併／修改 PR #16、發表 review、push branch／tag、發布 crate、
建立 GitHub Release 或更新已安裝 binary。這些動作須有後續適用的使用者指示；
保留更新使用者系統上的 Release 前須先提醒的要求。

## 決策紀錄

| 項目 | 狀態 |
| --- | --- |
| D1 — 預設行為與 release 邊界 | 2026-09-10 已核准；下一個 minor release、修正後預設、不新增全域 legacy mode |
| D2 — 未模擬效果及 opt-in stub | 2026-09-10 已核准；待實作 |
| D3 — 外部 schema CI／release gate | 2026-09-10 已核准；既有不隨專案散布政策不變；待實作 |
| 執行授權 | 2026-09-10 已核准：依計畫實作與驗證至結束；自己的工作直接 commit，不另開自己的 PR；可推送 hardening 分支跑 CI。研究納入 PR #16 要求的功能。不自動合併貢獻者 PR／主分支、不打 tag／發布／更新已安裝 binary。 |
| M0–M3 | 進行中；下列第一批實作不代表這些里程碑已全部完成 |
| M4–M6 | 尚未完成 |
| Release 版本、tag 與發布 | 本計畫尚未選定或授權 |

## 第一批實作

分支：`codex/mock-fidelity-hardening`。這是可獨立驗證的第一批，不代表整套
計畫或 PR #16 已驗收。

- 新增具資源上限及 namespace scope 的私有 scalar request parser，僅遷移
  Media1／Media2 `DeleteProfile`。不機械式改寫 fragment／子樹 caller；
  公開 client parser 與 replay 均未變更。
- 共用 Fault code／reason 文字已轉義，已知 prefix 已 binding。平面錯誤
  階層、HTTP status 行為及完整 structured builder 仍待處理。
- 修正前，三項 Fault safety tests 與兩項 request identity regressions
  失敗：既有 escaped token 被拒絕，巢狀 decoy 則錯誤刪除了 mock state。
  舊 unit fixture 補齊 namespace 宣告，不放寬 parser 來滿足錯誤 fixture。
- 初始基準：Windows workspace／all-features，1,127 passed、4 ignored。
  第一批最終 gate：workspace／all-features 為 1,141 passed、4 ignored；
  workspace／default 為 1,061 passed、4 ignored。Formatting 及兩種
  workspace／all-targets clippy 組態均通過，warning 視為錯誤；兩種 library
  文件組態均建置成功且無警告。適用命令使用 `--locked` 及隔離的
  `target/mock-fidelity-build` 目錄。
- 擾動證據：暫時恢復 token `trim()` 後，
  `text_decodes_once_and_preserves_whitespace` 在 payload assertion 失敗。
  已精確還原 parser 內容（比對 Git blob hash），並重新執行該測試。
- 既有 release CLI 基準：暫時性 discovery 發現 191 台設備；其中一台已儲存
  設備的 `info`、`profiles` 均 exit 0、`ok=true`，無結構化錯誤或 warning。
  未執行設備寫入、未輸出憑證、未保留設備位址或識別資料。這是 client 唯讀
  證據，不是 `DeleteProfile` 的實機驗證，也不是新 mock 行為的證據。
- 分支建置的 debug CLI 重複相同唯讀 smoke：discovery 發現 189 台設備，
  同一已儲存設備的讀取亦成功且無 warning。數量是不同掃描的觀察，不是固定
  fleet 大小；未覆寫已安裝／release binary。
- 本批次未執行外部 schema 驗證或 Linux／macOS native tests；四項 ignored
  tests 不計為通過。
- 剩餘工作：全部操作盤點／分類、結構化 Fault mapping、其他 request 遷移、
  opt-in stub 政策、獨立 schema CI、完整平台驗收及下一個 minor release 準備。

## 施工文件

2026-09-10 補強規劃，以實作 commit `b134f73` 建立索引：

| 文件 | 職責 |
| --- | --- |
| [施工檢查表](mock-fidelity-execution-checklist_zh.md) | W00–W26 相依性、程式碼／測試對照、C01–C12 稽核面向、操作工作卡開工條件、風險清單、命令及結案證據 |
| [逐操作清冊](mock-fidelity-operation-ledger_zh.md) | 10 個正式 sub-dispatcher 的全部 157 個字面值路由分支、確切 handler／arguments，以及分開的契約／request／Fault／行為／驗證狀態 |
| [唯讀清冊檢查器](check-mock-fidelity-inventory.ps1) | 原始碼／清冊相等與雙語追蹤；正向及十項拒絕自我測試；尚非 schema validator 或 CI gate |

原有里程碑表不足以單獨作為施工交接依據；以上文件是其執行層，不依賴對話記憶。
此基準的路由列舉已完成，但完整 Action 核對、間接 caller、逐欄位契約、Fault
映射及行為分類仍未完成。遷移 handler 前須完成該批次 W01 工作卡，保留明確的
調查步驟，不以推測代替規格。

接續 W00 完整 Action 核對與 W02 caller 盤點，再完成 W10 profile／binding
批次的 W01。此次文件／工具補強未修改 Rust runtime、CI、dependency、release
版本或已安裝 binary。主計畫里程碑仍為進行中；PR #16 不在此基準內。

本次規劃補強的 Windows 驗證：清冊檢查器及十項拒絕控制通過；六份規劃文件的
214 個本機連結／anchor 均可解析。Formatting 與兩種 workspace Clippy 通過。
重新執行 workspace 測試：all-features 1,141 passed／4 ignored；default
1,061 passed／4 ignored。這些是回歸檢查，不是新增 schema 或平台驗收。
本次文件修改未執行任何攝影機命令。

## 來源稽核檢查點

2026-09-10 從 `892aa94` 開始。[來源稽核](mock-fidelity-source-audit_zh.md)
記錄完整字面值 Action／方法對應及直接 reader 索引；
[profile／binding 開工盤點](mock-fidelity-profile-preflight_zh.md) 涵蓋第一批 13 個
操作工作卡。W00 已完成目前來源形式的核對；W01／W02 仍部分完成，不代表完整契約驗收。

Dispatch sweep 已納入 session 直接 request 路徑。修正 fixed-profile binding
的錯誤註解但不改 runtime，並新增兩種 Media 服務 Add／Remove 的 state 控制。
K13–K16 已有可執行 known-gap probe：產生 token 碰撞、拒絕刪除仍 notify、profile
name 成為 markup、後筆 binding 失敗卻保留前筆寫入。這些 probe 的通過表示重現缺陷，不是修復。

下一步完成開工盤點中的外部欄位／Fault 核對、共用 parsed-input、原子性及相容性
設計（W02–W06），再按範圍遷移 handler。Runtime routing、schema CI、Release
及已安裝 binary 均未改變。
