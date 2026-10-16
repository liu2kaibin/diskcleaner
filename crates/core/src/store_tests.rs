use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::scanner::{Node, Snapshot};
use crate::store::{default_snapshot_path, load_snapshot, save_snapshot};

fn dummy_snapshot() -> Snapshot {
    Snapshot {
        root_path: Path::new("C:\\Users\\test").to_path_buf(),
        root: Node::dir(
            "test",
            100_000,
            vec![
                Node::file("file1.txt", 40_000),
                Node::dir("sub", 60_000, vec![Node::file("file2.bin", 60_000)]),
            ],
        ),
        duration: Duration::from_millis(120),
        skipped: 3,
        scanned_at_secs: 1_700_000_000,
    }
}

#[test]
fn snapshot_save_and_load_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("snapshots").join("snap.json");

    let orig = dummy_snapshot();
    save_snapshot(&orig, &file_path).expect("保存应成功");

    let loaded = load_snapshot(&file_path).expect("读取应成功");

    assert_eq!(loaded.root_path, orig.root_path);
    assert_eq!(loaded.root.size(), orig.root.size());
    assert_eq!(loaded.skipped, orig.skipped);
    assert_eq!(loaded.scanned_at_secs, orig.scanned_at_secs);
}

#[test]
fn snapshot_corrupted_file_fails_gracefully() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("corrupted.json");

    fs::write(&file_path, "{ broken json content !!!").unwrap();

    let res = load_snapshot(&file_path);
    assert!(res.is_err(), "解析损坏的 JSON 必须返回 Err 优雅失败，不可 panic");
}

#[test]
fn snapshot_missing_file_fails_gracefully() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("non_existent.json");

    let res = load_snapshot(&file_path);
    assert!(res.is_err(), "不存在的文件必须返回 Err 优雅失败");
}

#[test]
fn default_snapshot_path_is_not_empty() {
    let p = default_snapshot_path();
    assert!(p.to_string_lossy().contains("snapshot.json"));
}
