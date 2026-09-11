# Audio 與 metadata 批次 AM1

[English](mock-fidelity-audio-metadata.md) | [繁體中文](mock-fidelity-audio-metadata_zh.md)

基準：9016268，2026-09-11。負責者：目前 hardening 工作。
狀態：AM1 已實作並完成本機驗證；提交後另追蹤託管 CI。
維護者於 2026-09-11 核准 D4。
範圍：W01／W10／W17–W19；K24／K25／K36／K37。15 張工作卡以單一 AM1 批次交付。

| 章節 | 用途 |
| --- | --- |
| [操作工作卡](#操作工作卡) | 完整子群與共用相依項目 |
| [決策](#決策) | 已核准預設值及新增 API 邊界 |
| [驗證](#驗證) | 一次子群 campaign 與最終 gate |
| [證據](#證據) | 重現及明確待辦 |

## 操作工作卡

身分均為專案 dispatch ID，不是複製的 schema 目錄。實作前先閱讀雙語操作清冊
及 services/media.rs／media2.rs 中的對應函式。

| ID | 目前 handler | 目標 |
| --- | --- | --- |
| media.GetAudioSources | resp_audio_sources | 空請求、轉義且有界的實體 catalogue |
| media.GetAudioSourceConfigurations | resp_audio_source_configurations | 空請求、一次快照及轉義 source reference |
| media.GetAudioEncoderConfiguration | resp_audio_encoder_configuration | 必要 scoped configuration selector、確切 NoConfig |
| media.GetAudioEncoderConfigurations | resp_audio_encoder_configurations | 空請求、一致的 Media1 codec view |
| media.SetAudioEncoderConfiguration | handle_set_audio_encoder_configuration / apply_audio_encoder_write | 完整 candidate、唯讀欄位、options 一致性及原子提交 |
| media.GetAudioEncoderConfigurationOptions | resp_audio_encoder_configuration_options | configuration／profile／generic selector、各服務 options |
| media2.GetAudioSourceConfigurations | resp_audio_source_configurations_media2 | qualified 選填 selector 與邏輯相容性 |
| media2.GetAudioEncoderConfigurations | resp_audio_encoder_configurations_media2 | qualified selector、Media2 codec 詞彙 |
| media2.SetAudioEncoderConfiguration | handle_set_audio_encoder_configuration_media2 | 各服務 codec 輸入、完整 candidate 及共用狀態 |
| media2.GetAudioEncoderConfigurationOptions | resp_audio_encoder_configuration_options_media2 | selector 驗證、可寫入的公告 codec／bitrate／rate 組合 |
| media2.GetAudioOutputConfigurations | resp_audio_output_configurations | scoped selector、轉義 output reference 及數值 view |
| media2.GetAudioDecoderConfigurations | resp_audio_decoder_configurations | scoped selector 與轉義身分 |
| media2.GetMetadataConfigurations | resp_metadata_configurations | scoped selector、確切不存在 reference Fault 及完整已建模 view |
| media2.SetMetadataConfiguration | handle_set_metadata_configuration | 完整 candidate、不捏造 multicast 值、拒絕未支援效果 |
| media2.GetMetadataConfigurationOptions | resp_metadata_configuration_options | generic／config／profile selector 及一致的 PTZ filter 支援 |

共用消費者：兩種 Media service 的 profile renderer／catalogue snapshot；
audio／metadata state、seed／持久化；types/audio.rs、types/media.rs、
types/video.rs 的 MulticastConfiguration；相關 client／session 方法、replay
提交相依圖、action snapshot、roundtrip／token property、跨服務測試與外部 corpus。

## 決策

D1–D3 持續有效：下一 minor 使用修正後預設值、明示非串流限制、固定外部 schema，
不隱含授權攝影機寫入或發布。

- K24：Media2 audio 須使用 media-subtype 詞彙，不直接照抄 Media1 label。
  Factory codec 對應須明示，區分 G.711 law 與 G.726 bitrate variant；選定 adapter
  前先檢查 client serializer 與 Other(String) 保留行為，不新增全域有損 alias。
- K25：AutoStart 是唯讀指示，不是啟動 RTP 的請求。合成裝置不產生持續串流，
  不得由非零群組位址推導 AutoStart、透過 configuration setter 改寫，或聲稱已有
  multicast 效果。
- K36／D4，**已於 2026-09-11 核准**：舊公開 MetadataConfiguration 僅有平面的 multicast
  address／port，遺失 TTL／AutoStart 與 session timeout。Setter 將 PTZStatus
  放在 Analytics 後，並省略必要欄位。僅修 mock 會使公開 setter 失敗；捏造值不能
  安全修正 read-modify-write。建議在下一 minor 遷移公開型別，保留結構化 multicast
  設定與 session timeout，嚴格處理必要讀寫欄位，並提供 Rust／JSON 遷移文件。
  使用單一資料來源，避免新舊可寫欄位矛盾。Media2 忽略已棄用 SessionTimeout 的值，
  但仍須處理必要 wire 欄位。Optional metadata 欄位與未支援內容另行核對，
  不將有限模型宣稱為任意資料的無損 roundtrip。
- MulticastConfiguration 目前能讀 IPv6，serializer 卻使用 IPv4 wrapper；
  此共用相依項目應隨核准的公開遷移檢查，不能只在 mock 隱藏 IPv6。
  額外公開相容性變更須在施工前記錄。

## 驗證

先以自行撰寫、具區分力的測試重現基準缺陷：audio codec 詞彙、後段無效數字時
完整 state／hook 保留、metadata request 被外部驗證拒絕但 mock 接受、
非串流 AutoStart 及 multicast 資料遺失。以真實 client 呼叫與獨立 raw qualified
request 驅動 in-process／HTTP transport。

涵蓋全部 15 張工作卡的正值與確切負值、generic／config／profile selector、
特殊字元身分、重複／錯誤 namespace、公告 option 組合、唯讀欄位、
未提供與明確提供、共用 profile rendering、無效 seed，以及拒絕／提交 replay
相依性。保留既有 property，僅明確遷移無效 fixture 假設。

外部 corpus 加入全部操作及代表性拒絕。執行一次完整子群 workspace／all-feature／
no-fail-fast mutation campaign，完整還原，再集中執行 all／default Clippy／tests、
格式、嚴格文件、清冊／連結及外部 Xerces／legacy structural check。
不逐 helper 重跑完整 gate。更新雙語公開文件、CHANGELOG、audit、ledger 及量測證據；
子群完成後才 commit／push。

## 證據

VE1 已以 9016268 提交並推送；本檢查點 CI 34579594778 仍執行中。本機
all／default workspace 為 1,257／1,154 項通過，各五項 ignored；
110 份 encoder／profile／source／rate XML instance 通過。以上均非 AM1 證據。

9016268 的外部自行撰寫離線診斷位於
C:/Users/smiti/AppData/Local/Temp/oxvif-metadata-preflight-20260911。
它使用真實 OnvifClient，透過 MockTransport 先 GetMetadataConfigurations
再 SetMetadataConfiguration；兩者均回成功，未連線或使用憑證。

固定 Xerces XSD 1.1 結果：
- 擷取的原始 exchange：以 cvc-complex-type.2.4.a 拒絕。
- 僅修正 PTZStatus 順序的 setter request：仍以 cvc-complex-type.2.4.b
  拒絕（內容不完整）。
- 修正順序並由同一次合成 getter 複製 multicast／session 結尾的診斷 request：
  一份 instance 通過。此為結構控制，不是已實作修正或實機安全語意證據。

官方資源仍置於 checkout 外。規範核對：
[Media2 26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§5.2.5、§5.2.8、§5.3，以及 packaging/schema-sources.json 所固定外部 onvif.xsd
的 metadata／multicast 型別。來源已確認公開型別遺失額外資料；未對實機操作。

施工採公開 metadata 及 mock snapshot 的結構化 multicast 與 session_timeout，
取代平面欄位。舊不完整 JSON 須明確遷移，不捏造遺失的 TTL／AutoStart。
保留既有 bool 欄位，另記未建模 optional metadata 內容。Mock 儲存 multicast
設定但不啟動 RTP；AutoStart 為唯讀。Factory G711 明示對應 PCMU、AAC 對應
MP4A-LATM。固定 ONVIF AudioEncodingMimeNames 明定以 G726 代表 bitrate 變體，
因此本批保留 G726 名稱加 bitrate，修正初稿僅依 IANA subtype 的方案。
公開 Other(String) 保留設備詞彙，
不新增全域實機 codec 轉換。

接續完整子群，不採僅讓 schema 通過的暫時修補。更廣泛工作仍依施工
檢查表追蹤，本檢查點不代表 W10 或任何整體里程碑完成。

### AM1 交付，2026-09-11

- services/audio_metadata.rs 實作全部 15 張工作卡：具作用域的 selector 與候選設定、
  精確未知參照 Fault、共用 codec／options／state view，以及原子條件提交。
  Profile 使用相同音訊 view。音訊與 metadata 提交明確使相依 replay read 失效；
  profile 變更亦使其 configuration／options recording 失效。未新增 Action 或公開 client 方法。
- D4 更新公開 MetadataConfiguration、mock MetadataEntry、client setter、
  IPv6 群播序列化及既有 fixture。[遷移指南](../audio-metadata_zh.md)
  明列舊 Rust／JSON 欄位及未建模的 optional metadata；不捏造補值，也不宣稱任意無損編輯。
- K37：獨立 corpus 08 拒絕音訊 options 的 datatype。Options 必須逐一輸出整數 Items。
  舊 client 只讀第一個 Items，現在保留全部同層 Items，並相容舊式空白清單。
  固定 ONVIF AudioEncodingMimeNames 明定 G726 集合名稱，因此修正初稿僅依 IANA
  subtype 的方案；不增加全域裝置 codec alias。
- 修改前，9016268 的三個 runtime failure（1789115957_cargo_test.log）
  重現錯誤編碼名稱、誤報持續串流及接受無效寫入。K36 外部診斷保留為歷史證據。
- tests/mock_audio_metadata.rs 涵蓋 in-process／HTTP 具作用域的 read／options、
  全部宣告的音訊組合、原子拒絕與 hook、唯讀欄位、IPv6 儲存、無效 seed 及兩種 replay。
  公開 client 測試檢查必要欄位路徑、完整 wire body、傳輸前拒絕及明確 JSON 遷移。
  既有 roundtrip／token／跨服務測試保留檢查目的，改用受支援值及服務專用編碼 view。
- 整批執行一次 workspace／all-features／no-fail-fast 突變：
  1789117105_cargo_test.log。將 TTL 固定為 1、parser 限制為第一個 Items、
  停用音訊／metadata replay 失效通知。三個 target 共七個 runtime failure，
  抓到 TTL、清單遺失與兩種音訊 replay。較早的音訊 assertion 掩蓋獨立 metadata replay
  敏感度，因此不單獨宣稱該項證據。三處突變均精確還原。
- 外部 oxvif-corpus-20260911-09 的 strict Xerces XSD 1.1 通過 148／148 份 instance：
  74 組 exchange、44 項操作、57 個成功、17 個 Fault；AM1 新增全部 15 張卡的
  19 組 exchange。Corpus 08 保留為歷史失敗產物。明確執行的 legacy structural check
  亦通過：169 個 response、110 個成功、59 個 Fault、零 finding；
  1518 個略過的 child 不算已驗證覆蓋。
- 完整 workspace 測試：all-features 1272 項、default 1167 項通過，
  各 39 個 suite、五項既有 ignored。兩種 locked workspace／all-target Clippy
  與兩種嚴格 workspace rustdoc 均通過。格式、diff 空白及清冊／self-test 通過。
  最後再將負向 assertion 加強為精確 Fault payload，受影響 suite 的十項測試通過。
  變更 Markdown 的 337 個相對檔案連結均存在；此掃描不檢查 anchor。
- 移除 23 個舊 reader 呼叫：159 個 Action site、157 route、
  191 個剩餘 reader（production 176、test 15；production symbol 56）。
  這是來源清冊，不是已驗收的 ONVIF 操作數。
- Windows x64，rustc 1.97.0（2d8144b78）、cargo 1.97.0（c980f4866）。
  Schema manifest SHA256：
  65c7a558d9d96dda26eec5ef5fedaef33ae9c55e4ee00de23ca37a9650bdeb08。
  官方資源及產生的驗證檔案均保留在 repo 外。

重跑時使用檢查表的 locked workspace gate，加入
--target-dir target/mock-fidelity-build。外部 exporter 為
tests/mock_schema_corpus.rs 的 export_reviewed_batches_for_independent_validation，
OXVIF_MOCK_CORPUS 必須指定新的 repo 外絕對路徑。
依 schema preflight 的固定 --root、--tool-root、--java 及 --corpus，
執行 packaging/verify_schemas_xerces.py validate。缺少資源或略過 exporter 均不算通過。

後續為 W10 其餘 URI／OSD／capability 收尾及其他服務批次。
W04／W06／W07／安全語意、更廣的 schema／語意覆蓋、feature／平台驗收及
PR #16 整合仍未完成。本子群不代表 W10 或 M0–M6 完成。
未寫入真實攝影機、建立 Release／tag／publish、安裝或合併主分支。
VE1 託管 CI 34579594778 已通過，AM1 CI 另行追蹤。
