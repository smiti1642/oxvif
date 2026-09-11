# PR 16 整合審查

[English](mock-fidelity-pr16-integration.md) | [繁體中文](mock-fidelity-pr16-integration_zh.md)

2026-09-11 原始碼審查；W26 仍為條件式工作，尚未整合。

| 章節 | 用途 |
| --- | --- |
| [已審版本](#已審版本) | 固定 head 及增量差異 |
| [處置](#處置) | 可重用程式碼與必要修正 |
| [整合順序](#整合順序) | 相依事項及驗收 |

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
- Operation／reference 文件未完整雙語同步：diff 未包含 `OPERATIONS_zh.md`
  與對應翻譯 reference page；公開描述也須明示 mock acknowledgment／effect 邊界，
  並說明 profile 關聯串流，而不只描述 video I-frame。

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
