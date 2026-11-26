use crate::model::{Project, Status};
use std::path::PathBuf;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn find_evidence(p: &Project, name: &str) -> Option<PathBuf> {
    let pth = PathBuf::from(name);
    if pth.is_absolute() && pth.exists() {
        return Some(pth);
    }
    let base = crate::store::project_dir(&p.id).join("laeufe");
    let mut runs: Vec<PathBuf> = std::fs::read_dir(&base).ok()?.flatten().map(|e| e.path()).collect();
    runs.sort();
    for r in runs.iter().rev() {
        let f = r.join(name);
        if f.exists() {
            return Some(f);
        }
    }
    None
}

pub fn markdown(p: &Project) -> String {
    let (ok, fail, blk, man, open) = p.counts();
    let mut s = format!("# Abnahmeprotokoll – {}\n\n", p.name);
    s.push_str(&format!("- Ziel-URL: {}\n- Browser: {}\n- Erstellt: {}\n", p.target_url, p.browser.label(), crate::store::now_string()));
    if let Some(r) = p.runs.last() {
        s.push_str(&format!("- Letzter Testlauf: {} bis {}\n", r.started, r.finished));
    }
    s.push_str(&format!(
        "\n**Ergebnis:** {ok} bestanden · {fail} fehlgeschlagen · {blk} blockiert · {man} manuell · {open} offen (gesamt {})\n\n",
        p.checklist.len()
    ));
    if !p.claude_summary.trim().is_empty() {
        s.push_str("## Bewertung von Claude\n\n");
        if let Some(d) = p.claude_recommendation {
            s.push_str(&format!("Empfehlung: **{}**\n\n", d.label()));
        }
        s.push_str(&format!("{}\n\n", p.claude_summary.trim()));
        if !p.claude_conditions.trim().is_empty() {
            s.push_str(&format!("Auflagen / offene Punkte:\n{}\n\n", p.claude_conditions.trim()));
        }
    }
    s.push_str("## Use Cases\n\n");
    for (i, uc) in p.use_cases.iter().enumerate() {
        s.push_str(&format!("**UC{} – {}**  \n{}\n\n", i + 1, uc.title, uc.description));
    }
    s.push_str("## Checkliste\n\n| Status | ID | Prüfpunkt | Erwartet | Beobachtung | Bestätigt |\n|---|---|---|---|---|---|\n");
    for c in &p.checklist {
        let note = if c.reviewer_note.trim().is_empty() {
            c.auto_note.clone()
        } else {
            format!("{} — Prüfer: {}", c.auto_note, c.reviewer_note)
        };
        s.push_str(&format!(
            "| {} {} | {} | {} | {} | {} | {} |\n",
            c.status.icon(),
            c.status.label(),
            c.id,
            c.title.replace('|', "/"),
            c.expected.replace('|', "/").replace('\n', " "),
            note.replace('|', "/").replace('\n', " "),
            if c.confirmed { "✔" } else { "" }
        ));
    }
    s.push_str(&format!(
        "\n## Abnahme\n\n- Entscheidung: **{}**\n- Prüfer: {}\n- Datum: {}\n- Kommentar: {}\n",
        p.signoff.decision.label(),
        p.signoff.reviewer,
        p.signoff.date,
        p.signoff.comment
    ));
    s
}

pub fn html(p: &Project) -> String {
    let (ok, fail, blk, man, open) = p.counts();
    let mut s = String::new();
    s.push_str("<!doctype html><html lang=de><meta charset=utf-8>");
    s.push_str(&format!("<title>Abnahmeprotokoll – {}</title>", esc(&p.name)));
    s.push_str("<style>body{font:14px -apple-system,Helvetica,sans-serif;max-width:1000px;margin:32px auto;color:#1d2433;padding:0 16px}\
h1{margin-bottom:4px}.meta{color:#667}table{border-collapse:collapse;width:100%;margin-top:12px}\
td,th{border-bottom:1px solid #e3e6ee;padding:8px;vertical-align:top;text-align:left}th{background:#f4f6fa}\
.s{font-weight:600;white-space:nowrap}.Passed{color:#1a8f4c}.Failed{color:#c62f3a}.Blocked{color:#b7791f}.Manual{color:#5161c9}.Open{color:#778}\
.sum span{display:inline-block;margin-right:16px;padding:6px 10px;border-radius:6px;background:#f4f6fa}\
img{max-width:320px;border:1px solid #ccd;border-radius:4px;margin-top:6px}.box{background:#f4f6fa;border-radius:8px;padding:12px 16px}\
pre{white-space:pre-wrap;font:inherit;margin:0}</style>");
    s.push_str(&format!("<h1>Abnahmeprotokoll – {}</h1>", esc(&p.name)));
    s.push_str(&format!(
        "<div class=meta>Ziel-URL: {} · Browser: {} · Erstellt: {}</div>",
        esc(&p.target_url),
        p.browser.label(),
        crate::store::now_string()
    ));
    s.push_str(&format!(
        "<p class=sum><span class=Passed>✔ {ok} bestanden</span><span class=Failed>✖ {fail} fehlgeschlagen</span>\
<span class=Blocked>⚠ {blk} blockiert</span><span class=Manual>☝ {man} manuell</span><span class=Open>○ {open} offen</span></p>"
    ));
    if !p.claude_summary.trim().is_empty() {
        s.push_str("<h2>Bewertung von Claude</h2><div class=box>");
        if let Some(d) = p.claude_recommendation {
            s.push_str(&format!("<b>Empfehlung: {}</b><br>", d.label()));
        }
        s.push_str(&format!("<pre>{}</pre>", esc(&crate::theme::plain(&p.claude_summary))));
        if !p.claude_conditions.trim().is_empty() {
            s.push_str(&format!("<p><b>Auflagen / offene Punkte</b></p><pre>{}</pre>", esc(&crate::theme::plain(&p.claude_conditions))));
        }
        s.push_str("</div>");
    }
    s.push_str("<h2>Use Cases</h2>");
    for (i, uc) in p.use_cases.iter().enumerate() {
        s.push_str(&format!("<p><b>UC{} – {}</b><br>{}</p>", i + 1, esc(&uc.title), esc(&uc.description)));
    }
    s.push_str("<h2>Checkliste</h2><table><tr><th>Status</th><th>ID</th><th>Prüfpunkt</th><th>Erwartet</th><th>Beobachtung</th></tr>");
    for c in &p.checklist {
        let cls = format!("{:?}", if c.status == Status::Running { Status::Open } else { c.status });
        let mut obs = esc(&c.auto_note);
        if !c.reviewer_note.trim().is_empty() {
            obs.push_str(&format!("<br><i>Prüfer: {}</i>", esc(&c.reviewer_note)));
        }
        if c.confirmed {
            obs.push_str("<br><small>✔ vom Prüfer bestätigt</small>");
        }
        for e in &c.evidence {
            if let Some(f) = find_evidence(p, e) {
                obs.push_str(&format!("<br><a href=\"file://{0}\"><img src=\"file://{0}\"></a>", esc(&f.to_string_lossy())));
            }
        }
        s.push_str(&format!(
            "<tr><td class='s {cls}'>{} {}</td><td>{}</td><td><b>{}</b><pre>{}</pre></td><td>{}</td><td>{}</td></tr>",
            c.status.icon(),
            c.status.label(),
            esc(&c.id),
            esc(&c.title),
            esc(&c.steps),
            esc(&c.expected),
            obs
        ));
    }
    s.push_str("</table>");
    s.push_str(&format!(
        "<h2>Abnahme</h2><div class=box><b>{}</b><br>Prüfer: {}<br>Datum: {}<br><pre>{}</pre></div>",
        p.signoff.decision.label(),
        esc(&p.signoff.reviewer),
        esc(&p.signoff.date),
        esc(&p.signoff.comment)
    ));
    s.push_str("</html>");
    s
}

/// Writes both report files into the project folder and returns the HTML path.
pub fn export(p: &Project) -> Result<PathBuf, String> {
    let dir = crate::store::project_dir(&p.id).join("berichte");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = crate::store::stamp();
    let md = dir.join(format!("abnahme-{stamp}.md"));
    let ht = dir.join(format!("abnahme-{stamp}.html"));
    std::fs::write(&md, markdown(p)).map_err(|e| e.to_string())?;
    std::fs::write(&ht, html(p)).map_err(|e| e.to_string())?;
    Ok(ht)
}
