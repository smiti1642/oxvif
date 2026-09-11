# 0.17 發布切點之後的工作

[English](post-0.17-backlog.md) | [繁體中文](post-0.17-backlog_zh.md)

狀態：排於有限 0.17 切點之後，不承諾實作或發布日期。
[發布阻擋](release-0.17-cut_zh.md#發布阻擋關卡) 留在 0.17 驗收，
不得只為了清單綠燈而搬到此處。

| 章節 | 用途 |
| --- | --- |
| [延後批次](#延後批次) | 剩餘範圍與原始編號 |
| [重新納入規則](#重新納入規則) | 何時成為本版阻擋 |
| [保留追蹤](#保留追蹤) | 不遺失技術工作卡 |

## 延後批次

| ID | 範圍 | 原始追蹤／前置條件 |
| --- | --- | --- |
| F01 | 其餘 Media OSD、URI／source mode 及不支援的 configuration／binding 語意 | W10；保留已接受 Media 子群，逐項分類其餘操作 |
| F02 | 超出 profile identity 的 PTZ configuration／space、movement／preset／home／tour 效果；Imaging 欄位及 focus 建模 | W11／W12；不宣稱現有 profile 檢查已模擬移動 |
| F03 | 完整 Device／DeviceIO／network／user／relay 契約與 Recording／Search／Replay 生命週期 | W13／W14；不改 host network；硬體效果另行授權 |
| F04 | Events filter、subscription、queue 隔離、renew／expiry／termination 及傳送建模 | W15；Media sync 不等於 Events sync；保留目前僅收件確認的預設拒絕 |
| F05 | 更完整 HTTP binding、WSSE freshness／replay protection、角色及 fault injection 重設計 | W07–W09；先完成本版安全／完整性判定；raw hook 保留明確逃生出口 |
| F06 | 全程式 capability 一致性、併發／rollback、replay 相依及超出 K27 碰撞保留修正的 key-format 重設計 | W16–W19；K27 儲存保留已在候選修復，並未延後；見[遷移](../replay-storage_zh.md) |
| F07 | 其餘 schema／QName／wildcard 控制、更廣 corpus 及 fuzz／property 覆蓋 | W20–W23；保留選定獨立 corpus 關卡，不宣稱全部操作覆蓋 |
| F08 | RTSP decoder／playback、批次檔案匯出、CLI 攝影機寫入及獨立導覽 crate | 既有 CLI 維護／導覽計畫；寫入前先建立威脅模型與權限 UX |
| F09 | 官方 Homebrew Core、Debian／Ubuntu 與 Windows 社群套件渠道申請 | 散布計畫；目前原生包裝驗證仍是發布關卡，產出 artifact 不代表已提交申請 |

不因 outdated 報告列出版本，就將新的相依 major 升級加入 0.17。
安全公告修復與相容性修正仍依發布阻擋政策處理。

## 重新納入規則

若可重現證據顯示資料遺失、機密暴露、不安全副作用、收錄契約中的誤導成功，
或共用修改破壞既有行為，該項返回本版。先於 G01／G02／G03 記錄精確案例、
影響、修正／回復／拒絕方案及驗收檢查，再決定處置。
僅缺少選用功能本身不構成阻擋。

## 保留追蹤

保留 [W00–W26](mock-fidelity-execution-checklist_zh.md)、C01–C12 及 K findings
的既有證據和真實 PARTIAL／TODO 狀態。R01–R08 收錄完成子群，不代表完整里程碑。
W00–W06 基礎及 W24／W25 發布關卡仍屬本版驗收，不整批延後。
後續施工須沿用操作工作卡、批次節奏及雙語要求，不依賴對話記憶重建範圍。
