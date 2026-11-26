//! Step 3: start a test run and follow Claude Code live.

use crate::app::Action;
use crate::claude::{JobKind, LogKind};
use crate::model::*;
use crate::theme::*;
use eframe::egui::{self, Color32, RichText, Vec2};

pub fn show(
    ui: &mut egui::Ui,
    p: &Project,
    busy: bool,
    kind: Option<JobKind>,
    running_ids: &[String],
    log: &[(LogKind, String)],
    actions: &mut Vec<Action>,
) {
    let total = running_ids.len().max(1);
    let done = p
        .checklist
        .iter()
        .filter(|i| running_ids.contains(&i.id) && i.status != Status::Running)
        .count();
    card(ui, |ui| {
        ui.horizontal(|ui| {
            if busy {
                ui.spinner();
                let t = if kind == Some(JobKind::Test) {
                    format!("Test läuft in {} – {done} von {total} Prüfpunkten bewertet", p.browser.label())
                } else {
                    "Arbeitet …".to_string()
                };
                ui.label(RichText::new(t).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⏹ Abbrechen").clicked() {
                        actions.push(Action::Stop);
                    }
                });
            } else {
                let all: Vec<String> = p.checklist.iter().filter(|i| i.status != Status::Manual).map(|i| i.id.clone()).collect();
                if primary(ui, &format!("▶ Alle automatisch testen ({})", all.len()), !all.is_empty()) {
                    actions.push(Action::Test(all));
                }
                let sel: Vec<String> = p.checklist.iter().filter(|i| i.selected).map(|i| i.id.clone()).collect();
                if ui.add_enabled(!sel.is_empty(), egui::Button::new(format!("Ausgewählte ({})", sel.len()))).clicked() {
                    actions.push(Action::Test(sel));
                }
                ui.label(
                    RichText::new(format!(
                        "Claude Code steuert {} ({}) über Playwright.",
                        p.browser.label(),
                        if p.headless { "unsichtbar" } else { "sichtbar" }
                    ))
                    .color(MUTED)
                    .size(12.5),
                );
            }
        });
        if busy && kind == Some(JobKind::Test) {
            ui.add(egui::ProgressBar::new(done as f32 / total as f32).desired_width(ui.available_width()));
        }
    });
    ui.add_space(10.0);
    let h = ui.available_height();
    ui.horizontal_top(|ui| {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(Vec2::new(w * 0.64, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            card(ui, |ui| {
                section_title(ui, "Protokoll", "Was Claude Code gerade im Browser macht.");
                egui::ScrollArea::vertical()
                    .id_source("log")
                    .stick_to_bottom(true)
                    .auto_shrink([false; 2])
                    .max_height((h - 90.0).max(100.0))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        if log.is_empty() {
                            ui.label(RichText::new("Noch keine Ausgabe. Starte einen Testlauf.").color(MUTED));
                        }
                        for (k, t) in log {
                            let (prefix, col) = match k {
                                LogKind::Info => ("·", MUTED),
                                LogKind::Text => ("»", TEXT),
                                LogKind::Tool => ("⚙", ACCENT),
                                LogKind::ToolResult => ("   ", Color32::from_gray(150)),
                                LogKind::Error => ("!", status_color(Status::Failed)),
                                LogKind::Result => ("★", status_color(Status::Passed)),
                            };
                            // one wrapped label per entry, laid out top-down so long
                            // text stays inside the card
                            let rt = if *k == LogKind::Tool || *k == LogKind::ToolResult {
                                RichText::new(format!("{prefix} {t}")).monospace().color(col)
                            } else {
                                RichText::new(format!("{prefix} {}", plain(t))).color(col)
                            };
                            ui.add(egui::Label::new(rt).wrap(true));
                        }
                    });
            });
        });
        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), h), egui::Layout::top_down(egui::Align::Min), |ui| {
            card(ui, |ui| {
                section_title(ui, "Prüfpunkte", "");
                egui::ScrollArea::vertical().id_source("runitems").max_height(h * 0.45).show(ui, |ui| {
                    for it in &p.checklist {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(it.status.icon()).color(status_color(it.status)).strong());
                            ui.label(RichText::new(&it.id).monospace().color(MUTED));
                            ui.label(RichText::new(&it.title).size(12.5));
                        });
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                section_title(ui, "Bisherige Läufe", "");
                if p.runs.is_empty() {
                    ui.label(RichText::new("Noch keine.").color(MUTED));
                }
                egui::ScrollArea::vertical().id_source("runs").max_height(200.0).show(ui, |ui| {
                    for r in p.runs.iter().rev() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&r.started).size(12.5));
                            ui.label(RichText::new(format!("✔{}", r.passed)).color(status_color(Status::Passed)));
                            ui.label(RichText::new(format!("✖{}", r.failed)).color(status_color(Status::Failed)));
                            ui.label(RichText::new(format!("⚠{}", r.blocked)).color(status_color(Status::Blocked)));
                            if ui.small_button("Ordner").clicked() {
                                actions.push(Action::Reveal(r.log_file.clone()));
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Auswerten").small())
                                .on_hover_text("Ergebnisse und Bewertung aus dem Protokoll dieses Laufs in Checkliste und Abnahme übernehmen")
                                .clicked()
                            {
                                actions.push(Action::Reextract(r.log_file.clone(), None));
                            }
                        });
                    }
                });
            });
        });
    });
}
