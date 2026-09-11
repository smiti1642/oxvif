# 社群 PR 整合計畫

[English](contributor-pr-integration-plan.md) | [繁體中文](contributor-pr-integration-plan_zh.md)

日期：2026-09-11。本機審查基準：`codex/mock-fidelity-hardening` 的 `ad564cb`。
狀態：**IN-PROGRESS；2026-09-11 已授權實作**。候選分支為基於 `ad564cb` 的
`codex/contributor-pr-integration`；不得以單純功能整合範圍發布其 hardening 歷史。
PR 內的驗證清單為作者陳述，不等於維護者驗收。

| 章節 | 用途 |
| --- | --- |
| [範圍與版本](#範圍與版本) | 固定貢獻版本及工作邊界 |
| [設計決策](#設計決策) | 建議預設方案與升級討論條件 |
| [B17 通知來源](#b17-通知來源) | 相容 API 與確定性 listener 測試 |
| [B16 媒體同步](#b16-媒體同步) | 保留 Client 與誠實的 Mock 政策 |
| [B14 相依套件](#b14-相依套件) | 獨立的相依套件驗收 |
| [驗證節奏](#驗證節奏) | 批次品質關卡與證據重用 |
| [整合與署名](#整合與署名) | 分支歷史、貢獻歸屬與回復 |
| [交付清單](#交付清單) | 文件與有限的完成條件 |
| [執行紀錄](#執行紀錄) | 實測結果與待補證據 |

## 範圍與版本

| 批次 | 已審 head | 2026-09-11 觀察到的目標 | 處置 |
| --- | --- | --- | --- |
| B17／[PR #17](https://github.com/smiti1642/oxvif/pull/17) | `89f8125863668006dda395f0fd54253ab5a4aac5` | `develop`，`9dccf9df4099183d7b1edbb1941c7610ab27dfc0` | 保留 peer 擷取，重整公開介面及測試 |
| B16／[PR #16](https://github.com/smiti1642/oxvif/pull/16) | `3db6459b03b6977b3a41b6055ee3f8ac9f42b049` | `master`，`9dccf9df4099183d7b1edbb1941c7610ab27dfc0` | 保留 Client／Session 貢獻，替換舊 Mock handler |
| B14／[PR #14](https://github.com/smiti1642/oxvif/pull/14) | `7c4db43efe4dbc28f8ed2093f5d08fde5eeb5114` | `master`，`03b8d30dd8a78a6224a13035978f7fa94ae5f9e1` | 獨立審查；舊 CI 通過不等於合併後版本通過 |

各批開工及整合前重新取得 head、base 與 checks。Head 改變須審查增量並更新受影響
的驗證證據。PR #14 標題與內文的更新數量不同，以實際 manifest／lock diff 為準。
不得將空白 checks 清單（本次 #16／#17）解讀為 CI 通過。

維護者已明確將 #14 納入本輪。順序為 B14、B17、B16，分別提交；先固定新的相依
基準，避免後續程式以立即過時的相依版本完成驗收。不必等待 W00–W25 全部完成才設計這些功能；
但完成這些功能不代表 Mock 可信度計畫完成，也不授權發布未驗收的前置工作。
本次規劃不包含 Release、升版、安裝、新 CLI 命令、force-push 貢獻者分支、PR
留言／關閉或主分支合併。

## 設計決策

| ID | 建議的實作決策 | 僅於下列情況再次討論 |
| --- | --- | --- |
| I1 | 保留 `NotificationMessage`、既有 struct literal、serde 格式與 listener signature | 無法用新增 API 完成，必須破壞既有介面 |
| I2 | 新增 `ReceivedNotification { message, peer }` 及明確選用的 listener；`peer` 是 TCP `SocketAddr`，不是已驗證的攝影機身分 | 需要公開的身分／信任／代理 header 機制 |
| I3 | Media1／Media2 分別使用既有 exact-operation acknowledgment 政策；預設拒絕，opt-in 僅表示收到請求 | 要求真實串流產生或變更擬真度預設 |
| I4 | 保留 Events synchronization 的既有語意／命名；採用 PR 的兩個 Media 方法名稱 | 實際整合基準發現 API 命名衝突 |
| I5 | 保留貢獻者署名，維護者修正以完整驗證的批次提交 | 需要作者協調或新增遠端操作權限 |
| I6 | 相依套件獨立更新，保留 Rust 1.88、CLI schema v3 與既有行為契約 | 升級需要調整 MSRV／schema 或無關的大型遷移 |

這些預設方案避免再次展開無限的設計討論。後續實作授權可整體接受本計畫；目前
沒有額外必須由維護者決定的產品事項。

## B17 通知來源

執行負責者：維護者。狀態：TODO。不依賴 W15 全部完成。

### 檔案與 API

- `src/types/events.rs`、`src/types/mod.rs`、`src/lib.rs`：匯出新的包裝型別，
  不替 `NotificationMessage` 加欄位。規劃欄位為 `message: NotificationMessage`
  與 `peer: std::net::SocketAddr`；推播連線一定有 peer。不將此欄位放進 ONVIF
  parser 或 PullMessages。
- `src/client/events.rs`：新增 async
  `notification_listener_with_peer(bind_addr) -> std::io::Result<...stream...>`。
  回傳前完成 bind，呼叫者可以取得 bind 錯誤並確定已可接收連線。抽出接受已綁定
  `TcpListener` 的 private helper；測試綁定 port zero 後持續持有 listener，
  不先釋放 port 再重新綁定。
- 保留原本同步 `notification_listener` 的回傳型別與訊息內容。共用解析／派送，
  舊 API 將包裝型別映射為 `message`；文件說明舊 API 仍有既有 bind 錯誤回報限制。
- 採用實際 `accept()` 位址，不以 `Forwarded`、`X-Forwarded-For`、XML Source
  或 reverse DNS 取代。不預設記錄來源位址。初始範圍不替包裝型別自動 derive
  serde；舊事件 JSON 不變，位址匯出由呼叫者明確決定。
- Listener／connection task 的生命週期應與 stream 綁定：receiver drop 後停止
  accept，取消並收束其擁有的連線 task。保留既有訊息／body 限制；不將此工作
  擴充為新 HTTP／TLS／認證伺服器，也不宣稱既有 listener 已適合直接暴露於網際網路。

### 驗收案例

| ID | 必須具備辨識力的證據 |
| --- | --- |
| N01 | Loopback 發送端 `local_addr()` 等於收到的 `peer`；同時斷言 topic／時間／source／data |
| N02 | 兩個並行發送端傳送相同 XML，仍能以完整 socket address 區分；逐筆建立對應 |
| N03 | 同一 POST 的多個通知保留相同 peer 與各自 payload；不假設跨連線抵達順序 |
| N04 | 舊 listener 與 PullMessages 的 payload 行為不變；外部 consumer 的舊 struct literal 仍可編譯，serde JSON 相同 |
| N05 | 新 async API 對已占用位址回傳 OS bind 錯誤；啟動依賴 readiness／持有 listener，而非固定 sleep 或搶占 free port |
| N06 | 在 idle accept 與未完整傳送的連線期間 drop stream；以有上限的 timeout 證明所屬 task 結束 |
| N07 | IPv4 通過；環境支援時 IPv6 loopback 通過，否則明記無法取得證據，不得默默視為成功 |
| N08 | Notify fixture 有完整 namespace 宣告；誤導性的代理 header 不得覆寫 TCP peer；malformed input 不產生虛構事件 |

主要測試：`src/tests/client/events_tests.rs`；必要時新增公開 API 回歸測試
`tests/notification_origin.rs`。PullMessages 使用既有 transport helper，struct literal
相容性使用獨立下游編譯驗證。Mutation：置換錯誤 peer、混淆各連線歸屬，N01／N02
的目標 assertion 必須失敗；驗收前精確還原。

文件須區分來源 port 與 ONVIF service port，說明 NAT／proxy 限制，並明示此位址
是本機 transport metadata，不是 ONVIF wire extension，也不是攝影機認證機制。

## B16 媒體同步

執行負責者：維護者。狀態：TODO；對應 W26。參閱
[PR 詳細審查](mock-fidelity-pr16-integration_zh.md)。本計畫取代該文件的舊前置工作
順序，但不撤銷尚未解決的發現。目前基準已有 scoped request、structured fault、
profile lookup 與 `AckOnlyOperation`；應確認實際傳遞相依性，不重新實作這些設施。

### 檔案與行為

- 重用 `src/client/media.rs`、`media2.rs` 與 `src/session.rs` 的貢獻：
  `media_set_synchronization_point`、`set_synchronization_point_media2`。
  保留完整且分 service 的 Action、token escaping 與 response 檢查。
- 將提案中 `src/mock/services/media.rs` 的文字搜尋 handler 替換為 scoped request
  實作，於 `src/mock/dispatch.rs` 加入不同路由。施工前完成兩張 operation card，
  包含既有 C01–C12 面向。
- `src/mock/policy.rs` 新增不同的 Media1／Media2 身分。Events variant 不得啟用
  任何一者。保留共用 identity／auth／policy 優先順序：預設 unmodeled-effect
  拒絕可先於操作欄位檢查；opt-in 請求仍須通過 scoped field／profile 驗證才能 ack。
- Opt-in 時拒絕 absent／duplicate／mislocated／wrong-namespace token 與不存在
  的 profile，使用已審查的 structured fault。文字僅 decode 一次，保留 token
  有意義的空白，以一致的 state snapshot 進行檢查。
- 拒絕與 opt-in acknowledgment 均不得改變 state、hook、event queue 或淘汰 replay
  紀錄。不宣稱 commit、I-frame、PTZ refresh 或 RTP delivery。明確選用的 raw／
  recorded fixture 保留已記載的優先順序，另行驗證，不與 synthetic 預設政策混淆。
- 視適用範圍擴充 `tests/mock_ack_policy.rs`、Client Media 測試、action snapshot、
  workflow、token discrimination、replay 與 corpus 測試。移除無條件成功的期待，
  不為了保留測試而放寬預設政策。

### 驗收案例

| ID | 必須具備辨識力的證據 |
| --- | --- |
| S01 | 各 Client 送出精確 Action／body／endpoint，包含 escaped 與 Unicode token；Session 選對 service |
| S02 | 各 Client 保留精確 SOAP Fault payload，拒絕錯誤 response wrapper；不得只使用 `is_err()` |
| S03 | Media1／Media2 預設均拒絕；opt-in 僅啟用所選操作，不影響另一個 Media service 或 Events |
| S04 | Opt-in 的 known／unknown／兩個 profile／特殊 token，以及 duplicate、namespace／Header／Extension 誘餌案例有精確結果 |
| S05 | 拒絕及 ack 後，完整 state snapshot、hook 次數、queue 與已錄製讀取結果均不變 |
| S06 | In-process 與 HTTP 一致；adapter／replay 不得意外繞過 synthetic policy；raw fixture 另設控制 |
| S07 | 固定外部資源獨立驗證 request／response／Fault 通過；成功／控制與拒絕 instance 分開計數 |
| S08 | 清冊對齊新增 Client Action、route／reader／card 與雙語 operation table；Media 成功宣稱不得套用到 Events |

Mutation：錯置 Action、繞過 policy、破壞回傳 Fault 或選到誘餌 token。規劃的子群
campaign 必須在預定 assertion 失敗。Mock 測試無法證明真實 I-frame。選擇性的
真實攝影機驗證，須另外取得 synchronization request 授權並觀察受影響串流；
CLI discovery 或 HTTP 成功不等於該證據。

依 pinned-source 政策重查官方 Media1／Media2 Service 文件與 WSDL；規範表格、
下載來源與 schema 衍生資料放在 repository 外。文件描述 profile 關聯串流同步，
而非僅 video I-frame；不得從舊 Mock 推論規範 Fault 對應。

## B14 相依套件

狀態：TODO，已納入本輪並可獨立審查。參閱[維護政策](dependency-maintenance-plan_zh.md)；
既有驗收僅涵蓋更早的 PR，不包含 #14。對選定 base 檢查目前 `Cargo.lock` 與
`crates/oxvif-cli/Cargo.toml` diff，包含 transitive 更新與已套用版本，不能整份
覆蓋回過時的 lockfile。

已審 PR 僅修改這兩個檔案，沒有修改 Rust 原始碼或 workflow。Manifest 變更位於
CLI 的 **dev-dependencies**：jsonschema 0.52.1 升至 0.53.0、shlex 1.3 升至 2.0。
Lockfile 同時更新 futures／futures-core、thiserror、toml 等 production dependencies
及其傳遞相依。因此，僅相依更新不代表僅影響測試，也不代表行為自動相容。

依上游 release 與實際呼叫位置，審查 thiserror、futures／futures-core、toml、
jsonschema、shlex。尤其確認 shlex major version 移除的 API、TOML 設定往返、
JSON Schema 接受／拒絕行為、async 行為及 Rust 1.88 相依解析。記錄安全／授權
通知與新增 feature default，不順便更新無關套件。

驗收：locked build／tests、兩種 Clippy、MSRV、`cargo audit`、審閱
`cargo outdated`、受影響 CLI config／schema 測試、XML feature-unification guard，
以及目前版本的原生 CI。若 packaging input 改變，在最終交付候選執行 package
dry-run 及不發布的 staging；不代表授權安裝到使用者系統或公開發布。不壓掉
advisory 來取得通過結果。

## 驗證節奏

遵循[已核准批次節奏](mock-fidelity-execution-checklist_zh.md#批次與驗證節奏)。
每批完成程式、針對性測試與雙語文件後才進行驗收及完整提交，不逐一 helper 跑全套。

1. 記錄精確 base／head／toolchain／dependencies／features，重用輸入相符的綠色
   baseline 證據；施工期間使用編譯與 focused tests。
2. 每個程式子群一個事先規劃的敏感度 campaign；依 repository 政策使用未過濾的
   all-feature `--no-fail-fast` 執行。記錄真正失敗的 assertion；前面的失敗不證明
   後面的檢查也有辨識力。
3. 還原 mutation 後，對候選版本執行一次各項程式提交關卡：

   ```text
   rtk cargo fmt --all --check
   rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
   rtk cargo clippy --locked --workspace --all-targets -- -D warnings
   rtk cargo test --locked --workspace --all-features --no-fail-fast
   rtk cargo test --locked --workspace --no-fail-fast
   ```

4. 公開文件改變時執行兩種 strict rustdoc 與 doctest。B16 執行 inventory self-test
   及外部 schema corpus／shape 驗證；B17 的獨立 XML 檢查須拒絕未宣告 prefix 的
   fixture。Schema 通過是結構證據，不是 ONVIF 認證。
5. 整合時執行 Rust 1.88、各支援 feature 的 Clippy 與下游 feature-unification
   控制。Workspace default 可能透過 CLI unify features；須另含真正 default-feature
   的 library consumer。
6. 在精確合併候選取得 Windows／Linux／macOS CI。既有 workflow 僅自動執行特定
   分支名稱；新增 integration branch 在取得授權後使用既有 manual dispatch，
   不為了觸發 CI 另開維護者 PR。無法執行／尚在執行者記為 NOT-RUN／PENDING，非 PASS。

失敗後先修正並重跑受影響檢查，再建立變更後候選的最終關卡證據。相同最終程式的
證據可附原 SHA／tooling 重用；相依套件改變使受影響的舊證據失效。不為每個小修正
重跑完整 release／staging matrix。

## 整合與署名

1. 取得實作授權後才施工。檢查本地／遠端 ancestry 與未提交檔案。B17 可從目前
   `develop` 建立 `codex/pr17-notification-origin`，不引入 hardening；B16 可從
   已驗收的 hardening 前置建立 `codex/pr16-media-sync`；B14 使用獨立維護分支。
   這些名稱目前均為規劃。
2. 重用程式保留原作者／commit 來源。優先採用有署名的移植與維護者修正，在
   完成整批驗證後提交；若 squash 或選擇性重寫，保留 PR 連結與適當貢獻歸屬，
   不偽造作者的同意或審查通過。
3. 檢查完整 merge diff 與 ancestry，不只查看功能檔案。以未完成 hardening 為基底
   的分支不得藉本計畫整條合進 master。必須先獨立驗收 base、另行審查最小前置
   集合，或等待主分支整合；此時 B17／B14 不必跟著等待 B16。
4. 優先單向整合到 `develop`，驗收精確候選後，才在具備明確合併授權及已接受
   ancestry 的情況同步 `master`。目標移動須重查，衝突解決須測試。
5. 不預設另開維護者 replacement PR。原 PR 可安全更新／合併且具授權時保留原
   管道；否則在實際整合後，另取得遠端操作授權，以留言說明有署名的實作並標記
   superseded 關閉，而非提前關閉。
6. 驗證前保留分支與證據。整合後有問題，以明確 revert commit 回復，不用
   force-push／reset 或刪除貢獻者成果。不自動 Release、cargo publish、tag、
   系統安裝或刪除分支。

## 交付清單

| 項目 | 狀態 | 結案條件 |
| --- | --- | --- |
| B17 | LOCAL-PASS | N01–N08、相容性／生命週期控制與兩組程式／文件關卡通過；hosted CI 待完成 |
| B16／W26 | TODO | S01–S08 證據、完整工作卡與明確 acknowledgment 邊界 |
| B14 | LOCAL-PASS | 相依審查、audit、Clippy／tests、MSRV／XML 通過；hosted CI 待完成 |
| 合併候選 | TODO | 精確 SHA 與 ancestry 已接受；最終 tests／CI 真正完成 |
| 遠端整合 | NOT AUTHORIZED | 明確授權、有署名的 commits 與驗證完成的目標分支 |

各批更新公開 method／type rustdoc、`src/lib.rs`、CHANGELOG Unreleased 及
`LIBRARY_GUIDE.md`／`_zh.md`。B16 另更新 `OPERATIONS.md`／`_zh.md`、Media reference
雙語文件、mock-server 雙語文件、相關 examples、operation ledger、source inventory、
W26 與詳細審查。README 僅在必要時增加短連結，不加入 API 細節。新 CLI 命令及其
人類／Agent schema 不在範圍內；間接造成 CLI 行為改變則視為回歸問題調查。

長文件保留章節導航表與互相切換的語言連結。規劃與發布宣稱分開，未實作的功能
不列入 Added。最終報告列出已接受 commit、精確測試／check 結果、未取得的證據、
相容性變更與延後的無關發現。B17／B16 完成條件不包含新發現的無關服務修復。

## 執行紀錄

B14 候選重現已審查的相依版本，未用舊分支 lockfile 覆蓋目前內容，也未修改 Rust
程式或 workflow。僅於 CLI 範例解析測試使用 `shlex::split`，未使用移除的 quote／
join 或 mutable-deref API。jsonschema 仍停用 default features。檢查的套件宣告
MIT 或 MIT/Apache-2.0 授權，MSRV 均不高於 1.85；仍須實際驗證 Rust 1.88，不能
僅依宣告推論通過。上游說明：[shlex](https://github.com/comex/rust-shlex/blob/master/CHANGELOG.md)、
[jsonschema](https://github.com/Stranger6667/jsonschema/blob/master/CHANGELOG.md)、
[futures](https://github.com/rust-lang/futures-rs/blob/main/CHANGELOG.md)。

2026-09-11 的 `cargo audit` 對 410 個 locked dependencies 通過。
已審閱 `cargo outdated --workspace --root-deps-only`：較新的 dirs、reqwest、
tokio-rustls、toml、ipnet、jsonschema、keyring 不在 #14 的有限更新範圍。
Keyring obsolete feature 警告針對候選 keyring 4，不代表目前保留的 keyring 3.6.3
已變動。

B14 本機關卡通過：all-features 1,272 項、workspace default 1,167 項，各有五項
既有 ignored、39 個 suites；兩組 Clippy 與 fmt 通過。Rust 1.88 locked workspace／
all-target／all-feature check 通過。獨立 XML consumer 在 encoding 關閉／開啟時
各通過三個案例。此批僅更新相依，未新增 production 邏輯或 assertion，因此不另跑
新的 mutation campaign。合併候選的 hosted 驗收仍待完成。

B17 已實作新增包裝型別／listener、維持舊 payload／serde 及所屬 task 清理。
九個 origin unit 控制與兩個外部 consumer 控制涵蓋 N01–N08；本機 Windows 的 IPv6
loopback 實際通過。獨立 namespace 控制發現 fixture 預期數量不正確（未宣告
element prefix 出現八次而非六次）；修正預期後正／負控制通過，無須改動 production
parser。單次完整 workspace 敏感度 campaign（本機 RTK 證據目錄的
`1789119597_cargo_test.log`）將 peer port 改為零，五個 origin 測試在精確位址斷言
失敗，包含多連線歸屬。擾動已移除。Strict docs、inventory self-test 與兩組 Clippy
通過；還原後 all-features 為 1,284 通過、五項 ignored、40 suites。
還原後 default 測試亦通過（1,179 通過、五項 ignored、40 suites）。最終 hosted
驗收仍待完成，不代表 ONVIF 認證或 Release 關卡已完成。
