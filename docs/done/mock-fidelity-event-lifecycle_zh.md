# Events pull-point 生命週期 EP2

[English](mock-fidelity-event-lifecycle.md) | [繁體中文](mock-fidelity-event-lifecycle_zh.md)

狀態：DONE（有限 EP2 批次）。2026-09-23、基準 `6ec107f`，修改前建立 W01。
負責項目：[執行佇列](../active/autonomous-completion-schedule_zh.md) Q01。

| 章節 | 用途 |
| --- | --- |
| [操作卡](#操作卡) | 身分、現有路徑與目標效果 |
| [契約](#契約) | 有限模型與相容性 |
| [驗收](#驗收) | C01–C12 與獨立證據 |

## 操作卡

已審閱 [Core 26.06 §9.1](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf)。
依 packaging/schema-sources.json 識別的 event.wsdl、b-2.xsd、r-2.xsd 在 checkout
外核對，不保存 schema 衍生 fixture。Client／wrapper 位於 client/events.rs／session.rs，
共用 events types 解析 subscription reference／time 與 notification。

| Ledger ID／完整 Action 後綴 | 目前程式／輸入 | 目標 state／fault／output |
| --- | --- | --- |
| events.GetEventProperties／EventPortType/GetEventPropertiesRequest | dispatch_events → events::resp_event_properties；靜態 topic／dialect | 保留靜態讀取，說明 filter 子集 |
| events.GetServiceCapabilities／EventPortType/GetServiceCapabilitiesRequest | events::resp_event_service_capabilities；靜態上限 4 | 落實四筆 pull point；不宣稱 push／policy／persistent storage |
| events.CreatePullPointSubscription／EventPortType/CreatePullPointSubscriptionRequest | events::resp_create_pull_point_subscription 全域擷取 TopicExpression、覆寫單一 filter、固定 endpoint、忽略 lifetime／policy | Scoped filter／time、唯一 endpoint、private volatile subscription；先驗容量才修改 |
| events.PullMessages／PullPointSubscription/PullMessagesRequest | events::resp_pull_messages 忽略 Timeout／MessageLimit／endpoint，消耗 instance queue／counter | Scoped 有界參數，只選有效存活 endpoint；独立 queue／filter／counter、時間一致 |
| events.Renew／SubscriptionManager/RenewRequest | 預設 policy 拒絕，opt-in events::resp_renew 回靜態時間 | 原子變更存活訂閱到期；未知／過期／時間拒絕不改 runtime；明確 receipt opt-in 保留 |
| events.Unsubscribe／SubscriptionManager/UnsubscribeRequest | 預設拒絕，opt-in resp_empty | 只移除指定訂閱／queue；未知／重複拒絕；receipt opt-in 保留 |
| events.SetSynchronizationPoint／PullPointSubscription/SetSynchronizationPointRequest | 預設拒絕，opt-in resp_empty | 完整 property 初始化語意另做；不宣稱已同步 |
| events.Subscribe／NotificationProducer/SubscribeRequest | 預設拒絕，opt-in resp_subscribe | Push 尚未建模，receipt opt-in 保留 |

EventPortType／PullPointSubscription Action 前綴為
`http://www.onvif.org/ver10/events/wsdl/`；SubscriptionManager／NotificationProducer
為 `http://docs.oasis-open.org/wsn/bw-2/`。Operation namespace 分別為 events namespace
及 `http://docs.oasis-open.org/wsn/b-2`。由 transport URL 選訂閱，不使用 Header／body 誘餌
或最後建立的全域 ID。

## 契約

- Built-in chain 私下傳遞 endpoint；保留 RequestCtx 四個公開欄位、custom responder
  順序及 raw／fault／auth 優先序。
- MockState 私有 runtime 保存訂閱，不放進持久化 DeviceState；新 instance 沒有訂閱，
  instance 內 ID 不重用。既有 event 欄位保留作 synthetic IO ingress；成功生命週期操作
  在所有鎖釋放後通知。
- 上限四筆存活訂閱，每個 queue 有明確有限容量；慢 consumer 不消耗別人的事件。
  所有拒絕條件先驗證，才排空 ingress 或修改 queue／counter。
- 支援的 topic prefix 依真正 namespace scope 解析；其他 dialect／content filter／policy／
  extension 明確拒絕，不默默把任意 XPath 當字串匹配。
- 有界相對 lifetime 及支援的絕對 UTC 時間；private 可控時鐘做確定性測試。
  PullMessages 立即回 queued 或一筆 synthetic event，不超過要求上限；明示不做真實 pacing
  與未建模的合法 lexical form。
- 未知／過期 reference、錯誤 time／limit、容量與未支援欄位有明確 structured fault；
  實際 Fault capture 外部驗證，mock 自訂政策不宣稱為規範 Fault。
- Built-in replay 不得用靜態 recording 回應存活 pull point；standalone replay 政策保留。

## 驗收

C01–C04：精確 Action／namespace／endpoint、scope、重複／缺少欄位、Header 誘餌、
scalar／attribute 限制及 escape。C05–C06：兩份不同 filter 訂閱、fan-out、慢 consumer
隔離、唯一 ID／容量、expiry／renew／unsubscribe、拒絕 snapshot／hook、reentrant／instance。
C07–C08：完整 typed 值、structured Fault 與實際 capture 外部驗證。C09 沿用 auth／resource。
C10–C11：capability、明確 receipt policy、replay、公開建構相容。C12：完整未過濾擾動一次、
精確還原、五項 workspace gate、適用 strict rustdoc、雙語清冊／連結檢查。

完整 topic／property sync、push 傳送及實體事件 timing 仍是 W15 的獨立工作，不在本批假裝驗收。

## 已實作模型與證據

上方工作卡保留修改前基準。正式 handler 現位於 services/events/lifecycle.rs，
已移除只供舊測試使用的重複模型，將 assertion 遷移到正式路徑。模型上限四份存活
訂閱、每 queue 128 件、lifetime 1–3600 秒、立即 Pull timeout 0–60 秒；接受整數 PT
組合及精確 UTC。小數／calendar time、policy、完整 XPath／content filter 明確拒絕。
未知／過期、時間與溢位用 Sender/mock:RequestPolicy，容量用 Receiver/mock:RequestLimit，
未建模設定用 Sender/mock:UnmodeledEffect。完整規範 WSNT Fault detail 不屬此有限批次。
event_filter 保留來源相容但不再控制訂閱；event_seq 仍累計合成選取並循環。
訂閱 runtime 是 volatile，不持久化到 DeviceState。

EP2-01：QName content scope 檢查發現 client 的 tns1 未宣告，SoapEnvelope 已補標準
topic namespace；Relay 通知改用既有公告的 Relay path。Capability 不再宣稱 policy／
push producer，MaxPullPoints=4 與實際上限一致。

mock_event_lifecycle 驗證兩個 transport、filter／fan-out、未知／malformed／時間拒絕
後 ingress／hook 保留、並行容量與 ID、選擇性移除及有實際 recording 的 fallback。
lifecycle 單元測試涵蓋可控 expiry／絕對 renew、ID 耗盡、calendar 算術、prefix 遮蔽、
慢 consumer 溢位與 commit／counter wrap 後 callback reentrant 移除。舊 event_pull／
workflow 測試已改為使用真實訂閱。

獨立證據：108 exchanges／216 XML／59 Actions 通過固定離線 Xerces XSD 1.1。
其中 13 exchanges 覆蓋 create、queued／filter／synthetic pull、renew、unsubscribe
及未知 endpoint Fault。Exporter 新增 Events portType Action 到 payload namespace
映射；第一輪正確拒絕原本不完整的推導映射。固定 Python 環境下七項 verifier qualification
通過。這是本機結構證據，不代表整服務語意或 hosted CI 驗收。

完整未過濾 workspace all-feature 聯合擾動 exit 101，四個 target 的十項測試失敗，
涵蓋 expiry、QName scope、filter、capacity、lifecycle、queue、capture；兩份修改
原始碼逐 byte 還原。移除／replay 擾動在聯合案例被其他拒絕遮蔽或屬冗餘保護，
不宣稱各 switch 單獨敏感度；一般成功／拒絕控制驗證交付行為。

最終本機 gate：fmt、兩種 workspace/all-target Clippy、workspace 測試均通過（all-feature 1368／default 1249）。Strict rustdoc 兩種模式通過；inventory 為 159 routes／161 Action sites／162 reader occurrences，連結檢查通過。W15 仍保留明列的未涵蓋工作。
