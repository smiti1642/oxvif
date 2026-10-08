# 下一版實作計畫

[English](next-version-plan.md) | [繁體中文](next-version-plan_zh.md)

狀態：IN-PROGRESS。程式走訪日期：2026-10-08；已提交基準為
`afe37316ac60aa8c4470f8608a6eab7fa1450692`，workspace 版本為 0.17.0。
候選版本為 0.18。使用者於 2026-10-08 在計畫提交 `2bad316` 後授權實作；版本修改、
攝影機寫入、發布、push 與背景排程仍不在此次授權範圍。

下一版主線是讓實機失敗有界、可解釋，再加入有限的 Metamorph 離線 reference 比較。
先交付 runtime 改善；完整 Mock fidelity 母計畫不列為本版必要前置條件。
各工作包在取得完成條件的證據前，實作狀態維持 TODO。

## 範圍與工作歸屬

| 切片 | 既有負責計畫 | 處置 |
| --- | --- | --- |
| SOAP transport／回應解析 | Library；F15 runtime 使用端、F07 受影響驗證 | 必要 P1／P2；這是新辨識的 client 工作，不表示 W07 request validation 尚未存在 |
| Session 診斷／CLI 執行 | F15 R2／R4、Q08 | 必要 P3／P4；重用已交付的重試分類與 schema 測試 |
| 實機回歸證據 | F15 R5 相容性；必要時 F10 snapshot 證據 | 必要 P5 基礎設施，實機驗收獨立追蹤 |
| 離線 reference 比較 | F14 M7、Q10 | 有限 P6 功能；M4 persona／control transition 分開處理 |
| 現有 HTTP binding 修改 | F05 W07／Q02／HB1 | 保留獨立歸屬與驗收；此計畫不宣稱整合或完成 |
| 相機寫入、播放、散布、navigation 拆分 | F08／F09／F12 | 延後；hostname preview／apply 留待後續產品切片 |

[post-0.17 backlog](post-0.17-backlog_zh.md) 仍是母計畫索引；本文件負責下一版
切點與程式修改地圖，不另複製 F01–F15。
[自主執行佇列](autonomous-completion-schedule_zh.md) 保留原歷史範圍；本次寫計畫
不代表恢復該佇列，也不繼承其 push 政策。

## 程式走訪

路徑相對 repository。行號描述本次走訪基準；後續提交移動行號時，以 symbol 定位。

| 發現 | 實作與既有控制 | 影響 |
| --- | --- | --- |
| C1 內層 timeout 固定 | [transport.rs](../../src/transport.rs) 的 `HttpTransport::new`／`with_root_certificates_pem`／`soap_post`（111–224）；[application.rs](../../crates/oxvif-cli/src/application.rs) 的 `build_http_transport`（1728）未傳入 `ExecutionOptions.timeout` | CLI timeout 調高至十秒以上，不會延長底層 HTTP deadline；加入 options 後，CA 重建 client 不能重設它們。 |
| C2 完整回應先載入 | `soap_post` 在 status 分類前呼叫 `response.text()`；[snapshot.rs](../../src/health/snapshot.rs) 的 `fetch` 對累積 chunk 限制 16 MiB | SOAP 沒有明確 body byte 上限，非成功 body 亦同；snapshot 政策不涵蓋 SOAP。 |
| C3 相容 parser 寬鬆 | [xml.rs](../../src/soap/xml.rs) 的 `XmlNode::parse`／`from_bytes_start`／`parse_soap_body`；[xml_compat.rs](../../tests/xml_compat.rs) 明確保留未閉合／多 root 等行為 | 完整性修改涉及相容性。公開 `XmlNode` 移除 namespace，不能證明 namespace 正確；獨立 Mock request parser 已有 byte／depth／node 上限。 |
| C4 Session 初始化遺失脈絡 | [session.rs](../../src/session.rs) 的 `OnvifSessionBuilder::build`（123）與 `fill_missing_service_urls`（164） | 時鐘同步失敗會中止；optional URL 缺漏觸發 best-effort `GetServices`，其失敗被略過。目前只補 Recording／Search／Replay／Media2／DeviceIO。 |
| C5 共用分類，未共用 delay | [application.rs](../../crates/oxvif-cli/src/application.rs) 的 `is_retryable_onvif_error`／`HealthRetryTransport`／`retry_delay`；[maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs) 的 `onvif_step` 固定等 100 ms | 應重用分類。單一 SOAP、完整 diagnostics、整次 Health 與 maintenance stage 的重試單位不同，共用 helper 必須保留這些差異。 |
| C6 Metadata schema 是封閉契約 | [contract.rs](../../crates/oxvif-cli/src/contract.rs) 的 `SCHEMA_VERSION = "3"`／`ResultMeta`；[envelope schema](../../crates/oxvif-cli/schema/oxvif-envelope.schema.json) 的 `$defs/meta.additionalProperties = false` | 在 `meta` 新增 attempts 不相容於已發布 v3 validator；版本遷移必須明確處理。 |
| C7 Trace 與 capture 是不同出口 | [client/mod.rs](../../src/client/mod.rs) 的 `OnvifClient::call`（200–202）trace 原始 request／response；[capture.rs](../../src/health/capture.rs)／[redact.rs](../../src/redact.rs) | CLI verbosity 的去敏不會保護 Library trace。Health capture 只去除 request Password／Nonce，保留 response／Fault／error 資料，並依 Action 尾段分組；不能當成安全 support bundle。 |
| C8 舊 recorder 會覆蓋身分 | [fixtures.rs](../../src/fixtures.rs) 的 `safe_action_name`／`CapturingTransport`；[record_fixtures.rs](../../examples/record_fixtures.rs) 只讀第一個 profile／source；[fixture.rs](../../src/metamorph/fixture.rs) 保留 collision bucket | 新增多 profile 回歸證據應使用 param-aware Metamorph store。Tracked fixture 目錄只有 README，不表示 checkout 外沒有私人 capture。 |
| C9 結構 diff 不等於數值比較 | [quirk.rs](../../src/metamorph/quirk.rs) 的 `element_paths`／`diff_details`／`QuirkDiff::compute`；[parse.rs](../../src/metamorph/parse.rs) 的 `parse_check`／`ParseReport::all_parsed` | Shape diff 不看值；顯示正規化會遮蔽 token／位址。`all_parsed` 容許 Faulted／Unverified，不能推論 reference 相等或規格符合。 |
| C10 既有 inventory 比較應保留 | [maintenance.rs](../../crates/oxvif-cli/src/maintenance.rs) 的 `compare`（1042）／`diff_value`；[registry.rs](../../crates/oxvif-cli/src/registry.rs) 的 `with_mutation` | `config.diff` 已檢查裝置身分並列 incomparable sections。Registry 已有 atomic replace；crash durability 是獨立 F15 工作，本次不宣稱已證明缺陷。 |

保留的既有控制：transport 200／400／500 與 Digest；Session 時鐘先於 capabilities、
不覆蓋 URL；CLI recovery／cancellation／Fleet 排序及六種 JSON／JSONL envelope；
`tests/mock_replay_key_gaps.rs` 的 collision／full Action；XML Unicode／entity 相容
fixture。原始碼走訪不等於這些測試已在本次重跑。

## 本版設計選擇

1. 保留 `Transport::soap_post -> Result<String, TransportError>` 與 custom transport
   注入；在旁邊新增設定，不新增必要 trait method。
2. 保留 `OnvifSessionBuilder::build()`／`with_clock_sync()` 行為。新增可回報初始化
   過程的入口，不默默把 `auto` clock sync 改為 best-effort 認證 fallback。
3. 必要 runtime 切片維持 stdout schema v3。Attempt 細節先放去敏 stderr，以及
   command schema 容許擴充的 diagnostic data。新增共用 `meta` 欄位需另審 v4
   遷移，不能只放寬 v3 schema。
4. 保留既有 typed parser 的 local-name 相容性。Production SOAP 改走完整、有界
   解析；直接 `XmlNode::parse` 保留已記錄的寬鬆行為。P2 不全面遷移 namespace。
5. 保留 fixture storage／replay key 格式。Comparator 依 full Action 加可證明的
   request identity 配對，保留歧義。
6. Synthetic schema instance、Mock capacity smoke、圖片 signature 或同一 parser
   比較，都不當作實機 conformance。

## 施工順序

| 工作包 | 前置條件 | 交付 | 狀態 |
| --- | --- | --- | --- |
| P0 | 無 | 固定實作輸入與相容案例 | DONE |
| P1 | P0 | 可設定 HTTP deadline／有界 SOAP body | DONE（本地）；cap 驗收待 P5 |
| P2 | P0；整合網路檢查需 P1 | Production XML 完整有界解析 | DONE（本地） |
| P3 | P1／P2 | Session 初始化 report／安全 trace | DONE（本地） |
| P4 | P1／P3 | 共用 delay policy／CLI 診斷整合 | TODO |
| P5 | P0；驗收需 P1–P4 | Param-aware 回歸 harness／實機證據 | TODO |
| P6a | P2／P5 synthetic corpus | 離線配對／明確 comparison outcome | TODO |
| P6b | P6a | 選定 typed projection／可用的比較 example | TODO |

P5 harness 可與 P1／P2 同時準備，不需要攝影機。缺少硬體輸入只阻擋對應相容性
宣告，不阻擋本地程式、測試或本文件完成。P6 是獨立功能切點；不以它尚未完成為由
無限延後已完成 runtime 修正。

## P0 實作基準

實作輸入為 `2bad316`，原有 HB1 tracked diff 與四個 untracked 檔案獨立保留。
已走訪 Library／Health transport 建構、CLI `build_http_transport`／typed retry
分類、Health capture 的 exhaustive `TransportError` match、recording／examples，
以及相鄰 oxdm Session custom transport（未發現 exhaustive error match）；不修改
oxdm。公開 error 新增 variant 需要下游 exhaustive-match 遷移；v3 封閉 metadata
schema 與直接 XML 寬鬆相容測試維持既有契約。

- 寫程式前記錄選定 commit 與 dirty worktree 處置。本次 HB1 修改 CHANGELOG、
  公開 Mock 文件、lib／mock 檔，並新增 `http_binding.rs` 與測試；仍未提交且為
  IN-PROGRESS。不把它們納入計畫提交，也不宣稱 HB1 完成。
- 區分已發布、已提交但未發布、本地未提交行為。實際版本決策前，manifest 維持 0.17.0。
- 新增 error／report 前列出 fixture／CLI 相容案例、enum match、schema consumer
  及下游編譯入口。

完成條件：明確實作 revision、保留無關修改，以及涵蓋 Library／CLI／Health／
record-replay／examples 的 consumer map。

## P1 HTTP options 與回應上限

入口：`HttpTransport` constructor／`soap_post`、CLI `build_http_transport`，以及
Health 預設 transport 建構。

- 所有 reqwest client 建構共用一份 options。預設 timeout 保留十秒，CLI 傳入
  per-attempt timeout。Private CA／credentials 行為不受 builder 順序影響；CLI
  外層 timeout 仍限制含 Digest challenge 的完整 attempt。
- SOAP 初始上限提議為累積 4 MiB；發布前由 P5 capture 核對，這是設計值，並非量到的
  裝置最大值。Library 可選正值有界上限，第一切片不必增加 CLI knob；錯誤 options
  在送出前拒絕。
- 逐 chunk 以防 overflow 的計數累積。Content-Length 可提前拒絕，但不能取代
  實際 byte counter；success／Fault／非預期 HTTP body 同樣有界。文件定義計數的
  bytes 表示及 UTF-8 行為。
- Size failure 使用型別化、不可重試的錯誤，不虛構 HTTP 413／503。公開
  `TransportError` 目前可 exhaustive match；新 variant 需 minor-version migration
  note，並更新 CLI／Health classifier。
- 保留 200／400／500 SOAP Fault body 行為。本工作包不順帶重設 redirect／proxy
  政策；改變前另行盤點。

驗收：exact-cap 成功／cap+1 拒絕；沒有或不實 Content-Length；chunked／非成功
body 超量；取消釋放連線；401 challenge 後 capped response；private CA 加非預設
timeout；builder 順序保留設定；新 error 不含 raw body。檢查精確 error 與 observed
request count。長 deadline 用 paused time／注入時鐘驗證，不新增每次等待十秒以上
的常設測試。

## P2 Production XML 邊界

入口：`XmlNode::parse`／`parse_soap_body`、SOAP error mapping，以及 client／Health／
Metamorph 對 SOAP helper 的使用端。

- 新增 production SOAP 的 complete-document parser path，讀到 EOF，拒絕未閉合、
  第二個 root、尾隨非空白、錯誤／重複 attribute 及禁止的 DTD。保留既有相容案例所需
  的合法 Unicode／entity／CDATA 行為。
- 初始限制提議：input 4 MiB、depth 64、65,536 element nodes。Start／Empty 均
  計入，attribute／text 計入 byte budget；測 exact limit／one-over。這是 client
  response 設定，與 Mock request 的 2 MiB／64／16,384 分開。
- 直接 `XmlNode::parse` 保留 legacy semantics 並說明限制；`parse_soap_body` 改走
  完整有界入口，使 custom transport 也不能繞過 production parse 上限。保留
  `XmlNode` 公開欄位與 local-name typed parsing；新 `SoapError` variant 需遷移。
- 同時檢查 recursive consumer 與深樹 drop；用 iterative loop 建樹不代表 stack／
  memory 安全已成立。這些是 workload 限制，不是實測總記憶體保證。

驗收：production helper 拒絕 legacy direct parser 保留的各種 malformed 案例；
direct XML compatibility 測試仍通過。網路與 injected transport 對超大／超深 SOAP
有同樣拒絕；驗證精確 syntax／resource 類別、token／text 值及後續合法呼叫恢復。
Namespace conformance 不在此工作包。

## P3 Session report 與安全診斷

入口：`OnvifSessionBuilder::build`／`fill_missing_service_urls`／`OnvifClient::call`、
Health 初始化及 CLI maintenance `connect`。

- 新增帶 report 的 build 入口，與 `build` 共用實作。Optional `GetServices` 失敗可
  隨成功結果帶 warning；mandatory stage 失敗同時返回原 typed cause 與 report。
  不對現有可 literal 建構的 public struct 新增必要欄位。
- Report 列 `clock_read`／`capabilities`／`services_fallback` outcome、耗時、可用的
  clock offset 與各 service URL 的來源。Safe projection 只用 service label／受控
  error class；例行 verbosity 不序列化 raw reqwest error、URL query／userinfo、
  SOAP body 或 Fault reason。內部保留 cause 供 caller 使用。
- 保留目前 URL 優先序與缺少服務行為；不混入猜測 hostname 替換、自動探測 endpoint
  或增加 routing family。
- 無條件 raw SOAP trace 改成 metadata-only：受控 operation identity、耗時、byte
  count／error class。Wire 調查沿用 explicit capture API；targeted redaction 不等於
  一般 secret／PII sanitizer。更新 recorder 過度保證安全的文件。
- 現有 String transport result 無法提供 success 的實際 HTTP 200／400／500。
  Session report 標示 unknown；telemetry wrapper 數的是 SOAP-call attempt，不一定
  等於 Digest HTTP exchange 次數。

驗收：正／負 clock offset、clock-read 失敗且未發 capability、capability refusal、
fallback 成功／失敗、原 URL 不覆蓋、不重複初始化 traffic、custom transport builder
順序。Trace／CLI 測試放入 credentials／userinfo／query／credential-shaped Fault，
全部不得出現在 safe output。

## P4 CLI 重試與診斷整合

入口：`retry_delay`、diagnostic／Health／enrichment execution、maintenance
`onvif_step`、`ExecutionOptions`／`ResultMeta`、output／describe／Agent guide 與 schemas。

- 抽小型 internal retry policy 給現有 call site 使用。保留分類與 attempt 上限，將
  maintenance 固定 delay 改成相同 bounded backoff；不建立 generic framework／新 crate。
- 保留 retry unit：selected diagnostic、整次 Health、單筆 enrichment read、單一
  maintenance stage。成功 discovery scan 不重跑；未來相機寫入不自動重試。
- `-v` 保持簡潔去敏 stderr；`-vv` 增加 service-resolution outcome、已執行 attempt、
  retry class／delay。名稱明確使用 SOAP／operation attempt，不稱為 HTTP request
  次數。每 operation 的診斷 entry 有界；Fleet worker 完成後依裝置固定排序。
- 保留 schema v3、exit code、JSON／JSONL framing。Diagnostic／workflow payload
  增欄位需驗證 command schema；此切片不加共用 `ResultMeta` 欄位。若需 v4，schema／
  Agent guide／descriptor／migration example／六種 envelope 一次遷移。

驗收：503 後成功有兩次 attempt；401／403／SOAP Fault／resource error 一次；backoff
取消；per-stage 與 whole-run 次數；Fleet worker 上限與排序；table／JSON／JSONL
在 quiet／normal／`-v`／`-vv` 下的結果；v3 success／error validation 與 stdout 純淨。
沿用已有 recovery／cancellation suite，不只檢查新增測試。

## P5 實機回歸 harness 與驗收

入口：`record_surface_with_progress`／`SurfaceSelection`／`FixtureStore::record`／
`lookup_request`／`verify_parsing`、conformance examples、`tests/fixtures/README.md`，
以及 CI 一般 integration-test job。

- 在既有 Metamorph recording 上建立 param-aware harness；多 profile corpus 不使用
  suffix 檔名的 last-write-wins recorder。走訪所有選定 profile／source，synthetic
  harness 至少有兩個 token，答案刻意不同且 assertion 讀得到。
- 每組 corpus 附 manifest：去識別裝置 ID、可分享 vendor／model／firmware、錄製
  commit、full Action／request identity、每 operation sweep outcome、expected typed
  field value、來源與分享範圍。Transport failure outcome 另存；FixtureStore 不保存
  transport 失敗回應。
- 不以 partial／看似相等作成功：Unverified／Faulted／Failed／Parsed 分開，
  `all_parsed()` 不足。驗證指定已知值，不只 parser success／list count。
- Raw capture／官方 schema 資源留在 checkout 外。只提交獨立審閱、允許分享的
  sanitized fixture／manifest；去除 secret 後仍審 username、serial／MAC、hostname、
  query token、vendor extension／log。不把 schema-derived instance／index 放進 repo。
- 修正 recorder 文件矛盾：目前 capture 已去除選定憑證形式，fixture README 卻寫
  不自動去敏；兩者都不能保證任意裝置資料可安全公開。

本地完成條件：synthetic multi-source corpus、保留 identity 的 runner、精確 expected
value／negative control 納入普通 CI；明確選定 corpus run 缺少檔案要失敗，不靜默 skip。

實機驗收目標：兩個 vendor、三組 device／firmware capture，其中一組多 source。
按 Action／firmware 記錄實際結果；這只是初期相容性目標，不等於商用五廠牌宣告。
缺少輸入時 harness 可 DONE，硬體驗收維持 WAITING FOR EVIDENCE。Hanwha snapshot
F10 仍需自己的失敗 body／metadata，不能從 synthetic CR／LF 圖片推論根因。

## P6 離線 reference 比較

入口：`FixtureStore`／`lookup_request`／private `recording_equivalent`／`parse_check`／
`QuirkReport` 與公開 Metamorph exports。

### P6a 配對與結果

- 新增獨立 comparison API／module；不重定義 structural quirk diff、
  `ParseReport::all_parsed`、CLI `config.diff` 或 fixture disk key。
- 依 exact full Action／可證明 request identity 配對。Canonical key 只當 bucket
  提示；重複 candidate 保留 Ambiguous，不任意依序 zip、join Action 尾段或合併 Media1／2。
- Outcome 提議：Equal／Different／MissingLeft／MissingRight／Ambiguous／Unparseable／
  Unsupported／DeviceFault。Report 列雙邊 fixture ordinal、受控 reason、comparison
  projection、explicit mask／field path；預設 summary 不放 raw XML／任意 Fault text。
- Request identity 保留有語意的 profile／config／source token。既有 malformed
  request exact-raw replay 不代表它有效或可語意比較；無法證明身分時回 Unsupported。
- File example 曝光前先限制檔案載入、pair count、change output／retained bytes。
  現有 `FixtureStore::load` 完整載 JSON。超量要明確拒絕，不能截斷後回 Equal。

### P6b 選定 projection

- 第一批支援 Device `GetDeviceInformation`／`GetHostname`，Media1／Media2 的
  `GetProfiles`／`GetVideoEncoderConfigurations`：六條 exact Action，使用現有 typed
  parser。Options／PTZ／Events／Recording 先回 Unsupported，後續各自映射。
  經 private 介面重用 `parse_check`，不複製 routing 或改公開 verdict 欄位。
- Local-name parser 前先檢查完整有界 XML／namespace-aware response identity。
  用 bounded `NsReader` 建 response projection，不把 Mock request validator 當作
  response conformance oracle。比較選定欄位 expanded name；未解析 QName-valued data
  維持 Unsupported。
- Equal 只表示宣告的 selected projection 相同，不表示全部 optional／vendor field
  或全部 protocol 規則相同。明列未檢查 extension／field，不計為 fully verified。
  相關 namespace 錯誤／改變不能因 local name／typed JSON 相同而回 Equal。
- 六條 Action 的預設 volatile mask 為空。不重用 `Masking::Value`／
  `normalize_instance_values`；token／firmware／位址／identity 改變要可見，除非 caller
  explicit field mask 排除，且 report 記錄每個 mask。
- Repeated profile／configuration 依 unique semantic token 配對；重複或缺少 token
  回 Ambiguous／Unparseable。不重排語意未知的任意 nested array，不把未知值變成空值。
- 提供 deterministic JSON、progress／exit 說明的離線 example；不要求新增 CLI
  parser path、MCP server 或 oxdm UI。下游後續可直接使用 report。

驗收：同結構／不同 hostname 或 bitrate；firmware 改變；token 改變；同 token／不同
namespace；只換 namespace alias；unique token list reorder；duplicate token；單邊
缺漏 Action；兩個 request 共用 legacy key；malformed response／request；unknown
Action；DeviceFault；未檢查 extension；只套 explicit mask；size／output 超量拒絕；
相同 input 重跑 summary 完全一致。同 parser JSON 比較、synthetic Mock matching
都與獨立 conformance verdict 分開。

## 相容性與發布驗證

| 契約 | 必要處理 |
| --- | --- |
| Public error enum | 新 variant 查 downstream exhaustive match、精確 error test／migration；resource refusal 不假裝 HTTP status |
| Public struct／builder | 保留 literal construction／old method；new report／options 用新 type 或 private field |
| CLI stdout | 保留 v3 common envelope；v4 若提出，另作明確遷移 |
| XML 行為 | Direct DOM 保留寬鬆行為；文件說明 production SOAP 完整性／resource 新限制 |
| Fixture storage | 保留 legacy collision／replay；comparison report 可有自己的 version |
| Snapshot／registry | 保留 destination／auth／TLS／no-overwrite／inventory identity；不加自動清理或宣稱 durability |

每個實作批次在行為存在後才更新 public docs／examples／CHANGELOG；雙語狀態同步。
使用 RTK 跑常設 repository gate，不把 ignored schema／native test 計入驗收：

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --all-features --locked --no-fail-fast
rtk cargo test --workspace --locked --no-fail-fast
```

Public API 改變後跑 feature／default strict rustdoc；再跑受影響 individual feature／
MSRV、既有 schema／CLI matrix，wire shape 改變時跑獨立 qualified external schema。
新 assertion 用 targeted fault／limit／identity 擾動證明敏感度並精確還原；保留每批
施工時使用者的無關修改。

版本驗收需 P1–P4 本地證據、明確 P5 實機處置；若交付 P6，需 P6a／P6b 證據。
本切片不宣告全面商用／VMS 或所有 operation conformance。本次寫計畫不含發布、
push 或修改版本。

## 實作證據

- P3：`build` 委派給 `build_with_report`，初始化順序、必要 clock／capabilities
  失敗與 missing-URL fallback 優先順序維持不變。Report 僅含受控 enum／number：
  三個 stage、可取得的 clock offset、已知 HTTP error status、十個 routing URL
  來源（不含 URL）。Boxed 必要失敗獨立保留原始 typed cause。
  `HealthCheck::run_with_session_report` 回傳原 HealthReport 加初始化 metadata；
  `run` 保留原有投影。
- Client trace 保留固定 operation Action、serialized request／returned text byte
  counts、耗時與受控 transport error class；收到 String 不代表 SOAP 成功或已知
  HTTP status。Trace 測試獨立執行，避免平行 client test 競爭 tracing callsite
  註冊。Recorder 文件區分選定去敏與安全分享保證。
- P3 驗收：八個新測試涵蓋既有流量、credentials／transport 建構順序、正／負／
  未知時差、必要失敗立即停止、fallback 優先順序與結果、原始 Fault cause、獨立
  Health metadata 與 trace 去敏。覆蓋 capability URL 在未篩選 all-feature run
  使兩個測試失敗；focused controls 破壞時差／未知值、authentication／Fault 分類、
  skipped stage 與 raw URL trace 去敏。原始碼逐 byte 還原。最終五項 gate 通過：
  all-feature／default 各 1,406／1,287 tests，七個 ignored、50 suites。兩種
  workspace 與無 feature Library 的嚴格 rustdoc，以及無 feature／health-only
  Library Clippy 皆通過。
- 實機 capture 輸入：使用者於 2026-10-08 確認目前沒有。繼續所有本地工作包；
  P5 裝置驗收維持 WAITING FOR EVIDENCE。

- P2：完整檢查先於有界 DOM 建構，預設 4 MiB input、包含 root 的 depth 64，
  65,536 Start／Empty elements。直接 DOM 相容入口獨立保留。Attribute key set
  避免驗證與已驗證 DOM pass 的 duplicate check 平方成長。檢查 XML 1.0 UTF-8
  declaration、names／characters／references 與 attribute 語法，不宣稱 namespace
  或 XSD conformance。既有 quick-xml 語法錯誤細節仍供呼叫端使用；resource error
  不保留 XML。Health 對應既有 Parse class；CLI 不重試；action outcome 的
  exhaustive test helper 已加入新 variant。
- P2 驗收：六個獨立語法／payload／resource／network／recovery／wide-attribute
  測試，以及未修改的直接 XML 相容 fixture。深度放寬一層，在未篩選 all-feature
  執行中使三個測試失敗；略過驗證使五個 focused test 失敗；改動 decoded attribute
  使 Unicode／wide-attribute 值斷言失敗。原始碼皆逐 byte 復原。最終 format、兩種
  Clippy、all-feature／default tests（1,398／1,279 通過；各七個 ignored、48 suites）
  與嚴格 rustdoc 通過。

- P1：`HttpTransportOptions` 控制每個 HTTP request deadline 與累積回應 bytes，
  預設為 10 s／4 MiB。CLI 轉送與 Health 內外層 timeout 的競態處理保留原有 timeout
  結果；成功、Fault 與 HTTP error body 都在保留前檢查上限。明確採用 UTF-8
  replacement decoding，不受下游 reqwest charset feature unification 影響。
  Digest helper 丟棄 challenge body；byte cap 適用於最後回傳的 response body。
- TCP fixture 驗證邊界 payload、超一 byte、宣告／未知／chunked 長度、無效 UTF-8、
  取消與連線 EOF。Private root 重建保留 options／roots；先設 Digest credentials
  再改 options 仍保留認證，精確發出一次 challenge 與一次 authenticated request。
  CLI 轉送測試未套用外層 timeout。
- 反向驗證：將 byte cap 放寬一個 byte，在未篩選 all-feature／no-fail-fast 執行中
  使三個超量拒絕測試失敗；該次亦暴露上述 Health timeout 競態，現已修正。將 CLI
  內層改回固定十秒，使 focused deadline assertion 失敗；兩個檔案都逐 byte 復原。
- 尚未量測實機最大回應；4 MiB 預設值是否適合發布，仍需 P5 硬體驗收。
- P1 最終 gate：format、兩種 workspace／all-target Clippy、locked 未篩選
  all-feature tests（1,392 通過）與 default tests（1,273 通過），兩者各七個 ignored、
  47 suites；兩種 feature mode 的嚴格 rustdoc。HB1 仍獨立保留。

## 原始計畫驗證

- 已完成：上述 source／control 走訪與工作歸屬核對。
- 目前 worktree 通過：240 個 local file-link destination、雙語工作包存在檢查、
  source spot check、`git diff --check`／format check；兩種 workspace／all-target
  Clippy；all-feature 測試 1,385 passed／七項 ignored／47 suites；default 測試
  1,266 passed／七項 ignored／47 suites。測試皆 locked 且使用 `--no-fail-fast`。
- 以上檢查包含先前未提交的 HB1 輸入，不代表 HB1 結案或 P0–P6 已實作。
  原 tracked diff 與四個既有 untracked HB1 檔案已核對未改變。
- 本次走訪未執行：實機、external schema、原生 terminal、package install、發布，
  或未來實作的驗收。
