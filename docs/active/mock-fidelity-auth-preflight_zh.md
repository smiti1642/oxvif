# Mock 認證開工盤點

[English](mock-fidelity-auth-preflight.md) | [繁體中文](mock-fidelity-auth-preflight_zh.md)

W08／W02／W06，2026-09-11，基準 `9f390d1`。此設計於實作前記錄。
本項為有界認證 parser 遷移，不是正式環境安全驗收。

| 章節 | 用途 |
| --- | --- |
| [範圍](#範圍) | Reader、state 與公開相容性 |
| [政策](#政策) | 支援的 token 表示及排除範圍 |
| [驗收](#驗收) | 具辨識力的回歸與下游檢查 |

## 範圍

`auth::validate_ws_security` 目前在整份 raw XML 以四個 local name 搜尋，可能
接受 Body／foreign element 中的憑證，並裁切身分空白。改用有界 request tree，
不變更預設 auth-off、精確 GetSystemDateAndTime 豁免、fault／auth／raw／replay
順序或公開錯誤結構。一般缺少憑證的診斷維持穩定；結構／不支援 token 的錯誤須
明確，且不得回傳 username、digest、nonce 或 raw XML。Device-user writer、角色
授權及共用憑證更新 transaction 另屬 W13／W18。

## 政策

僅讀取一條 qualified SOAP 1.2 Header／Security／UsernameToken 路徑，
Username／Password／Nonce／Created 須為唯一直接 scalar 欄位。僅解碼一次，
保留身分與 timestamp 文字。要求明確 PasswordDigest Type；缺少 Type 不等於
宣告 digest。Nonce EncodingType 可省略或為支援的 base64。拒絕重複／巢狀憑證
與不支援的 SOAP recipient role。精確比對 username，不正規化持久化 user。
不解析外部 entity。Base64 可於解碼時處理 lexical whitespace，username／Created
不得因此裁切。

此 mock 仍未實作 timestamp freshness、nonce replay cache、PasswordText、
HTTP Digest、完整 WS-Security header 處理或 user-level authorization。
不得公告這些保證，也不改預設政策以模擬支援。維持 Sender／wsse:FailedAuthentication
與 CLI 分類；私有診斷 reason 可更明確，但不得反射輸入。參考審閱：OASIS
UsernameToken 1.1.1 §3.1 與 ONVIF Core v24.12 §§5.9.4–5.9.5；規範筆記
保留於外部，不封裝 schema-derived fixture。

## 驗收

兩種 transport：不同 user 的正式 client 正向控制、特殊字元／空白身分解碼；
raw prefix／CDATA／header 控制；Body／foreign／巢狀／重複憑證、錯誤／缺少
Type、錯誤 EncodingType／base64、malformed／超界輸入及錯誤 digest 負向控制。
斷言精確 Fault payload、完整 state／hook 不變及回應不含憑證 marker。
驗證 auth-off、精確豁免、fault-before-auth 順序、即時 user table 與 CLI 人類／
Agent 契約。將碰到的空泛 unit assertion 改為精確 reason。修正前重現，執行完整
all-feature／no-fail-fast 擾動並還原，再跑五項 gate、strict docs 及雙語 inventory。
不得從既有 40 份 profile instance corpus 推論 auth instance 已通過外部驗收。

實作檢查點：`Request::header` 將認證範圍與 synthetic Action／body 處理分開；
`auth::field` 要求唯一、解碼後的 scalar 憑證。查詢即時 user table 前先檢查
Password／nonce 宣告與支援的 recipient role。Nonce bytes 須非空；base64 空白
處理不影響 Username／Created。公開錯誤結構及一般缺少憑證的 CLI 行為不變。
私有 unknown-user、nonce-decode、digest-mismatch reason 不再反射請求資料。

舊程式碼重現：兩個新 transport 測試均因特殊字元／空白身分失敗
（`1789102024_cargo_test.log`）。完整 workspace／all-feature／no-fail-fast
擾動中，接受第一個重複 Security 使兩種 transport 的拒絕斷言失敗；變更 mismatch
reason 使兩個強化後的 unit assertion 失敗（`1789102471_cargo_test.log`）。
兩項擾動已還原。負向 fixture 產生器會斷言請求確實被變更。過大的 HTTP 請求
維持既有 transport `413`；程序內請求將 parser 上限映射為認證 Fault，兩者
分別驗證，不強制等同。

舊 fragment unit fixture 已改為 qualified SoapEnvelope／UsernameToken，並精確
斷言成功／錯誤 payload。兩種 transport 亦涵蓋 base64 空白／預設 nonce encoding、
literal Created、raw prefix／CDATA、忽略 foreign decoy 及即時密碼更新。
舊 timestamp 與重複有效 token 仍刻意可通過，不宣稱已防止重播。

Windows 本機驗證：workspace all-feature 測試 1,217 passed、default 測試
1,121 passed，各為 32 suites、5 個條件式 ignore；兩種 Clippy、格式、strict
default／all-feature rustdoc 及清冊自我測試均通過。清冊目前為 159 個 Action
使用點、157 條 route、243 個直接 reader（正式程式 228、測試 15、71 個 symbol）。
這些是本機結果，不是 release 驗收。前一筆 adapter commit `9f390d1` 已通過
CI run `34563256364`。
