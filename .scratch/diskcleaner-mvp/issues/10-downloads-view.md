# 10: Downloads 专项视图

**What to build:** Downloads 目录专项审阅列表：超过 180 天未动的文件与超过 90 天的安装包分列显示、按大小/时间排序、逐项勾选进 Cleanup Script 并联动统计卡。本机 Downloads 约 13GB，此票交付后可实际清理。

**Blocked by:** 05

**Status:** resolved

- [x] 两组列表（陈旧文件/旧安装包）按大小与时间排序（分组为 🕒 超过 180 天未动大文件 与 📦 超过 90 天历史安装包，支持“按容量大小 ↓”与“按时间排序”）
- [x] 逐项勾选联动 Cleanup Script 与统计卡（点击复选框即时联动顶部 Free after cleanup 增量、右侧面板与 Copy script 输出）
- [x] 本机 Downloads 实测可用（递归检索 Downloads 目录，提供“勾选全部安装包”与“勾选全部陈旧文件”快捷操作）

## Comments

- 增加了 `ViewMode::Downloads` 与 `ViewMode::Treemap` 双模式平滑切换，Explorer 面包屑栏右上角提供一键切换按钮。
- 下载审阅视图支持分组展示、容量汇总、快捷批量勾选与展开查看详情。
- 解决机器上 13GB Downloads 目录的精准清理诉求。
