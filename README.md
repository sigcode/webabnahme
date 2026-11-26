# Web-Abnahme

Native macOS-App: Use Cases in eigenen Worten beschreiben, Claude Code leitet daraus eine
Abnahme-Checkliste ab, testet die Web-Applikation automatisiert in Chrome oder Firefox
(über Playwright) und füllt Checkliste und Abnahme aus. Aus den nicht erfüllten Punkten
entsteht auf Wunsch eine Vorgabe für einen Fix-Agenten.

## Starten

`dist/Web-Abnahme.app` per Doppelklick oder über Spotlight („Web-Abnahme“).

Voraussetzungen: Claude Code (`claude`, angemeldet) und Node.js (`npx`) für die Browser-Steuerung.

## Bauen

```sh
./build-app.sh            # baut dist/Web-Abnahme.app
./build-app.sh --install  # zusätzlich nach ~/Applications kopieren
```

Der Git-Hook in `packaging/git-hooks/pre-commit` baut die App vor jedem Commit neu und
legt sie mit ins Repository (`git config core.hooksPath packaging/git-hooks`).

Hinweis: Die Abhängigkeiten sind für Rust 1.68 festgepinnt (`Cargo.lock`, `pins/`).
