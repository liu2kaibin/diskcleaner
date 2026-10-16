# 02: Path Resolver + 规则引擎管道（接缝①）

**What to build:** 建立「快照+设置 → (结论清单, 脚本全文)」主测试接缝。Path Resolver 一律经环境变量与工具配置解析规则目标路径（如 OLLAMA_MODELS、CARGO_HOME、GRADLE_USER_HOME、PIP_CACHE_DIR、npm 缓存配置查询），禁止硬编码用户目录下的默认相对路径。规则引擎骨架：声明式规则表 → 匹配快照 → 分级结论（Rule / Safe to Remove / Semantic Category 术语见 GLOSSARY）。第一条规则 npm cache 落地。合成文件树 fixture 建立接缝①测试先例。UI 侧以简单列表显示 npm cache 命中结论与大小。

**Blocked by:** 01

**Status:** ready-for-human

- [x] 合成 fixture 上 npm cache 规则给出正确分级结论（命中/未命中、大小、类目）
- [x] Path Resolver 从环境变量/工具配置解析出缓存真实位置（npm_config_cache / NPM_CONFIG_CACHE / LOCALAPPDATA / USERPROFILE）
- [x] 接缝①测试：给定合成快照 → 断言结论清单，纯函数无磁盘 IO（11/11 tests pass）
- [x] UI 列表显示 npm cache 结论（右侧 Recommendations 面板支持卡片展示、标签与依据）

## Comments

- 建立了主测试接缝①：`evaluate_rules(&Snapshot, &PathResolver, &[Box<dyn Rule>]) -> Vec<Finding>` 纯函数管道。
- `PathResolver` 引入 `EnvLookup` trait，支持注入测试，不污染真实环境变量。
- `Snapshot::find_node` 实现快速树路径查找。
- egui 右侧添加 320px Recommendations 面板，展示命中项标题、大小、`● Safe to remove` 绿色标签、绝对路径与删除依据。
