# 04: Cleanup Script 生成器

**What to build:** 从勾选结论生成 PowerShell 清理脚本：头部打印完整将删清单与总量 → 总量超过阈值（默认 50GB）时插入输入确认门（Threshold Confirmation）→ 默认动作是移入待删项所在卷的 Recovery Directory → Direct-Delete Whitelist 内项直接删除。UI：结论项勾选框、Copy script 按钮、勾选变化实时更新 Free after cleanup 统计卡。接缝①覆盖脚本文本断言（清单头、确认门、move/直删指令）。

**Blocked by:** 02

**Status:** ready-for-human

- [x] 脚本头部打印完整清单与总量（清单标明 [直删白名单] 或 [移入回收目录] 及字节大小）
- [x] 超阈值场景生成确认门（超过 50GB 阈值时插入 Read-Host "YES" 确认门，fixture 5/5 全绿）
- [x] 默认动作 = 同卷 move 进 Recovery Directory（.diskcleaner-trash）；白名单项直删（Remove-Item）
- [x] UI 勾选 → Copy script 全链路可用，Free after cleanup 实时更新，复制后展示提示横幅
- [x] 脚本文本全部由接缝①测试断言（script_tests 5 项针对清单头、阈值门、直删、移动的测试）

## Comments

- 实现了纯函数脚本生成器 `crates/core/src/script.rs`。
- 回收目录规则：自动提取待删路径卷根（如 `C:\`），同卷移动至 `\.diskcleaner-trash\`，保留后悔期。
- 阈值防手滑：单次删除超过 50GB 脚本强制交互输入 "YES" 才执行。
- UI 实现：右侧面板提供项勾选、`Select safe` / `清空已选` 工具按钮、总已选统计、一键复制到剪贴板，顶部栏实时计算 `Free after cleanup: +XX GB`。
