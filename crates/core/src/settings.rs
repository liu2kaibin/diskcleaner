//! 用户设置持久化与回收目录期满检查。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub trash_retention_days: u64,
    pub confirmation_threshold_gb: u64,
    pub idle_project_days: u64,
    pub direct_delete_whitelist: HashSet<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut whitelist = HashSet::new();
        whitelist.insert("npm-cache".to_string());
        whitelist.insert("pip-cache".to_string());
        whitelist.insert("uv-cache".to_string());
        whitelist.insert("cargo-registry".to_string());
        whitelist.insert("gradle-maven-cache".to_string());
        whitelist.insert("user-temp".to_string());
        whitelist.insert("build-artifacts".to_string());
        whitelist.insert("thumbnail-cache".to_string());

        Self {
            trash_retention_days: 14,
            confirmation_threshold_gb: 50,
            idle_project_days: 90,
            direct_delete_whitelist: whitelist,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredTrashItem {
    pub path: PathBuf,
    pub size: u64,
    pub age_days: u64,
}

pub fn load_settings(path: &Path) -> AppSettings {
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(settings) = serde_json::from_str(&content) {
            return settings;
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化设置失败: {e}"))?;
    fs::write(path, json).map_err(|e| format!("写入设置文件失败: {e}"))
}

pub fn default_settings_path() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("diskcleaner").join("settings.json")
    } else if let Ok(user) = std::env::var("USERPROFILE") {
        PathBuf::from(user).join(".diskcleaner").join("settings.json")
    } else {
        PathBuf::from(".diskcleaner-settings.json")
    }
}

/// 扫描指定回收目录中超过保留期的项
pub fn scan_expired_trash(
    trash_dirs: &[PathBuf],
    retention_days: u64,
    now_secs: u64,
) -> Vec<ExpiredTrashItem> {
    let mut out = Vec::new();
    let retention_secs = retention_days * 86400;

    for dir in trash_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(m) = entry.metadata() {
                    let mtime = m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let diff_secs = now_secs.saturating_sub(mtime);
                    if diff_secs >= retention_secs {
                        let age_days = diff_secs / 86400;
                        let size = if m.is_dir() {
                            dir_size(&entry.path())
                        } else {
                            m.len()
                        };
                        out.push(ExpiredTrashItem {
                            path: entry.path(),
                            size,
                            age_days,
                        });
                    }
                }
            }
        }
    }
    out
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_dir() {
                    total += dir_size(&entry.path());
                } else {
                    total += m.len();
                }
            }
        }
    }
    total
}

/// 生成专门清空回收目录期满项的 PowerShell 脚本
pub fn generate_purge_trash_script(items: &[ExpiredTrashItem]) -> String {
    let mut out = String::new();
    let total_size: u64 = items.iter().map(|i| i.size).sum();

    out.push_str("# ========================================================\n");
    out.push_str("# DiskCleaner - 清空回收目录期满内容脚本\n");
    out.push_str("# 提示: 本脚本将永久删除已保留超过指定天数的回收目录文件\n");
    out.push_str("# ========================================================\n\n");
    out.push_str("$ErrorActionPreference = 'Continue'\n\n");
    out.push_str("Write-Host '========================================' -ForegroundColor Cyan\n");
    out.push_str("Write-Host ' DiskCleaner - 清空回收目录期满内容' -ForegroundColor Cyan\n");
    out.push_str("Write-Host '========================================' -ForegroundColor Cyan\n");
    out.push_str(&format!("Write-Host '待彻底清除项: {} 个'\n", items.len()));
    out.push_str(&format!("Write-Host '预估永久释放空间: {} 字节'\n", total_size));
    out.push_str("Write-Host '----------------------------------------'\n\n");

    for item in items {
        let p = item.path.to_string_lossy();
        out.push_str(&format!("Write-Host '正在永久清除已保留 {} 天的内容: {p}'\n", item.age_days));
        out.push_str(&format!(
            "if (Test-Path -LiteralPath '{p}') {{\n    Remove-Item -LiteralPath '{p}' -Recurse -Force -ErrorAction Continue\n}}\n\n"
        ));
    }

    out.push_str("Write-Host '回收目录期满内容已清除完成。' -ForegroundColor Green\n");
    out
}
