# oxvif 錄影按時間定位實作計畫

2026-11-16 規劃，2026-11-17 實作。走訪基準為 `master` 的 `026da13`。
R1–R3 為本次實作範圍，R4 實機互通性待驗收。

## 第一版範圍

呼叫端提供裝置、錄影 token 和絕對 UTC 時刻。oxvif 取得 Replay URI，建立一條
RTSP 1.0 session，以 TCP interleaved 接收一條正向的 H.264 錄影視訊軌。
先完成按時間起播，再用同一條 session 跳到第二個時間。回報要求的時間、實際
觀察到的錄影時間、錄影中斷的證據和關閉結果。

第一版交付 Replay 控制與帶時間資訊的 RTP 封包。播放器或 NVR 上層負責解碼、
顯示和封裝。反向回放、多軌同步、H.265、UDP／multicast、RTSP-over-WebSocket
及匯出留待各自有需求與實機證據時擴充。Rust API、腳本化 RTSP 測試和實機
驗收是本計畫的交付項目。CLI 和 OxDM 可各自接這個 API。

## 現有程式碼

| 入口 | 走訪結果與實作方向 |
| --- | --- |
| [Replay SOAP](../../src/client/recording.rs#L470)、[Session](../../src/session.rs#L1848) | `get_replay_uri` 序列化 `StreamSetup` 和錄影 token，回傳 URI 字串。Session 會從能力中的 Replay URL 呼叫，初始化時已有 `GetServices` 補缺機制。重用現有 SOAP 序列化和 Session 的服務解析。 |
| [ReplayServiceCapabilities](../../src/types/service_capabilities.rs#L838)、[EndpointPolicy](../../src/transport/endpoint.rs#L6) | 已解析 `RTP_RTSP_TCP`、反向播放、session timeout 範圍與 WebSocket URI。新流程以既有 HTTP(S) policy 驗證 SOAP 端點，再按 RTSP 自身的目的地規則檢查回傳的 URI。 |
| [基準 RTSP 探針](https://github.com/smiti1642/oxvif/blob/026da13/src/stream_probe.rs#L127)、[Connection](https://github.com/smiti1642/oxvif/blob/026da13/src/stream_probe.rs#L264)、[RTP 探測](https://github.com/smiti1642/oxvif/blob/026da13/src/stream_probe.rs#L543) | 現有程式能做 TCP／RTSPS、Digest、SDP、SETUP／PLAY／TEARDOWN，並辨識第一個匹配的 RTP 封包。`exchange` 等回覆時會略過 interleaved data。`is_rtp` 會跳過 RTP extension。Replay 需要持續分流、保留有效媒體，並讀取 Replay extension。 |
| [Talkback owner](../../src/stream_probe/talkback.rs#L82)、[Cargo features](../../Cargo.toml#L65) | Talkback 已用同一個私有連線，提供有限佇列、取消和獨立 TEARDOWN 時限。可沿用這些生命週期原則。首版另加選用 feature `recording-replay`，重用既有 RTSP 相依；預設 features 不額外啟動 Replay。 |
| [時間型別與時間軸](../../src/recording/timeline.rs#L25)、[錄影資訊](../../src/types/recording.rs#L145)、[Search owner](../../src/recording/search.rs#L351) | `RecordingTimestamp` 已可比較帶時區時間。摘要與 `RecordingInformation` 提供範圍，`media_attributes` 提供 track 資料，`event_search` 提供事件索引。這些可輔助選時段。已有錄影 token 的呼叫端可直接開始 Replay，且各段是否連續以 Replay 證據為準。 |
| [Mock Replay URI](../../src/mock/services/recording.rs#L455)、[RTSP 測試](../../tests/stream_probe.rs#L14) | 現有 Mock 回傳固定形式的 URI，驗證 SOAP 路由。RTSP 測試有獨立 loopback peer，能延伸成 Replay wire fixture，分別驗證控制與媒體。 |

## Rust API 與 session

下列名稱用來描述公開契約。實作時在 `recording::replay` 集中 Replay 型別，
把共用傳輸整理到私有模組，維持既有 `get_replay_uri`、`stream_probe` 和
`talkback` 的呼叫方式與結果。先以實作確實能保證的欄位定型。

| 介面 | 契約 |
| --- | --- |
| `OnvifSession::start_recording_replay` | 接收錄影 token、`ReplayRequest`、RTSP 憑證／CA 選項和取消訊號。驗證起點及選用終點，固定使用 `RTP-Unicast` 與 RTSP。成功回傳 `ReplaySession` 和 `ReplayStart`，失敗保留所處階段、已確認的證據及獨立清理結果。 |
| `ReplaySession::seek` | 在同一條已建立的 session 串行執行一次跳播，回傳該次的新 `ReplayStart`。跳播前已交付的封包仍屬於舊請求，成功後新呼叫只取得本次定位的封包。若送出後無法確認切換，回報狀態不明並停止交付。 |
| `ReplaySession::next` | 回傳有界的 RTP／錄影邊界觀察。公開選中視訊軌的 codec、payload type 和解碼所需的 SDP 參數，並提供封包與可讀到的媒體時間資訊。保留起播時已用來確認時間的首批封包，供呼叫端解碼。 |
| `ReplaySession::close` | 有界 TEARDOWN，重複呼叫回傳已有的清理結果。合作取消或可正常結束的失敗採同一條有界清理路徑。放棄 owner 時關閉本地連線，遠端清理結果維持未知，由 device session timeout 回收。 |

- 用現有 `RecordingTimestamp` 表達輸入時間，正規化為 RTSP `clock=` 的 UTC 緊湊
  格式，保留受支援的秒以下精度。正向有終點時須晚於起點。RTSP 操作逾時使用單調
  時鐘。WS-Security 時鐘偏移只供 SOAP 驗證。
- 單一 I/O owner 持有 socket、CSeq、session id 與媒體路由。定位與關閉走控制
  路徑，媒體走按封包數與位元組數限制的佇列。每次定位只允許一個控制請求在途。
  設定首播／seek、媒體閒置和清理各自的 deadline。依 `Session` timeout 與 Replay
  模式處理必要的 keep-alive。慢消費端觸及佇列上限時，回報下游跟不上並結束，
  控制與清理仍能完成。上限由 loopback 高流量測試驗證。
- 沿用 origin policy、憑證驗證、有限的認證重試與訊息大小上限。SOAP 自訂 transport
  照舊使用。RTSP 憑證與信任根由呼叫端明確傳入，區分 SOAP 成功與 Replay 的
  RTSP 認證。URI 限 RTSP(S) 與同裝置 host，SDP 控制 URI 限已驗證的 RTSP
  authority。HTTPS 裝置維持 RTSPS 要求，Basic 只走經驗證的 TLS。拒絕重導，
  不使用回傳 URI 內的 userinfo。錯誤與報告不帶完整 URI、query、帳密或原始 SDP。
- 型別化錯誤至少能區分缺 Replay 服務、SOAP 取 URI 失敗、目的地拒絕、RTSP
  認證／TLS、transport 或 codec 不支援、定位被拒、資料到達但無法驗證時間、
  等待逾時、消費端過慢和取消。主錯誤與 TEARDOWN 結果分開。若定位交換中斷
  且無法可靠關聯後續回覆，關閉本地連線並報清理未確認。

## 如何確認起播與錄影缺口

1. 透過回傳的 URI 建立 session，依 SDP 分開解析 track URI 及 aggregate URI。
   `SETUP`、`PLAY` 附 `Require: onvif-replay`，`PLAY` 以
   `Range: clock=…` 指定範圍。首版採伺服器依錄影節奏送流的正向模式。
   已有 session 的定位另送 `Immediate: yes`。SETUP 時核對 session id 和
   interleaved transport；後續回覆核對 CSeq，以及有回報時的 session id。
   處理 RTSP 控制回覆與 RTP／RTCP 交錯。
   [ONVIF Streaming §6.4–6.5、§6.10](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec.pdf)
2. 跳播時停發緩衝中的舊位置資料，為新請求建立獨立的媒體交付世代。用完整的
   RTSP PLAY CSeq、Replay extension 裡的低位元組及新位置首封包的 `D` 標記做關聯。
   在 RTSP 回覆前到達的封包可暫存於同一有界預算內，確認回覆與新邊界後才交付。
   忽略已能識別的舊位置資料。若低位元組回繞後無法可靠區分，讓此次定位失敗，
   由呼叫端明確重新開 session。無法辨識新的媒體邊界時維持未確認。
   [ONVIF Streaming §6.3、§6.10](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec.pdf)
3. 解析選中 track 的 RTP Replay extension `0xABAC`，驗證長度、協商的 channel、
   payload type、SSRC 一致性及封包界線。讀出 access unit 的 NTP 時間戳並轉成 UTC。
   用明確的錄影時間上下文解 NTP era，測跨 2036 邊界；無法消歧時回報時間未知。
   `ReplayStart` 分欄記錄
   要求的起點、可取得的 RTSP Range 回應、實際第一個可關聯 access unit 的 UTC，
   以及該點是否由裝置標為 clean point。起點以 RTP Replay 時間證據確認，
   RTSP 狀態、Range 回應、RTP 相對時鐘及 RTCP 時間各有不同用途。解碼器的第一個
   顯示畫面另外由上層測量。
   [ONVIF Streaming §6.3、§6.11](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec.pdf)
4. 若要求時間落在有錄影的區段，回放可從該時刻或前一個同步點開始；若落在
   缺口，正向回放從後面區段的第一個同步點開始。回報「實際起點較晚」及偏差。
   在同一有效播放位置上，觀察到上一段 `E` 和其後的時間邊界，或有獨立驗證的
   區段證據時，才回報已確認的錄影缺口。只有較晚的時間、單獨 `E`、`D`、
   序號跳動、連線中斷、閒置或摘要的首末時間時，保留各自含義。用 `T` 協助
   辨識終點，並容許持續錄影時暫時沒有新封包。
   [ONVIF Streaming §6.3、§6.5.4、§6.8–6.9](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec.pdf)

## 裝置相容性

| 裝置行為 | 首版預期 |
| --- | --- |
| Replay 缺失、Search 缺失或沒有錄影 | 分開呈現。已有錄影 token 時能獨立試 Replay，Search 用於找錄影和提供輔助線索。 |
| `RTP_RTSP_TCP` 為真、未知或明確為假 | 記錄服務宣告與實際 SETUP 結果。能力欄位缺漏可做有界探測，只有需要非首版 transport 時回報不支援。 |
| RTSP 551、其他拒絕、接受 PLAY 但忽略 Range 或 Immediate | 區分協議明確拒絕與定位未驗證。某個位址可以整筆播放，仍須另外通過 UTC 定位驗收。 |
| 回傳跨 host 的 URI、錯誤埠、憑證無效或 HTTP／RTSP 權限不同 | 保留各自錯誤。若 NVR 需要跨 host，先取得脫敏實機證據，再設計明確 opt-in 的 RTSP 目的地規則，首版測拒絕。 |
| 韌體缺 Replay extension、無法確認新位置或偶爾只對齊關鍵畫面 | 前兩種回報時間未確認。能驗證時間的同步點偏移照實回報。評估相容特例時，縮小到有證據的裝置行為並加 fixture。 |

## 交付順序與驗收

| 階段 | 完成條件 |
| --- | --- |
| **R1 共用傳輸與按時間起播** | 重構後既有 live probe 與 talkback 測試通過，新增 Replay wire fixture。測 UTC offset、日期與小數秒、SDP track／aggregate URI、SETUP／PLAY headers、RTSPS CA、Digest、各層拒絕與一次 TEARDOWN。腳本化 SOAP 回傳測試 RTSP peer 的 URI，涵蓋完整入口。 |
| **R2 同 session 跳播與起點證據** | 測出播放 200 但時間未知、回覆前後夾雜舊 RTP、新位置封包提前到達、CSeq 低位元組回繞、header 截斷與 NTP era。確認首包仍交付、成功 seek 後只交付可歸屬的新世代，並分開報告目標時間與實際起播時間。 |
| **R3 缺口、資源與回歸** | 同一時間軸 fixture 包含連續錄影、兩段間真缺口、結尾 E/T、單純 discontinuity 和傳輸中斷，各自得到不同結果。測消費端塞滿、keep-alive、取消會話、逾時和清理失敗，驗證控制不被媒體佇列拖住。跑 Windows／Linux 本地回歸、預設與 Replay feature 組合、全功能及 Rust 1.88 編譯檢查。 |
| **R4 實機** | 至少找一台支援 Replay 的攝影機或 NVR，可用帶時間標記的測試錄影。定位到區段內、已知缺口、邊界，並在同一 session 重複跳播。用可解碼樣本或裝置端錄影時間標記獨立核對第一畫面，記錄 RTSP 回應、RTP UTC、實際首個可解碼畫面及差值。另找一台不同廠牌／韌體的支援裝置和一台不支援的對照，逐項記錄成功、未驗證或拒絕。 |

實機記錄至少包含廠牌／型號／韌體、Search／Replay 服務、實際 transport、編碼、
認證與 TLS 模式、請求時間、觀察時間、相容性分類及 TEARDOWN 結果。共享證據使用
脫敏的 wire 片段與時間對照，測試影像用非敏感場景。用戶端測試能證明封包與控制
契約，裝置端錄影是否能正確定位以實機證據判定。

可沿用 [RTSP loopback 測試](../../tests/stream_probe.rs)、
[Search 生命週期測試](../../tests/recording_search_lifecycle.rs)、
[時間軸 SOAP fixture](../../tests/extended_onvif.rs#L506) 和
[I07–I10 既有互通性紀錄](onvif-interoperability-plan_zh.md#i07i10-交付證據)。
Replay URI 契約見 [ONVIF Replay Control §5.1](https://www.onvif.org/specs/srv/replay/ONVIF-ReplayControl-Service-Spec.pdf)。

## 實作入口

以 `recording-replay` feature 啟用 [Rust API](../../src/recording/replay.rs)。成功起播
後，`ReplaySession::next` 仍會交付用來確認起點的封包。應用程式可以在同一個
session 呼叫 `seek`，再從新的媒體佇列讀資料。若一次定位的播放 CSeq 低位元組
在該連線已用過，加上缺少可可靠區分舊封包的證據，該次要求會回報位置未確認；
上層可以重新建立連線。收到終結標記後仍可在閒置期限內跳播。

共用的 [RTSP 層](../../src/rtsp.rs) 保留 live probe 和 Talkback 的操作契約，
Replay 使用不同的 interleaved data 處理。錄影缺口以 [RTP 證據](../../src/recording/replay/media.rs)
確認，整合驗收使用 [腳本化 SOAP 和 RTSP peer](../../tests/recording_replay.rs)。

## R1–R3 本地驗收紀錄

- Windows workspace 全功能測試 1,532 項通過，預設功能測試 1,377 項通過。
- WSL workspace 全功能測試 1,533 項通過。僅啟用 `recording-replay` 時，
  7 項 Replay 整合測試全數通過。
- Windows 的 Rust 1.88 workspace 全功能編譯、嚴格 Clippy 與格式檢查均通過。
- Loopback 涵蓋同 session 跳播、舊封包隔離、位置未確認、錄影缺口、傳輸中斷、
  RTSPS 信任、取消及清理。慢消費端案例在 256 個 RTP 封包早於控制回覆時仍
  確認 TEARDOWN。實機錄影與首個可解碼畫面留待 R4。

## R4 實機盤點 2026-11-18

- 使用測試設定中每台裝置對應的帳號，盤點 28 組主要連線和兩個另列的資料埠。
  六組連線宣告 Recording、Search 和 Replay，候選版 SDK 也解析出這三項服務。
- 其中五組在錄影清單、搜尋或時間軸查詢回覆 `NotAuthorized`。
  分別以 HTTP Digest 和 WS-Security UsernameToken 重試錄影清單，仍未取得錄影。
  另一組的 `GetRecordings` 回傳空清單，搜尋進入取結果階段時回傳 SOAP Fault。
- 沒有取得可用的錄影 token 或可播放的時間區間。實機的 UTC 起播、同一 session
  跳播和首張可解碼畫面時間留待取得有 H.264 錄影且允許回放的攝影機或 NVR 後驗收。
