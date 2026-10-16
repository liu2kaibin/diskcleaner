//! Scan Scope 内的目录遍历器：产出快照树（路径 + 聚合大小）。
//! 见 spec：并行遍历、普通权限、独立线程由调用方负责。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant, UNIX_EPOCH};

use jwalk::{Parallelism, WalkDirGeneric};
use serde::{Deserialize, Serialize};

fn default_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 快照树节点。目录的 size 为子树聚合值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    File { name: String, size: u64, mtime: u64 },
    Dir { name: String, size: u64, mtime: u64, children: Vec<Node> },
}

impl Node {
    pub fn size(&self) -> u64 {
        match self {
            Node::File { size, .. } => *size,
            Node::Dir { size, .. } => *size,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Node::File { name, .. } | Node::Dir { name, .. } => name,
        }
    }

    pub fn mtime(&self) -> u64 {
        match self {
            Node::File { mtime, .. } | Node::Dir { mtime, .. } => *mtime,
        }
    }

    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Node::File { name: name.into(), size, mtime: 0 }
    }

    pub fn file_with_mtime(name: impl Into<String>, size: u64, mtime: u64) -> Self {
        Node::File { name: name.into(), size, mtime }
    }

    pub fn dir(name: impl Into<String>, size: u64, children: Vec<Node>) -> Self {
        let mtime = children.iter().map(|c| c.mtime()).max().unwrap_or(0);
        Node::Dir { name: name.into(), size, mtime, children }
    }

    pub fn dir_with_mtime(name: impl Into<String>, size: u64, mtime: u64, children: Vec<Node>) -> Self {
        Node::Dir { name: name.into(), size, mtime, children }
    }
}

/// 一次扫描的产物。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub root_path: PathBuf,
    pub root: Node,
    pub duration: Duration,
    /// 因权限等原因被跳过的条目数（08 号票细化）。
    pub skipped: u64,
    /// 扫描完成时刻的 Unix 时间戳（秒）
    #[serde(default = "default_timestamp")]
    pub scanned_at_secs: u64,
}

impl Snapshot {
    /// 在快照树中按绝对路径查找节点。若路径不属于该快照或未扫描到，返回 None。
    pub fn find_node(&self, path: &Path) -> Option<&Node> {
        let rel = match path.strip_prefix(&self.root_path) {
            Ok(r) => r,
            Err(_) => return None,
        };

        let mut curr = &self.root;
        for component in rel.components() {
            let comp_str = component.as_os_str().to_string_lossy();
            match curr {
                Node::Dir { children, .. } => {
                    curr = children.iter().find(|c| c.name() == comp_str)?;
                }
                Node::File { .. } => return None,
            }
        }
        Some(curr)
    }
}

/// 查询指定驱动器（如 "C:\\"）的总容量与可用容量 (total_bytes, free_bytes)。
pub fn get_disk_free_space(drive: &str) -> Option<(u64, u64)> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = OsStr::new(drive).encode_wide().chain(Some(0)).collect();
        let mut free_bytes_available = 0u64;
        let mut total_number_of_bytes = 0u64;
        let mut total_number_of_free_bytes = 0u64;

        extern "system" {
            fn GetDiskFreeSpaceExW(
                lpDirectoryName: *const u16,
                lpFreeBytesAvailableToCaller: *mut u64,
                lpTotalNumberOfBytes: *mut u64,
                lpTotalNumberOfFreeBytes: *mut u64,
            ) -> i32;
        }

        unsafe {
            let ret = GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_bytes_available,
                &mut total_number_of_bytes,
                &mut total_number_of_free_bytes,
            );
            if ret != 0 {
                Some((total_number_of_bytes, total_number_of_free_bytes))
            } else {
                None
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = drive;
        None
    }
}

/// 遍历 `root`，返回聚合大小的快照。阻塞式；调用方负责放线程。
pub fn scan(root: &Path) -> Snapshot {
    scan_with_progress(root, &AtomicU64::new(0))
}

/// 同 [`scan`]，并把已访问条目数累加到 `visited`（UI 进度用）。
pub fn scan_with_progress(root: &Path, visited: &AtomicU64) -> Snapshot {
    let start = Instant::now();
    let mut skipped = 0u64;

    // 扁平收集：父路径 -> 子节点原始信息。
    // metadata 在 process_read_dir 回调里预读——该回调在 rayon 并行池中执行，
    // 预读 size 与 mtime，不产生二次 stat。
    let mut raw: HashMap<PathBuf, Vec<RawNode>> = HashMap::new();
    for entry in WalkDirGeneric::<((), Option<(u64, u64)>)>::new(root)
        .follow_links(false)
        .parallelism(Parallelism::RayonNewPool(16))
        .process_read_dir(|_, _, _, results| {
            results.iter_mut().for_each(|r| {
                if let Ok(e) = r {
                    let (size, mtime) = if let Ok(m) = e.metadata() {
                        let len = if e.file_type.is_dir() { 0 } else { m.len() };
                        let mt = m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        (len, mt)
                    } else {
                        (0, 0)
                    };
                    e.client_state = Some((size, mtime));
                }
            })
        })
    {
        match entry {
            Ok(e) => {
                if e.depth() == 0 {
                    continue;
                }
                visited.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let (size, mtime) = e.client_state.unwrap_or((0, 0));
                raw.entry(e.parent_path.to_path_buf()).or_default().push(RawNode {
                    name: e.file_name().to_string_lossy().into_owned(),
                    is_dir: e.file_type.is_dir(),
                    size,
                    mtime,
                });
            }
            Err(_) => skipped += 1,
        }
    }

    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let root_node = build_node(root, &mut raw, root_name);

    let scanned_at_secs = default_timestamp();

    Snapshot {
        root_path: root.to_path_buf(),
        root: root_node,
        duration: start.elapsed(),
        skipped,
        scanned_at_secs,
    }
}

struct RawNode {
    name: String,
    is_dir: bool,
    size: u64,
    mtime: u64,
}

/// 从扁平映射递归装配子树，聚合目录大小与最新修改时间。consume raw 避免克隆。
fn build_node(dir: &Path, raw: &mut HashMap<PathBuf, Vec<RawNode>>, name: String) -> Node {
    let mut size = 0u64;
    let mut mtime = 0u64;
    let mut children = Vec::new();
    if let Some(list) = raw.remove(dir) {
        for c in list {
            if c.is_dir {
                let child_path = dir.join(&c.name);
                let node = build_node(&child_path, raw, c.name);
                size += node.size();
                mtime = mtime.max(node.mtime());
                children.push(node);
            } else {
                size += c.size;
                mtime = mtime.max(c.mtime);
                children.push(Node::File { name: c.name, size: c.size, mtime: c.mtime });
            }
        }
    }
    Node::Dir { name, size, mtime, children }
}
