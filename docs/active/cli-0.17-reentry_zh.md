# CLI 0.17 重新驗收

[English](cli-0.17-reentry.md) | [繁體中文](cli-0.17-reentry_zh.md)

使用者於 2026-09-12 重新開啟 CLI 切點。既有 CI 僅證明其對應提交，不涵蓋
本次 runtime 變更。另行核准發布前，版號維持 0.16.0；本項工作不授權發布或系統安裝。

| 章節 | 用途 |
| --- | --- |
| [實作批次](#實作批次) | 範圍及驗證 |
| [Mock 討論](#mock-討論) | 提案，非已實作功能 |
| [驗收](#驗收) | 證據及剩餘關卡 |

## 實作批次

1. 將 manage 的一般搜尋結果選單改接既有 discovery 瀏覽器。驗證 `/`、`r`、`n`、
   大寫 `A`、`c`、一般文字搜尋、空結果、詳情及原始紀錄選取。Manage 的 Enter
   選取本次工作階段設備；只有獨立 discovery 保留新增設備流程。
2. 重新納入 F12：以反白區分固定底部狀態列，分離操作提示，隱藏閒置的待完成
   按鍵，顯示可用上下文及 BUSY 經過時間。保留有界版面、差異重繪及終端恢復，
   不將經過時間呈現為已測量的完成百分比。
3. 返回 manage 操作／設備選單時保留選取及捲動位置，包含取消憑證輸入；
   清單長度或終端尺寸變更時限制於有效範圍。
4. 更新成對使用者／發布文件。執行針對性 UI 檢查、Windows ConPTY 驗收，
   以及一次預設／全功能 workspace 測試與 Clippy 批次，通過後提交。

不新增 RTSP 播放、攝影機寫入、公用導航 crate、相依套件、Agent envelope 變更，
亦不修改主機或網路設定。

## Mock 討論

目前原始碼區分三種情況：

- `examples/mock_server` 僅提供 loopback HTTP，未啟用探索。不同埠及設定檔
  可支援多開，但不會公告設備。
- `MockServerBuilder::discoverable` 每台設備各自建立 UDP 3702 listener；
  繫結失敗時記錄警告，HTTP 仍啟動。它不是共用 listener。
- `Fleet` 目前不透過 WS-Discovery 公告成員。

建議首階段採單一程序管理多台隔離設備與一個 discovery responder。每台須有
獨立且穩定的身分與狀態、可到達的公告服務位址，以及明確的關閉／移除語意。
以明確的 interface／bind／advertise 選項控制 LAN 暴露；不應默默將未認證的
Mock 開放至 loopback 之外。使用者要求的探索啟動失敗必須明確可見，不誤報完整成功。

多個獨立啟停的程序需要明確協調機制或不同位址，不能僅開啟 socket reuse。
目前等待使用者選擇主要程序模型。F11 仍屬討論，不表示本次建置已提供該功能。

實作前須定義重複身分／設定拒絕、type／scope 比對、回應大小限制、新增／移除
生命週期及多網卡行為。驗收須探索多個獨立設備並到達每個公告 HTTP 端點，確認
狀態隔離及關閉後不再出現；multicast／平台限制須與 unicast 測試結果分別記錄。

## 驗收

- 共用瀏覽器及版面的針對性回歸：本機通過。
- 一次完整 workspace 批次通過：全功能 1,311 通過／5 忽略；預設功能 1,199
  通過／5 忽略，各 41 個 suites。兩種 all-target Clippy 設定均於 `-D warnings`
  下通過；格式檢查通過。
- Windows ConPTY 通過實際 manage 網路探索、已存／未存篩選、搜尋及空結果恢復、
  符合條件的已存 ID 與 session-only 新設備選取、半頁按鍵、視窗縮放、底部反白、
  取消憑證／返回設備選單後位置保留、快照取消及終端恢復。使用無憑證的臨時
  設備清單；互動流程前後其檔案不變。探索結果不構成快照／播放互通性證據。
- 已審查 runtime Git blob：`interactive.rs` 為
  `3d879c3ab08143bdf3c9906cf0f28e1956c0b8ec`；`manage.rs` 為
  `171db25fe5d1255dd43715ac5c01d529bde474d8`。測試的 Windows debug binary SHA-256：
  `4569efd15f6c9bbe4a5cfef005ea1d3d1da42d5bca20a07aa9c29206f04fcf47`。
- macOS／Linux 原生終端及人類 IME 組字：Windows 按鍵注入不構成驗證，仍須驗收。
- 新提交的託管 CI，以及最終發布／版號／套件核准：待完成。

見[維護指南](../cli-maintenance_zh.md#人工驗收)與[發布切點](release-0.17-cut_zh.md)。
不可將原凍結清冊視為已審查變更後的檔案內容；本次重新驗收另列後續證據。
