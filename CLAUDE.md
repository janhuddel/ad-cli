# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

This repo ships two Rust CLIs, `ad` and `idm`, released together under one version. `ad` (crate `ad-cli`) queries Active Directory over LDAPS: user detail lookup and group-membership listing (including nested/transitive membership), built primarily for Windows with Linux as a secondary target. The design rationale behind crate choices, LDAP mechanics and credential-storage tradeoffs is summarized below — respect those decisions unless there's a concrete reason to revisit them.

### Workspace layout

The repo is a Cargo workspace (virtual root `Cargo.toml`, shared `[workspace.dependencies]` and release profile):
- `crates/ad-cli/` — the `ad` binary: CLI, commands, and everything AD-specific (filters, ANR, `userAccountControl`, FILETIME, ranged `memberOf`, IN_CHAIN, output views).
- `crates/idm-cli/` — the `idm` binary: queries a NetIQ/OpenText IdM on eDirectory. Unlike the single AD, the IdM exists once per stage (environment), each with its own server: `src/stages.rs` keeps one connection profile per stage in `idm-cli/stages/<name>/` (via `App::stage`), remembers the default in `idm-cli/settings.toml`, and picks the stage as `--stage`/`IDM_STAGE` → default → the only one. The first stage set up becomes the default; `idm stage [list|default <name>]` manages them; `logout --purge` removes a stage. `idm` has no `--config` override (ambiguous with stages). `idm user` shows a compact card of selected attributes (PNW/DirXML schema, manager and last modifier resolved to names by one extra lookup) and with `--all` every attribute of the entry (the schema isn't pinned down; `--output json` stays the raw entry), `idm rights` lists the multi-valued `rightvalue` attribute (rights are *not* modelled as groups). All schema assumptions — ID attributes, name-search attributes, the rights attribute, the card attributes — live as constants at the top of `src/directory.rs`; they were written without access to the real server, so verify them there first. Attribute names are matched case-insensitively because servers return the schema spelling. The real IdM is accessed anonymously.
- `crates/ldap-cli-core/` — library shared by both tools: `App` context, config/paths, encrypted credentials, LDAPS `connect`/`bind`, `paged_search`, `escape_filter_value`, `first`, the single `Error` type, the `login`/`logout`/`whoami` flows (`account.rs`) and `fuzzy_picker`. Keep directory-specific logic out of it: eDirectory has no ANR, no IN_CHAIN and no ranged attributes.

Anonymous access: `Config::bind_identity` is `Option` (`None` = anonymous, omitted from `config.toml`, so `ad`'s files are unchanged — pinned by a test). Then `load_session` loads no password, `ldap::open` skips the bind, and login/`whoami --verify` prove the connection with a base-scope read of the base DN (`check_base_dn`) instead. Only tools that set `LoginPrompts::allow_anonymous` offer it; `ad` doesn't.

Everything tool-specific in core is parameterized by `&App { bin_name, dir_name, stage }` (`ad`'s is `const APP` in `crates/ad-cli/src/main.rs`): the config directory (`stage` adds a `stages/<name>` subdirectory), the login hint in messages (`App::login_command`, e.g. "Run 'ad login' first.") and the Linux credential key (from `dir_name` only, so all stages share it). `dir_name` must never change for an existing tool, see below.

## Commands

```sh
cargo build                                  # native debug build -> target/debug/ad
cargo build --release                        # native release build
cargo check                                  # fast type-check, no binary
cargo clippy --all-targets                   # lint; keep this clean before committing
cargo test                                   # unit tests, all crates
```

Cross-compiling for Windows from macOS/Linux requires the `x86_64-pc-windows-gnu` target and a mingw-w64 toolchain (`brew install mingw-w64` on macOS; `ring`'s build script needs `x86_64-w64-mingw32-gcc` on PATH):

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu
```

For an actual Windows release build, use the MSVC target with a static CRT (per the plan):
```sh
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release -p ad-cli --target x86_64-pc-windows-msvc
```
For a fully static Linux binary: `cargo build --release --target x86_64-unknown-linux-musl` (works specifically because Unix uses the `tls-rustls-ring` feature avoids a dynamic OpenSSL dependency — see TLS note below).

macOS is **not** a supported target: it compiles via the `cfg(unix)` path, but `ldap-cli-core`'s `credentials/linux.rs` needs `/etc/machine-id`, which macOS lacks, so `ad login` fails there. Supporting it needs a dedicated backend (e.g. Keychain) — don't add a macOS job to the release matrix until that exists.

### CI, versioning and releases

- `.github/workflows/ci.yml` (push/PR to `main`): `cargo fmt --check`, `clippy -D warnings`, build/test on windows only (the Windows job is what type-checks `credentials/windows.rs`; the ubuntu build was disabled to save pipeline time, ubuntu lint still type-checks the `cfg(unix)` code). Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` locally before committing.
- `.github/workflows/release.yml` (push of a `v*` tag): fails unless the tag equals `v` + the workspace version, then builds `-p ad-cli -p idm-cli` for `x86_64-pc-windows-msvc` (static CRT) and publishes a GitHub release with the raw binaries (`ad-<ver>-x86_64-windows.exe`, `idm-<ver>-x86_64-windows.exe`), one `ad-cli-<ver>-x86_64-windows.zip` with both, and `SHA256SUMS.txt`. The `x86_64-unknown-linux-musl` matrix entry is commented out for now (Windows-only use, saves pipeline time); the Linux packaging steps are kept so re-enabling it is just uncommenting the entry. Tags containing `-` become pre-releases.
- SemVer, one shared version in the root `Cargo.toml` `[workspace.package]` (all crates use `version.workspace = true`; `ldap-cli-core` is internal, `publish = false`). The root `CHANGELOG.md` covers both tools. `2.0.0` added `idm`; before that only `ad` existed (up to `1.2.0`). `1.0.0` is the first stable release (preceded by `0.1.0-beta.N`); since then the CLI options and JSON/CSV output formats are a stable interface — breaking them needs a major bump. Pre-releases use a `-beta.N` suffix. Release steps: bump the workspace version → `cargo check` → move `[Unreleased]` in `CHANGELOG.md` → commit → annotated tag `vX.Y.Z` → `git push origin main --follow-tags`.

**Automated tests are minimal**: unit tests for output rendering/picker labels (`output/user_view.rs`, `user_picker.rs`, `group_picker.rs`, `members_view.rs`), filters in `ldap/mod.rs` and `member_kind` in `ldap/groups.rs` (all in `crates/ad-cli`), plus `escape_filter_value`, the Linux credential key-info string and the `config.toml` round trip in `ldap-cli-core`, and filters/labels/views in `idm-cli` (`cargo test`). Beyond that, verification has been: `cargo build`/`cargo clippy` on native + Windows-GNU cross-target, and manual CLI smoke tests of the error paths (not-logged-in, unreachable LDAPS host) — there's no mocked LDAP server or test AD wired up. The tool is in use against a real Active Directory (users with several hundred groups), but the LDAP-facing code has no automated coverage and the ranged-`memberOf` path (>1500 direct groups) has likely never been exercised; treat the LDAP-facing code with extra scrutiny and prefer testing against a lab AD or a disposable Samba AD-DC container when making changes there.

## Architecture

### Command flow

`main.rs` parses args via `clap` (`cli.rs` defines `Cli`/`Commands`) and dispatches to `commands/{login,logout,whoami,user,groups,members}.rs` (the first three are thin wrappers around `ldap_cli_core::account`). Every subcommand except `login` starts by calling `ldap_cli_core::config::load_session(APP, …)`, which loads `Config` (connection settings) and decrypts the stored password into a `Session`; if either the config file or the credential file is missing, this fails fast with `Error::NotLoggedIn` *before any network call* — `ad login` is a hard prerequisite enforced at this single chokepoint, not just a convenience.

Each command then: `ldap::open(&session)` (LDAPS connect + simple bind) → does its query via `ldap::user` or `ldap::groups` → renders via `output::user_view` / `output::groups_view` / `output::members_view`. User identifiers go through `commands::resolve_identifier` and group identifiers through `commands::resolve_group` (users: exact match, else ANR prefix search capped at 50 + `user_picker`; groups: exact sAMAccountName/CN/DN, else an unlimited paged substring search — every word must occur in cn/sAMAccountName/description, see `group_search_filter` — + `group_picker`; no argument offers all groups). Group search deliberately avoids ANR because it only prefix-matches, and group names usually carry prefixes like `GRP-`.

### Config vs. credentials — two separate files, same directory

`App::config_dir` (`ldap-cli-core/src/paths.rs`) resolves a per-user directory (`%APPDATA%\<dir_name>` on Windows, `~/.config/<dir_name>` on Linux — `ad-cli` for `ad`) holding:
- `config.toml` — plaintext: host, port, base DN, and the bind identity *exactly as the user typed it* (UPN, `DOMAIN\user`, or a full bind DN — never reconstructed or parsed, just passed through to `simple_bind`).
- `credentials.bin` — the encrypted password only, written by `credentials::save()`.

`ad login` only calls `.save()`/`credentials::save()` after a live bind has actually succeeded (see `account::login` in core) — never persist connection settings or a password speculatively.

### Platform-specific credential encryption

`ldap-cli-core/src/credentials/mod.rs` defines the file format (1-byte version prefix + payload) and dispatches via `#[cfg(windows)]` / `#[cfg(unix)]` to:
- `credentials/windows.rs` — raw DPAPI (`CryptProtectData`/`CryptUnprotectData` via the `windows` crate), scoped to the current user (no `CRYPTPROTECT_LOCAL_MACHINE`). DPAPI's own output is already authenticated, so there's no extra encryption layer here. Note the `windows` crate 0.58 API quirk: `LocalFree` takes a bare `HLOCAL`, not `Option<HLOCAL>` (newer crate versions changed this signature — don't "fix" it based on docs.rs for a different version without checking the pinned version first).
- `credentials/linux.rs` — no GUI keyring is assumed available (the tool is meant to also work headless over SSH), so the key is derived via HKDF-SHA256 from `/etc/machine-id` + UID, then AES-256-GCM encrypts the password. The HKDF info string is `<dir_name>-credential-key` — for `ad` exactly the pre-workspace constant `ad-cli-credential-key`, pinned by a unit test; changing it silently invalidates every stored Linux login. This is documented in-code as protecting against casual file copy / other non-root users on the box — **not** against root. Don't swap this for the generic `keyring` crate without re-confirming headless/no-Secret-Service support is still required.

Since this split is `cfg`-gated, the Windows path cannot be type-checked on a Mac/Linux dev machine without cross-compiling — use `cargo check --target x86_64-pc-windows-gnu -p ldap-cli-core` after touching `credentials/windows.rs`.

### LDAP layer (`crates/ad-cli/src/ldap/`, generic parts in `ldap-cli-core/src/ldap.rs`)

- core `ldap.rs`: connection/bind setup, `paged_search` and `escape_filter_value()` (RFC 4515) — every user-supplied identifier must go through this before being interpolated into a filter string; `ldap3` does not escape automatically. TLS backend is chosen per platform in `ldap-cli-core/Cargo.toml` (`ldap3`'s TLS features are mutually exclusive, so the workspace dependency carries none): **Schannel** (`tls-native`) on Windows, **rustls** (`tls-rustls-ring`, OS trust store via `rustls-native-certs`) on Unix. Windows switched to Schannel in 2.0.0-beta.2 because the IdM's eDirectory LDAPS aborted the rustls handshake (`received fatal alert: HandshakeFailure` — no shared cipher suite; rustls only does TLS 1.2/1.3 with ECDHE+AEAD), and Schannel negotiates whatever Windows allows; Unix stays on rustls to keep the musl build OpenSSL-free. There is **no** manual TLS config plumbing: `LdapConnSettings` in `ldap3` 0.12 only offers `set_no_tls_verify`/`set_starttls`/`set_conn_timeout`. `connect` sets a 10s connect timeout (covers TCP + TLS handshake — before, a server that accepted but never answered hung the tool forever), `bind`/`check_base_dn` a 30s operation timeout; big searches have none. `bind` maps AD's `data 52e`-style sub-codes to readable text via `explain_bind_failure`; for other servers it just passes the message through.
- `mod.rs`: re-exports `open`/`escape_filter_value` and holds the AD filters (`user_filter`, `anr_filter`, `group_filter`, `group_search_filter`).
- `user.rs`: finds a user by `sAMAccountName` or `userPrincipalName`, parses `userAccountControl` bit flags and AD's two time encodings (Windows FILETIME for `lastLogon`/`accountExpires`, generalized-time string for `whenCreated`) — these conversions have hand-rolled epoch math since no off-the-shelf crate handles AD's formats directly.
- `groups.rs`: the trickiest part.
  - `fetch_direct_group_dns` handles AD's *ranged* attribute retrieval: if a user is in enough groups to exceed the server's `MaxValRange` (default 1500), `memberOf` comes back as `memberOf;range=0-1499` etc. instead of a plain key, requiring follow-up base-scope searches on the user's own DN with incrementing range windows until a `-*` upper bound is returned. This is handled manually because `ldap3`'s `SearchEntry.attrs` keeps the literal returned attribute key rather than merging ranges itself.
  - `fetch_recursive_groups` resolves nested/transitive membership by querying *from the group side* with the `LDAP_MATCHING_RULE_IN_CHAIN` extensible match (OID `1.2.840.113556.1.4.1941`) rather than walking `tokenGroups` SIDs, and pages through results with the Simple Paged Results control (OID `1.2.840.113556.1.4.319`) since a heavily-nested user's recursive group count can exceed AD's default 1000-entry non-paged search limit.
  - `paged_search` (core) is the shared Simple-Paged-Results loop used by `fetch_recursive_groups`, `fetch_members` and `search_groups`.
  - `fetch_members` (`ad members`) queries *from the member side* — `(memberOf=<groupDN>)`, or the in-chain variant for `--recursive` — instead of reading the group's `member` attribute: one paged search returns display attributes directly and avoids `member`'s ranged retrieval for >1500 members. Primary-group membership (`primaryGroupID`, e.g. Domain Users) is in neither attribute and is deliberately not listed.
  - `resolve_group_records` (non-recursive path) batches all direct group DNs into a single `(|(distinguishedName=...)...)` subtree search instead of one round trip per group — this matters once a user is in 100+ groups.

### Output (`crates/ad-cli/src/output/`)

`groups_view::display` auto-detects TTY via `ldap_cli_core::stdout_is_interactive()`: non-interactive stdout (piped/redirected) always falls back to plain table/CSV/JSON regardless of flags, which is what makes scripting against `ad groups ... --output json` reliable. On an interactive TTY with no explicit `--output`, it launches a `fuzzy_picker::pick` loop over the group list — this is the primary UX for the "100+ groups" problem, not an afterthought; don't replace it with a plain dump without reproducing the fuzzy-filter behavior some other way.

`ldap-cli-core/src/fuzzy_picker.rs` (used by `groups_view`, `members_view` and both pickers) is a small hand-rolled `crossterm` picker that replaced `dialoguer::FuzzySelect`: dialoguer clears and redraws line by line (one Win32 call per line on Windows → heavy flicker on long lists), and `console`'s Windows key reader doesn't map PageUp/PageDown at all. The picker queues each frame and writes it in one flush, overwriting in place instead of clearing first. `dialoguer` is still used for the login prompts (`Input`/`Password`, in `account.rs`).
