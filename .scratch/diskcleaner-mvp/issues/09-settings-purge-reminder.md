# 09: 设置页 + 回收目录期满提示

**What to build:** 设置页四项：Recovery Directory 保留天数（默认 14）、Threshold Confirmation 阈值（默认 50GB）、Direct-Delete Whitelist 编辑、闲置天数（默认 90）。设置即时影响脚本生成与规则判定并持久化。启动时检查 Recovery Directory 中保留期满内容，提示清理并可一键生成清空脚本。

**Blocked by:** 04

**Status:** resolved

- [x] 四项设置可修改并即时生效（回收目录保留天数、50GB 确认门阈值、90 天闲置天数、直删白名单规则勾选列表）
- [x] 设置持久化，重启保留（保存于 `%LOCALAPPDATA%\diskcleaner\settings.json`，缺失/损坏自动回退默认，单元测试 5/5 全绿）
- [x] 期满提示列出待清项与总量（启动时扫描各卷 `.diskcleaner-trash`，期满自动在顶栏警示）
- [x] 一键生成回收目录清空脚本（提供清空期满项按钮，一键生成专属 PowerShell 清理脚本并拷入剪贴板）

## Comments

- 实现了纯函数设置与回收目录检查模块 `crates/core/src/settings.rs`。
- 界面顶栏增加 `⚙ 设置` 弹窗对话框，支持滑块调节天数与阈值、直选白名单规则。
- 实现了安全网闭环：放入回收目录的待清项不再成为永久垃圾，期满自动提醒并支持一键清空。
- 全项目 10 张票至此全部完工！
