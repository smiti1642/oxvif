# Event pull 一致性批次 EP1

[English](mock-fidelity-event-pull.md) | [繁體中文](mock-fidelity-event-pull_zh.md)

狀態：有限 EP1 一致性批次已完成（DONE）。基準 `cb25276`，2026-09-22。W15 與 W16 一致性。

| 章節 | 用途 |
| --- | --- |
| [操作卡](#操作卡) | W01 審查與選定缺陷 |
| [驗收](#驗收) | 有限證據及排除項目 |

## 操作卡

ID `events.PullMessages`；Action 為
`http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/PullMessagesRequest`。
Client／session 輪詢經 `dispatch_events` → `events::resp_pull_messages` →
`SharedState::modify_returning`、`io_event_response`、`soap` 與時鐘 helper。
共用 request boundary 後只傳 state；Timeout／MessageLimit 尚未建模。
目前每個 mock instance 共用一份 filter／queue／counter，並非每個 subscription。
不變更公開 request context 或 endpoint API。

EP1-QUEUE：pending IO 事件繞過 filter，但合成事件會套用。
EP1-SNAPSHOT：取 queue、更新 sequence、讀 filter 分別鎖定，on-change hook
可在 render 前改變 filter；IO token 輸出亦缺 XML escape。這些是來源確認的
內部一致性缺陷，不是完整 W15 契約。

目標是在單次 transaction 取 queue／推進 counter／取得 filter 快照，解鎖後
render。被過濾的 queued event 消耗一個 slot 並回空回應，沿用合成事件政策；
queue 保持優先，每次不任意清空 backlog。每次 pull 呼叫 hook 一次；reentrant
修改不能改變該 pull 已取得的 filter。u64 counter 用完時循環，不 panic。
Token escape；既有 lexical topic matching 保留。

外部參考：`packaging/schema-sources.json` 固定的 event WSDL 與 SOAP 1.2 資源；
[Core](https://www.onvif.org/specs/core/ONVIF-Core-Specification.pdf) 的事件模型
是較廣契約。本批維持立即回單筆的模型，不宣稱完整時間或 TopicExpression 語意。
輸出 `RelayOutput` 與宣告 `Relay` 不一致另行追蹤，不在此推定規範名稱並改名。

## 驗收

C01／C02：共用 Action／operation boundary 不變，沒有新增 reader。
C03／C04／C08／C09：沿用 parser／auth／limit／fault 控制，不新增欄位或一般 fault
政策；Timeout／MessageLimit 驗證未完成。
C05／C06：兩個 instance、queue 過濾及順序、sequence、其他狀態保持、單次 hook、
reentrant filter 快照、併發各取不同 queue 項目。
C07：解碼後 token 與選定完整回應的外部驗證。
C10／C11：維持立即模型及 typed parser，涵蓋 HTTP／in-process。
C12：敏感度實驗、精確還原、五道 gate 與外部 corpus。
生命週期、每份 subscription 的 identity／filter／queue、renew／expiry／termination、
topic namespace 語意與 replay retirement 仍未完成。

## 結果

Queue 選取、counter 與 filter 快照共用單一 transaction。兩種 transport 驗證
過濾 IO 消耗、escape input identity、typed notification、sequence、每次 pull
一次 hook 及 instance 隔離。四個 worker 併發消耗 32 筆，各一次且無重複。
Reentrant hook 在 commit 後修改現行 filter，測試確認已選事件仍用原快照；
u64 邊界不再 panic。

移除 IO filter 並將 synthetic 快照替換為即時讀取後，未篩選全功能／no-fail-fast
實驗讓兩種 transport、reentrant 與 corpus driver 均失敗，之後精確還原。
四組新 client exchange 涵蓋 IO／synthetic 的 include／exclude；Action 與
payload 對應明確處理 Events Request suffix 及 port namespace 的差異。

W15／W16 仍是 PARTIAL；尚未建立獨立 subscription、到期行為、Timeout／
MessageLimit 或修正宣告 Relay topic 命名。

最終驗收：固定 strict Xerces 通過 178 XML instance／89 exchange／51 operation（68 成功、21 既有 fault）。Workspace 全功能 1,351、預設 1,235 通過，各七項 ignored、43 suites；兩種 Clippy、格式、inventory 控制及文件連結通過。新 snapshot example 另行測試，不加入 workspace 總數。
