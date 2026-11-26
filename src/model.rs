use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Browser {
    Chrome,
    Firefox,
}

impl Browser {
    pub fn label(self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Firefox => "Firefox",
        }
    }
    pub fn playwright_name(self) -> &'static str {
        match self {
            Browser::Chrome => "chrome",
            Browser::Firefox => "firefox",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Open,
    Running,
    Passed,
    Failed,
    Blocked,
    Manual,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Open => "Offen",
            Status::Running => "Läuft",
            Status::Passed => "Bestanden",
            Status::Failed => "Fehlgeschlagen",
            Status::Blocked => "Blockiert",
            Status::Manual => "Manuell prüfen",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Status::Open => "○",
            Status::Running => "◌",
            Status::Passed => "✔",
            Status::Failed => "✖",
            Status::Blocked => "⚠",
            Status::Manual => "☝",
        }
    }
    pub fn all() -> [Status; 5] {
        [Status::Open, Status::Passed, Status::Failed, Status::Blocked, Status::Manual]
    }
    pub fn from_word(s: &str) -> Status {
        let s = s.trim().to_lowercase();
        if s.starts_with("pass") || s.starts_with("bestanden") || s == "ok" || s == "success" {
            Status::Passed
        } else if s.starts_with("fail") || s.starts_with("fehl") {
            Status::Failed
        } else if s.starts_with("block") || s.starts_with("skip") {
            Status::Blocked
        } else if s.starts_with("manu") {
            Status::Manual
        } else {
            Status::Open
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtraUrl {
    pub label: String,
    pub url: String,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UseCase {
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckItem {
    pub id: String,
    pub use_case: String,
    pub title: String,
    pub steps: String,
    pub expected: String,
    #[serde(default)]
    pub priority: String,
    pub status: Status,
    #[serde(default)]
    pub auto_note: String,
    #[serde(default)]
    pub reviewer_note: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default = "yes")]
    pub selected: bool,
    #[serde(default)]
    pub tested_at: String,
    /// Hand this item to the fix agent? None = default (ticked when failed).
    #[serde(default)]
    pub fix_selected: Option<bool>,
    /// Why Claude marked it blocked and what is needed to test it.
    #[serde(default)]
    pub blocker: String,
    /// Reviewer's instruction for the next test of this item.
    #[serde(default)]
    pub retest_hint: String,
    /// Test it anyway: Claude must not mark it blocked because of dependencies.
    #[serde(default)]
    pub force: bool,
}

fn yes() -> bool {
    true
}

impl CheckItem {
    pub fn new(id: String, use_case: String) -> Self {
        CheckItem {
            id,
            use_case,
            title: String::new(),
            steps: String::new(),
            expected: String::new(),
            priority: "mittel".into(),
            status: Status::Open,
            auto_note: String::new(),
            reviewer_note: String::new(),
            evidence: Vec::new(),
            confirmed: false,
            selected: true,
            tested_at: String::new(),
            fix_selected: None,
            blocker: String::new(),
            retest_hint: String::new(),
            force: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Pending,
    Accepted,
    AcceptedWithConditions,
    Rejected,
}

impl Decision {
    pub fn label(self) -> &'static str {
        match self {
            Decision::Pending => "Noch offen",
            Decision::Accepted => "Abgenommen",
            Decision::AcceptedWithConditions => "Abgenommen mit Auflagen",
            Decision::Rejected => "Nicht abgenommen",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signoff {
    pub reviewer: String,
    pub decision: Decision,
    pub comment: String,
    pub date: String,
}

impl Default for Signoff {
    fn default() -> Self {
        Signoff {
            reviewer: String::new(),
            decision: Decision::Pending,
            comment: String::new(),
            date: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunRecord {
    pub started: String,
    pub finished: String,
    pub summary: String,
    pub passed: usize,
    pub failed: usize,
    pub blocked: usize,
    pub cost_usd: f64,
    pub log_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub target_url: String,
    pub browser: Browser,
    pub headless: bool,
    pub extra_urls: Vec<ExtraUrl>,
    pub test_data: String,
    pub use_cases: Vec<UseCase>,
    pub checklist: Vec<CheckItem>,
    #[serde(default)]
    pub runs: Vec<RunRecord>,
    #[serde(default)]
    pub signoff: Signoff,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub claude_summary: String,
    #[serde(default)]
    pub claude_recommendation: Option<Decision>,
    #[serde(default)]
    pub claude_conditions: String,
    #[serde(default)]
    pub fix_brief: String,
    #[serde(default)]
    pub fix_brief_ids: Vec<String>,
}

impl Project {
    pub fn new(id: String, name: String) -> Self {
        Project {
            id,
            name,
            target_url: "https://".into(),
            browser: Browser::Chrome,
            headless: false,
            extra_urls: Vec::new(),
            test_data: String::new(),
            use_cases: vec![UseCase::default()],
            checklist: Vec::new(),
            runs: Vec::new(),
            signoff: Signoff::default(),
            created: crate::store::now_string(),
            claude_summary: String::new(),
            claude_recommendation: None,
            claude_conditions: String::new(),
            fix_brief: String::new(),
            fix_brief_ids: Vec::new(),
        }
    }

    pub fn example(id: String) -> Self {
        let mut p = Project::new(id, "Beispiel: Registrierung mit E-Mail".into());
        p.target_url = "http://localhost:3000".into();
        p.extra_urls = vec![ExtraUrl {
            label: "MailHog".into(),
            url: "http://localhost:8025".into(),
            note: "Empfängt alle E-Mails der Test-Umgebung".into(),
        }];
        p.test_data = "Test-Benutzer: anna.test+{zufall}@example.org\nPasswort: Sicher!2024".into();
        p.use_cases = vec![
            UseCase {
                title: "Registrierung".into(),
                description: "Ein neuer Besucher kann sich mit E-Mail und Passwort registrieren. \
                    Danach bekommt er eine Bestätigungs-Mail. Erst nach Klick auf den Link in der Mail \
                    kann er sich anmelden."
                    .into(),
            },
            UseCase {
                title: "Passwort vergessen".into(),
                description: "Auf der Anmeldeseite gibt es 'Passwort vergessen'. Nach Eingabe der E-Mail \
                    kommt eine Mail mit einem Link, über den man ein neues Passwort setzen kann."
                    .into(),
            },
        ];
        let rows = [
            ("UC1-01", "Registrierung", "Registrierungsformular ist erreichbar", "Startseite öffnen\nAuf 'Registrieren' klicken", "Formular mit E-Mail und Passwort wird angezeigt", "hoch", Status::Passed, "Formular erscheint unter /register."),
            ("UC1-02", "Registrierung", "Registrierung mit gültigen Daten", "Formular mit Test-Benutzer ausfüllen\nAbsenden", "Hinweis 'Bitte E-Mail bestätigen' erscheint", "hoch", Status::Passed, "Hinweis wird angezeigt."),
            ("UC1-03", "Registrierung", "Bestätigungs-Mail kommt in MailHog an", "MailHog öffnen\nNeueste Mail an Test-Benutzer öffnen", "Mail mit Bestätigungslink ist vorhanden", "hoch", Status::Failed, "Keine Mail nach 60 Sekunden in MailHog."),
            ("UC1-04", "Registrierung", "Anmeldung vor Bestätigung wird verhindert", "Mit Test-Benutzer anmelden", "Fehlermeldung 'Konto nicht bestätigt'", "mittel", Status::Blocked, "Hängt von UC1-03 ab."),
            ("UC2-01", "Passwort vergessen", "Link 'Passwort vergessen' vorhanden", "Anmeldeseite öffnen", "Link ist sichtbar und führt zum Formular", "mittel", Status::Open, ""),
            ("UC2-02", "Passwort vergessen", "Reset-Mail und neues Passwort setzen", "E-Mail eingeben\nMail in MailHog öffnen\nLink klicken\nNeues Passwort setzen", "Anmeldung mit neuem Passwort klappt", "hoch", Status::Open, ""),
        ];
        for (id, uc, t, s, e, pr, st, note) in rows {
            let mut c = CheckItem::new(id.into(), uc.into());
            c.title = t.into();
            c.steps = s.into();
            c.expected = e.into();
            c.priority = pr.into();
            c.status = st;
            c.auto_note = note.into();
            p.checklist.push(c);
        }
        p
    }

    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0, 0);
        for i in &self.checklist {
            match i.status {
                Status::Passed => c.0 += 1,
                Status::Failed => c.1 += 1,
                Status::Blocked => c.2 += 1,
                Status::Manual => c.3 += 1,
                Status::Open | Status::Running => c.4 += 1,
            }
        }
        c
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub claude_path: String,
    pub model: String,
    pub max_turns: u32,
    pub extra_instructions: String,
    /// Folder of the app's own git repository (empty = find automatically).
    #[serde(default)]
    pub repo_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            claude_path: "claude".into(),
            model: "sonnet".into(),
            max_turns: 200,
            extra_instructions: String::new(),
            repo_path: String::new(),
        }
    }
}
