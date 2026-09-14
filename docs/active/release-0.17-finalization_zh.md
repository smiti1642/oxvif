# 0.17 發布收尾紀錄

[English](release-0.17-finalization.md) | [繁體中文](release-0.17-finalization_zh.md)

更新日期：2026-09-14。已授權 v0.17.0 tag 及正式 GitHub Release。
準備分支：`codex/release-0.17-finalize`，基於 `d3ac1b6`。
候選：`9305f5d8c080feca6adf555d4177f10556f45db3`，以 `smiti1642` 提交及推送。
後續僅證據文件編輯不改變其 runtime／發布工具輸入。
準備與證據提交已 fast-forward 同步至本機／遠端 master 及 develop。
臨時準備分支已刪除，提交仍由兩主分支保留；準備階段未建立 tag 或對外上傳，
後續授權的發布行動記於下方。

| 章節 | 用途 |
| --- | --- |
| [差異審查](#差異審查) | 有限 G01／G03 核對 |
| [驗證](#驗證) | 實際結果及未完成關卡 |
| [發布邊界](#發布邊界) | 需要確認的動作 |

## 差異審查

本紀錄補充而非改寫原 208 路徑審查清冊。受審 runtime 差異為
`5a1821b..d3ac1b6`。本次準備僅修改版號、文件及 man page，不修改 runtime。

| 原始碼 | 審查重點及結果 |
| --- | --- |
| CLI application／main／manage／maintenance | 共用探索分類、正確篩選選取、明確新增且不改 current、成功才取代掃描快取、設備工作區／session／profile 隔離、成功及失敗證據保留、路徑／baseline 本機預檢；未發現新的發布阻擋問題 |
| CLI interactive | 文字搜尋與導覽分離、情境設定／說明、適應視窗及底部狀態列、UTF-8 輸入、密碼持有與清理、計算寬度／換行前轉義控制及 bidi 字元；未發現新阻擋問題，不宣稱全部終端／輸入法相容 |
| Mock fleet／server | 每台身分／狀態隔離、啟動失敗回滾、停止及有界等待、指定本機宣告位址與非 loopback 暴露；Mock HTTP 無認證，僅供隔離且可信的測試網路 |
| 共用 discovery responder | 有界封包／結構、namespace-aware Probe／Types、anonymous ReplyTo、拒絕未支援 scope、有界去重快取／設備數／回應延遲、只宣告已就緒 HTTP 成員；文件明列有限探索子集，不宣稱完整相容 |
| Fleet example／config | 嚴格且有界 TOML、唯一 ID／UUID／port／serial、指定 state 檔失敗不回退、manifest create-new、不回寫、Ctrl+C 清理及 LAN 警告；不宣稱 RTSP、品牌模擬或 VMS 長時間穩定性 |
| 終端 fixture／CI 控制 | 僅測試 fixture 及隔離 harness；d3ac1b6 修正跨平台 Python import 與 reader 清冊；保留失敗傳遞，不削弱斷言或發布防護 |

證據見 [CLI 連續性](cli-workflow-continuity_zh.md)、
[Fleet 驗收](mock-fleet-basic-plan_zh.md#本機證據)、
[基準審查](release-0.17-review_zh.md)及[後續清單](post-0.17-backlog_zh.md)。
本紀錄屬有限工程審查，不是獨立安全認證或 ONVIF 認證。

## 驗證

| 檢查 | 結果 |
| --- | --- |
| 原精確 runtime／CI 基準 | [34809397553](https://github.com/smiti1642/oxvif/actions/runs/34809397553)，d3ac1b6：五種原生目標共 27 jobs 通過 |
| 本機 0.17 workspace | `cargo check --workspace --all-features --offline` 通過 |
| 本機 strict rustdoc | Workspace／all-features／no-deps／locked 搭配 `RUSTDOCFLAGS=-D warnings` 通過 |
| 兩個實際套件 | 在乾淨 commit 9305f5d 再執行 `cargo package --workspace --locked --target-dir target/package-017-final` 通過，未使用 allow-dirty；Cargo 暫存 registry 讓 CLI 對打包後 library 編譯，未上傳 crates.io |
| 初次打包 | 無法覆寫使用者正在執行的 debug CLI；隔離 target 目錄後通過，未中止使用者程式 |
| 打包後 CLI | 版本 0.17.0、schema 3、guide 8；structured describe 及 manage／diagnose／snapshot／config export／diff help 通過 |
| 文件 | 706 個公開／目前紀錄的本機及版本連結、382 個錨點通過；另對全庫指向修改文件的連結檢查：604 個連結／343 個錨點通過。0.16.0 起的已發布 CHANGELOG 歷史未改動；fmt 及 diff 空白檢查通過 |
| 人工驗收 | 使用者回報人工 PASS，加上先前 Windows discover／manage／resize ConPTY 證據；不推定其他平台／設備／VMS 覆蓋 |
| 最終 0.17 託管 CI | [34813217979](https://github.com/smiti1642/oxvif/actions/runs/34813217979)，上述精確候選；全部 27 jobs 通過 |
| 最終 0.17 staging | [34813220307](https://github.com/smiti1642/oxvif/actions/runs/34813220307)，相同 workflow／source SHA，publish=false；17 個驗證 jobs 通過，公開 GitHub Release job 正確跳過 |
| 下載產物 | 五份 archive 及兩份 Debian package：七份 SHA-256 checksum 全數通過；未安裝至使用者系統 |
| SBOM 比對 | 五種目標均與[前次 staging 34811578831](https://github.com/smiti1642/oxvif/actions/runs/34811578831) 比對；兩類掃描均為 syft 1.51.1。各 source SBOM 有 411 筆，涵蓋全部 410 組 lockfile 套件／版本；只有自身套件更新至 0.17.0。Binary inventory 名稱／版本／計數未變：Windows 3 筆，其餘各 1 筆 |

先前完整測試計數、原生安裝、實機及 ConPTY 證據保留原輸入版本，不能稱為此次
版號建置新執行的結果。最終原生 CI／staging 必須涵蓋準備後版號。
Staging 安裝驗證不代表已收錄至官方 winget／Chocolatey、Homebrew core 或
Debian／Ubuntu repository。
Binary inventory 稀疏，不是完整 Rust 相依清單；source inventory 含開發與其他平台
相依，不代表各 binary 的精確 linkage。掃描器一致不代表消除這些限制。

## 發布邊界

維護者於 2026-09-14 08:04:23 UTC 發布 oxvif 0.17.0，於 08:11:04 UTC
發布 oxvif-cli 0.17.0；兩個 registry 項目均另行確認，且未撤回。本次工作不執行
registry 上傳。使用者已明確授權 v0.17.0 tag 及正式 GitHub Release。
發布日期及防護狀態已完成；既有 workflow 以 publish=true、prerelease=false
執行，通過關卡後發布。Release 本文使用純英文分類摘要，不使用繁中檔或完整
技術 Changelog。本次未授權安裝至使用者系統。
`smiti1642` 已建立並推送 annotated tag `v0.17.0`，指向
`b4c203ae7adbe1d2a69513269a664010cc091f56`。已發布 library 的 registry VCS
紀錄為 `f26336553702db46d4a1d5f9c0268386e9b72035`，與 tag 之間僅文件不同。
本機 CLI 已對該實際 crates.io 相依完成打包驗證，未再次上傳。
[Tag 原始碼 CI](https://github.com/smiti1642/oxvif/actions/runs/34822362829) 通過。
[正式發布 workflow](https://github.com/smiti1642/oxvif/actions/runs/34822362983)
以相同 tag 作為 workflow 及 source，設定 publish=true／prerelease=false。
GitHub 發布結果：**成功**，時間為 2026-09-14 08:47:44 UTC。
[正式 v0.17.0 Release](https://github.com/smiti1642/oxvif/releases/tag/v0.17.0)
已公開，非草稿或 prerelease。包含發布步驟的全部 18 個 job 通過，29 個資產
皆已上傳。公開本文與 tag 中的純英文摘要完全一致；五平台 archive、兩種 Debian
套件、Homebrew formula／bottle、checksum 及 SPDX 清單均已確認存在。
本機核對 Windows archive checksum，解壓執行回報 oxvif 0.17.0，未安裝至系統；
公開資產的 GitHub digest 與受測 archive 相同。遠端 annotated tag 仍指向 b4c203a。
Release 作者為 github-actions[bot]，由 smiti1642 啟動的 workflow 建立；tag
署名為 smiti1642，未切換帳號。本次發布後紀錄只修改文件，不移動 release tag。
