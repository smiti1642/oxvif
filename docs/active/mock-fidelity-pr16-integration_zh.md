# PR 16 整合審查

[English](mock-fidelity-pr16-integration.md) | [繁體中文](mock-fidelity-pr16-integration_zh.md)

2026-09-11 原始碼審查；W26 已在整合候選分支實作，本機關卡通過，hosted CI 待完成。

[社群整合計畫](contributor-pr-integration-plan_zh.md) 現在負責 #14／#17／#16 的
施工順序、目前前置條件檢查、驗收及分支邊界，取代下方歷史前置工作順序；
下方審查發現描述貢獻者 head，而非重寫後的候選版本；目前證據記於該計畫。

| 章節 | 用途 |
| --- | --- |
| [已審版本](#已審版本) | 固定 head 及增量差異 |
| [處置](#處置) | 可重用程式碼與必要修正 |
| [整合順序](#整合順序) | 相依事項及驗收 |
| [B16 操作工作卡](#b16-操作工作卡) | 目前有限的實作契約 |

## 已審版本

David Matthew Mattli（`dmm`）提出的 [PR #16](https://github.com/smiti1642/oxvif/pull/16)
仍為 open，head 為 `3db6459b03b6977b3a41b6055ee3f8ac9f42b049`。相較先前審查的
`dc69e9aaf46c607d1cffa4b55a1e733ba323f5b1`，新 head 合入 master 的 `9dccf9d`
CLI 行號工作；增量檔案清單不含 Media 實作或測試變更。本次重閱完整 PR diff，
不宣稱已執行目前 head。GitHub 觀察到的 merge state 為 UNSTABLE，不等於實作
有缺陷或 CI 通過的證明。此貢獻新增兩個 Media 操作，並非大幅擴充 Media2 服務。

重新核對的官方來源：[Media1 24.12，第 5.18.1 節](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
與 [Media2 26.06，第 5.7.1 節](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)。
規範 payload／fault 細節保留在外部審查筆記，不將複製表格或 schema 衍生 fixture
放入 repository。

## 處置

- Client 方法與 session delegation 可作為整合候選：完整 Action、service 專用
  wrapper、caller token escaping 與預期 response 檢查。移植時保留貢獻者署名。
- 新 mock handler 仍以 legacy `extract_tag` 將 raw escaped text 與 state token
  比對，未採用 hardening 分支的 scoped parser。只檢查 profile 存在便無條件 ack，
  沒有建模 stream effect；不直接移植此實作，也不稱為高擬真模擬。
- Mock fault 仍為 flat；應使用已審查的 structured serializer，保留既有 client
  第一層 subcode 契約。Malformed／ambiguous field 須先於 state／effect 觀察拒絕。
- 兩項 client 正向測試有檢查 Action 與 escaped body，但 PR 未增加專用的負向
  client／fault assertion、session routing 或 mock token／state 控制。兩項 workflow
  新增內容只有 unwrap 成功。Action snapshot 與 schema-shape probe 不能證明實際
  media delivery 或 I-frame 產生。
- Operation 文件未完整雙語同步：diff 未更新 `OPERATIONS_zh.md`。
  基底原本就沒有內部 Media reference 的翻譯版本，不能歸因於貢獻者遺漏。
  公開描述也須明示 mock acknowledgment／effect 邊界，
  並說明 profile 關聯串流，而不只描述 video I-frame。

## B16 操作工作卡

2026-09-11 已授權實作；基準 `07a7d61`。Scoped request、fault、profile catalogue
與 exact-operation policy 已存在。重新核對上方官方章節的不存在 profile 失敗及
關聯串流語意。詳細規範資料仍在外部；以下為專案行為工作卡。

| 欄位 | `media.SetSynchronizationPoint` | `media2.SetSynchronizationPoint` |
| --- | --- | --- |
| Action | `http://www.onvif.org/ver10/media/wsdl/SetSynchronizationPoint` | `http://www.onvif.org/ver20/media/wsdl/SetSynchronizationPoint` |
| Client／Session | `media_set_synchronization_point`，Media1 endpoint | `set_synchronization_point_media2`，Media2 endpoint |
| 輸入 | 唯一 scoped `ProfileToken`，僅 decode 一次、opaque identity、不 trim | 相同輸入，但限 Media2 namespace，不搜尋 local-name |
| 路徑 | `services/media.rs::handle_set_synchronization_point`、借用 Node、structured Fault、共用 profile catalogue | 獨立 dispatch arm，以明確 service 身分呼叫驗證 helper |
| 行為 | 預設拒絕；exact-operation opt-in 才在 shape／profile 驗證後 ack | Media2 獨立 opt-in，不受 Media1 或 Events 啟用 |
| 輸出／fault | Service 專用空 response；已審 nested unknown-profile Fault；malformed 使用共用 request policy | 分類相同，使用自己的 response namespace |
| 狀態／效果 | 一致 profile 讀取；不寫 state、不觸發 hook／queue／RTP、不淘汰 replay | 共用 catalogue，不產生跨 service mutation |
| Replay | 錄製的讀取與明確 fault injection 保留優先順序；錄製的寫入回覆不重播，synthetic fallback 遵守 policy | 同左，包含 identity 控制 |
| 相容性 | 新增方法，不承諾 Mock 預設成功 | 不自動 fallback 到 Media1 或 Events |
| 測試／交付 | S01–S08、Client tests、`tests/mock_media_sync.rs`、corpus、W26／inventory／雙語文件 | 同左，另含跨 service policy 隔離 |

C01–C05 對應 S01／S04／S08；C06 對應 S05；C07／C08 對應 S02／S07。
C09 重用 auth 及共用 request 控制，不新增 exemption（測試無效認證不能 ack）。
C10 對應 S03／S05；C11 對應 S01／S02／S06（新 CLI 方法為 NA）；C12 對應雙
transport、mutation 與外部驗證。兩張卡均不代表 W10／W15 完成。選擇性的硬體
actuation 須另行授權，不新增模擬編碼器。

## 整合順序

1. 完成共用 profile-token 解析、身分輸出與 PTZ／adapter／replay 依賴追蹤，包含
   K27 key collision；僅 synthetic 成功不能宣稱完整 replay 相容。
2. 實作 W16 的 exact-operation 政策：未建模效果預設拒絕，明確 per-operation
   acknowledgment opt-in，不提供全域 legacy-success 開關。Media1／Media2 身分
   必須與 Events synchronization 分離。
3. 授權整合前再次檢查 PR head。保留署名移植 client／session 貢獻；mock 使用共用
   parser、structured fault 與已核准政策獨立實作，不複製無條件成功的 handler，
   亦不默默合併貢獻者 PR。
4. 將新增操作加入來源清冊、operation card 與 coverage table。測試 literal／escaped／
   whitespace／duplicate／mislocated／absent token、unknown profile、policy 隔離、
   state／hook 不變、負向 response payload 及 in-process／HTTP 入口；mutation 必須
   使對應 assertion 失敗。
5. 使用固定外部資源獨立驗證選定 request／response instance，執行 feature／MSRV／
   原生平台及一般品質關卡。同步雙語公開文件、examples 與 changelog，不宣稱實際
   串流輸出。

未對真實攝影機發出 synchronization request：這是 actuating operation，並非唯讀
硬體探測。實際串流驗收、PR merge 與 Release 仍不在目前唯讀硬體／發布授權範圍內。
