# 08: Scanner Windows 边界（接缝②）

**What to build:** 真实临时目录 fixture 的集成测试覆盖四类 Windows 边界：junction 环（不死循环、不重复计大小）、超长路径（超过 260 字符正常遍历）、OneDrive 占位文件（只读属性位，绝不触发按需下载）、无权限目录（跳过并计数）。UI 显示跳过计数。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] 四类边界各有真实文件系统集成用例
- [ ] junction 不重复计入也不死循环
- [ ] 占位文件仅读属性不触发下载
- [ ] 无权限目录跳过且计数显示在 UI
