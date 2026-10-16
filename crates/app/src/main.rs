//! DiskCleaner —— 开发者视角的 Windows 磁盘清理工具（#01 骨架：扫描 + treemap）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use diskcleaner_core::{layout, scanner};
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
    state: ScanState,
    receiver: Option<Receiver<scanner::Snapshot>>,
    scan_error: Option<String>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            snapshot: None,
            state: ScanState::Idle,
            receiver: None,
            scan_error: None,
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
            });
            ui.add_space(6.0);
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
