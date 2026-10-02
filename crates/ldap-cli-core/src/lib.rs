//! Plumbing shared by the `ad` and `idm` CLIs: per-tool config directory,
//! encrypted credential storage, LDAPS connect/bind, paged search, filter
//! escaping, the login/logout/whoami flows and the fuzzy picker. Everything
//! directory-specific (schemas, filters, attribute parsing) stays in the
//! individual tool crates.

pub mod account;
pub mod config;
pub mod credentials;
pub mod error;
pub mod fuzzy_picker;
pub mod ldap;
pub mod paths;

pub use error::{Error, Result};

use std::io::IsTerminal;

/// Identifies the tool using this crate (and, for tools with several
/// environments, the selected stage), so each one gets its own config
/// directory and its messages name the right binary.
#[derive(Debug, Clone)]
pub struct App {
    /// Binary name as typed by the user, e.g. `ad` (used in messages like
    /// "Run 'ad login' first.").
    pub bin_name: &'static str,
    /// Per-user config directory name, e.g. `ad-cli`. Also feeds the Linux
    /// credential key derivation, so it must never change for an existing
    /// tool — stored credentials would no longer decrypt.
    pub dir_name: &'static str,
    /// Selected stage for tools that talk to one server per environment
    /// (`idm`): each stage keeps its own config and credentials in
    /// `<dir_name>/stages/<stage>`. `None` for single-server tools (`ad`).
    pub stage: Option<String>,
}

impl App {
    /// The command that sets up the current (stage's) connection, for
    /// "not logged in" hints.
    pub fn login_command(&self) -> String {
        match &self.stage {
            Some(stage) => format!("{} login --stage {stage}", self.bin_name),
            None => format!("{} login", self.bin_name),
        }
    }
}

pub fn stdout_is_interactive() -> bool {
    std::io::stdout().is_terminal()
}
