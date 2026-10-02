# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier dokumentiert.

Das Format basiert auf [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), die Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

## [2.0.0] - 2026-10-02

Stabile Version mit dem neuen Tool `idm`. Funktional identisch mit `2.0.0-beta.3`; die Änderungen gegenüber `1.2.0` stehen in den Abschnitten der Betas `2.0.0-beta.1` bis `2.0.0-beta.3`. Für `ad` ändern sich Befehle, Ausgaben und gespeicherte Anmeldungen nicht. Ab jetzt gelten auch die CLI-Optionen von `idm` sowie seine JSON- und CSV-Ausgabeformate als stabile Schnittstelle.

## [2.0.0-beta.3] - 2026-10-02

### Geändert
- `idm user` zeigt statt aller Rohattribute eine kompakte Übersicht wie `ad user`: Login- und Mitarbeiterstatus, Mail, Organisationseinheit, Kostenstelle, Vorgesetzte(r) und letzte(r) Bearbeiter(in) mit Namen, Ein- und Austrittsdatum (ein naher oder vergangener Austritt wird farbig hervorgehoben, `31.12.9999` erscheint als „no end date“) sowie die Zahl der Rechte. `idm user --all` hängt wie bisher alle Attribute an. Die JSON-Ausgabe ist unverändert.

## [2.0.0-beta.2] - 2026-10-02

### Behoben
- Windows: TLS läuft jetzt über den in Windows eingebauten TLS-Stack (Schannel) statt über rustls. Damit klappt die Verbindung auch zu Servern, die nur ältere Cipher-Suites anbieten – vorher brach der Login mit `received fatal alert: HandshakeFailure` ab (auch mit `--insecure-skip-verify`).
- Antwortet ein Server nicht, hängen `ad` und `idm` nicht mehr unbegrenzt: Verbindungsaufbau inkl. TLS-Handshake bricht nach 10 s ab, Bind und Login-Prüfung nach 30 s.

## [2.0.0-beta.1] - 2026-10-02

Ab dieser Version enthält das Repository zwei Tools, die gemeinsam versioniert und released werden: `ad` und das neue `idm`. Für `ad` ändert sich nichts – Befehle, Ausgaben und gespeicherte Anmeldungen bleiben unverändert gültig.

### Hinzugefügt
- Neues Tool `idm` für den Identity Manager (NetIQ/OpenText IdM auf eDirectory):
  - `idm user <id|name>` zeigt alle Attribute eines Benutzers, `idm rights <id|name>` seine Rechte (Attribut `rightvalue`) – interaktiv als Fuzzy-Filter oder per `--output table|csv|json`. Benutzer werden exakt über `cn`/`uid`/`mail`/`workforceID` oder per Namenssuche gefunden.
  - Mehrere Stages: `idm login --stage <name>` richtet je Stage eine eigene Verbindung ein, `--stage` bzw. `IDM_STAGE` wählt sie aus, sonst gilt die Default-Stage (`idm stage`, `idm stage default <name>`).
  - Anonymer Zugriff ohne Benutzer/Passwort (`idm login --anonymous`).
- Release enthält `ad-<version>-x86_64-windows.exe` und `idm-<version>-x86_64-windows.exe`; das ZIP heißt jetzt `ad-cli-<version>-x86_64-windows.zip` und enthält beide Tools.

## [1.2.0] - 2026-10-02

### Geändert
- `ad members`: Die Gruppensuche findet Teilbegriffe statt nur Namensanfänge – jedes eingegebene Wort muss irgendwo in CN, sAMAccountName oder Beschreibung vorkommen (z. B. `ad members sap admin` → „GRP-SAP-Admins“). Die Treffer erscheinen ohne 50er-Grenze in der Fuzzy-Auswahl.

### Hinzugefügt
- `ad members` ohne Argument bietet im Terminal alle Gruppen in der Fuzzy-Auswahl an.

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

[Unreleased]: https://github.com/janhuddel/ad-cli/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/janhuddel/ad-cli/compare/v2.0.0-beta.3...v2.0.0
[2.0.0-beta.3]: https://github.com/janhuddel/ad-cli/compare/v2.0.0-beta.2...v2.0.0-beta.3
[2.0.0-beta.2]: https://github.com/janhuddel/ad-cli/compare/v2.0.0-beta.1...v2.0.0-beta.2
[2.0.0-beta.1]: https://github.com/janhuddel/ad-cli/compare/v1.2.0...v2.0.0-beta.1
[1.2.0]: https://github.com/janhuddel/ad-cli/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/janhuddel/ad-cli/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.5...v1.0.0
[0.1.0-beta.5]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.4...v0.1.0-beta.5
[0.1.0-beta.4]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.3...v0.1.0-beta.4
[0.1.0-beta.3]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.2...v0.1.0-beta.3
[0.1.0-beta.2]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/janhuddel/ad-cli/releases/tag/v0.1.0-beta.1
