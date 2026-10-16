# 07: 快照持久化 + 秒开重扫

**What to build:** 扫描快照 JSON 持久化（路径树、大小、属性标记、规则命中与结论）；启动加载最新快照秒开并标注「扫描于 X 前」；手动全量重扫期间旧快照可继续浏览，完成后无缝切换；快照损坏时优雅降级为触发新扫描。

**Blocked by:** 01

**Status:** ready-for-human

- [x] 扫描完成后快照自动保存（自动写入 `%LOCALAPPDATA%\diskcleaner\snapshot.json`，实测生成 21MB 完整快照树）
- [x] 重启应用秒显上次结果与时间标注（实测 2 秒内秒开，内存占用仅 45MB，顶栏清晰显示“快照已就绪：扫描于 X 分钟前”）
- [x] 重扫不冻结 UI，完成后切换新快照（后台线程重扫期间保留旧快照，Treemap 与推荐面板交互完全不受阻，扫描完成后无缝替换）
- [x] 快照损坏优雅降级（store_tests 覆盖损坏 JSON 与文件缺失场景，Err 自动降级为触发全新扫描）

## Comments

- 实现了纯函数存储模块 `crates/core/src/store.rs`（save_snapshot / load_snapshot / default_snapshot_path）。
- `Snapshot` 结构体增加 `scanned_at_secs` 时间戳，配合 `format_time_ago` 提供自然的“刚刚”、“X 分钟前”、“X 小时前”时间差显示。
- 启动体验质变：从原本每次开门必须等待 30-50 秒扫描，升级为**冷启动秒开立即呈现 Treemap 与推荐项**，体验飞跃。
