# 01: 骨架——扫描器 + egui treemap

**What to build:** 打开应用即看到真实用户目录的 treemap。建 cargo workspace：核心 crate（扫描/规则/脚本，纯逻辑无 UI 依赖）+ egui app crate。Scanner 多线程并行遍历用户目录产出快照树；扫描在独立线程、UI 显示进度不阻塞；「重新扫描」按钮。treemap 用 squarified 算法自绘，本票先单一占位色。

**Blocked by:** None (can start immediately)

**Status:** ready-for-human

- [x] 启动应用触发用户目录扫描，60 秒内完成（普通权限，不用 MFT）——实测 30.8s / 33.2GB
- [x] 扫描期间 UI 可交互并显示进度，不冻结（独立线程 + 进度计数 + 人工目验）
- [x] treemap 按目录大小渲染面积，层级结构可辨（人工目验）
- [x] 「重新扫描」按钮可再次触发全量扫描
- [x] 核心扫描逻辑可在无 GUI 环境独立调用（核心 crate 不依赖 app crate）

## Comments

- TDD：7 测试全绿（scanner 平坦/嵌套 + layout 5 属性测试）。
- 性能：首版单线程 stat 77.6s 超预算 → metadata 挪入 jwalk process_read_dir 并行池 → 30.8s（守护测试 scan_user_dir_within_60s_budget 常驻，#[ignore] 手动跑）。
- 本机构建约束（重要）：8GB 内存 + Defender → Cargo.toml `profile.dev.debug=0` + `.cargo/config.toml` `jobs=1` + Defender 排除 target/（用户已加）；首次全量构建 ~62min，后续增量秒级。
- eframe 0.37 API：App trait 是 `fn ui(&mut self, ui, frame)`（非 update）；面板统一为 `egui::Panel::top(...)`/`CentralPanel::default().show(ui, ...)`；中文需加载 simhei.ttf 否则方块。
