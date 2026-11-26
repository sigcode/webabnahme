//! The app's own source repository: finds it, watches its state in the
//! background (uncommitted changes, commits not pushed) and offers commit & push.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct RepoStatus {
    pub path: PathBuf,
    pub branch: String,
    pub upstream: String,
    /// `git status --porcelain` lines (uncommitted changes)
    pub changes: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub last_commit: String,
    pub remote_url: String,
    pub error: Option<String>,
}

impl RepoStatus {
    pub fn needs_commit(&self) -> bool {
        !self.changes.is_empty()
    }
    pub fn needs_push(&self) -> bool {
        self.ahead > 0 || (self.upstream.is_empty() && !self.remote_url.is_empty() && !self.last_commit.is_empty())
    }
}

/// Folder that holds Cargo.toml and .git, found from the setting or by walking up
/// from the running program (works for target/… and dist/Web-Abnahme.app/…).
pub fn detect(setting: &str) -> Option<PathBuf> {
    let s = setting.trim();
    if !s.is_empty() {
        let p = PathBuf::from(s);
        return if p.join(".git").exists() { Some(p) } else { None };
    }
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    exe.ancestors()
        .find(|d| d.join(".git").exists() && d.join("Cargo.toml").exists())
        .map(Path::to_path_buf)
}

fn git(path: &Path, path_env: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("PATH", path_env)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git nicht gefunden: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn status(path: &Path, path_env: &str) -> RepoStatus {
    let mut st = RepoStatus { path: path.to_path_buf(), ..Default::default() };
    match git(path, path_env, &["status", "--porcelain"]) {
        Ok(s) => st.changes = s.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect(),
        Err(e) => {
            st.error = Some(e);
            return st;
        }
    }
    st.branch = git(path, path_env, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    st.upstream = git(path, path_env, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).unwrap_or_default();
    st.remote_url = git(path, path_env, &["remote", "get-url", "origin"]).unwrap_or_default();
    st.last_commit = git(path, path_env, &["log", "-1", "--format=%h · %s · %cr"]).unwrap_or_default();
    if !st.upstream.is_empty() {
        if let Ok(s) = git(path, path_env, &["rev-list", "--left-right", "--count", "HEAD...@{u}"]) {
            let mut it = s.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
            st.ahead = it.next().unwrap_or(0);
            st.behind = it.next().unwrap_or(0);
        }
    }
    st
}

/// Keeps a fresh status in the background (every 15 s, or sooner on request).
pub struct Watcher {
    pub status: Arc<Mutex<Option<RepoStatus>>>,
    pub refresh: Arc<Mutex<bool>>,
    pub setting: Arc<Mutex<String>>,
}

impl Watcher {
    pub fn start(path_env: Arc<Mutex<Option<String>>>, setting: String) -> Watcher {
        let status = Arc::new(Mutex::new(None));
        let refresh = Arc::new(Mutex::new(true));
        let setting = Arc::new(Mutex::new(setting));
        let (s2, r2, set2) = (status.clone(), refresh.clone(), setting.clone());
        std::thread::spawn(move || {
            let mut last = Instant::now() - Duration::from_secs(60);
            loop {
                let due = last.elapsed() > Duration::from_secs(15) || *r2.lock().unwrap();
                let pe = path_env.lock().unwrap().clone();
                if due {
                    if let Some(pe) = pe {
                        *r2.lock().unwrap() = false;
                        last = Instant::now();
                        let setting = set2.lock().unwrap().clone();
                        let st = detect(&setting).map(|p| self::status(&p, &pe));
                        *s2.lock().unwrap() = st;
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        Watcher { status, refresh, setting }
    }

    pub fn get(&self) -> Option<RepoStatus> {
        self.status.lock().unwrap().clone()
    }

    pub fn poke(&self) {
        *self.refresh.lock().unwrap() = true;
    }

    pub fn set_path(&self, s: &str) {
        *self.setting.lock().unwrap() = s.to_string();
        self.poke();
    }
}

/// Shell script for the commit/push job. $1 = repo folder, $2 = commit message.
pub const COMMIT_AND_PUSH: &str = r#"set -e
export GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes"
exec 2>&1
cd "$1"
git add -A
if ! git diff --cached --quiet; then
  echo "→ Commit (der Git-Hook baut dabei die App neu, das dauert etwas) …"
  git commit -m "$2"
fi
echo "→ Push …"
if git rev-parse --abbrev-ref --symbolic-full-name @{u} >/dev/null 2>&1; then
  git push
else
  git push -u origin "$(git rev-parse --abbrev-ref HEAD)"
fi
echo "✔ Fertig."
"#;

pub const PUSH_ONLY: &str = r#"set -e
export GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes"
exec 2>&1
cd "$1"
if git rev-parse --abbrev-ref --symbolic-full-name @{u} >/dev/null 2>&1; then
  git push
else
  git push -u origin "$(git rev-parse --abbrev-ref HEAD)"
fi
echo "✔ Fertig."
"#;

pub const FETCH: &str = r#"set -e
export GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes"
exec 2>&1
cd "$1"
git fetch --prune
echo "✔ Abgeglichen."
"#;
