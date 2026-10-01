# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier dokumentiert.

Das Format basiert auf [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), die Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

## [1.1.0] - 2026-10-01

### Hinzugefügt
- `ad members <gruppe>` listet die Mitglieder einer Gruppe (Name, sAMAccountName, Typ, Aktiv-Status, DN), optional inkl. verschachtelter Mitglieder (`--recursive`). Die Gruppe wird per sAMAccountName, CN oder DN angegeben oder per Namenssuche gefunden. Ausgabe wie bei `ad groups` als interaktiver Fuzzy-Filter oder per `--output table|csv|json`. Mitgliedschaften über die primäre Gruppe (z. B. „Domain Users“) werden nicht erfasst.

## [1.0.0] - 2026-10-01

Erste stabile Version. Funktional identisch mit `0.1.0-beta.5`; CLI-Optionen sowie die JSON- und CSV-Ausgabeformate gelten ab jetzt als stabile Schnittstelle (Semantic Versioning).

## [0.1.0-beta.5] - 2026-10-01

### Geändert
- Die interaktive Fuzzy-Auswahl (`ad groups`, Benutzerauswahl bei mehreren Treffern) flackert beim Navigieren nicht mehr und zeigt höchstens 15 Einträge gleichzeitig.

### Hinzugefügt
- In der Fuzzy-Auswahl blättern Bild↑/Bild↓ seitenweise, Pos1/Ende springen an Anfang/Ende der Liste; eine Fußzeile zeigt die aktuelle Position (z. B. `12/367`).

## [0.1.0-beta.4] - 2026-10-01

### Hinzugefügt
- `ad user` gibt das Büro (`physicalDeliveryOfficeName`) aus, sofern gesetzt.
- `ad user` und `ad groups` akzeptieren neben sAMAccountName/UPN auch einen Namen (z. B. Nachname oder „Vorname Nachname“). Gibt es keinen exakten Treffer, wird per Ambiguous Name Resolution gesucht: ein Treffer wird direkt verwendet, bei mehreren erscheint im Terminal eine Fuzzy-Auswahl. Ohne Terminal bricht der Befehl mit einer Kandidatenliste ab.

## [0.1.0-beta.3] - 2026-10-01

### Geändert
- `ad user` zeigt standardmäßig eine kompakte, farbige Übersicht (`--output compact`); leere Felder werden ausgeblendet, Zeitstempel in lokaler Zeit mit relativer Angabe. Die bisherige Tabelle ist weiterhin über `--output table` verfügbar.

## [0.1.0-beta.2] - 2026-10-01

### Geändert
- Releases enthalten jetzt zusätzlich die nackten Binaries (`.exe` bzw. Linux-Binary) zum direkten Download.
- Prüfsummen werden in einer gemeinsamen `SHA256SUMS.txt` statt in einzelnen `.sha256`-Dateien veröffentlicht.

## [0.1.0-beta.1] - 2026-10-01

Erste öffentliche Beta-Version. **Noch nicht gegen ein echtes Active Directory getestet.**

### Hinzugefügt
- `ad login` / `ad logout` / `ad whoami`: Anmeldung per LDAPS-Simple-Bind, lokale Speicherung der Verbindungsdaten und des verschlüsselten Passworts (Windows: DPAPI, Linux: AES-256-GCM mit aus machine-id und UID abgeleitetem Schlüssel).
- `ad user <id>`: Benutzerdetails als Tabelle oder JSON.
- `ad groups <id>`: Gruppenmitgliedschaften (direkt oder mit `--recursive` transitiv) mit interaktivem Fuzzy-Filter im Terminal oder als Tabelle/CSV/JSON.
- Release-Binaries für Windows (x86_64, statisches CRT) und Linux (x86_64, statisch gelinkt mit musl).

[Unreleased]: https://github.com/janhuddel/ad-cli/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/janhuddel/ad-cli/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.5...v1.0.0
[0.1.0-beta.5]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.4...v0.1.0-beta.5
[0.1.0-beta.4]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.3...v0.1.0-beta.4
[0.1.0-beta.3]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.2...v0.1.0-beta.3
[0.1.0-beta.2]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/janhuddel/ad-cli/releases/tag/v0.1.0-beta.1
