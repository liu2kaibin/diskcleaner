use std::collections::HashSet;
use std::path::PathBuf;

use crate::rules::{Category, Finding, Risk};
use crate::script::{generate_cleanup_script, ScriptConfig};

fn sample_finding(rule_id: &str, path: &str, size: u64, risk: Risk) -> Finding {
    Finding {
        rule_id: rule_id.to_string(),
        title: format!("Test {rule_id}"),
        path: PathBuf::from(path),
        size,
        risk,
        category: Category::PackageCache,
        reason: "Test reason".to_string(),
    }
}

#[test]
fn script_header_contains_item_list_and_total_size() {
    let f1 = sample_finding("npm-cache", "C:\\Users\\admin\\AppData\\Local\\npm-cache", 1024 * 1024 * 500, Risk::SafeToRemove);
    let f2 = sample_finding("old-logs", "D:\\projects\\logs", 1024 * 1024 * 200, Risk::NeedsReview);

    let config = ScriptConfig::default();
    let script = generate_cleanup_script(&[&f1, &f2], &config);

    assert!(script.contains("DiskCleaner"), "脚本头应标识 DiskCleaner");
    assert!(script.contains("C:\\Users\\admin\\AppData\\Local\\npm-cache"));
    assert!(script.contains("D:\\projects\\logs"));
    assert!(script.contains("700.0 MB") || script.contains("734003200 B") || script.contains("700 MB"));
}

#[test]
fn script_direct_delete_for_whitelisted_rules() {
    let f = sample_finding("npm-cache", "C:\\Users\\admin\\AppData\\Local\\npm-cache", 1000, Risk::SafeToRemove);
    let config = ScriptConfig::default(); // 默认 direct_delete_rules 含 "npm-cache"

    let script = generate_cleanup_script(&[&f], &config);

    assert!(script.contains("Remove-Item"), "白名单项应使用 Remove-Item 直接删除");
    assert!(!script.contains("Move-Item"), "白名单项不应移动到 Recovery Directory");
}

#[test]
fn script_move_to_recovery_directory_for_non_whitelisted() {
    let f = sample_finding("android-sdk", "C:\\Users\\admin\\AppData\\Local\\Android", 1000, Risk::NeedsReview);
    let mut config = ScriptConfig::default();
    config.direct_delete_rules.clear(); // 确保不走直删

    let script = generate_cleanup_script(&[&f], &config);

    assert!(script.contains("Move-Item"), "非白名单项应移入回收目录");
    assert!(script.contains(".diskcleaner-trash"), "应包含卷根目录的 .diskcleaner-trash 路径");
    assert!(script.contains("C:\\.diskcleaner-trash"), "C盘路径应移动到 C 盘回收目录");
}

#[test]
fn script_threshold_confirmation_injected_when_exceeding_threshold() {
    let big_size = 60 * 1024 * 1024 * 1024; // 60 GB
    let f = sample_finding("big-dump", "C:\\temp\\big.dump", big_size, Risk::NeedsReview);

    let config = ScriptConfig {
        confirmation_threshold: 50 * 1024 * 1024 * 1024, // 50 GB 阈值
        direct_delete_rules: HashSet::new(),
        trash_dir_name: ".diskcleaner-trash".to_string(),
    };

    let script = generate_cleanup_script(&[&f], &config);
    assert!(
        script.contains("Read-Host") && script.contains("YES"),
        "超过 50GB 阈值时必须生成交互确认门"
    );
}

#[test]
fn script_no_threshold_confirmation_when_below_threshold() {
    let small_size = 10 * 1024 * 1024 * 1024; // 10 GB < 50 GB
    let f = sample_finding("small-cache", "C:\\temp\\small", small_size, Risk::SafeToRemove);

    let config = ScriptConfig::default(); // 阈值 50GB
    let script = generate_cleanup_script(&[&f], &config);

    assert!(
        !script.contains("Read-Host"),
        "未超过阈值时不应生成阻断确认门"
    );
}
