# oxvif 支援與相容性政策

[English](support.md) | **繁體中文**

本政策分別說明 Rust 函式庫與 `oxvif` 命令列產品的支援範圍。目前內容描述預定推出的函式庫 0.16.0 與 CLI 0.1.0 beta；只有在套件與成品公開發布並通過驗證後，相關內容才構成正式的版本承諾。

## Rust 函式庫

- crates.io 公開版本遵循語意化版本原則，並採用 Rust 生態系對 1.0 版以前相容性的通行慣例。
- 目前受支援的公開版本線列於 `SECURITY.md`；`develop` 分支與 Git revision 均屬評估版本。
- 本專案宣告的最低支援 Rust 版本（MSRV）為 Rust 1.88。發布前必須在該工具鏈與 stable Rust 上通過工作區檢查。
- oxvif 是獨立實作，未取得 ONVIF 認證，亦不宣稱支援所有廠商裝置或全部 ONVIF Profile。

## CLI 診斷 beta

首個 CLI 版本定位為唯讀的裝置診斷 beta。CLI 可能變更本機 registry、Groups、Views、探索快照與憑證參照，但不提供修改裝置設定的寫入操作。

| 項目 | Beta 支援狀態 |
| --- | --- |
| Windows x86_64 | 已涵蓋測試、binary smoke test，以及經本機驗證的 Windows Credential Manager 契約 |
| Linux x86_64/aarch64 | 已實作 Secret Service；發布前仍須取得原生 CI 與套件安裝驗證證據 |
| macOS x86_64/arm64 | 已實作 Keychain；發布前仍須取得原生 CI 與 bottle 安裝驗證證據 |
| 結構化輸出 | Schema version 3；允許新增欄位，破壞性變更必須遞增 schema version |
| Agent 指南 | 與 stdout schema 分別進行版本管理 |
| 原生憑證 | 支援 Credential Manager、Keychain 與 Secret Service；所有原生契約通過前不得公開發布，且不提供明文 fallback |
| TLS | 使用平台信任根與可重現的明確 PEM `--ca-certificate` bundle；不提供略過安全檢查或主機名稱驗證的選項 |

`OXVIF_USERNAME` 與 `OXVIF_PASSWORD` 僅供受信任的 process-scoped 自動化使用，並非持久化憑證後端。若 headless Linux 無法使用或解鎖 Secret Service，CLI 會回傳 `CREDENTIAL_UNAVAILABLE`；此時應改用上述 process-scoped 方式。

## 攝影機相容性聲明

任何相容性聲明都必須明確列出作業系統／架構、oxvif 版本、攝影機廠商／型號／韌體、宣告的 ONVIF Profile、驗證方式與已測試命令。單一命令在某一型號或韌體上成功，不代表支援該廠商的完整產品線。經過敏感資訊移除的社群回報可作為驗證證據，但不等同於認證。

在版本強化計畫具備版本化的多廠商相容性矩陣、簽署成品、復原程序與明確支援協議前，本專案不宣稱提供廣泛的商業支援。

## 問題回報與回應

一般錯誤與移除敏感資訊後的相容性報告，請使用對應的 issue template；安全性漏洞請依 `SECURITY.md` 所述的非公開流程回報。社群支援採 best-effort 模式，不提供回應時間保證；任何商業服務等級均須另以書面協議明定。
