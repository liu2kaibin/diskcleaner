# 03: 语义类目着色 + 统计卡 + treemap 钻取

**What to build:** Semantic Category 分类（目录特征启发式：代码项目/文档媒体/包缓存/AI 模型/SDK/其他）驱动 treemap 着色与底部图例；顶部四统计卡（Safe to Remove、Needs Review 来自规则结论；Not Scanned 显示扫描范围外总量；Free after cleanup 本票暂为占位，由 04 联动）；点击瓦片钻取子目录 + 面包屑导航。

**Blocked by:** 01, 02

**Status:** ready-for-agent

- [ ] treemap 按语义类目着色，底部图例与参考截图结构一致
- [ ] 四统计卡数字与 treemap / 规则结论一致
- [ ] 点击瓦片钻取，面包屑可逐级返回
- [ ] 类目判定有合成 fixture 测试
