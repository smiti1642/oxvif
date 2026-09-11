# Video source 批次 VS1

[English](mock-fidelity-video-source.md) | [繁體中文](mock-fidelity-video-source_zh.md)

基準：`7e9b68f`，2026-09-11。負責者：目前 hardening 工作。W01／W10／W17／W18／W19。

| 章節 | 用途 |
| --- | --- |
| [操作卡](#操作卡) | 完整來源子群 |
| [決策](#決策) | 已審閱契約及相容邊界 |
| [驗證](#驗證) | 案例、擾動及 gate |
| [證據](#證據) | 實際結果與下一子群 |

## 操作卡

Action 為以下 namespace 加上 `/` 及各 ID 的操作名稱；body 使用相同 namespace 與名稱。
Media1：`http://www.onvif.org/ver10/media/wsdl`；Media2：
`http://www.onvif.org/ver20/media/wsdl`。Client 入口為 `src/client/media.rs`、
`media2.rs`，對應 session wrapper 位於 `src/session.rs`；endpoint 維持共用 synthetic
Action resolver，不新增 HTTP path 限制。

| ID | 現有 handler 與來源輸入 | 輸出／目標 |
| --- | --- | --- |
| media.GetVideoSources | `media::resp_video_sources(state)`；忽略 body | 實體 source catalogue，token 未轉義；驗證空操作並轉義身分 |
| media.GetVideoSourceConfigurations | `media::resp_video_source_configurations(state)`；忽略 body | 全部 configuration；驗證空操作並共用轉義 renderer |
| media.GetVideoSourceConfiguration | `media::resp_video_source_configuration`；全域必填 ConfigurationToken | 單筆 configuration 或平面錯誤；改直接且唯一 scalar 與 structured NoConfig |
| media.GetVideoSourceConfigurationOptions | `media::resp_video_source_configuration_options`；全域必填 ConfigurationToken，忽略 ProfileToken | 上限來自可變 crop，profile 上限寫死 5；改支援選填 selector 與 sensor 範圍 |
| media.SetVideoSourceConfiguration | `media::handle_set_video_source_configuration` → `apply_video_source_write`；全域 Configuration token、Name、SourceToken、Bounds width／height，忽略 offset／persistence | 存在檢查與寫入分鎖、錯誤數字被忽略；改完整 scoped candidate、原子提交與明確拒絕未建模設定 |
| media2.GetVideoSourceConfigurations | `media2::resp_video_source_configurations_media2(state)`；忽略兩個 selector | 重複且未轉義 renderer；改 scoped ConfigurationToken／ProfileToken 與共用 renderer |
| media2.GetVideoSourceConfigurationOptions | `media2::resp_video_source_configuration_options_media2`；同樣全域 token 與 crop 上限問題 | 相同 sensor-derived options，維持不同 service wrapper |
| media2.SetVideoSourceConfiguration | `media2::handle_set_video_source_configuration_media2` → 共用 writer | 相同原子狀態契約，但沒有 Media1 persistence 成員 |

共用路徑：`VideoSourceConfigEntry`、`VideoSourceEntry`、`MockState::modify_returning_if`、
透過 `media::render_vsc_body` 的兩種 profile renderer、`request::Node`、`fault.rs`、
`effect.rs` 與內建 replay。Client 的 `VideoSourceConfiguration`、`SourceBounds`
序列化完整已建模值；不計畫修改公開欄位或簽章。

實作前記錄的來源確認風險：**K31** options 上限隨 crop 變動及未驗證／丟棄的 bounds；
**K32** 全域／缺少／重複 selector、巢狀輸出未轉義與部分寫入；**K33** source 寫入尚未
得到 synthetic 結果，replay 即已失效。Runtime 重現及處置記於下方。

## 決策

已審閱一手參考：[Media1 v24.12](https://www.onvif.org/specs/2412/ONVIF-Media-Service-Spec-v2412.pdf)
§5.3–5.4、[Media2 v26.06](https://www.onvif.org/specs/2606/ONVIF-Media2-Service-Spec-v2606.pdf)
§5.2.2、§5.3.2–5.3.4，以及 `packaging/schema-sources.json` 固定外部來源。
Schema 表格與衍生 fixture 均保留於 checkout 外。

- Dispatch 僅解析一次；handler 使用具 namespace 的 operation 與 configuration。
  保留解碼身分／Name；重複 scalar／configuration、錯誤數字須在寫入前拒絕。
  UseCount 驗證型別但不允許 caller 改寫。
- 要求完整已建模 source configuration；僅儲存零原點 crop。非零 offset 與未建模
  configuration 設定明確拒絕，不默默丟棄。正值 crop 可調整至所選 sensor 範圍，
  後續 getter 回傳實際提交大小；不能儲存不存在的 SourceToken。
- Media1 persistence 輸入驗證 boolean；mock 僅提交記憶體狀態並呼叫既有使用者管理的
  persistence hook，不宣稱模擬重啟或磁碟效果。
- 空清單請求列舉；Media2 明確未知 configuration／profile 回 Fault。Profile context
  驗證存在，但目前模型允許所有 profile 重新指定 source，沒有實體 encoder-routing
  衝突模型，不宣稱等同設備相容性。Generic／僅 profile 的 options 保守聚合可用 source；
  明確 configuration options 依其實體 sensor，而非目前 crop 大小。
- Options 公告零原點、正值尺寸、已建模 profile 容量與實際 source token。讀取不修復
  使用者自行匯入的無效 snapshot。
- 已審閱的未知 configuration／profile 與無法設定錯誤採 structured Sender／InvalidArgVal，
  分別搭配 NoConfig、NoProfile、ConfigModify。XML 結構錯誤沿用共用邊界政策；
  模型限制明確標示。
- 成功提交通知一次並淘汰內建 replay 的 source／profile／options 相依錄製；拒絕須保留
  完整 state、hook 計數與 recording。獨立 replay constructor、完整 HTTP binding、encoder
  設定及實際媒體效果仍為獨立工作。

## 驗證

`tests/mock_video_source.rs`：`source_fields_and_atomicity`（C01–C06、C09）、
`source_selectors_and_options`（C01–C05／C07）、兩者 HTTP wrapper（C10／C11）、
`source_replay_commit_boundary` 及 HTTP wrapper（C06／C11）。必須斷言完整 snapshot、
兩個不同 sensor 上限、Unicode／特殊字元／空白身分、重複與巢狀 decoy、最後欄位錯誤、
source 重設、唯讀計數、clamp／回讀、generic／profile selector、精確 structured Fault，
以及拒絕後不變的狀態。保留既有 multi-sensor、roundtrip、profile assembly 與跨服務測試。
獨立外部 source corpus 包含成功讀寫與拒絕；共用 client parser 往返不算 namespace／XSD
證據（C07／C08）。

先以 focused 舊碼回歸重現，再完成整個子群並在必要節點跑針對性檢查。整批僅規劃一次
unfiltered workspace／all-feature／no-fail-fast 擾動，改變 source 選擇／原子提交後精確還原。
最終執行一次五項 gate、strict docs 與相關外部檢查；修復時重跑受影響檢查，不重複無關基準。
雙語指南、CHANGELOG、清冊、audit 數字與本批證據同步更新。
本批不授權實機寫入、發布、安裝或合併主分支。

## 證據

八張工作卡已透過共用 qualified `video_source` helper 實作；完整 candidate 先驗證再一次
提交，讀取與 renderer 保留 decoded identity，options 不隨 crop 縮小，內建 replay 僅在
成功後淘汰相依錄製。更廣契約的清冊列仍為 PARTIAL。

Windows、隔離 target、已還原程式碼的驗證：

- 舊碼回歸重現錯誤末欄位仍成功，以及 options 隨 crop 縮小；保留既有雙 sensor／client workflow 控制。
- 整批一次 workspace／all-feature／no-fail-fast 擾動，同時忽略 source 選擇並在拒絕時
  修改 state；35 suites 中有十二項 runtime assertion 失敗（log：
  `1789111312_cargo_test.log`），涵蓋 source options、完整拒絕狀態、replay 與 corpus。
  兩項擾動均已精確還原；聯合擾動不等於每個欄位或併發規則均已獨立驗收。
- 最終 fmt 與兩種 workspace／all-target Clippy 均通過，warnings 視為錯誤。
  全功能 **1,238 passed、5 ignored**；預設 **1,137 passed、5 ignored**，各 35 suites。
  收尾審查將過長 selector 的錯誤改為 generic InvalidArgs，避免誤用 setter 的 ConfigModify；
  重跑受影響的全功能 Clippy／測試均通過，預設行為未受影響。
- 兩種 strict rustdoc 通過；明確執行外部 legacy shape check 通過。
  Strict Xerces XSD 1.1 對外部 `oxvif-profile-corpus-20260911-05` 的 **70／70** instance
  通過（35 組 exchange、21 操作、十份 Fault）。
- 清冊與 self-test 通過：159 Action 宣告、157 路由、**233** 個直接 reader
  （218 個位於頂層 test module 前、15 個位於其中；68 個 production enclosing symbol）。
  VS1 移除五個 legacy call；這不是完成操作數。

更廣 scalar／operation attribute 規則、任意匯入 snapshot 驗證、實體 routing 相容性、
完整 HTTP binding 與所有一般 Fault 分支仍未完成。此結果不是 ONVIF 認證、三平台原生
驗收、實機寫入測試或 release 核准；沒有替換系統 binary。

下一完整子群為 encoder configuration／
options／instances（其餘八個 video 操作），再接 audio／metadata。VS1 不代表 W10 或全案結案。
