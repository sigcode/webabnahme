use crate::model::{Project, Settings};
use std::path::PathBuf;
use std::process::Command;

pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = PathBuf::from(home).join("Library/Application Support/Web-Abnahme");
    let _ = std::fs::create_dir_all(dir.join("projects"));
    dir
}

pub fn project_dir(id: &str) -> PathBuf {
    let d = data_dir().join("projects").join(id);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn load_projects() -> Vec<Project> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(data_dir().join("projects")) {
        for e in rd.flatten() {
            let f = e.path().join("project.json");
            if let Ok(text) = std::fs::read_to_string(&f) {
                if let Ok(p) = serde_json::from_str::<Project>(&text) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.created.cmp(&b.created));
    out
}

pub fn save_project(p: &Project) -> Result<(), String> {
    let f = project_dir(&p.id).join("project.json");
    let text = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    std::fs::write(f, text).map_err(|e| e.to_string())
}

pub fn delete_project(id: &str) {
    let d = data_dir().join("projects").join(id);
    let _ = std::fs::remove_dir_all(d);
}

pub fn load_settings() -> Settings {
    std::fs::read_to_string(data_dir().join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    if let Ok(t) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(data_dir().join("settings.json"), t);
    }
}

pub fn new_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("p{t}")
}

fn date_cmd(fmt: &str) -> String {
    Command::new("/bin/date")
        .arg(fmt)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn now_string() -> String {
    date_cmd("+%Y-%m-%d %H:%M")
}

pub fn stamp() -> String {
    date_cmd("+%Y%m%d-%H%M%S")
}

/// Apps started from Finder get a tiny PATH; ask the login shell for the real one
/// so `claude`, `node` and `npx` are found.
pub fn login_path() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let out = Command::new(&shell)
        .args(["-lic", "printf '__PATH__%s__END__' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        if let (Some(a), Some(b)) = (s.find("__PATH__"), s.find("__END__")) {
            if b > a + 8 {
                return s[a + 8..b].to_string();
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    format!(
        "{home}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:{}",
        std::env::var("PATH").unwrap_or_default()
    )
}

pub fn which(path_env: &str, name: &str) -> Option<String> {
    if name.contains('/') {
        return if std::path::Path::new(name).exists() { Some(name.into()) } else { None };
    }
    for d in path_env.split(':') {
        let p = std::path::Path::new(d).join(name);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

pub fn open_path(p: &str) {
    let _ = Command::new("/usr/bin/open").arg(p).spawn();
}

pub fn reveal(p: &str) {
    let _ = Command::new("/usr/bin/open").arg("-R").arg(p).spawn();
}
