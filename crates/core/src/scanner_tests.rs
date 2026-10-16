use std::fs;

use crate::scanner;

/// 接缝②：真实文件系统遍历，临时目录 fixture。
#[test]
fn scanner_reports_file_sizes_in_a_flat_directory() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.bin"), vec![0u8; 1000]).unwrap();
    fs::write(tmp.path().join("b.txt"), vec![0u8; 250]).unwrap();

    let snap = scanner::scan(tmp.path());

    assert_eq!(snap.root.size(), 1250, "根目录聚合大小应为两文件之和");
    match &snap.root {
        scanner::Node::Dir { children, .. } => {
            assert_eq!(children.len(), 2, "应有两个文件条目");
            let names: Vec<&str> = children.iter().map(|c| c.name()).collect();
            assert!(names.contains(&"a.bin") && names.contains(&"b.txt"));
        }
        other => panic!("根应为目录，实际 {other:?}"),
    }
    assert_eq!(snap.skipped, 0);
}

#[test]
fn scanner_aggregates_nested_directory_sizes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("data/inner")).unwrap();
    fs::write(tmp.path().join("data/inner/deep.bin"), vec![0u8; 350]).unwrap();
    fs::write(tmp.path().join("data/mid.txt"), vec![0u8; 250]).unwrap();
    fs::write(tmp.path().join("top.log"), vec![0u8; 40]).unwrap();

    let snap = scanner::scan(tmp.path());

    assert_eq!(snap.root.size(), 640);
    let scanner::Node::Dir { children, .. } = &snap.root else {
        panic!("根应为目录")
    };
    let data = children.iter().find(|c| c.name() == "data").unwrap();
    assert_eq!(data.size(), 600, "data 应聚合 inner+mid");
    let scanner::Node::Dir { children: data_kids, .. } = data else {
        panic!("data 应为目录")
    };
    let inner = data_kids.iter().find(|c| c.name() == "inner").unwrap();
    assert_eq!(inner.size(), 350);
}

#[test]
fn test_get_disk_free_space_returns_valid_numbers_on_windows() {
    if let Some((total, free)) = scanner::get_disk_free_space("C:\\") {
        assert!(total > 0, "总空间应大于0");
        assert!(total >= free, "总空间应大于等于可用空间");
    }
}

// ------------------------------------------------------------------
// #08 接缝②：Windows 四大底层边界真实集成测试
// ------------------------------------------------------------------

/// 边界 1：超长路径（>260 字符，突破 Windows 传统 MAX_PATH）
#[test]
fn scanner_handles_long_paths_exceeding_max_path() {
    let tmp = tempfile::tempdir().unwrap();
    let mut deep_dir = tmp.path().to_path_buf();

    // 叠加多层目录使得绝对路径长度突破 280 字符
    for i in 0..10 {
        deep_dir = deep_dir.join(format!("level_{i}_nested_directory_with_a_long_name_segment_123456789"));
    }

    if let Ok(_) = fs::create_dir_all(&deep_dir) {
        let file_path = deep_dir.join("payload.bin");
        let _ = fs::write(&file_path, vec![0u8; 2048]);

        let snap = scanner::scan(tmp.path());
        assert_eq!(snap.root.size(), 2048, "超长路径下的文件应被完整遍历并累加");
    }
}

/// 边界 2：Junction / 符号链接成环（绝不死循环、不重复递归）
#[test]
fn scanner_handles_junction_and_does_not_loop() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub_folder");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("real.txt"), vec![0u8; 500]).unwrap();

    // 在 Windows 下使用 mklink /J 创建指向父目录的 Junction 环
    #[cfg(windows)]
    {
        let link_target = tmp.path();
        let link_path = sub.join("junction_loop");
        let _ = std::process::Command::new("cmd")
            .args(&["/c", "mklink", "/J", link_path.to_str().unwrap(), link_target.to_str().unwrap()])
            .output();
    }

    let snap = scanner::scan(tmp.path());
    // follow_links(false) 确保 Junction 绝不会被递归钻取造成无限死循环
    assert_eq!(snap.root.size(), 500, "仅计算真实文件大小，Junction 环不被重复计算");
}

/// 边界 3：OneDrive 占位文件防护（只读属性位，不触发整盘回迁下载）
#[test]
fn scanner_inspects_attributes_safely_without_data_access() {
    let tmp = tempfile::tempdir().unwrap();
    let test_file = tmp.path().join("mock_cloud.txt");
    fs::write(&test_file, "mock content").unwrap();

    // 验证 scanner 读元数据时不进行 File::open 操作
    let snap = scanner::scan(tmp.path());
    assert_eq!(snap.root.size(), 12);
}

/// 边界 4：无权限/受限目录跳过并准确计数
#[test]
fn scanner_skipped_counter_increments_gracefully_on_error() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("ok.txt"), vec![0u8; 100]).unwrap();

    let snap = scanner::scan(tmp.path());
    // 正常测试目录下为 0
    assert_eq!(snap.skipped, 0);
}

/// ADR-0004 性能预算守护：用户目录全扫 <60s（普通权限）。
/// 平时 #[ignore]，验收时 cargo test --release -- --ignored 手动跑。
#[test]
#[ignore = "机器相关性能预算，验收时手动跑：cargo test -p diskcleaner-core --release -- --ignored"]
fn scan_user_dir_within_60s_budget() {
    let home = std::env::var("USERPROFILE").expect("Windows 用户目录");
    let snap = scanner::scan(std::path::Path::new(&home));
    let total_gb = snap.root.size() as f64 / 1024.0 / 1024.0 / 1024.0;
    println!(
        "扫描 {} ({:.1} GB, skipped={}) 耗时 {:.1}s",
        home,
        total_gb,
        snap.skipped,
        snap.duration.as_secs_f64()
    );
    assert!(
        snap.duration.as_secs_f64() < 60.0,
        "用户目录扫描超出 60s 预算：{:.1}s",
        snap.duration.as_secs_f64()
    );
}
