//! Treemap 布局：squarified 算法（Bruls et al.）。纯函数，core 内无 UI 依赖。

/// 轴对齐矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// 为 `sizes`（与输出等长、按原始顺序对齐）计算 squarified treemap 布局。
/// 输入顺序不要求排序；零大小项得到零面积矩形。
pub fn treemap_layout(sizes: &[u64], area: Rect) -> Vec<Rect> {
    let n = sizes.len();
    let mut out = vec![Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; n];
    let total: u64 = sizes.iter().sum();
    if n == 0 || total == 0 || area.w <= 0.0 || area.h <= 0.0 {
        return out;
    }
    let scale = (area.w * area.h) / total as f64; // 字节 -> 平方像素

    // 按面积降序布局（squarify 前提），零面积项排除
    let mut order: Vec<usize> = (0..n).filter(|&i| sizes[i] > 0).collect();
    order.sort_unstable_by(|&a, &b| sizes[b].cmp(&sizes[a]));

    let mut placed: Vec<Option<Rect>> = vec![None; n];
    let (mut x, mut y, mut w, mut h) = (area.x, area.y, area.w, area.h);
    let mut row_idx: Vec<usize> = Vec::new();
    let mut row_area: Vec<f64> = Vec::new();
    let mut i = 0;

    while i < order.len() {
        if w <= 0.0 || h <= 0.0 {
            break; // 剩余项拿零矩形
        }
        let side = w.min(h);
        let a = sizes[order[i]] as f64 * scale;
        let cand_worst = worst_with(&row_area, Some(a), side);
        if row_area.is_empty() || cand_worst <= worst_with(&row_area, None, side) {
            row_area.push(a);
            row_idx.push(order[i]);
            i += 1;
        } else {
            place_row(&mut placed, &row_idx, &row_area, x, y, w, h);
            let s: f64 = row_area.iter().sum();
            if w >= h {
                let t = s / h;
                x += t;
                w -= t;
            } else {
                let t = s / w;
                y += t;
                h -= t;
            }
            row_area.clear();
            row_idx.clear();
        }
    }
    if !row_idx.is_empty() {
        place_row(&mut placed, &row_idx, &row_area, x, y, w, h);
    }
    for (i, slot) in placed.into_iter().enumerate() {
        if let Some(r) = slot {
            out[i] = r;
        }
    }
    out
}

/// Bruls worst-ratio：行内矩形贴边长 `side` 排列时的最差长宽比。
/// `extra` 为试挥加入行的新项。
fn worst_with(row: &[f64], extra: Option<f64>, side: f64) -> f64 {
    let mut s = 0.0f64;
    let mut rmax = 0.0f64;
    let mut rmin = f64::INFINITY;
    for &a in row {
        s += a;
        rmax = rmax.max(a);
        rmin = rmin.min(a);
    }
    if let Some(a) = extra {
        s += a;
        rmax = rmax.max(a);
        rmin = rmin.min(a);
    }
    if s <= 0.0 || rmin <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let s2 = s * s;
    let l2 = side * side;
    (l2 * rmax / s2).max(s2 / (l2 * rmin))
}

/// 把一行矩形贴在当前剩余区域的短边侧。
fn place_row(
    placed: &mut [Option<Rect>],
    row_idx: &[usize],
    row_area: &[f64],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let s: f64 = row_area.iter().sum();
    if s <= 0.0 || w <= 0.0 || h <= 0.0 {
        return;
    }
    if w >= h {
        let thickness = s / h;
        let mut off = y;
        for (&idx, &a) in row_idx.iter().zip(row_area.iter()) {
            let ih = a / thickness;
            placed[idx] = Some(Rect::new(x, off, thickness, ih));
            off += ih;
        }
    } else {
        let thickness = s / w;
        let mut off = x;
        for (&idx, &a) in row_idx.iter().zip(row_area.iter()) {
            let iw = a / thickness;
            placed[idx] = Some(Rect::new(off, y, iw, thickness));
            off += iw;
        }
    }
}
