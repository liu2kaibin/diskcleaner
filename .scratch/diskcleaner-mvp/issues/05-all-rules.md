# 05: 15 条规则全量接入

**What to build:** Safe to Remove ×8：npm / pip / uv / Cargo registry / Gradle+Maven 缓存 / 用户 Temp / 构建产物（项目内 target、build、dist、.next、.turbo、out）/ 缩略图缓存。Needs Review ×7：AI 模型（ollama/HuggingFace，按单个模型出结论含名称/大小/最后使用时间）、Android SDK、闲置项目 node_modules（90 天无变动）、Docker/WSL 虚拟磁盘（动作类型扩展「压缩命令」：生成 prune + shrink 指令而非删除）、Downloads 陈旧文件（>180 天）、旧安装包（>90 天）、AI 工具会话数据。每条规则有合成 fixture 用例；Review 结论附判断依据。

**Blocked by:** 02, 04

**Status:** ready-for-human

- [x] 15 条规则各有命中/未命中 fixture 用例（28/28 测试通过，覆盖全量 15 条规范规则）
- [x] AI 模型逐模型出结论，含名称/大小/最后使用时间（支持 Ollama 与 HuggingFace models-- 逐个解析）
- [x] Docker/WSL 结论生成压缩命令而非删除（动作类型枚举扩展为 Action::CustomCommand，输出 wsl --shutdown 与 docker system prune 脚本）
- [x] 构建产物规则只在项目目录内匹配（检查 Cargo.toml/package.json/go.mod 等项目标识，不误伤普通目录）
- [x] 闲置 node_modules 仅标记 90 天无变动项目（通过 Node::mtime 聚合时间精确过滤活跃项目）

## Comments

- 全量落地 15 条规则：
  - Safe to remove ×8：npm-cache / pip-cache / uv-cache / cargo-registry / gradle-maven-cache / user-temp / build-artifacts / thumbnail-cache
  - Needs review ×7：ai-models / android-sdk / idle-node-modules / docker-wsl-vhdx / downloads-old-files / downloads-installers / ai-tool-sessions
- `Action` 枚举扩展支持 `CustomCommand`，Docker/WSL 虚拟磁盘绝不误删，输出收缩命令。
- UI Recommendations 增加细分标签：`直删白名单` / `进回收目录` / `⚡ 维护压缩命令`。
