# Mock 可信度施工檢查表

[English](mock-fidelity-execution-checklist.md) | [繁體中文](mock-fidelity-execution-checklist_zh.md)

規劃基準：`b134f73`，2026-09-10。政策依據：[已核准主計畫](mock-fidelity-hardening-plan_zh.md)。
範圍索引：[157 個路由操作](mock-fidelity-operation-ledger_zh.md)。
本文件將 M0–M6 拆成可追蹤工作，不代表里程碑已完成；本次規劃補充不改變 runtime 行為。

| 章節 | 用途 |
| --- | --- |
| [執行規則](#執行規則) | 不依賴對話歷史即可接續施工 |
| [開工條件與操作工作卡](#開工條件與操作工作卡) | 修改 handler 前必須完成的工作 |
| [稽核面向](#稽核面向) | 每個操作均須套用的檢查項目 |
| [工作清單](#工作清單) | 編號、相依性、位置及驗收 |
| [服務施工批次](#服務施工批次) | 可獨立提交的遷移範圍 |
| [程式碼與測試對照](#程式碼與測試對照) | 共用程式與獨立證據 |
| [風險清單](#風險清單) | 已知觀察與待查問題分開記錄 |
| [驗證命令](#驗證命令) | 可重跑的本機檢查 |
| [結案與交接](#結案與交接) | 證據、發布邊界及下一項工作 |

## 執行規則

1. 閱讀主計畫 D1–D3、本檢查表及逐操作清冊。以 `git status` 與實際 commit
   確認起點，不依賴上一輪對話的記憶。
2. 執行清冊檢查器，審閱記錄基準之後的程式碼差異。檢查器只比對 dispatch
   呼叫形式，不會發現所有 handler 內部變更或新增 helper。
3. 選擇相依條件已滿足的工作編號。服務遷移前，先完成該批次**所有操作列**的
   W01 工作卡，包含讀取操作。
4. 可獨立驗證的範圍分段 commit。同一批同步更新雙語文件、操作狀態、證據及
   下一項工作。不得因一個 helper 或兩個操作已修正就將整個里程碑標記完成。
5. 新發現須先記錄編號、來源 symbol、受影響操作 ID、重現方式或明確的
   `UNVERIFIED`、風險、測試位置與處置，再繼續施工。不得只留在對話或測試總數中。

工作狀態：`TODO`、`IN-PROGRESS`、`DONE`、`BLOCKED:<reason>`、
`NA:<reason/evidence>`。`PARTIAL` 僅用於已有紀錄的部分實作。
不以路由數量推估工期或完成百分比。

## 開工條件與操作工作卡

目前已列出全部路由位置，但**逐欄位契約、完整 Fault 映射及行為分類尚未完成稽核**。
W01 是下一批 handler 遷移前的必要設計工作，不是在修改測試後才補填的紀錄。
可以按批次進行；共用基礎工作開始前，不必先完成所有其他批次的規範核對。

每個清冊 ID 都須在 `docs/active/` 的批次紀錄中建立章節；文件須有英文／`_zh`
對應版本與表格式導覽，並填寫下列欄位：

| 必填欄位 | 實作前須記錄的內容 |
| --- | --- |
| 身分 | 穩定清冊 ID、來源 commit、client 完整 Action expression、dispatch 分支、operation namespace／local name、預期 endpoint 路由 |
| 程式路徑 | Client 方法及 session wrapper、handler、所有間接 request reader／validator／state writer／renderer，以及相關型別 parser／serializer |
| 目前輸入 | 由專案原始碼盤點欄位路徑、attribute、重複成員、預設值／trim／解碼、被忽略的參數；未接收 body 的 handler 也須明示 |
| 目前輸出 | Response renderer、共用 fragment 與 escaping 邊界、一般 Fault 的所有 return 路徑，以及 HTTP／in-process 行為差異 |
| 參考證據 | 適用官方 Service／Core／SOAP 文件 URL、版本與章節、外部 manifest ID／hash；記錄審閱結論或 `BLOCKED`，不得寫成「應該符合」 |
| 目標契約 | 修改內容與理由、相容性影響、必須保留的合法 extension／重複成員、未建模欄位的明確拒絕或文件限制 |
| 行為 | 已建模／靜態讀取／僅確認請求／未支援之一，並記錄可觀察限制；收到請求不是產生效果；列出 D2 預設與 opt-in 行為 |
| 狀態 | 寫入的確切 collection／key／field、對應 getter 或 state assertion、副作用、hook、queue、invalidation、rollback 與跨服務相依性 |
| 案例 | C01–C12 對應既有／新增測試的確切名稱；每項有測試或經審閱的 NA；無效輸入須驗證狀態及副作用均未改變 |
| 交付 | 工作編號、前置條件、受影響公開文件、開工時指定的負責者、預期證據位置及須由維護者決定的事項 |

不得將規範欄位表、schema 衍生 fixture 或產生的 schema index 放入工作卡。
依 D3，這些資料保留於外部；專案原始碼觀察及去識別化審閱結論可以納入 repository。
不得宣稱 parser 尚未具備的嚴格型別或 extension 驗證能力。

身分未明、必要輸入語意未核對、一般 Fault 分支未映射、狀態遺失未解釋，或存在
超出 D1 的公開 API 決策時，該批次不得開工。Discovery／唯讀實機測試不能取代寫入語意的驗證。

## 稽核面向

每個操作列均繼承以下面向；適用與否須記錄，不得直接假設。

| ID | 必查內容與具辨識力的案例 |
| --- | --- |
| C01 身分 | 正確／錯誤完整 Action、其他服務同名操作、operation／body 不一致、endpoint dispatch；不得假設 substring 比對合理 |
| C02 文字 | `&`、`<`、引號、Unicode、字面值 `&amp;`、numeric reference、CDATA、有意義的空白、空值與缺值；恰好一次 decode／escape |
| C03 XML scope | 替代／預設 namespace、兄弟／祖先元素 rebinding、Header／Extension／巢狀 body 誘餌、具 namespace 的 attribute、重複 expanded attribute |
| C04 結構與型別 | 必填／選填／重複成員、順序、子樹、extension；含數字寫法的 boolean、enum／範圍／overflow、適用時的 NaN／infinity；型別合法表示依外部規範核對 |
| C05 選擇目標 | 兩組刻意不同的 profile／sensor／configuration／job；缺少、未知、錯誤家族、escaped 及重複 token；區分「全部」filter 與缺少必要 selector |
| C06 狀態 | 所有接受的欄位均可觀察、失敗原子性、fixed／使用中／已刪除目標、配置／碰撞／連鎖刪除、共用 Media 狀態、hook／queue 與 instance 隔離 |
| C07 成功 wire | 獨立檢查 envelope／payload QName、attribute、順序／數量、字面文字及 URI escaping；不只依賴 client round trip |
| C08 Fault wire | Code 與有序巢狀 Subcode、實際 scope 中 QName binding、reason／language／detail、escaping、HTTP 映射；特定錯誤 payload 且不得修改狀態 |
| C09 Auth 與限制 | 開啟／關閉 auth、豁免操作、重複／錯置 WSSE 欄位、無效 digest／nonce／時間輸入；bytes／depth／nodes 有限且不回顯機密 |
| C10 行為分類 | Capability 與實作一致、靜態讀取有文件、未支援與未建模效果預設拒絕、opt-in stub 不宣稱實際效果 |
| C11 消費端相容性 | Client parsing／`SoapError`、HTTP transport、session fallback、CLI 人類輸出／Agent envelope／error／exit code；異常 injection 與 replay 維持明確邊界 |
| C12 證據敏感性 | 確切 assertion、正負控制、針對性擾動及精確還原、HTTP／in-process 一致性、外部 schema 覆蓋及平台／feature 證據 |

## 工作清單

下列路徑相對於 repository。既有檔案可由後方對照表開啟；提案中的新路徑不表示
工具或測試已存在。W10–W15 均包含 W05 之後逐操作的 Fault 遷移，不只有 request parsing。

| ID／里程碑／狀態 | 前置條件 | 位置及交付物 | 驗收證據 |
| --- | --- | --- | --- |
| W00／M0／DONE | 無 | `dispatch.rs`、清冊、檢查器、來源稽核：含 session 的字面值 Action site 與全部路由對應；runtime 寬鬆別名另列 K06／W07 | 完整 Action／site 索引及精確 route 集合相等已驗證；正向／缺少／改接／重複控制通過；完成的是來源清冊，不是 runtime 拒絕或規範驗證 |
| W01／M0／IN-PROGRESS | 所選批次的 W00 | 依上方模板建立逐操作工作卡，規範核對保留外部；逐操作盤點 field／Fault／effect | 全批次工作卡符合開工條件；C01–C12 均有安排；無未解釋預設或 response 分支；程式碼／規格衝突先記錄再修改 |
| W02／M0／PARTIAL | W00 | `xml_parse.rs`、全部 service、`request.rs`、`auth.rs`、`canon.rs`：列出 caller 與間接 helper 相依性，分類文字／attribute／子樹／raw 用途 | 每個舊 caller 都有遷移負責項目或明確隔離理由，包含 test-only reader；新找到的 reader 加入來源對照 |
| W03／M3／PARTIAL | W02 | 單一 parsed synthetic request 與 private route 驗證共用 XML／container／operation identity；DeleteProfile 借用 parsed operation | HTTP／in-process 靜態及狀態型邊界控制、raw 優先序與 quirk baseline 控制；完整 HTTP／header 策略及逐操作欄位遷移仍待完成 |
| W04／M1、M3／PARTIAL | W02 | `request.rs`：P-A private owning request、scalar／scoped attribute／subtree／repeated 存取已實作；typed field rules 與 QName-valued content 待完成 | 七個 P-A 控制已擾動並還原；C02–C04／C09 仍須涵蓋已遷移 handler、合法 extension 及欄位限制 |
| W05／M2／PARTIAL | 映射須 W01；caller 須 W02 | 結構化 auth／空 chain／選定 DeleteProfile fault，另納入已審查的 generic synthetic-boundary fault；資源／DTD 名稱明確屬 mock 自訂策略 | Expanded fault QName、typed failure、消費端及 state 控制；其他一般服務分支與 structured Detail 待完成，不概括宣稱符合規格 |
| W06／M2／PARTIAL | 預設切換前完成 W05 設計 | 已完成第一層 subcode 的 client／health 控制，以及認證 CLI JSON／table 子程序控制；公開錯誤欄位未改變 | 服務預設切換前仍須擴充 nested／flat／vendor、transport／session 與消費端覆蓋；維持診斷及 exit-code 意義 |
| W07／M2、M3／PARTIAL | W03／W05 設計、W06 | 已實作完整 synthetic Action 路由及共用 body identity；HTTP 擷取與 binding 仍未完成 | 全來源路由及 HTTP／in-process 邊界控制；content type／status／無效 UTF-8／缺少或衝突 header 及 endpoint 策略尚未驗收 |
| W08／M3／TODO | W02／W04／W05 | `auth.rs::requires_auth/validate_ws_security/auth_fault`、responder auth gate、Device users | Scoped WSSE parsing、豁免匹配、digest／時間／nonce 限制；明示 authentication 與 authorization 邊界；錯誤去除機密，不附帶變更 auth 預設 |
| W09／M2／TODO | W05／W06 | `fault_injection.rs`、`responder.rs`、server admin endpoint、公開 injection builder | 分離 literal／structured 與刻意 raw 異常輸出；自訂 QName、single-shot 匹配、順序、併發、clear／reset 及相容測試 |
| W10／M2–M4／PARTIAL | 批次 W01、W03–W06 | `services/media.rs`、`media2.rs`、共用狀態與 renderer；按下方批次施工 | 每個 Media 列通過 C01–C12；兩種 view 狀態一致但不共用錯誤 wire shape；E1 不代表 DeleteProfile 列結案 |
| W11／M2–M4／PARTIAL | W01、W03–W06 | PTZ1 已涵蓋 19 個既有 profile／head 使用端的 scoped ProfileToken 身分；其他 selector、座標 attribute／space、configuration 子樹、preset／tour 與 auxiliary command 仍待完成 | 兩種 transport 與兩個不同 head；完整欄位效果／fault policy／併發仍未驗收，不虛構移動／時間保證 |
| W12／M2–M4／TODO | W01、W03–W06 | `services/imaging.rs`：逐 source 的 settings／options／status／move／stop | 固定／可移動鏡頭、巢狀設定及型別範圍；無全域同名欄位 fallback 或靜默部分套用 |
| W13／M2–M4／TODO | W01、W03–W06、W08 設計 | `services/device.rs`、DeviceIO dispatch、device state | 重複 users／network entries／scopes、storage 子樹、relay token；失敗不改 state／auth／events／hooks；維護效果依 D2 分類 |
| W14／M2–M4／TODO | W01、W03–W06 | `services/recording.rs`：分開的 Recording／Search／Replay dispatch 與狀態生命週期 | Recording／track／job 辨識及連鎖處理；search token／終止／timeout、replay 選擇；有限模擬不代表實際錄影或媒體傳送 |
| W15／M2–M4／TODO | W01、W03–W06 | `services/events.rs`、IO event queue、subscription state | 核對 filter namespace／dialect、lifetime／renew／unsubscribe／pull 限制、queue 隔離／順序／終止；既有 Events sync 不是 PR #16 Media sync |
| W16／M4／TODO | W01 分類、W05／W06 | 提案中的 operation policy registry、mock transport／server builder、responder | 精確 service＋operation opt-in；未建模效果預設拒絕且不改狀態；區分 unsupported 與已啟用 ack；僅必要時加入有上限追蹤，排除憑證／raw envelope |
| W17／M4／TODO | W10–W16 分類 | 全部 capability renderer、`discovery_responder.rs`、`fleet.rs`、`snapshot.rs`、`font.rs`、公開 mock 文件 | Services／XAddrs／feature／limit 與建模行為一致；核對 discovery／snapshot 側路徑；靜態 URI／圖片不證明 codec／串流輸出 |
| W18／M4／PARTIAL | W10–W16 候選行為 | K13 無碰撞配置、K16 原子 binding plan、條件式通知；K08 hook 在鎖外接收 commit 快照，profile／catalogue 讀取共用一次快照 | 選定配置、binding、reentrant 及三路徑 profile snapshot 控制；更廣泛併發寫入、instance、rollback、其他 queue／read snapshot 及 replay 待完成；公開 signature 不變，callback 排序由使用者管理 |
| W19／M3、M6／PARTIAL | W03／W09 設計 | 內建 profile 建立／刪除、Media1 video binding 及 Media2 generic binding 使用私有 committed effect；跨服務讀取、HTTP、instance 及 chain 控制 | Configuration 寫入與其他 mutation、單獨 replay 政策、完整讀取依賴、正規化／key collision 及併發／callback 可見性仍待完成；不新增錄製設備機密 |
| W20／M5／PARTIAL | W04／W05 corpus | `tests/mock_schema_shape.rs` 已完成 scoped resolution、Envelope／Fault 納入及缺少資源即失敗；見 schema 前置檢查 | 七個一般控制與 Fault wrapper 擾動已驗證敏感度；QName 值、wildcard／未解析計數及 request corpus 仍待處理；pin 未修改 |
| W21／M5／PARTIAL | W20、D3 | 固定來源的離線工具、20 項無官方 schema 控制、七項獨立後端測試，以及第一批 13 操作 client／mock corpus 匯出與明確 payload anchor | 選定 DeleteProfile Fault 遷移後，34 份 instance 通過，包含四個不存在／固定 profile 拒絕。其餘操作及更廣的輸入／語意覆蓋尚未驗收 |
| W22／M5／PARTIAL | 清冊需 W00；schema job 需 W21 | Windows／Linux 清冊、Xerces 選型、官方來源編譯及選定 profile instance 驗證作為 package 前提 | 來源與 corpus 均置於外部，不上傳 artifact。選定 corpus 有 13 操作的 34 份 instance；全程式 instance 覆蓋及 release 證據檢查仍待完成 |
| W23／M1、M6／TODO | 各遷移批次 | 所有具名回歸 suite、client fixture、一般 parser tests | 檢查空殼正負測試及 namespace-stripped／fragment probe；擾動須在目標 assertion 失敗；以 `--no-fail-fast` 跑全部 target；有限 fuzz／property 測試記錄 seed／限制 |
| W24／M6／TODO | 整合候選版本 | Cargo feature／MSRV、`.github/workflows/ci.yml`、`packaging/check_xml_features.py`、文件建置 | Windows／Linux／macOS 原生 default／all-feature、per-feature warning sweep、MSRV、下游 XML feature-unification；缺乏證據明示 blocked／not-run |
| W25／M6／TODO | W00–W24 驗收 | 受影響雙語 mock／library／CLI／support 文件、`OPERATIONS`、README 連結、CHANGELOG、rustdoc、release 證據 | D1／D2 遷移有可用範例；核對目前宣告及歷史註記，不改寫已發布事實；publish／merge／push／install 依授權 |
| W26／條件式／TODO | 另行授權 PR 整合 | 重新審閱 PR #16 當時 head；Media `SetSynchronizationPoint` client／session／mock／tests／docs | Head 改變須重審；不自動合併；若納入則新增清冊／工作卡、特殊 token 負向案例、ack／effect 邊界及雙語 operation table，否則記錄未整合 |

## 服務施工批次

每組均包含所有對應 getter／options／capability 路徑，不只 setter。
逐操作清冊是完整範圍，下表決定施工順序。每批開工前，在紀錄列出所選確切
清冊 ID；服務完成時，檢查已結案批次 ID 的聯集等於該服務全部清冊列。

| 工作 | 建議可獨立提交順序 | 額外相互影響 |
| --- | --- | --- |
| W10 | Profile／create／delete／binding → video source／encoder／options → audio／metadata → OSD → stream／snapshot URI、source mode／靜態 capability | 共用 `ConfigKind`／selector／renderer；Media2 呼叫 Media1 helper；型別 attribute／element 差異；fixed／使用中 reference |
| W11 | Profile／node／config selector → configuration／space → movement／home／preset → tour／auxiliary／靜態 capability | 多 profile 共用 node 與不同 head 的差異；重複 tour spot；停用軸；不宣稱實際移動時間 |
| W12 | Source selector／settings／options → status／move options／move／stop → capability | Focus 支援、巢狀 mode／value 設定、目前忽略的輸入 |
| W13 | Hostname／time／scopes → users／auth → DNS／NTP／interface／protocol／gateway → storage → relay／DeviceIO → maintenance／discovery／services／capability／log／URI | 多筆更新的部分失敗、password、IO event 與儲存設定；模擬網路變更不得修改 host 網路 |
| W14 | Recording CRUD → track／job／state → Search 生命週期 → Replay URI／service capability | 刪除被引用 recording／track／job、產生 token 碰撞、靜態 session／URI 宣告 |
| W15 | Event properties／capability → create／filter／pull → subscribe／renew／unsubscribe／sync | 多 subscription、共用 filter／event queue、IO-to-event 可見性、timeout／limit |

## 程式碼與測試對照

納入共用及未經 operation dispatch 的路徑，避免「操作已列完」掩蓋周邊程式。
W02 發現新相依性時須擴充本表。

| 面向 | 既有位置 | 負責工作／證據起點 |
| --- | --- | --- |
| 路由與 reader | [dispatch](../../src/mock/dispatch.rs)、[舊擷取](../../src/mock/xml_parse.rs)、[request parser](../../src/mock/request.rs)、[helpers](../../src/mock/helpers.rs) | W00–W07；`tests/mock_request_identity.rs`、dispatch tests |
| Pipeline／transport | [responder](../../src/mock/responder.rs)、[mock transport](../../src/mock/transport.rs)、[server](../../src/mock/server.rs)、[HTTP client transport](../../src/transport.rs) | W03／W06–W09；HTTP／in-process request boundary 測試已實作，HTTP binding 測試仍待完成 |
| Auth／injection | [auth](../../src/mock/auth.rs)、[fault injection](../../src/mock/fault_injection.rs)、[SOAP security](../../src/soap/security.rs)、[envelope](../../src/soap/envelope.rs) | W08／W09；unit tests 加上無效 WSSE 的直接 HTTP 控制 |
| Service 實作 | [services module](../../src/mock/services/mod.rs)、操作清冊所連結的各檔案 | W10–W15；對應 `src/client/*.rs`、`src/tests/client/*_tests.rs`、`src/types/*.rs` |
| State／側路徑 | [state](../../src/mock/state.rs)、[discovery responder](../../src/mock/discovery_responder.rs)、[fleet](../../src/mock/fleet.rs)、[snapshot](../../src/mock/snapshot.rs)、[font](../../src/mock/font.rs) | W17／W18；既有 state tests、`tests/mock_multi_sensor.rs` |
| Replay／canonicalization | [canon](../../src/mock/canon.rs)、[Metamorph module](../../src/metamorph/mod.rs)、`src/metamorph/*.rs` | W19；module 內 replay／fixture／record／quirk tests，保留 raw fixture |
| Client 消費端 | [XML](../../src/soap/xml.rs)、[SOAP error](../../src/soap/error.rs)、[error](../../src/error.rs)、[session](../../src/session.rs)、`src/types/*.rs` | W06；`tests/xml_compat.rs`、session／client／type tests；不表示授權全面重寫 client |
| CLI 消費端 | [error](../../crates/oxvif-cli/src/error.rs)、[application](../../crates/oxvif-cli/src/application.rs)、[output](../../crates/oxvif-cli/src/output.rs)、[agent](../../crates/oxvif-cli/src/agent.rs)、[contract](../../crates/oxvif-cli/src/contract.rs)、[manage](../../crates/oxvif-cli/src/manage.rs)、[maintenance](../../crates/oxvif-cli/src/maintenance.rs) | W06／W25；`crates/oxvif-cli/tests/cli.rs`；用 `rg` 追蹤其他分類消費端，不重設 navigation |
| 語意 property | [round trip](../../tests/mock_roundtrip.rs)、[token discrimination](../../tests/mock_token_discrimination.rs)、[Media agreement](../../tests/mock_media1_media2_agree.rs) | W10–W18／W23；逐名稱核對既有 `Broken`／`Static`／`Blind` 列，不引用舊計數 |
| Wire／workflow | [schema shape](../../tests/mock_schema_shape.rs)、[workflow](../../tests/mock_workflow.rs)、[action snapshot](../../tests/mock_action_snapshot.rs)、[capability](../../tests/mock_service_capabilities.rs)、[fixture](../../tests/fixtures/README.md)、[共用測試 helper](../../src/tests/common.rs) | W20–W23；區分 helper 產生的 fixture 與獨立驗證 |
| 公開宣告／去識別化 | [mock module](../../src/mock/mod.rs)、[crate header](../../src/lib.rs)、[redaction](../../src/redact.rs)、`docs/mock-server*.md`、`docs/support*.md`、公開指南 | W08／W17／W25；文件及去識別化 artifact 審查 |

## 風險清單

以下是規劃起始項目，不代表已完成全面缺陷稽核。

| ID／證據層級 | 觀察或問題 | 負責工作／結案方式 |
| --- | --- | --- |
| K01／已重現、部分修正 | DeleteProfile escaped／decoy token 缺陷已由 E1 修正，其他操作未系統性重現 | W04／W10；保留 E1 並新增工作卡 |
| K02／程式碼確認 | Service 與 auth 仍使用舊 fragment reader | W02–W15；M3 結案時不得有未交代的一般 caller |
| K03／選定 DeleteProfile 分支已修正 | 兩個服務對不存在／固定 profile 使用巢狀 Sender Fault；選定 corpus 的獨立驗證通過 | W05／W06；其餘操作映射仍待完成。Client 仍回報第一層而非最深層 subcode；全程式驗收尚未完成 |
| K04／已記錄風險 | 舊 escaped 輸入經 escaping Fault helper 回顯可能重複轉義 | W05／W23；追蹤每個含插值的 reason，重現受影響路徑 |
| K05／程式碼確認 | Factory reset、Events unsubscribe／sync 存在空成功路由，意圖／效果待分類 | W13／W15／W16；逐操作分類，不將所有 `resp_empty` 視為缺陷 |
| K06／路由子批次已修正、HTTP 未完成 | Synthetic Action 別名不再進入 handler；HTTP handler 仍回 200 並使用 lossy UTF-8 conversion | W03／W07；參閱管線路由證據；共用 body 一致性與 generic boundary fault 已實作；HTTP 擷取／status 及 replay 仍未完成 |
| K07／程式碼確認 | Schema／namespace probe 有 scope／Fault 覆蓋缺口 | W20–W22；具失敗敏感性的獨立驗證 |
| K08／選定 hook 缺陷已重現並修正 | Hook 原本持有 read lock，且可能觀察介入寫入而非原 mutation；現以 owned commit 快照在鎖外執行 | W18／W19 部分完成；管線開工核對有確定性 reentrant／snapshot 控制；queue、read snapshot、廣泛寫入及 replay 可見性仍待完成 |
| K09／未驗證 | 必填欄位／範圍／extension／capability 宣告可能與契約不符 | W01／W10–W17；實作前完成批次工作卡 |
| K10／未執行 | E1 沒有完整外部 schema、Linux／macOS 原生、多廠牌驗證 | W21／W24；實機證據分開記錄，未另授權只做唯讀 |
| K11／待整合 | PR #16 不在來源基準內 | W26；另行重審，不默默納入 |
| K17／內建 DeleteProfile 路徑已修正 | 內建 in-process／HTTP clone 於拒絕刪除時保留錄製結果，commit 後才淘汰選定的跨服務 profile read | W19／W03／W18 部分完成；其他 mutation、單獨 responder 政策、完整相依圖及併發／callback 可見性仍待完成；見管線開工核對 |
| K18／選定 create／binding 依賴已修正 | 已提交的 CreateProfile 與已建模 Media binding 淘汰三個 profile-read Action；拒絕寫入保留錄製結果 | W19／W10 部分完成；兩種 transport、無關服務及獨立 instance 已測試；完整讀取相依圖與其他 mutation 仍待完成 |
| K27／選定 replay 回覆替換已限制 | 九組碰撞於無法確認 scoped identity 時轉入 synthetic；exact raw recording 與 qualified-header ephemera 仍可 replay | W19 部分完成；兩種 transport 已測試；index 仍碰撞、被覆蓋錄製無法恢復，完整 key／QName／protocol 遷移仍待完成 |
| K28／key 中的 URL 憑證已修正 | Canonical projection 與舊檔／caller key 清除 URL 帳密；載入唯讀，明確保存才持久化清理 | W19 部分完成；虛構資料的隱私／lookup／磁碟控制，JSON 格式不變；raw envelope 僅針對指定格式去憑證，K27 index 碰撞仍待修正；見管線開工核對 |
| K29／共用 XML 空白保留已修正 | 共用 escaping 在 text／attribute 以 numeric reference 保留 CR／LF／tab，Fault 使用相同 helper | W03／W06／W10 部分完成；已檢查兩種 transport、共用 helper 與 34 份外部 instance；raw token／nested renderer 與欄位限制仍待完成，詳見 profile preflight |
| K19／已重現的外部相容性發現 | 目前 Media 來源相依集合在獨立 XSD 1.0 驗證器中無法編譯，但可通過 strict XSD 1.1 編譯 | W21；見 [schema 前置檢查](mock-fidelity-schema-preflight_zh.md)；不修改 schema 或停用檢查，須驗收候選工具並明示 schema 語言 |
| K20／已重現並修正 formatter | `auth::auth_fault` 原先輸出未宣告的 wsse subcode 及原始 reason 文字 | W05 serializer 遷移維持 code／subcode，修正 scoped binding／text，並加入 client／health／CLI 控制；認證解析與政策仍屬 W08 待辦 |
| K21／Python 限制；已有獨立編譯路徑 | Python 回報 Device type-table warning；固定版本 Xerces 以 full checking 及 warnings-as-errors 編譯同一完整閉包通過 | W21；獨立 generic 選型通過，未修改 schema 或停用警告；不代表 Python 警告錯誤或 mock instance 有效 |
| K22／client 與選定 mock selector 已修正 | 完整 Media2 profile 查詢原先省略 Type，而 mock 一律回傳 configuration；client 現在明確要求 All | W06／W10；擷取請求回歸在舊 body 上失敗。Scoped mock Token／Type 選擇現有兩種 transport 及 token-table 控制；廣泛欄位／輸出仍屬 P-E；僅驗證 XSD 無法發現原始查詢意圖差異 |

來源稽核另追蹤輸出型別相關發現：K24 audio codec 詞彙（W01／W10）、
K25 multicast 唯讀／效果意義，以及 K26 共用服務 view 的 video codec 可表示性
（W01／W10／W17）。相關 profile／configuration 工作卡結案前，均須補上
具辨識力的 wire／state 重現及明確處置；來源審查不等於 runtime 驗證。

## 驗證命令

K15 已有有界 Name 修正：兩個 CreateProfile 操作讀取 scoped、解碼後 scalar
名稱，兩個 profile renderer 轉義一次。已實作 seeded-state 及 HTTP／in-process
create／read／refusal 控制。呼叫者提供的 profile token、configuration 文字及
其他欄位規則仍屬 W01／W10；不代表 profile 批次完成，也不繞過其餘開工條件。

在 repository 根目錄執行，使用隔離建置目錄，不覆寫已安裝 CLI。
`rtk` 是此 workspace 的命令代理。必須檢查 native exit code，不以摘要文字
單獨作為成功證據；紀錄包含 commit／toolchain／OS 的去識別化結果。

```powershell
rtk git status --short --branch
rtk git rev-parse HEAD
rtk rustc --version
rtk powershell -NoProfile -File docs/active/check-mock-fidelity-inventory.ps1 -SelfTest
rtk rg -n 'extract_tag|extract_all_tags|extract_attr|XmlNode|from_xml' src/mock
rtk rg -n 'resp_soap_fault|auth_fault|SoapError::Fault|subcode' src crates/oxvif-cli tests
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features --locked --target-dir target/mock-fidelity-build -- -D warnings
rtk cargo clippy --workspace --all-targets --locked --target-dir target/mock-fidelity-build -- -D warnings
rtk cargo test --workspace --all-features --locked --target-dir target/mock-fidelity-build --no-fail-fast
rtk cargo test --workspace --locked --target-dir target/mock-fidelity-build --no-fail-fast
```

Rustdoc 連結改變時，於該 process 設定 `RUSTDOCFLAGS=-D warnings`，執行兩種
`cargo doc --no-deps` 組態，採 locked dependencies 與隔離 target 並記錄環境。
W24 依 `CLAUDE.md` 執行 doctest、per-feature／MSRV／XML-unification 檢查。
`rg` 是稽核輔助，不是 call graph 證明；仍須檢查 alias、wrapper、子樹消費端及測試 fixture。

W21 選定工具後必須提供確切的外部 validator 命令。既有 `OXVIF_ONVIF_SCHEMA`
ignored test 只做 structural check，不能取代該 gate；目前不得虛構命令或標記通過。

## 結案與交接

操作工作卡、C／R／F／B／V 證據、相關共用工作及文件一致，才可標記 DONE。
須附確切測試名稱與 assertion 目的、負向控制、擾動／還原證據、命令／exit code、
commit、feature／toolchain／OS、外部 manifest hash。Passed／failed／blocked／
not-run 分開記錄；schema skip、既有 `Broken`／`Blind` 預期不算新的符合規格證據。
例外須有具名範圍、理由及處置。

每次交接記錄：已完成工作 ID／操作 ID、目前 commit／dirty files、未結案發現、
下一個可開工 ID、尚需測試、使用者決策或外部前置條件。待辦發現必須有 ID 及
負責工作項目才能移交；接續施工以這些紀錄為準，不依賴對話歷史。

目前進度：[來源稽核](mock-fidelity-source-audit_zh.md) 已完成 W00 字面值來源核對，
W02 直接 caller 索引完成但間接路徑仍待查；第一批 13 張
[W01 工作卡](mock-fidelity-profile-preflight_zh.md) 已建立，尚未符合遷移條件。
選定共用路徑已展開於[管線開工核對](mock-fidelity-pipeline-preflight_zh.md)，包含
K17 提前 replay 失效及 raw extension 控制。P-A private parsed accessor、
P-C typed generic fault 及 P-D 共用 synthetic 邊界均已實作。
**下一項為完成 P-B 欄位／語意開工條件及剩餘 W04 typed／QName 規則，以進行
P-E profile 遷移；W07 HTTP binding 及 W08 scoped auth 仍待完成。**
預設 boundary fault 僅更動已審查子集，廣泛的 W06 一般服務錯誤遷移尚未驗收。
W20／W21 可先準備，不必等待所有服務遷移結束。
[Schema 前置檢查](mock-fidelity-schema-preflight_zh.md) 記錄 W20 scoped checker／
Fault 子批次及 W21 工具實驗；兩個工作 ID 均未完成。

本次規劃不選定版本、不合併 PR #16、不 publish／push／安裝 binary。
最終發布驗收須有記錄候選 commit 上的 M0–M6 證據及適用維護者授權，並保留
更新使用者系統上的 Release 前須先提醒的要求。
