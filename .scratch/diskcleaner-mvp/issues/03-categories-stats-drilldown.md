# 03: 语义类目着色 + 统计卡 + treemap 钻取

**What to build:** Semantic Category 分类（目录特征启发式：代码项目/文档媒体/包缓存/AI 模型/SDK/其他）驱动 treemap 着色与底部图例；顶部四统计卡（Safe to Remove、Needs Review 来自规则结论；Not Scanned 显示扫描范围外总量；Free after cleanup 本票暂为占位，由 04 联动）；点击瓦片钻取子目录 + 面包屑导航。

**Blocked by:** 01, 02

**Status:** resolved

- [x] treemap 按语义类目着色，底部图例与参考截图结构一致（粉彩柔和配色：代码蓝紫、AI粉红、缓存薄荷绿、SDK青蓝、媒体桃橙、开发工具冷灰；底部图例完全对齐）
- [x] 四统计卡数字与 treemap / 规则结论一致（Free after cleanup / Safe to remove / Needs review / Not scanned 动态汇总）
- [x] 点击瓦片钻取，面包屑可逐级返回（点击目录瓦片下钻、顶部 ~ Home > dir 面包屑点击回退、右侧展示当前层级总大小）
- [x] 类目判定有合成 fixture 测试（category_tests 6/6 全绿，覆盖 AI/包缓存/SDK/文档/代码项目特征）

## Comments

- 实现了纯函数类目识别引擎 `crates/core/src/category.rs`，依据目录特征文件（Cargo.toml/package.json）与命名模式自动匹配。
- 引入 Windows 卷容量探测 `scanner::get_disk_free_space`（零额外依赖），精确计算 Not scanned（系统与应用）空间。
- 界面整体切换为浅色清新卡片风格（Visuals::light），四个白底圆角大统计卡横向排开，高度还原原型截图。
