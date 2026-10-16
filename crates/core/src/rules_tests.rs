use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::rules::{
    default_rules, evaluate_rules, Action, Category, EnvLookup, Finding, PathResolver, Risk,
    Rule,
};
use crate::scanner::{Node, Snapshot};

/// 内存 Mock 环境变量
struct MockEnv {
    vars: HashMap<String, String>,
    now: Option<u64>,
}

impl MockEnv {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            now: None,
        }
    }

    fn with_var(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.to_string(), v.to_string());
        self
    }

    fn with_now(mut self, now: u64) -> Self {
        self.now = Some(now);
        self
    }
}

impl EnvLookup for MockEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }

    fn now_secs(&self) -> u64 {
        self.now.unwrap_or(1_800_000_000)
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

#[test]
fn test_default_rules_contains_15_rules() {
    let rules = default_rules();
    assert_eq!(rules.len(), 15, "默认规则集必须包含全部 15 条规范规则");
}

#[test]
fn evaluate_pip_and_uv_cache() {
    let env = MockEnv::new()
        .with_var("LOCALAPPDATA", "C:\\Users\\test\\AppData\\Local")
        .with_var("USERPROFILE", "C:\\Users\\test");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test\\AppData\\Local"),
        Node::dir(
            "Local",
            100_000,
            vec![
                Node::dir("pip", 40_000, vec![Node::dir("cache", 40_000, vec![Node::file("whl", 40_000)])]),
                Node::dir("uv", 60_000, vec![Node::file("archive", 60_000)]),
            ],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    let pip = findings.iter().find(|f| f.rule_id == "pip-cache").expect("命中 pip");
    assert_eq!(pip.risk, Risk::SafeToRemove);
    assert_eq!(pip.action, Action::DirectDelete);
    assert_eq!(pip.size, 40_000);

    let uv = findings.iter().find(|f| f.rule_id == "uv-cache").expect("命中 uv");
    assert_eq!(uv.risk, Risk::SafeToRemove);
    assert_eq!(uv.action, Action::DirectDelete);
    assert_eq!(uv.size, 60_000);
}

#[test]
fn evaluate_cargo_and_gradle_maven() {
    let env = MockEnv::new()
        .with_var("USERPROFILE", "C:\\Users\\test");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test"),
        Node::dir(
            "test",
            300_000,
            vec![
                Node::dir(".cargo", 100_000, vec![Node::dir("registry", 100_000, vec![Node::file("src", 100_000)])]),
                Node::dir(".gradle", 100_000, vec![Node::dir("caches", 100_000, vec![Node::file("modules", 100_000)])]),
                Node::dir(".m2", 100_000, vec![Node::dir("repository", 100_000, vec![Node::file("jars", 100_000)])]),
            ],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    assert!(findings.iter().any(|f| f.rule_id == "cargo-registry"));
    assert!(findings.iter().any(|f| f.rule_id == "gradle-maven-cache"));
}

#[test]
fn evaluate_build_artifacts_only_in_projects() {
    let env = MockEnv::new().with_var("USERPROFILE", "C:\\Users\\test");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test"),
        Node::dir(
            "test",
            500_000,
            vec![
                // 项目目录：有 Cargo.toml
                Node::dir(
                    "my_rust_proj",
                    300_000,
                    vec![
                        Node::file("Cargo.toml", 500),
                        Node::dir("target", 290_000, vec![Node::file("debug.exe", 290_000)]),
                    ],
                ),
                // 普通目录：无项目标记，名为 target 的普通文件夹不应被误判为构建产物
                Node::dir(
                    "random_folder",
                    100_000,
                    vec![Node::dir("target", 100_000, vec![Node::file("doc.txt", 100_000)])],
                ),
            ],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    let build_hits: Vec<&Finding> = findings.iter().filter(|f| f.rule_id == "build-artifacts").collect();
    assert_eq!(build_hits.len(), 1, "只应匹配真实项目内部的 target 目录");
    assert!(build_hits[0].path.to_string_lossy().contains("my_rust_proj"));
    assert_eq!(build_hits[0].size, 290_000);
}

#[test]
fn evaluate_docker_wsl_generates_custom_command() {
    let env = MockEnv::new().with_var("LOCALAPPDATA", "C:\\Users\\test\\AppData\\Local");
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test\\AppData\\Local"),
        Node::dir(
            "Local",
            10_000_000_000,
            vec![
                Node::dir(
                    "Docker",
                    10_000_000_000,
                    vec![Node::dir("wsl", 10_000_000_000, vec![Node::dir("data", 10_000_000_000, vec![Node::file("ext4.vhdx", 10_000_000_000)])])],
                ),
            ],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    let docker_hit = findings.iter().find(|f| f.rule_id == "docker-wsl-vhdx").expect("应匹配 Docker 虚拟磁盘");
    assert_eq!(docker_hit.risk, Risk::NeedsReview);
    match &docker_hit.action {
        Action::CustomCommand { command, .. } => {
            assert!(command.contains("docker system prune") || command.contains("wsl"));
        }
        _ => panic!("Docker vhdx 动作必须是 CustomCommand，绝不可为直接删除"),
    }
}

#[test]
fn evaluate_idle_node_modules_only_when_older_than_90_days() {
    let now_secs = 1_800_000_000u64;
    let old_mtime = now_secs - 100 * 86400; // 100 天前（闲置）
    let recent_mtime = now_secs - 10 * 86400; // 10 天前（活跃）

    let env = MockEnv::new()
        .with_var("USERPROFILE", "C:\\Users\\test")
        .with_now(now_secs);
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test"),
        Node::dir(
            "test",
            200_000,
            vec![
                // 闲置项目
                Node::dir_with_mtime(
                    "old_project",
                    100_000,
                    old_mtime,
                    vec![
                        Node::file_with_mtime("package.json", 500, old_mtime),
                        Node::dir_with_mtime("node_modules", 90_000, old_mtime, vec![]),
                    ],
                ),
                // 活跃项目
                Node::dir_with_mtime(
                    "active_project",
                    100_000,
                    recent_mtime,
                    vec![
                        Node::file_with_mtime("package.json", 500, recent_mtime),
                        Node::dir_with_mtime("node_modules", 90_000, recent_mtime, vec![]),
                    ],
                ),
            ],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    let idle_hits: Vec<&Finding> = findings.iter().filter(|f| f.rule_id == "idle-node-modules").collect();
    assert_eq!(idle_hits.len(), 1, "只应标记超过90天闲置项目的 node_modules");
    assert!(idle_hits[0].path.to_string_lossy().contains("old_project"));
}

#[test]
fn evaluate_downloads_old_files_and_installers() {
    let now_secs = 1_800_000_000u64;
    let old_190d = now_secs - 190 * 86400; // 190 天前
    let old_100d = now_secs - 100 * 86400; // 100 天前
    let recent_10d = now_secs - 10 * 86400; // 10 天前

    let env = MockEnv::new()
        .with_var("USERPROFILE", "C:\\Users\\test")
        .with_now(now_secs);
    let resolver = PathResolver::new(&env);

    let snap = mock_snapshot(
        Path::new("C:\\Users\\test"),
        Node::dir(
            "test",
            300_000_000,
            vec![Node::dir(
                "Downloads",
                300_000_000,
                vec![
                    // >180 天大文件 (>50MB)
                    Node::file_with_mtime("backup.zip", 60 * 1024 * 1024, old_190d),
                    // >90 天安装包 (.exe)
                    Node::file_with_mtime("installer.exe", 20 * 1024 * 1024, old_100d),
                    // 最近下载的安装包（10 天前）-> 不应命中
                    Node::file_with_mtime("fresh.msi", 15 * 1024 * 1024, recent_10d),
                ],
            )],
        ),
    );

    let findings = evaluate_rules(&snap, &resolver, &default_rules());
    assert!(findings.iter().any(|f| f.rule_id == "downloads-old-files"), "应命中 180 天前大文件");
    assert!(findings.iter().any(|f| f.rule_id == "downloads-installers"), "应命中 100 天前安装包");
    assert!(!findings.iter().any(|f| f.path.to_string_lossy().ends_with("fresh.msi")), "最近安装包不可误判");
}
