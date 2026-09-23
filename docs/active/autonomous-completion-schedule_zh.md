# 剩餘計劃自主執行排程

[English](autonomous-completion-schedule.md) | [繁體中文](autonomous-completion-schedule_zh.md)

狀態：IN-PROGRESS。2026-09-23 授權，起點 `6ec107f`。
此清單排程既有 [F01–F15](post-0.17-backlog_zh.md)，不新增產品路線，也不代表母計劃已完成。
使用者要求所有項目列入排程、完成可做部分；遇到 blocker 先跳過做其他項，最後統一回報。

| 章節 | 用途 |
| --- | --- |
| [執行政策](#執行政策) | 完成、相容性與交付 |
| [工作佇列](#工作佇列) | 全部剩餘家族與驗收邊界 |
| [證據與阻礙](#證據與阻礙) | 可持續接續的紀錄 |

## 執行政策

- 依序完成可獨立驗收的服務／產品批次。Mock handler 修改前沿用 W01 卡與 C01–C12。
- 一般實作選擇自主決定；困難不等於 blocker。保持公開 API 相容，優先使用 private adapter。
- 外部 schema／capture 留在 checkout 外；驗證行為、拒絕後狀態、有意義擾動與適用的兩種 transport。
- 每批通過 repository gate 後 commit，更新雙語證據；只有完整完成的有限計劃移到 done，
  正常 fast-forward／push master 與 develop。不建立 PR、強推、發布或修改實體攝影機。
- 記錄確切缺少的輸入／平台或無法自行決定的互斥產品選擇，完成獨立部分再做其他列。
  不將缺少的實機驗收標記通過，也不以此延後無關的程式工作。
- 在本任務持續執行；此為依相依性排序的工作佇列，不是時間排程。誤建的 heartbeat
  已於使用者 2026-09-23 澄清後立即刪除。不逐批通知，全部可做工作完成後統一列出
  成果、剩餘 blocker 與解除所需輸入。互動執行仍可能顯示工具進度。

## 工作佇列

順序依相依性安排，不承諾日期。TODO 表示仍須稽核／實作，不代表相關服務目前不存在。

| ID | 既有負責計劃 | 狀態 | 工作與完成邊界 |
| --- | --- | --- | --- |
| Q01 | F04／W15 | PARTIAL：EP2 已交付 | Events 訂閱 endpoint 身分、filter／queue 隔離、有界容量、renew／expiry／unsubscribe；保留 RequestCtx 建構契約；可控時鐘／生命週期測試與獨立 wire 驗證 |
| Q02 | F05／W07–W09 | PARTIAL: AF1 delivered | Auth freshness、有界原子 nonce 防重用／到期；HTTP binding／status／header；structured 與刻意 raw fault injection、roles、拒絕不改狀態 |
| Q03 | F01／W10 | TODO | Media URI／source mode／binding 契約、OS1 以外 OSD extension；明確建模／拒絕欄位與雙 view 一致性 |
| Q04 | F02／W11–W12 | TODO | PTZ space／configuration／preset／home／tour，以及每來源 Imaging／focus 模擬效果、範圍與錯誤 |
| Q05 | F03／W13–W14 | TODO | Device／DeviceIO user／network／scope／storage／relay，以及 Recording／Search／Replay 生命週期、終止與連鎖處理；僅 synthetic 效果 |
| Q06 | F06／W16–W19 | TODO，隨 Q01–Q05 | 其餘 capability／effect／read 相依、原子性、instance 隔離、commit 後 replay 失效；standalone replay／key 設計另評估 |
| Q07 | F07／W20–W23 | TODO，隨服務批次 | 其餘 QName／wildcard／negative、外部 corpus、有界 fuzz／property；保留獨立驗證與覆蓋限制 |
| Q08 | F15／CLI 強化 | TODO | Observability、clock-skew、registry durability、命令 descriptor reachability 與可執行 example；復原測試與 Agent 契約保持 |
| Q09 | F11／Mock Fleet | TODO | 其餘 discovery 協定、有界 lifecycle／load；查核可用 native／LAN／VMS 證據，僅缺少的驗收列 blocked |
| Q10 | F14／Metamorph M4／M7 | TODO | 依驗收清單的 local-owner transition／明確 reference 比較候選；驗證 identity、歧義、失敗保留及 teardown，記錄必要公開 API 決策 |
| Q11 | F08／CLI 路線 | TODO | 評估並在既有產品／平台條件容許下交付有限批次匯出、受控寫入 preview／apply、RTSP／playback；不對實體攝影機執行寫入 |
| Q12 | F08／F12／導覽 | TODO | Native terminal／IME、取消／resize／復原；獨立 crate 拆分先核對實際第二個 consumer |
| Q13 | F09／散布 | TODO | 補本地 packaging／復原／簽章／安裝生命週期自動化；核對正式渠道資料與可用平台；外部收錄需 ownership／簽章證據 |
| Q14 | F10／snapshot | TODO | 可重現的有界診斷控制；實機格式根因需去敏感失敗 response 與 transport metadata |
| Q15 | F13／相依套件 | TODO | 可用平台的 keyring migration、locked／denied／unavailable store／recovery；驗收完成前不改正式版本 |
| Q16 | 全部母計劃／總驗收 | TODO | 每列對應 commit／測試／缺少輸入，歸檔符合條件者，確認雙分支同步並統一回報 |

## 證據與阻礙

- 起點：OS1 已於 `6ec107f` 交付；workspace 乾淨，master／develop 與兩個 origin ref 一致。
  既有 1,359／1,241 測試結果是 baseline，不宣稱本排程已重新執行。
- Q01 盤點：RequestCtx 是公開 literal 建構且沒有 endpoint 欄位；以 private adapter 傳遞
  transport endpoint 身分，避免新增必要公開欄位。既有 Events 雖按 instance 隔離，仍只有
  單一 filter／queue；EP1 保留為既有交付。
- 尚未將新發現分類為 blocker。既有缺少輸入候選見[剩餘驗收](remaining-plan-acceptance_zh.md)，
  每列結案前須核對，不能直接假定全部受阻。

Q01 EP2 已交付：[證據](../done/mock-fidelity-event-lifecycle_zh.md)。W15 剩 push／property 同步、完整 topic matching、規範 WSNT Fault detail，不視為硬體阻礙。下一獨立批次：Q02 auth freshness／nonce replay。

AF1（2026-09-23）：[有界認證時間／nonce 防重用](../done/mock-fidelity-auth-freshness_zh.md) 已交付；nonce 原子 admission、過期／未來／重用拒絕不觸發 hook，時鐘倒退不重開已淘汰區間。外部 corpus 224 XML／60 Actions，AF1 request Security header 去除後匯出。Roles、HTTP binding、structured／raw injection 仍為 Q02 工作。
