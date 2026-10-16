//! 领域模型与主测试接缝①：15 条全量规则引擎与路径解析。
//! 遵循 GLOSSARY 词汇：Rule, Safe to Remove, Needs Review, Semantic Category, Recovery Directory。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::scanner::{Node, Snapshot};

/// 空间安全分级（见 GLOSSARY）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    SafeToRemove,
    NeedsReview,
}

/// 语义类目（见 GLOSSARY）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    PackageCache,
    BuildArtifact,
    CodeProject,
    AiModel,
    Sdk,
    DocumentMedia,
    DevToolOther,
    Other,
}

/// 清理动作指令枚举（扩展支持自定义维护压缩命令）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 移动到卷回收目录
    MoveToTrash,
    /// 直接删除（白名单可再生缓存）
    DirectDelete,
    /// 压缩/维护命令（如 Docker prune / WSL shrink）
    CustomCommand {
        command: String,
        description: String,
    },
}

/// 规则扫描产物（建议项）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule_id: String,
    pub title: String,
    pub path: PathBuf,
    pub size: u64,
    pub risk: Risk,
    pub category: Category,
    pub action: Action,
    pub reason: String,
}

/// 环境配置查询接口（方便单元测试注入合成环境，避免污染真实环境）
pub trait EnvLookup: Send + Sync {
    fn get_var(&self, key: &str) -> Option<String>;
    /// 获取当前基准时间（秒数），默认读取系统时间
    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(1_800_000_000)
    }
}

/// 生产环境真实系统查询
pub struct SystemEnv;
impl EnvLookup for SystemEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// 工具缓存与资产路径解析器。
/// 拒绝硬编码 `~` 相对路径：优先环境变量与配置，最后回退系统已知默认位置。
pub struct PathResolver<'a> {
    pub env: &'a dyn EnvLookup,
}

impl<'a> PathResolver<'a> {
    pub fn new(env: &'a dyn EnvLookup) -> Self {
        Self { env }
    }

    pub fn resolve_npm_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("npm_config_cache") {
            return PathBuf::from(val);
        }
        if let Some(val) = self.env.get_var("NPM_CONFIG_CACHE") {
            return PathBuf::from(val);
        }
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local).join("npm-cache");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".npm");
        }
        PathBuf::from("C:\\Users\\Public\\npm-cache")
    }

    pub fn resolve_pip_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("PIP_CACHE_DIR") {
            return PathBuf::from(val);
        }
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local).join("pip").join("cache");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".cache").join("pip");
        }
        PathBuf::from("C:\\Users\\Public\\pip-cache")
    }

    pub fn resolve_uv_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("UV_CACHE_DIR") {
            return PathBuf::from(val);
        }
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local).join("uv");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".cache").join("uv");
        }
        PathBuf::from("C:\\Users\\Public\\uv-cache")
    }

    pub fn resolve_cargo_registry(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("CARGO_HOME") {
            return PathBuf::from(val).join("registry");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".cargo").join("registry");
        }
        PathBuf::from("C:\\Users\\Public\\.cargo\\registry")
    }

    pub fn resolve_gradle_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("GRADLE_USER_HOME") {
            return PathBuf::from(val).join("caches");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".gradle").join("caches");
        }
        PathBuf::from("C:\\Users\\Public\\.gradle\\caches")
    }

    pub fn resolve_maven_repo(&self) -> PathBuf {
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".m2").join("repository");
        }
        PathBuf::from("C:\\Users\\Public\\.m2\\repository")
    }

    pub fn resolve_user_temp(&self) -> PathBuf {
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local).join("Temp");
        }
        if let Some(val) = self.env.get_var("TEMP") {
            return PathBuf::from(val);
        }
        PathBuf::from("C:\\Windows\\Temp")
    }

    pub fn resolve_thumbnail_cache(&self) -> PathBuf {
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Explorer");
        }
        PathBuf::from("C:\\Users\\Public")
    }

    pub fn resolve_ollama_models(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("OLLAMA_MODELS") {
            return PathBuf::from(val);
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".ollama").join("models");
        }
        PathBuf::from("C:\\Users\\Public\\.ollama\\models")
    }

    pub fn resolve_hf_hub(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("HF_HOME") {
            return PathBuf::from(val).join("hub");
        }
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user)
                .join(".cache")
                .join("huggingface")
                .join("hub");
        }
        PathBuf::from("C:\\Users\\Public\\huggingface")
    }

    pub fn resolve_android_sdk(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("ANDROID_HOME") {
            return PathBuf::from(val);
        }
        if let Some(val) = self.env.get_var("ANDROID_SDK_ROOT") {
            return PathBuf::from(val);
        }
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local).join("Android").join("Sdk");
        }
        PathBuf::from("C:\\Users\\Public\\Android\\Sdk")
    }

    pub fn resolve_docker_vhdx(&self) -> PathBuf {
        if let Some(local) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local)
                .join("Docker")
                .join("wsl")
                .join("data")
                .join("ext4.vhdx");
        }
        PathBuf::from("C:\\Users\\Public\\ext4.vhdx")
    }

    pub fn resolve_downloads(&self) -> PathBuf {
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join("Downloads");
        }
        PathBuf::from("C:\\Users\\Public\\Downloads")
    }

    pub fn resolve_claude_projects(&self) -> PathBuf {
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".claude").join("projects");
        }
        PathBuf::from("C:\\Users\\Public\\.claude")
    }

    pub fn resolve_cursor(&self) -> PathBuf {
        if let Some(user) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user).join(".cursor");
        }
        PathBuf::from("C:\\Users\\Public\\.cursor")
    }
}

pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding>;
}

// -------------------------------------------------------------
// Safe to Remove 规则组 (8 条)
// -------------------------------------------------------------

pub struct NpmCacheRule;
impl Rule for NpmCacheRule {
    fn id(&self) -> &'static str { "npm-cache" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "npm cache",
            resolver.resolve_npm_cache(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "可再生的 npm 包下载缓存，删除后下次 npm install 会按需重新拉取",
        )
    }
}

pub struct PipCacheRule;
impl Rule for PipCacheRule {
    fn id(&self) -> &'static str { "pip-cache" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "pip cache",
            resolver.resolve_pip_cache(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "可再生的 Python pip 下载缓存",
        )
    }
}

pub struct UvCacheRule;
impl Rule for UvCacheRule {
    fn id(&self) -> &'static str { "uv-cache" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "uv cache",
            resolver.resolve_uv_cache(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "可再生的 Python uv 虚拟环境与包缓存",
        )
    }
}

pub struct CargoRegistryRule;
impl Rule for CargoRegistryRule {
    fn id(&self) -> &'static str { "cargo-registry" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "Cargo registry",
            resolver.resolve_cargo_registry(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "Rust Crates.io 源码与索引包缓存，cargo build 时可自动重新拉取",
        )
    }
}

pub struct GradleMavenCacheRule;
impl Rule for GradleMavenCacheRule {
    fn id(&self) -> &'static str { "gradle-maven-cache" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        out.extend(single_path_rule(
            snapshot,
            self.id(),
            "Gradle caches",
            resolver.resolve_gradle_cache(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "Java Gradle 依赖包缓存",
        ));
        out.extend(single_path_rule(
            snapshot,
            self.id(),
            "Maven repository",
            resolver.resolve_maven_repo(),
            Risk::SafeToRemove,
            Category::PackageCache,
            Action::DirectDelete,
            "Java Maven 本地仓库依赖包",
        ));
        out
    }
}

pub struct UserTempRule;
impl Rule for UserTempRule {
    fn id(&self) -> &'static str { "user-temp" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "用户 Temp 目录",
            resolver.resolve_user_temp(),
            Risk::SafeToRemove,
            Category::DevToolOther,
            Action::DirectDelete,
            "Windows 用户临时文件，占用中的文件会被系统自动跳过",
        )
    }
}

pub struct BuildArtifactsRule;
impl Rule for BuildArtifactsRule {
    fn id(&self) -> &'static str { "build-artifacts" }
    fn evaluate(&self, snapshot: &Snapshot, _resolver: &PathResolver) -> Vec<Finding> {
        let mut findings = Vec::new();
        scan_project_build_artifacts(&snapshot.root_path, &snapshot.root, &mut findings);
        findings
    }
}

fn scan_project_build_artifacts(curr_path: &Path, node: &Node, findings: &mut Vec<Finding>) {
    if let Node::Dir { name, children, .. } = node {
        // 判定是否是代码项目根目录
        let is_project = children.iter().any(|c| {
            let n = c.name();
            n == "Cargo.toml" || n == "package.json" || n == "go.mod" || n == "pom.xml" || n == "build.gradle"
        });

        if is_project {
            const ARTIFACT_NAMES: [&str; 6] = ["target", "build", "dist", ".next", ".turbo", "out"];
            for child in children {
                if ARTIFACT_NAMES.contains(&child.name()) && child.size() > 0 {
                    findings.push(Finding {
                        rule_id: "build-artifacts".to_string(),
                        title: format!("构建产物 ({})", child.name()),
                        path: curr_path.join(child.name()),
                        size: child.size(),
                        risk: Risk::SafeToRemove,
                        category: Category::BuildArtifact,
                        action: Action::DirectDelete,
                        reason: format!("代码项目 [{name}] 的构建中间产物，可通过重新编译恢复"),
                    });
                }
            }
        }

        for child in children {
            if child.size() > 0 {
                let next_path = curr_path.join(child.name());
                scan_project_build_artifacts(&next_path, child, findings);
            }
        }
    }
}

pub struct ThumbnailCacheRule;
impl Rule for ThumbnailCacheRule {
    fn id(&self) -> &'static str { "thumbnail-cache" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let path = resolver.resolve_thumbnail_cache();
        if let Some(Node::Dir { children, .. }) = snapshot.find_node(&path) {
            let thumb_size: u64 = children
                .iter()
                .filter(|c| c.name().starts_with("thumbcache_") && c.name().ends_with(".db"))
                .map(|c| c.size())
                .sum();
            if thumb_size > 0 {
                return vec![Finding {
                    rule_id: self.id().to_string(),
                    title: "缩略图缓存 (thumbcache)".to_string(),
                    path,
                    size: thumb_size,
                    risk: Risk::SafeToRemove,
                    category: Category::DevToolOther,
                    action: Action::DirectDelete,
                    reason: "Windows 资源管理器图标与缩略图缓存，浏览时会自动重建".to_string(),
                }];
            }
        }
        Vec::new()
    }
}

// -------------------------------------------------------------
// Needs Review 规则组 (7 条)
// -------------------------------------------------------------

pub struct AiModelsRule;
impl Rule for AiModelsRule {
    fn id(&self) -> &'static str { "ai-models" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        // 1. Ollama 模型
        let ollama_path = resolver.resolve_ollama_models();
        if let Some(Node::Dir { size, .. }) = snapshot.find_node(&ollama_path) {
            if *size > 0 {
                out.push(Finding {
                    rule_id: self.id().to_string(),
                    title: "Ollama 本地 AI 模型".to_string(),
                    path: ollama_path,
                    size: *size,
                    risk: Risk::NeedsReview,
                    category: Category::AiModel,
                    action: Action::MoveToTrash,
                    reason: "本地 Ollama 大模型权重文件，删除后需重新拉取 (如 ollama pull)".to_string(),
                });
            }
        }
        // 2. HuggingFace Hub 模型
        let hf_path = resolver.resolve_hf_hub();
        if let Some(Node::Dir { children, .. }) = snapshot.find_node(&hf_path) {
            for c in children {
                if c.name().starts_with("models--") && c.size() > 0 {
                    let model_name = c.name().strip_prefix("models--").unwrap_or(c.name()).replace("--", "/");
                    out.push(Finding {
                        rule_id: self.id().to_string(),
                        title: format!("HuggingFace 模型: {model_name}"),
                        path: hf_path.join(c.name()),
                        size: c.size(),
                        risk: Risk::NeedsReview,
                        category: Category::AiModel,
                        action: Action::MoveToTrash,
                        reason: "HuggingFace 本地模型快照文件，删除后可重新按需下载".to_string(),
                    });
                }
            }
        }
        out
    }
}

pub struct AndroidSdkRule;
impl Rule for AndroidSdkRule {
    fn id(&self) -> &'static str { "android-sdk" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        single_path_rule(
            snapshot,
            self.id(),
            "Android SDK & 工具包",
            resolver.resolve_android_sdk(),
            Risk::NeedsReview,
            Category::Sdk,
            Action::MoveToTrash,
            "Android SDK 与仿真器镜像，重装耗时较长，请根据当前开发需要确认",
        )
    }
}

pub struct IdleNodeModulesRule;
impl Rule for IdleNodeModulesRule {
    fn id(&self) -> &'static str { "idle-node-modules" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut findings = Vec::new();
        let now = resolver.env.now_secs();
        scan_idle_node_modules(&snapshot.root_path, &snapshot.root, now, &mut findings);
        findings
    }
}

fn scan_idle_node_modules(curr_path: &Path, node: &Node, now: u64, findings: &mut Vec<Finding>) {
    if let Node::Dir { name, children, mtime, .. } = node {
        let has_package_json = children.iter().any(|c| c.name() == "package.json");
        if has_package_json {
            // 闲置判定：90 天 (90 * 86400 = 7,776,000s)
            let is_idle = now.saturating_sub(*mtime) >= 90 * 86400;
            if is_idle {
                if let Some(nm) = children.iter().find(|c| c.name() == "node_modules") {
                    if nm.size() > 0 {
                        findings.push(Finding {
                            rule_id: "idle-node-modules".to_string(),
                            title: format!("闲置项目的 node_modules ({name})"),
                            path: curr_path.join("node_modules"),
                            size: nm.size(),
                            risk: Risk::NeedsReview,
                            category: Category::PackageCache,
                            action: Action::DirectDelete,
                            reason: format!("项目 [{name}] 已超过 90 天未修改，需要时可通过 npm/pnpm install 随时恢复"),
                        });
                    }
                }
            }
        }

        for child in children {
            if child.size() > 0 {
                let next_path = curr_path.join(child.name());
                scan_idle_node_modules(&next_path, child, now, findings);
            }
        }
    }
}

pub struct DockerWslVhdxRule;
impl Rule for DockerWslVhdxRule {
    fn id(&self) -> &'static str { "docker-wsl-vhdx" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        let docker_vhdx = resolver.resolve_docker_vhdx();
        if let Some(node) = snapshot.find_node(&docker_vhdx) {
            if node.size() > 0 {
                out.push(Finding {
                    rule_id: self.id().to_string(),
                    title: "Docker 虚拟磁盘 (ext4.vhdx)".to_string(),
                    path: docker_vhdx,
                    size: node.size(),
                    risk: Risk::NeedsReview,
                    category: Category::DevToolOther,
                    action: Action::CustomCommand {
                        command: "wsl --shutdown; docker system prune -a --volumes -f".to_string(),
                        description: "清理 Docker 未使用镜像容器与卷并收缩磁盘".to_string(),
                    },
                    reason: "Docker 虚拟机磁盘空间黑洞，不可直接删除！建议执行清理与收缩命令释放空间".to_string(),
                });
            }
        }
        out
    }
}

pub struct DownloadsOldFilesRule;
impl Rule for DownloadsOldFilesRule {
    fn id(&self) -> &'static str { "downloads-old-files" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        let dl_path = resolver.resolve_downloads();
        let now = resolver.env.now_secs();
        if let Some(Node::Dir { children, .. }) = snapshot.find_node(&dl_path) {
            for c in children {
                // >180 天 (180 * 86400 = 15,552,000s) 且 >50MB
                if !matches!(c, Node::Dir { .. })
                    && c.size() >= 50 * 1024 * 1024
                    && now.saturating_sub(c.mtime()) >= 180 * 86400
                {
                    out.push(Finding {
                        rule_id: self.id().to_string(),
                        title: format!("陈旧大文件: {}", c.name()),
                        path: dl_path.join(c.name()),
                        size: c.size(),
                        risk: Risk::NeedsReview,
                        category: Category::DocumentMedia,
                        action: Action::MoveToTrash,
                        reason: "下载目录中超过 180 天未变动的大文件 (>50MB)".to_string(),
                    });
                }
            }
        }
        out
    }
}

pub struct DownloadsInstallersRule;
impl Rule for DownloadsInstallersRule {
    fn id(&self) -> &'static str { "downloads-installers" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        let dl_path = resolver.resolve_downloads();
        let now = resolver.env.now_secs();
        if let Some(Node::Dir { children, .. }) = snapshot.find_node(&dl_path) {
            for c in children {
                let lower = c.name().to_lowercase();
                if (lower.ends_with(".exe") || lower.ends_with(".msi"))
                    && now.saturating_sub(c.mtime()) >= 90 * 86400
                    && c.size() > 0
                {
                    out.push(Finding {
                        rule_id: self.id().to_string(),
                        title: format!("历史安装包: {}", c.name()),
                        path: dl_path.join(c.name()),
                        size: c.size(),
                        risk: Risk::NeedsReview,
                        category: Category::DevToolOther,
                        action: Action::MoveToTrash,
                        reason: "下载目录中超过 90 天的旧软件安装包".to_string(),
                    });
                }
            }
        }
        out
    }
}

pub struct AiToolSessionsRule;
impl Rule for AiToolSessionsRule {
    fn id(&self) -> &'static str { "ai-tool-sessions" }
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let mut out = Vec::new();
        out.extend(single_path_rule(
            snapshot,
            self.id(),
            "Claude Code 本地历史工程数据",
            resolver.resolve_claude_projects(),
            Risk::NeedsReview,
            Category::DevToolOther,
            Action::MoveToTrash,
            "Claude Code 历史工作区的本地缓存和对话上下文",
        ));
        out.extend(single_path_rule(
            snapshot,
            self.id(),
            "Cursor 编辑器本地数据",
            resolver.resolve_cursor(),
            Risk::NeedsReview,
            Category::DevToolOther,
            Action::MoveToTrash,
            "Cursor 编辑器的本地历史索引和临时状态",
        ));
        out
    }
}

/// 辅助提取单一目录命中
fn single_path_rule(
    snapshot: &Snapshot,
    rule_id: &'static str,
    title: &str,
    path: PathBuf,
    risk: Risk,
    category: Category,
    action: Action,
    reason: &str,
) -> Vec<Finding> {
    if let Some(node) = snapshot.find_node(&path) {
        let size = node.size();
        if size > 0 {
            return vec![Finding {
                rule_id: rule_id.to_string(),
                title: title.to_string(),
                path,
                size,
                risk,
                category,
                action,
                reason: reason.to_string(),
            }];
        }
    }
    Vec::new()
}

/// 默认规则集全集（全部 15 条规范规则）
pub fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // Safe to remove (8 条)
        Box::new(NpmCacheRule),
        Box::new(PipCacheRule),
        Box::new(UvCacheRule),
        Box::new(CargoRegistryRule),
        Box::new(GradleMavenCacheRule),
        Box::new(UserTempRule),
        Box::new(BuildArtifactsRule),
        Box::new(ThumbnailCacheRule),
        // Needs review (7 条)
        Box::new(AiModelsRule),
        Box::new(AndroidSdkRule),
        Box::new(IdleNodeModulesRule),
        Box::new(DockerWslVhdxRule),
        Box::new(DownloadsOldFilesRule),
        Box::new(DownloadsInstallersRule),
        Box::new(AiToolSessionsRule),
    ]
}

/// 主测试接缝①：输入快照与设置，产出规则分级结论清单。
/// 纯函数，无磁盘 IO。
pub fn evaluate_rules(
    snapshot: &Snapshot,
    resolver: &PathResolver,
    rules: &[Box<dyn Rule>],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for rule in rules {
        findings.extend(rule.evaluate(snapshot, resolver));
    }
    findings
}
