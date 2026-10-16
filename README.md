# DiskCleaner 💽

> **开发者视角的 Windows 磁盘空间分析与安全清理工具**  
> *A developer-centric Windows disk space analyzer & safe cleanup tool built in pure Rust (`egui`).*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021_Edition-orange.svg)](Cargo.toml)
[![Tests: 41 Passed](https://img.shields.io/badge/Tests-41%20Passed-brightgreen.svg)](crates/core/src)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows_x64-blue.svg)](crates/app)

---

## 🌟 为什么需要 DiskCleaner？

作为开发者，当开发机 C 盘或 D 盘报红时，传统清理工具有两类典型缺陷：
1. **WizTree / SpaceSniffer 类**：只告诉你“这个文件夹占了 20GB”，却无法解释“这是什么资产、删了会发生什么”。8GB 的 Ollama 权重和 15GB 的 Android SDK 在它们眼里只是两块几何面积。
2. **CCleaner 类一键清理**：面向大众普通软件的垃圾清理，缺乏对开发工具链（npm/cargo/pip/uv/gradle、代码项目构建中间产物、虚拟磁盘）的语义理解，且“一键清理”极易造成不可逆的代码与环境破坏。

**DiskCleaner** 专门针对现代开发者的痛点设计：
- **语义分级识别**：将空间严格区分为 **Safe to remove**（可无损再生）与 **Needs review**（需人工确认）。
- **零直接误删架构**：应用本身**绝不直接执行任何删除**，唯一出口是供人工过目核验后自行执行的 **PowerShell 清理脚本**。
- **安全后悔期保障**：非白名单项默认同卷移入各盘根目录的 **`\.diskcleaner-trash\`**（14 天保留期）；单次删除超过 **50GB 强制交互确认门**。
- **虚拟磁盘安全收缩**：对于 Docker / WSL 的 `ext4.vhdx`，**绝不直接删除**，而是生成 `wsl --shutdown; docker system prune` 等维护收缩指令。

---

## 🖥️ 交互与界面布局

应用采用现代轻量粉彩（Pastel）设计，全面还原专业级开发者看板：

```text
+---------------------------------------------------------------------------------------------------------+
| [重新扫描]  Free after cleanup: 109.5 GB (+13.4 GB) | ● Safe to remove: 13.4 GB | ● Needs review | Not scanned |
+------------------------------------------------------------------------+--------------------------------+
| Explorer    ~ Home / Desktop / my_project                  135 GB      | Recommendations                |
| +--------------------------------------------------------------------+ | Select items to build a script |
| |                                                                    | |                                |
| |                         Treemap 交互区                             | | [Safe to remove]     13.4 GB |
| |               (点击瓦片下钻、粉彩语义类目配色)                     | | [✓] npm cache        13.4 GB>|
| |                                                                    | |     ~\AppData\Local\npm-cache|
| |                                                                    | |                                |
| +--------------------------------------------------------------------+ | [Review first]         32.0 GB |
| 🟧 Documents & media  ⬜ Dev tools  🟪 Code  🟦 SDK  🌸 AI  🟩 Caches | | [✓] Ollama models    16.7 GB>|
|                                                                        |                                |
|                                                                        | 13.4 GB selected · 1 items     |
|                                                                        | [Select safe]    [Copy script] |
+------------------------------------------------------------------------+--------------------------------+
```

### 1. 顶部 4 张大统计卡
- **Free after cleanup**：大号加粗数值，实时显示 `当前可用容量 + 已勾选项释放量`，带绿色 `+XX GB` 增量角标。
- **● Safe to remove**：绿色圆点，汇总全部可安全清理项的体积与数量。
- **● Needs review**：琥珀色圆点，汇总需人工确认的资产占用。
- **Not scanned**：通过 Windows 底层 API 查询卷容量并扣除扫描域，精准展示 `system & apps` 系统区域占用。

### 2. 交互式 Treemap 空间图与瓦片下钻
- **Squarified 算法**：纯数学几何布局，长宽比最优化，面积严格守恒。
- **语义类目着色**：代码项目（蓝紫）、AI 大模型（粉红）、包管理器缓存（薄荷绿）、SDK（青蓝）、文档媒体（桃橙）、开发工具（冷灰）。
- **逐级下钻（Drilldown）**：点击目录瓦片深入查看子结构，配合顶部 `~ Home / Desktop / ...` 面包屑可随时点击回退。

### 3. 📥 Downloads 专项审阅视图（直击 10GB+ 下载占用）
在 Explorer 右上角点击 **`[📥 Downloads 专项审阅]`** 即可切换至下载排查专用模式：
- **🕒 超过 180 天未动大文件（>50MB）**：排查长期遗忘的 ISO、大压缩包和数据备份。
- **📦 超过 90 天历史安装包（.exe / .msi）**：排查早已过期的软件安装包。
- 支持按容量排序、按时间排序，以及一键全选。

### 4. 推荐清理面板与脚本复制
- 胶囊分组显示各规则命中，路径智能缩短为 `~/.npm` 格式。
- 点击右侧箭头 `>` 可展开/折叠单项详细说明与具体执行动作。
- 底部提供 **`[Select safe]`**（一键勾选安全项）与 **`[Copy script]`**（将安全审计过的 PowerShell 脚本复制至剪贴板）。

---

## 📋 15 条规范规则清单

| 分级 | 规则 ID | 目标类型 | 路径解析策略 | 默认动作 |
| :--- | :--- | :--- | :--- | :--- |
| **Safe** | `npm-cache` | npm 下载缓存 | `npm_config_cache` / `%LocalAppData%\npm-cache` | 直删白名单 |
| **Safe** | `pip-cache` | Python pip 缓存 | `PIP_CACHE_DIR` / `%LocalAppData%\pip\cache` | 直删白名单 |
| **Safe** | `uv-cache` | Python uv 虚拟环境与包 | `UV_CACHE_DIR` / `%LocalAppData%\uv` | 直删白名单 |
| **Safe** | `cargo-registry` | Rust Crates.io 源码索引 | `CARGO_HOME/registry` / `~/.cargo/registry` | 直删白名单 |
| **Safe** | `gradle-maven-cache`| Gradle / Maven 依赖库 | `GRADLE_USER_HOME/caches` / `~/.m2/repository`| 直删白名单 |
| **Safe** | `user-temp` | Windows 用户级临时文件 | `TEMP` / `TMP` / `%LocalAppData%\Temp` | 直删白名单 |
| **Safe** | `build-artifacts` | 代码项目构建产物 | 项目内 `target/`, `build/`, `dist/`, `.next/`, `.turbo/`, `out/` | 直删白名单 |
| **Safe** | `thumbnail-cache` | Windows 缩略图缓存 | `%LocalAppData%\Microsoft\Windows\Explorer\thumbcache_*.db` | 直删白名单 |
| **Review** | `ai-models` | Ollama / HuggingFace 模型 | `OLLAMA_MODELS` 及 `HF_HOME/hub` 逐模型列出 | 移入回收目录 |
| **Review** | `android-sdk` | Android SDK 与模拟器 | `ANDROID_HOME` / `%LocalAppData%\Android\Sdk` | 移入回收目录 |
| **Review** | `idle-node-modules`| 闲置项目 node_modules | 项目内超过 90 天无修改的代码库 `node_modules` | 直删白名单 |
| **Review** | `docker-wsl-vhdx` | Docker/WSL 虚拟磁盘 | `%LocalAppData%\Docker\wsl\data\ext4.vhdx` 等 | **⚡ 维护压缩命令** |
| **Review** | `downloads-old-files`| Downloads 长期未用大文件 | `~/Downloads` 中 >180 天且 >50MB 文件 | 移入回收目录 |
| **Review** | `downloads-installers`| Downloads 历史安装包 | `~/Downloads` 中 >90 天的 `.exe` / `.msi` | 移入回收目录 |
| **Review** | `ai-tool-sessions` | Claude Code / Cursor 会话 | `~/.claude/projects`, `~/.cursor` | 移入回收目录 |

---

## ⚙️ 个性化配置与安全网

点击顶栏右上角 **`⚙ 设置`** 按钮，可呼出偏好配置弹窗（自动持久化保存于 `%LOCALAPPDATA%\diskcleaner\settings.json`）：
- **回收目录保留期**：自定义保留天数（1 ~ 90 天，默认 14 天）。
- **二次确认安全阈值**：当次删除总量超限报警门限（10 ~ 200 GB，默认 50 GB）。
- **闲置项目判定天数**：代码项目视为闲置的基准时间（30 ~ 365 天，默认 90 天）。
- **直删白名单规则编辑**：自由勾选哪些规则允许直接删除、哪些必须移入回收目录。
- **期满回收提醒与一键清空**：启动时自动扫描各卷回收目录，一旦发现保留期满内容，顶栏给出黄色告警，并提供 **`[清空期满项]`** 专用脚本生成按钮。

---

## 🚀 性能与底层边界

1. **并行元数据扫描**：基于 Rayon 线程池与 `jwalk` 深度优化，实测在普通权限下遍历包含 33.2GB / 数十万小文件的用户目录仅耗时 **30.8 秒**，远低于 60 秒性能预算（ADR-0004）。
2. **秒开冷启动**：扫描完成后自动持久化快照至本地 JSON，下次启动在 **0.8 秒内秒开呈现**，内存常驻仅 45MB；后台重扫完全不阻塞前台浏览。
3. **Windows 极端边界硬化**：
   - **Junction 循环防护**：遇到软链接或目录 Junction 时只记录单一实体，绝不死循环递归，不重复计算体积。
   - **超长路径支持**：支持突破 Windows 260 字符传统 MAX_PATH 限制的深度路径。
   - **OneDrive / 云盘占位安全**：仅通过文件系统元数据读取属性位，绝不调用 `File::open`，避免意外触发云端大文件全量回迁下载。
   - **受限权限容错**：无读取权限目录优雅跳过并准确计数，并在顶栏提示。

---

## 🛠️ 构建与开发指南

### 前置要求
- Windows 10/11 x64
- Rust 工具链（推荐 1.80+，MSVC ABI）
- C++ Build Tools（Visual Studio Build Tools 或带有 MSVC 链接器的环境）

### 克隆与编译
```bash
git clone https://github.com/liu2kaibin/diskcleaner.git
cd diskcleaner

# 运行所有自动化测试（41 项测试）
cargo test

# 运行真实机器用户目录性能守护测试（<60s 预算，可选）
cargo test -p diskcleaner-core --release -- --ignored

# 构建 Release 单可执行文件
cargo build --release
```

*生成的可执行文件位于 `target/release/diskcleaner.exe`。*

> **8GB 内存 Windows 开发机优化建议**：
> 本仓库已在 `Cargo.toml` 中配置 `[profile.dev] debug = 0`，并在 `.cargo/config.toml` 中限制 `[build] jobs = 1`。建议将 `target/` 目录添加至 Windows Defender 排除项，以获得数倍编译速度提升。

---

## 📄 许可证

本项目采用 [MIT 许可证](LICENSE)。
