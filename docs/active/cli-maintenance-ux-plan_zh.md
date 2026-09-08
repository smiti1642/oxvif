# CLI 維運 UX 改善計畫

[English](cli-maintenance-ux-plan.md) | [繁體中文](cli-maintenance-ux-plan_zh.md)

狀態：實作及本機驗證完成，尚未發布；實機與原生候選版本驗收仍待完成。
不修改設備、不發布套件、不替換已安裝執行檔。

| 章節 | 用途 |
| --- | --- |
| [實作項目](#實作項目) | 工作順序 |
| [驗收](#驗收) | 必要證據 |
| [驗證證據與限制](#驗證證據與限制) | 已完成檢查及待驗項目 |

## 實作項目

1. [x] 以執行檔測試重現文件中的後置選擇器／逾時參數失敗。在維運命令支援全域選項
   位置正規化，不誤移 discovery 本機 `--jobs`、profile 值或 `--` 之後的參數，保留
   既有語法。
2. [x] 沿用診斷已建立的 session 及 profile 查詢，提供選用的人類選單。顯示名稱與
   token，支援方向鍵／j/k、Enter 與取消。僅互動式人類終端可使用；明確錯誤 token、
   fleet 與 Agent 不自動改選。取消時保留已完成的診斷證據。
3. [x] 人類報告摘要優先，顯示可處理的失敗並合併尚未驗證的播放限制；`-v` 展開
   階段與耗時。設定比較呈現欄位／原值／目前值，區分不存在與 null。慢速作業只在
   互動 stderr 顯示進度，選單前清除並於結束後恢復。
4. [x] 新增結構化 profile 候選資料、選取原因碼及未測試原因。保留 schema v3
   既有欄位、階段錯誤分類與退出碼；透過新增欄位細分原因，不默默改名既有契約。
   同步更新 Agent guide 與指令描述。
5. [x] 更新中英文指南、導覽、README 摘要與 Changelog，將相關 Markdown 指令
   範例納入實際 parser 與本機 mock 測試。

## 驗收

- 參數矩陣：前置／後置／等號格式、缺值、互斥、類似選項的 opaque token、`--`，
  以及 discovery 自有 jobs。
- Profile 選單：單一／多個／空清單、錯誤 token、查詢失敗、取消、不重複交握與
  profile 查詢；JSON、重新導向與非互動模式不得開啟 UI。
- 報告：精簡成功／失敗、詳細輸出、設定差異與 null；單台／fleet JSON／JSONL
  符合 schema，涵蓋部分與全部失敗。
- 終端按鍵／尺寸測試；環境支援時以本機 mock 執行 Windows PTY 操作，未驗證項目
  必須明確記錄。
- 格式、workspace 預設／全功能 clippy 與測試、Rust 1.88 檢查。
- 實機與 macOS／Linux 原生驗收保留為發布關卡，不以 mock 代替。驗證後才提交，
  不發布 Release。

## 驗證證據與限制

Windows x64，2026-09-08：

- Workspace 全功能測試：1,101 項通過、4 項既有條件式忽略。
- Workspace 預設測試：1,021 項通過、4 項既有條件式忽略。
- 預設／全功能 workspace clippy（`-D warnings`）、格式及 Rust 1.88 的
  workspace／all-targets／all-features 檢查通過；CLI rustdoc 警告視為錯誤亦通過。
- Parser 測試涵蓋中英文維運範例、後置／等號格式、錯誤與缺值、互斥、opaque
  token 及 discovery 專屬 jobs。執行檔 mock 測試涵蓋後置 device／timeout、
  摘要／詳細輸出、比較、結構化候選資料及未加 `--non-interactive` 的 JSON。
- Profile 測試涵蓋唯一／多個／空清單、明確錯誤 token、查詢失敗、取消及介面
  失敗。在選單 callback 注入的 fault 於結束後仍可讀取，證實未再次交握或查詢。
- Windows ConPTY 使用原生 console handles 實測：`j` 再按 Enter 選到 `Profile_2`，
  成功退出 `0`；`q` 取消保留三項通過檢查，退出 `20`。本機延遲 HTTP 端點於一秒
  後顯示進度，逾時時清除進度行並回傳退出 `20` 的報告。RTK 預設管線 handles
  正確觸發非互動行為，因此實測 UI 時另接原生終端 handles。
- 按鍵／翻頁／邊界及窄畫面／Unicode／尺寸變更後的配置具單元測試；未實測實際
  拖曳縮放、所有 terminal emulator 或 macOS／Linux 互動介面。
- 預設建置曾因 Windows 鎖住執行中的 mock 執行檔而失敗；停止本輪啟動的 mock
  後，完整重跑關卡通過。

本輪未連線實機、未修改攝影機設定、已安裝執行檔、遠端分支、tag 或 Release。
合成測試產物保留於已忽略的 `target/cli-ux-implementation-20260908/`。實機及
原生候選版本的安裝驗收，仍須依操作者的發布流程完成。
