//! Scan Scope 内的目录遍历器：产出快照树（路径 + 聚合大小）。
//! 见 spec：并行遍历、普通权限、独立线程由调用方负责。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use jwalk::{Parallelism, WalkDirGeneric};

/// 快照树节点。目录的 size 为子树聚合值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    File { name: String, size: u64 },
    Dir { name: String, size: u64, children: Vec<Node> },
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
}

/// 一次扫描的产物。
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub root_path: PathBuf,
    pub root: Node,
    pub duration: Duration,
    /// 因权限等原因被跳过的条目数（08 号票细化）。
    pub skipped: u64,
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
    // 否则几十万次 stat 在消费循环里单线程串行（实测 77.6s → 预算 60s 的瓶颈）。
    let mut raw: HashMap<PathBuf, Vec<RawNode>> = HashMap::new();
    for entry in WalkDirGeneric::<((), Option<u64>)>::new(root)
        .follow_links(false)
        .parallelism(Parallelism::RayonNewPool(16))
        .process_read_dir(|_, _, _, results| {
            results.iter_mut().for_each(|r| {
                if let Ok(e) = r {
                    if !e.file_type.is_dir() {
                        e.client_state = Some(e.metadata().map(|m| m.len()).unwrap_or(0));
                    }
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
                raw.entry(e.parent_path.to_path_buf()).or_default().push(RawNode {
                    name: e.file_name().to_string_lossy().into_owned(),
                    is_dir: e.file_type.is_dir(),
                    size: e.client_state.unwrap_or(0),
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

    Snapshot {
        root_path: root.to_path_buf(),
        root: root_node,
        duration: start.elapsed(),
        skipped,
    }
}

struct RawNode {
    name: String,
    is_dir: bool,
    size: u64,
}

/// 从扁平映射递归装配子树，聚合目录大小。consume raw 避免克隆。
fn build_node(dir: &Path, raw: &mut HashMap<PathBuf, Vec<RawNode>>, name: String) -> Node {
    let mut size = 0u64;
    let mut children = Vec::new();
    if let Some(list) = raw.remove(dir) {
        for c in list {
            if c.is_dir {
                let child_path = dir.join(&c.name);
                let node = build_node(&child_path, raw, c.name);
                size += node.size();
                children.push(node);
            } else {
                size += c.size;
                children.push(Node::File { name: c.name, size: c.size });
            }
        }
    }
    Node::Dir { name, size, children }
}
