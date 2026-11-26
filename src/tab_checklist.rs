//! Step 2: the derived checklist, grouped by use case, editable.

use crate::app::{Action, Filter, Tab};
use crate::model::*;
use crate::theme::*;
use eframe::egui::{self, RichText, Vec2};
use std::collections::HashSet;

pub fn show(
    ui: &mut egui::Ui,
    p: &mut Project,
    busy: bool,
    actions: &mut Vec<Action>,
    filter: &mut Filter,
    expanded: &mut HashSet<String>,
) {
    toolbar(ui, p, busy, actions, filter, expanded);

    if p.checklist.is_empty() {
        card(ui, |ui| {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Noch keine Checkliste").size(18.0).strong());
                ui.label(
                    RichText::new("Beschreibe unter 'Vorgabe' deine Use Cases und lass Claude daraus Prüfpunkte ableiten.").color(MUTED),
                );
                ui.add_space(8.0);
                if primary(ui, "Zur Vorgabe", true) {
                    actions.push(Action::Goto(Tab::Spec));
                }
            });
            ui.add_space(30.0);
        });
        return;
    }

    let mut groups: Vec<String> = Vec::new();
    for i in &p.checklist {
        if !groups.contains(&i.use_case) {
            groups.push(i.use_case.clone());
        }
    }
    let mut delete = None;
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for g in groups {
            let items: Vec<usize> = (0..p.checklist.len())
                .filter(|&k| p.checklist[k].use_case == g)
                .filter(|&k| match *filter {
                    Filter::All => true,
                    Filter::Only(Status::Open) => matches!(p.checklist[k].status, Status::Open | Status::Running),
                    Filter::Only(s) => p.checklist[k].status == s,
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            let passed = items.iter().filter(|&&k| p.checklist[k].status == Status::Passed).count();
            ui.horizontal(|ui| {
                ui.label(RichText::new(if g.is_empty() { "Ohne Use Case" } else { g.as_str() }).size(15.0).strong());
                ui.label(RichText::new(format!("{passed} von {} bestanden", items.len())).color(MUTED).size(12.0));
            });
            card(ui, |ui| {
                for (row, k) in items.iter().enumerate() {
                    if row > 0 {
                        ui.separator();
                    }
                    if item_row(ui, &mut p.checklist[*k], *k, busy, actions, expanded) {
                        delete = Some(*k);
                    }
                }
            });
            ui.add_space(12.0);
        }
    });
    if let Some(k) = delete {
        p.checklist.remove(k);
    }
}

fn toolbar(
    ui: &mut egui::Ui,
    p: &mut Project,
    busy: bool,
    actions: &mut Vec<Action>,
    filter: &mut Filter,
    expanded: &mut HashSet<String>,
) {
    let selected: Vec<String> = p.checklist.iter().filter(|i| i.selected).map(|i| i.id.clone()).collect();
    ui.horizontal(|ui| {
        if primary(ui, &format!("▶ Ausgewählte testen ({})", selected.len()), !busy && !selected.is_empty()) {
            actions.push(Action::Test(selected.clone()));
        }
        let redo: Vec<String> = p
            .checklist
            .iter()
            .filter(|i| matches!(i.status, Status::Failed | Status::Blocked | Status::Open))
            .map(|i| i.id.clone())
            .collect();
        if ui
            .add_enabled(!busy && !redo.is_empty(), egui::Button::new(format!("↻ Offene & fehlgeschlagene ({})", redo.len())))
            .clicked()
        {
            actions.push(Action::Test(redo));
        }
        ui.separator();
        if ui.button("Alle wählen").clicked() {
            for i in p.checklist.iter_mut() {
                i.selected = true;
            }
        }
        if ui.button("Keine").clicked() {
            for i in p.checklist.iter_mut() {
                i.selected = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(!busy, egui::Button::new("Neu ableiten")).clicked() {
                actions.push(Action::Goto(Tab::Spec));
            }
            if ui.button("+ Prüfpunkt").clicked() {
                let uc = p.use_cases.first().map(|u| u.title.clone()).unwrap_or_default();
                let mut n = p.checklist.len() + 1;
                while p.checklist.iter().any(|c| c.id == format!("M-{n:02}")) {
                    n += 1;
                }
                let id = format!("M-{n:02}");
                let mut c = CheckItem::new(id.clone(), uc);
                c.title = "Neuer Prüfpunkt".into();
                p.checklist.push(c);
                expanded.insert(id);
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let all = p.checklist.len();
        if ui.selectable_label(*filter == Filter::All, format!("Alle {all}")).clicked() {
            *filter = Filter::All;
        }
        for s in Status::all() {
            let n = p
                .checklist
                .iter()
                .filter(|i| i.status == s || (s == Status::Open && i.status == Status::Running))
                .count();
            let t = RichText::new(format!("{} {} {}", s.icon(), s.label(), n)).color(status_color(s));
            if ui.selectable_label(*filter == Filter::Only(s), t).clicked() {
                *filter = Filter::Only(s);
            }
        }
    });
    ui.add_space(6.0);
}

/// One checklist row; returns true if the user asked to delete it.
fn item_row(
    ui: &mut egui::Ui,
    it: &mut CheckItem,
    k: usize,
    busy: bool,
    actions: &mut Vec<Action>,
    expanded: &mut HashSet<String>,
) -> bool {
    let mut delete = false;
    let open = expanded.contains(&it.id);
    ui.horizontal(|ui| {
        ui.checkbox(&mut it.selected, "");
        ui.allocate_ui_with_layout(Vec2::new(150.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(150.0);
            pill(ui, it.status);
        });
        ui.label(RichText::new(&it.id).monospace().color(MUTED));
        ui.label(RichText::new(&it.title).strong().color(TEXT));
        let pr_col = match it.priority.as_str() {
            "hoch" => status_color(Status::Failed),
            "niedrig" => MUTED,
            _ => status_color(Status::Blocked),
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(if open { "Schließen" } else { "Details" }).on_hover_text("Details bearbeiten").clicked() {
                if open {
                    expanded.remove(&it.id);
                } else {
                    expanded.insert(it.id.clone());
                }
            }
            if ui.add_enabled(!busy, egui::Button::new("▶").small()).on_hover_text("Nur diesen Punkt testen").clicked() {
                actions.push(Action::Test(vec![it.id.clone()]));
            }
            ui.label(RichText::new(&it.priority).color(pr_col).size(12.0));
        });
    });
    if !it.auto_note.is_empty() && !open {
        ui.horizontal(|ui| {
            ui.add_space(32.0);
            ui.label(RichText::new(format!("– {}", it.auto_note)).color(MUTED).size(12.5));
        });
    }
    if it.status == Status::Blocked && !it.blocker.trim().is_empty() && it.blocker.trim() != it.auto_note.trim() && !open {
        ui.horizontal(|ui| {
            ui.add_space(32.0);
            ui.label(RichText::new(format!("Blockiert weil: {}", it.blocker)).color(status_color(Status::Blocked)).size(12.5));
        });
    }
    if (!it.retest_hint.trim().is_empty() || it.force) && !open {
        ui.horizontal(|ui| {
            ui.add_space(32.0);
            let mut s = String::new();
            if it.force {
                s.push_str("Wird erzwungen getestet. ");
            }
            if !it.retest_hint.trim().is_empty() {
                s.push_str(&format!("Hinweis für Retest: {}", it.retest_hint.trim()));
            }
            ui.label(RichText::new(s).color(ACCENT).size(12.5));
        });
    }
    if !open {
        return false;
    }
    soft(ui, |ui| {
        egui::Grid::new(format!("g{k}")).num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("Prüfpunkt").color(MUTED));
            ui.add(egui::TextEdit::singleline(&mut it.title).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label(RichText::new("ID / Use Case").color(MUTED));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut it.id).desired_width(80.0));
                ui.add(egui::TextEdit::singleline(&mut it.use_case).desired_width(200.0));
                egui::ComboBox::from_id_source(format!("pr{k}"))
                    .selected_text(it.priority.clone())
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for pr in ["hoch", "mittel", "niedrig"] {
                            ui.selectable_value(&mut it.priority, pr.to_string(), pr);
                        }
                    });
            });
            ui.end_row();
            ui.label(RichText::new("Schritte").color(MUTED));
            ui.add(egui::TextEdit::multiline(&mut it.steps).desired_rows(3).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label(RichText::new("Erwartet").color(MUTED));
            ui.add(egui::TextEdit::multiline(&mut it.expected).desired_rows(2).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label(RichText::new("Beobachtung").color(MUTED));
            ui.add(egui::TextEdit::multiline(&mut it.auto_note).desired_rows(2).desired_width(f32::INFINITY));
            ui.end_row();
            if it.status == Status::Blocked {
                ui.label(RichText::new("Blockiert weil").color(status_color(Status::Blocked)));
                ui.add(egui::TextEdit::multiline(&mut it.blocker).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
            }
            ui.label(RichText::new("Hinweis für Retest").color(MUTED));
            ui.vertical(|ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut it.retest_hint)
                        .hint_text("Wird beim nächsten Test an Claude gegeben")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(&mut it.force, "Trotzdem testen (nicht wegen Abhängigkeiten blockieren)");
            });
            ui.end_row();
            ui.label(RichText::new("Status").color(MUTED));
            ui.horizontal_wrapped(|ui| {
                for s in Status::all() {
                    ui.selectable_value(&mut it.status, s, RichText::new(s.label()).color(status_color(s)));
                }
            });
            ui.end_row();
            if !it.evidence.is_empty() {
                ui.label(RichText::new("Belege").color(MUTED));
                ui.horizontal_wrapped(|ui| {
                    for e in &it.evidence {
                        if ui.link(format!("Screenshot {e}")).clicked() {
                            actions.push(Action::OpenEvidence(e.clone()));
                        }
                    }
                });
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            if !it.tested_at.is_empty() {
                ui.label(RichText::new(format!("Zuletzt getestet: {}", it.tested_at)).color(MUTED).size(12.0));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("Löschen").color(status_color(Status::Failed))).clicked() {
                    delete = true;
                }
            });
        });
    });
    delete
}
