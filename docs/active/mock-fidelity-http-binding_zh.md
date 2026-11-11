# HTTP binding HB1

[English](mock-fidelity-http-binding.md) | [繁體中文](mock-fidelity-http-binding_zh.md)

本地驗收完成，以 `b0be9ec` 提交。2026-09-23 在實作前記錄 W01，基準 `afe3731`。
2026-11-10 以 `d777925` 加上未提交的 HB1 worktree 審查。
缺口盤點及初次驗收時，HB1 維持未提交。
使用者後續已授權修正；下方本地實作與驗收章節取代先前的缺口處置。
負責範圍：Q02 / W07；159 條既有路由共用的私有 HTTP 配接層。

| 章節 | 用途 |
| --- | --- |
| [契約](#契約) | 原始碼觀察與目標政策 |
| [驗收](#驗收) | 適用審查軸與交付 |
| [缺口審查](#缺口審查2026-11-10) | 已重現缺陷、缺少的控制與下一步 |
| [本地實作與驗收](#本地實作與驗收2026-11-10) | 修正、敏感度與最終候選證據 |

## 契約

`afe3731` 的原始路徑：MockServer::handle_soap → helpers::extract_action → Chain::respond_at。
HttpTransport 接受 200/400/500 的 SOAP 內容。目前 Content-Type 以分號切割，
只識別小寫 action，管線回應全部使用 HTTP 200。Request::parse 保留 QName 範圍，
可用於回應判讀。不改服務內容、Action 登錄、公開 RequestCtx 或程序內 API。
未提交候選已改用 `http_binding::action` 解析，再由 `http_binding::response_status`
判讀回應；前段描述的是歷史起點，並非候選版本目前的行為。

參考：[W3C SOAP 1.2 Part 2 第二版](https://www.w3.org/TR/soap12-part2/)
§6.5.3、§7.5.2。審閱結論：action 是可省略的非空絕對 URI；Sender 對應 400，
其餘標準 envelope Fault code 對應 500。不支援封裝使用 415，HTTP 語法錯誤
在 SOAP 處理之前使用 400。

目標 Mock 政策：唯一 Content-Type、最多 8192 bytes、application/soap+xml；
媒體類型與參數名不分大小寫，可省略 UTF-8 charset；處理引號字串與跳脫，保留
action 大小寫，拒絕重複參數與錯誤值。不支援媒體／字元集／內容編碼回 415；
重複、錯誤或超長 header 回 400。Content-Encoding 可省略或僅有一個 identity。
忽略語法正確的未知參數。Header 拒絕回固定純文字、不回顯輸入，在 auth 與
故障消耗之前執行；既有 UTF-8 SOAP 內容拒絕政策維持原狀。

未提供 action 時保持缺省，由既有管線決定路由結果，不新增 Body 或 WS-Addressing
推導。SOAPAction 與既有行為相同，予以忽略。提供的 action 必須有 ASCII URI
scheme 與非空 ASCII 剩餘部分、合法百分比編碼，不接受 fragment、空白、控制字元
及必須編碼的 URI 字元。完整操作 URI 與 Body 一致性仍由既有政策處理。

僅判讀唯一直接 SOAP 1.2 Fault 的直接 Code/Value，依範圍解析 QName。
SOAP Sender 回 400，其餘已知 envelope code 回 500。舊有非標準 code、刻意的
錯誤 raw XML、非 Fault 回應維持 200；不改寫原始回應 bytes。保留 SOAP 回應
Content-Type 以相容客戶端。本範圍不宣稱完整 HTTP/SOAP/WS-Addressing 相容或端點路由。

## 驗收

C01/C03/C04：action 大小寫／URI 語法、重複 header／參數、引號內分號與跳脫、
Fault QName 替代／重綁 prefix 與巢狀誘餌。C02/C07/C08：固定 header 錯誤、
SOAP 內容不變與狀態碼。C06/C09：拒絕保留已排入故障、auth nonce cache、裝置 hooks；
header 長度限制；auth／服務 Fault 保留客戶端錯誤分類。C10/C11：記錄可省略 action、
不推導、legacy/raw 例外，以及與程序內 SOAP 內容一致。C05 不適用：不改 token 選取。
C12：HTTP／解析器測試、一次完整未篩選 mutation campaign 與精確還原、五項 workspace
gate、嚴格文件、inventory 與雙語連結檢查。HB1 不產生新 XML，既有外部 corpus
證據不變。更廣端點／WS-Addressing 路由、角色授權與結構化注入仍屬其他 Q02 工作。
交付前更新公開 server 文件、changelog、雙語 queue／驗收登錄，再封存此限定卡片。

## 缺口審查：2026-11-10

目前已有配接層、四個 parser/status 單元測試與三個實際 HTTP 測試。HB1 尚不能
驗收：已重現兩個實作缺陷，C12 敏感度與交付紀錄也不完整。本次只做審查，
未修改 runtime source，亦未 commit 或 push HB1。

### 已重現的實作缺陷

| ID | 來源與影響面 | 重現與影響 | 所需修改與驗收 |
| --- | --- | --- | --- |
| HB1-R01 | `src/mock/http_binding.rs::absolute_action`；進入 responder chain 前的所有 SOAP HTTP 請求 | `urn:bad[part]/GetDeviceInformation`、`http://[not-an-ip]/GetDeviceInformation`、`http://example.test:abc/GetDeviceInformation` 都通過字元白名單。若有對應的 Receiver 故障，每例都回 SOAP/500 並消耗故障，而非固定純文字 HTTP/400。 | 依 RFC 3986 驗證 component 結構，涵蓋方括號 host、port、path/query 字元；保留原始 action，不做 URL 正規化或網路查詢。錯誤案例須回精確純文字/400、保留故障與 nonce、不改 device/hooks；正向控制需含 opaque URI、IPv6 host、百分比編碼與引號內分號。 |
| HB1-R02 | `src/mock/http_binding.rs::response_status` 使用 `Request::parse`；HTTP replay/raw 回應狀態 | Sender envelope 若含 attribute 內未跳脫的 `<`、名為 `1bad` 的 attribute，或名為 `1bad` 的子元素，HTTP 都回 400。Bytes 保持不變，但文件承諾錯誤 raw XML 維持 200。共用 request parser 未驗證這些 XML lexical 錯誤。 | 對 scoped Fault 判讀前先確認有界的 XML lexical 有效性，保留 raw bytes 與 200 fallback。適當共用經審閱的驗證邊界，避免改動既有 in-process 解析或無必要地建立第二棵 DOM。涵蓋上述錯誤、對應的合法 Sender/Receiver，以及明確的 bytes/depth/nodes 上限 fallback。 |

參考結論：[SOAP Part 2 §6.5.3](https://www.w3.org/TR/soap12-part2/)
要求非空 absolute action URI；[RFC 3986 §3.2–3.4、§4.3](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2)
定義 component grammar，不能只檢查整串字元。方括號只用於 IP-literal host，port
使用數字。目前額外要求 scheme 後非空 remainder，應持續明載為 Mock 本地政策，
不能從 RFC 的「URI 非空」要求直接推導。SOAP Part 2 §7.5.2 Table 20 支持既有
Sender/400 與其他標準 code/500 對應，但不會讓錯誤 raw fixture 變成合法 Fault。

獨立本地 HTTP 探針已重現：三個錯誤 action 都消耗已排入故障；三個錯誤 raw XML
保留 bytes 卻回 400；合法 raw Sender 對照組回 400。兩個探針測試是藉由確認缺陷
而通過，並非正確性驗收。暫存程式與紀錄位於 ignored 的
`target/hb1_review_probe.rs`、`target/hb1-review-probe.log`，臨時 integration test
已移除；修正時應改為正常的永久 regression assertion。

### 尚缺的驗收工作

| 審查軸 | 已有證據 | 本切片仍需補齊 |
| --- | --- | --- |
| C01/C04/C09 headers | 單元測試含媒體/參數大小寫、重複、跳脫、錯誤值、content coding 與 8192-byte inclusive 邊界；HTTP 覆蓋部分 400/415。 | R01 結構案例；HTTP 實送 8192/8193 bytes、重複 Content-Encoding、錯誤引號跳脫，精確驗 status/type/body。記錄 framework 在 adapter 前拒絕 body/header 的優先順序。 |
| C03/C08 scoped status | 單元控制含標準 code、default/alternate/rebound prefix、重複 Value、nested decoy；注入故障有 HTTP/in-process bytes 比較。 | R02 lexical 與 raw replay HTTP 案例；重複 Code/Fault/Body、錯誤 namespace 誘餌與資源上限 fallback。直接比 bytes，不只搜尋 marker。 |
| C06/C09 拒絕效果 | 現有 HTTP 測試驗部分 header 拒絕保留故障與認證 nonce，並比唯讀裝置 snapshot、零 hooks。 | 加入真正會寫入狀態的 operation 與 replay-backed read：拒絕 header 必須保留 state、hooks、replay 可見性。只呼叫唯讀 GetDeviceInformation 無法證明寫入拒絕的原子性。 |
| C11 使用端 | 注入標準/legacy 故障經 HttpTransport 保留 `SoapError::Fault` 欄位；另有 body/action mismatch 的 HTTP assertion。 | 實際認證與有模型的服務拒絕，在新 status 下維持 typed client 分類；驗受影響的 Session/CLI error 與 exit code，可重用足以證明此邊界的既有 suite。 |
| C12 敏感度與 gates | 本次 focused tests 與 inventory 通過，詳見下方。 | 一次事先規劃的 mutation batch：未篩選 `--workspace --all-features --locked --no-fail-fast`，記錄預期失敗名稱並精確還原；還原後跑五項 workspace gates 與兩種 feature mode 的嚴格 rustdoc。先前 push gates 不含 HB1。 |
| C10/C11 交付 | 公開 MockServer/lib 文件、server guides、changelog 已描述預期的有限政策。 | 修正後核對宣稱；更新雙語 Q02/remaining acceptance/W07/K06 並連結 HB1 證據。更廣 endpoint/WS-Addressing、roles、structured injection 維持未完成。驗收及取得交付授權後才封存此卡。 |

本次實際執行：

- `cargo test --features mock-server --test mock_http_binding --locked`：三個通過。
- `cargo test --features mock-server --lib mock::http_binding::tests --locked`：
  四個通過；這是篩選結果，不是 workspace gate。
- 兩個臨時 `metamorph-server` HTTP 探針皆重現上述缺陷；未擾動 production source。
- Inventory 含與不含 `-SelfTest` 皆通過：159 routes、161 個 Action declaration
  sites、162 個 reader occurrences 與雙語登錄一致，正/反向 self-test 通過。
  這是 source 一致性，並非協定驗收。
- Format 與 `git diff --check` 通過；更新後另檢查本卡雙語連結。本次不宣稱完成
  full mutation、最終五項 gates、strict rustdoc、external schema/camera 或 native-store 驗收。

下一步順序：修正 R01/R02 並加永久回歸 → 補 state/replay、typed consumer 控制 →
一次敏感度 campaign 與最終 gates → 雙語交付登錄對帳。使用者改變目前指示前，
所有工作都維持未提交。兩個缺陷均不需實機或外部 capture 才能重現或修復。

## 本地實作與驗收：2026-11-10

處置：R01、R02 已實作；此範圍的本地驗收 PASSED。HB1 交付狀態維持 IN-PROGRESS，
依使用者明確指示保持未提交。

- R01：`absolute_action` 分別判讀 hierarchy/query、authority、user information、
  host、port，逐 component 套用 RFC 3986 字元類別。方括號 host 接受 IPv6 或
  IPvFuture grammar；port 僅接受數字（含 RFC 允許的空 port）。保留原始 Action，
  不做 URL 正規化、查詢、依賴升級，也不改公開 signature。
- R02：`response_status` 經 crate 私有 re-export 共用 production lexical validator，
  再建立 scoped request tree；不增加第二棵 DOM，不改既有 in-process parser。
  錯誤 raw XML 維持 200 及原始 bytes；超過 2 MiB、含 root 的深度 64 或 16,384
  元素時也 fallback 200。精確上限對照組仍將 Sender 映射 400；此為有界判讀，
  並非完整 Fault schema 驗證。
- Header 測試包含三個已重現的錯誤 URI、錯誤引號跳脫、重複 Content-Encoding、
  HTTP 實送 8192/8193-byte Content-Type。Framework body buffering 優先順序亦
  明載並測試：body 超過 2 MiB 時先回 413，即使媒體類型不支援。
- 認證唯讀控制驗證拒絕保留故障、nonce、state、hooks；replay-backed SetHostname
  另以合法寫入 Action 搭配錯誤 representation headers，先檢查 state/hooks/replay
  再比拒絕形狀，最後確認合法寫入只 commit 一次，錄製讀取切換為 live hostname。
- Raw replay 對合法 Sender/Receiver 及三個錯誤 XML 精確比對 bytes/status；單元
  控制補入重複 Code/Fault/Body、錯誤 namespace 誘餌與 bytes/depth/nodes inclusive 上限。
- 實際 auth/service 拒絕維持完整 typed Fault 欄位；Session failure report 維持
  `DeviceFault` 與未知 HTTP status，符合 String transport 契約。既有 CLI 測試
  `ordinary_mock_auth_fault_preserves_human_and_agent_error_contracts` 在 workspace
  gate 驗 human/JSON 分類及 exit 20，此相同邊界不另複製 CLI 測試。

敏感度證據：

1. 一次 batch 放寬 authority/path、忽略 lexical 與 scoped QName 檢查、將 header
   上限提高一個 byte。未篩選 locked workspace/all-feature/no-fail-fast 執行完
   52 suites，六個測試在 assertion 失敗：
   `malformed_ambiguous_and_unsupported_headers_refuse`、
   `status_uses_scoped_direct_soap_fault_code_only`、
   `http_header_length_and_encoding_boundaries_are_exact`、
   `rejected_write_headers_preserve_device_hooks_and_replay`、
   `header_refusals_preserve_fault_nonce_device_and_hooks`、
   `raw_replay_status_requires_valid_xml_and_preserves_exact_bytes`。
   `http_binding.rs` 已逐 byte 還原；campaign 前 SHA-256 為
   `50fb982246daac15b02e17b6fca4bc714c3fc124215e51068bf796e3709d3d0b`。
2. Campaign 發現長度單元 fixture 跟著實作常數計算，雖然獨立 HTTP 測試有抓到
   變異，單元測試本身仍偏弱。現已直接鎖定 8192，focused 8192→8193 擾動確認
   在 assertion 失敗，並精確還原。
3. 寫入 fixture 已補強：錯誤 media/charset/duplicate 仍帶合法 Action。Focused
   media-gate bypass 重現真實 hostname 修改，在 status 檢查前由 state assertion
   抓到；來源精確還原。最終 gates 在這些測試改善後執行。

Mutation 紀錄及還原摘要位於 ignored `target/`：`hb1-mutation.log`、
`hb1-mutation-summary.json`、`hb1-limit-sensitivity.log`、`hb1-write-sensitivity.log`。
這些是本地敏感度證據，不是外部設備或 native-store 驗收。Adapter 未產生新的 XML
payload，既有外部 corpus 證據不變。

最終候選驗證：精確還原與測試補強後全部 PASSED。

| 檢查 | 結果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通過 |
| Workspace/all-target Clippy，all-feature/default、locked、`-D warnings` | 兩種皆通過 |
| `cargo test --workspace --all-features --locked --no-fail-fast` | 1,429 通過；七個 ignored；52 suites |
| `cargo test --workspace --locked --no-fail-fast` | 1,295 通過；七個 ignored；52 suites |
| Workspace rustdoc，all-feature/default、locked、`--no-deps`、`RUSTDOCFLAGS=-D warnings` | 兩種皆通過 |
| Inventory 與 rejection self-tests | 159 routes、161 Action sites、162 reader occurrences 一致 |
| 雙語連結與 whitespace | 381 個本地目的地、八個 HB1 card anchors 通過；`git diff --check` 通過 |

All-feature 候選包含五個 HB1 單元測試與七個 HTTP 整合測試；default workspace
包含其中五個 HTTP 測試，不含兩個 Metamorph-only 控制。既有 CLI human/agent
auth 測試在兩種模式都通過。External-resource/native-store 的 ignored 測試仍
維持 ignored。本地 gate 紀錄與退出碼為 `target/hb1-gate-0.log` 至 `hb1-gate-6.log`、
`target/hb1-gates.json`。不宣稱實機、external schema 重跑、native-store 或 release
驗收；HEAD 維持 `d777925`，index 保持空白。

交付文件已區分未提交的 HB1 候選、AF1 與其餘 Q02/W07：雙語 queue、remaining
acceptance、execution checklist（含 K06）、hardening plan、backlog，以及公開
MockServer/lib 文件、server guides、changelog。更廣 endpoint/WS-Addressing、roles、
structured/raw injection、完整 SOAP/ONVIF conformance 維持未完成。目前指示下
不封存、不 commit、不 push 此候選。
