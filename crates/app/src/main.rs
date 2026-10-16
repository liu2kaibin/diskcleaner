//! DiskCleaner —— 开发者视角的 Windows 磁盘清理工具（#01 骨架 + #02 规则建议 + #04 脚本生成闭环）。

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use diskcleaner_core::{layout, rules, scanner, script};
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("DiskCleaner"),
        ..Default::default()
    };
    eframe::run_native(
        "DiskCleaner",
        options,
        Box::new(|cc| {
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
    script_config: script::ScriptConfig,
    state: ScanState,
    receiver: Option<Receiver<scanner::Snapshot>>,
    scan_error: Option<String>,
    toast: Option<(Instant, String)>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            snapshot: None,
            findings: Vec::new(),
            selected_indices: HashSet::new(),
            script_config: script::ScriptConfig::default(),
            state: ScanState::Idle,
            receiver: None,
            scan_error: None,
            toast: None,
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
        self.state = ScanState::Scanning { started: Instant::now(), progress };
        std::thread::spawn(move || {
            let snap = scanner::scan_with_progress(std::path::Path::new(&root), &progress_worker);
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

    fn treemap(ui: &mut egui::Ui, snapshot: &scanner::Snapshot) {
        let scanner::Node::Dir { children, .. } = &snapshot.root else {
            ui.label("扫描结果不是目录");
            return;
        };
        let available = ui.available_rect_before_wrap();
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

        for (child, r) in children.iter().zip(rects.iter()) {
            if r.w < 1.0 || r.h < 1.0 {
                continue;
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(r.x as f32, r.y as f32),
                egui::vec2(r.w as f32, r.h as f32),
            );
            // #01 占位单色；#03 换语义类目着色
            painter.rect_filled(
                rect.shrink(1.0),
                4.0,
                egui::Color32::from_rgb(0x5B, 0x6E, 0xEE),
            );
            painter.rect_stroke(
                rect.shrink(1.0),
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(0xC8, 0xD2, 0xFF)),
                egui::StrokeKind::Inside,
            );
            if rect.width() > 70.0 && rect.height() > 34.0 {
                let inner = rect.shrink2(egui::vec2(4.0, 4.0));
                painter.text(
                    inner.left_top() + egui::vec2(2.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    child.name(),
                    egui::FontId::proportional(13.0),
                    egui::Color32::WHITE,
                );
                painter.text(
                    inner.left_bottom() - egui::vec2(0.0, 6.0),
                    egui::Align2::LEFT_BOTTOM,
                    human_size(child.size()),
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(0xE8, 0xEC, 0xFF),
                );
            }
        }
        // 占住空间，避免 egui 认为中心面板为空
        ui.allocate_rect(available, egui::Sense::hover());
    }

    fn recommendations_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("Recommendations");
            ui.label(egui::RichText::new("清理建议").color(egui::Color32::GRAY));
        });
        ui.separator();

        if self.findings.is_empty() {
            if matches!(self.state, ScanState::Scanning { .. }) {
                ui.label("正在扫描计算中…");
            } else {
                ui.label("暂未发现匹配建议项 (npm cache 未在扫描范围)");
            }
            return;
        }

        // 快捷勾选工具栏
        ui.horizontal(|ui| {
            if ui.button("Select safe (仅安全项)").clicked() {
                self.selected_indices.clear();
                for (i, f) in self.findings.iter().enumerate() {
                    if f.risk == rules::Risk::SafeToRemove {
                        self.selected_indices.insert(i);
                    }
                }
            }
            if ui.button("清空已选").clicked() {
                self.selected_indices.clear();
            }
        });
        ui.add_space(4.0);

        // 底部固定的导出区域计算高度，其余滚动
        let bottom_height = 80.0;
        let scroll_height = (ui.available_height() - bottom_height).max(100.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), scroll_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, finding) in self.findings.iter().enumerate() {
                        let mut is_checked = self.selected_indices.contains(&i);
                        egui::Frame::group(ui.style())
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut is_checked, "").changed() {
                                        if is_checked {
                                            self.selected_indices.insert(i);
                                        } else {
                                            self.selected_indices.remove(&i);
                                        }
                                    }
                                    ui.strong(&finding.title);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.strong(human_size(finding.size));
                                    });
                                });
                                ui.horizontal(|ui| {
                                    match finding.risk {
                                        rules::Risk::SafeToRemove => {
                                            ui.label(
                                                egui::RichText::new("● Safe to remove")
                                                    .color(egui::Color32::from_rgb(0x22, 0xC5, 0x5E)),
                                            );
                                        }
                                        rules::Risk::NeedsReview => {
                                            ui.label(
                                                egui::RichText::new("● Needs review")
                                                    .color(egui::Color32::from_rgb(0xF5, 0x9E, 0x0B)),
                                            );
                                        }
                                    }
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(finding.path.to_string_lossy())
                                        .small()
                                        .color(egui::Color32::DARK_GRAY),
                                );
                                ui.label(egui::RichText::new(&finding.reason).small());
                            });
                        ui.add_space(6.0);
                    }
                });
            },
        );

        ui.separator();
        let selected_size = self.selected_total_size();
        let selected_count = self.selected_indices.len();

        ui.horizontal(|ui| {
            ui.label(format!("已选中 {} 项，共 {}", selected_count, human_size(selected_size)));
        });

        ui.add_space(2.0);
        let copy_btn = ui.add_sized(
            [ui.available_width(), 32.0],
            egui::Button::new(
                egui::RichText::new("📋 Copy cleanup script (复制清理脚本)")
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(0x25, 0x63, 0xEB)),
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
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_scan();

        egui::Panel::top("top_bar").show(ui, |ui| {
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

                // 统计指标：Free after cleanup 联动展示
                let selected_size = self.selected_total_size();
                ui.separator();
                ui.label("Free after cleanup:");
                ui.label(
                    egui::RichText::new(format!("+{}", human_size(selected_size)))
                        .strong()
                        .color(egui::Color32::from_rgb(0x22, 0xC5, 0x5E)),
                );
                ui.separator();

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
                        ui.colored_label(egui::Color32::from_rgb(0x22, 0xC5, 0x5E), format!("✓ {msg}"));
                    }
                }
            });
            ui.add_space(6.0);
        });

        egui::Panel::right("recommendations_panel")
            .exact_size(360.0)
            .show(ui, |ui| {
                self.recommendations_panel(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| match &self.snapshot {
            Some(snap) => Self::treemap(ui, snap),
            None => {
                ui.centered_and_justified(|ui| ui.label("等待首次扫描…"));
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(
            if matches!(self.state, ScanState::Scanning { .. }) { 200 } else { 1000 },
        ));
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
