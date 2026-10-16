//! PowerShell 清理脚本生成器（Seam ① 的另一核心组成部分）。
//! 严格遵守 ADR-0001、ADR-0002 与 GLOSSARY 词汇。

use std::collections::HashSet;
use std::path::Path;

use crate::rules::Finding;

pub struct ScriptConfig {
    /// 触发确认门的阈值大小（字节），默认 50GB
    pub confirmation_threshold: u64,
    /// 白名单规则 ID：可再生极低成本缓存，直接删除而不进回收目录
    pub direct_delete_rules: HashSet<String>,
    /// 每卷回收目录名，默认为 ".diskcleaner-trash"
    pub trash_dir_name: String,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        let mut direct = HashSet::new();
        direct.insert("npm-cache".to_string());
        Self {
            confirmation_threshold: 50 * 1024 * 1024 * 1024,
            direct_delete_rules: direct,
            trash_dir_name: ".diskcleaner-trash".to_string(),
        }
    }
}

/// 格式化字节为易读字符串（如 13.4 GB）
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

/// 解析路径所在的卷根目录，例如 "C:\Users\admin" -> "C:\"
fn extract_volume_root(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        format!("{}:\\", s[..1].to_uppercase())
    } else {
        "C:\\".to_string()
    }
}

/// 从选中的建议项生成 PowerShell 清理脚本。
/// 纯函数，入参为建议项切片与配置，输出脚本文本。
pub fn generate_cleanup_script(items: &[&Finding], config: &ScriptConfig) -> String {
    let mut out = String::new();
    let total_size: u64 = items.iter().map(|i| i.size).sum();

    // 1. 脚本头与清单摘要
    out.push_str("# ========================================================\n");
    out.push_str("# DiskCleaner - 自动化生成的安全清理脚本\n");
    out.push_str("# 提示: 本脚本由用户人工审核后自行执行 (零直接误删架构)\n");
    out.push_str("# ========================================================\n\n");
    out.push_str("$ErrorActionPreference = 'Continue'\n\n");

    out.push_str("Write-Host '========================================' -ForegroundColor Cyan\n");
    out.push_str("Write-Host ' DiskCleaner - 执行计划清单' -ForegroundColor Cyan\n");
    out.push_str("Write-Host '========================================' -ForegroundColor Cyan\n");
    out.push_str(&format!("Write-Host '待处理项数量: {}'\n", items.len()));
    out.push_str(&format!(
        "Write-Host '预估释放空间总量: {} ({total_size} 字节)'\n",
        format_bytes(total_size)
    ));
    out.push_str("Write-Host '----------------------------------------'\n");

    for item in items {
        let is_direct = config.direct_delete_rules.contains(&item.rule_id);
        let action_tag = if is_direct { "[直删白名单]" } else { "[移入回收目录]" };
        out.push_str(&format!(
            "Write-Host ' * {action_tag} {} ({}): {}'\n",
            item.title,
            format_bytes(item.size),
            item.path.to_string_lossy()
        ));
    }
    out.push_str("Write-Host '----------------------------------------'\n\n");

    // 2. 阈值确认门（Threshold Confirmation）
    if total_size > config.confirmation_threshold {
        out.push_str("# --- 安全阈值检查 (超过 50GB 限制) ---\n");
        out.push_str(&format!(
            "Write-Host '【警告】本次清理总量 {} 超过预设安全阈值 {}！' -ForegroundColor Yellow\n",
            format_bytes(total_size),
            format_bytes(config.confirmation_threshold)
        ));
        out.push_str("$confirm = Read-Host \"请确认是否继续？请输入 'YES' 继续，其它任意键取消\"\n");
        out.push_str("if ($confirm -ne 'YES') {\n");
        out.push_str("    Write-Host '操作已取消，未执行任何改动。' -ForegroundColor Gray\n");
        out.push_str("    exit\n");
        out.push_str("}\n\n");
    }

    // 3. 逐项执行动作
    out.push_str("# --- 执行清理命令 ---\n");
    for item in items {
        let path_str = item.path.to_string_lossy();
        let is_direct = config.direct_delete_rules.contains(&item.rule_id);

        if is_direct {
            out.push_str(&format!("Write-Host '正在直接删除: {path_str} ...'\n"));
            out.push_str(&format!(
                "if (Test-Path -LiteralPath '{path_str}') {{\n    Remove-Item -LiteralPath '{path_str}' -Recurse -Force -ErrorAction Continue\n}}\n"
            ));
        } else {
            let vol_root = extract_volume_root(&item.path);
            let trash_dir = format!("{vol_root}{}", config.trash_dir_name);
            out.push_str(&format!(
                "Write-Host '正在移入所在卷回收目录: {path_str} -> {trash_dir} ...'\n"
            ));
            out.push_str(&format!(
                "if (!(Test-Path -LiteralPath '{trash_dir}')) {{\n    New-Item -ItemType Directory -Path '{trash_dir}' -Force | Out-Null\n}}\n"
            ));
            out.push_str(&format!(
                "if (Test-Path -LiteralPath '{path_str}') {{\n    Move-Item -LiteralPath '{path_str}' -Destination '{trash_dir}' -Force -ErrorAction Continue\n}}\n"
            ));
        }
        out.push('\n');
    }

    out.push_str("Write-Host '========================================' -ForegroundColor Green\n");
    out.push_str("Write-Host '清理完成！' -ForegroundColor Green\n");
    out.push_str("Write-Host '========================================' -ForegroundColor Green\n");

    out
}
