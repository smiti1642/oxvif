# Mock 請求管線開工核對

[English](mock-fidelity-pipeline-preflight.md) | [繁體中文](mock-fidelity-pipeline-preflight_zh.md)

W02／W03／W06／W19 工程檢查點，2026-09-10；原始碼基準 `9978220`。
本文件是從程式碼整理的依賴圖，不是規範契約表。整體 W02 仍為 PARTIAL；
W03 預設驗證及 W06 錯誤遷移**尚未實作**。本階段不需要新的產品決策。

| 章節 | 用途 |
| --- | --- |
| [入口與責任](#入口與責任) | 追蹤輸入通過共用程式碼的路徑 |
| [Profile 依賴追蹤](#profile-依賴追蹤) | 解析、輸出及副作用 |
| [相容性約束](#相容性約束) | 保留刻意的原始資料行為 |
| [實作順序](#實作順序) | 前置條件與具體子項 |
| [證據與剩餘工作](#證據與剩餘工作) | 測試及尚未驗證的發現 |

## 入口與責任

| 原始碼符號 | 目前輸入／輸出及後續依賴 | 負責工作 |
| --- | --- | --- |
| `mock/transport.rs::soap_post` | 輸入已為 Rust String；忽略 URL；建立 RequestCtx 後交給 Chain::default_mock | W03／W07 |
| `mock/server.rs::handle_soap` | helpers::extract_action 只讀 Content-Type，缺少時成為空字串；bytes 經 lossy UTF-8 轉換；建立預設或 replay chain；所有結果皆回 HTTP 200 | W03／W07；仍需 HTTP binding 核對 |
| `metamorph/replay.rs::MetamorphTransport::soap_post` | 使用 Chain::mock_with_extra 加入 ReplayResponder；沒有 HTTP status 層 | W03／W19 |
| `responder.rs::Chain::{mock_with_extra,respond}` | Fault → Auth → custom／replay responders → Synthetic；第一個 Some(String) 結束處理；RequestCtx 欄位與 Responder trait 均為公開 API | W03／W06／W09／W19 |
| `FaultResponder::respond` | FaultInjector::take_for_action 消耗 suffix-matched 項目；resp_soap_fault 對 code／reason escaping | W05／W09；不可意外改成 auth 之後才消耗 |
| `AuthResponder::respond` | auth::requires_auth 精確比對豁免 URI；validate_ws_security 以舊 extract_tag 讀四個 local name；auth_fault 使用獨立的 raw reason formatter | W08／W05；一般 Fault helper 修正不涵蓋此處 |
| `ReplayResponder::respond` | Action 尾段分類寫入 → synthetic 執行前先使 family 失效；讀取使用 canonicalize(Key)、store.lookup(Action,key)、response_raw | W19；見 K17 |
| `canon::canonicalize` → `write_node` | 去 namespace 的 XmlNode；mask_text／mask_attr；排序 attribute、摺疊文字；解析失敗則摺疊原始字串空白 | W19；不是嚴格 synthetic request reader |
| `SyntheticResponder::respond` → `dispatch` | 傳遞原始 body；Action routing 寬鬆；目前只有已遷移 DeleteProfile 呼叫 required_text | W03／W04／W07 |
| `request::required_text` → `parse` | NsReader、scope identity、有界 tree、單一 SOAP Body operation 或獨立 operation；讀直接 scalar child | W04；attribute 驗證後丟棄，尚無 typed／repeated／subtree API |

正常 synthetic parser 不得悄悄變成錄製回應或所有公開 custom responder 的
驗證器。HTTP UTF-8／Action 擷取發生在此 chain 之前，必須獨立驗收；保留原始
String 不等於能保留無效 HTTP encoding 的原始 bytes。

## Profile 依賴追蹤

本節補充 [13 張 profile 工作卡](mock-fidelity-profile-preflight_zh.md)，不代表
其他服務的逐欄位審查完成。

1. `extract_tag` → `find_open_tag/find_close_tag`：掃描 local name，回傳 trim
   後的 raw inner fragment，不解碼 entity。`extract_all_tags` 重複此流程；
   `extract_attr` 掃描第一個符合 tag 的 header，保留原始拼寫。
2. `handle_create_profile` 與 `handle_create_profile_media2` 將 raw Name 傳給
   `create_profile_in_state`，原樣存入 state；Media1 的 Token 亦然。因此只改
   輸出 escaping 可能讓一般 client 輸入被雙重轉義；直接讓所有舊 helper 解碼
   則可能破壞仍將回傳值當作 subtree 的呼叫端。
3. `resp_profiles/resp_profile` 與 `resp_profiles_media2` 分開取得 profile 與
   catalogue snapshot；renderer 讀取儲存的 identity／text，呼叫 VSC、video、
   audio、PTZ renderer。Media1 建立回應亦使用 render_profile；Media2 則回傳
   產生的 token。K15 必須同時測 seeded literal state 及 client 建立時包含
   entity-looking 文字的資料，不可只測一個 getter。
4. `apply_media2_configuration` 擷取 entries、解析 kind、合成
   ProfileToken／ConfigurationToken fragment，再逐筆呼叫 bind／unbind。
   未來共用 mutation helper 應接收值或借用 parsed node，不可把解碼後的值再
   插回 XML 字串。K16 必須在 mutation 前完成全部驗證；逐筆多解析一次無法
   提供原子性。
5. `create_profile_in_state/delete_profile_in_state/bind_configuration` 呼叫
   `MockState::modify[_returning]` → `notify` → 持有 read guard 時執行 callback。
   集合相等、callback 次數及 replay 可見結果必須分別斷言；直接全域修改此
   generic helper 會影響其他服務。

**K17 — 原始碼確認，尚未執行重現：** ReplayResponder 在 SyntheticResponder
接受或拒絕寫入之前，就把 operation family 加入 invalidated；後續 Fault 不會
復原該失效狀態。因此只增加嚴格 synthetic rejection，仍無法保證 replay 的
可見結果不變。負責 W19／W03／W18；受影響工作卡包括兩服務的
CreateProfile／DeleteProfile、Media1 Add／RemoveVideoSourceConfiguration 與
Add／RemoveVideoEncoderConfiguration，以及 Media2 Add／RemoveConfiguration。
其他 mutation family 仍需同樣審查。
目標回歸：錄製可辨識的 Media1 GetProfile 結果，讓 DeleteProfile 被拒絕且
state 不變，再讀取時確認仍使用錄製結果；另以成功寫入使錄製結果失效作正向
控制。Chain 順序測試不代表 K17 已修復。

**K18 — 原始碼確認，尚未執行重現：** replay::family 只移除開頭動詞，沒有
建立讀寫依賴。GetProfiles 成為 Profiles，CreateProfile／DeleteProfile 卻
成為 Profile；binding 成為 VideoSourceConfiguration、VideoEncoderConfiguration
或 Configuration，也不是 profile read 的 key。因此即使 mutation 成功，仍可能
繼續使用錄製的 profile list。Invalidation key 亦不含 service identity（store
lookup 則包含完整 Action）。由 W19／W10 負責；設計 outcome-based invalidation
前須核對同一批 13 張卡的讀寫依賴。目標測試：成功 create／delete／bind 後的
GetProfiles 應反映新 state；無關 service／instance 的錄製結果仍可使用。
不可只測 GetHostname／SetHostname 這類名稱恰好相符的組合。

## 相容性約束

- 設計 private parsed request／outcome 路徑時，保持 RequestCtx 與
  Responder::respond 的原始碼相容性；不可只為快取解析結果而新增公開 struct
  的必要欄位。保留供 extension／replay 使用的原始 body。
- 正常 synthetic 驗證放在 synthetic boundary；Fault injection、auth、replay
  順序必須保留或另外審查。最終 static handler 也須經過驗證，不可因忽略 body
  就繞過驗證。
- K17 必須使用明確的成功效果資訊處理，才可宣稱拒絕寫入沒有副作用；不可搜尋
  XML 字串猜測成功，也不可收到任何 generic state hook 就使所有 fixture 失效。
  K18 需要明確的 affected-read 依賴，不能只延後使用去除動詞後的 family key。
  K16 的部分寫入也須處理；僅延後 invalidation 不等於 transaction rollback。
- 保留 SoapError::Fault 的 code／subcode／detail 語意。soap::find_response
  只暴露第一層 Subcode；health::CheckError::from 複製它，CheckError::is_auth、
  health assessment／JUnit、CLI application diagnostics，以及
  metamorph::parse::extract_fault 都是消費端。Nested Fault 必須測這些消費端，
  不只測 renderer。本檢查點沒有批准變更公開錯誤欄位或 CLI exit code。
- Auth 仍有自己的舊 parser／formatter；嚴格 DeleteProfile 與已 escaping 的
  resp_soap_fault 不能證明 auth 安全性或 authorization。

## 實作順序

| 子項 | 下一工作與驗收 | 前置條件 |
| --- | --- | --- |
| P-A | Private parsed-node accessor：decoded text、scoped attribute、有序 repeated children；generic namespace／normalization／resource 控制；保留獨立 operation 測試入口 | 上述 W04 選定依賴；不宣稱整體 W02 完成 |
| P-B | 在外部核對官方逐欄位／Fault 參考，固定 source closure／hash 紀錄；解決 13 張卡的 defaults、repeats、capacity／conflicts 與 state effects | W01；不可用本 source table 取代 WSDL／XSD 證據 |
| P-C | 定義 structured internal Fault／outcome；預設切換前驗證 client／health／CLI 相容性；決定成功後 replay invalidation 的 private hook | W05／W06／W19；不可隱含重新設計公開 trait |
| P-D | 兩個 synthetic 入口使用同一 parsed request；拒絕 Action／body mismatch 與 malformed input，包含 static read，並保留 chain 控制 | P-A／P-C，加上 W07 binding 核對與 K17 處理 |
| P-E | 分批遷移 profile read／create／delete／binding；輸入解碼、輸出只 escaping 一次；原子驗證且拒絕寫入不通知 | P-B／P-D；K13–K16 測試改為正確不變量 |

這些是既有工作 ID 的子項，不取代里程碑。外部 schema CI 仍屬 W20–W22；新接入
的 inventory job 只檢查原始碼清單漂移。

## 證據與剩餘工作

新增兩個 mock::responder::tests 控制既有 extension seam：

- `fault_precedes_auth_and_extras_even_for_malformed_input`：第一個回應精確
  比對注入 Fault 的 code／reason；一次性消耗後，第二次精確比對 auth Fault；
  custom responder 完全沒有被呼叫。
- `extras_preserve_raw_input_order_and_short_circuit_output`：第一個 extra
  放行、第二個回答、第三個不執行；request／response 字串原樣保留，包含 CRLF、
  entity、空白與刻意錯誤。

這是 in-process 順序測試，不是 HTTP、實際 replay store、schema 或符合性驗收。
其他服務的間接 reader、欄位契約、原生 Linux／macOS、外部驗證及 K17／K18 重現
仍待完成。

本機證據：Windows、rustc 1.97.0、PowerShell 7.5.4，使用獨立
`target/mock-fidelity-build`。兩個新測試先通過，再修改注入 code 及 trim
captured request text，確認各自在預期斷言失敗，之後均已還原。
格式、預設／全部功能 workspace Clippy 與測試通過：全部功能 1,148、預設
1,068 passed，各 4 ignored。清冊自我測試／核對、192 個本機檔案連結及 10 個
新章節 anchor 均通過。本檢查點未變更 runtime 實作、公開 API、安裝、Release
或實機行為。接續從 P-A 開始；P-B／P-C 仍是廣泛遷移 handler 的前置條件，
不必重做已完成的 W00 盤點。
