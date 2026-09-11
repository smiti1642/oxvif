# 錄製儲存與報告身分

[English](replay-storage.md) | [繁體中文](replay-storage_zh.md)

狀態：0.17 候選行為；不屬於已發布的 0.16 產物。

| 章節 | 用途 |
| --- | --- |
| [儲存與查詢](#儲存與查詢) | 保留碰撞資料及 API 遷移 |
| [既有檔案與隱私](#既有檔案與隱私) | 載入、保存及降版限制 |
| [報告](#報告) | 非唯一分組 key 與保守差異比對 |
| [範圍](#範圍) | 本修正未證明的事項 |

## 儲存與查詢

`FixtureStore` 以 `(action, key_canon)` 尋找群組，而不是唯一請求。
Legacy projection 會去除部分 namespace 差異、正規化部分文字，並依 local name
遮罩欄位，因此不同 XML 請求可能具有相同 key。

- `record` 只有在同群組內的去憑證請求等價時才替換，否則新增另一份 fixture。
  替換保留原位置；不同請求依插入順序保存。
- `lookup(action, key_canon)` 僅在群組只有一筆時回傳 fixture。
  具有歧義時回傳 `None`，不再選取最後一筆錄製。
- 已知完整請求時使用 `lookup_request(action, request_xml)`；它套用既有去憑證
  轉換後選取唯一匹配。沒有唯一匹配時回傳 `None`。內建 replay 使用此 API，
  並保留原有 synthetic fallback 及 committed-effect 失效政策。

請求等價比較支援一般 namespace 別名、解碼後的 scalar 表示，以及選定的
qualified SOAP-header 暫態欄位。未限定 namespace 的 `Header/MessageID` 或
body 內的 `Created` 不視為可信任的傳輸暫態資料。非同一原文的 malformed XML、
mixed content 與未解析 `xsi:type` binding 不構成等價；刻意使用的 raw fixture
仍支援去憑證後完全相同的 bytes。這是有限比較，不是通用 XML 正規化或 ONVIF 驗證。

## 既有檔案與隱私

`fixtures.json` 格式不變。載入會在記憶體清除 legacy key 的 URL 帳密並建立
碰撞群組，保留不同請求；同群組的等價重複請求採最後一筆。載入不改寫原檔，
明確呼叫 `save` 才會保存目前 store。

降版前應保留備份：舊讀取器雖能讀取此 JSON，卻可能合併碰撞群組，後續保存將
遺失項目。本修正無法重建舊版已覆蓋的錄製。

新增錄製沿用指定 WS-Security Password／Nonce 與 literal URL `user:pass@`
去憑證轉換。載入不重新清理舊 raw envelope；兩者都不保證任意裝置資料無機密，
分享前應檢查 capture 及舊副本。本修正不包含保存耐久性／原子替換或版本化 key
格式重設計。

## 報告

`(action, key_canon)` 是分組 key，不是唯一列 ID。解析、結構比較及並排報告
保留每份 fixture。同一未變動 store 的完整報告可用插入順序關聯；
`FixtureProgress.done` 是該輪從一開始的序號。已篩選的 `QuirkReport` 不可僅用
該 pair 與其他報告關聯，也不可假設其列位置仍對應完整 store。

Legacy `QuirkReport` 列不含完整請求身分。`QuirkDiff` 任一側群組出現重複 pair 時，
會將目前各列完整保留於 `appeared`、先前各列保留於 `resolved`，不虛構 `changed`
配對。這些是**尚未配對的觀察值**，不證明裝置新增缺陷或已修復缺陷。因此即使兩份
具有歧義的報告完全相同，diff 仍可能非空；須檢查原始錄製以建立對應。
雙方均只有一筆的群組保留既有 path 差異比對。

## 範圍

本修正處理 K27 儲存遺失及相關報告 map 合併，不改 ONVIF 方法、SOAP 編碼、
JSON 格式或 raw adapter 契約；不代表 W19、全部 replay 相依、QName 語意或
協議相容性完成。完整發布驗收仍依[0.17 發布切點](active/release-0.17-cut_zh.md)執行。
