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
    // root/
    //   data/          600
    //     inner/       350
    //       deep.bin   350
    //     mid.txt      250
    //   top.log        40
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
