# Media synchronization

[English](media-synchronization.md) | [繁體中文](media-synchronization_zh.md)

於下一版提供，不屬於已發布的 0.16.0 API。

| 章節 | 用途 |
| --- | --- |
| [Client 與 Session](#client-與-session) | 請求 profile 關聯串流的同步點 |
| [Mock 行為](#mock-行為) | 明確的僅收件確認測試 |
| [驗證限制](#驗證限制) | 成功回應可以證明的範圍 |

## Client 與 Session

| Service | Client 方法 | Session 方法 |
| --- | --- | --- |
| Media1 | `media_set_synchronization_point(media_url, profile_token)` | `media_set_synchronization_point(profile_token)` |
| Media2 | `set_synchronization_point_media2(media_url, profile_token)` | `set_synchronization_point_media2(profile_token)` |

請明確選擇目標 profile。這些方法請求其關聯串流同步，包含 video intra frame
以及適用的 metadata 狀態更新。與 Events `set_synchronization_point` 不同，後者
指定的是 subscription 而不是 media profile。Session 不會默默改用不同 service；
transport、SOAP 與 response shape 錯誤均會回傳。

```rust
// 需要已建立的 OnvifSession 與明確選擇的 profile token。
session.media_set_synchronization_point(&profile.token).await?;
// 若裝置提供 Media2，改用 Media2 方法：
session.set_synchronization_point_media2(&profile.token).await?;
```

## Mock 行為

Mock 沒有影音編碼器或 RTP／metadata 傳送管線，兩個方法預設均回傳
`s:Receiver`／`mock:UnmodeledEffect`。僅收件確認測試須明確選擇對應 service 操作：

```rust
use oxvif::mock::{AckOnlyOperation, MockTransport};
let transport = MockTransport::new()
    .with_acknowledgment_only(AckOnlyOperation::MediaSynchronizationPoint);
// Media2SynchronizationPoint 須獨立選擇；Events 不受影響。
```

Opt-in 仍驗證 request identity 與唯一的 scoped profile token。缺少、歧義或位置
不正確的 selector 會被拒絕，不存在的 profile 回傳 structured fault。拒絕及收件
確認均不改變 state、hook、event queue 或已錄製的讀取結果。Replay 不回傳錄製
過的寫入確認，而是套用相同 synthetic policy；明確的 fault injection 另行處理。

## 驗證限制

Client wire 測試、HTTP／in-process Mock 測試與外部 XML 驗證檢查的是請求、
回應及政策，不是實際 media delivery。SOAP acknowledgment 本身不能證明已觀察
到 I-frame 或 metadata refresh。真實串流驗收需要明確授權的請求與該串流觀測。
參閱[整合驗證紀錄](active/contributor-pr-integration-plan_zh.md)。
