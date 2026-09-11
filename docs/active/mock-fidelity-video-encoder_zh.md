# Video encoder 批次 VE1

[English](mock-fidelity-video-encoder.md) | [繁體中文](mock-fidelity-video-encoder_zh.md)

基準：`e6962b5`，2026-09-11。負責者：目前 hardening 工作。W01／W10／W17–W19。

| 章節 | 用途 |
| --- | --- |
| [操作卡](#操作卡) | 完整 encoder 子群及共用消費端 |
| [決策與風險](#決策與風險) | 契約、模型與 API 邊界 |
| [驗證](#驗證) | 具辨識力案例與整批 gate |
| [證據](#證據) | 實際結果、待決策項目與接續工作 |

## 操作卡

Action 為 client 原始碼 namespace 加上 `/` 及操作名稱。Media1：
`http://www.onvif.org/ver10/media/wsdl`；Media2：`http://www.onvif.org/ver20/media/wsdl`。
Operation body 使用相同 qualified identity。Client 入口位於 `src/client/media.rs`、
`media2.rs`，session wrapper 位於 `src/session.rs`；沿用共用 Action resolver。

| ID | 基準 handler／輸入 | 目標行為 |
| --- | --- | --- |
| media.GetVideoEncoderConfigurations | `media::resp_video_encoder_configurations(state, body)`；空訊息操作卻接受全域選填 token | 驗證空 body、轉義完整 catalogue、明確的不可表示 view 政策 |
| media.GetVideoEncoderConfiguration | `media::resp_video_encoder_configuration`；全域必填 token | Qualified 唯一 scalar、單一 snapshot 及 structured NoConfig |
| media.GetVideoEncoderConfigurationOptions | `media::resp_video_encoder_configuration_options`；全域必填 token，忽略 profile | 選填 configuration／profile、generic options、一致且可寫入的範圍／codec，保留深層 options 控制 |
| media.SetVideoEncoderConfiguration | `media::handle_set_video_encoder_configuration` → `apply_video_encoder_write` | 完整 scoped candidate、必填 persistence、禁止部分寫入、明確拒絕未建模設定 |
| media2.GetVideoEncoderConfigurations | `media2::resp_video_encoder_configurations`；全域 token，未知值變空清單 | Scoped configuration／profile selector、structured 未知參考 Fault 及轉義 Media2 view |
| media2.GetVideoEncoderConfigurationOptions | `media2::resp_video_encoder_configuration_options_media2`；全域必填 token 與固定 codec／rate 清單 | 與 setter 共用模型、各 source 上限不同、維持服務特定 wire shape |
| media2.SetVideoEncoderConfiguration | `media2::handle_set_video_encoder_configuration` → 共用全域 writer | 服務特定 attribute／element 契約、原子 candidate 與 committed effect |
| media2.GetVideoEncoderInstances | `media2::resp_video_encoder_instances()`；忽略輸入 | 必填 **source configuration** token、已建模容量而非使用數量、structured NoConfig |

共用消費端：`VideoEncoderState`、兩個 profile inline renderer、Media1 `render_vec_body`、
Media2 `render_video_encoder`、`VideoEncoderConfiguration`、`VideoEncoderConfiguration2`、
replay 相依、source 重設及 profile 引用計數。須一併審查，不附帶新增無關操作。

## 決策與風險

一手審閱：[Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§5.5、[Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§5.2.3／5.3.2–5.3.5 及固定外部 schema。官方資源與 schema 衍生表格／fixture 不進 repository。

- **K26：** H265 可進入共用 state 並產生無效 Media1 view。保留 codec 身分；若所選 encoder
  無法表示，Media1 回覆明確標示的 receiver 模型政策 Fault，不默默省略 binding／configuration，
  也不任意轉換 codec。Media2 維持可用。
- **K34 基準缺陷：** Media2 rate control 公開欄位為 `u32`，12.5 解析成零，mock state 亦為整數。
  原始碼已確認並有專屬重現。維護者於 2026-09-11 核准下一 minor 改為 `f32` 並要求施工。
  區分選填省略與已提供但無效的欄位；非有限幀率在 transport 前拒絕；遷移共用 mock state，
  明確拒絕無法表示的 Media1 view。保留舊整數 JSON 載入，記錄 Rust／JSON 遷移，不弱化測試。
- **K35：** 全域部分 setter、未轉義名稱／token、忽略 selector 及未驗證數字可能與 options
  矛盾。完整已建模值先驗證再一次條件提交；UseCount 唯讀，省略 rate control 保留現值。
- Quality／rate 可調整；語法合法但超出範圍的 bitrate 必須調整。讀寫共用明確的 synthetic
  options 政策，保留各 encoder resolution 清單與各實體來源不同上限；可寫 JPEG 就必須
  公告 JPEG，不只 H264。此設定不代表實機編碼效能或真正 RTP 效果。
- Media1 codec child 與 Media2 codec attribute 不互通。要求完整已建模必填欄位，驗證巢狀
  數字、boolean、唯一直接欄位及 namespace。未建模 extension、multicast destination、
  encoding interval、signing、guaranteed-rate 變更明確拒絕；允許 Get→Set 所需的不變無串流預設值。
- Source／profile context 採已記錄的邏輯重設模型，不宣稱實體 routing 衝突。Instances
  描述明確的各 source synthetic 容量，不是目前 binding 數；configuration／source／profile
  effect 均須淘汰真正相依的錄製。
- 未知 configuration／profile 為 Sender／InvalidArgVal 加 NoConfig／NoProfile；無法設定的
  已建模值用 ConfigModify。結構錯誤沿用共用邊界政策，未支援效果使用明確模型政策。

## 驗證

新增 `tests/mock_video_encoder.rs`，共用 in-process／HTTP driver：末欄位錯誤後完整
snapshot／hook、有效 Get→Set→Get、轉義、重複、錯誤 namespace、巢狀 decoy、非有限
float、數值界線、generic／profile／config selector、雙 source options、公告值實際寫入、
bitrate 調整、H265 與 Media1 view、必填 source-token instances、拒絕與成功 replay。
另加獨立 client 小數 fps 重現，不以 production parser 當 oracle。保留既有 roundtrip、token、
workflow 與深層 options 測試；僅有明確理由才修正無效輸入或過時預期。

外部無憑證 corpus 擴充八項操作、寫入回讀及代表性 Fault。先 focused 舊碼重現，再做整批
一次 workspace／all-feature／no-fail-fast 擾動，精確還原後執行最終五項 gate。整批執行
strict docs、清冊及外部驗證；必要時只重跑受影響檢查。新核准的公開幀率契約作為 VE1 中
可獨立驗收的 K34 交付：client 讀寫、共用 mock rate／Media1 view、有限值拒絕、serde 相容
與遷移文件一併完成，在此交付邊界做一次擾動及最終 gate，不逐 helper 重跑。
其餘八操作的 selector／options／完整 candidate 仍以整個子群驗收。這是將公開 API 遷移與
其他 encoder 語意分開，不代表 VE1 結案。每次交付同步雙語指南、changelog、audit／清冊／
檢查表及證據後直接 commit／push。不做實機寫入、發布、安裝或合併主分支。

## 證據

Preflight 已於 VE1 runtime 修改前記錄。K34 已在 `e6962b5` 透過公開 client 與外部、
自行撰寫的離線 responder 重現：控制值 `25` 解析成 25，`12.5` 卻解析成 **0**，
使明確的數值斷言失敗。重現程式位於 checkout 外：
`C:/Users/smiti/AppData/Local/Temp/oxvif-fractional-rate-20260911`。
命令：`cargo run --offline --manifest-path <reproducer>/Cargo.toml --target-dir target/mock-fidelity-docs`。
此外部診斷有獨立 lockfile，不取代 locked workspace gate 或外部 schema 驗證；沒有連線攝影機或網路。

K34 公開 API 選擇已於 2026-09-11 核准，並依下列 rate 契約交付實作；
Media1 自身的公開整數型別不變。八項操作的 VE1 剩餘工作**尚未完成**。
VE1 之後的完整服務子群為 audio／metadata；VE1 本身不代表 W10 結案。
VS1 已以 `e6962b5` 提交並推送，CI `34574805465` 後續已通過。

### K34 交付，2026-09-11

- `VideoRateControl2` 與 `VideoEncoderState` 改用 `f32`。保留選填未提供；已提供
  但缺漏、重複、巢狀或無效的 rate 欄位回明確解析錯誤。負值／非有限 outbound
  數值在 transport 前拒絕。Serde 接受一般舊整數 JSON，無效值明確拒絕，不存成 `null`。
- 兩種 mock setter 在改變狀態前驗證 qualified rate block，並以一次 conditional
  lock 完成既有欄位更新。Media1 在 encoder／profile 頂層明確拒絕小數或其他無法
  表示的幀率；不修復或輸出無效 seed。不受影響的獨立 encoder 仍可讀取。成功寫入
  才淘汰相依的內建 replay 讀取，拒絕時保留 recording；其他欄位仍為 K35 legacy 邊界。
- `tests/media2_rate_control.rs` 涵蓋小數讀取、未提供／無效／歧義 rate control、
  明確欄位錯誤、擷取 outbound Action／body、transport 前拒絕及 JSON 遷移。
  舊碼失敗為 `fractional_rate_survives_public_client_read` 與
  `malformed_rate_is_not_zero`（RTK `1789112740_cargo_test.log`）。外部重現程式
  修正後亦通過：wire 25 → 25、12.5 → 12.5。
- `tests/mock_video_rate.rs` 的 `rate_state_and_views` 及 HTTP 版本驗證錯誤末欄位、
  qualified 結構拒絕後完整 state／hook 保留，以及共用小數儲存、雙服務 view 和整數
  恢復。`public_mock_rate_roundtrip_and_old_snapshot` 驗證 29.97、省略保留及舊整數
  JSON；`invalid_seed_rates_are_explicit_and_not_persisted_as_null` 驗證明確 Fault、
  seed 不變、持久化拒絕及真正的零。`rate_replay_commit_boundary` 及 HTTP 版本
  驗證拒絕／成功的相依行為，並保留不相關 recording。
- 一次未篩選的 workspace／all-feature／no-fail-fast 擾動
  （`1789113536_cargo_test.log`）在三個 target 產生 **10 個 runtime 失敗**：client
  小數讀取、outbound 防護、JSON 測試；全部六個 mock-rate 測試；以及 corpus 的
  `captures_fractional_rate_and_explicit_media1_limit`。擾動將解析值取 floor，並關閉
  outbound、序列化及 Media1 view 防護；四處均精確還原。這證明上述失敗路徑，
  不代表每一獨立斷言或完整 encoder 語意均經擾動證實。
- 本機 all-feature workspace：**1,250 passed、5 ignored、37 suites**；default
  workspace：**1,147 passed、5 ignored、37 suites**。兩種 locked workspace／all-target
  Clippy 均以 `-D warnings` 通過，獨立 `mock`／`serde` all-target 掃描亦通過。
  格式、diff 空白及 strict all-feature／default workspace rustdoc 通過；
  新計畫／遷移文件的相對連結目標均存在。
- 明確外部匯出新增三項操作的七組 rate exchange。外部新目錄
  `oxvif-profile-corpus-20260911-06` 的 **84／84 instance** 通過 strict Xerces XSD 1.1：
  42 組 exchange、24 操作、30 個成功及 12 個 Fault；explicit legacy shape 驗證亦通過。
  僅為選定結構檢查，不代表完整語意符合性；ignored 不計為驗證通過。
- 清冊／self-test 通過：159 Action site、157 route、**231** 個直接 reader
  （216 個位於頂層 test module 前、15 個位於其中；68 個 production enclosing symbol）。
  雙語 ledger／source audit 一致。公開遷移文件提供
  [English](../media2-frame-rate.md)／[繁體中文](../media2-frame-rate_zh.md)。

交接：K34 本機交付 gate 通過，以上結果不包含該批託管 CI。未發布、安裝、寫入實機、
合併主分支或合併貢獻者 PR。下一步執行剩餘 VE1 工作卡，包含 K26 codec view 與
K35 完整 candidate／options／selector／capacity 語意。更廣的 HTTP／安全、其他服務、
feature／平台驗收及 PR #16 整合仍列於[施工檢查表](mock-fidelity-execution-checklist_zh.md)。
K34 與已核准的 VE1 政策目前不需新增決策。
