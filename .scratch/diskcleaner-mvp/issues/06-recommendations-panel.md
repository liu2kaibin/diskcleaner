# 06: 推荐面板完整版

**What to build:** 右侧 Recommendations 面板完整交互：按语义类目分组列出全部规则命中、每项显示大小与依据行、Select safe 一键勾选全部 Safe to Remove 项、Needs Review 项逐项勾选、勾选状态贯穿统计卡与 Cleanup Script 生成——达成参考截图的交互原貌。

**Blocked by:** 04, 05

**Status:** resolved

- [x] 面板按类目分组显示全部规则命中（分为 Safe to remove 与 Review first 两大独立分组，带胶囊徽标与组容量统计）
- [x] 每项显示大小与依据（对齐截图格式：[✓] 标题 + 缩略路径 ~/.xxx | 容量 + 右箭头展开详情）
- [x] Select safe 一键勾选全部 Safe 项（清空并全部选中 Safe to remove 规则项）
- [x] 勾选状态贯穿脚本生成与统计卡（实时更新已选总量、顶部 Free after cleanup 增量角标、Copy script 仅包含勾选项）
- [x] 交互结构与参考截图一致（底部已选统计、白色 [Select safe] 边框按钮 + 黑色加粗 [Copy script] 主按钮）

## Comments

- 实现了推荐面板的完整分组与对齐：`Safe to remove` 绿色胶囊徽标与 `Review first` 琥珀色胶囊徽标。
- 单项右侧提供折叠箭头 `>` / `v`，点击可下拉展示清理原因与具体清理动作指令。
- 路径显示自动缩写用户主目录为 `~`，视觉纯净轻量。
- 达成 M2 里程碑（规则全量 + 推荐面板完整版）。
