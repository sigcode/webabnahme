//! Step 4: confirm results, decide and export the acceptance report.

use crate::app::Action;
use crate::model::*;
use crate::store;
use crate::theme::*;
use eframe::egui::{self, RichText, Stroke, Vec2};
use std::path::PathBuf;

pub fn show(
    ui: &mut egui::Ui,
    p: &mut Project,
    busy: bool,
    job_kind: Option<crate::claude::JobKind>,
    actions: &mut Vec<Action>,
    last: &Option<PathBuf>,
    confirm_delete: &mut bool,
) {
    let (ok, fail, blk, man, open) = p.counts();
    let total = p.checklist.len();
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 4.0 * 8.0) / 5.0;
        for (n, s) in [(ok, Status::Passed), (fail, Status::Failed), (blk, Status::Blocked), (man, Status::Manual), (open, Status::Open)] {
            ui.allocate_ui(Vec2::new(w, 70.0), |ui| {
                egui::Frame::none()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_width(w - 24.0);
                        ui.label(RichText::new(format!("{n}")).size(24.0).strong().color(status_color(s)));
                        ui.label(RichText::new(format!("{} {}", s.icon(), s.label())).color(MUTED).size(12.5));
                    });
            });
        }
    });
    ui.add_space(4.0);
    if total > 0 {
        ui.add(
            egui::ProgressBar::new(ok as f32 / total as f32)
                .text(format!("{ok} von {total} Prüfpunkten bestanden"))
                .desired_width(ui.available_width()),
        );
    }
    ui.add_space(8.0);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        if !p.claude_summary.trim().is_empty() || p.claude_recommendation.is_some() {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    section_title(ui, "Bewertung von Claude", "");
                    if let Some(d) = p.claude_recommendation {
                        ui.label(RichText::new(format!("Empfehlung: {}", d.label())).strong().color(decision_color(d)));
                    }
                });
                ui.add(egui::Label::new(RichText::new(plain(&p.claude_summary)).color(TEXT)).wrap(true));
                if !p.claude_conditions.trim().is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("Auflagen / offene Punkte").color(MUTED).size(12.5));
                    ui.add(egui::Label::new(plain(&p.claude_conditions)).wrap(true));
                }
                ui.add_space(4.0);
                if ui.button("Vorschlag in die Abnahme übernehmen").clicked() {
                    if let Some(d) = p.claude_recommendation {
                        p.signoff.decision = d;
                    }
                    let c = if p.claude_conditions.trim().is_empty() { &p.claude_summary } else { &p.claude_conditions };
                    p.signoff.comment = plain(c);
                    p.signoff.date = store::now_string();
                }
            });
            ui.add_space(10.0);
        }
        if p.checklist.iter().any(|i| i.status == Status::Blocked) {
            crate::tab_blocked::show(ui, p, busy, actions);
            ui.add_space(10.0);
        }
        card(ui, |ui| {
            section_title(
                ui,
                "Prüfergebnisse bestätigen",
                "Ergebnisse von Claude durchsehen, bei Bedarf korrigieren und per Haken bestätigen.",
            );
            egui::Grid::new("signgrid").num_columns(5).striped(true).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(RichText::new("ID").color(MUTED));
                ui.label(RichText::new("Prüfpunkt").color(MUTED));
                ui.label(RichText::new("Status").color(MUTED));
                ui.label(RichText::new("Bestätigt").color(MUTED));
                ui.label(RichText::new("Anmerkung Prüfer").color(MUTED));
                ui.end_row();
                for (k, it) in p.checklist.iter_mut().enumerate() {
                    ui.label(RichText::new(&it.id).monospace());
                    ui.vertical(|ui| {
                        ui.set_max_width(330.0);
                        ui.label(&it.title);
                        if !it.auto_note.is_empty() {
                            ui.label(RichText::new(&it.auto_note).color(MUTED).size(12.0));
                        }
                        if it.status == Status::Blocked && !it.blocker.trim().is_empty() && it.blocker.trim() != it.auto_note.trim() {
                            ui.label(RichText::new(format!("Blockiert weil: {}", it.blocker)).color(status_color(Status::Blocked)).size(12.0));
                        }
                    });
                    egui::ComboBox::from_id_source(format!("st{k}"))
                        .selected_text(RichText::new(format!("{} {}", it.status.icon(), it.status.label())).color(status_color(it.status)))
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for s in Status::all() {
                                ui.selectable_value(&mut it.status, s, RichText::new(format!("{} {}", s.icon(), s.label())).color(status_color(s)));
                            }
                        });
                    ui.checkbox(&mut it.confirmed, "");
                    ui.add(egui::TextEdit::singleline(&mut it.reviewer_note).hint_text("optional").desired_width(220.0));
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            if ui.button("Alle bestandenen bestätigen").clicked() {
                for it in p.checklist.iter_mut() {
                    if it.status == Status::Passed {
                        it.confirmed = true;
                    }
                }
            }
        });
        ui.add_space(10.0);
        crate::tab_fix::show(ui, p, busy, job_kind == Some(crate::claude::JobKind::Brief), actions);
        ui.add_space(10.0);
        card(ui, |ui| {
            section_title(ui, "Abnahme-Entscheidung", "");
            egui::Grid::new("decision").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                ui.label("Prüfer/in");
                ui.add(egui::TextEdit::singleline(&mut p.signoff.reviewer).hint_text("Name").desired_width(260.0));
                ui.end_row();
                ui.label("Entscheidung");
                ui.horizontal_wrapped(|ui| {
                    for d in [Decision::Accepted, Decision::AcceptedWithConditions, Decision::Rejected, Decision::Pending] {
                        let col = decision_color(d);
                        if ui.selectable_value(&mut p.signoff.decision, d, RichText::new(d.label()).color(col)).changed() {
                            p.signoff.date = store::now_string();
                        }
                    }
                });
                ui.end_row();
                ui.label("Kommentar / Auflagen");
                ui.add(egui::TextEdit::multiline(&mut p.signoff.comment).desired_rows(3).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label("Datum");
                ui.label(if p.signoff.date.is_empty() { "–".to_string() } else { p.signoff.date.clone() });
                ui.end_row();
            });
            if fail > 0 && p.signoff.decision == Decision::Accepted {
                ui.label(RichText::new(format!("⚠ Achtung: {fail} Prüfpunkte sind fehlgeschlagen.")).color(status_color(Status::Blocked)));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if primary(ui, "Abnahmeprotokoll erstellen", true) {
                    actions.push(Action::Export);
                }
                if let Some(f) = last {
                    if ui.button("Letzten Bericht öffnen").clicked() {
                        actions.push(Action::Open(f.to_string_lossy().to_string()));
                    }
                    if ui.button("Im Finder zeigen").clicked() {
                        actions.push(Action::Reveal(f.to_string_lossy().to_string()));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if *confirm_delete {
                        if ui.button("Abbrechen").clicked() {
                            *confirm_delete = false;
                        }
                        if ui.button(RichText::new("Wirklich löschen").color(status_color(Status::Failed))).clicked() {
                            *confirm_delete = false;
                            actions.push(Action::DeleteProject);
                        }
                    } else if ui.button(RichText::new("Projekt löschen").color(MUTED)).clicked() {
                        *confirm_delete = true;
                    }
                });
            });
        });
    });
}

fn decision_color(d: Decision) -> egui::Color32 {
    match d {
        Decision::Accepted => status_color(Status::Passed),
        Decision::AcceptedWithConditions => status_color(Status::Blocked),
        Decision::Rejected => status_color(Status::Failed),
        Decision::Pending => MUTED,
    }
}
