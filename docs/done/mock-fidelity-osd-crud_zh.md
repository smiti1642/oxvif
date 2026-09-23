# OSD CRUD 批次 OS1

[English](mock-fidelity-osd-crud.md) | [繁體中文](mock-fidelity-osd-crud_zh.md)

狀態：已完成（2026-09-23）。2026-09-23、基準 `34cccdb`，修改 handler 前建立 W01 工作卡。
本批限 W10／W16／W19 選定範圍，不宣稱實機或影像疊加效果。

## 操作卡

Action 為 `http://www.onvif.org/ver10/media/wsdl/` 加操作名稱。
`respond_with_effect` 擁有解析結果，Media1 dispatch 傳遞所選 operation；
Media2 不支援 OSD 的既有宣告不變。

| Ledger ID | 現有讀取／目標 | 狀態、Fault 與 replay |
| --- | --- | --- |
| media.GetOSDs | media::resp_osds；fragment → 直接、可省略的 qualified ConfigurationToken | 驗證唯一來源 configuration；唯讀篩選／全量快照；未知來源 NoConfig |
| media.GetOSD | media::resp_osd；既有 RS1 selector 與共用 renderer | 保留 RS1 容忍範圍及未知 token 舊 Fault；驗證 CRUD readback，不擴為完整 read Fault 稽核 |
| media.GetOSDOptions | media::resp_osd_options；忽略 body → 必要 ConfigurationToken | 驗證來源；各來源使用相同 synthetic options，唯讀 |
| media.CreateOSD | media::handle_create_osd；全域 fragment → 單一 qualified candidate | 欄位／來源驗證、每來源配額與唯一 token 分配同鎖；超額 Receiver/Action/MaxOSDs |
| media.SetOSD | media::handle_set_osd；全域 token／fragment → 單一 candidate | 唯一既有 OSD、來源綁定不得變更、排除自身的替換配額；Sender/InvalidArgVal/NoConfig 或 ConfigModify |
| media.DeleteOSD | media::handle_delete_osd；fragment → 直接 qualified token | 同鎖移除唯一 OSD；未知 NoConfig，重複儲存 identity 為無效 snapshot |

參考：[Media 24.12 §5.20](https://www.onvif.org/specs/srv/media/ONVIF-Media-Service-Spec.pdf)
的來源配額與操作 Fault。固定 WSDL／XSD 依 `packaging/schema-sources.json`
留在 checkout 外；核對另發現 client position／color／persistence 與文字順序的
wire 差異；已修正正式 client 並驗證實際 capture，不加入 schema 衍生 fixture 清冊。

## 已交付契約

- 保留公開 state shape；建模 Text／Image、位置、有限座標、文字類型／格式／字級／
  內容與字色。XML 只解碼一次，保留字面 identity／text。
- 未建模欄位／attribute／extension、背景色及暫存文字明確拒絕。
  省略／true persistence 使用既有持久化 hook，不宣稱磁碟耐久性或實體疊圖；
  image URI 只儲存、不連線下載。
- 驗證 shape／multiplicity／scalar；缺漏／重複走 RequestError，未建模走
  mock:UnmodeledEffect，無效值走 ConfigModify。Set 不得默默忽略綁定變更。
- 圖片亦受總配額限制，文字子類配額按來源計算，替換時排除自身。
  Counter 溢位／碰撞不得 panic、重複 identity、拒絕卻消耗 ID 或部分寫入。
- 成功寫入一次通知及 OsdCommitted；拒絕零通知、零 effect。內建 replay
  只在 commit 後退休 Media1 GetOSD／GetOSDs；options、無關操作與 standalone
  replay 政策保留。

## 驗收

C01–C04：ledger／reader、Action／operation、prefix、Header／extension decoy、
重複／缺漏／錯誤 namespace、attribute 及 entity／CDATA。
C05–C06：生命週期、兩個來源、獨立 instance、完整狀態／hook、並行配額與唯一 token。
C07–C08：精確 Fault、client 值與獨立 wire 驗證，含顏色、自訂座標、圖片、options。
C09 沿用 auth／resource gate；C10–C11 檢查 options、持久化 hook 與 replay，
不宣稱硬體效果。C12：一次 unfiltered all-feature/no-fail-fast mutation campaign、
精確還原、五項 workspace gate、清冊／連結及外部 schema 驗證。

## 交付結果

- 五個 route 已改用 `services/osd.rs` 的 `list`、`options`、`create`、
  `set`、`delete`；上方操作卡保留修改前清冊。GetOSD 沿用 RS1 selector，
  並共用修正後的 renderer。
- 七項生命週期／配額／counter／replay 測試涵蓋 in-process 與 HTTP；
  client attribute 優先序另有回歸 assertion。Token discrimination probe
  先排序配額 map 再比較 Debug 輸出，保留所有配額並排除 HashMap 順序誤判。
- 完整、未過濾的 `cargo test --workspace --all-features --no-fail-fast`
  組合擾動在 CRUD、配額、replay、capture 產生七項預期失敗，之後精確還原。
  個別移除顏色／persistence attribute 讀取也各使 client assertion 失敗；
  還原後 focused test 通過。
- 五項 workspace gate 全通過：fmt、兩種 all-target Clippy（拒絕警告）、
  all-feature／default 測試（含 workspace doctest，各 1359／1241 通過）。
  兩種 strict workspace rustdoc 也通過。
- 選定外部 corpus 共 99 exchange／198 instance／56 operation，含 22 Fault
  response。固定 Xerces XSD 1.1 strict 驗證 198 份全通過，七項 validator
  qualification control 通過。首次 capture 拒絕空 ImageOption；補上 synthetic
  ImagePath reference 後重新匯出並通過。Schema 與 capture 留在 checkout 外；
  此 reference 不代表圖片下載或實際渲染。
- 雙語清冊 self-test 及 Markdown 路徑／anchor 檢查通過。

OS1 以有限批次完成歸檔；背景色、暫存文字、任意 extension 與完整 read Fault
稽核不在本次驗收範圍。[Active 執行清單](../active/mock-fidelity-execution-checklist_zh.md)
仍保留其餘 W10／W16／W19 工作。本機結果不宣稱 hosted CI、實機符合性或
ONVIF 認證。
