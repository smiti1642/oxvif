# Media profile 與 binding 開工盤點

[English](mock-fidelity-profile-preflight.md) | [繁體中文](mock-fidelity-profile-preflight_zh.md)

工作：第一批 W10 的 W01／W02。基準 `892aa94`；2026-09-10 開始調查。
**IN-PROGRESS，尚未符合廣泛遷移 handler 的開工條件。**
負責者：目前 hardening 分支。[施工檢查表](mock-fidelity-execution-checklist_zh.md) ·
[來源索引](mock-fidelity-source-audit_zh.md) · [逐操作清冊](mock-fidelity-operation-ledger_zh.md)

| 章節 | 用途 |
| --- | --- |
| [範圍與身分](#範圍與身分) | 確切 13 個操作工作卡 |
| [共用路徑與目前行為](#共用路徑與目前行為) | 輸入、效果及消費端 |
| [參考資料審閱](#參考資料審閱) | 已確認結論及尚缺證據 |
| [案例與開工條件](#案例與開工條件) | 必要回歸與剩餘設計 |

## 範圍與身分

下表是各操作工作卡的「目前行為」部分；13 張工作卡均繼承下方共用路徑、
參考審閱及 C01–C12。輸入是既有原始碼觀察，**不是**規範欄位表或核准的預設值。
各操作完整 Action URI／來源方法已分別記錄於來源索引；
handler 位於 `src/mock/services/media.rs`／`media2.rs`，dispatch 位於
`src/mock/dispatch.rs`。完整 URI 必須符合索引值，不只最後一段。
Client service URL 由 session service discovery 取得；目前 MockTransport 忽略 URL，
MockServer 採 catch-all POST 路由。

全部列在 handler 之前均繼承共用 synthetic XML／container／Action-body 驗證。
下表「輸入擷取」描述的是 handler 欄位，不代表繞過共用邊界。DeleteProfile
借用 parsed operation；`required_text` 僅保留為 test helper。Generic boundary
fault 優先於表列操作專屬分支；其他欄位層級解析尚未遷移。

| 清冊 ID | Client → handler | 目前輸入擷取 | 狀態／renderer 路徑 | 目前一般 Fault code（未註明者為平面） |
| --- | --- | --- | --- | --- |
| `media.GetProfiles` | get_profiles → resp_profiles | 不接收 body | profiles + catalogues → render_profile | Handler 無操作專屬錯誤分支 |
| `media.GetProfile` | get_profile → resp_profile | GetProfile fragment → ProfileToken；缺少時為空字串 | profiles + catalogues → render_profile | ter:NoProfile |
| `media.CreateProfile` | create_profile → handle_create_profile | Name 預設 Profile；選填 Token；舊 fragment／text | create_profile_in_state → profiles, next_token_id → render_profile | ter:ProfileExists |
| `media.DeleteProfile` | delete_profile → handle_delete_profile | parsed operation.required_child_text(ProfileToken)，嚴格 scalar identity | delete_profile_in_state → profiles → empty response | 欄位驗證：env:Sender；不存在／固定：巢狀 s:Sender（見下方審閱） |
| `media.AddVideoSourceConfiguration` | add_video_source_configuration → handle_add_video_source_configuration | ProfileToken；ConfigurationToken，並以 Token fallback | bind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoSourceConfiguration` | remove_video_source_configuration → handle_remove_video_source_configuration | ProfileToken；舊 scalar | unbind_configuration(VideoSource) → profile slot | env:Sender / ter:NoProfile |
| `media.AddVideoEncoderConfiguration` | add_video_encoder_configuration → handle_add_video_encoder_configuration | ProfileToken；ConfigurationToken，並以 Token fallback | bind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile / ter:NoConfig |
| `media.RemoveVideoEncoderConfiguration` | remove_video_encoder_configuration → handle_remove_video_encoder_configuration | ProfileToken；舊 scalar | unbind_configuration(VideoEncoder) → profile slot | env:Sender / ter:NoProfile |
| `media2.GetProfiles` | get_profiles_media2 → resp_profiles_media2 | Parsed Token／Type、scoped scalar 及直接 sequence 檢查 | cloned profile projection + media::catalogues → render_profile_media2 | Generic field fault；巢狀 s:Sender／InvalidArgVal／NoProfile |
| `media2.CreateProfile` | create_profile_media2 → handle_create_profile_media2 | Name 預設 Profile；不讀 Configuration | media::create_profile_in_state(None) → profiles, counter → Token response | ter:ProfileExists 分支（目前傳 None 不會到達） |
| `media2.DeleteProfile` | delete_profile_media2 → handle_delete_profile_media2 | parsed operation.required_child_text(Token)，嚴格 scalar identity | media::delete_profile_in_state → profiles → empty response | 欄位驗證：env:Sender；不存在／固定：巢狀 s:Sender（見下方審閱） |
| `media2.AddConfiguration` | add_configuration_media2 → handle_add_configuration_media2 | ProfileToken；重複 Configuration／Type／Token；不讀 Name | apply_media2_configuration → atomic media::apply_configuration_bindings(add=true) | env:Sender / ter:ConfigurationConflict / ter:NoProfile / ter:NoConfig |
| `media2.RemoveConfiguration` | remove_configuration_media2 → handle_remove_configuration_media2 | ProfileToken；重複 Configuration／Type／Token | apply_media2_configuration → atomic media::apply_configuration_bindings(add=false) | env:Sender / ter:ConfigurationConflict / ter:NoProfile |

## 共用路徑與目前行為

選定的間接依賴、消費端清單及 P-A–P-E 實作順序見
[管線開工核對](mock-fidelity-pipeline-preflight_zh.md)。K17 將成功前的 replay
失效行為獨立追蹤，不與 K14 的 state hook 問題混為一談。

- Request：session wrapper（含 Media 版本偏好／fallback）→ client method →
  `OnvifClient::call` envelope／security → MockTransport／MockServer →
  FaultResponder → AuthResponder → 選用 replay → SyntheticResponder → dispatch。
  13 個 client 方法會 escape 呼叫者字串；舊 mock 擷取不解碼。共用 identity／XML
  驗證現在也涵蓋靜態 handler。
- Scalar：`xml_parse::{extract_tag,extract_all_tags,extract_attr}` 依 local name
  找位置並傳回裁切／raw fragment；Delete 使用已解析 operation 的 scalar accessor。
  Binding 現在將擷取的值傳入共用原子 plan，不再合成及重複解析單筆 XML。
  有 scope 的輸入解碼仍待獨立 parser 遷移。
- Create：explicit duplicate 檢查與新增現在使用同一 write lock；產生 token 時跳過已用身分。
  新增 `ProfileEntry` 的 configuration slot 全為 None，fixed 為 false；未建模
  capacity 檢查。Media2 忽略初始 Configuration。K13 碰撞在兩服務已有正確回歸；
  共用 helper 控制涵蓋併發配置、重複請求完整 state 保留、通知次數及 counter 邊界。
- Delete：`delete_profile_in_state` 於 write lock 下找 token，拒絕 fixed／missing，
  否則移除一個 profile。K14 已使用明確的 committed-outcome 通知 predicate 修正：
  拒絕時保留 state 且不呼叫 hook，成功刪除則通知一次。這不是 rollback，也不改變
  公開 state helper 的語意。
  Profile-change event／reference cascade 尚未完成稽核，不宣稱缺少它們符合規格。
- Binding：`ConfigKind::{from_media2_type,known_token,slot}` 選五種已建模類型。
  兩個 wrapper 都使用 `apply_configuration_bindings`，在同一 write lock 中
  檢查 profile／config 並寫入全部 slot。Media2 先解析 kind 再提交完整列表。
  Fixed 不阻止 binding 變更。已重現的 K16 部分寫入已修正，兩種 transport
  皆有完整 state 及單次通知控制。
  未建模類型、Type=All、只更新 name、configuration conflict 須明確審查目標行為。
- Rendering：`profile_snapshot` 在同一 read guard 內取得 profile 及所有 configuration catalogue。
  `render_profile`／`render_profile_media2` 內嵌 VSC、encoder、audio source／encoder、
  PTZ renderer。Profile name／token 目前直接插入；K15 證明兩服務 Name 會形成 XML child。
  其他巢狀 renderer escaping 與其他快照路徑仍屬 W10／W18。
- State hook：`MockState::{modify,modify_returning,notify}` 在 write lock 內取得
  mutation 快照，釋放鎖後才呼叫 callback，支援有界重入寫入。Callback 序列化與失敗寫入的 replay invalidation
  屬 W18／W19；mock 本身不實作外部持久化。
- 消費端：`src/types/media.rs` 的 `MediaProfile`／`MediaProfile2` 與巢狀
  configuration parser；`parse_soap_body/find_response` 處理 SOAP error；
  session profile 選擇、CLI profile view／error。巢狀 Fault 前須檢查
  `SoapError` 分類，不以放寬測試消除不一致。
- 行為分類：profile mutation 是**存在缺口的狀態模型**，不是 acknowledgment-only；
  read 是 state-backed snapshot。D2 不表示全部改成 stub；未建模子功能須明確處理。
- 公開面：`docs/mock-server.md`／`_zh`、client／session rustdoc、library guide、
  受影響 CLI 範例及 CHANGELOG。本盤點不新增公開 API、不選版本、不寫入實機。

## 參考資料審閱

本批參考 [Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§4.1、§5.2.1–5.2.5、§5.2.13–5.2.14、§5.2.22，以及
[Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§4.1、§5.1.1–5.1.5。Fixed-profile 結論已核對：fixed 阻止刪除，不阻止 configuration
binding 變更。已修正相反的舊註解，並新增直接 state assertion。

Media2 參考資料揭露現有 reader 未處理的 selector／初始 configuration／name／list
語意；這些是審查目標，不代表可默默擴充公開 client 方法。

K22：已直接核對 §5.1.2 及固定來源的 Media2 請求宣告。既有完整 profile 查詢方法
省略 `Type`，因此符合規格的裝置會省略 configuration 資料；mock 忽略 selector 而
掩蓋此差異。Client 現在明確傳送 `Type=All`，未新增參數或修改回傳型別；session
轉呼叫同一方法。既有欄位測試現會記錄並斷言 endpoint、完整 Action 與請求選擇，
舊請求在完整全部功能 no-fail-fast 執行中確實觸發該斷言失敗。Mock selector 的有界子批次已實作，
其餘 W10／P-E 欄位及輸出驗證仍待完成；僅驗證 XSD 無法攔截此合法但不符合查詢目的的請求。

亦已直接檢查固定 WSDL 閉包中全部 13 操作的直接輸入序列；詳細欄位筆記保留於
checkout 外的來源根目錄（`profile-contract-review-20260910.md`）。這不代表完整
輸出型別或 Core／共用錯誤審查完成。來源中的容量及 capability 不一致仍待處理：
建立 profile 未遵守公告上限，Media2 公告的 configuration 種類也與現有 binding
實作不一致。

此次局部 DeleteProfile Fault 審查依據 Media1 §5.2.22 與 Media2 §5.1.5：兩個服務
對不存在 profile 均使用 Sender → InvalidArgVal → NoProfile，對固定 profile 均使用
Sender → Action → DeletionOfFixedProfile。這四個分支已改用私有 serializer；client
維持第一層 subcode 語意，reason 文字及錯誤型別不變。Corpus 檢查兩層 subcode、
client／health 分類，以及固定 profile 拒絕前後的序列化 state。K14 通知已另行修正，
內建 replay 現依已提交的 DeleteProfile effect 處理選定 profile read（見管線開工核對）。
其餘 mutation／相依路徑仍待完成；無效請求 Fault 與 virtual-profile 行為不屬於此次局部遷移。

固定來源的外部編譯及 34 份選定 instance 已通過，見
[schema 前置檢查](mock-fidelity-schema-preflight_zh.md)。尚需完整 WSDL／XSD 欄位
驗證、Core／共用及其他操作 Fault 映射，以及 capacity／conflict／extension 規則。
不可僅憑選定 instance 的結果將 C 標記完成。

## 案例與開工條件

已實作有界 W18 read-snapshot 子批次：Media1 GetProfiles／GetProfile 及
Media2 GetProfiles 在同一 read guard 內取得 profile 與 catalogue，保留回應
形狀及 selector 行為。`catalogues_from_state` 借用 DeviceState，不重新取得鎖。
世代標記控制涵蓋三條讀取路徑、有界 writer burst 及 writer 實際前進。此項
修正共用狀態一致性，不代表 schema 驗收或多次獨立請求之間的原子性。

初始單次寫入交接未揭露競爭，故改為每次交接包含 256 次有界寫入及 ready
訊號。加強後的控制在原本分開快照上失敗（`1789046635_cargo_test.log`）。
在共用 helper 重新拆開快照後，三條讀取路徑均偵測到混合世代
（`1789046899_cargo_test.log`：每條 400 次中依序為 295／314／274 次）。
擾動已還原。此為受排程影響的壓力驗證，不是固定重現次數或一般 linearizability
證明。測試會 join writer 並斷言最後的已提交世代，未使用實機。

還原後通過 formatting、兩種 workspace Clippy、1,193 項 all-feature 及
1,108 項 default 測試（各 5 項 ignored、24 suites）、兩種 warnings-as-errors
文件建置及未變動的 inventory self-test（157 routes／159 Action sites／258
直接 reader occurrences）。此純狀態修正不宣稱新增 XML instance 驗收。

已實作有界 P-E 讀取子批次 `media2.GetProfiles` 選擇行為，實作前重新核對
Media2 §5.1.2 及固定 request 宣告。使用共用 parsed operation，保留解碼後
scalar，區分省略及明確提供的 selector；僅投影 cloned profile slot，不改 state。
查詢前檢查重複、scalar 及 sequence。兩種 transport 涵蓋空集合、預設及指定
profile、重複及組合 configuration list、alias、錯置 decoy、完整巢狀
missing-profile fault、整份 state 不變及沒有 hook。既有完整 profile client
仍要求全部 configuration；token-discrimination table 增加兩個 profile 的
raw-selector row。本批次不驗收 profile 輸出／escaping、容量、configuration
conflict、未建模 catalogue storage、欄位長度／attribute 政策或全部 13 張工作卡。

Selector 證據：兩個新 transport 控制在實作前的 `65cd053` 均失敗
（`1789044668_cargo_test.log`）。停用 sequence 順序拒絕使兩者失敗
（`1789044986_cargo_test.log`）；忽略 profile token 則使兩者及新增 token-table
row 失敗（`1789045103_cargo_test.log`）。還原後格式、兩種 workspace Clippy、
all-feature 1,190 項及預設 1,105 項測試（各 5 ignored、23 suites）、兩種
warnings-as-errors 文件及未改變的 159／157／258 清冊與自我測試均通過。
嚴格外部 Xerces corpus 09 的 34 份選定 client instance 通過。舊 shape probe
明確要求 Media2 完整 configuration，保留 158 responses、111 success payloads、
47 faults、1,242 anchors 及全部未改動的零 finding pin。這些 corpus 不涵蓋
全部新增 raw selector 或語意規則。前一 boundary commit `65cd053` 已通過
遠端 CI 34478736427，不代表後續 selector 子批次或 Release 已驗收。

限定的 K16 原子性批次：保留既有 request 擷取、支援 kind 及一般 Fault payload，
將逐筆寫入改為共用、以值表示的 binding plan。在同一 write lock 中檢查 profile
及全部必要 configuration token，通過後才變更 slot。Media1 傳入單筆，Media2
傳入完整且已解析 kind 的列表，不再合成 XML fragment。拒絕時保留完整 state
且不通知；成功的 plan 通知一次，依既有 helper 契約也包含成功的冪等移除。
Repeated kind 暫保留既有依序、最後一筆生效的行為。本 state 批次不裁定規範上
的 repeated-kind conflict、name-only update、Type=All、不支援 kind、嚴格
解析、Fault 階層或 replay。控制須涵蓋後筆未知／空值／錯誤 family token、
有效多 slot 寫入／讀取、fixed profile、移除及每個請求僅通知一次。

K16 驗證：原實作在修正後的完整 state assertion 失敗。抑制成功 plan 通知後，
完整 workspace、all-feature、no-fail-fast 執行中的兩種 transport 控制，均在
確切的 committed-slot observation 失敗；擾動已還原。格式、兩種 workspace
Clippy、全功能 1,179 項及預設 1,095 項測試（各 5 ignored、21 suites）、兩種
warnings-as-errors 文件建置，以及未變動的 157／159／260 清冊均通過。
新外部 corpus 的 34 份 XML 通過固定版本 Xerces 嚴格 XSD 1.1 驗證；完整操作
語意尚未驗收。

前一筆配置 commit `2a488be` 的託管 run 34471659927，僅 Windows CLI 行號
輸出比較失敗，原因是兩個子程序的 `meta.elapsed_ms` 分別為 0 與 9；因此
packaging 被跳過。這不是託管驗收通過。此獨立測試框架問題將另行 commit
修正，不改變 CLI 的耗時契約。

限定的 K13 state 批次：外部已查核的 token 唯一性要求及既有重複拒絕行為，
足以修正配置，無須同時遷移 request parsing 或 Fault 契約。將明確 token 的
重複檢查移入與新增相同的 write lock，自動配置時跳過預載 token 碰撞，僅在
成功建立後通知。持久化的 u32 counter 視為搜尋起點，不保證所有產生 token
的數字皆落在 u32；使用較寬的暫存候選值避免邊界算術溢位。保留序列化欄位
型別，無碰撞時沿用原有 token 拼法。驗證兩服務入口、預載碰撞、counter
邊界、重複請求完整 state 保留，以及明確／自動 token 的併發請求。容量、
名稱解碼／轉義、初始 binding 與 replay 另行處理；本批不關閉 CreateProfile
工作卡。

K13 驗證：原實作在修正後的碰撞 assertion 失敗。停用重複拒絕及擾動 counter
起點後，完整 workspace、all-feature、no-fail-fast 執行中的三項新 helper
控制，均在預期 payload／invariant 檢查失敗；所有擾動已還原。格式、兩種
workspace Clippy 及兩種 warnings-as-errors 文件建置通過。還原後全功能
1,177 項與預設 1,093 項測試通過（各 5 ignored、20 suites）；清冊維持
157 routes／159 Action 位置／260 個直接 reader。這不代表容量或 HTTP／parser
驗收完成。前一個刪除效果 commit `340fc89` 已通過託管 CI run 34470506265；
該託管結果不包含本次後續配置修正。

| 面向 | 既有證據或確切下一項案例 | 狀態 |
| --- | --- | --- |
| C01 | 來源索引及 runtime alias／body／service 不一致拒絕控制已實作；HTTP binding 政策仍屬 W03／W07 | PARTIAL |
| C02 | `delete_profile_preserves_escaped_and_whitespace_identity`、K15 markup 基準；新增 create／get／bind 的 literal name／token round trip | PARTIAL |
| C03 | `delete_profile_rejects_ambiguous_or_mislocated_identity_without_mutation`；擴展至其他 11 列的 namespace／decoy 控制 | PARTIAL |
| C04 | 外部欄位核對後新增 required／empty／duplicate／repeat／extension 案例，保留合法 repeat | TODO |
| C05 | 既有 `mock_token_discrimination`／`mock_media1_media2_agree`；全部 binding 增加 escaped／wrong-family token | PARTIAL |
| C06 | K13 配置、K14 通知及 K16 部分 binding 已有正確回歸；更廣泛的 transaction、conflict 及 callback 尚待完成 | PARTIAL |
| C07 | `known_gap_k15_profile_name_is_interpreted_as_markup`；補完巢狀 renderer escaping、獨立 namespace／shape 檢查 | 已重現缺口 |
| C08 | `unknown_token_fault_preserves_literal_text_and_state`；corpus 檢查不存在／固定 DeleteProfile 巢狀 Fault；其他 mapping／HTTP code 待 W05–W07 | PARTIAL |
| C09 | 兩種 transport 的共用靜態／狀態型 depth／node 限制已有控制；byte limit 涵蓋 in-process，scoped auth 及 HTTP byte 對應待查 | PARTIAL |
| C10 | 模型限制及 K12 修正；`fixed_profile_configuration_remains_mutable_in_both_media_services` 證明 fixed profile 的 Add／Remove 實際改變 state | PARTIAL |
| C11 | 已驗證選定 DeleteProfile client／health first-subcode 控制與 K22 請求選擇；其餘 consumer／CLI 審查待 W06 | PARTIAL |
| C12 | 擾動四個 known-gap assertion 均於 payload／state assertion 失敗；反轉 fixed-binding attachment 預期亦失敗，還原後通過 | PARTIAL |

`tests/mock_fidelity_known_gaps.rs` 的 K15 刻意斷言目前缺陷，用途與既有
Broken／Blind property table 相同。**通過表示已重現，不表示已修復。**
修正時須改成正確 invariant 並更新 finding；不得為恢復綠燈而保留缺陷。

開工條件仍受工程工作限制：W02 間接呼叫閉包、W03／W04 parsed-input 邊界、
W05／W06 Fault 設計、外部逐欄位核對，以及明確原子性／capacity 行為。
目前未發現需要維護者新增產品決策的事項。下一範圍是完成上述設計，再將 profile
read／create／delete 與 binding 遷移拆成獨立驗證 commit。初始來源稽核未改變 handler；
後續選定 Fault 與 K14 通知修正已記錄於上方，不代表廣泛遷移的前置條件已完成。
