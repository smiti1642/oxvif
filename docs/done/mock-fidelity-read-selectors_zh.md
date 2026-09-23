# 讀取選擇器範圍批次 RS1

[English](mock-fidelity-read-selectors.md) | [繁體中文](mock-fidelity-read-selectors_zh.md)

狀態：有限 RS1 批次已完成（DONE）。來源基準 `4a9d0a5`，2026-09-22。這是 W04 共用解析
遷移，對 W10／W11／W14 提供有限的 selector 證據。

| 章節 | 用途 |
| --- | --- |
| [操作卡](#操作卡) | 修改 handler 前的 W01 審查 |
| [驗收](#驗收) | C01–C12 適用性與剩餘範圍 |

## 操作卡

四項讀取都經過 `dispatch::respond_with_effect`、已解析的
`Request::operation` 及服務 dispatcher。Action 是服務 namespace、`/` 與
操作名稱；HTTP 服務端點與 in-process transport 必須一致。Client／session
簽名不變。讀取只選擇不可變快照，不寫入狀態、不觸發 replay invalidation
或 on-change hook。Replay 命中仍回傳既有錄製資料，RS1 只改 synthetic dispatch。

| Ledger ID／namespace | Client／handler／現有 reader | 目標與輸出邊界 |
| --- | --- | --- |
| `media.GetOSD`／`http://www.onvif.org/ver10/media/wsdl` | `get_osd`；`media::resp_osd`；巢狀 `extract_tag(GetOSD)`、`extract_tag(OSDToken)` | 直接、正確 namespace 的 scalar `OSDToken`；保留解碼後字面身分與 extension 範圍；`render_osd_entry` 與單數 OSD wrapper；未知 token 保留原 fault |
| `ptz.GetNode`／`http://www.onvif.org/ver20/ptz/wsdl` | `ptz_get_node`；`ptz::resp_ptz_node`；全域 `extract_tag(NodeToken)` | 直接、正確 namespace 的 scalar `NodeToken`；既有 `render_node`；缺少／空值／未知 token 保留原 fault |
| `ptz.GetConfiguration`／同 PTZ namespace | `ptz_get_configuration`；`ptz::resp_ptz_configuration`；全域 `extract_tag(PTZConfigurationToken)` | 直接、正確 namespace 的 scalar `PTZConfigurationToken`；既有 `render_config`；保留缺少／空值／未知 token 的 fault |
| `recording.GetRecordingJobState`／`http://www.onvif.org/ver10/recording/wsdl` | `get_recording_job_state`；`recording::resp_recording_job_state`；全域 `extract_tag(JobToken)` | 直接、正確 namespace 的 scalar `JobToken`；回傳該 job 的 recording token／mode，輸出字串 escape；缺少／未知 token 保留原 fault |

參考審查：已在 checkout 外下載 23 份資源，依 `packaging/schema-sources.json`
驗證 SHA-256。審閱 [Media WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl)、
[PTZ WSDL](https://www.onvif.org/ver20/ptz/wsdl/ptz.wsdl) 與
[Recording WSDL](https://www.onvif.org/ver10/recording.wsdl) 的上述 request 宣告；
鎖定取得日期 2026-09-10。Selector 屬於操作本身；Header、外來 namespace、
extension 後代不能提供它。Media extension 不得混入必要 selector。
本批不是完整 request-shape 或 fault 相容性：不相關子節點沿用原寬容行為。
缺少／空值保留原操作錯誤；重複或非 scalar 使用既有 `RequestError` InvalidArgs
政策；未知 token 的 legacy Code 結構另待 fault 遷移。

## 驗收

RS1-WIRE 發現：第一份外部 candidate corpus 的 GetOSD 因
`cvc-complex-type.2.4.a` 被拒。`render_osd_text` 將 PlainText 放在其他已建模
文字成員之前；`render_osd_entry` 亦缺少 image 容器。GetOSD／GetOSDs 共用此
renderer。修正兩者、保留專案自製文字／圖片 capture，再跑外部驗證。
解碼後身分亦要求 OSD／PTZ 字串 escape；共用輸出修正不代表寫入 handler 已驗收。

- C01／C02：dispatch／Action 不變、共用單次 owning parse；同步雙語 ledger
  參數與 reader inventory。
- C03／C04：替代／預設 prefix、Header／外來／Extension 誘餌、重複與巢狀
  selector、entity／CDATA 僅解碼一次；完整子節點順序、不相關成員及 token
  最大長度不在本批範圍。
- C05／C06：兩種不同物件、未知／錯誤家族 token、兩種 transport 的狀態／hook
  保持與 instance 隔離；不宣稱 mutation lifecycle 或運動／錄製模擬。
- C07／C08：確認字面值與結構化拒絕；獨立驗證選定成功回應及一般拒絕。
  未知 token 的 legacy fault 仍未完整審查。
- C09：共用 auth／resource gate 不變，沿用常設測試；不增加驗證例外或資源政策。
- C10／C11：靜態／模型讀取保持現有能力界線；涵蓋 typed client 與 raw request，
  client API 不變。
- C12：完成單次批次敏感度實驗、還原後五道 gate 與外部 corpus 驗證才結案。
  RS1 與 route 數量都不代表 W10／W11／W14 結案。

## 結果（2026-09-22）

四個 route 已改用直接、正確 namespace、解碼後的 selector。兩種 transport
測試涵蓋誘餌、重複、巢狀、缺少／未知身分、CDATA、兩種不同物件、instance
隔離、完整狀態不變及 hook 次數。五組實際 client capture 亦比對選定值，
包括 escape 後的 OSD 文字與圖片路徑。共用 OSD／PTZ renderer escape 字串；
錄影狀態 escape recording identity／mode。第一份外部 export 失敗後修正了
OSD 文字順序與圖片巢狀容器。其他 request-shape 寬容與原操作 fault 仍未結案。

單次合併敏感度實驗關閉重複拒絕與選定 renderer escape；未篩選的 workspace
全功能／no-fail-fast 測試失敗，包括兩項新 transport 測試與新 capture 測試，
隨後精確還原。第一份 168-instance export 揭露 OSD 問題；修正後 170-instance
export 通過固定 Xerces XSD 1.1 與 payload anchor（85 組 exchange、50 個操作、
64 個成功與 21 個既有拒絕）。七項後端 qualification 控制通過。Python 已知
K21 編譯警告仍視為失敗，沒有抑制。新的 malformed-selector 回應由常設精確
fault 測試涵蓋，未加入要求有效 request 的 schema corpus。

兩組 inventory self-test 通過：159 routes、161 Action sites、190 次 reader
呼叫（減少五次）。還原後 workspace 格式、兩種 Clippy／test 為最終 commit
gate。較廣的 W04／W10／W11／W14 與 W20–W23 仍為 PARTIAL，不宣稱新 CI 或發布。

最終驗收：workspace 全功能 1,346、預設 1,230 通過，各七項 ignored、42 suites。兩種 all-target Clippy、格式、strict rustdoc、inventory 控制及本地文件連結均通過。
