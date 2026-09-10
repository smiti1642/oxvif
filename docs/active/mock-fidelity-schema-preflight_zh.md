# Mock schema 驗證前置檢查

[English](mock-fidelity-schema-preflight.md) | [繁體中文](mock-fidelity-schema-preflight_zh.md)

W20／W21 檢查點，2026-09-10。完整計畫仍在執行中。

| 章節 | 用途 |
| --- | --- |
| [結構檢查器](#結構檢查器) | 已交付的 W20 範圍 |
| [驗證證據](#驗證證據) | 正向與拒絕控制 |
| [外部驗證器評估](#外部驗證器評估) | 工具選擇與 K19 |
| [可重跑的驗證工具](#可重跑的驗證工具) | 固定來源、離線解析與 generic 控制 |
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

**K21 — 完整編譯仍受阻：** 將 warning 視為錯誤的 strict 編譯，在 Device 來源回報
`XMLSchemaTypeTableWarning`。分開編譯亦於 Device 重現；其餘九個 ONVIF service
root 與兩個支援 WSDL root 均無警告通過。這是外部驗證器／schema 選型問題，
不是已證實的 mock 缺陷。不可停用警告、改寫 schema，或把失敗的完整命令計為通過。
較早的 SOAP／Media 實驗不代表全服務支援，因此尚未執行完整清單的 instance gate。

`validate` 額外要求 `--corpus` 指向外部目錄，其中 `cases.json` 使用 format 1，
非空的 `cases` 每筆提供不重複的簡單 `file` 名稱與明確預期 `root` expanded name。
工具先確認 root 相符且有 schema 宣告，再嚴格驗證；缺檔、重複、路徑逸出及無效
instance 均失敗。Corpus 產生與操作覆蓋率核對仍待實作；這不驗證請求語意、效果、
WSDL binding 或實機行為。

CI 已加入 Windows／Linux 的 **generic 工具控制測試**，package job 依賴其結果。
這些 job 僅下載固定 Python wheel，不下載官方 schema，也不發布 schema artifact；
尚非 W22 的完整外部 instance gate。

## 後續工作

W20 仍為 PARTIAL：須核對未解析／wildcard 計數、Fault 的 QName 文字，以及擴充
fragment probe 以外的 corpus。W21 仍為 PARTIAL：須完成 K21 候選工具評估，並以
mock corpus 的 envelope、payload、Fault 正負 instance 驗證候選工具。
之後加入 W22 缺少前提即失敗的完整 schema CI job。此工具實驗不能取代
P-B 的逐操作欄位、Fault 與語意審查。

此診斷檢查點不需維護者新增決策。若選定的 gate 無法維持 D3 的散布或嚴格
驗證邊界，必須先提出討論，不能弱化規則。PR #16 整合仍依賴經審查的 Fault
與 D2 policy 路徑；其現有 acknowledgment-only mock 尚未驗收。
