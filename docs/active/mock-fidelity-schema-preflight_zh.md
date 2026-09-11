# Mock schema 驗證前置檢查

[English](mock-fidelity-schema-preflight.md) | [繁體中文](mock-fidelity-schema-preflight_zh.md)

W20／W21 檢查點，2026-09-10。完整計畫仍在執行中。

| 章節 | 用途 |
| --- | --- |
| [結構檢查器](#結構檢查器) | 已交付的 W20 範圍 |
| [驗證證據](#驗證證據) | 正向與拒絕控制 |
| [外部驗證器評估](#外部驗證器評估) | 工具選擇與 K19 |
| [可重跑的驗證工具](#可重跑的驗證工具) | 固定來源、離線解析與 generic 控制 |
| [獨立 Xerces 後端](#獨立-xerces-後端) | 全服務編譯與工具驗收 |
| [Profile exchange corpus](#profile-exchange-corpus) | Client 產生的 instance 與已知失敗 |
| [後續工作](#後續工作) | 尚待驗收事項，而非完成宣告 |

## 結構檢查器

`tests/mock_schema_shape.rs` 現在逐節點保存 namespace binding，不再將文件最後
累積的 prefix map 套用到所有節點。此修正涵蓋輸出 element／attribute 名稱及
schema 的 `type`、`base`、`ref` 值。無 prefix 的 attribute 仍不屬於 default
namespace；無 prefix 的 QName 值使用所屬節點的 default namespace，而非 schema
的 target namespace。無效／未宣告名稱及 expanded name 重複的 attribute 不再
默默退回空 namespace 或被略過。

明確選取執行的外部測試現在要求 `OXVIF_ONVIF_SCHEMA` 與可定位的 SOAP 1.2
Envelope／Fault 宣告；缺少資源即失敗。一般無 schema 測試仍維持 ignored。
每個生成回應須具有 SOAP Envelope、單一 Body 及單一 payload。Envelope 與
payload 分別檢查；Fault 不再排除於定位／結構檢查之外。計數分開呈現成功
payload、實際 Fault 及遞迴定位的節點數。

這仍是結構稽核，而非完整 XSD 驗證器。它不驗證 Fault code 的 lexical value，
也不能證明逐操作錯誤映射正確。Wildcard、未解析型別、值及 corpus 覆蓋仍需
W20／W21 處理。Probe corpus 仍使用舊式 fragment，不構成 client request 合法的證據。

## 驗證證據

七個無外部 schema 的控制使用專案自行設計的名稱及 schema：
`checker_restores_namespace_scope_after_children_and_empty_elements`、
`checker_attributes_are_scoped_normalized_and_not_default_qualified`、
`checker_qname_values_use_owner_scope_not_the_final_file_map`、
`checker_schema_index_keeps_local_type_base_and_ref_bindings`、
`checker_rejects_unbound_or_duplicate_expanded_attributes`、
`checker_does_not_silently_replace_roots_or_accept_incomplete_documents`、
`checker_payload_selection_rejects_wrong_envelopes_and_ambiguous_bodies`。
逐一擾動 fixture 後，七項皆在預期 payload assertion 失敗，之後完整還原。
未設定環境變數的明確執行也依預期失敗。

本機外部結構檢查：Windows、Rust 1.97.0，20 個外部檔案於 2026-09-10 取得；
158 個回應，包含 108 個成功 payload、50 個 Fault、1,216 個定位節點、
1,366 個略過子節點及 384 個受檢 attribute。十項既有 finding pin 仍為零，
沒有修改 pin。將一般 Fault 的 Reason wrapper 改為 Detail 後，產生 50 個
finding（一種 missing-required finding），測試於 pin assertion 失敗；之後
還原 helper。官方資源及實驗腳本均保留在 checkout 外。
本機 gate：格式與兩種 workspace Clippy 均通過；全功能 1,162 項與預設功能
1,082 項測試通過（各有 4 項 ignored）。清冊核對及 135 個本地文件連結通過。
這些結果不是完整驗證器的驗收結果。

前一檢查點 `e8fda59` 已通過
[CI run 34454688822](https://github.com/smiti1642/oxvif/actions/runs/34454688822)
全部 23 個 job，包含 Windows／Linux／macOS 原生測試與 smoke check。該 run
早於這次檢查器修正，不是本次修改的託管驗收，也不代表 M6 最終完成。

## 外部驗證器評估

K19 是外部資源與工具相容性發現，不是 mock 實作缺陷。相同且未修改的目前
Media 來源相依集合已接受以下測試：

| 驗證器 | 結果 |
| --- | --- |
| lxml 6.1.1／libxml2 2.11.9，XSD 1.0 | SOAP 編譯成功；Media1 相依集合因非確定內容模型遭拒 |
| xmlschema 4.3.2／elementpath 5.1.4，XSD 1.0 | SOAP 編譯成功；兩個 Media 相依集合因 Unique Particle Attribution 衝突遭拒 |
| xmlschema 4.3.2／elementpath 5.1.4，XSD 1.1 | SOAP 與兩個 Media 相依集合均通過 strict schema 編譯 |

XSD 1.1 結果僅是候選工具設定，**不是** XSD 1.0 通過或 instance 驗證結果。
未修改 schema，也未停用驗證器檢查。Python 套件只安裝於外部暫存 venv；
Rust runtime dependency graph 與使用者已安裝版本均未改變。

代表性來源的 SHA-256：

| 官方來源 | SHA-256 |
| --- | --- |
| [Media1 WSDL](https://www.onvif.org/ver10/media/wsdl/media.wsdl) | `bb4596f47ff0f907b8113fd2c86f649faa098df0e0df9f1c5f4de34f592de1eb` |
| [Media2 WSDL](https://www.onvif.org/ver20/media/wsdl/media.wsdl) | `0a1d59636910074ec4fb80b9a43f398ce208f4e38a6c7dff414eb6ddba3a1613` |
| [ONVIF schema](https://www.onvif.org/ver10/schema/onvif.xsd) | `1de6e9dd31a18a6773b611c4f7eaa0528f219098ddfc883b380802aa9a7ff647` |
| [Common schema](https://www.onvif.org/ver10/schema/common.xsd) | `d945394fe823febcd873ed8444ccfd73f3b5234d6dfbf0e151a33f5e611b5077` |
| [SOAP envelope](https://www.w3.org/2003/05/soap-envelope) | `3ae8caa9a74e83528cc0e1a59fc12784435e8e0086e890d0e1b04d881c079bec` |

完整 URL／hash manifest 現在記錄於 `packaging/schema-sources.json`；上表保留
原始選型實驗的代表性識別值。
工具參考：[xmlschema API](https://xmlschema.readthedocs.io/en/stable/api.html)、
[XSD 1.1 支援](https://xmlschema.readthedocs.io/en/stable/features.html)、
[lxml validation](https://lxml.de/validation.html)。

## 可重跑的驗證工具

`packaging/verify_schemas.py` 分別提供 `fetch`、`check`、`compile` 與 `validate`。
23 份來源的 manifest 僅含 URL／hash metadata。下載文件、venv、instance corpus
及任何衍生資料必須保留於 checkout 與套件外；工具拒絕 checkout 內或其上層的
資源目錄。Fetch 使用 HTTPS、大小限制、精確雜湊與排他建立；既有檔案內容不符時
不會覆寫，也不會為了通過驗證器而改寫來源。

編譯前先核對所有雜湊及 XSD／WSDL 宣告的相依位置。目錄結構對應來源 URL 路徑，
保留相對 import 的解析基準。URI mapper 與僅支援檔案的 opener 共同要求最終
絕對位置精確命中固定清單；不提供網路 handler 或 schema fallback。拒絕 XML DTD，
包含 UTF-16 宣告。驗證專用的 `xmlschema`／`elementpath` wheel 版本與雜湊固定於
`packaging/schema-requirements.txt`，不影響 Rust runtime。Instance 的 location hint
不能載入額外 schema。

請使用獨立的外部 virtual environment，以下 `python` 代表該環境的直譯器。
資源路徑須替換成實際的外部絕對目錄；Windows 亦可使用 `C:/Temp/oxvif-schema-check`。

```text
python -m pip install --require-hashes --only-binary=:all: -r packaging/schema-requirements.txt
python -m unittest discover -s packaging -p test_verify_schemas.py -v
python packaging/verify_schemas.py fetch --root /absolute/external/oxvif-schema-check
python packaging/verify_schemas.py check --root /absolute/external/oxvif-schema-check
python packaging/verify_schemas.py compile --root /absolute/external/oxvif-schema-check
```

本機證據：全新環境以固定 wheel 雜湊安裝成功；23 份檔案、12 個獨立 schema root、
30 條宣告相依邊的下載及離線閉包核對通過。15 項 generic 測試涵蓋路徑、雜湊、
相依及 DTD 拒絕、無網路編譯、scoped QName／純量值、數量、順序、必要 attribute、
繼承的 WSDL namespace binding、相依版本、明確 root 與已清理的失敗訊息。
暫時移除雜湊與 instance 驗證後，雜湊控制及七種無效 instance 控制均失敗；
還原後全部 19 項 packaging 測試通過。這些 generic
fixture 為專案自行設計，不是 ONVIF 衍生資料。

**K21 — Python 後端限制：** 將 warning 視為錯誤的 strict 編譯，在 Device 來源回報
`XMLSchemaTypeTableWarning`。分開編譯亦於 Device 重現；其餘九個 ONVIF service
root 與兩個支援 WSDL root 均無警告通過。這是外部驗證器／schema 選型問題，
不是已證實的 mock 缺陷。不可停用警告、改寫 schema，或把失敗的完整命令計為通過。
較早的 SOAP／Media 實驗不代表全服務支援，因此未執行 Python 完整清單的 instance
gate。下方的獨立 Xerces 後端已提供可用的編譯路徑，不必弱化 Python 診斷或修改來源。

`validate` 額外要求 `--corpus` 指向外部目錄，其中 `cases.json` 使用 format 1，
非空的 `cases` 每筆提供不重複的簡單 `file` 名稱與明確預期 `root` expanded name。
工具先確認 root 相符且有 schema 宣告，再嚴格驗證；缺檔、重複、路徑逸出及無效
instance 均失敗。可選的 `payload_path` 是有長度限制的 expanded name 清單，每一步
選取唯一直接 child，最後一層的 parent 必須僅有一個 element。選中的 payload 亦須
以有宣告的 root 獨立通過驗證，防止 lax envelope wildcard 隱藏未宣告操作或多個
payload，並保留 namespace scope。下方已開始產生 corpus，但全程式覆蓋率仍未完成；
這不驗證請求語意、效果、
WSDL binding 或實機行為。

最初的工具檢查點加入 Windows／Linux generic 控制；下節說明後續擴充及獨立的
來源編譯 job。兩者均非 W22 的完整 mock instance gate。

## 獨立 Xerces 後端

`packaging/verify_schemas_xerces.py` 與 `SchemaVerifier.java` 使用
[Apache Xerces-J 2.12.2 XSD 1.1 distribution](https://xerces.apache.org/xerces2-j/)
提供獨立後端。下載封存的 SHA-512 及四個必要 JAR 的 SHA-256 固定於
`packaging/xerces-validator.json`；只解開這些已核對的成員。官方 schema 與驗證器
distribution 均不隨 oxvif 打包。JDK 17+ 可直接執行 Java source，不需系統安裝或
修改 PATH。本機比對使用外部可攜式 Temurin 17.0.20.1+1，封存 SHA-256 已對照
Adoptium metadata 核對：`e53a79c3c3d86865bd7e787903884331068e71321714ffd44f145785affc7cb0`。

Adapter 保留完整 schema checking，將 warning／error 視為失敗。精確清單 resolver
不會回傳 null 以請求預設查找。WSDL 擷取保留繼承的 namespace binding 及後代的
重新宣告，並以原始 owner 目錄解析相對 import。暫存衍生檔位於 checkout 外，
執行後自動清除；不改寫來源。Instance 解析停用 DTD、外部 entity 與 XInclude，
驗證前先核對預期 root。錯誤只輸出穩定的例外／constraint identifier，不含 schema
片段或輸入值。

本機結果：相同 23 檔閉包，以 12 個明確 root 合併成一個 schema set，無警告編譯
通過。這證實 K21 有可用的獨立編譯路徑，**不代表** Python 警告已被證明錯誤，
也不代表 mock 符合規格；未停用 Python 的警告。五項必要選型測試涵蓋有效的
imported-schema instance、八種無效值／QName／shape／root、無效 schema、
DTD／import 拒絕及 location-hint 控制；拒絕結果精確比對已清理的 constraint
identifier。移除 Java instance-validator 呼叫後，七種 schema-invalid 控制均失敗，
之後已還原。另外三項不依賴官方 schema 的測試檢查擷取 scope、metadata injection
及相依檔竄改。全部 22 項 packaging 測試已在本檢查點最終 gate 前通過；必要的
後端選型不會因缺少工具而靜默跳過。

執行前述來源 `fetch`／`check` 後，使用已驗收的編譯器：

```text
python packaging/verify_schemas_xerces.py fetch-tool --tool-root /absolute/external/oxvif-xerces
python packaging/qualify_xerces.py --tool-root /absolute/external/oxvif-xerces --java /absolute/jdk/bin/java
python packaging/verify_schemas_xerces.py compile --root /absolute/external/oxvif-schema-check --tool-root /absolute/external/oxvif-xerces --java /absolute/jdk/bin/java
```

`validate` 使用前述外部 `--corpus` 格式。全程式的 corpus 產生及操作覆蓋率仍待完成；
編譯官方 schema 並不驗證任何 mock exchange。較早的 `verify_schemas.py compile`
仍是 Python 後端診斷命令，預期會揭露 K21。

Windows／Linux CI 現在先執行 generic 控制及獨立 Xerces 選型，再執行獨立的
**Official schemas and selected profile corpus** job，使用固定來源雜湊、外部目錄且不上傳 artifact。
後者在編譯後明確匯出並驗證選定的 40 份 profile instance。每個 native 命令失敗均
終止 job；驗證要求既有且非空的 corpus，因此缺少匯出不會視為通過。
兩者均作為 package 前提，但不可回報為完整操作／corpus 驗收。前次 CI
[34461384194](https://github.com/smiti1642/oxvif/actions/runs/34461384194)
已通過 `4fdd9f2` 的全部 25 個 job；該 run 早於 Xerces adapter 及新增編譯 job。

## Profile exchange corpus

`tests/mock_schema_corpus.rs` 現在透過未設定憑證的 `OnvifClient` 呼叫 in-process
mock，擷取完整 request／response 字串；不讀取官方 schema，也不連線至攝影機或
網路。測試精確比對第一批 13 張 profile 工作卡的來源 Action 集合。20 組 exchange
涵蓋 13 個操作、四個不存在／固定 profile 拒絕及三個空身分政策拒絕（Create Sender
與兩種 list Receiver 回應）；綁定、建立與刪除皆有 state 斷言，
並非只確認呼叫成功。Fault 預期由測試流程指定，不以搜尋回應字串猜測。

Ignored 匯出測試要求 `OXVIF_MOCK_CORPUS` 指向**尚未存在的外部絕對目錄，且其
parent 已存在**。工具拒絕空資料、含認證欄位的 request、相對／既有目錄及
checkout／其上層位置。匯出保留 XML bytes，產生 40 個檔案及 `cases.json`，並為
request、成功與 Fault response 記錄明確的 Envelope／Body／operation 預期。
不讀取環境憑證或覆寫檔案；這是診斷 corpus，不是完整逐操作驗收。

```powershell
$env:OXVIF_MOCK_CORPUS = 'C:/Temp/oxvif-profile-corpus-new'
cargo test --all-features --test mock_schema_corpus export_first_profile_batch -- --ignored --nocapture
Remove-Item Env:OXVIF_MOCK_CORPUS
```

將匯出目錄傳給 Xerces 的 `validate --corpus`，並使用編譯時相同的固定 `--root`、
`--tool-root` 與 `--java`。`1a0ac1c` 的初始 corpus 為 **28 份有效／2 份無效**：
兩個不存在 profile 的 DeleteProfile 回應因舊 helper 將 `ter:NoProfile` 放在
SOAP Code 而觸發 `cvc-enumeration-valid`。完成經審查的 DeleteProfile Fault 遷移
後，新外部匯出為 **34 份有效／0 份無效**（17 份 request、13 份成功回應、四份
不存在／固定 profile Fault）。含明確 payload anchor 的獨立 Xerces 嚴格驗證回傳
exit 0。這修正選定的 K03 分支，不包含其餘一般 Fault mapping；也不代表省略欄位、
其他輸入、state 語意或剩餘 route 已驗收。

敏感度控制分別將固定 profile 的頂層 code 改為 Receiver，以及將不存在 profile 的
最深層改為 `WrongProfile`。完整全部功能 `--no-fail-fast` 執行分別在精確的 client
code 與擷取回應 leaf 斷言失敗，兩項擾動均已還原。Exporter 改從擷取資料計算數量，
不再輸出過時常數。官方 schema 內容未進入 checkout。

兩個驗證後端均新增 namespace-scope 正向控制及缺少、未宣告、多 payload 的負向
控制。停用 payload 檢查後，每個後端新增的兩項測試均失敗，之後已還原。Rust 的
Action 集合及目錄錯誤斷言也在完整全部功能 `--no-fail-fast` 擾動執行中失敗，再予
還原。Ignored exporter 是普通測試之外的明確操作，不代表已通過 schema 驗證。

託管 CI [34463097025](https://github.com/smiti1642/oxvif/actions/runs/34463097025)
已通過 `cdfeaec` 的全部 27 個 job，包含 Windows／Linux 官方 schema 編譯；
該 run 早於此次 corpus 新增。未變更 Release、安裝版本、實機設定、公開 API 或
貢獻者 PR。

## 後續工作

W20 仍為 PARTIAL：須核對未解析／wildcard 計數、Fault 的 QName 文字，以及擴充
第一批 13 操作以外的 corpus。W21 仍為 PARTIAL：須以 mock corpus 的 envelope、
payload、Fault 正負 instance 驗收更廣泛的實際 exchange。W22 已在 Windows／Linux
加入選定的 40 份 profile corpus 驗證；缺少前提即失敗的完整 instance gate 仍須涵蓋
其餘操作批次。此工具實驗不能取代
P-B 的逐操作欄位、Fault 與語意審查。

此診斷檢查點不需維護者新增決策。若選定的 gate 無法維持 D3 的散布或嚴格
驗證邊界，必須先提出討論，不能弱化規則。PR #16 整合仍依賴經審查的 Fault
與 D2 policy 路徑；其現有 acknowledgment-only mock 尚未驗收。
