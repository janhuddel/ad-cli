# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier dokumentiert.

Das Format basiert auf [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), die Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

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

[Unreleased]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.2...HEAD
[0.1.0-beta.2]: https://github.com/janhuddel/ad-cli/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/janhuddel/ad-cli/releases/tag/v0.1.0-beta.1
