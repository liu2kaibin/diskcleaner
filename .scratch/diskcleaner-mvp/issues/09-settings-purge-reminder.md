# 09: 设置页 + 回收目录期满提示

**What to build:** 设置页四项：Recovery Directory 保留天数（默认 14）、Threshold Confirmation 阈值（默认 50GB）、Direct-Delete Whitelist 编辑、闲置天数（默认 90）。设置即时影响脚本生成与规则判定并持久化。启动时检查 Recovery Directory 中保留期满内容，提示清理并可一键生成清空脚本。

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] 四项设置可修改并即时生效
- [ ] 设置持久化，重启保留
- [ ] 期满提示列出待清项与总量
- [ ] 一键生成回收目录清空脚本
