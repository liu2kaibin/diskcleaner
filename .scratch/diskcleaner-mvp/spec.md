# DiskCleaner MVP 规格

Status: ready-for-agent

## Problem Statement

作为 Windows 开发者，开发机磁盘被占满时不知道什么能删。现有工具失效于两个方向：WizTree/TreeSize 只给尺寸不解释语义——17GB 的 Android SDK 和 8GB 的 AI 模型在它们眼里只是两块面积；CCleaner 类"一键清理"面向大众软件垃圾，不懂开发资产且误删风险高。手动 `du` 加手删每次都心惊胆战：删掉模型要重新下载几十 GB，删错项目文件不可挽回。结果是磁盘长期紧张、重复排查劳动、以及"不敢删"。

## Solution

一个 Rust + egui 的桌面应用：扫描用户域（Scan Scope），用语义规则（Rule）把空间分成 Safe to Remove / Needs Review / Not Scanned 三级；顶部统计卡给出总量预期（含 Free after cleanup）；右侧推荐面板逐项勾选，Needs Review 项附判断依据。出口是人工过目后自行执行的 PowerShell 清理脚本（Cleanup Script）——默认动作是移入所在卷的 Recovery Directory（14 天后悔期），Direct-Delete Whitelist 内的可再生缓存直删，单次总量超阈值强制二次确认（Threshold Confirmation）。零误删是第一设计约束。

## User Stories

1. 作为 Windows 开发者，我想一键扫描用户目录与已知缓存位置，所以不用手动到处排查空间去哪了
2. 作为 Windows 开发者，我想让用户目录全扫在 60 秒内完成，所以扫描是"喝口水的等待"而不是去泡咖啡
3. 作为 Windows 开发者，我想扫描期间界面保持流畅并显示进度，所以我能随时看到扫到了哪
4. 作为 Windows 开发者，我想用 treemap 按面积浏览目录占用，所以一眼看出空间大头
5. 作为 Windows 开发者，我想 treemap 按语义类目（Semantic Category）着色（AI 模型/包缓存/构建产物/文档媒体…），所以不用逐层点进去就知道每块是什么
6. 作为 Windows 开发者，我想点击 treemap 瓦片钻取子目录，所以能从全盘逐级下钻到具体文件夹
7. 作为 Windows 开发者，我想顶部统计卡同时显示 Free after cleanup / Safe to Remove / Needs Review / Not Scanned，所以先看总量预期再决定细看哪块
8. 作为 Windows 开发者，我想推荐面板按类目分组列出每条规则命中项的大小与路径依据，所以每条建议可核实
9. 作为 Windows 开发者，我想 Needs Review 项附带判断依据（最后使用时间、所属项目、模型用途），所以我不需要自己再去查
10. 作为 Windows 开发者，我想"Select safe"一键勾选全部 Safe to Remove 项，所以常规清理三步完成（勾→复制→执行）
11. 作为 Windows 开发者，我想勾选变化时 Free after cleanup 实时更新，所以执行前确切知道能换回多少空间
12. 作为 Windows 开发者，我想点 Copy script 得到完整 PowerShell 清理脚本，所以删除动作完全在我自己手里
13. 作为 Windows 开发者，我想脚本头部打印将删项完整清单与总量，所以执行前最后一眼可审计
14. 作为 Windows 开发者，我想脚本默认把待删项移入所在卷的 Recovery Directory 而非直接删除，所以我有 14 天后悔期且同卷移动瞬时完成
15. 作为 Windows 开发者，我想保留期满后应用提示我清空 Recovery Directory，所以回收目录不会变成新的垃圾场
16. 作为 Windows 开发者，我想直删白名单内的可再生缓存跳过 Recovery Directory 直接删除，所以最常见的清理不产生二次搬运
17. 作为 Windows 开发者，我想单次脚本删除总量超过 50GB 时被要求输入确认才能继续，所以手滑执行不了超大删除
18. 作为 Windows 开发者，我想 AI 模型按单个模型列出名称/大小/最后使用时间，所以留 deepseek-coder 删 llama3 这种决定可以逐模型做
19. 作为 Windows 开发者，我想 Docker/WSL 虚拟磁盘被识别为"压缩"对象并生成 prune/shrink 命令而非删除，所以最大的空间黑洞能安全处理
20. 作为 Windows 开发者，我想 90 天无变动的项目里的 node_modules 被逐项标出，所以闲置项目占用一目了然且不会碰到活跃项目
21. 作为 Windows 开发者，我想 Downloads 里超过 180 天未动的文件和超过 90 天的安装包有专门列表，所以 13GB 的下载目录不再失控
22. 作为 Windows 开发者，我想启动时秒开上次扫描快照并看到"扫描于 X 小时前"，所以不用每次开门等扫描
23. 作为 Windows 开发者，我想随时手动触发全量重扫且期间可继续查看旧数据，所以刷新不阻塞使用
24. 作为 Windows 开发者，我想界面文案是中文而术语与代码是英文，所以读得快且和文档/代码对得上
25. 作为 Windows 开发者，我想无权限目录被优雅跳过并显示跳过计数，所以扫描永不因权限中断也不静默丢数据
26. 作为 Windows 开发者，我想在设置页调整保留天数/阈值/白名单/闲置天数，所以安全参数由我掌控

## Implementation Decisions

- **模块划分**（cargo workspace：核心 crate 纯逻辑无 UI 依赖 + egui app crate）：
  - **Scanner**：多线程并行目录遍历器，普通用户权限，产出快照。长路径（`\\?\` 前缀）、junction/symlink 防环、OneDrive 占位文件（只读属性位，绝不触发按需下载）、无权限目录跳过并计数。
  - **Path Resolver**：规则目标路径一律经环境变量与工具配置解析（`OLLAMA_MODELS`、`CARGO_HOME`、`GRADLE_USER_HOME`、`PIP_CACHE_DIR`、`npm config get cache` 等），禁止硬编码用户目录下的默认相对路径——本机侦察证实多数缓存不在默认位置。
  - **Rule Engine**：声明式规则表（15 条初始规则：8 Safe + 7 Review），输入快照、输出带分级的命中结论。每条结论包含类目、大小、依据（mtime/环境变量来源等）。
  - **Script Generator**：从勾选结论生成 PowerShell 脚本：头部完整清单+总量 → 阈值确认门 → move 到所在卷 Recovery Directory / 白名单直删 / Docker 与 WSL 生成 prune+shrink 命令。动作类型是开放枚举（删除/移动/压缩命令），Docker 已证明需要第三种。
  - **Snapshot Store**：快照 JSON 持久化（路径树、size、mtime、属性标记、规则命中与分级结论）；启动加载最新快照、标注扫描时间；手动触发全量重扫，期间旧数据可继续浏览。
  - **App（egui）**：treemap 自绘（squarified，按类目着色，点击钻取+面包屑）、右侧推荐面板、顶部统计卡、设置页。界面中文，术语英文（与 GLOSSARY 一致）。
- **安全模型**（ADR-0001）：应用零删除；每卷 Recovery Directory（`\ .diskcleaner-trash\`）+ 14 天保留 + 50GB Threshold Confirmation；Direct-Delete Whitelist 仅限可再生成本极低的包管理器缓存。
- **扫描范围**（ADR-0002）：用户目录+解析出的缓存位置；系统区只报 Not Scanned 总量。
- **技术栈**（ADR-0003）：纯 Rust，GUI 用 egui，单 exe 交付。
- **性能预算**（ADR-0004）：用户目录全扫 <60s，普通权限（不用 MFT），扫描独立线程，UI 非阻塞。

## Testing Decisions

- 好测试只测外部行为：给定快照与设置，断言规则结论（分级/类目/依据）与生成脚本文本（清单头、确认门、move/直删/压缩指令）；绝不测内部数据结构。
- **接缝①（主接缝）**：「快照+设置 → (结论清单, 脚本全文)」纯函数管道。规则引擎与脚本生成的全部测试打在这里，用合成文件树 fixture，不碰真磁盘与 UI。
- **接缝②（唯一辅助接缝）**：真实 Scanner 对临时目录 fixture 的 Windows 边界用例——junction 环、超长路径、OneDrive 占位文件、无权限目录，各一例（M4 的边界处理落点）。
- egui UI 层无自动化测试，按里程碑人工验证。
- 新仓库无先例：M1 建立的 fixture 即后续测试的先例模式。

## Out of Scope

- 应用内删除、一键清理（永久排除"一键"，二期引入应用内删除也必须走 Recovery Directory 模型）
- 重复文件查找、跨平台（macOS/Linux 规则集）
- 系统区清理（WinSxS、SoftwareDistribution、休眠文件等，需管理员权限，永不进入自动建议）
- 自动/增量/定时扫描
- MFT 直读与管理员权限提升
- 安装器、代码签名、自动更新（个人工具）

## Further Notes

- **视觉原型**：用户提供的截图（macOS 应用）为布局蓝本——左 treemap + 右推荐面板 + 顶部四统计卡 + 底部类目图例；平台内容全部替换为 Windows 语义。
- **本机侦察事实**（`.scratch/recon-cache-sizes.txt`）：Downloads 13GB 是最大真实目标；多数开发缓存不在 `~` 默认位置（npm/pip/gradle/cargo 未检出）——Path Resolver 的环境变量解析由此从" nice-to-have "升级为硬要求。
- 领域术语以根 `GLOSSARY.md` 为准（9 术语）；架构决策见 `docs/adr/0001`–`0004`。
