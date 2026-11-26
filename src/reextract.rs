//! Re-read the stream log of an earlier run and fill the checklist and the
//! acceptance assessment from it (without testing again).

use crate::claude::{extract_results, item_from_value, Ev, Job, JobKind, LogKind};
use crate::model::{Project, Settings};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn transcript_of(text: &str) -> String {
    let mut transcript = String::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else { continue };
                for c in content {
                    match c.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            transcript.push_str(c.get("text").and_then(|t| t.as_str()).unwrap_or(""));
                            transcript.push_str("\n\n");
                        }
                        Some("tool_use") if c.get("name").and_then(|n| n.as_str()) == Some("mcp__abnahme__ergebnis_melden") => {
                            if let Some(r) = c.get("input").and_then(item_from_value) {
                                transcript.push_str(&format!("[Ergebnis {} = {:?}: {}]\n", r.id, r.status, r.note));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("result") => {
                transcript.push_str(v.get("result").and_then(|r| r.as_str()).unwrap_or(""));
            }
            _ => {}
        }
    }
    transcript
}

pub fn start(p: &Project, log_file: &str, only: Option<Vec<String>>, settings: &Settings, path_env: &str) -> Job {
    let (tx, rx) = channel();
    let with_summary = only.is_none();
    let wanted: Vec<(String, String)> = p
        .checklist
        .iter()
        .filter(|i| only.as_ref().map(|o| o.contains(&i.id)).unwrap_or(true))
        .map(|i| (i.id.clone(), i.title.clone()))
        .collect();
    let ids: Vec<String> = wanted.iter().map(|w| w.0.clone()).collect();
    let file = PathBuf::from(log_file);
    let dir = file.parent().map(|d| d.to_path_buf()).unwrap_or_else(std::env::temp_dir);
    let settings = settings.clone();
    let path_env = path_env.to_string();
    let _ = tx.send(Ev::Log(LogKind::Info, "Werte das Protokoll des Laufs aus und fülle Checkliste und Abnahme …".into()));
    std::thread::spawn(move || {
        let transcript = transcript_of(&std::fs::read_to_string(&file).unwrap_or_default());
        if transcript.trim().is_empty() {
            let _ = tx.send(Ev::Done { ok: false, summary: "Im Protokoll dieses Laufs steht nichts Auswertbares.".into(), cost: 0.0 });
            return;
        }
        match extract_results(&transcript, &wanted, &settings, &path_env, &dir) {
            Ok((results, summary, rec, cond, cost)) => {
                let n = results.len();
                for r in results {
                    let _ = tx.send(Ev::Item(r));
                }
                if with_summary && !summary.trim().is_empty() {
                    let _ = tx.send(Ev::Summary { text: summary, recommendation: rec, conditions: cond });
                }
                let _ = tx.send(Ev::Done { ok: true, summary: format!("{n} Prüfpunkte aus dem Protokoll übernommen."), cost });
            }
            Err(e) => {
                let _ = tx.send(Ev::Done { ok: false, summary: format!("Auswertung fehlgeschlagen: {e}"), cost: 0.0 });
            }
        }
    });
    Job {
        kind: JobKind::Extract,
        project_id: p.id.clone(),
        rx,
        child: Arc::new(Mutex::new(None)),
        started: crate::store::now_string(),
        log_file: PathBuf::new(),
        ids,
    }
}
