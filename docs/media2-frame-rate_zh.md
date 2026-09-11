# Media2 幀率遷移

[English](media2-frame-rate.md) | [繁體中文](media2-frame-rate_zh.md)

狀態：尚未發布，已核准納入下一個 minor release。這是原始碼不相容的修正，
不代表已安裝的 0.16.0 binary 具有此行為。

| 章節 | 用途 |
| --- | --- |
| [Rust 呼叫端](#rust-呼叫端) | 小數幀率與原始碼遷移 |
| [錯誤與未提供](#錯誤與未提供) | 區分缺漏、無效值與零 |
| [JSON 與 mock 狀態](#json-與-mock-狀態) | 持久化數值與 Media1 相容性 |
| [限制](#限制) | 本次修正未涵蓋的項目 |

## Rust 呼叫端

`VideoRateControl2.frame_rate_limit` 由 `u32` 改為 `f32`。Media2 先前會將 `12.5`
等有效小數幀率解析為 `0`；現在讀寫會保留小數。請調整 struct 的整數 literal
及僅接受整數的 helper 簽章；不要為了通過編譯而將回傳幀率轉成整數。

```rust
use oxvif::VideoRateControl2;

let rate = VideoRateControl2 {
    frame_rate_limit: 12.5, // 整數幀率請使用 25.0 等 literal。
    bitrate_limit: 2048,
};
assert_eq!(rate.frame_rate_limit, 12.5);
```

`OnvifClient` 與 `OnvifSession` 使用同一契約。寫入前應查詢所選 encoder 的 options；
有限數值可被接受，不代表攝影機支援該設定。`f32` 具有一般二進位浮點捨入特性，
不應要求十進位字串完全相同，亦不應使用未檢查的浮點轉整數 cast。
Media1 獨立的 `VideoRateControl` 公開型別不變。

## 錯誤與未提供

| 輸入 | 結果 |
| --- | --- |
| 沒有 `RateControl` | `None`，表示設備未提供 |
| 有效的 `FrameRateLimit` 為 `0` | `Some` 內為 `0.0`，不是錯誤替代值 |
| 已提供 `RateControl`，但缺少必填幀率／bitrate | `SoapError::MissingField`，包含欄位路徑 |
| 無效、重複或巢狀 scalar；重複 `RateControl` | `SoapError::InvalidValue`，包含受影響路徑 |
| 負值或非有限幀率 | 讀取回錯誤；寫入於 transport 呼叫前拒絕 |

路徑為 `Configuration/RateControl/FrameRateLimit` 與
`Configuration/RateControl/BitrateLimit`。Bitrate 仍為 `u32`，無效文字不再默默變成零。
選填欄位未提供，不應被解讀為要求補造幀率。

## JSON 與 mock 狀態

Serde 仍可載入一般舊版整數 JSON，例如
`{"frame_rate_limit":25,"bitrate_limit":2048}`。新版 JSON 使用可包含小數的數值，
呼叫端不可限制為整數表示法；大型整數受 `f32` 精度限制。序列化與反序列化會以
`frame rate must be finite and nonnegative` 拒絕負值及非有限幀率；序列化不會將
非有限幀率替換成 `null`。

`mock::VideoEncoderState.frame_rate_limit` 亦改為 `f32`，兩種 Media service 共用。
Mock 在改變狀態前驗證已提供的 scoped rate block；省略時保留現值。拒絕寫入會保留
狀態、change hook 及內建 replay recording。Encoder 成功寫入後，才淘汰兩種服務
相依的 encoder／profile recording。

Media1 無法表示小數幀率。Mock Media1 清單或 profile 若包含不相容 encoder，會以
頂層 Receiver／`mock:RequestPolicy` Fault 回覆：
`Encoder frame rate cannot be represented by Media1; use Media2`。
不會取整數、省略 encoder 或改寫共用狀態；不受影響的個別 encoder 仍可讀取。
Media1 寫入須為非負 `i32`，且可由共用 `f32` 精確表示；不相容的大型整數明確拒絕，
不默默捨入。手動 seed 的無效幀率亦會拒絕輸出，且不修改 seed。
以上為明示的 mock 政策，不代表真實設備必須回傳相同 Fault。

## 限制

本次修正未完成 encoder selector、options、bitrate 調整、全部欄位驗證、codec
相容性及 capacity 建模。CLI profile 報告仍使用 Media1，未新增自動 Media2 fallback。
Mock 驗證不代表實機行為或完整 ONVIF 符合性。詳見
[encoder 計畫與證據](active/mock-fidelity-video-encoder_zh.md)。
