//! Sign-off page section: tick the failed checks and let Claude write a
//! copy-ready bullet-point brief for a coding agent that fixes them.

use crate::app::Action;
use crate::fixbrief::{candidates, is_ticked, ticked_ids};
use crate::model::*;
use crate::theme::*;
use eframe::egui::{self, RichText};

pub fn show(ui: &mut egui::Ui, p: &mut Project, busy: bool, writing: bool, actions: &mut Vec<Action>) {
    card(ui, |ui| {
        section_title(
            ui,
            "Auftrag für den Fix-Agenten",
            "Angehakte Fehler schreibt Claude als stichpunktartige Vorgabe für einen Agenten auf – was nicht ging und wie getestet wurde. \
Nicht angehakte Punkte kommen im Text nicht vor.",
        );
        let cands = candidates(p);
        if cands.is_empty() {
            ui.label(RichText::new("Keine fehlgeschlagenen oder blockierten Prüfpunkte – nichts zu beheben.").color(MUTED));
            return;
        }
        for &k in &cands {
            let it = &mut p.checklist[k];
            let mut on = is_ticked(it);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut on, "").changed() {
                    it.fix_selected = Some(on);
                }
                pill(ui, it.status);
                ui.label(RichText::new(&it.id).monospace().color(MUTED));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(&it.title).color(if on { TEXT } else { MUTED }));
                    if !it.auto_note.is_empty() {
                        ui.add(egui::Label::new(RichText::new(&it.auto_note).color(MUTED).size(12.0)).wrap(true));
                    }
                });
            });
        }
        let ticked = ticked_ids(p);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let label = if writing { "Claude schreibt …".to_string() } else { format!("Vorgabe mit Claude schreiben ({})", ticked.len()) };
            if primary(ui, &label, !busy && !ticked.is_empty()) {
                actions.push(Action::FixBrief);
            }
            if writing {
                ui.spinner();
            }
            if ui.button("Alle Fehler anhaken").clicked() {
                for &k in &cands {
                    let it = &mut p.checklist[k];
                    it.fix_selected = Some(it.status == Status::Failed);
                }
            }
            if ui.button("Alle anhaken").clicked() {
                for &k in &cands {
                    p.checklist[k].fix_selected = Some(true);
                }
            }
            if ui.button("Keine").clicked() {
                for &k in &cands {
                    p.checklist[k].fix_selected = Some(false);
                }
            }
        });

        if p.fix_brief.trim().is_empty() {
            return;
        }
        ui.add_space(6.0);
        let mut a = p.fix_brief_ids.clone();
        let mut b = ticked.clone();
        a.sort();
        b.sort();
        let stale = a != b;
        if stale {
            ui.label(
                RichText::new(
                    "⚠ Die Auswahl hat sich geändert. Der Text unten passt nicht mehr dazu – bitte neu schreiben lassen, \
damit nur die angehakten Fehler enthalten sind.",
                )
                .color(status_color(Status::Blocked)),
            );
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Vorgabe für den Agenten").strong());
            ui.label(RichText::new(format!("enthält: {}", p.fix_brief_ids.join(", "))).color(MUTED).size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let copy = ui
                    .add_enabled(!stale, egui::Button::new("In Zwischenablage kopieren"))
                    .on_disabled_hover_text("Erst neu schreiben lassen – die Auswahl hat sich geändert.");
                if copy.clicked() {
                    let text = p.fix_brief.clone();
                    ui.output_mut(|o| o.copied_text = text);
                    ui.ctx().memory_mut(|m| m.data.insert_temp(egui::Id::new("fix_copied"), true));
                }
                let copied = ui.ctx().memory(|m| m.data.get_temp::<bool>(egui::Id::new("fix_copied")).unwrap_or(false));
                if copied && !stale {
                    ui.label(RichText::new("✔ kopiert").color(status_color(Status::Passed)));
                }
            });
        });
        let resp = ui.add(
            egui::TextEdit::multiline(&mut p.fix_brief)
                .font(egui::TextStyle::Monospace)
                .desired_rows(14)
                .desired_width(f32::INFINITY),
        );
        if resp.changed() {
            ui.ctx().memory_mut(|m| m.data.insert_temp(egui::Id::new("fix_copied"), false));
        }
    });
}
