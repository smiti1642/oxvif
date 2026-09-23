# 認證時間與 nonce 防重用 AF1

[English](mock-fidelity-auth-freshness.md) | [繁體中文](mock-fidelity-auth-freshness_zh.md)

狀態：DONE（AF1）。2026-09-23、基準 `3e5d2b2`，修改前建立 W01。
負責：Q02／W08；保留既有 scoped auth 驗收。

| 路徑 | 現況 | 預定效果／拒絕 |
| --- | --- | --- |
| MockState | 私有 state 與 volatile subscription runtime | 私有有界 nonce cache、測試時鐘；不改公開欄位與持久化 |
| AuthResponder → validate_ws_security | Scoped Header、live user 密碼、digest、精確免認證 Action | Digest 後才檢查時間與原子 nonce admission；保留 fault／auth／raw／synthetic 順序 |
| 四種內建 transport | 啟用才走共用 gate | 同 state 的 clone 共用 cache，其他 instance 隔離；停用／免認證不消耗 cache |
| 認證失敗 | 固定 Sender/wsse:FailedAuthentication，不寫狀態／hook | 時間／重用／容量固定 reason，不反射輸入、不改 cache |

參考：[OASIS UsernameToken 1.1.1 §3.1](https://docs.oasis-open.org/wss-m/wss/v1.1.1/os/wss-UsernameTokenProfile-v1.1.1-os.html)。
規範建議 freshness 與 nonce retention；以下數值是明示 mock 政策，不宣稱完整 WSSE。

Created 接受過去最多 300 秒、未來最多 60 秒，邊界包含；限精確 UTC 秒格式，時間
檢查可去除外側 XML 空白，但 digest 仍 hash 原 decoded Created。Nonce 以 decoded
bytes 為身分，最多 256 bytes、cache 4096 件；保留到建立時間的 freshness interval
及 admission 後 300 秒都結束，不為新請求淘汰尚有效 nonce。單鎖驗證／插入，
重用／時間／digest／容量拒絕不改 cache。成功 auth 即消耗 nonce，後續操作拒絕不退回。
Armed fault 先於 auth，不消耗 nonce；auth 不通知 device hook。使用最後成功時間下限，
避免系統時鐘倒退重開已淘汰的 freshness interval。Restart 清除 volatile cache。

驗收：可控時間邊界／到期／倒退／容量／拒絕、並行相同 token 僅一次成功、base64
空白變體同 nonce、live user 改動、兩種 transport、停用／免認證、fault 優先、
instance／clone 隔離、zero hook。既有 scope 正向案例改用新鮮獨立 token，保留原有
namespace／空白／literal assertion；完整擾動與精確還原、五項 gate、文件與雙語清冊。
HTTP binding、role authorization、structured／raw injection 仍是 Q02 其他批次。

## 驗證

私有 cache 與時鐘下限已落實上述政策。原 scoped auth 正向案例重新簽署新鮮獨立
token，保留 namespace、base64／Created 空白及 literal 身分 assertion。新增單元及
兩種 transport 測試涵蓋邊界／容量復原／倒退／並行、跨使用者 decoded nonce 身分、
錯誤 digest 不佔用、豁免、fault 順序、停用及 zero hook；auth 成功而服務拒絕仍消耗 nonce。

完整未過濾 workspace all-feature 聯合擾動產生 9 項失敗、exit 101；auth.rs
逐 byte 還原後，fmt、兩種 workspace/all-target Clippy、workspace 測試通過：
all-feature 1378／default 1259。兩種 strict rustdoc 通過；inventory 維持
159 routes／161 Action sites／162 reader occurrences，連結檢查通過。

固定外部 Xerces 通過 224 XML／112 exchanges／60 Actions。AF1 增加一筆成功與
三筆實際 auth Fault response。保留 exporter 禁止憑證 guard，先移除這四筆 request
中的 synthetic Security header 才匯出；因此驗證 service payload／response wire，
不宣稱匯出的 auth header 經 schema 驗證。Runtime 測試使用原完整 token。
未使用實際憑證、修改攝影機、發布或宣稱完整 WSSE／授權驗收。
