use crate::model::Status;
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(59, 110, 245);
pub const SIDEBAR: Color32 = Color32::from_rgb(28, 34, 50);
pub const BG: Color32 = Color32::from_rgb(244, 246, 250);
pub const CARD: Color32 = Color32::WHITE;
pub const BORDER: Color32 = Color32::from_rgb(222, 226, 236);
pub const MUTED: Color32 = Color32::from_rgb(110, 118, 135);
pub const TEXT: Color32 = Color32::from_rgb(29, 36, 51);
pub const SOFT: Color32 = Color32::from_rgb(248, 249, 252);

pub fn status_color(s: Status) -> Color32 {
    match s {
        Status::Open => Color32::from_rgb(130, 138, 155),
        Status::Running => ACCENT,
        Status::Passed => Color32::from_rgb(26, 143, 76),
        Status::Failed => Color32::from_rgb(198, 47, 58),
        Status::Blocked => Color32::from_rgb(196, 125, 20),
        Status::Manual => Color32::from_rgb(98, 82, 200),
    }
}

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = Color32::from_rgb(250, 251, 253);
    v.selection.bg_fill = ACCENT.linear_multiply(0.25);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_fill = Color32::from_rgb(236, 239, 245);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(236, 239, 245);
    v.widgets.inactive.rounding = Rounding::same(6.0);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(208, 213, 226));
    v.widgets.hovered.rounding = Rounding::same(6.0);
    v.widgets.active.rounding = Rounding::same(6.0);
    v.window_rounding = Rounding::same(10.0);
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_style(style);
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn soft<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(SOFT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn primary(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    let b = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(if enabled { ACCENT } else { Color32::from_rgb(160, 175, 215) })
        .min_size(Vec2::new(0.0, 30.0));
    ui.add_enabled(enabled, b).clicked()
}

pub fn pill(ui: &mut egui::Ui, s: Status) {
    let c = status_color(s);
    egui::Frame::none()
        .fill(c.linear_multiply(0.13))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(format!("{} {}", s.icon(), s.label())).color(c).size(12.5).strong());
        });
}

pub fn section_title(ui: &mut egui::Ui, t: &str, sub: &str) {
    ui.label(RichText::new(t).size(16.0).strong().color(TEXT));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(MUTED).size(12.5));
    }
    ui.add_space(4.0);
}

/// Strip the most common Markdown marks so Claude's text reads cleanly.
pub fn plain(md: &str) -> String {
    md.lines()
        .map(|l| {
            let l = l.trim_end();
            let l = l.trim_start_matches('#').trim_start();
            l.replace("**", "").replace('`', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
