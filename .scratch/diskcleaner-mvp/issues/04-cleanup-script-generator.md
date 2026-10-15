# 04: Cleanup Script 生成器

**What to build:** 从勾选结论生成 PowerShell 清理脚本：头部打印完整将删清单与总量 → 总量超过阈值（默认 50GB）时插入输入确认门（Threshold Confirmation）→ 默认动作是移入待删项所在卷的 Recovery Directory → Direct-Delete Whitelist 内项直接删除。UI：结论项勾选框、Copy script 按钮、勾选变化实时更新 Free after cleanup 统计卡。接缝①覆盖脚本文本断言（清单头、确认门、move/直删指令）。

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] 脚本头部打印完整清单与总量
- [ ] 超阈值场景生成确认门（合成 fixture 断言）
- [ ] 默认动作 = 同卷 move 进 Recovery Directory；白名单项直删
- [ ] UI 勾选 → Copy script 全链路可用，Free after cleanup 实时更新
- [ ] 脚本文本全部由接缝①测试断言
