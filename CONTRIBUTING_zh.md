# 參與 oxvif 開發

[English](CONTRIBUTING.md) | [繁體中文](CONTRIBUTING_zh.md)

歡迎提交問題回報、相容性觀察、文件修正，以及範圍明確的 Pull Request。

| 章節 | 用途 |
| --- | --- |
| [提出變更前](#提出變更前) | 範圍與相容性 |
| [本機驗證](#本機驗證) | 必要檢查 |
| [測試資料與裝置回報](#測試資料與裝置回報) | 敏感資訊清理 |
| [Pull Request](#pull-request) | 驗收條件 |
| [相依套件與發布工具變更](#相依套件與發布工具變更) | 額外審查關卡 |

## 提出變更前

1. 搜尋既有 Issue 與 `docs/active/` 中的執行計畫。
2. 分別評估函式庫 API、CLI 人類輸出與結構化 Agent 輸出的相容性。
3. 新增會修改裝置的行為前，必須有明確定義確認、plan/apply、復原與實機驗證關卡的計畫。

## 本機驗證

使用 Rust 1.88 或更新版本。提交 Pull Request 前執行：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo +1.88.0 check --workspace --all-features --locked
cargo audit
```

修改 `oxvif-cli` 時，也必須測試相關的人類與結構化輸出路徑。
JSON schema、descriptor、結束代碼與公開說明的變更，必須有對應的針對性測試。

## 測試資料與裝置回報

實機證據在納入 Issue、測試資料、日誌、提交或 CI 產物前，必須移除敏感資訊：

- IP 與 MAC 位址；必要時改用 `192.0.2.0/24` 等文件專用位址。
- 帳號、密碼、授權標頭、WS-Security 密碼摘要、nonce、Cookie、Token 與私密金鑰。
- URI userinfo，以及可從外部存取的快照或串流網址。
- 序號、UUID、主機名稱、場域或客戶名稱，除非有明確必要且可安全公開。

建議記錄作業系統與架構、oxvif 版本、攝影機廠牌、型號、韌體、宣告的 ONVIF
Profile、清理後的命令、結束代碼、結構化錯誤代碼，以及預期與實際結果。
不得提交 ONVIF WSDL/XSD 或其衍生 schema 測試資料；請參閱 `.gitignore` 與專案政策。

## Pull Request

保持變更可審查，並說明可觀察的行為契約。測試通過、文件符合行為、診斷不洩漏敏感資訊，
且沒有無關的格式或產生檔案變動時，才具備合併條件。

## 相依套件與發布工具變更

逐項審查上游變更、feature 需求與 MSRV，再測試整合後的 lockfile。
維持指紋、XML 與原生憑證契約。不得以完整 `cargo update` 解決單一更新 PR 的衝突。
XML 相依套件更新另須執行（需要 Python 3.11 或更新版本）：

```text
cargo fetch --locked
python packaging/check_xml_features.py
```

修改 `.github/workflows/release.yml`、其中固定版本的 action 或 `packaging/`，
**合併前必須通過手動、不發布的 release staging**；一般 PR CI 不足以取代此關卡。
維護者必須先審查候選 workflow，再手動啟動。不得透過 `pull_request_target` 執行
未審查的 PR 程式碼，也不得向相依套件 PR 提供發布憑證。

```text
gh workflow run release.yml --ref <candidate-branch> -f tag=<full-candidate-commit-sha> -f publish=false
```

`--ref` 選取修改後的 workflow；`tag` 選取原始碼 checkout。
`tag` 請使用完整提交 SHA，因為包含 `/` 的分支名稱不適合作為產物標籤。
PR 必須記錄兩者 SHA 與執行網址，並確認各原生平台產物、憑證測試、SPDX 輸出、
APT 安裝與移除，以及 Homebrew 安裝、bottle 建立與重新安裝均通過。
應與前次 staging 比較掃描器版本及實際相依套件涵蓋範圍；SBOM 非空並不代表充分。
程式碼或發布工具再次變更後必須重新執行；純文件變更可引用相同程式碼與工具樹的驗證結果。

Release staging 會為每個目標產生兩份清單：`.spdx.json` 是執行檔掃描，
`.source.spdx.json` 則盤點 Cargo.lock 與 workspace manifest。
Rust 執行檔掃描可能遺漏相依套件，或將程式版本標為未知。來源清單會核對每個鎖定的
套件與版本（包含 CLI），但也包含開發及其他平台的套件，不得宣稱是精確的單一執行檔連結清單。

`publish=false` 僅上傳暫存 Actions 產物，不代表允許建立 GitHub Release、
發布 crate 或修改公開套件通路。
