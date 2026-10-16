//! 领域模型与主测试接缝①：规则引擎与路径解析。
//! 遵循 GLOSSARY 词汇：Rule, Safe to Remove, Needs Review, Semantic Category。

use std::path::PathBuf;

use crate::scanner::Snapshot;

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

/// 规则扫描产物（建议项）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule_id: String,
    pub title: String,
    pub path: PathBuf,
    pub size: u64,
    pub risk: Risk,
    pub category: Category,
    pub reason: String,
}

/// 环境配置查询接口（方便单元测试注入合成环境，避免污染真实环境）
pub trait EnvLookup: Send + Sync {
    fn get_var(&self, key: &str) -> Option<String>;
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
    env: &'a dyn EnvLookup,
}

impl<'a> PathResolver<'a> {
    pub fn new(env: &'a dyn EnvLookup) -> Self {
        Self { env }
    }

    /// 解析 npm 缓存路径：
    /// 1. npm_config_cache / NPM_CONFIG_CACHE 环境变量
    /// 2. %LOCALAPPDATA%\npm-cache
    /// 3. %USERPROFILE%\.npm
    pub fn resolve_npm_cache(&self) -> PathBuf {
        if let Some(val) = self.env.get_var("npm_config_cache") {
            return PathBuf::from(val);
        }
        if let Some(val) = self.env.get_var("NPM_CONFIG_CACHE") {
            return PathBuf::from(val);
        }
        if let Some(local_app_data) = self.env.get_var("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("npm-cache");
        }
        if let Some(user_profile) = self.env.get_var("USERPROFILE") {
            return PathBuf::from(user_profile).join(".npm");
        }
        PathBuf::from("C:\\Users\\Public\\npm-cache")
    }
}

pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding>;
}

pub struct NpmCacheRule;

impl Rule for NpmCacheRule {
    fn id(&self) -> &'static str {
        "npm-cache"
    }

    fn evaluate(&self, snapshot: &Snapshot, resolver: &PathResolver) -> Vec<Finding> {
        let path = resolver.resolve_npm_cache();
        if let Some(node) = snapshot.find_node(&path) {
            let size = node.size();
            if size > 0 {
                return vec![Finding {
                    rule_id: self.id().to_string(),
                    title: "npm cache".to_string(),
                    path,
                    size,
                    risk: Risk::SafeToRemove,
                    category: Category::PackageCache,
                    reason: "可再生的 npm 包下载缓存，删除后下次 npm install 会按需重新拉取".to_string(),
                }];
            }
        }
        Vec::new()
    }
}

/// 默认规则集全集提供函数
pub fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![Box::new(NpmCacheRule)]
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
