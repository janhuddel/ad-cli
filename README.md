# ad-cli

[![CI](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/janhuddel/ad-cli?include_prereleases)](https://github.com/janhuddel/ad-cli/releases)

> **Beta:** Das Tool ist noch nicht gegen ein echtes Active Directory getestet. Bitte zunächst in einer Test-Umgebung ausprobieren.

Kommandozeilen-Tool zum Auslesen von Active-Directory-Daten über LDAPS: Benutzerdetails und Gruppenmitgliedschaften (inkl. verschachtelter/rekursiver Mitgliedschaft). Primär für Windows gebaut, läuft ebenso unter Linux.

## Funktionen

- **`ad login`** – einmalige Anmeldung am AD; Verbindungsdaten und Zugangsdaten werden danach lokal im Home-Verzeichnis gespeichert, sodass spätere Aufrufe keine erneute Eingabe benötigen.
- **`ad user <id>`** – zeigt Details eines Benutzers (Name, Mail, Titel, Abteilung, Account-Status, letzter Login, Ablaufdatum, …) als kompakte Übersicht, alternativ als ausführliche Tabelle oder JSON.
- **`ad groups <id>`** – listet die Gruppenmitgliedschaften eines Benutzers. Bei vielen (100+) Gruppen interaktiv durchsuchbar (Fuzzy-Filter), alternativ als CSV/JSON für die Weiterverarbeitung in Skripten. Optional rekursiv (`--recursive`), um auch verschachtelte Mitgliedschaften aufzulösen.
- **`ad members [gruppe]`** – listet die Mitglieder einer Gruppe (Benutzer, Gruppen, Computer, Kontakte), ebenfalls interaktiv durchsuchbar oder als Tabelle/CSV/JSON, optional rekursiv.
- **`ad whoami`** / **`ad logout`** – aktuelle Verbindung anzeigen bzw. gespeicherte Zugangsdaten entfernen.

## Download

Fertige Binaries gibt es auf der [Releases-Seite](https://github.com/janhuddel/ad-cli/releases) unter *Assets*. Das Binary ist statisch gelinkt und brauchen keine weiteren Abhängigkeiten (unter Windows keine Visual-C++-Runtime):

| Plattform | Direkt-Download | Archiv (inkl. README, LICENSE, CHANGELOG) |
|---|---|---|
| Windows (x86_64) | `ad-<version>-x86_64-windows.exe` | `ad-<version>-x86_64-windows.zip` |

Linux-Binaries werden derzeit nicht automatisch gebaut; unter Linux kann das Tool selbst kompiliert werden (siehe [Statisches Linux-Binary](#statisches-linux-binary)).

Die Datei kann nach dem Download beliebig umbenannt werden, z. B. in `ad.exe`.

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
ad user müller               # Suche per Name (Nachname, Vorname, "Max Mül", Mail-Präfix …)
```

Wird kein Benutzer mit genau dieser Kennung (sAMAccountName/UPN) gefunden, sucht `ad` per *Ambiguous Name Resolution* in Anzeigename, Vor-/Nachname, Kontoname und Mail (Präfix-Suche). Bei genau einem Treffer wird er direkt angezeigt. Bei mehreren erscheint im Terminal eine Fuzzy-Auswahl, ohne Terminal endet der Befehl mit einer Kandidatenliste (Exit-Code 1). Das gilt genauso für `ad groups`.

### Gruppenmitgliedschaften abfragen

```sh
ad groups jdoe                 # interaktiver Fuzzy-Filter im Terminal
ad groups jdoe --recursive     # inkl. verschachtelter Gruppen
ad groups jdoe --output csv > gruppen.csv
ad groups jdoe --output json
```

Läuft die Ausgabe nicht in einem Terminal (z. B. Pipe oder Umleitung), wird automatisch auf eine einfache Tabellenausgabe umgeschaltet statt des interaktiven Filters.

### Mitglieder einer Gruppe abfragen

```sh
ad members                            # alle Gruppen in der Fuzzy-Auswahl, dann Mitglieder
ad members sap admin                  # Suche: findet z. B. „GRP-SAP-Admins“
ad members App-Admins                 # interaktiver Fuzzy-Filter im Terminal
ad members App-Admins --recursive     # inkl. Mitglieder verschachtelter Gruppen
ad members "CN=App-Admins,OU=Gruppen,DC=example,DC=com" --output csv > mitglieder.csv
ad members App-Admins --output json
```

Die Gruppe kann per sAMAccountName, CN oder DN angegeben werden. Ohne exakten Treffer gilt die Eingabe als Suchbegriff: Jedes Wort muss *irgendwo* in CN, sAMAccountName oder Beschreibung der Gruppe vorkommen (Groß-/Kleinschreibung egal). Ein einzelner Treffer wird direkt verwendet, mehrere öffnen im Terminal die Fuzzy-Auswahl, in der weiter gefiltert werden kann. Ohne Argument werden alle Gruppen zur Auswahl angeboten. Ohne Terminal endet der Befehl bei mehreren Treffern mit einer Liste der passenden DNs (Exit-Code 1); gibt es denselben CN in mehreren OUs, hilft die Angabe des DN.

Ausgegeben werden Name, sAMAccountName, Typ (`user`, `group`, `computer`, `contact`, `other`), Aktiv-Status (nur für Benutzer/Computer) und DN. Mit `--recursive` erscheinen verschachtelte Gruppen selbst ebenfalls in der Liste.

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

- Automatisierte Tests decken nur Ausgabeformatierung und Auswahl-Logik ab; die LDAP-Logik (Ranged-`memberOf`-Abruf, rekursive Gruppenauflösung, Attribut-Parsing) wird nicht automatisiert gegen ein AD getestet. Der Ranged-Abruf greift erst ab mehr als 1500 direkten Gruppen und ist in der Praxis kaum erprobt.
- `ad members` listet keine Mitgliedschaften über die *primäre Gruppe* eines Kontos (`primaryGroupID`, typischerweise „Domain Users“ / „Domänen-Benutzer“): AD speichert diese weder im `member`- noch im `memberOf`-Attribut.
- `lastLogon` wird nicht zwischen Domain Controllern repliziert und spiegelt daher nur den antwortenden DC wider; `lastLogonTimestamp` ist repliziert, kann aber bis zu ~14 Tage nachhinken.

## Versionierung & Release

Das Projekt folgt [Semantic Versioning](https://semver.org/lang/de/). Die maßgebliche Version steht in `Cargo.toml` (`ad --version` zeigt sie an), Git-Tags tragen ein `v`-Präfix:

- **`1.0.0`** ist die erste stabile Version (davor Betas `0.1.0-beta.N`). Ab hier gelten CLI-Optionen und die JSON-/CSV-Ausgabeformate als stabile Schnittstelle: inkompatible Änderungen daran erfordern eine neue Major-Version, neue Funktionen eine Minor-, Fehlerbehebungen eine Patch-Version.
- Vorabversionen tragen ein Suffix (z. B. `1.1.0-beta.1`); Tags mit Bindestrich werden auf GitHub automatisch als *Pre-release* markiert.

Ein Release erstellen:

```sh
# 1. Version in Cargo.toml anpassen, z. B. version = "1.0.1"
cargo check                       # aktualisiert Cargo.lock
# 2. CHANGELOG.md: Abschnitt [Unreleased] in die neue Version überführen
git commit -am "chore: release v1.0.1"
git tag -a v1.0.1 -m "v1.0.1"
git push origin main --follow-tags
```

Der Tag-Push startet den Workflow `.github/workflows/release.yml`. Er prüft, dass Tag und `Cargo.toml`-Version übereinstimmen, baut das Windows-Binary und legt ein GitHub-Release mit den Binaries, Archiven und `SHA256SUMS.txt` an.

## Lizenz

[MIT](LICENSE)

Weiterführende technische Details zur Architektur finden sich in [CLAUDE.md](CLAUDE.md).
