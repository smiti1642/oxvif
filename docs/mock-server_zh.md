# oxvif Mock ONVIF 裝置參考

[English](mock-server.md) | **繁體中文**

本文件完整說明 `oxvif::mock`：程序內 [`MockTransport`] 與繫結連接埠的 [`MockServer`]，包括 mock 的回應內容、狀態儲存方式、可調整項目，以及未納入模型的行為。

本文件所述行為均可對照具名的原始碼 symbol 驗證。若某項行為是刻意簡化，而非真實度聲明，文件會明確指出。未記錄的遺漏視為錯誤；已記錄的限制則屬設計決策。

- **適用對象**：oxvif 測試、下游 Rust crate，或連線至實體連接埠的非 Rust ONVIF client，例如 Frigate、ODM、gSOAP 或 C++ conformance suite。
- **版本**：0.16.0。
- **Feature flag**：程序內 transport 使用 `mock`，HTTP server 使用 `mock-server`。本 crate 不啟用任何 default feature；若未選用其中一項，以下 API 不會編譯。

## 快速導覽

| 章節 | 說明 |
| --- | --- |
| [1. 快速開始](#1-快速開始) | 啟動程序內或 HTTP mock |
| [2. 請求路由](#2-請求路由) | path、service URL 與 namespace 的分派方式 |
| [3. Envelope 與 namespace 契約](#3-envelope-與-namespace-契約) | mock 保證的 SOAP/XML 結構 |
| [4. 驗證](#4-驗證) | 已建模的驗證行為 |
| [5. 狀態模型](#5-狀態模型) | 服務之間共用的可變狀態 |
| [6. 預載 fixture](#6-預載-fixture) | 初始的裝置、媒體、PTZ、音訊與錄影資料 |
| [6.2.1 Source configuration 契約](#621-source-configuration-契約) | 原子寫入、sensor options 與模型限制 |
| [6.2.2 Encoder 幀率](#622-encoder-幀率) | 小數幀率、狀態遷移與 Media1 限制 |
| [7. 操作參考](#7-操作參考) | stateful、static 與不支援的操作 |
| [8. 實作範例](#8-實作範例) | 代表性請求與回應 |
| [9. 錯誤模型](#9-錯誤模型) | SOAP fault 結構與代碼 |
| [10. Fault injection 與控制 endpoint](#10-fault-injection-與控制-endpoint) | 強制產生 transport 或 protocol failure |
| [11. 調整裝置](#11-調整裝置) | 自訂狀態與 responder |
| [12. 保證範圍](#12-保證範圍與對應測試) | 各項真實度聲明的驗證測試 |
| [13. 已知限制](#13-已知限制) | 刻意簡化或尚未實作的項目 |
| [13.5. Acknowledgment-only 政策](#135-明確的-acknowledgment-only-政策) | 如何逐項啟用僅確認收件、不宣稱效果的回應 |
| [14. 擴充 mock](#14-擴充-mock) | 安全新增行為的方式 |

---

## 1. 快速開始

### 1.1 程序內 transport（`feature = "mock"`）

此模式不使用 socket 或 `axum`，除 client 已使用的 runtime 外也不需要其他 runtime。它是速度最快的路徑，也是 oxvif 單元測試採用的方式。

```rust
use std::sync::Arc;
use oxvif::{OnvifClient, mock::MockTransport};

let client = OnvifClient::new("http://mock")
    .with_transport(Arc::new(MockTransport::new()));
let info = client.get_device_info().await?;
assert_eq!(info.manufacturer, "oxvif-mock");
```

`MockTransport` 的 clone 成本低，且所有 clone 共用同一份裝置狀態與 fault queue（`src/mock/transport.rs`）。

### 1.2 繫結連接埠的 HTTP server（`feature = "mock-server"`）

當測試、其他 process 或非 Rust client 需要實際 endpoint 時，請使用真實 TCP listener。

```rust
use oxvif::mock::MockServer;

let server = MockServer::start().await?;          // ephemeral 127.0.0.1 port
let client = oxvif::OnvifClient::new(server.device_url());
```

Server 在背景 task 中執行，並於 `MockServer` 被 drop 時關閉，因此必須在使用期間保留該 binding。`MockServer::start()` 繫結 `127.0.0.1:0`；固定連接埠可使用 `MockServer::builder().port(8080)`（`src/mock/server.rs`）。

### 1.3 Builder 選項

| 方法 | 預設值 | 作用 |
|---|---|---|
| `.port(u16)` | `0`（ephemeral） | 要繫結的 TCP port |
| `.initial_state(DeviceState)` | factory default | 設定完整初始裝置 |
| `.on_change(ChangeHook)` | 無 | 每次 mutation 後觸發，可供持久化；server 本身不存取檔案系統 |
| `.enforce_auth(bool)` | `false` | 要求 WS-Security `PasswordDigest` |
| `.discoverable(Vec<String>)` | 關閉 | 使用指定 scope，於 UDP `3702` 回應 WS-Discovery `Probe` |
| `.replay(FixtureStore)` | 無 | 提供錄製的攝影機 clone；需要 `metamorph` feature |

`.discoverable()` 採 best-effort：若 `:3702` 因連接埠占用或 CI sandbox 而無法繫結，HTTP server 仍會啟動，但不可被探索。每台 host 最多只能有一個 discoverable server。

---

## 2. 請求路由

### 2.1 URL path 不參與路由

所有 path 的 `POST` 都由同一個 axum route `/{*path}` 處理（`src/mock/server.rs`）。分派完全依據 SOAP action；SOAP 1.2 將 action 放在 `Content-Type` header：

```text
Content-Type: application/soap+xml; charset=utf-8; action="http://www.onvif.org/ver10/device/wsdl/GetHostname"
```

`helpers::extract_action` 負責擷取 action，因此：

- 將 Media action 傳至 `/onvif/device` 仍會成功。
- 缺少或格式錯誤的 `action` 會產生 `Not implemented` fault，而非 404 或 path 提示。
- mock 公告的 service URL 僅用於模擬真實裝置，讓遵循 `GetCapabilities` 的 client 正常運作。

### 2.2 公告的 service URL

`GetCapabilities` 與 `GetServices` 會回傳以下相對於 server base URL 的位址（`src/mock/services/device.rs`）：

| Service | XAddr |
|---|---|
| Device | `{base}/onvif/device` |
| DeviceIO | `{base}/onvif/deviceio` |
| Media（1） | `{base}/onvif/media` |
| Media2 | `{base}/onvif/media2` |
| PTZ | `{base}/onvif/ptz` |
| Imaging | `{base}/onvif/imaging` |
| Events | `{base}/onvif/events` |
| Recording | `{base}/onvif/recording` |
| Search | `{base}/onvif/search` |
| Replay | `{base}/onvif/replay` |

### 2.3 Namespace 與 dispatcher

`src/mock/dispatch.rs` 的 `dispatch()` 依 action namespace 選擇 dispatcher，而不是依 operation name；原因是九項服務都包含 `GetServiceCapabilities`。

Synthetic dispatch 現在比對完整的受支援 Action URI，包含 Events 的 port
segment。下表的縮寫 prefix 僅為概覽，不代表子字串匹配規則。錯誤主機、插入的
path segment、不同 scheme 及跨 port 的別名都會遭拒；既有 client Action 不變。
HTTP header 解析及 replay invalidation 仍為獨立強化項目；fault／auth／replay／
custom responder 的優先順序維持不變。Operation／body 身分已由下述共用
synthetic 邊界檢查。

| Action prefix | Dispatcher | 操作數 |
|---|---|---|
| `…/ver10/device/wsdl/` | `dispatch_device` | 38 |
| `…/ver10/deviceio/wsdl/` | `dispatch_device_io` | 1 |
| `…/ver10/media/wsdl/` | `dispatch_media` | 32 |
| `…/ver20/media/wsdl/` | `dispatch_media2` | 26 |
| `…/ver20/ptz/wsdl/` | `dispatch_ptz` | 27 |
| `…/ver20/imaging/wsdl/` | `dispatch_imaging` | 8 |
| 完整 ONVIF Events／OASIS WSN port path | `dispatch_events` | 8 |
| `…/ver10/recording/wsdl/` | `dispatch_recording` | 11 |
| `…/ver10/search/wsdl/` | `dispatch_search` | 4 |
| `…/ver10/replay/wsdl/` | `dispatch_replay` | 2 |

總計 **157 項操作**。0.15.0 只將 `GetDigitalInputs` 由 `dispatch_device` 移至 `dispatch_device_io`，並未增加操作數。`deviceio` action prefix 使用小寫，符合 `deviceio.wsdl`；其 element 則位於 `…/ver10/deviceIO/wsdl`。Events action URI 另含 portType segment 與 `Request` suffix，因此 operation name 為 `GetServiceCapabilitiesRequest`、`PullMessagesRequest` 等。

未匹配任何分派規則的 action 會回傳：

```xml
<s:Fault>
  <s:Code><s:Value>s:Receiver</s:Value></s:Code>
  <s:Reason><s:Text xml:lang="en">Not implemented: {action}</s:Text></s:Reason>
</s:Fault>
```

同時在 stderr 記錄 `[WARN] unhandled action:`。

---

## 3. Envelope 與 namespace 契約

### Synthetic 請求邊界

到達 synthetic dispatch 的請求會解析一次，建立有資源上限且保留 namespace 的
樹狀結構。Operation 的 namespace／local name 必須符合解析後的 Action，
靜態讀取也適用。SOAP envelope 必須在可選 Header 之後包含單一 Body，Body
內含單一 operation，container text 僅能為 XML 空白；Header 誘導內容不能取代
Body operation。已遷移的 DeleteProfile handler 借用同一棵樹，不重新解析。

直接呼叫測試工具須提供可辨識身分的 operation，不可再送空字串或單獨欄位
fragment。例如 `<GetHostname xmlns="http://www.onvif.org/ver10/device/wsdl"/>`
仍受共用引擎的 standalone operation 路徑支援；實際 client 應傳送 SOAP 1.2
envelope。保留 standalone 支援不代表 HTTP SOAP binding 已驗收。

Malformed XML、namespace 錯誤、operation 不符及 container shape 錯誤改以
結構化 generic fault 回應；錯誤 SOAP Envelope 版本使用 `s:VersionMismatch`。
既有 operation-specific fault 保留各自遷移狀態。Synthetic XML 上限為
2 MiB UTF-8 text、深度 64、16,384 個 node；這些是 mock parser 限制，並非
攝影機容量。資源拒絕使用 `mock:RequestLimit`，不支援的 DTD 使用
`mock:RequestPolicy`，兩者均綁定 `urn:oxvif:mock:error`，reason 不包含請求
文字。HTTP 可能在進入引擎之前拒絕請求；header／status／encoding 策略仍待審查。

此功能不是 runtime XSD validation，也不是完整欄位驗證。Authentication、
must-understand／encoding 策略、其他 handler reader 及 replay effect 仍需
各自遷移。Fault／auth／raw／replay responder 保留優先順序。Metamorph 比較
採用較嚴格的 synthetic baseline，但不改寫已儲存的 request／response bytes。

### 回應 envelope

所有回應都由 `helpers::soap` 建立為 SOAP 1.2 envelope：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            {service namespace}>
  <s:Body>…</s:Body>
</s:Envelope>
```

`xmlns:s` 與 `xmlns:tt` 一定存在，各 handler 會加入所屬 service namespace。以下規則會對全部 157 項操作進行機械式驗證（`src/mock/dispatch.rs`）：

| Guard | 規則 |
|---|---|
| `no_response_declares_an_attribute_twice` | envelope start tag 不得重複宣告 attribute name（XML 1.0 §3.1） |
| `every_response_binds_the_prefixes_it_uses` | 文件使用的每一個 element prefix 都必須完成 namespace binding |

這兩項問題在 0.15.0 前確實存在，但 oxvif 的 `find_response` 只比對 local name，且 quick-xml 不驗證上述規則，因此原有測試無法偵測。嚴格遵循 namespace 的 gSOAP、`lxml` 等 consumer 應特別注意此契約。

| Prefix | Namespace |
|---|---|
| `tt` | `http://www.onvif.org/ver10/schema` |
| `tds` | `http://www.onvif.org/ver10/device/wsdl` |
| `trt` | `http://www.onvif.org/ver10/media/wsdl` |
| `tr2` | `http://www.onvif.org/ver20/media/wsdl` |
| `tptz` | `http://www.onvif.org/ver20/ptz/wsdl` |
| `timg` | `http://www.onvif.org/ver20/imaging/wsdl` |
| `trc` | `http://www.onvif.org/ver10/recording/wsdl` |
| `tse` | `http://www.onvif.org/ver10/search/wsdl` |
| `trp` | `http://www.onvif.org/ver10/replay/wsdl` |
| `tev` | `http://www.onvif.org/ver10/events/wsdl` |
| `wsnt` | `http://docs.oasis-open.org/wsn/b-2` |
| `tns1` | `http://www.onvif.org/ver10/topics`；於 event topic set 的 element 上宣告 |

---

### 3.1 尚未發布的 request 強化

Media1、Media2 `DeleteProfile` 現在從已識別 operation 的直接 child 讀取必要
token，並比對正確的 service namespace。XML 文字僅解碼一次，不修剪 token。
巢狀誤導欄位、重複欄位、錯誤 operation／namespace、未 binding prefix、DTD
及 malformed document 均在修改狀態前拒絕。接受完整 SOAP 1.2 envelope 或
具有 namespace 宣告的 operation document；不接受 prefix 未宣告的測試片段。

遷移後的 parser 限制為最多 2 MiB UTF-8 輸入、64 層 element、16,384 個
element，包含 envelope。這些是 mock 資源限制，不是 ONVIF 協定限制。
失敗時回傳 `env:Sender`，reason 帶有 `InvalidRequest-DELETEPROFILE`。
缺少／空 token 也改用此錯誤，不再回傳舊的 `ter:InvalidArgs`。

目前僅上述兩個操作完成遷移。其他 handler 仍使用舊 extractor；authentication、
client response parsing 與錄製 replay 均未變更。這不是完整 XSD 驗證或整體
mock conformance。詳見[強化計畫](active/mock-fidelity-hardening-plan_zh.md)。

---

## 4. 驗證

驗證預設關閉，因此無憑證的 client 可直接連線。可使用 `MockTransport::with_auth()` 或 `MockServerBuilder::enforce_auth(true)` 啟用。

啟用後會驗證 WS-Security **`PasswordDigest`**（`src/mock/auth.rs`），計算方式與實機相同：`Base64(SHA1(nonce + created + password))`。

憑證僅從唯一 qualified SOAP 1.2 Header／Security／UsernameToken 路徑讀取。
必要欄位須為唯一直接 scalar，僅解碼一次；username 與 Created 空白保留。
Password 必須宣告完整 PasswordDigest Type。Nonce EncodingType 可省略或宣告
Base64Binary，解碼後須為非空 bytes；只有 base64 解碼會忽略 lexical whitespace。
重複／巢狀欄位、foreign 或 Body 憑證、不支援的 recipient role、malformed／超界
XML 不得通過認證。Role 僅支援省略或明確 SOAP ultimateReceiver。一般缺少憑證
仍回報 `Missing Username`；其餘診斷 reason 為固定文字，不反射憑證。

這是認證測試替身，不是正式環境存取控制。尚未檢查 Created freshness 或 nonce
重用、實施 user-level authorization、支援 PasswordText 或全部 WS-Security 功能。
重複／過期但 hash 正確的 token 仍可通過。Raw／replay 回應不繞過啟用的 auth gate；
fault injection 仍優先處理。既有 HTTP body 上限可能在 SOAP 認證前回傳 `413`。
公開 Fault 分類與 auth-off 預設值不變。

- 預載帳號為 `admin` / `admin`（Administrator）與 `operator` / `operator`（Operator）。
- `GetSystemDateAndTime` 不需要驗證。ONVIF 規範要求此操作允許未驗證存取，因為 client 必須先取得裝置時間才能產生有效 digest。
- 未實作 HTTP Digest。只設定 HTTP Digest 的 client 即使憑證正確也無法通過 mock 驗證。

---

## 5. 狀態模型

`DeviceState`（`src/mock/state.rs`）為具有 35 個欄位的 flat serde struct，其中 32 個持久化、3 個僅供 runtime 使用。`MockState` 以 lock 包裝此狀態，並提供 `read()`、`modify()`、`modify_returning()` 與 `set_on_change()`。

所有持久化欄位都有 `#[serde(default = …)]`，因此可載入部分 JSON snapshot，其餘欄位會套用 factory fixture。

| 欄位 | 類型 | 預載內容 | 寫入來源 |
|---|---|---|---|
| `info` | `DeviceInfo` | 有 | 唯讀 |
| `hostname` | `String` | `"mock-camera"` | `SetHostname` |
| `hostname_from_dhcp` | `bool` | `false` | `SetHostname` |
| `users` | `Vec<MockUser>` | 2 筆 | `CreateUsers`、`DeleteUsers`、`SetUser` |
| `scopes` | `Vec<String>` | 有 | `SetScopes` |
| `timezone` | `String` | 有 | `SetSystemDateAndTime` |
| `daylight_savings` | `bool` | `false` | `SetSystemDateAndTime` |
| `dns` | `DnsState` | 有 | `SetDNS` |
| `ntp` | `NtpState` | 有 | `SetNTP` |
| `gateway_ipv4` | `Vec<String>` | 有 | `SetNetworkDefaultGateway` |
| `discovery_mode` | `String` | `"Discoverable"` | `SetDiscoveryMode` |
| `imaging_sources` | `Vec<ImagingState>` | 2 筆 | `SetImagingSettings`、`Move`、`Stop` |
| `ptz` | `PtzState` | 2 channel，以 PTZ node token 為 key | 12 項 PTZ 操作 |
| `ptz_nodes` | `Vec<PtzNodeEntry>` | 2 筆 | 唯讀 |
| `ptz_configs` | `Vec<PtzConfigEntry>` | 2 筆 | `SetConfiguration` |
| `interface` | `NetworkInterfaceState` | 有 | `SetNetworkInterfaces` |
| `protocols` | `Vec<NetworkProtocolState>` | 有 | `SetNetworkProtocols` |
| `osd` | `OsdState` | 有 | `CreateOSD`、`SetOSD`、`DeleteOSD` |
| `profiles` | `ProfilesState` | 4 筆 | profile create/delete、config add/remove |
| `recording` | `RecordingState` | 2 個 recording、2 個 job | 8 項 Recording 操作 |
| `video_sources` | `Vec<VideoSourceEntry>` | 2 筆 | 唯讀 |
| `video_source_configs` | `Vec<VideoSourceConfigEntry>` | 2 筆 | 兩種服務的 `SetVideoSourceConfiguration` |
| `video_encoders` | `Vec<VideoEncoderState>` | 4 筆 | 兩種服務的 `SetVideoEncoderConfiguration` |
| `relay_outputs` | `Vec<RelayOutputState>` | 2 筆 | `SetRelayOutputState`、`SetRelayOutputSettings` |
| `digital_inputs` | `Vec<DigitalInputState>` | 2 筆 | 僅 REST simulator |
| `storage` | `Vec<StorageEntry>` | 3 筆 | `SetStorageConfiguration` |
| `audio_sources` | `Vec<AudioSourceEntry>` | 2 筆 | 唯讀 |
| `audio_source_configs` | `Vec<AudioSourceConfigEntry>` | 2 筆 | 唯讀；oxvif 無 ONVIF setter |
| `audio_encoders` | `Vec<AudioEncoderEntry>` | 2 筆 | 兩種服務的 `SetAudioEncoderConfiguration` |
| `audio_outputs` | `Vec<AudioOutputEntry>` | 1 筆 | 唯讀 |
| `audio_decoders` | `Vec<AudioDecoderEntry>` | 1 筆 | 唯讀 |
| `metadata` | `Vec<MetadataEntry>` | 2 筆 | `SetMetadataConfiguration` |
| `event_seq` | `u64` | runtime | `PullMessages` |
| `event_filter` | `Option<Vec<String>>` | runtime | `CreatePullPointSubscription` |
| `pending_io_events` | `Vec<PendingIoEvent>` | runtime | REST simulator |

最後三個欄位標記為 `#[serde(skip)]`，只存在於個別 instance，不會持久化。

### 5.1 Media1 與 Media2 共用狀態

兩個 profile 建立操作都要求唯一的直接 scalar `Name`。名稱以解碼後文字存入
狀態，並在 profile 回應轉義一次；透過 `MockState` 設定的名稱亦然，請勿預先
轉義。空名稱及有效空白會保留於狀態與原始 XML（公開 client DOM 仍修剪文字）。
共用 escaping 以 character reference 表示 CR／LF／tab 資料，避免 XML normalization
改變 decoded value。Client request 與 structured Fault serializer 亦使用此機制；
含上述資料字元的 bytewise raw fixture 採用新的 reference 表示。
缺少、重複或巢狀名稱會在配置 token 前被拒絕，不通知狀態 hook。這不代表
巢狀 configuration、attribute、長度、容量或整個操作已完成驗證。
既有持久化 snapshot 不會自動解碼，因為類似 entity 的名稱也可能是刻意使用的
literal 文字。請檢查舊 mock 版本建立的名稱；若誤存 XML 拼法，須明確修正 fixture 值。

Media1 CreateProfile 的 optional Token、GetProfile 的 ProfileToken，以及六個
Media binding 入口，現在讀取唯一、直接 qualified scalar 身分。兩種 profile view
都將 token attribute 轉義一次，保留有效空白；重複或巢狀身分欄位在 mutation 前
回傳 fault。Seed 與持久化的 profile token 應為 literal 字串，而非預先轉義 XML；
不自動重新命名或解碼舊 snapshot。明確空值的 Create token 或 read selector 在
mutation 前回傳 `s:Sender`／`mock:RequestPolicy`；若要自動配置 token，請省略
Create token。非空字串的空白仍有意義。未篩選的 profile 讀取若包含空 seed token，
回傳 `s:Receiver`／`mock:RequestPolicy`；仍可個別選取有效 profile。此類 snapshot
entry 須明確修正。這是 mock 限制，不表示 ONVIF 一律禁止空字串。Raw／replay
override 保留原有優先順序。完整欄位政策、configuration-token 解析、adapter 與
replay-key 遷移仍待完成。

Media1 `GetProfiles`／`GetProfile` 與 Media2 `GetProfiles` 在同一 read lock
內取得 profile 及全部 configuration catalogue，再產生回應，避免將不同版本
的 binding 與 catalogue 拼接。不同請求仍可觀察不同版本，這不是跨呼叫 transaction。

兩者是同一台裝置的不同檢視。兩個 dispatcher 共有的操作會讀寫同一份 `DeviceState`，只有 XML rendering 不同。Media1 將完整 configuration 列為 `Name` 的 sibling；Media2 則放在 `<tr2:Configurations>` 下，且其中兩種 type 不同。`tests/mock_media1_media2_agree.rs` 會持續驗證兩個介面的一致性。

---

## 6. 預載 fixture

Factory device 是一台**雙感測器攝影機**。單 channel fixture 無法辨識 handler 是否真正依 token 取值；因此兩組預載值刻意不同。

### 6.1 識別資訊

| 欄位 | 值 |
|---|---|
| Manufacturer | `oxvif-mock` |
| Model | `MockCam-1080p` |
| Firmware | `1.0.0` |
| Serial | `MOCK-0001` |
| Hardware ID | `1.0` |

### 6.2 Video chain

| Sensor | Source config | Encoder config | Native resolution |
|---|---|---|---|
| `VS_1` | `VSC_1`（`VSConfig1`） | `VEC_1` `MainStream` 1920×1080、`VEC_2` `SubStream` 704×480 | 2592×1944 |
| `VS_2` | `VSC_2`（`VSConfig2`） | `VEC_3` `MainStream2` 1280×720、`VEC_4` | 1280×720 |

`VEC_1` 公告六種解析度，最高 2592×1944；`VEC_3` 最高為 1280×720。只有 `VS_1` 公告 H.265，因此對解析度或 encoding set 的 assertion 可偵測 handler 是否回傳錯誤 channel。

### 6.2.1 Source configuration 契約

**尚未發布：**兩種 Media service 均要求完整已建模 source configuration，先驗證 scoped
欄位，再一次原子提交。Name 與身分文字解碼、轉義一次。UseCount 與 ViewMode 為唯讀，
caller 不能改寫引用計數。Media1 ForcePersistence 必須是有效 boolean；儲存仍為記憶體
狀態及既有選填、由使用者管理的 persistence hook。

僅建模零原點 crop；非零 x／y、錯誤數字及不存在的 source reference 均拒絕。超過所選
sensor 的正值尺寸會 clamp，回讀顯示實際提交尺寸。非空 extension／其他未建模設定
明確拒絕，不默默捨棄。未知 configuration／profile 採 Sender／InvalidArgVal／NoConfig
或 NoProfile；無法套用的設定採 ConfigModify；結構錯誤沿用共用 request Fault 政策。

Source options 依實體 sensor 尺寸，不再隨 crop 縮小。省略 configuration selector
回傳保守的 generic 範圍與可用 source；ProfileToken 會驗證存在。Media2 清單支援
ConfigurationToken／ProfileToken。Mock 允許所有 profile 重新指定 source，未建模
實體 encoder-routing 衝突。內建 replay 拒絕時保留 recording，成功後才淘汰相依的
source／profile／options 讀取。詳見 [VS1 證據與限制](active/mock-fidelity-video-source_zh.md)。

### 6.2.2 Encoder configuration 契約

**尚未發布：**兩種 Media service 均先驗證完整、具 namespace 範圍的 encoder candidate，
再一次原子提交。拒絕時保留所有欄位、change hook 與內建 replay。Name／token 解碼、
轉義一次；UseCount 為唯讀。既有部分 raw request fixture 須補齊必要 configuration
欄位。Media1 另要求有效 boolean ForcePersistence；持久化仍為記憶體及選填、
由使用者管理的 hook。

Configuration selector 回傳指定 encoder，不存在時回覆 Sender／InvalidArgVal／NoConfig。
Profile selector 驗證存在；明示的全 profile 邏輯相容模型回傳相容 catalogue，而非只列出
目前綁定的 configuration。省略 options selector 回傳整台設備的聯集，不是預設 channel，
也不保證每個 encoder 均接受聯集中所有值。寫入前應查詢實際 configuration。
明確空值、重複、巢狀及錯誤 namespace 欄位均拒絕。

Options 與 setter 共用各 encoder 的解析度／codec／幀率限制。兩個 sensor 均建模
JPEG／H264；H265 僅在 Media2 的 VS_1 提供。Quality 限制於 0–10；有效但超出範圍
的有號 bitrate 調整至公告範圍。提供的幀率確定性地調整至最近公告值，等距時取較低值；
零調整為 1。共用幀率使用 f32，包含 12.5，以及 VS_1 的 29.97。省略 RateControl
保留原幀率／bitrate，即使切換 codec 亦然；需對齊新 codec 限制時應明確提供此欄位。

Media1 encoder／profile view 若包含 H265 或小數幀率，回覆頂層 Receiver／
mock:RequestPolicy Fault，不修改共用狀態。手動 seed 的無效數字亦拒絕輸出。
舊整數 JSON 幀率仍可讀取；負值／非有限幀率不能持久化。不受影響的個別 encoder
仍可讀取。詳見[幀率遷移](media2-frame-rate_zh.md)。

不執行實際串流。Media1 僅接受未變更的 interval 1、零 multicast 位址／port、
TTL 1、AutoStart false 與零 session timeout。不支援的 multicast、簽章或
constant-bitrate 效果明確拒絕。Media2 GetVideoEncoderInstances 要求
**source configuration** token：預設 VSC_1 的 total 為 4、VSC_2 為 2，另列各 codec
限制。這些是合成容量邊界，非使用量或硬體效能；匯入 source catalogue 超過八個 profile
模型上限時拒絕此 view。Source 提交淘汰 capacity recording；profile 提交淘汰具
profile selector 的 encoder options；encoder 提交淘汰相依的 encoder／profile 讀取。
詳見 [VE1 範圍與證據](active/mock-fidelity-video-encoder_zh.md)。

### 6.3 Profile

支援的 configuration binding 會在同一 write lock 中，先驗證完整請求再寫入
slot。拒絕時保留 state 且不通知；成功的多筆 Media2 請求僅通知一次，包含
冪等移除。這不表示已完整建模 configuration conflict。

自動配置的 profile 身分會跳過已用 token；明確 token 的重複檢查與新增共用
同一 write lock。重複拒絕會保留 state 且不呼叫 change hook。持久化 counter
是可環回的搜尋起點，不保證永不重用歷史上已刪除的 token。建立時遵守公告的八個
profile 上限；較大的匯入 fixture 仍可讀取及刪除，但數量低於上限前拒絕新增，
不會默默刪除匯入的 profile。

Media2 建立會原子套用初始 configuration。AddConfiguration 可僅更新 Name，不提供
時保留原名；create／add 忽略 All，remove-All 清空已建模 slot。相同 reference 重複
為冪等，同一 slot 的不同 reference 則拒絕。支援 VideoSource、VideoEncoder、
AudioSource、AudioEncoder 及 PTZ，其他種類仍明確拒絕。拒絕不改名、不配置 token、
不變更計數。受影響的 configuration `UseCount` 在同一鎖中依已提交的 reference 重算，
包含刪除；不改動無關 seed 的任意計數。完整實體 configuration 相容性及欄位驗證仍待
完成，詳見 [組裝批次](active/mock-fidelity-profile-assembly_zh.md)。

| Token | Name | Fixed | Source cfg | Encoder cfg | PTZ cfg | Audio cfg |
|---|---|---|---|---|---|---|
| `Profile_1` | `mainStream` | 是 | `VSC_1` | `VEC_1` | `PTZConfig_1` | `ASC_1` + `AEC_1` |
| `Profile_2` | `subStream` | 否 | `VSC_1` | `VEC_2` | `PTZConfig_1` | 無 |
| `Profile_3` | `mainStream2` | 是 | `VSC_2` | `VEC_3` | `PTZConfig_2` | 無 |
| `Profile_4` | `subStream2` | 否 | `VSC_2` | `VEC_4` | 無 | 無 |

`fixed="true"` 的 profile 不允許刪除，會回傳 `ter:DeletionOfFixedProfile`。`Profile_4` 刻意不繫結 PTZ configuration，用於測試不具 PTZ 能力的 profile；任何以該 profile 執行的 PTZ 操作都會產生 fault。

此旗標不代表 configuration binding 不可修改：已支援的 Add／Remove configuration
操作仍可變更其繫結。這是既有預期行為，不是 acknowledgment-only stub，也不是 mock 專屬例外。

### 6.4 PTZ：每個鏡頭一個 head

Profile 不直接擁有 head，而是透過 PTZ configuration 取得：

```text
ProfileToken → ProfileEntry.ptz_config_token → PtzConfigEntry.node_token → PtzChannel
```

| Node | 可由哪些 profile 存取 | Space | Home | Fixed home | Max presets | Aux |
|---|---|---|---|---|---|---|
| `PTZNode_1` | `Profile_1`、`Profile_2`（lens 1） | 全部 8 種 | 是 | 否 | 100 | 2 |
| `PTZNode_2` | `Profile_3`（lens 2） | 僅 zoom（4 種） | 否 | 是 | 8 | 0 |

| Node | Position（pan、tilt、zoom） | Preset | Tour |
|---|---|---|---|
| `PTZNode_1` | 0.0、0.0、0.0 | `Home`、`Door` | 1 |
| `PTZNode_2` | 0.0、0.0、0.80 | `Lobby`、`Dock`、`Roof` | 0 |

`PTZNode_2` 不宣告 pan/tilt space；若 `Profile_3` 的 `AbsoluteMove`、`RelativeMove` 或 `ContinuousMove` request 含 `<tt:PanTilt>`，即使值為 `x="0" y="0"` 也會被拒絕。oxvif 的 move API 目前一律輸出 `<tt:PanTilt>`，因此 zoom-only head 應使用 `GotoPreset` 定位；這是 client 面對真實 zoom-only hardware 的功能缺口，不是 mock 特例。

| Token | Node | UseCount | Default space | DefaultPTZSpeed | PanTiltLimits | ZoomLimits | Timeout | Options |
|---|---|---|---|---|---|---|---|---|
| `PTZConfig_1` | `PTZNode_1` | 2 | 全部 6 種 | 0.5 / 0.5 / 0.5 | ±0.9 × ±0.7 | 0.0–1.0 | `PT10S` | `PT1S`–`PT60S` |
| `PTZConfig_2` | `PTZNode_2` | 1 | 3 種 zoom | 無 | 無 | 0.1–0.95 | `PT30S` | `PT5S`–`PT30S` |

`DefaultAbsolutePantTiltPositionSpace` 中的 `Pant`（兩個 `t`）是 `onvif.xsd` 的規範拼字，必須保留。

### 6.5 Audio 與 option shape

預載兩組可由 token 定址、且重要值彼此不同的 audio source/configuration。`AEC_1` 為 G711、64 kbps、8 kHz；`AEC_2` 為 AAC、128 kbps、48 kHz。只有 `Profile_1` 繫結 audio。

`GetAudioEncoderConfigurationOptions` 在兩個服務中的 nesting 不同：

```text
Media1  Response/Options   tt:AudioEncoderConfigurationOptions   ← wrapper
                /Options   tt:AudioEncoderConfigurationOption    ← repeated entry
Media2  Response/Options   tt:AudioEncoder2ConfigurationOptions  ← repeated entry
```

0.15.0 已修正兩者原先互換的問題；parser 現在可讀取兩種結構。Wire-level 測試仍直接驗證 raw bytes，以確保兩種 shape 不會因 parser 相容性而被混淆。

### 6.6 Storage、metadata、recording 與 I/O

| Storage token | Type | LocalPath | StorageUri | User |
|---|---|---|---|---|
| `SD_01` | `LocalStorage` | `/mnt/sd` | 無 | 無 |
| `NAS_01` | `NFS` | `/mnt/nas` | `nfs://192.168.1.50/records` | `recorder` |
| `CIFS_01` | `CIFS` | 無 | `smb://192.168.1.60/cam` | 無 |

Metadata 有 `MetaConf_1` 與 `MetaConf_2`；兩者在 analytics、PTZ status/position、multicast 與 status capability 上刻意不同。`Multicast` block 為必要 element；無 multicast group 時省略可選的 `Address/IPv4Address`，並將 `AutoStart` 設為 false。

| Recording | Track | Bounds | Status |
|---|---|---|---|
| `Rec_001` | `VIDEO001`（Video） | 2026-01-01 → 2026-04-01 | `Stopped` |
| `Rec_002` | 無 | 2026-05-01 → 2026-06-01 | `Recording` |

預載 job 為 `Job_001` → `Rec_001`（`Active`）與 `Job_002` → `Rec_002`（`Idle`）。I/O 包含兩個 relay output 與兩個 digital input：`RelayOutput_1`（Bistable、idle closed）、`RelayOutput_2`（Monostable、`PT1S`、idle open）、`DigitalInput_1`（idle closed）及 `DigitalInput_2`（idle open）。

---

## 7. 操作參考

圖例：● 表示由 `DeviceState` 支援的讀寫操作；○ 表示每次回覆相同的 static fixture；**T** 表示依 token 回答，且預載 fixture 中至少有兩個 token 的結果不同。

### 7.1 Device 與 DeviceIO

Device 共 38 項操作。裝置資訊、日期時間設定、hostname、NTP、DNS、scope、user、network interface/protocol/gateway、discovery mode、relay 與 storage 均為 ●。`GetCapabilities`、`GetServices` 與 `GetServiceCapabilities` 使用 static service metadata；`GetSystemLog`／`GetSystemUris` 為 static read fixture。`SetSystemFactoryDefault`、auxiliary、upgrade／restore 與 reboot 預設拒絕，須逐項明確 opt-in 僅回覆收件，不執行對應效果（§13.5）。

DeviceIO 的唯一操作是 `GetDigitalInputs`，由 REST simulator 驅動。其 endpoint 為 `{base}/onvif/deviceio`；action segment 使用小寫 `deviceio`，element namespace 則為 `…/ver10/deviceIO/wsdl`。

### 7.2 Media1（32 項操作）

Profile、video source/configuration、video encoder、OSD 與 audio catalog/configuration 均由共享狀態支援。`GetStreamUri` 與 `GetSnapshotUri` 對所有 profile 回傳同一組 canned URI；`GetOSDOptions` 與 `GetServiceCapabilities` 為 static。`SetAudioEncoderConfiguration` 若缺少規範要求的 `Multicast` 或 `SessionTimeout` 會拒絕 request。

### 7.3 Media2（26 項操作）

`GetProfiles` 的可省略 `Token` 使用解碼後完整值，選出單一 profile 或回傳 Fault。
省略 `Type` 時不回傳 configuration；單一 `All` 回傳全部 binding，其他 list
僅投影符合的已建模 configuration kind，不影響 profile 數量或共享 state。
既有完整 profile client 方法明確傳送 `Type=All`。重複 Token、巢狀 scalar 及
不合法欄位順序會被拒絕；完整欄位長度／attribute 政策仍屬未完成的驗證範圍。

Media2 與 Media1 共用 profile、video、audio 狀態，並另提供 metadata。`GetMetadataConfigurations` 的 `ConfigurationToken` 是 filter；無結果時回傳空 list，而 `GetMetadataConfigurationOptions` 的未知 token 會 fault。`GetVideoSourceModes` 為已宣告 stub；`SetVideoSourceMode` 一律回傳 `ter:ActionNotSupported`，不會宣稱已儲存未建模的 sensor mode。

### 7.4 PTZ（27 項操作）

使用 profile／head resolution 的 19 個 handler，從共用 parsed operation 讀取
直接、namespace-qualified `ProfileToken`。XML 文字只 decode 一次，空白具有意義。
重複或內含子節點的 scalar 在 state 變更前回傳通用 `InvalidArgs`；foreign、nested
與 Header 欄位不能提供 token。保留缺失／空值及未知 token 的既有普通 Fault payload。
Configuration／preset／tour token、座標解析、完整 Fault policy、replay invalidation
與時間行為保證仍為獨立工作。

使用 profile／head resolver 的操作要求 `ProfileToken`。缺少 token 時回傳 `env:Sender`；不存在的 profile 回傳 `ter:NoProfile`；未繫結 PTZ configuration 的 profile 回傳 `ter:NoConfig`。PTZ `SendAuxiliaryCommand` 為獨立 receipt-only stub：預設拒絕，opt-in 保留 command allowlist，但不驗證 ProfileToken，也不執行命令（§13.5）。Move 為立即完成，不模擬移動時間。以 node 或 configuration token 定址的 getter/setter 對未知 token 會 fault；`GetCompatibleConfigurations` 對不具 PTZ 能力的 profile 回傳空 list。

### 7.5 Imaging、Events、Recording、Search 與 Replay

- Imaging 的七項影像操作依 `VideoSourceToken` 存取狀態，`GetServiceCapabilities` 為 static。
- Events 的 `CreatePullPointSubscriptionRequest` 會儲存 topic filter；`PullMessagesRequest` 會輸出週期性 synthetic stream 與 REST 注入的 I/O event，並遞增 `event_seq`。`SubscribeRequest`／`RenewRequest`／`UnsubscribeRequest`／`SetSynchronizationPointRequest` 預設拒絕；逐項 opt-in 只確認收件，不建立 push subscription、延長 lifetime 或送出同步事件（§13.5）。`GetServiceCapabilitiesRequest` 為 static read fixture。
- Recording 的 recording、track 與 job 操作均由狀態支援；刪除 recording 會一併刪除所屬 job。
- `FindRecordings` 只提供單一 search token，不模擬 cursor；`GetRecordingSearchResults` 讀取目前 recording list。
- `EndSearch` 預設拒絕；opt-in 僅回傳 fixture `Endpoint` timestamp，不終止搜尋（§13.5）。
- `GetReplayUri` 依 recording token 回答，未知 token 會 fault。

---

## 8. 實作範例

以下內容取自實際 dispatcher；envelope attribute 與 element 順序保持原樣。

### 8.1 簡單讀取

Request action：`http://www.onvif.org/ver10/device/wsdl/GetHostname`

```xml
<tds:GetHostname/>
```

```xml
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
  <s:Body>
    <tds:GetHostnameResponse>
      <tds:HostnameInformation>
        <tt:FromDHCP>false</tt:FromDHCP>
        <tt:Name>lobby-cam</tt:Name>
      </tds:HostnameInformation>
    </tds:GetHostnameResponse>
  </s:Body>
</s:Envelope>
```

### 8.2 無回傳值的寫入

```xml
<tds:SetHostname><tds:Name>lobby-cam</tds:Name></tds:SetHostname>
```

```xml
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
  <s:Body><tds:SetHostnameResponse/></s:Body>
</s:Envelope>
```

### 8.3 Media1 與 Media2

兩者會呈現同一份 profile 狀態，但 XML shape 不同。Media1 直接 inline `VideoSourceConfiguration`、`VideoEncoderConfiguration` 等完整 configuration；Media2 則在 `<tr2:Configurations>` 下使用 `VideoSource`、`AudioSource`、`VideoEncoder`、`AudioEncoder`、`PTZ`。Media2 的 `VideoEncoder` 與 `AudioEncoder` 使用各自的 version 2 type；其餘三項與 Media1 使用相同 type。

Media2 member 順序為 `VideoSource, AudioSource, VideoEncoder, AudioEncoder, Analytics, PTZ, …`。`tt:VideoEncoder2Configuration` 將 `GovLength` 與 `Profile` 表示為 attribute：

```xml
<tr2:VideoEncoder token="VEC_1" GovLength="25" Profile="Main">
```

### 8.4 Per-channel 回應與可選 element

`VEC_1`（`VS_1`）的 H264 options 最高支援 2592×1520，`VEC_3`（`VS_2`）最高為 1280×720。Response 另含 nested `Extension`：

```text
Options/H264                          no BitrateRange
Options/Extension/H264                adds BitrateRange
Options/Extension/Extension/H265      the only place H265 lives
```

Parser 應優先使用最深層節點，並逐層向外 fallback。不存在的可選值會省略 element，而不輸出空 element。

### 8.5 Fault

`DeleteRecording` 使用不存在的 `Rec_999` 時：

```xml
<s:Fault>
  <s:Code><s:Value>ter:NoRecording</s:Value></s:Code>
  <s:Reason><s:Text xml:lang="en">NoSuchRecording-DELREC-5701: Rec_999</s:Text></s:Reason>
</s:Fault>
```

---

## 9. 錯誤模型

Fault 使用 SOAP 1.2 `<s:Fault>`，包含 `Code/Value` 與 `Reason/Text`（多數服務 handler 仍使用 `helpers::resp_soap_fault`）。目前 HTTP status 維持 **200**；SOAP HTTP binding 稽核尚未完成。XML 結構正確不能證明 HTTP 行為正確。

在強化分支中，Media1／Media2 `DeleteProfile` 對不存在或固定 profile 的拒絕使用
巢狀 Sender Fault。`SoapError::Fault.code` 為 `s:Sender`，`subcode` 維持**第一層**：
不存在 profile 為 `ter:InvalidArgVal`，固定 profile 為 `ter:Action`。Wire 最深層條件
分別為 `ter:NoProfile` 與 `ter:DeletionOfFixedProfile`；reason 文字不變。
此局部遷移不代表其他操作已修正。DeleteProfile 現在對不存在／固定 profile 的拒絕
不通知 change hook，成功刪除則通知一次。內部通知 predicate 並非 rollback；公開
`modify`／`modify_returning` 語意不變。內建 replay clone 現在於拒絕建立或刪除時保留
profile 錄製結果，成功 synthetic 建立或刪除後才淘汰 Media1 GetProfile／GetProfiles
與 Media2 GetProfiles。因此建立會刷新錄製的 profile 清單及單筆 profile 檢視。
已提交的 Media1 video source／encoder Add／Remove 與 Media2 Add／RemoveConfiguration
亦淘汰上述 profile 讀取，包含成功的冪等 remove。這些已提交 effect 亦淘汰依賴 binding／
引用計數的已建模 configuration read、PTZ compatible configuration 及具 profile selector
的 encoder options。Encoder capacity 不依賴 profile 使用量，故 binding 不淘汰其
recording；source configuration 提交則會淘汰。
被拒絕的 binding 保留錄製結果，
也不會使其他服務中同 family 名稱的讀取失效。其他 configuration 寫入與 mutation、
單獨建構 ReplayResponder、更多相依關係及併發可見性
仍待審查。

Profile 組裝亦遷移重複 token 及 binding reference 的拒絕。公開 `subcode` 仍保留
第一層，不改成最深層條件：

| 拒絕原因 | SOAP code | 依序巢狀 subcode |
| --- | --- | --- |
| 明確 profile token 重複 | Sender | InvalidArgVal → ProfileExists |
| Binding 的 profile／configuration 不存在 | Sender | InvalidArgVal → NoProfile／NoConfig |
| 建立 profile 達容量上限 | Receiver | Action → MaxNVTProfiles |
| 同一已建模 slot 指派不同 configuration | Receiver | Action → ConfigurationConflict |

其他 legacy service Fault 與完整實體相容性仍屬後續工作。

Replay key 現在於投影後清除 URL `user:pass@`，包含 XML entity decoding 後的帳密。
舊檔載入僅清理記憶體中的 key，不自動覆寫磁碟；需明確保存，分享前亦須檢查舊副本。
僅憑證不同而碰撞的 key 保留最後一筆。Raw envelope 仍僅針對指定格式去除憑證，
不保證任意裝置資料均無秘密。詳見[函式庫指南](../LIBRARY_GUIDE_zh.md#metamorphmetamorph--metamorph-server-feature)。

Replay 現在於 key 命中後檢查 scoped XML 身分；不同 namespace、scalar 或 structure
轉入 synthetic，完全相同的 raw recording 仍受支援。非同一原文的 malformed XML、
mixed content 與未解析 `xsi:type` 不構成等價。此防護限制選定的 response substitution，
未遷移 index 或恢復被覆蓋的錄製，亦非完整 protocol／QName-valued content 驗證。

許多 token-error reason 帶有 operation tag 與 numeric id，例如：

```text
NoSuchRecording-DELREC-5701: Rec_999
│              │      │      └─ offending value
│              │      └──────── unique numeric id
│              └─────────────── operation abbreviation
└────────────────────────────── condition
```

| Code | 意義 | Reason tag 範例 |
|---|---|---|
| `env:Sender` | request 格式錯誤或缺少必要輸入 | `NoProfileToken-STATUS-5601`、`NoStorageType-STOR-5801` |
| `ter:NoProfile` | profile token 不存在 | `NoSuchProfile-ABSMOVE-5606` |
| `ter:ProfileExists` | 建立時 profile token 重複 | |
| `ter:DeletionOfFixedProfile` | 嘗試刪除 `fixed="true"` profile | |
| `ter:NoConfig` | configuration token 不存在 | `NoSuchMetadataConfig-SETMETA-5811` |
| `ter:ConfigurationConflict` | Media2 `AddConfiguration` type 未建模 | `UnmodelledConfigType-CFG2-5542` |
| `ter:InvalidArgs` | Media operation argument 無效 | |
| `ter:InvalidArgVal` | 值不在可接受範圍 | `NoSuchStorage-STOR-5802`、`BadJobMode-SETJOBMODE-5705` |
| `ter:NoRecording` / `ter:NoTrack` / `ter:NoJob` | recording family token 不存在 | `NoSuchRecording-REPLAY-5709` |
| `ter:ActionNotSupported` | action 已路由，但刻意未建模 | `NotModelled-VSMODE-5813` |
| `s:Receiver` | action 未路由 | `Not implemented: {action}` |

**尚未發布的修正：** 共用 Fault helper 現在宣告 `ter:`、`env:` 及既有的 `s:`，
並轉義 code／reason 文字，包含明確注入的 Fault。Injection API 應傳入原始文字，
不要預先轉義 XML。Helper 不會自動解析自訂、未知的 QName prefix。

認證 Fault 及空 responder chain 的防禦性 Receiver Fault 已使用私有結構化
serializer。認證錯誤維持 `s:Sender` 與第一層 subcode `wsse:FailedAuthentication`，
並於其 Value element 宣告 namespace。Reason 原始文字只轉義一次；XML 不允許
的字元改為 U+FFFD，原始 CR 使用 character reference 表示。憑證解析另依 §4
更新；豁免項目、認證預設值及 HTTP status 不變。Client 仍只公開第一層 subcode，
而非最深層。

**仍有偏差：** 其他服務 Fault 仍採平面的 `Code/Value` 表示。Namespace binding
修正並未完成各操作的 code／subcode 階層，也未驗證 HTTP status 行為；這些
遷移仍待完成，不得因此宣稱完整 SOAP／ONVIF Fault conformance。尚未遷移的
request extractor 也可能將已轉義文字放入 reason，因此目前只有已遷移路徑
具備 token 原始文字回傳正確性的驗證。

---

## 10. Fault injection 與控制 endpoint

### 10.1 Rust API

```rust
server.inject_fault("GetProfiles", "ter:NoProfile", "injected");
// next action whose URI ends with "GetProfiles" faults, once
server.clear_faults();
```

注入 fault 僅觸發一次，第一次符合的 action 會消耗該項目。`MockTransport` 提供相同 API。

### 10.2 HTTP API（僅 `mock-server`）

| Endpoint | Method | 參數 |
|---|---|---|
| `/admin/inject_fault` | POST | 必要 `action`；`code` 預設 `s:Receiver`；`reason` 預設 `Injected fault` |
| `/admin/clear_faults` | POST | 無 |
| `/mock/snapshot.jpg` | GET | 產生 JPEG |
| `/mock/digital-input/{token}/pulse` | POST | 觸發 event 後還原 |
| `/mock/digital-input/{token}/set` | POST | 固定目前狀態 |

缺少 `action` 會回傳 `400`。`/admin` **沒有驗證機制**；請維持預設 loopback binding，切勿對外暴露。

---

## 11. 調整裝置

### 11.1 直接修改狀態

```rust
server.device().modify(|s| {
    s.info.model = "MyCam-4K".into();
    s.video_encoders[0].width = 3840;
});
```

`device()` 回傳 `MockState`；測試 assertion 使用 `read()`，修改則使用 `modify()`。

### 11.2 提供完整 `DeviceState`

```rust
let state: DeviceState = serde_json::from_str(&saved)?;   // `serde` feature
let server = MockServer::builder().initial_state(state).start().await?;
```

所有欄位都有 serde default，因此允許部分 JSON，未指定欄位會使用 factory fixture。

### 11.3 變更時持久化

```rust
let server = MockServer::builder()
    .on_change(Arc::new(|s: &DeviceState| { /* write to disk */ }))
    .start().await?;
```

Library 本身不存取檔案系統；此 hook 是唯一的持久化接點。
快照在 mutation 的 write lock 內擷取，釋放鎖後才執行 hook。Hook 可以進行
有界重入寫入，但必須自行避免無限遞迴。併發 callback 不保證按 commit 順序
執行；持久化需要順序時，請協調 mutation 或使用具版本的儲存機制。因此快照
可能不同於 hook 內重新 `read()` 的結果。未註冊 hook 時不複製快照。明確拒絕的
conditional outcome 仍不通知；此機制不提供部分寫入的 rollback。

### 11.4 插入 responder

`Chain`、`Responder` 與 `RequestCtx`（`src/mock/responder.rs`）可在 synthetic dispatcher 前插入自訂 handler；`metamorph` replay clone 即使用此機制。

### 11.5 無法透過 ONVIF API 修改的內容

以下內容在 SOAP 介面中為唯讀，請使用 §11.1：

- `info` 與 `video_sources`。
- `digital_inputs`；只能由 REST simulator 驅動。
- `MetadataEntry::pan_tilt_status_supported`、`zoom_status_supported`、`multicast_address` 與 `multicast_port`。
- 所有 `use_count`；實機會由 binding 關係推導。

---

## 12. 保證範圍與對應測試

Mock 契約由使用 public API、且每次使用全新 server 的 property test 驗證。

| 保證 | 測試 |
|---|---|
| Client 可送出的全部 157 個 action 都有路由 | `mock_handles_every_action_the_client_can_send` |
| Response 不重複 attribute | `no_response_declares_an_attribute_twice` |
| Response 不使用未宣告 prefix | `every_response_binds_the_prefixes_it_uses` |
| 選定的 49 組 write／read 配對可 round-trip | `tests/mock_roundtrip.rs`；不涵蓋全部 effectful 操作 |
| 每個接受 token 的操作都能區分 token，或明確宣告為 blind | `tests/mock_token_discrimination.rs` |
| Media1 與 Media2 的共享狀態保持一致 | `tests/mock_media1_media2_agree.rs` |
| Per-sensor 回應確實不同 | `tests/mock_multi_sensor.rs` |
| End-to-end flow | `tests/mock_workflow.rs` |
| XML namespace、name、cardinality 與 sequence order 符合 ONVIF schema | `tests/mock_schema_shape.rs`；限制如下 |

`tests/mock_schema_shape.rs` 標記為 `#[ignore]`，執行時由 `$OXVIF_ONVIF_SCHEMA` 讀取 repository 外的 ONVIF schema。明確選取執行時，缺少資源即失敗；現在也要求外部 SOAP 1.2 envelope schema。逐節點 namespace 解析與分別執行的 Envelope／payload 檢查涵蓋 Fault 結構，但不驗證全部 XSD 值或錯誤語意。獨立 Windows／Linux CI 已設定使用固定版本的 Xerces 與外部 schema，驗證選定的 110 份 profile／source／rate／encoder request／response instance。此有限 corpus 不涵蓋所有操作或認證；清冊 job 本身不驗證 XML。詳見[驗證檢查點](active/mock-fidelity-schema-preflight_zh.md)。0.15.0 的十項計數均為 0，但這不等同於宣告 mock 已通過 ONVIF conformant 認證；`xs:any` 與全 optional child 等 schema 特性仍可能掩蓋語意錯誤。

目前 49 組 round-trip 全數為 working，無 static 或 known-broken；35 組 token row 中 30 組可區分、5 組明確標記為 blind。測試表的每個 row 都宣告意圖，避免已知限制演變成未追蹤的永久盲點。

---

## 13. 已知限制

### 13.1 已宣告的 static read stub

`tests/mock_roundtrip.rs` 的 49 組配對已無 `Static` row，但不涵蓋全部 `Set` 或 effectful 操作；§13.5 另列選定的拒絕與 receipt-only stub。仍為 static 的 read family 包括：

- Media2 `GetVideoSourceModes`：所有 `VideoSourceToken` 都回傳同一個 `Mode_1`。
- 兩種 media service 的 `GetStreamUri` / `GetSnapshotUri`：所有 profile 都回傳同一 URI。
- Media1 `GetOSDOptions`。Media2 `GetVideoEncoderInstances` 已改為依 source configuration
  選擇合成容量，詳見 §6.2.2，不再列為 static stub。

### 13.2 型別真實度缺口

- ONVIF schema 的 `PTZConfiguration` 可含 `MoveRamp`、`PresetRamp`、`PresetTourRamp`，`PTZNode` 可含 `GeoMove`；oxvif public type 目前未解析這四個 attribute。
- `VideoSourceMode/@Enabled` 未包含在 `oxvif::VideoSourceMode` 中，因此無法透過 getter 驗證 active mode；補足此能力需要 public API 變更。

### 13.3 已記錄的簡化

- 不模擬移動時間；PTZ move 立即更新 position，`MoveStatus` 固定為 `IDLE`。Monostable relay 也不會自動還原，請使用 REST pulse hook。
- 不模擬 search cursor；`FindRecordings` 只提供一個 token，結果一次回傳完整目前清單。
- Source crop 僅支援零原點；非零 `Bounds/@x`／`@y` 現在回 Fault，正值尺寸可 clamp 至所選 sensor 範圍，詳見 §6.2.1。
- Media1 audio encoder request 缺少 required `Multicast` 或 `SessionTimeout` 時會回傳 `ter:ConfigModify` / `IncompleteAudioEncoder-SETAEC-5715`。
- Media2 `SetAudioEncoderConfiguration` 無法表示 `SessionTimeout`，所以會保留原值；可選 `Multicast` 則會寫入，包括 `None`。
- `SetConfiguration` 忽略 `ForcePersistence`，一律持久儲存；`UseCount` 不由 caller 修改。
- 新建 recording 不建立虛構 time bound，因此省略 `Earliest` / `Latest`；刪除 recording 會同時刪除其 job。
- Media1 encoder options 不提供 H.265；H.265 僅位於 extension。Media2 只在 `VS_1` 公告 H.265。
- `SetRelayOutputState` 會寫入 `logical_state`，但 ONVIF getter 不回傳 live state；可由 Rust state 與 event 觀察。
- `SetVideoSourceMode` 回傳 `ter:ActionNotSupported` / `NotModelled-VSMODE-5813`，不會回報無法驗證的成功。

### 13.4 未實作的 protocol surface

- HTTP Digest authentication；只支援 WS-Security `PasswordDigest`。
- RTSP；`GetStreamUri` 只回傳 URI，不提供 media stream。
- Media2 `AddConfiguration` 會拒絕 `Metadata`、`Analytics`、`AudioOutput` 與 `AudioDecoder` configuration type，錯誤為 `UnmodelledConfigType-CFG2-5542`。`ProfileEntry` 與 `MediaProfile2` 未提供可觀察這些 binding 的欄位。

---

### 13.5 明確的 acknowledgment-only 政策

**尚未發布：**十一項已分類操作預設回傳 `s:Receiver`，第一層 subcode 為
`mock:UnmodeledEffect`。若測試流程僅需確認收件，可逐項啟用：

```rust
use oxvif::mock::{AckOnlyOperation, MockTransport};

let mock = MockTransport::new()
    .with_acknowledgment_only(AckOnlyOperation::EventsUnsubscribe);
```

| 選項 | 收件回應不證明的效果 |
| --- | --- |
| `DeviceFactoryDefault` | 裝置 state 已重設 |
| `EventsUnsubscribe` | Subscription 已終止或佇列事件已移除 |
| `EventsSynchronizationPoint` | 已產生或送出同步事件 |
| `DeviceAuxiliaryCommand` | 已執行 Device auxiliary command |
| `PtzAuxiliaryCommand` | 已執行 PTZ auxiliary command 或驗證 profile |
| `DeviceReboot` | 已重啟；收件訊息明確表示未執行重啟 |
| `DeviceFirmwareUpgrade` | Upload endpoint 可用或已升級韌體 |
| `DeviceSystemRestore` | Upload endpoint 可用或已還原設定 |
| `EventsSubscribe` | 已建立 push subscription 或推送通知 |
| `EventsRenew` | 已延長 subscription lifetime |
| `SearchEnd` | 已終止搜尋或使其過期 |

`MockServer::builder()`、`MetamorphTransport` 與 `AdapterTransport` 亦提供相同
方法。重複呼叫會累加選定操作；transport clone 個別複製政策，但保留既有共用
裝置 state。`AckOnlyOperation::action()` 顯示精確 Action URI；沒有全域或 suffix
opt-in。Events synchronization 與 Media synchronization 為不同操作。

拒絕與 acknowledgment 均不改變 state、change hook、committed effect 或 replay
invalidation。共用 XML／Action／body 身分檢查仍執行，但不代表各操作全部欄位及
subscription 已驗證。Fault／auth 與呼叫端 raw adapter 回應仍保有優先順序。
選項不寫入裝置 snapshot。PTZ auxiliary 保留 legacy command allowlist，但不完整
驗證 ProfileToken／欄位。Upload URI、subscription reference 與 timestamp 均為
fixture 資料，不代表服務可用或效果已完成。Capability 回應仍為 static fixture，
可能高估已建模行為；完整核對另列 W17。其他操作契約及部分建模效果仍須審閱。
詳見[政策檢查點](active/mock-fidelity-ack-policy-preflight_zh.md)
及 `tests/mock_ack_policy.rs`；這不是 hardware-effect 或 conformance 測試。

## 14. 擴充 mock

完整程序請參閱 `CLAUDE.md` 的 *Adding a new ONVIF service* 第 5a–5c 步。摘要如下：

1. 在 `src/mock/dispatch.rs` 的正確 `dispatch_*` arm 加入 action URI。
2. 在 `src/mock/services/<service>.rs` 加入 `resp_<operation>()` 或 `handle_<operation>()`。
3. 若操作同時存在於 Media1 與 Media2，必須讀寫同一份狀態。將狀態操作放在 `services/media.rs`，再由各服務輸出各自的 envelope。
4. 每個 `Set` 都必須在 `tests/mock_roundtrip.rs` 中宣告 `Works`、`Broken(audit §)` 或 `Static(audit §)`。
5. 每個接受 token 的操作都必須在 `tests/mock_token_discrimination.rs` 中宣告 `Discriminates` 或 `Blind(audit §)`，並指定兩個預載值不同的 token。

第 4、5 步不可省略；`Broken` 是可接受且可追蹤的狀態，缺少 row 則不可接受。路由會由 `mock_handles_every_action_the_client_can_send` 自動檢查；payload 不會自動驗證，因此新增 handler 時仍必須提供符合規範且有針對性的 response assertion。
