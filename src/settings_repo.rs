//! Settings section "Quellcode & GitHub" and the small status hint in the sidebar.

use crate::app::{Action, App};
use crate::claude::{JobKind, LogKind};
use crate::model::Status;
use crate::repo;
use crate::theme::*;
use eframe::egui::{self, Color32, RichText, Vec2};

impl App {
    /// One-line hint for the sidebar; returns true when clicked.
    pub(crate) fn repo_hint(&self, ui: &mut egui::Ui) -> bool {
        let Some(st) = self.repo.get() else { return false };
        if st.error.is_some() {
            return false;
        }
        let (c, t) = if st.needs_commit() {
            (Color32::from_rgb(235, 180, 80), format!("Quellcode: {} Änderungen offen", st.changes.len()))
        } else if st.needs_push() {
            let n = st.ahead.max(1);
            (Color32::from_rgb(235, 180, 80), format!("Quellcode: {n} nicht gepusht"))
        } else if st.behind > 0 {
            (Color32::from_rgb(120, 170, 255), format!("Quellcode: {} neu auf GitHub", st.behind))
        } else {
            (Color32::from_gray(140), "Quellcode: alles gepusht".to_string())
        };
        let resp = ui
            .horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, c);
                ui.add(egui::Label::new(RichText::new(t).color(c).size(11.5)).wrap(true).sense(egui::Sense::click()))
            })
            .inner;
        resp.on_hover_text("In den Einstellungen committen und pushen").clicked()
    }

    pub(crate) fn repo_section(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Quellcode & GitHub").strong().size(15.0));
        let st = self.repo.get();
        let git_running = self.job.as_ref().map(|j| j.kind == JobKind::Git).unwrap_or(false);
        let busy = self.job.is_some();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Repository-Ordner").color(MUTED));
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.settings.repo_path)
                    .hint_text("automatisch erkannt")
                    .desired_width(300.0),
            );
            if r.lost_focus() {
                self.repo.set_path(&self.settings.repo_path);
            }
        });

        let Some(st) = st else {
            ui.label(
                RichText::new("Kein Git-Repository gefunden. Die App muss aus ihrem Projektordner (z. B. dist/Web-Abnahme.app) laufen, oder trage den Ordner oben ein.")
                    .color(MUTED),
            );
            return;
        };
        if let Some(e) = &st.error {
            ui.label(RichText::new(format!("Git-Fehler: {e}")).color(status_color(Status::Failed)));
            return;
        }
        egui::Grid::new("repo").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.label(RichText::new("Ordner").color(MUTED));
            ui.label(st.path.to_string_lossy().to_string());
            ui.end_row();
            ui.label(RichText::new("Branch").color(MUTED));
            ui.label(if st.upstream.is_empty() { st.branch.clone() } else { format!("{} (folgt {})", st.branch, st.upstream) });
            ui.end_row();
            ui.label(RichText::new("Remote").color(MUTED));
            ui.label(if st.remote_url.is_empty() { "– (kein origin)".to_string() } else { st.remote_url.clone() });
            ui.end_row();
            ui.label(RichText::new("Letzter Commit").color(MUTED));
            ui.label(&st.last_commit);
            ui.end_row();
        });

        let warn = status_color(Status::Blocked);
        if st.needs_commit() {
            ui.label(RichText::new(format!("⚠ {} nicht committete Änderungen – Commit & Push nötig", st.changes.len())).color(warn).strong());
            egui::CollapsingHeader::new("Geänderte Dateien").id_source("repo_changes").show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    for c in &st.changes {
                        ui.label(RichText::new(c).monospace().size(12.0));
                    }
                });
            });
        } else if st.needs_push() {
            ui.label(RichText::new(format!("⚠ {} Commit(s) noch nicht gepusht", st.ahead.max(1))).color(warn).strong());
        } else {
            ui.label(RichText::new("✔ Alles committet und gepusht").color(status_color(Status::Passed)));
        }
        if st.behind > 0 {
            ui.label(RichText::new(format!("Auf GitHub gibt es {} neuere Commits.", st.behind)).color(ACCENT));
        }

        if st.needs_commit() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Commit-Nachricht").color(MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.commit_msg)
                        .hint_text(format!("Update {}", crate::store::now_string()))
                        .desired_width(300.0),
                );
            });
        }
        ui.horizontal(|ui| {
            if st.needs_commit() {
                if primary(ui, "Commit & Push", !busy) {
                    let msg = if self.commit_msg.trim().is_empty() {
                        format!("Update {}", crate::store::now_string())
                    } else {
                        self.commit_msg.trim().to_string()
                    };
                    self.commit_msg.clear();
                    actions.push(Action::Git(repo::COMMIT_AND_PUSH, msg));
                }
            } else if st.needs_push() && primary(ui, "Push", !busy) {
                actions.push(Action::Git(repo::PUSH_ONLY, String::new()));
            }
            if ui.add_enabled(!busy, egui::Button::new("Mit GitHub abgleichen")).clicked() {
                actions.push(Action::Git(repo::FETCH, String::new()));
            }
            if ui.button("Status prüfen").clicked() {
                self.repo.poke();
            }
            if git_running {
                ui.spinner();
            }
        });
        if git_running || self.log.iter().any(|(_, t)| t.starts_with("$ /bin/zsh")) {
            let tail: Vec<&(LogKind, String)> = self
                .log
                .iter()
                .filter(|(_, t)| !t.starts_with("$ /bin/zsh"))
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            for (k, t) in tail {
                let c = if *k == LogKind::Error { status_color(Status::Failed) } else { MUTED };
                ui.label(RichText::new(t).monospace().size(11.5).color(c));
            }
        }
    }
}
