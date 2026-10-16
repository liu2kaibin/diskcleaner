//! 扫描快照的磁盘持久化与加载（JSON 存储）。
//! 支持跨会话秒级开屏加载、损坏容错优雅降级。

use std::fs;
use std::path::{Path, PathBuf};

use crate::scanner::Snapshot;

/// 保存快照到指定路径（若父目录不存在则自动创建）
pub fn save_snapshot(snapshot: &Snapshot, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string(snapshot).map_err(|e| format!("序列化快照失败: {e}"))?;
    fs::write(path, json).map_err(|e| format!("写入快照文件失败: {e}"))
}

/// 从指定路径加载快照，解析失败或文件不存在时返回 Err，绝不 panic
pub fn load_snapshot(path: &Path) -> Result<Snapshot, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("读取快照文件失败: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("反序列化快照失败: {e}"))
}

/// 默认快照存储路径（优先 %LOCALAPPDATA%\diskcleaner\snapshot.json）
pub fn default_snapshot_path() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("diskcleaner").join("snapshot.json")
    } else if let Ok(user) = std::env::var("USERPROFILE") {
        PathBuf::from(user).join(".diskcleaner").join("snapshot.json")
    } else {
        PathBuf::from(".diskcleaner-snapshot.json")
    }
}
