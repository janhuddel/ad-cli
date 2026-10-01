# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`ad-cli` is a Rust CLI (binary name `ad`) that queries Active Directory over LDAPS: user detail lookup and group-membership listing (including nested/transitive membership), built primarily for Windows with Linux as a secondary target. The design rationale behind crate choices, LDAP mechanics and credential-storage tradeoffs is summarized below — respect those decisions unless there's a concrete reason to revisit them.

## Commands

```sh
cargo build                                  # native debug build -> target/debug/ad
cargo build --release                        # native release build
cargo check                                  # fast type-check, no binary
cargo clippy --all-targets                   # lint; keep this clean before committing
```

Cross-compiling for Windows from macOS/Linux requires the `x86_64-pc-windows-gnu` target and a mingw-w64 toolchain (`brew install mingw-w64` on macOS; `ring`'s build script needs `x86_64-w64-mingw32-gcc` on PATH):

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu
```

For an actual Windows release build, use the MSVC target with a static CRT (per the plan):
```sh
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target x86_64-pc-windows-msvc
```
For a fully static Linux binary: `cargo build --release --target x86_64-unknown-linux-musl` (works specifically because the `tls-rustls-ring` feature avoids a dynamic OpenSSL dependency — see TLS note below).

macOS is **not** a supported target: it compiles via the `cfg(unix)` path, but `credentials/linux.rs` needs `/etc/machine-id`, which macOS lacks, so `ad login` fails there. Supporting it needs a dedicated backend (e.g. Keychain) — don't add a macOS job to the release matrix until that exists.

### CI, versioning and releases

- `.github/workflows/ci.yml` (push/PR to `main`): `cargo fmt --check`, `clippy -D warnings`, build/test on windows only (the Windows job is what type-checks `credentials/windows.rs`; the ubuntu build was disabled to save pipeline time, ubuntu lint still type-checks the `cfg(unix)` code). Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` locally before committing.
- `.github/workflows/release.yml` (push of a `v*` tag): fails unless the tag equals `v` + the `Cargo.toml` version, then builds `x86_64-pc-windows-msvc` (static CRT) and publishes a GitHub release with the raw binary (`ad-<ver>-x86_64-windows.exe`), a `.zip` archive and `SHA256SUMS.txt`. The `x86_64-unknown-linux-musl` matrix entry is commented out for now (Windows-only use, saves pipeline time); the Linux packaging steps are kept so re-enabling it is just uncommenting the entry. Tags containing `-` become pre-releases.
- SemVer, `Cargo.toml` is the source of truth. `1.0.0` is the first stable release (preceded by `0.1.0-beta.N`); since then the CLI options and JSON/CSV output formats are a stable interface — breaking them needs a major bump. Pre-releases use a `-beta.N` suffix. Release steps: bump `Cargo.toml` → `cargo check` → move `[Unreleased]` in `CHANGELOG.md` → commit → annotated tag `vX.Y.Z` → `git push origin main --follow-tags`.

**Automated tests are minimal**: unit tests for output rendering/picker labels (`output/user_view.rs`, `user_picker.rs`, `group_picker.rs`, `members_view.rs`), filter escaping in `ldap/mod.rs` and `member_kind` in `ldap/groups.rs` (`cargo test`). Beyond that, verification has been: `cargo build`/`cargo clippy` on native + Windows-GNU cross-target, and manual CLI smoke tests of the error paths (not-logged-in, unreachable LDAPS host) — there's no mocked LDAP server or test AD wired up. The tool is in use against a real Active Directory (users with several hundred groups), but the LDAP-facing code has no automated coverage and the ranged-`memberOf` path (>1500 direct groups) has likely never been exercised; treat the LDAP-facing code with extra scrutiny and prefer testing against a lab AD or a disposable Samba AD-DC container when making changes there.

## Architecture

### Command flow

`main.rs` parses args via `clap` (`cli.rs` defines `Cli`/`Commands`) and dispatches to `commands/{login,logout,whoami,user,groups,members}.rs`. Every subcommand except `login` starts by calling `config::load_session()`, which loads `Config` (connection settings) and decrypts the stored password into a `Session`; if either the config file or the credential file is missing, this fails fast with `AppError::NotLoggedIn` *before any network call* — `ad login` is a hard prerequisite enforced at this single chokepoint, not just a convenience.

Each command then: `ldap::connect(&session.config)` (opens the LDAPS connection) → `ldap::bind(...)` (simple bind) → does its query via `ldap::user` or `ldap::groups` → renders via `output::user_view` / `output::groups_view` / `output::members_view`. User identifiers go through `commands::resolve_identifier` and group identifiers through `commands::resolve_group` (exact match first, else ANR name search + `user_picker`/`group_picker`).

### Config vs. credentials — two separate files, same directory

`config::paths` resolves a per-user directory (`%APPDATA%\ad-cli` on Windows, `~/.config/ad-cli` on Linux) holding:
- `config.toml` — plaintext: host, port, base DN, and the bind identity *exactly as the user typed it* (UPN, `DOMAIN\user`, or a full bind DN — never reconstructed or parsed, just passed through to `simple_bind`).
- `credentials.bin` — the encrypted password only, written by `credentials::save()`.

`ad login` only calls `.save()`/`credentials::save()` after a live bind has actually succeeded (see `commands/login.rs`) — never persist connection settings or a password speculatively.

### Platform-specific credential encryption

`credentials/mod.rs` defines the file format (1-byte version prefix + payload) and dispatches via `#[cfg(windows)]` / `#[cfg(unix)]` to:
- `credentials/windows.rs` — raw DPAPI (`CryptProtectData`/`CryptUnprotectData` via the `windows` crate), scoped to the current user (no `CRYPTPROTECT_LOCAL_MACHINE`). DPAPI's own output is already authenticated, so there's no extra encryption layer here. Note the `windows` crate 0.58 API quirk: `LocalFree` takes a bare `HLOCAL`, not `Option<HLOCAL>` (newer crate versions changed this signature — don't "fix" it based on docs.rs for a different version without checking the pinned version first).
- `credentials/linux.rs` — no GUI keyring is assumed available (the tool is meant to also work headless over SSH), so the key is derived via HKDF-SHA256 from `/etc/machine-id` + UID, then AES-256-GCM encrypts the password. This is documented in-code as protecting against casual file copy / other non-root users on the box — **not** against root. Don't swap this for the generic `keyring` crate without re-confirming headless/no-Secret-Service support is still required.

Since this split is `cfg`-gated, the Windows path cannot be type-checked on a Mac/Linux dev machine without cross-compiling — use `cargo check --target x86_64-pc-windows-gnu` after touching `credentials/windows.rs`.

### LDAP layer (`src/ldap/`)

- `mod.rs`: connection/bind setup and `escape_filter_value()` (RFC 4515) — every user-supplied identifier must go through this before being interpolated into a filter string; `ldap3` does not escape automatically. `connect()` uses `ldap3`'s `tls-rustls-ring` feature, which internally pulls in `rustls-native-certs` for OS trust-store validation — there is **no** manual `rustls::ClientConfig` plumbing in this codebase (an earlier design draft assumed one was needed; it isn't, because `LdapConnSettings` in `ldap3` 0.12 has no `set_config` hook — only `set_no_tls_verify`/`set_starttls`/`set_conn_timeout`).
- `user.rs`: finds a user by `sAMAccountName` or `userPrincipalName`, parses `userAccountControl` bit flags and AD's two time encodings (Windows FILETIME for `lastLogon`/`accountExpires`, generalized-time string for `whenCreated`) — these conversions have hand-rolled epoch math since no off-the-shelf crate handles AD's formats directly.
- `groups.rs`: the trickiest part.
  - `fetch_direct_group_dns` handles AD's *ranged* attribute retrieval: if a user is in enough groups to exceed the server's `MaxValRange` (default 1500), `memberOf` comes back as `memberOf;range=0-1499` etc. instead of a plain key, requiring follow-up base-scope searches on the user's own DN with incrementing range windows until a `-*` upper bound is returned. This is handled manually because `ldap3`'s `SearchEntry.attrs` keeps the literal returned attribute key rather than merging ranges itself.
  - `fetch_recursive_groups` resolves nested/transitive membership by querying *from the group side* with the `LDAP_MATCHING_RULE_IN_CHAIN` extensible match (OID `1.2.840.113556.1.4.1941`) rather than walking `tokenGroups` SIDs, and pages through results with the Simple Paged Results control (OID `1.2.840.113556.1.4.319`) since a heavily-nested user's recursive group count can exceed AD's default 1000-entry non-paged search limit.
  - `paged_search` is the shared Simple-Paged-Results loop used by both `fetch_recursive_groups` and `fetch_members`.
  - `fetch_members` (`ad members`) queries *from the member side* — `(memberOf=<groupDN>)`, or the in-chain variant for `--recursive` — instead of reading the group's `member` attribute: one paged search returns display attributes directly and avoids `member`'s ranged retrieval for >1500 members. Primary-group membership (`primaryGroupID`, e.g. Domain Users) is in neither attribute and is deliberately not listed.
  - `resolve_group_records` (non-recursive path) batches all direct group DNs into a single `(|(distinguishedName=...)...)` subtree search instead of one round trip per group — this matters once a user is in 100+ groups.

### Output (`src/output/`)

`groups_view::display` auto-detects TTY via `std::io::stdout().is_terminal()`: non-interactive stdout (piped/redirected) always falls back to plain table/CSV/JSON regardless of flags, which is what makes scripting against `ad groups ... --output json` reliable. On an interactive TTY with no explicit `--output`, it launches a `fuzzy_picker::pick` loop over the group list — this is the primary UX for the "100+ groups" problem, not an afterthought; don't replace it with a plain dump without reproducing the fuzzy-filter behavior some other way.

`output/fuzzy_picker.rs` (used by both `groups_view` and `user_picker`) is a small hand-rolled `crossterm` picker that replaced `dialoguer::FuzzySelect`: dialoguer clears and redraws line by line (one Win32 call per line on Windows → heavy flicker on long lists), and `console`'s Windows key reader doesn't map PageUp/PageDown at all. The picker queues each frame and writes it in one flush, overwriting in place instead of clearing first. `dialoguer` is still used for the `ad login` prompts (`Input`/`Password`).
