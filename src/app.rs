use crate::claude::{self, Ev, Job, JobKind, LogKind};
use crate::model::*;
use crate::store;
use crate::theme::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Spec,
    Checklist,
    Run,
    Signoff,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Only(Status),
}

pub enum Action {
    Derive,
    Test(Vec<String>),
    Stop,
    InstallFirefox,
    CheckClaude,
    OpenUrl(String, Option<Browser>),
    Export,
    Open(String),
    OpenEvidence(String),
    Reextract(String, Option<Vec<String>>),
    FixBrief,
    Reveal(String),
    NewProject,
    DeleteProject,
    Goto(Tab),
}

pub struct App {
    pub(crate) projects: Vec<Project>,
    pub(crate) sel: usize,
    pub(crate) tab: Tab,
    pub(crate) settings: Settings,
    pub(crate) path_env: Arc<Mutex<Option<String>>>,
    pub(crate) job: Option<Job>,
    pub(crate) log: Vec<(LogKind, String)>,
    pub(crate) message: Option<(bool, String)>,
    pub(crate) show_settings: bool,
    pub(crate) confirm_delete: bool,
    pub(crate) confirm_replace: bool,
    pub(crate) filter: Filter,
    pub(crate) expanded: HashSet<String>,
    pub(crate) last_report: Option<PathBuf>,
    pub(crate) saved_hash: u64,
    pub(crate) frame_no: u32,
    pub(crate) screenshot: Option<PathBuf>,
    pub(crate) shot_done: bool,
}

fn hash_of(p: &[Project]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(p).unwrap_or_default().hash(&mut h);
    h.finish()
}

pub fn find_in_runs(dir: &std::path::Path, name: &str) -> Option<String> {
    let p = std::path::Path::new(name);
    if p.is_absolute() && p.exists() {
        return Some(name.to_string());
    }
    let mut runs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    runs.sort();
    for r in runs.iter().rev() {
        let f = r.join(name);
        if f.exists() {
            return Some(f.to_string_lossy().to_string());
        }
    }
    None
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, start_tab: Option<u8>) -> Self {
        setup_style(&cc.egui_ctx);
        let mut projects = store::load_projects();
        if projects.is_empty() {
            let p = Project::example(store::new_id());
            let _ = store::save_project(&p);
            projects.push(p);
        }
        let path_env = Arc::new(Mutex::new(None));
        let pe = path_env.clone();
        std::thread::spawn(move || {
            let p = store::login_path();
            *pe.lock().unwrap() = Some(p);
        });
        let tab = match start_tab {
            Some(1) => Tab::Spec,
            Some(3) => Tab::Run,
            Some(4) => Tab::Signoff,
            Some(_) => Tab::Checklist,
            None if projects[0].checklist.is_empty() => Tab::Spec,
            None => Tab::Checklist,
        };
        let saved_hash = hash_of(&projects);
        let mut log = Vec::new();
        if screenshot.is_some() && start_tab == Some(3) {
            // sample lines so a check picture shows how the protocol wraps
            log.push((LogKind::Info, "Claude Code gestartet · Modell sonnet · Browser-Werkzeug: playwright (connected), abnahme (connected)".to_string()));
            log.push((LogKind::Tool, "browser_navigate {\"url\":\"http://localhost:3000/register?token=2bfd461edd1ef40a5d73c1c4cd799203359569c1ebb4771b784a2b22ef317c3c\"}".to_string()));
            log.push((LogKind::ToolResult, "### Ran Playwright code ```js await page.goto('http://localhost:3000/register'); ``` ### Page state - Page URL: http://localhost:3000/register - Page Title: Registrierung".to_string()));
            log.push((LogKind::Text, "## Zusammenfassung\n**Bestanden (2):** UC1-01, UC1-02\n**Fehlgeschlagen (1):** UC1-03 – Nach 60 Sekunden ist keine Bestätigungs-Mail in MailHog angekommen, obwohl die Registrierung eine Erfolgsmeldung angezeigt hat.".to_string()));
            log.push((LogKind::Result, "✖ UC1-03 – Fehlgeschlagen: Keine Mail nach 60 Sekunden in MailHog.".to_string()));
        }
        App {
            projects,
            sel: 0,
            tab,
            settings: store::load_settings(),
            path_env,
            job: None,
            log,
            message: None,
            show_settings: false,
            confirm_delete: false,
            confirm_replace: false,
            filter: Filter::All,
            expanded: HashSet::new(),
            last_report: None,
            saved_hash,
            frame_no: 0,
            screenshot,
            shot_done: false,
        }
    }

    fn path_env(&self) -> String {
        for _ in 0..100 {
            if let Some(p) = self.path_env.lock().unwrap().clone() {
                return p;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        std::env::var("PATH").unwrap_or_default()
    }

    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub(crate) fn poll_job(&mut self) {
        let Some(job) = self.job.as_ref() else { return };
        let mut events = Vec::new();
        while let Ok(ev) = job.rx.try_recv() {
            events.push(ev);
        }
        if events.is_empty() {
            return;
        }
        let kind = job.kind;
        let pid = job.project_id.clone();
        let ids = job.ids.clone();
        let started = job.started.clone();
        let log_file = job.log_file.to_string_lossy().to_string();
        let pidx = self.projects.iter().position(|p| p.id == pid);
        let mut finished = false;
        for ev in events {
            match ev {
                Ev::Log(k, t) => self.log.push((k, t)),
                Ev::Item(r) => {
                    if let Some(pi) = pidx {
                        if let Some(it) = self.projects[pi].checklist.iter_mut().find(|i| i.id == r.id) {
                            it.status = r.status;
                            it.auto_note = r.note.clone();
                            it.evidence = r.evidence.clone();
                            it.tested_at = store::now_string();
                            it.confirmed = false;
                            it.fix_selected = None;
                            it.blocker = if r.status == Status::Blocked { r.blocker.clone() } else { String::new() };
                        }
                    }
                    self.log.push((
                        LogKind::Result,
                        format!("{} {} – {}: {}", r.status.icon(), r.id, r.status.label(), r.note),
                    ));
                }
                Ev::FixBrief(text) => {
                    if let Some(pi) = pidx {
                        let p = &mut self.projects[pi];
                        p.fix_brief = text;
                        p.fix_brief_ids = ids.clone();
                    }
                }
                Ev::Summary { text, recommendation, conditions } => {
                    if let Some(pi) = pidx {
                        let p = &mut self.projects[pi];
                        p.claude_summary = text;
                        if recommendation.is_some() {
                            p.claude_recommendation = recommendation;
                        }
                        if !conditions.trim().is_empty() {
                            p.claude_conditions = conditions;
                        }
                    }
                }
                Ev::Checklist(items) => {
                    if let Some(pi) = pidx {
                        self.projects[pi].checklist = items;
                        self.expanded.clear();
                    }
                }
                Ev::Done { ok, summary, cost } => {
                    finished = true;
                    if cost > 0.0 {
                        self.log.push((LogKind::Info, format!("Kosten laut Claude Code: {:.2} US$", cost)));
                    }
                    self.log.push((if ok { LogKind::Result } else { LogKind::Error }, summary.clone()));
                    match kind {
                        JobKind::Test => {
                            if let Some(pi) = pidx {
                                let p = &mut self.projects[pi];
                                for it in p.checklist.iter_mut() {
                                    if ids.contains(&it.id) && it.status == Status::Running {
                                        it.status = Status::Open;
                                        it.auto_note = "Im letzten Lauf nicht bewertet.".into();
                                    }
                                }
                                let sel: Vec<&CheckItem> = p.checklist.iter().filter(|i| ids.contains(&i.id)).collect();
                                let rec = RunRecord {
                                    started: started.clone(),
                                    finished: store::now_string(),
                                    summary: summary.clone(),
                                    passed: sel.iter().filter(|i| i.status == Status::Passed).count(),
                                    failed: sel.iter().filter(|i| i.status == Status::Failed).count(),
                                    blocked: sel.iter().filter(|i| i.status == Status::Blocked).count(),
                                    cost_usd: cost,
                                    log_file: log_file.clone(),
                                };
                                let msg = format!(
                                    "Testlauf fertig: {} bestanden, {} fehlgeschlagen, {} blockiert.",
                                    rec.passed, rec.failed, rec.blocked
                                );
                                p.runs.push(rec);
                                self.message = Some((ok, msg));
                            }
                        }
                        JobKind::Derive => {
                            self.message = Some((ok, summary.clone()));
                            if ok {
                                self.tab = Tab::Checklist;
                            }
                        }
                        _ => self.message = Some((ok, summary.clone())),
                    }
                }
            }
        }
        if self.log.len() > 4000 {
            let cut = self.log.len() - 4000;
            self.log.drain(0..cut);
        }
        if finished {
            self.job = None;
        }
    }

    pub(crate) fn apply(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::Goto(t) => self.tab = t,
                Action::Derive => {
                    if self.busy() {
                        continue;
                    }
                    let p = &self.projects[self.sel];
                    if p.use_cases.iter().all(|u| u.description.trim().is_empty() && u.title.trim().is_empty()) {
                        self.message = Some((false, "Bitte zuerst mindestens einen Use Case beschreiben.".into()));
                        continue;
                    }
                    self.log.clear();
                    let pe = self.path_env();
                    let p = &self.projects[self.sel];
                    self.job = Some(claude::start_derive(p, &self.settings, &pe));
                    self.message = None;
                }
                Action::Test(ids) => {
                    if self.busy() || ids.is_empty() {
                        continue;
                    }
                    let pe = self.path_env();
                    let p = &mut self.projects[self.sel];
                    for it in p.checklist.iter_mut() {
                        if ids.contains(&it.id) {
                            it.status = Status::Running;
                            it.auto_note.clear();
                            it.evidence.clear();
                        }
                    }
                    self.log.clear();
                    let p = &self.projects[self.sel];
                    let _ = store::save_project(p);
                    self.job = Some(claude::start_test(p, &ids, &self.settings, &pe));
                    self.tab = Tab::Run;
                    self.message = None;
                }
                Action::Stop => {
                    if let Some(j) = &self.job {
                        j.stop();
                        self.log.push((LogKind::Error, "Abgebrochen.".into()));
                    }
                }
                Action::InstallFirefox => {
                    if self.busy() {
                        continue;
                    }
                    self.log.clear();
                    let pe = self.path_env();
                    let pid = self.projects[self.sel].id.clone();
                    self.job = Some(claude::start_simple(
                        JobKind::Install,
                        "npx",
                        &["-y", "playwright@latest", "install", "firefox"],
                        &pe,
                        &pid,
                    ));
                    self.tab = Tab::Run;
                }
                Action::CheckClaude => {
                    if self.busy() {
                        continue;
                    }
                    self.log.clear();
                    let pe = self.path_env();
                    let pid = self.projects[self.sel].id.clone();
                    let exe = self.settings.claude_path.clone();
                    self.job = Some(claude::start_simple(JobKind::Check, &exe, &["--version"], &pe, &pid));
                }
                Action::OpenUrl(u, b) => {
                    let mut c = std::process::Command::new("/usr/bin/open");
                    match b {
                        Some(Browser::Chrome) => {
                            c.args(["-a", "Google Chrome"]);
                        }
                        Some(Browser::Firefox) => {
                            c.args(["-a", "Firefox"]);
                        }
                        None => {}
                    }
                    let _ = c.arg(u).spawn();
                }
                Action::Export => match crate::report::export(&self.projects[self.sel]) {
                    Ok(f) => {
                        self.message = Some((true, format!("Abnahmeprotokoll gespeichert: {}", f.to_string_lossy())));
                        store::open_path(&f.to_string_lossy());
                        self.last_report = Some(f);
                    }
                    Err(e) => self.message = Some((false, format!("Bericht konnte nicht gespeichert werden: {e}"))),
                },
                Action::Open(p) => store::open_path(&p),
                Action::OpenEvidence(name) => {
                    let dir = store::project_dir(&self.projects[self.sel].id).join("laeufe");
                    let f = find_in_runs(&dir, &name).unwrap_or_else(|| dir.to_string_lossy().to_string());
                    store::open_path(&f);
                }
                Action::Reveal(p) => store::reveal(&p),
                Action::FixBrief => {
                    if self.busy() {
                        continue;
                    }
                    self.log.clear();
                    let pe = self.path_env();
                    let p = &self.projects[self.sel];
                    self.job = Some(crate::fixbrief::start(p, &self.settings, &pe));
                    self.message = None;
                }
                Action::Reextract(log, only) => {
                    if self.busy() {
                        continue;
                    }
                    self.log.clear();
                    let pe = self.path_env();
                    let p = &self.projects[self.sel];
                    let whole = only.is_none();
                    self.job = Some(crate::reextract::start(p, &log, only, &self.settings, &pe));
                    if whole {
                        self.tab = Tab::Run;
                    }
                    self.message = None;
                }
                Action::NewProject => {
                    let n = self.projects.len() + 1;
                    self.projects.push(Project::new(store::new_id(), format!("Neues Projekt {n}")));
                    self.sel = self.projects.len() - 1;
                    self.tab = Tab::Spec;
                    self.expanded.clear();
                }
                Action::DeleteProject => {
                    if self.busy() {
                        continue;
                    }
                    let id = self.projects[self.sel].id.clone();
                    store::delete_project(&id);
                    self.projects.remove(self.sel);
                    if self.projects.is_empty() {
                        self.projects.push(Project::new(store::new_id(), "Neues Projekt".into()));
                    }
                    self.sel = 0;
                    self.saved_hash = 0;
                }
            }
        }
    }

    pub(crate) fn autosave(&mut self) {
        if self.frame_no % 30 != 0 {
            return;
        }
        let h = hash_of(&self.projects);
        if h != self.saved_hash {
            for p in &self.projects {
                let _ = store::save_project(p);
            }
            self.saved_hash = h;
        }
    }
}
