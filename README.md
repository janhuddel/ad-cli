# ad-cli

[![CI](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/janhuddel/ad-cli?include_prereleases)](https://github.com/janhuddel/ad-cli/releases)

> **Beta:** Das Tool ist noch nicht gegen ein echtes Active Directory getestet. Bitte zunächst in einer Test-Umgebung ausprobieren.

Kommandozeilen-Tool zum Auslesen von Active-Directory-Daten über LDAPS: Benutzerdetails und Gruppenmitgliedschaften (inkl. verschachtelter/rekursiver Mitgliedschaft). Primär für Windows gebaut, läuft ebenso unter Linux.

## Funktionen

- **`ad login`** – einmalige Anmeldung am AD; Verbindungsdaten und Zugangsdaten werden danach lokal im Home-Verzeichnis gespeichert, sodass spätere Aufrufe keine erneute Eingabe benötigen.
- **`ad user <id>`** – zeigt Details eines Benutzers (Name, Mail, Titel, Abteilung, Account-Status, letzter Login, Ablaufdatum, …) als kompakte Übersicht, alternativ als ausführliche Tabelle oder JSON.
- **`ad groups <id>`** – listet die Gruppenmitgliedschaften eines Benutzers. Bei vielen (100+) Gruppen interaktiv durchsuchbar (Fuzzy-Filter), alternativ als CSV/JSON für die Weiterverarbeitung in Skripten. Optional rekursiv (`--recursive`), um auch verschachtelte Mitgliedschaften aufzulösen.
- **`ad whoami`** / **`ad logout`** – aktuelle Verbindung anzeigen bzw. gespeicherte Zugangsdaten entfernen.

## Download

Fertige Binaries gibt es auf der [Releases-Seite](https://github.com/janhuddel/ad-cli/releases) unter *Assets*. Beide sind statisch gelinkt und brauchen keine weiteren Abhängigkeiten (unter Windows keine Visual-C++-Runtime):

| Plattform | Direkt-Download | Archiv (inkl. README, LICENSE, CHANGELOG) |
|---|---|---|
| Windows (x86_64) | `ad-<version>-x86_64-windows.exe` | `ad-<version>-x86_64-windows.zip` |
| Linux (x86_64) | `ad-<version>-x86_64-linux` | `ad-<version>-x86_64-linux.tar.gz` |

Die Datei kann nach dem Download beliebig umbenannt werden, z. B. in `ad.exe` bzw. `ad` (unter Linux danach `chmod +x ad`).

Die Prüfsummen aller Dateien stehen in `SHA256SUMS.txt`:

```sh
sha256sum -c --ignore-missing SHA256SUMS.txt
```
```powershell
(Get-FileHash ad-<version>-x86_64-windows.exe -Algorithm SHA256).Hash   # mit dem Eintrag in SHA256SUMS.txt vergleichen
```

**macOS wird derzeit nicht unterstützt:** Die Verschlüsselung der gespeicherten Zugangsdaten setzt unter Unix `/etc/machine-id` voraus, die es auf macOS nicht gibt. Ein Build ist möglich, `ad login` schlägt dort aber fehl.

## Build aus dem Quellcode

Benötigt wird ein Rust-Toolchain (`rustup`, stable).

```sh
cargo build --release
```

Das fertige Binary liegt danach unter `target/release/ad` (bzw. `ad.exe` unter Windows).

### Build für Windows (Cross-Compile von macOS/Linux)

```sh
rustup target add x86_64-pc-windows-gnu
# macOS: mingw-w64-Toolchain wird benötigt (brew install mingw-w64)
cargo build --release --target x86_64-pc-windows-gnu
# -> target/x86_64-pc-windows-gnu/release/ad.exe
```

Für ein natives Windows-Release mit statisch gelinktem CRT (kein MinGW-Runtime-Abhängigkeit) direkt auf einer Windows-Maschine oder in CI:

```sh
$env:RUSTFLAGS="-C target-feature=+crt-static"
cargo build --release --target x86_64-pc-windows-msvc
```

### Statisches Linux-Binary

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Verwendung

### Anmelden

```sh
ad login
```

Fragt interaktiv nach:
- **AD-Server-Host** (z. B. `dc01.corp.example.com`)
- **Base DN** (z. B. `DC=corp,DC=example,DC=com`)
- **Bind-Identität** – UPN (`user@corp.example.com`), `DOMAIN\user` oder eine vollständige Bind-DN; wird unverändert an den LDAP-Bind übergeben
- **Passwort** (maskierte Eingabe)

Alle Werte lassen sich auch als Flags übergeben (`--host`, `--base-dn`, `--bind-identity`, `--port`), z. B. für automatisierte Logins mit `--password-stdin`:

```sh
echo "$PASSWORT" | ad login --host dc01.corp.example.com --base-dn "DC=corp,DC=example,DC=com" --bind-identity user@corp.example.com --password-stdin
```

Verbindung erfolgt per LDAPS (Port 636). Für Labor-/Testumgebungen mit selbstsigniertem Zertifikat kann `--insecure-skip-verify` genutzt werden (deaktiviert die Zertifikatsprüfung – nicht für produktive Umgebungen).

Erst nach erfolgreichem Bind werden Verbindungsdaten und Passwort gespeichert. Jeder andere Befehl (`user`, `groups`, `whoami`) bricht mit einer klaren Fehlermeldung ab, solange kein erfolgreicher Login stattgefunden hat.

### Benutzerdetails abfragen

```sh
ad user jdoe                 # kompakte Übersicht (Standard)
ad user jdoe --output table  # ausführliche Tabelle
ad user jdoe --output json
```

### Gruppenmitgliedschaften abfragen

```sh
ad groups jdoe                 # interaktiver Fuzzy-Filter im Terminal
ad groups jdoe --recursive     # inkl. verschachtelter Gruppen
ad groups jdoe --output csv > gruppen.csv
ad groups jdoe --output json
```

Läuft die Ausgabe nicht in einem Terminal (z. B. Pipe oder Umleitung), wird automatisch auf eine einfache Tabellenausgabe umgeschaltet statt des interaktiven Filters.

### Abmelden / Status

```sh
ad whoami              # zeigt gespeicherte Verbindungsdaten (nie das Passwort)
ad whoami --verify     # prüft per Live-Bind, ob die gespeicherten Zugangsdaten noch funktionieren
ad logout              # entfernt nur das gespeicherte Passwort
ad logout --purge      # entfernt zusätzlich die Verbindungsdaten
```

## Speicherort & Schutz der Zugangsdaten

Verbindungsdaten und Zugangsdaten werden pro Benutzer getrennt in zwei Dateien abgelegt:

| Plattform | Verzeichnis | Dateien |
|---|---|---|
| Windows | `%APPDATA%\ad-cli\` | `config.toml`, `credentials.bin` |
| Linux | `~/.config/ad-cli/` (bzw. `$XDG_CONFIG_HOME`) | `config.toml`, `credentials.bin` |

- `config.toml` enthält Host, Port, Base DN und die Bind-Identität im Klartext (keine Geheimnisse).
- `credentials.bin` enthält ausschließlich das verschlüsselte Passwort:
  - **Windows**: Verschlüsselung über die Windows-DPAPI (`CryptProtectData`), gebunden an den aktuellen Windows-Benutzer – entschlüsselbar nur mit demselben Benutzerkonto auf demselben Rechner.
  - **Linux**: Es wird keine laufende Desktop-Keyring-Instanz vorausgesetzt (das Tool soll auch headless über SSH funktionieren). Der Schlüssel wird aus `/etc/machine-id` und der UID abgeleitet und das Passwort mit AES-256-GCM verschlüsselt; die Datei erhält die Berechtigung `600`. **Das schützt vor zufälligem Kopieren der Datei oder Zugriff durch andere, nicht-root-Benutzer auf demselben Rechner – nicht vor root-Zugriff.**

Ein Passwort wird nie als Kommandozeilenargument akzeptiert (würde in der Shell-History und im Prozess-Listing sichtbar bleiben); nur über die maskierte interaktive Eingabe oder `--password-stdin`.

## Bekannte Einschränkungen

- Es gibt noch keine automatisierten Tests; die LDAP-Logik (Ranged-`memberOf`-Abruf, rekursive Gruppenauflösung, Attribut-Parsing) wurde bisher nicht gegen ein echtes Active Directory verifiziert.
- `lastLogon` wird nicht zwischen Domain Controllern repliziert und spiegelt daher nur den antwortenden DC wider; `lastLogonTimestamp` ist repliziert, kann aber bis zu ~14 Tage nachhinken.

## Versionierung & Release

Das Projekt folgt [Semantic Versioning](https://semver.org/lang/de/). Die maßgebliche Version steht in `Cargo.toml` (`ad --version` zeigt sie an), Git-Tags tragen ein `v`-Präfix:

- **Beta-Phase:** `0.1.0-beta.1`, `0.1.0-beta.2`, … – Tags mit Bindestrich (`v0.1.0-beta.1`) werden auf GitHub automatisch als *Pre-release* markiert.
- **`0.1.0`**: erste stabile Version, sobald das Tool gegen ein echtes AD verifiziert ist. In der 0.x-Reihe dürfen Minor-Versionen noch inkompatible Änderungen enthalten.
- **`1.0.0`**: sobald CLI-Optionen und die JSON-/CSV-Ausgabeformate als stabile Schnittstelle gelten.

Ein Release erstellen:

```sh
# 1. Version in Cargo.toml anpassen, z. B. version = "0.1.0-beta.2"
cargo check                       # aktualisiert Cargo.lock
# 2. CHANGELOG.md: Abschnitt [Unreleased] in die neue Version überführen
git commit -am "chore: release v0.1.0-beta.2"
git tag -a v0.1.0-beta.2 -m "v0.1.0-beta.2"
git push origin main --follow-tags
```

Der Tag-Push startet den Workflow `.github/workflows/release.yml`. Er prüft, dass Tag und `Cargo.toml`-Version übereinstimmen, baut die Windows- und Linux-Binaries und legt ein GitHub-Release mit den Binaries, Archiven und `SHA256SUMS.txt` an.

## Lizenz

[MIT](LICENSE)

Weiterführende technische Details zur Architektur finden sich in [CLAUDE.md](CLAUDE.md).
