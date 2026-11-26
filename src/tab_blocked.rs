//! Sign-off page section: why checks are blocked, and how to test them again
//! (with a note for Claude and/or "test anyway").

use crate::app::Action;
use crate::model::*;
use crate::theme::*;
use eframe::egui::{self, RichText};

pub fn show(ui: &mut egui::Ui, p: &mut Project, busy: bool, actions: &mut Vec<Action>) {
    let blocked: Vec<usize> = (0..p.checklist.len()).filter(|&k| p.checklist[k].status == Status::Blocked).collect();
    if blocked.is_empty() {
        return;
    }
    let last_log = p.runs.last().map(|r| r.log_file.clone()).filter(|l| !l.is_empty());
    card(ui, |ui| {
        section_title(
            ui,
            &format!("Blockierte Prüfpunkte ({})", blocked.len()),
            "Diese Punkte konnte Claude nicht durchführen. Hier steht warum. Gib einen Hinweis für den nächsten Versuch \
und/oder lass sie trotzdem testen – dann darf Claude sie nicht erneut wegen Abhängigkeiten blockieren.",
        );
        let unexplained: Vec<String> = blocked
            .iter()
            .filter(|&&k| p.checklist[k].blocker.trim().is_empty())
            .map(|&k| p.checklist[k].id.clone())
            .collect();
        if !unexplained.is_empty() {
            if let Some(log) = &last_log {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("Für {} Punkte fehlt die Begründung.", unexplained.len())).color(status_color(Status::Blocked)),
                    );
                    if ui
                        .add_enabled(!busy, egui::Button::new("Gründe aus dem letzten Lauf ermitteln"))
                        .on_hover_text("Claude liest das Protokoll des letzten Testlaufs und trägt ein, woran diese Punkte gescheitert sind.")
                        .clicked()
                    {
                        actions.push(Action::Reextract(log.clone(), Some(unexplained.clone())));
                    }
                });
                ui.add_space(4.0);
            }
        }
        for &k in &blocked {
            let it = &mut p.checklist[k];
            soft(ui, |ui| {
                ui.horizontal(|ui| {
                    pill(ui, it.status);
                    ui.label(RichText::new(&it.id).monospace().color(MUTED));
                    ui.label(RichText::new(&it.title).strong().color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "↻ Erneut testen", !busy) {
                            actions.push(Action::Test(vec![it.id.clone()]));
                        }
                    });
                });
                let why = if !it.blocker.trim().is_empty() {
                    it.blocker.clone()
                } else if !it.auto_note.trim().is_empty() {
                    it.auto_note.clone()
                } else {
                    "Kein Grund angegeben.".to_string()
                };
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new("Grund:").color(status_color(Status::Blocked)).strong());
                    ui.add(egui::Label::new(RichText::new(plain(&why)).color(TEXT)).wrap(true));
                });
                if !it.blocker.trim().is_empty() && !it.auto_note.trim().is_empty() && it.auto_note.trim() != it.blocker.trim() {
                    ui.add(egui::Label::new(RichText::new(format!("Beobachtung: {}", it.auto_note)).color(MUTED).size(12.5)).wrap(true));
                }
                ui.add(
                    egui::TextEdit::multiline(&mut it.retest_hint)
                        .hint_text("Hinweis an Claude für den nächsten Versuch, z. B. „Mailversand ist jetzt aktiv“, „Nutze Benutzer test2@…“, „Lege den Termin vorher selbst an“")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(
                    &mut it.force,
                    "Trotzdem testen (erzwingen): Vorbedingungen selbst herstellen, nicht wegen Abhängigkeiten blockieren",
                );
            });
        }
        ui.add_space(4.0);
        let ids: Vec<String> = blocked.iter().map(|&k| p.checklist[k].id.clone()).collect();
        ui.horizontal(|ui| {
            if primary(ui, &format!("↻ Alle blockierten erneut testen ({})", ids.len()), !busy) {
                actions.push(Action::Test(ids.clone()));
            }
            if ui.button("Bei allen „Trotzdem testen“ setzen").clicked() {
                for &k in &blocked {
                    p.checklist[k].force = true;
                }
            }
        });
    });
}
