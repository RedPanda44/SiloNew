//! The Silo installer UI: dark themed, animated, page-based.

use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use egui::{
    Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    pos2, vec2,
};

use crate::ops::{self, Msg};
use crate::theme as t;
use crate::VERSION;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Install,
    Uninstall,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Welcome,
    Working,
    Finished,
    Failed,
    Confirm,
    Done,
}

/// Font family bound to a real bold face (Segoe UI Bold), set up in `main`.
fn strong_family() -> egui::FontFamily {
    egui::FontFamily::Name("strong".into())
}

pub struct InstallerApp {
    active_op: Op,
    page: Page,
    page_entered: f64,
    install_dir: PathBuf,
    desktop_shortcut: bool,
    launch_after: bool,
    remove_data: bool,
    data_removed: bool,
    progress_target: f32,
    progress_shown: f32,
    step_label: String,
    error: String,
    pop_at: f64,
    rx: Option<mpsc::Receiver<Msg>>,
}

impl InstallerApp {
    pub fn new(uninstall: bool) -> Self {
        let mode = if uninstall { Op::Uninstall } else { Op::Install };
        let page = if uninstall { Page::Confirm } else { Page::Welcome };
        let install_dir = if uninstall {
            ops::uninstall_source_dir()
        } else {
            ops::default_install_dir()
        };
        Self {
            active_op: mode,
            page,
            page_entered: 0.0,
            install_dir,
            desktop_shortcut: true,
            launch_after: true,
            remove_data: false,
            data_removed: false,
            progress_target: 0.0,
            progress_shown: 0.0,
            step_label: String::new(),
            error: String::new(),
            pop_at: 0.0,
            rx: None,
        }
    }

    fn set_page(&mut self, page: Page, time: f64) {
        self.page = page;
        self.page_entered = time;
    }

    fn poll(&mut self, time: f64) {
        let Some(rx) = self.rx.take() else { return };
        let mut keep = true;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Progress(fraction, label) => {
                    self.progress_target = fraction;
                    self.step_label = label;
                }
                Msg::Done => {
                    keep = false;
                    self.progress_target = 1.0;
                    self.pop_at = time;
                    let next = match self.active_op {
                        Op::Install => Page::Finished,
                        Op::Uninstall => Page::Done,
                    };
                    self.set_page(next, time);
                }
                Msg::Failed(err) => {
                    keep = false;
                    self.error = err;
                    self.pop_at = time;
                    self.set_page(Page::Failed, time);
                }
            }
        }
        if keep {
            self.rx = Some(rx);
        }
    }

    fn begin_install(&mut self, time: f64) {
        let dir = self.install_dir.clone();
        if dir.as_os_str().is_empty() {
            return;
        }
        self.active_op = Op::Install;
        self.error.clear();
        self.progress_target = 0.0;
        self.progress_shown = 0.0;
        self.step_label = "Preparing…".to_string();
        self.set_page(Page::Working, time);
        let (tx, rx) = mpsc::channel();
        ops::start_install(dir, self.desktop_shortcut, crate::PAYLOAD, tx);
        self.rx = Some(rx);
    }

    fn begin_uninstall(&mut self, time: f64) {
        self.active_op = Op::Uninstall;
        self.error.clear();
        self.data_removed = self.remove_data;
        self.progress_target = 0.0;
        self.progress_shown = 0.0;
        self.step_label = "Preparing…".to_string();
        self.set_page(Page::Working, time);
        let (tx, rx) = mpsc::channel();
        ops::start_uninstall(self.install_dir.clone(), self.remove_data, tx);
        self.rx = Some(rx);
    }

    fn primary(&mut self, ctx: &egui::Context, time: f64) {
        match self.page {
            Page::Welcome => self.begin_install(time),
            Page::Confirm => self.begin_uninstall(time),
            Page::Finished => {
                if self.launch_after {
                    let exe = self.install_dir.join(ops::APP_EXE);
                    let _ = Command::new(exe).spawn();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Page::Failed => match self.active_op {
                Op::Install => self.begin_install(time),
                Op::Uninstall => self.begin_uninstall(time),
            },
            Page::Done => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Page::Working => {}
        }
    }

    fn close(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // ------------------------------------------------------------------ UI --

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = ctx.input(|i| i.time);
        self.poll(time);

        // Smooth the progress bar toward the real worker progress.
        let delta = self.progress_target - self.progress_shown;
        if delta.abs() > 0.001 {
            self.progress_shown += delta * 0.14;
            ctx.request_repaint();
        } else {
            self.progress_shown = self.progress_target;
        }

        let (enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if enter && self.page != Page::Working {
            self.primary(ctx, time);
        }
        if escape && self.page != Page::Working {
            self.close(ctx);
        }

        // Footer with the action buttons.
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(t::FOOTER)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                self.footer(ui, ctx, time);
            });

        // Main content.
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t::BG)
                    .inner_margin(egui::Margin::symmetric(26, 16)),
            )
            .show(ctx, |ui| {
                self.header(ui, time);

                let body_rect = ui.available_rect_before_wrap();
                let enter_t = ((time - self.page_entered) / 0.32) as f32;
                let e = t::ease_out_cubic(enter_t);
                if e < 1.0 {
                    ui.add_space((1.0 - e) * 12.0);
                }
                self.body(ui, time);

                // Cross-fade curtain: painted last, fades from the background
                // color to fully transparent as the page settles.
                if e < 1.0 {
                    let alpha = ((1.0 - e) * 255.0).round() as u8;
                    ui.painter().rect_filled(
                        body_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(t::BG.r(), t::BG.g(), t::BG.b(), alpha),
                    );
                }

                // Hairline above the footer.
                let bottom = ui.max_rect().bottom();
                ui.painter().line_segment(
                    [
                        Pos2::new(ui.max_rect().min.x, bottom - 0.5),
                        Pos2::new(ui.max_rect().max.x, bottom - 0.5),
                    ],
                    Stroke::new(1.0_f32, t::HAIRLINE),
                );
            });

        // Keep animating (breathing glow, shimmer, spinner, transitions).
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn header(&self, ui: &mut egui::Ui, time: f64) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 70.0), Sense::hover());
        let painter = ui.painter_at(rect);
        t::gradient_v(&painter, rect, t::HEADER_TOP, t::HEADER_BOTTOM, 12);
        painter.line_segment(
            [
                Pos2::new(rect.min.x, rect.max.y - 0.5),
                Pos2::new(rect.max.x, rect.max.y - 0.5),
            ],
            Stroke::new(1.0_f32, t::HAIRLINE),
        );

        let badge = Rect::from_min_size(
            pos2(rect.min.x + 2.0, rect.min.y + 15.0),
            vec2(42.0, 42.0),
        );

        // Breathing ambient glow behind the badge.
        let wave = 0.5 + 0.5 * ((time as f32) * 1.5).sin();
        let glow = Color32::from_rgba_unmultiplied(30, 120, 255, (12.0 + 9.0 * wave) as u8);
        painter.circle_filled(badge.center(), 31.0 + wave * 3.0, glow);

        draw_logo(&painter, badge);

        let tx = badge.max.x + 15.0;
        painter.text(
            pos2(tx, badge.center().y - 9.0),
            Align2::LEFT_CENTER,
            "Silo Setup",
            FontId::new(17.0, strong_family()),
            t::TEXT,
        );
        painter.text(
            pos2(tx, badge.center().y + 11.5),
            Align2::LEFT_CENTER,
            format!("Version {VERSION} for Windows"),
            FontId::proportional(11.5),
            t::DIM,
        );
    }

    fn body(&mut self, ui: &mut egui::Ui, time: f64) {
        match self.page {
            Page::Welcome => self.page_welcome(ui),
            Page::Working => self.page_working(ui),
            Page::Finished => self.page_finished(ui, time),
            Page::Failed => self.page_failed(ui, time),
            Page::Confirm => self.page_confirm(ui),
            Page::Done => self.page_done(ui, time),
        }
    }

    fn page_welcome(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Install Silo");
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Personalize your iPhone from this PC. The wizard installs Silo for your account only — no administrator rights are required.",
            )
            .size(13.0)
            .color(t::DIM),
        );
        ui.add_space(14.0);

        ui.label(
            egui::RichText::new("Install to")
                .size(12.0)
                .color(t::DIM),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let browse_w = 78.0;
            let field_w = ui.available_width() - browse_w - 10.0;
            let (rect, _) = ui.allocate_exact_size(vec2(field_w, 32.0), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, egui::CornerRadius::same(8), t::FIELD);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(8),
                Stroke::new(1.0_f32, t::BORDER),
                StrokeKind::Inside,
            );
            painter.text(
                rect.min + vec2(12.0, rect.height() / 2.0),
                Align2::LEFT_CENTER,
                t::truncate_middle(&self.install_dir.to_string_lossy(), 46),
                FontId::monospace(11.5),
                t::TEXT,
            );
            ui.add_space(10.0);
            if button(ui, "Browse…", Kind::Secondary, true, browse_w).clicked() {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_directory(&self.install_dir)
                    .pick_folder()
                {
                    self.install_dir = dir;
                }
            }
        });
        ui.add_space(14.0);

        let mut desktop = self.desktop_shortcut;
        if checkbox(ui, Id::new("cb-desktop"), &mut desktop, "Create a desktop shortcut", true)
            .changed()
        {
            self.desktop_shortcut = desktop;
        }
        let mut launch = self.launch_after;
        if checkbox(ui, Id::new("cb-launch"), &mut launch, "Launch Silo when setup finishes", true)
            .changed()
        {
            self.launch_after = launch;
        }

        if self.install_dir.join(ops::APP_EXE).exists() {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("An existing installation will be updated.")
                    .size(12.0)
                    .color(t::ACCENT),
            );
        }
    }

    fn page_working(&self, ui: &mut egui::Ui) {
        let verb = match self.active_op {
            Op::Install => "Installing Silo",
            Op::Uninstall => "Removing Silo",
        };
        heading(ui, verb);
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("This will only take a moment — please keep this window open.")
                .size(13.0)
                .color(t::DIM),
        );
        ui.add_space(22.0);

        ui.horizontal(|ui| {
            ui.spinner();
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(self.step_label.clone())
                    .size(13.0)
                    .color(t::TEXT),
            );
        });
        ui.add_space(14.0);
        let now = ui.ctx().input(|i| i.time);
        progress_bar(ui, self.progress_shown, now);

        ui.add_space(8.0);
        let (row, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), 16.0),
            Sense::hover(),
        );
        let painter = ui.painter_at(row);
        painter.text(
            row.min + vec2(0.0, row.height() / 2.0),
            Align2::LEFT_CENTER,
            match self.active_op {
                Op::Install => format!("{}  →  {}", t::truncate_middle(&self.install_dir.to_string_lossy(), 42), ops::APP_EXE),
                Op::Uninstall => t::truncate_middle(&self.install_dir.to_string_lossy(), 56),
            },
            FontId::monospace(11.0),
            t::DIM,
        );
        painter.text(
            pos2(row.max.x, row.min.y + row.height() / 2.0),
            Align2::RIGHT_CENTER,
            format!("{}%", (self.progress_shown * 100.0).round()),
            FontId::monospace(11.5),
            t::TEXT,
        );
    }

    fn page_finished(&mut self, ui: &mut egui::Ui, time: f64) {
        status_row(
            ui,
            t::SUCCESS,
            Mark::Check,
            time - self.pop_at,
            "Installation complete",
            "Silo is ready to use.",
        );
        ui.add_space(18.0);

        let mut launch = self.launch_after;
        if checkbox(ui, Id::new("cb-finish"), &mut launch, "Launch Silo now", true).changed() {
            self.launch_after = launch;
        }
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(format!(
                "Installed to {}",
                t::truncate_middle(&self.install_dir.to_string_lossy(), 54)
            ))
            .size(11.5)
            .color(t::DIM),
        );
    }

    fn page_failed(&self, ui: &mut egui::Ui, time: f64) {
        status_row(
            ui,
            t::ERROR,
            Mark::Bang,
            time - self.pop_at,
            match self.active_op {
                Op::Install => "Installation failed",
                Op::Uninstall => "Uninstall failed",
            },
            "You can retry the operation.",
        );
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(self.error.clone())
                .size(12.5)
                .color(t::ERROR),
        );
    }

    fn page_confirm(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Uninstall Silo");
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "This removes the application, its shortcuts and the Apps & features entry from this computer.",
            )
            .size(13.0)
            .color(t::DIM),
        );
        ui.add_space(14.0);

        let mut remove = self.remove_data;
        if checkbox(
            ui,
            Id::new("cb-data"),
            &mut remove,
            "Also delete my settings, saved cards and backups",
            true,
        )
        .changed()
        {
            self.remove_data = remove;
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Your personal data is kept unless you tick the box above.")
                .size(11.5)
                .color(t::DIM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(format!(
                "Removes  {}",
                t::truncate_middle(&self.install_dir.to_string_lossy(), 52)
            ))
            .size(11.5)
            .color(t::DIM),
        );
    }

    fn page_done(&mut self, ui: &mut egui::Ui, time: f64) {
        status_row(
            ui,
            t::SUCCESS,
            Mark::Check,
            time - self.pop_at,
            "Silo has been removed",
            if self.data_removed {
                "The application and your personal data are gone."
            } else {
                "The application is gone — your personal data was kept."
            },
        );
    }

    // -------------------------------------------------------------- footer --

    fn footer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, time: f64) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let running = self.page == Page::Working;
            let (primary_label, primary_enabled) = match self.page {
                Page::Welcome => ("Install", true),
                Page::Working => (
                    match self.active_op {
                        Op::Install => "Installing…",
                        Op::Uninstall => "Removing…",
                    },
                    false,
                ),
                Page::Finished => ("Finish", true),
                Page::Failed => ("Retry", true),
                Page::Confirm => ("Uninstall", true),
                Page::Done => ("Close", true),
            };

            let primary_w = match self.page {
                Page::Working => 124.0,
                Page::Confirm => 124.0,
                _ => 116.0,
            };
            if button(ui, primary_label, Kind::Primary, primary_enabled, primary_w).clicked()
                && primary_enabled
            {
                self.primary(ctx, time);
            }
            ui.add_space(10.0);

            match self.page {
                Page::Welcome | Page::Confirm => {
                    if button(ui, "Cancel", Kind::Secondary, !running, 96.0).clicked() {
                        self.close(ctx);
                    }
                }
                Page::Working => {
                    let r = button(ui, "Cancel", Kind::Secondary, false, 96.0);
                    let _ = r;
                }
                Page::Finished => {
                    if button(ui, "Open folder", Kind::Secondary, true, 108.0).clicked() {
                        let target = self.install_dir.join(ops::APP_EXE);
                        let mut cmd = Command::new("explorer");
                        cmd.arg(format!("/select,{}", target.display()));
                        #[cfg(windows)]
                        {
                            use std::os::windows::process::CommandExt;
                            cmd.creation_flags(0x0800_0000);
                        }
                        let _ = cmd.spawn();
                    }
                    ui.add_space(10.0);
                    if button(ui, "Close", Kind::Secondary, true, 96.0).clicked() {
                        self.close(ctx);
                    }
                }
                Page::Failed => {
                    if button(ui, "Close", Kind::Secondary, true, 96.0).clicked() {
                        self.close(ctx);
                    }
                }
                Page::Done => {}
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    Primary,
    Secondary,
}

fn button(ui: &mut egui::Ui, label: &str, kind: Kind, enabled: bool, width: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
    let ctx = ui.ctx().clone();
    let hover_t = if enabled {
        ctx.animate_bool_responsive(response.id.with("hover"), response.hovered())
    } else {
        0.0
    };
    let press_t = if enabled {
        ctx.animate_bool_responsive(response.id.with("press"), response.is_pointer_button_down_on())
    } else {
        0.0
    };

    let painter = ui.painter_at(rect);
    let radius = egui::CornerRadius::same(9);
    let text_font = FontId::new(13.0, if matches!(kind, Kind::Primary) { strong_family() } else { FontFamily::Proportional });

    match kind {
        Kind::Primary => {
            let base = if enabled { t::ACCENT } else { t::fade(t::ACCENT, 0.38) };
            let hover_col = if enabled { t::ACCENT_HOVER } else { base };
            let mut fill = t::mix(base, hover_col, hover_t);
            fill = t::mix(fill, Color32::from_rgb(0, 90, 200), 0.25 * press_t);
            painter.rect_filled(rect, radius, fill);
            let text_col = if enabled { Color32::WHITE } else { Color32::from_white_alpha(160) };
            painter.text(rect.center(), Align2::CENTER_CENTER, label, text_font, text_col);
        }
        Kind::Secondary => {
            let hover_fill = if enabled {
                Color32::from_white_alpha(16)
            } else {
                Color32::TRANSPARENT
            };
            let fill = t::mix(Color32::TRANSPARENT, hover_fill, hover_t);
            let border = if enabled {
                t::mix(t::BORDER, Color32::from_white_alpha(90), hover_t)
            } else {
                t::fade(t::BORDER, 0.5)
            };
            if fill.a() > 0 {
                painter.rect_filled(rect, radius, fill);
            }
            painter.rect_stroke(rect, radius, Stroke::new(1.0_f32, border), StrokeKind::Inside);
            let text_col = if enabled { t::TEXT } else { t::fade(t::TEXT, 0.4) };
            painter.text(rect.center(), Align2::CENTER_CENTER, label, text_font, text_col);
        }
    }
    response
}

/// Animated checkbox row; the whole row is clickable.
fn checkbox(
    ui: &mut egui::Ui,
    id: Id,
    checked: &mut bool,
    label: &str,
    enabled: bool,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        vec2(ui.available_width(), 26.0),
        Sense::click(),
    );
    if enabled && response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let t_anim = ui
        .ctx()
        .animate_bool_responsive(id, *checked && enabled);

    let side = 17.0;
    let box_rect = Rect::from_center_size(
        pos2(rect.min.x + side / 2.0 + 1.0, rect.center().y),
        vec2(side, side),
    );
    let painter = ui.painter_at(rect);

    let fill = t::mix(Color32::TRANSPARENT, t::ACCENT, t_anim);
    let border = t::mix(
        t::BORDER,
        t::ACCENT,
        if enabled { t_anim } else { 0.0 },
    );
    painter.rect_filled(box_rect, egui::CornerRadius::same(5), fill);
    painter.rect_stroke(
        box_rect,
        egui::CornerRadius::same(5),
        Stroke::new(1.2_f32, border),
        StrokeKind::Inside,
    );

    // Check mark draws itself in as the box fills.
    let u = ((t_anim - 0.35) / 0.65).clamp(0.0, 1.0);
    if u > 0.0 {
        let c = box_rect.center();
        let p1 = pos2(c.x - 4.6, c.y - 0.2);
        let p2 = pos2(c.x - 1.4, c.y + 3.2);
        let p3 = pos2(c.x + 5.0, c.y - 3.8);
        let u1 = (u * 2.0).clamp(0.0, 1.0);
        let u2 = (u * 2.0 - 1.0).clamp(0.0, 1.0);
        let lerp = |a: Pos2, b: Pos2, k: f32| Pos2::new(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
        painter.line_segment([p1, lerp(p1, p2, u1)], Stroke::new(2.0_f32, Color32::WHITE));
        if u2 > 0.0 {
            painter.line_segment([p2, lerp(p2, p3, u2)], Stroke::new(2.0_f32, Color32::WHITE));
        }
    }

    let label_col = if enabled { t::TEXT } else { t::DIM };
    painter.text(
        pos2(box_rect.max.x + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t::mix(label_col, t::ACCENT, if enabled { hover_t(&response) * 0.25 } else { 0.0 }),
    );
    response
}

fn hover_t(response: &Response) -> f32 {
    if response.hovered() {
        1.0
    } else {
        0.0
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(16.0)
            .color(t::TEXT)
            .strong(),
    );
}

fn progress_bar(ui: &mut egui::Ui, value: f32, time: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::same(5), t::ELEVATED);

    let w = rect.width() * value.clamp(0.0, 1.0);
    if w > 2.0 {
        let fill = Rect::from_min_size(rect.min, vec2(w, rect.height()));
        painter.rect_filled(fill, egui::CornerRadius::same(5), t::ACCENT);

        // Sweeping highlight for a lively feel.
        let cycle = ((time as f32) * 0.75) % 1.0;
        let x = fill.min.x + cycle * (fill.width() + 80.0) - 40.0;
        let band = Rect::from_min_max(
            pos2(x, fill.min.y),
            pos2(x + 18.0, fill.max.y),
        )
        .intersect(fill);
        if band.width() > 1.0 {
            painter.rect_filled(band, 0.0, Color32::from_white_alpha(64));
        }
    }
}

#[derive(Clone, Copy)]
enum Mark {
    Check,
    Bang,
}

/// Animated success/failure circle with an overshoot pop.
fn status_row(
    ui: &mut egui::Ui,
    color: Color32,
    mark: Mark,
    elapsed: f64,
    title: &str,
    subtitle: &str,
) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (icon_rect, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
        let t = ((elapsed / 0.5) as f32).clamp(0.0, 1.0);
        let scale = t::ease_out_back(t);
        if scale > 0.01 {
            let painter = ui.painter_at(icon_rect);
            let c = icon_rect.center();
            let r = 22.0 * scale;
            let alpha = (t * 2.0).clamp(0.0, 1.0);
            painter.circle_filled(c, r + 7.0, t::fade(color, 0.16 * alpha));
            painter.circle_filled(c, r, t::fade(color, alpha));

            let u = ((t - 0.35) / 0.65).clamp(0.0, 1.0);
            if u > 0.0 {
                match mark {
                    Mark::Check => {
                        let p1 = pos2(c.x - 7.6 * scale, c.y + 0.4 * scale);
                        let p2 = pos2(c.x - 2.2 * scale, c.y + 5.6 * scale);
                        let p3 = pos2(c.x + 8.4 * scale, c.y - 6.6 * scale);
                        let u1 = (u * 2.0).clamp(0.0, 1.0);
                        let u2 = (u * 2.0 - 1.0).clamp(0.0, 1.0);
                        let lerp = |a: Pos2, b: Pos2, k: f32| {
                            Pos2::new(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k)
                        };
                        painter.line_segment(
                            [p1, lerp(p1, p2, u1)],
                            Stroke::new(3.0 * scale, Color32::WHITE),
                        );
                        if u2 > 0.0 {
                            painter.line_segment(
                                [p2, lerp(p2, p3, u2)],
                                Stroke::new(3.0 * scale, Color32::WHITE),
                            );
                        }
                    }
                    Mark::Bang => {
                        painter.text(
                            pos2(c.x, c.y + 1.0),
                            Align2::CENTER_CENTER,
                            "!",
                            FontId::new(24.0 * scale, strong_family()),
                            t::fade(Color32::WHITE, u),
                        );
                    }
                }
            }
        }

        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.add_space(3.0);
            ui.label(
                egui::RichText::new(title)
                    .size(16.0)
                    .color(t::TEXT)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(subtitle).size(13.0).color(t::DIM));
        });
    });
    ui.add_space(4.0);
}

/// The black wallet badge (mirrors the app's sidebar logo).
fn draw_logo(painter: &egui::Painter, rect: Rect) {
    let k = rect.width() / 36.0;
    painter.rect_filled(
        rect,
        egui::CornerRadius::same((9.0 * k) as u8),
        Color32::from_rgb(46, 46, 48),
    );
    let c = rect.center();
    let angle: f32 = -12f32.to_radians();
    let (s, co) = angle.sin_cos();
    let half = vec2(8.7 * k, 6.3 * k);
    let back_center = c + vec2(0.0, -1.0 * k);
    let back: Vec<Pos2> = [
        vec2(-half.x, -half.y),
        vec2(half.x, -half.y),
        vec2(half.x, half.y),
        vec2(-half.x, half.y),
    ]
    .iter()
    .map(|pt| {
        pos2(
            back_center.x + pt.x * co - pt.y * s,
            back_center.y + pt.x * s + pt.y * co,
        )
    })
    .collect();
    painter.add(egui::Shape::Path(egui::epaint::PathShape {
        points: back,
        closed: true,
        fill: Color32::from_white_alpha(125),
        stroke: Stroke::NONE.into(),
    }));
    let front =
        Rect::from_center_size(c + vec2(0.0, 1.0 * k), vec2(19.0 * k, 13.5 * k));
    painter.rect_filled(
        front,
        egui::CornerRadius::same((3.0 * k) as u8),
        Color32::WHITE,
    );
    painter.rect_filled(
        Rect::from_min_max(
            pos2(front.min.x, front.min.y + 3.9 * k),
            pos2(front.max.x, front.min.y + 8.1 * k),
        ),
        0.0,
        Color32::from_rgb(26, 26, 28),
    );
}

// ---------------------------------------------------------------------------
// eframe entry point
// ---------------------------------------------------------------------------

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        InstallerApp::update(self, ctx, frame)
    }
}
