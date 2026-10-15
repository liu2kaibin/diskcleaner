# 05: 15 条规则全量接入

**What to build:** Safe to Remove ×8：npm / pip / uv / Cargo registry / Gradle+Maven 缓存 / 用户 Temp / 构建产物（项目内 target、build、dist、.next、.turbo、out）/ 缩略图缓存。Needs Review ×7：AI 模型（ollama/HuggingFace，按单个模型出结论含名称/大小/最后使用时间）、Android SDK、闲置项目 node_modules（90 天无变动）、Docker/WSL 虚拟磁盘（动作类型扩展「压缩命令」：生成 prune + shrink 指令而非删除）、Downloads 陈旧文件（>180 天）、旧安装包（>90 天）、AI 工具会话数据。每条规则有合成 fixture 用例；Review 结论附判断依据。

**Blocked by:** 02, 04

**Status:** ready-for-agent

- [ ] 15 条规则各有命中/未命中 fixture 用例
- [ ] AI 模型逐模型出结论，含名称/大小/最后使用时间
- [ ] Docker/WSL 结论生成压缩命令而非删除（动作类型枚举扩展）
- [ ] 构建产物规则只在项目目录内匹配
- [ ] 闲置 node_modules 仅标记 90 天无变动项目
