# 基礎多攝影機 Mock 計畫

[English](mock-fleet-basic-plan.md) | [繁體中文](mock-fleet-basic-plan_zh.md)

狀態：施工中，2026-09-12 授權執行。首階段為供 VMS 使用的簡易多台啟動與設定。
B1 的 init／check 與三個針對性設定測試通過。B2 HTTP 服務、明確網路設定、
成員隔離及可等待的清理通過五個 Fleet 測試與針對性 Clippy；B3 完成前共用
探索仍明確回報尚不可用。不宣稱已納入發布或通過 64／256 台穩定性驗證。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | 第一階段及排除項目 |
| [操作流程](#操作流程) | 擬議命令與設定 |
| [執行規則](#執行規則) | 身分、網路與失敗處理 |
| [施工批次](#施工批次) | 檔案、交付項目及完成條件 |
| [驗收](#驗收) | 有限自動化及 VMS 檢查 |
| [決策](#決策) | 預設方向及須再次討論的條件 |

## 範圍

提供一個持續執行的前景程序，同時啟動可獨立設定的虛擬攝影機。重用 `MockServer`
與 `Fleet`，不建立程序管理服務，也不為每台設備建立獨立 OS 程序。預設四台，
允許明確設定至 256 台，並回報實際成功啟動數量。

包含基本共用 WS-Discovery，讓 VMS 不必逐台手動輸入網址。預設僅提供 loopback
HTTP 且不啟用探索；LAN HTTP 與 multicast 公告須明確設定。

排除 RTSP／影像產生、長時間壓力測試編排、自動故障情境、熱重載、管理 TUI、
主機 IP alias／防火牆修改、容器及新發布的 crate。Metamorph 多品牌混合屬後續
階段：保留逐台設定邊界，但不先新增未使用的 fixture 選項或宣稱品牌模擬完成。
既有 HTTP Digest 與 WS-Discovery 比對限制須維持明確揭露，不擴張為協定相容性聲明。

## 操作流程

新增由 `mock-server` feature 控制的長駐 `mock_fleet_serve` 範例，保留既有
`mock_server` 及短期執行的 `mock_fleet` 範例行為，不將 Mock 服務相依套件加入
已安裝的 `oxvif` CLI。

擬議命令，**目前建置尚未提供**：

```sh
cargo run --example mock_fleet_serve --features mock-server -- init lab.toml --count 4 --base-port 18080
cargo run --example mock_fleet_serve --features mock-server -- check lab.toml
cargo run --example mock_fleet_serve --features mock-server -- serve lab.toml
```

`init` 產生可編輯、具有版本的 TOML manifest，不覆寫既有檔案，並產生穩定且
不同的 UUID、序號、名稱及連續埠號。`--count 64` 或 `256` 僅改變產生數量，
不代表已驗證容量。`check` 驗證設定及引用的狀態檔，不繫結埠或傳送 probe。
`serve` 列出設備 ID／URL 與探索狀態後持續執行，Ctrl+C 關閉全部自有 listener。

待實作的 manifest 契約：

| 位置 | 欄位／語意 |
| --- | --- |
| 根層 | `schema_version = 1`、`bind_ip`（預設 `127.0.0.1`）、選用的明確 `advertise_ip`、`discovery`（預設 false），啟用探索時須提供 `discovery_interface` |
| 每個 `[[devices]]` | 唯一 `id`、持久保存的 `uuid`、固定 `port`、`name`、`manufacturer`、`model`、`serial_number`，以及選用的 `scopes`、`state_file` |
| 狀態來源 | 選用既有 `DeviceState` TOML，路徑相對於 manifest；manifest 的身分欄位覆蓋載入的身分，並明確記載優先順序 |

拒絕未知欄位、不支援的版本、空白／重複身分、重複／超出範圍的埠，以及無效的
引用狀態檔。不得將格式錯誤的狀態檔默默換成出廠預設。明列的設備項目為唯一
啟動依據，不再建立第二套 count／template 展開機制。

設定在啟動時生效。執行期間 ONVIF 寫入僅影響該設備的記憶體狀態；重啟時重新
載入 manifest／狀態檔。第一階段 Fleet 不回寫來源設定或狀態檔。既有單台持久化
行為不變；日後新增自動回寫須另行設計。

## 執行規則

- 為 `MockServerBuilder` 增加相容的 bind／advertise 設定，保留 loopback 預設。
  Fleet 初版使用同 IP、不同埠；公告可到達的明確位址，不公告 `0.0.0.0` 或其他
  主機無法使用的 loopback。
- LAN 存取須明確指定非 loopback `bind_ip`。Wildcard 繫結須提供明確
  `advertise_ip`；探索須明確指定本機 IPv4 multicast 介面。衝突組合須提供
  可操作的錯誤，不自動改防火牆、設定 IP 或猜測網卡。
- 這是測試服務，不是正式環境的認證邊界。保留既有測試認證行為，啟動時列出
  實際認證狀態；不輸出密碼，也不將未認證的 LAN 暴露描述為安全部署。
- 一個 responder 持有 UDP 3702，公告所有已啟動成員。不建立 256 個 listener，
  不依賴 socket reuse。逐台隔離 UUID、HTTP 狀態及回應；回應須關聯至 probe，
  限制回應大小及工作量，不將全群設備拼成單一過大的 datagram。
- 修改探索前，對照官方 ONVIF／WS-Discovery 資料核對受影響的回應、身分、type
  及 scope 處理。保留並揭露既有限制，或明確拒絕未支援比對，不默默擴張相容性聲明。
- 啟動前驗證完整 manifest。任一要求的 HTTP 埠或探索服務啟動失敗時，關閉已
  建立的自有 listener 並回傳非零退出碼，不將部分成功誤報為完整成功，也不終止
  占用埠的其他程序。
- 僅公告已可接受 HTTP 請求的端點；關閉 Fleet 時停止公告。不宣稱 Hello／Bye、
  動態成員或完整生命週期相容，除非確實實作並另行驗證。

## 施工批次

| 批次 | 主要檔案／交付項目 | 完成條件 |
| --- | --- | --- |
| B1 設定與啟動器 | 新增 `examples/mock_fleet_serve/` 解析器／啟動器；`Cargo.toml` 範例 feature gate | Init／check 契約、穩定身分、不覆寫、錯誤／重複設定拒絕；舊範例不變 |
| B2 HTTP Fleet 與網路 | `src/mock/fleet.rs`、`src/mock/server.rs`，僅按需增加匯出 | 相容重用既有 API；逐台狀態／埠及明確 bind／advertise；啟動失敗回收與 Ctrl+C 清理 |
| B3 共用探索 | `src/mock/discovery_responder.rs` 與 Fleet 整合 | 單一 listener 公告多台已就緒成員；精確身分／URL、比對邊界、有界回應及正常關閉 |
| B4 驗收與文件 | 針對性 Fleet／Discovery 整合測試；成對 Mock 指南及相關 CLI 維護連結 | 四台 VMS／CLI 基本驗證、選用 64／256 台檢查、最終批次關卡；通過後才於 changelog／release notes 宣告已實作功能 |

每個完整批次依驗證結果及限制提交。施工中執行針對性測試，最終驗收再合併執行
一次預設／全功能 workspace 測試、all-target Clippy、格式及 strict rustdoc，
不在每個 helper 或文字修改後重跑全套。其後若修正程式碼，重跑受影響關卡。
保持協定、狀態與 UI 範圍清楚，不將無關 Mock fidelity 工作納入此階段。
實作後再調整發布切點追蹤，不提前宣告收錄。

## 驗收

1. 四台設備同時持續執行；探索得到四個不同身分，每個公告 URL 均可呼叫
   `GetDeviceInformation` 並回傳該台預期資料。重複探索不增加重複身分。
2. 對其中一台執行受允許的測試狀態變更，不影響另一台。重啟後保留設定的
   UUID／埠，並依文件重新載入原始狀態。
3. 無效 manifest、HTTP／UDP 埠占用及無效網卡須明確失敗，不殘留自有 listener
   或修改輸入檔。以可重現的生命週期測試檢查 Ctrl+C 及其後埠的重新使用。
4. 一般測試透過 loopback unicast 驗證多成員探索的精確回應；在經授權的介面
   另測真實 LAN multicast 與 VMS 加入。Unicast 通過不等於 multicast／平台驗收。
5. 選用的 64／256 台測試量測啟動、不同設備探索、有 timeout 的併發基本讀取、
   失敗及資源使用；記錄主機與併發度，檢查 VMS 的設備身分處理。這是容量基本
   檢查，不宣稱長時間穩定性、串流、事件訂閱或品牌相容性通過。
6. 在可用環境驗證 Windows／Linux／macOS 原生建置與網路生命週期。缺少原生
   VMS／multicast 檢查時如實記錄，不宣告通過。

## 決策

依上述小範圍設計推進；撰寫計畫不需要其他決策。單一程序、同 IP 不同埠及僅在
啟動時載入設定，是實作預設，不保證每套 VMS 均接受此網路配置。

若目標 VMS 以 IP 去重，加入多主機 IP 前須再討論；RTSP、執行期間狀態回寫、
Metamorph 混合及指定版本收錄亦須另行確認。LAN 驗收前確認經授權的實驗網卡及
目標 VMS，不以修改主機網路代替確認。

本計畫為[後續清單](post-0.17-backlog_zh.md) F11 的縮小首階段，接續
[CLI 重新驗收中的 Mock 討論](cli-0.17-reentry_zh.md#mock-討論)。
