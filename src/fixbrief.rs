//! Turns the ticked, not-passed checklist items into a bullet-point brief for a
//! coding agent that should fix them. Only ticked items are sent to Claude, so
//! unticked ones can never show up in the text.

use crate::claude::{Ev, Job, JobKind, LogKind};
use crate::model::{CheckItem, Project, Settings, Status};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

/// Default tick: failed items are handed to the fix agent, others only when ticked.
pub fn is_ticked(it: &CheckItem) -> bool {
    it.fix_selected.unwrap_or(it.status == Status::Failed)
}

/// Items that may appear in the fix list (everything that is not passed).
pub fn candidates(p: &Project) -> Vec<usize> {
    (0..p.checklist.len())
        .filter(|&k| matches!(p.checklist[k].status, Status::Failed | Status::Blocked))
        .collect()
}

pub fn ticked_ids(p: &Project) -> Vec<String> {
    candidates(p)
        .into_iter()
        .filter(|&k| is_ticked(&p.checklist[k]))
        .map(|k| p.checklist[k].id.clone())
        .collect()
}

fn evidence_paths(p: &Project, it: &CheckItem) -> Vec<String> {
    let dir = crate::store::project_dir(&p.id).join("laeufe");
    it.evidence
        .iter()
        .map(|e| crate::app::find_in_runs(&dir, e).unwrap_or_else(|| e.clone()))
        .collect()
}

fn prompt(p: &Project, items: &[&CheckItem]) -> String {
    let mut ctx = format!(
        "Web-Applikation: {}\nGetestet mit: {} (automatisiert über Playwright)\n",
        p.target_url.trim(),
        p.browser.label()
    );
    let urls: Vec<String> = p
        .extra_urls
        .iter()
        .filter(|u| !u.url.trim().is_empty())
        .map(|u| format!("- {}: {}", u.label.trim(), u.url.trim()))
        .collect();
    if !urls.is_empty() {
        ctx.push_str(&format!("Zusätzliche URLs:\n{}\n", urls.join("\n")));
    }
    if !p.test_data.trim().is_empty() {
        ctx.push_str(&format!("Testdaten:\n{}\n", p.test_data.trim()));
    }
    let list: Vec<Value> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "id": i.id,
                "use_case": i.use_case,
                "pruefpunkt": i.title,
                "status": i.status.label(),
                "schritte": i.steps.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>(),
                "erwartet": i.expected,
                "beobachtet": i.auto_note,
                "anmerkung_pruefer": i.reviewer_note,
                "screenshots": evidence_paths(p, i),
                "getestet_am": i.tested_at,
            })
        })
        .collect();
    format!(
        "Schreibe eine Arbeitsvorgabe für einen Coding-Agenten, der die folgenden in der Abnahme \
festgestellten Fehler einer Web-Applikation beheben soll.\n\n{ctx}\n\
Fehler (JSON):\n{}\n\n\
Regeln:\n\
- Nur Stichpunkte (Markdown-Liste), knapp und präzise, auf Deutsch, direkt kopierbar.\n\
- Keine Einleitung, keine Grußformel, kein Fazit, kein Code-Block um den ganzen Text.\n\
- Beginne mit einem Stichpunkt-Block 'Kontext' (App-URL, Browser, Testdaten/Zugänge falls relevant).\n\
- Danach pro Fehler ein Abschnitt '### <ID> – <Prüfpunkt>' mit den Stichpunkten: \
Erwartet, Tatsächlich, Wie getestet (Schritte/URLs), Belege (Screenshot-Pfade, falls vorhanden), \
Vermutete Ursache / Ansatz (nur wenn aus der Beobachtung ableitbar, sonst weglassen), Nachprüfung (wie der Fix geprüft wird).\n\
- Am Ende ein Block 'Fertig wenn' mit überprüfbaren Kriterien.\n\
- Erwähne AUSSCHLIESSLICH die oben gelisteten Fehler. Erfinde keine weiteren Probleme.",
        serde_json::to_string_pretty(&list).unwrap_or_default()
    )
}

pub fn start(p: &Project, settings: &Settings, path_env: &str) -> Job {
    let (tx, rx) = channel();
    let ids = ticked_ids(p);
    let items: Vec<&CheckItem> = p.checklist.iter().filter(|i| ids.contains(&i.id)).collect();
    let text = prompt(p, &items);
    let dir = crate::store::project_dir(&p.id);
    let exe = crate::store::which(path_env, settings.claude_path.trim()).unwrap_or_else(|| settings.claude_path.trim().to_string());
    let model = settings.model.trim().to_string();
    let path_env = path_env.to_string();
    let child: Arc<Mutex<Option<std::process::Child>>> = Arc::new(Mutex::new(None));
    let slot = child.clone();
    let n = items.len();
    let _ = tx.send(Ev::Log(LogKind::Info, format!("Claude schreibt die Vorgabe für den Fix-Agenten ({n} Fehler) …")));
    std::thread::spawn(move || {
        let mut cmd = Command::new(exe);
        cmd.env("PATH", &path_env).current_dir(&dir);
        cmd.args([
            "-p",
            &text,
            "--output-format",
            "json",
            "--model",
            &model,
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
        ]);
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let c = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Ev::Done { ok: false, summary: format!("Claude Code konnte nicht starten: {e}"), cost: 0.0 });
                return;
            }
        };
        let pid = c.id();
        *slot.lock().unwrap() = Some(c);
        // wait without holding the lock for long, so "Stopp" can still kill it
        let out = loop {
            let mut g = slot.lock().unwrap();
            let Some(c) = g.as_mut() else { return };
            match c.try_wait() {
                Ok(Some(_)) => break g.take().and_then(|c| c.wait_with_output().ok()),
                Ok(None) => {}
                Err(_) => break None,
            }
            drop(g);
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        let _ = pid;
        let v: Option<Value> = out.and_then(|o| serde_json::from_slice(&o.stdout).ok());
        let result = v.as_ref().and_then(|v| v.get("result")).and_then(|r| r.as_str()).unwrap_or("").trim().to_string();
        let cost = v.as_ref().and_then(|v| v.get("total_cost_usd")).and_then(|c| c.as_f64()).unwrap_or(0.0);
        if result.is_empty() {
            let _ = tx.send(Ev::Done { ok: false, summary: "Claude hat keine Vorgabe geliefert.".into(), cost });
        } else {
            let _ = tx.send(Ev::FixBrief(result));
            let _ = tx.send(Ev::Done { ok: true, summary: format!("Vorgabe für den Fix-Agenten erstellt ({n} Fehler)."), cost });
        }
    });
    Job {
        kind: JobKind::Brief,
        project_id: p.id.clone(),
        rx,
        child,
        started: crate::store::now_string(),
        log_file: PathBuf::new(),
        ids,
    }
}
