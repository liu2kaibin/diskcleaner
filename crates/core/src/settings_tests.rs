use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::settings::{
    default_settings_path, generate_purge_trash_script, load_settings, save_settings,
    scan_expired_trash, AppSettings, ExpiredTrashItem,
};

#[test]
fn settings_save_and_load_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("settings.json");

    let mut custom = AppSettings::default();
    custom.trash_retention_days = 30;
    custom.confirmation_threshold_gb = 100;
    custom.idle_project_days = 60;
    custom.direct_delete_whitelist.remove("npm-cache");

    save_settings(&custom, &file).expect("保存应成功");
    let loaded = load_settings(&file);

    assert_eq!(loaded.trash_retention_days, 30);
    assert_eq!(loaded.confirmation_threshold_gb, 100);
    assert_eq!(loaded.idle_project_days, 60);
    assert!(!loaded.direct_delete_whitelist.contains("npm-cache"));
}

#[test]
fn settings_fallback_to_default_on_missing_or_corrupt_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("broken.json");
    fs::write(&file, "not valid json").unwrap();

    let s1 = load_settings(&file);
    assert_eq!(s1.trash_retention_days, 14, "损坏文件应优雅回退至默认设置");

    let s2 = load_settings(&tmp.path().join("non_existent.json"));
    assert_eq!(s2.trash_retention_days, 14, "缺失文件应优雅回退至默认设置");
}

#[test]
fn test_default_settings_path() {
    let p = default_settings_path();
    assert!(p.to_string_lossy().contains("settings.json"));
}

#[test]
fn scan_expired_trash_detects_items_older_than_retention_days() {
    let tmp = tempfile::tempdir().unwrap();
    let trash_dir = tmp.path().join(".diskcleaner-trash");
    fs::create_dir_all(&trash_dir).unwrap();

    let old_file = trash_dir.join("old_item.bin");
    fs::write(&old_file, vec![0u8; 1000]).unwrap();

    let now_secs = 1_800_000_000u64;
    // 假设保留期为 14 天
    // 实际文件时间在磁盘创建为当前时间，测试中传入 retention_days = 0 模拟期满
    let expired = scan_expired_trash(&[trash_dir], 0, now_secs);

    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].size, 1000);
    assert_eq!(expired[0].path, old_file);
}

#[test]
fn generate_purge_trash_script_outputs_safe_powershell() {
    let item = ExpiredTrashItem {
        path: PathBuf::from("C:\\.diskcleaner-trash\\old_node_modules"),
        size: 500_000_000,
        age_days: 20,
    };

    let script = generate_purge_trash_script(&[item]);
    assert!(script.contains("Remove-Item -LiteralPath 'C:\\.diskcleaner-trash\\old_node_modules' -Recurse -Force"));
    assert!(script.contains("DiskCleaner - 清空回收目录期满内容"));
}
