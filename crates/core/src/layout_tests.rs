use crate::layout::{treemap_layout, Rect};

fn area(r: &Rect) -> f64 {
    r.w * r.h
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x - 1e-9
        && inner.y >= outer.y - 1e-9
        && inner.x + inner.w <= outer.x + outer.w + 1e-9
        && inner.y + inner.h <= outer.y + outer.h + 1e-9
}

/// 面积守恒：全部矩形面积之和 = 容器面积（字节数按比例映射）。
#[test]
fn layout_preserves_total_area() {
    let sizes = [300u64, 200, 150, 100, 50];
    let container = Rect::new(0.0, 0.0, 800.0, 600.0);
    let rects = treemap_layout(&sizes, container);

    assert_eq!(rects.len(), sizes.len());
    let total: f64 = rects.iter().map(area).sum();
    assert!(
        (total - area(&container)).abs() < 1e-6,
        "总面积应守恒: {total} vs {}",
        area(&container)
    );
}

/// 面积按大小比例分配：最大项面积 = 容器 × (最大/总和)。
#[test]
fn layout_area_proportional_to_size() {
    let sizes = [600u64, 300, 100];
    let container = Rect::new(0.0, 0.0, 1000.0, 500.0);
    let rects = treemap_layout(&sizes, container);

    let total = sizes.iter().sum::<u64>() as f64;
    for (r, s) in rects.iter().zip(sizes.iter()) {
        let want = area(&container) * (*s as f64) / total;
        assert!(
            (area(r) - want).abs() < 1e-6,
            "大小 {s} 的矩形面积 {} 应为 {want}",
            area(r)
        );
    }
}

/// 包含性：每个矩形都落在容器内。
#[test]
fn layout_rects_stay_inside_container() {
    let sizes = [500u64, 400, 300, 200, 100, 50, 20, 1];
    let container = Rect::new(10.0, 20.0, 640.0, 480.0);
    let rects = treemap_layout(&sizes, container);
    for r in &rects {
        assert!(contains(&container, r), "矩形越界: {r:?}");
    }
}

/// 零大小项：得到零面积矩形，且不影响其它项面积守恒。
#[test]
fn layout_handles_zero_sizes() {
    let sizes = [100u64, 0, 100];
    let container = Rect::new(0.0, 0.0, 400.0, 300.0);
    let rects = treemap_layout(&sizes, container);
    assert_eq!(rects.len(), 3);
    assert_eq!(area(&rects[1]), 0.0, "零大小项应为零面积");
    let total: f64 = rects.iter().map(area).sum();
    assert!((total - area(&container)).abs() < 1e-6);
}

/// 空输入与零容器不 panic。
#[test]
fn layout_degenerate_inputs_are_safe() {
    let empty: Vec<Rect> = treemap_layout(&[], Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!(empty.is_empty());
    let rects = treemap_layout(&[10u64], Rect::new(0.0, 0.0, 0.0, 100.0));
    assert_eq!(rects.len(), 1);
    assert_eq!(area(&rects[0]), 0.0);
}
