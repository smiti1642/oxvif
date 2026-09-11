# 音訊與 metadata 遷移指南

[English](audio-metadata.md) | [繁體中文](audio-metadata_zh.md)

本文件適用於尚未發布的下一個次版本，不代表已發布的 0.16.0 API。

| 章節 | 用途 |
| --- | --- |
| [Metadata](#metadata) | Rust 與 JSON 遷移 |
| [音訊](#音訊) | 服務編碼名稱與 options |
| [合成裝置契約](#合成裝置契約) | Mock 行為與限制 |
| [驗證](#驗證) | 證據與未完成工作 |

## Metadata

`MetadataConfiguration` 將 `multicast_address` 與 `multicast_port` 改為
`multicast: MulticastConfiguration` 及 `session_timeout: String`。
`mock::MetadataEntry` 同樣改用 `multicast: MulticastEntry` 與
`session_timeout`。Rust struct literal 與序列化快照均須更新。

完整 JSON 範例如下：

```json
{
  "token": "MetaConf_1",
  "name": "Entrance metadata",
  "use_count": 1,
  "analytics": true,
  "ptz_status": false,
  "ptz_position": true,
  "multicast": {
    "address": "239.0.1.10",
    "port": 40010,
    "ttl": 1,
    "auto_start": false
  },
  "session_timeout": "PT60S"
}
```

以上為範例，不可直接作為舊裝置資料的補值。TTL、AutoStart 與 timeout
應取自重新讀取的裝置設定或已知完整快照。只有舊式平面欄位的 JSON
將無法反序列化；程式不會推測遺失資訊。Mock entry 另須保留兩個 PTZ 能力欄位。

讀取要求 Name、UseCount、完整 multicast block 與 session timeout。
格式錯誤或重複的已建模欄位會明確報錯。群播位址須為明確且有效的 IPv4／IPv6；
公開型別無法表示沒有位址的 IPAddress block。寫入前會拒絕空 token、無效 IP、
大於 65535 的 port、大於 255 的 TTL，以及無效的非負 duration，不會送出請求。
共用序列化器會依位址家族輸出對應 IPv4／IPv6 元素。

Setter 將 PTZStatus 放在 Analytics 前，並包含 multicast／session timeout。
Media2 已棄用並忽略 timeout，但 wire 結構仍要求此欄位。
AutoStart 是持續串流的唯讀狀態，不是啟動命令。

此型別僅涵蓋已建模子集，**不是任意設定的無損編輯器**。Events filter、
壓縮、analytics-engine 設定、sensor／geo／shape 與廠商 extension 均未表示。
真實攝影機若使用這些欄位，不可假設讀取後寫回能保留其內容。

## 音訊

Media1 與 Media2 編碼字串不能直接互換。寫入前應查詢目標服務的 options。
公開 `AudioEncoding::Other(String)` 保留裝置提供的值；本次未加入真實攝影機的
自動跨服務編碼轉換。

預設 mock 明確採用 G711 為 µ-law 的約定，因此 Media2 顯示 PCMU；
AAC 顯示 MP4A-LATM；G726 使用 ONVIF 的 G726 名稱，由 bitrate 選擇變體。
此約定不表示任意 Media1 裝置的 G711 都是 µ-law。

音訊 options 現在以重複 Items 元素逐一輸出整數。Client 讀取全部 Items；
為相容既有裝置與 fixture，仍接受舊式空白分隔清單。
這也修正先前只讀取第一個重複 Items 的問題。

## 合成裝置契約

AM1 的 15 個音訊／metadata 操作均使用具作用域的請求解析。
Media2 清單與 options selector 驗證 configuration／profile 身分；
未知參照回傳 Fault，不再以空清單表示成功。Generic options 為 catalogue 聯集，
不保證每個 configuration 都支援全部組合。所有既有 profile 在邏輯模型中相容；
未實作實體路由衝突，以及 metadata／output／decoder 的 profile 繫結。

音訊 setter 要求完整已建模候選設定，且編碼、bitrate、sample rate
須為該 configuration 所宣告的組合。不套用視訊專用的 bitrate 調整規則。
Media1 要求 multicast、session timeout 與 boolean ForcePersistence。
Media2 音訊省略 multicast 時保留共用設定與 timeout；UseCount 為唯讀。

Metadata setter 儲存名稱、PTZ／analytics 旗標，以及群播位址／port／TTL。
省略可選旗標時保留原值；必要但已棄用的 timeout 僅驗證、不更新。
若兩個 PTZ 能力均不支援，則拒絕啟用 PTZ filter。
AutoStart 輸入會驗證但忽略；不產生串流的 mock 回報 false。
匯入快照若宣稱已有持續群播，將拒絕輸出該設定。

完整驗證後才執行一次原子提交與變更通知。拒絕時保留全部狀態及 replay recording；
成功提交才使相依設定與 profile recording 失效，不影響無關服務。
儲存仍為記憶體狀態及可選的呼叫端持久化 hook；不產生 RTP，也不寫入真實攝影機。

## 驗證

測試、外部 schema 驗證及限制詳見 [AM1 執行證據](active/mock-fidelity-audio-metadata_zh.md)。
結構驗證不是 ONVIF 認證，也不證明真實裝置的串流行為。
