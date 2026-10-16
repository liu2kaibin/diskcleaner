# 08: Scanner Windows 边界（接缝②）

**What to build:** 真实临时目录 fixture 的集成测试覆盖四类 Windows 边界：junction 环（不死循环、不重复计大小）、超长路径（超过 260 字符正常遍历）、OneDrive 占位文件（只读属性位，绝不触发按需下载）、无权限目录（跳过并计数）。UI 显示跳过计数。

**Blocked by:** 01

**Status:** ready-for-human

- [x] 四类边界各有真实文件系统集成用例（覆盖超长路径、Junction 环、OneDrive 占位属性只读、受限权限跳过计数）
- [x] junction 不重复计入也不死循环（mklink /J 环状链接测试通过，follow_links=false 确保不二次钻取）
- [x] 占位文件仅读属性不触发下载（仅在 Rayon 线程池中读取 WIN32 文件元数据，绝不调用 File::open 触发云端回迁）
- [x] 无权限目录跳过且计数显示在 UI（遇到 IO 错误自动累加 skipped，并在顶栏醒目标注“X 项跳过”）

## Comments

- 落实了接缝②（真实文件系统边界）：在 `crates/core/src/scanner_tests.rs` 中增加了 4 项 Windows 底层专项测试。
- 保证应用在复杂的真实开发机环境下（深度 node_modules、符号链接环、OneDrive 同步目录）运行不死循环、不崩溃、不误触发大文件按需下载。
