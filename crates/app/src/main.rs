//! DiskCleaner —— 开发者视角的 Windows 磁盘清理工具
//! #01 骨架 + #02 规则建议 + #04 脚本生成闭环 + #03 语义类目与统计卡 + #06 推荐面板完整交互对齐

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use diskcleaner_core::{category, layout, rules, scanner, script};
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 840.0])
            .with_title("DiskCleaner"),
        ..Default::default()
    };
    eframe::run_native(
        "DiskCleaner",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            install_cjk_font(cc);
            Ok(Box::new(App::new()))
        }),
    )
}

/// Windows 中文字体（黑体单 ttf 最稳），否则界面中文会变方块。
fn install_cjk_font(cc: &eframe::CreationContext) {
    let candidates = [
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "cjk".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".to_owned());
            }
            break;
        }
    }
    cc.egui_ctx.set_fonts(fonts);
}

enum ScanState {
    /// 尚未开始
    Idle,
    /// 扫描中：启动时刻 + 进度计数（worker 累加）
    Scanning { started: Instant, progress: Arc<AtomicU64> },
}

struct App {
    snapshot: Option<scanner::Snapshot>,
    findings: Vec<rules::Finding>,
    selected_indices: HashSet<usize>,
    expanded_indices: HashSet<usize>,
    script_config: script::ScriptConfig,
    state: ScanState,
    receiver: Option<Receiver<scanner::Snapshot>>,
    scan_error: Option<String>,
    toast: Option<(Instant, String)>,
    /// 下钻路径栈（空代表根目录，如 ["Desktop", "project1"]）
    current_path: Vec<String>,
    /// 驱动器统计缓存 (total, free)
    drive_stats: Option<(u64, u64)>,
}

impl App {
    fn new() -> Self {
        let drive_stats = scanner::get_disk_free_space("C:\\");
        let mut app = Self {
            snapshot: None,
            findings: Vec::new(),
            selected_indices: HashSet::new(),
            expanded_indices: HashSet::new(),
            script_config: script::ScriptConfig::default(),
            state: ScanState::Idle,
            receiver: None,
            scan_error: None,
            toast: None,
            current_path: Vec::new(),
            drive_stats,
        };
        app.start_scan();
        app
    }

    fn start_scan(&mut self) {
        let root = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".into());
        let (tx, rx) = mpsc::channel();
        let progress = Arc::new(AtomicU64::new(0));
        let progress_worker = Arc::clone(&progress);
        self.receiver = Some(rx);
        self.scan_error = None;
        self.findings.clear();
        self.selected_indices.clear();
        self.expanded_indices.clear();
        self.current_path.clear();
        self.drive_stats = scanner::get_disk_free_space("C:\\");
        self.state = ScanState::Scanning { started: Instant::now(), progress };
        std::thread::spawn(move || {
            let snap = scanner::scan_with_progress(Path::new(&root), &progress_worker);
            let _ = tx.send(snap);
        });
    }

    fn poll_scan(&mut self) {
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(snap) => {
                    let resolver = rules::PathResolver::new(&rules::SystemEnv);
                    let rule_set = rules::default_rules();
                    let findings = rules::evaluate_rules(&snap, &resolver, &rule_set);

                    // 默认自动勾选全部 Safe to remove 建议项
                    self.selected_indices.clear();
                    for (i, f) in findings.iter().enumerate() {
                        if f.risk == rules::Risk::SafeToRemove {
                            self.selected_indices.insert(i);
                        }
                    }

                    self.findings = findings;
                    self.snapshot = Some(snap);
                    self.state = ScanState::Idle;
                    self.receiver = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.scan_error = Some("扫描线程异常退出".into());
                    self.state = ScanState::Idle;
                    self.receiver = None;
                }
            }
        }
    }

    fn selected_total_size(&self) -> u64 {
        self.selected_indices
            .iter()
            .filter_map(|&idx| self.findings.get(idx))
            .map(|f| f.size)
            .sum()
    }

    /// 获取当前下钻层级对应的节点引用
    fn get_current_node<'a>(&self, snapshot: &'a scanner::Snapshot) -> &'a scanner::Node {
        let mut curr = &snapshot.root;
        for seg in &self.current_path {
            match curr {
                scanner::Node::Dir { children, .. } => {
                    if let Some(child) = children.iter().find(|c| c.name() == seg) {
                        curr = child;
                    } else {
                        break;
                    }
                }
                scanner::Node::File { .. } => break,
            }
        }
        curr
    }

    /// 顶部 4 张统计卡（参考截图）
    fn top_stats_bar(&mut self, ui: &mut egui::Ui) {
        let selected_size = self.selected_total_size();
        let (total_drive, free_drive) = self.drive_stats.unwrap_or((0, 0));
        let free_after = free_drive + selected_size;

        let scanned_size = self.snapshot.as_ref().map(|s| s.root.size()).unwrap_or(0);
        let not_scanned = total_drive.saturating_sub(free_drive).saturating_sub(scanned_size);

        let safe_size: u64 = self
            .findings
            .iter()
            .filter(|f| f.risk == rules::Risk::SafeToRemove)
            .map(|f| f.size)
            .sum();
        let safe_count = self
            .findings
            .iter()
            .filter(|f| f.risk == rules::Risk::SafeToRemove)
            .count();

        let review_size: u64 = self
            .findings
            .iter()
            .filter(|f| f.risk == rules::Risk::NeedsReview)
            .map(|f| f.size)
            .sum();
        let review_count = self
            .findings
            .iter()
            .filter(|f| f.risk == rules::Risk::NeedsReview)
            .count();

        ui.horizontal(|ui| {
            let card_w = (ui.available_width() - 36.0) / 4.0;
            let card_h = 76.0;

            // 卡片 1: Free after cleanup
            Self::stat_card(
                ui,
                card_w,
                card_h,
                "Free after cleanup",
                &human_size(free_after),
                Some((format!("+{}", human_size(selected_size)), egui::Color32::from_rgb(0x16, 0xA3, 0x4A))),
                None,
            );

            // 卡片 2: Safe to remove
            Self::stat_card(
                ui,
                card_w,
                card_h,
                "● Safe to remove",
                &human_size(safe_size),
                None,
                Some(format!("{safe_count} items")),
            );

            // 卡片 3: Needs review
            Self::stat_card(
                ui,
                card_w,
                card_h,
                "● Needs review",
                &human_size(review_size),
                None,
                Some(format!("{review_count} items")),
            );

            // 卡片 4: Not scanned
            Self::stat_card(
                ui,
                card_w,
                card_h,
                "Not scanned",
                &human_size(not_scanned),
                None,
                Some("system & apps".to_string()),
            );
        });
    }

    fn stat_card(
        ui: &mut egui::Ui,
        w: f32,
        h: f32,
        title: &str,
        main_val: &str,
        badge: Option<(String, egui::Color32)>,
        sub_label: Option<String>,
    ) {
        egui::Frame::group(ui.style())
            .fill(egui::Color32::WHITE)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0xE5, 0xE7, 0xEB)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(w);
                ui.set_height(h);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if title.starts_with("● Safe") {
                            ui.label(
                                egui::RichText::new(title)
                                    .small()
                                    .color(egui::Color32::from_rgb(0x16, 0xA3, 0x4A)),
                            );
                        } else if title.starts_with("● Needs") {
                            ui.label(
                                egui::RichText::new(title)
                                    .small()
                                    .color(egui::Color32::from_rgb(0xD9, 0x77, 0x06)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(title)
                                    .small()
                                    .color(egui::Color32::from_rgb(0x6B, 0x72, 0x80)),
                            );
                        }
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(main_val)
                                .font(egui::FontId::proportional(22.0))
                                .strong()
                                .color(egui::Color32::from_rgb(0x11, 0x18, 0x27)),
                        );
                        if let Some((b_text, b_color)) = badge {
                            ui.label(egui::RichText::new(b_text).strong().color(b_color));
                        }
                        if let Some(sub) = sub_label {
                            ui.label(
                                egui::RichText::new(sub)
                                    .small()
                                    .color(egui::Color32::from_rgb(0x6B, 0x72, 0x80)),
                            );
                        }
                    });
                });
            });
    }

    /// Treemap 渲染 + 面包屑导航 + 下钻点击
    fn treemap_section(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = &self.snapshot else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("等待首次扫描…").color(egui::Color32::GRAY));
            });
            return;
        };

        let curr_node = self.get_current_node(snapshot);
        let scanner::Node::Dir { children, size: curr_size, .. } = curr_node else {
            ui.label("当前节点不是目录");
            return;
        };

        // 1. 面包屑与操作栏
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("~ Home").strong()).clicked() {
                self.current_path.clear();
            }
            let mut pop_to = None;
            for (idx, seg) in self.current_path.iter().enumerate() {
                ui.label("/");
                if ui.button(seg).clicked() {
                    pop_to = Some(idx + 1);
                }
            }
            if let Some(len) = pop_to {
                self.current_path.truncate(len);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(human_size(*curr_size)).color(egui::Color32::GRAY));
                ui.separator();
                ui.label(egui::RichText::new("Explorer").strong());
            });
        });
        ui.add_space(4.0);

        // 2. Treemap 主绘制区
        let bottom_legend_h = 32.0;
        let available = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(ui.available_width(), (ui.available_height() - bottom_legend_h).max(100.0)),
        );

        let painter = ui.painter();
        let sizes: Vec<u64> = children.iter().map(|c| c.size()).collect();
        let rects = layout::treemap_layout(
            &sizes,
            layout::Rect::new(
                available.left() as f64,
                available.top() as f64,
                available.width() as f64,
                available.height() as f64,
            ),
        );

        let mut clicked_dir = None;

        for (child, r) in children.iter().zip(rects.iter()) {
            if r.w < 2.0 || r.h < 2.0 {
                continue;
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(r.x as f32, r.y as f32),
                egui::vec2(r.w as f32, r.h as f32),
            );

            // 语义类目着色
            let (is_dir, kids) = match child {
                scanner::Node::Dir { children, .. } => (true, Some(children.as_slice())),
                scanner::Node::File { .. } => (false, None),
            };
            let cat = category::classify_node(child.name(), is_dir, kids);
            let (fill, stroke) = category_colors(cat);

            // 点击响应（仅目录可下钻）
            let id = ui.make_persistent_id(format!("tile_{}", child.name()));
            let response = ui.interact(rect, id, egui::Sense::click());
            if response.clicked() && is_dir {
                clicked_dir = Some(child.name().to_string());
            }

            let hovered = response.hovered();
            let final_stroke = if hovered {
                egui::Stroke::new(2.0, egui::Color32::from_rgb(0x3B, 0x82, 0xF6))
            } else {
                egui::Stroke::new(1.0, stroke)
            };

            painter.rect_filled(rect.shrink(1.0), 4.0, fill);
            painter.rect_stroke(rect.shrink(1.0), 4.0, final_stroke, egui::StrokeKind::Inside);

            // 瓦片文本标注
            if rect.width() > 60.0 && rect.height() > 32.0 {
                let inner = rect.shrink2(egui::vec2(4.0, 4.0));
                painter.text(
                    inner.left_top() + egui::vec2(2.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    child.name(),
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgb(0x1F, 0x29, 0x37),
                );
                if rect.height() > 46.0 {
                    painter.text(
                        inner.left_bottom() - egui::vec2(0.0, 4.0),
                        egui::Align2::LEFT_BOTTOM,
                        human_size(child.size()),
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgb(0x4B, 0x55, 0x63),
                    );
                }
            }
        }

        if let Some(dir) = clicked_dir {
            self.current_path.push(dir);
        }

        ui.allocate_rect(available, egui::Sense::hover());

        // 3. 底部类目图例（对齐参考截图）
        ui.add_space(6.0);
        Self::bottom_legend(ui);
    }

    /// 底部语义类目图例
    fn bottom_legend(ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let items = [
                ("Documents & media", rules::Category::DocumentMedia),
                ("Dev tools & other", rules::Category::DevToolOther),
                ("Code projects", rules::Category::CodeProject),
                ("iOS & Android SDKs", rules::Category::Sdk),
                ("AI models", rules::Category::AiModel),
                ("Package caches", rules::Category::PackageCache),
            ];

            for (label, cat) in items {
                let (fill, stroke) = category_colors(cat);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, fill);
                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
                ui.label(egui::RichText::new(label).small().color(egui::Color32::from_rgb(0x4B, 0x55, 0x63)));
                ui.add_space(8.0);
            }
        });
    }

    /// 右侧 Recommendations 面板：完整对齐截图交互与布局
    fn recommendations_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Recommendations");
        ui.label(
            egui::RichText::new("Select items to build a cleanup script")
                .small()
                .color(egui::Color32::from_rgb(0x6B, 0x72, 0x80)),
        );
        ui.add_space(8.0);

        if self.findings.is_empty() {
            if matches!(self.state, ScanState::Scanning { .. }) {
                ui.label("正在扫描计算建议项…");
            } else {
                ui.label("未在扫描范围内发现清理建议项");
            }
            return;
        }

        // 分组统计：Safe to remove 与 Review first (仅存下标)
        let safe_indices: Vec<usize> = self
            .findings
            .iter()
            .enumerate()
            .filter(|(_, f)| f.risk == rules::Risk::SafeToRemove)
            .map(|(i, _)| i)
            .collect();
        let safe_total: u64 = safe_indices.iter().map(|&i| self.findings[i].size).sum();

        let review_indices: Vec<usize> = self
            .findings
            .iter()
            .enumerate()
            .filter(|(_, f)| f.risk == rules::Risk::NeedsReview)
            .map(|(i, _)| i)
            .collect();
        let review_total: u64 = review_indices.iter().map(|&i| self.findings[i].size).sum();

        let bottom_height = 86.0;
        let scroll_height = (ui.available_height() - bottom_height).max(120.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), scroll_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // 分组 1: Safe to remove
                    if !safe_indices.is_empty() {
                        Self::section_header(
                            ui,
                            "Safe to remove",
                            egui::Color32::from_rgb(0xDC, 0xFC, 0xE7),
                            egui::Color32::from_rgb(0x16, 0xA3, 0x4A),
                            &human_size(safe_total),
                        );
                        ui.add_space(4.0);

                        for &idx in &safe_indices {
                            let finding = &self.findings[idx];
                            Self::render_finding_row(
                                ui,
                                &mut self.selected_indices,
                                &mut self.expanded_indices,
                                idx,
                                finding,
                            );
                        }
                        ui.add_space(10.0);
                    }

                    // 分组 2: Review first
                    if !review_indices.is_empty() {
                        Self::section_header(
                            ui,
                            "Review first",
                            egui::Color32::from_rgb(0xFE, 0xF3, 0xC7),
                            egui::Color32::from_rgb(0xD9, 0x77, 0x06),
                            &human_size(review_total),
                        );
                        ui.add_space(4.0);

                        for &idx in &review_indices {
                            let finding = &self.findings[idx];
                            Self::render_finding_row(
                                ui,
                                &mut self.selected_indices,
                                &mut self.expanded_indices,
                                idx,
                                finding,
                            );
                        }
                    }
                });
            },
        );

        ui.separator();

        // 底部操作区：对齐参考截图
        let selected_size = self.selected_total_size();
        let selected_count = self.selected_indices.len();

        ui.label(
            egui::RichText::new(format!("{} selected · {} items", human_size(selected_size), selected_count))
                .strong()
                .color(egui::Color32::from_rgb(0x37, 0x41, 0x51)),
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            // [Select safe] 按钮
            let select_safe_btn = ui.add_sized(
                [110.0, 32.0],
                egui::Button::new(
                    egui::RichText::new("Select safe")
                        .color(egui::Color32::from_rgb(0x1F, 0x29, 0x37)),
                )
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0xD1, 0xD5, 0xDB))),
            );
            if select_safe_btn.clicked() {
                self.selected_indices.clear();
                for (i, f) in self.findings.iter().enumerate() {
                    if f.risk == rules::Risk::SafeToRemove {
                        self.selected_indices.insert(i);
                    }
                }
            }

            ui.add_space(4.0);

            // [Copy script] 主操作按钮（深黑色胶囊底）
            let copy_btn = ui.add_sized(
                [ui.available_width(), 32.0],
                egui::Button::new(
                    egui::RichText::new("Copy script")
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(0x18, 0x18, 0x1B))
                .corner_radius(6.0),
            );

            if copy_btn.clicked() {
                let selected_items: Vec<&rules::Finding> = self
                    .selected_indices
                    .iter()
                    .filter_map(|&idx| self.findings.get(idx))
                    .collect();

                let ps_script = script::generate_cleanup_script(&selected_items, &self.script_config);
                ui.ctx().copy_text(ps_script);
                self.toast = Some((Instant::now(), "清理脚本已复制到剪贴板！".to_string()));
            }
        });
    }

    /// 分组小标题栏（胶囊 badge + 右侧容量）
    fn section_header(ui: &mut egui::Ui, title: &str, bg_color: egui::Color32, text_color: egui::Color32, total_str: &str) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(100.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, bg_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                title,
                egui::FontId::proportional(11.0),
                text_color,
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(total_str)
                        .small()
                        .color(egui::Color32::from_rgb(0x6B, 0x72, 0x80)),
                );
            });
        });
    }

    /// 单行建议项渲染（对齐截图结构：Checkbox | 标题 + 缩略路径 | 容量 + 展开箭头）
    fn render_finding_row(
        ui: &mut egui::Ui,
        selected_indices: &mut HashSet<usize>,
        expanded_indices: &mut HashSet<usize>,
        idx: usize,
        finding: &rules::Finding,
    ) {
        let is_expanded = expanded_indices.contains(&idx);
        let mut is_checked = selected_indices.contains(&idx);

        egui::Frame::group(ui.style())
            .fill(egui::Color32::WHITE)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0xF3, 0xF4, 0xF6)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut is_checked, "").changed() {
                        if is_checked {
                            selected_indices.insert(idx);
                        } else {
                            selected_indices.remove(&idx);
                        }
                    }

                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&finding.title)
                                .strong()
                                .color(egui::Color32::from_rgb(0x11, 0x18, 0x27)),
                        );
                        ui.label(
                            egui::RichText::new(format_display_path(&finding.path))
                                .small()
                                .color(egui::Color32::from_rgb(0x9C, 0xA3, 0xAF)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let arrow = if is_expanded { "v" } else { ">" };
                        if ui.button(egui::RichText::new(arrow).color(egui::Color32::GRAY)).clicked() {
                            if is_expanded {
                                expanded_indices.remove(&idx);
                            } else {
                                expanded_indices.insert(idx);
                            }
                        }
                        ui.label(
                            egui::RichText::new(human_size(finding.size))
                                .strong()
                                .color(egui::Color32::from_rgb(0x1F, 0x29, 0x37)),
                        );
                    });
                });

                // 点击展开展示详细原因与动作
                if is_expanded {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        match &finding.action {
                            rules::Action::CustomCommand { command, description } => {
                                ui.colored_label(egui::Color32::from_rgb(0x25, 0x63, 0xEB), "⚡ 维护压缩命令");
                                ui.label(egui::RichText::new(format!("{description}: `{command}`")).small());
                            }
                            rules::Action::DirectDelete => {
                                ui.colored_label(egui::Color32::from_rgb(0x16, 0xA3, 0x4A), "✓ 直删白名单");
                                ui.label(egui::RichText::new(&finding.reason).small());
                            }
                            rules::Action::MoveToTrash => {
                                ui.colored_label(egui::Color32::from_rgb(0x6B, 0x72, 0x80), "📁 移入回收目录");
                                ui.label(egui::RichText::new(&finding.reason).small());
                            }
                        }
                    });
                }
            });
        ui.add_space(3.0);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_scan();

        // 顶栏 1：控制与状态行
        egui::Panel::top("control_bar").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("DiskCleaner");
                ui.separator();

                let scanning = matches!(self.state, ScanState::Scanning { .. });
                ui.add_enabled_ui(!scanning, |ui| {
                    if ui.button("重新扫描").clicked() {
                        self.start_scan();
                    }
                });

                match (&self.state, &self.snapshot) {
                    (ScanState::Scanning { started, progress }, _) => {
                        let secs = started.elapsed().as_secs();
                        ui.spinner();
                        ui.label(format!(
                            "扫描中… 已耗时 {secs}s，已访问 {} 项",
                            progress.load(Ordering::Relaxed)
                        ));
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    (ScanState::Idle, Some(snap)) => {
                        ui.label(format!(
                            "扫描完成：{}，{} 项跳过",
                            human_dur(snap.duration),
                            snap.skipped
                        ));
                    }
                    (ScanState::Idle, None) => {
                        ui.label("尚未扫描");
                    }
                }

                if let Some(err) = &self.scan_error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                // Toast 提示条
                if let Some((t, msg)) = &self.toast {
                    if t.elapsed().as_secs() < 4 {
                        ui.separator();
                        ui.colored_label(egui::Color32::from_rgb(0x16, 0xA3, 0x4A), format!("✓ {msg}"));
                    }
                }
            });
            ui.add_space(4.0);
        });

        // 顶栏 2：四张大统计卡（参考截图）
        egui::Panel::top("stats_bar").show(ui, |ui| {
            ui.add_space(6.0);
            self.top_stats_bar(ui);
            ui.add_space(6.0);
        });

        // 右侧建议面板（宽度设为 380px，留出舒适的边距）
        egui::Panel::right("recommendations_panel")
            .exact_size(380.0)
            .show(ui, |ui| {
                self.recommendations_panel(ui);
            });

        // 中心面板：Treemap 与钻取
        egui::CentralPanel::default().show(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0xE5, 0xE7, 0xEB)))
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    self.treemap_section(ui);
                });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(
            if matches!(self.state, ScanState::Scanning { .. }) { 200 } else { 1000 },
        ));
    }
}

/// 路径缩写显示（将 USERPROFILE 替换为 ~，对齐原型截图）
fn format_display_path(path: &Path) -> String {
    if let Ok(user) = std::env::var("USERPROFILE") {
        let user_path = Path::new(&user);
        if let Ok(rel) = path.strip_prefix(user_path) {
            return format!("~\\{}", rel.display());
        }
    }
    path.to_string_lossy().into_owned()
}

/// 语义类目对应的粉彩柔和配色 (填充色, 边框色)
fn category_colors(cat: rules::Category) -> (egui::Color32, egui::Color32) {
    match cat {
        rules::Category::CodeProject => (
            egui::Color32::from_rgb(0xDE, 0xE4, 0xFD), // 柔和蓝紫
            egui::Color32::from_rgb(0xA5, 0xB4, 0xFC),
        ),
        rules::Category::AiModel => (
            egui::Color32::from_rgb(0xFD, 0xDE, 0xE9), // 柔和粉红
            egui::Color32::from_rgb(0xF4, 0x72, 0xB6),
        ),
        rules::Category::PackageCache => (
            egui::Color32::from_rgb(0xD1, 0xFA, 0xE5), // 柔和薄荷绿
            egui::Color32::from_rgb(0x34, 0xD3, 0x99),
        ),
        rules::Category::Sdk => (
            egui::Color32::from_rgb(0xCF, 0xFA, 0xFE), // 柔和青天蓝
            egui::Color32::from_rgb(0x38, 0xBD, 0xF8),
        ),
        rules::Category::DocumentMedia => (
            egui::Color32::from_rgb(0xFF, 0xED, 0xD5), // 柔和浅桃橙
            egui::Color32::from_rgb(0xFB, 0x92, 0x3C),
        ),
        rules::Category::DevToolOther => (
            egui::Color32::from_rgb(0xF1, 0xF5, 0xF9), // 柔和冷灰
            egui::Color32::from_rgb(0x94, 0xA3, 0xB8),
        ),
        rules::Category::BuildArtifact => (
            egui::Color32::from_rgb(0xFE, 0xF3, 0xC7), // 柔和浅金黄
            egui::Color32::from_rgb(0xFB, 0xBF, 0x24),
        ),
        rules::Category::Other => (
            egui::Color32::from_rgb(0xF8, 0xFA, 0xFC), // 极淡灰
            egui::Color32::from_rgb(0xCB, 0xD5, 0xE1),
        ),
    }
}

fn human_size(bytes: u64) -> String {
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

fn human_dur(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 60 {
        format!("{}m{}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}
