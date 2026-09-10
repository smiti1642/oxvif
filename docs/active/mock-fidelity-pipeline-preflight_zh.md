# Mock 請求管線開工核對

[English](mock-fidelity-pipeline-preflight.md) | [繁體中文](mock-fidelity-pipeline-preflight_zh.md)

W02／W03／W06／W19 工程檢查點，2026-09-10；原始碼基準 `9978220`。
本文件是從程式碼整理的依賴圖，不是規範契約表。整體 W02 仍為 PARTIAL；
W03 預設驗證及廣泛的 W06 服務錯誤遷移**尚未實作**；已完成的共用基礎補充於
下方。本階段不需要新的產品決策。

| 章節 | 用途 |
| --- | --- |
| [入口與責任](#入口與責任) | 追蹤輸入通過共用程式碼的路徑 |
| [Profile 依賴追蹤](#profile-依賴追蹤) | 解析、輸出及副作用 |
| [相容性約束](#相容性約束) | 保留刻意的原始資料行為 |
| [實作順序](#實作順序) | 前置條件與具體子項 |
| [證據與剩餘工作](#證據與剩餘工作) | 測試及尚未驗證的發現 |
| [Parsed-node 實作](#parsed-node-實作) | P-A 已交付範圍及 W04 剩餘工作 |
| [結構化 Fault 基礎](#結構化-fault-基礎) | P-C serializer 子批次及消費端邊界 |
| [已提交的刪除效果](#已提交的刪除效果) | 選定 K17 修正及剩餘 replay 邊界 |
| [完整 Action 路由](#完整-action-路由) | K06 路由子批次及 W03／W07 剩餘工作 |

## 入口與責任

下表為歷史原始碼基準；目前路由修正記錄於[完整 Action 路由](#完整-action-路由)。

| 原始碼符號 | 基準輸入／輸出及後續依賴 | 負責工作 |
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
4. 基準的 `apply_media2_configuration` 合成單筆 XML 並重複呼叫 bind／unbind。
   K16 state 修正後，會將擷取的值組成 plan，交給 `apply_configuration_bindings`，
   在 write lock 中驗證完整 plan 後才變更 slot。有 scope 的解碼仍屬 parser 工作；
   共用 writer 已不再把值插回 XML，也不重新解析 fragment。
5. `create_profile_in_state/delete_profile_in_state/bind_configuration` 呼叫
   `MockState::modify[_returning]` → `notify` → 持有 read guard 時執行 callback。
   集合相等、callback 次數及 replay 可見結果必須分別斷言；直接全域修改此
   generic helper 會影響其他服務。

**初始 K17 發現 — 內建 DeleteProfile 修正見下方：** ReplayResponder 在 SyntheticResponder
接受或拒絕寫入之前，就把 operation family 加入 invalidated；後續 Fault 不會
復原該失效狀態。因此只增加嚴格 synthetic rejection，仍無法保證 replay 的
可見結果不變。負責 W19／W03／W18；受影響工作卡包括兩服務的
CreateProfile／DeleteProfile、Media1 Add／RemoveVideoSourceConfiguration 與
Add／RemoveVideoEncoderConfiguration，以及 Media2 Add／RemoveConfiguration。
其他 mutation family 仍需同樣審查。
目標回歸：錄製可辨識的 Media1 GetProfile 結果，讓 DeleteProfile 被拒絕且
state 不變，再讀取時確認仍使用錄製結果；另以成功寫入使錄製結果失效作正向
控制。Chain 順序測試不代表 K17 已修復。

**K18 — create／list 不一致已重現，尚未修正：** replay::family 只移除開頭動詞，沒有
建立讀寫依賴。GetProfiles 成為 Profiles，CreateProfile／DeleteProfile 卻
成為 Profile；binding 成為 VideoSourceConfiguration、VideoEncoderConfiguration
或 Configuration，也不是 profile read 的 key。因此即使 mutation 成功，仍可能
繼續使用錄製的 profile list。Invalidation key 亦不含 service identity（store
lookup 則包含完整 Action）。由 W19／W10 負責；設計 outcome-based invalidation
前須核對同一批 13 張卡的讀寫依賴。目標測試：成功 create／delete／bind 後的
GetProfiles 應反映新 state；無關 service／instance 的錄製結果仍可使用。
不可只測 GetHostname／SetHostname 這類名稱恰好相符的組合。

`mock_fidelity_known_gaps.rs` 中最初以 `metamorph` 功能控制的 K17／K18 測試，使用
專案自製的 raw identity marker 重現兩條路徑；這些 marker 不是 schema fixture。
K17 精確斷言拒絕內容、完整序列化 device state 相等，以及錯誤切換至 synthetic。
K18 斷言新增資料確實儲存、清單仍過時、單筆讀取失效，以及獨立 instance 不受影響。
Binding／service 依賴仍僅完成原始碼確認。暫時停用 DeleteProfile invalidation，並讓
CreateProfile 額外使 Profiles 失效後，兩項 baseline 在完整全部功能
`--no-fail-fast` 執行中，皆於預期 replay 斷言失敗；兩項變動均已還原。
這些測試揭露缺陷，不代表已實作或驗收 invalidation 設計。
還原後本機 gate：格式與兩種 workspace Clippy 通過；全部功能 1,169、預設
1,087 測試通過，兩種模式各四項 ignored。清冊自我測試與原始碼核對通過，數量未變。
遠端 CI [34458754641](https://github.com/smiti1642/oxvif/actions/runs/34458754641)
已通過前一個認證 Fault commit `e6145b3`，不代表本次新增測試的遠端驗收。

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
  K16 選定 binding 的原子性已另行修正；僅延後 invalidation 不等於 transaction rollback。
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
| P-C | Structured Fault 基礎及選定 DeleteProfile commit observer 已實作；廣泛切換預設前完成其餘 mapping、effect 與消費端審查 | W05／W06／W19；不可隱含重新設計公開 trait |
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
其他服務的間接 reader、欄位契約與外部驗證仍待完成。K17／K18 的執行證據
記錄於下方；選定刪除路徑已修正，其餘 mutation 修正仍未完成。

本機證據：Windows、rustc 1.97.0、PowerShell 7.5.4，使用獨立
`target/mock-fidelity-build`。兩個新測試先通過，再修改注入 code 及 trim
captured request text，確認各自在預期斷言失敗，之後均已還原。
格式、預設／全部功能 workspace Clippy 與測試通過：全部功能 1,148、預設
1,068 passed，各 4 ignored。清冊自我測試／核對、192 個本機檔案連結及 10 個
新章節 anchor 均通過。本檢查點未變更 runtime 實作、公開 API、安裝、Release
或實機行為。接續從 P-A 開始；P-B／P-C 仍是廣泛遷移 handler 的前置條件，
不必重做已完成的 W00 盤點。

## Parsed-node 實作

P-A 已在 `src/mock/request.rs` 實作私有 owning Request 與借用式 Node accessor：
唯一直接 child、有序 repeated children、scalar text、必要且非空的 child text，
以及 expanded-name attribute。required_text 改用此表示法，兩種 DeleteProfile
契約不變。前面的來源表描述 `9978220` 基準；目前 attribute 已保留，不再丟棄。
尚未遷移 handler 使用的 attribute accessor 僅暫時允許非測試編譯的 dead code，
首個 handler 採用時須移除此標註。

Generic 控制涵蓋 attribute identity／default namespace、XML 空白與字元參照
正規化、repeated／subtree scope、缺少／空值／結構化值的區別、同一 tree 重用、
正規化 namespace alias，以及 depth／node 邊界的合法輸入。七個新測試在同一輪
各自擾動，均於預期斷言失敗，之後已還原。
參考：[XML 1.0 §2.11／§3.3.3](https://www.w3.org/TR/REC-xml/) 與
[Namespaces §6.2](https://www.w3.org/TR/xml-names/#defaulting)。

這不代表完整 W04 或全域 parse-once dispatch 完成：schema-specific 型別、範圍、
QName-valued content、extension policy 與 P-B／P-C 仍待完成。沿用既有 resource
bound，不將它解釋為裝置容量限制；未增加依賴、公開 API、schema table 或操作。

P-A 本機 gate：格式、兩種 workspace Clippy、全部功能 1,155 與預設 1,075 tests
通過（各 4 ignored）；清冊仍為 159 個宣告位置／157 routes／260 個直接 reader。
上述數字包含既有 known-gap assertion，不代表 K13–K18 已結案。

## 結構化 Fault 基礎

P-C 已具有私有 `Fault` 表示，包含型別化 Sender／Receiver code、有序 subcode
及 reason。可信 QName 定義使用編譯期 ASCII NCName；每個 Value 各自宣告
namespace，因此相同 prefix 對應不同 namespace 時不會互相污染。這不是任意
vendor／injection QName API。Reason 原始文字轉義一次；XML 不允許的字元改為
U+FFFD，CR 使用 character reference 表示。

本批僅遷移 `auth::auth_fault` 與空 chain 的防禦性 Receiver 回應。K20 已於修正
前重現：認證 QName 未宣告，XML 形式的 reason 可插入 child 並改變文字。
兩個測試皆在預期 assertion 失敗。修正保留 `s:Sender`、
`wsse:FailedAuthentication` 及既有憑證政策。Fault injection 與一般服務 Fault
映射仍採舊路徑。認證 request parsing 仍使用舊式 local-name reader；這不是
W08 安全性驗收。

一般巢狀 Fault 控制獨立檢查 QName scope／depth，並確認 client 與 health
消費端維持第一層 subcode，而非最深層。認證錯誤仍判定為認證失敗；一般巢狀
錯誤則不會。CLI 子程序控制使用 loopback 上啟用認證的 mock，測試 JSON 與
純表格模式：維持 exit 20、DEVICE_CONNECTION_FAILED、確切 reason 及不可重試
判定。未使用實機或憑證。

兩個認證控制於實作前失敗。之後以完整 workspace 擾動測試改變認證 code 與
兩個一般 fixture：認證 payload、兩個一般 Fault 測試、既有 chain-order
控制及 CLI JSON 控制均於預期 assertion 失敗。所有擾動均已還原；該次執行
沒有使用狹窄 filter 而漏跑 CLI target。

SOAP 設計參考：[SOAP 1.2 Part 1 §5.4](https://www.w3.org/TR/soap12-part1/#soapfault)。
Optional structured Detail、完整一般錯誤映射、HTTP status、其餘成功後 replay
outcome，以及更廣泛的巢狀錯誤消費端覆蓋仍待完成。未新增或重新定義公開
`SoapError` 欄位；文件現明示既有 subcode 只保留第一層與 prefix 拼法，Detail
則為擷取的文字，而非原始 XML。

本機驗收：格式、default／all-feature workspace Clippy、全功能 1,167 項與
預設 1,087 項測試通過（各 4 ignored）；Rust 1.88 workspace check 通過。
清冊仍為 157 routes／159 Action 位置／260 個直接 reader。前一個 checker
commit `9469bb6` 已通過 CI run 34456850826 全部 23 個 job；該託管執行不包含
本次後續的 Fault 修正。

## 已提交的刪除效果

P-C／W19 現在隨 synthetic XML 攜帶 optional 私有 `Effect`。既有兩個 DeleteProfile
route arm 傳入 per-request effect slot，僅 `Deleted` 分支設定 `ProfilesChanged`。
Terminal 在 handler 完成後呼叫私有 observer，不搜尋 XML，也不訂閱一般 persistence
hook。`RequestCtx` 欄位與 `Responder::respond` 不變；內部分析使用的既有
`dispatch` wrapper 仍只回傳 XML。

內建 MetamorphTransport 及 HTTP replay clone 對兩個精確 DeleteProfile Action
延後處理 invalidation。成功刪除後，依完整 Action 身分淘汰 Media1 GetProfile／
GetProfiles 及 Media2 GetProfiles。不存在／固定 profile 的拒絕保留錄製結果；其他
service 同尾名操作及獨立 instance 不受影響。K17 舊 known-gap 測試已改為保留結果
的回歸。`tests/mock_replay_effects.rs` 涵蓋兩服務寫入、兩種 transport、三個選定
讀取 view、完整 state 相等及無關 service／instance 控制。
`committed_effect_observer_is_not_a_request_or_fault_hook` 檢查注入 Fault、認證、
原始自訂回應、無效輸入、成功刪除及重複拒絕。

公開的單獨 ReplayResponder constructor 保留既有政策，因為它無法觀察由呼叫者
擁有的下游 responder。內建 clone 僅在自行掌管 terminal 時啟用私有 commit-aware
路徑。這是分階段遷移，不是公開設定切換，也不是整體 W19 驗收。其他 mutation 仍
使用舊 family invalidation，包含已重現的 K18 create 行為。更多 profile 相依讀取、
malformed Action、callback 順序及併發 linearizability 仍待處理。Effect observer
在既有 state-change callback 之後執行；本批次未使 callback 與 replay invalidation
成為原子操作，也未新增 rollback。

清冊擷取器現可接受多行 dispatcher 參數列表，新增一項正向自我測試，並保留全部
既有拒絕控制。兩個清冊的參數列納入 effect slot，route／Action／reader 數量不變。
未新增 schema 衍生 metadata 或 runtime dependency。

刪除效果驗證：恢復提前 invalidation 時，K17 與兩種 transport 回歸均失敗；
抑制 committed effect 傳遞時，observer 控制與兩種 transport 回歸均失敗。
兩輪完整 workspace、all-feature、no-fail-fast 擾動執行均於 assertion 失敗並
回傳非零狀態，所有擾動皆已還原。格式、兩種 workspace Clippy、單獨 mock
Clippy、兩種 warnings-as-errors 文件建置及全部 24 項 packaging 控制通過。
還原後全功能 1,174 項與預設 1,090 項測試通過（各 5 ignored、20 suites）。
新匯出的外部 corpus 共 34 份 XML，全部通過固定版本 Xerces 嚴格 XSD 1.1
驗證；這不代表語意符合性或整體計畫驗收。

## 完整 Action 路由

K06 路由子批次，以 `4bcb024` 為基準：`respond_with_effect` 現在從最後一個
分隔符切開，完整比對前段 service／port 身分。Events operation 另受所屬 port
限制；共用 dispatcher 不代表接受其他 port 的 operation tail。公開 client／session
Action 宣告及 157 項路由操作保持不變。

身分集合來自既有原始碼 Action 清冊，固定 Events 與 WSN 資源已於外部核對。
[Basic Profile 2.0 R2744 與 R2900](https://docs.oasis-open.org/ws-brsp/BasicProfile/v2.0/BasicProfile-v2.0.html)
要求符合宣告的 Action。R2757 另允許省略 Content-Type action 參數，本次路由
修正**尚未**實作該 HTTP fallback。SOAP 1.2 status、media type／encoding、
WSA 一致性、body 身分、parse-once dispatch 及通用 fault 對應仍待完成。

`action_aliases_never_reach_a_service_handler` 對從 client 來源取得的每個 Action
施加五種變形，斷言完整既有拒絕回應、沒有 committed effect，且整份狀態不變。
兩個 `*_action_identity_rejects_aliases_before_state_changes` 控制另涵蓋錯誤
Events port，以及經 HTTP／in-process 入口的 hostname 寫入，確認僅正確 Action
的寫入觸發通知。三項測試在舊路由的完整 all-feature、no-fail-fast 執行中均於
payload assertion 失敗（本機 log `1789041634_cargo_test.log`）；hostname
控制並證實非預期寫入。既有全來源正向路由與 chain 順序測試保留。格式、兩種
workspace Clippy 及兩種 warnings-as-errors 文件建置通過。修正後全功能
1,184 項及預設 1,100 項測試通過（各 5 ignored、21 suites）。清冊自我測試
通過，159 Action site／157 route／260 reader 不變。新匯出的外部 corpus 07
共 34 份選定 instance 全部通過 Xerces 嚴格 XSD 1.1 驗證；該 corpus 與 ignored
測試均不代表整體計畫驗收。

範圍僅限正常 synthetic 路由。公開 raw responder、suffix 型 fault injection
及既有 replay invalidation 仍為獨立工作。未知 Action 保留原有 flat code／reason，
不宣稱已符合最終規範 fault 契約。未變更公開 API 或已安裝 binary。
