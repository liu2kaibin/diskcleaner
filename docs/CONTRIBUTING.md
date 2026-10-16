# DiskCleaner 贡献与开发指南 (Contributing Guide)

感谢你关注 DiskCleaner！本项目采用 **测试驱动开发 (TDD)** 和 **接缝驱动设计**。

---

## 1. 本地开发环境设置

- **操作系统**：Windows 10 / 11 x64
- **Rust 版本**：1.80+ (Stable MSVC)
- **C++ 工具链**：Visual Studio Build Tools 或带有 MSVC link.exe 的环境

### 8GB 内存设备特别提示
在资源受限设备上编译大型 Rust GUI crate（如 eframe/wgpu/vello 依赖栈）时，建议：
1. 保持 `Cargo.toml` 中 `[profile.dev] debug = 0` 配置，大幅降低内存占用。
2. 保持 `.cargo/config.toml` 中 `[build] jobs = 1` 限制，防止多 rustc 并发换页。
3. 将项目根目录下的 `target/` 目录添加至 Windows Defender 排除项，避免反病毒引擎逐文件扫描放慢 I/O。

---

## 2. 如何添加一条新的清理规则 (Adding a New Rule)

DiskCleaner 的规则引擎设计为高度模块化和可测试的。添加一条新规则只需 4 步：

### 第一步：在 `crates/core/src/rules.rs` 中为目标资产解析路径
不要硬编码类似 `C:\Users\xxx\.cache` 的固定路径，必须使用 `PathResolver` 统一解析环境变量并提供回退：

```rust
impl<'a> PathResolver<'a> {
    pub fn resolve_my_tool_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("MY_TOOL_CACHE") {
            return PathBuf::from(val);
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".my_tool").join("cache");
        }
        PathBuf::from("C:\\Users\\Public\\my_tool")
    }
}
```

### 第二步：定义规则结构体并实现 `Rule` trait
```rust
pub struct MyToolCacheRule;

impl Rule for MyToolCacheRule {
    fn id(&self) -> &'static str {
        "my-tool-cache"
    }

    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let path = resolver.resolve_my_tool_cache();
        if let Some(node) = snapshot.find_node(&path) {
            if node.size() > 0 {
                return vec![Finding {
                    rule_id: self.id().to_string(),
                    title: "MyTool 缓存".to_string(),
                    path,
                    size: node.size(),
                    risk: Risk::SafeToRemove,           // 或 Risk::NeedsReview
                    category: Category::PackageCache,    // 对应的语义类目
                    action: Action::DirectDelete,        // 或 Action::MoveToTrash
                    reason: "可再生的工具下载缓存".to_string(),
                }];
            }
        }
        Vec::new()
    }
}
```

### 第三步：在 `crates/core/src/rules_tests.rs` 中添加纯内存测试
在主测试接缝①上，使用合成快照树 `Node::dir` / `Node::file` 验证规则判定（毫秒级执行，不碰磁盘）：

```rust
#[test]
fn evaluate_my_tool_cache_hit() {
    let env = MockEnv::new().with_var("MY_TOOL_CACHE", "D:\\custom\\cache");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("D:\\custom"),
        Node::dir("custom", 500, vec![Node::file("cache", 500)]),
    );

    let findings = evaluate_rules(&snap, &resolver, &[Box::new(MyToolCacheRule)]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].size, 500);
}
```

### 第四步：注册到 `default_rules()`
在 `crates/core/src/rules.rs` 的 `default_rules()` 函数中将该规则加入清单，前端和脚本生成器将自动获得完整支持！

---

## 3. 代码审查与提交前检查

在提交 Pull Request 之前，请确保以下命令在本地通过：

```bash
# 1. 运行所有单元与集成测试（必须全绿）
cargo test

# 2. 检查代码格式
cargo fmt --check

# 3. 运行代码静态检查
cargo clippy --workspace --all-targets -- -D warnings
```
