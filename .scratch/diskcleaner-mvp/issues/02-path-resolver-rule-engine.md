# 02: Path Resolver + 规则引擎管道（接缝①）

**What to build:** 建立「快照+设置 → (结论清单, 脚本全文)」主测试接缝。Path Resolver 一律经环境变量与工具配置解析规则目标路径（如 OLLAMA_MODELS、CARGO_HOME、GRADLE_USER_HOME、PIP_CACHE_DIR、npm 缓存配置查询），禁止硬编码用户目录下的默认相对路径。规则引擎骨架：声明式规则表 → 匹配快照 → 分级结论（Rule / Safe to Remove / Semantic Category 术语见 GLOSSARY）。第一条规则 npm cache 落地。合成文件树 fixture 建立接缝①测试先例。UI 侧以简单列表显示 npm cache 命中结论与大小。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 合成 fixture 上 npm cache 规则给出正确分级结论（命中/未命中、大小、类目）
- [ ] Path Resolver 从环境变量/工具配置解析出缓存真实位置（本机实测：默认位置不存在时也能找到）
- [ ] 接缝①测试：给定合成快照 → 断言结论清单，纯函数无磁盘 IO
- [ ] UI 列表显示 npm cache 结论
