//! Talks to the Claude Code CLI: derives checklists and runs browser tests
//! through the Playwright MCP server.

use crate::model::{CheckItem, Decision, Project, Settings, Status};
use std::collections::HashSet;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobKind {
    Derive,
    Test,
    Install,
    Check,
    Extract,
    Brief,
    Git,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogKind {
    Info,
    Text,
    Tool,
    ToolResult,
    Error,
    Result,
}

#[derive(Clone, Debug)]
pub struct ItemResult {
    pub id: String,
    pub status: Status,
    pub note: String,
    pub evidence: Vec<String>,
    pub blocker: String,
}

pub enum Ev {
    Log(LogKind, String),
    Item(ItemResult),
    Checklist(Vec<CheckItem>),
    FixBrief(String),
    Summary { text: String, recommendation: Option<Decision>, conditions: String },
    Done { ok: bool, summary: String, cost: f64 },
}

pub struct Job {
    pub kind: JobKind,
    pub project_id: String,
    pub rx: Receiver<Ev>,
    pub child: Arc<Mutex<Option<Child>>>,
    pub started: String,
    pub log_file: PathBuf,
    pub ids: Vec<String>,
}

impl Job {
    pub fn stop(&self) {
        if let Ok(mut g) = self.child.lock() {
            if let Some(c) = g.as_mut() {
                // kill the whole process group (claude + npx + browser)
                let pid = c.id() as i32;
                let _ = Command::new("/bin/kill").args(["-TERM", &format!("-{pid}")]).status();
                let _ = c.kill();
            }
        }
    }
}

fn base_command(settings: &Settings, path_env: &str) -> Command {
    let exe = crate::store::which(path_env, settings.claude_path.trim())
        .unwrap_or_else(|| settings.claude_path.trim().to_string());
    let mut c = Command::new(exe);
    c.env("PATH", path_env);
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c
}

fn describe_project(p: &Project) -> String {
    let mut s = String::new();
    s.push_str(&format!("Ziel-URL der zu testenden Web-Applikation: {}\n", p.target_url.trim()));
    if !p.extra_urls.is_empty() {
        s.push_str("\nZusätzliche URLs, die für die Use Cases genutzt werden dürfen:\n");
        for u in &p.extra_urls {
            if u.url.trim().is_empty() {
                continue;
            }
            s.push_str(&format!("- {}: {}", if u.label.trim().is_empty() { "URL" } else { u.label.trim() }, u.url.trim()));
            if !u.note.trim().is_empty() {
                s.push_str(&format!(" ({})", u.note.trim()));
            }
            s.push('\n');
        }
    }
    if !p.test_data.trim().is_empty() {
        s.push_str(&format!("\nTestdaten / Zugänge / Hinweise:\n{}\n", p.test_data.trim()));
    }
    s.push_str("\nUse Cases und Testkriterien (vom Menschen geschrieben):\n");
    for (i, uc) in p.use_cases.iter().enumerate() {
        if uc.title.trim().is_empty() && uc.description.trim().is_empty() {
            continue;
        }
        s.push_str(&format!("\nUC{} – {}\n{}\n", i + 1, uc.title.trim(), uc.description.trim()));
    }
    s
}

pub fn derive_prompt(p: &Project, settings: &Settings) -> String {
    format!(
        "Du bist ein erfahrener QA-Analyst. Leite aus den folgenden, frei formulierten Use Cases \
eine präzise Abnahme-Checkliste für eine Web-Applikation ab.\n\n{}\n\n\
Regeln:\n\
- Jeder Prüfpunkt ist einzeln überprüfbar und hat ein eindeutiges, beobachtbares erwartetes Ergebnis.\n\
- Decke Normalfall, wichtige Fehlerfälle (z. B. ungültige Eingaben) und Abhängigkeiten (z. B. E-Mail-Empfang über die zusätzlichen URLs) ab.\n\
- Schritte sind kurze, konkrete Anweisungen für einen Tester im Browser.\n\
- IDs im Format UC<Nummer>-<zweistellige laufende Nummer>, z. B. UC1-01.\n\
- automatable=false nur, wenn ein Punkt sich wirklich nicht im Browser prüfen lässt (z. B. Druckbild, echte SMS).\n\
- Schreibe auf Deutsch.\n{}\n\
Antworte AUSSCHLIESSLICH mit JSON in genau diesem Format, ohne weiteren Text:\n\
{{\"items\":[{{\"id\":\"UC1-01\",\"use_case\":\"<Titel des Use Case>\",\"title\":\"<kurzer Prüfpunkt>\",\
\"steps\":[\"<Schritt 1>\",\"<Schritt 2>\"],\"expected\":\"<erwartetes Ergebnis>\",\
\"priority\":\"hoch|mittel|niedrig\",\"automatable\":true}}]}}",
        describe_project(p),
        if settings.extra_instructions.trim().is_empty() {
            String::new()
        } else {
            format!("- Zusätzliche Vorgaben: {}\n", settings.extra_instructions.trim())
        }
    )
}

pub fn test_prompt(p: &Project, items: &[&CheckItem], artifacts: &str, settings: &Settings) -> String {
    let list: Vec<Value> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "id": i.id, "use_case": i.use_case, "title": i.title,
                "steps": i.steps.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>(),
                "expected": i.expected,
                "hinweis_vom_pruefer": i.retest_hint.trim(),
                "erzwingen": i.force,
                "letzter_blocker": if i.blocker.trim().is_empty() { i.auto_note.trim() } else { i.blocker.trim() },
            })
        })
        .collect();
    format!(
        "Du bist ein gewissenhafter Software-Tester und führst eine Abnahmeprüfung einer \
Web-Applikation durch. Steuere dazu den Browser ausschließlich über die Playwright-Werkzeuge \
(mcp__playwright__*).\n\n{}\n\n\
Zu prüfende Checkliste (JSON):\n{}\n\n\
Vorgehen:\n\
1. Arbeite die Prüfpunkte der Reihe nach ab. Führe die Schritte wirklich im Browser aus \
(navigieren, klicken, tippen, Snapshot lesen). Nutze für E-Mails die zusätzlichen URLs (z. B. MailHog) in einem eigenen Tab.\n\
2. Bewerte jeden Punkt streng anhand des erwarteten Ergebnisses: passed (vollständig erfüllt), \
failed (nicht erfüllt / Fehler), blocked (konnte objektiv nicht durchgeführt werden). \
Melde blocked nur als letzten Ausweg: versuche vorher Workarounds (Vorbedingungen selbst herstellen, Testdaten anlegen, anderen Weg nutzen). \
Bei blocked IMMER das Feld blocker ausfüllen: was genau den Test verhindert hat (z. B. Abhängigkeit von <ID>, fehlende Testdaten, fehlender Zugang, Fehlermeldung) \
und was nötig wäre, damit er durchgeführt werden kann.\n\
2b. Hat ein Prüfpunkt einen hinweis_vom_pruefer, befolge ihn vorrangig. Ist erzwingen=true, darfst du den Punkt NICHT wegen \
Abhängigkeiten oder fehlender Vorbedingungen als blocked melden: stelle die Vorbedingungen selbst her bzw. teste den Punkt \
isoliert und bewerte passed oder failed. Nur wenn es technisch absolut unmöglich ist, blocked mit exakter Begründung. \
letzter_blocker beschreibt, woran der vorige Versuch gescheitert ist.\n\
3. Mache bei failed oder blocked einen Screenshot mit einem Dateinamen wie <ID>.png.\n\
4. PFLICHT: Sofort nachdem ein Prüfpunkt bewertet ist, rufe das Werkzeug mcp__abnahme__ergebnis_melden auf \
(id, status passed|failed|blocked, note = konkrete Beobachtung, evidence = Screenshot-Dateinamen, blocker = Grund bei blocked). \
Ohne diesen Aufruf gilt der Punkt in der Abnahme als nicht geprüft. Melde JEDEN Punkt der Checkliste, auch blockierte.\n\
5. Wenn der Browser fehlt, rufe zuerst browser_install auf.\n\
6. Erfinde keine Ergebnisse. Ändere keine Daten außerhalb dessen, was der Testfall verlangt.\n\
7. Am Ende: Browser schließen, dann mcp__abnahme__zusammenfassung_melden aufrufen (Zusammenfassung, \
Empfehlung accept|accept_with_conditions|reject, Auflagen) und eine kurze Zusammenfassung auf Deutsch ausgeben.\n\
Screenshots und Dateien landen in: {}\n{}",
        describe_project(p),
        serde_json::to_string_pretty(&list).unwrap_or_default(),
        artifacts,
        if settings.extra_instructions.trim().is_empty() {
            String::new()
        } else {
            format!("\nZusätzliche Vorgaben: {}\n", settings.extra_instructions.trim())
        }
    )
}

fn spawn_reader(
    mut cmd: Command,
    tx: Sender<Ev>,
    log_file: PathBuf,
    child_slot: Arc<Mutex<Option<Child>>>,
    handler: impl FnMut(&str, &Sender<Ev>) + Send + 'static,
    finish: impl FnOnce(bool, &Sender<Ev>) + Send + 'static,
) {
    std::thread::spawn(move || {
        let mut handler = handler;
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Ev::Log(
                    LogKind::Error,
                    format!("Konnte das Programm nicht starten: {e}. Ist Claude Code installiert? (Einstellungen prüfen)"),
                ));
                let _ = tx.send(Ev::Done { ok: false, summary: format!("Start fehlgeschlagen: {e}"), cost: 0.0 });
                return;
            }
        };
        let mut child = child;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        *child_slot.lock().unwrap() = Some(child);

        let mut log = std::fs::File::create(&log_file).ok();
        if let Some(err) = stderr {
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().flatten() {
                    if !line.trim().is_empty() {
                        let _ = tx2.send(Ev::Log(LogKind::Error, line));
                    }
                }
            });
        }
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().flatten() {
                if let Some(f) = log.as_mut() {
                    use std::io::Write;
                    let _ = writeln!(f, "{line}");
                }
                handler(&line, &tx);
            }
        }
        let ok = child_slot
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|c| c.wait().ok())
            .map(|s| s.success())
            .unwrap_or(false);
        finish(ok, &tx);
    });
}

fn short(s: &str, n: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}

pub fn parse_result_lines(text: &str) -> Vec<ItemResult> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches(['*', '-', '`', ' ']);
        if let Some(rest) = l.strip_prefix("ERGEBNIS:") {
            let rest = rest.trim().trim_end_matches('`');
            if let Ok(v) = serde_json::from_str::<Value>(rest) {
                if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                    out.push(ItemResult {
                        id: id.to_string(),
                        status: Status::from_word(v.get("status").and_then(|x| x.as_str()).unwrap_or("")),
                        note: v.get("note").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        evidence: v
                            .get("evidence")
                            .and_then(|x| x.as_array())
                            .map(|a| a.iter().filter_map(|e| e.as_str().map(String::from)).collect())
                            .unwrap_or_default(),
                        blocker: v.get("blocker").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    });
                }
            }
        }
    }
    out
}

/// Find the first JSON object in a text (also inside ```json fences).
pub fn extract_json(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        return Some(v);
    }
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&text[start..=end]).ok()
}

pub fn parse_checklist(v: &Value) -> Vec<CheckItem> {
    let arr = v
        .get("items")
        .and_then(|x| x.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    let mut out = Vec::new();
    for (n, it) in arr.iter().enumerate() {
        let gs = |k: &str| it.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id = {
            let i = gs("id");
            if i.is_empty() { format!("P-{:02}", n + 1) } else { i }
        };
        let mut c = CheckItem::new(id, gs("use_case"));
        c.title = gs("title");
        c.steps = match it.get("steps") {
            Some(Value::Array(a)) => a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        c.expected = gs("expected");
        let pr = gs("priority");
        c.priority = if pr.is_empty() { "mittel".into() } else { pr };
        if it.get("automatable").and_then(|x| x.as_bool()) == Some(false) {
            c.status = Status::Manual;
            c.selected = false;
        }
        out.push(c);
    }
    out
}

/// What a run collected from Claude's output stream.
#[derive(Default)]
pub struct StreamState {
    pub result: String,
    pub cost: f64,
    pub ok: bool,
    pub transcript: String,
    pub reported: HashSet<String>,
    pub summary_seen: bool,
}

type Shared = Arc<Mutex<StreamState>>;

fn str_field(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub fn item_from_value(v: &Value) -> Option<ItemResult> {
    let id = str_field(v, "id");
    if id.is_empty() {
        return None;
    }
    Some(ItemResult {
        id,
        status: Status::from_word(&str_field(v, "status")),
        note: str_field(v, "note"),
        evidence: v
            .get("evidence")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|e| e.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        blocker: str_field(v, "blocker"),
    })
}

fn decision_from(s: &str) -> Option<Decision> {
    match s.trim() {
        "accept" => Some(Decision::Accepted),
        "accept_with_conditions" => Some(Decision::AcceptedWithConditions),
        "reject" => Some(Decision::Rejected),
        _ => None,
    }
}

/// Handle one line of `--output-format stream-json`.
fn handle_stream_line(line: &str, tx: &Sender<Ev>, state: &Shared) {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            if !line.trim().is_empty() {
                let _ = tx.send(Ev::Log(LogKind::Info, line.to_string()));
            }
            return;
        }
    };
    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "system" => {
            if v.get("subtype").and_then(|s| s.as_str()) == Some("init") {
                let model = v.get("model").and_then(|m| m.as_str()).unwrap_or("?");
                let mcp = v
                    .get("mcp_servers")
                    .and_then(|m| m.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|s| {
                                format!(
                                    "{} ({})",
                                    s.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
                                    s.get("status").and_then(|x| x.as_str()).unwrap_or("?")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let _ = tx.send(Ev::Log(LogKind::Info, format!("Claude Code gestartet · Modell {model} · Browser-Werkzeug: {mcp}")));
            }
        }
        "assistant" => {
            if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                for c in content {
                    match c.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                        "text" => {
                            let t = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            for r in parse_result_lines(t) {
                                state.lock().unwrap().reported.insert(r.id.clone());
                                let _ = tx.send(Ev::Item(r));
                            }
                            if !t.trim().is_empty() {
                                let mut st = state.lock().unwrap();
                                st.transcript.push_str(t.trim());
                                st.transcript.push_str("\n\n");
                                drop(st);
                                let _ = tx.send(Ev::Log(LogKind::Text, t.trim().to_string()));
                            }
                        }
                        "tool_use" => {
                            let full = c.get("name").and_then(|t| t.as_str()).unwrap_or("?");
                            let inp = c.get("input").cloned().unwrap_or(Value::Null);
                            if full == "mcp__abnahme__ergebnis_melden" {
                                if let Some(r) = item_from_value(&inp) {
                                    let mut st = state.lock().unwrap();
                                    st.reported.insert(r.id.clone());
                                    st.transcript.push_str(&format!("[Ergebnis {} = {:?}: {}]\n", r.id, r.status, r.note));
                                    drop(st);
                                    let _ = tx.send(Ev::Item(r));
                                }
                                continue;
                            }
                            if full == "mcp__abnahme__zusammenfassung_melden" {
                                let text = str_field(&inp, "summary");
                                state.lock().unwrap().summary_seen = true;
                                let _ = tx.send(Ev::Summary {
                                    text,
                                    recommendation: decision_from(&str_field(&inp, "recommendation")),
                                    conditions: str_field(&inp, "conditions"),
                                });
                                continue;
                            }
                            let name = full.trim_start_matches("mcp__playwright__");
                            let input = c.get("input").map(|i| i.to_string()).unwrap_or_default();
                            let _ = tx.send(Ev::Log(LogKind::Tool, format!("{name} {}", short(&input, 160))));
                        }
                        _ => {}
                    }
                }
            }
        }
        "user" => {
            if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                for c in content {
                    if c.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        let err = c.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                        let text = match c.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Array(a)) => a
                                .iter()
                                .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join(" "),
                            _ => String::new(),
                        };
                        if text.starts_with("Ergebnis für ") || text.starts_with("Zusammenfassung gespeichert") {
                            continue;
                        }
                        let kind = if err { LogKind::Error } else { LogKind::ToolResult };
                        let _ = tx.send(Ev::Log(kind, short(&text, 220)));
                    }
                }
            }
        }
        "result" => {
            let text = v.get("result").and_then(|r| r.as_str()).unwrap_or("").to_string();
            let cost = v.get("total_cost_usd").and_then(|c| c.as_f64()).unwrap_or(0.0);
            let is_err = v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
            let mut st = state.lock().unwrap();
            st.result = text;
            st.cost = cost;
            st.ok = !is_err;
        }
        _ => {}
    }
}

pub fn start_derive(p: &Project, settings: &Settings, path_env: &str) -> Job {
    let (tx, rx) = channel();
    let dir = crate::store::project_dir(&p.id);
    let log_file = dir.join(format!("ableitung-{}.jsonl", crate::store::stamp()));
    let mut cmd = base_command(settings, path_env);
    cmd.current_dir(&dir);
    cmd.args([
        "-p",
        &derive_prompt(p, settings),
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        settings.model.trim(),
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
    ]);
    let _ = tx.send(Ev::Log(LogKind::Info, "Claude Code leitet die Checkliste aus den Use Cases ab …".into()));
    let child = Arc::new(Mutex::new(None));
    let fin: Shared = Arc::new(Mutex::new(StreamState::default()));
    let fin2 = fin.clone();
    spawn_reader(
        cmd,
        tx,
        log_file.clone(),
        child.clone(),
        move |line, tx| handle_stream_line(line, tx, &fin),
        move |ok, tx| {
            let (text, cost, rok) = {
                let st = fin2.lock().unwrap();
                (st.result.clone(), st.cost, st.ok)
            };
            let items = extract_json(&text).map(|v| parse_checklist(&v)).unwrap_or_default();
            if items.is_empty() {
                let _ = tx.send(Ev::Done {
                    ok: false,
                    summary: if text.is_empty() {
                        "Claude Code hat keine Antwort geliefert (Anmeldung und Einstellungen prüfen).".into()
                    } else {
                        format!("Antwort war keine gültige Checkliste: {}", short(&text, 300))
                    },
                    cost,
                });
            } else {
                let n = items.len();
                let _ = tx.send(Ev::Checklist(items));
                let _ = tx.send(Ev::Done { ok: ok && rok, summary: format!("{n} Prüfpunkte abgeleitet."), cost });
            }
        },
    );
    Job {
        kind: JobKind::Derive,
        project_id: p.id.clone(),
        rx,
        child,
        started: crate::store::now_string(),
        log_file,
        ids: Vec::new(),
    }
}

pub fn start_test(p: &Project, ids: &[String], settings: &Settings, path_env: &str) -> Job {
    let (tx, rx) = channel();
    let dir = crate::store::project_dir(&p.id);
    let stamp = crate::store::stamp();
    let artifacts = dir.join("laeufe").join(&stamp);
    let _ = std::fs::create_dir_all(&artifacts);
    let log_file = artifacts.join("protokoll.jsonl");
    let items: Vec<&CheckItem> = p.checklist.iter().filter(|i| ids.contains(&i.id)).collect();
    let prompt = test_prompt(p, &items, &artifacts.to_string_lossy(), settings);

    let mut args = vec![
        "-y".to_string(),
        "@playwright/mcp@latest".into(),
        "--browser".into(),
        p.browser.playwright_name().into(),
        "--isolated".into(),
        "--output-dir".into(),
        artifacts.to_string_lossy().to_string(),
        "--viewport-size".into(),
        "1280x800".into(),
    ];
    if p.headless {
        args.push("--headless".into());
    }
    let npx = crate::store::which(path_env, "npx").unwrap_or_else(|| "npx".into());
    let me = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let mcp = serde_json::json!({
        "mcpServers": {
            "playwright": { "command": npx, "args": args, "env": { "PATH": path_env } },
            "abnahme": { "command": me, "args": ["--mcp-ergebnis"] }
        }
    });
    let mcp_file = artifacts.join("mcp.json");
    let _ = std::fs::write(&mcp_file, serde_json::to_string_pretty(&mcp).unwrap_or_default());

    let mut cmd = base_command(settings, path_env);
    cmd.current_dir(&artifacts);
    cmd.args([
        "-p",
        &prompt,
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        settings.model.trim(),
        "--mcp-config",
        &mcp_file.to_string_lossy(),
        "--strict-mcp-config",
        "--tools",
        "Read",
        "--allowedTools",
        "mcp__playwright mcp__abnahme Read",
        "--max-turns",
        &settings.max_turns.to_string(),
        "--no-session-persistence",
    ]);
    let _ = tx.send(Ev::Log(
        LogKind::Info,
        format!("Starte Testlauf mit {} Prüfpunkten in {} …", items.len(), p.browser.label()),
    ));
    let child = Arc::new(Mutex::new(None));
    let fin: Shared = Arc::new(Mutex::new(StreamState::default()));
    let fin2 = fin.clone();
    let wanted: Vec<(String, String)> = items.iter().map(|i| (i.id.clone(), i.title.clone())).collect();
    let settings2 = settings.clone();
    let path2 = path_env.to_string();
    let art2 = artifacts.clone();
    spawn_reader(
        cmd,
        tx,
        log_file.clone(),
        child.clone(),
        move |line, tx| handle_stream_line(line, tx, &fin),
        move |ok, tx| {
            let (text, mut cost, rok, transcript, mut reported, summary_seen) = {
                let st = fin2.lock().unwrap();
                (st.result.clone(), st.cost, st.ok, st.transcript.clone(), st.reported.clone(), st.summary_seen)
            };
            for r in parse_result_lines(&text) {
                reported.insert(r.id.clone());
                let _ = tx.send(Ev::Item(r));
            }
            let missing: Vec<(String, String)> = wanted.iter().filter(|(id, _)| !reported.contains(id)).cloned().collect();
            if (!missing.is_empty() || !summary_seen) && !transcript.trim().is_empty() {
                let _ = tx.send(Ev::Log(
                    LogKind::Info,
                    format!(
                        "{} Prüfpunkte wurden nicht direkt gemeldet – werte das Protokoll von Claude aus …",
                        missing.len()
                    ),
                ));
                match extract_results(&transcript, &missing, &settings2, &path2, &art2) {
                    Ok((results, summary, rec, cond, c2)) => {
                        cost += c2;
                        for r in results {
                            let _ = tx.send(Ev::Item(r));
                        }
                        if !summary_seen && !summary.trim().is_empty() {
                            let _ = tx.send(Ev::Summary { text: summary, recommendation: rec, conditions: cond });
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Ev::Log(LogKind::Error, format!("Auswertung fehlgeschlagen: {e}")));
                    }
                }
            }
            if !summary_seen && !text.trim().is_empty() {
                // keep Claude's final words as summary if nothing better arrived
                let _ = tx.send(Ev::Summary { text: text.clone(), recommendation: None, conditions: String::new() });
            }
            let _ = tx.send(Ev::Done {
                ok: ok && rok,
                summary: if text.trim().is_empty() { "Testlauf beendet.".into() } else { text },
                cost,
            });
        },
    );
    Job {
        kind: JobKind::Test,
        project_id: p.id.clone(),
        rx,
        child,
        started: crate::store::now_string(),
        log_file,
        ids: ids.to_vec(),
    }
}

/// Runs a simple helper command and streams its output (install Firefox driver, version check).
pub fn start_simple(kind: JobKind, program: &str, args: &[&str], path_env: &str, project_id: &str) -> Job {
    let (tx, rx) = channel();
    let exe = crate::store::which(path_env, program).unwrap_or_else(|| program.to_string());
    let mut cmd = Command::new(exe);
    cmd.args(args).env("PATH", path_env);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let log_file = std::env::temp_dir().join("web-abnahme-helper.log");
    let _ = tx.send(Ev::Log(LogKind::Info, format!("$ {program} {}", args.join(" "))));
    let child = Arc::new(Mutex::new(None));
    spawn_reader(
        cmd,
        tx,
        log_file.clone(),
        child.clone(),
        |line, tx| {
            let _ = tx.send(Ev::Log(LogKind::Text, line.to_string()));
        },
        |ok, tx| {
            let _ = tx.send(Ev::Done {
                ok,
                summary: if ok { "Fertig.".into() } else { "Mit Fehler beendet.".into() },
                cost: 0.0,
            });
        },
    );
    Job {
        kind,
        project_id: project_id.to_string(),
        rx,
        child,
        started: crate::store::now_string(),
        log_file,
        ids: Vec::new(),
    }
}

pub type Extracted = (Vec<ItemResult>, String, Option<Decision>, String, f64);

/// Fallback: ask Claude (no tools, structured output) to turn the run's transcript
/// into per-item results and a summary.
pub fn extract_results(
    transcript: &str,
    missing: &[(String, String)],
    settings: &Settings,
    path_env: &str,
    dir: &std::path::Path,
) -> Result<Extracted, String> {
    let tail: String = {
        let chars: Vec<char> = transcript.chars().collect();
        let start = chars.len().saturating_sub(80_000);
        chars[start..].iter().collect()
    };
    let list = missing.iter().map(|(id, t)| format!("- {id}: {t}")).collect::<Vec<_>>().join("\n");
    let prompt = format!(
        "Unten steht das Protokoll eines Browser-Testlaufs einer Abnahmeprüfung. Ermittle für jeden der folgenden \
Prüfpunkte das Ergebnis, so wie es im Protokoll festgestellt wurde: passed, failed oder blocked. Wenn ein Punkt \
im Protokoll gar nicht bewertet wurde, nimm not_tested und schreibe in note 'Im Testlauf nicht geprüft'. \
Bei blocked fülle blocker: was genau den Test verhindert hat und was nötig wäre, um ihn durchzuführen (aus dem Protokoll). \
note = konkrete Beobachtung auf Deutsch. Erfinde nichts.\n\nPrüfpunkte:\n{}\n\n\
Außerdem: summary = Zusammenfassung für das Abnahmeprotokoll, recommendation = accept | accept_with_conditions | reject, \
conditions = Auflagen/offene Punkte.\n\nPROTOKOLL:\n{}",
        if list.is_empty() { "(keine – nur Zusammenfassung)".to_string() } else { list },
        tail
    );
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "results": {"type": "array", "items": {"type": "object", "properties": {
                "id": {"type": "string"},
                "status": {"type": "string", "enum": ["passed", "failed", "blocked", "not_tested"]},
                "note": {"type": "string"},
                "blocker": {"type": "string"},
                "evidence": {"type": "array", "items": {"type": "string"}}
            }, "required": ["id", "status", "note"]}},
            "summary": {"type": "string"},
            "recommendation": {"type": "string", "enum": ["accept", "accept_with_conditions", "reject"]},
            "conditions": {"type": "string"}
        },
        "required": ["results", "summary", "recommendation"]
    });
    let mut cmd = base_command(settings, path_env);
    cmd.current_dir(dir);
    cmd.args([
        "-p",
        &prompt,
        "--output-format",
        "json",
        "--model",
        settings.model.trim(),
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--json-schema",
        &schema.to_string(),
    ]);
    let out = cmd.output().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("keine gültige Antwort ({e})"))?;
    let cost = v.get("total_cost_usd").and_then(|c| c.as_f64()).unwrap_or(0.0);
    let data = match v.get("structured_output") {
        Some(Value::Object(_)) => v["structured_output"].clone(),
        _ => extract_json(v.get("result").and_then(|r| r.as_str()).unwrap_or("")).ok_or("keine Ergebnisse gefunden")?,
    };
    let wanted: HashSet<&String> = missing.iter().map(|(id, _)| id).collect();
    let results = data
        .get("results")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(item_from_value).filter(|r| wanted.contains(&r.id)).collect())
        .unwrap_or_default();
    Ok((
        results,
        str_field(&data, "summary"),
        decision_from(&str_field(&data, "recommendation")),
        str_field(&data, "conditions"),
        cost,
    ))
}
