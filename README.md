# ad-cli

[![CI](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/janhuddel/ad-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/janhuddel/ad-cli)](https://github.com/janhuddel/ad-cli/releases)

Zwei Kommandozeilen-Tools zum Auslesen von Verzeichnisdaten über LDAPS, primär für Windows gebaut, lauffähig auch unter Linux:

- **`ad`** – Active Directory: Benutzerdetails und Gruppenmitgliedschaften (inkl. verschachtelter/rekursiver Mitgliedschaft).
- **`idm`** – Identity Manager (NetIQ/OpenText IdM auf eDirectory): Benutzerdetails und die zugewiesenen fachlichen Rechte, für mehrere Stages (Umgebungen).

## Funktionen

- **`ad login`** – einmalige Anmeldung am AD; Verbindungsdaten und Zugangsdaten werden danach lokal im Home-Verzeichnis gespeichert, sodass spätere Aufrufe keine erneute Eingabe benötigen.
- **`ad user <id>`** – zeigt Details eines Benutzers (Name, Mail, Titel, Abteilung, Account-Status, letzter Login, Ablaufdatum, …) als kompakte Übersicht, alternativ als ausführliche Tabelle oder JSON.
- **`ad groups <id>`** – listet die Gruppenmitgliedschaften eines Benutzers. Bei vielen (100+) Gruppen interaktiv durchsuchbar (Fuzzy-Filter), alternativ als CSV/JSON für die Weiterverarbeitung in Skripten. Optional rekursiv (`--recursive`), um auch verschachtelte Mitgliedschaften aufzulösen.
- **`ad members [gruppe]`** – listet die Mitglieder einer Gruppe (Benutzer, Gruppen, Computer, Kontakte), ebenfalls interaktiv durchsuchbar oder als Tabelle/CSV/JSON, optional rekursiv.
- **`ad whoami`** / **`ad logout`** – aktuelle Verbindung anzeigen bzw. gespeicherte Zugangsdaten entfernen.
- **`idm user <id>`** / **`idm rights <id>`** – alle Attribute eines IdM-Benutzers bzw. seine Rechte (Attribut `rightvalue`), wahlweise für jede eingerichtete Stage (`--stage`). Details unter [IdM-Abfragen](#idm-abfragen-idm).

## Download

Fertige Binaries gibt es auf der [Releases-Seite](https://github.com/janhuddel/ad-cli/releases) unter *Assets*. Die Binaries sind statisch gelinkt und brauchen keine weiteren Abhängigkeiten (unter Windows keine Visual-C++-Runtime). Beide Tools erscheinen gemeinsam mit derselben Version:

| Plattform | Direkt-Download | Archiv mit beiden Tools (inkl. README, LICENSE, CHANGELOG) |
|---|---|---|
| Windows (x86_64) | `ad-<version>-x86_64-windows.exe`, `idm-<version>-x86_64-windows.exe` | `ad-cli-<version>-x86_64-windows.zip` |

Linux-Binaries werden derzeit nicht automatisch gebaut; unter Linux kann das Tool selbst kompiliert werden (siehe [Statisches Linux-Binary](#statisches-linux-binary)).

Die Dateien können nach dem Download beliebig umbenannt werden, z. B. in `ad.exe` und `idm.exe`.

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

Die fertigen Binaries liegen danach unter `target/release/ad` und `target/release/idm` (bzw. `.exe` unter Windows). Einzeln bauen: `cargo build --release -p ad-cli` bzw. `-p idm-cli`.

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

## IdM-Abfragen (`idm`)

`idm` funktioniert wie `ad`, fragt aber den Identity Manager ab. Den IdM gibt es in mehreren Stages (z. B. Entwicklung, Test, Produktion), jede mit eigenem LDAP-Server. Jede Stage wird deshalb einmal eingerichtet und bekommt einen frei wählbaren Namen (Buchstaben, Ziffern, `-`, `_`).

### Stages einrichten

```sh
idm login --stage prod --host <idm-host> --base-dn <base-dn-der-benutzer> --anonymous
idm login --stage test --host <idm-test-host> --base-dn <base-dn-der-benutzer> --anonymous
```

Mit `--anonymous` (oder einer leer gelassenen Bind-DN im Prompt) wird ohne Benutzer und Passwort zugegriffen; der Login prüft dann, ob die Base DN lesbar ist, und speichert erst danach. Für einen Zugriff mit Kennung stattdessen `--bind-identity <bind-dn>` angeben – das Passwort wird wie bei `ad` abgefragt und verschlüsselt gespeichert.

Die zuerst eingerichtete Stage wird zur **Default-Stage**. Welche Stage ein Befehl verwendet:

1. `--stage <name>` bzw. die Umgebungsvariable `IDM_STAGE`,
2. sonst die Default-Stage,
3. sonst die einzige eingerichtete Stage.

```sh
idm stage                  # eingerichtete Stages auflisten (* = Default)
idm stage default test     # Default-Stage ändern
idm --stage test whoami    # Verbindungsdaten einer Stage (--verify prüft die Verbindung)
idm --stage test logout --purge   # Stage komplett entfernen
```

### Benutzer und Rechte abfragen

```sh
idm user U123456                   # alle Attribute des Benutzers (Default-Stage)
idm user max müller                # Namenssuche, mehrere Treffer öffnen eine Auswahl
idm rights U123456                 # Rechte, interaktiv durchsuchbar
idm --stage test rights U123456    # Rechte in einer anderen Stage
idm rights U123456 --output csv > rechte.csv
idm rights U123456 --output json
```

Der Benutzer wird zuerst exakt über `cn`, `uid`, `mail` oder `workforceID` gesucht, sonst per Namenssuche (jedes Wort muss in `cn`, Vor-/Nachname, `fullName` oder `mail` vorkommen). `idm user` zeigt sämtliche Attribute, die der Server liefert; die Rechte erscheinen dort nur als Anzahl. `idm rights` listet die Werte des Attributs `rightvalue` sortiert und ohne Duplikate – im Terminal als Fuzzy-Filter (Enter zeigt den vollständigen Wert), sonst als Tabelle/CSV/JSON (`[{"value": "…"}]`).

## Speicherort & Schutz der Zugangsdaten

Verbindungsdaten und Zugangsdaten werden pro Benutzer getrennt in zwei Dateien abgelegt:

| Plattform | Verzeichnis | Dateien |
|---|---|---|
| Windows | `%APPDATA%\ad-cli\` | `config.toml`, `credentials.bin` |
| Linux | `~/.config/ad-cli/` (bzw. `$XDG_CONFIG_HOME`) | `config.toml`, `credentials.bin` |

`idm` legt seine Daten getrennt davon in `%APPDATA%\idm-cli\` bzw. `~/.config/idm-cli/` ab: pro Stage ein Unterverzeichnis `stages/<name>/` mit denselben beiden Dateien (`credentials.bin` nur bei Zugriff mit Kennung), dazu `settings.toml` mit der Default-Stage.

- `config.toml` enthält Host, Port, Base DN und die Bind-Identität im Klartext (keine Geheimnisse).
- `credentials.bin` enthält ausschließlich das verschlüsselte Passwort:
  - **Windows**: Verschlüsselung über die Windows-DPAPI (`CryptProtectData`), gebunden an den aktuellen Windows-Benutzer – entschlüsselbar nur mit demselben Benutzerkonto auf demselben Rechner.
  - **Linux**: Es wird keine laufende Desktop-Keyring-Instanz vorausgesetzt (das Tool soll auch headless über SSH funktionieren). Der Schlüssel wird aus `/etc/machine-id` und der UID abgeleitet und das Passwort mit AES-256-GCM verschlüsselt; die Datei erhält die Berechtigung `600`. **Das schützt vor zufälligem Kopieren der Datei oder Zugriff durch andere, nicht-root-Benutzer auf demselben Rechner – nicht vor root-Zugriff.**

Ein Passwort wird nie als Kommandozeilenargument akzeptiert (würde in der Shell-History und im Prozess-Listing sichtbar bleiben); nur über die maskierte interaktive Eingabe oder `--password-stdin`.

## Bekannte Einschränkungen

- Automatisierte Tests decken nur Ausgabeformatierung und Auswahl-Logik ab; die LDAP-Logik (Ranged-`memberOf`-Abruf, rekursive Gruppenauflösung, Attribut-Parsing) wird nicht automatisiert gegen ein AD getestet. Der Ranged-Abruf greift erst ab mehr als 1500 direkten Gruppen und ist in der Praxis kaum erprobt.
- `ad members` listet keine Mitgliedschaften über die *primäre Gruppe* eines Kontos (`primaryGroupID`, typischerweise „Domain Users“ / „Domänen-Benutzer“): AD speichert diese weder im `member`- noch im `memberOf`-Attribut.
- `idm` ist neu und wurde ohne Zugriff auf einen echten IdM-Server entwickelt: Welche Attribute einen Benutzer identifizieren und wo die Rechte stehen, beruht auf Annahmen (siehe `crates/idm-cli/src/directory.rs`). Rechte lassen sich bisher nur pro Benutzer anzeigen, nicht umgekehrt (wer hat ein bestimmtes Recht), und nicht im Vergleich über Stages.
- `lastLogon` wird nicht zwischen Domain Controllern repliziert und spiegelt daher nur den antwortenden DC wider; `lastLogonTimestamp` ist repliziert, kann aber bis zu ~14 Tage nachhinken.

## Versionierung & Release

Das Projekt folgt [Semantic Versioning](https://semver.org/lang/de/). Die maßgebliche Version steht im Workspace-`Cargo.toml` unter `[workspace.package]` und gilt für beide Tools (`ad --version`, `idm --version`); Git-Tags tragen ein `v`-Präfix:

- **`1.0.0`** ist die erste stabile Version (davor Betas `0.1.0-beta.N`). Ab hier gelten CLI-Optionen und die JSON-/CSV-Ausgabeformate als stabile Schnittstelle: inkompatible Änderungen daran erfordern eine neue Major-Version, neue Funktionen eine Minor-, Fehlerbehebungen eine Patch-Version.
- **`2.0.0`** bringt `idm` hinzu; beide Tools werden seitdem gemeinsam versioniert und released, und die Stabilitätszusage gilt auch für `idm`. Für `ad` ändert sich nichts.
- Vorabversionen tragen ein Suffix (z. B. `1.1.0-beta.1`); Tags mit Bindestrich werden auf GitHub automatisch als *Pre-release* markiert.

Ein Release erstellen:

```sh
# 1. Version im Workspace-Cargo.toml ([workspace.package]) anpassen, z. B. version = "2.0.1"
cargo check                       # aktualisiert Cargo.lock
# 2. CHANGELOG.md: Abschnitt [Unreleased] in die neue Version überführen
git commit -am "chore: release v2.0.1"
git tag -a v2.0.1 -m "v2.0.1"
git push origin main --follow-tags
```

Der Tag-Push startet den Workflow `.github/workflows/release.yml`. Er prüft, dass Tag und `Cargo.toml`-Version übereinstimmen, baut die Windows-Binaries beider Tools und legt ein GitHub-Release mit den Binaries, Archiven und `SHA256SUMS.txt` an.

## Lizenz

[MIT](LICENSE)

Weiterführende technische Details zur Architektur finden sich in [CLAUDE.md](CLAUDE.md).
