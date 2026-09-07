# 相依套件維護與 Dependabot 集中更新計畫

[English](dependency-maintenance-plan.md) | [繁體中文](dependency-maintenance-plan_zh.md)

狀態：執行中；本機遷移已實作，遠端驗證與整合尚待完成。  
日期：2026-09-07。審查基準：`master` 的 `ee0460f`。

| 章節 | 用途 |
| --- | --- |
| [目標](#目標) | 範圍與完成條件 |
| [執行紀錄](#執行紀錄) | 現有結果與待完成關卡 |
| [審查證據](#審查證據) | 現有六個 PR |
| [執行順序](#執行順序) | 實作與整合步驟 |
| [Dependabot 設定](#dependabot-設定) | 單一例行跨生態系 PR |
| [驗證](#驗證) | 測試與實際運作證據 |
| [文件與發布](#文件與發布) | 交付與發布邊界 |
| [參考資料](#參考資料) | GitHub 官方設定文件 |

## 目標

完成 PR #6–#11 及必要的程式遷移，將未來 Cargo 與 GitHub Actions 的例行更新
集中至同一個每週 Dependabot 群組。維持 Rust 1.88、CLI schema v3、現有
fingerprint 與 XML 行為相容性。保留人工審查；分組不會啟用自動合併。

「一個 PR」指每個群組／更新週期的一個集中例行版本更新 PR。GitHub 的安全更新
使用獨立機制，仍可能另外建立 PR。不得為了限制 PR 數量而停用安全更新或隱藏警示。
若更新服務出錯或不支援設定組合，必須明確回報，不能直接改成每個 ecosystem
各一個 PR，卻宣稱已完成單一 PR 的需求。

2026-09-07 已授權開始執行。正式發布仍不在本次工作範圍內。

## 執行紀錄

- 整合分支：`codex/dependency-maintenance`，基於 `ee0460f`。開始執行時六個
  已審查 PR 的提交均未變動；預設分支未設定 branch protection 或 repository ruleset。
- 依序 cherry-pick #7、#8、#10、#11、#9、#6，保留機器人的原始作者資訊。
  將以單一替代 PR 驗證整合結果；完成整合驗證前保留舊 PR。
- sha2 遷移前已擷取 0.10.9 的固定結果。新編碼器通過相同的紀錄、計畫與開頭
  零位元組測試向量；既有套用計畫測試也使用該舊指紋。
- 三項合成 XML 相容性測試先在 quick-xml 0.41 通過，再以獨立下游使用者
  在 0.42 的 encoding 關閉與開啟情境通過。選用 schema 測試中的解析器也需要
  遷移 API；未新增任何外部 ONVIF schema 檔案或衍生資料。
- 本機 workspace 全 feature 測試：**1,076 項通過、4 項忽略**；全 feature
  Clippy 通過。遠端原生平台關卡與 staging 尚未驗收。
- 2026-09-07 `cargo audit`：411 個相依套件，無已知漏洞。
  `cargo outdated --workspace --root-deps-only` 已完成；其餘更新（包含 keyring 4.2
  與更新的 ipnet）留待後續批次審查。
- `CONTRIBUTING.md` 已明確要求未來發布工具變更，合併前必須使用候選 workflow ref
  與原始碼 SHA 完成手動 staging。
- 基準 staging `33856099748` 顯示既有 SBOM 涵蓋不足：Linux 僅有掃描目錄，
  Windows 另外列出兩個版本未知的執行檔，均無 Rust 相依套件清單。候選版本保留
  執行檔 SBOM，另新增 `.source.spdx.json`，核對全部 411 組鎖定套件與版本。
  本機 Syft 1.51.1 已通過核對，下載封存檔已比對上游發布 checksum；四項正向與
  負向核對器測試通過。來源清單包含開發與平台限定套件，不代表單一執行檔連結清單。
  首次候選 staging `34092581258` 已為此修正主動取消，不列為驗收證據。
- 候選 `391880f` 的 21 項 CI 全部通過（`34092932594`）。其 staging
  `34092981151` 發現 Ubuntu 22.04 發布 runner 使用 Python 3.10：來源 SBOM
  產生成功，但核對器無法匯入 `tomllib`。Workflow 已透過經審查並固定 SHA 的
  `actions/setup-python` v7.0.0 明確選擇 Python 3.12。中止的 staging 不視為通過。
- 分組設定啟用、遠端 updater 日誌、第一個集中 PR 與後續無重複 PR 的行為仍待驗證。
  此計畫尚不能移至 `done/`。

## 審查證據

六個 PR 的審查基準均為 `ee0460f`；執行前必須重新確認各 PR 的最新提交。

| PR | 已審查提交 | 發現 | 處理方式 |
| --- | --- | --- | --- |
| [#7 base64](https://github.com/smiti1642/oxvif/pull/7) | `6b8f7e0` | 僅更新 lockfile；上游修正測試，正式編解碼邏輯未變；21 項 CI 通過。 | 確認最新差異後整合。 |
| [#8 ipnet](https://github.com/smiti1642/oxvif/pull/8) | `4f4208c` | 僅更新 lockfile；修正 subnet iterator 邊界；CLI 的解析與包含判定路徑未變；21 項 CI 通過。 | 確認最新差異後整合。 |
| [#10 async-trait](https://github.com/smiti1642/oxvif/pull/10) | `062ec48` | 僅更新 lockfile；移除重複生成的 `must_use`；21 項 CI 通過。 | 確認最新差異後整合。 |
| [#11 sha2](https://github.com/smiti1642/oxvif/pull/11) | `25de4f7` | 新 digest 型別未實作 `LowerHex`，`registry.rs:1735` 與 `:1824` 編譯失敗。 | 修正 fingerprint 編碼並建立相容性回歸證據。 |
| [#9 quick-xml](https://github.com/smiti1642/oxvif/pull/9) | `f0607a6` | decoder API 移除及 bytes／字串介面變更，使 `src/soap/xml.rs` 無法編譯。 | 遷移解析器並驗證語意相容性。 |
| [#6 SBOM action](https://github.com/smiti1642/oxvif/pull/6) | `9406771` | 同時更新 Syft 1.42.3 → 1.51.1 與安裝行為；一般 PR CI 未執行該步驟。 | 接受前完成非發布模式的 release staging。 |

#9 與 #11 在 stable 與 MSRV 工作中的失敗原因是程式介面不相容；目前沒有因此
必須提高最低 Rust 版本的證據。

## 執行順序

### 1. 確立整合狀態

- [ ] 取得最新預設分支及 PR 提交，記錄完整 SHA 與 CI 執行紀錄。
- [ ] 測試 PR 時，必要時使用隔離 checkout，保留使用者其他變更。
- [ ] 確認合併政策與必要檢查，依正常 PR 流程整合，不繞過失敗檢查或強制推送受保護分支。
- [ ] 在變更已整合，或有明確替代 PR 包含變更且通過驗證前，保留現有個別 PR。

### 2. 整合已審查的修補更新

- [ ] 依序處理 #7、#8、#10，保留原始提交的作者歸屬。
- [ ] 基準前進後重新確認 lockfile 衝突與 CI；只調整必要項目，不混入完整
  `cargo update` 所產生的其他更新。
- [ ] 驗證組合後的結果；三次歷史 CI 通過不能取代最終整合版本的驗證。

### 3. 完成 sha2 遷移

- [ ] 擴充 #11；若機器人重新生成分支可能覆蓋人工修改，改用明確連結的替代 PR。
- [ ] 明確將 digest 每個 byte 編碼為兩位小寫十六進位，保留 `sha256:` 前綴及
  恰好 64 位十六進位字串。
- [ ] 修改前先以舊實作產生固定 fingerprint 測試資料，要求新實作結果完全相同，
  並涵蓋摘要起始 byte 為零的情況。
- [ ] 驗證舊的 discovery 記錄及已審查匯入計畫仍可使用；記錄或選項改變時，
  舊計畫仍須在任何寫入前被拒絕。
- [ ] CLI 測試、Clippy 與 Rust 1.88 檢查通過後整合。

### 4. 完成 quick-xml 遷移

- [ ] 同步更新一般與開發用的相依宣告。
- [ ] 將 `Reader::decoder`、reference 解碼、文字／CDATA、local name、namespace
  判斷與 attribute normalization 遷移至 0.42 API。
- [ ] 保留 entity 事件之間的空白、具名與數字 entity、未知 entity 處理、CDATA、
  Unicode、屬性正規化及 namespace 去除行為。維持既有 malformed-input 行為；
  若存在無法避免的變化，接受前必須記錄。
- [ ] 分別驗證 `encoding` 開啟與關閉，避免重新使用開啟該 feature 後會消失的 API，
  再次引入既有 feature-unification 問題。
- [ ] 執行 library、discovery、SOAP fault、mock、health 與 CLI 測試資料，並納入
  解析器變更相關的既有 feature 組合檢查。
- [ ] stable 與 Rust 1.88 檢查通過後整合。

### 5. 驗證 SBOM action 與發布流程

- [ ] 確認 #6 仍固定到預期的上游提交，審查輸入參數、執行環境需求、內建掃描器
  版本及下載行為。
- [ ] 從候選 workflow ref 執行 `publish=false`；只在舊 workflow 傳入新的原始碼
  SHA，不能驗證 workflow 本身的更新。
- [ ] 驗證五個原生架構產物及 SPDX JSON 生成，核對 CLI 名稱／版本、掃描器資訊及
  代表性相依套件清單。對照舊輸出調查遺漏或大幅變動，不能只確認檔案非空。
- [ ] 維持 action 自動上傳 artifact／release asset 的既有停用設定，由 workflow
  既有的受控步驟上傳 staging 產物。
- [ ] 未來修改 release workflow 或 packaging 的 PR，必須有非發布驗證路徑，或
  明確要求並記錄人工觸發的 staging 檢查。不得以一般 Rust CI 宣稱 SBOM 已驗證。
  若自動化，使用無額外權限的 PR 執行方式，不以具高權限的 `pull_request_target`
  checkout 並執行 PR 程式碼。

### 6. 啟用 Dependabot 集中更新

- [ ] 先完成現有 PR，再啟用下方設定。
- [ ] 驗證 YAML 與當時的 Dependabot 設定 schema／選項，確認 GitHub 更新服務接受
  Cargo 與 GitHub Actions 的組合。
- [ ] 將設定合併到預設分支，檢查實際 Dependabot job log 及產生的 PR；本地 YAML
  可以解析不等於服務已成功啟用。
- [ ] 集中 PR 尚未關閉時，再確認一次更新工作的行為，檢查是否產生重複例行 PR，
  記錄實際結果與服務限制。
- [ ] 確認變更已合併或由已驗證的替代 PR 承接後，才處理過時機器人 PR，避免批次
  關閉或刪除尚未完成的項目。

## Dependabot 設定

預定替換 `.github/dependabot.yml` 的內容如下；本計畫尚未套用此設定：

```yaml
version: 2

multi-ecosystem-groups:
  maintenance:
    schedule:
      interval: weekly
      day: monday
      time: "09:00"
      timezone: Asia/Taipei
    labels: [dependencies]

updates:
  - package-ecosystem: cargo
    directory: /
    patterns: ["*"]
    multi-ecosystem-group: maintenance
    labels: [rust]
  - package-ecosystem: github-actions
    directory: /
    patterns: ["*"]
    multi-ecosystem-group: maintenance
    labels: [ci]
```

同一群組涵蓋所有例行更新類型，包含破壞相容性的更新。任一相依套件失敗時，整批
必須修復後才能接受。不得默默忽略 major 更新，或另開多個個別例行 PR。
若某項更新無法完成，必須先記錄原因與具日期的後續工作，再提出暫時排除方案。
安全修補可獨立處理。

只把兩個 ecosystem 的 `open-pull-requests-limit` 各設為 1，仍可能產生兩個 PR；
跨 ecosystem 分組才是集中變更的機制。不假設既有 PR 會隨設定修改自動轉換。

## 驗證

對最終整合候選版本執行以下檢查，並遵守各平台原生執行需求：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy -p oxvif --no-default-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test -p oxvif --no-default-features --locked
cargo +1.88.0 check --workspace --all-features --locked
cargo audit
cargo outdated --workspace --root-deps-only
```

- [ ] Windows x64、Linux x64／ARM64、macOS Intel／Apple Silicon CI 通過，涵蓋
  原生憑證生命週期、CLI smoke、套件與文件檢查。
- [ ] fingerprint 與 XML 測試證明新舊相容，不能只比較兩個同樣使用新實作的函式。
- [ ] 最終整合版本通過非發布模式的 release staging。
- [ ] #6–#11 全部合併，或由已整合且具明確對應的變更取代。
- [ ] 有更新可用時，實際觀察到一個跨 ecosystem 例行 PR，且服務紀錄證明兩個
  ecosystem 都已執行。若沒有可用更新，標示為待運作驗證，不建立虛構測試版本。
- [ ] 在本計畫記錄 SHA、workflow URL、版本、結果及未解決例外。

## 文件與發布

- [ ] 更新 `docs/dependency-pitfalls.md`，記錄 XML 與 digest 遷移注意事項。
- [ ] 在 `CHANGELOG.md` 的 Unreleased 區段加入維護紀錄，更新貢獻指南中的集中
  審查方式與 release workflow 驗證要求。
- [ ] 修改公開文件時同步更新英文與 `_zh` 對應版本。
- [ ] 依既有合併政策將整合後的 `master` 同步回 `develop`。
- [ ] 保留已發布的 `v0.16.0`、crate 內容及 release assets。本計畫不發布新版本，
  也不恢復 APT／Homebrew 的發布工作。
- [ ] 整合完成後才準備 maintenance release 候選版本；依先前要求，在任何對外
  發布動作前先提醒專案擁有者。
- [ ] 整合與分組運作驗證皆完成後，才將本計畫及英文版移至 `docs/done/` 並更新連結。

## 參考資料

查核日期：2026-09-07。

- [設定跨 ecosystem 更新](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/secure-your-dependencies/configuring-multi-ecosystem-updates)
- [跨 ecosystem 分組行為](https://docs.github.com/en/code-security/concepts/supply-chain-security/multi-ecosystem-updates)
- [安全更新分組](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/secure-your-dependencies/configure-security-updates)
- [Dependabot 錯誤與獨立更新限制](https://docs.github.com/en/code-security/reference/supply-chain-security/troubleshoot-dependabot/dependabot-errors)
