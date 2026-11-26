#!/bin/zsh
# Builds the release binary and packages it as dist/Web-Abnahme.app
# (found by Spotlight, double-clickable). Optional: --install also copies
# the app to ~/Applications.
set -euo pipefail
cd "${0:A:h}"

APP="dist/Web-Abnahme.app"
VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
BUILD=$(date +%Y%m%d%H%M)

echo "→ Baue Release ($VERSION, $BUILD) …"
cargo build --release

echo "→ Packe $APP …"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# copy then rename, so a running copy of the app keeps working while it is replaced
cp -f target/release/web-abnahme "$APP/Contents/MacOS/web-abnahme.new"
mv -f "$APP/Contents/MacOS/web-abnahme.new" "$APP/Contents/MacOS/web-abnahme"
cp -f packaging/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
sed -e "s/__VERSION__/$VERSION/" -e "s/__BUILD__/$BUILD/" packaging/Info.plist > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# ad-hoc signature so macOS starts it without complaints on this Mac
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || echo "  (Signieren übersprungen)"
touch "$APP"
mdimport "$APP" >/dev/null 2>&1 || true

if [[ "${1:-}" == "--install" ]]; then
  mkdir -p "$HOME/Applications"
  ditto "$APP" "$HOME/Applications/Web-Abnahme.app"
  mdimport "$HOME/Applications/Web-Abnahme.app" >/dev/null 2>&1 || true
  echo "→ Installiert nach ~/Applications/Web-Abnahme.app"
fi
echo "✔ Fertig: $APP"
