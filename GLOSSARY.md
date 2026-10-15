# DiskCleaner

开发者视角的 Windows 磁盘清理工具：扫描用户目录与已知缓存位置，用语义规则识别"可安全回收"与"需要人工判断"的空间，生成人工过目后执行的清理脚本。安全零误删是第一设计约束。

## Language

### 分类与推荐

**Rule**（规则）:
识别一类可清理的开发者资产（如 npm cache、构建产物）的模式+元数据，并给出 Safe to Remove / Needs Review 分级。
_Avoid_: 检测器、清理项

**Safe to Remove**（可安全移除）:
删除后可无损再生或确定无价值的资产类别（缓存、构建产物）。此类清理不需要用户逐项判断。
_Avoid_: 垃圾（junk）

**Needs Review**（需要复核）:
删除与否取决于用户判断的资产类别（AI 模型、SDK、闲置项目的 node_modules）。逐项由用户决定，工具只给依据。
_Avoid_: 危险项

**Not Scanned**（未扫描）:
Scan Scope 之外的磁盘区域。只显示总量，永不产生清理建议。
_Avoid_: 系统垃圾

**Semantic Category**（语义类目）:
按"资产对开发者的意义"划分的类目（AI 模型、包管理器缓存、构建产物、SDK……），驱动着色、推荐与统计卡。
_Avoid_: 文件类型

### 范围与执行

**Scan Scope**（扫描范围）:
用户目录 + 已知缓存位置。系统区域（WinSxS、Windows Update 缓存、休眠文件等）不在其中，只作 Not Scanned 总量展示。
_Avoid_: 全盘扫描

**Cleanup Script**（清理脚本）:
MVP 中唯一的删除机制：应用生成的 PowerShell 脚本，用户过目后自行执行。动作默认是移入 Recovery Directory；Direct-Delete Whitelist 内的可再生缓存除外。
_Avoid_: 一键清理（本产品永远不做）

**Recovery Directory**（回收目录）:
每个卷一个的自建回收文件夹（如 `C:\.diskcleaner-trash\`）。Cleanup Script 把待删项 move 进去，保留期满后再由用户清除。跨卷不 move，只进同卷目录。
_Avoid_: 回收站（Windows Recycle Bin）

**Direct-Delete Whitelist**（直删白名单）:
可再生成本极低的缓存类目，脚本直接删除、不经 Recovery Directory。
_Avoid_: 例外清单

**Threshold Confirmation**（阈值确认）:
单次脚本将删除总量超过设定上限（默认 50GB）时，脚本打印醒目警告要求二次确认。
