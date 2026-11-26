//! Step 1: target URL, browser, extra URLs, test data and the use cases.

use crate::app::{Action, Tab};
use crate::model::*;
use crate::theme::*;
use eframe::egui::{self, RichText};

pub fn show(ui: &mut egui::Ui, p: &mut Project, busy: bool, actions: &mut Vec<Action>, confirm_replace: &mut bool) {
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.columns(2, |cols| {
            left(&mut cols[0], p, busy, actions);
            right(&mut cols[1], p, busy, actions, confirm_replace);
        });
    });
}

fn left(ui: &mut egui::Ui, p: &mut Project, busy: bool, actions: &mut Vec<Action>) {
    card(ui, |ui| {
        section_title(ui, "Ziel", "Die Web-Applikation, die getestet und abgenommen werden soll.");
        ui.label(RichText::new("Ziel-URL").color(MUTED).size(12.5));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut p.target_url).desired_width((ui.available_width() - 90.0).max(100.0)));
            if ui.button("Öffnen ↗").clicked() {
                actions.push(Action::OpenUrl(p.target_url.clone(), Some(p.browser)));
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Browser für den Test").color(MUTED).size(12.5));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut p.browser, Browser::Chrome, "  Google Chrome  ");
            ui.selectable_value(&mut p.browser, Browser::Firefox, "  Firefox  ");
            ui.add_space(8.0);
            let mut visible = !p.headless;
            if ui.checkbox(&mut visible, "Browser sichtbar").changed() {
                p.headless = !visible;
            }
        });
        if p.browser == Browser::Firefox {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Firefox wird über Playwright gesteuert und braucht einmalig dessen Firefox-Version.")
                        .color(MUTED)
                        .size(12.0),
                );
                if ui.add_enabled(!busy, egui::Button::new("Firefox-Treiber installieren").small()).clicked() {
                    actions.push(Action::InstallFirefox);
                }
            });
        }
    });
    ui.add_space(10.0);
    card(ui, |ui| {
        section_title(
            ui,
            "Zusätzliche URLs",
            "Hilfsseiten für die Use Cases, z. B. MailHog für den Mail-Empfang oder ein Admin-Bereich.",
        );
        let mut remove = None;
        for (i, u) in p.extra_urls.iter_mut().enumerate() {
            soft(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut u.label).hint_text("Bezeichnung").desired_width(100.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut u.url)
                            .hint_text("http://localhost:8025")
                            .desired_width((ui.available_width() - 96.0).max(80.0)),
                    );
                    if ui.small_button("↗").on_hover_text("Öffnen").clicked() {
                        actions.push(Action::OpenUrl(u.url.clone(), None));
                    }
                    if ui.small_button("✖").on_hover_text("Entfernen").clicked() {
                        remove = Some(i);
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut u.note)
                        .hint_text("Wofür? (z. B. 'empfängt alle Mails')")
                        .desired_width(f32::INFINITY),
                );
            });
        }
        if let Some(i) = remove {
            p.extra_urls.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button("+ URL hinzufügen").clicked() {
                p.extra_urls.push(ExtraUrl::default());
            }
            if !p.extra_urls.iter().any(|u| u.label.to_lowercase().contains("mailhog")) && ui.button("+ MailHog").clicked() {
                p.extra_urls.push(ExtraUrl {
                    label: "MailHog".into(),
                    url: "http://localhost:8025".into(),
                    note: "Empfängt alle E-Mails der Test-Umgebung".into(),
                });
            }
        });
    });
    ui.add_space(10.0);
    card(ui, |ui| {
        section_title(ui, "Testdaten & Zugänge", "Benutzer, Passwörter, Testkarten, Hinweise – wird an Claude weitergegeben.");
        ui.add(
            egui::TextEdit::multiline(&mut p.test_data)
                .hint_text("z. B. Admin: admin@test.de / geheim123")
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
    });
}

fn right(ui: &mut egui::Ui, p: &mut Project, busy: bool, actions: &mut Vec<Action>, confirm_replace: &mut bool) {
    card(ui, |ui| {
        section_title(
            ui,
            "Use Cases & Testkriterien",
            "In eigenen Worten beschreiben, was die Anwendung können muss. Claude macht daraus eine prüfbare Checkliste.",
        );
        let mut remove = None;
        let n = p.use_cases.len();
        for (i, uc) in p.use_cases.iter_mut().enumerate() {
            soft(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("UC{}", i + 1)).strong().color(ACCENT));
                    ui.add(
                        egui::TextEdit::singleline(&mut uc.title)
                            .hint_text("Titel, z. B. Registrierung")
                            .desired_width((ui.available_width() - 50.0).max(80.0)),
                    );
                    if n > 1 && ui.small_button("✖").on_hover_text("Use Case entfernen").clicked() {
                        remove = Some(i);
                    }
                });
                ui.add(
                    egui::TextEdit::multiline(&mut uc.description)
                        .hint_text("Was soll passieren? Was ist das erwartete Ergebnis? Welche Sonderfälle gibt es?")
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
            });
        }
        if let Some(i) = remove {
            p.use_cases.remove(i);
        }
        if ui.button("+ Use Case hinzufügen").clicked() {
            p.use_cases.push(UseCase::default());
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        if *confirm_replace {
            ui.label(
                RichText::new("Es gibt schon eine Checkliste. Neu ableiten ersetzt sie samt Ergebnissen.").color(status_color(Status::Blocked)),
            );
            ui.horizontal(|ui| {
                if primary(ui, "Ja, neu ableiten", !busy) {
                    *confirm_replace = false;
                    actions.push(Action::Derive);
                }
                if ui.button("Abbrechen").clicked() {
                    *confirm_replace = false;
                }
            });
        } else {
            ui.horizontal(|ui| {
                if primary(ui, "Checkliste mit Claude ableiten", !busy) {
                    if p.checklist.is_empty() {
                        actions.push(Action::Derive);
                    } else {
                        *confirm_replace = true;
                    }
                }
                if !p.checklist.is_empty() && ui.button("Zur Checkliste").clicked() {
                    actions.push(Action::Goto(Tab::Checklist));
                }
            });
        }
    });
}
