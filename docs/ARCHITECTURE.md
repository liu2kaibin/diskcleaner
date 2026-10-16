# DiskCleaner 技术架构文档 (Architecture Guide)

本文档详细描述 DiskCleaner 的核心架构模型、模块边界、数据流转与测试接缝设计。

---

## 1. 架构总览 (Workspace Structure)

DiskCleaner 采用严格分层的 Cargo Workspace 结构，实现**纯领域内核**与**即时模式图形界面**的完全解耦：

```text
diskcleaner/
├── Cargo.toml                 # Workspace 根配置，限制 dev debug=0 适配低配机器
├── .cargo/config.toml         # 限制单任务编译线程，控制内存抖动
├── GLOSSARY.md                # 统一领域模型词汇表
├── docs/                      # 架构文档与 ADR 决策日志
│   ├── ARCHITECTURE.md        # 系统技术架构（本文档）
│   ├── CONTRIBUTING.md        # 开发者与代码规范贡献指南
│   └── adr/                   # 架构决策记录 (ADR 0001 - 0004)
├── crates/
│   ├── core/                  # 纯 Rust 领域内核（无任何 GUI 依赖）
│   │   ├── src/
│   │   │   ├── scanner.rs     # 多线程并行目录遍历器与快照树结构
│   │   │   ├── layout.rs      # Squarified Treemap 空间几何自绘算法
│   │   │   ├── rules.rs       # 15 条规范规则引擎与 Finding 抽象
│   │   │   ├── category.rs    # 语义类目特征启发式判定器
│   │   │   ├── script.rs      # 安全 PowerShell 清理脚本生成器
│   │   │   ├── settings.rs    # 用户配置持久化与期满回收扫描
│   │   │   └── store.rs       # 快照 JSON 持久化与秒开加载器
│   │   └── Cargo.toml
│   └── app/                   # eframe / egui 桌面客户端应用
│       ├── src/
│       │   └── main.rs        # UI 状态机、交互面板、线程调度与数据绑定
│       └── Cargo.toml
```

---

## 2. 核心数据流与状态机

### 2.1 扫描线程与 UI 交互循环 (Scanner Pipeline)

```text
[ 用户触发 / 启动 ]
       ↓
std::thread::spawn (后台工作线程)
       ↓
jwalk::WalkDirGeneric (16 线程 Rayon 并行池预读元数据)
       ↓  (AtomicU64 通报已扫描项，UI 保持 60fps 响应与进度展示)
Snapshot 树装配 (聚合各层级目录字节大小与 mtime)
       ↓  (mpsc::channel)
App::poll_scan (主线程非阻塞轮询)
       ↓
PathResolver (解析环境变量及系统路径)
       ↓
Rule Engine (匹配 15 条规则，输出带安全分级的 Finding 列表)
       ↓
store::save_snapshot (异步落盘，保障下次秒开)
```

### 2.2 视图驱动与数据绑定
- **Treemap View**：从当前下钻节点 `get_current_node` 出发，提取一级子项列表，调用 `layout::treemap_layout` 生成坐标映射，`egui::Painter` 批量绘制矩形与文字。
- **Recommendations View**：按 `Risk::SafeToRemove` 与 `Risk::NeedsReview` 拆分为两级胶囊段落，复选框状态直连 `App.selected_indices`。
- **Downloads View**：专门过滤 `downloads-old-files` 与 `downloads-installers` 规则命中，提供按容量/时间排序与批量勾选。
- **Top Bar Stats**：每次帧循环中，通过 `selected_indices` 聚合实时计算 `Free after cleanup: +XX GB`，所有视图共用单一事实来源。

---

## 3. 测试接缝设计 (Testing Seams)

遵循规范设计，全系统仅设 **2 个测试接缝**，确保高测试杠杆率与重构免疫：

### 接缝 ①（主接缝）：纯函数业务管道
- **输入**：合成 `Snapshot` 快照树、`PathResolver`（注入 `MockEnv`）、规则集切片、`ScriptConfig` 配置。
- **输出**：`Vec<Finding>` 分级结论列表、PowerShell 脚本文本。
- **特点**：
  - 纯内存运行，无真实磁盘 IO，毫秒级执行。
  - 15 条规则的全部命中/未命中边界、50GB 确认门阻断、直删白名单切换、回收目录路径构造，全部打在此接缝上。

### 接缝 ②（辅助接缝）：真实文件系统集成边界
- **测试面**：`scanner::scan` 针对 `tempfile::tempdir()` 临时目录夾具的真实遍历。
- **用例集**：
  1. **突破 MAX_PATH**：绝对路径超过 280 字符的深层嵌套目录遍历。
  2. **Junction 成环防护**：Windows `mklink /J` 指向父目录的硬链接环，验证 `follow_links(false)` 不发生死循环与大小重复统计。
  3. **OneDrive 云盘占位安全**：仅读取元数据属性位，绝不触发按需文件数据回迁下载。
  4. **权限不足跳过计数**：遭遇 IO 错误时不 Panic，准确累加 `skipped` 计数。

---

## 4. 关键架构决策索引 (ADR Index)

| 编号 | 标题 | 核心结论 |
| :--- | :--- | :--- |
| **ADR-0001** | `0001-script-only-deletion.md` | **零直接删除**：应用自身绝不执行任何物理删除，唯一出口是供人工过目执行的 PowerShell 脚本。 |
| **ADR-0002** | `0002-scan-scope-user-domain.md` | **扫描范围限定用户域**：系统区域（WinSxS/休眠文件等）仅在统计卡中展示 Not Scanned 总量，避免权限提升与误伤操作系统。 |
| **ADR-0003** | `0003-egui-pure-rust-gui.md` | **纯 Rust GUI 技术选型**：使用 egui / eframe 单可执行文件交付，支持 Treemap 纯算法高自由度自绘，免除前端 Node 工具链。 |
| **ADR-0004** | `0004-scan-perf-budget.md` | **扫描性能硬预算**：用户目录全扫必须在 60 秒内完成（普通权限），多线程并行预读 stat，后台线程与 UI 交互分离。 |
