# DiskCleaner v0.1.0 - Initial MVP Release 💽

DiskCleaner 是专为现代开发者打造的 Windows 磁盘空间分析与安全清理工具，基于纯 Rust (`egui`) 构建。

### 🌟 核心特性
- **语义分级与 15 条规范规则**：
  - 8 条可安全清除规则（npm / pip / uv / cargo / gradle / temp / 构建产物 / 缩略图缓存）
  - 7 条人工复核规则（Ollama/HuggingFace 大模型、Android SDK、闲置 node_modules、Downloads 陈旧大文件与历史安装包、AI 会话缓存）
  - **Docker & WSL 虚拟磁盘**：不执行误删，生成 `wsl --shutdown; docker system prune` 收缩维护指令
- **零直接误删架构**：
  - 软件本身不执行任何物理删除，统一生成经过审计的 PowerShell 脚本供人工核验执行
  - 各卷独立自建 `\.diskcleaner-trash\` 回收目录，提供 14 天安全后悔期
  - 单次处理超过 50GB 时插入交互式输入 `'YES'` 确认门
- **现代高颜值交互界面**：
  - 纯数学 Squarified Treemap，面积严格守恒，支持多层级无缝下钻与面包屑导航
  - 6 种柔和粉彩（Pastel）语义类目着色
  - 顶部 4 张大统计卡实时动态汇总（Free after cleanup 随右侧勾选项动态联动）
  - 右侧 Recommendations 分组面板，支持胶囊标签、路径缩写与详情展开
- **📥 Downloads 专项审阅视图**：
  - 专门排查 Downloads 目录中超过 180 天的大文件与超过 90 天的历史安装包，支持排序与批量操作
- **极致性能与底层加固**：
  - Rayon 16 线程并行扫描，33GB 用户目录实测仅需 30.8 秒
  - 本地快照 JSON 缓存，实现冷启动 0.8 秒秒开（常驻内存仅 45MB），后台重扫不阻塞浏览
  - 严格防御 Windows Junction 成环、突破 260 字符 MAX_PATH、防御 OneDrive 占位文件意外全量回迁
- **⚙ 个性化偏好设置**：
  - 自定义保留天数、安全阈值、直删白名单规则编辑，启动自动检查期满回收目录并支持一键清空

### 🧪 自动化测试
- 包含 41 项单元与真实文件系统集成测试，100% 通过（0 失败、0 警告）。
