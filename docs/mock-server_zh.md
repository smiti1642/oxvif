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
| [7. 操作參考](#7-操作參考) | stateful、static 與不支援的操作 |
| [8. 實作範例](#8-實作範例) | 代表性請求與回應 |
| [9. 錯誤模型](#9-錯誤模型) | SOAP fault 結構與代碼 |
| [10. Fault injection 與控制 endpoint](#10-fault-injection-與控制-endpoint) | 強制產生 transport 或 protocol failure |
| [11. 調整裝置](#11-調整裝置) | 自訂狀態與 responder |
| [12. 保證範圍](#12-保證範圍與對應測試) | 各項真實度聲明的驗證測試 |
| [13. 已知限制](#13-已知限制) | 刻意簡化或尚未實作的項目 |
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

| Action prefix | Dispatcher | 操作數 |
|---|---|---|
| `…/ver10/device/wsdl/` | `dispatch_device` | 38 |
| `…/ver10/deviceio/wsdl/` | `dispatch_device_io` | 1 |
| `…/ver10/media/wsdl/` | `dispatch_media` | 32 |
| `…/ver20/media/wsdl/` | `dispatch_media2` | 26 |
| `…/ver20/ptz/wsdl/` | `dispatch_ptz` | 27 |
| `…/ver20/imaging/wsdl/` | `dispatch_imaging` | 8 |
| `…/events/wsdl/` 或 `docs.oasis-open.org/wsn/` | `dispatch_events` | 8 |
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

## 4. 驗證

驗證預設關閉，因此無憑證的 client 可直接連線。可使用 `MockTransport::with_auth()` 或 `MockServerBuilder::enforce_auth(true)` 啟用。

啟用後會驗證 WS-Security **`PasswordDigest`**（`src/mock/auth.rs`），計算方式與實機相同：`Base64(SHA1(nonce + created + password))`。

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

### 6.3 Profile

| Token | Name | Fixed | Source cfg | Encoder cfg | PTZ cfg | Audio cfg |
|---|---|---|---|---|---|---|
| `Profile_1` | `mainStream` | 是 | `VSC_1` | `VEC_1` | `PTZConfig_1` | `ASC_1` + `AEC_1` |
| `Profile_2` | `subStream` | 否 | `VSC_1` | `VEC_2` | `PTZConfig_1` | 無 |
| `Profile_3` | `mainStream2` | 是 | `VSC_2` | `VEC_3` | `PTZConfig_2` | 無 |
| `Profile_4` | `subStream2` | 否 | `VSC_2` | `VEC_4` | 無 | 無 |

`fixed="true"` 的 profile 不允許刪除，會回傳 `ter:DeletionOfFixedProfile`。`Profile_4` 刻意不繫結 PTZ configuration，用於測試不具 PTZ 能力的 profile；任何以該 profile 執行的 PTZ 操作都會產生 fault。

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

Device 共 38 項操作。裝置資訊、日期時間設定、hostname、NTP、DNS、scope、user、network interface/protocol/gateway、discovery mode、relay 與 storage 均為 ●。`GetCapabilities`、`GetServices` 與 `GetServiceCapabilities` 使用 static service metadata；系統維護類操作會回覆已接受，但不模擬後續行為。

DeviceIO 的唯一操作是 `GetDigitalInputs`，由 REST simulator 驅動。其 endpoint 為 `{base}/onvif/deviceio`；action segment 使用小寫 `deviceio`，element namespace 則為 `…/ver10/deviceIO/wsdl`。

### 7.2 Media1（32 項操作）

Profile、video source/configuration、video encoder、OSD 與 audio catalog/configuration 均由共享狀態支援。`GetStreamUri` 與 `GetSnapshotUri` 對所有 profile 回傳同一組 canned URI；`GetOSDOptions` 與 `GetServiceCapabilities` 為 static。`SetAudioEncoderConfiguration` 若缺少規範要求的 `Multicast` 或 `SessionTimeout` 會拒絕 request。

### 7.3 Media2（26 項操作）

Media2 與 Media1 共用 profile、video、audio 狀態，並另提供 metadata。`GetMetadataConfigurations` 的 `ConfigurationToken` 是 filter；無結果時回傳空 list，而 `GetMetadataConfigurationOptions` 的未知 token 會 fault。`GetVideoSourceModes` 為已宣告 stub；`SetVideoSourceMode` 一律回傳 `ter:ActionNotSupported`，不會宣稱已儲存未建模的 sensor mode。

### 7.4 PTZ（27 項操作）

所有 per-profile 操作都要求 `ProfileToken`。缺少 token 時回傳 `env:Sender`；不存在的 profile 回傳 `ter:NoProfile`；未繫結 PTZ configuration 的 profile 回傳 `ter:NoConfig`。Move 為立即完成，不模擬移動時間。以 node 或 configuration token 定址的 getter/setter 對未知 token 會 fault；`GetCompatibleConfigurations` 對不具 PTZ 能力的 profile 回傳空 list。

### 7.5 Imaging、Events、Recording、Search 與 Replay

- Imaging 的七項影像操作依 `VideoSourceToken` 存取狀態，`GetServiceCapabilities` 為 static。
- Events 的 `CreatePullPointSubscriptionRequest` 會儲存 topic filter；`PullMessagesRequest` 會輸出週期性 synthetic stream 與 REST 注入的 I/O event，並遞增 `event_seq`。其餘操作為 static。
- Recording 的 recording、track 與 job 操作均由狀態支援；刪除 recording 會一併刪除所屬 job。
- `FindRecordings` 只提供單一 search token，不模擬 cursor；`GetRecordingSearchResults` 讀取目前 recording list。
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

Fault 使用 SOAP 1.2 `<s:Fault>`，包含 `Code/Value` 與 `Reason/Text`（`helpers::resp_soap_fault`）。HTTP status 維持 **200**；fault 由 response body 傳輸，符合 ONVIF 裝置的常見行為。

每個 reason 都帶有 operation tag 與唯一 numeric id，例如：

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

已知偏差：`Code/Value` 內的 `ter:` 與 `env:` 是 QName，但 fault envelope 未宣告這兩個 prefix。Element prefix 均已正確 binding；只有會自行解析 fault-code QName 的 client 會受到影響。此項目前記錄為設計議題，因為許多實機也會輸出相同形式。

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
| 每個 `Set` 都能 round-trip，或明確宣告為 static | `tests/mock_roundtrip.rs` |
| 每個接受 token 的操作都能區分 token，或明確宣告為 blind | `tests/mock_token_discrimination.rs` |
| Media1 與 Media2 的共享狀態保持一致 | `tests/mock_media1_media2_agree.rs` |
| Per-sensor 回應確實不同 | `tests/mock_multi_sensor.rs` |
| End-to-end flow | `tests/mock_workflow.rs` |
| XML namespace、name、cardinality 與 sequence order 符合 ONVIF schema | `tests/mock_schema_shape.rs`；限制如下 |

`tests/mock_schema_shape.rs` 標記為 `#[ignore]`，執行時由 `$OXVIF_ONVIF_SCHEMA` 讀取 repository 外的 ONVIF schema。若輸出 `SKIPPED`，代表未執行 schema 驗證。0.15.0 的十項計數均為 0，但這不等同於宣告 mock 已通過 ONVIF conformant 認證；`xs:any` 與全 optional child 等 schema 特性仍可能掩蓋語意錯誤。

目前 49 組 round-trip 全數為 working，無 static 或 known-broken；34 組 token row 中 28 組可區分、6 組明確標記為 blind。測試表的每個 row 都宣告意圖，避免已知限制演變成未追蹤的永久盲點。

---

## 13. 已知限制

### 13.1 已宣告的 static read stub

`tests/mock_roundtrip.rs` 已無 `Static` row，每個 `Set` 都會 round-trip。仍為 static 的 read family 包括：

- Media2 `GetVideoSourceModes`：所有 `VideoSourceToken` 都回傳同一個 `Mode_1`。
- 兩種 media service 的 `GetStreamUri` / `GetSnapshotUri`：所有 profile 都回傳同一 URI。
- Media1 `GetOSDOptions` 與 Media2 `GetVideoEncoderInstances`。

### 13.2 型別真實度缺口

- ONVIF schema 的 `PTZConfiguration` 可含 `MoveRamp`、`PresetRamp`、`PresetTourRamp`，`PTZNode` 可含 `GeoMove`；oxvif public type 目前未解析這四個 attribute。
- `VideoSourceMode/@Enabled` 未包含在 `oxvif::VideoSourceMode` 中，因此無法透過 getter 驗證 active mode；補足此能力需要 public API 變更。

### 13.3 已記錄的簡化

- 不模擬移動時間；PTZ move 立即更新 position，`MoveStatus` 固定為 `IDLE`。Monostable relay 也不會自動還原，請使用 REST pulse hook。
- 不模擬 search cursor；`FindRecordings` 只提供一個 token，結果一次回傳完整目前清單。
- `Bounds/@x` 與 `@y` 會由 wire 讀取後捨棄；`VideoSourceConfigEntry` 只建模 size。
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

## 14. 擴充 mock

完整程序請參閱 `CLAUDE.md` 的 *Adding a new ONVIF service* 第 5a–5c 步。摘要如下：

1. 在 `src/mock/dispatch.rs` 的正確 `dispatch_*` arm 加入 action URI。
2. 在 `src/mock/services/<service>.rs` 加入 `resp_<operation>()` 或 `handle_<operation>()`。
3. 若操作同時存在於 Media1 與 Media2，必須讀寫同一份狀態。將狀態操作放在 `services/media.rs`，再由各服務輸出各自的 envelope。
4. 每個 `Set` 都必須在 `tests/mock_roundtrip.rs` 中宣告 `Works`、`Broken(audit §)` 或 `Static(audit §)`。
5. 每個接受 token 的操作都必須在 `tests/mock_token_discrimination.rs` 中宣告 `Discriminates` 或 `Blind(audit §)`，並指定兩個預載值不同的 token。

第 4、5 步不可省略；`Broken` 是可接受且可追蹤的狀態，缺少 row 則不可接受。路由會由 `mock_handles_every_action_the_client_can_send` 自動檢查；payload 不會自動驗證，因此新增 handler 時仍必須提供符合規範且有針對性的 response assertion。
