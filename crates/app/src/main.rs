//! DiskCleaner —— 开发者视角的 Windows 磁盘清理工具
//! #01 骨架 + #02 规则建议 + #04 脚本生成闭环 + #03 语义类目与统计卡 + #06 推荐面板完整交互对齐

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use diskcleaner_core::{category, layout, rules, scanner, script, settings, store};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Treemap,
    Downloads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadSort {
    SizeDesc,
    AgeDesc,
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
    /// 视图模式（Treemap 概览 vs Downloads 专项审阅）
    view_mode: ViewMode,
    download_sort: DownloadSort,
    /// 用户偏好设置
    settings: settings::AppSettings,
    show_settings: bool,
    expired_trash: Vec<settings::ExpiredTrashItem>,
}

impl App {
    fn new() -> Self {
        let drive_stats = scanner::get_disk_free_space("C:\\");
        let snap_file = store::default_snapshot_path();

        let (init_snap, init_findings, init_selected, toast_msg) =
            if let Ok(snap) = store::load_snapshot(&snap_file) {
                let resolver = rules::PathResolver::new(&rules::SystemEnv);
                let rule_set = rules::default_rules();
                let findings = rules::evaluate_rules(&snap, &resolver, &rule_set);
                let mut selected = HashSet::new();
                for (i, f) in findings.iter().enumerate() {
                    if f.risk == rules::Risk::SafeToRemove {
                        selected.insert(i);
                    }
                }
                let ago = format_time_ago(snap.scanned_at_secs);
                (
                    Some(snap),
                    findings,
                    selected,
                    Some(format!("已秒开上次快照（扫描于 {ago} 前）")),
                )
            } else {
                (None, Vec::new(), HashSet::new(), None)
            };

        let needs_initial_scan = init_snap.is_none();

        let loaded_settings = settings::load_settings(&settings::default_settings_path());
        let mut script_cfg = script::ScriptConfig::default();
        script_cfg.confirmation_threshold = loaded_settings.confirmation_threshold_gb * 1024 * 1024 * 1024;
        script_cfg.direct_delete_rules = loaded_settings.direct_delete_whitelist.clone();

        let trash_dirs = vec![
            std::path::PathBuf::from("C:\\.diskcleaner-trash"),
            std::path::PathBuf::from("D:\\.diskcleaner-trash"),
        ];
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let expired_trash = settings::scan_expired_trash(&trash_dirs, loaded_settings.trash_retention_days, now_secs);

        let mut app = Self {
            snapshot: init_snap,
            findings: init_findings,
            selected_indices: init_selected,
            expanded_indices: HashSet::new(),
            script_config: script_cfg,
            state: ScanState::Idle,
            receiver: None,
            scan_error: None,
            toast: toast_msg.map(|m| (Instant::now(), m)),
            current_path: Vec::new(),
            drive_stats,
            view_mode: ViewMode::Treemap,
            download_sort: DownloadSort::SizeDesc,
            settings: loaded_settings,
            show_settings: false,
            expired_trash,
        };

        if needs_initial_scan {
            app.start_scan();
        }

        app
    }

    fn start_scan(&mut self) {
        let root = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".into());
        let (tx, rx) = mpsc::channel();
        let progress = Arc::new(AtomicU64::new(0));
        let progress_worker = Arc::clone(&progress);
        self.receiver = Some(rx);
        self.scan_error = None;
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

                    // 自动持久化新快照到本地磁盘
                    let _ = store::save_snapshot(&snap, &store::default_snapshot_path());

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
                    self.toast = Some((Instant::now(), "最新扫描快照已保存并生效！".to_string()));
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
                let dl_btn = ui.button("📥 Downloads 专项审阅");
                if dl_btn.clicked() {
                    self.view_mode = ViewMode::Downloads;
                }
                ui.separator();
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

    /// 专项 Downloads 审阅视图（解决 13GB 下载占用）
    fn downloads_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("📥 Downloads 专项审阅");
            ui.label(
                egui::RichText::new("陈旧大文件与历史安装包清理")
                    .small()
                    .color(egui::Color32::from_rgb(0x6B, 0x72, 0x80)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("📊 返回 Treemap 空间图").clicked() {
                    self.view_mode = ViewMode::Treemap;
                }
            });
        });
        ui.separator();

        let mut old_files_indices: Vec<usize> = self
            .findings
            .iter()
            .enumerate()
            .filter(|(_, f)| f.rule_id == "downloads-old-files")
            .map(|(i, _)| i)
            .collect();

        let mut installers_indices: Vec<usize> = self
            .findings
            .iter()
            .enumerate()
            .filter(|(_, f)| f.rule_id == "downloads-installers")
            .map(|(i, _)| i)
            .collect();

        let old_files_size: u64 = old_files_indices.iter().map(|&i| self.findings[i].size).sum();
        let installers_size: u64 = installers_indices.iter().map(|&i| self.findings[i].size).sum();
        let total_dl_cleanable = old_files_size + installers_size;

        // 顶部操作栏：汇总 + 快捷勾选 + 排序
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "可释放总量: {} (陈旧大文件 {} + 历史安装包 {})",
                    human_size(total_dl_cleanable),
                    human_size(old_files_size),
                    human_size(installers_size)
                ))
                .strong()
                .color(egui::Color32::from_rgb(0x1F, 0x29, 0x37)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("勾选全部安装包").clicked() {
                    for &idx in &installers_indices {
                        self.selected_indices.insert(idx);
                    }
                }
                if ui.button("勾选全部陈旧文件").clicked() {
                    for &idx in &old_files_indices {
                        self.selected_indices.insert(idx);
                    }
                }
                ui.separator();
                if ui.selectable_label(self.download_sort == DownloadSort::AgeDesc, "按时间排序").clicked() {
                    self.download_sort = DownloadSort::AgeDesc;
                }
                if ui.selectable_label(self.download_sort == DownloadSort::SizeDesc, "按大小 ↓").clicked() {
                    self.download_sort = DownloadSort::SizeDesc;
                }
                ui.label("排序:");
            });
        });
        ui.add_space(6.0);

        match self.download_sort {
            DownloadSort::SizeDesc => {
                old_files_indices.sort_by(|&a, &b| self.findings[b].size.cmp(&self.findings[a].size));
                installers_indices.sort_by(|&a, &b| self.findings[b].size.cmp(&self.findings[a].size));
            }
            DownloadSort::AgeDesc => {
                old_files_indices.sort_by(|&a, &b| self.findings[a].title.cmp(&self.findings[b].title));
                installers_indices.sort_by(|&a, &b| self.findings[a].title.cmp(&self.findings[b].title));
            }
        }

        let bottom_legend_h = 32.0;
        let available_h = (ui.available_height() - bottom_legend_h).max(120.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), available_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // 分组 1: 🕒 超过 180 天未动大文件
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🕒 超过 180 天未动的大文件 (>50MB)")
                                .strong()
                                .color(egui::Color32::from_rgb(0x1F, 0x29, 0x37)),
                        );
                        ui.label(
                            egui::RichText::new(format!("({} 项，共 {})", old_files_indices.len(), human_size(old_files_size)))
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    });
                    ui.add_space(4.0);

                    if old_files_indices.is_empty() {
                        ui.label(
                            egui::RichText::new("（未在 Downloads 目录下检出超过 180 天且 >50MB 的大文件）")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        for &idx in &old_files_indices {
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

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // 分组 2: 📦 超过 90 天历史安装包
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("📦 超过 90 天的历史安装包 (.exe / .msi)")
                                .strong()
                                .color(egui::Color32::from_rgb(0x1F, 0x29, 0x37)),
                        );
                        ui.label(
                            egui::RichText::new(format!("({} 项，共 {})", installers_indices.len(), human_size(installers_size)))
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    });
                    ui.add_space(4.0);

                    if installers_indices.is_empty() {
                        ui.label(
                            egui::RichText::new("（未在 Downloads 目录下检出超过 90 天的 .exe/.msi 安装包）")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        for &idx in &installers_indices {
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

        ui.add_space(6.0);
        Self::bottom_legend(ui);
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

        egui::ScrollArea::vertical()
            .max_height(scroll_height)
            .auto_shrink(false)
            .show(ui, |ui| {
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
            let total_w = ui.available_width();
            let select_safe_w = (total_w * 0.42).max(100.0);
            let copy_w = (total_w - select_safe_w - 8.0).max(120.0);

            // [Select safe] 按钮
            let select_safe_btn = ui.add_sized(
                [select_safe_w, 32.0],
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
                [copy_w, 32.0],
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

                    // 为右侧容量与箭头预留 95px，中间标题和路径严格截断，绝不水平撑爆导致整栏错位
                    let text_w = (ui.available_width() - 95.0).max(60.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(text_w, 36.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&finding.title)
                                        .strong()
                                        .color(egui::Color32::from_rgb(0x11, 0x18, 0x27)),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format_display_path(&finding.path))
                                        .small()
                                        .color(egui::Color32::from_rgb(0x9C, 0xA3, 0xAF)),
                                )
                                .truncate(),
                            );
                        },
                    );

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
                    ui.horizontal_wrapped(|ui| {
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
                    (ScanState::Scanning { started, progress }, Some(_)) => {
                        let secs = started.elapsed().as_secs();
                        ui.spinner();
                        ui.label(format!(
                            "后台重扫中… 已耗时 {secs}s，已访问 {} 项（当前可继续浏览旧快照）",
                            progress.load(Ordering::Relaxed)
                        ));
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    (ScanState::Scanning { started, progress }, None) => {
                        let secs = started.elapsed().as_secs();
                        ui.spinner();
                        ui.label(format!(
                            "首次扫描中… 已耗时 {secs}s，已访问 {} 项",
                            progress.load(Ordering::Relaxed)
                        ));
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    (ScanState::Idle, Some(snap)) => {
                        ui.label(format!(
                            "快照已就绪：扫描于 {} 前（用时 {}，{} 项跳过）",
                            format_time_ago(snap.scanned_at_secs),
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

                // 期满回收目录提醒
                if !self.expired_trash.is_empty() {
                    let exp_size: u64 = self.expired_trash.iter().map(|i| i.size).sum();
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(0xD9, 0x77, 0x06),
                        format!("⚠️ 回收目录已积压 {} 项（共 {}）超过 {} 天", self.expired_trash.len(), human_size(exp_size), self.settings.trash_retention_days),
                    );
                    if ui.button("清空期满项").clicked() {
                        let purge_script = settings::generate_purge_trash_script(&self.expired_trash);
                        ui.ctx().copy_text(purge_script);
                        self.toast = Some((Instant::now(), "清空回收目录脚本已复制到剪贴板！".to_string()));
                    }
                }

                // Toast 提示条
                if let Some((t, msg)) = &self.toast {
                    if t.elapsed().as_secs() < 4 {
                        ui.separator();
                        ui.colored_label(egui::Color32::from_rgb(0x16, 0xA3, 0x4A), format!("✓ {msg}"));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙ 设置").clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // 顶栏 2：四张大统计卡（参考截图）
        egui::Panel::top("stats_bar").show(ui, |ui| {
            ui.add_space(6.0);
            self.top_stats_bar(ui);
            ui.add_space(6.0);
        });

        // 右侧建议面板（支持用户拖拽调整宽度，默认 380px）
        egui::Panel::right("recommendations_panel")
            .default_size(380.0)
            .min_size(320.0)
            .max_size(600.0)
            .resizable(true)
            .show(ui, |ui| {
                self.recommendations_panel(ui);
            });

        // 中心面板：Treemap 与钻取 或 Downloads 专项审阅
        egui::CentralPanel::default().show(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0xE5, 0xE7, 0xEB)))
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| match self.view_mode {
                    ViewMode::Treemap => self.treemap_section(ui),
                    ViewMode::Downloads => self.downloads_view(ui),
                });
        });

        // 设置弹窗
        if self.show_settings {
            egui::Window::new("⚙ 偏好设置")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.set_width(460.0);
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("回收目录保留期:");
                        ui.add(egui::Slider::new(&mut self.settings.trash_retention_days, 1..=90).suffix(" 天"));
                    });
                    ui.label(egui::RichText::new("存放超过该天数的移入项将提示彻底清空").small().color(egui::Color32::GRAY));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("二次确认安全阈值:");
                        ui.add(egui::Slider::new(&mut self.settings.confirmation_threshold_gb, 10..=200).suffix(" GB"));
                    });
                    ui.label(egui::RichText::new("单次清理脚本释放空间超出此阈值时强制输入 'YES' 确认").small().color(egui::Color32::GRAY));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("闲置项目判定天数:");
                        ui.add(egui::Slider::new(&mut self.settings.idle_project_days, 30..=365).suffix(" 天"));
                    });
                    ui.label(egui::RichText::new("项目超过此天数无代码改动时，其 node_modules 视为闲置").small().color(egui::Color32::GRAY));
                    ui.add_space(10.0);

                    ui.separator();
                    ui.label(egui::RichText::new("直删白名单规则（勾选则直接删除，不产生二次搬运）:").strong());
                    ui.add_space(4.0);

                    let candidates = [
                        ("npm-cache", "npm 依赖包缓存"),
                        ("pip-cache", "Python pip 包缓存"),
                        ("uv-cache", "Python uv 虚拟环境缓存"),
                        ("cargo-registry", "Rust Cargo Crates 源码索引"),
                        ("gradle-maven-cache", "Java Gradle / Maven 依赖包"),
                        ("user-temp", "Windows 用户级临时目录 (Temp)"),
                        ("build-artifacts", "代码项目构建产物 (target/build/dist)"),
                        ("thumbnail-cache", "Windows 缩略图缓存 (thumbcache)"),
                    ];

                    for (r_id, desc) in candidates {
                        let mut checked = self.settings.direct_delete_whitelist.contains(r_id);
                        if ui.checkbox(&mut checked, format!("{desc} ({r_id})")).changed() {
                            if checked {
                                self.settings.direct_delete_whitelist.insert(r_id.to_string());
                            } else {
                                self.settings.direct_delete_whitelist.remove(r_id);
                            }
                        }
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        let save_btn = ui.add_sized(
                            [120.0, 30.0],
                            egui::Button::new(
                                egui::RichText::new("保存并生效")
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(0x16, 0xA3, 0x4A))
                            .corner_radius(6.0),
                        );

                        if save_btn.clicked() {
                            let _ = settings::save_settings(&self.settings, &settings::default_settings_path());
                            self.script_config.confirmation_threshold = self.settings.confirmation_threshold_gb * 1024 * 1024 * 1024;
                            self.script_config.direct_delete_rules = self.settings.direct_delete_whitelist.clone();
                            self.show_settings = false;
                            self.toast = Some((Instant::now(), "设置已保存并即时生效！".to_string()));
                        }

                        if ui.button("关闭").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
        }

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

fn format_time_ago(scanned_at_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(scanned_at_secs);
    let diff = now.saturating_sub(scanned_at_secs);
    if diff < 60 {
        "刚刚".to_string()
    } else if diff < 3600 {
        format!("{} 分钟", diff / 60)
    } else if diff < 86400 {
        format!("{} 小时", diff / 3600)
    } else {
        format!("{} 天", diff / 86400)
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
