use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::rules::{
    evaluate_rules, Category, EnvLookup, Finding, NpmCacheRule, PathResolver, Risk, Rule,
};
use crate::scanner::{Node, Snapshot};

/// 内存 Mock 环境变量
struct MockEnv {
    vars: HashMap<String, String>,
}

impl MockEnv {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    fn with_var(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.to_string(), v.to_string());
        self
    }
}

impl EnvLookup for MockEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}

/// 辅助构造合成 Snapshot
fn mock_snapshot(root_path: &Path, root_node: Node) -> Snapshot {
    Snapshot {
        root_path: root_path.to_path_buf(),
        root: root_node,
        duration: Duration::from_millis(50),
        skipped: 0,
    }
}

#[test]
fn resolver_prefers_npm_config_cache_env_var() {
    let env = MockEnv::new()
        .with_var("npm_config_cache", "D:\\custom\\npm-cache")
        .with_var("LOCALAPPDATA", "C:\\Users\\test\\AppData\\Local");
    let resolver = PathResolver::new(&env);

    assert_eq!(
        resolver.resolve_npm_cache(),
        PathBuf::from("D:\\custom\\npm-cache")
    );
}

#[test]
fn resolver_falls_back_to_localappdata_when_no_env_var() {
    let env = MockEnv::new()
        .with_var("LOCALAPPDATA", "C:\\Users\\test\\AppData\\Local")
        .with_var("USERPROFILE", "C:\\Users\\test");
    let resolver = PathResolver::new(&env);

    assert_eq!(
        resolver.resolve_npm_cache(),
        PathBuf::from("C:\\Users\\test\\AppData\\Local\\npm-cache")
    );
}

/// 接缝①主测试：给定合成快照与规则，产生正确的 Finding 分级清单。
/// 纯函数，不碰实际磁盘。
#[test]
fn evaluate_rules_detects_npm_cache_in_snapshot() {
    let custom_npm_dir = "D:\\dev-cache\\npm";
    let env = MockEnv::new().with_var("npm_config_cache", custom_npm_dir);
    let resolver = PathResolver::new(&env);

    // 构造包含自定义 npm 缓存目录的合成快照树
    let snap = mock_snapshot(
        Path::new("D:\\dev-cache"),
        Node::Dir {
            name: "dev-cache".to_string(),
            size: 1024 * 1024 * 250, // 250 MB
            children: vec![
                Node::Dir {
                    name: "npm".to_string(),
                    size: 1024 * 1024 * 150, // 150 MB
                    children: vec![Node::File {
                        name: "_cacache".to_string(),
                        size: 1024 * 1024 * 150,
                    }],
                },
                Node::Dir {
                    name: "other".to_string(),
                    size: 1024 * 1024 * 100,
                    children: vec![],
                },
            ],
        },
    );

    let rules: Vec<Box<dyn Rule>> = vec![Box::new(NpmCacheRule)];
    let findings = evaluate_rules(&snap, &resolver, &rules);

    assert_eq!(findings.len(), 1, "应准确命中一条 npm-cache 结论");
    let f = &findings[0];
    assert_eq!(f.rule_id, "npm-cache");
    assert_eq!(f.risk, Risk::SafeToRemove);
    assert_eq!(f.category, Category::PackageCache);
    assert_eq!(f.size, 1024 * 1024 * 150);
    assert_eq!(f.path, PathBuf::from(custom_npm_dir));
}

#[test]
fn evaluate_rules_returns_empty_when_path_not_in_snapshot() {
    let env = MockEnv::new().with_var("npm_config_cache", "D:\\dev-cache\\npm");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test"),
        Node::Dir {
            name: "test".to_string(),
            size: 1000,
            children: vec![Node::File {
                name: "file.txt".to_string(),
                size: 1000,
            }],
        },
    );

    let rules: Vec<Box<dyn Rule>> = vec![Box::new(NpmCacheRule)];
    let findings = evaluate_rules(&snap, &resolver, &rules);

    assert!(findings.is_empty(), "未命中缓存目录时应返回空列表");
}
