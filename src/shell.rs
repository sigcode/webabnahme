//! The window frame: project sidebar, header, step tabs, settings and screenshot mode.

use crate::app::{Action, App, Tab};
use crate::claude::JobKind;
use crate::model::Status;
use crate::store;
use crate::theme::*;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.frame_no += 1;
        self.poll_job();
        let mut actions: Vec<Action> = Vec::new();

        self.sidebar(ctx, &mut actions);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(24.0, 18.0)))
            .show(ctx, |ui| {
                let busy = self.job.is_some();
                let job_kind = self.job.as_ref().map(|j| j.kind);
                let sel = self.sel;
                {
                    let p = &mut self.projects[sel];
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut p.name)
                                .font(egui::FontId::proportional(22.0))
                                .frame(false)
                                .desired_width((ui.available_width() - 320.0).max(200.0)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if busy {
                                if ui.button("⏹ Stopp").clicked() {
                                    actions.push(Action::Stop);
                                }
                                ui.spinner();
                                ui.label(
                                    RichText::new(match job_kind {
                                        Some(JobKind::Derive) => "Claude leitet Checkliste ab …",
                                        Some(JobKind::Test) => "Test läuft …",
                                        _ => "Arbeitet …",
                                    })
                                    .color(ACCENT),
                                );
                            } else {
                                ui.label(RichText::new(p.target_url.as_str()).color(MUTED).size(12.5));
                            }
                        });
                    });
                }
                ui.add_space(6.0);
                self.stepper(ui, &mut actions);
                ui.add_space(10.0);
                if let Some((ok, m)) = self.message.clone() {
                    let c = if ok { status_color(Status::Passed) } else { status_color(Status::Failed) };
                    egui::Frame::none()
                        .fill(c.linear_multiply(0.1))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(m).color(c));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("✖").clicked() {
                                        self.message = None;
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                }

                match self.tab {
                    Tab::Spec => {
                        let p = &mut self.projects[sel];
                        crate::tab_spec::show(ui, p, busy, &mut actions, &mut self.confirm_replace);
                    }
                    Tab::Checklist => {
                        let p = &mut self.projects[sel];
                        crate::tab_checklist::show(ui, p, busy, &mut actions, &mut self.filter, &mut self.expanded);
                    }
                    Tab::Run => {
                        let p = &self.projects[sel];
                        let running_ids = self.job.as_ref().map(|j| j.ids.clone()).unwrap_or_default();
                        crate::tab_run::show(ui, p, busy, job_kind, &running_ids, &self.log, &mut actions);
                    }
                    Tab::Signoff => {
                        let p = &mut self.projects[sel];
                        crate::tab_signoff::show(ui, p, busy, job_kind, &mut actions, &self.last_report, &mut self.confirm_delete);
                    }
                }
            });

        if self.show_settings {
            self.settings_window(ctx, &mut actions);
        }

        self.apply(actions);
        self.autosave();

        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        if self.screenshot.is_some() {
            ctx.request_repaint();
            if self.frame_no == 120 {
                frame.request_screenshot();
            }
            if self.frame_no > 600 {
                frame.close();
            }
        }
    }

    fn post_rendering(&mut self, _size: [u32; 2], frame: &eframe::Frame) {
        if self.shot_done {
            return;
        }
        if let (Some(path), Some(img)) = (self.screenshot.clone(), frame.screenshot()) {
            if let Err(e) = crate::save_png(&path, &img) {
                eprintln!("Screenshot konnte nicht gespeichert werden: {e}");
            }
            self.shot_done = true;
            std::process::exit(0);
        }
    }

    fn on_close_event(&mut self) -> bool {
        if let Some(j) = &self.job {
            j.stop();
        }
        for p in &self.projects {
            let _ = store::save_project(p);
        }
        true
    }
}

impl App {
    fn sidebar(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::SidePanel::left("projects")
            .resizable(false)
            .exact_width(236.0)
            .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(egui::Margin::same(14.0)))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 8.0, ACCENT);
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "✔", egui::FontId::proportional(17.0), Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("Web-Abnahme").color(Color32::WHITE).strong().size(16.0));
                        ui.label(RichText::new("Tests mit Claude Code").color(Color32::from_gray(150)).size(11.5));
                    });
                });
                ui.add_space(18.0);
                ui.label(RichText::new("PROJEKTE").color(Color32::from_gray(130)).size(11.0).strong());
                ui.add_space(2.0);
                let mut new_sel = None;
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 120.0).max(60.0))
                    .show(ui, |ui| {
                        for (i, p) in self.projects.iter().enumerate() {
                            let selected = i == self.sel;
                            let (ok, fail, blk, _, _) = p.counts();
                            let fill = if selected { Color32::from_rgb(48, 58, 84) } else { Color32::TRANSPARENT };
                            let resp = egui::Frame::none()
                                .fill(fill)
                                .rounding(8.0)
                                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.y = 3.0;
                                    ui.label(RichText::new(&p.name).color(Color32::WHITE).size(13.5));
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        if p.checklist.is_empty() {
                                            ui.label(RichText::new("noch keine Checkliste").color(Color32::from_gray(140)).size(11.5));
                                        } else {
                                            ui.label(RichText::new(format!("✔ {ok}")).color(Color32::from_rgb(90, 200, 130)).size(11.5));
                                            ui.label(RichText::new(format!("✖ {fail}")).color(Color32::from_rgb(240, 110, 110)).size(11.5));
                                            ui.label(RichText::new(format!("⚠ {blk}")).color(Color32::from_rgb(235, 180, 80)).size(11.5));
                                            ui.label(
                                                RichText::new(format!("von {}", p.checklist.len())).color(Color32::from_gray(140)).size(11.5),
                                            );
                                        }
                                    });
                                })
                                .response;
                            if resp.interact(egui::Sense::click()).clicked() {
                                new_sel = Some(i);
                            }
                        }
                    });
                if let Some(i) = new_sel {
                    if i != self.sel {
                        self.sel = i;
                        self.expanded.clear();
                        self.confirm_delete = false;
                        self.confirm_replace = false;
                        self.tab = if self.projects[i].checklist.is_empty() { Tab::Spec } else { Tab::Checklist };
                    }
                }
                ui.add_space(6.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("+ Neues Projekt").color(Color32::WHITE))
                            .fill(Color32::from_rgb(48, 58, 84))
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(ui.available_width(), 30.0)),
                    )
                    .clicked()
                {
                    actions.push(Action::NewProject);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("⚙ Einstellungen").color(Color32::from_gray(200))).frame(false))
                        .clicked()
                    {
                        self.show_settings = true;
                    }
                    let pe = self.path_env.lock().unwrap().clone();
                    let found = pe.as_ref().and_then(|pe| store::which(pe, self.settings.claude_path.trim()));
                    let (c, t) = match (&found, pe.is_some()) {
                        (Some(_), _) => (Color32::from_rgb(90, 200, 130), "Claude Code bereit"),
                        (None, true) => (Color32::from_rgb(240, 110, 110), "Claude Code nicht gefunden"),
                        (None, false) => (Color32::from_gray(150), "Suche Claude Code …"),
                    };
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 4.0, c);
                        ui.label(RichText::new(t).color(c).size(11.5));
                    });
                    if self.repo_hint(ui) {
                        self.show_settings = true;
                    }
                });
            });
    }

    fn stepper(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let p = &self.projects[self.sel];
            let steps = [
                (Tab::Spec, "1", "Vorgabe", format!("{} Use Cases", p.use_cases.len())),
                (Tab::Checklist, "2", "Checkliste", format!("{} Prüfpunkte", p.checklist.len())),
                (Tab::Run, "3", "Testlauf", format!("{} Läufe", p.runs.len())),
                (Tab::Signoff, "4", "Abnahme", p.signoff.decision.label().to_string()),
            ];
            let w = ((ui.available_width() - 3.0 * 8.0) / 4.0).max(120.0);
            for (t, n, name, sub) in steps {
                let active = self.tab == t;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 50.0), egui::Sense::click());
                let fill = if active {
                    CARD
                } else if resp.hovered() {
                    Color32::from_rgb(234, 237, 244)
                } else {
                    BG
                };
                let painter = ui.painter();
                painter.rect(rect, 10.0, fill, Stroke::new(1.0, if active { ACCENT } else { BORDER }));
                let c = rect.left_center() + Vec2::new(24.0, 0.0);
                painter.circle_filled(c, 12.0, if active { ACCENT } else { Color32::from_rgb(210, 215, 228) });
                painter.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    n,
                    egui::FontId::proportional(13.0),
                    if active { Color32::WHITE } else { TEXT },
                );
                painter.text(
                    rect.left_center() + Vec2::new(46.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(14.5),
                    TEXT,
                );
                painter.text(
                    rect.left_center() + Vec2::new(46.0, 10.0),
                    egui::Align2::LEFT_CENTER,
                    sub,
                    egui::FontId::proportional(11.5),
                    MUTED,
                );
                if resp.clicked() {
                    actions.push(Action::Goto(t));
                }
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let mut open = true;
        let mut save = false;
        egui::Window::new("Einstellungen")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("settings").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Claude Code Programm");
                    ui.add(egui::TextEdit::singleline(&mut self.settings.claude_path).desired_width(300.0));
                    ui.end_row();
                    ui.label("Modell");
                    ui.horizontal(|ui| {
                        for m in ["sonnet", "opus", "haiku"] {
                            ui.selectable_value(&mut self.settings.model, m.to_string(), m);
                        }
                        ui.add(egui::TextEdit::singleline(&mut self.settings.model).desired_width(90.0));
                    });
                    ui.end_row();
                    ui.label("Max. Schritte pro Lauf");
                    ui.add(egui::DragValue::new(&mut self.settings.max_turns).clamp_range(10..=1000));
                    ui.end_row();
                    ui.label("Zusätzliche Vorgaben\nfür Claude");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.settings.extra_instructions)
                            .hint_text("z. B. 'Keine echten Bestellungen absenden'")
                            .desired_rows(3)
                            .desired_width(300.0),
                    );
                    ui.end_row();
                });
                ui.add_space(6.0);
                let pe = self.path_env.lock().unwrap().clone();
                let found = pe.as_ref().and_then(|pe| store::which(pe, self.settings.claude_path.trim()));
                match found {
                    Some(f) => ui.label(RichText::new(format!("✔ Claude Code: {f}")).color(status_color(Status::Passed))),
                    None => ui.label(
                        RichText::new("✖ Claude Code nicht gefunden. Installieren und im Terminal einmal mit 'claude' anmelden.")
                            .color(status_color(Status::Failed)),
                    ),
                };
                let npx = pe.as_ref().and_then(|pe| store::which(pe, "npx"));
                match npx {
                    Some(f) => ui.label(RichText::new(format!("✔ Node/npx für die Browser-Steuerung: {f}")).color(status_color(Status::Passed))),
                    None => ui.label(
                        RichText::new("✖ npx (Node.js) fehlt – wird für die Browser-Steuerung (Playwright) gebraucht.")
                            .color(status_color(Status::Failed)),
                    ),
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Speichern", true) {
                        save = true;
                    }
                    if ui.button("Claude Code testen").clicked() {
                        actions.push(Action::CheckClaude);
                    }
                    if ui.button("Datenordner öffnen").clicked() {
                        actions.push(Action::Open(store::data_dir().to_string_lossy().to_string()));
                    }
                });
                self.repo_section(ui, actions);
            });
        if save || !open {
            store::save_settings(&self.settings);
            self.show_settings = false;
        }
    }
}
